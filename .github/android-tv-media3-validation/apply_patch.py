from pathlib import Path

ROOT = Path(".media")

def replace_once(path, before, after):
    file = ROOT / path
    text = file.read_text()
    count = text.count(before)
    if count != 1:
        raise RuntimeError(f"{path}: expected exactly one source anchor, found {count}")
    file.write_text(text.replace(before, after, 1))

replace_once(
    "libraries/common/src/main/java/androidx/media3/common/C.java",
    "@IntDef({VIDEO_CHANGE_FRAME_RATE_STRATEGY_OFF, VIDEO_CHANGE_FRAME_RATE_STRATEGY_ONLY_IF_SEAMLESS})",
    """@IntDef({
    VIDEO_CHANGE_FRAME_RATE_STRATEGY_OFF,
    VIDEO_CHANGE_FRAME_RATE_STRATEGY_ONLY_IF_SEAMLESS,
    VIDEO_CHANGE_FRAME_RATE_STRATEGY_ALWAYS
  })""",
)
replace_once(
    "libraries/common/src/main/java/androidx/media3/common/C.java",
    """  @UnstableApi
  public static final int VIDEO_CHANGE_FRAME_RATE_STRATEGY_ONLY_IF_SEAMLESS =
      Surface.CHANGE_FRAME_RATE_ONLY_IF_SEAMLESS;
""",
    """  @UnstableApi
  public static final int VIDEO_CHANGE_FRAME_RATE_STRATEGY_ONLY_IF_SEAMLESS =
      Surface.CHANGE_FRAME_RATE_ONLY_IF_SEAMLESS;

  /**
   * Strategy to allow a non-seamless display refresh-rate switch when required to match the
   * video's exact frame rate. On API 30 this falls back to the two-argument platform call.
   */
  @UnstableApi
  public static final int VIDEO_CHANGE_FRAME_RATE_STRATEGY_ALWAYS = Surface.CHANGE_FRAME_RATE_ALWAYS;
""",
)

replace_once(
    "libraries/exoplayer/src/main/java/androidx/media3/exoplayer/ExoPlayer.java",
    """     * <p>The strategy only applies if a {@link MediaCodec}-based video {@link Renderer} is enabled.
     * Applications wishing to use {@link Surface#CHANGE_FRAME_RATE_ALWAYS} should set the mode to
     * {@link C#VIDEO_CHANGE_FRAME_RATE_STRATEGY_OFF} to disable calls to {@link
     * Surface#setFrameRate} from ExoPlayer, and should then call {@link Surface#setFrameRate}
     * directly from application code.
""",
    """     * <p>The strategy only applies if a {@link MediaCodec}-based video {@link Renderer} is enabled.
     * Applications that allow non-seamless display mode changes can use {@link
     * C#VIDEO_CHANGE_FRAME_RATE_STRATEGY_ALWAYS}. The exact fractional media rate is passed through
     * to the platform rather than being rounded by application code.
""",
)

helper = "libraries/exoplayer/src/main/java/androidx/media3/exoplayer/video/VideoFrameReleaseHelper.java"
replace_once(
    helper,
    "  @VisibleForTesting public static final long VSYNC_SAMPLE_UPDATE_PERIOD_MS = 500;\n",
    """  @VisibleForTesting public static final long VSYNC_SAMPLE_UPDATE_PERIOD_MS = 500;

  /** Maximum time playback is held while a requested non-seamless display switch settles. */
  @VisibleForTesting static final long FRAME_RATE_SWITCH_SETTLE_TIMEOUT_NS = 1_000_000_000L;

  /**
   * Maximum deviation of display/media refresh ratio from an integer multiple. This is tight
   * enough to distinguish 23.976 from 24.000 and 29.97 from 30.000.
   */
  @VisibleForTesting static final double FRAME_RATE_MULTIPLE_TOLERANCE = 0.0005d;
""",
)
replace_once(
    helper,
    "  private @C.VideoChangeFrameRateStrategy int changeFrameRateStrategy;\n",
    """  private @C.VideoChangeFrameRateStrategy int changeFrameRateStrategy;
  private boolean surfaceFrameRateSwitchPending;
  private long surfaceFrameRateSwitchStartNs = C.TIME_UNSET;
""",
)
replace_once(
    helper,
    """    this.surfacePlaybackFrameRate = surfacePlaybackFrameRate;
    Api30.setSurfaceFrameRate(surface, surfacePlaybackFrameRate);
""",
    """    this.surfacePlaybackFrameRate = surfacePlaybackFrameRate;
    Api30.setSurfaceFrameRate(surface, surfacePlaybackFrameRate, changeFrameRateStrategy);
    surfaceFrameRateSwitchPending =
        SDK_INT >= 31
            && changeFrameRateStrategy == C.VIDEO_CHANGE_FRAME_RATE_STRATEGY_ALWAYS
            && surfacePlaybackFrameRate > 0;
    surfaceFrameRateSwitchStartNs = C.TIME_UNSET;
""",
)
replace_once(
    helper,
    """    surfacePlaybackFrameRate = 0;
    Api30.setSurfaceFrameRate(surface, /* frameRate= */ 0);
""",
    """    surfacePlaybackFrameRate = 0;
    surfaceFrameRateSwitchPending = false;
    surfaceFrameRateSwitchStartNs = C.TIME_UNSET;
    Api30.setSurfaceFrameRate(surface, /* frameRate= */ 0, changeFrameRateStrategy);
""",
)
replace_once(
    helper,
    "  private long findClosestVsyncAndUpdateHysteresis(\n",
    """  /**
   * Returns whether frame release should wait briefly for a requested non-seamless display mode
   * change. The wait is bounded and fails open if the display cannot provide the requested mode.
   */
  boolean isSurfaceFrameRateSwitching(long nowNs) {
    long vsyncDurationNs =
        vsyncSampler == null ? C.TIME_UNSET : vsyncSampler.vsyncDurationNs;
    return isSurfaceFrameRateSwitching(nowNs, vsyncDurationNs);
  }

  @VisibleForTesting
  boolean isSurfaceFrameRateSwitching(long nowNs, long vsyncDurationNs) {
    if (!surfaceFrameRateSwitchPending || surfacePlaybackFrameRate <= 0) {
      return false;
    }
    if (isDisplayRefreshRateCompatible(surfacePlaybackFrameRate, vsyncDurationNs)) {
      surfaceFrameRateSwitchPending = false;
      surfaceFrameRateSwitchStartNs = C.TIME_UNSET;
      return false;
    }
    if (surfaceFrameRateSwitchStartNs == C.TIME_UNSET) {
      surfaceFrameRateSwitchStartNs = nowNs;
      return true;
    }
    if (nowNs < surfaceFrameRateSwitchStartNs
        || nowNs - surfaceFrameRateSwitchStartNs >= FRAME_RATE_SWITCH_SETTLE_TIMEOUT_NS) {
      surfaceFrameRateSwitchPending = false;
      surfaceFrameRateSwitchStartNs = C.TIME_UNSET;
      return false;
    }
    return true;
  }

  @VisibleForTesting
  void setSurfaceFrameRateSwitchPendingForTest(float requestedFrameRate) {
    surfacePlaybackFrameRate = requestedFrameRate;
    surfaceFrameRateSwitchPending = requestedFrameRate > 0;
    surfaceFrameRateSwitchStartNs = C.TIME_UNSET;
  }

  @VisibleForTesting
  static boolean isDisplayRefreshRateCompatible(float mediaFrameRate, long vsyncDurationNs) {
    if (mediaFrameRate <= 0 || vsyncDurationNs <= 0 || vsyncDurationNs == C.TIME_UNSET) {
      return false;
    }
    double displayRefreshRate = (double) C.NANOS_PER_SECOND / vsyncDurationNs;
    double ratio = displayRefreshRate / mediaFrameRate;
    long nearestMultiple = Math.round(ratio);
    return nearestMultiple >= 1
        && nearestMultiple <= 8
        && abs(ratio - nearestMultiple) <= FRAME_RATE_MULTIPLE_TOLERANCE;
  }

  private long findClosestVsyncAndUpdateHysteresis(
""",
)
old_api = """  @RequiresApi(30)
  private static final class Api30 {
    public static void setSurfaceFrameRate(Surface surface, float frameRate) {
      int compatibility =
          frameRate == 0
              ? Surface.FRAME_RATE_COMPATIBILITY_DEFAULT
              : Surface.FRAME_RATE_COMPATIBILITY_FIXED_SOURCE;
      try {
        surface.setFrameRate(frameRate, compatibility);
      } catch (IllegalStateException e) {
        Log.e(TAG, "Failed to call Surface.setFrameRate", e);
      }
    }
  }
"""
new_api = """  @RequiresApi(30)
  private static final class Api30 {
    public static void setSurfaceFrameRate(
        Surface surface,
        float frameRate,
        @C.VideoChangeFrameRateStrategy int changeFrameRateStrategy) {
      int compatibility =
          frameRate == 0
              ? Surface.FRAME_RATE_COMPATIBILITY_DEFAULT
              : Surface.FRAME_RATE_COMPATIBILITY_FIXED_SOURCE;
      try {
        if (SDK_INT >= 31) {
          Api31.setSurfaceFrameRate(
              surface, frameRate, compatibility, changeFrameRateStrategy);
        } else {
          surface.setFrameRate(frameRate, compatibility);
        }
      } catch (IllegalStateException e) {
        Log.e(TAG, "Failed to call Surface.setFrameRate", e);
      }
    }
  }

  @RequiresApi(31)
  private static final class Api31 {
    public static void setSurfaceFrameRate(
        Surface surface,
        float frameRate,
        int compatibility,
        @C.VideoChangeFrameRateStrategy int changeFrameRateStrategy) {
      surface.setFrameRate(frameRate, compatibility, changeFrameRateStrategy);
    }
  }
"""
replace_once(helper, old_api, new_api)

replace_once(
    "libraries/exoplayer/src/main/java/androidx/media3/exoplayer/video/VideoFrameReleaseControl.java",
    """    if (shouldForceRelease(positionUs, frameReleaseInfo.earlyUs, outputStreamStartPositionUs)) {
      return FRAME_RELEASE_IMMEDIATELY;
    }
""",
    """    if (frameReleaseHelper.isSurfaceFrameRateSwitching(clock.nanoTime())) {
      return FRAME_RELEASE_TRY_AGAIN_LATER;
    }
    if (shouldForceRelease(positionUs, frameReleaseInfo.earlyUs, outputStreamStartPositionUs)) {
      return FRAME_RELEASE_IMMEDIATELY;
    }
""",
)

replace_once(
    "demos/main/src/main/java/androidx/media3/demo/main/PlayerActivity.java",
    """      ExoPlayer.Builder playerBuilder =
          new ExoPlayer.Builder(/* context= */ this)
              .setMediaSourceFactory(createMediaSourceFactory());
""",
    """      ExoPlayer.Builder playerBuilder =
          new ExoPlayer.Builder(/* context= */ this)
              .setVideoChangeFrameRateStrategy(C.VIDEO_CHANGE_FRAME_RATE_STRATEGY_ALWAYS)
              .setMediaSourceFactory(createMediaSourceFactory());
""",
)

test_file = ROOT / "libraries/exoplayer/src/test/java/androidx/media3/exoplayer/video/StremioFrameRateSwitchTest.java"
test_file.write_text(r'''/*
 * Copyright 2026
 * Licensed under the Apache License, Version 2.0.
 */
package androidx.media3.exoplayer.video;

import static com.google.common.truth.Truth.assertThat;

import android.content.Context;
import android.view.Surface;
import androidx.media3.common.C;
import androidx.test.core.app.ApplicationProvider;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import org.junit.Test;
import org.junit.runner.RunWith;

@RunWith(AndroidJUnit4.class)
public final class StremioFrameRateSwitchTest {
  private static long vsyncDurationNs(double refreshRate) {
    return Math.round(C.NANOS_PER_SECOND / refreshRate);
  }

  @Test
  public void alwaysStrategy_mapsToPlatformAlways() {
    assertThat(C.VIDEO_CHANGE_FRAME_RATE_STRATEGY_ALWAYS)
        .isEqualTo(Surface.CHANGE_FRAME_RATE_ALWAYS);
  }

  @Test
  public void fractionalRates_areNotAcceptedAsRoundedIntegerRates() {
    assertThat(
            VideoFrameReleaseHelper.isDisplayRefreshRateCompatible(
                24_000f / 1_001f, vsyncDurationNs(24d)))
        .isFalse();
    assertThat(
            VideoFrameReleaseHelper.isDisplayRefreshRateCompatible(
                30_000f / 1_001f, vsyncDurationNs(30d)))
        .isFalse();
  }

  @Test
  public void exactFractionalMultiples_areAccepted() {
    assertThat(
            VideoFrameReleaseHelper.isDisplayRefreshRateCompatible(
                24_000f / 1_001f, vsyncDurationNs(120_000d / 1_001d)))
        .isTrue();
    assertThat(
            VideoFrameReleaseHelper.isDisplayRefreshRateCompatible(
                30_000f / 1_001f, vsyncDurationNs(60_000d / 1_001d)))
        .isTrue();
    assertThat(
            VideoFrameReleaseHelper.isDisplayRefreshRateCompatible(
                25f, vsyncDurationNs(50d)))
        .isTrue();
    assertThat(
            VideoFrameReleaseHelper.isDisplayRefreshRateCompatible(
                24f, vsyncDurationNs(120d)))
        .isTrue();
  }

  @Test
  public void incompatibleDisplay_waitsOnlyForBoundedSettlementWindow() {
    Context context = ApplicationProvider.getApplicationContext();
    VideoFrameReleaseHelper helper = new VideoFrameReleaseHelper(context);
    helper.setSurfaceFrameRateSwitchPendingForTest(24_000f / 1_001f);

    long startNs = 1_000_000L;
    assertThat(helper.isSurfaceFrameRateSwitching(startNs, vsyncDurationNs(60d))).isTrue();
    assertThat(
            helper.isSurfaceFrameRateSwitching(
                startNs + VideoFrameReleaseHelper.FRAME_RATE_SWITCH_SETTLE_TIMEOUT_NS - 1,
                vsyncDurationNs(60d)))
        .isTrue();
    assertThat(
            helper.isSurfaceFrameRateSwitching(
                startNs + VideoFrameReleaseHelper.FRAME_RATE_SWITCH_SETTLE_TIMEOUT_NS,
                vsyncDurationNs(60d)))
        .isFalse();
  }

  @Test
  public void compatibleDisplay_releasesImmediatelyAfterObservedSwitch() {
    Context context = ApplicationProvider.getApplicationContext();
    VideoFrameReleaseHelper helper = new VideoFrameReleaseHelper(context);
    helper.setSurfaceFrameRateSwitchPendingForTest(24_000f / 1_001f);

    long startNs = 1_000_000L;
    assertThat(helper.isSurfaceFrameRateSwitching(startNs, vsyncDurationNs(60d))).isTrue();
    assertThat(
            helper.isSurfaceFrameRateSwitching(
                startNs + 100_000_000L, vsyncDurationNs(24_000d / 1_001d)))
        .isFalse();
  }

  @Test
  public void commonIntegerMismatch_isNotMistakenForExactCadence() {
    assertThat(
            VideoFrameReleaseHelper.isDisplayRefreshRateCompatible(
                50f, vsyncDurationNs(60d)))
        .isFalse();
    assertThat(
            VideoFrameReleaseHelper.isDisplayRefreshRateCompatible(
                24f, vsyncDurationNs(60d)))
        .isFalse();
  }
}
''')

print("Applied exact fractional frame-rate and bounded non-seamless switch candidate")
