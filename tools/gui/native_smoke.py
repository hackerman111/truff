#!/usr/bin/env python3
"""Native keyboard smoke test for the Lua Hyprland API; uses a fresh temporary vault.

Requires hyprctl and grim. Keys and close requests address only the spawned PID.
Restores the keyboard layout on exit. Leaves its vault/log/screenshots in /tmp.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time


def hypr(*args):
    result = subprocess.run(["hyprctl", *args], text=True, capture_output=True, check=True)
    if "warning:" in result.stdout or "error:" in result.stdout:
        raise RuntimeError(result.stdout)
    return result.stdout


def wait(predicate, message, timeout=15):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(.05)
    raise RuntimeError(message)


class Window:
    def __init__(self, binary, vault, log):
        self.process = subprocess.Popen([str(binary), "--vault", str(vault)], stdout=log, stderr=log)
        self.selector = f"pid:{self.process.pid}"
        try:
            wait(lambda: self.client() is not None, "Truff window did not appear")
            time.sleep(1)
        except BaseException:
            # Construction can fail before the caller receives this owned child.
            if self.process.poll() is None:
                self.process.terminate()
                try:
                    self.process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    self.process.kill()
                    self.process.wait()
            raise

    def client(self):
        return next((c for c in json.loads(hypr("-j", "clients")) if c["pid"] == self.process.pid), None)

    def key(self, symbol, mods=""):
        for state in ("down", "up"):
            code = "hl.dispatch(hl.dsp.send_key_state({mods=" + json.dumps(mods) + ",key=" + json.dumps(symbol) + ",state=" + json.dumps(state) + ",window=" + json.dumps(self.selector) + "}))"
            hypr("eval", code)
        time.sleep(.04)

    def text(self, value):
        punctuation = {" ": ("space", ""), ":": ("semicolon", "SHIFT"), "#": ("3", "SHIFT"), ".": ("period", ""), "/": ("slash", ""), "-": ("minus", "")}
        for ch in value:
            symbol, mods = punctuation.get(ch, (ch.lower(), "SHIFT" if ch.isupper() else ""))
            self.key(symbol, mods)

    def command(self, value):
        self.text(":" + value)
        self.key("Return")
        time.sleep(.4)

    def screenshot(self, path):
        client = self.client()
        x, y = client["at"]
        width, height = client["size"]
        subprocess.run(["grim", "-g", f"{x},{y} {width}x{height}", str(path)], check=True)

    def close(self):
        hypr("eval", "hl.dispatch(hl.dsp.window.close({window=" + json.dumps(self.selector) + "}))")
        self.process.wait(timeout=15)
        assert self.process.returncode == 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/debug/truff"))
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    vault = Path(tempfile.mkdtemp(prefix="truff-native-"))
    (vault / "a.md").write_text("alpha beta\n", encoding="utf-8")
    (vault / "b.md").write_text("# Target\n\n[[a]]\n", encoding="utf-8")
    keyboards = json.loads(hypr("-j", "devices"))["keyboards"]
    keyboard = next(k for k in keyboards if k["main"])
    original_layout = keyboard["active_layout_index"]
    english_layout = [part.strip() for part in keyboard["layout"].split(",")].index("us")
    window = None
    try:
        hypr("switchxkblayout", keyboard["name"], str(english_layout))
        with (vault / "native.log").open("w") as log:
            window = Window(binary, vault, log)
            window.key("w", "CTRL"); window.key("h")
            window.key("j"); window.key("Return")
            time.sleep(.4)
            window.screenshot(vault / "panes.png")
            window.key("w", "CTRL"); window.key("h")
            window.text("gg"); window.key("Return")
            time.sleep(.4)
            window.key("w", "CTRL"); window.key("l")
            window.key("w", "CTRL"); window.key("h")
            window.key("i"); window.text("Hello "); window.key("Escape")
            window.key("s"); window.key("d"); window.text("ve")
            window.command("color #3388ff"); window.command("w")
            wait(lambda: 'color:#3388ff' in (vault / 'a.md').read_text(), "Color/save failed")
            window.screenshot(vault / "color.png")
            window.text("gg0ve"); window.command("link b"); window.command("w")
            wait(lambda: '[[b|Hello]]' in (vault / 'a.md').read_text(), "Link/save failed")
            window.command("follow b#target"); window.command("backlinks")
            window.screenshot(vault / "backlinks.png")
            window.key("Return"); window.command("rename renamed.md")
            wait(lambda: (vault / 'renamed.md').exists() and not (vault / 'a.md').exists(), "Rename failed")
            assert '[[renamed' in (vault / 'b.md').read_text()
            window.command("w")
            expected = (vault / 'renamed.md').read_text()
            window.screenshot(vault / "renamed.png")
            window.close(); window = None
            assert 'renamed.md' in (vault / '.truff/session.toml').read_text()
            window = Window(binary, vault, log)
            window.screenshot(vault / "restart.png")
            rss = next(line for line in Path(f"/proc/{window.process.pid}/status").read_text().splitlines() if line.startswith("VmRSS:"))
            smaps = Path(f"/proc/{window.process.pid}/smaps_rollup").read_text()
            stat = lambda: Path(f"/proc/{window.process.pid}/stat").read_text().split(") ",1)[1].split()
            before = stat()
            started = time.monotonic()
            time.sleep(30)
            duration = time.monotonic() - started
            after = stat()
            idle_ticks = int(after[11])+int(after[12])-int(before[11])-int(before[12])
            sample = {
                "boundary": "whole native GUI process, two-note vault, after restart",
                "binary": str(binary),
                "profile_directory": binary.parent.name,
                "host": list(os.uname()),
                "window_size": window.client()["size"],
                "rss": rss,
                "smaps_rollup": smaps,
                "idle_seconds": duration,
                "idle_cpu_ticks": idle_ticks,
                "clock_ticks_per_second": os.sysconf("SC_CLK_TCK"),
                "idle_cpu_one_core_percent": 100 * idle_ticks / os.sysconf("SC_CLK_TCK") / duration,
            }
            (vault / "process-sample.json").write_text(json.dumps(sample, indent=2))
            assert (vault / 'renamed.md').read_text() == expected
            window.close(); window = None
        print(f"PASS native keyboard workflow; artifacts: {vault}")
    finally:
        try:
            if window is not None and window.process.poll() is None:
                window.close()
        finally:
            hypr("switchxkblayout", keyboard["name"], str(original_layout))


if __name__ == "__main__":
    main()
