# Give counter-faith construction time to finish

Native run `civvis-20260921T174009Z`, using 06c3e2a0 (including the air-pillage
exporter), lost to Hungary's Religion victory on turn 131. The army had taken
Mongolia's original capital Xanadu on turn 82, then lost it to loyalty on 122.
This investigation addresses religious infrastructure, not that separate
campaign-retention problem.

Hami joined our empire on turn 98 and followed Buddhism through turn 123.
It had no Shrine and no legal Missionary purchase during the preparation
window. The existing defense ordered a Holy Site on turn 115 (visible in the queue
on 116), completed it
on 122, and began the Shrine on 123. Hami lost its majority on 124 while the
Shrine was unfinished; by the final board it followed Catholicism. There
were no own Missionaries in this game. A missed legal purchase is therefore
not the explanation.

The construction alarm waited for a rival faith to hold at least half our
cities. An earlier fact was available: Catholicism already held Sweden and
Hungary, two of four living major civilizations, before its arrival in our
cities. On turn 110 our ten cities included four Buddhist, two Catholic and
four without a majority. Waiting for the home threshold left too little time
for a Holy Site plus Shrine in the surviving alternative-faith city.

The experiment adds an earlier warning only to construction: a rival faith
must have arrived in an own city, hold at least half the living majors, and
have converted another civilization beyond its founder. The existing home
alarm remains authoritative once active. Sanctuary selection still requires
a safe alternative faith, sufficient saved Faith, available infrastructure
or a legal build site, and no immediate home-defense reservation conflict.
The ordinary spread and purchase alarms retain their existing thresholds.

## Evidence limits

Earlier construction orders on a paired replay are not completed buildings
in native Civilization VI. The old future never executed the revised queue;
no successful recruitment, retained city majority or prevented loss can be
claimed from that counterfactual history.

## Paired replay and validation

The isolated comparison covers all 365 decision frames, with baseline
production source 040ccce and candidate ba166a3c1. Twelve internal-action
frames (108/0 through 111/2) and seven exported frames differ. The candidate
issues Holy Site orders for Xanadu on 108/0, 109/0 and 110/0; the first is seven
turns before the baseline's Hami order on 115/0. Xanadu had a legal site at
108 while Hami did not. The candidate also removes the queued Entertainment
Complex orders on 111/0 through 111/2. This is an explicit construction
tradeoff in a city losing loyalty, not evidence that its survival improves.
The existing selector prefers the shortest legal chain; immediate home-defense
reservations remain excluded.

Repeated orders and failed receipts come from the unchanged recorded future,
which kept the old Builder/Entertainment Complex production. They are not
three actually executed new districts. No completed defensive supplier or
native victory is demonstrated.

The new positive regression fails on baseline at the missing reservation.
Thirteen sanctuary tests cover the early warning, its survival through
production review, unchanged ordinary alarm, missing home arrival, missing
foreign conversion, insufficient global majority, and existing constraints.
After integrating 11bce44 (founded-faith recruitment recovery), the full Rust
suite passes 4,097 tests with 53 ignored. Fourteen append checks pass, and
eight four-player 180-turn soak games complete (start seed 368200). Scoped
formatting, changed-line Rust quality and diff whitespace checks pass.
