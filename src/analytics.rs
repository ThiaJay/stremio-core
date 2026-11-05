use std::{
    collections::VecDeque,
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use derivative::Derivative;
use futures::{future, Future, FutureExt};
use serde::Serialize;

use crate::{
    models::{ctx::Ctx, streaming_server::StreamingServer},
    runtime::{Env, EnvError, TryEnvFuture},
    types::{
        api::{fetch_api, APIRequest, APIResult, SuccessResponse},
        profile::AuthKey,
    },
};

#[cfg(debug_assertions)]
use crate::{runtime::EnvFutureExt, types::True};

#[derive(Clone, PartialEq, Serialize, Debug)]
struct Event {
    #[serde(flatten)]
    data: serde_json::Value,
    #[serde(rename = "eventName")]
    name: String,
    #[serde(rename = "eventTime")]
    time: i64,
    #[serde(rename = "eventNumber")]
    number: u64,
    #[serde(rename = "app")]
    context: serde_json::Value,
}

#[derive(Clone, PartialEq, Debug)]
struct EventsBatch {
    auth_key: AuthKey,
    events: Vec<Event>,
}

#[derive(Default)]
struct State {
    consented: bool,
    consent_generation: u64,
    number: u64,
    queue: VecDeque<EventsBatch>,
    pending: Option<EventsBatch>,
}

impl State {
    fn clear_events(&mut self) {
        self.number = 0;
        self.queue.clear();
        self.pending = None;
    }
    fn set_consent(&mut self, consented: bool) {
        if !consented {
            if self.consented {
                if let Some(next_generation) = self.consent_generation.checked_add(1) {
                    self.consent_generation = next_generation;
                }
            }
            self.consented = false;
            self.clear_events();
            return;
        }
        if self.consented {
            return;
        }
        let Some(next_generation) = self.consent_generation.checked_add(1) else {
            self.consented = false;
            self.clear_events();
            return;
        };
        self.consent_generation = next_generation;
        self.consented = true;
    }
    fn can_send(&self, batch: &EventsBatch, consent_generation: u64) -> bool {
        self.consented
            && self.consent_generation == consent_generation
            && self.pending.as_ref() == Some(batch)
    }
    fn has_consent_generation(&self, consent_generation: u64) -> bool {
        self.consented && self.consent_generation == consent_generation
    }
    fn next_number(&mut self) -> u64 {
        self.number = self.number.wrapping_add(1);
        self.number
    }
    fn pop_batch(&mut self) -> Option<EventsBatch> {
        self.consented.then(|| self.queue.pop_front()).flatten()
    }
    fn push_event(&mut self, event: Event, auth_key: AuthKey) {
        if !self.consented {
            return;
        }
        match self.queue.back_mut() {
            Some(batch) if batch.auth_key == auth_key => {
                batch.events.push(event);
            }
            _ => self.queue.push_back(EventsBatch {
                auth_key,
                events: vec![event],
            }),
        };
    }
    fn revert_pending(&mut self) {
        if let Some(batch) = self.pending.take() {
            self.queue.push_front(batch);
        };
    }
}

#[derive(Derivative)]
#[derivative(Default)]
pub struct Analytics<E: Env> {
    state: Arc<Mutex<State>>,
    env: PhantomData<E>,
}

impl<E: Env + 'static> Analytics<E> {
    /// Enables analytics only after an explicit client opt-in. Revocation is
    /// fail-closed: queued and pending events are discarded without sending.
    pub fn set_consent(&self, consented: bool) {
        self.state
            .lock()
            .expect("analytics state lock failed")
            .set_consent(consented);
    }
    pub fn has_consent(&self) -> bool {
        self.state
            .lock()
            .expect("analytics state lock failed")
            .consented
    }
    pub fn emit(
        &self,
        name: String,
        data: serde_json::Value,
        ctx: &Ctx,
        streaming_server: &StreamingServer,
        path: &str,
    ) {
        let mut state = self.state.lock().expect("analytics state lock failed");
        if !state.consented {
            return;
        }
        let auth_key = match ctx.profile.auth_key() {
            Some(auth_key) => auth_key.to_owned(),
            _ => return,
        };
        let event = Event {
            name,
            data,
            number: state.next_number(),
            time: E::now().timestamp_millis(),
            context: E::analytics_context(ctx, streaming_server, path),
        };
        state.push_event(event, auth_key);
    }
    pub fn send_next_batch(&self) -> impl Future<Output = ()> {
        let state = Arc::clone(&self.state);
        let next = {
            let mut state = state.lock().expect("analytics state lock failed");
            if !state.consented || state.pending.is_some() {
                None
            } else {
                state.pop_batch().map(|batch| {
                    let consent_generation = state.consent_generation;
                    state.pending = Some(batch.to_owned());
                    (batch, consent_generation)
                })
            }
        };
        async move {
            let Some((batch, consent_generation)) = next else {
                return;
            };
            let authorised = state
                .lock()
                .expect("analytics state lock failed")
                .can_send(&batch, consent_generation);
            if !authorised {
                return;
            }
            let result = send_events_batch_to_api::<E>(&batch).await;
            let result = match result {
                Ok(APIResult::Err(error)) if error.code != 1 => Err(()),
                Err(EnvError::Fetch(_)) | Err(EnvError::Serde(_)) => Err(()),
                _ => Ok(()),
            };
            let mut state = state.lock().expect("analytics state lock failed");
            if state.can_send(&batch, consent_generation) {
                match result {
                    Ok(_) => {
                        state.pending = None;
                    }
                    Err(_) => {
                        state.revert_pending();
                    }
                };
            };
        }
    }
    pub fn flush(&self) -> impl Future<Output = ()> {
        let state = Arc::clone(&self.state);
        let flush = {
            let mut state = state.lock().expect("analytics state lock failed");
            if !state.consented {
                state.clear_events();
                None
            } else {
                let consent_generation = state.consent_generation;
                state.pending = None;
                Some((
                    consent_generation,
                    state.queue.drain(..).collect::<Vec<_>>(),
                ))
            }
        };
        async move {
            let Some((consent_generation, batches)) = flush else {
                return;
            };
            let authorised = state
                .lock()
                .expect("analytics state lock failed")
                .has_consent_generation(consent_generation);
            if !authorised {
                return;
            }
            future::join_all(
                batches
                    .iter()
                    .map(|batch| send_events_batch_to_api::<E>(batch)),
            )
            .await;
        }
    }
}

fn send_events_batch_to_api<E: Env>(
    batch: &EventsBatch,
) -> TryEnvFuture<APIResult<SuccessResponse>> {
    #[cfg(debug_assertions)]
    if cfg!(debug_assertions) {
        E::log(format!("send_events_batch_to_api: {:#?}", &batch));
        return future::ok(APIResult::Ok(SuccessResponse { success: True })).boxed_env();
    };

    fetch_api::<E, _, _, _>(&APIRequest::Events {
        auth_key: batch.auth_key.to_owned(),
        events: batch
            .events
            .iter()
            .map(|value| serde_json::to_value(value).unwrap())
            .collect(),
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    fn event(number: u64) -> Event {
        Event {
            data: serde_json::json!({ "test": true }),
            name: "test".to_owned(),
            time: 1,
            number,
            context: serde_json::json!({ "context": "test" }),
        }
    }

    fn batch(number: u64) -> EventsBatch {
        EventsBatch {
            auth_key: AuthKey("test-auth-key".to_owned()),
            events: vec![event(number)],
        }
    }

    #[test]
    fn analytics_consent_is_disabled_by_default() {
        let mut state = State::default();

        state.push_event(event(1), AuthKey("test-auth-key".to_owned()));

        assert!(!state.consented);
        assert!(state.queue.is_empty());
        assert!(state.pop_batch().is_none());
    }

    #[test]
    fn explicit_consent_allows_queueing() {
        let mut state = State::default();
        state.set_consent(true);

        state.push_event(event(1), AuthKey("test-auth-key".to_owned()));

        assert!(state.consented);
        assert_eq!(state.queue.len(), 1);
        assert_eq!(state.pop_batch(), Some(batch(1)));
    }

    #[test]
    fn revoking_consent_discards_queued_and_pending_events() {
        let mut state = State::default();
        state.set_consent(true);
        state.number = 9;
        state.queue.push_back(batch(9));
        state.pending = Some(batch(10));

        state.set_consent(false);

        assert!(!state.consented);
        assert_eq!(state.number, 0);
        assert!(state.queue.is_empty());
        assert!(state.pending.is_none());
    }

    #[test]
    fn reconsent_does_not_restore_discarded_events() {
        let mut state = State::default();
        state.set_consent(true);
        state.queue.push_back(batch(1));
        state.pending = Some(batch(2));
        state.set_consent(false);

        state.set_consent(true);

        assert!(state.consented);
        assert!(state.queue.is_empty());
        assert!(state.pending.is_none());
    }

    #[test]
    fn revocation_invalidates_an_unpolled_batch_after_reconsent() {
        let mut state = State::default();
        state.set_consent(true);
        let consent_generation = state.consent_generation;
        let queued = batch(1);
        state.pending = Some(queued.to_owned());
        assert!(state.can_send(&queued, consent_generation));

        state.set_consent(false);
        state.set_consent(true);
        state.pending = Some(queued.to_owned());

        assert!(!state.can_send(&queued, consent_generation));
    }

    #[test]
    fn revocation_invalidates_an_unpolled_flush_after_reconsent() {
        let mut state = State::default();
        state.set_consent(true);
        let consent_generation = state.consent_generation;
        assert!(state.has_consent_generation(consent_generation));

        state.set_consent(false);
        state.set_consent(true);

        assert!(!state.has_consent_generation(consent_generation));
    }

    #[test]
    fn repeated_positive_consent_does_not_invalidate_current_work() {
        let mut state = State::default();
        state.set_consent(true);
        let consent_generation = state.consent_generation;

        state.set_consent(true);

        assert!(state.has_consent_generation(consent_generation));
    }

    #[test]
    fn consent_generation_overflow_fails_closed() {
        let mut state = State {
            consent_generation: u64::MAX,
            ..State::default()
        };
        state.queue.push_back(batch(1));
        state.pending = Some(batch(2));

        state.set_consent(true);

        assert!(!state.consented);
        assert_eq!(state.number, 0);
        assert!(state.queue.is_empty());
        assert!(state.pending.is_none());
    }
}
