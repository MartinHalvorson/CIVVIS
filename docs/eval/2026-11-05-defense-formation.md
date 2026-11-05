# Defensive formations keep their assigned city in reach

The operator's native lane runs Indonesia/Gitarja, Deity, Online, Science-targeted games through their natural ends. This work uses completed logs and offline captured-input queries. No native game or lock-waiting controller is started; new native testing requires operator approval.

## Observed weakness

In `civvis-20261105T214426Z`, Majapahit fell at turn 31. At turn 29 the native capital at (18,15) had 132 damage and several hostile Cree Warriors within two tiles. The controller journal assigned all four soldiers to **Defend Majapahit**, with the capital as anchor and (19,14) as objective. The formation instead chose a scout at (16,10), five hexes from the capital. Archer 327683 moved to (17,11), four hexes from the capital, and fired at that scout.

The original frozen executable is SHA-256 `989aa5e7caf2a3e3ce5986deed729b209b9dc4067b019571729c5dd5e055a75e`, sourced at `d5016cae6e4899613ecabc93a2f0216364249bf1`. The archived decision at turn 29/frame 0 and its actual captured input supply this observation. It does not prove that different orders save the city.

An offline OFF/ON/ON/OFF diagnostic of the existing `capital-keeps-its-defender-3` option covered turns 25, 27 and 29: twelve queries, 204 registered reads, all fixed-input bytes checked, same-arm payload repeats and one-bit effective genomes verified. It changed some melee directions at 25/27 but left the turn-29 archer orders unchanged. That option is not promoted as a demonstrated remedy.

## Candidate

Independent opt-in `defenders-keep-the-city-in-reach`, off in both controllers. For an assigned land **Defend** force whose own city is under fire:

- the joint combat planner admits a healthy ranged member's firing stands within two tiles of the city, or no farther than its current stand while arriving;
- the later formation uses the city and its nearby attackers, with combat slots inside the two-tile wall-defender radius;
- the later discretionary movement score cannot restore a denied outward ranged firing position.

Shots from the current stand remain available. The wounded-member recovery search retains its ordinary safe retreat range. Melee blows, Relieve, Siege, Reserve, missing assignments, unthreatened cities and arena play retain their ordinary behavior. No combat rule changes.

## Validation status

Four focused formation/recovery tests passed for the initial implementation. Two successive twelve-query controlled captured-input matrices (204 registered reads each) changed formation targets but left all emitted orders identical, including the turn-29 scout pursuit. Both nonfiring attempts are preserved. They exposed the earlier joint combat planner's firing-position choice; changing later passes alone cannot correct an already-spent move.

The current candidate adds the firing-position restriction before the joint planner moves the shooter, and keeps the later handoffs consistent. All nine focused regressions passed locally, including joint firing-position selection, an available shot at a besieger, current-radius fire while arriving, and consistent later handoffs. The current-candidate captured-input comparison remains pending. No deployment genome, city-survival, native actuation or win-rate claim is made.

The two nonfiring matrices are retained in [negative v1](2026-11-05-defense-formation.negative-v1.tar.xz) and [negative v2](2026-11-05-defense-formation.negative-v2.tar.xz). Verify their raw registered reads, repeated payloads, effective one-bit genomes and pair records with:

```sh
python3 docs/eval/2026-11-05-defense-formation.verify.py docs/eval/2026-11-05-defense-formation.negative-v1.tar.xz
python3 docs/eval/2026-11-05-defense-formation.verify.py docs/eval/2026-11-05-defense-formation.negative-v2.tar.xz
```

Both report 12 queries, 204 verified registered reads and no changed orders in any case. These are negative results, not firing evidence.
