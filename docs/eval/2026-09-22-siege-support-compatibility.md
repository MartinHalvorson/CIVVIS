# Siege support compatibility

The siege formation previously inferred wall bypass from the presence of any
adjacent Siege Tower. This overestimated capture damage for cavalry and against
Renaissance walls, Tsikhe, and Steel defenses. It also allowed ineffective
support to authorize a melee attack into healthy walls. The support mover could
choose cavalry as its escort, and production counted an obsolete Ram against
the sole support slot while rejecting a replacement Tower as a duplicate.

Combat and AI now share the defense-immunity query. Capture estimates and melee
permission use the combat query with the attacking unit's promotion class.
Equipment selection uses the same building effects, including replacements,
and the Urban Defenses technology effect. The mover follows melee or
anti-cavalry infantry toward a compatible city whose walls still stand.
Production requires eligible fielded or queued infantry; obsolete breach
support no longer occupies the composition slot needed by effective equipment.
Existing or queued effective support still prevents redundant production.

The combat restrictions themselves are unchanged. Shipped Gathering Storm
`DLC/Expansion2/Data/Expansion2_UnitAbilities.xml:366-381` assigns
`ENABLE_WALL_ATTACK_MELEE`, `ENABLE_WALL_ATTACK_ANTI_CAVALRY`,
`BYPASS_WALLS_MELEE`, and `BYPASS_WALLS_ANTI_CAVALRY` to their corresponding
promotion classes. The existing engine immunity effects remain authoritative.

Validation results will be recorded after the integrated test run. These are
correctness fixes to production and combat decisions; no native win-rate gain
is claimed.
