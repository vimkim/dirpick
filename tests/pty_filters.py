"""Exercise visible filter editing and traversal controls through a real PTY."""
import os
import sys
import tempfile
from pty_support import Picker

binary = sys.argv[1]
with tempfile.TemporaryDirectory(prefix="dirpick-filters-") as root:
    for name in ["kept.rs", "notes.txt", ".secret.rs", "ignored.rs"]:
        with open(os.path.join(root, name), "w") as source:
            source.write(f"preview of {name}\n")
    with open(os.path.join(root, ".ignore"), "w") as rules:
        # Keep the rule's preview text distinct from the candidate name so an
        # absent-result assertion cannot mistake source text for a result.
        rules.write("ignored.*\n!.secret.rs\n")
    for columns, rows in [(120, 32), (70, 18)]:
        picker = Picker(binary, root, columns, rows, environment={"NO_COLOR": "1"})
        wait_for, send = picker.wait, picker.send
        try:
            wait_for("Include")
            wait_for("Exclude")
            wait_for("Hidden:off")
            wait_for("Ignored:off")
            wait_for("kept.rs")
            send(b"\t[", "Invalid include regex")
            wait_for("kept.rs", absent=True)
            send(b"\x15\\.rs$", "kept.rs")
            send(b"\tkept", "No matching files")
            send(b"\x15[", "Invalid exclude regex")
            send(b"\x15", "kept.rs")
            # Shift-Tab returns from Exclude to Include; clearing Include reveals notes.
            send(b"\x1b[Z\x15", "notes.txt")
            send(b"\x1b[Zno-such", "No matching files")
            send(b"\x15", "kept.rs")
            # Query -> Include -> Exclude -> Scope, then reverse the whole cycle.
            send(b"\t\t\t", "> Scope")
            send(b"\x1b[Z\x1b[Z\x1b[Z", "> Query")
            send(b"\x1b", " NORMAL ")
            send(b" h", "Hidden:on")  # Space h
            wait_for(".secret.rs")
            wait_for("ignored.rs", absent=True)
            send(b" i", "Ignored:on")  # Space i
            wait_for("ignored.rs")
            send(b" h", "Hidden:off")
            wait_for(".secret.rs", absent=True)
            wait_for("ignored.rs")
            send(b"\t\tiignored", "ignored.rs", absent=True)
            send(b"\x1b", " NORMAL ")
            send(b" ?", "Keyboard help")
            wait_for("Space h / i")
            send(b"\x1b", "Keyboard help", absent=True)
            picker.send(b"\x03")
            assert picker.finish() == b""
        finally:
            picker.close()
