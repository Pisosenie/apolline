# Wired: power lines against a crimson dusk, scanlines and grain. Original art for the rice.
import os, random
from PIL import Image, ImageDraw, ImageFont, ImageFilter
W, H = 2880, 1800
random.seed(7)
img = Image.new("RGB", (W, H))
px = img.load()
top, mid, low = (6, 4, 8), (92, 10, 18), (196, 38, 34)
for y in range(H):
    t = y / H
    if t < 0.62:
        k = (t / 0.62) ** 1.8
        c = tuple(int(top[i] + (mid[i] - top[i]) * k) for i in range(3))
    else:
        k = min(1, (t - 0.62) / 0.16)
        c = tuple(int(mid[i] + (low[i] - mid[i]) * k) for i in range(3))
    for x in range(W):
        px[x, y] = c
d = ImageDraw.Draw(img)
ground = int(H * 0.78)
# A low city far off.
x = 0
while x < W:
    w = random.randint(40, 140); h = random.randint(20, 120)
    d.rectangle([x, ground - h, x + w, ground], fill=(14, 4, 7))
    for _ in range(random.randint(0, 4)):
        wx = x + random.randint(6, max(7, w - 8)); wy = ground - random.randint(6, max(7, h - 6))
        d.rectangle([wx, wy, wx + 3, wy + 3], fill=(255, 120, 90))
    x += w + random.randint(0, 20)
d.rectangle([0, ground, W, H], fill=(5, 2, 4))
# Utility poles, each with crossarms, and wires sagging between them.
poles = [int(W * f) for f in (0.08, 0.36, 0.64, 0.92)]
heights = [int(H * f) for f in (0.12, 0.2, 0.16, 0.1)]
arms = []
for x, top_y in zip(poles, heights):
    d.rectangle([x - 9, top_y, x + 9, H], fill=(3, 1, 2))
    ys = []
    for j, off in enumerate((40, 110, 170)):
        y = top_y + off; half = 120 - j * 18
        d.rectangle([x - half, y, x + half, y + 10], fill=(3, 1, 2))
        for k in range(-2, 3):
            d.rectangle([x + k * half // 2 - 4, y - 14, x + k * half // 2 + 4, y], fill=(3, 1, 2))
        ys.append((y, half))
    d.ellipse([x + 30, top_y + 210, x + 80, top_y + 290], fill=(3, 1, 2))
    arms.append(ys)
for a in range(len(poles) - 1):
    for j in range(3):
        (y0, h0), (y1, h1) = arms[a][j], arms[a + 1][j]
        for k in range(-2, 3):
            x0 = poles[a] + k * h0 // 2; x1 = poles[a + 1] + k * h1 // 2
            sag = 60 + j * 14 + abs(k) * 6
            pts = []
            for s in range(61):
                t = s / 60
                pts.append((x0 + (x1 - x0) * t, y0 - 12 + (y1 - y0) * t + sag * 4 * t * (1 - t)))
            d.line(pts, fill=(2, 1, 2), width=3)
# Words, faint: the line everyone remembers, and a name in the corner.
mono = ImageFont.truetype("/usr/share/fonts/TTF/DejaVuSansMono.ttf", 30)
jp = ImageFont.truetype("/usr/share/fonts/wenquanyi/wqy-zenhei/wqy-zenhei.ttc", 300)
d.text((W - 640, H - 150), "PRESENT DAY\nPRESENT TIME", font=mono, fill=(214, 70, 64), spacing=10)
d.text((150, H - 110), "> close the world, open the next", font=mono, fill=(120, 30, 34))
layer = Image.new("L", (W, H), 0)
ImageDraw.Draw(layer).text((W - 640, 160), "レ\nイ\nン", font=jp, fill=34, spacing=-20)
img.paste((255, 70, 70), (0, 0), layer)
# Scanlines and grain.
img = img.filter(ImageFilter.GaussianBlur(0.6))
px = img.load()
for y in range(H):
    dim = 0.82 if y % 4 < 2 else 1.0
    for x in range(0, W):
        r, g, b = px[x, y]
        n = random.randint(-9, 9)
        px[x, y] = (max(0, min(255, int(r * dim) + n)), max(0, min(255, int(g * dim) + n)), max(0, min(255, int(b * dim) + n)))
img.save(os.path.expanduser("~/.local/share/apolline/wallpapers/wired.png"))
