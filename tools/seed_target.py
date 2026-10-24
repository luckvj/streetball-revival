"""Seed a helper's cargo target dir from the merger's warm cache.

Every third-party artifact under target/claude/debug is hard-linked (same
metadata hash in every worktree, so cargo keeps it fresh and never rewrites
it); the workspace's own crates (street-*) and the examples are left out so
the helper builds them into files of its own. Hard links share mtimes, so
cargo's fingerprints stay valid.

Usage: python seed_target.py <repo> <dest-target-dir-relative-to-repo>
"""
import os
import sys
import time

repo, dest = sys.argv[1], sys.argv[2]
src = os.path.join(repo, 'target', 'claude', 'debug')
out = os.path.join(repo, dest, 'debug')
t = time.time()
linked = copied = skipped = 0
for root, dirs, files in os.walk(src):
    rel = os.path.relpath(root, src)
    parts = rel.replace('\\', '/').split('/')
    if parts[0] in ('incremental', 'examples') or any('street' in p.lower() for p in parts):
        dirs[:] = []
        continue
    os.makedirs(os.path.join(out, rel), exist_ok=True)
    for f in files:
        if 'street' in f.lower() or f == '.cargo-lock':
            skipped += 1
            continue
        s, d = os.path.join(root, f), os.path.join(out, rel, f)
        if os.path.exists(d):
            continue
        try:
            os.link(s, d)
            linked += 1
        except OSError:
            import shutil
            shutil.copy2(s, d)
            copied += 1
print(f'seeded {out}: {linked} linked, {copied} copied, {skipped} skipped in {time.time() - t:.0f}s')
