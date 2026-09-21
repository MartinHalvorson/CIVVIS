# Reserved capture-unit pressure: investigation

The final paired replay confirms changed attacks on the failing native boards.
A native capture or victory benefit remains unverified. Native run `civvis-20260921T212727Z`, pinned
`82741ed94`, left Man-at-Arms `3080208` at `(33,16)` with 100HP and three moves
at the start of turns 92–94. Teayo `(34,16)` had no walls and healed most ranged
damage; walls appeared at turn 95. The siege doctrine reserves a capture unit
until the city's health fits one blow.

The experiment permits a Domination capture unit with at least 80HP to attack
an unwalled city before the final blow only when speculative damage exceeds
20HP and the immediate loss, and at least 60HP remains after the strike-reach
reply estimate. Reservation stays in place. Standing walls, embarked units,
spent attacks, wounded units, and other victory lanes retain the hold.

Frozen native input through turn100 is at
`/tmp/civvis-taker-pressure-replay/native-events.jsonl`, SHA256
`48cb9cf8bb677a09841391d729e3ee95cffefd1f68c2fdc5e88f2b135551f2df`.
Both final replay sides complete 283 frames. Parent is
`55dec09d0f542fa5fec46ded2098e08ce3cabc7a`; the baseline binary is from
`63edd088ae8fca064bf356533b07dd55c0fb412e`, verified to have identical production
Rust, data, and Cargo files. Candidate source is
`b0d03ba75079561aaad7d035a132712ecfba2ee5`. Binary hashes are in the replay
folder's `provenance.json`.

Three internal-action frames change: turn 92 frames 0–2 now order Man-at-Arms
`3080208` to attack Teayo. Frame 0 replaces an internal Fortify. These are
repeated proposals on the same recorded turn, not three executed attacks.
The journal predicts 48 city damage for 16 immediate damage in the first two
frames, and 45 for 18 in the third. Four exported frames change: the three
attack frames and turn-93 receipts. Frame 92/2 also adds the existing
`DOOMED_BLOW_VETO` combat policy. The unchanged recorded future produces
`target_unharmed` receipts; these do not show native rejection of the proposed
attacks. No other internal actions change.

The final comparison equals the initial isolated comparison byte-for-byte as
parsed JSON: `/tmp/civvis-3695-final-comparison.json`. Final replay durations
31.05/24.94 seconds are not a controlled performance result.

Three focused tests cover successful pressure with healthy survivors, standing
walls, wounded or spent units, other victory lanes, and a nearby Tank's reply.
The initial test failed on old code when the hold returned without acting;
the final test asserts actual city damage and retained health directly.
`cargo test --profile ci --locked` after integration: 4,127 passed, zero failed,
53 ignored across six suites. All 14 append-point checks pass. Eight four-player
180-turn soak games complete, seeds 369500–369507, on the isolated siege change
before integrating the independently validated air-research rule.

The source native game later lost to Aztec Religious victory at turn 122 with
no enemy original capital held. This patch does not demonstrate that the
capture opportunity would have prevented that loss.
