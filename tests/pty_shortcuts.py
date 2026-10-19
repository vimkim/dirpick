"""Modal editing, Space guide, navigation, and accessible colors on a real PTY."""
import base64
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
            for label in ("f Files", "d Directories", "c Contents", "Esc Normal mode"):
                assert label in header, header
            picker.wait(" INSERT ")
            picker.send("two words", "Query two words")  # Startup accepts text immediately.
            picker.send(b"\x15", "Query two words", absent=True)
            picker.send(b"\x1b", " NORMAL ")
            picker.wait("Space Shortcuts")
            picker.wait("q Quit")
            picker.send("x", "Query x", absent=True)  # Normal typing cannot edit fields.
            picker.send(" ", "Space → Shortcuts")
            for label in ("Files", "Directories", "Contents", "Content regex",
                          "Project / CWD", "Hidden paths", "Ignored paths",
                          "Results / Preview", "Help & full feedback",
                          "Parent directory", "Previous scope & search",
                          "Quit", "Edit Query", "Esc cancel"):
                picker.wait(label)
            picker.send("x", "Space → Shortcuts")  # Unknown keys cannot edit fields.
            picker.send(b"\x1b", "Space → Shortcuts", absent=True)
            picker.wait("Query x", absent=True)
            picker.send(b"\x1b", " NORMAL ")  # Esc in Normal never quits.
            picker.send("i", " INSERT ")
            picker.send("two words", "Query two words")  # Ordinary spaces in Insert.
            picker.wait("Space → Shortcuts", absent=True)
            picker.send("q jk /?", "Query two wordsq jk /?")  # Commands are literal text.
            picker.send(b"\x15two words", "Query two words")
            picker.wait("Scanning", absent=True)
            picker.send(b"\x1b", " NORMAL ")

            def copied_path():
                picker.send(b"\x19", "payload sent")
                payloads = re.findall(rb"\x1b\]52;c;([^\x07]*)\x07", picker.output)
                return base64.b64decode(payloads[-1])

            selected = copied_path()
            picker.send("j")
            assert copied_path() != selected, "j did not select the next result"
            picker.send("k")
            assert copied_path() == selected, "k did not restore the previous result"
            picker.wait("Query two words")
            picker.send(" c", "[c Contents]")
            picker.wait("Query two words")
            picker.send(" r", "Regex:on")
            picker.send(" r", "Regex:off")
            picker.send(" d", "[d Directories]")
            picker.send(" f", "[f Files]")
            picker.send("\titwo words", "Include two words")
            picker.send(b"\x1b", " NORMAL ")
            picker.send(" h", "Hidden:on")  # Focused fields survive a mode change.
            picker.wait("Include two words")
            picker.send(" i", "Ignored:on")
            picker.send("\tiskip me", "Exclude skip me")
            picker.send(b"\x1b", " NORMAL ")
            picker.send("/", " INSERT ")  # / focuses Query from another field.
            picker.send(b"\x15two words", "Query two words")
            picker.wait("Exclude skip me")
            picker.send("\x0c\x15scope with spaces", "Scope scope with spaces")
            picker.wait("Enter Apply Scope")  # Keep the hint complete at 70 columns.
            picker.send(b"\x1b", " NORMAL ")
            picker.wait("Enter Apply Scope")
            picker.wait("q Quit")
            picker.send("i", " INSERT ")
            picker.send("\r", "[Custom]")
            picker.wait("> Query")
            picker.wait("two words.rs")
            picker.wait("Include two words")
            picker.wait("Exclude skip me")
            picker.send(b"\x1b", " NORMAL ")
            picker.send(" ", "Space → Shortcuts")
            for width, height in [(60, 12), (120, 32), (columns, rows)]:
                picker.resize(width, height)
                picker.wait("Space → Shortcuts")
                picker.wait("Esc cancel")
            picker.send("?", "Keyboard help")
            picker.send(b"\x1b", "Keyboard help", absent=True)
            picker.wait("Include two words")
            picker.send(" /", " INSERT ")  # The guide also offers Query editing.
            picker.send("two words", "Query two words")
            picker.send(b"\x1b", " NORMAL ")
            if columns == 70:
                picker.send(" ", "Space → Shortcuts")
                picker.send(b"\x03")  # Ctrl-C exits from the guide too.
            else:
                picker.send(" q" if columns == 120 else "q")
            assert picker.finish() == b""
            if columns == 80:
                assert not re.search(rb"\x1b\[[0-9;]*(?:38|48);(?:2|5);", picker.output)
            else:
                for rgb in (b"166;227;161", b"137;180;250", b"250;179;135"):
                    assert b"38;2;" + rgb in picker.output, f"Mode accent missing: {rgb!r}"
        finally:
            picker.close()
