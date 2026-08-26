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
    Ship { at: (col, row), guns, stars: 0, name }
}

/// A ship with an admiral's flag in her, and how good he was.
const fn flag(col: i32, row: i32, guns: u16, stars: u8, name: &'static str) -> Ship {
    Ship { at: (col, row), guns, stars, name }
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
    flag(16, 10, 100, 5, "Victory — Vice-Admiral Lord Nelson, commander-in-chief"),
    ship(15, 10, 98, "Temeraire"),
    ship(14, 10, 98, "Neptune"),
    ship(13, 10, 74, "Conqueror"),
    ship(12, 10, 74, "Leviathan"),
    ship(11, 10, 74, "Ajax"),
    ship(10, 10, 74, "Orion"),
    ship(9, 10, 64, "Agamemnon"),
    ship(8, 10, 74, "Minotaur"),
    ship(7, 10, 74, "Spartiate"),
    flag(6, 10, 100, 3, "Britannia — Rear-Admiral the Earl of Northesk"),
    // --- Lee column. Vice-Admiral Collingwood, in Royal Sovereign.
    flag(17, 15, 100, 4, "Royal Sovereign — Vice-Admiral Collingwood, second in command"),
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
    ship(16, 1, 64, "Africa — separated in the night, north of the van"),
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
    flag(20, 5, 80, 2, "Formidable (Fr) — Rear-Admiral Dumanoir le Pelley, the van"),
    ship(21, 6, 74, "Duguay-Trouin (Fr)"),
    ship(21, 7, 74, "Mont-Blanc (Fr)"),
    ship(21, 8, 74, "San Francisco de Asis (Sp)"),
    ship(21, 9, 74, "San Agustin (Sp)"),
    // --- Centre. Vice-Admiral Villeneuve, in Bucentaure.
    ship(21, 10, 74, "Heros (Fr)"),
    flag(22, 10, 136, 3, "Santisima Trinidad (Sp) — Rear-Admiral Cisneros"),
    flag(21, 11, 80, 2, "Bucentaure (Fr) — Vice-Admiral Villeneuve, commander-in-chief"),
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
    flag(21, 20, 112, 4, "Principe de Asturias (Sp) — Admiral Gravina, the squadron of observation"),
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
    starts.iter().all(|pos| wm.tiles.contains_key(pos)).then_some(starts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Game, GameOptions};
    use crate::rules::Rules;
    use crate::setup::{MapScript, MapTopology, TacticsRules};

    /// The scenario as the lobby and the launcher actually build it.
    fn battle(seed: u64) -> Game {
        Game::new_with(GameOptions {
            map_script: MapScript::Trafalgar,
            map_topology: MapTopology::Flat,
            // Everything a player could ask for and a scenario refuses: a
            // city each, production, gold, research, uniques and a flag.
            tactics: TacticsRules {
                cities: 1,
                production: 90,
                gold: 90,
                turns_per_tech: 3,
                unique_units: true,
                flag: true,
                heal: false,
                ..TacticsRules::default()
            },
            civs: vec!["Nubia".to_string(), "Scythia".to_string()],
            randomize_civs: true,
            ..GameOptions::new(2, WIDTH, HEIGHT, seed, 100, 4)
        })
    }

    /// The chart has to be a rectangle before anything reading it by column
    /// means what it says. A short row would shift every cell after it north
    /// and west without any of the tables below noticing.
    #[test]
    fn the_chart_is_rectangular() {
        for (row, line) in CHART.iter().enumerate() {
            assert_eq!(
                line.len() as i32,
                WIDTH,
                "chart row {row} is {} cells wide, not {WIDTH}",
                line.len()
            );
            assert!(
                line.bytes().all(|cell| matches!(cell, b'~' | b':' | b'.' | b'#')),
                "chart row {row} has a cell that is not sea, shoal, shore or inland"
            );
        }
    }

    /// Sixty ships of the line, which is the count both fleets are known by:
    /// twenty-seven against thirty-three. If either table is edited into a
    /// different battle, this is where it is caught.
    #[test]
    fn both_fleets_are_the_size_history_records() {
        assert_eq!(BRITISH.len(), 27, "Nelson had twenty-seven of the line");
        assert_eq!(COMBINED.len(), 33, "the Combined Fleet had thirty-three");
    }

    /// Every ship rates something a ship of the line rated, and the two fleets
    /// carry the rates history gave them. A gun figure typed with a digit
    /// missing would otherwise silently move a ship between bands.
    #[test]
    fn every_ship_rates_what_a_ship_of_the_line_rated() {
        for ship in BRITISH.iter().chain(COMBINED.iter()) {
            assert!(
                (64..=136).contains(&ship.guns),
                "{} rates {} guns, which is not a ship of the line",
                ship.name,
                ship.guns
            );
        }
        let heaviest = COMBINED.iter().max_by_key(|ship| ship.guns).unwrap();
        assert_eq!(heaviest.guns, 136, "the Santisima Trinidad was the largest ship afloat");
        assert!(heaviest.name.starts_with("Santisima Trinidad"));
        // First rates: three British against four. The Combined Fleet was the
        // heavier as well as the larger, which is the position Nelson chose to
        // attack and the reason it is worth modelling at all.
        let first_rates = |fleet: &[Ship]| fleet.iter().filter(|ship| ship.guns >= 100).count();
        assert_eq!(first_rates(&BRITISH), 3);
        assert_eq!(first_rates(&COMBINED), 4);
    }

    /// A ship's rate reaches the board as promotions, by one rule applied to
    /// both fleets — and every promotion it grants has to be one the Frigate
    /// could take and one that does something in *this* battle. Most of the
    /// naval tree is anti-land, anti-district, anti-air or healing, and a
    /// grant out of those branches would look like modelling while changing
    /// nothing.
    #[test]
    fn a_ships_rate_grants_promotions_that_do_something_here() {
        let rules = Rules::embedded();
        // What a fleet action can actually feel: a bigger broadside against
        // ships, and reach. Nothing else on the board is a land unit, a
        // district or an aircraft, and `unit_heal_rate` returns 0 on every
        // Tactics map, so a healing promotion is inert here by construction.
        let live = ["ranged_vs_naval", "ranged_vs_units", "range"];
        for guns in [64u16, 74, 80, 98, 100, 112, 136] {
            let granted = rate_promotions(guns);
            let held: BTreeSet<&str> = granted.iter().copied().collect();
            assert_eq!(held.len(), granted.len(), "{guns} guns grants a promotion twice");
            for name in granted {
                let spec = rules
                    .promotions
                    .get(&crate::name::Name::new(name))
                    .unwrap_or_else(|| panic!("{name} is not a promotion in the ruleset"));
                // The Frigate's own tree, so these are promotions this ship
                // could be offered rather than borrowed from another class.
                assert_eq!(
                    spec.class, "naval_ranged",
                    "{name} is a {} promotion, not one a ship of the line can take",
                    spec.class
                );
                assert!(
                    spec.effects.keys().any(|effect| live.contains(&effect.as_str())),
                    "{name} is granted at {guns} guns but its effects {:?} do nothing in a \
                     fleet action",
                    spec.effects.keys().collect::<Vec<_>>()
                );
            }
        }

        // The ladder itself: a heavier ship never carries less, and never
        // loses something a lighter one has.
        assert!(rate_promotions(64).is_empty(), "a 64 is the plain ship");
        let ladder = [64u16, 74, 80, 98, 100, 112, 136];
        for pair in ladder.windows(2) {
            let (lighter, heavier) = (rate_promotions(pair[0]), rate_promotions(pair[1]));
            assert!(
                heavier.len() >= lighter.len()
                    && lighter.iter().all(|name| heavier.contains(name)),
                "{} guns does not carry everything {} guns does",
                pair[1],
                pair[0]
            );
        }
        // The bands are read off the gun figure alone, so the same rate gets
        // the same ship whichever line she was in.
        assert_eq!(rate_promotions(74), rate_promotions(98));
        assert_eq!(rate_promotions(100), rate_promotions(136));
        // And a first rate is a ship of the line and no more. The band that
        // would have separated her was measured and removed — see
        // `rate_promotions` for the runs. Asserted rather than assumed,
        // because the tempting fix is to give the biggest ships the only
        // remaining lever (+1 attack range) and that lever decides the battle.
        assert_eq!(
            rate_promotions(136),
            rate_promotions(74),
            "a three-decker has been given something a 74 has not; re-measure before keeping it"
        );
        assert!(
            !rate_promotions(136).contains(&"coincidence_rangefinding"),
            "outranging the whole board turned this scenario into a rout inside thirty turns"
        );
    }

    /// Every ship is on open water, on the chart, and alone on her tile. Only
    /// one military unit stands on a hex, so a duplicate coordinate would
    /// silently drop a ship from the board — and a ship placed on the shore or
    /// on the shoals could not be there at all.
    #[test]
    fn every_ship_is_afloat_and_on_her_own_tile() {
        let mut taken: BTreeSet<(i32, i32)> = BTreeSet::new();
        for Ship { at: (col, row), name: ship, .. } in BRITISH.iter().chain(COMBINED.iter()) {
            assert!(
                (0..WIDTH).contains(col) && (0..HEIGHT).contains(row),
                "{ship} is off the chart at ({col}, {row})"
            );
            assert_eq!(
                cell(*col, *row),
                Some(b'~'),
                "{ship} is not on open water at ({col}, {row})"
            );
            assert!(taken.insert((*col, *row)), "{ship} shares a tile at ({col}, {row})");
        }
        assert_eq!(taken.len(), 60, "sixty ships of the line were in the action");
    }

    /// The seats are the two commanders-in-chief, in that order. Britain first
    /// is the whole of "Britain moves first", so it is asserted rather than
    /// left to the order two constants happen to be written in.
    #[test]
    fn the_seats_are_victory_then_bucentaure() {
        assert_eq!(FLAGSHIPS[0], BRITISH[0].at);
        assert!(BRITISH[0].name.starts_with("Victory"));
        assert_eq!(FLAGSHIPS[1], COMBINED[11].at);
        assert!(COMBINED[11].name.starts_with("Bucentaure"));
    }

    /// The two British columns start west of the Combined Fleet's line, and
    /// clear of it. A scenario whose fleets began interleaved would open with
    /// the approach already over — and the approach is the battle.
    #[test]
    fn the_british_are_to_windward_with_water_still_to_cross() {
        let van_and_line = COMBINED.iter().map(|ship| ship.at.0).min().unwrap();
        let leading_briton = BRITISH.iter().map(|ship| ship.at.0).max().unwrap();
        assert!(
            leading_briton < van_and_line,
            "the leading British ship is at column {leading_briton}, not west of the \
             Combined Fleet's nearest ship at {van_and_line}"
        );
        // Collingwood beat Nelson into action, so his column starts nearer.
        let royal_sovereign = &BRITISH[11];
        assert!(royal_sovereign.name.starts_with("Royal Sovereign"));
        assert!(
            royal_sovereign.at.0 > BRITISH[0].at.0,
            "Royal Sovereign should start ahead of Victory"
        );
    }

    /// The Combined Fleet's line has to be sailable along its whole length, or
    /// the crescent is really two fleets with a wall between them.
    #[test]
    fn the_enemy_line_stands_in_open_water_from_van_to_rear() {
        let rows: Vec<i32> = COMBINED.iter().map(|ship| ship.at.1).collect();
        let (top, bottom) = (*rows.iter().min().unwrap(), *rows.iter().max().unwrap());
        assert!(bottom - top >= 20, "the line should span most of the map north to south");
        for row in top..=bottom {
            assert!(
                COMBINED.iter().any(|ship| ship.at.1 == row),
                "the line has a gap at row {row}"
            );
        }
    }

    /// The cape is under the Combined Fleet's lee, which is the position they
    /// could not retreat out of. Shoals exist, and they are to seaward of the
    /// headland where a fleet driven east would strike them first.
    #[test]
    fn cape_trafalgar_and_its_shoals_are_on_the_lee_side() {
        let shoals: Vec<(i32, i32)> = (0..HEIGHT)
            .flat_map(|row| (0..WIDTH).map(move |col| (col, row)))
            .filter(|(col, row)| cell(*col, *row) == Some(b':'))
            .collect();
        assert!(!shoals.is_empty(), "the cape has no shoals");
        for (col, row) in &shoals {
            assert!(
                !is_sea(col + 2, *row) || cell(col + 2, *row) == Some(b':'),
                "the shoal at ({col}, {row}) has open water behind it rather than the cape"
            );
        }
        let rear = COMBINED.last().unwrap().at;
        assert!(
            shoals.iter().any(|(col, _)| *col > rear.0),
            "the shoals should lie to leeward of the Combined Fleet's rear"
        );
    }

    /// The whole scenario, built the way the lobby builds it: sixty ships of
    /// the line on the water, twenty-seven to twenty-three, each one on the
    /// tile its table names. This is the test the tables above exist to
    /// support — they describe a battle, and this is the battle arriving.
    #[test]
    fn the_two_fleets_reach_the_board_ship_for_ship() {
        let game = battle(1_805);
        for (pid, fleet) in [(0usize, &BRITISH[..]), (1, &COMBINED[..])] {
            for Ship { at: (col, row), name: ship, .. } in fleet {
                let pos = hex::offset_to_axial(*col, *row);
                let standing = game.units_at(pos);
                assert_eq!(standing.len(), 1, "no ship stands where {ship} should");
                let unit = &game.units[&standing[0]];
                assert_eq!(unit.owner, pid, "{ship} is flying the wrong flag");
                assert_eq!(unit.kind.as_str(), SHIP_OF_THE_LINE, "{ship} is the wrong ship");
            }
        }
        let afloat = |pid: usize| game.player_unit_ids(pid).len();
        assert_eq!(afloat(0), BRITISH.len());
        assert_eq!(afloat(1), COMBINED.len());
    }

    /// A ship's rate reaches the water, and it reaches combat.
    ///
    /// The table and the ladder are both checked above; this is the wiring
    /// between them and the engine. The last assertion is the one that
    /// matters — a promotion the combat layer never reads would pass every
    /// other test in this file and change nothing on the board.
    /// Nine flag officers, where history put them, rated on one scale.
    ///
    /// The counts are the load-bearing part: three British flags against six
    /// in the Combined Fleet is why an admiral's rating is spent on his
    /// fleet's flanking rather than on his own hull — see
    /// `fleet_flanking_bonus_pct`. If a rating is ever edited, this says out
    /// loud that the larger fleet still holds more flags.
    #[test]
    fn the_nine_flag_officers_are_aboard_the_ships_they_flew_in() {
        let flags = |fleet: &'static [Ship]| -> Vec<&'static Ship> {
            fleet.iter().filter(|ship| ship.stars > 0).collect()
        };
        let (british, combined) = (flags(&BRITISH), flags(&COMBINED));
        assert_eq!(british.len(), 3, "Nelson, Collingwood and Northesk");
        assert_eq!(combined.len(), 6, "Villeneuve, Gravina, Alava, Magon, Dumanoir, Cisneros");
        assert!(
            combined.len() > british.len(),
            "the Combined Fleet held more flags, which is the whole reason a \
             per-flagship bonus was rejected"
        );
        for ship in british.iter().chain(combined.iter()) {
            assert!(
                (2..=5).contains(&ship.stars),
                "{} is rated {} stars, off the 2-to-5 scale",
                ship.name,
                ship.stars
            );
        }
        let nelson = commander_in_chief(0);
        let villeneuve = commander_in_chief(1);
        assert!(nelson.name.contains("Nelson") && nelson.stars == 5);
        assert!(villeneuve.name.contains("Villeneuve") && villeneuve.stars == 2);

        // The threshold is what turns more Combined flags into less Combined
        // advantage, so it is asserted as the counts rather than as a rule.
        let worth = |fleet: &'static [Ship]| -> usize {
            fleet.iter().filter(|ship| admiral_formation(ship.stars) > 0).count()
        };
        assert_eq!(worth(&BRITISH), 2, "Nelson and Collingwood");
        assert_eq!(worth(&COMBINED), 1, "Gravina alone");
        assert!(
            worth(&BRITISH) > worth(&COMBINED),
            "the side with fewer flags must still come out ahead on the ones that count"
        );
        assert_eq!(admiral_formation(3), 0, "three stars is not a fighting tier");
        assert_eq!(admiral_formation(4), 1);
        assert_eq!(admiral_formation(5), 1);
    }

    /// The admirals reach the board: every flagship sails a tile further, and
    /// the ones worth a fighting tier fire ten heavier.
    ///
    /// The broadside is fired for real rather than read off a strength
    /// function, for the same reason the rate test does it: this whole feature
    /// once ran through `flanking_bonus`, which the ranged attack path never
    /// consults, and every table-level assertion passed while the board did
    /// not move at all.
    #[test]
    fn an_admirals_stars_reach_his_flagship() {
        let game = battle(1_805);
        let plain = game.rules.units[SHIP_OF_THE_LINE].moves;
        for (pid, fleet) in [(0usize, &BRITISH[..]), (1, &COMBINED[..])] {
            for ship in fleet {
                let uid = game.units_at(hex::offset_to_axial(ship.at.0, ship.at.1))[0];
                let flagship = ship.stars > 0;
                let moves = game.unit_max_moves(uid);
                let expected = plain + if flagship { ADMIRAL_MOVEMENT_BONUS } else { 0.0 };
                assert!(
                    (moves - expected).abs() < 1e-9,
                    "{} (seat {pid}) has {moves} movement, not {expected}",
                    ship.name
                );
                assert_eq!(
                    game.units[&uid].formation,
                    admiral_formation(ship.stars),
                    "{} is in the wrong fighting tier",
                    ship.name
                );
            }
        }

        // Two ships alike in every way but the flag aboard, in separate copies
        // of the same battle, firing on the same enemy from the same water.
        let one_broadside = |stars: u8| -> i32 {
            let mut game = battle(1_805);
            let (from, at) = (hex::offset_to_axial(4, 2), hex::offset_to_axial(5, 2));
            let firing = game.spawn_unit(SHIP_OF_THE_LINE, 0, from);
            let struck = game.spawn_unit(SHIP_OF_THE_LINE, 1, at);
            let unit = game.units.get_mut(&firing).unwrap();
            unit.promotions.extend(
                rate_promotions(100).iter().map(|name| crate::name::Name::new(name)),
            );
            unit.formation = admiral_formation(stars);
            game.apply(0, &crate::game::Action::Ranged { unit: firing, target: at })
                .expect("a ship of the line can fire on an enemy alongside");
            game.units[&struck].hp
        };
        let (nelson, villeneuve) = (one_broadside(5), one_broadside(2));
        assert!(
            nelson < villeneuve,
            "a five-star admiral's ship left {nelson} hit points where a two-star's left \
             {villeneuve}"
        );
        assert_eq!(one_broadside(4), nelson, "four and five stars are the same tier");
        assert_eq!(one_broadside(3), villeneuve, "three stars is not a fighting tier");
    }

    #[test]
    fn a_ship_of_the_line_fights_heavier_than_a_sixty_four() {
        let game = battle(1_805);
        let afloat = |at: (i32, i32)| {
            let standing = game.units_at(hex::offset_to_axial(at.0, at.1));
            &game.units[&standing[0]]
        };
        for ship in BRITISH.iter().chain(COMBINED.iter()) {
            let unit = afloat(ship.at);
            let expected: BTreeSet<String> = rate_promotions(ship.guns)
                .iter()
                .map(|name| (*name).to_string())
                .collect();
            let held: BTreeSet<String> =
                unit.promotions.iter().map(|name| name.to_string()).collect();
            assert_eq!(held, expected, "{} carries the wrong rate", ship.name);
            assert_eq!(unit.level, 1 + expected.len() as i32, "{}'s level", ship.name);
        }

        // Santisima Trinidad, 136, against San Leandro, 64 — the heaviest and
        // the lightest ship in the same line, so nothing but the rate differs.
        let trinidad = afloat(COMBINED[10].at);
        let leandro = afloat(COMBINED[15].at);
        assert!(COMBINED[10].name.starts_with("Santisima Trinidad"));
        assert!(COMBINED[15].name.starts_with("San Leandro"));
        assert_eq!(trinidad.kind, leandro.kind, "the two differ by rate alone");

        // Reach is deliberately equal: every ship on the board fires two
        // tiles, whatever she rates.
        for at in [trinidad.id, leandro.id, afloat(COMBINED[12].at).id] {
            assert_eq!(game.unit_attack_range(at), 2, "reach is not a rate difference here");
        }

        // And the broadside is heavier. Fired for real rather than read off a
        // strength function, because the bonus is applied inside the attack
        // and a promotion the combat layer never consults would satisfy every
        // other assertion in this file while changing nothing on the board.
        //
        // Two identical ships on the same empty water at the top of the chart,
        // in separate copies of the same battle, so the only difference
        // between the two firings is the rate of the ship firing.
        let one_broadside = |rate: u16| -> i32 {
            let mut game = battle(1_805);
            let (from, at) = (hex::offset_to_axial(4, 2), hex::offset_to_axial(5, 2));
            assert!(game.units_at(from).is_empty() && game.units_at(at).is_empty());
            let firing = game.spawn_unit(SHIP_OF_THE_LINE, 0, from);
            let struck = game.spawn_unit(SHIP_OF_THE_LINE, 1, at);
            let promotions: Vec<crate::name::Name> = rate_promotions(rate)
                .iter()
                .map(|name| crate::name::Name::new(name))
                .collect();
            game.units.get_mut(&firing).unwrap().promotions.extend(promotions);
            game.apply(0, &crate::game::Action::Ranged { unit: firing, target: at })
                .expect("a ship of the line can fire on an enemy alongside");
            game.units[&struck].hp
        };
        let (heavy, light) = (one_broadside(74), one_broadside(64));
        assert!(
            heavy < light,
            "a ship of the line's broadside left {heavy} hit points where a 64 left {light}"
        );
    }

    /// Britain moves first. Seat 0 is Britain, seat 0 opens the game, and
    /// nothing about the roster the caller asked for changes either.
    #[test]
    fn britain_holds_the_first_seat_and_the_first_turn() {
        let game = battle(1_805);
        assert_eq!(game.players[0].civ, "England");
        assert_eq!(game.players[1].civ, "France");
        assert_eq!(game.current, 0, "Britain moves first");
        assert_eq!(game.turn, 1);
    }

    /// The seed rolls the map on every other script. Here it must move
    /// nothing: two launches of a scenario are the same battle or the
    /// scenario is not one.
    #[test]
    fn two_seeds_produce_the_same_battle() {
        let first = battle(1_805);
        let second = battle(20_261_021);
        let fleet = |game: &Game| {
            let mut ships: Vec<(usize, (i32, i32))> = game
                .units
                .values()
                .map(|unit| (unit.owner, hex::axial_to_offset(unit.pos.0, unit.pos.1)))
                .collect();
            ships.sort();
            ships
        };
        assert_eq!(fleet(&first), fleet(&second), "the fleets moved with the seed");
        let ground = |game: &Game| {
            let mut chart: Vec<((i32, i32), String, Option<String>)> = game
                .map
                .tiles
                .iter()
                .map(|(pos, tile)| {
                    (
                        hex::axial_to_offset(pos.0, pos.1),
                        tile.terrain.to_string(),
                        tile.feature.map(|feature| feature.to_string()),
                    )
                })
                .collect();
            chart.sort();
            chart
        };
        assert_eq!(ground(&first), ground(&second), "the chart moved with the seed");
    }

    /// The board is the chart: the sea both fleets manoeuvre over is sailable
    /// coast, Cape Trafalgar is dry land, and the shoals are reef. A scenario
    /// that quietly grew ice or an island would still pass every table test
    /// above and be unplayable.
    #[test]
    fn the_board_is_the_chart_that_was_drawn() {
        let game = battle(1_805);
        let (mut sea, mut shore, mut shoal) = (0, 0, 0);
        for row in 0..HEIGHT {
            for col in 0..WIDTH {
                let tile = &game.map.tiles[&hex::offset_to_axial(col, row)];
                let feature = tile.feature.map(|feature| feature.to_string());
                match cell(col, row) {
                    Some(b'~') => {
                        assert_eq!(tile.terrain.as_str(), "coast", "({col}, {row}) is not sea");
                        assert_eq!(feature, None, "({col}, {row}) grew something in open water");
                        sea += 1;
                    }
                    Some(b':') => {
                        assert_eq!(tile.terrain.as_str(), "coast");
                        assert_eq!(feature.as_deref(), Some("reef"), "({col}, {row}) is not shoal");
                        shoal += 1;
                    }
                    _ => {
                        assert!(
                            !game.rules.is_water(tile),
                            "({col}, {row}) should be the Andalusian shore"
                        );
                        shore += 1;
                    }
                }
            }
        }
        assert_eq!(sea + shoal + shore, WIDTH * HEIGHT);
        assert!(shoal > 0 && shore > 0 && sea > shore * 8, "{sea} sea, {shoal} shoal, {shore} shore");
    }

    /// A scenario overrules the arena economy it was handed. Nothing was
    /// produced, bought, researched or defended at Trafalgar, and a city or a
    /// flag on this water would be a different battle — so the request the
    /// fixture makes for all of them is refused, and the two things that are
    /// only about how long you want to play survive it.
    #[test]
    fn the_scenario_fixes_the_economy_it_is_fought_under() {
        let game = battle(1_805);
        assert_eq!(game.tactics.cities, 0);
        assert_eq!(game.tactics.production, 0);
        assert_eq!(game.tactics.gold, 0);
        assert_eq!(game.tactics.turns_per_tech, 0);
        assert!(!game.tactics.unique_units);
        assert!(!game.tactics.flag);
        assert!(!game.tactics.fog);
        assert!(game.cities.is_empty(), "no city stands on this water");
        assert!(game.arena_flags.is_empty(), "no flag is planted on this water");
        // Still the caller's to choose, because neither is a claim about 1805.
        assert_eq!(game.tactics.turn_limit, TacticsRules::default().turn_limit);
        assert_eq!(game.tactics.best_of, TacticsRules::default().best_of);
        // A Tactics map seats no city-states whatever the request asked for.
        // The dormant Free Cities seat every world reserves is not one.
        assert_eq!(
            game.players
                .iter()
                .filter(|player| player.is_minor && !player.is_free_city)
                .count(),
            0
        );
    }

    /// The two sides are at war and know it, and every ship can reach the
    /// fighting. A fleet that opened at peace, or one walled off from the
    /// enemy by unsailable water, would sit still for a hundred turns and
    /// report a draw.
    #[test]
    fn both_fleets_open_at_war_with_the_water_between_them_sailable() {
        let game = battle(1_805);
        assert!(game.is_at_war(0, 1), "the fleets must open in action");
        let victory = game.units_at(hex::offset_to_axial(FLAGSHIPS[0].0, FLAGSHIPS[0].1))[0];
        let bucentaure = hex::offset_to_axial(FLAGSHIPS[1].0, FLAGSHIPS[1].1);
        assert!(
            game.route_step(victory, bucentaure, 0).is_some(),
            "Victory cannot bear down on Bucentaure"
        );
        // And the far corners of the action are connected too: Africa is off
        // on her own in the north and has to come the length of the line.
        let africa_at = BRITISH.last().unwrap().at;
        let africa = game.units_at(hex::offset_to_axial(africa_at.0, africa_at.1))[0];
        let rear = COMBINED.last().unwrap().at;
        assert!(
            game.route_step(africa, hex::offset_to_axial(rear.0, rear.1), 0).is_some(),
            "Africa cannot reach the enemy rear"
        );
    }
}
