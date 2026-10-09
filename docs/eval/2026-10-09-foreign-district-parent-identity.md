# Observed parent cities for foreign districts

Before this change, a visible rival district used the nearest known city as
its mirrored parent. Two Encampments belonging to different cities can therefore enter one
city's district map under the same name, dropping one defensive position. This
was reproduced before changing production code.

The shipped Gathering Storm `DLC/Expansion2/UI/CityBanners/CityBannerManager.lua`
reads the district's actual parent at lines 2635–2638:

```lua
local pDistrict = pPlayer:GetDistricts():FindID(districtID);
if (pDistrict ~= nil) then
    local pCity = pDistrict:GetCity();
    local cityID = pCity:GetID();
```

The observation is limited to visible, completed foreign districts
whose parent city center is revealed. Own plots retain the existing purchase
city API; older recordings retain the nearest-city fallback. An explicit but
unresolved parent must not be reassigned to another known city. Parent IDs are
scoped to the native owner.

Eight new Rust cases cover two distinct fort parents and actual independent
shots, farther known parent, legacy fallback, unknown parent, city-state parent,
owner-scoped IDs, persistent parent changes, and a nondefending Campus. Twenty-one
Lua cases cover observation, deltas, fog, failed getters, replacement identity,
parent visibility and ownership, completion, own plots, native city zero, and
nondefending districts, a newly unrevealed parent, and capture by us followed
by a return to foreign ownership. These were exercised against unchanged
production code before applying the fix.

No native candidate games have been run and no win-rate gain is established.

## Unchanged-production evidence

Computer checkpoint `8d07778e83c4a8d0cc62875a44f82d91986d2ef2` adds
fixtures and a test-module registration only. The exporter is byte-identical
and the importer has no production change. The complete mirror run executes
396 cases: all 388 existing cases and the new legacy fallback pass, seven new
cases fail, and two existing cases remain ignored. All 633 registered Rust,
data, Cargo and mod Lua inputs remain byte-identical through the run. The
failures include both physical fort ownership and the persistent parent change.

All 87 discovered Lua suites run: all 86 existing suites pass, while the new
19-case suite has seven expected failures. All 92 discovered mod scripts parse
under Lua 5.1. The expanded 21-case fixture, with explicit plot coordinates and
two additional identity/visibility cases, has nine failures and twelve passes
on an exact copy of that frozen production exporter. Original failure logs
and assertions are retained.

Independent CI `37943491151` also fails the physical-parent assertion, assigning
the fort to city 17 instead of its observed parent city 16. Its fail-fast run
passes 3,617 tests before that failure; later tests are unexecuted.

## Candidate validation

Source checkpoint `1090467a6629fd4f332ad7c4de1eeb6f75f707ad` passes all
396 local mirror cases, including all eight new cases and both actual
independent fort shots, with two existing cases ignored.

The complete, unfiltered local Nextest run passes all 4,774 tests across five
binaries, with 51 existing skips, one worker and zero retries (534.222 seconds).
All 396 mirror cases, eight parent cases, nine foreign-health cases and all
other crate/binary tests pass. Documentation examples have zero failures and
four existing ignored examples. All 633 registered source/input hashes stay
unchanged through the focused/full/documentation sequence.

The actual worktree Lua run passes all 87 discovered suites, all 21 new parent
cases and all 19 foreign-health cases; all 92 mod scripts parse under Lua 5.1.
All 633 registered input hashes remain unchanged.

Independent source-head CI `37949522999` passes all 4,774 tests, 51 existing
skips, four ignored documentation examples, all 396 mirror cases, all eight
parent cases, all nine foreign-health cases, and the overlapping 71/21/22
tournament/provenance sets. Quality, mod, policy, overwrite, publication and
security checks pass. Cost CI `37949522883` passes at -0.25% median over five
pairs/600 turns per arm, within the ±1% noise floor; IQR 1.60pp
[-2.59%, +0.09%], resolution ±1.06%, pooled -0.75%, +8% budget passed.
This is simulator CPU cost, not a native winning measurement. No speed gain
is claimed.

The final evaluation-only checkpoint leaves all 633 validated input files
byte-identical; independent final-head CI remains the integration gate.
The candidate keeps the eight original Rust cases byte-identical. Explicit
native parents resolve within the observed owner; observations without a
resolved native ID retain the legacy nearest-city approximation. Unknown
parent forts are not claimed to be fully modeled.
