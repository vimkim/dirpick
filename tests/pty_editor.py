"""Real child/editor handoff, argv, tty flags, restoration, and clean stdout."""
import errno
import fcntl
import json
import os
import pty
import select
import shlex
import shutil
import signal
import struct
import sys
import tempfile
import termios
import time
from pty_support import Screen

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
        if os.path.exists(record):
            os.unlink(record)
        stdout_read, stdout_write = os.pipe()
        ready_read, ready_write = os.pipe()
        pid, master = pty.fork()
        if pid == 0:
            os.close(stdout_read)
            os.dup2(stdout_write, 1)
            os.close(stdout_write)
            os.close(ready_write)
            os.read(ready_read, 1)
            os.close(ready_read)
            os.chdir(root)
            os.environ.update(TERM="xterm-256color", NO_COLOR="1", EDITOR_RECORD=record, EDITOR_TEST_MODE=mode)
            selected_editor = editor if mode not in ("missing", "unsupported") else os.path.join(temporary, "missing", "nvim" if mode == "missing" else "code")
            if mode == "generic":
                selected_editor = generic_editor
            os.environ["VISUAL"] = shlex.quote(selected_editor) + " 'literal $(no shell)'"
            os.environ["EDITOR"] = "/do/not/run/fallback"
            os.execv(binary, [binary, "--json"] if mode == "success" else [binary])
        os.close(stdout_write)
        os.close(ready_read)
        before = termios.tcgetattr(master)
        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        os.write(ready_write, b"1")
        os.close(ready_write)
        screen = Screen(rows, columns)
        output = bytearray()

        def wait_for(text, absent=False):
            deadline = time.monotonic() + 8
            while (text in screen.text()) == absent:
                if time.monotonic() > deadline:
                    raise AssertionError(f"{mode}: expected {text!r}, absent={absent}:\n{screen.text()}")
                readable, _, _ = select.select([master], [], [], 0.1)
                if readable:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            raise AssertionError(f"{mode}: exited early:\n{screen.text()}")
                        raise
                    output.extend(chunk)
                    screen.feed(chunk)
                    if b"\x1b[6n" in chunk:
                        raise AssertionError("Global cursor query must not write to stdout")

        def send(keys, text):
            os.write(master, keys)
            wait_for(text)

        try:
            wait_for("Results · 1")
            if mode not in ("files", "generic"):
                send(b"\x1bOR", "Type a content query")
                send(b"needle", "needle 251")
                wait_for("needle 260")
                if columns < 100:
                    send(b"\x1b[21~", "Lines 241")  # F10 preview
                # Matching rows are grouped beneath the file, then selected by line.
                send(b"\x1b[B", "Lines 250")
                send(b"\x1b[6~", "Lines 260")
                send(b"\t\\.rs$", "Include \\.rs$")
                # Effective settings select the first row; select the second again.
                wait_for("Lines 241")
                send(b"\x1b[B", "Lines 250")
                send(b"\x1b[6~", "Lines 260")
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
                wait_for("Content locations require Vim/Neovim")
                assert not os.path.exists(record)
            # Full action errors remain available even when their footer row clips.
            send(b"\x1b[24~", "Keyboard help")
            send(b"\x1b[F", "retained")
            os.write(master, b"\x1b")
            wait_for("Keyboard help", absent=True)
            if mode not in ("files", "generic"):
                wait_for("Query needle")
                wait_for("Include \\.rs$")
                wait_for("Lines 260")
            assert not termios.tcgetattr(master)[3] & termios.ICANON, "Picker did not reacquire raw mode"
            os.write(master, b"\x03")
            deadline = time.monotonic() + 5
            while True:
                result, status = os.waitpid(pid, os.WNOHANG)
                if result:
                    assert os.waitstatus_to_exitcode(status) == 0
                    break
                assert time.monotonic() < deadline, "Picker did not exit"
                select.select([master], [], [], 0.05)
            assert termios.tcgetattr(master) == before, "Final terminal flags were not restored"
            assert os.read(stdout_read, 65536) == b"", "Editor or redraw output leaked to stdout"
        finally:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.close(master)
            os.close(stdout_read)
