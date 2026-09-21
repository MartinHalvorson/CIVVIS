# Preserve a founded faith's recruitment source

Native run `civvis-20260921T171759Z` used 12a52ccea and lost to Japan's
Religion victory on turn 143. Gran Colombia founded Confucianism on turn 94.
Maracaibo was its only city with a Shrine and Temple. At turn 120 it still
followed Confucianism; at 121 it had no majority, and at 123 it followed
Buddhism. Every own city had lost the founded faith by 125.

A baseline replay of all 388 decision frames reproduces the missed recovery:
Missionary 4849689 moves to native (21,14) on 120/0, then heads toward Bogotá
at (18,16) on 121/0 instead of spreading at Maracaibo. Another Missionary
continues spending charges in the west. Both are gone by turn 124; a unit-loss
event alone does not distinguish expended charges from combat destruction.

The existing `counterfaith_recruitment_targets` priority applies only to
non-founders. It restores a converted, equipped city before ordinary
population targets, but immediately releases that priority when any equipped
city can already supply the faith. The experiment extends this same rule to
our own founded religion during explicit Domination, retaining the religious
victory, usable Shrine/Holy Site, and no-existing-supplier guards. Foreign
faith carried by a founder's unit does not gain this priority.

This is separate from the native run's missing holy-city field: the captured
states omit it despite exporting our founded religion. That may have prevented
Inquisition preparation, but the cause of the missing export is not established
here. Recovery of recruitment depends on city majority and usable infrastructure,
not a guessed holy-city identity.

## Evidence limits

A paired replay can establish changed decisions on recorded boards. It cannot
show that the new spreads survive hostile Apostles, restore the majority in
native Civilization VI, or prevent the later religious loss. The original
recorded future never executed new orders.
