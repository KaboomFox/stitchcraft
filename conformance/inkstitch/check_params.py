"""Check conformance/inkstitch-params.toml against the parameter declarations in an Ink/Stitch checkout.

The data file is the interoperability contract: every parameter an Ink/Stitch SVG can carry, with its
type, unit, default and the stitch types (or conditions) it applies to. Defaults are part of the contract,
so a stale row would make a file sew differently in the two tools. This script compares every row with
Ink/Stitch's own `@param` declarations, the method identifiers with its option lists, the lock
identifiers with its lock definitions, the commands with its command lists, the other attributes with
its attribute list, and the file version and renamed commands with its updater, and prints each
difference.

A declared default is what Ink/Stitch's settings window shows. What it sews for a file that sets no value
is the fallback its code reads the attribute with, `self.get_..._param("name", fallback)`, and the two
can differ. A row records the fallback (`fallback`) where it sews differently from the default, and the
script compares every getter call in Ink/Stitch's `lib` with the row it reads. A fallback that differs in
form but sews as the default is listed in `SAME_FALLBACK`, with the reason.

Ink/Stitch's Python is read as text and parsed with `ast`: nothing from it is imported or run, and only
facts are compared, never copied (ADR-0012). Run it whenever the contract moves to a new Ink/Stitch commit:

    git clone --depth 1 https://github.com/inkstitch/inkstitch target/inkstitch
    python3 -I conformance/inkstitch/check_params.py target/inkstitch

Exit status 0 when everything agrees, 1 when anything differs or cannot be compared.
"""

import ast
import re
import sys
import tomllib
from pathlib import Path

DATA = Path(__file__).resolve().parent.parent / "inkstitch-params.toml"

# Where each section of the data file is declared in Ink/Stitch.
ELEMENTS = {
    "common": "lib/elements/element.py",
    "stroke": "lib/elements/stroke.py",
    "satin": "lib/elements/satin_column/satin_column.py",
    "fill": "lib/elements/fill_stitch.py",
    "clone": "lib/elements/clone.py",
}
LOCKS = "lib/stitch_plan/lock_stitch.py"
COMMANDS = "lib/commands.py"
ATTRIBUTES = "lib/svg/tags.py"
UPDATE = "lib/update.py"
# Ink/Stitch's extensions: tools that read elements' settings to compare them, not to sew them.
TOOLS = "lib/extensions/"

# The parameters whose values are stitch types; every other condition is listed by its value alone.
METHODS = {"stroke": "stroke_method", "satin": "satin_method", "fill": "fill_method"}

# The methods Ink/Stitch reads a parameter with (`get_param`, `get_float_param`, `get_split_mm_param_as_px`
# and the like), defined in the common section's file.
GETTER = re.compile(r"get_(?:\w+_)?param(?:_as_px)?")

# Fallbacks that differ in form from the row's default but sew as it, with the reason, read in Ink/Stitch's
# code. A listed fallback that is the default again is reported, so the list cannot go stale.
SAME_FALLBACK = {
    ("satin", "satin_method"): "'' is none of the ids the satin code tests for, so it sews as satin_column, option 0",
    ("fill", "cross_stitch_method"): "'simple cross' holds none of the words the cross stitch code looks for in an "
    "id (dense, upright, double, smyrna, flip, half), so it sews as simple_cross, option 0",
    ("fill", "meander_pattern"): "the least tile is the first of the sorted tiles the options list, option 0",
    ("stroke", "satin_guide_pattern_position"): "'line_count' is neither id the ripple code tests for "
    "(render_at_rungs, adaptive), so it sews as default",
}

# Conditions the data file leaves out on purpose, with the reason. Everything else must match.
OMITTED = {
    ("fill", "guided_fill_angle", "0"): "the guided fill strategy's first option (copy): a dropdown index, "
    "recorded when guided fill lands",
}


def literal(node):
    """The node's value if it is a literal, else None (with its source text) for the caller to resolve."""
    try:
        return ast.literal_eval(node), None
    except (ValueError, SyntaxError, TypeError):
        return None, ast.unparse(node)


def gettext(node):
    """The string inside `_("…")`, or the literal value."""
    if isinstance(node, ast.Call) and getattr(node.func, "id", None) == "_" and node.args:
        return literal(node.args[0])[0]
    return literal(node)[0]


def lock_ids(root):
    """Ink/Stitch's lock identifiers: (start ids, end ids, {class name: ids})."""
    tree = ast.parse((root / LOCKS).read_text(encoding="utf-8"))
    kinds, lists, defaults = {}, {}, {"start": [], "end": []}
    for node in tree.body:
        if isinstance(node, ast.Assign) and isinstance(node.value, ast.Call) and node.value.args:
            lock_id = literal(node.value.args[0])[0]
            kinds.setdefault(getattr(node.value.func, "id", ""), []).append(lock_id)
            for target in node.targets:
                lists[getattr(target, "id", "")] = lock_id
        if isinstance(node, ast.Assign) and any(getattr(t, "id", "") == "LOCK_DEFAULTS" for t in node.targets):
            defaults = {literal(k)[0]: [lists.get(getattr(e, "id", ""), "?") for e in v.elts] for k, v in zip(node.value.keys, node.value.values)}
    return defaults["start"], defaults["end"], kinds


def resolve(source, locks):
    """The values of a computed condition: the lock ids of the given kinds, the one pattern Ink/Stitch uses."""
    match = re.fullmatch(r"\[\('(lock_start|lock_end)', lock\.id\) for lock in LOCK_DEFAULTS\['(start|end)'\] if isinstance\(lock, \(([\w, ]+)\)\)\]", source)
    if not match:
        return None
    start, end, kinds = locks
    wanted = {name.strip() for name in match.group(3).split(",")}
    ids = start if match.group(2) == "start" else end
    return [i for i in ids if any(i in kinds.get(kind, []) for kind in wanted)]


def declarations(root, locks):
    """{(section, name): fact dict} from every `@param(...)` in Ink/Stitch, plus problems."""
    found, problems = {}, []
    for section, path in ELEMENTS.items():
        tree = ast.parse((root / path).read_text(encoding="utf-8"))
        for node in ast.walk(tree):
            for dec in getattr(node, "decorator_list", []):
                if not (isinstance(dec, ast.Call) and getattr(dec.func, "id", None) == "param" and dec.args):
                    continue
                name = literal(dec.args[0])[0]
                kw = {k.arg: k.value for k in dec.keywords}
                default = literal(kw["default"])[0] if "default" in kw else None
                conditions = []
                if "select_items" in kw:
                    value, source = literal(kw["select_items"])
                    if source is None:
                        conditions = [str(v) for _, v in value]
                    else:
                        conditions = resolve(source, locks)
                        if conditions is None:
                            problems.append(f"{section} {name}: cannot compare the computed condition `{source}`")
                            conditions = []
                found[(section, name)] = {
                    "type": literal(kw["type"])[0] if "type" in kw else None,
                    "unit": gettext(kw["unit"]) if "unit" in kw else None,
                    "default": default,
                    "applies_to": conditions,
                    "options": option_ids(tree, kw.get("options")),
                }
    return found, problems


def option_ids(tree, node):
    """The ids of a combo's `ParamOption`s, from the list itself or from the list a name in the same file holds."""
    if isinstance(node, ast.Name):
        held = (a.value for a in ast.walk(tree) if isinstance(a, ast.Assign) and any(getattr(t, "id", "") == node.id for t in a.targets))
        node = next(held, None)
    return [literal(o.args[0])[0] for o in getattr(node, "elts", []) if isinstance(o, ast.Call) and getattr(o.func, "id", "") == "ParamOption"]


def implicit_fallbacks(root):
    """{getter: the fallback it reads with when a call passes none}, from the getters' own signatures. The last
    definition of a name is the method itself: its typing overloads come before it."""
    tree = ast.parse((root / ELEMENTS["common"]).read_text(encoding="utf-8"))
    found = {}
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef) and GETTER.fullmatch(node.name):
            given = dict(zip(reversed([a.arg for a in node.args.args]), reversed(node.args.defaults)))
            found[node.name] = literal(given["default"]) if "default" in given else (None, None)
    return found


def as_read(getter, value):
    """A literal fallback as its getter reads it: a toggle's as on or off, and a pair of equal numbers as one."""
    if getter == "get_boolean_param" and (value is None or isinstance(value, bool)):
        return bool(value)
    if isinstance(value, (tuple, list)) and value and all(v == value[0] for v in value):
        return value[0]
    return value


def getters(root, declared):
    """{(section, name): [fallback]} for every getter call in Ink/Stitch's `lib`, but its updater's and
    its extensions', that reads a declared parameter by its literal name. A call in an element's file reads that element's parameter where the
    element declares it, and a call elsewhere reads the parameter of every section that declares the name. A
    fallback is (value, None) for a literal, as its getter reads it, and (None, source) for a computed one."""
    implicit = implicit_fallbacks(root)
    files = {path: section for section, path in ELEMENTS.items()}
    found = {}
    for path in sorted((root / "lib").rglob("*.py")):
        relative = path.relative_to(root).as_posix()
        # The updater reads what a file sets to tell whether to change it (`updater`), and the extensions'
        # tools read it to compare elements: neither reads it to sew it.
        if relative == UPDATE or relative.startswith(TOOLS):
            continue
        own = files.get(relative)
        for node in ast.walk(ast.parse(path.read_text(encoding="utf-8"))):
            if not (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.args and GETTER.fullmatch(node.func.attr)):
                continue
            name = literal(node.args[0])[0]
            sections = [section for section, declared_name in declared if declared_name == name]
            keywords = {k.arg: k.value for k in node.keywords}
            given = node.args[1] if len(node.args) > 1 else keywords.get("default")
            value, source = literal(given) if given is not None else implicit.get(node.func.attr, (None, None))
            for section in [own] if own in sections else sections:
                found.setdefault((section, name), []).append((as_read(node.func.attr, value), source))
    return found


def same_fallback(expected, value, source, options):
    """Whether a getter's fallback sews as the data file's `expected`: a computed one where the row declares no
    default, an option's id where the row's default is the option's place, or an equal value."""
    if source is not None:
        return expected == "—"
    if isinstance(value, bool) and expected == "—":
        expected = "false"
    if isinstance(expected, str) and expected.isdigit() and options and isinstance(value, str):
        place = int(expected)
        return place < len(options) and options[place] == value
    return same_default(expected, value)


def same_default(ours, theirs):
    """Whether a data-file default ("—" for none) equals a declared one."""
    if theirs is None or theirs == "":
        return ours == "—"
    if isinstance(theirs, bool):
        return ours == ("true" if theirs else "false")
    try:
        return float(ours) == float(theirs)
    except (TypeError, ValueError):
        return ours == str(theirs)


def methods_of(root, section):
    """The option ids of a section's method list (`_stroke_methods` and the like)."""
    tree = ast.parse((root / ELEMENTS[section]).read_text(encoding="utf-8"))
    return option_ids(tree, ast.Name(id=f"_{section}_methods"))


def commands(root):
    """Ink/Stitch's commands by name, with where each applies: object, layer or document."""
    tree = ast.parse((root / COMMANDS).read_text(encoding="utf-8"))
    names, scopes = [], {}
    for node in tree.body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name):
            target = node.targets[0].id
            if target == "COMMANDS" and isinstance(node.value, ast.Dict):
                names = [literal(k)[0] for k in node.value.keys]
            for scope, listed in (("object", "OBJECT_COMMANDS"), ("layer", "LAYER_COMMANDS"), ("document", "GLOBAL_COMMANDS")):
                if target == listed:
                    for name in literal(node.value)[0] or []:
                        scopes[name] = scope
    return {name: scopes.get(name) for name in names}


def attributes(root):
    """Every `inkstitch:*` attribute name Ink/Stitch reads."""
    tree = ast.parse((root / ATTRIBUTES).read_text(encoding="utf-8"))
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "inkstitch_attribs" for t in node.targets):
            return literal(node.value)[0] or []
    return []


def updater(root):
    """Ink/Stitch's current file version, and the commands its updater renames, each old name with the new."""
    tree = ast.parse((root / UPDATE).read_text(encoding="utf-8"))
    version, renamed = None, []
    for node in ast.walk(tree):
        if isinstance(node, ast.Assign) and any(isinstance(t, ast.Name) and t.id == "INKSTITCH_SVG_VERSION" for t in node.targets):
            version = literal(node.value)[0]
        if isinstance(node, ast.Call) and getattr(node.func, "id", None) == "_rename_command" and len(node.args) == 4:
            old, new = (literal(arg)[0] for arg in node.args[2:])
            renamed.append((str(old).removeprefix("inkstitch_"), new))
    return version, renamed


def main(checkout):
    root = Path(checkout)
    data = tomllib.loads(DATA.read_text(encoding="utf-8"))
    locks = lock_ids(root)
    theirs, problems = declarations(root, locks)
    reads = getters(root, theirs)
    ours = {(p["element"], p["name"]): p for p in data["param"]}
    for key in sorted(set(ours) | set(theirs)):
        if key not in ours:
            problems.append(f"{key[0]} {key[1]}: declared by Ink/Stitch, missing from the data file")
            continue
        if key not in theirs:
            problems.append(f"{key[0]} {key[1]}: in the data file, not declared by Ink/Stitch")
            continue
        row, fact = ours[key], theirs[key]
        if row["type"] != fact["type"]:
            problems.append(f"{key[0]} {key[1]}: type {row['type']!r} here, {fact['type']!r} in Ink/Stitch")
        if row["unit"] != (fact["unit"] or "—"):
            problems.append(f"{key[0]} {key[1]}: unit {row['unit']!r} here, {fact['unit']!r} in Ink/Stitch")
        if not same_default(row["default"], fact["default"]):
            problems.append(f"{key[0]} {key[1]}: default {row['default']!r} here, {fact['default']!r} in Ink/Stitch")
        expected = row.get("fallback", row["default"])
        fallbacks = {repr(f): f for f in reads.get(key, [])}
        if not fallbacks:
            problems.append(f"{key[0]} {key[1]}: no getter reads it by its name, so its fallback is not compared")
        if len(fallbacks) > 1:
            problems.append(f"{key[0]} {key[1]}: read with {len(fallbacks)} different fallbacks in Ink/Stitch: {sorted(fallbacks)}")
        differ = [(v, s) for v, s in fallbacks.values() if not same_fallback(expected, v, s, fact["options"])]
        if key in SAME_FALLBACK and not differ:
            problems.append(f"{key[0]} {key[1]}: listed in SAME_FALLBACK, but read with its default")
        for value, source in [] if key in SAME_FALLBACK else differ:
            shown = source if source is not None else repr(value)
            problems.append(f"{key[0]} {key[1]}: read as {shown} when a file sets none, {expected!r} here")
        if "fallback" in row and same_default(row["default"], row["fallback"]):
            problems.append(f"{key[0]} {key[1]}: `fallback` repeats the default")
        declared = sorted(c for c in fact["applies_to"] if (key[0], key[1], c) not in OMITTED)
        if sorted(row["applies_to"]) != declared:
            problems.append(f"{key[0]} {key[1]}: applies to {sorted(row['applies_to'])} here, {declared} in Ink/Stitch")
    start, end, _ = locks
    if data["lock_ids"] != start or sorted(start) != sorted(end):
        problems.append(f"lock ids: {data['lock_ids']} here, start {start} and end {end} in Ink/Stitch")
    for section, param in METHODS.items():
        here = [m["value"] for m in data["method"] if m["param"] == param]
        if here != methods_of(root, section):
            problems.append(f"{param}: {here} here, {methods_of(root, section)} in Ink/Stitch")
    here = {c["name"]: c["tied_to"] for c in data.get("command", [])}
    if here != commands(root):
        problems.append(f"commands: {here} here, {commands(root)} in Ink/Stitch")
    params = {p["name"] for p in data["param"]}
    others = sorted(a for a in attributes(root) if a not in params)
    if sorted(a["name"] for a in data.get("attribute", [])) != others:
        problems.append(f"attributes: {sorted(a['name'] for a in data.get('attribute', []))} here, {others} in Ink/Stitch")
    version, renamed = updater(root)
    here = [(r["old"], r["new"]) for r in data.get("renamed_command", [])]
    if here != renamed:
        problems.append(f"renamed commands: {here} here, {renamed} in Ink/Stitch")
    newest = max((u["version"] for u in data.get("update", [])), default=None)
    if newest != version:
        problems.append(f"file version: the updates here reach {newest}, Ink/Stitch's is {version}")
    for problem in problems:
        print(problem)
    print(f"{len(ours)} parameters compared: {len(problems)} differences")
    return 1 if problems else 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1]))
