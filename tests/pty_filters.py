"""Exercise visible filter editing and traversal controls through a real PTY."""
import errno
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import tempfile
import termios
import time

from pty_support import Screen


binary = sys.argv[1]
with tempfile.TemporaryDirectory(prefix="dirpick-filters-") as root:
    for name in ["kept.rs", "notes.txt", ".secret.rs", "ignored.rs"]:
        with open(os.path.join(root, name), "w") as source:
            source.write(f"preview of {name}\n")
    with open(os.path.join(root, ".ignore"), "w") as rules:
        rules.write("ignored.rs\n!.secret.rs\n")
    for columns, rows in [(120, 32), (70, 18)]:
        ready_read, ready_write = os.pipe()
        pid, master = pty.fork()
        if pid == 0:
            os.close(ready_write)
            os.read(ready_read, 1)
            os.close(ready_read)
            os.chdir(root)
            os.environ["TERM"] = "xterm-256color"
            os.environ["NO_COLOR"] = "1"
            os.execv(binary, [binary])
        os.close(ready_read)
        before = termios.tcgetattr(master)
        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        os.write(ready_write, b"1")
        os.close(ready_write)
        screen = Screen(rows, columns)

        def wait_for(text, absent=False):
            deadline = time.monotonic() + 8
            while (text in screen.text()) == absent:
                if time.monotonic() > deadline:
                    raise AssertionError(f"Expected {text!r}, absent={absent}, {columns} columns:\n{screen.text()}")
                readable, _, _ = select.select([master], [], [], 0.1)
                if readable:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            raise AssertionError(f"Picker exited before {text!r}:\n{screen.text()}")
                        raise
                    screen.feed(chunk)
                    if b"\x1b[6n" in chunk:
                        os.write(master, b"\x1b[1;1R")

        def send(keys, expected, absent=False):
            os.write(master, keys)
            wait_for(expected, absent)

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
            send(b"\x1b[19~", "Hidden:on")  # F8
            wait_for(".secret.rs")
            wait_for("ignored.rs", absent=True)
            send(b"\x1b[20~", "Ignored:on")  # F9
            wait_for("ignored.rs")
            send(b"\x1b[19~", "Hidden:off")
            wait_for(".secret.rs", absent=True)
            wait_for("ignored.rs")
            send(b"\t\tignored", "ignored.rs", absent=True)
            send(b"\x1b[24~", "Keyboard help")
            wait_for("F8 / F9")
            send(b"\x1b", "Keyboard help", absent=True)
            os.write(master, b"\x03")
            deadline = time.monotonic() + 5
            while True:
                result, status = os.waitpid(pid, os.WNOHANG)
                if result:
                    assert os.waitstatus_to_exitcode(status) == 0
                    break
                if time.monotonic() > deadline:
                    raise AssertionError("Picker failed to exit")
                select.select([master], [], [], 0.05)
            assert termios.tcgetattr(master) == before
        finally:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.close(master)
