# Writing style

These rules hold for StitchCraft's prose: docs, messages, help text and CHANGELOG entries. The docs
have 3 kinds of reader. Embroiderers and contributors read them, and AI agents load them as context.
Plain sentences serve all three, and they cost an agent fewer words.

`cargo xtask prose` checks every Markdown line that a branch adds. It runs [Vale](https://vale.sh)
with two styles from `.vale/styles`:

- **StitchCraft**, the house rules. It checks the projects' names and statements that give software a
  mind.
- **ai-tells**, phrasing and punctuation that machine-written text overuses, from
  [vale-ai-tells](https://github.com/tbhb/vale-ai-tells) (MIT). The rules are kept in the repository as
  their authors wrote them.

Lines that a branch does not touch are not checked. Older pages follow the rules when they are
rewritten. Pages generated from the registries are not checked either: their tables, labels and
deviation notes have a format of their own. Their prose is the registries' help text, which follows these
rules all the same.

## Rules

- **State the fact first.** Start with what the software does: "The reader leaves connectors out." Put
  the reason after it. Give a reason only when someone has confirmed it.
- **Give one idea per sentence.** A sentence that needs a semicolon or a dash is two sentences. Use a
  list for 3 or more items.
- **Say what software does.** It reads, writes, sews, skips and reports. It does not know, want, decide
  or carry.
- **Use one name for one thing.** Take the name from the glossary below, and do not swap in a synonym
  for variety.
- **Give the number.** Write "0.3 mm" or "3 jump records" in place of "very short" or "several".
- **Cut filler.** Delete words such as `simply`, `just`, `note that` and `in order to`.
- **Write plain statements.** Avoid contrast formulas such as "X, not Y", a colon that sets up a reveal,
  slogans and dramatic fragments.
- **Keep dashes out of prose.** Use a comma, a full stop or brackets in place of an em dash. Write a
  range with "to": "M3.1 to M3.5", "0.1 to 25 mm".
- **Name a thing or a task in a heading.** "Lock stitches" and "Convert a file" are good headings. A
  question or a "How it works" heading is not.
- **Describe the current design.** History goes in `CHANGELOG.md` and Git. A design page says how the
  code works today.
- **Keep each fact on one page.** Other pages link to it. `cargo xtask docs --check` reports a sentence
  of 12 or more words that is on 2 pages.
- **Spell in British English**, as the code does: colour, centre, millimetre.

A rule can have an exception. To accept a match, turn the rule off around the lines with a comment,
and give the reason in the same comment:

```markdown
<!-- A field name and its value, as the machine shows them. -->
<!-- vale ai-tells.ColonUsage = NO -->
Format: PES version 1
<!-- vale ai-tells.ColonUsage = YES -->
```

## Glossary

| Use | For | Do not use |
|---|---|---|
| design | what is embroidered: its elements and settings | artwork, pattern (for the whole) |
| element | one object of the design, sewn as a unit | object, shape (for the element) |
| stitch plan, plan | the needle positions and commands, in sewing order | stitch list, stitch file |
| needle point | one place where the needle goes through the fabric | penetration, drop |
| stitch | the thread between two needle points | step |
| jump | a move of the frame without sewing | travel (for a jump) |
| trim | a cut of the thread | cut (in prose about plans) |
| stop | a pause of the machine | halt, pause command |
| colour change | a change to the next thread | thread change |
| colour block | the stitches sewn with one thread | colour section |
| lock stitches | the tie-in at the start of a run and the tie-off at its end | knots, tacks |
| machine file | a PES or DST file | embroidery file, export |
| sew-out | sewing a design on a machine to check it | test run |
| test sheet | a design made to test one thing on a machine | sampler |
| hoop | the frame the fabric is stretched in | frame (for the hoop) |
| Ink/Stitch, StitchCraft, VectorCraft, pyembroidery | the projects, spelled their way | `Inkstitch`, `Stitchcraft` |
