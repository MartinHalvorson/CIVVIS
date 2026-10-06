# Native Anti-Air Gun unit mapping

Firaxis exports `UNIT_ANTIAIR_GUN`. CIVVIS stores the supported unit as
`anti_air_gun`. Production queues already translate that spelling through
`civvis_node_name`, but observed units use a separate lookup,
`resolved_civvis_unit_name`, whose direct-name mapper lacked the alias.

Fresh reconstruction could therefore substitute a Battering Ram through the
support promotion-class fallback. Persistent arrival instead left the unit
unresolved and absent. A visible rival's defense also needs the exact unit row:
the ram has none of the gun's anti-air strength.

## Native evidence

The completed Emperor run `civvis-20261006T165459Z`, on private revision
`e70c1b37557757e30da2ee3d14f57ae41a70e0bc`, exported our Anti-Air Gun
`11599904` in 80 state frames from turn 195 frame 0 through turn 221 frame 3.
The first record identifies `PROMOTION_CLASS_SUPPORT`, with no replacement
base. This establishes exposure to the missing alias. It does not establish
that the wrong import caused the Science loss at turn 221.

The installed game provides the spelling and defense profile in
`Base/Assets/Gameplay/Data/Units.xml:831`:
`UnitType="UNIT_ANTIAIR_GUN"`, `AntiAirCombat="90"`, `Range="1"`, and
`PromotionClass="PROMOTION_CLASS_SUPPORT"`. Its row at line 879 upgrades the
gun to `UNIT_MOBILE_SAM`. CIVVIS already has the corresponding unit specs;
this change repairs the name lookup rather than changing their stats.

## Correction and validation

Add the exact `antiair_gun` to `anti_air_gun` alias to the observed-unit mapper.
Construction and sync share this mapper, including foreign-unit imports and
host-reported upgrade types. Persistent refresh also updates an existing
unit's supported type without changing its board id. The upgrade regression
exposed the previous stale-type behavior independently of the missing alias:
an observed Archer-to-Crossbowman upgrade also retained the Archer row.
Native observations supply the resulting health, experience and treasury;
refresh does not simulate or charge for an upgrade.

The unchanged-id type-change cases are contract tests, not observed native
upgrade behavior: the 20-run native audit found no such transitions.
A separate regression executes a modeled
upgrade, then syncs an unchanged host type and treasury. Without type refresh,
the model keeps the stronger successor while its treasury returns to the host
reading. The correction restores both observations. Current native verification
uses fresh boards, so this persistent-state correction is not evidence of an
improved result in those runs.

Keep the existing rules-table guard: a ruleset
without `anti_air_gun` cannot resolve this exact unit through the alias.
An unresolved refreshed type retains the previous supported row rather than
assigning an unchecked name to the rules table.

Ten regression cases cover queue/import agreement, fresh own-unit defense,
persistent own arrival and refresh, visible rival defense across fresh and
persistent boards, an observed upgrade retaining host identity, a missing
rules-row control, the existing exact Mobile SAM spelling, an unrelated
known upgrade without a simulated payment, an unresolved refresh control,
and restoration of the host's type after a modeled upgrade.

Original-source and corrected-source test logs, source hashes, the native
exposure census, and the minimized native record are retained under
`civvis-tactics-results/2026-10-06/native-anti-air-unit-mapping` on the host.
Full CI and focused validation results belong in the PR. No changed native
orders or win-rate gain are established by these import tests.

## Recorded-history decision check

The exact private source of `civvis-20261006T165459Z` was archived for a separate
offline replay. All 2,346 tracked inputs were compared; only `src/mirror.rs`
changed, with the same alias and supported-type refresh. The private unique
unit roster and all 180 recorded force-on tags were retained.

Across 626 frames, seven frames changed actionable orders and eleven changed
the modeled native actions. At turn 200 frame 0, the first change to the
complete decision or orders, the baseline's complete orders and entire
decision both match the saved native record. The candidate changes a redundant
Anti-Air Gun production order to a Builder while recognizing the existing gun.

The earlier diagnostic `note` changes at turn 195 frame 0. That baseline frame
matches the saved entire decision, but its retrospective verification telemetry
differs, so the registered first-raw-reply fidelity gate is retained as failed.
The later functional-frame comparison does not erase that failed gate or drop
any orders or decision fields. These frozen inputs show decision effects only;
no changed candidate order was executed in Civilization VI, and neither
counterfactual survival nor native win-rate improvement is measured.
