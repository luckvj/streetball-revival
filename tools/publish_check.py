"""Keep game data out of what gets published.

Walks the repository and fails when it finds anything taken from the disc:
files over 2 MB, game-data file extensions, or game-data magic numbers.
Build output (target/), git metadata (.git/), and ignored local agent
worktrees (.claude/) are skipped; none are release contents.

Run from the repository root:  python tools/publish_check.py
"""

import os
import sys

# Anything bigger is an extracted asset or a disc image by mistake.
SIZE_LIMIT = 2 * 1024 * 1024

BAD_EXTENSIONS = {
    ".iso", ".nfx", ".abk", ".ast", ".aem", ".big", ".viv", ".ssh",
    ".mus", ".mpf", ".asf", ".png", ".jpg", ".wav",
    ".max", ".gci", ".psu", ".ps2", ".mcd", ".mpc", ".e3b", ".adf", ".ssb",
}

# Game-data magic at the start of a file.
MAGIC = (
    b"SCHl", b"BNKl", b"AB\x09\x00", b"AE\x15\x00", b"PFDx",
    b"SHPS", b"MPCh", b"\xc0\xfb",
    b"Ps2PowerSave",
)

# An ISO9660 volume descriptor where the disc's primary one sits.
ISO_MAGIC = b"CD001"
ISO_MAGIC_OFFSET = 0x8001

SKIP_DIRS = {"target", ".git", ".claude"}

# The original's publisher's and league's names stay out of the project's
# own text; LEGAL.md is the one place that names them.
import re

_LEAGUE = b"N" + b"BA"
_PUBLISHER = b"E" + b"A"
NAMES = re.compile(
    rb"\b" + _LEAGUE + rb"\b|\bW" + _LEAGUE + rb"\b|Electronic" + rb" Arts|\b"
    + _PUBLISHER + rb" (SPORTS|Sports)\b"
)
NAMES_ALLOWED = {"LEGAL.md"}
TEXT_EXTENSIONS = {".rs", ".md", ".json", ".py", ".toml", ".yml", ".yaml", ".txt", ".svg", ".html", ".ps1", ".cmd"}

# The README's in-game screenshots: JPEGs in this one folder only, each
# small. Every other check (size, magic numbers) still applies to them.
SCREENSHOTS = os.path.join("docs", "screenshots")
SCREENSHOT_LIMIT = 512 * 1024
# The launcher's own art (the MyBaller mark and banner, the author's): PNGs
# in this one folder only, each small. Every other check still applies.
LAUNCHER_ART = os.path.join("crates", "street-launcher", "assets")


_TRACKED = None


def tracked(root, rel):
    """Whether git tracks `rel` (untracked local notes are not published)."""
    global _TRACKED
    if _TRACKED is None:
        import subprocess
        try:
            out = subprocess.run(["git", "-C", root, "ls-files"], capture_output=True, text=True).stdout
            _TRACKED = {os.path.normpath(p) for p in out.splitlines()}
        except OSError:
            _TRACKED = set()
        if not _TRACKED:
            return True
    return os.path.normpath(rel) in _TRACKED


def offenders(root):
    found = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for name in filenames:
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, root)
            try:
                size = os.path.getsize(path)
            except OSError:
                continue
            if size > SIZE_LIMIT:
                found.append(f"{rel}: over 2 MB ({size} bytes)")
                continue
            ext = os.path.splitext(name)[1].lower()
            if os.path.dirname(rel) == SCREENSHOTS and ext == ".jpg":
                if size > SCREENSHOT_LIMIT:
                    found.append(f"{rel}: screenshot over 512 KB")
                    continue
            elif os.path.dirname(rel) == LAUNCHER_ART and ext == ".png":
                if size > SCREENSHOT_LIMIT:
                    found.append(f"{rel}: launcher art over 512 KB")
                    continue
            elif ext in BAD_EXTENSIONS:
                found.append(f"{rel}: game-data extension")
                continue
            try:
                with open(path, "rb") as f:
                    head = f.read(16)
                    iso = b""
                    if size >= ISO_MAGIC_OFFSET + len(ISO_MAGIC):
                        f.seek(ISO_MAGIC_OFFSET)
                        iso = f.read(len(ISO_MAGIC))
            except OSError:
                continue
            if any(head.startswith(m) for m in MAGIC) or iso == ISO_MAGIC:
                found.append(f"{rel}: game-data magic")
                continue
            if ext in TEXT_EXTENSIONS and name not in NAMES_ALLOWED and tracked(root, rel):
                with open(path, "rb") as f:
                    for number, line in enumerate(f, 1):
                        if NAMES.search(line):
                            found.append(f"{rel}:{number}: a rights holder's name (keep it to LEGAL.md)")
                            break
    return found


def main():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    bad = offenders(root)
    if bad:
        print("publish check: offenders found")
        for line in bad:
            print(f"  {line}")
        return 1
    print("publish check: clean")
    return 0


if __name__ == "__main__":
    sys.exit(main())
