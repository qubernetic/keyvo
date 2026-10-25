# Diagrams

Diagrams as code with [archify](https://github.com/tt-a1i/archify) (Apache-2.0).
The JSON sources under `src/` are the tracked truth; the rendered standalone
HTML next to this file is tracked so the diagrams open without the tool.
`just diagrams` and the CI freshness check land with issue #4; until then,
regenerate by hand with archify pinned at tag `v3.2.0`:

```bash
git clone --depth 1 --branch v3.2.0 https://github.com/tt-a1i/archify.git ~/.cache/archify-v3.2.0
export ARCHIFY_CHROME=<path to a Chromium or Chrome binary>   # the browser-check gate
A=~/.cache/archify-v3.2.0/archify/bin/archify.mjs
node $A finalize workflow docs/diagrams/src/options-plus-display-pipeline.workflow.json docs/diagrams/options-plus-display-pipeline.html --quality showcase --json
node $A finalize sequence docs/diagrams/src/options-plus-page-paint.sequence.json docs/diagrams/options-plus-page-paint.html --quality showcase --json
```

Run the commands from the repository root; `meta.output` in every source is
relative to it. `finalize` writes receipts (`*.finalize.json`,
`*.finalize-summary.json`, `*.delivery.json`, `*.browser-check.json`) next to
the HTML; they are build evidence, not sources, and are ignored by git.

| Diagram | Type | What it shows | Source of the facts |
|---|---|---|---|
| `options-plus-display-pipeline.html` | workflow | How Logi Options+ turns a redraw request into one `0x19A1` fn2 `setImage` command: one 480x480 canvas, bounding-box crop, JPEG encode and band split, the helper's queue of two, one image per command | `docs/research/options-plus-display-pipeline.md` |
| `options-plus-page-paint.html` | sequence | The messages of a page change (one 435x434 image), a single-button change (one 118x118 image) and the keep-alive beside them | same report and the hcooper capture, frame 521 |

Both diagrams are reference material for the composite paint of the pre-alpha
epic (#34): the facts are Options+ behaviour measured and decompiled within the
boundary of ADR-0008, not keyvo's design.
