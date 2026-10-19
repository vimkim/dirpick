"""Shell-style field editing through real terminal key sequences."""
import os
import sys
import tempfile
from pathlib import Path

from pty_support import Picker


binary = os.path.abspath(sys.argv[1])
with tempfile.TemporaryDirectory(prefix="dirpick-editing-") as root:
    Path(root, "needle.rs").write_text("fn main() {}\n")
    picker = Picker(binary, root, 120, 32)
    try:
        def edit(keys, text, label="Query"):
            prefix = f"> {label} "
            picker.send(keys, prefix + text)
            row = next(row for row in picker.screen.text().splitlines() if prefix in row)
            value = row.split(prefix, 1)[1].split("│", 1)[0].rstrip()
            assert value == text.rstrip(), (value, text)

        picker.wait(" INSERT ")
        picker.send("needle extra", "Query needle extra")
        picker.wait("No matching files")
        picker.send(b"\x17", "Query needle extra", absent=True)
        picker.wait("Query needle ")
        picker.wait("Results · 1")
        picker.wait("needle.rs")

        # Line/character movement and deletion use the input library.
        edit(b"\x15alpha omega", "alpha omega")
        edit(b"\x01>", ">alpha omega")
        edit(b"\x05<", ">alpha omega<")
        edit(b"\x02!", ">alpha omega!<")
        edit(b"\x0b", ">alpha omega!")
        edit(b"\x0b", ">alpha omega!")  # End-of-field is a no-op.
        edit(b"\x01\x04", "alpha omega!")  # Ctrl-D deletes forward.
        edit(b"\x05\x08", "alpha omega")  # Ctrl-H / Backspace deletes back.
        edit(b"\x01\x0b", "")

        # Alt letters and Ctrl arrows reach word operations on Linux terminals.
        edit("one two three", "one two three")
        edit(b"\x1bbX", "one two Xthree")
        edit(b"\x01\x1bfY", "one Ytwo Xthree")
        edit(b"\x01\x1bd", "Ytwo Xthree")
        edit(b"\x01\x1b[1;5CZ", "Ytwo ZXthree")
        edit(b"\x05\x1b[1;5DQ", "Ytwo QZXthree")
        edit(b"\x01\x1b[3;5~", "QZXthree")  # Ctrl-Delete.
        edit(b"\x05\x1bd", "QZXthree")
        edit(b"\x01\x1bb\x17", "QZXthree")  # Start-of-field is a no-op.
        edit(b"\x15one two", "one two")
        edit(b"\x1b\x7f", "one ")  # Alt-Backspace.

        # Ctrl-W keeps punctuation within a word, skips trailing whitespace,
        # and leaves the text after the cursor intact.
        edit(b"\x15one path/to-file.rs   ", "one path/to-file.rs   ")
        edit(b"\x17", "one ")
        edit("two tail", "one two tail")
        edit(b"\x1bb\x02\x17", "one  tail")
        edit("new", "one new tail")
        edit(b"\x15", "")
        edit(b"\x17\x17", "")

        # Library cursor indices are codepoints; drawing uses UTF-8 offsets.
        edit("한글 café 😀 tail", "한글 café 😀 tail")
        edit(b"\x1bb\x02\x17", "한글 café  tail")
        edit("새말", "한글 café 새말 tail")
        edit(b"\x15", "")
        edit("cafe\u0301", "cafe\u0301")
        edit(b"\x7f", "caf")  # Delete the entire composed grapheme.
        edit(b"\x17needle", "needle")
        picker.wait("Results · 1")

        # The same editing applies to filters and to the unapplied scope field.
        for label, keys, initial, expected in [
            ("Include", b"\t", "rs extra", "rs "),
            ("Exclude", b"\t", "skip extra", "skip "),
            ("Scope", b"\x0c\x15", "unapplied extra", "unapplied "),
        ]:
            edit(keys + initial.encode(), initial, label)
            edit(b"\x17", expected, label)
            edit(b"\x01\x0b", "", label)
        # Global Ctrl-F and Ctrl-L still focus fields and enter Insert.
        edit(b"\x06", "needle")
        picker.wait("Results · 1")
        picker.send(b"\x1b", " NORMAL ")
        edit(b"\x17\x01\x05\x02\x0b\x1bb\x1bf\x1bd", "needle")
        edit(b"\x0c", "", "Scope")
        picker.wait(" INSERT ")
        edit(b"\x06", "needle")

        picker.send(b"\x1b", " NORMAL ")
        picker.send(" ?", "Keyboard help")
        picker.send(b"\x1b[6~", "Ctrl-W")
        picker.send(b"\x1b", " NORMAL ")
        picker.send(b"\x03")
        assert picker.finish() == b"", "Terminal output leaked to stdout"
    finally:
        picker.close()
