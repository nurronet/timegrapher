"""Historical lower-arm edge kernel for the low and intermediate takes."""
import json, numpy as np
from scipy.ndimage import map_coordinates
from scipy.optimize import least_squares
from scipy.special import erf

def measure(A, base, geo, r0=88, r1=103, shift=(0, 0), scale=(1, 1), limits=(140, 220), skip_early=False):
    v = geo.copy()
    v[:2] += shift
    q = np.array([[v[2], v[3]], [v[3], v[4]]])
    e, V = np.linalg.eigh(q)
    M = V @ np.diag(1 / np.sqrt(e)) @ V.T
    M /= np.sqrt(np.linalg.det(M))
    M = M @ np.diag(scale)
    amp, period, phase, off = base['fit']
    first = (2 * np.pi - phase) * period / (2 * np.pi)
    targets = []
    if skip_early and first < 4:
        first += period
    for old in [(amp + off + 45) % 90 - 45, (-amp + off + 45) % 90 - 45]:
        pixel = np.array([150 + 92 * np.sin(np.deg2rad(old)), 154 - 92 * np.cos(np.deg2rad(old))])
        uv = np.linalg.solve(M, pixel - v[:2])
        targets.append(np.rad2deg(np.arctan2(uv[0], -uv[1])))
    targets = np.array(targets) + 180
    a = np.arange(*limits, 0.2)
    r = np.linspace(r0, r1, 20)
    u = np.sin(np.deg2rad(a))[:, None] * r
    w = -np.cos(np.deg2rad(a))[:, None] * r
    x = v[0] + M[0, 0] * u + M[0, 1] * w
    y = v[1] + M[1, 0] * u + M[1, 1] * w
    A = A.astype(float)
    gold = (A[:, :, :, 0] + A[:, :, :, 1]) / 2 - A[:, :, :, 2]
    p = np.array([map_coordinates(im, [y, x], order=1).mean(1) for im in gold])
    turns = []
    for j in range(16):
        f = first + period / 2 * j
        if f + 4 >= len(A):
            break
        ids = np.arange(round(f) - 3, round(f) + 4)
        rows = []
        target = targets[j % 2]
        ismax = j % 2 == 0
        for i in ids:
            center = target + (-1 if ismax else 1) * amp * (2 * np.pi / period) ** 2 / 2 * (i - f) ** 2
            mask = abs(a - center) < 18
            ang = a[mask]
            z = p[i, mask]

            def model(v):
                b, s, h, l, width, g1, g2 = v
                return b + s * (ang - center) + h * 0.5 * (erf((ang - l) / (2 ** 0.5 * g1)) - erf((ang - l - width) / (2 ** 0.5 * g2)))
            rr = least_squares(lambda v: model(v) - z, [np.percentile(z, 10), 0, max(z) - min(z), center - 5, 10, 1.5, 1.5], bounds=([-50, -3, 0, center - 14, 2, 0.3, 0.3], [100, 3, 200, center + 4, 23, 6, 6]), loss='soft_l1', f_scale=3, max_nfev=180)
            rows.append([rr.x[3], rr.x[3] + rr.x[4]])
        rows = np.array(rows)
        ends = []
        times = []
        for edge in range(2):
            keep = abs(ids - f) < 2.6
            c = np.polyfit(ids[keep] - f, rows[keep, edge], 2)
            dt = -c[1] / (2 * c[0])
            ends.append(float(np.polyval(c, dt)))
            times.append(float(f + dt))
        turns.append({'nominal_frame': f, 'vertex_frames': times, 'edges': ends, 'samples': rows.tolist()})
    pairs = []
    for j in range(0, len(turns) - 1, 2):
        pairs.append(((360 + np.array(turns[j]['edges']) - turns[j + 1]['edges']) / 2).tolist())
    return {'start_s': base['start_s'], 'center': v[:2].tolist(), 'matrix': M.tolist(), 'radii': [r0, r1], 'targets': targets, 'turns': turns, 'amplitudes': pairs}
