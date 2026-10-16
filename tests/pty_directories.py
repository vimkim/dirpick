"""Directory browsing and JSON stdout separation on a real controlling PTY."""
import errno
import fcntl
import json
import os
import pty
import select
import shutil
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

from pty_support import Screen


binary = sys.argv[1]
with tempfile.TemporaryDirectory(prefix="dirpick-dirs-") as root:
    name = "destination '한글'\t \n"
    destination = os.path.join(root, name)
    os.makedirs(os.path.join(destination, "nested-child"))
    with open(os.path.join(destination, "visible-child.txt"), "w") as source:
        source.write("source\n")
    wrapper = subprocess.run([binary, "--print-nu-wrapper"], capture_output=True, check=True).stdout
    with open(os.path.join(root, "dirpick.nu"), "wb") as module:
        module.write(wrapper)
    with open(os.path.join(root, "check.nu"), "w") as script:
        script.write("use ./dirpick.nu dp\ndp\n$env.PWD | to json --raw\n")
    scenarios = [(False, True, False, 120, 32), (True, True, False, 80, 24), (True, False, False, 70, 18)]
    scenarios += [(True, False, True, columns, rows) for columns, rows in [(120, 32), (80, 24), (70, 18)]]
    for integration, cancel, via_nu, columns, rows in scenarios:
        ready_read, ready_write = os.pipe()
        stdout_read, stdout_write = os.pipe()
        pid, master = pty.fork()
        if pid == 0:
            os.close(stdout_read)
            os.dup2(stdout_write, 1)
            os.close(stdout_write)
            os.close(ready_write)
            os.read(ready_read, 1)
            os.close(ready_read)
            os.chdir(root)
            os.environ["TERM"] = "xterm-256color"
            os.environ["NO_COLOR"] = "1"
            if via_nu:
                os.environ["PATH"] = os.path.dirname(binary) + os.pathsep + os.environ["PATH"]
                nu = shutil.which("nu")
                os.execv(nu, [nu, "--no-config-file", "check.nu"])
            os.execv(binary, [binary] + (["--json"] if integration else []))
        os.close(ready_read)
        os.close(stdout_write)
        before = termios.tcgetattr(master)
        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        os.write(ready_write, b"1")
        os.close(ready_write)
        screen = Screen(rows, columns)
        output = bytearray()

        def wait_for(text, absent=False):
            deadline = time.monotonic() + 8
            while (text in screen.text()) == absent:
                assert time.monotonic() < deadline, f"Expected {text!r}, absent={absent}:\n{screen.text()}"
                if select.select([master], [], [], 0.1)[0]:
                    try:
                        chunk = os.read(master, 65536)
                    except OSError as error:
                        if error.errno == errno.EIO:
                            raise AssertionError(f"Picker exited before {text!r}") from error
                        raise
                    output.extend(chunk)
                    screen.feed(chunk)
                    if b"\x1b[6n" in chunk:
                        os.write(master, b"\x1b[1;1R")

        def send(keys, text, absent=False):
            os.write(master, keys)
            wait_for(text, absent)

        try:
            wait_for("Workbench")
            send(b"\x1bOQ", "Workbench │ Directories")  # F2
            if columns < 100:
                send(b"\x1b[21~", "visible-child.txt")  # F10 preview
            wait_for("nested-child/")
            wait_for("visible-child.txt")
            send(b"destination", "Query destination")
            wait_for("Scanning", absent=True)
            wait_for("nested-child/")
            send(b"\r", "[Custom]")
            wait_for("Query destination", absent=True)
            wait_for("nested-child")
            send(b"\x1b[1;3D", "Query destination")  # Alt-Left
            wait_for("nested-child/")
            if not integration:
                send(b"\x07", "--print-nu-wrapper")
                wait_for("Workbench │ Directories")
            if cancel:
                os.write(master, b"\x03")
            else:
                os.write(master, b"\x07")
            deadline = time.monotonic() + 8
            while True:
                waited, status = os.waitpid(pid, os.WNOHANG)
                if waited:
                    assert os.waitstatus_to_exitcode(status) == 0
                    break
                assert time.monotonic() < deadline, "Picker did not exit"
                if select.select([master], [], [], 0.05)[0]:
                    try:
                        output.extend(os.read(master, 65536))
                    except OSError:
                        pass
            while select.select([master], [], [], 0)[0]:
                try:
                    chunk = os.read(master, 65536)
                    if not chunk:
                        break
                    output.extend(chunk)
                except OSError:
                    break
            assert b"\x1b[?1049l" in output
            assert termios.tcgetattr(master) == before
            stdout = os.read(stdout_read, 65536)
            if cancel:
                assert stdout == b"", f"Cancellation contaminated stdout: {stdout!r}"
            elif via_nu:
                assert json.loads(stdout) == destination, stdout
            else:
                assert json.loads(stdout) == {"action": "cd", "path": destination}, stdout
        finally:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.close(master)
            os.close(stdout_read)
