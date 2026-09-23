#!/usr/bin/env bash
set -euo pipefail

mkdir -p evidence/android-mobile-media3
EVIDENCE=evidence/android-mobile-media3
APK=.media/demos/main/buildout/outputs/apk/noDecoderExtensions/debug/demo-noDecoderExtensions-debug.apk
FIXTURE=mobile-runtime-smoke.mp4

if [[ ! -s "$APK" ]]; then
  echo "Missing demo APK: $APK" >&2
  exit 1
fi
if [[ ! -s "$FIXTURE" ]]; then
  echo "Missing deterministic media fixture: $FIXTURE" >&2
  exit 1
fi

AAPT="$(find "${ANDROID_SDK_ROOT:-$ANDROID_HOME}/build-tools" -type f -name aapt -print | sort -V | tail -n 1)"
if [[ -z "$AAPT" || ! -x "$AAPT" ]]; then
  echo "Unable to locate aapt" >&2
  exit 1
fi
PACKAGE="$($AAPT dump badging "$APK" | sed -n "s/^package: name='\([^']*\)'.*/\1/p" | head -n 1)"
if [[ -z "$PACKAGE" ]]; then
  echo "Unable to derive application id" >&2
  exit 1
fi
ACTIVITY="$PACKAGE/.PlayerActivity"
printf '%s\n' "$PACKAGE" > "$EVIDENCE/application-id.txt"

adb wait-for-device
adb shell getprop ro.build.version.sdk | tee "$EVIDENCE/android-sdk.txt"
adb shell getprop ro.product.model | tee "$EVIDENCE/device-model.txt"
adb shell getprop ro.product.cpu.abi | tee "$EVIDENCE/device-abi.txt"
adb install -r "$APK" | tee "$EVIDENCE/install.txt"
adb shell pm list packages | grep -F "package:$PACKAGE" | tee "$EVIDENCE/package.txt"

adb shell pm grant "$PACKAGE" android.permission.READ_MEDIA_VIDEO >"$EVIDENCE/permission.txt" 2>&1 || true
adb push "$FIXTURE" /sdcard/Download/mobile-runtime-smoke.mp4 | tee "$EVIDENCE/push.txt"

adb logcat -c
adb shell am force-stop "$PACKAGE"
adb shell am start -W \
  -a androidx.media3.demo.main.action.VIEW \
  -d file:///sdcard/Download/mobile-runtime-smoke.mp4 \
  -n "$ACTIVITY" | tee "$EVIDENCE/am-start.txt"

sleep 3
adb shell dumpsys activity activities > "$EVIDENCE/activity.txt"
grep -F "$PACKAGE/.PlayerActivity" "$EVIDENCE/activity.txt" > "$EVIDENCE/activity-match.txt"

sleep 10
adb logcat -d -v threadtime > "$EVIDENCE/logcat.txt"
adb shell dumpsys SurfaceFlinger --list > "$EVIDENCE/surfaceflinger.txt" || true
adb shell dumpsys media.codec > "$EVIDENCE/media-codec.txt" || true
adb exec-out screencap -p > "$EVIDENCE/final-screen.png" || true

python3 - <<'PY'
from pathlib import Path
import re

log = Path('evidence/android-mobile-media3/logcat.txt').read_text(errors='replace')
checks = {
    'state_ready': r'EventLogger.*state.*READY',
    'is_playing': r'EventLogger.*isPlaying.*true',
    'video_decoder': r'EventLogger.*videoDecoderInitialized',
    'first_frame': r'EventLogger.*renderedFirstFrame',
    'video_size': r'EventLogger.*videoSize.*w=',
    'audio_track': r'EventLogger.*audioTrackInit',
    'audio_advancing': r'EventLogger.*audioPositionAdvancing',
    'state_ended': r'EventLogger.*state.*ENDED',
}
missing = [name for name, pattern in checks.items() if re.search(pattern, log) is None]
Path('evidence/android-mobile-media3/assertions.txt').write_text(
    '\n'.join(f'{name}=' + ('PASS' if name not in missing else 'FAIL') for name in checks) + '\n'
)
if missing:
    raise SystemExit('Android mobile runtime evidence missing: ' + ', '.join(missing))
PY

cat "$EVIDENCE/assertions.txt"
