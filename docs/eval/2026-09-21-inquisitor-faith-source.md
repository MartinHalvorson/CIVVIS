# Preserve the last religious purchase source

Native run `civvis-20260921T221854Z-cont1` lost by Religion to Cree at turn 176. Cuenca was its only completed Holy Site and source of replacement religious units. The freshly purchased Inquisitor left Cuenca at turn 172 for cities with more foreign pressure. Cuenca became neutral at 174 and Hindu at 175, closing the own-faith supply despite a rising Faith balance.

During Domination, one charged own-faith Inquisitor now prioritizes the last threatened religious purchase city. A source needs an intact Holy Site and an intact Shrine or Temple family building. The priority engages for a converted/neutral source or a foreign charged spreader within four hexes. It releases when there is another faithful source or the nearby spreading threat disappears. Other Inquisitors retain ordinary restoration duties, and worthwhile removal of heresy in the current city still happens first.

The selected guard prefers healthy units, then proximity and stable unit ID. This does not assume passive protection from standing in a city: the existing Remove Heresy action spends charges when foreign pressure justifies it. Guarding the source can delay restoring other cities, and does not by itself defeat hostile religious combat units.

## Validation

Final integrated production source `f30eead353a151092f0916e8f40a92e3872646b5`, exact baseline `79aff8db9fba345fef717ad6e9599d0bec7a8af8`. This includes the separately merged local religious-spreader coverage fix (#3701).

- Four focused tests pass, including three new guard/restoration tests. The baseline fails the guard test by moving away from the purchase city.
- Full Rust suite: 4,146 passed, zero failed, 53 ignored across six suites.
- Fourteen append-integrity tests and changed-line Rust quality pass.
- Eight four-player smoke games with a 180-turn limit completed on the final integrated source, seeds 370200–370207. Earlier source runs also completed these seeds and default seeds 0–7.
- Integrated source CI performance: +0.33% per completed turn, NOISE; IQR 0.35 percentage points, resolution ±0.23%, within the 8% gate.

## Paired native replay

Both final integrated binaries process the same 166 frames through turn 175. Their complete paired differences are identical to the pre-integration replay. Twelve internal-action frames and fifteen exported frames change. The first change at 163 redirects an existing Inquisitor toward Cuenca. At 172 the new Inquisitor's departure from Cuenca is removed entirely. Later frames, which still contain the recorded departure, redirect it back toward the source.

The replay also removes two Remove Heresy proposals elsewhere and changes movement by another unit at 163/frame 1. Receipt differences such as moved_away, did_not_move and unit_gone compare counterfactual orders with the unchanged recorded future; they do not establish host rejection of new orders. No recovered source, prevented loss, or Domination victory is established by replay.

Artifacts: `/tmp/civvis-inquisitor-source-replay/`, comparison `/tmp/civvis-3702-integrated-comparison.json`.

- Input SHA256: `3586d8d174d325b9c080093b12ecda1c43d6e6d0b0b123ef98cdc567c4683006`.
- Baseline binary SHA256: `0dae18da761e81f5053cc03828fd0e472cf6756c45f9f6d1ee649985124d91cc`.
- Candidate binary SHA256: `7aa4781954ce88381f42b17463447a88b6bbbb5c7fe4253aae4f908f9281f951`.
