# Observed parent cities for foreign districts

A visible rival district currently uses the nearest known city as its mirrored
parent. Two Encampments belonging to different cities can therefore enter one
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

The proposed observation is limited to visible, completed foreign districts
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

Pending: complete mirror controls, every Lua suite and parse check, full
process-isolated Rust tests, documentation examples, and independent final CI.
The candidate keeps the eight original Rust cases byte-identical. Explicit
native parents resolve within the observed owner; observations without a
resolved native ID retain the legacy nearest-city approximation. Unknown
parent forts are not claimed to be fully modeled.
