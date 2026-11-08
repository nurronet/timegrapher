"""Historical rim-marker kernel; selection seeds are frozen, not refitted to audio."""
import numpy as np, json
from scipy.ndimage import map_coordinates
from scipy.optimize import least_squares

def measure_mark(b, A, v, r0=109, r1=113, shift=(0, 0)):
    n = b['start_s']
    A = A.astype(float)
    v = v.copy()
    v[:2] += shift
    q = np.array([[v[2], v[3]], [v[3], v[4]]])
    e, V = np.linalg.eigh(q)
    M = V @ np.diag(1 / np.sqrt(e)) @ V.T
    M /= np.sqrt(np.linalg.det(M))
    angles = np.arange(-30, 210, 0.1)
    r = np.linspace(r0, r1, 9)
    u = np.sin(np.deg2rad(angles))[:, None] * r
    w = -np.cos(np.deg2rad(angles))[:, None] * r
    x = v[0] + M[0, 0] * u + M[0, 1] * w
    y = v[1] + M[1, 0] * u + M[1, 1] * w
    grey = A.mean(3)
    p = np.array([map_coordinates(im, [y, x], order=1).mean(1) for im in grey])
    p = np.median(p, axis=0) - p
    amp, per, phase, off = b['fit']
    first = (2 * np.pi - phase) * per / (2 * np.pi)
    turns = []
    for j in range(16):
        f = first + j * per / 2
        if f + 4 >= len(A):
            break
        target = (amp + off + 268) % 360 if j % 2 == 0 else (-amp + off + 268) % 360
        if target > 300:
            target -= 360
        ids = np.arange(round(f) - 3, round(f) + 4)
        centers = []
        for i in ids:
            pred = target + (-1 if j % 2 == 0 else 1) * amp * (2 * np.pi / per) ** 2 / 2 * (i - f) ** 2
            mask = abs(angles - pred) < 13
            a = angles[mask]
            z = p[i, mask]

            def model(v):
                bg, s, h, c, sig = v
                return bg + s * (a - pred) + h * np.exp(-0.5 * ((a - c) / sig) ** 2)
            fit = least_squares(lambda v: model(v) - z, [np.percentile(z, 20), 0, max(5, max(z) - min(z)), pred, 2], bounds=([-100, -5, 0, pred - 8, 0.4], [100, 5, 220, pred + 8, 6]), loss='soft_l1', f_scale=3, max_nfev=160)
            centers.append(float(fit.x[3]))
        keep = abs(ids - f) < 2.6
        coef = np.polyfit(ids[keep] - f, np.array(centers)[keep], 2)
        dt = -coef[1] / (2 * coef[0])
        turns.append({'frame': float(f + dt), 'angle': float(np.polyval(coef, dt)), 'centers': centers})
    amps = [(360 + turns[j]['angle'] - turns[j + 1]['angle']) / 2 for j in range(0, len(turns) - 1, 2)]
    o = {'start_s': n, 'turns': turns, 'amplitudes': amps}
    return o
