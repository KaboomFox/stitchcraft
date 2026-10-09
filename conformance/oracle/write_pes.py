"""Write the PES fixtures that pyembroidery's own writer makes: conformance/fixtures/pes/pyembroidery-*.pes.

pyembroidery 1.4.32 to 1.5.1 (the version pinned in requirements.txt) write PEC stitch data without the
4-byte origin field that Brother's software writes, so StitchCraft's reader is tested against files laid
out both ways. Run it with the pinned interpreter after a change to the pin, and commit the output:

    python3 -I conformance/oracle/write_pes.py conformance/fixtures/pes

Only pyembroidery's public API is used.
"""

import sys

import pyembroidery as pe

# Absolute positions in 0.1 mm, as pyembroidery takes them.
DESIGNS = {
    # The first stitch at the origin: the stitch data starts with a short stitch record.
    "pyembroidery-origin-start": [(pe.STITCH, 0, 0), (pe.STITCH, 100, 0), (pe.STITCH, 100, 100)],
    # The first stitch away from the origin: pyembroidery starts with a long jump to it, shaped like the
    # origin field.
    "pyembroidery-offset-start": [(pe.STITCH, 200, 150), (pe.STITCH, 300, 150), (pe.STITCH, 300, 250)],
}


def main(out):
    for name, stitches in DESIGNS.items():
        pattern = pe.EmbPattern()
        pattern.add_thread({"color": 0xFF0000})
        for command, x, y in stitches:
            pattern.add_stitch_absolute(command, x, y)
        pattern.end()
        pe.write_pes(pattern, f"{out}/{name}.pes", {"pes version": 1})
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
