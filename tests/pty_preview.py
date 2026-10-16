"""Preview reachability through real keys at the supported minimum size."""
import os
import sys
import tempfile
from pty_support import Picker

binary = os.path.abspath(sys.argv[1])
with tempfile.TemporaryDirectory(prefix="dirpick-preview-") as root:
    with open(os.path.join(root, "source.txt"), "w") as source:
        source.writelines(f"source-{number:03}\n" for number in range(1, 51))
    destination = os.path.join(root, "destination")
    os.mkdir(destination)
    for number in range(1, 51):
        open(os.path.join(destination, f"child-{number:03}"), "w").close()

    picker = Picker(binary, root, 70, 18)
    try:
        picker.send("source.txt", "Results · 1")
        picker.send(b" p", "source-001")
        picker.wait("source-005")
        # Five rows are visible. Each page overlaps one row, so the first
        # page must expose 6–9 instead of jumping over unseen source.
        for first in range(5, 50, 4):
            picker.send(b"\x1b[6~", f"source-{first:03}")
            for number in range(first, min(first + 5, 51)):
                picker.wait(f"source-{number:03}")
        picker.send(b"\x1b[5~", "source-045")
        picker.wait("Query source.txt")
        picker.send(b" d\x15destination", "[d Directories]")
        picker.wait("child-001")
        for first in range(5, 50, 4):
            picker.send(b"\x1b[6~", f"child-{first:03}")
            for number in range(first, min(first + 5, 51)):
                picker.wait(f"child-{number:03}")
        picker.send(b"\x1b[5~", "child-045")
        picker.wait("Query destination")
        picker.send(b"\x03")
        assert picker.finish() == b""
    finally:
        picker.close()
