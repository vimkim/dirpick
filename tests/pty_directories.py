"""Directory browsing and JSON stdout separation on a real controlling PTY."""
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pty_support import Picker

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
        environment = {"NO_COLOR": "1"}
        command, arguments = binary, ["--json"] if integration else []
        if via_nu:
            environment["PATH"] = os.path.dirname(binary) + os.pathsep + os.environ["PATH"]
            command, arguments = shutil.which("nu"), ["--no-config-file", "check.nu"]
        picker = Picker(command, root, columns, rows, arguments, environment)
        wait_for, send = picker.wait, picker.send
        try:
            wait_for("dirpick")
            send(b"\x1b", " NORMAL ")
            send(b" d", "[d Directories]")  # Space d
            if columns < 100:
                send(b" p", "visible-child.txt")  # Space p preview
            wait_for("nested-child/")
            wait_for("visible-child.txt")
            send(b"idestination", "Query destination")
            send(b"\x1b", " NORMAL ")
            wait_for("Scanning", absent=True)
            wait_for("nested-child/")
            send(b"\r", "[Custom]")
            wait_for("Query destination", absent=True)
            if columns < 100:
                send(b" p", "nested-child")  # Results identify the child even with a long scope title.
            wait_for("nested-child")
            send(b"\x1b[1;3D", "Query destination")  # Alt-Left
            if columns < 100:
                send(b" p", "nested-child/")
            wait_for("nested-child/")
            if not integration:
                send(b"\x07", "--print-nu-wrapper")
                wait_for("[d Directories]")
            if cancel:
                picker.send(b"\x03")
            else:
                picker.send(b"\x07")
            stdout = picker.finish()
            if cancel:
                assert stdout == b"", f"Cancellation contaminated stdout: {stdout!r}"
            elif via_nu:
                assert json.loads(stdout) == destination, stdout
            else:
                assert json.loads(stdout) == {"action": "cd", "path": destination}, stdout
        finally:
            picker.close()
