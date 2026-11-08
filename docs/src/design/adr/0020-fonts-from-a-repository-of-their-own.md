# ADR-0020: Embroidery fonts come from a repository of their own, under licences that allow shared adaptations

<!-- The status line, a label and its value, which the ADR index is checked against. -->
<!-- vale ai-tells.ColonUsage = NO -->
**Status:** Proposed · 2026-11-08, spike M14.1 · amends decision 4 of [ADR-0001](0001-license-and-clean-room.md)
<!-- vale ai-tells.ColonUsage = YES -->

## Context

M14 sews text in embroidery fonts, folders of glyphs digitized by hand in Ink/Stitch's format
([fonts](../fonts.md#inkstitchs-fonts)). The fonts are StitchCraft's own, those of Ink/Stitch's library
whose licence allows it, and those a user has installed for Ink/Stitch. Decision 4 of
[ADR-0001](0001-license-and-clean-room.md) says that fonts are never copied, so no font file enters this
repository. Spike M14.1 chooses where StitchCraft's fonts live and which licences they may be under. It
also chooses the reader of the format, which M14.2's lettering builds on.

What was measured or read, on 2026-11-08:

- **Ink/Stitch's fonts** at `c7e3a05`. The 131 fonts whose licence file the corpus takes hold 302 MB of
  files, 247 MB of them glyph files, and 59 MB as a gzipped tar archive. One font's kerning table holds
  196,633 pairs. Ink/Stitch's own packages compress every font file with xz (`bin/compress-fonts` at
  `d59c9ab`).
- **VectorCraft's fonts** are in storytold/craft-fonts. Its `fonts/manifest.txt` gives each font file a row
  of its family, style, file, scripts, licence, licence file, SHA-256 and source, and its `ATTRIBUTION.md`
  credits each font. VectorCraft's build embeds the fonts of the checkout that the build option
  `CRAFT_FONTS_DIR` names, and none without it. Release jobs check craft-fonts out at a pinned commit, and
  release packages include each embedded font's licence file. Tests that need the fonts skip with a message
  without them. The web build embeds 1 font of 4.7 MB, so that its wasm stays under a static host's limit
  of 25 MiB (VectorCraft's `docs/development.md` and `crates/text/build.rs` at `3f8d7a0`).
- **The corpus** of M8.6 pins Ink/Stitch's font repository by a commit and the SHA-256 of the archive GitHub
  makes of it, and `cargo xtask corpus` fetches it into a cache outside the repository
  ([the corpus](../fonts.md#the-corpus)).
- **The licence files** of the 131 fonts the corpus takes name an allowed licence first. 32 of them also
  point at the terms of the font each was made from, or at a permission its author gave
  ([licences in the font repository](../fonts.md#licences-in-the-font-repository)).
- **Ink/Stitch's lettering**, read at `d59c9ab` and run as a black box, gives each key of `font.json` a
  default. It finds the glyph files of each direction and measures each glyph's box and its file's
  baseline ([the reader](../fonts.md#the-reader)).

Not read: craftrules' `standards/fonts.md`, the rules craft-fonts follows. Its page wasn't fetched on
2026-11-08. The decision below rests on what VectorCraft's own files say of craft-fonts, and the owner
checks it against the standard.

## Decision

### 1. The font repository

1. **A repository of their own**, `KaboomFox/stitchcraft-fonts`, owned with this repository by its owner,
   who creates it. Each font is a folder under `fonts/` in Ink/Stitch's format, with its licence file named
   `LICENSE`, as Ink/Stitch reads it. A row of `fonts/manifest.toml` describes each font, as `ASSETS.md`
   describes an asset:
   - its folder and its name
   - its author, its source and the commit it was taken at
   - its licence and licence file
   - the credit its licence requires, and what was changed
   - who checked its licence, and when

   `ATTRIBUTION.md` lists the credits.
2. **What it holds.** StitchCraft's own fonts, and the fonts of inkstitch/embroidery-fonts whose licence it
   takes (decision 2), copied whole from a commit the manifest records. Ported fonts join after a sew-out,
   when the owner decides so ([fonts](../fonts.md#decisions-for-the-owner)).
3. **Pinned** by `fonts.toml` at this repository's root, by its commit and the SHA-256 of GitHub's archive
   of the commit, as `conformance/inkstitch/fonts.toml` pins the corpus. A new xtask command, `fonts`,
   fetches the archive into the cache, checks its sum and unpacks it there, with the corpus's code. Moving
   the pin is a pull request, whose CI reads every font of the new commit.
4. **`stitch` finds a font** by its folder's name or by its `name` (M14.2). It looks in each folder that
   `--fonts` names, then in `STITCHCRAFT_FONTS`, then in `stitchcraft/fonts` in the user's data folder. The
   folders where Ink/Stitch keeps a user's fonts come last. Each release publishes the pinned repository's
   fonts as an archive beside the binaries, for that data folder, and no binary holds them.
5. **The VectorCraft plug-in** doesn't read files. Its build embeds the fonts that a list in its crate
   names, from the checkout that the build option `STITCHCRAFT_FONTS_DIR` names, as VectorCraft's build
   embeds craft-fonts. Without the option it embeds none. M14.4 chooses the list within the plug-in's size.
6. **Without the repository**, everything builds and tests. No font file is committed to this repository,
   and none is needed. Tests draw their fonts in code. A case of a real font reads it from the cache, and
   says it is skipped where the cache doesn't have it. Every CI job that runs `cargo xtask ci` fetches the
   corpus, and will fetch the repository once it is pinned.

### 2. The licences it takes

1. **Taken:** the SIL Open Font License and CC0 or the public domain, and CC BY and CC BY-SA. A font's
   adaptations under each may be shared for any use. Licences that forbid commercial use or adaptations
   are left out, and so is the GPL. A font's own terms are left out too, as are terms that are unclear.
2. **A font's licence** is the one its licence file names first, by the corpus's rule
   (`stitchcraft_svg::font::licence`). The `font_license` of `font.json` is a label. Where it names another
   licence the file counts, the manifest row says so, and the font's author hears of it.
3. **Checked when a font joins.** The repository's CI reads every font with StitchCraft's reader. It fails
   when a font doesn't read or has no licence file, when the licence is not one of those taken, or when
   the manifest row names another. Then a person reads the whole licence file, and the licence of the font
   it was made from. For the OFL that includes the Reserved Font Names, which a modified font may not
   use. The row records who checked it and when, and the owner approves each pull request that adds a
   font.
4. **An ambiguous licence file keeps its font out.** The file is ambiguous when the rule doesn't find a
   licence that is taken in it, or when the font rests on a permission given for Ink/Stitch alone, or on
   the terms of the font it was made from that don't say adaptations may be shared. The manifest lists
   such a font as left out, with why, until its author makes the terms clear. At `c7e3a05` that leaves out
   7 fonts whose original's author allowed its use for Ink/Stitch, and a person reads 10 more before they
   join ([licences in the font repository](../fonts.md#licences-in-the-font-repository)).
5. **A CC BY or CC BY-SA font's credit reaches the user** in 4 places. The manifest row and
   `ATTRIBUTION.md` give it, with the licence's address and the changes. The release's font archive
   includes both files and every licence file. `stitch` prints the credit whenever it sets text in such a
   font, and lists it beside the font (M14.2). The plug-in shows it with the font's name in VectorCraft
   (M14.4). StitchCraft says the licence and the credit, and a user who shares a design made with the font
   follows the licence.

### 3. The reader

1. **In `stitchcraft-svg`**, as `stitchcraft_svg::font::read`. A glyph file is an Ink/Stitch SVG file, and
   each glyph's design is read by the SVG reader. Its box and its file's baseline are measured with the
   SVG reader's path data, transforms and page, which no other crate has. The reader takes a folder's files
   as bytes. The plug-in can then read fonts built into it, and `stitch` the files on disk.
2. **What it reads**, as [the reader](../fonts.md#the-reader) describes it:
   - every key of `font.json` that Ink/Stitch reads, with Ink/Stitch's default, as the
     [font settings](../../user/reference/font-settings.md) page gives them
   - the glyph files of each direction, and each glyph's name, label and design
   - each glyph's box and its file's baseline, as Ink/Stitch's lettering measures them
   - the licence file, and the licence it names

   It also reads what lettering needs of the keys, such as a pair's kerning with an older font's pairs and
   a glyph's advance with its defaults, and the heights the font sews at and its default direction.
3. **Bounded and never crashing.** One meter of the budget counts the whole font, and a font that needs
   more is refused with `SC-E0004` (`REQ-FONT-007`). Property tests read malformed fonts.
4. **Coded diagnostics** for what it can't read. `SC-E1101` refuses a font it can't read. `SC-W1102` names a
   setting read at its default, and `SC-W1103` a compressed file left out. `SC-W1104` names a glyph left out,
   as Ink/Stitch's lettering leaves it out. `SC-I1105` says where the font is read as Ink/Stitch reads it,
   in a way its author may not expect.
5. **The line with M14.2.** M14.1 reads a font into its settings and glyphs, and what each key means.
   M14.2 sets text. It matches characters to glyphs, with ligatures, Arabic forms, the letter case and the
   default glyph, and places them by advances, kerning and the spacings the user sets. It aligns the lines
   and keeps the scale within the font's heights, and auto-satin routes the text. M14.2 also chooses the
   crate of lettering, and how it places the glyphs of the 3 fonts whose box StitchCraft offsets from
   Ink/Stitch's ([the reader](../fonts.md#the-reader)).

## Consequences

- ADR-0001's decision 4 gains the amendment, in a note that links here.
- The repository, its manifest's check, `fonts.toml`, the xtask command that fetches it and the release's
  font archive come once the owner has created the repository, with M14.2 or a step she names. Until then
  the cases of a real font read the corpus.
- M15.2's crate, planned as `stitchcraft-fonts`, takes another name, `stitchcraft-outlines`, so that one name
  means one thing.
- Every CI job that runs `cargo xtask ci` fetches the corpus: 69 MB, kept in the Actions cache under the
  pin's hash.

## Alternatives considered

- **Fonts in storytold/craft-fonts**, beside VectorCraft's. Rejected: craft-fonts is for the font files
  that the apps build in, with a row for each face and its scripts. Embroidery fonts are folders of
  digitized designs, 302 MB of them, read and checked otherwise, and craft-fonts belongs to VectorCraft's
  maintainers. The fonts can move there if StitchCraft moves into VectorCraft
  ([ADR-0011](0011-movable-into-vectorcraft.md)).
- **A fork of inkstitch/embroidery-fonts.** Rejected: it includes the 11 fonts StitchCraft can't take, with
  their GPL and non-commercial licence files and their history.
- **Fonts in this repository.** Rejected by ADR-0001's decision 4 and by VectorCraft's rule that no font file
  is committed. The clean-room check would also find the GPL's text in a licence file.
- **`stitch` fetching Ink/Stitch's repository at the corpus's pin**, and reading the fonts the licence rule
  takes. Rejected: it has no place for StitchCraft's own fonts or for corrections, and every user would
  download the 11 fonts StitchCraft can't take. `stitch` would also need the network.
- **Every font built into `stitch` and the plug-in.** Rejected: 302 MB of glyph files, where VectorCraft's
  web build keeps its wasm under 25 MiB.
- **The OFL alone.** Rejected: 29 of the 131 fonts are in the public domain or under CC BY or CC BY-SA.
  Their adaptations may be shared for any use too.
- **Non-commercial licences as well.** Rejected: StitchCraft can't check how a design is used, and a user who
  sells embroidery couldn't use them. The roadmap rules them out.
- **`font_license` as the font's licence.** Rejected: it disagrees with the licence file for 3 fonts, and the
  file grants the rights.
- **The reader in a crate of its own**, above `stitchcraft-svg`. Rejected for M14.1: the box and the baseline
  need the SVG reader's private path data, transforms and page. Lettering may have a crate of its own
  (M14.2).
- **Each glyph file read once into one design**, its elements sorted by glyph. Rejected: the SVG reader
  would have to keep each element's glyph, and the glyph differential cuts each glyph out anyway. Cutting
  gives each glyph the design the differential sews in both programs.
