use crate::types::{
    player::SkipSegmentState,
    profile::SkipIntroMode,
    skip_segments::SkipSegmentKind,
};

/// Shared half-open segment window and seek destination for every client.
pub fn state(
    kind: SkipSegmentKind,
    from: u64,
    to: u64,
    video_id: &str,
    generation: u64,
    time: u64,
    duration: u64,
    mode: &SkipIntroMode,
    dismissal: Option<(u64, SkipSegmentKind, u64, u64)>,
) -> Option<SkipSegmentState> {
    if video_id.is_empty()
        || generation == 0
        || duration == 0
        || time > duration
        || to <= from
        || to > duration
        || (kind != SkipSegmentKind::Outro && to == duration)
    {
        return None;
    }

    let active = time >= from && time < to;
    let dismissed = dismissal == Some((generation, kind, from, to));
    Some(SkipSegmentState {
        kind,
        generation,
        video_id: video_id.to_owned(),
        from,
        to,
        duration,
        mode: mode.clone(),
        active,
        dismissed,
        seek_to: (active && !dismissed && *mode != SkipIntroMode::Never).then_some(to),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundaries_modes_kinds_and_dismissal_are_deterministic() {
        for kind in [
            SkipSegmentKind::Intro,
            SkipSegmentKind::Recap,
            SkipSegmentKind::Outro,
        ] {
            let (from, to) = if kind == SkipSegmentKind::Outro {
                (80_000, 100_000)
            } else {
                (10_000, 20_000)
            };
            for mode in [
                SkipIntroMode::Ask,
                SkipIntroMode::Always,
                SkipIntroMode::Never,
            ] {
                for time in [from.saturating_sub(1), from, to - 1, to] {
                    let actual =
                        state(kind, from, to, "episode", 1, time, 100_000, &mode, None).unwrap();
                    let expected = (from..to).contains(&time) && mode != SkipIntroMode::Never;
                    assert_eq!(actual.seek_to, expected.then_some(to));
                }
            }
        }

        let dismissed = state(
            SkipSegmentKind::Recap,
            10_000,
            20_000,
            "episode",
            1,
            15_000,
            100_000,
            &SkipIntroMode::Always,
            Some((1, SkipSegmentKind::Recap, 10_000, 20_000)),
        )
        .unwrap();
        assert_eq!(dismissed.seek_to, None);

        let reloaded = state(
            SkipSegmentKind::Recap,
            10_000,
            20_000,
            "episode",
            2,
            15_000,
            100_000,
            &SkipIntroMode::Always,
            Some((1, SkipSegmentKind::Recap, 10_000, 20_000)),
        )
        .unwrap();
        assert_eq!(reloaded.seek_to, Some(20_000));
    }

    #[test]
    fn malformed_segments_and_missing_runtime_information_fail_closed() {
        for (kind, from, to, time, duration, generation, id) in [
            (SkipSegmentKind::Intro, 0, 0, 0, 100, 1, "episode"),
            (SkipSegmentKind::Intro, 20, 10, 12, 100, 1, "episode"),
            (SkipSegmentKind::Intro, 0, 100, 12, 100, 1, "episode"),
            (SkipSegmentKind::Recap, 0, 101, 12, 100, 1, "episode"),
            (SkipSegmentKind::Outro, 90, 101, 95, 100, 1, "episode"),
            (SkipSegmentKind::Intro, 0, 20, 12, 0, 1, "episode"),
            (SkipSegmentKind::Intro, 0, 20, 101, 100, 1, "episode"),
            (SkipSegmentKind::Intro, 0, 20, 12, 100, 0, "episode"),
            (SkipSegmentKind::Intro, 0, 20, 12, 100, 1, ""),
        ] {
            assert!(state(
                kind,
                from,
                to,
                id,
                generation,
                time,
                duration,
                &SkipIntroMode::Ask,
                None
            )
            .is_none());
        }
    }
}
