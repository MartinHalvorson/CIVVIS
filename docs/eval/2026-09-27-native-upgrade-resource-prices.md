# Native unit upgrade resource prices

The mirrored board used a rules-derived strategic-resource quote for unit
upgrades even when the native game supplied the upgrade successor, Gold price,
and command eligibility. A modeled 20-Iron bill could therefore prevent a
10-Iron native upgrade from entering the domination funding plan. The reverse
disagreement could reserve too little material for multiple upgrades in one
controller turn.

The installed Gathering Storm UI reads the authoritative two-return accessor
in `DLC/Expansion2/UI/Replacements/UnitPanel_Expansion2.lua:94`:

```lua
local upgradeResource, upgradeResourceCost = pUnit:GetUpgradeResourceCost();
```

The control agent exports the resource Index resolved to its native
`ResourceType` and the complete nonnegative price for a unit with an upgrade
offer. It preserves explicit zero, including the native `-1, 0` no-material
case. Missing, invalid, or unknown native values stay absent. Mirror rebuilds
and incremental synchronization replace these observations each frame, so an
old price cannot survive an export that omits it.

For the exact host-named successor, domination funding and upgrade execution
use the complete native material bill. They do not apply the board's formation
or policy discounts to that bill again. The host's blocked verdict remains
final; an earlier upgrade in the same planning frame also cannot overdraw a
known material stockpile. Simulator boards and older native exports retain the
ordinary modeled quote. A native resource that disagrees with the modeled
successor's requirement falls back rather than charging the wrong stockpile.

This change establishes the readback and accounting path. No actual native
upgrade material bill has yet been observed with the new exporter, and no
forward-game gain or verified Domination win is claimed. The ongoing fresh
King Gran Colombia game uses the preceding production build until this change
is integrated and deployed.

## Validation

- The shipped Lua agent helper passes a Lua 5.1 test for both accessor returns,
  exact and zero bills, missing data, bad values, throwing calls, and unknown
  resource indices. All 71 discovered control-mod Lua suites pass.
- Focused Rust tests pass for native rebuild and synchronization, absent and
  invalid fallback, exact resource deduction, sequential stock limits, and
  domination upgrade funding.
- Full Rust, formatting, and integration checks will be recorded on the PR
  after merging the latest mainline.
