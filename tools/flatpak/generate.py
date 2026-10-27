#!/usr/bin/env python3
"""Generate contrib/flatpak/cargo-sources.json and node-sources.json, the
offline sources of the Flatpak manifest, with the pinned flatpak-builder-tools
generators.

Usage, from the repository root (`just flatpak-sources`, in the container):

    python3 tools/flatpak/generate.py [--check] [--tools DIR]

The generators come from flatpak-builder-tools at the commit TOOLS_COMMIT,
fetched into a temporary directory (or taken from `--tools DIR`, a checkout
that must be at that commit). They need python3 with aiohttp, tomlkit and
PyYAML (the dev image's python3-aiohttp, python3-tomlkit, python3-yaml).

cargo-sources.json covers both lockfiles: Cargo.lock (the `keyvo` CLI) and
app/src-tauri/Cargo.lock (the app, outside the root workspace). The crates
both share appear once. node-sources.json covers app/package-lock.json.

`--check` generates into a temporary directory and fails when the committed
files differ, so a lockfile change without regeneration fails CI.

To bump the generators, move TOOLS_COMMIT, run the script, review the diff
of the two JSON files and build the Flatpak (`just flatpak` or the CI job).
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

TOOLS_REPO = "https://github.com/flatpak/flatpak-builder-tools"
# master of 2026-10-25
TOOLS_COMMIT = "9d86d336a39b9af38b86f114b6ce5820fe260f91"

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "contrib/flatpak"
CARGO_LOCKS = ["Cargo.lock", "app/src-tauri/Cargo.lock"]
NPM_LOCK = "app/package-lock.json"


def run(args: list[str], cwd: Path | None = None, env: dict | None = None) -> None:
    result = subprocess.run(
        args, cwd=cwd, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT
    )
    if result.returncode != 0:
        sys.stderr.write(result.stdout.decode(errors="replace"))
        sys.exit(f"{' '.join(args)}: exit {result.returncode}")


def fetch_tools(into: Path) -> Path:
    run(["git", "init", "--quiet", str(into)])
    run(["git", "fetch", "--quiet", "--depth", "1", TOOLS_REPO, TOOLS_COMMIT], cwd=into)
    run(["git", "checkout", "--quiet", "FETCH_HEAD"], cwd=into)
    return into


def check_commit(tools: Path) -> None:
    head = subprocess.run(
        ["git", "rev-parse", "HEAD"], cwd=tools, capture_output=True, text=True
    ).stdout.strip()
    if head != TOOLS_COMMIT:
        sys.exit(f"{tools} is at {head or 'no commit'}, expected {TOOLS_COMMIT}")


def merge(parts: list[list[dict]]) -> list[dict]:
    """The sources of every part in order, each once; two different sources
    for the same destination file are an error."""
    merged, seen = [], {}
    for part in parts:
        for source in part:
            key = (source.get("dest"), source.get("dest-filename"), source.get("url"))
            if key in seen:
                if seen[key] != source:
                    sys.exit(f"conflicting sources for {key}")
                continue
            seen[key] = source
            merged.append(source)
    return merged


def write(path: Path, sources: list[dict]) -> None:
    path.write_text(json.dumps(sources, indent=4) + "\n", encoding="utf-8")


def generate(tools: Path, out: Path, scratch: Path) -> None:
    parts = []
    for index, lock in enumerate(CARGO_LOCKS):
        part = scratch / f"cargo-{index}.json"
        run(
            [
                sys.executable,
                str(tools / "cargo/flatpak-cargo-generator.py"),
                str(ROOT / lock),
                "-o",
                str(part),
            ]
        )
        parts.append(json.loads(part.read_text(encoding="utf-8")))
    write(out / "cargo-sources.json", merge(parts))

    node = scratch / "node.json"
    env = dict(os.environ, PYTHONPATH=str(tools / "node"))
    run(
        [
            sys.executable,
            "-m",
            "flatpak_node_generator",
            "--no-requests-cache",
            "npm",
            str(ROOT / NPM_LOCK),
            "-o",
            str(node),
        ],
        cwd=scratch,
        env=env,
    )
    write(out / "node-sources.json", json.loads(node.read_text(encoding="utf-8")))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--check", action="store_true", help="fail on a diff")
    parser.add_argument("--tools", type=Path, help="a flatpak-builder-tools checkout")
    args = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="keyvo-flatpak-") as temp:
        temp = Path(temp)
        if args.tools:
            tools = args.tools.resolve()
            check_commit(tools)
        else:
            tools = fetch_tools(temp / "flatpak-builder-tools")
        out = temp / "out" if args.check else OUT
        out.mkdir(exist_ok=True)
        scratch = temp / "scratch"
        scratch.mkdir()
        generate(tools, out, scratch)

        if args.check:
            stale = [
                name
                for name in ("cargo-sources.json", "node-sources.json")
                if (out / name).read_bytes() != (OUT / name).read_bytes()
            ]
            if stale:
                sys.exit(
                    f"stale in contrib/flatpak/: {', '.join(stale)}; "
                    "run `just flatpak-sources` and commit the result"
                )
            print("contrib/flatpak sources match the lockfiles")


if __name__ == "__main__":
    main()
