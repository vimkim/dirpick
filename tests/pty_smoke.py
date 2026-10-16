"""Semantic native-terminal checks, without a GUI or third-party Python packages."""
import base64
import os
import re
import sys
import tempfile
from pty_support import Picker

binary = sys.argv[1]
with tempfile.TemporaryDirectory(prefix="dirpick-pty-") as root:
    filename = "needle '한글'\t \n.rs"
    selected_path = os.path.join(root, filename)
    with open(selected_path, "w") as source:
        source.write("fn main() {} // real source preview\nsecond line\n")
        source.writelines(f"// preview line {line}\n" for line in range(3, 41))
    for columns, rows in [(120, 32), (80, 24), (70, 18)]:
        picker = Picker(binary, root, columns, rows,
                        environment={"NO_COLOR": "1"} if columns == 80 else {})
        master, output = picker.master, picker.output

        def wait_for(text, absent=False):
            picker.wait(text.decode(), absent)

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
            wait_for(f"preview line {rows - 13}".encode())
            wait_for(b"Query needle")
            os.write(master, b"\x1b[5~")  # PageUp
            wait_for(b"real source preview")
            os.write(master, b"\x03")
            assert picker.finish() == b"", "Terminal/clipboard output leaked to stdout"
            if columns == 80:
                assert not re.search(rb"\x1b\[[0-9;]*(?:38|48);(?:2|5);", output), "NO_COLOR emitted RGB/index colors"
        finally:
            picker.close()
