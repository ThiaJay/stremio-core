'use strict';

function normaliseCodec(codec) {
    return typeof codec === 'string' ? codec.trim().toLowerCase() : null;
}

function normaliseDolbyVisionProfile(profile) {
    if (Number.isInteger(profile) && profile >= 0) return profile;
    if (typeof profile !== 'string') return null;

    var value = profile.trim().toLowerCase();
    var codecMatch = value.match(/dv(?:he|h1)\.(\d{2})/);
    if (codecMatch) return Number.parseInt(codecMatch[1], 10);

    var profileMatch = value.match(/(?:profile\s*)?(\d{1,2})/);
    return profileMatch ? Number.parseInt(profileMatch[1], 10) : null;
}

function normaliseEnhancementLayer(value) {
    if (typeof value !== 'string') return null;
    var layer = value.trim().toLowerCase();
    return layer === 'mel' || layer === 'fel' ? layer : null;
}

function createPlaybackHealthObservation(input) {
    input = input || {};

    return {
        engine: typeof input.engine === 'string' ? input.engine : undefined,
        audioCodec: normaliseCodec(input.audioCodec) || undefined,
        videoCodec: normaliseCodec(input.videoCodec) || undefined,
        dolbyVisionProfile: normaliseDolbyVisionProfile(input.dolbyVisionProfile) || undefined,
        dolbyVisionEnhancementLayer: normaliseEnhancementLayer(input.dolbyVisionEnhancementLayer) || undefined,
        hdr10BaseLayer: input.hdr10BaseLayer === true,
        audioExpected: input.audioExpected === true,
        audioPresent: input.audioPresent === true,
        videoStable: input.videoStable !== false,
        videoFramePresent: input.videoFramePresent !== false,
        controlsResponsive: input.controlsResponsive !== false,
        startupTimedOut: input.startupTimedOut === true,
        transient: input.transient === true,
        canConvertDolbyVisionProfile7: input.canConvertDolbyVisionProfile7 === true,
        canFallbackHdr10: input.canFallbackHdr10 === true,
        canTranscodeAudio: input.canTranscodeAudio === true,
        canSwitchEngine: input.canSwitchEngine === true,
        canRestoreStableVideo: input.canRestoreStableVideo === true,
        canSwitchStream: input.canSwitchStream === true,
    };
}

function classifyPlaybackHealth(observation) {
    var value = createPlaybackHealthObservation(observation);

    if (value.transient) {
        return { healthy: false, transient: true, audioHealthy: null, videoHealthy: null };
    }

    var audioHealthy = !value.audioExpected || value.audioPresent;
    var videoHealthy = value.videoStable && value.videoFramePresent && value.controlsResponsive && !value.startupTimedOut;

    return {
        healthy: audioHealthy && videoHealthy,
        transient: false,
        audioHealthy: audioHealthy,
        videoHealthy: videoHealthy,
    };
}

function selectPlaybackRecovery(observation) {
    var value = createPlaybackHealthObservation(observation);
    var health = classifyPlaybackHealth(value);

    if (health.transient || health.healthy) {
        return null;
    }

    if (!health.videoHealthy && value.dolbyVisionProfile === 7) {
        if (value.canConvertDolbyVisionProfile7) return 'convertDolbyVisionProfile7To8_1';
        if (value.hdr10BaseLayer && value.canFallbackHdr10) return 'fallbackToHdr10BaseLayer';
    }

    if (!health.audioHealthy && health.videoHealthy) {
        if (value.canTranscodeAudio) return 'transcodeAudio';
        if (value.canSwitchEngine) return 'switchPlaybackEngine';
        if (value.canSwitchStream) return 'switchStream';
        return null;
    }

    if (health.audioHealthy && !health.videoHealthy) {
        if (value.canRestoreStableVideo && value.canTranscodeAudio) {
            return 'restoreStableVideoAndTranscodeAudio';
        }
        if (value.canSwitchStream) return 'switchStream';
        if (value.canSwitchEngine) return 'switchPlaybackEngine';
        return null;
    }

    if (value.canSwitchEngine) return 'switchPlaybackEngine';
    if (value.canSwitchStream) return 'switchStream';
    return null;
}

function createBackendCapabilityManifest(input) {
    input = input || {};
    return {
        engine: typeof input.engine === 'string' ? input.engine : 'unknown',
        observeAudioHealth: input.observeAudioHealth === true,
        observeVideoHealth: input.observeVideoHealth === true,
        observeStartupHealth: input.observeStartupHealth === true,
        convertDolbyVisionProfile7: input.convertDolbyVisionProfile7 === true,
        fallbackHdr10: input.fallbackHdr10 === true,
        transcodeAudio: input.transcodeAudio === true,
        switchEngine: input.switchEngine === true,
        restoreStableVideo: input.restoreStableVideo === true,
        switchStream: input.switchStream === true,
    };
}

module.exports = {
    normaliseDolbyVisionProfile,
    createPlaybackHealthObservation,
    classifyPlaybackHealth,
    selectPlaybackRecovery,
    createBackendCapabilityManifest,
};
