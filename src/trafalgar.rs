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
//!   64-gun *Africa* are both it. Rate, gunnery and the Royal Navy's rate of
//!   fire — the things that actually decided the exchange once the lines were
//!   locked — are not modelled, so what the board tests is the part Nelson
//!   could choose: where sixty ships were, and what was done with them.
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
pub const FLAGSHIPS: [(i32, i32); 2] = [BRITISH[0].0, COMBINED[11].0];

/// One ship: where it starts, and what it was.
///
/// The name carries the rate and, where it matters, whose flag was in her. It
/// is documentation rather than data — the engine has no per-unit name — but
/// it is what makes the two tables below auditable against a line of battle
/// instead of being sixty coordinates nobody can check.
type Ship = ((i32, i32), &'static str);

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
    ((16, 10), "Victory, 100 — Nelson / Hardy"),
    ((15, 10), "Temeraire, 98"),
    ((14, 10), "Neptune, 98"),
    ((13, 10), "Conqueror, 74"),
    ((12, 10), "Leviathan, 74"),
    ((11, 10), "Ajax, 74"),
    ((10, 10), "Orion, 74"),
    ((9, 10), "Agamemnon, 64"),
    ((8, 10), "Minotaur, 74"),
    ((7, 10), "Spartiate, 74"),
    ((6, 10), "Britannia, 100 — Rear-Admiral Northesk"),
    // --- Lee column. Vice-Admiral Collingwood, in Royal Sovereign.
    ((17, 15), "Royal Sovereign, 100 — Collingwood"),
    ((16, 15), "Belleisle, 74"),
    ((15, 15), "Mars, 74"),
    ((14, 15), "Tonnant, 80"),
    ((13, 15), "Bellerophon, 74"),
    ((12, 15), "Colossus, 74"),
    ((11, 15), "Achille, 74"),
    ((10, 15), "Polyphemus, 64"),
    ((9, 15), "Revenge, 74"),
    ((8, 15), "Swiftsure, 74"),
    ((7, 15), "Defiance, 74"),
    ((6, 15), "Thunderer, 74"),
    ((5, 15), "Defence, 74"),
    ((4, 15), "Prince, 98"),
    ((3, 15), "Dreadnought, 98"),
    // --- Detached, and a long way from help.
    ((16, 1), "Africa, 64 — separated in the night, north of the van"),
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
    ((19, 1), "Neptuno, 80 (Sp)"),
    ((19, 2), "Scipion, 74 (Fr)"),
    ((20, 3), "Intrepide, 74 (Fr)"),
    ((20, 4), "Rayo, 100 (Sp)"),
    ((20, 5), "Formidable, 80 (Fr) — Dumanoir le Pelley"),
    ((21, 6), "Duguay-Trouin, 74 (Fr)"),
    ((21, 7), "Mont-Blanc, 74 (Fr)"),
    ((21, 8), "San Francisco de Asis, 74 (Sp)"),
    ((21, 9), "San Agustin, 74 (Sp)"),
    // --- Centre. Vice-Admiral Villeneuve, in Bucentaure.
    ((21, 10), "Heros, 74 (Fr)"),
    ((22, 10), "Santisima Trinidad, 136 (Sp) — Rear-Admiral Cisneros"),
    ((21, 11), "Bucentaure, 80 (Fr) — Villeneuve, commander-in-chief"),
    ((22, 11), "Redoutable, 74 (Fr) — Lucas"),
    ((21, 12), "San Justo, 74 (Sp)"),
    ((22, 12), "Neptune, 80 (Fr)"),
    ((21, 13), "San Leandro, 64 (Sp)"),
    ((22, 13), "Santa Ana, 112 (Sp) — Vice-Admiral Alava"),
    ((21, 14), "Indomptable, 80 (Fr)"),
    ((22, 14), "Fougueux, 74 (Fr)"),
    // --- Rear, and the squadron of observation to leeward of it.
    ((21, 15), "Monarca, 74 (Sp)"),
    ((22, 15), "Pluton, 74 (Fr)"),
    ((21, 16), "Algesiras, 74 (Fr) — Rear-Admiral Magon"),
    ((22, 16), "Bahama, 74 (Sp)"),
    ((20, 17), "Aigle, 74 (Fr)"),
    ((21, 17), "Swiftsure, 74 (Fr)"),
    ((20, 18), "Montanes, 74 (Sp)"),
    ((21, 18), "Argonaute, 74 (Fr)"),
    ((20, 19), "Argonauta, 80 (Sp)"),
    ((21, 19), "San Ildefonso, 74 (Sp)"),
    ((20, 20), "Achille, 74 (Fr)"),
    ((21, 20), "Principe de Asturias, 112 (Sp) — Admiral Gravina"),
    ((19, 21), "Berwick, 74 (Fr)"),
    ((19, 22), "San Juan Nepomuceno, 74 (Sp)"),
];

/// What every ship on the board is built as.
///
/// The Frigate is the ruleset's Renaissance sailing warship — Square Rigging,
/// four movement, and a two-tile broadside it fires without closing — and it
/// is the only thing in the roster a ship of the line can be. See the module
/// header for what that abstraction costs.
pub const SHIP_OF_THE_LINE: &str = "frigate";

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

    /// Every ship is on open water, on the chart, and alone on her tile. Only
    /// one military unit stands on a hex, so a duplicate coordinate would
    /// silently drop a ship from the board — and a ship placed on the shore or
    /// on the shoals could not be there at all.
    #[test]
    fn every_ship_is_afloat_and_on_her_own_tile() {
        let mut taken: BTreeSet<(i32, i32)> = BTreeSet::new();
        for ((col, row), ship) in BRITISH.iter().chain(COMBINED.iter()) {
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
        assert_eq!(FLAGSHIPS[0], BRITISH[0].0);
        assert!(BRITISH[0].1.starts_with("Victory"));
        assert_eq!(FLAGSHIPS[1], COMBINED[11].0);
        assert!(COMBINED[11].1.starts_with("Bucentaure"));
    }

    /// The two British columns start west of the Combined Fleet's line, and
    /// clear of it. A scenario whose fleets began interleaved would open with
    /// the approach already over — and the approach is the battle.
    #[test]
    fn the_british_are_to_windward_with_water_still_to_cross() {
        let van_and_line = COMBINED.iter().map(|((col, _), _)| *col).min().unwrap();
        let leading_briton = BRITISH.iter().map(|((col, _), _)| *col).max().unwrap();
        assert!(
            leading_briton < van_and_line,
            "the leading British ship is at column {leading_briton}, not west of the \
             Combined Fleet's nearest ship at {van_and_line}"
        );
        // Collingwood beat Nelson into action, so his column starts nearer.
        let royal_sovereign = BRITISH[11];
        assert!(royal_sovereign.1.starts_with("Royal Sovereign"));
        assert!(
            royal_sovereign.0 .0 > BRITISH[0].0 .0,
            "Royal Sovereign should start ahead of Victory"
        );
    }

    /// The Combined Fleet's line has to be sailable along its whole length, or
    /// the crescent is really two fleets with a wall between them.
    #[test]
    fn the_enemy_line_stands_in_open_water_from_van_to_rear() {
        let rows: Vec<i32> = COMBINED.iter().map(|((_, row), _)| *row).collect();
        let (top, bottom) = (*rows.iter().min().unwrap(), *rows.iter().max().unwrap());
        assert!(bottom - top >= 20, "the line should span most of the map north to south");
        for row in top..=bottom {
            assert!(
                COMBINED.iter().any(|((_, at), _)| *at == row),
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
        let rear = COMBINED.last().unwrap().0;
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
            for ((col, row), ship) in fleet {
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
        let africa_at = BRITISH.last().unwrap().0;
        let africa = game.units_at(hex::offset_to_axial(africa_at.0, africa_at.1))[0];
        let rear = COMBINED.last().unwrap().0;
        assert!(
            game.route_step(africa, hex::offset_to_axial(rear.0, rear.1), 0).is_some(),
            "Africa cannot reach the enemy rear"
        );
    }
}
