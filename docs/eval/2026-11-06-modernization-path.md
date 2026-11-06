# Wartime modernization prices its research path

`modernization-prices-the-path` is an unpriced opt-in and ships off. It changes
which existing wartime upgrade goal wins its existing research priority slot.
It does not activate a native experiment or alter the live force list.

In ordinary Indonesia/Gitarja Deity Online assigned-Science run
`civvis-20261106T115056Z`, the actual planner selected Iron Working toward
Machinery at62/0. At68/0 it changed its goal to Stirrups and selected
Apprenticeship. The source's immediate-strength ranking then valued three
Heavy Chariot upgrades at66 total base strength against three Archer upgrades
at45. The native remaining prerequisite quotes totalled345 science for
Stirrups and112 for Machinery (Engineering80 minus47 banked, Machinery150
minus71 banked; Apprenticeship150 plus Stirrups195). This is a reason to
investigate delayed modernization, not a claim of wasted research or a saved
city. The game lost its original capital at102 and ended in local defeat116.

The opt-in compares aggregate strength improvement per remaining research
beaker. It reuses `war_remaining_research_cost`, which unions prerequisites,
excludes known nodes, uses native remaining quotes without a second speed or
boost discount, and accounts for active model progress when no native quote
exists. A zero-cost path uses a one-beaker denominator to keep scores finite.
The original ranking is retained exactly when the option is off.

The existing major-war requirement, two-unit threshold, immediate successor
eligibility, research priority ordering and ongoing-project behavior remain.
The flag adds no stockpile requirement: a technology may reveal its own
strategic material, and trade or improvements can also supply it. It does not
price Gold, material acquisition, production, positioning or survival.

The routing fixture uses the observed known technologies, army counts and
four quotes, with model map/positions and other fields. It is not a complete
native-board reconstruction. Complete suite results are recorded in the PR.

The candidate's fixed-input proposal probe uses the original68/0 final-read
prefix,9,049,888bytes. OFF/ON/ON/OFF gives Apprenticeship/Engineering/Engineering/
Apprenticeship, with17 independently verified actual reads and exit0 for each
query. Observed genomes and arguments differ only by this option. Both replies
have13actions; only the research action/order changes. Repeated canonical
payloads match within each arm. The adjacent raw archive retains the driver,
source/binary hashes, stdout/stderr, exact commands, captures and both pairs.

This is a cold fixed-prefix proposal comparison on the new candidate, not the
original changing-read history or persistent agent. It proves no native
application, saved turns, survival or victory benefit. The option remains
unpriced and off.

Reconstruct the actual reads and repeat controls independently with:

```sh
python3 docs/eval/2026-11-06-modernization-path.verify.py \
  docs/eval/2026-11-06-modernization-path.action-pairs.tar.xz
python3 tools/gene_fires.py --max 0
```
