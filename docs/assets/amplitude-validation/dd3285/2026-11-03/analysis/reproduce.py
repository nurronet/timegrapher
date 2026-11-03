#!/usr/bin/env python3
"""Replay frozen DD3285 excerpts; never revise the committed blind estimates."""

import argparse
import csv
import hashlib
import json
from pathlib import Path
import subprocess

import numpy as np
from scipy.io import wavfile

from edges import measure
from marker import measure_mark

EVIDENCE = Path(__file__).resolve().parent.parent


def read(path):
    return json.loads(path.read_text())


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_source(path, config, role):
    expected = next(s for s in config["sources"] if s["role"] == role)
    if sha256(path) != expected["sha256"]:
        raise ValueError(f"Source hash mismatch: {role}")


def edge_mean(result):
    """The two edge means have equal weight; reject remote quadratic vertices."""
    values = np.array(result["amplitudes"])
    means = []
    for edge in (0, 1):
        keep = []
        for pair in range(len(values)):
            turns = [result["turns"][2 * pair + j] for j in (0, 1)]
            if all(abs(t["vertex_frames"][edge] - t["nominal_frame"]) <= 2.5
                   for t in turns):
                keep.append(values[pair, edge])
        means.append(np.mean(keep))
    return float(np.mean(means))


def stored_results(folder, config):
    if config["method"] == "marker":
        return read(folder / "black-mark-endpoints.json")
    return [r for r in read(folder / "edge-checks.json") if r["feature"] == "bottom"]


def verify():
    manifest = read(EVIDENCE / "manifest.json")
    windows = 0
    for config in manifest["takes"]:
        folder = EVIDENCE / config["id"]
        assert sha256(folder / "blind-result.json") == config["blind_sha256"]
        blind = read(folder / "blind-result.json")
        video_hash = blind.get("source_video_sha256", blind.get("video_sha256"))
        assert video_hash == next(s["sha256"] for s in config["sources"] if s["role"] == "video.mov")
        results = stored_results(folder, config)
        for section in blind["sections"]:
            result = next(r for r in results if r["start_s"] == section["video_start_s"])
            mean = (float(np.mean(result["amplitudes"])) if config["method"] == "marker"
                    else edge_mean(result))
            expected = section.get("optical_amplitude_deg", section.get("marker_mean_deg"))
            assert abs(mean - expected) < 1e-8, (config["id"], mean, expected)
            assert round(mean) == section["rounded_amplitude_deg"]
            windows += 1
        for crop in config["native_frame_crops"]:
            assert sha256(folder / crop["file"]) == crop["sha256"]
        for row in read(folder / "matched-windows.json"):
            assert abs((row["tick_deg"] + row["tock_deg"]) / 2 - row["pair_mean_deg"]) < 1e-8
            start, lift = row["video_start_s"], row["lift_deg"]
            filename = (f"windows-{lift}.csv" if config["id"] == "low"
                        else f"windows-{lift}-{start % 2}.csv")
            with (folder / filename).open() as stream:
                original = next(r for r in csv.DictReader(stream) if r["kind"] == "amplitude"
                                and float(r["start_s"]) == start - start % 2)
            assert float(original["amplitude_even_deg"]) == row["tick_deg"]
            assert float(original["amplitude_odd_deg"]) == row["tock_deg"]
    print(f"Verified {windows} frozen optical windows, crop hashes and acoustic pair means.")


def frames(video, start, config):
    x, y = config["crop_xy"]
    raw = subprocess.check_output([
        "ffmpeg", "-v", "error", "-ss", str(start), "-i", str(video), "-t", "2",
        "-vf", f"crop=300:300:{x}:{y}", "-fps_mode", "passthrough",
        "-f", "rawvideo", "-pix_fmt", "rgb24", "-",
    ])
    return np.frombuffer(raw, np.uint8).reshape(-1, 300, 300, 3)


def optical(video, output, config):
    check_source(video, config, "video.mov")
    folder = EVIDENCE / config["id"]
    geometry = read(folder / "geometry.json")
    saved = stored_results(folder, config)
    reproduced = []
    for base in read(folder / "selection-fits.json"):
        start = base["start_s"]
        images = frames(video, start, config)
        rim = np.array(geometry[str(start)])
        if config["method"] == "marker":
            result = measure_mark(base, images, rim)
        else:
            result = measure(images, base, rim, limits=config["angle_limits"],
                             skip_early=config["first_turn_skip_early"])
        expected = next(r for r in saved if r["start_s"] == start)
        np.testing.assert_allclose(result["amplitudes"], expected["amplitudes"], atol=1e-7, rtol=0)
        reproduced.append(result)
    (output / "replayed-optical.json").write_text(
        json.dumps(reproduced, indent=2, default=lambda x: x.tolist()) + "\n")
    print(f"Reproduced {config['id']} endpoint amplitudes from source video.")


def audio(wav, cli, output, config):
    check_source(wav, config, "audio-take/audio-001.wav")
    rate, samples = wavfile.read(wav)
    expected = read(EVIDENCE / config["id"] / "matched-windows.json")
    for shift in sorted({s % 2 for s in config["starts_s"]}):
        aligned = output / f"aligned-{shift}.wav"
        cut = round((config["mic_minus_video_s"] + shift) * rate)
        wavfile.write(aligned, rate, samples[cut:])
        for lift in (55, 58, 59):
            windows = output / f"windows-{lift}-{shift}.csv"
            result = subprocess.check_output([
                str(cli), "analyze", str(aligned), "--bph", "28800", "--lift", str(lift),
                "--windows", str(windows), "--json",
            ])
            (output / f"analysis-{lift}-{shift}.json").write_bytes(result)
            rows = list(csv.DictReader(windows.open()))
            for item in expected:
                start = item["video_start_s"]
                if item["lift_deg"] != lift or start % 2 != shift:
                    continue
                row = next(r for r in rows if r["kind"] == "amplitude"
                           and float(r["start_s"]) == start - shift)
                assert float(row["amplitude_even_deg"]) == item["tick_deg"]
                assert float(row["amplitude_odd_deg"]) == item["tock_deg"]
    print(f"Reproduced {config['id']} matched Tick/Tock values at 55, 58 and 59 degrees.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--take", choices=("high", "low", "intermediate", "final"))
    parser.add_argument("--video", type=Path)
    parser.add_argument("--audio", type=Path)
    parser.add_argument("--cli", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    verify()
    if not (args.video or args.audio):
        return
    if not args.take or not args.output or (args.audio and not args.cli):
        parser.error("Replay needs --take, --output and --cli when --audio is supplied")
    destination = args.output.resolve()
    if destination == EVIDENCE or EVIDENCE in destination.parents:
        parser.error("Write replay outputs outside the evidence folder")
    destination.mkdir(parents=True, exist_ok=True)
    config = next(c for c in read(EVIDENCE / "manifest.json")["takes"] if c["id"] == args.take)
    if args.video:
        optical(args.video, destination, config)
    if args.audio:
        audio(args.audio, args.cli, destination, config)


if __name__ == "__main__":
    main()
