use crate::{
    models::{
        ctx::Ctx,
        player::{Player, Selected},
    },
    runtime::{
        msg::{Action, ActionPlayer, Msg},
        UpdateWithCtx,
    },
    types::{
        player::{PlaybackHealthObservation, PlaybackHealthStatus, PlaybackRecoveryAction},
        resource::{Stream, StreamBehaviorHints, StreamSource},
    },
    unit_tests::TestEnv,
};

fn selected() -> Selected {
    Selected {
        stream: Stream {
            source: StreamSource::Url {
                url: "https://source_url".parse().unwrap(),
            },
            name: None,
            description: None,
            thumbnail: None,
            subtitles: vec![],
            behavior_hints: StreamBehaviorHints::default(),
        },
        stream_request: None,
        meta_request: None,
        subtitles_path: None,
    }
}

fn observation() -> PlaybackHealthObservation {
    PlaybackHealthObservation {
        engine: Some("exoplayer".to_owned()),
        audio_codec: Some("dts".to_owned()),
        audio_expected: true,
        audio_present: true,
        video_stable: true,
        transient: false,
        can_transcode_audio: true,
        can_switch_engine: true,
        can_restore_stable_video: false,
        can_switch_stream: true,
    }
}

fn report(player: &mut Player, ctx: &Ctx, observation: PlaybackHealthObservation) {
    <Player as UpdateWithCtx<TestEnv>>::update(
        player,
        &Msg::Action(Action::Player(ActionPlayer::PlaybackHealthObserved {
            observation,
        })),
        ctx,
    );
}

#[test]
fn silent_dts_on_stable_video_prefers_audio_transcode() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };
    let mut sample = observation();
    sample.audio_present = false;

    report(&mut player, &ctx, sample.clone());
    assert_eq!(
        player.playback_health.status,
        PlaybackHealthStatus::Monitoring
    );

    report(&mut player, &ctx, sample);
    assert_eq!(
        player.playback_health.status,
        PlaybackHealthStatus::Recovering
    );
    assert_eq!(
        player.playback_health.recovery,
        Some(PlaybackRecoveryAction::TranscodeAudio)
    );
}

#[test]
fn silent_dts_without_transcoder_falls_back_to_another_engine() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };
    let mut sample = observation();
    sample.audio_present = false;
    sample.can_transcode_audio = false;

    report(&mut player, &ctx, sample.clone());
    report(&mut player, &ctx, sample);

    assert_eq!(
        player.playback_health.recovery,
        Some(PlaybackRecoveryAction::SwitchPlaybackEngine)
    );
}

#[test]
fn audio_fixed_with_stuttering_video_is_not_accepted_as_recovered() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };

    let mut exo = observation();
    exo.audio_present = false;
    exo.can_transcode_audio = false;
    report(&mut player, &ctx, exo.clone());
    report(&mut player, &ctx, exo);
    assert_eq!(
        player.playback_health.recovery,
        Some(PlaybackRecoveryAction::SwitchPlaybackEngine)
    );

    let mut vlc = observation();
    vlc.engine = Some("vlc".to_owned());
    vlc.audio_present = true;
    vlc.video_stable = false;
    vlc.can_restore_stable_video = true;
    report(&mut player, &ctx, vlc.clone());
    report(&mut player, &ctx, vlc);

    assert_eq!(
        player.playback_health.status,
        PlaybackHealthStatus::Recovering
    );
    assert_eq!(
        player.playback_health.recovery,
        Some(PlaybackRecoveryAction::RestoreStableVideoAndTranscodeAudio)
    );
    assert_eq!(player.playback_health.generation, 2);
}

#[test]
fn stream_change_is_last_resort_when_video_path_cannot_be_restored() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };
    let mut sample = observation();
    sample.engine = Some("vlc".to_owned());
    sample.video_stable = false;
    sample.can_restore_stable_video = false;
    sample.can_transcode_audio = false;
    sample.can_switch_engine = false;

    report(&mut player, &ctx, sample.clone());
    report(&mut player, &ctx, sample);

    assert_eq!(
        player.playback_health.recovery,
        Some(PlaybackRecoveryAction::SwitchStream)
    );
}

#[test]
fn transient_startup_does_not_trigger_recovery() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };
    let mut sample = observation();
    sample.audio_present = false;
    sample.video_stable = false;
    sample.transient = true;

    for _ in 0..4 {
        report(&mut player, &ctx, sample.clone());
    }

    assert_eq!(player.playback_health.status, PlaybackHealthStatus::Monitoring);
    assert_eq!(player.playback_health.recovery, None);
    assert_eq!(player.playback_health.generation, 0);
}

#[test]
fn action_deserializes() {
    let action = serde_json::from_value::<Action>(serde_json::json!({
        "action": "Player",
        "args": {
            "action": "PlaybackHealthObserved",
            "args": {
                "observation": {
                    "engine": "exoplayer",
                    "audioCodec": "dts",
                    "audioExpected": true,
                    "audioPresent": false,
                    "videoStable": true,
                    "canTranscodeAudio": true,
                    "canSwitchEngine": true,
                    "canSwitchStream": true
                }
            }
        }
    }))
    .expect("Should deserialize playback health observation");

    assert!(matches!(
        action,
        Action::Player(ActionPlayer::PlaybackHealthObserved {
            observation: PlaybackHealthObservation {
                audio_expected: true,
                audio_present: false,
                video_stable: true,
                can_transcode_audio: true,
                ..
            }
        })
    ));
}
