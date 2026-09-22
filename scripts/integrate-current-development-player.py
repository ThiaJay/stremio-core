from pathlib import Path


def once(text, before, after):
    assert text.count(before) == 1, 'Development integration anchor missing or ambiguous'
    return text.replace(before, after, 1)


path = Path('src/models/player.rs')
text = path.read_text()

if 'pub skip_segment_dismissals:' not in text:
    text = once(
        text,
        '    pub skip_segment_dismissal: Option<(u64, SkipSegmentKind, u64, u64)>,\n',
        '    pub skip_segment_dismissal: Option<(u64, SkipSegmentKind, u64, u64)>,\n'
        '    /// Consumed segments for this playback lifecycle only, bounded to 64 entries.\n'
        '    #[serde(skip_serializing)]\n'
        '    pub skip_segment_dismissals: Vec<(u64, SkipSegmentKind, u64, u64)>,\n',
    )

if 'self.skip_segment_dismissals.clear();' not in text:
    text = once(
        text,
        '                self.skip_segment_dismissal = None;\n',
        '                self.skip_segment_dismissal = None;\n'
        '                self.skip_segment_dismissals.clear();\n',
    )

history_block = '''        // Preserve earlier consumed segments when a later segment is dismissed.
        // Invalid dismissal messages cannot insert a new identity.
        if matches!(
            msg,
            Msg::Action(Action::Player(
                ActionPlayer::DismissSkipSegment { .. } | ActionPlayer::DismissSkipIntro { .. }
            ))
        ) {
            if let Some(identity) = self.skip_segment_dismissal {
                if self.skip_segment_dismissals.len() < 64
                    && !self.skip_segment_dismissals.contains(&identity)
                {
                    self.skip_segment_dismissals.push(identity);
                }
            }
        }
'''
if history_block not in text:
    text = once(text, '        let skip_segment = self\n', history_block + '        let skip_segment = self\n')

identity_block = '''                .and_then(|segment| {
                    let identity = (
                        self.playback_generation,
                        segment.kind,
                        segment.from_ms,
                        segment.to_ms,
                    );
                    let dismissal = if self.skip_segment_dismissals.contains(&identity)
                        || self.skip_segment_dismissals.len() >= 64
                    {
                        Some(identity)
                    } else {
                        self.skip_segment_dismissal
                    };
                    crate::models::skip_intro::state(
'''
if 'let dismissal = if self.skip_segment_dismissals.contains(&identity)' not in text:
    text = once(
        text,
        '''                .and_then(|segment| {
                    crate::models::skip_intro::state(
''',
        identity_block,
    )
    text = once(
        text,
        '''                        &ctx.profile.settings.skip_intro_mode,
                        self.skip_segment_dismissal,
                    )
''',
        '''                        &ctx.profile.settings.skip_intro_mode,
                        dismissal,
                    )
''',
    )

path.write_text(text)
print('Integrated current development skip delivery hardening')
