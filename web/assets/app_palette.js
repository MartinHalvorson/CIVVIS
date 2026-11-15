// The map's palette, its pure tile predicates and the sky's catalogue, carved
// out of app.js.
//
// ⚠⚠ THIS SCRIPT IS LOADED **BEFORE** app.js AND THAT DIRECTION IS THE WHOLE
// SAFETY ARGUMENT. Every script the page loads shares one global scope, and
// `const`/`let` have a temporal dead zone: a name read before its declaration
// runs throws, and a top-level throw in one script leaves every later `const`
// in that script uninitialised. app.js's own header records the near-miss —
// calling `initAdvancedSettings` during load "throws, and the crash cascades:
// `RULES` never leaves its TDZ, so app_setup.js's own load-time
// `syncSetupMode()` dies too and the page never registers as a viewer".
//
// Moving definitions EARLIER cannot create that. Moving them later can. So a
// carve out of app.js belongs in a script ahead of it. Image callbacks can
// fire while app.js is still downloading, so they mark their atlas ready but
// only request a redraw once the renderer has declared its game state.
//
// `tools/test_web_assets.py` holds the rest of the contract — that the page,
// the binary's `include_str!`, and the serving route all name the same scripts,
// and that no `const` is declared at the top level of two of them, which is a
// SyntaxError that kills the page rather than a shadowed variable.

const S = 36, SQ3 = Math.sqrt(3);
// Mountain ground is deliberately much darker than workable land: its broad,
// grey-brown mass reads as a near-impassable wall before the peak symbol does.
// Volcanoes keep the same terrain language, but sink another step toward fresh
// basalt so the lava can carry the only hot light in the tile.
const MOUNTAIN_TILE_COLOR = "#49453e";
const VOLCANO_TILE_COLOR = "#292421";
const TERRAIN = { grassland:"#5e9440", plains:"#ab9e4e", desert:"#d8c184",
                  tundra:"#98a089", snow:"#e8eef2", coast:"#20647e",
                  ocean:"#12305c", mountain:MOUNTAIN_TILE_COLOR, lake:"#2b7a86" };
const COASTLINE_INK = "#0b3049";
function tileGroundColor(tile, fallback = "#333") {
  return tile.feature === "volcano"
    ? VOLCANO_TILE_COLOR
    : (TERRAIN[tile.terrain] || fallback);
}
const isWater = tile => tile.terrain === "coast" || tile.terrain === "ocean" ||
  tile.terrain === "lake";
// Cliffs live on a shared shore edge.  State from an old save may carry a
// stale edge bit after terrain changes, so every renderer checks the terrain
// as well as the serialized mask before drawing it.
const isCoastalCliffEdge = (tile, neighbor) =>
  !!neighbor && isWater(tile) !== isWater(neighbor);
// The map stores a cliff bit on both tiles, but all coastline marks are
// collected from the land side.  Keeping this test land-sided means the flat
// six-direction board and the globe's pentagonal cells agree without having
// to infer a reciprocal direction for every projected edge.
const isLandCliffEdge = (tile, side) =>
  !isWater(tile) && !!tile?.cliff_edges?.[side];
// Every natural wonder names the compact strategic silhouette and colors used
// to keep it recognizable on the command map.
const NATURAL_WONDER_ART = {
  great_barrier_reef:{form:"reef",     tint:"#2f9fae"},
  crater_lake:       {form:"basin",    tint:"#2f7fa6", rim:"#6d6455"},
  pantanal:          {form:"wetland",  tint:"#4f8f6a"},
  uluru:             {form:"monolith", tint:"#b85e35"},
  yosemite:          {form:"cliffs",   tint:"#8b8780"},
  dead_sea:          {form:"basin",    tint:"#4d90a1", rim:"#ddd6bd"},
  mount_everest:     {form:"peak",     tint:"#77726a", snow:.52},
  pamukkale:         {form:"terrace",  tint:"#e6ece4"},
  torres_del_paine:  {form:"towers",   tint:"#8e8880", cap:"#e7ecef", count:3},
  eye_of_the_sahara: {form:"rings",    tint:"#bb9660"},
  zhangye_danxia:    {form:"bands",    tint:"#b4533c",
                      palette:["#b4533c", "#d9924a", "#e6c06a", "#9a5f6d"]},
  ha_long_bay:       {form:"islets",   tint:"#2b8296", rim:"#5e6f52"},
  cliffs_of_dover:   {form:"seawall",  tint:"#eff0ea"},
  giants_causeway:   {form:"columns",  tint:"#4c5551"},
  galapagos_islands: {form:"islets",   tint:"#1f7f95", rim:"#6b5f4e", cones:true},
  matterhorn:        {form:"peak",     tint:"#6f6a63", snow:.46, horn:true},
  kilimanjaro:       {form:"peak",     tint:"#6b6152", snow:.7,  broad:true},
  piopiotahi:        {form:"seawall",  tint:"#5d6b52", water:"#1f5468", inlet:true},
  ik_kil:            {form:"basin",    tint:"#1f6f7a", rim:"#5f6c48", narrow:true},
  gobustan:          {form:"bands",    tint:"#8d8272",
                      palette:["#8d8272", "#a2957f", "#6f6759"]},
  ubsunur_hollow:    {form:"wetland",  tint:"#7d8a5a"},
  mato_tipila:       {form:"towers",   tint:"#9c7550", count:1, fluted:true},
  delicate_arch:     {form:"arch",     tint:"#b9713f"},
  chocolate_hills:   {form:"domes",    tint:"#8a6a44",
                      palette:["#8a6a44", "#9c7a4e", "#7a5c3b"]},
  vesuvius:          {form:"volcano",  tint:"#5c5148"},
  lake_retba:        {form:"basin",    tint:"#d4708f", rim:"#c3b291"},
  bermuda_triangle:  {form:"vortex",   tint:"#123f5c"},
  eyjafjallajokull:  {form:"volcano",  tint:"#5f5a55", ice:true},
  fountain_of_youth: {form:"basin",    tint:"#63c6c0", rim:"#8d8a6e",
                      narrow:true, aura:"#c8fff4"},
  lysefjord:         {form:"seawall",  tint:"#77736a", water:"#27596c", inlet:true},
  paititi:           {form:"ruins",    tint:"#b0a077"},
  mount_roraima:     {form:"towers",   tint:"#6d6455", cap:"#5c7248", count:2},
  tsingy_de_bemaraha:{form:"towers",   tint:"#a9a08c", count:5, fluted:true},
  sahara_el_beyda:   {form:"domes",    tint:"#eee7d5",
                      palette:["#eee7d5", "#f5f1e3", "#ded4bc"]},
};
// A feature is a Natural Wonder if the roster above says so, or — for anything
// a later ruleset adds — if the served rules say so. The art table is checked
// first because it is available on the very first frame, before /rules lands.
function isNaturalWonder(feature) {
  if (!feature) return false;
  if (NATURAL_WONDER_ART[feature]) return true;
  return !!(RULES && RULES.features && RULES.features[feature]
            && RULES.features[feature].natural_wonder);
}
// Gathering Storm's `Features_XP2.Volcano` flag rides four features, not one:
// the generic cone plus Vesuvius, Kilimanjaro and Eyjafjallajokull. Ash
// therefore has four possible sources, and soil a volcanic Natural Wonder threw
// has to read as coming from that wonder. The roster is checked first for the
// same reason the wonder art table is: it is available on the very first frame,
// before /rules lands.
const VOLCANO_FEATURES = new Set([
  "volcano", "vesuvius", "kilimanjaro", "eyjafjallajokull",
]);
function isVolcano(feature) {
  if (!feature) return false;
  if (VOLCANO_FEATURES.has(feature)) return true;
  return !!(RULES && RULES.features && RULES.features[feature]
            && RULES.features[feature].volcano);
}
// A footprint continues only through the same named Natural Wonder. This is
// deliberately stricter than "another wonder": two different landmarks that
// happen to touch still keep the line that distinguishes them, while every
// shared edge inside Great Barrier Reef, Pantanal, and the other multi-tile
// wonders disappears.
function naturalWonderContinues(tile, neighbor) {
  return !!(tile && neighbor && isNaturalWonder(tile.feature)
            && neighbor.feature === tile.feature);
}
// The blank reaches of a player's chart are an illustrated object, not a dark
// mask. Thirty period-map sea and land tales share one transparent atlas cell
// apiece and are always painted *under* known ground, so decoration can never
// disclose a hidden coast, biome or resource. Baba Yaga, the headless horseman,
// a sleeping giant, a dragon hoard, a beanstalk castle and woodland fairies
// keep the marginalia from making every unknown place another ocean story.
const HIDDEN_MAP_MONSTER_ATLAS = new Image();
let HIDDEN_MAP_MONSTER_ATLAS_READY = false;
HIDDEN_MAP_MONSTER_ATLAS.onload = () => {
  HIDDEN_MAP_MONSTER_ATLAS_READY = true; if (typeof state !== "undefined" && state) draw();
};
// Fetched the first time a chart asks for it: 881 KB that nobody watching the AI
// game ever sees, since a known world has no blank reaches to illustrate.
function hiddenMapMonstersReady() {
  if (!HIDDEN_MAP_MONSTER_ATLAS_READY && !HIDDEN_MAP_MONSTER_ATLAS.getAttribute("src"))
    HIDDEN_MAP_MONSTER_ATLAS.src = "/assets/hidden-map-monsters.png";
  return HIDDEN_MAP_MONSTER_ATLAS_READY;
}
const HIDDEN_MAP_MONSTER_CELL = 256, HIDDEN_MAP_MONSTER_COLUMNS = 6,
      HIDDEN_MAP_MONSTER_VARIANTS = 30;
// These are quiet map notes rather than full marginal scenes. The enlarged
// scale and reach keep the drawings legible, while the 20%-tighter, variable
// keep-out envelope adds a few more tales without forming a grid or clusters.
const HIDDEN_MAP_TALE_SCALE = 1.7,
      HIDDEN_MAP_TALE_SPACING_SCALE = .8,
      HIDDEN_MAP_TALE_SIZE_MIN = 10.6 * HIDDEN_MAP_TALE_SCALE,
      HIDDEN_MAP_TALE_SIZE_RANGE = 2.1 * HIDDEN_MAP_TALE_SCALE,
      HIDDEN_MAP_TALE_REACH = S * 9 * HIDDEN_MAP_TALE_SCALE,
      HIDDEN_MAP_TALE_CANDIDATE_RATE = .01,
      HIDDEN_MAP_TALE_MIN_SEPARATION = 17,
      HIDDEN_MAP_TALE_SEPARATION_RANGE = 10;
const PARCH = "#c7b58a", PARCH_LIGHT = "#dfcea2",
      PARCH_GRID = "#8a795533", PARCH_INK = "#6d5c3a";
const FEATURE_LABEL = {
  forest:"Woods", jungle:"Rainforest", marsh:"Marsh", oasis:"Oasis",
  floodplains:"Desert Floodplains", grassland_floodplains:"Grassland Floodplains",
  plains_floodplains:"Plains Floodplains", reef:"Reef",
  geothermal_fissure:"Geothermal Fissure", ice:"Sea Ice", volcano:"Volcano",
  volcanic_soil:"Volcanic Soil", impact_zone:"Impact Zone",
  burning_forest:"Burning Woods", burnt_forest:"Burnt Woods",
  burning_jungle:"Burning Rainforest", burnt_jungle:"Burnt Rainforest",
  // Every Natural Wonder, under the name Civilization VI gives it. The tile
  // panel used to fall back to the raw id for twenty-six of them, so the map
  // called Tsingy de Bemaraha "tsingy_de_bemaraha".
  great_barrier_reef:"Great Barrier Reef", crater_lake:"Crater Lake",
  pantanal:"Pantanal", uluru:"Uluru", yosemite:"Yosemite",
  dead_sea:"Dead Sea", mount_everest:"Mount Everest", pamukkale:"Pamukkale",
  torres_del_paine:"Torres del Paine", eye_of_the_sahara:"Eye of the Sahara",
  zhangye_danxia:"Zhangye Danxia", ha_long_bay:"Hạ Long Bay",
  cliffs_of_dover:"Cliffs of Dover", giants_causeway:"Giant's Causeway",
  galapagos_islands:"Galápagos Islands", matterhorn:"Matterhorn",
  kilimanjaro:"Mount Kilimanjaro", piopiotahi:"Piopiotahi", ik_kil:"Ik-Kil",
  gobustan:"Gobustan", ubsunur_hollow:"Ubsunur Hollow",
  mato_tipila:"Mato Tipila", delicate_arch:"Delicate Arch",
  chocolate_hills:"Chocolate Hills", vesuvius:"Mount Vesuvius",
  lake_retba:"Lake Retba", bermuda_triangle:"Bermuda Triangle",
  eyjafjallajokull:"Eyjafjallajökull", fountain_of_youth:"Fountain of Youth",
  lysefjord:"Lysefjord", paititi:"Païtiti", mount_roraima:"Mount Roraima",
  tsingy_de_bemaraha:"Tsingy de Bemaraha", sahara_el_beyda:"Sahara el Beyda"
};
// Every resource has a code-native pictogram. These names describe what grows,
// grazes or lies in the ground — not the modern product somebody eventually
// makes from it. Mercury is cinnabar crystals rather than a thermometer; niter
// is a pale mineral bloom rather than dynamite; wine begins as grapes, and
// cocoa as a pod. The renderer below gives related resources a shared visual
// grammar while the per-resource descriptor keeps the whole catalogue audited.
const RESOURCE_PICTOGRAM = {
  wheat:"grain", cattle:"cattle", sheep:"sheep", stone:"stone", deer:"antler",
  fish:"fish", bananas:"bananas", copper:"copper_ore", crabs:"crab",
  maize:"maize", rice:"rice",
  horses:"horse", iron:"iron_ore", niter:"niter_crystal", coal:"coal",
  oil:"oil_seep", aluminum:"bauxite", uranium:"uraninite",
  silk:"cocoon", wine:"grapes", salt:"salt_crystal", amber:"amber",
  citrus:"citrus", cocoa:"cocoa_pod", coffee:"coffee_branch",
  cosmetics:"pigment_flower", cotton:"cotton", diamonds:"diamond",
  dyes:"dye_flower", furs:"pelt", gypsum:"gypsum_crystal", honey:"honeycomb",
  incense:"resin_branch", ivory:"tusks", jade:"jade_stone", jeans:"indigo_cloth",
  marble:"marble_block", mercury:"cinnabar", olives:"olive_branch",
  pearls:"pearl_shell", perfume:"lavender", silver:"silver_ore",
  spices:"spice_pods", sugar:"sugar_cane", tea:"tea_branch",
  tobacco:"tobacco_leaf", toys:"wooden_top", truffles:"truffles",
  turtles:"turtle", whales:"whale", antiquity_site:"amphora",
  shipwreck:"wreck"
};
// Every deposit uses the screen-relative bottom corner. The world can open at
// an unknown bearing or turn under the camera, but the resource annotation
// remains in the same easy-to-scan place inside its tile.
// A resource is terrain information, not the tile's main actor.  Keep the
// symbol materially distinct but compact enough to sit in the far lower seat,
// clear of the command token that can occupy the centre above it.  Disc, rim,
// survey dot and pictogram share this one scale so the mark remains coherent.

// ------------------------------------------------------- the sky's catalogue
//
// The three tables the sky is built from, as published figures and nothing
// else: no function is called here and nothing here is read at load except by
// app.js, which turns each row into a body (`skyMakeBody`, `skyMakeStar`). They
// are here rather than beside that code because they are data, and because the
// renderer is the script that has to stay small enough to work in.
//
// ---------------------------------------------------------------- the system
//
// `km` is the body's own radius, `au` its semi-major axis about the Sun and
// `moonKm` its semi-major axis about its parent, in kilometres; `period` is its
// year in ours. `floor` is the smallest it is ever drawn — at true scale a
// planet is a fraction of a pixel across the moment its whole orbit is in
// frame, and a solar system of nothing is not a picture of a solar system.
// `frequency` is that world's own tiling: the same hexagons-and-twelve-
// pentagons construction the Earth has, coarser on a smaller world so a tile is
// about the same size on screen whichever one you are standing over.
//
// `skyResources` is a survey annotation, not a ruleset deposit: it reuses an
// existing Civilization resource pictogram to say what has actually been found
// on a body, without creating a yield, stockpile, action, or gameplay hook.
// That keeps the sky truthful in both Future Era modes while leaving any future
// off-world economy to the Modified Future Era's own design.
//
// `earthTiles` marks a world whose tiling follows the Earth's: see
// `skyFrequency`. Its catalogue `frequency` is then only the answer for a map
// with no globe to measure against. `deposits` is what such a world is seeded
// with each game, as how common each resource is on a piece of its ground
// against the same piece of the Earth's land: see `skyDepositCounts`.
const SKY_SYSTEM_SPECS = [
  {id:"sun", name:"The Sun", km:695700, au:0, period:0, rung:"eye", floor:3.4,
   tint:"#fff2c0", dark:"#e07d10", light:true},

  // Mercury's enormous core is predominantly iron. Its surface is sulfur-rich
  // too, but sulfur is not a Civilization resource, so the honest available
  // marker is iron rather than the planet's misleadingly namesake resource.
  {id:"mercury", name:"Mercury", km:2439.7, au:0.38710, period:0.24085,
   rung:"eye", floor:2.2, tint:"#a49a90", dark:"#2b2723", frequency:8,  seed:2617,
   skyResources:["iron"]},
  {id:"venus",   name:"Venus",   km:6051.8, au:0.72333, period:0.61520,
   rung:"eye", floor:2.4, tint:"#e8dcb0", dark:"#4d4126", frequency:10, seed:3121},
  {id:"earth",   name:"Earth",   km:6371,   au:1.00000, period:1.00000,
   rung:"eye", floor:2.4, tint:"#5aa0d8", dark:"#123048"},
  {id:"moon",    name:"The Moon", km:1737.4, moonKm:384400, parent:"earth",
   period:0.074803, rung:"eye", floor:2.0, tint:"#c3c6cb", dark:"#31333a",
   frequency:9, seed:1721, earthTiles:true,
   deposits:{stone:1, aluminum:1.7, uranium:.6}},
  {id:"mars",    name:"Mars",    km:3389.5, au:1.52371, period:1.88082,
   rung:"eye", floor:2.2, tint:"#c8683c", dark:"#3d1d13", frequency:11, seed:4409,
   earthTiles:true,
   deposits:{stone:1, aluminum:.6, uranium:.25, copper:5, salt:.2, mercury:.2}},

  // Dawn found sodium-carbonate salt crusts left by brine in Occator crater.
  {id:"ceres",   name:"Ceres",   km:473,    au:2.76750, period:4.6011,
   rung:"glass", floor:1.6, tint:"#8c8378", dark:"#2a2622", frequency:6, seed:5501,
   skyResources:["salt"]},

  {id:"jupiter", name:"Jupiter", km:69911,  au:5.20440, period:11.862,
   rung:"eye", floor:3.0, tint:"#d8bd94", dark:"#5a4127", frequency:13, seed:6113,
   giant:true},
  {id:"io",       name:"Io",       km:1821.6, moonKm:421700,  parent:"jupiter",
   period:0.0048450, rung:"glass", floor:1.4, tint:"#e2d08a", dark:"#4a3a12",
   frequency:7, seed:6217},
  {id:"europa",   name:"Europa",   km:1560.8, moonKm:671034,  parent:"jupiter",
   period:0.0097225, rung:"glass", floor:1.4, tint:"#ddd4c4", dark:"#4a4438",
   frequency:7, seed:6301},
  {id:"ganymede", name:"Ganymede", km:2634.1, moonKm:1070412, parent:"jupiter",
   period:0.0195900, rung:"glass", floor:1.5, tint:"#9d9384", dark:"#332f28",
   frequency:8, seed:6373},
  // Galileo's bulk model is ice mixed with rock that includes iron and iron
  // sulfide. Iron is the matching resource already in the Civ catalogue.
  {id:"callisto", name:"Callisto", km:2410.3, moonKm:1882709, parent:"jupiter",
   period:0.0456890, rung:"glass", floor:1.5, tint:"#7c7264", dark:"#2a251f",
   frequency:8, seed:6449, skyResources:["iron"]},

  {id:"saturn",  name:"Saturn",  km:58232,  au:9.58260, period:29.457,
   rung:"eye", floor:3.0, tint:"#e6d3a4", dark:"#5e4d2c", frequency:13, seed:7219,
   giant:true, rings:[1.24, 2.27]},
  // Cassini measured methane and ethane seas: hydrocarbons, represented by
  // the existing oil resource rather than inventing a Titan-only one.
  {id:"titan",    name:"Titan",    km:2574.7, moonKm:1221870, parent:"saturn",
   period:0.0436880, rung:"glass", floor:1.5, tint:"#d9a05a", dark:"#4a3418",
   frequency:8, seed:7307, skyResources:["oil"]},

  {id:"uranus",  name:"Uranus",  km:25362,  au:19.2184, period:84.021,
   rung:"glass", floor:2.4, tint:"#b6e3e6", dark:"#1f5761", frequency:12, seed:8291,
   giant:true, rings:[1.64, 2.00]},
  {id:"neptune", name:"Neptune", km:24622,  au:30.0700, period:164.79,
   rung:"glass", floor:2.4, tint:"#7ba4dd", dark:"#1b3161", frequency:12, seed:9377,
   giant:true},
  {id:"pluto",   name:"Pluto",   km:1188.3, au:39.4820, period:247.94,
   rung:"glass", floor:1.4, tint:"#cbb9a4", dark:"#40372e", frequency:6, seed:1031},
];

// ------------------------------------------------------- the neighbourhood
//
// Every star within about fifty light-years that anybody has a name for, at its
// real distance and in its real direction. `l` and `b` are galactic longitude
// and latitude in degrees and `ly` is the distance: `l` = 0 points at the
// galactic centre, `b` = 0 is the plane of the disc. That is the frame these
// distances are published in, so storing them this way is storing what was
// measured rather than a projection of it.
//
// The roster runs out past the twenty light-years the zoom now stops at, and it
// has to: a destination is a planet *around one of these stars*, and the
// expedition can be aimed at a world forty light-years away. What the stop
// truncates is how far out the camera goes on its own, not what is out there.
//
// `mag` is absolute visual magnitude, used only to decide how brightly a star
// is drawn, and `km` is its radius where it is known. The list is dominated by
// red dwarfs because the real neighbourhood is: of the sixty-odd systems
// inside 16 light-years, over three quarters are M dwarfs and exactly four —
// Sirius, Procyon, Altair and Alpha Centauri A — are brighter than the Sun.
const SKY_STAR_SPECS = [
  {id:"alpha_cen",  name:"Alpha Centauri",   ly:4.37,  l:315.73, b:-0.68,  type:"G2V",  km:854000},
  {id:"barnard",    name:"Barnard's Star",   ly:5.96,  l:31.01,  b:14.06,  type:"M4V",  km:135000},
  {id:"wolf359",    name:"Wolf 359",         ly:7.86,  l:244.09, b:56.11,  type:"M6V",  km:112000},
  {id:"lalande",    name:"Lalande 21185",    ly:8.31,  l:185.14, b:65.39,  type:"M2V",  km:274000},
  {id:"sirius",     name:"Sirius",           ly:8.60,  l:227.23, b:-8.89,  type:"A1V",  km:1191000},
  {id:"luyten726",  name:"Luyten 726-8",     ly:8.73,  l:175.46, b:-75.65, type:"M5V",  km:98000},
  {id:"ross154",    name:"Ross 154",         ly:9.70,  l:11.30,  b:-10.28, type:"M3V",  km:167000},
  {id:"ross248",    name:"Ross 248",         ly:10.30, l:110.00, b:-16.90, type:"M5V",  km:112000},
  {id:"eps_eri",    name:"Epsilon Eridani",  ly:10.48, l:195.84, b:-48.05, type:"K2V",  km:512000},
  {id:"lac9352",    name:"Lacaille 9352",    ly:10.72, l:8.36,   b:-66.01, type:"M1V",  km:328000},
  {id:"ross128",    name:"Ross 128",         ly:11.01, l:270.06, b:59.63,  type:"M4V",  km:141000},
  {id:"ez_aqr",     name:"EZ Aquarii",       ly:11.10, l:47.13,  b:-57.05, type:"M5V",  km:97000},
  {id:"61_cyg",     name:"61 Cygni",         ly:11.40, l:82.31,  b:-5.82,  type:"K5V",  km:466000},
  {id:"procyon",    name:"Procyon",          ly:11.46, l:213.70, b:13.02,  type:"F5IV", km:1440000},
  {id:"struve2398", name:"Struve 2398",      ly:11.53, l:89.29,  b:24.22,  type:"M3V",  km:246000},
  {id:"groom34",    name:"Groombridge 34",   ly:11.62, l:116.66, b:-18.44, type:"M1V",  km:265000},
  {id:"dx_cnc",     name:"DX Cancri",        ly:11.68, l:201.24, b:32.42,  type:"M6V",  km:80000},
  {id:"eps_ind",    name:"Epsilon Indi",     ly:11.87, l:336.19, b:-48.04, type:"K5V",  km:494000},
  {id:"tau_ceti",   name:"Tau Ceti",         ly:11.91, l:173.11, b:-73.44, type:"G8V",  km:552000},
  {id:"gj1061",     name:"GJ 1061",          ly:12.04, l:251.87, b:-34.55, type:"M5V",  km:110000},
  {id:"yz_cet",     name:"YZ Ceti",          ly:12.13, l:170.03, b:-72.06, type:"M4V",  km:117000},
  {id:"luyten",     name:"Luyten's Star",    ly:12.35, l:212.30, b:9.60,   type:"M3V",  km:203000},
  {id:"teegarden",  name:"Teegarden's Star", ly:12.50, l:160.26, b:-16.99, type:"M7V",  km:87000},
  {id:"kapteyn",    name:"Kapteyn's Star",   ly:12.83, l:250.53, b:-36.05, type:"M1V",  km:203000},
  {id:"lac8760",    name:"Lacaille 8760",    ly:12.95, l:5.11,   b:-40.96, type:"M0V",  km:344000},
  {id:"kruger60",   name:"Kruger 60",        ly:13.08, l:109.61, b:7.42,   type:"M3V",  km:174000},
  {id:"ross614",    name:"Ross 614",         ly:13.35, l:212.93, b:-6.37,  type:"M4V",  km:157000},
  {id:"wolf1061",   name:"Wolf 1061",        ly:14.05, l:4.20,   b:13.85,  type:"M3V",  km:212000},
  {id:"van_maanen", name:"Van Maanen's Star",ly:14.07, l:121.90, b:-57.42, type:"DZ",   km:9000},
  {id:"gliese1",    name:"Gliese 1",         ly:14.22, l:315.05, b:-75.94, type:"M2V",  km:265000},
  {id:"tz_ari",     name:"TZ Arietis",       ly:14.60, l:161.16, b:-33.63, type:"M4V",  km:100000},
  {id:"gj1245",     name:"GJ 1245",          ly:14.81, l:71.85,  b:16.60,  type:"M5V",  km:97000},
  {id:"gliese687",  name:"Gliese 687",       ly:14.79, l:98.03,  b:31.20,  type:"M3V",  km:346000},
  {id:"gliese674",  name:"Gliese 674",       ly:14.84, l:337.06, b:-11.65, type:"M3V",  km:246000},
  {id:"gliese876",  name:"Gliese 876",       ly:15.24, l:52.86,  b:-59.58, type:"M4V",  km:250000},
  {id:"groom1618",  name:"Groombridge 1618", ly:15.85, l:175.42, b:43.15,  type:"K7V",  km:430000},
  {id:"gliese832",  name:"Gliese 832",       ly:16.20, l:353.09, b:-42.10, type:"M2V",  km:340000},
  {id:"40_eri",     name:"40 Eridani",       ly:16.33, l:200.79, b:-38.06, type:"K1V",  km:562000},
  {id:"70_oph",     name:"70 Ophiuchi",      ly:16.59, l:29.83,  b:11.35,  type:"K0V",  km:601000},
  {id:"altair",     name:"Altair",           ly:16.73, l:47.74,  b:-8.91,  type:"A7V",  km:1300000},
  {id:"sigma_dra",  name:"Sigma Draconis",   ly:18.80, l:88.60,  b:29.94,  type:"K0V",  km:539000},
  {id:"eta_cas",    name:"Eta Cassiopeiae",  ly:19.32, l:122.53, b:-5.11,  type:"F9V",  km:695000},
  {id:"36_oph",     name:"36 Ophiuchi",      ly:19.50, l:358.32, b:6.87,   type:"K2V",  km:483000},
  {id:"82_eri",     name:"82 Eridani",       ly:19.71, l:234.62, b:-52.02, type:"G8V",  km:640000},
  {id:"delta_pav",  name:"Delta Pavonis",    ly:19.92, l:329.32, b:-25.94, type:"G8IV", km:820000},
  {id:"gliese581",  name:"Gliese 581",       ly:20.42, l:354.40, b:34.00,  type:"M3V",  km:200000},
  {id:"hd219134",   name:"HD 219134",        ly:21.34, l:109.71, b:-14.29, type:"K3V",  km:539000},
  {id:"gliese667",  name:"Gliese 667",       ly:23.62, l:0.06,   b:11.31,  type:"K3V",  km:528000},
  {id:"vega",       name:"Vega",             ly:25.04, l:67.45,  b:19.24,  type:"A0V",  km:1930000},
  {id:"fomalhaut",  name:"Fomalhaut",        ly:25.13, l:20.49,  b:-64.90, type:"A3V",  km:1240000},
  {id:"chi1_ori",   name:"Chi-1 Orionis",    ly:28.26, l:188.53, b:2.10,   type:"G0V",  km:695000},
  {id:"gliese12",   name:"Gliese 12",        ly:39.70, l:79.99,  b:-31.30, type:"M4V",  km:170000},
  {id:"trappist1",  name:"TRAPPIST-1",       ly:40.66, l:69.50,  b:-47.14, type:"M8V",  km:83000},
  {id:"55_cnc",     name:"55 Cancri",        ly:41.06, l:196.40, b:31.60,  type:"K0V",  km:659000},
  {id:"ups_and",    name:"Upsilon Andromedae",ly:44.00,l:132.00, b:-19.00, type:"F8V",  km:1090000},
  {id:"lhs1140",    name:"LHS 1140",         ly:48.93, l:175.10, b:-71.30, type:"M4.5V",km:127000},
  {id:"tau_boo",    name:"Tau Bootis",       ly:51.00, l:358.30, b:73.90,  type:"F7V",  km:986000},
];

// ------------------------------------------------------------ the destination
//
// The expedition is going to a real place. These are the worlds inside fifty
// light-years that are actually argued about as targets — the temperate ones,
// around the stars named above — with the distance the trip really is and the
// thing that is really wrong with each of them.
//
// `radius` is the planet's own radius in Earths, `hz` is a plain grade for how
// habitable it is thought to be, and `note` is the catch. There is a genuine
// trade-off in this list and it is the trade-off the field actually has: the
// nearest worlds are around flare stars and tidally locked, and the calmest
// star with a temperate planet is nearly five times further away.
const SKY_TARGET_SPECS = [
  {id:"proxima_b",  name:"Proxima Centauri b", star:"alpha_cen", ly:4.24,  radius:1.03,
   hz:"marginal", tint:"#b08a6a", dark:"#3a2a1e", frequency:11, seed:4241,
   note:"Earth-mass, in the temperate band, and one stellar flare from bare rock."},
  {id:"tau_ceti_e", name:"Tau Ceti e",         star:"tau_ceti",  ly:11.91, radius:1.81,
   hz:"marginal", tint:"#c9a06a", dark:"#3d2c18", frequency:12, seed:1191,
   note:"A quiet Sun-like star, but the inner edge of the band and a debris disc."},
  {id:"gj273_b",    name:"Luyten's Star b",    star:"luyten",    ly:12.35, radius:1.51,
   hz:"good",     tint:"#7fae8e", dark:"#20402f", frequency:12, seed:1235,
   note:"A super-Earth in the band around one of the calmer red dwarfs."},
  {id:"teegarden_b",name:"Teegarden's Star b", star:"teegarden", ly:12.50, radius:1.04,
   hz:"good",     tint:"#8fb083", dark:"#26402a", frequency:11, seed:1250,
   note:"The best Earth-similarity score on the list, around a very old, very quiet star."},
  {id:"wolf1061_c", name:"Wolf 1061 c",        star:"wolf1061",  ly:14.05, radius:1.66,
   hz:"marginal", tint:"#b6a072", dark:"#39301c", frequency:12, seed:1405,
   note:"In the band, probably tidally locked, probably a thick atmosphere."},
  {id:"hd20794_d",  name:"82 Eridani d",       star:"82_eri",    ly:19.71, radius:1.55,
   hz:"good",     tint:"#79a691", dark:"#1f3d33", frequency:12, seed:1971,
   note:"A Sun-like star with a temperate super-Earth, confirmed only in 2025."},
  {id:"gj667_cc",   name:"Gliese 667 Cc",      star:"gliese667", ly:23.62, radius:1.54,
   hz:"good",     tint:"#84ab7e", dark:"#243d26", frequency:12, seed:2362,
   note:"A super-Earth in the band of a triple system: three suns in its sky."},
  {id:"gliese12_b", name:"Gliese 12 b",        star:"gliese12",  ly:39.70, radius:0.96,
   hz:"marginal", tint:"#c2a98a", dark:"#3b3226", frequency:11, seed:3970,
   note:"Earth-sized and temperate, and it may have no atmosphere left at all."},
  {id:"trappist1_e",name:"TRAPPIST-1 e",       star:"trappist1", ly:40.66, radius:0.92,
   hz:"good",     tint:"#6fa2a8", dark:"#1d3b41", frequency:11, seed:4066,
   note:"One of seven, three of them temperate: a whole system to settle, not a world."},
  {id:"lhs1140_b",  name:"LHS 1140 b",         star:"lhs1140",   ly:48.93, radius:1.73,
   hz:"best",     tint:"#4f9fb0", dark:"#123a45", frequency:14, seed:4893,
   note:"The strongest candidate anywhere: a temperate super-Earth that may be an ocean."},
];
