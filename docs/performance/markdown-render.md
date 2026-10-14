# Markdown lists — 2026-10-14

Live Preview renders unordered lists with cycling markers `•`, `◦`, `▪`, task
checkboxes, and numbered lists. Wrapped rows align with the item text.
Task clicks use one buffer transaction and support undo. Block context comes
from the existing incremental Markdown tree; inline projection uses the existing
pulldown-cmark dependency. Plain documents bypass the block tree. Visible line
context and galleys are cached by buffer revision.

## Measurement

Run sequentially with an optimized build:

```sh
rtk cargo run -p truff-ui --example ui_perf --release --offline
rtk cargo run -p truff-ui --example markdown_render_perf --release --offline
```

The existing fixture contains 30,000 plain lines with inline Markdown and Unicode.
Times are averages over 100 frames, in milliseconds.

| Scenario | Before | After |
| --- | ---: | ---: |
| Idle | 0.041 | 0.042 |
| Insert character | 0.090 | 0.086 |
| Insert newline | 0.135 | 0.128 |
| Move down | 0.061 | 0.062 |

These small differences do not establish a speed improvement. Layout build counts
and cached paragraph counts remained the same.

The list fixture repeats a parent, a child, a task with Unicode, and a blank line.
The insertion measurement changes the first item and paints the production editor
30 times; idle paints 100 times.

| Lines | First frame, ms | Insert and paint, ms | Idle, ms | Cached paragraphs |
| --- | ---: | ---: | ---: | ---: |
| 1,000 | 7.28 | 0.42 | 0.039 | 30 |
| 30,000 | 81.81 | 5.85 | 0.039 | 30 |

Idle built zero new layouts. First-frame costs include initializing the block tree.
These are CPU measurements through headless egui, without a platform window or GPU.
They cover this fixture, not every possible large list or Markdown document.

## Verification

276 workspace tests passed. Formatting, workspace checks with all targets,
Clippy with all targets and features and warnings denied, and the diff whitespace
check passed. Tests cover nested and mixed lists, UTF-8 coordinate mapping,
literal code blocks, changing fences above cached rows, wrapping, pointer task
toggle, undo, and unchanged-frame layout reuse.
