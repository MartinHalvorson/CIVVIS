# Research the missing siege capability

A Conquest plan can name a walled city before the army can take it. The
mobilization path can then raise the production budget, but production cannot
train a siege design its player has never researched. The existing wartime
modernization goal only upgrades at least two standing units of a kind, only
during an active major war. It cannot request the first siege unit before the
war begins. Generic technology values do not guarantee that prerequisite path.

`domination-siege-research` is an independently screenable opt-in. For a
Conquest plan with a legal enemy objective and observed walls, it chooses the
lowest remaining research cost among missing land siege designs. It does
nothing if a land siege unit is already fielded or queued, or a non-obsolete
siege design is already unlocked. A production or resource shortage is not
mistaken for a missing technology. Civilization replacements and obsolete
technologies are respected. In a live observation profile, only the last
reported wall state is used; unknown walls do not trigger this beeline.

The goal joins the existing prerequisite picker after defensive research,
appointed war and air breakthroughs, and wartime modernization. It precedes
optional Prophet, luxury and bargain detours while the missing capability
holds up the campaign. Existing research is not interrupted. Expansion,
Recovery and peaceful lane plans do not request the goal. A Conquest response
to a rival victory can use it as well as an explicit Domination target.

This is a first-capability hypothesis, not a full military research tree.
Once a design is unlocked, force composition, material supply, production,
upgrades, and whether that unit is strong enough for later defenses remain
separate decisions. The gene stays off unless the normal measured deployment
rule selects it; a mechanism test alone is not promotion evidence.

Validation and fixed-size native screen results pending.
