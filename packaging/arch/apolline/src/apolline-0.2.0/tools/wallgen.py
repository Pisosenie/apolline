"""Renders the bundled wallpapers: uv run --with numpy --with pillow tools/wallgen.py assets/wallpapers"""

import sys

import numpy as np
from PIL import Image

W, H = 2880, 1800
yy = np.arange(H, dtype=np.float32)[:, None]
xs = np.arange(W, dtype=np.float32)


def hexc(h):
    return np.array([int(h[i:i + 2], 16) for i in (1, 3, 5)], dtype=np.float32) / 255.0


def smooth(t):
    return t * t * (3 - 2 * t)


def fbm(x, seed, octaves, ridged):
    rng = np.random.default_rng(seed)
    total = np.zeros_like(x)
    amp, freq, norm = 1.0, 1.0, 0.0
    for _ in range(octaves):
        table = rng.random(4096).astype(np.float32)
        p = x * freq + rng.random() * 100
        i = np.floor(p).astype(int)
        f = p - i
        n = table[i % 4096] + (table[(i + 1) % 4096] - table[i % 4096]) * smooth(f)
        n = n + ((1 - np.abs(2 * n - 1)) - n) * ridged
        total += n * amp
        norm += amp
        amp *= 0.5
        freq *= 2.0
    return total / norm


def sky(stops):
    out = np.zeros((H, 1, 3), dtype=np.float32)
    y = yy / H
    for (p0, c0), (p1, c1) in zip(stops, stops[1:]):
        m = ((y >= p0) & (y <= p1))[:, 0]
        t = smooth(np.clip((y - p0) / max(p1 - p0, 1e-6), 0, 1))
        col = hexc(c0) + (hexc(c1) - hexc(c0)) * t[..., None]
        out[m] = col[m]
    return np.broadcast_to(out, (H, W, 3)).copy()


def body(img, cx, cy, r, disc, glow, strength, spread, alpha=1.0):
    d = np.sqrt((xs[None, :] - cx) ** 2 + (yy - cy) ** 2)
    g = np.exp(-(d / (r * spread)) ** 2) * strength
    img += hexc(glow) * g[..., None]
    m = np.clip((r + 1.5 - d) / 3.0, 0, 1)[..., None] * alpha
    img[:] = img * (1 - m) + hexc(disc) * m


def stars(img, n, top, seed):
    rng = np.random.default_rng(seed)
    x = rng.integers(0, W - 2, n)
    y = (rng.random(n) ** 1.6 * top * H).astype(int)
    b = rng.random(n) ** 3 * (1 - y / (top * H)) ** 1.5
    for xi, yi, bi in zip(x, y, b):
        s = 2 if bi > 0.35 else 1
        img[yi:yi + s, xi:xi + s] += bi * 0.9


def ridges(img, layers, fog, seed):
    fog = hexc(fog)
    k = 361
    for i, L in enumerate(layers):
        n = fbm(xs / W * L["scale"], seed + i * 17, L["oct"], L["ridged"])
        top = L["base"] * H - (n - 0.5) * 2 * L["amp"]
        cover = np.clip((yy - top[None, :]) / 1.5 + 0.5, 0, 1)
        soft = np.convolve(np.pad(top, k // 2, mode="edge"), np.ones(k) / k, mode="valid")
        depth = np.clip((yy - soft[None, :]) / L["fade"], 0, 1)
        f = L["fogTop"] + (L["fogBottom"] - L["fogTop"]) * depth
        col = hexc(L["color"]) + (fog - hexc(L["color"])) * f[..., None]
        img[:] = img * (1 - cover[..., None]) + col * cover[..., None]


def finish(img, seed, vignette):
    u = (xs[None, :] / W - 0.5) * 2
    v = (yy / H - 0.5) * 2
    img *= (1 - vignette * (u ** 2 + v ** 2) / 2)[..., None]
    rng = np.random.default_rng(seed)
    img += rng.normal(0, 0.012, (H, W, 1)).astype(np.float32)
    return Image.fromarray((np.clip(img, 0, 1) * 255 + 0.5).astype(np.uint8))


def layer(base, amp, color, fogTop, fogBottom, fade, scale, ridged, oct=7):
    return dict(base=base, amp=amp, color=color, fogTop=fogTop, fogBottom=fogBottom,
                fade=fade, scale=scale, ridged=ridged, oct=oct)


def fog():
    img = sky([(0, "#3a4756"), (0.42, "#7d8d9d"), (0.6, "#c0c9d0"), (1, "#c0c9d0")])
    body(img, 0.68 * W, 0.4 * H, 64, "#eef1f2", "#ffffff", 0.16, 6, alpha=0.8)
    ridges(img, [
        layer(0.52, 260, "#8d9bab", 0.35, 0.95, 220, 2.2, 0.6),
        layer(0.58, 280, "#6f7f90", 0.25, 0.92, 240, 2.6, 0.6),
        layer(0.65, 300, "#55667a", 0.18, 0.88, 260, 3.0, 0.5),
        layer(0.72, 300, "#3d4c5e", 0.12, 0.82, 280, 3.4, 0.5),
        layer(0.80, 280, "#283543", 0.06, 0.72, 320, 3.0, 0.4),
        layer(0.90, 240, "#151d26", 0.0, 0.5, 400, 2.4, 0.3),
    ], "#c6cfd6", 11)
    return finish(img, 1, 0.22)


def ember():
    img = sky([(0, "#12060a"), (0.3, "#3b0d14"), (0.55, "#a3271c"), (0.66, "#ff6a2a"), (0.72, "#ffb36b"), (1, "#ffb36b")])
    body(img, 0.38 * W, 0.66 * H, 150, "#ffe2b0", "#ff8a3c", 0.55, 3.2)
    ridges(img, [
        layer(0.66, 140, "#b8361f", 0.45, 0.9, 180, 1.6, 0.0, 5),
        layer(0.72, 180, "#7d1f17", 0.3, 0.8, 220, 1.8, 0.1, 5),
        layer(0.79, 220, "#4a1210", 0.15, 0.65, 260, 2.0, 0.2, 5),
        layer(0.86, 240, "#260a0b", 0.05, 0.5, 300, 2.2, 0.2, 5),
        layer(0.94, 220, "#0d0406", 0.0, 0.3, 380, 1.8, 0.1, 5),
    ], "#ff7d45", 23)
    return finish(img, 2, 0.3)


def night():
    img = sky([(0, "#05070f"), (0.4, "#0d1226"), (0.62, "#232a4a"), (0.7, "#3a3f63"), (1, "#3a3f63")])
    stars(img, 1400, 0.62, 5)
    body(img, 0.74 * W, 0.24 * H, 58, "#f3e9cf", "#f2c983", 0.22, 7)
    ridges(img, [
        layer(0.58, 260, "#2c3356", 0.35, 0.85, 240, 2.4, 0.6),
        layer(0.64, 280, "#222844", 0.25, 0.8, 260, 2.8, 0.6),
        layer(0.70, 300, "#191d34", 0.15, 0.7, 280, 3.2, 0.5),
        layer(0.77, 300, "#111426", 0.1, 0.6, 300, 3.4, 0.5),
        layer(0.85, 280, "#0a0c18", 0.05, 0.5, 340, 3.0, 0.4),
        layer(0.93, 240, "#05060c", 0.0, 0.35, 400, 2.4, 0.3),
    ], "#2b3156", 31)
    return finish(img, 3, 0.3)


if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "assets/wallpapers"
    for name, render in [("fog", fog), ("ember", ember), ("night", night)]:
        render().save(f"{out}/{name}.jpg", quality=90, optimize=True, progressive=True)
