//! A curated catalogue of historical battles for the Tactics mode.
//!
//! The catalogue is deliberately data-first.  The browser uses it to build
//! its folding scenario picker, the setup endpoint publishes the same rows,
//! and the game/map generator use the order of battle and terrain tags when a
//! named battle is launched.  That keeps a battle's title, date, sides, map
//! promise, and pieces from drifting apart between the briefing and the
//! board.

use serde::Serialize;

use crate::setup::MapScript;

/// One side of a historical order of battle.
///
/// The pieces are the engine's, and one of them stands for a whole wing: a
/// `hoplite` is the Athenian line, not eight men. What the list is answerable
/// for is the *composition* — which arms a commander actually had, and in what
/// proportion — because that is what a reader takes away from the briefing.
/// Five were corrected when the fields were drawn (2026-08-16), each because
/// the arm it named belonged to the other side or to another century:
///
/// - **Marathon** listed Persian cavalry. It was not on the field; the ancient
///   sources' "the cavalry are away" is the standard explanation for why
///   Miltiades attacked on that morning at all.
/// - **Gaugamela** gave Alexander a scythed chariot. The chariots were Darius'
///   — the ground was levelled for them — and Alexander's answer was the
///   sarissa phalanx, which the list now carries instead.
/// - **Hastings** gave Harold crossbowmen. The missile arm at Senlac was
///   William's; the English fought as a shieldwall of housecarls and fyrd,
///   which is exactly why the feigned retreats mattered.
/// - **Agincourt** gave Henry a mounted knight and two archers. His army was
///   five or six archers to every man-at-arms and every man-at-arms fought on
///   foot.
/// - **Kadesh** gave both sides the same chariot. The Egyptian machine was a
///   light two-man archery platform and the Hittite a heavier three-man shock
///   vehicle, and that difference is the battle.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct HistoricalForce {
    pub label: &'static str,
    pub commander: &'static str,
    pub units: &'static [&'static str],
}

/// One playable historical engagement.
///
/// `civs` are the closest ruleset identities used for the side banners.  A
/// few modern and civil-war engagements intentionally use the same banner or
/// an adjacent ruleset identity: the force label and commander remain the
/// historical fact, while the engine still gets a real, supported leader
/// roster and unique-unit vocabulary.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct HistoricalScenario {
    pub id: &'static str,
    pub name: &'static str,
    pub date: &'static str,
    pub era: &'static str,
    pub era_index: usize,
    pub location: &'static str,
    /// One of land, land_water, water, water_air, land_air, or
    /// land_water_air.  These are the six terrain lenses in the lobby.
    pub terrain: &'static str,
    pub map: &'static str,
    pub objective: &'static str,
    pub summary: &'static str,
    pub civs: [&'static str; 2],
    pub forces: [HistoricalForce; 2],
    pub turns: u32,
    pub width: i32,
    pub height: i32,
    /// The random-disaster classes this battle is actually remembered for —
    /// empty for nearly every engagement, because a battlefield is decided by
    /// the fighting and the arena runs no random disasters at all (see
    /// `Game::script_disaster_allowed`). A class earns its place here only
    /// when the weather was significant to the real battle: the thirst at
    /// Hattin, the storms that broke the Armada, the winter at Stalingrad.
    ///
    /// Considered and deliberately left empty: Agincourt's and Waterloo's
    /// rain-mud (the engine has no rain class, and a river flood on those
    /// fields would be invention); Cannae's Volturnus dust (a wind in Livy's
    /// telling, not a storm, and its weight is debated); Trafalgar's great
    /// gale (it wrecked the prizes *after* the action was decided); Midway's
    /// squalls (concealment, not disaster); Inchon's Typhoon Kezia (it
    /// brushed the approach convoy, not the landing).
    pub disasters: &'static [&'static str],
}

/// The twenty-four headline engagements: three for every historical Civ VI
/// era.  Future has no historical battles yet and is intentionally represented
/// by an empty branch in the browser rather than by invented history.
pub const SCENARIOS: [HistoricalScenario; 24] = [
    HistoricalScenario {
        id: "kadesh",
        name: "Kadesh",
        date: "1274 BCE",
        era: "Ancient",
        era_index: 0,
        location: "Orontes river valley",
        terrain: "land_water",
        map: "River valley · chariot lanes and a defended crossing",
        objective: "Break the Hittite counterattack before it reaches the Egyptian camp.",
        summary: "Ramesses II's expeditionary army is surprised beside the Orontes while Muwatalli II's chariots sweep in from the north. Hold the crossing, then turn the ambush into a withdrawal.",
        civs: ["Egypt", "Gaul"],
        forces: [
            HistoricalForce { label: "Egyptian field army", commander: "Ramesses II", units: &["maryannu_chariot_archer", "archer", "spearman", "warrior"] },
            HistoricalForce { label: "Hittite coalition", commander: "Muwatalli II", units: &["heavy_chariot", "heavy_chariot", "swordsman", "archer"] },
        ],
        turns: 34,
        width: 22,
        height: 16,
        disasters: &[],
    },
    HistoricalScenario {
        id: "marathon",
        name: "Marathon",
        date: "490 BCE",
        era: "Ancient",
        era_index: 0,
        location: "Marathon coastal plain",
        terrain: "land_water",
        map: "Coastal plain · marsh, beach, and open running ground",
        objective: "Cross the killing ground and break the Persian landing force.",
        summary: "Miltiades refuses to wait behind the Athenian wall. Hoplites must cross the open plain quickly enough to deny Persian archery its full advantage while the wings close on the center.",
        civs: ["Greece", "Persia"],
        forces: [
            HistoricalForce { label: "Athenian hoplite line", commander: "Miltiades", units: &["hoplite", "hoplite", "spearman", "archer"] },
            HistoricalForce { label: "Persian landing force", commander: "Datis", units: &["archer", "archer", "spearman", "spearman"] },
        ],
        turns: 28,
        width: 20,
        height: 14,
        disasters: &[],
    },
    HistoricalScenario {
        id: "thermopylae",
        name: "Thermopylae",
        date: "480 BCE",
        era: "Ancient",
        era_index: 0,
        location: "The Gates of Thermopylae",
        terrain: "land",
        map: "Mountain pass · a narrow road below the ridge",
        objective: "Hold the pass long enough for the allied army to withdraw.",
        summary: "Leonidas turns a bottleneck into a force multiplier. The Persian host has numbers, but only a few can fight at once—until the mountain path opens a second front.",
        civs: ["Greece", "Persia"],
        forces: [
            HistoricalForce { label: "Hellenic rearguard", commander: "Leonidas", units: &["hoplite", "hoplite", "spearman", "archer"] },
            HistoricalForce { label: "Persian expedition", commander: "Xerxes I", units: &["spearman", "archer", "horseman", "swordsman"] },
        ],
        turns: 24,
        width: 26,
        height: 14,
        disasters: &[],
    },
    HistoricalScenario {
        id: "gaugamela",
        name: "Gaugamela",
        date: "331 BCE",
        era: "Classical",
        era_index: 1,
        location: "Nineveh plain",
        terrain: "land",
        map: "Open plain · prepared lanes for cavalry and chariots",
        objective: "Punch through to the Persian center without losing the flanks.",
        summary: "Alexander pulls the Persian line apart with a feint, then drives the Companion cavalry at Darius. The wings must survive the numerical tide while the center makes the decisive charge.",
        civs: ["Macedon", "Persia"],
        forces: [
            HistoricalForce { label: "Macedonian army", commander: "Alexander the Great", units: &["horseman", "pikeman", "swordsman", "spearman", "archer"] },
            HistoricalForce { label: "Achaemenid host", commander: "Darius III", units: &["horseman", "heavy_chariot", "archer", "spearman", "swordsman"] },
        ],
        turns: 40,
        width: 24,
        height: 18,
        disasters: &[],
    },
    HistoricalScenario {
        id: "cannae",
        name: "Cannae",
        date: "216 BCE",
        era: "Classical",
        era_index: 1,
        location: "Aufidus river plain",
        terrain: "land",
        map: "Windy plain · river bend and enveloping wings",
        objective: "Complete the double envelopment before the Roman legions escape.",
        summary: "Hannibal offers a deliberately yielding center and holds his veteran African infantry on the wings. Rome's mass becomes its trap when the cavalry closes the rear.",
        // Carthage is the historical side; Phoenicia is the closest supported
        // ruleset identity with a selectable leader and true-start point.
        civs: ["Phoenicia", "Rome"],
        forces: [
            HistoricalForce { label: "Carthaginian army", commander: "Hannibal Barca", units: &["swordsman", "horseman", "horseman", "spearman", "archer"] },
            HistoricalForce { label: "Roman consular army", commander: "Varro", units: &["legion", "legion", "spearman", "archer", "horseman"] },
        ],
        turns: 38,
        width: 22,
        height: 16,
        disasters: &[],
    },
    HistoricalScenario {
        id: "actium",
        name: "Actium",
        date: "31 BCE",
        era: "Classical",
        era_index: 1,
        location: "Ambracian Gulf",
        terrain: "land_water",
        map: "Gulf and headlands · fleets fighting for the channel",
        objective: "Force a passage through the strait while keeping the shore army covered.",
        summary: "Octavian seals the gulf and waits for Antony and Cleopatra to break out. The fleet battle is decided by the channel, the wind, and the moment the flagship squadron commits.",
        civs: ["Rome", "Egypt"],
        forces: [
            HistoricalForce { label: "Octavian's fleet", commander: "Agrippa", units: &["galley", "galley", "quadrireme", "archer"] },
            HistoricalForce { label: "Antonian fleet", commander: "Mark Antony", units: &["quadrireme", "quadrireme", "galley", "archer"] },
        ],
        turns: 36,
        width: 24,
        height: 18,
        disasters: &[],
    },
    HistoricalScenario {
        id: "hastings",
        name: "Hastings",
        date: "1066",
        era: "Medieval",
        era_index: 2,
        location: "Senlac Hill",
        terrain: "land",
        map: "Ridge and open slope · shieldwall frontage",
        objective: "Break the English shieldwall without letting the pursuit destroy the army.",
        summary: "Harold's housecarls hold the ridge while William alternates pressure with feigned retreats. A single undisciplined pursuit can open the gate the Norman cavalry needs.",
        civs: ["England", "France"],
        forces: [
            HistoricalForce { label: "English shieldwall", commander: "Harold Godwinson", units: &["man_at_arms", "man_at_arms", "spearman", "spearman"] },
            HistoricalForce { label: "Norman host", commander: "William the Conqueror", units: &["knight", "knight", "crossbowman", "man_at_arms"] },
        ],
        turns: 34,
        width: 20,
        height: 16,
        disasters: &[],
    },
    HistoricalScenario {
        id: "hattin",
        name: "Hattin",
        date: "1187",
        era: "Medieval",
        era_index: 2,
        location: "Horns of Hattin",
        terrain: "land",
        map: "Dry hills · smoke, heat, and a blocked spring",
        objective: "Cut the crusader army off from water and close around the True Cross.",
        summary: "Saladin turns thirst into a weapon. The crusader host must cross broken ground toward the lake while mounted archers and disciplined reserves deny every clean escape.",
        civs: ["Arabia", "England"],
        forces: [
            HistoricalForce { label: "Ayyubid field army", commander: "Saladin", units: &["horseman", "horseman", "crossbowman", "man_at_arms"] },
            HistoricalForce { label: "Crusader host", commander: "Guy of Lusignan", units: &["knight", "knight", "man_at_arms", "crossbowman"] },
        ],
        turns: 36,
        width: 22,
        height: 16,
        // The thirst is the battle: Saladin held the springs, the Crusader
        // army marched a waterless July plateau, and it broke around a dry
        // camp at the Horns. The parched field is historical fact here.
        disasters: &["drought"],
    },
    HistoricalScenario {
        id: "agincourt",
        name: "Agincourt",
        date: "1415",
        era: "Medieval",
        era_index: 2,
        location: "Pas de Calais",
        terrain: "land",
        map: "Mudbound corridor · hedges, stakes, and two woods",
        objective: "Let the mud and stakes break the French charge, then survive the crush.",
        summary: "Henry V's exhausted army occupies the narrowest ground it can find. English longbow fire turns the French advantage in armor and numbers into a series of isolated collisions.",
        civs: ["England", "France"],
        forces: [
            HistoricalForce { label: "English army", commander: "Henry V", units: &["crossbowman", "crossbowman", "crossbowman", "man_at_arms"] },
            HistoricalForce { label: "French host", commander: "Charles d'Albret", units: &["knight", "knight", "man_at_arms", "crossbowman"] },
        ],
        turns: 35,
        width: 20,
        height: 16,
        disasters: &[],
    },
    HistoricalScenario {
        id: "constantinople_1453",
        name: "Constantinople",
        date: "1453",
        era: "Renaissance",
        era_index: 3,
        location: "Theodosian Walls",
        terrain: "land_water",
        map: "Walled city · sea walls, land walls, and the Golden Horn",
        objective: "Open a breach before the defenders can shift the reserve across the walls.",
        summary: "Mehmed II brings the new age of gunpowder to the oldest walls in Europe. The defenders have a narrow inner line, a chain across the harbor, and no depth once the breach opens.",
        civs: ["Ottomans", "Byzantium"],
        forces: [
            HistoricalForce { label: "Ottoman besieging army", commander: "Mehmed II", units: &["bombard", "man_at_arms", "knight", "crossbowman"] },
            HistoricalForce { label: "Constantinopolitan garrison", commander: "Constantine XI", units: &["crossbowman", "man_at_arms", "spearman", "bombard"] },
        ],
        turns: 42,
        width: 24,
        height: 18,
        disasters: &[],
    },
    HistoricalScenario {
        id: "lepanto",
        name: "Lepanto",
        date: "1571",
        era: "Renaissance",
        era_index: 3,
        location: "Gulf of Patras",
        terrain: "water",
        map: "Enclosed sea · three squadrons and a crowded center",
        objective: "Break the opposing center before the flank galleys can turn inward.",
        summary: "The Holy League and Ottoman fleet meet in the last great galley battle. The board is all formation: hold the line, protect the flagship, and use the wings before the center locks.",
        civs: ["Venice", "Ottomans"],
        forces: [
            HistoricalForce { label: "Holy League fleet", commander: "Don John of Austria", units: &["galley", "galley", "quadrireme", "quadrireme"] },
            HistoricalForce { label: "Ottoman fleet", commander: "Ali Pasha", units: &["galley", "galley", "quadrireme", "quadrireme"] },
        ],
        turns: 32,
        width: 24,
        height: 18,
        disasters: &[],
    },
    HistoricalScenario {
        id: "spanish_armada",
        name: "Spanish Armada",
        date: "1588",
        era: "Renaissance",
        era_index: 3,
        location: "English Channel",
        terrain: "water",
        map: "Channel sea lanes · shoals, wind, and a lee shore",
        objective: "Escort the invasion fleet through the channel while denying English fire ships.",
        summary: "The Armada's crescent tries to preserve a tight formation and reach the rendezvous in the Netherlands. English ships need distance, weather, and one opening in the line.",
        civs: ["Spain", "England"],
        forces: [
            HistoricalForce { label: "Spanish Armada", commander: "Alonso Pérez de Guzmán", units: &["frigate", "frigate", "caravel", "galley"] },
            HistoricalForce { label: "English fleet", commander: "Charles Howard", units: &["frigate", "frigate", "caravel", "galley"] },
        ],
        turns: 34,
        width: 26,
        height: 18,
        // "He blew with His winds, and they were scattered": the Atlantic
        // gales sank more of the Armada than English gunnery did. A storm on
        // this water is the campaign's own weather.
        disasters: &["hurricane"],
    },
    HistoricalScenario {
        id: "waterloo",
        name: "Waterloo",
        date: "1815",
        era: "Industrial",
        era_index: 4,
        location: "Mont-Saint-Jean ridge",
        terrain: "land",
        map: "Rolling ridge · sunken road, farms, and a final reserve",
        objective: "Hold the ridge until the Prussian arrival, or break it before dusk.",
        summary: "Napoleon must crack Wellington's reverse-slope defense before Blücher's army appears. The French attack has to synchronize artillery, infantry, cavalry, and the Imperial Guard reserve.",
        civs: ["France", "England"],
        forces: [
            HistoricalForce { label: "French Army of the North", commander: "Napoleon Bonaparte", units: &["line_infantry", "cuirassier", "field_cannon", "cavalry", "infantry"] },
            HistoricalForce { label: "Anglo-Allied army", commander: "Arthur Wellesley", units: &["line_infantry", "field_cannon", "cavalry", "infantry", "line_infantry"] },
        ],
        turns: 44,
        width: 28,
        height: 18,
        disasters: &[],
    },
    HistoricalScenario {
        id: "gettysburg",
        name: "Gettysburg",
        date: "1863",
        era: "Industrial",
        era_index: 4,
        location: "Pennsylvania crossroads",
        terrain: "land",
        map: "Ridges and wheat fields · the Round Tops anchor the flank",
        objective: "Take the high ground before the opposing army can entrench it.",
        summary: "A meeting engagement hardens around Cemetery Ridge. The attacker has room to maneuver but must pay for every slope; the defender must keep the line from being rolled at either end.",
        civs: ["America", "America"],
        forces: [
            HistoricalForce { label: "Union Army of the Potomac", commander: "George G. Meade", units: &["line_infantry", "field_cannon", "cavalry", "infantry", "field_cannon"] },
            HistoricalForce { label: "Confederate Army of Northern Virginia", commander: "Robert E. Lee", units: &["line_infantry", "field_cannon", "cavalry", "infantry", "line_infantry"] },
        ],
        turns: 46,
        width: 26,
        height: 20,
        disasters: &[],
    },
    HistoricalScenario {
        id: "trafalgar",
        name: "Trafalgar",
        date: "1805",
        era: "Industrial",
        era_index: 4,
        location: "Cape Trafalgar",
        terrain: "water",
        map: "Open sea and Andalusian lee shore · two crossing columns",
        objective: "Break the Combined Fleet's line before its van can turn back into the battle.",
        summary: "Nelson's two columns bear down on Villeneuve's crescent. The existing fixed chart preserves the unequal sixty-ship order of battle and the British approach that made this battle decisive.",
        civs: ["England", "France"],
        forces: [
            HistoricalForce { label: "Royal Navy", commander: "Horatio Nelson", units: &["frigate", "frigate", "frigate", "frigate"] },
            HistoricalForce { label: "Combined Fleet", commander: "Pierre-Charles Villeneuve", units: &["frigate", "frigate", "frigate", "frigate"] },
        ],
        turns: 40,
        width: 30,
        height: 24,
        disasters: &[],
    },
    HistoricalScenario {
        id: "stalingrad",
        name: "Stalingrad",
        date: "1942–43",
        era: "Modern",
        era_index: 5,
        location: "Volga industrial district",
        terrain: "land_air",
        map: "Ruined city · factories, river bank, and shattered blocks",
        objective: "Take the factories without opening a corridor for the encircling relief force.",
        summary: "The battle is fought in meters inside the city while the wider front decides whether the pocket survives. Close infantry combat and the Soviet counterstroke share one board.",
        civs: ["Russia", "Germany"],
        forces: [
            HistoricalForce { label: "Soviet defenders", commander: "Georgy Zhukov", units: &["infantry", "infantry", "artillery", "tank", "machine_gun"] },
            HistoricalForce { label: "German Sixth Army", commander: "Friedrich Paulus", units: &["infantry", "infantry", "artillery", "tank", "bomber"] },
        ],
        turns: 48,
        width: 28,
        height: 20,
        // The Russian winter is inseparable from the battle: the November
        // counteroffensive rolled through snow, and the pocket froze before
        // it starved. A blizzard here is the history, not a die roll.
        disasters: &["blizzard"],
    },
    HistoricalScenario {
        id: "normandy",
        name: "Normandy",
        date: "6 June 1944",
        era: "Modern",
        era_index: 5,
        location: "Normandy beaches and bocage",
        terrain: "land_water_air",
        map: "Beachhead · surf, seawalls, hedgerows, and air cover",
        objective: "Secure the beachhead and connect the separated landing sectors before the armor arrives.",
        summary: "The opening hours of D-Day ask three questions at once: can the landing craft reach shore, can infantry clear the exits, and can the defenders move reserves through the bocage under allied air power?",
        civs: ["America", "Germany"],
        forces: [
            HistoricalForce { label: "Allied landing force", commander: "Dwight D. Eisenhower", units: &["infantry", "infantry", "tank", "artillery", "fighter"] },
            HistoricalForce { label: "German coastal defense", commander: "Erwin Rommel", units: &["infantry", "infantry", "field_cannon", "tank", "fighter"] },
        ],
        turns: 46,
        width: 28,
        height: 20,
        // The invasion rode a one-day break in a Channel gale — Rommel was
        // away because of the forecast — and the 19 June storm, the worst in
        // forty years, wrecked the American Mulberry and cost the buildup
        // more than the defenders did. Channel storms belong to this battle.
        disasters: &["hurricane"],
    },
    HistoricalScenario {
        id: "midway",
        name: "Midway",
        date: "1942",
        era: "Modern",
        era_index: 5,
        location: "Central Pacific",
        terrain: "water_air",
        map: "Open ocean · carrier search arcs around the island",
        objective: "Find the enemy carriers first and preserve enough air strength for the second strike.",
        summary: "Midway is a contest of scouting, timing, and the fragile bridge between a carrier deck and an enemy flight deck. The island is an anchor; the decision lives in the air above the water.",
        civs: ["America", "Japan"],
        forces: [
            HistoricalForce { label: "United States carrier force", commander: "Chester W. Nimitz", units: &["aircraft_carrier", "destroyer", "fighter", "bomber"] },
            HistoricalForce { label: "Japanese Combined Fleet", commander: "Isoroku Yamamoto", units: &["aircraft_carrier", "destroyer", "fighter", "bomber"] },
        ],
        turns: 42,
        width: 28,
        height: 20,
        disasters: &[],
    },
    HistoricalScenario {
        id: "inchon",
        name: "Inchon",
        date: "1950",
        era: "Atomic",
        era_index: 6,
        location: "Inchon tidal harbor",
        terrain: "land_water",
        map: "Tidal estuary · seawalls, mudflats, and a city objective",
        objective: "Seize the port at high tide and cut the opposing army's supply line to Seoul.",
        summary: "MacArthur's landing turns the Korean War's front. The approach is narrow and tidal, so the first wave must hold the seawall before the road network becomes a battlefield.",
        civs: ["America", "Korea"],
        forces: [
            HistoricalForce { label: "X Corps landing force", commander: "Douglas MacArthur", units: &["infantry", "tank", "artillery", "destroyer"] },
            HistoricalForce { label: "Korean People's Army", commander: "Kim Il-sung", units: &["infantry", "tank", "artillery", "field_cannon"] },
        ],
        turns: 40,
        width: 24,
        height: 18,
        disasters: &[],
    },
    HistoricalScenario {
        id: "dien_bien_phu",
        name: "Dien Bien Phu",
        date: "1954",
        era: "Atomic",
        era_index: 6,
        location: "Muong Thanh valley",
        terrain: "land_air",
        map: "Valley fortress · surrounding hills and a threatened airstrip",
        objective: "Keep the airstrip open while the surrounding artillery positions close in.",
        summary: "A garrison that expected to control the valley finds itself overlooked by a concealed siege. Every artillery piece is a position to find, and every supply flight is a turn the perimeter must survive.",
        civs: ["France", "Vietnam"],
        forces: [
            HistoricalForce { label: "French entrenched camp", commander: "Christian de Castries", units: &["infantry", "artillery", "field_cannon", "machine_gun"] },
            HistoricalForce { label: "Viet Minh siege army", commander: "Vo Nguyen Giap", units: &["infantry", "artillery", "field_cannon", "infantry"] },
        ],
        turns: 44,
        width: 22,
        height: 18,
        // The monsoon broke over the siege's last weeks: the Nam Yum rose,
        // trenches and dugouts flooded, the airstrip drowned, and the drop
        // zones shrank. Flooding is the valley's own weather.
        disasters: &["river_flood"],
    },
    HistoricalScenario {
        id: "six_day_war",
        name: "Six-Day War",
        date: "1967",
        era: "Atomic",
        era_index: 6,
        location: "Sinai and the Levant",
        terrain: "land_air",
        map: "Desert frontier · armored corridors and airfields",
        objective: "Win the opening air battle, then turn the armored breakthrough into a collapse.",
        summary: "The campaign compresses air superiority and mobile ground warfare into a few violent days. Armor can move far, but only if the first strike keeps the sky clear.",
        civs: ["Israel", "Egypt"],
        forces: [
            HistoricalForce { label: "Israeli Defense Forces", commander: "Moshe Dayan", units: &["modern_armor", "modern_armor", "fighter", "infantry"] },
            HistoricalForce { label: "Egyptian field army", commander: "Abdel Hakim Amer", units: &["modern_armor", "infantry", "fighter", "artillery"] },
        ],
        turns: 38,
        width: 26,
        height: 18,
        disasters: &[],
    },
    HistoricalScenario {
        id: "desert_storm",
        name: "Desert Storm",
        date: "1991",
        era: "Information",
        era_index: 7,
        location: "Kuwait and southern Iraq",
        terrain: "land_air",
        map: "Open desert · armored hook under a precision-air umbrella",
        objective: "Break the prepared line and complete the armored envelopment before reserves regroup.",
        summary: "The ground campaign is short because the air campaign has already changed the geometry. Armor, artillery, and aircraft must keep the corridor moving rather than chase every position.",
        civs: ["America", "Arabia"],
        forces: [
            HistoricalForce { label: "Coalition VII Corps", commander: "Norman Schwarzkopf", units: &["modern_armor", "modern_armor", "rocket_artillery", "jet_fighter", "helicopter"] },
            HistoricalForce { label: "Iraqi Republican Guard", commander: "Saddam Hussein", units: &["modern_armor", "modern_armor", "rocket_artillery", "fighter", "infantry"] },
        ],
        turns: 40,
        width: 28,
        height: 18,
        // February 1991 was the theater's worst weather in years: shamal
        // sandstorms and rain grounded sorties and slowed the left hook.
        // Dust is this desert's own weather.
        disasters: &["dust_storm"],
    },
    HistoricalScenario {
        id: "fallujah",
        name: "Fallujah",
        date: "2004",
        era: "Information",
        era_index: 7,
        location: "Fallujah, Iraq",
        terrain: "land_air",
        map: "Dense city · alleys, compounds, and overwatch lanes",
        objective: "Clear the city block by block while preserving the breach force for the next district.",
        summary: "Urban combat makes every street a decision. The attacker needs armor and infantry to cooperate at close range; the defender wins time by turning buildings into mutually supporting strongpoints.",
        civs: ["America", "Arabia"],
        forces: [
            HistoricalForce { label: "Multi-National Force", commander: "James Mattis", units: &["infantry", "modern_armor", "machine_gun", "helicopter"] },
            HistoricalForce { label: "Insurgent defense", commander: "Abu Musab al-Zarqawi", units: &["infantry", "machine_gun", "rocket_artillery", "infantry"] },
        ],
        turns: 42,
        width: 22,
        height: 18,
        disasters: &[],
    },
    HistoricalScenario {
        id: "mosul",
        name: "Mosul",
        date: "2016–17",
        era: "Information",
        era_index: 7,
        location: "Mosul and the Tigris",
        terrain: "land_water_air",
        map: "River city · bridges, dense districts, and drone overwatch",
        objective: "Take the east bank, secure the bridges, and isolate the old city.",
        summary: "The battle combines an urban siege with a river crossing and a persistent sensor battle. The coalition's advantage is coordination; the defender's is the density and depth of the city.",
        civs: ["Arabia", "America"],
        forces: [
            HistoricalForce { label: "Iraqi Security Forces", commander: "Abadi coalition", units: &["infantry", "modern_armor", "rocket_artillery", "drone", "helicopter"] },
            HistoricalForce { label: "Islamic State defense", commander: "Mosul garrison", units: &["infantry", "machine_gun", "modern_at", "drone", "rocket_artillery"] },
        ],
        turns: 48,
        width: 26,
        height: 20,
        // The defenders timed counterattacks to the dust storms that
        // grounded coalition air cover — the weather was fought with, on the
        // record, through the whole battle.
        disasters: &["dust_storm"],
    },
];

/// The scenario rows that are not the original, separately charted Trafalgar
/// fixture.  Trafalgar already occupies the base map-script roster and its
/// map/deployment module is more exact than the generic profile below.
pub fn generic_scenarios() -> impl Iterator<Item = &'static HistoricalScenario> {
    SCENARIOS
        .iter()
        .filter(|scenario| scenario.id != "trafalgar")
}

pub fn all() -> &'static [HistoricalScenario] {
    &SCENARIOS
}

pub fn by_id(id: &str) -> Option<&'static HistoricalScenario> {
    SCENARIOS.iter().find(|scenario| scenario.id == id)
}

pub fn by_script(script: MapScript) -> Option<&'static HistoricalScenario> {
    by_id(script.id())
}

pub fn script_from_id(id: &str) -> Option<MapScript> {
    Some(match id {
        "kadesh" => MapScript::Kadesh,
        "marathon" => MapScript::Marathon,
        "thermopylae" => MapScript::Thermopylae,
        "gaugamela" => MapScript::Gaugamela,
        "cannae" => MapScript::Cannae,
        "actium" => MapScript::Actium,
        "hastings" => MapScript::Hastings,
        "hattin" => MapScript::Hattin,
        "agincourt" => MapScript::Agincourt,
        "constantinople_1453" => MapScript::Constantinople1453,
        "lepanto" => MapScript::Lepanto,
        "spanish_armada" => MapScript::SpanishArmada,
        "waterloo" => MapScript::Waterloo,
        "gettysburg" => MapScript::Gettysburg,
        "stalingrad" => MapScript::Stalingrad,
        "normandy" => MapScript::Normandy,
        "midway" => MapScript::Midway,
        "inchon" => MapScript::Inchon,
        "dien_bien_phu" => MapScript::DienBienPhu,
        "six_day_war" => MapScript::SixDayWar,
        "desert_storm" => MapScript::DesertStorm,
        "fallujah" => MapScript::Fallujah,
        "mosul" => MapScript::Mosul,
        _ => return None,
    })
}

/// The dimensions and historical row need to agree before a map can be drawn.
pub fn size(script: MapScript) -> Option<(i32, i32)> {
    by_script(script).map(|scenario| (scenario.width, scenario.height))
}

/// A scenario's ground, its water and its two anchors are drawn per battle in
/// [`crate::historical_terrain`]. They used to be generated here from a
/// coordinate hash — grassland with a scatter of hills, a few mountains at
/// `hash % 19`, and a straight river down the middle column for five battles —
/// which meant every field in this catalogue was the same field with a
/// different tint, and the `map` line above was a promise nothing kept.
/// A scenario's forces are already sorted in the intended tactical order.
pub fn force_units(script: MapScript, pid: usize) -> Option<&'static [&'static str]> {
    by_script(script).map(|scenario| scenario.forces[pid.min(1)].units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_headline_battles_cover_each_historical_era() {
        for era in [
            "Ancient",
            "Classical",
            "Medieval",
            "Renaissance",
            "Industrial",
            "Modern",
            "Atomic",
            "Information",
        ] {
            assert_eq!(
                SCENARIOS
                    .iter()
                    .filter(|scenario| scenario.era == era)
                    .count(),
                3,
                "{era}"
            );
        }
        assert!(SCENARIOS
            .iter()
            .all(|scenario| (10..=50).contains(&scenario.turns)));
    }

    #[test]
    fn every_piece_is_in_the_ruleset_vocabulary() {
        let rules = crate::rules::Rules::embedded();
        for scenario in SCENARIOS {
            for force in scenario.forces {
                for unit in force.units {
                    assert!(
                        rules.units.contains_key(*unit),
                        "{} names unknown unit {unit}",
                        scenario.id
                    );
                }
            }
        }
    }

    #[test]
    fn generic_scenarios_have_two_reachable_anchors() {
        let rules = crate::rules::Rules::embedded();
        for scenario in generic_scenarios() {
            let mut rng = crate::rng::Rng::new(42);
            let (map, _) = crate::mapgen::generate_with_script(
                &rules,
                scenario.width,
                scenario.height,
                2,
                0,
                0,
                1,
                script_from_id(scenario.id).unwrap(),
                crate::setup::MapTopology::Flat,
                crate::setup::MapPoles::Poles,
                &mut rng,
            );
            let plan = crate::historical_terrain::by_id(scenario.id).unwrap();
            let afloat = crate::historical_terrain::sides_afloat(&rules, scenario);
            assert_eq!(
                crate::historical_terrain::major_starts(&map, plan, afloat)
                    .unwrap()
                    .len(),
                2,
                "{}",
                scenario.id
            );
        }
    }

    #[test]
    fn every_generic_scenario_deploys_its_named_force() {
        for scenario in generic_scenarios() {
            let mut options = crate::game::GameOptions::new(
                2,
                scenario.width,
                scenario.height,
                2026,
                scenario.turns,
                0,
            );
            options.map_script = script_from_id(scenario.id).unwrap();
            options.start_era = scenario.era_index;
            options.barbarians = false;
            let game = crate::game::Game::new_with(options);
            for pid in 0..2 {
                assert_eq!(
                    game.player_unit_ids(pid).len(),
                    scenario.forces[pid].units.len(),
                    "{} side {} opening order of battle",
                    scenario.id,
                    pid
                );
            }
        }
    }
}
