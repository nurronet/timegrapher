#!/usr/bin/env python3
"""Check retained tap offsets; this does not calibrate phone image/audio latency."""

import argparse
from pathlib import Path
import subprocess
import tempfile

import numpy as np
from scipy.io import wavfile
from scipy.signal import butter, correlate, sosfiltfilt

from reproduce import EVIDENCE, check_source, read


def envelope(path):
    rate, samples = wavfile.read(path)
    assert rate == 48000
    samples = samples.astype(float)
    if samples.ndim > 1:
        samples = samples.mean(1)
    samples /= abs(samples).max()
    filtered = sosfiltfilt(butter(4, [2000, 12000], btype="bandpass", fs=rate,
                                output="sos"), samples)
    return np.sqrt((filtered[:len(filtered) // 48 * 48].reshape(-1, 48) ** 2).mean(1))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--take", required=True, choices=("high", "low", "intermediate", "final"))
    parser.add_argument("--video", type=Path, required=True)
    parser.add_argument("--audio", type=Path, required=True)
    args = parser.parse_args()
    config = next(t for t in read(EVIDENCE / "manifest.json")["takes"] if t["id"] == args.take)
    check_source(args.video, config, "video.mov")
    check_source(args.audio, config, "audio-take/audio-001.wav")
    sync = read(EVIDENCE / args.take / "audio-video-comparison.json")["sync"]
    taps = sync.get("phone_taps_s", [sync.get("phone_tap_s")])
    with tempfile.TemporaryDirectory() as directory:
        phone = Path(directory) / "phone.wav"
        subprocess.run(["ffmpeg", "-v", "error", "-i", str(args.video), "-vn",
                        "-ar", "48000", str(phone)], check=True)
        phone_env, mic_env = envelope(phone), envelope(args.audio)
    offset = config["mic_minus_video_s"]
    for tap in taps:
        start, end = round((tap - .1) * 1000), round((tap + .2) * 1000)
        lower, upper = round((offset - .2) * 1000), round((offset + .2) * 1000)
        scores = correlate(mic_env[start + lower:end + upper], phone_env[start:end],
                           mode="valid")
        measured = (lower + np.argmax(scores)) / 1000
        print(f"Phone tap {tap:.3f} s: mic-minus-phone {measured:.3f} s")
        assert abs(measured - offset) <= .003


if __name__ == "__main__":
    main()
