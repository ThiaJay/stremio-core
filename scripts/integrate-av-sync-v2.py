from pathlib import Path


def once(text, before, after):
    assert text.count(before) == 1, 'Integration anchor missing or ambiguous'
    return text.replace(before, after, 1)


root = Path('.')
model_path = root / 'src/models/player.rs'
if 'pub av_sync_v2:' in model_path.read_text():
    print('A/V session safety bindings already integrated')
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
model = once(model, '        };\n        if matches!(', '        };\n        let effects = effects.join(sync_effects);\n        if matches!(')
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
