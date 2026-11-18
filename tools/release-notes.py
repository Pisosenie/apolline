#!/usr/bin/env python3
"""The notes of one version from CHANGELOG.md, for its GitHub release: release-notes.py 0.5.0"""

import re
import sys
from pathlib import Path

version = sys.argv[1].removeprefix("v")
lines = (Path(__file__).resolve().parent.parent / "CHANGELOG.md").read_text().splitlines()
heading = re.compile(r"^## (\S+)")
notes, inside = [], False
for line in lines:
    match = heading.match(line)
    if match:
        if inside:
            break
        inside = match.group(1) == version
        continue
    if inside:
        notes.append(line)
if not inside and not notes:
    sys.exit(f"CHANGELOG.md has no ## {version}")
print("\n".join(notes).strip())
