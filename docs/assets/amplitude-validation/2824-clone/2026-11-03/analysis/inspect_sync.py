#!/usr/bin/env python3
"""Inspect candidate matching transients without using amplitude measurements."""

import argparse
import json
from pathlib import Path
import subprocess

import numpy as np
from scipy.io import wavfile
from scipy.signal import butter, correlate, find_peaks, sosfiltfilt

from reproduce import check_source


def envelope(path):
    rate, samples = wavfile.read(path)
    samples = samples.astype(float)
    if samples.ndim > 1:
        samples = samples.mean(axis=1)
    samples /= np.max(np.abs(samples))
    filtered = sosfiltfilt(
        butter(4, [2000, 12000], btype="bandpass", fs=rate, output="sos"), samples
    )
    count = rate // 1000
    return np.sqrt(
        (filtered[: len(filtered) // count * count].reshape(-1, count) ** 2).mean(1)
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--video", required=True, type=Path)
    parser.add_argument("--mic", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    check_source(args.video, "IMG_5824.MOV")
    check_source(args.mic, "audio-001.wav")
    args.output.mkdir(parents=True, exist_ok=True)
    phone_file = args.output / "phone.wav"
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-y",
            "-i",
            str(args.video),
            "-vn",
            "-ac",
            "1",
            str(phone_file),
        ],
        check=True,
    )
    mic, phone = envelope(args.mic), envelope(phone_file)
    correlation = correlate(mic, phone, mode="full", method="fft")
    lag = np.arange(len(correlation)) - len(phone) + 1
    eligible = (lag > 0) & (lag < 8000)
    offset = int(lag[eligible][np.argmax(correlation[eligible])])
    mic_peaks, _ = find_peaks(mic, distance=500, prominence=mic.max() * 0.2)
    phone_peaks, _ = find_peaks(phone, distance=500, prominence=phone.max() * 0.2)
    pairs = []
    for peak in phone_peaks:
        matches = mic_peaks[np.abs(mic_peaks - peak - offset) < 20]
        if len(matches) == 1:
            match = int(matches[0])
            pairs.append(
                [float(peak / 1000), match / 1000, float((match - peak) / 1000)]
            )
    result = {"cross_correlation_offset_s": offset / 1000, "phone_mic_offset_s": pairs}
    (args.output / "sync.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
