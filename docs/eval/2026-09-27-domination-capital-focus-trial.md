# Fixed-front capital prioritization: registered trial

This measures the existing, default-OFF `domination-capital-focus` option.
There is no controller, deployment, gene-default, or evaluator change. The
native Domination objective and difficulty ladder remain unchanged.

## Registration before outcomes

Registered at **2026-09-27 03:46:05.404739 UTC**, before any fresh trial game.
The frozen source is claim commit
`a2949626977077a5f039fe247b422c606967a441`, whose complete tracked tree is
identical to parent main `be8a36019d3c7af08aa9e8767a5f939fd334eefb`.
This includes #3799 and #3801; it excludes the withheld #3791, #3796,
and #3800 policies and the in-progress #3802 prototype.

Eight fresh seeds, **37923000–37923007**, use the existing
`victory_eval --domination-pair domination-capital-focus --difficulty prince`
probe. Both arms have four majors, Gran Colombia in seat zero targeting
Domination, Tiny 60×38 Pangaea, six city-states, Online speed, the natural
250-turn clock, all victories, and Prince players and barbarians. Only seat
zero's named option changes. All rival controllers remain fixed.

The exact 21-option focal force bundle is `deploy/live-force-on.txt`, SHA-256
`439757684071b807eb4151e055b4a9adf6ac24b7b8b6c0b5da7327b5344dcea0`.
It does not contain `domination-capital-focus`. Four independent two-pair
segments preserve the alternating OFF/ON execution order. Outcomes are
inspected only after all eight pairs finish. Every ending, action comparison,
major-city conquest, capital retention, and home-capital loss is retained.
An evaluator rejection for absent behavioral contrast remains a rejection.

Before fresh games, record binary hashes, actual policy readback, relevant
existing tests, and a persistent replay of all 202 archived native frames.
Those archived future states stay fixed: changed replay orders cannot establish
counterfactual conquest, city survival, or a native win. This eight-pair pilot
is directional evidence. A promising result requires a separately registered
16-pair replication before considering activation. There is no tuning after
fresh outcomes without a new registration.

## What the existing option can change

`AdvancedAi::assess` selects the rival first. Its capital fallback ordinarily
uses the cheapest missing capital across rivals, then requires that capital's
present owner to match the selected front. The option instead ranks eligible
missing capitals inside that front. The helper uses current ownership and can
include recapturing our own original capital.

Emergency objectives, an opening rush capital, victory suppression, conversion
campaigns, and an existing city campaign all precede this fallback. Siege
commitment can retain an already selected enemy city afterward. War readiness
and capture-deferral checks still apply. The option therefore does not promise
to redirect every campaign or overcome an inadequate army.

## Evidence location and status

Local evidence root:
`~/civvis-war-evidence-20260926/prince-capital-focus/`.
`preregistration.json` records the complete registration and fixed force bundle.
Build, replay, and fresh-game results are pending. The option stays OFF.
