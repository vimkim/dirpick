"""Real child/editor handoff, argv, tty flags, restoration, and clean stdout."""
import json
import os
import shlex
import shutil
import sys
import tempfile
import termios
from pty_support import Picker

binary = sys.argv[1]
with tempfile.TemporaryDirectory(prefix="dirpick-editor-") as temporary:
    root = os.path.join(temporary, "sources")
    os.mkdir(root)
    filename = "-quoted '한글'\t \n.rs"
    selected_path = os.path.join(root, filename)
    with open(selected_path, "w") as source:
        for number in range(1, 301):
            source.write(f"needle {number}\n" if number in (251, 260) else f"context {number}\n")
    editor_directory = os.path.join(temporary, "editor path")
    os.mkdir(editor_directory)
    editor = os.path.join(editor_directory, "nvim")
    generic_editor = os.path.join(editor_directory, "generic")
    record = os.path.join(temporary, "editor.json")
    with open(editor, "w") as program:
        program.write("""#!/usr/bin/env python3
import json, os, sys, termios, tty
flags = termios.tcgetattr(0)
with open(os.environ['EDITOR_RECORD'], 'w') as record:
    json.dump({'argv': sys.argv[1:], 'tty': [os.isatty(fd) for fd in (0, 1, 2)], 'canonical': bool(flags[3] & termios.ICANON), 'echo': bool(flags[3] & termios.ECHO)}, record)
print('isolated editor owns terminal', flush=True)
print('editor stderr stays on tty', file=sys.stderr, flush=True)
input()
if os.environ['EDITOR_TEST_MODE'] == 'raw':
    tty.setraw(0)
sys.exit(7 if os.environ['EDITOR_TEST_MODE'] in ('exit', 'raw') else 0)
""")
    os.chmod(editor, 0o700)
    shutil.copyfile(editor, generic_editor)
    os.chmod(generic_editor, 0o700)
    scenarios = [(120, 32, mode) for mode in ("success", "exit", "interrupt", "missing", "unsupported", "files", "generic")]
    scenarios += [(80, 24, "success"), (70, 18, "missing"), (70, 18, "unsupported"), (70, 18, "interrupt"), (70, 18, "raw")]
    for columns, rows, mode in scenarios:
        paged_context = f"context {260 - (rows - 13) // 2 + max(1, rows - 14)}"
        if os.path.exists(record):
            os.unlink(record)
        selected_editor = editor if mode not in ("missing", "unsupported") else os.path.join(temporary, "missing", "nvim" if mode == "missing" else "code")
        if mode == "generic":
            selected_editor = generic_editor
        environment = {
            "NO_COLOR": "1", "EDITOR_RECORD": record, "EDITOR_TEST_MODE": mode,
            "VISUAL": shlex.quote(selected_editor) + " 'literal $(no shell)'",
            "EDITOR": "/do/not/run/fallback",
        }
        picker = Picker(binary, root, columns, rows,
                        ["--json"] if mode == "success" else [], environment)
        master, output = picker.master, picker.output
        wait_for, send = picker.wait, picker.send
        try:
            wait_for("Results · 1")
            if mode not in ("files", "generic"):
                send(b"\x1b", " NORMAL ")
                send(b" c", "Type a content query")
                send(b"ineedle", "needle 251")
                wait_for("needle 260")
                if columns < 100:
                    send(b"\x1b", " NORMAL ")
                    send(b" p", "needle 251")  # Space p preview
                # Matching rows are grouped beneath the file, then selected by line.
                send(b"\x1b[B", "needle 260")
                send(b"\x1b[6~", paged_context)
                send(b"\x06\t\\.rs$", "Include \\.rs$")
                # Effective settings select the first row; select the second again.
                wait_for("needle 251")
                send(b"\x1b[B", "needle 260")
                send(b"\x1b[6~", paged_context)
            os.write(master, b"\r")
            if mode in ("success", "exit", "interrupt", "files", "generic", "raw"):
                wait_for("isolated editor owns terminal")
                with open(record) as saved:
                    observed = json.load(saved)
                line = 1 if mode in ("files", "generic") else 260
                expected = ["literal $(no shell)"] + ([] if mode == "generic" else [f"+{line}", "--"]) + [selected_path]
                assert observed["argv"] == expected, observed
                assert observed["tty"] == [True, True, True], observed
                assert observed["canonical"] and observed["echo"], observed
                assert b"\x1b[?1049l" in output, "Editor did not leave alternate screen"
                send(b"\x03" if mode == "interrupt" else b"return\n", "Editor failed" if mode in ("exit", "interrupt", "raw") else "Editor closed")
            elif mode == "missing":
                wait_for("Editor failed")
                assert not os.path.exists(record)
            else:
                wait_for("Content locations require")
                assert not os.path.exists(record)
            # Full action errors remain available even when their footer row clips.
            send(b"\x1b", " NORMAL ")
            send(b" ?", "Keyboard help")
            send(b"\x1b[F", "retained")
            if mode == "unsupported":
                wait_for("Vim/Neovim")  # Full error is available beyond the clipped footer.
            os.write(master, b"\x1b")
            wait_for("Keyboard help", absent=True)
            if mode not in ("files", "generic"):
                wait_for("Query needle")
                wait_for("Include \\.rs$")
                wait_for(paged_context)
            assert not termios.tcgetattr(master)[3] & termios.ICANON, "Picker did not reacquire raw mode"
            picker.send(b"\x03")
            assert picker.finish() == b"", "Editor or redraw output leaked to stdout"
        finally:
            picker.close()
