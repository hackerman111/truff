#!/usr/bin/env python3
"""Capture real eframe windows in a private Xvfb server; leaves the desktop alone."""
import argparse
import ctypes as C
import csv
import os
from pathlib import Path
import subprocess
import tempfile
import time


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--theme", choices=["nord", "dark", "light", "comfortable"], default="nord")
    parser.add_argument("--surfaces", action="store_true", help="Capture P1 creation/settings/dock/focus surfaces too")
    parser.add_argument("--p2", action="store_true", help="Capture Navigator providers and command/file workflows")
    providers = [("folders", 1), ("content", 2), ("mixed", 3), ("sessions", 4), ("recent", 5), ("vaults", 6), ("outline", 7), ("links", 8), ("commands", 9), ("colors", 10), ("boards", 11), ("tasks", 12)]
    parser.add_argument("--only-provider", choices=[name for name, _ in providers])
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    args.out.mkdir(parents=True, exist_ok=True)
    x = C.CDLL("libX11.so.6")
    xt = C.CDLL("libXtst.so.6")
    display_type = C.c_void_p
    window_type = C.c_ulong
    x.XOpenDisplay.argtypes = [C.c_char_p]
    x.XOpenDisplay.restype = display_type
    x.XDefaultRootWindow.argtypes = [display_type]
    x.XDefaultRootWindow.restype = window_type
    x.XQueryTree.argtypes = [display_type, window_type, C.POINTER(window_type), C.POINTER(window_type), C.POINTER(C.POINTER(window_type)), C.POINTER(C.c_uint)]
    x.XFetchName.argtypes = [display_type, window_type, C.POINTER(C.c_void_p)]
    x.XFree.argtypes = [C.c_void_p]
    x.XStringToKeysym.argtypes = [C.c_char_p]
    x.XStringToKeysym.restype = C.c_ulong
    x.XKeysymToKeycode.argtypes = [display_type, C.c_ulong]
    x.XKeysymToKeycode.restype = C.c_uint
    x.XSetInputFocus.argtypes = [display_type, window_type, C.c_int, C.c_ulong]
    x.XMapWindow.argtypes = [display_type, window_type]
    x.XResizeWindow.argtypes = [display_type, window_type, C.c_uint, C.c_uint]
    x.XMoveWindow.argtypes = [display_type, window_type, C.c_int, C.c_int]
    x.XFlush.argtypes = [display_type]
    x.XCloseDisplay.argtypes = [display_type]
    xt.XTestFakeKeyEvent.argtypes = [display_type, C.c_uint, C.c_int, C.c_ulong]

    with tempfile.TemporaryDirectory(prefix="truff-p0-gui-") as tmp:
        root = Path(tmp)
        config = root / "config" / "truff"
        config.mkdir(parents=True)
        (config / "config.toml").write_text(f'theme = "{args.theme}"\n' + ('[themes.comfortable]\ndensity = "comfortable"\n' if args.theme == "comfortable" else ''))
        vault = root / "vault"
        vault.mkdir()
        for i in range(1000):
            (vault / f"note-{i:04}.md").write_text("# Heading\n\nword 界 👩‍💻\n", encoding="utf-8")
        if args.p2:
            (vault / "note-0000.md").write_text("# Heading\n\nword 界 👩‍💻\n\n- [ ] first task\n- [x] finished task\n\n[[note-0001]]\n\nSection\n-------\n", encoding="utf-8")
            (vault / "docs" / "nested").mkdir(parents=True)
            (vault / "docs" / "nested" / "guide.md").write_text("# Guide\nword\n")
        number = next(i for i in range(190, 240) if not Path(f"/tmp/.X11-unix/X{i}").exists())
        display_name = f":{number}"
        with (args.out / (f"native-{args.only_provider}.log" if args.only_provider else "native.log")).open("w") as log:
            server = subprocess.Popen(["Xvfb", display_name, "-screen", "0", "2000x1200x24", "-nolisten", "tcp"], stdout=log, stderr=log)
            process = None
            display = None
            try:
                for _ in range(100):
                    display = x.XOpenDisplay(display_name.encode())
                    if display:
                        break
                    time.sleep(.05)
                if not display:
                    raise RuntimeError("Xvfb did not start")
                env = dict(os.environ, DISPLAY=display_name, WAYLAND_DISPLAY="", WINIT_UNIX_BACKEND="x11", XDG_CONFIG_HOME=str(root / "config"), XDG_STATE_HOME=str(root / "state"))
                process = subprocess.Popen([str(binary), "--vault", str(vault)], env=env, stdout=log, stderr=log)
                def find_window():
                    children = C.POINTER(window_type)()
                    count = C.c_uint()
                    parent, tree_root = window_type(), window_type()
                    x.XQueryTree(display, x.XDefaultRootWindow(display), C.byref(tree_root), C.byref(parent), C.byref(children), C.byref(count))
                    found = None
                    for i in range(count.value):
                        name = C.c_void_p()
                        x.XFetchName(display, children[i], C.byref(name))
                        if name.value:
                            title = C.string_at(name).decode(errors="replace")
                            x.XFree(name)
                            if "truff" in title.lower():
                                found = children[i]
                    if children:
                        x.XFree(children)
                    return found
                window = None
                for _ in range(200):
                    if process.poll() is not None:
                        raise RuntimeError(f"Truff exited {process.returncode}")
                    window = find_window()
                    if window:
                        break
                    time.sleep(.05)
                if not window:
                    raise RuntimeError("Truff window did not appear")
                x.XMapWindow(display, window)
                x.XFlush(display)
                time.sleep(.5)
                x.XSetInputFocus(display, window, 1, 0)
                x.XMoveWindow(display, window, 0, 0)
                x.XFlush(display)
                time.sleep(.5)
                def key(symbol, shift=False, ctrl=False):
                    code = x.XKeysymToKeycode(display, x.XStringToKeysym(symbol.encode()))
                    shift_code = x.XKeysymToKeycode(display, x.XStringToKeysym(b"Shift_L"))
                    ctrl_code = x.XKeysymToKeycode(display, x.XStringToKeysym(b"Control_L"))
                    if ctrl:
                        xt.XTestFakeKeyEvent(display, ctrl_code, 1, 0)
                    if shift:
                        xt.XTestFakeKeyEvent(display, shift_code, 1, 0)
                    xt.XTestFakeKeyEvent(display, code, 1, 0)
                    xt.XTestFakeKeyEvent(display, code, 0, 0)
                    if shift:
                        xt.XTestFakeKeyEvent(display, shift_code, 0, 0)
                    if ctrl:
                        xt.XTestFakeKeyEvent(display, ctrl_code, 0, 0)
                    x.XFlush(display)
                    time.sleep(.025)
                memory_rows = []
                def shot(name):
                    time.sleep(.5)
                    values = {}
                    for filename in ["status", "smaps_rollup"]:
                        for line in Path(f"/proc/{process.pid}/{filename}").read_text().splitlines():
                            parts = line.split()
                            if parts and parts[0] in ["VmRSS:", "VmHWM:", "Pss:"]:
                                values[parts[0][:-1]] = int(parts[1])
                    memory_rows.append([name, values.get("VmRSS"), values.get("VmHWM"), values.get("Pss")])
                    subprocess.run(["import", "-display", display_name, "-window", str(window), str(args.out / name)], check=True)
                def command(text):
                    key("semicolon", shift=True)
                    for character in text:
                        key({" ": "space", "-": "minus", ".": "period", "/": "slash"}.get(character, character))
                    key("Return")
                if args.theme != "nord":
                    command(f"theme {args.theme}")
                for width, height in [(800, 600), (1280, 720), (1920, 1080)]:
                    x.XResizeWindow(display, window, width, height)
                    x.XFlush(display)
                    time.sleep(.5)
                    if args.only_provider:
                        command("open note-0000.md")
                        time.sleep(.5)
                        key("space")
                        key("f")
                        for _ in range(dict(providers)[args.only_provider]):
                            key("Tab")
                        shot(f"{args.only_provider}-{width}x{height}.png")
                        key("Escape")
                        continue
                    key("slash", shift=True)
                    shot(f"help-{width}x{height}.png")
                    key("g", shift=True)
                    shot(f"help-last-{width}x{height}.png")
                    key("Escape")
                    key("space")
                    key("f")
                    shot(f"navigator-{width}x{height}.png")
                    for _ in range(100):
                        key("j")
                    shot(f"navigator-down-{width}x{height}.png")
                    key("Escape")
                    if args.p2:
                        command("open note-0000.md")
                        time.sleep(.5)
                        for name, tabs in providers:
                            key("space")
                            key("f")
                            for _ in range(tabs):
                                key("Tab")
                            if name in ["content", "mixed"]:
                                key("slash")
                                for ch in "word":
                                    key(ch)
                                key("Escape")
                                time.sleep(1)
                            if name == "sessions":
                                key("p", shift=True)
                            shot(f"{name}-{width}x{height}.png")
                            if name == "content":
                                key("bracketright")
                                shot(f"content-page2-{width}x{height}.png")
                            key("Escape")
                        for name, symbol, shifted in [("create-file", "a", False), ("create-folder", "a", True), ("rename", "r", False), ("trash-confirm", "d", False)]:
                            key("space")
                            key("f")
                            key("slash")
                            for ch in "note-0000":
                                key("minus" if ch == "-" else ch)
                            key("Escape")
                            key(symbol, shift=shifted)
                            shot(f"{name}-{width}x{height}.png")
                            key("Escape")
                            shot(f"file-loop-{name}-{width}x{height}.png")
                            key("Escape")
                        key("semicolon", shift=True)
                        for ch in "panel":
                            key(ch)
                        shot(f"command-completion-{width}x{height}.png")
                        key("Escape")
                        shot(f"command-normal-{width}x{height}.png")
                        key("Escape")
                        key("v")
                        key("l")
                        key("space")
                        key("c")
                        shot(f"visual-color-{width}x{height}.png")
                        key("Escape")
                        key("Escape")
                    if args.surfaces:
                        key("w", ctrl=True)
                        key("h")
                        for _ in range(100):
                            key("j")
                        shot(f"sidebar-{width}x{height}.png")
                        key("w", ctrl=True)
                        key("l")
                        for name, shifted in [("file",False),("folder",True)]:
                            key("space")
                            key("f")
                            key("a", shift=shifted)
                            time.sleep(.2)
                            for character in "session":
                                key(character)
                            shot(f"create-{name}-{width}x{height}.png")
                            key("Escape")
                            if args.p2:
                                key("Escape")
                        command("settings")
                        shot(f"settings-{width}x{height}.png")
                        for _ in range(12):
                            key("Tab")
                        shot(f"settings-tab-{width}x{height}.png")
                        key("Escape")
                        command("focus")
                        shot(f"focus-{width}x{height}.png")
                        command("focus")
                with (args.out / (f"memory-{args.only_provider}.csv" if args.only_provider else "memory.csv")).open("w", newline="") as memory_file:
                    writer = csv.writer(memory_file)
                    writer.writerow(["surface", "rss_kib", "high_water_kib", "pss_kib"])
                    writer.writerows(memory_rows)
            finally:
                if process and process.poll() is None:
                    process.terminate()
                    process.wait(timeout=10)
                if display:
                    x.XCloseDisplay(display)
                server.terminate()
                server.wait(timeout=10)


if __name__ == "__main__":
    main()
