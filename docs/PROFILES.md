# Game profiles

Profiles are versioned setup/rule bundles. The browser puts the profile
directly below the AI-only/players control; the CLI exposes the same choice as
`--game-profile`. A profile establishes defaults first, after which map, world
size, speed, disaster intensity, turn cap, and victory controls can override
ordinary lobby values.

Every game, observation, supervisor handoff, and save records the selected
profile. This prevents a resumed or replayed game from silently changing its
climate or tournament policy.

## Civ VI tournament (default)

ID: `civ6-tournament`

This is CIVVIS' AI-simulation translation of the CPL standard FFA setup,
pinned to the dated `cpl-ffa-2026-07` enforceable policy:

- Ancient start, Online speed, map-size-default civilizations/city-states,
  balanced starts, standard climate controls, all victories, Barbarians and
  Tribal Villages on, and no optional Firaxis game modes;
- client map tacks disabled, matching the CPL/MPH lobby rule;
- CPL restrictions on Gold/strategic trades, city trades, Military Alliances,
  city-state keep tokens, same-turn resurrection, and the other policy rules
  listed in [COMPETITIVE.md](COMPETITIVE.md);
- Pangaea is the initial map selection, while the four implemented map scripts
  remain selectable because current CPL games vote or announce their map.

CPL also specifies simultaneous turns and Multiplayer Helper's dynamic timer.
Those concepts have no effect in an all-AI deterministic simulator, whose
authoritative action log is sequential. The profile therefore pins every
gameplay/lobby value the current engine can execute; it does not mislabel a
network timer or a full BBG import as implemented. See the remaining import and
multiplayer boundaries in [COMPETITIVE.md](COMPETITIVE.md).

Source: [current CPL in-game rules](https://cpl.gg/rules/in-game-rules/).

## Civ 6.5 · Gathering Storm+

ID: `civ65`

This is the expanded CIVVIS ruleset rather than a Firaxis product name. It
keeps the complete implemented Gathering Storm systems—Power and strategic
fuel, lifetime CO2, seven irreversible sea-level phases, natural disasters,
World Congress, Emergencies/Aid Requests, Governors, and future-era
progression—and removes the CPL policy layer.

Its gameplay mod component is the supported core of JNR's **Climate Balance –
Complete Pack**:

- climate phase thresholds require 1.5× as much CO2 (50% slower warming);
- nuclear power and uranium-unit emissions are reduced by 75%;
- climate-driven storms become more frequent and reach higher severity sooner
  as warming phases advance.

The profile also identifies the QoL features already native to the CIVVIS
client: Quick Deals, policy-card effect details, detailed map tacks, and the
combined spectator/event-log tools. UI-only helpers do not alter deterministic
game state.

The name is intentionally a curated profile, not the removed 72-item Steam
collection that happens to share “Civilization 6.5 Pack” in its title. That
collection is marked incompatible and includes many visual/content mods that
do not map to a headless simulator. The climate component and its exact public
behavior are documented on the
[Climate Balance Workshop page](https://steamcommunity.com/sharedfiles/filedetails/?id=1667883116).
