# CIVVIS keyboard shortcuts

The bindings below are not chosen. They are Civilization VI's own defaults,
read out of that game's `InputSettings.json` on a machine it is installed on,
because the whole point of the map is that a person who has played Civ 6 can
sit down at CIVVIS without learning one.

Two maps, for two things a person can be doing.

## Playing

Everything here needs a seat, so none of it is live while you are watching a
simulation play itself.

| Key | Civ 6 action | Here |
| --- | --- | --- |
| `1` | EndTurn | Resolve the next blocker, or end the turn |
| `Shift`+`1` | — | End the turn regardless of blockers |
| `B` | FoundCity | Found a city with the selected Settler |
| `F` | Fortify | Fortify the selected unit |
| `H` | FortifyUntilHeal | Fortify and wait out the damage |
| `Space` | SkipTurn | Done for this turn only |
| `Z` | Sleep | Rest until something changes |
| `V` | Alert | Stand watch until an enemy comes near |
| `E` | AutoExplore | Walk toward the edge of the known world |
| `.` | NextUnit | Next unit needing orders, in roster order |
| `,` | PrevUnit | Previous unit needing orders |
| `N` | — | Next action: the nearest unit still waiting |
| `]` | NextCity | Next city in placement order |
| `[` | PrevCity | Previous city in placement order |
| `\` | CapitalCity | Go to the capital |
| `→` | — | Next city — this client's own pair, kept |
| `←` | — | Previous city |
| `T` | ToggleTechTree | Technology tree |
| `C` | ToggleCivicsTree | Civics tree |
| `L` | ToggleReligion | Empire ▸ Religion |
| `O` | ToggleGreatPeople | Empire ▸ Great People |
| `D` | OpenQDPopup | Quick Deals |
| `F7` | ToggleGovernment | Empire ▸ Government |
| `F10` | ToggleGovernors | Empire ▸ Governors |
| `F2` | ToggleCityStates | Empire ▸ City-States |
| `F3` | ToggleEspionage | Empire ▸ Espionage |
| `F4` | ToggleTradeRoutes | Empire ▸ Trade |
| `F8` | ToggleReports | Empire ▸ Cities — the report Civ 6 keeps here |

City placement order is ascending city ID, which is allocated as each city is
placed.

## Watching, or playing

These describe the picture rather than the empire, so they work over a
simulation too.

| Key | Civ 6 action | Here |
| --- | --- | --- |
| `F1` | ToggleRankings | The rankings report — standings and victory tracker |
| `F9` | OpenCivilopedia | Civilopedia |
| `Y` | ToggleYield | Tile yields |
| `G` | ToggleGrid | The hex grid |
| `Q` | ToggleResources | Resource markers |
| `End` | ToggleFSMap | Fullscreen map |
| `2` | LensContinent | Continent lens |
| `3` | LensAppeal | Appeal lens |
| `4` | LensSettler | Settler lens |
| `5` | LensGovernment | Government lens |
| `6` | LensPolitical | Political lens |
| `7` | LensTourism | Tourism lens |
| `8` | LensLoyalty | Loyalty lens |
| `9` | LensEmpire | Empire lens |
| `0` | LensPower | Power lens |
| `Shift`+`A` | AddMapTack | Map-tack placement |
| `Escape` | — | Close the topmost screen, then unwind map modes and the selection |

A lens key pressed twice puts the lens away, which is what its button does.

## Where this map cannot be Civ 6's

- **`A` is Attack there**, and CIVVIS attacks by pointing at a tile. `A` is
  left unbound rather than given a second meaning, and Alert sits on `V`,
  which is where that game has always had it. `A` was CIVVIS's Alert key
  before this table was reconciled with the game's.
- **F5, F6, F11 and F12 belong to the browser** — reload, address bar,
  fullscreen, developer tools — so Civ 6's QuickSave and QuickLoad are not
  taken. Saving and loading are in the command deck.
- **Tab is not bound.** Civ 6 leaves it alone and so does this: Tab is how
  somebody navigating by keyboard reaches every control on the page.
- **`M`, `R` and `A`** — MoveTo, RangedAttack, Attack — are pointer
  interactions here, so they have no key.
- **`W` ToggleGreatWorks, `F11` ToggleTimeline, `F6` QuickLoad** and the rest
  of Civ 6's map name screens this client does not have.

Buttons and pointer controls remain available for everything in both tables.
