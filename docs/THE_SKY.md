# The sky beyond

A Planet world hangs in a solar system, in a neighbourhood of stars, in a
galaxy. Pulling the camera back past the surface leaves the Earth behind and
keeps going, and the space race — the one victory whose progress happens
entirely off the map — happens out there in front of you.

**Everything out here is at true scale.** That is the change this document
exists to record. The sky before it put the Moon six Earth radii away, Mars at
eighteen and the exoplanet at fifty-two, so three zoom steps could visit all of
them. It made a picture, and the picture was a lie about the one thing the view
is for. The real gaps *are* the content.

## The ruler

The Earth's own radius, because it is the one length the player already has
underfoot. Everything below is stored in the unit it is published in and
converted in exactly one place, in `web/index.html`:

| | |
|---|---|
| Earth radius | 6,371 km (IUGG mean) |
| 1 AU | 149,597,870.7 km = **23,481 Earth radii** |
| 1 light-year | 9.4607×10¹² km = **1.485×10⁹ Earth radii** |

So the camera spans about **fourteen orders of magnitude** between a hexagon on
the ground and the far rim of the galaxy. Two consequences run through the whole
implementation:

- **Positions are one linear space in Earth radii**, not a stack of separate
  scenes. `float64` carries fifteen significant figures, which leaves 25 km of
  precision at the galactic centre and sixty metres on the surface of a planet
  fifty light-years away. Nothing needs a change of frame.
- **A body's drawn size needs a floor** (`floor` in the tables). At true scale a
  planet is a fraction of a pixel across the moment its own orbit fits on the
  stage, and a solar system of nothing is not a picture of a solar system.

## Four rungs

The sky is not the same sky for everyone under it. Which parts of it a
civilization may look at is decided on the server, in `src/obs.rs`, beside the
rule about who has found north — reported, never enforced, and a spectator
always has all of it. The rungs are the actual history of finding each thing
out:

| rung | gate | what is out there | the zoom stops at |
|---|---|---|---|
| `chart` | — | nothing. A flat sheet. | the chart limit |
| `eye` | `knows_globe` | the Sun, the Moon, and the five wandering stars | Saturn's orbit, 28 AU |
| `glass` | `scientific_theory`, or **Isaac Newton** | the outer system, and a neighbourhood with real distances in it | the 52-light-year bubble |
| `space` | `launch_earth_satellite` | the destination, and the galaxy | the galaxy, 160,000 ly |

Mercury, Venus, Mars, Jupiter and Saturn are naked-eye objects and were known to
every civilization that ever looked up, which is why the `eye` rung is a real
solar system and not two rocks — and why its universe *ends* at Saturn, exactly
where the naked eye's does.

The rung above is the one the telescope opened, and it opened as one piece:
Uranus in 1781, Ceres in 1801, Neptune in 1846 by being predicted before it was
looked for, and in 1838 the first measured distance to another star. So the
outer system and a neighbourhood with distances on it arrive together.

No planet at another star had ever been seen from the ground; the first came in
1992, and the shape of our own galaxy was not settled until the 1920s. Both wait
for the satellite, which was already the gate for the exoplanet.

## What is in it

**The system** — the Sun, eight planets, Ceres and Pluto, the Moon, the four
Galilean moons and Titan. Real radii, real semi-major axes, real periods. The
arrangement of the orbits is seeded from the game rather than taken from the
in-game date: at forty years a turn Mercury goes round a hundred and sixty times
between two of them, and a date-accurate orrery would teleport every planet on
every end of turn and read as a fault rather than as an orbit.

**The neighbourhood** — 57 real stars, every system within about fifty
light-years that anybody has a name for, stored as galactic longitude, latitude
and distance. That is the coordinate system the galaxy itself is defined in, so
the same numbers place a star in the bubble and place the bubble in the galaxy
with no second table that could disagree with the first. A star's drawn size and
brightness are its apparent magnitude, so the picture is dominated by dim red
dwarfs with a handful of conspicuous A and F stars in it — which is what the
real neighbourhood is.

**The destination** — ten real candidate worlds, from Proxima Centauri b at 4.24
light-years to **LHS 1140 b at 48.9**, which is the strongest habitable-zone
candidate anybody actually has and very nearly the fifty light-years the engine
has always quoted. The list carries the real trade-off: the nearest worlds are
around flare stars and tidally locked, and the calmest star with a temperate
planet is nearly five times further away.

**The galaxy** — a barred spiral, the Sun 26,700 light-years out from
Sagittarius A* (GRAVITY, 2019), four major arms written by where they cross our
own line so that the Sagittarius-Carina arm falls inside us and Perseus outside,
which is what puts the Sun between them.

## Still tiled

Every solid body carries its own tiling: the same subdivided-icosahedron
construction the Earth has — hexagons everywhere and exactly twelve pentagons —
coarser on a smaller world so a tile is about the same size on screen whichever
one you are standing over. Each has its own ground rule reading two ranked noise
fields plus its own latitude, and each is the thing that world is actually known
for: Io is sulphur, Europa is cracked ice, Titan has dark equatorial dunes and
methane lakes at its north pole, Mars has its caps, Saturn has its bands and the
hexagon at its pole. A gas giant tiled into hexagons is a strange object and it
is meant to be — there is nothing to stand on, so what the tiling quantises is
the cloud deck.

The destination is painted by its **grade**, not by its name: `best` is the
ocean world the expedition is hoping for, `good` is a world with water and land
on it, and `marginal` is what the nearby candidates mostly are — a tidally
locked rock with its atmosphere piled up on the night side.

## The survey, and where the expedition actually goes

The destination is not a fixed place any more. `EXOPLANET_TARGETS` in
`src/game.rs` is the roster the client draws, and which of it a civilization has
found is what its space programme bought:

| project | worlds found |
|---|---|
| `launch_earth_satellite` | 3 |
| `launch_moon_landing` | +2 |
| `launch_mars_colony` | +2 |
| each laser station | +1 |

The order the neighbourhood is found in is a fact about the sky, not about a
civilization, so it is one Fisher-Yates shuffle drawn from the game's seed and
shared by everybody: a deeper survey is always a strict superset of a shallower
one, and a rival who has looked less can never know a world you do not. It is
shuffled rather than ordered by distance because detection is not a matter of
distance — TRAPPIST-1 is forty light-years out and was found early, because its
star is small enough for a planet to blot out a real fraction of it.

The choice is made the day the ship leaves and never revisited. A survey that
deepens afterwards does not turn it round, which is what makes finishing the
Moon and the Mars colony *before* launching worth anything — and those two
projects had no reason to exist before this. The Moon paid a one-off Culture
bonus, Mars paid nothing at all, so a civilization racing the science victory
correctly skipped straight past both.

**The trip is the same length whichever world it is, deliberately.**
`EXOPLANET_DESTINATION` is still 50 and the expedition still crosses it at the
same pace, so nothing about who wins a game or when has changed. The roster's
real distances span 4.24 to 48.9 light-years — eleven to one — and letting that
spread loose on the victory race would be a large balance change that has to be
measured over whole 500-turn games before it ships, not asserted. The wiring for
it is `Game::exoplanet_target(pid).light_years`, already reported to the client
as `exoplanet_target_ly`; the measurement is the work, not the code.

## Fifty light-years, in galactic terms

This is what the last zoom step is for, and it is drawn rather than asserted:
the bubble at the far end is the same fifty light-years it was one step earlier,
in the same coordinates, and it comes out the size it comes out.

- **1/2,000th** of the way across the disc.
- **1/534th** of the way to the galactic centre.
- About **2,000 stars**, against a hundred billion or more in the galaxy.
- On a picture of the Milky Way the size of a dinner plate, the entire
  neighbourhood the previous screen was full of is **half a millimetre** wide.

The longest journey anybody in the game ever makes does not leave the doorstep.

## Working on it

`web/index.html`, in the block between `the sky beyond` and `the flat board's
sky`. The camera, its stops and what a drag means are unchanged from the
[globe rules](../web/index.html); what changed is that `skyBodyPos` now reads a
real arrangement and `planetMinScale` frames the outermost thing the current
rung knows about rather than a bounding box over three bodies.

Measured on a 1600×1000 stage: the whole-galaxy shot draws in **8 ms**, the
neighbourhood and the system in about **2.5 ms**, and a tiled world close up in
**0.8 ms** — against roughly 55 ms for the surface map, so nothing out here is
near a frame budget.
