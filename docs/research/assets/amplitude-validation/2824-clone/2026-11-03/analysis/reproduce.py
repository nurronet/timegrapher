#!/usr/bin/env python3
"""Replay the 2026-11-03 optical experiment, not a general amplitude estimator.

Requires ffmpeg/ffprobe on PATH. Uses pre-audio frame-selection seeds; no acoustic
measurement is used to select optical endpoints. Outputs go to a separate folder.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import numpy as np
from scipy.ndimage import map_coordinates, gaussian_filter1d
from scipy.optimize import least_squares
from scipy.special import erf

EVIDENCE = Path(__file__).resolve().parent.parent


def check_source(path, name):
    manifest = json.loads((EVIDENCE / "manifest.json").read_text())
    expected = next(x for x in manifest["sources"] if x["original_filename"] == name)
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    if digest.hexdigest() != expected["sha256"]:
        raise ValueError(f"Source hash mismatch: {name}")


def frames(video, start):
    raw = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-ss",
            str(start),
            "-i",
            str(video),
            "-t",
            "2",
            "-vf",
            "crop=320:300:150:440",
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ]
    )
    return np.frombuffer(raw, np.uint8).reshape(-1, 300, 320, 3)


def fit_rim(A):
    im = np.median(A[::10], axis=0)
    gold = (im[:, :, 0] + im[:, :, 1]) / 2 - im[:, :, 2]
    a = np.arange(-80, 181, 2)
    r = np.arange(105, 141, 0.2)
    x = 158 + np.sin(np.deg2rad(a))[:, None] * r
    y = 173 - np.cos(np.deg2rad(a))[:, None] * r
    p = map_coordinates(gold, [y, x], order=1)
    d = np.gradient(gaussian_filter1d(p, 3, axis=1), 0.2, axis=1)
    rs = []
    for row in d:
        rs.append(r[np.argmin(row)])
    rs = np.array(rs)
    xs = 158 + np.sin(np.deg2rad(a)) * rs
    ys = 173 - np.cos(np.deg2rad(a)) * rs
    # symmetric quadratic form: ellipse with freely oriented axes
    mask = ((a < 110) & (a > -72)) | ((a > 160) & (a < 178))

    def fun(v):
        cx, cy, qxx, qxy, qyy = v
        u = xs[mask] - cx
        w = ys[mask] - cy
        return (qxx * u * u + 2 * qxy * u * w + qyy * w * w - 1) * 120

    res = least_squares(
        fun,
        [158, 173, 1 / 125**2, 0, 1 / 125**2],
        loss="soft_l1",
        f_scale=1,
        x_scale="jac",
        max_nfev=2000,
    )
    return res.x


def measure(gold, M, cx, cy, first, period, target_max, target_min):
    r0, r1 = 90, 108
    ratio = 1
    angles = np.arange(-75, 95, 0.2)
    r = np.linspace(r0, r1, 16)
    u = np.sin(np.deg2rad(angles))[:, None] * r
    v = -np.cos(np.deg2rad(angles))[:, None] * r
    x = cx + M[0, 0] * u + M[0, 1] * v
    y = cy + M[1, 0] * u + M[1, 1] * v
    p = np.array([map_coordinates(g, [y, x], order=1).mean(1) for g in gold])
    turn = []
    for j in range(16):
        f = first + period / 2 * j
        if f + 4 >= len(gold):
            continue
        ismax = j % 2 == 0
        target = target_max if ismax else target_min
        ids = np.arange(round(f) - 3, round(f) + 4)
        rows = []
        for i in ids:
            # approximate motion only sets a broad search window; measured edge positions are free.
            center = target + (-1 if ismax else 1) * 1.28 * (i - f) ** 2
            mask = abs(angles - center) < 16
            ang = angles[mask]
            z = p[i, mask]
            h = max(z) - min(z)

            def model(v):
                b, s, h, l, w, g1, g2 = v
                return (
                    b
                    + s * (ang - center)
                    + h
                    * 0.5
                    * (
                        erf((ang - l) / (np.sqrt(2) * g1))
                        - erf((ang - l - w) / (np.sqrt(2) * g2))
                    )
                )

            v = [np.percentile(z, 10), 0, h, center - 4, 8, 1.5, 1.5]
            fit = least_squares(
                lambda v: model(v) - z,
                v,
                bounds=(
                    [-50, -3, 0, center - 12, 2, 0.3, 0.3],
                    [80, 3, 180, center + 2, 20, 6, 6],
                ),
                loss="soft_l1",
                f_scale=3,
                max_nfev=150,
            )
            rows.append([fit.x[3], fit.x[3] + fit.x[4]])
        rows = np.array(rows)
        ends = []
        for edge in range(2):
            # Only five nearest frames: quadratic interpolation at reversal.
            keep = abs(ids - f) < 2.6
            c = np.polyfit(ids[keep] - f, rows[keep, edge], 2)
            v = -c[1] / (2 * c[0])
            ends.append(float(np.polyval(c, v)))
        turn.append({"frame": f, "edges": ends, "samples": rows.tolist()})
    pairs = []
    for j in range(0, len(turn) - 1, 2):
        # Following the same arm through the intermediate frames establishes an additional full revolution.
        amp = (360 + np.array(turn[j]["edges"]) - np.array(turn[j + 1]["edges"])) / 2
        pairs.append(amp.tolist())
    return {"geometry": [cx, cy, ratio, r0, r1], "turns": turn, "amplitudes": pairs}


def optical(video):
    results = []
    for seed in json.loads((EVIDENCE / "refit.json").read_text()):
        start = seed["start_s"]
        amplitude, period, phase, offset = seed["fit"]
        first = (2 * np.pi - phase) * period / (2 * np.pi)
        target_max = (amplitude + offset + 60) % 120 - 60
        if target_max < 0:
            target_max += 120
        target_min = (-amplitude + offset + 60) % 120 - 60
        array = frames(video, start)
        rim = fit_rim(array)
        q = np.array([[rim[2], rim[3]], [rim[3], rim[4]]])
        eigenvalues, vectors = np.linalg.eigh(q)
        if np.any(eigenvalues <= 0):
            raise ValueError("Rim fit is not an ellipse")
        matrix = vectors @ np.diag(1 / np.sqrt(eigenvalues)) @ vectors.T
        matrix /= np.sqrt(np.linalg.det(matrix))
        a = array.astype(float)
        gold = (a[:, :, :, 0] + a[:, :, :, 1]) / 2 - a[:, :, :, 2]
        result = measure(gold, matrix, *rim[:2], first, period, target_max, target_min)
        result.update(start_s=start, matrix=matrix.tolist())
        results.append(result)
    return results


def audio(mic, executable, out):
    # Offset is the frozen three-transient alignment, in original recording time.
    aligned = out / "aligned-mic.wav"
    subprocess.run(
        [
            "ffmpeg",
            "-v",
            "error",
            "-y",
            "-ss",
            "2.284",
            "-i",
            str(mic),
            "-t",
            "21.873333",
            "-c:a",
            "pcm_s16le",
            str(aligned),
        ],
        check=True,
    )
    report = subprocess.check_output(
        [
            str(executable),
            "analyze",
            str(aligned),
            "--bph",
            "28800",
            "--lift",
            "50",
            "--windows",
            str(out / "aligned-windows.csv"),
            "--json",
        ]
    )
    (out / "aligned-analysis.json").write_bytes(report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--video", required=True, type=Path)
    parser.add_argument("--mic", type=Path)
    parser.add_argument("--timegrapher", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if bool(args.mic) != bool(args.timegrapher):
        parser.error("--mic and --timegrapher must be supplied together")
    check_source(args.video, "IMG_5824.MOV")
    if args.mic:
        check_source(args.mic, "audio-001.wav")
    args.output.mkdir(parents=True, exist_ok=True)
    results = optical(args.video)
    (args.output / "corrected-endpoints.json").write_text(
        json.dumps(results, indent=2) + "\n"
    )
    reference = json.loads((EVIDENCE / "corrected-endpoints.json").read_text())
    for measured, saved in zip(results, reference, strict=True):
        difference = np.max(
            np.abs(np.array(measured["amplitudes"]) - saved["amplitudes"])
        )
        print(
            f"{measured['start_s']} s: mean {np.mean(measured['amplitudes']):.3f} deg; "
            f"maximum difference from saved endpoints {difference:.6f} deg"
        )
        if difference > 0.1:
            raise ValueError(
                "Replay differs by more than 0.1 degree; inspect decoder/dependencies"
            )
    if args.mic:
        audio(args.mic, args.timegrapher.resolve(), args.output)


if __name__ == "__main__":
    main()
