"""Validate a temporary Cargo installation, with no source or PATH runtime data."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from pty_support import Picker

binary = os.path.abspath(sys.argv[1])
repository = Path(__file__).resolve().parent.parent
for option, artifact in [("--print-nu-wrapper", "integrations/dirpick.nu"),
                         ("--third-party-notices", "THIRD_PARTY_NOTICES.txt")]:
    result = subprocess.run([binary, option], capture_output=True, check=True, env={"PATH": ""})
    assert result.stdout == (repository / artifact).read_bytes(), f"Installed {artifact} differs"
    assert result.stderr == b"", result.stderr
assert b"dirpick" in subprocess.check_output([binary, "--help"], env={"PATH": ""})
with tempfile.TemporaryDirectory(prefix="dirpick-installed-") as root:
    with open(os.path.join(root, "installed-syntax.rs"), "w") as source:
        source.write("fn main() {} // embedded Rust syntax\n")
    picker = Picker(binary, root, 120, 32, environment={"PATH": ""})
    try:
        picker.wait("installed-syntax.rs")
        picker.wait("embedded Rust syntax")
        assert b"38;2;203;166;247" in picker.output, "Installed syntax data is missing"
        picker.wait("CWD fallback")
        picker.send(b"\x03")
        assert picker.finish() == b""
    finally:
        picker.close()
