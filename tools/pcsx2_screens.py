"""Drive the project's PCSX2 copy from the keyboard and grab the original's screens.

    python tools/pcsx2_screens.py launch wait 35 size 678 559 key enter wait 4 grab main

Commands, run in order:
  launch          start pcsx2-qt with the ISO (a fresh boot; no state is loaded), detached,
                  unless one is already running; the window in front before it is put back
  size W H        set the window's client size (678 x 559 shows the 4:3 frame at 1x)
  key NAME [ms]   press a key in the game window, held 30 ms by default (Play It and
                  Choose Baller step twice on a press of 60 to 120 ms): enter k i j l up
                  down left right bs q e, f2 f3 f4, or any letter or digit, with the
                  runtime's keyboard pads: Pad1
                  Enter Start, K Cross, I Triangle, J Square, L Circle, BS Select, Q 1 E 3
                  the turbos (L1 L2 R1 R2), 2 L3; Pad2 0 Start, M Cross, B Triangle, V Square,
                  N Circle, P Select, 8 5 7 9 the directions; F3 loads the save state of the
                  current slot, F2 picks the next slot (F1, which saves over a slot, is
                  deliberately not offered)
  keys NAME...    several presses, one after another (ends at the next command word)
  down NAME       hold a key down (a chord: down q, then key l, then up q)
  up NAME         let it go
  wait S          sleep S seconds
  grab NAME       capture the game area to $EVIDENCE/<folder>/NAME.png in the layouts' 640x480
                  space (the PS2 frame holds layout rows 16..464: scaled to 640x448, set 16 down)
  raw NAME        the window's client area as it is, to NAME_raw.png beside the grabs

By default the keys are posted to PCSX2's display window (its Qt `QWindowIcon` child,
WM_KEYDOWN / WM_KEYUP) and the grab is PrintWindow's copy of the window, so the game is
driven with its window in the background and nothing is typed into the window in front.
STREET_SENDINPUT=1 brings the old way back (SendInput to the foreground window, which this
then puts PCSX2 into, and a screen grab of the window; it refuses to type when another
window stays in front).

Environment: STREET_PCSX2 the runtime folder (with pcsx2-qt.exe and a portable ini whose Pad1
is the keyboard), STREET_ISO the disc image, STREET_EVIDENCE the evidence root (default the
temp folder), STREET_SCREENS the folder under it (default street-screens-original).
Windows only. Launch from PowerShell or cmd: PCSX2 reports a forward-slash ISO path given
through Git Bash as missing.
"""
import ctypes
import ctypes.wintypes as wt
import os
import subprocess
import sys
import tempfile
import time

from PIL import Image, ImageGrab, PngImagePlugin

RUNTIME = os.environ.get("STREET_PCSX2")
ISO = os.environ.get("STREET_ISO")
EVIDENCE = os.path.join(
    os.environ.get("STREET_EVIDENCE", tempfile.gettempdir()),
    os.environ.get("STREET_SCREENS", "street-screens-original"),
)
SENDINPUT = os.environ.get("STREET_SENDINPUT") == "1"
user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
VK = {
    "enter": 0x0D, "k": 0x4B, "i": 0x49, "j": 0x4A, "l": 0x4C, "up": 0x26, "down": 0x28,
    "left": 0x25, "right": 0x27, "bs": 0x08, "q": 0x51, "e": 0x45, "esc": 0x1B, "f8": 0x77,
    "f2": 0x71, "f3": 0x72, "f4": 0x73,
}
# Any other letter or digit by its own name (the runtime's Pad2: 8 Up, 5 Down,
# 7 Left, 9 Right, M Cross, N Circle, B Triangle, V Square, P Select, 0 Start).
for _c in "abcdefghijklmnopqrstuvwxyz0123456789":
    VK.setdefault(_c, ord(_c.upper()))
EXTENDED = {0x26, 0x28, 0x25, 0x27}
MENU_BAR = 22
COMMANDS = {"launch", "size", "key", "keys", "down", "up", "wait", "grab", "raw"}


class RECT(ctypes.Structure):
    _fields_ = [("l", ctypes.c_long), ("t", ctypes.c_long), ("r", ctypes.c_long), ("b", ctypes.c_long)]


class POINT(ctypes.Structure):
    _fields_ = [("x", ctypes.c_long), ("y", ctypes.c_long)]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [("wVk", wt.WORD), ("wScan", wt.WORD), ("dwFlags", wt.DWORD), ("time", wt.DWORD),
                ("dwExtraInfo", ctypes.POINTER(ctypes.c_ulong))]


class INPUT(ctypes.Structure):
    class U(ctypes.Union):
        _fields_ = [("ki", KEYBDINPUT), ("pad", ctypes.c_byte * 32)]

    _anonymous_ = ("u",)
    _fields_ = [("type", wt.DWORD), ("u", U)]


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [("biSize", wt.DWORD), ("biWidth", ctypes.c_long), ("biHeight", ctypes.c_long),
                ("biPlanes", wt.WORD), ("biBitCount", wt.WORD), ("biCompression", wt.DWORD),
                ("biSizeImage", wt.DWORD), ("biXPelsPerMeter", ctypes.c_long),
                ("biYPelsPerMeter", ctypes.c_long), ("biClrUsed", wt.DWORD),
                ("biClrImportant", wt.DWORD)]


def pcsx2_pids():
    out = subprocess.run(["tasklist", "/FI", "IMAGENAME eq pcsx2-qt.exe", "/FO", "CSV", "/NH"],
                         capture_output=True, text=True).stdout
    pids = []
    for line in out.splitlines():
        parts = [p.strip('"') for p in line.split('","')]
        if len(parts) > 1 and parts[0].lower().startswith("pcsx2-qt"):
            pids.append(int(parts[1]))
    return pids


def find_window():
    """The pcsx2-qt top window showing the game (its title names the game)."""
    pids = set(pcsx2_pids())
    found = []

    @ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)
    def each(h, _):
        p = wt.DWORD()
        user32.GetWindowThreadProcessId(h, ctypes.byref(p))
        n = user32.GetWindowTextLengthW(h)
        if n and user32.IsWindowVisible(h):
            buf = ctypes.create_unicode_buffer(n + 1)
            user32.GetWindowTextW(h, buf, n + 1)
            if p.value in pids or "PCSX2" in buf.value or "the original game" in buf.value:
                found.append((h, buf.value))
        return True

    user32.EnumWindows(each, 0)
    found.sort(key=lambda x: "league" not in x[1])
    return found[0][0] if found else None


def display_child(top):
    """The Qt display surface inside the top window, which takes posted keys."""
    found = []

    @ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)
    def child(c, _):
        cls = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(c, cls, 256)
        if "QWindowIcon" in cls.value:
            found.append(c)
        return True

    user32.EnumChildWindows(top, child, 0)
    return found[0] if found else None


def window():
    h = find_window()
    if not h:
        raise SystemExit("no PCSX2 window")
    return h


def focus():
    h = window()
    if user32.GetForegroundWindow() == h:
        return h
    # A background process may take the foreground after a modifier tap;
    # the tap only when the window is not already in front, since a lone
    # Alt on a focused Qt window moves the focus to its menu bar.
    user32.keybd_event(0x12, 0, 0, 0)
    user32.keybd_event(0x12, 0, 2, 0)
    user32.ShowWindow(h, 9)
    user32.SetForegroundWindow(h)
    time.sleep(0.3)
    front = user32.GetForegroundWindow()
    if front != h:
        # Never type into another window (a PCSX2 dialog, another app).
        n = user32.GetWindowTextLengthW(front)
        buf = ctypes.create_unicode_buffer(n + 1)
        user32.GetWindowTextW(front, buf, n + 1)
        raise SystemExit(f"the game window is not in front: {buf.value!r} is")
    return h


def send(vk, down):
    if SENDINPUT:
        focus()
        scan = user32.MapVirtualKeyW(vk, 0)
        flags = 0x8 | (0 if down else 0x2) | (0x1 if vk in EXTENDED else 0)
        entry = INPUT(type=1, ki=KEYBDINPUT(vk, scan, flags, 0, None))
        user32.SendInput(1, ctypes.byref(entry), ctypes.sizeof(INPUT))
        return
    target = display_child(window())
    if not target:
        raise SystemExit("no PCSX2 display window (is a game running?)")
    scan = user32.MapVirtualKeyW(vk, 0)
    lparam = 1 | (scan << 16) | ((1 if vk in EXTENDED else 0) << 24)
    if not down:
        lparam |= 3 << 30
    user32.PostMessageW(target, 0x100 if down else 0x101, vk, lparam)


def key(name, ms=30):
    send(VK[name], True)
    time.sleep(ms / 1000.0)
    send(VK[name], False)
    time.sleep(0.25)


def client_box(h):
    r = RECT()
    user32.GetClientRect(h, ctypes.byref(r))
    p = POINT(0, 0)
    user32.ClientToScreen(h, ctypes.byref(p))
    return (p.x, p.y, p.x + r.r, p.y + r.b)


def size(h, w, hh):
    r = RECT()
    user32.GetWindowRect(h, ctypes.byref(r))
    c = RECT()
    user32.GetClientRect(h, ctypes.byref(c))
    # SWP_NOZORDER | SWP_NOACTIVATE: the window keeps its place behind others.
    user32.SetWindowPos(h, 0, r.l, r.t, w + (r.r - r.l) - c.r, hh + (r.b - r.t) - c.b, 0x0014)
    time.sleep(0.5)


def print_window(h):
    """The client area through PrintWindow (PW_CLIENTONLY | PW_RENDERFULLCONTENT),
    which copies the D3D surface even when other windows cover it."""
    r = RECT()
    user32.GetClientRect(h, ctypes.byref(r))
    w, hh = r.r, r.b
    dc = user32.GetDC(h)
    mdc = gdi32.CreateCompatibleDC(dc)
    bmp = gdi32.CreateCompatibleBitmap(dc, w, hh)
    gdi32.SelectObject(mdc, bmp)
    user32.PrintWindow(h, mdc, 3)
    bi = BITMAPINFOHEADER()
    bi.biSize = ctypes.sizeof(BITMAPINFOHEADER)
    bi.biWidth, bi.biHeight, bi.biPlanes, bi.biBitCount = w, -hh, 1, 32
    buf = ctypes.create_string_buffer(w * hh * 4)
    gdi32.GetDIBits(mdc, bmp, 0, hh, buf, ctypes.byref(bi), 0)
    gdi32.DeleteObject(bmp)
    gdi32.DeleteDC(mdc)
    user32.ReleaseDC(h, dc)
    return Image.frombuffer("RGBA", (w, hh), buf, "raw", "BGRA", 0, 1).convert("RGB")


def menu_bar_height(im):
    """Where the game area starts: the first row that is black across the
    window (the menu bar above it is grey). Falls back to MENU_BAR."""
    w, h = im.size
    px = im.load()
    for y in range(0, min(h, 64)):
        dark = sum(1 for x in range(0, w, 8) if max(px[x, y][:3]) < 24)
        if dark * 8 >= w * 0.9:
            return y
    return MENU_BAR


def window_image(h):
    if SENDINPUT:
        return ImageGrab.grab(bbox=client_box(h), all_screens=True)
    if not user32.IsIconic(h):
        return print_window(h)
    # A minimised window has no client area to copy: show it behind every
    # other window without activating it (SW_SHOWNOACTIVATE, HWND_BOTTOM),
    # copy it once it has drawn, and minimise it again (SW_SHOWMINNOACTIVE).
    user32.ShowWindow(h, 4)
    user32.SetWindowPos(h, 1, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0010)
    time.sleep(0.6)
    im = print_window(h)
    user32.ShowWindow(h, 7)
    return im


def grab(h, name):
    im = window_image(h)
    w = im.size[0]
    # The game area under the menu bar, 4:3, then the layouts' space.
    top = menu_bar_height(im)
    im = im.crop((0, top, w, top + round(w * 3 / 4))).resize((640, 448), Image.LANCZOS)
    out = Image.new("RGB", (640, 480), (0, 0, 0))
    out.paste(im, (0, 16))
    os.makedirs(EVIDENCE, exist_ok=True)
    meta = PngImagePlugin.PngInfo()
    meta.add_text("layout", "1")
    path = os.path.join(EVIDENCE, name + ".png")
    out.save(path, pnginfo=meta)
    print("grab:", path)


def raw(h, name):
    os.makedirs(EVIDENCE, exist_ok=True)
    path = os.path.join(EVIDENCE, name + "_raw.png")
    window_image(h).save(path)
    print("raw:", path)


def launch():
    if pcsx2_pids():
        print("launch: pcsx2-qt is already running; using it")
        return
    if not RUNTIME or not ISO:
        raise SystemExit("set STREET_PCSX2 and STREET_ISO")
    front = user32.GetForegroundWindow()
    # Detached, so the emulator outlives this script (and the task that ran it).
    flags = 0x00000008 | 0x00000200  # DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    subprocess.Popen([os.path.join(RUNTIME, "pcsx2-qt.exe"), ISO], cwd=RUNTIME,
                     creationflags=flags, close_fds=True)
    for _ in range(100):
        time.sleep(0.2)
        if find_window():
            break
    if front and not SENDINPUT:
        time.sleep(1.0)
        user32.SetForegroundWindow(front)


def main(args):
    i = 0
    while i < len(args):
        c = args[i]
        i += 1
        if c == "launch":
            launch()
        elif c == "wait":
            time.sleep(float(args[i]))
            i += 1
        elif c == "key":
            name = args[i]
            i += 1
            ms = 30
            if i < len(args) and args[i].isdigit():
                ms = int(args[i])
                i += 1
            key(name, ms)
        elif c == "keys":
            while i < len(args) and args[i] not in COMMANDS:
                key(args[i])
                i += 1
        elif c in ("down", "up"):
            send(VK[args[i]], c == "down")
            i += 1
            time.sleep(0.05)
        elif c == "size":
            size(window(), int(args[i]), int(args[i + 1]))
            i += 2
        elif c == "grab":
            grab(window(), args[i])
            i += 1
        elif c == "raw":
            raw(window(), args[i])
            i += 1
        else:
            raise SystemExit(f"unknown command {c}")


if __name__ == "__main__":
    main(sys.argv[1:])
