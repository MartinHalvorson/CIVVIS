# Observe enemy district pillage before assigning bomber missions

Native continuation `civvis-20260922T013415Z-cont3`, pinned to `590641be9305e94fd42020f26dc93e07e3053198`, repeatedly attacked the Aerodrome at (11,16) on turns 218–222, the Campus at (29,28) on 223–226 and 231, and the Campus at (32,27) on 227–229. By turn 227 a healthy Modern Armor stood next to Qusqu, whose 400 wall HP had fallen by only three. No bomber order targeted Qusqu through 232.

The plot exporter carries a district name and completion flag, but no district pillage state. Rival city records do not carry district lists. Each fresh board therefore reconstructs those districts without observing whether earlier native sorties pillaged them. The recording cannot establish the missing fact; repeated attacks alone are not proof of successful pillage or the sole cause of the failed siege.

The patch extends the existing plot `p` bit to observed districts, includes it in the delta signature, and applies it when reconstructing foreign infrastructure. Own districts retain their richer city observations. A visible district can update or clear the bit; outside sight, the exporter retains its last observation. Old recordings without the bit retain legacy behavior. This does not supply missing enemy-building pillage states.

Native accessor: shipped `Base/Assets/UI/WorldBuilderPlayerEditor.lua:732` reads `pDistrict:IsPillaged()`. The existing `CityManager.GetDistrictAt(x,y)` supplies the same district object used for the completion flag. Air-pillage remains the existing AIR_ATTACK operation: shipped `Base/Assets/UI/Panels/UnitPanel.lua:3629` explicitly includes air-pillage in air-attack targets.

## Validation in progress

Two Rust regressions fail on the previous mirror because a true district pillage bit is discarded. The mission test first verifies that an intact district is offered as a legal AirPillage target, then requires an observed pillaged district to be excluded. The repair test checks that an observed false bit clears the previous state. The initial test compile needed a corrected Action import; the subsequent baseline failure is the intended pillage-state assertion.

The actual Lua export function passes a harness covering a pillage-only delta, no resend on unchanged state, observed repair, no hidden repair leakage, improvement compatibility and a missing district accessor. All 65 mod Lua files parse under Lua 5.1 and all 60 discovered Lua harnesses pass. Full Rust checks, release validation and integration remain pending. No native behavior improvement or victory is claimed.
