# ADR-0013: The reference machine is a Brother PE800, with a profile for each of its hoops

<!-- The status line, a label and its value, which the ADR index is checked against. -->
<!-- vale ai-tells.ColonUsage = NO -->
**Status:** Accepted · 2026-10-10
<!-- vale ai-tells.ColonUsage = YES -->

## Context

[ADR-0007](0007-pes-first-brother-profile.md) took the first test machine to be a Brother with a
200 × 200 mm (8 × 8 in) hoop, and gave it the profile `brother-200x200`, with a 150 × 150 mm comfort
zone. The machine's owner has since said what it is: a Brother PE800 (2026-10-10). Brother's page for the
PE800 gives a 5 × 7 in maximum embroidery area, and hoop sellers give the PE800's 5 × 7 in hoop (SA444) a
130 × 180 mm field, the size PES v1's large-hoop indicator names.

The owner has 3 hoops: the 5 × 7 in that comes with the machine, a 4 × 4 in (100 × 100 mm) and a small
hoop sold as 1 × 2.5 in. That is the size Brother's US site gives its small SA442 hoop. Brother's Canadian
site gives the same hoop as 3/4 × 2 3/8 in, and Brother's small frames are sold in Europe as 2 × 6 cm.

The old profile allowed designs the machine cannot sew. Test sheets TS-02 and TS-02B were 140 mm wide,
and the TS-10 frames 150 and 190 mm. A profile is one machine with one hoop
([data model](../data-model.md#machine-profiles)), so a machine with 3 hoops needs 3.

## Decision

1. Built-in profiles `brother-pe800-5x7` (130 × 180 mm), `brother-pe800-4x4` (100 × 100 mm) and
   `brother-pe800-small` (20 × 60 mm) replace `brother-200x200`. Everything but the hoop stays as it was.
   Those values were common Brother home-machine values rather than facts about an 8 × 8 in machine, and
   the checkpoints still confirm them.
2. `brother-pe800-5x7` is the reference profile, `profiles::REFERENCE`. The command line plans for it
   unless `--profile` names another, and code that means the machine the checkpoints run on imports it
   rather than naming a hoop.
3. No profile has a comfort zone. The 150 × 150 mm zone belonged to the 8 × 8 in hoop, and nothing is
   known yet about where the PE800's hoops hold the fabric less well. A sew-out report that
   shows it gives that hoop's profile one.
4. The small hoop's field is 20 × 60 mm, the metric figure that both inch sizes round. A design that fits
   it fits the hoop. It stands upright, 20 mm across and 60 mm from back to front as the 5 × 7 in's
   field does, until test sheet TS-10C shows which way the machine holds it.
5. Test sheets fit the reference hoop. TS-02 and TS-02B narrow to 120 mm, with their longest jump 30 mm
   long where it was 40. The TS-10 sheets become frames filling each hoop's field, 100 × 100 mm (A),
   130 × 180 mm (B) and 20 × 60 mm (C). Each test sheet names the profile of the hoop it is sewn in, and
   `stitch testsheet` checks the sheet against it unless `--profile` names another.
6. This replaces decisions 2 and 3 of ADR-0007. Its first decision, PES v1 first and DST second, stands.

## Consequences

- MC-1's TS-02 and TS-10 files change, and so does MC-2's TS-02B. The kits are rebuilt with new golden
  files. The other sheets' files are the same, byte for byte.
- PES v1's hoop indicator names the PE800's 2 large fields, 100 × 100 and 130 × 180 mm. The question
  ADR-0007 left to MC-1, whether a machine takes larger designs from PES v1, does not arise for the
  reference machine. TS-10 still checks that each hoop takes a design as large as its profile allows.
- By default, designs must fit 130 × 180 mm. A larger one gets `SC-E0701`, which offers to turn it 90°
  when its sides fit the other way round.
- `SC-W0702` stays in the code and its tests, though no built-in profile raises it.

## Alternatives considered

- **Keep `brother-200x200` alongside**, for 8 × 8 in machines: a profile for a machine nobody sews on is
  a guess. Rejected until someone does.
- **One PE800 profile, for the 5 × 7 in hoop only:** the owner sews in all 3 hoops, and a design meant for
  the 4 × 4 in should be checked against it. Rejected.
- **The small hoop at its nominal 25.4 × 63.5 mm:** designs up to that size may not fit its field.
  Rejected.
- **A comfort zone a margin inside each hoop**, such as 120 × 170 mm: no evidence for any number yet.
  Rejected until a sew-out report gives one.
