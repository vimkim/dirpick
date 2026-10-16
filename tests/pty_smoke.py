"""Semantic native-terminal checks, without a GUI or third-party Python packages."""
import codecs
import base64
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
import re
import unicodedata


class Screen:
    """Only VT commands used by the native backend; assert visible text, not draws."""
    def __init__(self, rows, columns):
        self.rows, self.columns = rows, columns
        self.cells = [[" "] * columns for _ in range(rows)]
        self.x = self.y = 0
        self.pending = ""
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")

    def feed(self, chunk):
        self.pending += self.decoder.decode(chunk)
        while self.pending:
            if self.pending.startswith("\x1b]"):
                end = self.pending.find("\x07")
                if end < 0:
                    return
                self.pending = self.pending[end + 1:]
                continue
            if self.pending.startswith("\x1b["):
                command = re.match(r"\x1b\[([0-9;?]*)([ -/]*)([@-~])", self.pending)
                if not command:
                    return
                parameters, _, final = command.groups()
                values = [int(value or 0) for value in parameters.lstrip("?").split(";")]
                if final in "Hf":
                    self.y = max(0, (values[0] or 1) - 1)
                    self.x = max(0, (values[1] if len(values) > 1 else 1) - 1)
                elif final == "J" and values[0] in (2, 3):
                    self.cells = [[" "] * self.columns for _ in range(self.rows)]
                elif final == "K":
                    if self.y < self.rows:
                        for x in range(min(self.x, self.columns), self.columns):
                            self.cells[self.y][x] = " "
                elif final == "C":
                    self.x += values[0] or 1
                elif final == "D":
                    self.x = max(0, self.x - (values[0] or 1))
                self.pending = self.pending[command.end():]
                continue
            if self.pending == "\x1b":
                return
            character, self.pending = self.pending[0], self.pending[1:]
            if character == "\r":
                self.x = 0
            elif character == "\n":
                self.y += 1
            elif ord(character) >= 32:
                if self.y < self.rows and self.x < self.columns:
                    self.cells[self.y][self.x] = character
                self.x += 0 if unicodedata.combining(character) else (2 if unicodedata.east_asian_width(character) in "WF" else 1)

    def text(self):
        return "\n".join("".join(row) for row in self.cells)

binary = sys.argv[1]
with tempfile.TemporaryDirectory(prefix="dirpick-pty-") as root:
    filename = "needle '한글'\t \n.rs"
    selected_path = os.path.join(root, filename)
    with open(selected_path, "w") as source:
        source.write("fn main() {} // real source preview\nsecond line\n")
        source.writelines(f"// preview line {line}\n" for line in range(3, 41))
    for columns, rows in [(120, 32), (80, 24), (70, 18)]:
        start_read, start_write = os.pipe()
        stdout_read, stdout_write = os.pipe()
        pid, master = pty.fork()
        if pid == 0:
            os.close(stdout_read)
            os.dup2(stdout_write, 1)
            os.close(stdout_write)
            os.close(start_write)
            os.read(start_read, 1)
            os.close(start_read)
            os.chdir(root)
            os.environ["TERM"] = "xterm-256color"
            if columns == 80:
                os.environ["NO_COLOR"] = "1"
            else:
                os.environ.pop("NO_COLOR", None)
            os.execv(binary, [binary])
        os.close(start_read)
        os.close(stdout_write)
        before = termios.tcgetattr(master)
        fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        os.write(start_write, b"1")
        os.close(start_write)
        output = bytearray()
        screen = Screen(rows, columns)

        def wait_for(text, absent=False):
            deadline = time.monotonic() + 8
            while (text.decode() in screen.text()) == absent:
                if time.monotonic() > deadline:
                    raise AssertionError(f"Missing {text!r} at {columns} columns: {screen.text()}")
                ready, _, _ = select.select([master], [], [], 0.1)
                if ready:
                    try:
                        chunk = os.read(master, 65536)
                        output.extend(chunk)
                        screen.feed(chunk)
                        if b"\x1b[6n" in chunk:
                            os.write(master, b"\x1b[1;1R")
                    except OSError as error:
                        if error.errno == errno.EIO:
                            raise AssertionError(f"Picker exited before {text!r}: {output!r}")
                        raise

        try:
            wait_for(b"needle")
            wait_for(b"real source preview") if columns >= 100 else None
            assert b"\x1b[?1049h" in output
            if columns >= 100:
                assert re.search(rb"38;2;203;166;247", output), "Rust syntax keyword was not colored"
            os.write(master, b"no-such-file")
            wait_for(b"No matching files")
            os.write(master, b"\x15needle")
            wait_for(b"Query needle")
            wait_for("Results · 1".encode())
            os.write(master, b"\x19")  # Ctrl-Y, isolated PTY clipboard only
            wait_for(b"payload sent")
            clipboards = re.findall(rb"\x1b\]52;c;([^\x07]*)\x07", output)
            assert clipboards, "No clipboard payload reached the controlling terminal"
            assert base64.b64decode(clipboards[-1]) == os.fsencode(selected_path)
            wait_for(b"Query needle")
            os.write(master, b"\x1b[24~")  # F12 help
            wait_for(b"Keyboard help")
            os.write(master, b"\x1b")  # Close help
            wait_for(b"Keyboard help", absent=True)
            if columns < 100:
                os.write(master, b"\x1b[21~")  # F10 preview pane
                wait_for(b"real source preview")
            os.write(master, b"\x1b[6~")  # PageDown, selection/query stay intact
            wait_for(b"preview line 11")
            wait_for(b"Query needle")
            os.write(master, b"\x1b[5~")  # PageUp
            wait_for(b"real source preview")
            os.write(master, b"\x03")
            deadline = time.monotonic() + 5
            while True:
                result, status = os.waitpid(pid, os.WNOHANG)
                if result:
                    assert os.waitstatus_to_exitcode(status) == 0
                    break
                if time.monotonic() > deadline:
                    raise AssertionError("Picker failed to exit on Ctrl-C")
                select.select([master], [], [], 0.05)
            while select.select([master], [], [], 0)[0]:
                try:
                    chunk = os.read(master, 65536)
                    if not chunk:
                        break
                    output.extend(chunk)
                except OSError:
                    break
            assert b"\x1b[?1049l" in output, "Alternate screen was not restored"
            assert termios.tcgetattr(master) == before, "Terminal flags changed after exit"
            assert os.read(stdout_read, 65536) == b"", "Terminal/clipboard output leaked to stdout"
            if columns == 80:
                import re
                assert not re.search(rb"\x1b\[[0-9;]*(?:38|48);(?:2|5);", output), "NO_COLOR emitted RGB/index colors"
        finally:
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            os.close(master)
            os.close(stdout_read)
