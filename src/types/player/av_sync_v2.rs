//! Session bound supervisory recovery. Native players retain normal clock control.
//! Times are integer milliseconds in the backend and bridge monotonic clock domain.
use serde::{Deserialize, Serialize};

const MAX_SAFE: u64 = 9_007_199_254_740_991;
const FRESH_MS: u64 = 1_500;
const MIN_INTERVAL_MS: u64 = 500;
const PERSIST_MS: u64 = 3_000;
const SETTLE_MS: u64 = 2_000;
const COOLDOWN_MS: u64 = 30_000;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Correction {
    NativeClock,
    Reseek,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    Unknown,
    Monitoring,
    Stable,
    AwaitingAck,
    Recovering,
    Unsupported,
    Exhausted,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub session_id: u64,
    pub epoch: u64,
    pub sample_id: u64,
    pub captured_at_ms: u64,
    /// Native residual clock difference. Not a user audio delay adjustment.
    pub offset_ms: i64,
    pub active: bool,
    pub video_stable: bool,
    pub can_soft_correct: bool,
    pub can_hard_correct: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub session_id: u64,
    pub epoch: u64,
    pub generation: u64,
    pub sample_id: u64,
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
    pub correction: Correction,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Acknowledgement {
    pub session_id: u64,
    pub epoch: u64,
    pub generation: u64,
    /// Receipt of the attempt, never evidence of restored physical synchronisation.
    pub accepted: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Controller {
    pub protocol_version: u8,
    pub session_id: u64,
    pub status: Status,
    pub offset_ms: Option<i64>,
    pub pending: Option<Request>,
    #[serde(skip)]
    open: bool,
    #[serde(skip)]
    epoch: u64,
    #[serde(skip)]
    sample_id: u64,
    #[serde(skip)]
    last_capture: Option<u64>,
    #[serde(skip)]
    last_now: Option<u64>,
    #[serde(skip)]
    bad_since: Option<u64>,
    #[serde(skip)]
    stable_since: Option<u64>,
    #[serde(skip)]
    direction: i64,
    #[serde(skip)]
    next_allowed: u64,
    #[serde(skip)]
    soft_used: bool,
    #[serde(skip)]
    hard_used: bool,
    #[serde(skip)]
    generation: u64,
    #[serde(skip)]
    interrupted_at_sample: Option<u64>,
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            protocol_version: 2,
            session_id: 0,
            status: Status::Unknown,
            offset_ms: None,
            pending: None,
            open: false,
            epoch: 0,
            sample_id: 0,
            last_capture: None,
            last_now: None,
            bad_since: None,
            stable_since: None,
            direction: 0,
            next_allowed: 0,
            soft_used: false,
            hard_used: false,
            generation: 0,
            interrupted_at_sample: None,
        }
    }
}

impl Controller {
    pub fn begin(&mut self) {
        let next = self.session_id.saturating_add(1);
        *self = Self::default();
        self.session_id = next;
        self.open = next < MAX_SAFE;
        if !self.open {
            self.status = Status::Exhausted;
        }
    }

    pub fn close(&mut self) {
        self.begin();
        self.open = false;
    }

    pub fn interrupt(&mut self) {
        self.pending = None;
        self.offset_ms = None;
        self.bad_since = None;
        self.stable_since = None;
        self.interrupted_at_sample = Some(self.epoch);
        if self.status != Status::Exhausted {
            self.status = Status::Unknown;
        }
    }

    fn accept_time(&mut self, now: u64) -> bool {
        if now > MAX_SAFE || self.last_now.is_some_and(|last| now < last) {
            return false;
        }
        self.last_now = Some(now);
        true
    }

    pub fn tick(&mut self, session: u64, now: u64) {
        if !self.open || session != self.session_id || !self.accept_time(now) {
            return;
        }
        if self.pending.as_ref().is_some_and(|p| now > p.expires_at_ms) {
            self.pending = None;
            self.status = Status::Exhausted;
            return;
        }
        if self
            .last_capture
            .is_some_and(|last| now.saturating_sub(last) > FRESH_MS)
        {
            self.offset_ms = None;
            self.bad_since = None;
            self.stable_since = None;
            if !matches!(self.status, Status::Exhausted | Status::AwaitingAck) {
                self.status = Status::Unknown;
            }
        }
    }

    pub fn acknowledge(&mut self, ack: &Acknowledgement, now: u64) {
        if !self.open || ack.session_id != self.session_id || !self.accept_time(now) {
            return;
        }
        let Some(request) = self.pending.as_ref() else {
            return;
        };
        if ack.epoch != request.epoch || ack.generation != request.generation {
            return;
        }
        let accepted = ack.accepted && now <= request.expires_at_ms;
        self.pending = None;
        self.bad_since = None;
        self.stable_since = None;
        self.status = if accepted {
            Status::Recovering
        } else {
            Status::Exhausted
        };
    }

    pub fn observe(&mut self, sample: &Observation, now: u64, player_active: bool) {
        if !self.open
            || sample.session_id != self.session_id
            || sample.epoch > MAX_SAFE
            || sample.sample_id == 0
            || sample.sample_id > MAX_SAFE
            || sample.sample_id <= self.sample_id
            || sample.epoch < self.epoch
            || sample.captured_at_ms > now
            || now.saturating_sub(sample.captured_at_ms) > FRESH_MS
            || sample
                .offset_ms
                .checked_abs()
                .is_none_or(|offset| offset > 60_000)
            || !self.accept_time(now)
        {
            return;
        }
        self.tick(sample.session_id, now);
        if self.status == Status::Exhausted {
            return;
        }
        if sample.epoch != self.epoch {
            // Recovery itself may advance the native epoch before its receipt arrives.
            // Keep the original request until receipt, expiry or an explicit user interruption.
            self.bad_since = None;
            self.stable_since = None;
            self.last_capture = None;
            self.epoch = sample.epoch;
            if self.pending.is_none() {
                self.status = Status::Unknown;
            }
        }
        if self
            .interrupted_at_sample
            .is_some_and(|epoch| sample.epoch <= epoch)
        {
            return;
        }
        self.interrupted_at_sample = None;
        if self.last_capture.is_some_and(|last| {
            sample.captured_at_ms <= last || sample.captured_at_ms - last < MIN_INTERVAL_MS
        }) {
            return;
        }
        if self
            .last_capture
            .is_some_and(|last| sample.captured_at_ms - last > FRESH_MS)
        {
            self.bad_since = None;
            self.stable_since = None;
        }
        self.last_capture = Some(sample.captured_at_ms);
        self.sample_id = sample.sample_id;
        if self.pending.is_some() {
            self.offset_ms = if player_active && sample.active && sample.video_stable {
                Some(sample.offset_ms)
            } else {
                None
            };
            return;
        }
        if !player_active || !sample.active || !sample.video_stable {
            self.interrupt();
            return;
        }
        self.offset_ms = Some(sample.offset_ms);
        if sample.offset_ms.abs() <= 60 {
            self.bad_since = None;
            let since = *self.stable_since.get_or_insert(sample.captured_at_ms);
            if sample.captured_at_ms - since >= SETTLE_MS {
                self.status = Status::Stable;
            } else if self.status != Status::Recovering {
                self.status = Status::Monitoring;
            }
            return;
        }
        self.stable_since = None;
        if self.direction != sample.offset_ms.signum() {
            self.bad_since = None;
            self.direction = sample.offset_ms.signum();
        }
        let since = *self.bad_since.get_or_insert(sample.captured_at_ms);
        self.status = Status::Monitoring;
        if sample.captured_at_ms - since < PERSIST_MS || now < self.next_allowed {
            return;
        }
        let correction = if sample.can_soft_correct && !self.soft_used {
            self.soft_used = true;
            Correction::NativeClock
        } else if sample.can_hard_correct && !self.hard_used && sample.offset_ms.abs() >= 250 {
            self.hard_used = true;
            Correction::Reseek
        } else {
            self.status = if self.soft_used && self.hard_used {
                Status::Exhausted
            } else {
                Status::Unsupported
            };
            return;
        };
        self.generation += 1;
        self.next_allowed = now.saturating_add(COOLDOWN_MS);
        self.pending = Some(Request {
            session_id: self.session_id,
            epoch: sample.epoch,
            generation: self.generation,
            sample_id: sample.sample_id,
            issued_at_ms: now,
            expires_at_ms: now.saturating_add(FRESH_MS),
            correction,
        });
        self.status = Status::AwaitingAck;
        self.bad_since = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(c: &Controller, id: u64, time: u64, offset: i64) -> Observation {
        Observation {
            session_id: c.session_id,
            epoch: 1,
            sample_id: id,
            captured_at_ms: time,
            offset_ms: offset,
            active: true,
            video_stable: true,
            can_soft_correct: true,
            can_hard_correct: true,
        }
    }
    fn feed(c: &mut Controller, id: u64, time: u64, offset: i64) {
        c.observe(&sample(c, id, time, offset), time, true);
    }
    fn started() -> Controller {
        let mut c = Controller::default();
        c.begin();
        c
    }
    fn request(c: &mut Controller) {
        for i in 1..=7 {
            feed(c, i, i * 500, 400);
        }
    }
    fn ack(c: &mut Controller, accepted: bool, now: u64) {
        let p = c.pending.as_ref().unwrap();
        c.acknowledge(
            &Acknowledgement {
                session_id: p.session_id,
                epoch: p.epoch,
                generation: p.generation,
                accepted,
            },
            now,
        );
    }
    #[test]
    fn no_measurement_is_not_stable_zero() {
        assert_eq!(started().offset_ms, None);
        assert_eq!(started().status, Status::Unknown);
    }
    #[test]
    fn elapsed_time_not_sample_volume() {
        let mut c = started();
        for i in 1..1000 {
            feed(&mut c, i, i, 400);
        }
        assert!(c.pending.is_none());
    }
    #[test]
    fn soft_precedes_hard_even_for_large_drift() {
        let mut c = started();
        request(&mut c);
        assert_eq!(c.pending.unwrap().correction, Correction::NativeClock);
    }
    #[test]
    fn old_sessions_cannot_request_repair() {
        let mut c = started();
        let s = sample(&c, 1, 500, 400);
        c.begin();
        c.observe(&s, 500, true);
        assert_eq!(c.offset_ms, None);
    }
    #[test]
    fn duplicate_and_reversed_samples_are_ignored() {
        let mut c = started();
        feed(&mut c, 1, 500, 400);
        let old = c.clone();
        feed(&mut c, 1, 900, 400);
        assert_eq!(c, old);
        feed(&mut c, 2, 499, 400);
        assert_eq!(c, old);
    }
    #[test]
    fn stale_future_and_extreme_values_are_ignored() {
        for (capture, now, offset) in [
            (500, 2101, 400),
            (501, 500, 400),
            (500, 500, i64::MIN),
            (500, 500, i64::MAX),
        ] {
            let mut c = started();
            let s = sample(&c, 1, capture, offset);
            c.observe(&s, now, true);
            assert_eq!(c.offset_ms, None);
        }
    }
    #[test]
    fn sign_changes_restart_persistence() {
        let mut c = started();
        for i in 1..40 {
            feed(&mut c, i, i * 500, if i % 2 == 0 { 400 } else { -400 });
        }
        assert!(c.pending.is_none());
    }
    #[test]
    fn gaps_restart_persistence() {
        let mut c = started();
        for i in 1..40 {
            feed(&mut c, i, i * 2000, 400);
        }
        assert!(c.pending.is_none());
    }
    #[test]
    fn interruption_rejects_old_epoch_and_preserves_budget() {
        let mut c = started();
        request(&mut c);
        c.interrupt();
        feed(&mut c, 8, 4000, 400);
        assert_eq!(c.offset_ms, None);
        assert!(c.soft_used);
        let mut s = sample(&c, 9, 4500, 400);
        s.epoch = 2;
        c.observe(&s, 4500, true);
        assert_eq!(c.offset_ms, Some(400));
        assert!(c.pending.is_none());
    }
    #[test]
    fn no_ack_exhausts_instead_of_retrying() {
        let mut c = started();
        request(&mut c);
        c.tick(c.session_id, 5001);
        assert_eq!(c.status, Status::Exhausted);
        assert!(c.pending.is_none());
    }
    #[test]
    fn wrong_ack_does_not_consume_request() {
        let mut c = started();
        request(&mut c);
        c.acknowledge(
            &Acknowledgement {
                session_id: c.session_id,
                epoch: 1,
                generation: 99,
                accepted: true,
            },
            3600,
        );
        assert!(c.pending.is_some());
    }
    #[test]
    fn ack_is_not_success_and_rejection_stops() {
        let mut c = started();
        request(&mut c);
        ack(&mut c, true, 3600);
        assert_eq!(c.status, Status::Recovering);
        let mut d = started();
        request(&mut d);
        ack(&mut d, false, 3600);
        assert_eq!(d.status, Status::Exhausted);
    }
    #[test]
    fn recovery_requires_fresh_stable_timing() {
        let mut c = started();
        request(&mut c);
        ack(&mut c, true, 3600);
        for i in 8..=12 {
            feed(&mut c, i, i * 500, 0);
        }
        assert_eq!(c.status, Status::Stable);
        c.tick(c.session_id, 8000);
        assert_eq!(c.status, Status::Unknown);
    }
    #[test]
    fn cooldown_and_two_attempt_session_budget() {
        let mut c = started();
        request(&mut c);
        ack(&mut c, true, 3600);
        for i in 8..67 {
            feed(&mut c, i, i * 500, 400);
            assert!(c.pending.is_none());
        }
        feed(&mut c, 67, 33500, 400);
        assert_eq!(c.pending.as_ref().unwrap().correction, Correction::Reseek);
        ack(&mut c, true, 33600);
        for i in 68..200 {
            feed(&mut c, i, i * 500, 400);
        }
        assert_eq!(c.generation, 2);
        assert_eq!(c.status, Status::Exhausted);
    }
    #[test]
    fn incapable_backends_cannot_trigger_commands() {
        let mut c = started();
        for i in 1..40 {
            let mut s = sample(&c, i, i * 500, 400);
            s.can_soft_correct = false;
            s.can_hard_correct = false;
            c.observe(&s, i * 500, true);
        }
        assert!(c.pending.is_none());
        assert_eq!(c.status, Status::Unsupported);
    }
    #[test]
    fn unstable_video_or_inactive_player_cannot_trigger_repair() {
        for active in [false, true] {
            let mut c = started();
            let mut s = sample(&c, 1, 500, 400);
            s.video_stable = false;
            c.observe(&s, 500, active);
            assert!(c.pending.is_none());
            assert_eq!(c.offset_ms, None);
        }
    }
    #[test]
    fn four_hour_stable_sessions_do_not_repair() {
        for _rate in [
            (24000, 1001),
            (24, 1),
            (25, 1),
            (30000, 1001),
            (30, 1),
            (50, 1),
            (60000, 1001),
            (60, 1),
        ] {
            let mut c = started();
            for i in 1..=28_800 {
                feed(&mut c, i, i * 500, 2);
            }
            assert_eq!(c.generation, 0);
            assert_eq!(c.status, Status::Stable);
        }
    }
    #[test]
    fn closed_session_ignores_queued_observations() {
        let mut c = started();
        c.close();
        feed(&mut c, 1, 500, 400);
        assert_eq!(c.offset_ms, None);
    }
    #[test]
    fn wire_format_preserves_required_fields() {
        let c = started();
        let s = sample(&c, 1, 500, 400);
        let value = serde_json::to_value(&s).unwrap();
        assert_eq!(value["sessionId"], 1);
        assert_eq!(serde_json::from_value::<Observation>(value).unwrap(), s);
        assert!(
            serde_json::from_value::<Observation>(serde_json::json!({"offsetMs": 400})).is_err()
        );
    }
}

#[cfg(test)]
mod acknowledgement_epoch_regressions {
    use super::*;
    fn pending() -> Controller {
        let mut c = Controller::default();
        c.begin();
        for i in 1..=7 {
            c.observe(
                &Observation {
                    session_id: c.session_id,
                    epoch: 1,
                    sample_id: i,
                    captured_at_ms: i * 500,
                    offset_ms: 400,
                    active: true,
                    video_stable: true,
                    can_soft_correct: true,
                    can_hard_correct: true,
                },
                i * 500,
                true,
            );
        }
        assert!(c.pending.is_some());
        c
    }
    fn native_transition(c: &mut Controller) {
        c.observe(
            &Observation {
                session_id: c.session_id,
                epoch: 2,
                sample_id: 8,
                captured_at_ms: 4000,
                offset_ms: 0,
                active: true,
                video_stable: false,
                can_soft_correct: false,
                can_hard_correct: false,
            },
            4000,
            true,
        );
    }
    #[test]
    fn native_epoch_cannot_erase_missing_acknowledgement() {
        let mut c = pending();
        let request = c.pending.clone();
        native_transition(&mut c);
        assert_eq!(c.pending, request);
        assert_eq!(c.status, Status::AwaitingAck);
        c.tick(c.session_id, 5001);
        assert_eq!(c.status, Status::Exhausted);
    }
    #[test]
    fn delayed_receipt_matches_original_request_after_native_epoch_changes() {
        let mut c = pending();
        let request = c.pending.clone().unwrap();
        native_transition(&mut c);
        c.acknowledge(
            &Acknowledgement {
                session_id: request.session_id,
                epoch: request.epoch,
                generation: request.generation,
                accepted: true,
            },
            4100,
        );
        assert!(c.pending.is_none());
        assert_eq!(c.status, Status::Recovering);
        assert_eq!(c.offset_ms, None);
    }
    #[test]
    fn explicit_user_interruption_still_revokes_recovery_immediately() {
        let mut c = pending();
        native_transition(&mut c);
        c.interrupt();
        assert!(c.pending.is_none());
        assert_eq!(c.status, Status::Unknown);
    }
}
