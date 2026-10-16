"""Small VT screen for semantic native-terminal filter checks."""
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

    def feed(self, chunk):
        self.pending += self.decoder.decode(chunk)
        while self.pending:
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
