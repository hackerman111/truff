#!/usr/bin/env python3
"""Measure whole native GUI process memory; preserve raw Linux accounting."""
import argparse
import json
import os
from pathlib import Path
import re
import tempfile
import time

from native_smoke import Window


def mappings(pid):
    totals = {}
    current = None
    for line in Path(f"/proc/{pid}/smaps").read_text().splitlines():
        if re.match(r"^[0-9a-f]+-[0-9a-f]+ ", line):
            parts = line.split(maxsplit=5)
            current = parts[5] if len(parts) == 6 else "[anonymous]"
        elif current and line.startswith("Rss:"):
            totals[current] = totals.get(current, 0) + int(line.split()[1])
    return sorted(totals.items(), key=lambda item: item[1], reverse=True)[:20]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=Path("target/release/truff"))
    parser.add_argument("--backends", nargs="+", default=["default", "vulkan", "gl"])
    parser.add_argument("--note-bytes", type=int, default=0)
    args = parser.parse_args()
    output = Path(tempfile.mkdtemp(prefix="truff-backends-"))
    samples = []
    original = os.environ.get("WGPU_BACKEND")
    try:
        for backend in args.backends:
            vault = output / backend
            vault.mkdir()
            if args.note_bytes:
                line = "# Заголовок 👩‍💻\n\nСодержимое **заметки** [[note]].\n"
                (vault / "note.md").write_text(line * (args.note_bytes // len(line.encode()) + 1))
            if backend == "default":
                os.environ.pop("WGPU_BACKEND", None)
            else:
                os.environ["WGPU_BACKEND"] = backend
            window = None
            started = time.monotonic()
            with (vault / "native.log").open("w") as log:
                try:
                    window = Window(args.binary.resolve(), vault, log)
                    time.sleep(3)
                    pid = window.process.pid
                    sample = {
                        "backend": backend,
                        "boundary": "whole native GUI process",
                        "requested_note_bytes": args.note_bytes,
                        "note_bytes": sum(path.stat().st_size for path in vault.glob("*.md")),
                        "elapsed_with_stabilization_seconds": time.monotonic() - started,
                        "stabilization_seconds": 3,
                        "egl_vendor_override": os.environ.get("__EGL_VENDOR_LIBRARY_FILENAMES"),
                        "window_size": window.client()["size"],
                        "status": Path(f"/proc/{pid}/status").read_text(),
                        "smaps_rollup": Path(f"/proc/{pid}/smaps_rollup").read_text(),
                        "largest_rss_mappings_kib": mappings(pid),
                    }
                    window.screenshot(vault / "window.png")
                    samples.append(sample)
                    rss = next(line for line in sample["status"].splitlines() if line.startswith("VmRSS:"))
                    print(f"{backend}: {rss}", flush=True)
                except Exception as error:
                    samples.append({"backend": backend, "error": str(error)})
                    print(f"{backend}: {error}", flush=True)
                finally:
                    if window is not None and window.process.poll() is None:
                        window.close()
            (output / "samples.json").write_text(json.dumps(samples, ensure_ascii=False, indent=2))
    finally:
        if original is None:
            os.environ.pop("WGPU_BACKEND", None)
        else:
            os.environ["WGPU_BACKEND"] = original
    print(f"Artifacts: {output}")


if __name__ == "__main__":
    main()
