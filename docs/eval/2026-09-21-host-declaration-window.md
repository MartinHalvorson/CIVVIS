# Use the host's declaration window

In native `civvis-20260921T134621Z`, the Aztec war ended on 91/2. The host's `can_declare` remained false through 98, despite a denouncement on 92. The planner treated that mature denouncement as permission to declare on 97–98. Firaxis refused those requests. When `can_declare` became true on 99, the bridge withheld otherwise selected declarations on 99–100 under its ten-turn `not_at_war` retry cooldown.

The host permission now masks voluntary declarations on the mirrored board, including both normal and casus-belli actions and their direct appliers. It is a current-turn observation, not an invented treaty duration. Missing permission remains unknown, while a later explicit permission clears the mask. Existing wars and other actors retain their normal behavior.

The retry filter records permission even when there is no declaration order. A transition from false to true clears that rival's declaration cooldown for `not_at_war`/`cannot_declare`; an unchanged true permission or missing data does not repeatedly bypass a genuine failure. Other orders and rivals retain their cooldowns.

Primary source: the shipped `Base/Assets/UI/PartialScreens/CityStates.lua:1496` reads `CanDeclareWarOn = pLocalPlayer:GetDiplomacy():CanDeclareWarOn(iPlayer)`. CIVVIS already exports the same engine accessor and its executor checks it before requesting war. This change makes the planner and retry filter consume that observation rather than infer permission from the denouncement alone.

Validation in progress. Later war against the Aztecs did execute on native 154/1; the earlier failure was a lost opportunity, not proof the declaration API never works. Frozen replay cannot establish that an earlier war would capture a capital or win. No native Domination victory is claimed.
