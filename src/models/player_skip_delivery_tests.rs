use super::*;
use crate::types::{
    api::{SeekEvent, SkipGaps},
    library::LibraryItemState,
    profile::SkipIntroMode,
    resource::{PosterShape, StreamBehaviorHints},
    skip_segments::{SkipSegmentMatch, SkipSegmentStreamSpecificity},
};
use crate::unit_tests::TestEnv;
use std::collections::HashMap;

fn item() -> LibraryItem {
    LibraryItem {
        id: "tt0903747".into(),
        name: "Test episode".into(),
        r#type: "series".into(),
        poster: None,
        poster_shape: PosterShape::Poster,
        removed: false,
        temp: true,
        ctime: None,
        mtime: Utc::now(),
        behavior_hints: Default::default(),
        state: LibraryItemState {
            last_watched: None,
            time_watched: 0,
            time_offset: 15_000,
            overall_time_watched: 0,
            times_watched: 0,
            flagged_watched: 0,
            duration: 100_000,
            video_id: Some("tt0903747:1:5".into()),
            watched: None,
            no_notif: true,
        },
    }
}

fn candidate(source: SkipSegmentSource, start_ms: u64) -> SkipSegmentCandidate {
    SkipSegmentCandidate {
        kind: SkipSegmentKind::Intro,
        start_ms,
        end_ms: start_ms + 10_000,
        source,
        source_match: SkipSegmentMatch::ExactEpisode,
        source_confidence: Some(0.95),
        adjusted: false,
        evidence_count: 20,
        stream_specificity: SkipSegmentStreamSpecificity::Episode,
    }
}

fn context(generation: u64) -> SkipSegmentContext {
    SkipSegmentContext {
        playback_generation: generation,
        item_id: "tt0903747".into(),
        media_type: "series".into(),
        season: Some(1),
        episode: Some(5),
        duration_ms: Some(100_000),
        open_subtitles_hash: None,
        stream_name_hash: None,
    }
}

fn selected() -> Selected {
    Selected {
        stream: Stream {
            source: StreamSource::Url {
                url: "https://example.org/video.mp4".parse().unwrap(),
            },
            name: None,
            description: None,
            thumbnail: None,
            subtitles: vec![],
            behavior_hints: StreamBehaviorHints::default(),
        },
        stream_request: Some(ResourceRequest {
            base: "https://example.org/manifest.json".parse().unwrap(),
            path: ResourcePath {
                resource: "stream".into(),
                r#type: "series".into(),
                id: "tt0903747:1:5".into(),
                extra: vec![],
            },
        }),
        meta_request: None,
        subtitles_path: None,
    }
}

fn update(player: &mut Player, msg: Msg, ctx: &Ctx) -> Effects {
    <Player as UpdateWithCtx<TestEnv>>::update(player, &msg, ctx)
}

#[test]
fn successful_empty_result_withdraws_previous_external_intro() {
    let mut value = None;
    let library = item();
    apply_resolved_skip_segments(
        &mut value,
        &None,
        Some(&library),
        &[candidate(SkipSegmentSource::IntroDb, 10_000)],
    );
    assert!(value.as_ref().unwrap().intro.is_some());
    apply_resolved_skip_segments(&mut value, &None, Some(&library), &[]);
    assert!(value.is_none());
}

#[test]
fn conflicting_fresh_providers_withdraw_previous_external_intro() {
    let library = item();
    let mut value = None;
    let a = candidate(SkipSegmentSource::IntroDb, 10_000);
    apply_resolved_skip_segments(&mut value, &None, Some(&library), std::slice::from_ref(&a));
    assert!(value.as_ref().unwrap().intro.is_some());
    apply_resolved_skip_segments(
        &mut value,
        &None,
        Some(&library),
        &[a, candidate(SkipSegmentSource::TheIntroDb, 50_000)],
    );
    assert!(value.as_ref().unwrap().intro.is_none());
}

#[test]
fn duration_loss_clears_previous_resolved_evidence() {
    let mut library = item();
    let mut value = None;
    let candidates = [candidate(SkipSegmentSource::IntroDb, 10_000)];
    apply_resolved_skip_segments(&mut value, &None, Some(&library), &candidates);
    library.state.duration = 0;
    apply_resolved_skip_segments(&mut value, &None, Some(&library), &candidates);
    assert!(value.is_none());
}

#[test]
fn cache_restore_ignores_old_session_generation_not_media_identity() {
    let now = Utc::now();
    let mut entry = SkipSegmentCacheEntry {
        context: context(1),
        source: SkipSegmentSource::IntroDb,
        candidates: vec![candidate(SkipSegmentSource::IntroDb, 10_000)],
        cached_at: now,
    };
    assert!(valid_skip_cache(
        &entry,
        &context(2),
        SkipSegmentSource::IntroDb,
        now
    ));
    entry.context.episode = Some(6);
    assert!(!valid_skip_cache(
        &entry,
        &context(2),
        SkipSegmentSource::IntroDb,
        now
    ));
}

#[test]
fn future_expired_wrong_provider_and_malformed_cache_fail_closed() {
    let now = Utc::now();
    let entry = SkipSegmentCacheEntry {
        context: context(1),
        source: SkipSegmentSource::IntroDb,
        candidates: vec![candidate(SkipSegmentSource::IntroDb, 10_000)],
        cached_at: now,
    };
    for timestamp in [
        now + Duration::milliseconds(1),
        now - Duration::days(30),
        now - Duration::days(31),
    ] {
        let mut invalid = entry.clone();
        invalid.cached_at = timestamp;
        assert!(!valid_skip_cache(
            &invalid,
            &context(1),
            SkipSegmentSource::IntroDb,
            now
        ));
    }
    for confidence in [f64::NAN, f64::INFINITY, -1.0, 1.01] {
        let mut invalid = entry.clone();
        invalid.candidates[0].source_confidence = Some(confidence);
        assert!(!valid_skip_cache(
            &invalid,
            &context(1),
            SkipSegmentSource::IntroDb,
            now
        ));
    }
    let mut invalid = entry.clone();
    invalid.candidates[0].source = SkipSegmentSource::SkipDb;
    assert!(!valid_skip_cache(
        &invalid,
        &context(1),
        SkipSegmentSource::IntroDb,
        now
    ));
    let mut invalid = entry.clone();
    invalid.candidates[0].end_ms = 100_000;
    assert!(!valid_skip_cache(
        &invalid,
        &context(1),
        SkipSegmentSource::IntroDb,
        now
    ));
    assert!(!valid_skip_cache(
        &entry,
        &context(1),
        SkipSegmentSource::SkipDb,
        now
    ));
}

#[test]
fn late_provider_and_cache_results_cannot_cross_player_generations() {
    let _guard = TestEnv::reset().unwrap();
    let mut player = Player {
        skip_segment_context: Some(context(2)),
        ..Default::default()
    };
    let ctx = Ctx::default();
    let candidates = vec![candidate(SkipSegmentSource::IntroDb, 10_000)];
    let result = update(
        &mut player,
        Msg::Internal(Internal::SkipSegmentsResult(
            SkipSegmentSource::IntroDb,
            context(1),
            Ok(candidates.clone()),
        )),
        &ctx,
    );
    assert!(!result.has_changed);
    assert!(player.skip_segment_candidates.is_empty());
    let entry = SkipSegmentCacheEntry {
        context: context(1),
        source: SkipSegmentSource::IntroDb,
        candidates,
        cached_at: Utc::now(),
    };
    let result = update(
        &mut player,
        Msg::Internal(Internal::SkipSegmentsCacheResult(
            SkipSegmentSource::IntroDb,
            context(1),
            Ok(Some(entry)),
        )),
        &ctx,
    );
    assert!(!result.has_changed);
    assert!(player.skip_segment_candidates.is_empty());
}

#[test]
fn fresh_intro_ending_at_media_duration_fails_closed() {
    let _guard = TestEnv::reset().unwrap();
    let mut player = Player {
        library_item: Some(item()),
        skip_segment_context: Some(context(3)),
        ..Default::default()
    };
    let mut boundary = candidate(SkipSegmentSource::SkipDb, 90_000);
    boundary.end_ms = 100_000;

    let result = update(
        &mut player,
        Msg::Internal(Internal::SkipSegmentsResult(
            SkipSegmentSource::SkipDb,
            context(3),
            Ok(vec![boundary]),
        )),
        &Ctx::default(),
    );

    assert!(!result.has_changed);
    assert!(player.skip_segment_candidates.is_empty());
    assert!(player.intro_outro.is_none());
}

#[test]
fn model_dismissal_is_generation_bound_and_preference_updates_shared_target() {
    let _guard = TestEnv::reset().unwrap();
    let mut player = Player {
        selected: Some(selected()),
        library_item: Some(item()),
        playback_generation: 2,
        skip_intro_playback: Some((15_000, 100_000)),
        intro_outro: Some(IntroOutro {
            intro: Some(IntroData {
                from: 10_000,
                to: 20_000,
                duration: None,
            }),
            outro: None,
        }),
        skip_segment_candidates: vec![candidate(SkipSegmentSource::IntroDb, 10_000)],
        ..Default::default()
    };
    let mut ctx = Ctx::default();
    update(&mut player, Msg::Internal(Internal::ProfileChanged), &ctx);
    assert_eq!(player.skip_segment.as_ref().unwrap().kind, SkipSegmentKind::Intro);
    assert_eq!(player.skip_segment.as_ref().unwrap().seek_to, Some(20_000));
    assert_eq!(player.skip_intro.as_ref().unwrap().seek_to, Some(20_000));
    update(
        &mut player,
        Msg::Action(Action::Player(ActionPlayer::DismissSkipIntro {
            generation: 1,
            from: 10_000,
            to: 20_000,
        })),
        &ctx,
    );
    assert_eq!(player.skip_intro.as_ref().unwrap().seek_to, Some(20_000));
    ctx.profile.settings.skip_intro_mode = SkipIntroMode::Never;
    update(&mut player, Msg::Internal(Internal::ProfileChanged), &ctx);
    assert_eq!(player.skip_intro.as_ref().unwrap().seek_to, None);
    ctx.profile.settings.skip_intro_mode = SkipIntroMode::Always;
    update(&mut player, Msg::Internal(Internal::ProfileChanged), &ctx);
    assert_eq!(player.skip_intro.as_ref().unwrap().seek_to, Some(20_000));
    update(
        &mut player,
        Msg::Action(Action::Player(ActionPlayer::DismissSkipIntro {
            generation: 2,
            from: 10_000,
            to: 20_000,
        })),
        &ctx,
    );
    assert_eq!(player.skip_intro.as_ref().unwrap().seek_to, None);
    let json = serde_json::to_value(&player).unwrap();
    assert_eq!(json["skipIntro"]["videoId"], "tt0903747:1:5");
    assert_eq!(json["skipIntro"]["dismissed"], true);
    update(&mut player, Msg::Action(Action::Unload), &ctx);
    assert!(player.skip_segment.is_none());
    assert!(player.skip_intro.is_none());
    assert!(player.intro_outro.is_none());
    assert!(player.skip_intro_playback.is_none());
    assert!(player.skip_segment_dismissal.is_none());
}

#[test]
fn new_load_resets_descriptor_even_for_identical_source() {
    let _guard = TestEnv::reset().unwrap();
    let mut player = Player {
        selected: Some(selected()),
        playback_generation: 4,
        skip_intro_playback: Some((15_000, 100_000)),
        ..Default::default()
    };
    update(
        &mut player,
        Msg::Action(Action::Load(ActionLoad::Player(Box::new(selected())))),
        &Ctx::default(),
    );
    assert_eq!(player.playback_generation, 5);
    assert!(player.skip_segment.is_none());
    assert!(player.skip_intro.is_none());
    assert!(player.skip_intro_playback.is_none());
}

#[test]
fn invalid_native_durations_cannot_panic_or_finish_episode() {
    let response = SkipGapsResponse {
        accuracy: "byEpisode".into(),
        gaps: HashMap::from([
            (
                0,
                SkipGaps {
                    seek_history: vec![SeekEvent {
                        records: 100,
                        from: 0,
                        to: u64::MAX,
                    }],
                    outro: None,
                },
            ),
            (
                100_000,
                SkipGaps {
                    seek_history: vec![SeekEvent {
                        records: 100,
                        from: 10_000,
                        to: 100_000,
                    }],
                    outro: None,
                },
            ),
        ]),
    };
    assert!(stremio_native_candidates(&response, 100_000).is_empty());
}
