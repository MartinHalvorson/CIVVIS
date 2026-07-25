# Key bindings and movement controls

CIVVIS uses Civilization VI's own default controls. A player who knows Civ 6
should not have to learn a second board.

The table below is not remembered — it was read out of the shipped game on this
machine, from

```
Sid Meier's Civilization VI/Civ6.app/Contents/Assets/Base/
  Assets/Configuration/Data/InputConfiguration.xml
```

(the `InputActionDefaultGestures` table), plus the rows `Expansion1_Config.xml`
and `Expansion2_Config.xml` add. `ActionId` below is Civ 6's own, so any row
here can be checked against that file directly.

## What matches Civ 6

### Units

| Key | Civ 6 `ActionId` | CIVVIS |
| --- | --- | --- |
| `Space` | `SkipTurn` | Skip this unit; with nothing selected, end the turn |
| `F` | `Fortify` | Fortify |
| `H` | `FortifyUntilHeal` | Fortify and stay out of the unit cycle until the damage is gone |
| `Z` | `Sleep` | Sleep — only its own wounds wake it |
| `V` | `Alert` | Stand watch — also wakes when something hostile comes within two tiles |
| `E` | `AutoExplore` | Walk toward the nearest edge of the known world, turn after turn |
| `B` | `FoundCity` | Found a city |
| `M` | `MoveTo` | Arm the next click as a move order |
| `A` | `Attack` | Arm the next click as an attack |
| `R` | `RangedAttack` | Arm the next click as a ranged or theological attack |
| `,` | `PrevUnit` | Previous unit needing orders |
| `.` | `NextUnit` | Next unit needing orders |

`M`, `A` and `R` do not act on their own in Civ 6 either: they arm the next
click. Clicking a tile already moves and already attacks here, so these are what
make the board playable without reaching for the mouse first.

### Screens

| Key | Civ 6 `ActionId` | CIVVIS |
| --- | --- | --- |
| `Return` | `EndTurn` | Walk the turn blockers, then end the turn (`Shift+Return` ends it anyway) |
| `T` | `ToggleTechTree` | Technology tree |
| `C` | `ToggleCivicsTree` | Civics tree |
| `F7` | `ToggleGovernment` | Empire → Government |
| `L` | `ToggleReligion` | Empire → Religion |
| `O` | `ToggleGreatPeople` | Empire → Great People |
| `F10` | `ToggleGovernors` | Empire → Governors |
| `F2` | `ToggleCityStates` | Empire → City-States |
| `F3` | `ToggleEspionage` | Empire → Espionage |
| `F4` | `ToggleTradeRoutes` | Quick Deals |
| `F9` | `OpenCivilopedia` | Civilopedia |
| `Ctrl+,` | `CivilopediaBack` | Back along the Civilopedia trail |
| `Ctrl+.` | `CivilopediaForward` | Forward along it |
| `End` | `ToggleFSMap` | Full-screen map: deck away, whole world framed |
| `Home` | `PauseMenu` | Game settings, which is where saving and starting live |
| `F5` | `QuickSave` | Write `quicksave` |
| `F6` | `QuickLoad` | Load `quicksave`, or the newest save if there is none |
| `[` | `PrevCity` | Previous city |
| `]` | `NextCity` | Next city |
| `\` | `CapitalCity` | Go to the capital |
| `P` | `OnlinePause` | Pause (spectator board) |

Each key both opens and closes its screen, as in Civ 6. While a screen is up it
owns the keyboard: only the screen keys and `Escape` reach through it.

### The map

| Key | Civ 6 `ActionId` | CIVVIS |
| --- | --- | --- |
| `Y` | `ToggleYield` | Tile yields |
| `G` | `ToggleGrid` | Hex grid |
| `Q` | `ToggleResources` | Resource icons |
| `+` | `Toggle2DView` | Flat strategic view, and back to the view you were in |
| `←` `→` `↑` `↓` | `CameraPan*` | Pan the camera (`Shift` for twice the step) |
| Numpad `+` / `−` | `ZoomIn` / `ZoomOut` | Zoom |

Civ 6 spends the main-row `+` on the 2D view and zooms on the keypad only. Both
are honoured here; `=` and `−` are added as aliases because a laptop has no
keypad (see the additions below).

### Mouse

| Gesture | Civ 6 | CIVVIS |
| --- | --- | --- |
| Left click | `OnMouseSelectionEnd` — select what is in the plot | Same |
| Left drag | `StartDragMap` — pan | Same |
| Right press | `OnMouseSelectionUnitMoveStart` — show the movement path | Same |
| Right release | `OnMouseSelectionUnitMoveEnd` — move there | Same, and a tile out of this turn's range becomes a multi-turn travel order |
| Middle click | `OnMouseSelectionSnapToPlot` — centre on that plot | Same |
| Wheel | `OnMouseWheelZoom` | Same |
| Alt + drag | Spin the camera | Rotates, and in cinematic also tilts |
| Pointer at a map edge | `EdgePan` (Gameplay option, ships on) | Same, and a setting under Display settings |

## Deliberate overrides

The operator asked for these three, so they keep their CIVVIS meaning. Civ 6
spends `1`, `2` and `3` on the religion, continent and appeal lenses, none of
which exist here.

| Key | CIVVIS | Civ 6 would be |
| --- | --- | --- |
| `1` | Next action | `LensReligion` |
| `2` | Settler lens | `LensContinent` |
| `3` | Place a map tack | `LensAppeal` |

## CIVVIS additions

These live only on keys and chords Civ 6 leaves unbound, so nothing here can
shadow a binding somebody arrived with.

| Key | Does |
| --- | --- |
| `F8` | Diplomacy — Civ 6 has no hotkey for its diplomacy screen, and `F8` is free |
| `Tab` | Next unit needing orders (`.` is the Civ 6 way to the same place) |
| `=` / `−` | Zoom, for a keyboard with no numeric keypad |
| `Ctrl+A` | Hand the seat to an agent for a stretch of turns |
| `Ctrl+R` | Face north and reset the view |
| `Ctrl+U` | Hide and show the command deck |
| `Ctrl+←` `Ctrl+→` | Rotate the map |
| `Ctrl+↑` `Ctrl+↓` | Tilt the map |
| `Shift`+scroll | Rotate; `Alt`+scroll tilts |
| Double click | Dive in; `Shift`+double click pulls back |
| `Escape` | Close the topmost screen, then cancel an armed mode, a follow, the full-screen map, and finally the selection |

Two spectator-only keys, `M` for cinema audio and `C` for cinema mode, are live
only while a cinema is running — a board no seated player is ever looking at —
so they cannot shadow Civ 6's `M` and `C`.

## Civ 6 actions with nothing here to bind

Left unbound rather than pointed at an approximation:

| Civ 6 `ActionId` | Key | Why |
| --- | --- | --- |
| `DeleteUnit` | `Delete` | The engine has no player-facing disband |
| `ToggleGreatWorks` | `W` | No Great Works screen yet |
| `ToggleRankings` | `F1` | No rankings screen; the victory tracker is always on screen |
| `OpenMapSearch` | `Ctrl+F` | No map search |
| `ToggleTimeline` | `F11` | No timeline; the event log is always on the deck |
| `ToggleEraProgress` | Numpad `/` | No era-progress screen |
| `ToggleWorldClimate` | `PageDown` | No climate screen |
| `LensSettler` and the rest of `Lens*` | `4`–`0` | The settler lens is the only lens CIVVIS has, and it is on `2` by the override above |

When one of those screens is built, its Civ 6 key is the one it should get.

## Where this lives in the code

`web/index.html` holds the whole table in `CIV6_BINDINGS`, keyed by Civ 6's own
`ActionId`, with the CIVVIS-only rows in `CIVVIS_BINDINGS` beside it. Both are
flattened into one `KEY_ACTIONS` lookup, so adding a binding is adding a row and
never another branch in the key handler.
