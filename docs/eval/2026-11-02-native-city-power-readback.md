# Direct native city power observation

The Coal purchase candidate needs native power evidence before an apparent
fuel shortage or a mirrored yield gain can justify a strategy change.
Future captured city records now include an optional `power` object with
`free`, `temporary`, `required`, `fully_powered` and
`fully_powered_by_active_project`. Each is a direct host reading.

The installed shipped script
`DLC/Expansion2/UI/Additions/CityPanelPower.lua:36` obtains `city:GetPower()`;
lines 42–45 read free, temporary and required power, line 51 checks
`IsFullyPowered()`, and line 54 checks `IsFullyPoweredByActiveProject()`.
The last distinction matters: the UI sets current power equal to required
power for an active project. The export retains the original readings
without deriving a replacement supply total or powered flag.

Every accessor is guarded separately. Zero and false are observed values;
missing accessors, errors, invalid types and nonfinite numbers are omitted.
An entirely unavailable observation has no `power` field. This changes no
game command, model yield or controller policy. Historical captures remain
unknown and are not backfilled from the mirror.

The Lua behavior test executes the actual exporter for normal supply,
project-provided power, missing/throwing accessors and invalid readings.
The existing CI discovers every `*_test.lua` automatically. All 114 local
Lua test scripts pass using Lupa's Lua 5.1 runtime, all mod Lua files compile,
and 139 Python control-install tests pass. The register-budget test reports
182 of 200 registers for the agent. The scope scanner retains the same 169
baseline findings, with no new names introduced as scope slips.

A future live capture must still confirm these fields from the running
host. Synthetic API tests alone do not prove that the installed mod exported
them. They establish error handling and wire behavior without issuing host
commands or changing the active game.
