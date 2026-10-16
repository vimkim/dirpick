#!/usr/bin/env python3
"""Reproduce the notice bundle from locked Cargo registry sources; no downloads."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parent.parent
UNLINKED = {
    "r-efi": "UEFI-only dependency of getrandom; not built for the Linux release",
    "winapi-i686-pc-windows-gnu": "Windows GNU import libraries; not built for the Linux release",
    "winapi-x86_64-pc-windows-gnu": "Windows GNU import libraries; not built for the Linux release",
}


def generate():
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"], cwd=ROOT
    ))
    header = """Third-party notices for Dirpick 0.1.0

This bundle covers the complete locked Cargo dependency inventory, including
development and platform-specific crates, and the embedded syntax definitions.
It does not assign a license to Dirpick itself. Cargo.lock supplies exact
versions and registry checksums. docs/dependency-notices.json records selected
upstream files and their SHA-256 values. The selected MIT option applies where
upstream offers MIT or Apache-2.0. Required additional notices are retained.
Non-Linux packages without license files in their crate archive are identified
in the inventory; they are not built into the supported Linux command.

License text below is copied verbatim from the locked crate archives. Regenerate
with python3 -B scripts/dependency_notices.py after cargo fetch --locked.

Syntect 5.3.0 embeds assets/default_newlines.packdump. Its release Makefile builds
that pack from sublimehq/Packages at fa6b8629c95041bf262d4c1dab95c456a0530122.
The default theme dump is disabled. Dirpick constructs the approved Mocha theme.

https://github.com/trishume/syntect/tree/v5.3.0
https://github.com/trishume/syntect/blob/v5.3.0/Makefile
https://github.com/sublimehq/Packages/tree/fa6b8629c95041bf262d4c1dab95c456a0530122
"""
    output = bytearray(header.encode())
    inventory = []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if package["name"] == "dirpick":
            continue
        directory = Path(package["manifest_path"]).parent
        candidates = sorted(path for path in directory.iterdir() if path.is_file()
                            and path.name.lower().startswith(("license", "licence", "copying", "notice", "unlicense")))
        mit = [path for path in candidates if path.name.lower().replace("_", "-") in ("license-mit", "license-mit.md")]
        # Preserve generic LICENSE files too: these can contain embedded-code
        # attribution beyond the selected crate-level license.
        if mit:
            selected = [path for path in candidates if path in mit
                        or not path.name.lower().startswith(("license-", "license_", "unlicense"))
                        or "unicode" in path.name.lower()]
        elif any(path.name == "LICENSE-BOOST" for path in candidates):
            selected = [path for path in candidates if path.name != "LICENSE-APACHE"]
        else:
            selected = candidates
        record = {"name": package["name"], "version": package["version"],
                  "license_expression": package["license"], "repository": package["repository"], "notices": []}
        output.extend(("\n\n" + "=" * 72 + "\n" + package["name"] + " " + package["version"]
                       + "\nhttps://crates.io/crates/" + package["name"] + "/" + package["version"] + "\n").encode())
        if not selected:
            if package["name"] not in UNLINKED:
                raise SystemExit(f"Review required: no upstream license files for {package['name']}")
            record["linux_release"] = UNLINKED[package["name"]]
            output.extend(("\n" + record["linux_release"] + ".\n"
                           "Upstream crate archive has no license text; metadata declares "
                           + package["license"] + ".\n").encode())
        for path in selected:
            content = path.read_bytes()
            record["notices"].append({"file": path.name, "sha256": hashlib.sha256(content).hexdigest()})
            output.extend(("\n--- upstream " + path.name + " ---\n\n").encode())
            output.extend(content)
            if not content.endswith(b"\n"):
                output.extend(b"\n")
        inventory.append(record)
    output.extend(b"\n\n")
    output.extend((ROOT / "notices/syntax.txt").read_bytes())
    return {ROOT / "THIRD_PARTY_NOTICES.txt": bytes(output),
            ROOT / "docs/dependency-notices.json": (json.dumps(inventory, indent=2, ensure_ascii=False) + "\n").encode()}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="Fail if committed notice files differ")
    arguments = parser.parse_args()
    for path, content in generate().items():
        if arguments.check:
            if not path.exists() or path.read_bytes() != content:
                raise SystemExit(f"Notice bundle is stale: {path.relative_to(ROOT)}")
        else:
            path.write_bytes(content)
