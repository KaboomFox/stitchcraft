"""Assembles the release of the subject cut-out's models, the one `stitch` fetches them from (ADR-0019,
decision 5; apps/stitchcraft-cutout/README.md › The models). The workflow .github/workflows/models.yml runs
it. Nothing here publishes: that workflow's publish job does, when the owner asks.

Usage, with Python 3.11 or later:

    python -I release.py pins                 # what models.toml pins, as NAME=VALUE lines for the workflow
    python -I release.py fetch DIR            # U²-Net-p's file into DIR, with curl, as rembg publishes it
    python -I release.py assemble DIR NOTES   # DIR holds the models alone: check them, then add the rest

`assemble` runs in the virtual environment that exported MobileSAM's files (export.py), whose packages the
export record names. It compares each model's size and SHA-256 with its pin in models.toml, and stops when one
differs, after saying both. Only then does it put beside the models the licences and NOTICE.txt (release/),
export.py, requirements.txt, EXPORT.md (release/EXPORT.md, with this build's run, tools and sums) and
SHA256SUMS.txt, and write the release's notes to NOTES.

The release's tag is the one the models' addresses name, so models.toml stays the one place that says it.
"""

import hashlib
import importlib.metadata
import os
import platform
import shutil
import subprocess
import sys
import tomllib
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
TEXTS = HERE / "release"
# What the release holds beside the models, the export record and the sums.
COPIED = [
    TEXTS / "LICENSE-MobileSAM.txt",
    TEXTS / "LICENSE-U-2-Net.txt",
    TEXTS / "LICENSE-rembg.txt",
    TEXTS / "NOTICE.txt",
    HERE / "export.py",
    HERE / "requirements.txt",
]
RECORD = "EXPORT.md"
SUMS = "SHA256SUMS.txt"
# The line of release/EXPORT.md that this build's part of the record takes the place of.
BUILD_LINE = "<!-- build: release.py puts the release, the run that built it, its tools and the files' sums here -->"
# The packages the record names: export.py exports with them and checks its files with them.
TOOLS = ["torch", "torchvision", "timm", "onnx", "onnxruntime", "numpy"]
# How each model was made, by its name in models.toml.
MADE = {
    "u2netp": "rembg's conversion of U²-Net-p, released as rembg publishes it",
    "encoder": "exported from MobileSAM's checkpoint by `export.py`",
    "decoder": "exported from MobileSAM's checkpoint by `export.py`",
}
GITHUB = "https://github.com/"
TAG_START = "cutout-models-"
# The decision the notes link to, from the repository's root.
ADR = "docs/src/design/adr/0019-subject-cut-out-before-tracing.md"


def sha256(path: Path) -> str:
    with path.open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()


def pinned() -> dict:
    return tomllib.loads((HERE / "models.toml").read_text(encoding="utf-8"))


def release_of(models: dict) -> tuple[str, str]:
    """The repository and the tag of the one release that every model's address names."""
    named = set()
    for name, model in models.items():
        start, _, rest = model["address"].partition("/releases/download/")
        tag, _, file = rest.partition("/")
        if not start.startswith(GITHUB) or not tag or file != model["file"]:
            sys.exit(f"models.toml: the address of {name} isn't its file in a GitHub release: {model['address']}")
        named.add((start.removeprefix(GITHUB), tag))
    if len(named) != 1:
        sys.exit(f"models.toml: the models' addresses name {len(named)} releases, and one release holds them all: {sorted(named)}")
    return named.pop()


def title_of(tag: str) -> str:
    """The release's title: `cutout-models-1` is "Cut-out models 1"."""
    number = tag.removeprefix(TAG_START)
    if not tag.startswith(TAG_START) or not number.isdecimal():
        sys.exit(f"models.toml: the release's tag is {tag}, and a tag is {TAG_START} and a number")
    return f"Cut-out models {number}"


def pins() -> None:
    toml = pinned()
    repository, tag = release_of(toml["model"])
    mobilesam = toml["source"]["mobilesam"]
    if not mobilesam["repository"].startswith(GITHUB):
        sys.exit(f"models.toml: MobileSAM's repository isn't on GitHub: {mobilesam['repository']}")
    values = {
        "tag": tag,
        "title": title_of(tag),
        "repository": repository,
        "mobilesam": mobilesam["repository"].removeprefix(GITHUB),
        "commit": mobilesam["commit"],
    }
    for name, value in values.items():
        print(f"{name}={value}")


def fetch(folder: Path) -> None:
    toml = pinned()
    source = toml["source"]["u2netp"]
    folder.mkdir(parents=True, exist_ok=True)
    target = folder / toml["model"]["u2netp"]["file"]
    curl = ["curl", "--fail", "--location", "--silent", "--show-error", "--retry", "3", "--output", str(target), source["address"]]
    if subprocess.run(curl, check=False).returncode != 0:
        sys.exit(f"curl couldn't fetch {source['address']}")
    size, digest = target.stat().st_size, sha256(target)
    if (size, digest) != (source["size"], source["sha256"]):
        sys.exit(f"{source['address']} is {size} bytes with SHA-256 {digest}, and models.toml pins {source['size']} and {source['sha256']}")
    print(f"{target.name}: {size} bytes, SHA-256 {digest}, from {source['address']}")


def files_table(rows: list[tuple[str, int, str, str]]) -> str:
    lines = ["| File | Bytes | SHA-256 | Made |", "|---|---:|---|---|"]
    lines += [f"| `{file}` | {size:,} | `{digest}` | {made} |" for file, size, digest, made in rows]
    return "\n".join(lines)


def say(markdown: str) -> None:
    """Prints `markdown`, and adds it to the job's summary in GitHub Actions."""
    print(markdown)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as f:
            f.write(markdown + "\n\n")


def processor() -> str:
    try:
        for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
            if line.startswith("model name"):
                return line.partition(":")[2].strip()
    except OSError:
        pass
    return platform.processor() or platform.machine()


def stale_texts(toml: dict, tag: str, notice: str, record: str) -> list[str]:
    """What release/'s texts no longer say of what models.toml pins: they name the pins in prose."""
    mobilesam, u2netp = toml["source"]["mobilesam"], toml["source"]["u2netp"]
    stale = [f"NOTICE.txt doesn't name {what}" for what in (tag, mobilesam["commit"][:7], u2netp["address"]) if what not in notice]
    stale += [f"{RECORD} doesn't name {what}" for what in (mobilesam["commit"], mobilesam["sha256"], u2netp["address"]) if what not in record]
    if record.count(BUILD_LINE) != 1:
        stale.append(f"{RECORD} doesn't have the one line this build's part takes the place of")
    if not (HERE.parents[2] / ADR).is_file():
        stale.append(f"the notes link to {ADR}, which isn't there")
    return stale


def assemble(folder: Path, notes: Path) -> None:
    toml = pinned()
    models = toml["model"]
    repository, tag = release_of(models)
    title_of(tag)
    unknown = sorted(set(models) - set(MADE))
    if unknown:
        sys.exit(f"release.py doesn't say how {unknown} of models.toml were made: add them to MADE")
    notice = (TEXTS / "NOTICE.txt").read_text(encoding="utf-8")
    record = (TEXTS / RECORD).read_text(encoding="utf-8")
    stale = stale_texts(toml, tag, notice, record)
    if stale:
        sys.exit("release/ doesn't say what models.toml pins: " + ", ".join(stale))

    expected = sorted(model["file"] for model in models.values())
    present = sorted(path.name for path in folder.iterdir()) if folder.is_dir() else []
    if present != expected:
        sys.exit(f"{folder} holds {present}, and it must hold the models alone: {expected}")

    rows, wrong = [], []
    for name, model in models.items():
        path = folder / model["file"]
        size, digest = path.stat().st_size, sha256(path)
        rows.append((model["file"], size, digest, MADE[name]))
        if (size, digest) != (model["size"], model["sha256"]):
            wrong.append(f"{model['file']} is {size} bytes with SHA-256 {digest}, and models.toml pins {model['size']} bytes and {model['sha256']}")
    table = files_table(rows)
    if wrong:
        say("### The cut-out's models differ from their pins\n\n" + table + "\n\n" + "\n".join(f"- {line}" for line in wrong))
        for line in wrong:
            print(f"::error::{line}")
        sys.exit(f"{len(wrong)} of the {len(rows)} models differ from their pins: nothing is assembled")

    versions = []
    for tool in TOOLS:
        try:
            versions.append((tool, importlib.metadata.version(tool)))
        except importlib.metadata.PackageNotFoundError:
            sys.exit(f"{tool} isn't installed here: assemble in the virtual environment that exported the files")

    env = os.environ
    today = datetime.now(timezone.utc).date().isoformat()
    if "GITHUB_RUN_ID" in env:
        run = f"{env['GITHUB_SERVER_URL']}/{env['GITHUB_REPOSITORY']}/actions/runs/{env['GITHUB_RUN_ID']}"
        built = f"GitHub Actions built it on {today} in the run {run}, attempt {env.get('GITHUB_RUN_ATTEMPT', '1')}, from StitchCraft's commit `{env.get('GITHUB_SHA', 'unknown')}`."
        builder = f"GitHub Actions built these files from their authors' own, in the run {run}, and each matched its pin before it was published."
    else:
        built = f"It was built by hand on {today}, outside GitHub Actions."
        builder = "These files were built by hand from their authors' own, and each matched its pin."
    image = f", image {env['ImageOS']} {env['ImageVersion']}" if "ImageOS" in env and "ImageVersion" in env else ""
    build = "\n".join(
        [
            f"Release `{tag}` of {GITHUB}{repository}. {built} `NOTICE.txt` names the models' authors and licences.",
            "",
            "## The files",
            "",
            table,
            "",
            f"Each of the {len(rows)} had the size and SHA-256 that StitchCraft's `apps/stitchcraft-cutout/models/models.toml` pins before this record was written. `{SUMS}` holds the sums of every file of the release.",
            "",
            "## What built them",
            "",
            "| | |",
            "|---|---|",
            f"| Runner | {platform.platform()}{image} |",
            f"| Processor | {processor()} |",
            f"| Python | {platform.python_implementation()} {platform.python_version()} |",
            *[f"| {tool} | {version} |" for tool, version in versions],
        ]
    )

    for path in COPIED:
        shutil.copyfile(path, folder / path.name)
    (folder / RECORD).write_text(record.replace(BUILD_LINE, build), encoding="utf-8")
    names = sorted(path.name for path in folder.iterdir())
    (folder / SUMS).write_text("".join(f"{sha256(folder / name)}  {name}\n" for name in names), encoding="utf-8")

    notes.write_text(
        f"The {len(rows)} models that StitchCraft's subject cut-out runs (`stitch plan --subject`, `stitch export --subject` and `stitch cutout`, as [ADR-0019]({GITHUB}{repository}/blob/main/{ADR}) decided). On first use `stitch` fetches the {len(rows)} `.onnx` files from this release and checks each one against the SHA-256 the repository pins.\n\n"
        f"{table}\n\n"
        f"{builder} The models are their authors' work, under the Apache License 2.0, and StitchCraft didn't train or change them. `NOTICE.txt` names the authors and the licences, with the licence texts beside it. `{RECORD}` records where each file comes from, how the MobileSAM files were exported, what built them and how all {len(rows)} were checked. `{SUMS}` holds the sums of every file here.\n",
        encoding="utf-8",
    )
    say(f"### The cut-out's models, each as its pin says\n\n{table}\n\nThe folder of the release `{tag}` holds {len(names) + 1} files: " + ", ".join(f"`{name}`" for name in [*names, SUMS]) + ".")


def main() -> None:
    match sys.argv[1:]:
        case ["pins"]:
            pins()
        case ["fetch", folder]:
            fetch(Path(folder).resolve())
        case ["assemble", folder, notes]:
            assemble(Path(folder).resolve(), Path(notes).resolve())
        case _:
            sys.exit(__doc__)


if __name__ == "__main__":
    main()
