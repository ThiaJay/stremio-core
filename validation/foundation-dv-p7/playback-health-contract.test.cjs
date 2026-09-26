const test = require('node:test');
const assert = require('node:assert/strict');

const {
    normaliseDolbyVisionProfile,
    createPlaybackHealthObservation,
    classifyPlaybackHealth,
    selectPlaybackRecovery,
    createBackendCapabilityManifest,
} = require('./playback-health-contract.cjs');

test('silent DTS with stable video prefers audio compatibility over a player switch', () => {
    const observation = createPlaybackHealthObservation({
        engine: 'ExoPlayer',
        audioCodec: 'DTS',
        audioExpected: true,
        audioPresent: false,
        videoStable: true,
        canTranscodeAudio: true,
        canSwitchEngine: true,
        canSwitchStream: true,
    });

    assert.equal(observation.audioCodec, 'dts');
    assert.equal(selectPlaybackRecovery(observation), 'transcodeAudio');
});

test('silent DTS falls back to another engine when no audio compatibility path exists', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        audioCodec: 'dts',
        audioExpected: true,
        audioPresent: false,
        videoStable: true,
        canTranscodeAudio: false,
        canSwitchEngine: true,
        canSwitchStream: true,
    }), 'switchPlaybackEngine');
});

test('VLC audio success with picture stutter is not accepted as healthy', () => {
    const observation = {
        engine: 'libVLC',
        audioCodec: 'dts',
        audioExpected: true,
        audioPresent: true,
        videoStable: false,
        canTranscodeAudio: true,
        canRestoreStableVideo: true,
        canSwitchEngine: true,
        canSwitchStream: true,
    };

    assert.deepEqual(classifyPlaybackHealth(observation), {
        healthy: false,
        transient: false,
        audioHealthy: true,
        videoHealthy: false,
    });
    assert.equal(
        selectPlaybackRecovery(observation),
        'restoreStableVideoAndTranscodeAudio'
    );
});

test('source switching remains behind restoration and audio compatibility', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'libVLC',
        audioExpected: true,
        audioPresent: true,
        videoStable: false,
        canTranscodeAudio: false,
        canRestoreStableVideo: false,
        canSwitchEngine: false,
        canSwitchStream: true,
    }), 'switchStream');
});

test('startup buffering seeking and refresh changes are transient rather than failures', () => {
    const health = classifyPlaybackHealth({
        engine: 'ExoPlayer',
        audioExpected: true,
        audioPresent: false,
        videoStable: false,
        transient: true,
        canSwitchEngine: true,
    });

    assert.equal(health.transient, true);
    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        audioExpected: true,
        audioPresent: false,
        videoStable: false,
        transient: true,
        canSwitchEngine: true,
    }), null);
});

test('unsupported backends fail closed instead of pretending to recover', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'HTMLVideo',
        audioExpected: true,
        audioPresent: false,
        videoStable: false,
    }), null);
});

test('backend capability manifests are explicit and do not infer support from engine names', () => {
    assert.deepEqual(createBackendCapabilityManifest({
        engine: 'libVLC',
        observeAudioHealth: true,
        observeVideoHealth: true,
        switchEngine: true,
    }), {
        engine: 'libVLC',
        observeAudioHealth: true,
        observeVideoHealth: true,
        observeStartupHealth: false,
        convertDolbyVisionProfile7: false,
        fallbackHdr10: false,
        transcodeAudio: false,
        switchEngine: true,
        restoreStableVideo: false,
        switchStream: false,
    });
});

for (const codec of ['dts', 'dts-hd', 'truehd', 'ac3', 'eac3']) {
    test(codec + ' uses the same audio-health policy', () => {
        assert.equal(selectPlaybackRecovery({
            engine: 'native',
            audioCodec: codec,
            audioExpected: true,
            audioPresent: false,
            videoStable: true,
            canTranscodeAudio: true,
            canSwitchEngine: true,
            canSwitchStream: true,
        }), 'transcodeAudio');
    });
}

test('combined audio and video failure may use a bounded engine fallback', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'native',
        audioExpected: true,
        audioPresent: false,
        videoStable: false,
        canSwitchEngine: true,
        canSwitchStream: true,
    }), 'switchPlaybackEngine');
});

test('healthy audio and stable video request no recovery', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'native',
        audioExpected: true,
        audioPresent: true,
        videoStable: true,
        canTranscodeAudio: true,
        canSwitchEngine: true,
        canSwitchStream: true,
    }), null);
});

test('Dolby Vision codec identifiers expose Profile 7 deterministically', () => {
    assert.equal(normaliseDolbyVisionProfile('dvhe.07.06'), 7);
    assert.equal(normaliseDolbyVisionProfile('Profile 7'), 7);
    assert.equal(normaliseDolbyVisionProfile(7), 7);
    assert.equal(normaliseDolbyVisionProfile('dvhe.08.06'), 8);
});

test('Profile 7 black screen prefers compatibility conversion before engine or source switching', () => {
    const observation = createPlaybackHealthObservation({
        engine: 'ExoPlayer',
        videoCodec: 'HEVC',
        dolbyVisionProfile: 'dvhe.07.06',
        dolbyVisionEnhancementLayer: 'FEL',
        hdr10BaseLayer: true,
        audioCodec: 'truehd',
        audioExpected: true,
        audioPresent: false,
        videoStable: false,
        videoFramePresent: false,
        controlsResponsive: false,
        startupTimedOut: true,
        canConvertDolbyVisionProfile7: true,
        canFallbackHdr10: true,
        canTranscodeAudio: true,
        canSwitchEngine: true,
        canSwitchStream: true,
    });

    assert.equal(observation.dolbyVisionProfile, 7);
    assert.equal(observation.dolbyVisionEnhancementLayer, 'fel');
    assert.equal(selectPlaybackRecovery(observation), 'convertDolbyVisionProfile7To8_1');
});

test('Profile 7 falls back to its HDR10 base layer when conversion is unavailable', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        dolbyVisionProfile: 7,
        hdr10BaseLayer: true,
        audioExpected: true,
        audioPresent: true,
        videoFramePresent: false,
        controlsResponsive: false,
        startupTimedOut: true,
        canConvertDolbyVisionProfile7: false,
        canFallbackHdr10: true,
        canSwitchEngine: true,
        canSwitchStream: true,
    }), 'fallbackToHdr10BaseLayer');
});

test('Profile 7 conversion is not triggered during transient player initialisation', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        dolbyVisionProfile: 7,
        videoFramePresent: false,
        controlsResponsive: false,
        transient: true,
        canConvertDolbyVisionProfile7: true,
    }), null);
});

test('healthy Profile 7 playback is left untouched', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        dolbyVisionProfile: 7,
        audioExpected: true,
        audioPresent: true,
        videoStable: true,
        videoFramePresent: true,
        controlsResponsive: true,
        canConvertDolbyVisionProfile7: true,
        canFallbackHdr10: true,
    }), null);
});

test('Profile 8 does not enter the Profile 7 conversion path', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        dolbyVisionProfile: 8,
        audioExpected: true,
        audioPresent: true,
        videoFramePresent: false,
        controlsResponsive: false,
        startupTimedOut: true,
        canConvertDolbyVisionProfile7: true,
        canSwitchStream: true,
    }), 'switchStream');
});

test('Profile 7 recovery waits for the bounded startup timeout even when transient is false', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        dolbyVisionProfile: 7,
        hdr10BaseLayer: true,
        videoStable: false,
        videoFramePresent: false,
        controlsResponsive: false,
        startupTimedOut: false,
        transient: false,
        canConvertDolbyVisionProfile7: true,
        canFallbackHdr10: true,
        canSwitchStream: true,
    }), null);
});

test('Profile 7 recovery budget advances from conversion to HDR10 and never loops conversion', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        dolbyVisionProfile: 7,
        hdr10BaseLayer: true,
        videoStable: false,
        videoFramePresent: false,
        controlsResponsive: false,
        startupTimedOut: true,
        profile7ConversionAttempted: true,
        canConvertDolbyVisionProfile7: true,
        canFallbackHdr10: true,
        canSwitchEngine: true,
        canSwitchStream: true,
    }), 'fallbackToHdr10BaseLayer');

    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        dolbyVisionProfile: 7,
        hdr10BaseLayer: true,
        videoStable: false,
        videoFramePresent: false,
        controlsResponsive: false,
        startupTimedOut: true,
        profile7ConversionAttempted: true,
        hdr10FallbackAttempted: true,
        canConvertDolbyVisionProfile7: true,
        canFallbackHdr10: true,
        canSwitchEngine: true,
        canSwitchStream: true,
    }), 'switchStream');
});

test('a recovered Profile 7 presentation can become healthy after the startup timeout has fired', () => {
    const observation = {
        engine: 'ExoPlayer',
        dolbyVisionProfile: 7,
        audioExpected: true,
        audioPresent: true,
        videoStable: true,
        videoFramePresent: true,
        controlsResponsive: true,
        startupTimedOut: true,
        profile7ConversionAttempted: true,
        canConvertDolbyVisionProfile7: true,
        canFallbackHdr10: true,
        canSwitchEngine: true,
        canSwitchStream: true,
    };

    assert.deepEqual(classifyPlaybackHealth(observation), {
        healthy: true,
        transient: false,
        audioHealthy: true,
        videoHealthy: true,
    });
    assert.equal(selectPlaybackRecovery(observation), null);
});

test('Profile 7 recovered video routes a later audio failure through the audio policy', () => {
    assert.equal(selectPlaybackRecovery({
        engine: 'ExoPlayer',
        dolbyVisionProfile: 7,
        audioCodec: 'truehd',
        audioExpected: true,
        audioPresent: false,
        videoStable: true,
        videoFramePresent: true,
        controlsResponsive: true,
        startupTimedOut: true,
        profile7ConversionAttempted: true,
        canConvertDolbyVisionProfile7: true,
        canFallbackHdr10: true,
        canTranscodeAudio: true,
        canSwitchEngine: true,
        canSwitchStream: true,
    }), 'transcodeAudio');
});
