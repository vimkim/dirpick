"""Result selection, direct mode keys, and scope navigation without Alt."""
import base64
import os
import re
import sys
import tempfile
import time
from pathlib import Path

from pty_support import Picker


binary = os.path.abspath(sys.argv[1])
with tempfile.TemporaryDirectory(prefix="dirpick-navigation-") as root:
    child = Path(root, "child")
    child.mkdir()
    for name in ("item-a", "item-b", "item-c"):
        directory = child / name
        directory.mkdir()
        (directory / "item.rs").write_text("item\n")
    for columns, rows in [(120, 32), (80, 24), (70, 18)]:
        picker = Picker(binary, str(child), columns, rows)
        try:
            def copied_path():
                # Audit each new OSC payload even if feedback is unchanged.
                count = len(re.findall(rb"\x1b\]52;c;([^\x07]*)\x07", picker.output))
                picker.send(b"\x19")
                deadline = time.monotonic() + 8
                while True:
                    payloads = re.findall(rb"\x1b\]52;c;([^\x07]*)\x07", picker.output)
                    if len(payloads) > count:
                        break
                    assert time.monotonic() < deadline, "Copy did not emit a new payload"
                    assert picker.read(), "Picker exited during copy"
                assert len(payloads) == count + 1
                return os.fsdecode(base64.b64decode(payloads[-1]))

            picker.send("item", "Query item")
            picker.wait("Results · 3")
            picker.send(b"\x1b", " NORMAL ")
            for key, label in [("d", "Directories"), ("c", "Contents"), ("f", "Files")]:
                picker.send(key, f"[{key} {label}]")
                picker.wait("Results · 3")
                for editing in (False, True):
                    if editing:
                        picker.send("i", " INSERT ")
                    first = copied_path()
                    picker.send(b"\x0e")  # Ctrl-N
                    assert copied_path() != first, (key, editing, "Ctrl-N")
                    picker.send(b"\x10")  # Ctrl-P
                    assert copied_path() == first, (key, editing, "Ctrl-P")
                    picker.wait("Query item")
                    picker.wait(" INSERT " if editing else " NORMAL ")
                picker.send(b"\x1b", " NORMAL ")

            # These keys remain ordinary text in Insert.
            picker.send("ifdcH", "Query itemfdcH")
            picker.wait("[f Files]")
            picker.send(b"\x15one two", "Query one two")
            # Former Alt word-editing shortcuts cannot change text or cursor.
            picker.send(b"\x1bb\x1bf\x1bd\x1b\x7f\x1b[1;3D\x1b[1;3A!")
            picker.wait("Query one two!")
            picker.wait(" INSERT ")
            picker.send(b"\x15item", "Query item")
            picker.wait("Results · 3")
            picker.send(b"\x1b", " NORMAL ")

            # H uses the active scope, independently of the selected file.
            picker.send(b"\x0e")
            before = copied_path()
            picker.send("H", "[Custom]")
            picker.wait("Query item", absent=True)
            picker.wait("child/item-a/item.rs")
            picker.wait(" NORMAL ")
            picker.send(" b", "Query item")
            picker.wait("[Project]")
            picker.wait("Results · 3")
            assert copied_path() == before, "Back did not restore the selected file"

            # The same parent action is discoverable through the Space guide.
            picker.send(" ", "Space → Shortcuts")
            picker.wait("Parent directory")
            picker.wait("Previous scope & search")
            picker.send("u", "[Custom]")
            picker.wait("Query item", absent=True)
            picker.send(" b", "Query item")
            picker.wait("[Project]")
            picker.send(b"\x03")
            assert picker.finish() == b"", "Terminal output leaked to stdout"
        finally:
            picker.close()
