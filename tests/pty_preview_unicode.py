"""Observe emitted source, independent of the screen model's emoji support."""
import os
import re
import sys
import tempfile
from pty_support import Picker

binary = os.path.abspath(sys.argv[1])
family = "👨‍👩‍👧‍👦"
with tempfile.TemporaryDirectory(prefix="dirpick-unicode-preview-") as root:
    with open(os.path.join(root, "unicode.txt"), "w") as source:
        source.write(f"{family * 10}TAIL\néX\tcontrol\x1b\n")
    picker = Picker(binary, root, 70, 18)
    try:
        picker.wait("unicode.txt")
        picker.send(b" p", "Lines 1–2/2")
        # Ten composed families use 20 cells, leaving ample room for TAIL
        # within the 61 source cells at 70×18. Inspect the terminal bytes so
        # this assertion does not depend on Python's Unicode width database.
        emitted = re.sub(rb"\x1b\[[0-9;?]*[ -/]*[@-~]", b"", picker.output).decode()
        assert family * 10 + "TAIL" in emitted, emitted
        assert "éX\\tcontrol\\u{1b}" in emitted, emitted
        start = len(picker.output)
        picker.send(" ci👩", "1 matching lines")
        picker.wait("Lines 1–2/2")
        highlighted = picker.output[start:]
        assert b"48;2;249;226;175" in highlighted, highlighted
        # Matching a component must style complete glyphs without splitting
        # their emitted UTF-8 sequences or changing their stored match ranges.
        assert family.encode() in highlighted, highlighted
        picker.send(b"\x03")
        assert picker.finish() == b""
    finally:
        picker.close()
