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

## The ladder, and how long it takes to climb

Fourteen orders of magnitude is a lot to ask of a wheel. The zoom used to move
by one fixed ratio per notch — the same ratio the flat board uses, where the
whole zoom is a factor of thirteen and fifteen notches cross all of it — and at
that ratio the galaxy is **two hundred notches** from the ground. Nobody scrolls
two hundred times, so most of what is written above could be built and reached
by nothing but a caption.

So a notch is a fixed fraction of the ladder in front of it instead. The step is
taken in log scale and divided by the height of the ladder that rung actually
has, which makes the trip the same length in the hand at every rung and only
changes how far it goes:

| rung | span of the ladder, in log scale | wheel notches before | after |
|---|---|---|---|
| `chart` | 1.9 | 12 | 12 |
| `eye` | 14.7 | 87 | 58 |
| `glass` | 27.3 | 161 | 64 |
| `space` | 34.4 | 203 | 65 |

The gearing is for crossing the dark, and crossing it is all it is for. Four
places out there are somewhere anyone is actually going — home, the Moon, Mars
and the world the expedition is aimed at — and over the last sixty-fourfold of
an approach to one of them the gearing is handed back in proportion to how far
in the camera is, until, standing over it, one notch means exactly what one
notch means on the flat board. Nothing in the sky is ever slower than it was
before there was a ladder at all, and nothing on the board changed: home is one
of the four, so a zoom over the map is the zoom it has always been.

Two things this had to be told. A world's drawn size is a property of the zoom
alone, so an arrival is a size test **and** a distance test — at a tile's zoom
over the Atlantic the Moon is nominally four stages wide, and the size test by
itself calls that an arrival at the Moon. And out at the destination the stop
belongs to the **star**: LHS 1140 is twelve times its own planet and sits at
the same point in the catalogue, so `planetMaxScale` answers with its ceiling and
the zoom ends while the planet is still a bead. Keyed to the planet alone, the
one arrival the whole expedition is about never got past 0.46 and ran out at
full gearing.

A pinch is not a step of intent — its scale follows the spread of the fingers
and is absolute — so it is geared at its own site, by raising that spread to the
pace the gesture started with, and capped: a hand opens by a factor of five, and
five to the fourth is already most of the solar system.

## The way about

Gearing made the whole ladder *reachable*. It did not make it navigable, and
those are different problems. Sixty-five notches is a reachable distance and an
unusable one; worse, every one of those notches has to be aimed, because a zoom
out here goes where the pointer says and the pointer is over empty space for
most of the way. Coming back from the galaxy to a city was a minute of
scrolling that could be lost at any point in it.

So the sky carries its own way about, on a bar that stands over the zoom
buttons — the same control, in the same place — and is only there while there
is a sky to cross. Home wholly on the stage with room around it puts it up; home
filling the frame takes it down again. Three things on it, in the order a hand
reaches for them:

**The places.** Home, the Moon, Mars, and the world the expedition is aimed at.
Those four and no others, because those four are already what the sky calls an
arrival: they are the three destinations of the space programme and the one
board they are launched from, and the gearing has handed itself back over
exactly them since there was a ladder at all. The bar names what the zoom
already knew. Each is offered only once this civilization can see it, off the
same roster everything else out here is drawn from, so the bar can never point
at something the sky is withholding.

A press lands with that world four fifths of the frame across — short of the
ceiling the zoom itself has for it, deliberately, because a jump that lands
exactly on the stop leaves the zoom-in button dead in the hand on arrival.

**The shots.** The whole system, the neighbourhood, the galaxy: one press each.
They are the frames the zoom already stops at, taken out of `skySystemFrame` so
that a named shot and the far stop are the same arithmetic rather than two
opinions that could drift apart. At the `eye` rung `System` *is* the far stop,
because the naked eye's universe ends at Saturn.

**The ladder.** The zoom laid out end to end, with the camera's own place on it
and how wide the stage is beside it in the unit that reads best there — twelve
thousand kilometres is a hemisphere, an AU is the inner system, a light-year is
the dark. It is the same ladder the gearing is measured against, so the handle
is not a second opinion about how far out anything is; it is the gearing's own
ruler with a grip on it.

A ladder is a road, and a road has to go somewhere. This one goes to whatever
the camera is standing on and home when it is standing on nothing, which is the
journey out here everybody makes and the one the wheel is worst at. The pan
falls out of the scale — at the bottom the shot this rung's horizon wants, by
the top over the world the road leads to, crossing between them exactly as that
world's drawn size crosses the stage — so the handle never has to remember
anything.

What it must *not* do is go through the ordinary zoom, and that had to be found
out by dragging it. A zoom in stops at the ceiling of the world nearest the
camera, which is right for one notch and catastrophic for a whole ladder in one
move: from the galaxy the nearest world is some red dwarf, its ceiling is a
hundredth of the way up, and dragging the handle to the ground put the camera
there and left it.

### The far half of the sky was never coming back

Dragging the handle to the far stop also found a bug that had been in the sky
for as long as the sky has had true distances in it, and it is the same bug as
the pinch's, one guard along.

The eased planet zoom moves by `cam.scale *= exp(log(target / max(1e-6,
cam.scale)) * ease)`. That guard's only job is to keep a divisor off zero, and
it was chosen when the smallest scale in the world was the flat board's. **The
sky past about a hundred AU is a smaller number than `1e-6`**, so out there the
divisor stopped tracking the camera and became a constant: every frame
multiplied the scale by the same number below one, and the ease could not
converge. `scaleLeft`, which decides when the move is over, was measured against
the same lie, so nothing ever noticed.

Measured on unmodified `main`: seventy wheel notches out from a tile come to
rest at a scale of **2.2×10⁻¹⁵⁵** against a far stop of 3.7×10⁻¹⁵. That is a
hundred and forty orders of magnitude past the end of the universe, the camera
is still zooming, and it never stops. Everything out past Neptune was a blank
screen you could not scroll back from — which is most of what the four rungs
above were built to show. The guard is `1e-30` now, and the same seventy notches
come to rest exactly on the stop.

### A jump is a path, not an ease

Easing a zoom and a pan toward the same target together is the obvious thing and
it is unwatchable across fourteen orders of magnitude: the trip from a tile to
the destination spends its entire middle at a tile's zoom over empty space with
nothing on the stage at all. Doing the pan first and the zoom after is a smear.

Van Wijk and Nuij's construction is the answer, and it is not a curve anybody
drew — given both ends it is the *shortest* path under a metric that counts how
much the picture moves in the eye, and what falls out is what anyone would do by
hand: pull back until both ends are in view, cross at that height, come back
down. It is written entirely in `w`, how wide the stage is in Earth radii,
because that is the one number a zoom and a pan both change.

Two things it had to be told. Its `ln(-b + sqrt(b² + 1))` has to be written as
`-asinh(b)`: a camera crossing fourteen orders of magnitude makes `b` as large
as 10¹⁵, at which point `sqrt(b² + 1)` is exactly `b` in float64, the
subtraction cancels to zero, the logarithm is negative infinity and the whole
flight comes out `NaN` with the camera never moving. And its own answer for how
long the trip takes — a constant speed along the path — is twenty-five seconds
for the whole sky, which is right for a film and wrong for a control. It keeps
his pace for a short hop and is capped after that.

The zoom buttons themselves now repeat while held, which on the flat board is a
convenience and out here is the difference between a control and an ornament.

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
