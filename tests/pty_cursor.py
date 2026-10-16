"""Audit repaint cursor ordering even when synchronized updates are ignored."""
import os
import re
import sys
import tempfile
from pathlib import Path

from pty_support import Picker, Screen


def audit(output, check_from, expected_caret):
    # Replay the preceding stream to obtain visibility instead of assuming that
    # the edit started with a visible cursor. Screen also tracks text advances.
    screen = Screen(32, 120)
    visible, synchronized, caret_ready = True, False, True
    frames = 0
    for token in re.finditer(rb"\x1b\][^\x07]*\x07|\x1b\[[0-?]*[ -/]*[@-~]|[^\x1b]+", output):
        command = token.group()
        check = token.start() >= check_from
        previous = (screen.y, screen.x)
        screen.feed(command)
        position = (screen.y, screen.x)
        if command == b"\x1b[?2026h":
            assert not synchronized, "Nested synchronized frames"
            synchronized = True
            frames += check
        elif command == b"\x1b[?2026l":
            synchronized = False
        elif command == b"\x1b[?25l":
            visible = False
        elif command == b"\x1b[?25h":
            if check:
                assert caret_ready, f"Cursor shown at a paint position: {position}"
            visible = True
        elif command.startswith(b"\x1b["):
            if command[-1:] in (b"H", b"f", b"C", b"D"):
                if check:
                    # Intentionally ignore synchronization here: this proves
                    # safety for terminals that do not implement mode 2026.
                    assert not visible or position == previous, f"Exposed cursor move: {previous} -> {position}"
                caret_ready = True
            elif command[-1:] in (b"J", b"K"):
                caret_ready = False
        elif not command.startswith(b"\x1b]"):
            if check:
                assert not visible, f"Painting with a visible cursor at {previous}"
            caret_ready = False
    assert frames > 0, "No synchronized frame was presented"
    assert not synchronized, "Synchronization remained open between frames"
    assert visible == (expected_caret is not None), "Incorrect final cursor visibility"
    if expected_caret is not None:
        assert (screen.y, screen.x) == expected_caret, ((screen.y, screen.x), expected_caret)


def caret(picker, label="Query", offset=0):
    prefix = f"> {label} "
    for row, text in enumerate(picker.screen.text().splitlines()):
        if prefix in text:
            return row, text.index(prefix) + len(prefix) + offset
    raise AssertionError(f"Focused {label} field not found")


binary = os.path.abspath(sys.argv[1])
with tempfile.TemporaryDirectory(prefix="dirpick-cursor-") as root:
    Path(root, "needle.rs").write_text("fn main() {}\n")
    picker = Picker(binary, root, 120, 32)
    try:
        picker.wait("needle.rs")
        picker.send(b"i", " INSERT ")

        def edit(keys, text, offset=0, label="Query", result=None):
            start = len(picker.output)
            picker.send(keys, text)
            if result:
                picker.wait(result)
            audit(picker.output, start, caret(picker, label, offset))

        for _ in range(3):
            edit(b"x", "Query x", 1, result="No matching files")
            edit(b"\x7f", "Results · 1", result="needle.rs")
        edit(b"needle", "Query needle", 6)
        edit(b"\x1b[D", "Query needle", 5)
        edit(b"\x1b[H", "Query needle", 0)
        edit(b"\x1b[F", "Query needle", 6)
        edit(b"\t", "> Include", 0, "Include")
        edit(b"rs", "Include rs", 2, "Include")
        edit(b"\x06", "> Query", 6)

        for columns, rows in [(80, 24), (70, 18), (120, 32)]:
            start = len(picker.output)
            picker.resize(columns, rows)
            picker.wait("Query needle")
            audit(picker.output, start, caret(picker, offset=6))

        for keys, text in [(b"\x1b", " NORMAL "), (b" ", "Space → Shortcuts"),
                           (b"?", "Keyboard help"), (b"\x1b", " NORMAL ")]:
            start = len(picker.output)
            picker.send(keys, text)
            audit(picker.output, start, None)
        edit(b"i", " INSERT ", 6)
        picker.send(b"\x03")
        assert picker.finish() == b"", "Terminal output leaked to stdout"
    finally:
        picker.close()
