# Defensive formations keep their assigned city in reach

The operator's native lane runs Indonesia/Gitarja, Deity, Online, Science-targeted games through their natural ends. This work uses completed logs and offline captured-input queries. No native game or lock-waiting controller is started; new native testing requires operator approval.

## Observed weakness

In `civvis-20261105T214426Z`, Majapahit fell at turn 31. At turn 29 the native capital at (18,15) had 132 damage and several hostile Cree Warriors within two tiles. The controller journal assigned all four soldiers to **Defend Majapahit**, with the capital as anchor and (19,14) as objective. The formation instead chose a scout at (16,10), five hexes from the capital. Archer 327683 moved to (17,11), four hexes from the capital, and fired at that scout.

The original frozen executable is SHA-256 `989aa5e7caf2a3e3ce5986deed729b209b9dc4067b019571729c5dd5e055a75e`, sourced at `d5016cae6e4899613ecabc93a2f0216364249bf1`. The archived decision at turn 29/frame 0 and its actual captured input supply this observation. It does not prove that different orders save the city.

An offline OFF/ON/ON/OFF diagnostic of the existing `capital-keeps-its-defender-3` option covered turns 25, 27 and 29: twelve queries, 204 registered reads, all fixed-input bytes checked, same-arm payload repeats and one-bit effective genomes verified. It changed some melee directions at 25/27 but left the turn-29 archer orders unchanged. That option is not promoted as a demonstrated remedy.

## Candidate

Independent opt-in `defenders-keep-the-city-in-reach`, off in both controllers. For an assigned land **Defend** force whose own city is under fire:

- form around that city instead of a distant exchange focus;
- retain formation contacts within the existing two-tile wall-threat radius;
- keep healthy combat slots inside the existing two-tile wall-defender radius.

The wounded-member recovery search retains its ordinary safe retreat range. Relieve, Siege, Reserve, missing assignments, unthreatened cities and arena formations retain their ordinary planning. The change is in formation placement; it does not replace the kill search or change combat rules.

## Validation status

Focused tests and current-candidate captured-input comparisons are pending. The earlier defender diagnostic concerns another gene and is not evidence that this candidate fires or improves outcomes. No deployment genome or win-rate claim is made.
