// The drawn battlefields. Included by `historical_terrain.rs`, which owns the
// shape language and the painter; this file is nothing but the twenty-three
// charts and the reasoning for each, so a reader checking one against a source
// has the claim and the drawing side by side.
//
// Conventions used throughout:
//   * `x` runs west (0.0) to east (1.0), `y` runs north (0.0) to south (1.0).
//     Each battle states its own orientation, because that is the one thing a
//     reader cannot infer and the easiest to draw backwards.
//   * Strokes paint in order, later over earlier, like map layers.
//   * `fronts[0]` belongs to the catalogue row's first force, `fronts[1]` to
//     its second, and each runs from the wing that decided the battle.

use Paint::*;
use Shape::*;

/// Common paints, named so a plan reads as terrain rather than as syntax.
const SEA: &[Paint] = &[Terrain("coast"), Feature(None), Hills(false)];
const DEEP: &[Paint] = &[Terrain("ocean"), Feature(None), Hills(false)];
const CRAG: &[Paint] = &[Terrain("mountain"), Feature(None), Hills(false)];
const RIDGE: &[Paint] = &[Hills(true), Feature(None)];
const WOOD: &[Paint] = &[Feature(Some("forest")), Hills(false)];
const MUD: &[Paint] = &[Feature(Some("marsh")), Hills(false)];
const OPEN: &[Paint] = &[Feature(None), Hills(false)];
const SAND: &[Paint] = &[Terrain("desert"), Feature(None)];
const TILLED: &[Paint] = &[Terrain("plains"), Feature(None), Hills(false)];

pub static PLANS: &[Plan] = &[
    // ---------------------------------------------------------------- Ancient
    //
    // Kadesh, 1274 BCE. North is up; the Orontes runs south to north through
    // the field. Ramesses' Division of Amun camped on the WEST bank; the city
    // of Kadesh stood on its tell on the EAST bank, at the confluence with the
    // el-Mukadiyah, and Muwatalli held his chariotry hidden behind the city
    // before crossing at the southern ford to fall on the Division of Re as it
    // marched up. So: the river is the field's spine, the crossings are the
    // only places the battle can happen, and the city mound is the eastern
    // anchor nobody can take on the day.
    Plan {
        id: "kadesh",
        base: "plains",
        base_hills: false,
        strokes: &[
            // The valley floor either side of the river is watered ground.
            stroke(Area { from: p(0.0, 0.0), to: p(1.0, 1.0) }, &[Terrain("plains")]),
            stroke(Band { from: p(0.30, 0.0), to: p(0.34, 1.0), reach: 0.10 },
                   &[Terrain("grassland"), Feature(Some("floodplains"))]),
            // The Orontes itself, and the two fords that are the only ways
            // over it: one north below the city, one south where Muwatalli's
            // chariots actually crossed.
            stroke(Band { from: p(0.42, -0.05), to: p(0.46, 1.05), reach: 0.035 },
                   &[Terrain("coast"), Feature(None), Hills(false)]),
            stroke(Blob { at: p(0.44, 0.28), radius: 0.05 },
                   &[Terrain("grassland"), Feature(None)]),
            stroke(Blob { at: p(0.45, 0.78), radius: 0.05 },
                   &[Terrain("grassland"), Feature(None)]),
            // The tell of Kadesh: a walled mound on the east bank that the
            // Hittite army forms up behind.
            stroke(Blob { at: p(0.60, 0.44), radius: 0.10 }, RIDGE),
            stroke(Blob { at: p(0.60, 0.44), radius: 0.05 },
                   &[Hills(true), Improvement(Some("fort"))]),
            // Dry scrub rising to the eastern hills the ambush came around.
            stroke(Beyond { from: p(1.0, 0.05), to: p(0.78, 1.0) }, &[Terrain("plains"), Hills(true)]),
            stroke(Blob { at: p(0.16, 0.30), radius: 0.07 }, &[Improvement(Some("camp"))]),
        ],
        fronts: [
            // The Egyptian camp faces east across the river toward the city.
            front(p(0.18, 0.22), p(0.18, 0.62)),
            // The Hittite chariotry comes round the city from the south-east.
            front(p(0.72, 0.72), p(0.72, 0.30)),
        ],
    },
    // Marathon, 490 BCE. The plain lies between Mount Agrieliki and the bay.
    // North is up, the sea is EAST: the Persian fleet is beached along it and
    // the Great Marsh closes the plain's northern end. The Athenians came down
    // from the sanctuary of Herakles in the south-western foothills and ran
    // across about a mile and a half of open ground — the run is the battle,
    // so the middle of this chart has to be genuinely empty.
    Plan {
        id: "marathon",
        base: "grassland",
        base_hills: false,
        strokes: &[
            stroke(Beyond { from: p(0.84, 0.0), to: p(0.88, 1.0) }, SEA),
            // The Great Marsh at the northern end, where much of the Persian
            // rout drowned.
            stroke(Blob { at: p(0.74, 0.12), radius: 0.17 }, MUD),
            stroke(Blob { at: p(0.62, 0.06), radius: 0.10 }, MUD),
            // The Charadra stream cuts the plain.
            stroke(Band { from: p(0.30, 0.30), to: p(0.82, 0.44), reach: 0.03 }, &[River]),
            // Agrieliki and Kotroni: the wooded foothills the Greek line came
            // down from and rested its flanks on.
            stroke(Beyond { from: p(0.20, 1.0), to: p(0.06, 0.0) }, &[Hills(true), Feature(Some("forest"))]),
            stroke(Blob { at: p(0.10, 0.80), radius: 0.14 }, &[Terrain("mountain"), Feature(None)]),
            stroke(Blob { at: p(0.12, 0.20), radius: 0.10 }, RIDGE),
            // The open running ground itself, kept clear on purpose.
            stroke(Area { from: p(0.26, 0.22), to: p(0.80, 0.86) }, OPEN),
            // The beach the fleet is drawn up on.
            stroke(Band { from: p(0.83, 0.20), to: p(0.83, 0.92), reach: 0.03 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
        ],
        fronts: [
            // Miltiades' line, strong on the wings and thin in the centre,
            // formed across the plain's south-western mouth.
            front(p(0.28, 0.68), p(0.28, 0.26)),
            // Datis' army with its back to the ships.
            front(p(0.76, 0.62), p(0.76, 0.26)),
        ],
    },
    // Thermopylae, 480 BCE. THE pass. East is to the right, and the road runs
    // along it: the Malian Gulf is the NORTH wall and Mount Kallidromos the
    // SOUTH wall, and between them the coast road pinches three times — the
    // West Gate, the Middle Gate where the Phocians' old wall stood and where
    // Leonidas fought, and the East Gate behind him. Xerxes came from the
    // west, so the Persians enter along the wide western approach and have to
    // funnel into a gate a few men across. The Anopaea path is the other half
    // of the battle: it leaves the western end, climbs over Kallidromos, and
    // comes down BEHIND the Greeks in the east, which is how the position
    // fell. It is drawn as a thread of passable high ground along the southern
    // edge — long, exposed and slow, but real.
    Plan {
        id: "thermopylae",
        base: "plains",
        base_hills: false,
        strokes: &[
            // The Malian Gulf holds the whole northern edge; Kallidromos the
            // whole southern one. Everything between them is the road, and
            // every gate below is the mountain reaching north toward the water.
            stroke(Beyond { from: p(0.0, 0.27), to: p(1.0, 0.27) }, SEA),
            stroke(Beyond { from: p(1.0, 0.75), to: p(0.0, 0.75) }, CRAG),
            // The three gates, west to east. The Middle Gate is the narrowest
            // — it is the one the Greeks stood in — and the two others are a
            // hex wider, against six hexes of open road between them.
            stroke(Blob { at: p(0.24, 0.73), radius: 0.10 }, CRAG),
            stroke(Blob { at: p(0.52, 0.69), radius: 0.12 }, CRAG),
            stroke(Blob { at: p(0.80, 0.73), radius: 0.10 }, CRAG),
            // The hot springs the pass is named for, on the road beside the
            // Middle Gate.
            stroke(Blob { at: p(0.47, 0.33), radius: 0.03 },
                   &[Terrain("plains"), Feature(Some("geothermal_fissure"))]),
            // The Phocian wall, rebuilt by the Greeks across the Middle Gate
            // and the position Leonidas actually held.
            stroke(Blob { at: p(0.55, 0.36), radius: 0.035 },
                   &[Terrain("plains"), Feature(None), Improvement(Some("fort"))]),
            // The Anopaea path: the goat track over Kallidromos that the
            // Malians showed Hydarnes. It leaves the western end, climbs the
            // mountain's shoulder, and comes down EAST of the wall — which is
            // how a position that could not be forced was turned. Passable
            // high ground threaded through impassable rock: long, exposed and
            // slow, but real, and it decides the battle exactly as it did.
            // Its western end has to touch the road, or the track is a
            // decoration nobody can set foot on.
            stroke(Band { from: p(0.13, 0.66), to: p(0.46, 0.95), reach: 0.022 },
                   &[Terrain("plains"), Hills(true), Feature(None)]),
            stroke(Band { from: p(0.46, 0.95), to: p(0.82, 0.88), reach: 0.022 },
                   &[Terrain("plains"), Hills(true), Feature(None)]),
            stroke(Band { from: p(0.82, 0.88), to: p(0.88, 0.50), reach: 0.022 },
                   &[Terrain("plains"), Hills(true), Feature(None)]),
        ],
        fronts: [
            // Leonidas holds the Middle Gate, facing west, the wall at his
            // back and the sea on his right.
            front(p(0.60, 0.36), p(0.60, 0.52)),
            // Xerxes' host, packed into the western approach where its numbers
            // have room and no way to use them.
            front(p(0.05, 0.40), p(0.05, 0.66)),
        ],
    },
    // -------------------------------------------------------------- Classical
    //
    // Gaugamela, 331 BCE. Darius chose this ground and had it LEVELLED so his
    // scythed chariots could run, which makes flatness the historical fact
    // this chart has to carry: the middle of the field is deliberately without
    // a hill, a wood or a stone. Alexander advanced from the south-west and
    // slid his line rightward until a gap opened in the Persian left; the
    // Bumodus stream and the low ridge behind the Persian camp are the only
    // relief on the board, and both are at the edges.
    Plan {
        id: "gaugamela",
        base: "plains",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("plains"), Feature(None), Hills(false)]),
            // The Jebel Maqlub ridge along the northern skyline, behind the
            // Persian position.
            stroke(Beyond { from: p(0.0, 0.06), to: p(1.0, 0.10) }, RIDGE),
            stroke(Blob { at: p(0.50, 0.02), radius: 0.10 }, CRAG),
            // The Bumodus, off the southern edge behind Alexander.
            stroke(Band { from: p(0.0, 0.95), to: p(1.0, 0.92), reach: 0.03 }, &[River]),
            // Scrub at the margins, so "cleared" reads as a choice rather than
            // as an empty chart.
            stroke(Blob { at: p(0.06, 0.30), radius: 0.06 }, &[Feature(Some("forest"))]),
            stroke(Blob { at: p(0.95, 0.72), radius: 0.06 }, &[Feature(Some("forest"))]),
            stroke(Blob { at: p(0.03, 0.80), radius: 0.05 }, RIDGE),
            // The levelled ground: the last stroke, so nothing survives in it.
            stroke(Area { from: p(0.12, 0.18), to: p(0.88, 0.84) }, OPEN),
        ],
        fronts: [
            // Alexander's right, where he rode with the Companions, is the
            // wing that decided it — so it fills first.
            front(p(0.30, 0.80), p(0.30, 0.34)),
            // Darius' centre and the chariot line before it.
            front(p(0.70, 0.30), p(0.70, 0.76)),
        ],
    },
    // Cannae, 216 BCE. The Aufidus (Ofanto) runs across the north of the
    // field; the ruined citadel of Cannae stands on its bluff to the east.
    // Varro drew the Roman line up with the river covering his RIGHT flank and
    // deliberately deepened it, which shortened his frontage — and a short
    // frontage between a river and open ground is precisely what let Hannibal's
    // crescent bend back and his Libyan veterans close on both sides. The
    // chart therefore has to pin the north against water and leave the south
    // open for the wings to swing.
    Plan {
        id: "cannae",
        base: "plains",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("plains"), Feature(None)]),
            // The Aufidus along the northern edge, with the flood meadow south
            // of it that the Roman right rested on. The channel is kept to a
            // thread: it is a river a line anchors against, not a sea.
            stroke(Band { from: p(0.0, 0.12), to: p(1.0, 0.18), reach: 0.022 },
                   &[Terrain("coast"), Feature(None), Hills(false)]),
            stroke(Band { from: p(0.0, 0.19), to: p(1.0, 0.25), reach: 0.03 },
                   &[Terrain("grassland"), Feature(Some("floodplains"))]),
            // The citadel bluff of Cannae, Hannibal's camp and supply.
            stroke(Blob { at: p(0.88, 0.34), radius: 0.09 }, RIDGE),
            stroke(Blob { at: p(0.90, 0.32), radius: 0.04 },
                   &[Hills(true), Improvement(Some("fort"))]),
            // The dusty plain the Volturnus wind blew across.
            stroke(Area { from: p(0.06, 0.34), to: p(0.94, 0.94) }, OPEN),
            stroke(Blob { at: p(0.30, 0.95), radius: 0.07 }, &[Hills(true)]),
        ],
        fronts: [
            // Hannibal's line, its centre bowed forward toward the Romans and
            // its Libyan veterans set back on the wings — the shape that let
            // it give ground in the middle and close on both flanks.
            // (Catalogue order: seat 0 is the Carthaginian army.)
            front(p(0.62, 0.30), p(0.66, 0.62)),
            // The Roman consular army: an unusually deep, narrow line with its
            // right flank on the Aufidus, which is what left it no room to
            // deploy out of the envelopment when it came.
            front(p(0.34, 0.30), p(0.34, 0.62)),
        ],
    },
    // Actium, 31 BCE. The mouth of the Ambracian Gulf is a strait barely a
    // kilometre across between two headlands — Actium's promontory to the
    // SOUTH, Preveza to the NORTH. Antony's fleet had to come OUT of the gulf
    // (east) through that gap; Agrippa's waited in the open Ionian Sea (west).
    // The gap is the whole battle: it is why Antony could not use his heavier
    // ships' numbers and why the fight turned on breaking out to the west.
    Plan {
        id: "actium",
        base: "coast",
        base_hills: false,
        strokes: &[
            stroke(All, SEA),
            // Open sea to the west, where Agrippa's line waited.
            stroke(Area { from: p(0.0, 0.0), to: p(0.22, 1.0) }, DEEP),
            // The two headlands that make the mouth: Preveza reaching down
            // from the north, the promontory of Actium up from the south. What
            // is between them is the whole battle — a gap a fraction of the
            // open sea's width, which is why numbers and heavier ships could
            // not be brought to bear until a fleet was through it.
            stroke(Blob { at: p(0.50, 0.06), radius: 0.24 },
                   &[Terrain("grassland"), Hills(true), Feature(None)]),
            stroke(Blob { at: p(0.50, 0.94), radius: 0.24 },
                   &[Terrain("grassland"), Hills(true), Feature(None)]),
            // Antony's camp and the temple of Apollo on the Actian shore.
            stroke(Blob { at: p(0.52, 0.90), radius: 0.05 },
                   &[Terrain("grassland"), Hills(true), Improvement(Some("fort"))]),
        ],
        fronts: [
            // Agrippa's line, standing off to the west in the open sea and
            // waiting for Antony to come out to it. (Catalogue order: seat 0
            // is Octavian's fleet.)
            front(p(0.14, 0.50), p(0.14, 0.24)),
            // Antony's squadrons in the mouth of the gulf, Cleopatra's
            // treasure squadron behind them to the east.
            front(p(0.80, 0.50), p(0.80, 0.26)),
        ],
    },
    // ---------------------------------------------------------------- Medieval
    //
    // Hastings, 14 October 1066. Harold's army held the ridge at Senlac with
    // its shieldwall along the crest; William attacked UPHILL from the south
    // all day. North is up: the English are on the ridge across the middle of
    // the chart, the Normans on the lower ground below it, the Andredsweald
    // forest closes the English rear, and the boggy stream at the foot of the
    // slope is the ground that broke up the Norman cavalry's charges. The
    // shieldwall's whole virtue is that its flanks cannot be reached, so the
    // ridge runs off both edges of the field.
    Plan {
        id: "hastings",
        base: "grassland",
        base_hills: false,
        strokes: &[
            // The ridge, and the steeper ground on the two spurs that made the
            // English flanks unturnable.
            stroke(Band { from: p(0.0, 0.34), to: p(1.0, 0.34), reach: 0.075 }, RIDGE),
            stroke(Blob { at: p(0.05, 0.34), radius: 0.10 }, &[Terrain("mountain")]),
            stroke(Blob { at: p(0.95, 0.34), radius: 0.10 }, &[Terrain("mountain")]),
            // The Andredsweald behind the English line.
            stroke(Beyond { from: p(0.0, 0.16), to: p(1.0, 0.16) }, WOOD),
            // The marshy bottom the Normans had to cross to reach the slope,
            // and the Malfosse gully on the English left.
            stroke(Band { from: p(0.10, 0.60), to: p(0.90, 0.62), reach: 0.05 }, MUD),
            stroke(Blob { at: p(0.72, 0.52), radius: 0.06 }, MUD),
            // The open slope between them: the killing ground the shieldwall
            // looked down on.
            stroke(Area { from: p(0.12, 0.40), to: p(0.88, 0.56) }, OPEN),
            stroke(Area { from: p(0.10, 0.70), to: p(0.90, 1.0) }, OPEN),
        ],
        fronts: [
            // The shieldwall along the crest. (Seat 0 is the English.)
            front(p(0.30, 0.32), p(0.72, 0.32)),
            // William's three battles drawn up on the low ground below.
            front(p(0.30, 0.86), p(0.72, 0.86)),
        ],
    },
    // Hattin, 4 July 1187. The Crusader army marched EAST from Saffuriya toward
    // Tiberias and was cut off from water on a waterless basalt plateau; it
    // camped there overnight, and in the morning Saladin's army stood between
    // it and the lake while the scrub was fired. The twin Horns are the
    // extinct crater the remnant made its last stand on. North is up, the lake
    // is east, and the point of the chart is that every drop of water on it is
    // behind the Ayyubid line.
    Plan {
        id: "hattin",
        base: "plains",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("plains"), Feature(None), Hills(false)]),
            // The dry plateau the Crusaders were caught on.
            stroke(Area { from: p(0.10, 0.10), to: p(0.72, 0.90) }, SAND),
            // The Horns of Hattin: two hills side by side, the ground the
            // last of the army was driven onto.
            stroke(Blob { at: p(0.60, 0.40), radius: 0.075 }, RIDGE),
            stroke(Blob { at: p(0.60, 0.60), radius: 0.075 }, RIDGE),
            stroke(Blob { at: p(0.60, 0.40), radius: 0.03 }, &[Terrain("mountain")]),
            stroke(Blob { at: p(0.60, 0.60), radius: 0.03 }, &[Terrain("mountain")]),
            // The Sea of Galilee beyond the eastern edge, and the springs of
            // Hattin below the Horns — the water the army never reached.
            stroke(Beyond { from: p(0.94, 0.0), to: p(0.98, 1.0) }, SEA),
            stroke(Blob { at: p(0.82, 0.50), radius: 0.05 },
                   &[Terrain("desert"), Feature(Some("oasis")), Hills(false)]),
            // The springs of Turan behind them, which they had left at dawn.
            stroke(Blob { at: p(0.06, 0.50), radius: 0.045 },
                   &[Terrain("desert"), Feature(Some("oasis")), Hills(false)]),
            stroke(Blob { at: p(0.30, 0.20), radius: 0.06 }, RIDGE),
            stroke(Blob { at: p(0.34, 0.82), radius: 0.06 }, RIDGE),
        ],
        fronts: [
            // Saladin's army, standing between the Crusaders and the lake.
            // (Seat 0 is the Ayyubid field army.)
            front(p(0.80, 0.30), p(0.80, 0.70)),
            // Guy's column, strung out on the waterless plateau.
            front(p(0.26, 0.36), p(0.26, 0.64)),
        ],
    },
    // Agincourt, 25 October 1415. A ploughed field between two woods — the
    // village of Agincourt on one side, Tramecourt on the other — after a
    // night of rain. The corridor is drawn running north–south, woods on the
    // east and west, English at its southern mouth and the French blocking the
    // road north. Two things are the battle and both are ground: the woods
    // squeezed the French frontage until their numbers could not be used, and
    // the mud drowned an advance in armour on foot. The corridor NARROWS
    // toward the English, which is why the squeeze got worse as they closed.
    Plan {
        id: "agincourt",
        base: "grassland",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("grassland"), Feature(None), Hills(false)]),
            // The two woods, converging toward the southern (English) end.
            stroke(Band { from: p(0.06, 0.0), to: p(0.28, 1.0), reach: 0.11 }, WOOD),
            stroke(Band { from: p(0.94, 0.0), to: p(0.72, 1.0), reach: 0.11 }, WOOD),
            // The ploughed field between them, sodden after the rain: the
            // heart of the corridor is mud, and the mud is the killing.
            stroke(Area { from: p(0.30, 0.28), to: p(0.70, 0.78) }, MUD),
            stroke(Band { from: p(0.50, 0.30), to: p(0.50, 0.76), reach: 0.10 }, MUD),
            // Firm ground at either mouth, where the two armies formed up.
            // Kept inside the treelines: a wider patch here would cut away the
            // very convergence that squeezed the French, which is the point of
            // the ground.
            stroke(Area { from: p(0.42, 0.82), to: p(0.60, 1.0) }, TILLED),
            stroke(Area { from: p(0.22, 0.0), to: p(0.78, 0.18) }, TILLED),
        ],
        fronts: [
            // Henry's line: men-at-arms in the centre with the archers and
            // their stakes on the wings, drawn across the narrow end.
            // (Seat 0 is the English army.)
            front(p(0.36, 0.86), p(0.64, 0.86)),
            // The French first battle, crowded into the wider end.
            front(p(0.34, 0.10), p(0.66, 0.10)),
        ],
    },
    // ------------------------------------------------------------ Renaissance
    //
    // Constantinople, 29 May 1453. The Theodosian Walls run north–south across
    // the peninsula, from the Golden Horn in the north to the Sea of Marmara
    // in the south, and the city is behind them to the east. The Ottoman camp
    // and the great bombards are west of the walls. The Lycus valley crosses
    // the middle of the line — the Mesoteichion, the low ground where the wall
    // could be reached by the heaviest guns and where the final assault broke
    // in. The chart's whole content is that one line and the one soft place
    // in it.
    Plan {
        id: "constantinople_1453",
        base: "grassland",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("grassland"), Feature(None), Hills(false)]),
            // The Golden Horn to the north and the Marmara to the south: the
            // walls run between two seas, so there are no flanks at all.
            stroke(Beyond { from: p(0.0, 0.14), to: p(1.0, 0.10) }, SEA),
            stroke(Beyond { from: p(1.0, 0.88), to: p(0.0, 0.92) }, SEA),
            // The ridges the walls were built along, and the Lycus valley
            // cutting through them at the middle.
            stroke(Band { from: p(0.46, 0.12), to: p(0.46, 0.88), reach: 0.05 }, RIDGE),
            stroke(Band { from: p(0.0, 0.50), to: p(1.0, 0.50), reach: 0.055 },
                   &[Hills(false), Feature(None)]),
            // The walls themselves, and the moat before them.
            stroke(Band { from: p(0.48, 0.13), to: p(0.48, 0.87), reach: 0.022 },
                   &[Terrain("grassland"), Hills(true), Feature(None), Improvement(Some("fort"))]),
            stroke(Band { from: p(0.42, 0.13), to: p(0.42, 0.87), reach: 0.02 }, MUD),
            // The city behind the walls.
            stroke(Area { from: p(0.56, 0.16), to: p(1.0, 0.84) },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
            stroke(Blob { at: p(0.84, 0.50), radius: 0.09 },
                   &[Terrain("plains"), Improvement(Some("fort")), Hills(false)]),
        ],
        fronts: [
            // The Ottoman assault columns and the bombard battery, massed
            // opposite the Mesoteichion. (Seat 0 is the besieging army.)
            front(p(0.24, 0.50), p(0.24, 0.22)),
            // The garrison, spread thin along the whole length of the wall.
            front(p(0.54, 0.50), p(0.54, 0.20)),
        ],
    },
    // Lepanto, 7 October 1571. Fought in the mouth of the Gulf of Patras, with
    // the Aetolian shore to the north and the Peloponnese to the south. The
    // Holy League came from the west in three squadrons and a reserve, the
    // Ottoman fleet from the east out of Lepanto; both lines ran north–south
    // across the gulf and anchored their wings on the two coasts, which is why
    // neither could be outflanked and why the fight became a boarding action
    // in the centre. The shoals off the Curzolaris are where the Ottoman right
    // came to grief.
    Plan {
        id: "lepanto",
        base: "coast",
        base_hills: false,
        strokes: &[
            stroke(All, SEA),
            stroke(Beyond { from: p(0.0, 0.12), to: p(1.0, 0.10) }, &[Terrain("grassland"), Hills(true)]),
            stroke(Beyond { from: p(1.0, 0.88), to: p(0.0, 0.90) }, &[Terrain("grassland"), Hills(true)]),
            // The shoal water off the northern shore, clear of the beach so
            // the wings still have a coast to anchor on.
            stroke(Blob { at: p(0.30, 0.24), radius: 0.075 }, &[Terrain("coast"), Feature(Some("reef"))]),
            stroke(Blob { at: p(0.15, 0.28), radius: 0.055 }, &[Terrain("coast"), Feature(Some("reef"))]),
            // Open, deep water down the middle of the gulf.
            stroke(Area { from: p(0.10, 0.30), to: p(0.90, 0.70) }, &[Terrain("ocean"), Feature(None)]),
        ],
        fronts: [
            // Don John's line, the galleasses pushed out in front of it.
            // (Seat 0 is the Holy League.)
            front(p(0.20, 0.50), p(0.20, 0.22)),
            // Ali Pasha's line, coming out of the gulf.
            front(p(0.82, 0.50), p(0.82, 0.22)),
        ],
    },
    // The Armada in the Channel, July–August 1588. Drawn at the running fight
    // off Gravelines: the English coast to the north, the Flemish banks — the
    // lee shore that nearly destroyed the fleet — to the south-east, and the
    // westerly wind behind the English. The Armada is to the east, holding its
    // crescent and trying to work back up-Channel; Howard's fleet has the
    // weather gage to the west. The shoals are the real enemy on this chart.
    Plan {
        id: "spanish_armada",
        base: "ocean",
        base_hills: false,
        strokes: &[
            stroke(All, DEEP),
            // The English shore along the north.
            stroke(Beyond { from: p(0.0, 0.10), to: p(1.0, 0.07) }, &[Terrain("grassland"), Hills(true)]),
            stroke(Band { from: p(0.0, 0.12), to: p(1.0, 0.09), reach: 0.03 }, SEA),
            // The Flemish coast and the banks off it: shoal water a great ship
            // driven to leeward could not come off.
            stroke(Beyond { from: p(1.0, 0.94), to: p(0.55, 0.99) }, &[Terrain("grassland"), Hills(false)]),
            stroke(Band { from: p(0.55, 0.90), to: p(1.0, 0.84), reach: 0.06 },
                   &[Terrain("coast"), Feature(Some("reef"))]),
            stroke(Blob { at: p(0.78, 0.78), radius: 0.10 }, &[Terrain("coast"), Feature(Some("reef"))]),
            // The open Channel between them.
            stroke(Area { from: p(0.05, 0.20), to: p(0.95, 0.66) }, DEEP),
        ],
        fronts: [
            // The Armada's crescent, crowded toward the banks after the
            // fireships broke its anchorage. (Seat 0 is the Armada.)
            front(p(0.74, 0.44), p(0.74, 0.70)),
            // Howard and Drake to windward.
            front(p(0.16, 0.40), p(0.16, 0.66)),
        ],
    },
    // ------------------------------------------------------------- Industrial
    //
    // Waterloo, 18 June 1815. Two low ridges face each other across a shallow
    // valley about a kilometre wide. Wellington held the northern one —
    // Mont-Saint-Jean — and kept most of his army on its REVERSE slope, out of
    // sight of the French guns, which is the single most important fact about
    // this ground. Napoleon's army stood on the southern ridge by La Belle
    // Alliance. Three walled strongpoints sit in the valley between them:
    // Hougoumont on the allied right (west), La Haye Sainte in the centre on
    // the highway, Papelotte on the left (east). The sunken Ohain road runs
    // along the allied crest. The Bois de Paris is on the eastern edge, and it
    // is where the Prussians came from.
    Plan {
        id: "waterloo",
        base: "grassland",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("grassland"), Feature(None), Hills(false)]),
            // The two ridges.
            stroke(Band { from: p(0.06, 0.30), to: p(0.94, 0.30), reach: 0.055 }, RIDGE),
            stroke(Band { from: p(0.06, 0.76), to: p(0.94, 0.76), reach: 0.05 }, RIDGE),
            // The valley of standing rye between them, and the mud it had
            // become after the night's rain.
            stroke(Area { from: p(0.05, 0.42), to: p(0.95, 0.64) }, OPEN),
            stroke(Band { from: p(0.20, 0.56), to: p(0.80, 0.58), reach: 0.035 }, MUD),
            // The three strongpoints, west to east.
            stroke(Blob { at: p(0.20, 0.50), radius: 0.045 },
                   &[Terrain("grassland"), Feature(None), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.50, 0.46), radius: 0.04 },
                   &[Terrain("grassland"), Feature(None), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.80, 0.48), radius: 0.045 },
                   &[Terrain("grassland"), Feature(None), Improvement(Some("fort"))]),
            // Hougoumont's wood and orchard, which took a French corps all day.
            stroke(Blob { at: p(0.16, 0.58), radius: 0.055 }, WOOD),
            // The Bois de Paris on the eastern flank: the Prussian approach.
            stroke(Beyond { from: p(0.92, 0.30), to: p(0.96, 1.0) }, WOOD),
        ],
        fronts: [
            // The French, drawn up on the southern ridge with the Guard behind.
            // (Seat 0 is the Army of the North.)
            front(p(0.34, 0.80), p(0.66, 0.80)),
            // Wellington's line, on and behind the northern crest.
            front(p(0.34, 0.26), p(0.66, 0.26)),
        ],
    },
    // Gettysburg, 1–3 July 1863, drawn at the second and third days. The Union
    // line is the famous fishhook: Culp's Hill and Cemetery Hill make the barb
    // in the NORTH, Cemetery Ridge runs SOUTH from it, and Little Round Top
    // and Big Round Top anchor its southern end. Lee's army faces it from
    // Seminary Ridge to the WEST, a mile off and parallel. Between the ridges
    // lie the open fields Pickett's divisions had to cross, with the Peach
    // Orchard, the Wheatfield and the boulders of Devil's Den on the southern
    // half. The town itself sits at the northern end, between the two armies.
    Plan {
        id: "gettysburg",
        base: "grassland",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("grassland"), Feature(None), Hills(false)]),
            // Cemetery Ridge, running south, with the hook at its northern end.
            stroke(Band { from: p(0.66, 0.20), to: p(0.70, 0.82), reach: 0.05 }, RIDGE),
            stroke(Band { from: p(0.58, 0.14), to: p(0.74, 0.20), reach: 0.05 }, RIDGE),
            // The Round Tops: the flank anchor, and the ground the line would
            // have been rolled up from had it been lost.
            stroke(Blob { at: p(0.70, 0.86), radius: 0.055 }, &[Hills(true), Feature(Some("forest"))]),
            stroke(Blob { at: p(0.72, 0.96), radius: 0.06 }, &[Terrain("mountain"), Feature(None)]),
            // Seminary Ridge opposite, where Lee's line and its guns stood.
            stroke(Band { from: p(0.28, 0.18), to: p(0.30, 0.84), reach: 0.045 }, RIDGE),
            stroke(Band { from: p(0.26, 0.20), to: p(0.28, 0.70), reach: 0.03 }, WOOD),
            // The open fields between the ridges — Pickett's ground.
            stroke(Area { from: p(0.36, 0.26), to: p(0.60, 0.66) }, OPEN),
            // The Peach Orchard, the Wheatfield and Devil's Den, south of it.
            stroke(Blob { at: p(0.46, 0.74), radius: 0.05 }, WOOD),
            stroke(Blob { at: p(0.56, 0.80), radius: 0.05 }, OPEN),
            stroke(Blob { at: p(0.64, 0.88), radius: 0.045 }, &[Hills(true), Feature(Some("forest"))]),
            // Willoughby Run and the town at the northern end.
            stroke(Band { from: p(0.20, 0.10), to: p(0.24, 0.60), reach: 0.02 }, &[River]),
            stroke(Blob { at: p(0.50, 0.10), radius: 0.07 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
        ],
        fronts: [
            // The Army of the Potomac along the ridge, from the hook southward
            // to the Round Tops. (Seat 0 is the Union army.)
            front(p(0.70, 0.30), p(0.70, 0.80)),
            // The Army of Northern Virginia on Seminary Ridge.
            front(p(0.30, 0.30), p(0.30, 0.78)),
        ],
    },
    // ----------------------------------------------------------------- Modern
    //
    // Stalingrad, autumn 1942, in the northern factory district. The Volga runs
    // down the EASTERN edge; the 62nd Army held a strip of the west bank
    // sometimes only a few hundred metres deep, with every reinforcement and
    // every round crossing that river behind it. The three great works — the
    // Tractor Factory, Barrikady and Red October — are the northern half of
    // the map, and Mamayev Kurgan, the burial mound that dominates the whole
    // city, is its centre; it changed hands more often than anything else on
    // the field. The Sixth Army attacks from the west across open steppe.
    Plan {
        id: "stalingrad",
        base: "plains",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("plains"), Feature(None), Hills(false)]),
            // The Volga, and the sandbanks in it that the crossings used.
            stroke(Beyond { from: p(0.90, 0.0), to: p(0.94, 1.0) }, SEA),
            stroke(Blob { at: p(0.90, 0.44), radius: 0.05 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
            // Mamayev Kurgan in the centre: the height the whole battle turned
            // on, because whoever held it could see the crossings.
            stroke(Blob { at: p(0.62, 0.50), radius: 0.085 }, RIDGE),
            // The factory district: ruined works, walls and rubble that turned
            // the fight into a room-by-room one.
            stroke(Area { from: p(0.52, 0.06), to: p(0.88, 0.34) },
                   &[Terrain("plains"), Hills(true), Feature(None)]),
            stroke(Blob { at: p(0.62, 0.12), radius: 0.06 },
                   &[Terrain("plains"), Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.72, 0.20), radius: 0.06 },
                   &[Terrain("plains"), Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.80, 0.28), radius: 0.055 },
                   &[Terrain("plains"), Hills(true), Improvement(Some("fort"))]),
            // The rest of the city, west and south of the works.
            stroke(Area { from: p(0.56, 0.60), to: p(0.88, 0.94) },
                   &[Terrain("plains"), Hills(true), Feature(None)]),
            // The Tsaritsa gully cutting in from the west.
            stroke(Band { from: p(0.20, 0.66), to: p(0.86, 0.70), reach: 0.025 }, &[River]),
            // Open steppe west of the city, where the German approach lay.
            stroke(Area { from: p(0.0, 0.10), to: p(0.44, 0.90) }, OPEN),
        ],
        fronts: [
            // The 62nd Army, backed against the river bank. (Seat 0 is the
            // Soviet defenders.)
            front(p(0.84, 0.30), p(0.84, 0.72)),
            // The Sixth Army, coming in from the steppe.
            front(p(0.24, 0.30), p(0.24, 0.72)),
        ],
    },
    // Normandy, 6 June 1944, drawn as an assault beach of the Omaha kind. The
    // sea is NORTH; below it the tidal flat the landing craft grounded on, then
    // the shingle, then a seawall, and then the bluffs — high ground cut by a
    // handful of draws, which are the only vehicle exits off the beach and are
    // exactly where the defence was sited. Behind the bluffs is bocage: the
    // Norman hedgerow country, small fields walled by earth banks and thorn,
    // which fought the Allies for two months after the beach was won.
    Plan {
        id: "normandy",
        base: "grassland",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("grassland"), Feature(None), Hills(false)]),
            // The Channel, the obstacle belt, and the beach itself.
            stroke(Beyond { from: p(0.0, 0.22), to: p(1.0, 0.22) }, SEA),
            stroke(Band { from: p(0.0, 0.235), to: p(1.0, 0.235), reach: 0.022 },
                   &[Terrain("coast"), Feature(Some("reef"))]),
            stroke(Band { from: p(0.0, 0.30), to: p(1.0, 0.30), reach: 0.045 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
            // The bluffs above the beach, with cliff edges facing the sea.
            stroke(Band { from: p(0.0, 0.40), to: p(1.0, 0.40), reach: 0.05 },
                   &[Terrain("grassland"), Hills(true), Feature(None), Cliff]),
            // The draws: the few cuts through the bluff a vehicle can use.
            stroke(Band { from: p(0.22, 0.34), to: p(0.22, 0.48), reach: 0.022 }, OPEN),
            stroke(Band { from: p(0.52, 0.34), to: p(0.52, 0.48), reach: 0.022 }, OPEN),
            stroke(Band { from: p(0.80, 0.34), to: p(0.80, 0.48), reach: 0.022 }, OPEN),
            // Strongpoints sited to cover them.
            stroke(Blob { at: p(0.30, 0.40), radius: 0.035 },
                   &[Terrain("grassland"), Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.62, 0.40), radius: 0.035 },
                   &[Terrain("grassland"), Hills(true), Improvement(Some("fort"))]),
            // The bocage inland: a lattice of hedged fields.
            stroke(Area { from: p(0.0, 0.54), to: p(1.0, 1.0) }, OPEN),
            stroke(Band { from: p(0.0, 0.62), to: p(1.0, 0.62), reach: 0.022 }, WOOD),
            stroke(Band { from: p(0.0, 0.78), to: p(1.0, 0.78), reach: 0.022 }, WOOD),
            stroke(Band { from: p(0.0, 0.94), to: p(1.0, 0.94), reach: 0.022 }, WOOD),
            stroke(Band { from: p(0.16, 0.54), to: p(0.16, 1.0), reach: 0.018 }, WOOD),
            stroke(Band { from: p(0.44, 0.54), to: p(0.44, 1.0), reach: 0.018 }, WOOD),
            stroke(Band { from: p(0.72, 0.54), to: p(0.72, 1.0), reach: 0.018 }, WOOD),
            // The flooded meadows the Germans made behind the beaches.
            stroke(Blob { at: p(0.08, 0.70), radius: 0.07 }, MUD),
            stroke(Blob { at: p(0.90, 0.86), radius: 0.07 }, MUD),
        ],
        fronts: [
            // The assault waves, still in the water and on the sand.
            // (Seat 0 is the Allied landing force.)
            front(p(0.30, 0.16), p(0.70, 0.16)),
            // The coastal defence: the bluff line and the hedged villages
            // immediately behind it, which is where the guns covering the
            // draws actually sat.
            front(p(0.30, 0.52), p(0.70, 0.52)),
        ],
    },
    // Midway, 4 June 1942. Almost nothing but ocean: two carrier forces
    // hundreds of miles apart, each hunting for the other, and one small atoll
    // that is the reason both are there. The island is the only fixed point on
    // the chart — an unsinkable airfield in the south — and the fight is
    // decided by which force finds the other first. Drawn with the American
    // task forces north-east of the atoll and the Kido Butai to the
    // north-west, which is where each actually was on the morning.
    Plan {
        id: "midway",
        base: "ocean",
        base_hills: false,
        strokes: &[
            stroke(All, DEEP),
            // Midway atoll: the lagoon, the reef around it, and the two islets
            // carrying the airstrip.
            stroke(Blob { at: p(0.50, 0.82), radius: 0.115 },
                   &[Terrain("coast"), Feature(Some("reef"))]),
            stroke(Blob { at: p(0.50, 0.82), radius: 0.07 }, &[Terrain("coast"), Feature(None)]),
            stroke(Blob { at: p(0.47, 0.83), radius: 0.032 },
                   &[Terrain("plains"), Feature(None), Hills(false), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.55, 0.80), radius: 0.022 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
        ],
        fronts: [
            // Fletcher and Spruance, north-east of the atoll and upwind of the
            // Japanese search arcs. (Seat 0 is the American carrier force.)
            front(p(0.80, 0.24), p(0.92, 0.44)),
            // The Kido Butai, coming down from the north-west.
            front(p(0.18, 0.20), p(0.08, 0.42)),
        ],
    },
    // ----------------------------------------------------------------- Atomic
    //
    // Inchon, 15 September 1950. A landing with almost no beach: the approach
    // is a narrow buoyed channel from the Yellow Sea in the WEST, the harbour
    // dries to miles of mud at low tide, and the "beaches" are stone seawalls
    // the assault went over on ladders. Wolmi-do, the fortified island in the
    // middle of the harbour joined to the city by a causeway, had to be taken
    // first because it covers everything. The city of Inchon is east behind
    // the wall.
    Plan {
        id: "inchon",
        base: "coast",
        base_hills: false,
        strokes: &[
            stroke(All, SEA),
            // Open water to the west, and the tidal mud that surrounds the
            // approach: the reason the landing could only be made on two tides.
            stroke(Area { from: p(0.0, 0.0), to: p(0.20, 1.0) }, DEEP),
            stroke(Area { from: p(0.22, 0.0), to: p(0.62, 0.26) }, MUD),
            stroke(Area { from: p(0.22, 0.74), to: p(0.62, 1.0) }, MUD),
            // The buoyed channel through it, dead straight, the only way in.
            stroke(Band { from: p(0.18, 0.50), to: p(0.60, 0.50), reach: 0.06 }, SEA),
            // Wolmi-do, and its causeway to the city.
            stroke(Blob { at: p(0.56, 0.50), radius: 0.075 },
                   &[Terrain("grassland"), Hills(true), Feature(None)]),
            stroke(Blob { at: p(0.56, 0.50), radius: 0.035 },
                   &[Terrain("grassland"), Hills(true), Improvement(Some("fort"))]),
            stroke(Band { from: p(0.62, 0.50), to: p(0.74, 0.50), reach: 0.022 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
            // The seawall, and the city behind it.
            stroke(Band { from: p(0.74, 0.10), to: p(0.74, 0.90), reach: 0.022 },
                   &[Terrain("plains"), Hills(true), Feature(None), Cliff]),
            stroke(Area { from: p(0.78, 0.06), to: p(1.0, 0.94) },
                   &[Terrain("plains"), Hills(true), Feature(None)]),
            stroke(Blob { at: p(0.90, 0.50), radius: 0.08 },
                   &[Terrain("plains"), Hills(true), Improvement(Some("fort"))]),
        ],
        fronts: [
            // The landing force, still afloat in the channel. (Seat 0 is
            // X Corps.)
            front(p(0.30, 0.50), p(0.24, 0.36)),
            // The garrison, on the wall and in the streets behind it.
            front(p(0.82, 0.50), p(0.82, 0.24)),
        ],
    },
    // Dien Bien Phu, March–May 1954. A French garrison in a flat valley
    // holding an airstrip, and a Viet Minh army on the hills all around it.
    // The French assumption was that artillery could not be brought onto those
    // slopes; it was, dug into the reverse sides, and once the airstrip was
    // under observed fire the camp could not be supplied. So the chart is a
    // ring: high ground on every edge, a flat floor with the strip and the
    // strongpoints on it, and the Nam Yum river running through the middle.
    Plan {
        id: "dien_bien_phu",
        base: "grassland",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("grassland"), Feature(None), Hills(false)]),
            // The hills, closing the valley on every side.
            stroke(Beyond { from: p(0.0, 0.16), to: p(1.0, 0.16) }, &[Hills(true), Feature(Some("forest"))]),
            stroke(Beyond { from: p(1.0, 0.84), to: p(0.0, 0.84) }, &[Hills(true), Feature(Some("forest"))]),
            stroke(Beyond { from: p(0.18, 1.0), to: p(0.18, 0.0) }, &[Hills(true), Feature(Some("forest"))]),
            stroke(Beyond { from: p(0.82, 0.0), to: p(0.82, 1.0) }, &[Hills(true), Feature(Some("forest"))]),
            // The peaks the guns were dug into.
            stroke(Blob { at: p(0.50, 0.05), radius: 0.07 }, &[Terrain("mountain"), Feature(None)]),
            stroke(Blob { at: p(0.08, 0.50), radius: 0.07 }, &[Terrain("mountain"), Feature(None)]),
            stroke(Blob { at: p(0.92, 0.50), radius: 0.07 }, &[Terrain("mountain"), Feature(None)]),
            stroke(Blob { at: p(0.50, 0.95), radius: 0.07 }, &[Terrain("mountain"), Feature(None)]),
            // The valley floor, and the Nam Yum through it.
            stroke(Area { from: p(0.24, 0.24), to: p(0.76, 0.76) }, OPEN),
            stroke(Band { from: p(0.56, 0.20), to: p(0.52, 0.80), reach: 0.022 }, &[River]),
            // The airstrip: the camp's only line of supply, and the thing the
            // hills' guns could see.
            stroke(Band { from: p(0.44, 0.34), to: p(0.44, 0.64), reach: 0.028 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
            // The outlying strongpoints — Beatrice, Gabrielle, Isabelle and the
            // rest — sited on low rises around the strip.
            stroke(Blob { at: p(0.36, 0.30), radius: 0.035 },
                   &[Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.62, 0.32), radius: 0.035 },
                   &[Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.34, 0.66), radius: 0.035 },
                   &[Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.64, 0.68), radius: 0.035 },
                   &[Hills(true), Improvement(Some("fort"))]),
        ],
        fronts: [
            // The garrison on the valley floor, around the strip. (Seat 0 is
            // the French entrenched camp.)
            front(p(0.46, 0.44), p(0.46, 0.58)),
            // The siege army, on the high ground looking down into it.
            front(p(0.50, 0.12), p(0.26, 0.16)),
        ],
    },
    // Sinai, June 1967. The Israeli armour crossed from the north-east and
    // drove west and south-west across open desert; the Egyptian army held
    // fortified positions covering the routes, and the whole campaign ended at
    // the two passes through the central massif — Mitla and Giddi — where a
    // retreating army has to funnel and can be caught. The chart is desert
    // with a dune belt no tank crosses quickly, a ridge across the middle, and
    // exactly two ways through it.
    Plan {
        id: "six_day_war",
        base: "desert",
        base_hills: false,
        strokes: &[
            stroke(All, SAND),
            // The central massif, with the two passes cut through it.
            stroke(Band { from: p(0.42, 0.0), to: p(0.38, 1.0), reach: 0.075 },
                   &[Terrain("mountain"), Feature(None)]),
            stroke(Band { from: p(0.30, 0.28), to: p(0.52, 0.30), reach: 0.03 },
                   &[Terrain("desert"), Feature(None), Hills(true)]),
            stroke(Band { from: p(0.28, 0.70), to: p(0.50, 0.72), reach: 0.03 },
                   &[Terrain("desert"), Feature(None), Hills(true)]),
            // The dune sea in the south, and the coastal strip in the north.
            stroke(Blob { at: p(0.66, 0.88), radius: 0.16 }, &[Terrain("desert"), Hills(true)]),
            stroke(Beyond { from: p(0.0, 0.07), to: p(1.0, 0.07) }, SEA),
            stroke(Band { from: p(0.0, 0.12), to: p(1.0, 0.12), reach: 0.035 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
            // Fortified positions covering the eastern approaches.
            stroke(Blob { at: p(0.70, 0.22), radius: 0.05 },
                   &[Terrain("desert"), Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.74, 0.52), radius: 0.05 },
                   &[Terrain("desert"), Hills(true), Improvement(Some("fort"))]),
            // Wells and the odd oasis: the only water on the board.
            stroke(Blob { at: p(0.20, 0.46), radius: 0.04 },
                   &[Terrain("desert"), Feature(Some("oasis")), Hills(false)]),
        ],
        fronts: [
            // The Israeli armoured columns, entering from the north-east.
            // (Seat 0 is the IDF.)
            front(p(0.90, 0.22), p(0.90, 0.60)),
            // The Egyptian army, holding the routes and the passes behind it.
            front(p(0.56, 0.26), p(0.56, 0.68)),
        ],
    },
    // ------------------------------------------------------------ Information
    //
    // The ground war in Kuwait and southern Iraq, February 1991. Flat, open,
    // trackless desert — the reason the "left hook" worked at all is that
    // there was nothing in the west to stop a corps going round. The Iraqi
    // defence faced south behind a berm, a fire trench and minefields, and was
    // dug in around the oil fields in the east; the coalition's armoured
    // sweep came from the empty west, which is why the fortified line was
    // still facing the wrong way when it was reached.
    Plan {
        id: "desert_storm",
        base: "desert",
        base_hills: false,
        strokes: &[
            stroke(All, SAND),
            // The Persian Gulf on the eastern edge, and the sabkha flats along
            // it that armour avoids.
            stroke(Beyond { from: p(0.95, 0.0), to: p(0.98, 1.0) }, SEA),
            stroke(Band { from: p(0.90, 0.0), to: p(0.93, 1.0), reach: 0.035 }, MUD),
            // The border berm and its trench line, facing south.
            stroke(Band { from: p(0.42, 0.56), to: p(0.92, 0.56), reach: 0.028 },
                   &[Terrain("desert"), Hills(true), Feature(None)]),
            stroke(Blob { at: p(0.58, 0.56), radius: 0.045 },
                   &[Terrain("desert"), Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.76, 0.56), radius: 0.045 },
                   &[Terrain("desert"), Hills(true), Improvement(Some("fort"))]),
            // The Wadi al-Batin, the one piece of ground on the map, running
            // down toward the border.
            stroke(Band { from: p(0.34, 0.10), to: p(0.40, 0.56), reach: 0.025 }, &[River]),
            // The burning oil field in the east.
            stroke(Blob { at: p(0.80, 0.28), radius: 0.09 },
                   &[Terrain("desert"), Hills(true), Feature(None)]),
            // The empty west: the flank the hook went round, kept deliberately
            // featureless because that emptiness is the operational fact.
            stroke(Area { from: p(0.02, 0.10), to: p(0.30, 0.94) }, OPEN),
        ],
        fronts: [
            // VII Corps, coming up out of the empty desert in the west.
            // (Seat 0 is the coalition corps.)
            front(p(0.14, 0.80), p(0.14, 0.40)),
            // The Republican Guard, dug in behind the berm and around the oil.
            front(p(0.70, 0.40), p(0.70, 0.18)),
        ],
    },
    // Fallujah, November 2004. A dense low-rise city on a grid, the Euphrates
    // closing its western side, and an assault that came in from the NORTH and
    // pushed south down the length of it. What makes the ground is the
    // building line: walled compounds and narrow streets where fires are
    // measured in tens of metres, cut by a few wide north–south routes that
    // are the only places armour can move quickly and are therefore the only
    // places it can be ambushed at range.
    Plan {
        id: "fallujah",
        base: "desert",
        base_hills: false,
        strokes: &[
            stroke(All, SAND),
            // The Euphrates along the west, with its green belt of irrigated
            // ground and palm groves.
            stroke(Beyond { from: p(0.10, 1.0), to: p(0.07, 0.0) }, SEA),
            stroke(Band { from: p(0.14, 0.0), to: p(0.12, 1.0), reach: 0.05 },
                   &[Terrain("grassland"), Feature(Some("forest")), Hills(false)]),
            // The built-up city: dense blocks, walls and roofs.
            stroke(Area { from: p(0.24, 0.10), to: p(0.92, 0.94) },
                   &[Terrain("plains"), Hills(true), Feature(None)]),
            // The wide routes through it, north–south and east–west.
            stroke(Band { from: p(0.40, 0.06), to: p(0.40, 0.96), reach: 0.022 },
                   &[Terrain("plains"), Hills(false), Feature(None)]),
            stroke(Band { from: p(0.66, 0.06), to: p(0.66, 0.96), reach: 0.022 },
                   &[Terrain("plains"), Hills(false), Feature(None)]),
            stroke(Band { from: p(0.22, 0.52), to: p(0.94, 0.52), reach: 0.022 },
                   &[Terrain("plains"), Hills(false), Feature(None)]),
            // Strongpoints in the old quarter, where the defence was deepest.
            stroke(Blob { at: p(0.52, 0.68), radius: 0.06 },
                   &[Terrain("plains"), Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.78, 0.74), radius: 0.05 },
                   &[Terrain("plains"), Hills(true), Improvement(Some("fort"))]),
            // The open desert outside the city on the north and east, where the
            // cordon sat.
            stroke(Area { from: p(0.20, 0.0), to: p(1.0, 0.07) }, OPEN),
            stroke(Area { from: p(0.94, 0.06), to: p(1.0, 1.0) }, OPEN),
        ],
        fronts: [
            // The assault, forming up on the northern edge. (Seat 0 is the
            // Multi-National Force.)
            front(p(0.44, 0.03), p(0.76, 0.03)),
            // The defence, in the compounds of the old city.
            front(p(0.52, 0.60), p(0.80, 0.66)),
        ],
    },
    // Mosul, 2016–17. The Tigris runs north–south through the middle of the
    // city and splits the battle in two: the eastern half fell in months, and
    // then the army had to cross a river whose bridges were all down and fight
    // for the Old City on the west bank — a medieval quarter of alleys too
    // narrow for a vehicle, where the last defence held out. So the chart is
    // two cities of different textures with a river between them, and the
    // crossings are the hinge.
    Plan {
        id: "mosul",
        base: "plains",
        base_hills: false,
        strokes: &[
            stroke(All, &[Terrain("plains"), Feature(None), Hills(false)]),
            // The Tigris.
            stroke(Band { from: p(0.48, 0.0), to: p(0.52, 1.0), reach: 0.035 }, SEA),
            // The crossings: two damaged bridges, the only ways over.
            stroke(Blob { at: p(0.49, 0.30), radius: 0.035 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
            stroke(Blob { at: p(0.51, 0.70), radius: 0.035 },
                   &[Terrain("plains"), Feature(None), Hills(false)]),
            // East Mosul: modern blocks, wide streets.
            stroke(Area { from: p(0.58, 0.10), to: p(0.94, 0.90) },
                   &[Terrain("plains"), Hills(true), Feature(None)]),
            stroke(Band { from: p(0.56, 0.46), to: p(0.96, 0.46), reach: 0.025 },
                   &[Terrain("plains"), Hills(false), Feature(None)]),
            // The Old City on the west bank: the densest ground on any chart
            // in this catalogue, and the last to fall.
            stroke(Area { from: p(0.18, 0.30), to: p(0.46, 0.78) },
                   &[Terrain("plains"), Hills(true), Feature(None)]),
            stroke(Blob { at: p(0.32, 0.54), radius: 0.09 },
                   &[Terrain("plains"), Hills(true), Improvement(Some("fort"))]),
            stroke(Blob { at: p(0.24, 0.40), radius: 0.05 },
                   &[Terrain("plains"), Hills(true), Improvement(Some("fort"))]),
            // Irrigated ground and the airport in the south-west outskirts.
            stroke(Blob { at: p(0.14, 0.84), radius: 0.09 },
                   &[Terrain("grassland"), Feature(Some("forest")), Hills(false)]),
            stroke(Area { from: p(0.04, 0.06), to: p(0.40, 0.24) }, OPEN),
        ],
        fronts: [
            // The Iraqi divisions, working in from the east. (Seat 0 is the
            // Iraqi Security Forces.)
            front(p(0.88, 0.50), p(0.88, 0.24)),
            // The defence, holding the Old City on the far bank.
            front(p(0.32, 0.46), p(0.32, 0.68)),
        ],
    },
];
