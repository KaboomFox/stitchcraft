"""The cases of release.py's `verdict`: what a build does with the 3 models it has, by their sizes and sums
(release/EXPORT.md, The checks). The encoder's export has one of 2 sums by the machine that runs it, so a
build can't count on the pinned one, and these cases stand in for the runs that chance would have to give.
The workflow .github/workflows/models.yml runs them before it exports anything.

Usage, with Python 3.11 or later: python -I test_release.py
"""

import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path

# With -I, Python doesn't look for modules beside this file. No compiled copy of release.py is left beside it.
sys.dont_write_bytecode = True
SPEC = importlib.util.spec_from_file_location("release", Path(__file__).resolve().parent / "release.py")
release = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(release)

MODELS = release.pinned()["model"]
PINS = {name: (model["size"], model["sha256"]) for name, model in MODELS.items()}
ENCODER = MODELS["encoder"]
# The encoder's other sum, as 2 runners exported it (release/EXPORT.md).
OTHER = "71268b41f17ab47538384f408a7e1d25c7f7451f4e8f5cba31b34a047aeff7ff"
RUN = "43667028020"


def found(**changed: tuple[int, str]) -> dict:
    return {**PINS, **changed}


class Verdict(unittest.TestCase):
    def test_files_that_equal_their_pins_are_assembled(self):
        self.assertEqual(release.verdict(MODELS, found(), None, False), (release.PINNED, []))
        self.assertEqual(release.verdict(MODELS, found(), None, True), (release.PINNED, []))

    def test_a_named_run_isnt_looked_at_when_this_builds_encoder_is_the_pinned_one(self):
        self.assertEqual(release.verdict(MODELS, found(), (RUN, None), True), (release.PINNED, []))

    def test_another_sum_of_the_encoder_is_a_warning_that_says_both_sums(self):
        state, lines = release.verdict(MODELS, found(encoder=(ENCODER["size"], OTHER)), None, False)
        self.assertEqual(state, release.DIFFERS)
        self.assertEqual(len(lines), 1)
        self.assertIn(OTHER, lines[0])
        self.assertIn(ENCODER["sha256"], lines[0])

    def test_another_sum_and_no_run_named_refuses_a_publish_and_says_what_to_type(self):
        state, lines = release.verdict(MODELS, found(encoder=(ENCODER["size"], OTHER)), None, True)
        self.assertEqual(state, release.REFUSED)
        self.assertIn("Nothing is published", lines[0])
        self.assertIn(f"`{release.INPUT}`", lines[0])

    def test_a_named_run_whose_encoder_is_the_pinned_file_takes_its_place(self):
        named = (RUN, (ENCODER["size"], ENCODER["sha256"]))
        for publish in (False, True):
            state, lines = release.verdict(MODELS, found(encoder=(ENCODER["size"], OTHER)), named, publish)
            self.assertEqual(state, release.NAMED)
            self.assertIn(RUN, lines[0])

    def test_a_named_run_whose_encoder_has_another_sum_or_size_is_refused(self):
        for other in ((ENCODER["size"], OTHER), (ENCODER["size"] - 1, ENCODER["sha256"])):
            state, lines = release.verdict(MODELS, found(encoder=(ENCODER["size"], OTHER)), (RUN, other), True)
            self.assertEqual(state, release.REFUSED)
            self.assertIn(other[1], lines[0])
            self.assertIn(str(other[0]), lines[0])

    def test_a_named_run_without_the_file_is_refused_and_named(self):
        state, lines = release.verdict(MODELS, found(encoder=(ENCODER["size"], OTHER)), (RUN, None), False)
        self.assertEqual(state, release.REFUSED)
        self.assertIn(RUN, lines[0])
        self.assertIn("expired", lines[0])

    def test_the_encoders_size_and_the_other_files_stay_strict(self):
        named = (RUN, (ENCODER["size"], ENCODER["sha256"]))
        for name in MODELS:
            size, digest = PINS[name]
            changed = [(size + 1, digest)] if name == "encoder" else [(size + 1, digest), (size, OTHER)]
            for pair in changed:
                state, lines = release.verdict(MODELS, found(**{name: pair}), named, False)
                self.assertEqual(state, release.REFUSED, name)
                self.assertIn(MODELS[name]["file"], lines[0])


class Output(unittest.TestCase):
    def test_an_output_is_a_line_of_the_file_github_names(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "output"
            before = os.environ.get("GITHUB_OUTPUT")
            os.environ["GITHUB_OUTPUT"] = str(path)
            try:
                release.output("encoder", release.DIFFERS)
                release.output("encoder_sha256", OTHER)
            finally:
                if before is None:
                    del os.environ["GITHUB_OUTPUT"]
                else:
                    os.environ["GITHUB_OUTPUT"] = before
            self.assertEqual(path.read_text(encoding="utf-8"), f"encoder=differs\nencoder_sha256={OTHER}\n")


if __name__ == "__main__":
    unittest.main(argv=sys.argv[:1])
