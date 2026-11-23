#!/usr/bin/env python3
"""Draw the worlds Civilization VI's own map scripts draw, for the Rust port
to be held to.

``src/mapgen/shipped`` is a port of six shipped map scripts (Continents,
Pangaea, Small Continents, Island Plates, Inland Sea and Lakes) and of the
three utilities they share. A port is a claim, and this is the check: it loads
the game's own Lua, unmodified, from the install on this machine, runs each
script as far as ``ApplyBaseTerrain`` under Lua 5.1, and writes down the world
it drew. ``src/mapgen/shipped/tests.rs`` then requires the port to draw the
same world from the same seed, plot for plot.

Two things the game keeps inside the engine are supplied here so the scripts
can run at all:

* **The dice.** ``TerrainBuilder.GetRandomNumber`` is CIVVIS's own generator
  (``src/rng.rs``, xoshiro256++), so a seed means the same throws on both
  sides. Fixtures are therefore not worlds the game would roll from that seed;
  they are the worlds the game's scripts draw on our dice.
* **The fractal.** ``Fractal.Create``, ``CreateRifts``, ``BuildRidges`` and
  ``GetHeight`` are a second copy, in Python, of ``shipped/fractal.rs``, which
  follows Civilization V's public ``CvFractal.cpp``. The fixtures prove the
  scripts were ported faithfully. They cannot prove the fractal matches the
  Civilization VI engine, whose source nobody outside Firaxis can read.

Usage::

    python3 tools/civ6_map_script_golden.py write    # refresh the fixtures
    python3 tools/civ6_map_script_golden.py check    # fixtures still match?
    python3 tools/civ6_map_script_golden.py show Continents 74 46 7

``write`` and ``check`` cover ``FIXTURES`` below. ``show`` prints one world of
any size and seed. All three need the game installed and the ``lupa`` package
(``pip install lupa``; it bundles Lua 5.1), so this runs on a fleet Mac and
not on a runner. What a runner does hold is the fixtures themselves, and
the Rust test that reads them.

Exit status is 0 on success, 1 when ``check`` finds a fixture that no longer
matches, 2 with no install or no ``lupa``.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_env  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
GOLDEN = REPO / "src" / "mapgen" / "shipped" / "golden"

#: The shipped script behind each fixture name. Gathering Storm's copies: they
#: are the ones a game with both expansions runs.
SCRIPTS = {
    "continents": "Continents",
    "pangaea": "Pangaea",
    "small_continents": "Small_Continents",
    "island_plates": "Island_Plates",
    "inland_sea": "InlandSea",
    "lakes": "Lakes",
}

#: Loaded before the script, in this order, from these places. The scripts
#: `include` more than this, but nothing else is reached before the world is
#: handed to `ApplyBaseTerrain`.
UTILITIES = (
    "DLC/Expansion2/Maps/Utility/MapEnums.lua",
    "Base/Assets/Maps/Utility/MapUtilities.lua",
    "Base/Assets/Maps/Utility/MountainsCliffs.lua",
    "DLC/Expansion2/Maps/Utility/TerrainGenerator.lua",
)
SCRIPT_DIR = "DLC/Expansion2/Maps"

#: Width, height and seed of every committed fixture, for every script. Duel
#: twice because it is cheap and its narrow map is where the scripts' edge
#: handling shows; Small once because it is the size the games are played on.
FIXTURES = ((44, 26, 1), (44, 26, 2), (74, 46, 3))

MASK = (1 << 64) - 1


def rotl(x, k):
    return ((x << k) | (x >> (64 - k))) & MASK


class Rng:
    def __init__(self, seed):
        sm = seed

        def nxt():
            nonlocal sm
            sm = (sm + 0x9E3779B97F4A7C15) & MASK
            z = sm
            z = ((z ^ (z >> 30)) * 0xBF58476D1CE4E5B9) & MASK
            z = ((z ^ (z >> 27)) * 0x94D049BB133111EB) & MASK
            return z ^ (z >> 31)

        self.s = [nxt(), nxt(), nxt(), nxt()]
        self.draws = 0

    def next(self):
        s0, s1, s2, s3 = self.s
        result = (rotl((s0 + s3) & MASK, 23) + s0) & MASK
        t = (s1 << 17) & MASK
        s2 ^= s0
        s3 ^= s1
        s1 ^= s2
        s0 ^= s3
        s2 ^= t
        s3 = rotl(s3, 45)
        self.s = [s0, s1, s2, s3]
        self.draws += 1
        return result

    def below(self, n):
        return self.next() % n


def tdiv(a, b):
    """C integer division: toward zero."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b >= 0) else -q


WRAP_X, WRAP_Y, POLAR, CENTER_RIFT, INVERT = 1, 2, 8, 0x10, 0x20
DIRS = [(0, 1, -1), (1, 0, -1), (1, -1, 0), (0, -1, 1), (-1, 0, 1), (-1, 1, 0)]


def hexspace_x(x, y):
    return x - ((y >> 1) if y >= 0 else tdiv(y - 1, 2))


def hex_distance(dx, dy):
    if (dx >= 0) == (dy >= 0):
        return abs(dx) + abs(dy)
    return max(abs(dx), abs(dy))


def estimate_direction(dx, dy):
    if dx == 0 and dy == 0:
        return None
    dz = -dx - dy
    best, best_index = 0, None
    for i, d in enumerate(DIRS):
        dot = dx * d[0] + dy * d[1] + dz * d[2]
        if dot > best:
            best, best_index = dot, i
    return best_index


class Fractal:
    def __init__(self, rng, xs, ys, grain, flags, rifts, xexp, yexp):
        if xexp < 0:
            xexp = 7
        if yexp < 0:
            yexp = 6
        fx, fy = 1 << xexp, 1 << yexp
        self.fx, self.fy, self.xs, self.ys, self.flags = fx, fy, xs, ys, flags
        self.xinc = (fx * 1000) // xs
        self.yinc = (fy * 1000) // ys
        f = [[0] * (fy + 1) for _ in range(fx + 1)]
        self.f = f
        wrap_x, wrap_y, polar = bool(flags & WRAP_X), bool(flags & WRAP_Y), bool(flags & POLAR)
        min_exp = min(xexp, yexp)
        smooth = max(0, min(min_exp, min_exp - grain))
        for p in range(smooth, -1, -1):
            screen = 0
            for b in range(p + 1):
                screen |= 1 << b
            if wrap_y:
                for x in range(fx + 1):
                    f[x][fy] = f[x][0]
            elif polar:
                for x in range(fx + 1):
                    f[x][0] = 0
                    f[x][fy] = 0
            if wrap_x:
                for y in range(fy + 1):
                    f[fx][y] = f[0][y]
            elif polar:
                for y in range(fy + 1):
                    f[0][y] = 0
                    f[fx][y] = 0
            cols = (fx >> p) + (0 if wrap_x else 1)
            rows = (fy >> p) + (0 if wrap_y else 1)
            noise = 1 << (8 - smooth + p)
            half = 1 << (7 - smooth + p)
            for x in range(cols):
                for y in range(rows):
                    cx, cy = x << p, y << p
                    if p == smooth:
                        f[cx][cy] = rng.below(256)
                        continue
                    if cx & screen:
                        if cy & screen:
                            s = (f[(x - 1) << p][(y - 1) << p] + f[(x + 1) << p][(y - 1) << p]
                                 + f[(x - 1) << p][(y + 1) << p] + f[(x + 1) << p][(y + 1) << p]) >> 2
                        else:
                            s = (f[(x - 1) << p][cy] + f[(x + 1) << p][cy]) >> 1
                    elif cy & screen:
                        s = (f[cx][(y - 1) << p] + f[cx][(y + 1) << p]) >> 1
                    else:
                        continue
                    s = s + rng.below(noise) - half
                    f[cx][cy] = max(0, min(255, s))
        if rifts is not None:
            rift_x = (fx // 4) * 3
            width = 16
            for y in range(fy + 1):
                for x in range(width):
                    centre = tdiv(tdiv((rifts.f[rift_x][y] - 128) * fx, 128), 8)
                    r = self.yield_x(centre + x)
                    l = self.yield_x(centre - x)
                    f[r][y] = (f[r][y] * x) // width
                    f[l][y] = (f[l][y] * x) // width
            for y in range(fy + 1):
                f[fx][y] = f[0][y]
        if flags & INVERT:
            for x in range(fx):
                for y in range(fy):
                    f[x][y] = 255 - f[x][y]

    def yield_x(self, x):
        if x < 0:
            return x + self.fx
        if x >= self.fx:
            return x - self.fx
        return x

    def height(self, x, y):
        if x < 0 or x >= self.xs or y < 0 or y >= self.ys:
            return 0
        lx = min((self.xinc * x) // 1000, self.fx - 1)
        ly = min((self.yinc * y) // 1000, self.fy - 1)
        ex = self.xinc * x - lx * 1000
        ey = self.yinc * y - ly * 1000
        f = self.f
        s = ((1000 - ex) * (1000 - ey) * f[lx][ly] + ex * (1000 - ey) * f[lx + 1][ly]
             + (1000 - ex) * ey * f[lx][ly + 1] + ex * ey * f[lx + 1][ly + 1])
        s //= 1000000
        return max(0, min(255, s))

    def percent(self, pct):
        pct = max(0, min(100, pct))
        cells = self.fx * self.fy
        flat = [self.f[x][y] for x in range(self.fx) for y in range(self.fy)]
        lower, upper = 0, 255
        est = 255 * pct // 100
        while est != lower:
            below = sum(1 for v in flat if v < est)
            if (100 * below) // cells > pct:
                upper = est
            else:
                lower = est
            est = (upper + lower) // 2
        return est

    def ridges(self, rng, n, ridge_flags, blend_ridge, blend_fract):
        n = max(n, 3)
        seeds = []
        for _ in range(n):
            hy = rng.below(self.fy)
            hx = hexspace_x(rng.below(self.fx), hy)
            weak = max(rng.below(7) - 3, 0)
            bias = rng.below(6)
            strength = max(rng.below(8) - 4, 0)
            redraws = 0
            while redraws < 10000 and any(hex_distance(hx - o[0], hy - o[1]) < 7 for o in seeds):
                hx = rng.below(self.fx)
                hy = rng.below(self.fy)
                redraws += 1
            seeds.append((hx, hy, weak, bias, strength))
        f = self.f
        for x in range(self.fx):
            for y in range(self.fy):
                tx, ty = hexspace_x(x, y), y
                closest = nxt = 1 << 31
                for (hx, hy, weak, bias, strength) in seeds:
                    d = hex_distance(tx - hx, ty - hy)
                    if ridge_flags:
                        d += weak
                        d += rng.below(3)
                        direction = estimate_direction(hx - tx, hy - ty)
                        if direction == bias:
                            d -= strength
                        elif direction == (bias + 3) % 6:
                            d += strength
                        d = max(d, 1)
                    if d < closest:
                        nxt = closest
                        closest = d
                    elif d < nxt:
                        nxt = d
                ridge = (255 * closest) // nxt if nxt != 0 else 0
                f[x][y] = (ridge * blend_ridge + f[x][y] * blend_fract) // max(blend_ridge + blend_fract, 1)


SHIM = r'''
DirectionTypes = {DIRECTION_NORTHEAST=0, DIRECTION_EAST=1, DIRECTION_SOUTHEAST=2,
  DIRECTION_SOUTHWEST=3, DIRECTION_WEST=4, DIRECTION_NORTHWEST=5, NUM_DIRECTION_TYPES=6}
function include() end
print = function() end
function math.clamp(v, lo, hi) if v < lo then return lo elseif v > hi then return hi else return v end end
function table.fill(v, n) local t = {} for i = 1, n do t[i] = v end return t end
local ids = {TERRAIN_GRASS=0, TERRAIN_GRASS_HILLS=1, TERRAIN_GRASS_MOUNTAIN=2, TERRAIN_PLAINS=3,
  TERRAIN_PLAINS_HILLS=4, TERRAIN_PLAINS_MOUNTAIN=5, TERRAIN_DESERT=6, TERRAIN_DESERT_HILLS=7,
  TERRAIN_DESERT_MOUNTAIN=8, TERRAIN_TUNDRA=9, TERRAIN_TUNDRA_HILLS=10, TERRAIN_TUNDRA_MOUNTAIN=11,
  TERRAIN_SNOW=12, TERRAIN_SNOW_HILLS=13, TERRAIN_SNOW_MOUNTAIN=14, TERRAIN_COAST=15, TERRAIN_OCEAN=16}
local nextid = 100
function GetGameInfoIndex(tbl, name)
  if ids[name] == nil then ids[name] = nextid; nextid = nextid + 1 end
  return ids[name]
end
local plots, terrain = {}, {}
local Plot = {}
Plot.__index = Plot
function Plot:GetX() return self.x end
function Plot:GetY() return self.y end
function Plot:GetIndex() return self.i end
function Plot:GetArea() return {} end
for y = 0, H - 1 do for x = 0, W - 1 do
  local i = y * W + x
  plots[i] = setmetatable({x = x, y = y, i = i}, Plot)
  terrain[i] = 16
end end
Map = {}
function Map.GetGridSize() return W, H end
function Map.GetPlotByIndex(i) return plots[i] end
function Map.GetPlot(x, y)
  if x == nil or y == nil then return nil end
  if x < 0 or x >= W or y < 0 or y >= H then return nil end
  return plots[y * W + x]
end
function Map.GetMapSize() return 987654321 end
function Map.IsWrapX() return WRAP end
function Map.GetAdjacentPlot(x, y, d)
  local odd = y % 2
  local nx, ny
  if d == 0 then nx, ny = x + odd, y + 1
  elseif d == 1 then nx, ny = x + 1, y
  elseif d == 2 then nx, ny = x + odd, y - 1
  elseif d == 3 then nx, ny = x + odd - 1, y - 1
  elseif d == 4 then nx, ny = x - 1, y
  else nx, ny = x + odd - 1, y + 1 end
  if ny < 0 or ny >= H then return nil end
  if WRAP then nx = nx % W elseif nx < 0 or nx >= W then return nil end
  return plots[ny * W + nx]
end
TerrainBuilder = {}
function TerrainBuilder.GetRandomNumber(n, label) return py_roll(n) end
function TerrainBuilder.GetFractalFlags() if WRAP then return {FRAC_WRAP_X = true} else return {} end end
function TerrainBuilder.SetTerrainType(plot, t) terrain[plot.i] = t end
function TerrainBuilder.AnalyzeChokepoints() end
function TerrainBuilder.StampContinents() end
AreaBuilder = {}
function AreaBuilder.Recalculate() end
Areas = {}
function Areas.FindBiggestArea(water)
  local seen, biggest = {}, 0
  for start = 0, W * H - 1 do
    if terrain[start] ~= 16 and terrain[start] ~= 15 and not seen[start] then
      seen[start] = true
      local stack, size = {start}, 0
      while #stack > 0 do
        local i = table.remove(stack)
        size = size + 1
        local p = plots[i]
        for d = 0, 5 do
          local a = Map.GetAdjacentPlot(p.x, p.y, d)
          if a and terrain[a.i] ~= 16 and terrain[a.i] ~= 15 and not seen[a.i] then
            seen[a.i] = true
            stack[#stack + 1] = a.i
          end
        end
      end
      if size > biggest then biggest = size end
    end
  end
  return {GetPlotCount = function() return biggest end, GetID = function() return 1 end}
end
local function bits(flags)
  local b = 0
  if flags.FRAC_WRAP_X then b = b + 1 end
  if flags.FRAC_WRAP_Y then b = b + 2 end
  if flags.FRAC_POLAR then b = b + 8 end
  if flags.FRAC_INVERT_HEIGHTS then b = b + 32 end
  return b
end
local function wrap(id)
  return {id = id,
    GetHeight = function(self, a, b)
      if b == nil then return py_frac_percent(id, a) else return py_frac_height(id, a, b) end
    end,
    BuildRidges = function(self, n, flags, br, bf) py_frac_ridges(id, n, bits(flags), br, bf) end}
end
Fractal = {}
function Fractal.Create(w, h, grain, flags, xe, ye) return wrap(py_frac_create(w, h, grain, bits(flags), -1, xe, ye)) end
function Fractal.CreateRifts(w, h, grain, flags, rifts, xe, ye) return wrap(py_frac_create(w, h, grain, bits(flags), rifts.id, xe, ye)) end
local sizes = {{MapSizeType="MAPSIZE_DUEL", PlateValue=3}, {MapSizeType="MAPSIZE_TINY", PlateValue=3},
  {MapSizeType="MAPSIZE_SMALL", PlateValue=4}, {MapSizeType="MAPSIZE_STANDARD", PlateValue=4},
  {MapSizeType="MAPSIZE_LARGE", PlateValue=5}, {MapSizeType="MAPSIZE_HUGE", PlateValue=5}}
local function typed(known)
  local auto = 100
  return setmetatable({}, {__index = function(t, name)
    local index = known and known[name]
    if index == nil then index = auto; auto = auto + 1 end
    local row = {Index = index}
    rawset(t, name, row)
    return row
  end})
end
GameInfo = setmetatable({
  Maps = setmetatable({}, {
    __call = function() local i = 0; return function() i = i + 1; return sizes[i] end end,
    __index = function() return {Continents = 3, NumNaturalWonders = 4} end}),
  Terrains = typed(ids)},
  {__index = function(t, name) local tbl = typed(nil); rawset(t, name, tbl); return tbl end})
MapConfiguration = {GetValue = function(name) return 2 end}
-- InlandSea.lua hangs a method on this table as it loads.
FeatureGenerator = {}
'''

OVERRIDE = r'''
function ApplyBaseTerrain(plotTypes, terrainTypes, iW, iH)
  CAPTURED_PLOTS, CAPTURED_TERRAIN = plotTypes, terrainTypes
  error("STOP")
end
local ok, err = pcall(GenerateMap)
if CAPTURED_PLOTS == nil then error(err) end
'''



#: The engine's Lua takes type annotations (``local t:table = {}``), which
#: stock Lua 5.1 does not parse.
ANNOTATION = re.compile(r"(\b[A-Za-z_]\w*)\s*:\s*(table|number|string|boolean|object)\b(?=\s*[=;,)])")


def strip_annotations(source: str) -> str:
    return ANNOTATION.sub(r"\1", source)


LETTERS = {0: "g", 3: "p", 6: "d", 9: "t", 12: "s"}


def render(plots, terrain, width: int, height: int) -> str:
    """The world as text, north at the top, two characters a plot: Ocean is
    blank and Coast a dot; land is its climate's letter (grass, plains,
    desert, tundra, snow), in capitals on Hills and after a caret on a
    Mountain. ``plots`` and ``terrain`` are read by plot index, and a plot the
    script never wrote is the flat land every later pass takes it for."""
    lines = []
    for y in range(height - 1, -1, -1):
        cells = []
        for x in range(width):
            index = y * width + x
            plot = plots[index]
            kind = terrain[index]
            if plot == 3:
                cells.append(". " if kind == 15 else "  ")
            elif plot == 0:
                cells.append("^" + LETTERS[int(kind) - 2])
            else:
                letter = LETTERS[int(kind)]
                cells.append((letter.upper() if plot == 1 else letter) + " ")
        lines.append("".join(cells).rstrip())
    return "\n".join(lines) + "\n"


def draw(assets: Path, name: str, width: int, height: int, seed: int) -> str:
    """Run one shipped script and return the world it drew."""
    import lupa.lua51 as lua51

    script = SCRIPTS[name]
    rng = Rng(seed)
    fractals: list[Fractal] = []
    lua = lua51.LuaRuntime(unpack_returned_tuples=True)
    env = lua.globals()
    # Inland Sea is the one script that asks for a map with four edges.
    env.W, env.H, env.WRAP = width, height, script != "InlandSea"

    def roll(sides):
        sides = int(sides)
        return 0 if sides <= 0 else rng.below(sides)

    def create(xs, ys, grain, flags, rifts, x_exp, y_exp):
        parent = fractals[int(rifts)] if int(rifts) >= 0 else None
        fractals.append(Fractal(rng, int(xs), int(ys), int(grain), int(flags), parent, int(x_exp), int(y_exp)))
        return len(fractals) - 1

    env.py_roll = roll
    env.py_frac_create = create
    env.py_frac_height = lambda i, x, y: fractals[int(i)].height(int(x), int(y))
    env.py_frac_percent = lambda i, percent: fractals[int(i)].percent(int(percent))
    env.py_frac_ridges = lambda i, seeds, flags, ridge, fract: fractals[int(i)].ridges(
        rng, int(seeds), int(flags), int(ridge), int(fract))
    lua.execute(SHIM)
    for relative in (*UTILITIES, f"{SCRIPT_DIR}/{script}.lua"):
        source = (assets / relative).read_text(encoding="latin-1")
        lua.execute(strip_annotations(source))
    lua.execute(OVERRIDE)
    return render(env.CAPTURED_PLOTS, env.CAPTURED_TERRAIN, width, height)


def fixture_path(name: str, width: int, height: int, seed: int) -> Path:
    return GOLDEN / f"{name}_{width}x{height}_{seed}.txt"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("command", choices=("write", "check", "show"))
    parser.add_argument("script", nargs="?", help="for show: a script name, as in SCRIPTS")
    parser.add_argument("width", nargs="?", type=int, default=74)
    parser.add_argument("height", nargs="?", type=int, default=46)
    parser.add_argument("seed", nargs="?", type=int, default=1)
    parser.add_argument("--install", help="the Civilization VI install, if it is not where civ6_env looks")
    args = parser.parse_args(argv)

    try:
        assets = civ6_env.assets_dir(args.install)
        if not (assets / SCRIPT_DIR).is_dir():
            raise FileNotFoundError(f"no {SCRIPT_DIR} under {assets}")
        import lupa.lua51  # noqa: F401
    except (Exception, SystemExit) as error:  # noqa: BLE001 - any of these is "cannot run here"
        print(f"cannot run the shipped scripts here: {error}", file=sys.stderr)
        return 2

    if args.command == "show":
        names = {script.lower(): name for name, script in SCRIPTS.items()} | {name: name for name in SCRIPTS}
        name = names.get((args.script or "").lower())
        if name is None:
            print(f"show needs one of: {', '.join(SCRIPTS)}", file=sys.stderr)
            return 2
        sys.stdout.write(draw(assets, name, args.width, args.height, args.seed))
        return 0

    stale = []
    for name in SCRIPTS:
        for width, height, seed in FIXTURES:
            world = draw(assets, name, width, height, seed)
            path = fixture_path(name, width, height, seed)
            if args.command == "write":
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(world, encoding="utf-8")
                print(f"wrote {path.relative_to(REPO)}")
            elif not path.is_file() or path.read_text(encoding="utf-8") != world:
                stale.append(str(path.relative_to(REPO)))
            else:
                print(f"ok    {path.relative_to(REPO)}")
    for path in stale:
        print(f"STALE {path}: the install's script no longer draws this world", file=sys.stderr)
    return 1 if stale else 0


if __name__ == "__main__":
    sys.exit(main())
