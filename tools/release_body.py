"""The release page's text for a tag: its CHANGELOG.md section as "What's
new", then .github/RELEASE_NOTES.md.

Usage: python tools/release_body.py <tag> > release_body.md
"""
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def section(changelog, tag):
    """The lines under `## <tag>` up to the next `## `."""
    out, inside = [], False
    for line in changelog.splitlines():
        if line.startswith("## "):
            if inside:
                break
            inside = line[3:].strip() == tag
            continue
        if inside:
            out.append(line)
    return "\n".join(out).strip()


def main():
    tag = sys.argv[1]
    notes = (ROOT / ".github" / "RELEASE_NOTES.md").read_text(encoding="utf-8")
    new = section((ROOT / "CHANGELOG.md").read_text(encoding="utf-8"), tag)
    body = f"## What's new in {tag}\n\n{new}\n\n---\n\n{notes}" if new else notes
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stdout.write(body)


if __name__ == "__main__":
    main()
