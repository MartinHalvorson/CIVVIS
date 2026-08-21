// The map's palette and its pure tile predicates, carved out of app.js.
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
// carve out of app.js belongs in a script ahead of it, and this one was checked
// before it moved: of the 29 names declared here, ZERO reference anything
// app.js declares above them.
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
const FEATURE_ATLAS = new Image();
let FEATURE_ATLAS_READY = false;
FEATURE_ATLAS.onload = () => { FEATURE_ATLAS_READY = true; if (state) draw(); };
FEATURE_ATLAS.src = "/assets/feature-atlas.png";
const ENVIRONMENT_FEATURE_CELL = {
  floodplains:[0,0], grassland_floodplains:[0,0], plains_floodplains:[0,0],
  reef:[1,0], geothermal_fissure:[2,0], ice:[3,0],
  volcano:[0,1], volcanic_soil:[1,1], fallout:[2,1], impact_zone:[3,1]
};
const ENVIRONMENT_FEATURE_ATLAS = new Image();
let ENVIRONMENT_FEATURE_ATLAS_READY = false;
ENVIRONMENT_FEATURE_ATLAS.onload = () => { ENVIRONMENT_FEATURE_ATLAS_READY = true; if (state) draw(); };
ENVIRONMENT_FEATURE_ATLAS.src = "/assets/environment-feature-atlas.png";
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
  HIDDEN_MAP_MONSTER_ATLAS_READY = true; if (state) draw();
};
HIDDEN_MAP_MONSTER_ATLAS.src = "/assets/hidden-map-monsters.png";
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
