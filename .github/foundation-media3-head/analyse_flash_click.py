from __future__ import annotations

from pathlib import Path
import argparse
import hashlib
import json
import re
import subprocess
import tempfile
import wave

import numpy as np


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def parse_crop(value: str | None) -> str | None:
    if not value:
        return None
    if not re.fullmatch(r"\d+:\d+:\d+:\d+", value):
        raise ValueError("--crop must be numeric w:h:x:y")
    return value


def collect_video_luma(recording: Path, crop: str | None):
    filters = []
    if crop:
        filters.append(f"crop={crop}")
    filters.extend(["signalstats", "metadata=print:file=-"])
    proc = subprocess.run(
        [
            "ffmpeg",
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            str(recording),
            "-vf",
            ",".join(filters),
            "-an",
            "-f",
            "null",
            "-",
        ],
        check=True,
        capture_output=True,
        text=True,
    )

    times = []
    luma = []
    current_time = None
    frame_re = re.compile(r"\bpts_time:([-+0-9.eE]+)")
    for line in proc.stdout.splitlines():
        if line.startswith("frame:"):
            match = frame_re.search(line)
            current_time = float(match.group(1)) if match else None
        elif current_time is not None and line.startswith("lavfi.signalstats.YAVG="):
            times.append(current_time)
            luma.append(float(line.rsplit("=", 1)[1]))
            current_time = None

    t = np.asarray(times, dtype=np.float64)
    y = np.asarray(luma, dtype=np.float64)
    if len(t) < 12 or len(y) != len(t):
        raise RuntimeError("Insufficient video frame metadata for physical sync analysis")
    if np.any(np.diff(t) < 0):
        raise RuntimeError("Video frame timestamps are not monotonic")
    return t, y


def rising_events(times: np.ndarray, values: np.ndarray, threshold: float, min_gap: float):
    crossing = np.flatnonzero((values[1:] >= threshold) & (values[:-1] < threshold)) + 1
    events = []
    for idx in crossing:
        ts = float(times[idx])
        if not events or ts - events[-1] >= min_gap:
            events.append(ts)
    return np.asarray(events, dtype=np.float64)


def audio_click_times(audio: np.ndarray, rate: int, min_gap: float):
    if len(audio) < rate:
        raise RuntimeError("Recording audio is too short for physical sync analysis")
    abs_audio = np.abs(audio.astype(np.float64))
    window = max(1, int(round(rate * 0.005)))
    csum = np.concatenate(([0.0], np.cumsum(abs_audio)))
    smooth = (csum[window:] - csum[:-window]) / window
    smooth_times = (np.arange(len(smooth), dtype=np.float64) + window / 2.0) / rate

    baseline = float(np.percentile(smooth, 50))
    high = float(np.percentile(smooth, 99.7))
    span = high - baseline
    if span < 100:
        raise RuntimeError("Audio click contrast is too weak for reliable physical sync analysis")
    threshold = baseline + max(span * 0.35, 250.0)
    return rising_events(smooth_times, smooth, threshold, min_gap), threshold, baseline, high


def pair_events(flash_times: np.ndarray, click_times: np.ndarray, window: float):
    pairs = []
    used = set()
    for ft in flash_times:
        candidates = [
            i
            for i, ct in enumerate(click_times)
            if i not in used and abs(float(ct) - float(ft)) <= window
        ]
        if not candidates:
            continue
        idx = min(candidates, key=lambda i: abs(float(click_times[i]) - float(ft)))
        used.add(idx)
        ct = float(click_times[idx])
        pairs.append((float(ft), ct, ct - float(ft)))
    return pairs


def main() -> int:
    p = argparse.ArgumentParser()
    p.add_argument("recording")
    p.add_argument("--fps", type=float, default=None, help="Optional expected camera frame rate for provenance only")
    p.add_argument("--crop", default=None, help="Optional numeric TV region as w:h:x:y")
    p.add_argument("--pair-window-ms", type=float, default=250.0)
    p.add_argument("--out", default="physical-sync-result.json")
    args = p.parse_args()

    recording = Path(args.recording).resolve()
    if not recording.is_file():
        raise FileNotFoundError(recording)
    crop = parse_crop(args.crop)
    if args.pair_window_ms <= 0 or args.pair_window_ms >= 500:
        raise ValueError("--pair-window-ms must be greater than 0 and less than 500")

    with tempfile.TemporaryDirectory() as td_value:
        td = Path(td_value)
        wav = td / "audio.wav"
        subprocess.run(
            [
                "ffmpeg",
                "-y",
                "-hide_banner",
                "-loglevel",
                "error",
                "-i",
                str(recording),
                "-vn",
                "-ac",
                "1",
                "-ar",
                "48000",
                "-c:a",
                "pcm_s16le",
                str(wav),
            ],
            check=True,
        )

        frame_times, brightness = collect_video_luma(recording, crop)
        with wave.open(str(wav), "rb") as w:
            audio = np.frombuffer(w.readframes(w.getnframes()), dtype=np.int16)
            rate = w.getframerate()

    low = float(np.percentile(brightness, 10))
    high = float(np.percentile(brightness, 99))
    span = high - low
    if span < 8:
        raise RuntimeError("Video flash contrast is too weak for reliable physical sync analysis")
    flash_threshold = low + span * 0.5
    flash_times = rising_events(frame_times, brightness, flash_threshold, 0.5)
    click_times, audio_threshold, audio_baseline, audio_high = audio_click_times(audio, rate, 0.5)

    pairs = pair_events(flash_times, click_times, args.pair_window_ms / 1000.0)
    if len(pairs) < 5:
        raise RuntimeError(
            f"Only {len(pairs)} flash/click pairs matched; at least 5 are required for physical sync analysis"
        )

    pair_array = np.asarray(pairs, dtype=np.float64)
    pair_video_times = pair_array[:, 0]
    ms = pair_array[:, 2] * 1000.0
    if len(ms) >= 2 and np.ptp(pair_video_times) > 0:
        slope_ms_per_s, intercept_ms = np.polyfit(pair_video_times, ms, 1)
        fitted_start = slope_ms_per_s * pair_video_times[0] + intercept_ms
        fitted_end = slope_ms_per_s * pair_video_times[-1] + intercept_ms
        fitted_drift = float(fitted_end - fitted_start)
    else:
        slope_ms_per_s = 0.0
        fitted_drift = 0.0

    frame_deltas = np.diff(frame_times)
    positive_frame_deltas = frame_deltas[frame_deltas > 0]
    measured_fps = float(1.0 / np.median(positive_frame_deltas)) if len(positive_frame_deltas) else None

    result = {
        "schemaVersion": 2,
        "recordingSha256": sha256_file(recording),
        "analyserSha256": sha256_file(Path(__file__).resolve()),
        "pairs": int(len(ms)),
        "flashEvents": int(len(flash_times)),
        "clickEvents": int(len(click_times)),
        "medianOffsetMs": float(np.median(ms)),
        "p95AbsoluteOffsetMs": float(np.percentile(np.abs(ms), 95)),
        "minOffsetMs": float(ms.min()),
        "maxOffsetMs": float(ms.max()),
        "fittedProgressiveDriftMs": fitted_drift,
        "driftRateMsPerMinute": float(slope_ms_per_s * 60.0),
        "firstMatchedVideoTimeSeconds": float(pair_video_times[0]),
        "lastMatchedVideoTimeSeconds": float(pair_video_times[-1]),
        "measuredCameraFps": measured_fps,
        "expectedCameraFps": args.fps,
        "crop": crop,
        "pairWindowMs": float(args.pair_window_ms),
        "videoFlashThresholdY": float(flash_threshold),
        "videoLumaLow": low,
        "videoLumaHigh": high,
        "audioThreshold": float(audio_threshold),
        "audioBaseline": float(audio_baseline),
        "audioHigh": float(audio_high),
        "physicalMeasurementCompleted": True,
        "acceptanceInferred": False,
        "note": "Physical flash/click measurements only. Apply the project acceptance thresholds separately and preserve output-path context.",
    }
    Path(args.out).write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
