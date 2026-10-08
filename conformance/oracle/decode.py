"""Decode a machine file with pyembroidery and print its stitches as JSON: the REQ-FMT-005 oracle.

`cargo xtask conformance` runs this with the pinned pyembroidery from requirements.txt and compares what
it reads with what StitchCraft's own reader reads (docs/src/design/conformance.md). Only pyembroidery's
public API is used, and only its output is compared; none of its code is copied.

Usage: python3 -I decode.py FILE
"""

import json
import sys

import pyembroidery

COMMANDS = {
    pyembroidery.STITCH: "stitch",
    pyembroidery.JUMP: "jump",
    pyembroidery.TRIM: "trim",
    pyembroidery.STOP: "stop",
    pyembroidery.COLOR_CHANGE: "color_change",
    pyembroidery.END: "end",
}


def main(path):
    pattern = pyembroidery.read(path)
    if pattern is None:
        print(json.dumps({"error": "pyembroidery could not read the file"}))
        return 1
    stitches = []
    for x, y, command in pattern.stitches:
        name = COMMANDS.get(command & pyembroidery.COMMAND_MASK)
        if name is None:
            print(json.dumps({"error": "unknown command %d" % command}))
            return 1
        stitches.append([name, int(round(x)), int(round(y))])
    print(json.dumps({"version": pyembroidery.__version__ if hasattr(pyembroidery, "__version__") else "", "stitches": stitches}))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
