"""Space guide, visible modes, field spaces, and accessible colors on a real PTY."""
import os
import re
import sys
import tempfile
from pty_support import Picker

binary = os.path.abspath(sys.argv[1])
with tempfile.TemporaryDirectory(prefix="dirpick-shortcuts-") as root:
    child = os.path.join(root, "scope with spaces")
    os.mkdir(child)
    for directory in (root, child):
        with open(os.path.join(directory, "two words.rs"), "w") as source:
            source.write("// two words\n")
    for columns, rows in [(120, 32), (80, 24), (70, 18)]:
        picker = Picker(binary, root, columns, rows,
                        environment={"NO_COLOR": "1"} if columns == 80 else {})
        try:
            picker.wait("[f Files]")
            header = picker.screen.text().splitlines()[0]
            for label in ("f Files", "d Directories", "c Contents", "Space Shortcuts"):
                assert label in header, header
            picker.send(" ", "Space → Shortcuts")
            for label in ("Files", "Directories", "Contents", "Content regex",
                          "Project / CWD", "Hidden paths", "Ignored paths",
                          "Results / Preview", "Help & full feedback",
                          "Quit", "Insert a space", "Esc cancel"):
                picker.wait(label)
            picker.send("x", "Space → Shortcuts")  # Unknown keys cannot edit fields.
            picker.send(b"\x1b", "Space → Shortcuts", absent=True)
            picker.wait("Query x", absent=True)
            picker.send("two", "Query two")  # Direct typing still searches.
            picker.send(" ", "Space → Shortcuts")
            picker.send(b"\x1b", "Space → Shortcuts", absent=True)
            picker.wait("Query two")
            picker.send("  words", "Query two words")  # Two Spaces insert one.
            picker.send(" c", "[c Contents]")
            picker.wait("Query two words")
            picker.send(" r", "Regex:on")
            picker.send(" r", "Regex:off")
            picker.send(" d", "[d Directories]")
            picker.send(" f", "[f Files]")
            picker.send("\ttwo  words", "Include two words")
            picker.send(" h", "Hidden:on")  # Leader works while editing Include.
            picker.wait("Include two words")
            picker.send(" i", "Ignored:on")
            picker.send("\tskip  me", "Exclude skip me")
            picker.send("\x0c\x15scope  with  spaces", "Scope scope with spaces")
            picker.send("\r", "[Custom]")
            picker.wait("> Query")
            picker.wait("two words.rs")
            picker.wait("Include two words")
            picker.wait("Exclude skip me")
            picker.send(" ", "Space → Shortcuts")
            for width, height in [(60, 12), (120, 32), (columns, rows)]:
                picker.resize(width, height)
                picker.wait("Space → Shortcuts")
                picker.wait("Esc cancel")
            picker.send("?", "Keyboard help")
            picker.send(b"\x1b", "Keyboard help", absent=True)
            picker.wait("Include two words")
            picker.send(" q")
            assert picker.finish() == b""
            if columns == 80:
                assert not re.search(rb"\x1b\[[0-9;]*(?:38|48);(?:2|5);", picker.output)
            else:
                for rgb in (b"166;227;161", b"137;180;250", b"250;179;135"):
                    assert b"38;2;" + rgb in picker.output, f"Mode accent missing: {rgb!r}"
        finally:
            picker.close()
