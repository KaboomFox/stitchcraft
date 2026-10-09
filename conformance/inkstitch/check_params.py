"""Check conformance/inkstitch-params.toml against the parameter declarations in an Ink/Stitch checkout.

The data file is the interoperability contract: every parameter an Ink/Stitch SVG can carry, with its
type, unit, default and the stitch types (or conditions) it applies to. Defaults are part of the contract,
so a stale row would make a file sew differently in the two tools. This script compares every row with
Ink/Stitch's own `@param` declarations, the method identifiers with its option lists, the lock
identifiers with its lock definitions, the commands with its command lists and the other attributes with
its attribute list, and prints each difference.

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

# The parameters whose values are stitch types; every other condition is listed by its value alone.
METHODS = {"stroke": "stroke_method", "satin": "satin_method", "fill": "fill_method"}

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
                    "options": [literal(o.args[0])[0] for o in getattr(kw.get("options"), "elts", []) if isinstance(o, ast.Call) and getattr(o.func, "id", "") == "ParamOption"],
                }
    return found, problems


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
    for node in ast.walk(tree):
        if isinstance(node, ast.Assign) and any(getattr(t, "id", "") == f"_{section}_methods" for t in node.targets):
            return [literal(e.args[0])[0] for e in node.value.elts]
    return []


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


def main(checkout):
    root = Path(checkout)
    data = tomllib.loads(DATA.read_text(encoding="utf-8"))
    locks = lock_ids(root)
    theirs, problems = declarations(root, locks)
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
    for problem in problems:
        print(problem)
    print(f"{len(ours)} parameters compared: {len(problems)} differences")
    return 1 if problems else 0


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    sys.exit(main(sys.argv[1]))
