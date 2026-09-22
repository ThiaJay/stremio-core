use crate::{
    models::{
        ctx::Ctx,
        player::{Player, Selected},
    },
    runtime::{
        msg::{Action, ActionLoad, ActionPlayer, Msg},
        UpdateWithCtx,
    },
    types::{
        player::{AvSyncCorrection, AvSyncObservation, AvSyncStatus},
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

fn observation(offset_ms: i64) -> AvSyncObservation {
    AvSyncObservation {
        offset_ms,
        buffering: false,
        seeking: false,
        refresh_rate_switching: false,
        can_soft_correct: true,
        can_hard_correct: true,
    }
}

fn report(player: &mut Player, ctx: &Ctx, observation: AvSyncObservation) {
    <Player as UpdateWithCtx<TestEnv>>::update(
        player,
        &Msg::Action(Action::Player(ActionPlayer::AvSyncObserved { observation })),
        ctx,
    );
}

#[test]
fn persistent_drift_requests_soft_correction() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };

    report(&mut player, &ctx, observation(120));
    assert_eq!(player.av_sync.status, AvSyncStatus::Monitoring);
    assert_eq!(player.av_sync.correction, None);

    report(&mut player, &ctx, observation(118));
    assert_eq!(player.av_sync.status, AvSyncStatus::Monitoring);
    assert_eq!(player.av_sync.correction, None);

    report(&mut player, &ctx, observation(121));
    assert_eq!(player.av_sync.status, AvSyncStatus::Correcting);
    assert_eq!(player.av_sync.correction, Some(AvSyncCorrection::Soft));
    assert_eq!(player.av_sync.generation, 1);
}

#[test]
fn large_persistent_drift_prefers_hard_resync() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };

    for _ in 0..3 {
        report(&mut player, &ctx, observation(-420));
    }

    assert_eq!(player.av_sync.status, AvSyncStatus::Correcting);
    assert_eq!(player.av_sync.correction, Some(AvSyncCorrection::Hard));
}

#[test]
fn transients_do_not_trigger_correction() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };
    let mut transient = observation(500);
    transient.buffering = true;

    for _ in 0..5 {
        report(&mut player, &ctx, transient.clone());
    }

    assert_eq!(player.av_sync.status, AvSyncStatus::Monitoring);
    assert_eq!(player.av_sync.correction, None);
    assert_eq!(player.av_sync.generation, 0);
}

#[test]
fn unavailable_correction_is_reported_without_guessing() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };
    let mut unsupported = observation(180);
    unsupported.can_soft_correct = false;
    unsupported.can_hard_correct = false;

    for _ in 0..3 {
        report(&mut player, &ctx, unsupported.clone());
    }

    assert_eq!(player.av_sync.status, AvSyncStatus::Uncorrectable);
    assert_eq!(player.av_sync.correction, None);
}

#[test]
fn cooldown_prevents_immediate_correction_loop() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };

    for _ in 0..3 {
        report(&mut player, &ctx, observation(120));
    }
    assert_eq!(player.av_sync.generation, 1);

    for _ in 0..5 {
        report(&mut player, &ctx, observation(120));
        assert_eq!(player.av_sync.correction, None);
        assert_eq!(player.av_sync.generation, 1);
    }

    for _ in 0..3 {
        report(&mut player, &ctx, observation(120));
    }
    assert_eq!(player.av_sync.correction, Some(AvSyncCorrection::Soft));
    assert_eq!(player.av_sync.generation, 2);
}

#[test]
fn player_load_resets_previous_sync_state() {
    let _env_mutex = TestEnv::reset().expect("Should have exclusive lock to TestEnv");
    let ctx = Ctx::default();
    let mut player = Player {
        selected: Some(selected()),
        ..Default::default()
    };

    for _ in 0..3 {
        report(&mut player, &ctx, observation(120));
    }
    assert_eq!(player.av_sync.correction, Some(AvSyncCorrection::Soft));

    <Player as UpdateWithCtx<TestEnv>>::update(
        &mut player,
        &Msg::Action(Action::Load(ActionLoad::Player(Box::new(selected())))),
        &ctx,
    );

    assert_eq!(player.av_sync.status, AvSyncStatus::Stable);
    assert_eq!(player.av_sync.correction, None);
    assert_eq!(player.av_sync.generation, 0);
}

#[test]
fn action_deserializes() {
    let action = serde_json::from_value::<Action>(serde_json::json!({
        "action": "Player",
        "args": {
            "action": "AvSyncObserved",
            "args": {
                "observation": {
                    "offsetMs": 95,
                    "canSoftCorrect": true,
                    "canHardCorrect": true
                }
            }
        }
    }))
    .expect("Should deserialize A/V sync observation");

    assert!(matches!(
        action,
        Action::Player(ActionPlayer::AvSyncObserved {
            observation: AvSyncObservation {
                offset_ms: 95,
                can_soft_correct: true,
                can_hard_correct: true,
                ..
            }
        })
    ));
}
