"""Release journeys at the supported sizes; no screenshots or private seams."""
import os
import base64
import json
import re
import shlex
import subprocess
import sys
import tempfile
from pty_support import Picker

binary = os.path.abspath(sys.argv[1])
with tempfile.TemporaryDirectory(prefix="dirpick-workflow-") as root:
    with open(os.path.join(root, "source.rs"), "w") as source:
        source.write("fn main() {}\n")
    picker = Picker(binary, root, 70, 18)
    try:
        picker.wait("source.rs")
        picker.send(b"\x1b", " NORMAL ")
        picker.resize(60, 12)
        picker.wait("Resize terminal")
        picker.send(b" ?", "Keyboard help")
        picker.wait("Ctrl-C")
        picker.send(b"\x1b", "Keyboard help", absent=True)
        picker.send(b"\x03")
        assert picker.finish() == b""
    finally:
        picker.close()

    viewport_root = os.path.join(root, "viewport-fixture")
    os.mkdir(viewport_root)
    with open(os.path.join(viewport_root, "context.rs"), "w") as source:
        for number in range(1, 301):
            source.write("selected needle 260\n" if number == 260 else f"context {number}\n")
    picker = Picker(binary, viewport_root, 70, 18)
    try:
        picker.wait("context.rs")
        picker.send(b"\x1b", " NORMAL ")
        picker.send(b" cineedle", "Results · 1")
        picker.send(b"\x1b", " NORMAL ")
        picker.send(b" p", "selected needle 260")
        picker.send(b"\x03")
        assert picker.finish() == b""
    finally:
        picker.close()

    memory_root = os.path.join(root, "memory-fixture")
    os.mkdir(memory_root)
    with open(os.path.join(memory_root, "minified.txt"), "w") as source:
        source.write("x" * (1024 * 1024))
    picker = Picker(binary, memory_root, 70, 18)
    try:
        picker.wait("minified.txt")
        picker.send(b"\x1b", " NORMAL ")
        picker.send(b" cix", "PARTIAL")
        picker.wait("16 MiB")
        picker.send(b"\x1b", " NORMAL ")
        picker.wait("Space Shortcuts")
        picker.send(b" ?", "Keyboard help")
        picker.send(b"\x1b[F", "filters")
        picker.send(b"\x03")  # Ctrl-C also exits directly from help.
        assert picker.finish() == b""
    finally:
        picker.close()

    with open(os.path.join(root, "many.txt"), "w") as source:
        source.write("limit needle\n" * 5001)
    picker = Picker(binary, root, 70, 18)
    try:
        picker.wait("source.rs")
        picker.send(b"\x1b", " NORMAL ")
        picker.send(b" cilimit", "PARTIAL")  # Contents, row retention limit
        picker.send(b"\x1b", " NORMAL ")
        picker.wait("Space Shortcuts")
        picker.send(b"\x19", "payload sent")
        picker.wait("PARTIAL")
        picker.send(b"\x1b", " NORMAL ")
        picker.wait("Space Shortcuts")
        picker.send(b"\x03")
        assert picker.finish() == b""
    finally:
        picker.close()

    # A long filesystem error must be readable without swallowing help/exit.
    error_root = os.path.join(root, "error-fixture")
    os.mkdir(error_root)
    locked = os.path.join(error_root, "unreadable-" + "long-path-" * 15 + ".txt")
    with open(locked, "w") as source:
        source.write("needle\n")
    os.chmod(locked, 0)
    picker = Picker(binary, error_root, 70, 18)
    try:
        picker.wait("dirpick")
        picker.send(b"\x1b", " NORMAL ")
        picker.send(b" cineedle", "1 errors")
        picker.send(b"\x1b", " NORMAL ")
        picker.wait("Space Shortcuts")
        picker.send(b" ?", "Keyboard help")
        picker.send(b"\x1b[F", "(os error 13)")  # End: full feedback across wrapped lines
        picker.send(b"\x1b", "Keyboard help", absent=True)
        picker.send(b"\x03")
        assert picker.finish() == b""
    finally:
        picker.close()
        os.chmod(locked, 0o600)

with tempfile.TemporaryDirectory(prefix="dirpick-journey-") as temporary:
    project = os.path.join(temporary, "project")
    launch = os.path.join(project, "launch")
    os.makedirs(launch)
    subprocess.run(["git", "init", "-q", project], check=True)
    selected = os.path.join(launch, "needle '한界' é🙂.rs")
    with open(selected, "w") as source:
        for number in range(1, 301):
            source.write(f"needle {number}\n" if number in (251, 260) else f"context {number}\n")
    with open(os.path.join(project, "top-needle.rs"), "w") as source:
        source.write("needle outside\n")
    destination = os.path.join(launch, "needle-destination '界'\t \n")
    os.mkdir(destination)
    os.mkdir(os.path.join(destination, "nested-child"))
    for index in range(35):
        with open(os.path.join(destination, f"child-{index:02}.txt"), "w") as source:
            source.write("directory preview\n")
    editor = os.path.join(temporary, "nvim")
    record = os.path.join(temporary, "editor.json")
    with open(editor, "w") as program:
        program.write("""#!/usr/bin/env python3
import json, os, sys, termios
flags = termios.tcgetattr(0)
with open(os.environ['EDITOR_RECORD'], 'w') as output:
    json.dump({'args': sys.argv[1:], 'tty': [os.isatty(fd) for fd in (0, 1, 2)], 'canonical': bool(flags[3] & termios.ICANON), 'echo': bool(flags[3] & termios.ECHO)}, output)
print('journey editor owns terminal', flush=True)
input()
""")
    os.chmod(editor, 0o700)
    for columns, rows in [(120, 32), (80, 24), (70, 18)]:
        environment = {"VISUAL": shlex.quote(editor), "EDITOR_RECORD": record}
        if columns < 100:
            environment["NO_COLOR"] = "1"
        picker = Picker(binary, launch, columns, rows, ["--json"], environment)
        preview_pane = False
        try:
            picker.wait("[Project]")
            picker.send("needle", "Query needle")
            picker.send("\t\\.rs$", "Include \\.rs$")
            picker.send("\ttop", "Exclude top")
            picker.wait("Results · 1")
            picker.send(b"\x1b[Z", "> Include")  # Shift-Tab reverses focus
            picker.send(b"\x06\x15", "> Query")
            picker.send("한X界", "Query 한X界")
            picker.send(b"\x1b[H\x1b[C\x1b[3~", "Query 한界")  # scalar-safe editing
            picker.wait("Results · 1")
            # The caret follows terminal display width, including CJK characters.
            assert picker.screen.x == 11, (picker.screen.x, picker.screen.text())
            picker.send(b"\x1b[F\x7f", "Query 한")
            picker.send("界", "Query 한界")
            picker.send("\x15é🙂", "Query é🙂")
            assert picker.screen.x == 12, (picker.screen.x, picker.screen.text())
            if columns < 100:
                # End must expose its neighboring text after a joined emoji,
                # even when the field scrolls beyond the composed glyph.
                picker.send("\x15" + "a" * 20 + "👨‍👩‍👧‍👦ABCDEFGHIJKLMNOPQRSTUVWXYZ1234", "XYZ1234")
            picker.send(b"\x15needle", "Query needle")
            picker.send(b"\x19", "payload sent")
            payloads = re.findall(rb"\x1b\]52;c;([^\x07]*)\x07", picker.output)
            assert base64.b64decode(payloads[-1]) == os.fsencode(selected)
            picker.send(b"\x1b", " NORMAL ")
            picker.send(b" s", "[CWD]")  # Space s, shared fields preserved
            picker.wait("Query needle")
            picker.wait("Include \\.rs$")
            picker.send(b" b", "[Project]")  # Back restores preset/search
            picker.send(b" c", "[c Contents]")
            picker.wait("Results · 2")
            picker.send(b" r", "Regex:on")  # Space r
            if columns < 100:
                picker.send(b" p", "needle 251")
                preview_pane = True
            else:
                picker.wait("needle 251")
            picker.send(b"\x1b[B", "needle 260")
            picker.send(b"\x1b[A", "needle 251")
            picker.send(b"\x1b[B", "needle 260")
            paged_context = 260 - (rows - 13) // 2 + max(1, rows - 14)
            picker.send(b"\x1b[6~", f"context {paged_context}")
            scroll_first = min(map(int, re.findall(r"context (\d+)", picker.screen.text())))
            for width, height in [(70, 18), (120, 32), (80, 24), (columns, rows)]:
                picker.resize(width, height)
                picker.wait("Query needle")
                picker.wait("Include \\.rs$")
                picker.wait("Ignored:off")
                if width < 100 and not preview_pane:
                    picker.send(b" p", f"context {scroll_first}")
                    preview_pane = True
                picker.wait(f"context {scroll_first}")
            picker.send(b"\r", "journey editor owns terminal")
            with open(record) as saved:
                observed = json.load(saved)
            assert observed == {"args": ["+260", "--", selected], "tty": [True] * 3, "canonical": True, "echo": True}, observed
            picker.send(b"return\n", "Editor closed")
            picker.wait("Query needle")
            picker.wait(f"context {scroll_first}")
            # Invalid manual scopes retain the current result and field focus.
            picker.send(b"\x0c\x15missing-directory\r", "Cannot apply scope")
            picker.wait("> Scope missing-directory")
            picker.send(b"\x15.\r", "> Query")
            picker.wait("Query needle", absent=True)
            picker.send(b"\x1b", " NORMAL ")
            picker.send(b" b", "Query needle")
            picker.wait(f"context {scroll_first}")
            picker.send(b"\x06", "> Query")
            picker.send(b"\x1b", " NORMAL ")
            picker.send(b" h i", "Hidden:on  i Ignored:on")
            picker.send(b" d", "[d Directories]")
            picker.send(b"\ti\x15", "> Include")
            picker.wait("Children 1") if columns >= 100 or preview_pane else None
            children_page = f"Children {rows - 13}"
            picker.send(b"\x06\x1b[6~", children_page)
            picker.send(b"\r", "[Custom]")
            picker.wait("Query needle", absent=True)
            # Deliberately change shared state inside the browsed scope.
            picker.send(b"\x1b", " NORMAL ")
            picker.send(b" fijunk", "0 files")
            picker.send(b"\x1b", " NORMAL ")
            picker.send(b" h r", "Hidden:off")
            picker.send(b" b", "[d Directories]")
            picker.wait("Query needle")
            picker.wait("Hidden:on  i Ignored:on")
            picker.wait(children_page)
            picker.send(b"\x19", "payload sent")
            payloads = re.findall(rb"\x1b\]52;c;([^\x07]*)\x07", picker.output)
            assert base64.b64decode(payloads[-1]) == os.fsencode(destination)
            picker.send(b"\x07")
            assert json.loads(picker.finish()) == {"action": "cd", "path": destination}
            if columns < 100:
                assert not re.search(rb"\x1b\[[0-9;]*(?:38|48);(?:2|5);", picker.output), "NO_COLOR emitted explicit colors"
            else:
                assert b"38;2;180;190;254" in picker.output, "Mocha accent missing"
        finally:
            picker.close()
        # Cancellation after superseding several searches never emits a cd record.
        picker = Picker(binary, launch, columns, rows, ["--json"])
        try:
            picker.wait("dirpick")
            picker.send(b"needle")
            picker.send(b"\x1b", " NORMAL ")
            picker.send(b" ci\x15context")
            picker.send(b"\x1b", " NORMAL ")
            picker.send(b" d")
            picker.send(b"q" if columns == 80 else b"\x03")
            assert picker.finish() == b""
        finally:
            picker.close()
