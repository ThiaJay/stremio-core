use crate::types::{
    player::{IntroData, SkipIntroState},
    profile::SkipIntroMode,
};

/// The half open intro window and seek destination are shared by every client.
pub fn state(
    intro: Option<&IntroData>,
    video_id: &str,
    generation: u64,
    time: u64,
    duration: u64,
    mode: &SkipIntroMode,
    dismissal: Option<(u64, u64, u64)>,
) -> Option<SkipIntroState> {
    let intro = intro?;
    if video_id.is_empty()
        || generation == 0
        || duration == 0
        || time > duration
        || intro.to <= intro.from
        || intro.to >= duration
    {
        return None;
    }
    let active = time >= intro.from && time < intro.to;
    let dismissed = dismissal == Some((generation, intro.from, intro.to));
    Some(SkipIntroState {
        generation,
        video_id: video_id.to_owned(),
        from: intro.from,
        to: intro.to,
        duration,
        mode: mode.clone(),
        active,
        dismissed,
        seek_to: (active && !dismissed && *mode != SkipIntroMode::Never).then_some(intro.to),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_modes_and_dismissal_are_deterministic() {
        let intro = IntroData {
            from: 10_000,
            to: 20_000,
            duration: None,
        };
        for mode in [
            SkipIntroMode::Ask,
            SkipIntroMode::Always,
            SkipIntroMode::Never,
        ] {
            for time in [9_999, 10_000, 19_999, 20_000, 20_001] {
                let actual = state(Some(&intro), "episode", 1, time, 100_000, &mode, None).unwrap();
                let expected = (10_000..20_000).contains(&time) && mode != SkipIntroMode::Never;
                assert_eq!(actual.seek_to, expected.then_some(20_000));
            }
        }
        let dismissed = state(
            Some(&intro),
            "episode",
            1,
            15_000,
            100_000,
            &SkipIntroMode::Always,
            Some((1, 10_000, 20_000)),
        )
        .unwrap();
        assert_eq!(dismissed.seek_to, None);
        let reloaded = state(
            Some(&intro),
            "episode",
            2,
            15_000,
            100_000,
            &SkipIntroMode::Always,
            Some((1, 10_000, 20_000)),
        )
        .unwrap();
        assert_eq!(reloaded.seek_to, Some(20_000));
    }
    #[test]
    fn malformed_segments_and_missing_runtime_information_fail_closed() {
        for (from, to, time, duration, generation, id) in [
            (0, 0, 0, 100, 1, "episode"),
            (20, 10, 12, 100, 1, "episode"),
            (0, 100, 12, 100, 1, "episode"),
            (0, 101, 12, 100, 1, "episode"),
            (0, 20, 12, 0, 1, "episode"),
            (0, 20, 101, 100, 1, "episode"),
            (0, 20, 12, 100, 0, "episode"),
            (0, 20, 12, 100, 1, ""),
        ] {
            let intro = IntroData {
                from,
                to,
                duration: None,
            };
            assert!(state(
                Some(&intro),
                id,
                generation,
                time,
                duration,
                &SkipIntroMode::Ask,
                None
            )
            .is_none());
        }
        assert!(state(None, "episode", 1, 12, 100, &SkipIntroMode::Ask, None).is_none());
    }
}
