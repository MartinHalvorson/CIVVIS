# The sky beyond

A Planet world hangs in a solar system, in a neighbourhood of stars. Pulling the
camera back past the surface leaves the Earth behind and keeps going, and the
space race — the one victory whose progress happens entirely off the map —
happens out there in front of you.

**It stops at twenty light-years.** The sky used to carry on out to the whole
Milky Way, and that was true and it was not a place: nothing anybody does in
this game happens out there, and the price of drawing it was that every picture
worth having sat in the first two per cent of the zoom. The far stop is the
**voyage** now — the Sun at one end, the world the expedition is aimed at at the
other, and the stars it crosses in between.

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

So the camera spans about **eleven orders of magnitude** between a hexagon on
the ground and the far end of the voyage. Two consequences run through the whole
implementation:

- **Positions are one linear space in Earth radii**, not a stack of separate
  scenes. `float64` carries fifteen significant figures, which leaves sixty
  metres of precision on the surface of a planet fifty light-years away — and it
  had enough left at the galactic centre, back when there was one. Nothing needs
  a change of frame.
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
| `glass` | `scientific_theory`, or **Isaac Newton** | the outer system, and a neighbourhood with real distances in it | 20 light-years across |
| `space` | `launch_earth_satellite` | the destination | the voyage — the same 20 across |

Mercury, Venus, Mars, Jupiter and Saturn are naked-eye objects and were known to
every civilization that ever looked up, which is why the `eye` rung is a real
solar system and not two rocks — and why its universe *ends* at Saturn, exactly
where the naked eye's does.

The rung above is the one the telescope opened, and it opened as one piece:
Uranus in 1781, Ceres in 1801, Neptune in 1846 by being predicted before it was
looked for, and in 1838 the first measured distance to another star. So the
outer system and a neighbourhood with distances on it arrive together.

No planet at another star had ever been seen from the ground: the first came in
1992, and it took a telescope above the air to make the rest of them ordinary.
So the destination waits for the satellite, which was already its gate.

## What is in it

**The system** — the Sun, eight planets, Ceres and Pluto, the Moon, the four
Galilean moons and Titan. Real radii, real semi-major axes, real periods. The
arrangement of the orbits is seeded from the game rather than taken from the
in-game date: at forty years a turn Mercury goes round a hundred and sixty times
between two of them, and a date-accurate orrery would teleport every planet on
every end of turn and read as a fault rather than as an orbit.

**The neighbourhood** — 57 real stars, every system within about fifty
light-years that anybody has a name for, stored as galactic longitude, latitude
and distance, which is the frame those distances are published in. A star's
drawn size and brightness are its apparent magnitude, so the picture is
dominated by dim red dwarfs with a handful of conspicuous A and F stars in it —
which is what the real neighbourhood is.

The roster deliberately runs out past the twenty light-years the zoom stops at.
A destination is a planet *around one of these stars* and the expedition can be
aimed at a world forty out; what the stop truncates is how far the camera goes
on its own, not what is there to be flown to.

**The destination** — ten real candidate worlds, from Proxima Centauri b at 4.24
light-years to **LHS 1140 b at 48.9**, which is the strongest habitable-zone
candidate anybody actually has and very nearly the fifty light-years the engine
has always quoted. The list carries the real trade-off: the nearest worlds are
around flare stars and tidally locked, and the calmest star with a temperate
planet is nearly five times further away.

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

## Why it stops where it does

There used to be one more rung out here: the whole Milky Way, a hundred thousand
light-years across, with the entire neighbourhood one dot on one arm of it. It
was drawn from real numbers and it made the point it was built to make — the
longest journey anybody in the game ever makes does not leave the doorstep.

It is gone, and what it cost is the argument for dropping it. That last picture
was **three thousand times** wider than the one below it, so the far stop sat at
144,741 light-years across while every shot anybody would ever want — the
system, the neighbourhood, the trip — lived in the first fraction of a per cent
of the ladder. Two thirds of the sky's whole depth was a scale nothing in the
game happens at, and every wheel notch and every drag out there was spent
crossing it.

The stop is twenty light-years now, and the far end of the zoom is a picture of
the voyage rather than a picture of a scale.

**And twenty means twenty on the screen.** The stop is written as the *width of
the stage* — `SKY_STOP_LY`, the number the bar prints beside the handle — and not
as a reach. Every other shot out here is a radius inside the padded frame, which
is a different length by a factor of two and by the padding: a stop written as a
twenty-light-year *reach* printed **45.7 ly** underneath itself, which is a view
arguing with its own caption. `skyFrameFor` and `skyReachForStageWidth` are the
two directions of one conversion, so the two numbers cannot drift apart. The
shell that is actually drawn is half the stop, so the ring spans the stage
instead of hanging off the edge of it.

## The ladder, and how long it takes to climb

Eleven orders of magnitude is a lot to ask of a wheel. The zoom used to move by
one fixed ratio per notch — the same ratio the flat board uses, where the whole
zoom is a factor of thirteen and fifteen notches cross all of it — and at that
ratio the far stop is **eighty-four notches** from the ground. Nobody scrolls
eighty-four times, so most of what is written above could be built and reached
by nothing but a caption.

So a notch is a fixed fraction of the ladder in front of it instead. The step is
taken in log scale and divided by the height of the ladder that rung actually
has, which makes the trip the same length in the hand at every rung and only
changes how far it goes. Measured on a 1600×1000 stage, from a tile's own
ceiling down to the far stop with the pan held over home:

| rung | span of the ladder, in log scale | notches at a fixed ratio | notches here |
|---|---|---|---|
| `chart` | 1.4 | 5 | 5 |
| `eye` | 14.2 | 48 | 32 |
| `glass` | 25.0 | 84 | 35 |
| `space` | 25.0 | 84 | 35 |

Truncating the sky did not change the hand. The same measurement on the version
with the galaxy in it gave `space` a span of 33.9 and 113 notches at a fixed
ratio — and **36** geared, one more than now. The gearing had already absorbed
the galaxy, which is why dropping it cost nothing at the wheel and bought back
seven thousand times less empty sky to cross.

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
those are different problems. Thirty-five notches is a reachable distance and a
tiring one; worse, every one of those notches has to be aimed, because a zoom
out here goes where the pointer says and the pointer is over empty space for
most of the way. Coming back from the far stop to a city was a long scroll that
could be lost at any point in it.

So the sky carries its own way about, on a bar that stands over the zoom
buttons — the same control, in the same place — and is only there while there
is a sky to cross. Home wholly on the stage with room around it puts it up; home
filling the frame takes it down again. Three things on it, in the order a hand
reaches for them:

**The bar reads outward, in one order:** `Earth`, `Moon`, `Mars`, `Solar
system`, `Voyage`, `Exoplanet`. The divider in the middle of it is no longer the
difference between a world you land on and a shot you frame — it falls where the
sky stops being somewhere anyone has stood and becomes something looked at from
here, which is also where the space programme's own reach runs out. So the
destination sits with the far pictures rather than beside the Moon, which is
where it belongs on every reading except the implementation's.

**The places.** Home, the Moon, Mars, and the world the expedition is aimed at.
Those four and no others, because those four are already what the sky calls an
arrival: they are the three destinations of the space programme and the one
board they are launched from, and the gearing has handed itself back over
exactly them since there was a ladder at all. The bar names what the zoom
already knew. Each is offered only once this civilization can see it, off the
same roster everything else out here is drawn from, so the bar can never point
at something the sky is withholding.

The destination's button says **Exoplanet** and not the world's catalogue name.
Which world it is depends on what that civilization surveyed, so the same button
would read `Teegarden's Star b` in one game and `82 Eridani d` in the next, and
a control that renames itself between games is one nobody learns the position
of. The real name is a fact and it is kept in the three places a fact belongs:
the button's tooltip, the caption under the world, and the scene caption that
comes up on arrival with the distance and the catch.

A press lands with that world four fifths of the frame across — short of the
ceiling the zoom itself has for it, deliberately, because a jump that lands
exactly on the stop leaves the zoom-in button dead in the hand on arrival.

**The shots.** `Solar system` and `Voyage`: one press each. They are the frames
the zoom already stops at, taken out of `skySystemFrame` so that a named shot and
the far stop are the same arithmetic rather than two opinions that could drift
apart. At the `eye` rung `Solar system` *is* the far stop, because the naked
eye's universe ends at Saturn.

`Voyage` is **the view the race is watched from**. The expedition leaves home and
crawls outward across the rest of the game, and this is the one shot that holds
where it started, where it is going, and everything it has to cross. Its reach
*is* the stop — the shot decides where the camera leans, never how far it pulls
back, because a stop that quietly opened to hold whatever the expedition happened
to be aimed at would not be a stop.

It is centred **a third of the way along the trip**, not half. The two ends are
not worth the same amount of frame: what is being tracked spends almost all of
its life at the near end of the route, and the far end is a world that is not
going anywhere. On the midpoint, half the stage went to the empty side of a dot.
A third along leaves the Sun the better placed of the two with the road ahead of
it opened up — measured, the Sun sits exactly half as far from the middle of the
frame as the destination does.

How much of a trip that holds is the projection's business. The sky is drawn in
the plane of the galaxy with a star's height out of it kept separately, so what
is across the page is `ly · cos b`: LHS 1140 is 48.9 light-years off and
seventy-one degrees out of the plane, which is **15.7 across**. The Sun is on the
stage for all ten destinations; the far end is for seven of them, and on the
three widest — Gliese 667 Cc, TRAPPIST-1 e and Gliese 12 b, at 23 to 34 across —
the offset caps, the road stays in frame, and `Exoplanet` flies the rest.

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
move: at the far stop the nearest world is some red dwarf, its ceiling is a
hundredth of the way up, and dragging the handle to the ground put the camera
there and left it.

### Known, and not fixed here: the middle of the road is empty

`skyLadderPan` anchors the pan at `skyOutermostFrame()`'s centre and only starts
moving toward the subject once that subject is 2% of the stage across. Every rung
below that is therefore centred wherever the far stop is centred, which is a long
way from anything — so most of the handle's travel draws almost nothing. Walking
it in tenths and counting lit pixels on the canvas, from the far stop to the
ground: **0.2–0.4%** of the canvas at every step between 0.1 and 0.8, then 100%
at 0.9.

It is not new and it is not the truncation: the same walk on the version with the
galaxy in it gives the same 0.2–0.4%, and one rung — 0.9, at 76,452 km across —
that draws **nothing at all**. Twenty light-years is an improvement by accident
rather than by design, because the far stop's centre moved from 13,350
light-years off the Sun to 7.8.

The fix is to stop keying the pan on the subject's drawn size, which says nothing
about whether the subject is *on screen*, and cap the camera's distance from the
subject at a fraction of the stage instead — `t = max(ease, 1 - span·0.35/away)`,
which converges as fast as the stage shrinks. That changes `skyLadderPan`'s
contract and belongs in its own change.

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
it is unwatchable across eleven orders of magnitude: the trip from a tile to
the destination spends its entire middle at a tile's zoom over empty space with
nothing on the stage at all. Doing the pan first and the zoom after is a smear.

Van Wijk and Nuij's construction is the answer, and it is not a curve anybody
drew — given both ends it is the *shortest* path under a metric that counts how
much the picture moves in the eye, and what falls out is what anyone would do by
hand: pull back until both ends are in view, cross at that height, come back
down. It is written entirely in `w`, how wide the stage is in Earth radii,
because that is the one number a zoom and a pan both change.

Two things it had to be told. Its `ln(-b + sqrt(b² + 1))` has to be written as
`-asinh(b)`: a camera crossing this many orders of magnitude makes `b` as large
as 10¹⁵, at which point `sqrt(b² + 1)` is exactly `b` in float64, the
subtraction cancels to zero, the logarithm is negative infinity and the whole
flight comes out `NaN` with the camera never moving. And its own answer for how
long the trip takes — a constant speed along the path — is twenty-five seconds
for the whole sky, which is right for a film and wrong for a control. It keeps
his pace for a short hop and is capped after that — at 5,100 ms, three times as
long as it first shipped at. The distances out here are the content, and at the
first pace the crossing between two of these places was over before it had said
anything about what lay between them, which throws away the whole reason for
flying a path rather than cutting. `SKY_TRAVEL_PACE` multiplies both ends of the
clamp, so every switch is 3× longer in both directions:

| switch | first shipped | now |
|---|---|---|
| Earth ↔ Moon | 1,533 ms | **4,599 ms** |
| Earth ↔ Mars, Solar system, Voyage, Exoplanet | 1,700 ms | **5,100 ms** |
| the floor | 420 ms | **1,260 ms** |

Even so it is a fifth of Van Wijk's own answer for a trip across the whole sky,
which is twenty-five seconds — this is still a control and not a film.

The zoom buttons themselves now repeat while held, which on the flat board is a
convenience and out here is the difference between a control and an ornament.

## Working on it

`web/index.html`, in the block between `the sky beyond` and `the flat board's
sky`. The camera, its stops and what a drag means are unchanged from the
[globe rules](../web/index.html); what changed is that `skyBodyPos` now reads a
real arrangement and `planetMinScale` frames the outermost thing the current
rung knows about rather than a bounding box over three bodies.

Measured on a 1600×1000 stage: the neighbourhood and the system draw in about
**2.5 ms** each and a tiled world close up in **0.8 ms** — against roughly 55 ms
for the surface map, so nothing out here is near a frame budget. The shot that
cost the most, the whole galaxy at 8 ms, is gone.
