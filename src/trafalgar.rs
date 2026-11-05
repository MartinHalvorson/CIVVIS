//! The Battle of Trafalgar, 21 October 1805, as a Tactics scenario.
//!
//! Everything that makes this battle *this* battle rather than a rolled arena
//! lives here: the chart the fleets are on, and the two orders of battle laid
//! out on it. `mapgen` asks this module for the ground and the two seats;
//! `game` asks it for the ships. Neither has to know any history, and the
//! history is all in one file where it can be checked against a source.
//!
//! # What is being claimed
//!
//! The position is the one at about noon, as Collingwood's lee column came
//! within range and twenty minutes before Nelson's weather column reached the
//! line. Twenty-seven British ships of the line are bearing down from the west
//! in two columns, roughly at right angles to the Combined Fleet's thirty-three,
//! which are strung out on the starboard tack heading north for Cádiz in a
//! ragged crescent — Collingwood's dispatch has it "formed in a close line of
//! battle ahead ... their line was formed in a crescent convexing to leeward",
//! and leeward, with the wind in the west, is the shore.
//!
//! # What is not
//!
//! - **The two sides are not even.** That is the point of the battle and it is
//!   why this map is a scenario and not an arena; see
//!   [`crate::setup::MapScript::is_scenario`]. Nothing here should ever be used
//!   to compare two agents.
//! - **Every ship of the line is a Frigate.** The ruleset has exactly one
//!   sailing warship of the age, and a 136-gun *Santísima Trinidad* and a
//!   64-gun *Africa* are both it. What separates them here is a promotion:
//!   [`rate_promotions`] gives every ship rated 74 and over the heavier
//!   broadside her guns are worth, and the four smallest nothing. That is as
//!   fine as the ruleset's own naval tree can cut it — a first rate is a ship
//!   of the line and no more, and `rate_promotions` records what happened when
//!   the scenario tried to say otherwise.
//! - **Gunnery is still not modelled**, and it is what actually decided the
//!   exchange once the lines were locked: the Royal Navy fired two to three
//!   times as fast, after years of blockade drill against ships shut up in
//!   Cádiz. That is a claim about crews rather than about ships, and the rate
//!   ladder above deliberately reads a gun figure and asks nothing about whose
//!   flag is up. So what the board still tests is the part Nelson could
//!   choose: where sixty ships were, and what was done with them.
//! - **The wind is not modelled either**, and it mattered enormously: it was
//!   light and westerly, it put the British columns on a slow approach under
//!   fire they could not answer, and it left the Combined Fleet's van unable
//!   to beat back down to the action for hours. What survives of it here is
//!   geometry — the van starts far from the fighting, and it starts north.
//! - **The distances are compressed.** Ships in a column stand one hex apart
//!   where they were one or two cables apart; the shore is drawn a few hexes
//!   off the rear where it was nine miles. Both are so the position is legible
//!   on a board a battle can be played out on.
//!
//! Frigates, the schooner *Pickle* and the cutter *Entreprenante* are left off
//! entirely. They were present, Blackwood's *Euryalus* repeated Nelson's
//! signals throughout, and not one of them fired into the line — putting them
//! on the board would add nine ships to a ledger they took no part in.

use std::collections::BTreeSet;

use crate::hex;
use crate::world::WorldMap;
use crate::Pos;

/// The water and shore the battle is fought over.
///
/// Thirty columns by twenty-four rows of odd-r offset cells, west at the left
/// and north at the top. Row 0 is the sea toward Cádiz, twenty miles north of
/// the action; the eastern edge is the Andalusian coast; the headland reaching
/// out at rows 18 and 19 is **Cape Trafalgar**, and the bank west of it is its
/// shoals — the lee shore the Combined Fleet had under its stern all day, and
/// the one the gale drove a dozen prizes onto that night.
///
/// | cell | what it is |
/// | --- | --- |
/// | `~` | open sea |
/// | `:` | shoal water off the cape |
/// | `.` | the shore itself |
/// | `#` | inland |
///
/// Every sea cell is `coast` rather than `ocean`. That is true enough of the
/// shelf water the fleets fought and anchored on, and it also means no ship is
/// walled in by Cartography, which this scenario grants nobody.
pub const CHART: [&str; 24] = [
    // 000000000011111111112222222222
    // 012345678901234567890123456789
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~.", //  0
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~.", //  1
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~.", //  2
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~.", //  3
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~.", //  4
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~.", //  5
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~.", //  6
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~.#", //  7
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~.#", //  8
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~.#", //  9
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~.#", // 10
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~.#", // 11
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~.##", // 12
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~.##", // 13
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~.##", // 14
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~.##", // 15
    "~~~~~~~~~~~~~~~~~~~~~~~~~~.###", // 16
    "~~~~~~~~~~~~~~~~~~~~~~~~:~.###", // 17
    "~~~~~~~~~~~~~~~~~~~~~~~::.####", // 18  <- Cape Trafalgar
    "~~~~~~~~~~~~~~~~~~~~~~~::.####", // 19
    "~~~~~~~~~~~~~~~~~~~~~~~~::.###", // 20
    "~~~~~~~~~~~~~~~~~~~~~~~~~~.###", // 21
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~.##", // 22
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~.##", // 23
];

/// The chart's width in cells. [`crate::setup::BATTLEFIELD_SIZES`] publishes
/// the same figure, and [`chart_land`] asserts the map it is handed agrees.
pub const WIDTH: i32 = CHART[0].len() as i32;
/// The chart's height in cells.
pub const HEIGHT: i32 = CHART.len() as i32;

/// The ship each side's commander-in-chief flew his flag in, and so the tile
/// the generator seats that side on: `Victory` for Britain, `Bucentaure` for
/// the Combined Fleet. Britain first, because Britain moves first.
pub const FLAGSHIPS: [(i32, i32); 2] = [BRITISH[0].at, COMBINED[11].at];

/// One ship: where she starts, what she rated, and what she was called.
///
/// `guns` is data — it decides which promotions she carries, see
/// [`rate_promotions`]. The name is documentation, because the engine has no
/// per-unit name, but it is what makes the two tables below auditable against
/// a published line of battle instead of being sixty coordinates nobody can
/// check. It deliberately does *not* repeat the gun figure: one of the two
/// would eventually drift and the wrong one would be believed.
pub struct Ship {
    pub at: (i32, i32),
    pub guns: u16,
    /// The flag officer aboard, rated 2 to 5 stars, or 0 for the great
    /// majority of ships that carried none. See [`admiral_formation`].
    pub stars: u8,
    pub name: &'static str,
}

const fn ship(col: i32, row: i32, guns: u16, name: &'static str) -> Ship {
    Ship {
        at: (col, row),
        guns,
        stars: 0,
        name,
    }
}

/// A ship with an admiral's flag in her, and how good he was.
const fn flag(col: i32, row: i32, guns: u16, stars: u8, name: &'static str) -> Ship {
    Ship {
        at: (col, row),
        guns,
        stars,
        name,
    }
}

/// Nelson's fleet, twenty-seven of the line, in the order they were stationed.
///
/// Two columns in line ahead heading east, the weather column on row 10 and
/// the lee column on row 15 — about a mile apart at this scale. Collingwood's
/// lee column starts one hex nearer the enemy than Nelson's, which is the
/// twenty minutes by which the freshly-coppered `Royal Sovereign` beat
/// `Victory` into the fight.
///
/// `Africa` is the exception and is on the board where she really was: away to
/// the north, separated in the night, with the whole enemy van between her and
/// her own fleet. She spent the morning running down their line alone.
pub const BRITISH: [Ship; 27] = [
    // --- Weather column. Vice-Admiral Lord Nelson, in Victory.
    flag(
        16,
        10,
        100,
        5,
        "Victory — Vice-Admiral Lord Nelson, commander-in-chief",
    ),
    ship(15, 10, 98, "Temeraire"),
    ship(14, 10, 98, "Neptune"),
    ship(13, 10, 74, "Conqueror"),
    ship(12, 10, 74, "Leviathan"),
    ship(11, 10, 74, "Ajax"),
    ship(10, 10, 74, "Orion"),
    ship(9, 10, 64, "Agamemnon"),
    ship(8, 10, 74, "Minotaur"),
    ship(7, 10, 74, "Spartiate"),
    flag(
        6,
        10,
        100,
        3,
        "Britannia — Rear-Admiral the Earl of Northesk",
    ),
    // --- Lee column. Vice-Admiral Collingwood, in Royal Sovereign.
    flag(
        17,
        15,
        100,
        4,
        "Royal Sovereign — Vice-Admiral Collingwood, second in command",
    ),
    ship(16, 15, 74, "Belleisle"),
    ship(15, 15, 74, "Mars"),
    ship(14, 15, 80, "Tonnant"),
    ship(13, 15, 74, "Bellerophon"),
    ship(12, 15, 74, "Colossus"),
    ship(11, 15, 74, "Achille"),
    ship(10, 15, 64, "Polyphemus"),
    ship(9, 15, 74, "Revenge"),
    ship(8, 15, 74, "Swiftsure"),
    ship(7, 15, 74, "Defiance"),
    ship(6, 15, 74, "Thunderer"),
    ship(5, 15, 74, "Defence"),
    ship(4, 15, 98, "Prince"),
    ship(3, 15, 98, "Dreadnought"),
    // --- Detached, and a long way from help.
    ship(
        16,
        1,
        64,
        "Africa — separated in the night, north of the van",
    ),
];

/// Villeneuve's fleet, thirty-three of the line, from the van southward.
///
/// One crescent running north to south on the starboard tack, bowed two hexes
/// to leeward at the centre. The line is one ship deep in the van and two deep
/// from the centre aft, which is what "ragged" meant on the day: the rear had
/// bunched, and Gravina's squadron of observation was to leeward of it rather
/// than in it. Every doubled ship is therefore the eastern of its pair.
///
/// The nine ships of the van sit at the top of the map with a great deal of
/// water between them and the fighting. That distance is the battle: with the
/// wind where it was, Dumanoir could not get back into it, and ten of these
/// thirty-three never fired a broadside.
pub const COMBINED: [Ship; 33] = [
    // --- Van. Rear-Admiral Dumanoir le Pelley, in Formidable.
    ship(19, 1, 80, "Neptuno (Sp)"),
    ship(19, 2, 74, "Scipion (Fr)"),
    ship(20, 3, 74, "Intrepide (Fr)"),
    ship(20, 4, 100, "Rayo (Sp)"),
    flag(
        20,
        5,
        80,
        2,
        "Formidable (Fr) — Rear-Admiral Dumanoir le Pelley, the van",
    ),
    ship(21, 6, 74, "Duguay-Trouin (Fr)"),
    ship(21, 7, 74, "Mont-Blanc (Fr)"),
    ship(21, 8, 74, "San Francisco de Asis (Sp)"),
    ship(21, 9, 74, "San Agustin (Sp)"),
    // --- Centre. Vice-Admiral Villeneuve, in Bucentaure.
    ship(21, 10, 74, "Heros (Fr)"),
    flag(
        22,
        10,
        136,
        3,
        "Santisima Trinidad (Sp) — Rear-Admiral Cisneros",
    ),
    flag(
        21,
        11,
        80,
        2,
        "Bucentaure (Fr) — Vice-Admiral Villeneuve, commander-in-chief",
    ),
    ship(22, 11, 74, "Redoutable (Fr) — Lucas"),
    ship(21, 12, 74, "San Justo (Sp)"),
    ship(22, 12, 80, "Neptune (Fr)"),
    ship(21, 13, 64, "San Leandro (Sp)"),
    flag(22, 13, 112, 3, "Santa Ana (Sp) — Vice-Admiral Alava"),
    ship(21, 14, 80, "Indomptable (Fr)"),
    ship(22, 14, 74, "Fougueux (Fr)"),
    // --- Rear, and the squadron of observation to leeward of it.
    ship(21, 15, 74, "Monarca (Sp)"),
    ship(22, 15, 74, "Pluton (Fr)"),
    flag(21, 16, 74, 3, "Algesiras (Fr) — Rear-Admiral Magon"),
    ship(22, 16, 74, "Bahama (Sp)"),
    ship(20, 17, 74, "Aigle (Fr)"),
    ship(21, 17, 74, "Swiftsure (Fr)"),
    ship(20, 18, 74, "Montanes (Sp)"),
    ship(21, 18, 74, "Argonaute (Fr)"),
    ship(20, 19, 80, "Argonauta (Sp)"),
    ship(21, 19, 74, "San Ildefonso (Sp)"),
    ship(20, 20, 74, "Achille (Fr)"),
    flag(
        21,
        20,
        112,
        4,
        "Principe de Asturias (Sp) — Admiral Gravina, the squadron of observation",
    ),
    ship(19, 21, 74, "Berwick (Fr)"),
    ship(19, 22, 74, "San Juan Nepomuceno (Sp)"),
];

/// What every ship on the board is built as.
///
/// The Frigate is the ruleset's Renaissance sailing warship — Square Rigging,
/// four movement, and a two-tile broadside it fires without closing — and it
/// is the only thing in the roster a ship of the line can be. See the module
/// header for what that abstraction costs, and [`rate_promotions`] for the
/// part of it that is bought back.
pub const SHIP_OF_THE_LINE: &str = "frigate";

/// What a ship's rate is worth, as promotions.
///
/// One Frigate has to stand for both a 136-gun *Santísima Trinidad* and a
/// 64-gun *Africa*, and those are not the same ship. Promotions are how the
/// difference is said: they are the engine's own per-unit modifiers, they are
/// read by combat without any new mechanism, and a scenario granting them at
/// setup is the same act as a veteran unit having earned them.
///
/// Two bands, from the one figure that separates these ships in every
/// published line of battle — how many guns she carried:
///
/// | rate | promotion | ships |
/// | --- | --- | --- |
/// | 64 and under | none | 3 British, 1 Spanish |
/// | 74 and over | `line_of_battle` | 24 British, 32 Combined |
///
/// `line_of_battle` is **+7 Ranged Strength against naval units** — weight of
/// broadside, which is very nearly what a gun figure measures, and the reason
/// a 64 was not stationed in the line if a 74 could be had instead.
///
/// # Why only two bands, when there are three sizes of ship
///
/// A third band for the seven three-deckers was built and then measured out
/// again, and the measurement is worth keeping because the answer was not
/// close. The Frigate's own promotion tree has exactly **one** promotion that
/// adds broadside against ships — the one above. Everything else in it is
/// anti-land (`preparatory_fire`), anti-district (`bombardment`,
/// `rolling_barrage`), anti-air (`proximity_fuses`), or healing
/// (`supply_fleet`, which `Game::unit_heal_rate` switches off outright on
/// every Tactics map). This battle has no land units, no districts, no
/// aircraft, and nothing in it heals. So the only way to give a first rate
/// *more* was `coincidence_rangefinding`, +1 attack range.
///
/// Three runs a configuration, stock controllers, 100-turn clock:
///
/// | ladder | seed 1805 | seed 7 | seed 42 |
/// | --- | --- | --- | --- |
/// | no promotions at all | draw | draw | draw |
/// | `line_of_battle` at 74+ | draw, 25 against 12 | draw | draw |
/// | plus +1 range at 100+ | **France, turn 29** | **France, turn 28** | **France, turn 27** |
///
/// A ship that outranges everything on the board fires without reply, and the
/// Combined Fleet had four of them against Britain's three. That turned a
/// hundred-turn action into a rout inside thirty — not because the Combined
/// Fleet was heavier, which it was, but because the stand-in was far stronger
/// than the thing it stood in for. A three-decker's guns did not shoot
/// appreciably further; what she had was weight. Since the board has no way
/// to say "heavier still", she is a ship of the line and no more, and the
/// scenario says so rather than reaching for the only lever left.
///
/// **The same rule, both fleets.** It reads a gun figure and asks nothing
/// about whose flag is up. What it therefore does *not* model is the Royal
/// Navy's rate of fire, two to three times the Combined Fleet's after years of
/// blockade drill against ships shut up in Cádiz, and the factor that actually
/// decided the exchange once the lines were locked. That is a claim about
/// crews rather than about ships, and this scenario does not make it.
pub fn rate_promotions(guns: u16) -> &'static [&'static str] {
    match guns {
        0..=64 => &[],
        _ => &["line_of_battle"],
    }
}

/// What a flag officer aboard is worth to the ship he is in.
///
/// One extra Movement, and nothing else. A flagship was better handled than
/// the ships around her — she carried the admiral's own staff, she repeated
/// signals, and she was usually the smartest sailer in her division because
/// that is where an admiral chose to put his flag. It is granted through
/// `Unit::bonus_moves`, which is the engine's existing field for movement a
/// unit was given rather than earned.
///
/// Every flagship gets it, whatever her admiral's rating: putting a flag in a
/// ship is what made her better handled, not how good the man was. What his
/// rating decides is separate and is [`admiral_formation`] below.
pub const ADMIRAL_MOVEMENT_BONUS: f64 = 1.0;

/// The admiral commanding this side, and the tile his flag is on.
pub fn commander_in_chief(pid: usize) -> &'static Ship {
    let at = FLAGSHIPS[pid.min(1)];
    fleet(pid)
        .iter()
        .find(|ship| ship.at == at)
        .expect("each fleet's commander-in-chief is in its own order of battle")
}

/// The fighting tier an admiral's rating is worth to the ship he is in.
///
/// A **Fleet** (`formation` 1) for four and five stars, and nothing above the
/// ordinary for two and three. The engine prices a Fleet at **+10 Strength**,
/// applied through `Game::unit_formation_bonus` inside `unit_ranged_strength`,
/// so it reaches a ship of the line's broadside rather than only a boarding
/// action.
///
/// # Why a threshold, and not stars times two
///
/// Because of how the flags were distributed. There were **three** British
/// flag officers at Trafalgar and **six** in the Combined Fleet, so any bonus
/// paid out per flagship hands more of it to the larger, more admiral-heavy
/// side — which is backwards, and would have the feature arguing the opposite
/// of what it exists to say. A threshold pays only for the admirals who were
/// actually good, and the asymmetry then falls out of rating the men rather
/// than out of a thumb on the scale:
///
/// | | flags | of those, 4 stars or better | worth |
/// | --- | --- | --- | --- |
/// | Britain | 3 | **2** — Nelson, Collingwood | +20 |
/// | Combined Fleet | 6 | **1** — Gravina | +10 |
///
/// | admiral | stars | |
/// | --- | --- | --- |
/// | Nelson, in *Victory* | 5 | Fleet |
/// | Collingwood, in *Royal Sovereign* | 4 | Fleet |
/// | Gravina, in *Príncipe de Asturias* | 4 | Fleet |
/// | Northesk, Cisneros, Álava, Magon | 3 | — |
/// | Villeneuve, in *Bucentaure* | 2 | — |
/// | Dumanoir le Pelley, in *Formidable* | 2 | — |
///
/// The two at the bottom carry the most weight and so are worth defending.
/// Villeneuve was a capable seaman who believed in neither the plan, the fleet
/// nor his orders, had been told he was about to be relieved, and signalled
/// nothing of consequence after the action opened. Dumanoir commanded a van
/// that took four hours to come about and never meaningfully engaged. Gravina
/// at four is the other side of it: the best-fought squadron in the Combined
/// Fleet, its commander mortally wounded holding the rear together.
///
/// # What was tried first, and why it is not here
///
/// The first version of this spent an admiral's rating on **flanking** —
/// `Game::flanking_bonus`, +2 Strength for every friendly ship adjacent to
/// the target beyond the attacker, multiplied by the owner's naval flanking
/// bonus, with a five-star admiral set to the 50% the ruleset already gives
/// its own Horatio Nelson. Cutting a line and doubling on the ships it
/// isolates is Nelson's whole plan, so that looked like the right home for it.
///
/// It cannot work here, for two independent reasons, and both were measured
/// rather than reasoned about:
///
/// 1. **`flanking_bonus` is only ever called from `do_attack`** — the melee
///    path. Every ship in this battle is a Frigate, which is `naval_ranged`
///    and attacks through `do_ranged`, and `do_ranged` never consults it.
/// 2. **The ships never close anyway.** Over 120 turns of the scenario played
///    out by the stock controllers, *no ship was ever adjacent to two
///    enemies* — the most any ship ever had alongside was one. A unit that
///    shoots two tiles away has no reason to come to contact, and it does not.
///
/// A "Fleet" in the shipped rules is two ships merged into one unit, so using
/// it to mean "an admiral is aboard" is a reinterpretation, and one with a
/// visible edge: `Game::unit_production_cost` prices a Fleet at 1.5x, so the
/// three flagships weigh half again as much in any material ledger. For a
/// scenario that is close enough to true — a first-rate flagship *was* worth
/// more than a 74 — but it is a reinterpretation, and it is recorded here
/// rather than left to be discovered.
pub fn admiral_formation(stars: u8) -> u8 {
    u8::from(stars >= 4)
}

/// One side's order of battle, Britain as seat 0.
pub fn fleet(pid: usize) -> &'static [Ship] {
    if pid == 0 {
        &BRITISH
    } else {
        &COMBINED
    }
}

/// The chart cell at an offset coordinate, or `None` off the chart.
fn cell(col: i32, row: i32) -> Option<u8> {
    let line = CHART.get(usize::try_from(row).ok()?)?.as_bytes();
    line.get(usize::try_from(col).ok()?).copied()
}

/// Whether a chart cell can be sailed through.
#[cfg(test)]
fn is_sea(col: i32, row: i32) -> bool {
    matches!(cell(col, row), Some(b'~' | b':'))
}

/// The dry cells of the chart, as map positions.
///
/// A map built at any other size would read the chart through a window and
/// silently lose the cape, so the size is asserted rather than scaled. Nothing
/// can reach here with the wrong one — the script publishes exactly one size,
/// and it is the chart's — which is what makes the assertion cheap to keep.
pub fn chart_land(wm: &WorldMap) -> BTreeSet<Pos> {
    assert_eq!(
        (wm.width, wm.height),
        (WIDTH, HEIGHT),
        "the Trafalgar scenario is drawn at the size of its chart and no other"
    );
    (0..HEIGHT)
        .flat_map(|row| (0..WIDTH).map(move |col| (col, row)))
        .filter(|(col, row)| matches!(cell(*col, *row), Some(b'.' | b'#')))
        .map(|(col, row)| hex::offset_to_axial(col, row))
        .filter(|pos| wm.tiles.contains_key(pos))
        .collect()
}

/// Lay the scenario's sea and shore over whatever the world passes made of it.
///
/// Same argument as `mapgen::paint_battlefield_ground`, for the same reason: a
/// rolled climate has nothing useful to say about seven hundred tiles of the
/// Gulf of Cádiz, and left to itself it grows ice, jungle and a resource
/// lottery over the battle. This reads the chart instead, and it is the last
/// word on every tile.
///
/// The shoals carry the `reef` feature, the ruleset's own name for water that
/// is dangerous to be driven onto and worth +3 to whoever is already in it.
pub fn paint_chart(wm: &mut WorldMap) {
    let all: Vec<Pos> = wm.tiles.keys().copied().collect();
    for pos in all {
        let (col, row) = hex::axial_to_offset(pos.0, pos.1);
        let cell = cell(col, row).unwrap_or(b'~');
        let tile = wm.tiles.get_mut(&pos).unwrap();
        tile.terrain = match cell {
            // Sandy pine coast, and grassland behind it. Neither is ground
            // this battle is decided on; both keep the shore reading as
            // somewhere rather than as a wall.
            b'.' => "plains".into(),
            b'#' => "grassland".into(),
            _ => "coast".into(),
        };
        tile.feature = (cell == b':').then(|| "reef".into());
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.river_edges = [false; 6];
        tile.cliff_edges = [false; 6];
        tile.coastal_lowland = 0;
        tile.continent = Some(0);
    }
}

/// The two fleets' seats, Britain first, or `None` if this map cannot hold
/// them — which on the one size the script publishes it always can.
///
/// Unlike every other layout the generator picks, the order matters and is not
/// dealt out: swapping Nelson and Villeneuve would be a different battle with
/// the same name.
pub fn major_starts(wm: &WorldMap) -> Option<Vec<Pos>> {
    let starts: Vec<Pos> = FLAGSHIPS
        .iter()
        .map(|(col, row)| hex::offset_to_axial(*col, *row))
        .collect();
    starts
        .iter()
        .all(|pos| wm.tiles.contains_key(pos))
        .then_some(starts)
}

#[cfg(test)]
mod tests;
