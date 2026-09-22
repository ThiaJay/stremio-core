from pathlib import Path


def once(text, before, after):
    assert text.count(before) == 1, 'Integration anchor missing or ambiguous'
    return text.replace(before, after, 1)


root = Path('.')
policy_path = root / 'src/types/player/av_sync_v2.rs'
policy = policy_path.read_text()
if 'mod acknowledgement_epoch_regressions' not in policy:
    policy = once(policy, '''        if sample.epoch != self.epoch {
            self.pending = None;
            self.bad_since = None;
            self.stable_since = None;
            self.last_capture = None;
            self.epoch = sample.epoch;
            self.status = Status::Unknown;
        }''', '''        if sample.epoch != self.epoch {
            // Recovery itself may advance the native epoch before its receipt arrives.
            // Keep the original request until receipt, expiry or an explicit user interruption.
            self.bad_since = None;
            self.stable_since = None;
            self.last_capture = None;
            self.epoch = sample.epoch;
            if self.pending.is_none() {
                self.status = Status::Unknown;
            }
        }''')
    policy = once(policy, '''        self.sample_id = sample.sample_id;
        if !player_active || !sample.active || !sample.video_stable {''', '''        self.sample_id = sample.sample_id;
        if self.pending.is_some() {
            self.offset_ms = if player_active && sample.active && sample.video_stable {
                Some(sample.offset_ms)
            } else {
                None
            };
            return;
        }
        if !player_active || !sample.active || !sample.video_stable {''')
    policy = once(policy, '''        self.offset_ms = Some(sample.offset_ms);
        if self.pending.is_some() {
            return;
        }
        if sample.offset_ms.abs() <= 60 {''', '''        self.offset_ms = Some(sample.offset_ms);
        if sample.offset_ms.abs() <= 60 {''')
    policy += '''
#[cfg(test)]
mod acknowledgement_epoch_regressions {
    use super::*;
    fn pending() -> Controller {
        let mut c = Controller::default();
        c.begin();
        for i in 1..=7 {
            c.observe(&Observation { session_id: c.session_id, epoch: 1, sample_id: i,
                captured_at_ms: i * 500, offset_ms: 400, active: true, video_stable: true,
                can_soft_correct: true, can_hard_correct: true }, i * 500, true);
        }
        assert!(c.pending.is_some());
        c
    }
    fn native_transition(c: &mut Controller) {
        c.observe(&Observation { session_id: c.session_id, epoch: 2, sample_id: 8,
            captured_at_ms: 4000, offset_ms: 0, active: true, video_stable: false,
            can_soft_correct: false, can_hard_correct: false }, 4000, true);
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
        c.acknowledge(&Acknowledgement { session_id: request.session_id, epoch: request.epoch,
            generation: request.generation, accepted: true }, 4100);
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
'''
    policy_path.write_text(policy)

model_path = root / 'src/models/player.rs'
if 'pub av_sync_v2:' in model_path.read_text():
    print('A/V session safety bindings and acknowledgement transition guards integrated')
    raise SystemExit(0)

types = root / 'src/types/player.rs'
types.write_text(once(types.read_text(), 'use serde::{Deserialize, Serialize};', 'use serde::{Deserialize, Serialize};\n\npub mod av_sync_v2;'))
model = model_path.read_text()
model = once(model, '    pub av_sync: AvSyncState,', '    pub av_sync: AvSyncState,\n    pub av_sync_v2: crate::types::player::av_sync_v2::Controller,')
preamble = '''        let mut sync = self.av_sync_v2.clone();
        match msg {
            Msg::Action(Action::Load(ActionLoad::Player(_))) => sync.begin(),
            Msg::Action(Action::Unload) => sync.close(),
            Msg::Action(Action::Player(ActionPlayer::Seek { .. } | ActionPlayer::Ended | ActionPlayer::PausedChanged { paused: true })) => sync.interrupt(),
            Msg::Action(Action::Player(ActionPlayer::AvSyncV2Observed { observation, now_ms })) => {
                sync.observe(observation, *now_ms, self.selected.is_some() && self.paused == Some(false) && !self.ended && self.playback_health.recovery.is_none());
                return eq_update(&mut self.av_sync_v2, sync);
            }
            Msg::Action(Action::Player(ActionPlayer::AvSyncV2Acknowledged { acknowledgement, now_ms })) => {
                sync.acknowledge(acknowledgement, *now_ms);
                return eq_update(&mut self.av_sync_v2, sync);
            }
            Msg::Action(Action::Player(ActionPlayer::AvSyncV2Tick { session_id, now_ms })) => {
                sync.tick(*session_id, *now_ms);
                return eq_update(&mut self.av_sync_v2, sync);
            }
            _ => {}
        }
        let sync_effects = eq_update(&mut self.av_sync_v2, sync);
'''
model = once(model, '        let effects = match msg {', preamble + '        let effects = match msg {')
if '        };\n        if matches!(' in model:
    model = once(model, '        };\n        if matches!(', '        };\n        let effects = effects.join(sync_effects);\n        if matches!(')
else:
    model = once(model, '        };\n        let effects = if matches!(', '        };\n        let effects = effects.join(sync_effects);\n        let effects = if matches!(')
model_path.write_text(model)
action_path = root / 'src/runtime/msg/action.rs'
actions = '''pub enum ActionPlayer {
    #[serde(rename_all = "camelCase")]
    AvSyncV2Observed { observation: crate::types::player::av_sync_v2::Observation, now_ms: u64 },
    #[serde(rename_all = "camelCase")]
    AvSyncV2Acknowledged { acknowledgement: crate::types::player::av_sync_v2::Acknowledgement, now_ms: u64 },
    #[serde(rename_all = "camelCase")]
    AvSyncV2Tick { session_id: u64, now_ms: u64 },'''
action_path.write_text(once(action_path.read_text(), 'pub enum ActionPlayer {', actions))
serializer = root / 'stremio-core-web/src/model/serialize_player.rs'
text = once(serializer.read_text(), "        pub av_sync: &'a AvSyncState,", "        pub av_sync: &'a AvSyncState,\n        pub av_sync_v2: &'a stremio_core::types::player::av_sync_v2::Controller,")
text = once(text, '        av_sync: &player.av_sync,', '        av_sync: &player.av_sync,\n        av_sync_v2: &player.av_sync_v2,')
serializer.write_text(text)
print('Integrated session bound A/V recovery without changing DTS or legacy playback logic')
