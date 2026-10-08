# Security policy

StitchCraft parses untrusted files (SVG, `.vectorcraft`, PES, DST and other machine formats). A crash,
hang, excessive memory use or any panic on crafted input is a security bug.

**Report privately** through GitHub's "Report a vulnerability" (Security tab) on this repository. Please
include the input file (or a description of how to build it) and the StitchCraft version or commit.

We aim to acknowledge reports within a week and to ship a fix, with a regression case in the
conformance suite and a fuzz-corpus entry, in the next release.
