"""Build a reverse-engineering workspace from the user's disc image.

    python tools/re_workspace.py <Vol2.iso> <workspace dir> [--ghidra <ghidra install dir>]

Outside the repository (it holds game data), this creates:

    disc/             the small files: SLUS_206.51, /FE, /LOCALE, /CSCRIPTS, /DATABASE,
                      /SCRIPTS, /COURTFX, /CREDITS, /MCARD, /ANIM, audio event maps
    loc_fe.txt        every front-end string as `<file>#<index>: <text>`
    loc_gm.txt        every in-game string
    disc.txt          the full file list (path, size, LBA)
    dec/SLUS_206.51   the executable with EE-only opcodes rewritten (tools/ee_patch.py)
    gproj/, gout/     with --ghidra: a Ghidra project of the patched executable and
                      gout/decomp.c (every function), functions.txt (sizes and callers),
                      strings.txt (strings and the functions that use them)

The Ghidra run takes about 15 minutes. Never point it at another Ghidra project.
"""
import argparse
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SMALL = ['/ANIM', '/COURTFX', '/CREDITS', '/CSCRIPTS', '/DATABASE', '/FE', '/FRPRELD', '/LOCALE',
         '/MCARD', '/SCRIPTS', '/SFX/FE_EVT', '/SFX/GAME_EVT', '/SFX/STDELAY', '/SPEECH/COMNTARY',
         '/SPEECH/PLAYERS', '/SPEECH/PLYRHDRS', '/SPEECH/SPCHHDRS', '/SLUS', '/SYSTEM', '/XLD',
         '/XPRELOAD', '/XF/XF000003', '/XF/XF000004', '/XF/XF000007']


def run(*args, **kw):
    print('>', ' '.join(str(a) for a in args))
    subprocess.run([str(a) for a in args], check=True, **kw)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('iso')
    ap.add_argument('workspace')
    ap.add_argument('--ghidra', help='Ghidra install directory (contains support/analyzeHeadless)')
    a = ap.parse_args()
    ws = os.path.abspath(a.workspace)
    os.makedirs(ws, exist_ok=True)
    py = sys.executable
    with open(os.path.join(ws, 'disc.txt'), 'w') as f:
        run(py, os.path.join(HERE, 'iso_list.py'), a.iso, stdout=f)
    with open(os.devnull, 'w') as null:
        run(py, os.path.join(HERE, 'iso_extract.py'), a.iso, os.path.join(ws, 'disc'), *SMALL, stdout=null)
    for name, out in (('XLOC_FE.EUS', 'loc_fe.txt'), ('XLOC_GM.EUS', 'loc_gm.txt')):
        run(py, os.path.join(HERE, 'locale_dump.py'), os.path.join(ws, 'disc', 'LOCALE', name), os.path.join(ws, out))
    os.makedirs(os.path.join(ws, 'dec'), exist_ok=True)
    run(py, os.path.join(HERE, 'ee_patch.py'), os.path.join(ws, 'disc', 'SLUS_206.51'),
        os.path.join(ws, 'dec', 'SLUS_206.51'))
    if a.ghidra:
        exe = 'analyzeHeadless.bat' if os.name == 'nt' else 'analyzeHeadless'
        for d in ('gproj', 'gout'):
            os.makedirs(os.path.join(ws, d), exist_ok=True)
        run(os.path.join(a.ghidra, 'support', exe), os.path.join(ws, 'gproj'), 'street2ee',
            '-import', os.path.join(ws, 'dec', 'SLUS_206.51'),
            '-processor', 'MIPS:LE:64:64-32addr', '-cspec', 'n32',
            '-scriptPath', os.path.join(HERE, 'ghidra'), '-postScript', 'ExportAll.java',
            os.path.join(ws, 'gout'), '-max-cpu', '8')


if __name__ == '__main__':
    main()
