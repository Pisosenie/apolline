#!/usr/bin/env python3
"""Fills the site from the repository: releases from CHANGELOG.md, the docs from docs/CONFIG.md,
the catalog, screenshots, fonts and the logo. Run before `zola build`; what it writes is ignored."""

import re
import shutil
import subprocess
import sys
from pathlib import Path

SITE = Path(__file__).resolve().parent
ROOT = SITE.parent
REPO = "https://github.com/Pisosenie/apolline"
LINK = re.compile(r"\]\(([^)\s]+)\)")
RELEASE = re.compile(r"^## (\d+\.\d+\.\d+) — (\d{4}-\d{2}-\d{2})\s*$")


def linked(text, base):
    """Links into the repository go to GitHub, links into the docs stay on the site."""

    def fix(match):
        target = match.group(1)
        if re.match(r"^(https?:|mailto:|#|@/)", target):
            return match.group(0)
        path, _, anchor = target.partition("#")
        resolved = (base / path).resolve().relative_to(ROOT).as_posix()
        if resolved == "docs/CONFIG.md":
            return "](@/docs/_index.md" + ("#" + anchor if anchor else "") + ")"
        kind = "tree" if (ROOT / resolved).is_dir() else "blob"
        return "](%s/%s/main/%s%s)" % (REPO, kind, resolved, "#" + anchor if anchor else "")

    return LINK.sub(fix, text)


def releases():
    out = SITE / "content" / "news"
    for old in out.glob("*.md"):
        if old.name != "_index.md":
            old.unlink()
    current, body = None, []
    found = []
    for line in (ROOT / "CHANGELOG.md").read_text().splitlines():
        match = RELEASE.match(line)
        if match:
            if current:
                found.append((current, body))
            current, body = match.groups(), []
        elif current:
            body.append(line)
    if current:
        found.append((current, body))
    for weight, ((version, date), lines) in enumerate(found, 1):
        text = linked("\n".join(lines).strip(), ROOT)
        front = (
            '+++\ntitle = "Apolline %s"\ndate = %s\nweight = %d\n'
            'slug = "%s"\n[extra]\nversion = "%s"\n+++\n\n'
        ) % (version, date, weight, version.replace(".", "-"), version)
        (out / ("%s.md" % version.replace(".", "-"))).write_text(front + text + "\n")
    return len(found)


def docs():
    lines = (ROOT / "docs" / "CONFIG.md").read_text().splitlines()
    if lines and lines[0].startswith("# "):
        lines = lines[1:]
    text = linked("\n".join(lines).strip(), ROOT / "docs")
    front = (
        '+++\ntitle = "Docs"\ndescription = "Everything Apolline does and every option it takes."\n'
        'template = "docs.html"\n+++\n\n'
    )
    out = SITE / "content" / "docs"
    out.mkdir(exist_ok=True)
    (out / "_index.md").write_text(front + text + "\n")


def static():
    out = SITE / "static"
    for name in ("shots", "fonts", "catalog", "icons"):
        shutil.rmtree(out / name, ignore_errors=True)
    shutil.copytree(ROOT / "docs" / "screenshots", out / "shots")
    shutil.copy(ROOT / "examples" / "wired" / "terminals.jpg", out / "shots" / "wired.jpg")
    (out / "fonts").mkdir()
    for font in ("Geist-ExtraLight", "Geist-Regular", "Geist-Medium", "Geist-SemiBold",
                 "GeistMono-Regular", "GeistMono-Medium"):
        shutil.copy(ROOT / "assets" / "fonts" / (font + ".ttf"), out / "fonts")
    shutil.copy(ROOT / "assets" / "fonts" / "OFL.txt", out / "fonts")
    catalog = out / "catalog"
    catalog.mkdir()
    shutil.copy(ROOT / "catalog" / "index.toml", catalog)
    for preview in (ROOT / "catalog").glob("*/preview.jpg"):
        (catalog / preview.parent.name).mkdir()
        shutil.copy(preview, catalog / preview.parent.name)
    shutil.copytree(ROOT / "assets" / "icons", out / "icons", ignore=shutil.ignore_patterns("LICENSE"))
    shutil.copy(ROOT / "assets" / "apolline.svg", out / "logo.svg")


def hypr2niri():
    """The converter as WebAssembly, its Lua sandbox and the example configs, for /hyprland/."""
    out = SITE / "static" / "hypr2niri"
    shutil.rmtree(out, ignore_errors=True)
    (out / "examples").mkdir(parents=True)
    subprocess.run(
        ["cargo", "build", "-q", "-p", "hypr2niri-web", "--release",
         "--target", "wasm32-unknown-unknown"],
        cwd=ROOT, check=True,
    )
    target = Path(__import__("os").environ.get("CARGO_TARGET_DIR", ROOT / "target"))
    shutil.copy(target / "wasm32-unknown-unknown" / "release" / "hypr2niri_web.wasm",
                out / "hypr2niri.wasm")
    shutil.copy(ROOT / "crates" / "hypr2niri" / "src" / "sandbox.lua", out)
    for example in (ROOT / "crates" / "hypr2niri" / "tests" / "configs").iterdir():
        shutil.copy(example, out / "examples")


def main():
    count = releases()
    if count == 0:
        sys.exit("no releases read from CHANGELOG.md")
    docs()
    static()
    hypr2niri()
    print("site: %d releases, docs and pictures in place" % count)


if __name__ == "__main__":
    main()
