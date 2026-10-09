# Observed parent cities for foreign districts

A visible rival district currently uses the nearest known city as its mirrored
parent. Two Encampments belonging to different cities can therefore enter one
city's district map under the same name, dropping one defensive position. This
is a static finding; the unchanged-production regression run is pending.

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
owner-scoped IDs, persistent parent changes, and a nondefending Campus. Nineteen
Lua cases cover observation, deltas, fog, failed getters, replacement identity,
parent visibility and ownership, completion, own plots, native city zero, and
nondefending districts. These are registered before production changes.

No native candidate games have been run and no win-rate gain is established.
