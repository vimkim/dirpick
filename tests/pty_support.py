"""Shared VT screen and process lifecycle for semantic native-terminal checks."""
import codecs
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
        self.normal_screen = None

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
                elif final == "h" and parameters == "?1049":
                    self.normal_screen = (self.cells, self.x, self.y)
                    self.cells = [[" "] * self.columns for _ in range(self.rows)]
                    self.x = self.y = 0
                elif final == "l" and parameters == "?1049":
                    # Resizing starts a fresh model while the process is still
                    # in its alternate screen. Its original normal screen is blank.
                    if self.normal_screen is None:
                        self.cells = [[" "] * self.columns for _ in range(self.rows)]
                        self.x = self.y = 0
                    else:
                        self.cells, self.x, self.y = self.normal_screen
                    self.normal_screen = None
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
                width = 0 if unicodedata.combining(character) else (2 if unicodedata.east_asian_width(character) in "WF" else 1)
                if self.y < self.rows and self.x < self.columns:
                    if width == 0 and self.x:
                        previous = self.x - 1
                        while previous and self.cells[self.y][previous] == "":
                            previous -= 1
                        self.cells[self.y][previous] += character
                    else:
                        self.cells[self.y][self.x] = character
                        if width == 2 and self.x + 1 < self.columns:
                            self.cells[self.y][self.x + 1] = ""
                self.x += width

    def text(self):
        return "\n".join("".join(row) for row in self.cells)


class Picker:
    """Real controlling PTY, separate stdout, semantic waits, and resize input."""
    def __init__(self, binary, root, columns, rows, arguments=(), environment=None):
        import fcntl
        import os
        import pty
        import struct
        import termios
        self.stdout_read, stdout_write = os.pipe()
        ready_read, ready_write = os.pipe()
        self.pid, self.master = pty.fork()
        if self.pid == 0:
            os.close(self.stdout_read)
            os.dup2(stdout_write, 1)
            os.close(stdout_write)
            os.close(ready_write)
            os.read(ready_read, 1)
            os.close(ready_read)
            os.chdir(root)
            os.environ["TERM"] = "xterm-256color"
            os.environ.pop("NO_COLOR", None)
            os.environ.update(environment or {})
            os.execv(binary, [binary, *arguments])
        os.close(stdout_write)
        os.close(ready_read)
        self.before = termios.tcgetattr(self.master)
        self.output = bytearray()
        self.screen = Screen(rows, columns)
        self.exited = False
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        os.write(ready_write, b"1")
        os.close(ready_write)

    def read(self, timeout=0.1):
        import errno
        import os
        import select
        if select.select([self.master], [], [], timeout)[0]:
            try:
                chunk = os.read(self.master, 65536)
            except OSError as error:
                if error.errno == errno.EIO:
                    return False
                raise
            self.output.extend(chunk)
            self.screen.feed(chunk)
            assert b"\x1b[6n" not in chunk, "Cursor queries must not leak to stdout"
            return bool(chunk)
        return True

    def wait(self, text, absent=False):
        import time
        deadline = time.monotonic() + 8
        visible_since = None
        while True:
            # A backend flush may be split across reads; consume the current
            # draw before treating an old row as still-visible guidance.
            if (text in self.screen.text()) != absent:
                visible_since = visible_since or time.monotonic()
                if time.monotonic() - visible_since >= 0.1:
                    return
            else:
                visible_since = None
            assert time.monotonic() < deadline, f"Expected {text!r}, absent={absent}:\n{self.screen.text()}"
            assert self.read(), f"Picker exited before {text!r}:\n{self.screen.text()}"

    def send(self, keys, text=None, absent=False):
        import os
        os.write(self.master, keys.encode() if isinstance(keys, str) else keys)
        if text is not None:
            self.wait(text, absent)

    def resize(self, columns, rows):
        import fcntl
        import os
        import signal
        import struct
        import termios
        self.screen = Screen(rows, columns)
        fcntl.ioctl(self.master, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        os.kill(self.pid, signal.SIGWINCH)

    def finish(self):
        import os
        import termios
        import time
        deadline = time.monotonic() + 8
        while True:
            waited, status = os.waitpid(self.pid, os.WNOHANG)
            if waited:
                self.exited = True
                assert os.waitstatus_to_exitcode(status) == 0, self.output
                break
            assert time.monotonic() < deadline, "Picker did not exit"
            self.read(0.05)
        while self.read(0):
            import select
            if not select.select([self.master], [], [], 0)[0]:
                break
        assert b"\x1b[?1049l" in self.output, "Alternate screen was not restored"
        assert termios.tcgetattr(self.master) == self.before, "Terminal flags were not restored"
        return os.read(self.stdout_read, 65536)

    def close(self):
        import os
        import signal
        if not self.exited:
            try:
                os.kill(self.pid, signal.SIGKILL)
                os.waitpid(self.pid, 0)
            except ProcessLookupError:
                pass
        os.close(self.master)
        os.close(self.stdout_read)
