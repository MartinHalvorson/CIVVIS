//! Descriptive census of legal Faith outlets in completed production saves.
//!
//! This command changes no saved file and runs no controller. For each living
//! major it clones the terminal position, removes only the game-over guard,
//! makes that seat current, and enumerates already-legal purchase and empire
//! actions. `docs/TERMINAL_FAITH_OPPORTUNITIES.md` freezes the interpretation:
//! an offer is an opportunity for a future causal study, not evidence that the
//! action should have been taken.
use civvis::game::{Action, ActionFamilies, Game};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DEFAULT_DIR: &str = "target/spectator/results";
const DEFAULT_LATEST: usize = 50;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn selected_saves(
    dir: &Path,
    latest: usize,
    through: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|error| format!("read {}: {error}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.ends_with(".save.json") && through.is_none_or(|cutoff| name <= cutoff)
                })
        })
        .collect();
    paths.sort_by(|left, right| right.file_name().cmp(&left.file_name()));
    paths.truncate(latest);
    Ok(paths)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FaithClass {
    CultureAsset,
    ReligiousUnit,
    MilitaryUnit,
    OtherUnit,
    Building,
    District,
    GreatPerson,
    Pantheon,
}

impl FaithClass {
    const ALL: [FaithClass; 8] = [
        FaithClass::CultureAsset,
        FaithClass::ReligiousUnit,
        FaithClass::MilitaryUnit,
        FaithClass::OtherUnit,
        FaithClass::Building,
        FaithClass::District,
        FaithClass::GreatPerson,
        FaithClass::Pantheon,
    ];

    fn label(self) -> &'static str {
        match self {
            FaithClass::CultureAsset => "Naturalist/Rock Band",
            FaithClass::ReligiousUnit => "religious unit",
            FaithClass::MilitaryUnit => "military unit",
            FaithClass::OtherUnit => "other unit",
            FaithClass::Building => "building",
            FaithClass::District => "district",
            FaithClass::GreatPerson => "Great Person",
            FaithClass::Pantheon => "Pantheon",
        }
    }
}

/// Class and exact subtype. The subtype is retained only for unit and Great
/// Person coverage, where one repeated item could otherwise masquerade as a
/// broad opportunity family.
fn classify_faith_action(game: &Game, action: &Action) -> Option<(FaithClass, Option<String>)> {
    match action {
        Action::Buy { unit, currency, .. } if currency == "faith" => {
            let unit_name = unit.as_str();
            let class = if matches!(unit_name, "naturalist" | "rock_band") {
                FaithClass::CultureAsset
            } else {
                match game.rules.units.get_interned(*unit)?.class.as_str() {
                    "religious" => FaithClass::ReligiousUnit,
                    "military" => FaithClass::MilitaryUnit,
                    _ => FaithClass::OtherUnit,
                }
            };
            Some((class, Some(format!("unit:{unit_name}"))))
        }
        Action::BuyBuilding { currency, .. } if currency == "faith" => {
            Some((FaithClass::Building, None))
        }
        Action::BuyDistrict { currency, .. } if currency == "faith" => {
            Some((FaithClass::District, None))
        }
        Action::PatronizeGreatPerson { kind, currency } if currency == "faith" => Some((
            FaithClass::GreatPerson,
            Some(format!("great_person:{kind}")),
        )),
        Action::ChoosePantheon { .. } => Some((FaithClass::Pantheon, None)),
        _ => None,
    }
}

#[derive(Default)]
struct SeatReading {
    faith: f64,
    won: bool,
    blocked: bool,
    offers: BTreeMap<FaithClass, usize>,
    subtypes: BTreeMap<String, usize>,
}

fn read_seat(game: &Game, pid: usize) -> SeatReading {
    let mut probe = game.clone();
    probe.winner = None;
    probe.victory_type = None;
    probe.current = pid;
    let actions =
        probe.legal_actions_within(pid, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE);
    let blocked = actions.iter().any(|action| {
        matches!(
            action,
            Action::KeepCity { .. } | Action::RazeCity { .. } | Action::LiberateCity { .. }
        )
    });
    let mut reading = SeatReading {
        faith: game.players[pid].faith,
        won: game.winner == Some(pid),
        blocked,
        ..SeatReading::default()
    };
    if blocked {
        return reading;
    }
    for action in &actions {
        let Some((class, subtype)) = classify_faith_action(&probe, action) else {
            continue;
        };
        *reading.offers.entry(class).or_default() += 1;
        if let Some(subtype) = subtype {
            *reading.subtypes.entry(subtype).or_default() += 1;
        }
    }
    reading
}

#[derive(Default)]
struct Coverage {
    offers: usize,
    seats: usize,
    seats_2k: usize,
    seats_5k: usize,
}

impl Coverage {
    fn record(&mut self, offers: usize, faith: f64) {
        self.offers += offers;
        self.seats += 1;
        self.seats_2k += (faith >= 2_000.0) as usize;
        self.seats_5k += (faith >= 5_000.0) as usize;
    }
}

fn percentile(values: &[f64], proportion: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let index = ((sorted.len() as f64 * proportion).ceil() as usize)
        .saturating_sub(1)
        .min(sorted.len() - 1);
    sorted[index]
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

fn file_label(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("<non-UTF8 filename>")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = PathBuf::from(text(&args, "--dir", DEFAULT_DIR));
    let latest = number(&args, "--latest", DEFAULT_LATEST).max(1);
    let through = args
        .iter()
        .position(|arg| arg == "--through")
        .and_then(|index| args.get(index + 1))
        .cloned();
    let paths = selected_saves(&dir, latest, through.as_deref()).unwrap_or_else(|why| {
        eprintln!("terminal_faith_census: {why}");
        std::process::exit(2);
    });
    if paths.is_empty() {
        eprintln!(
            "terminal_faith_census: no *.save.json files in {}",
            dir.display()
        );
        std::process::exit(2);
    }

    println!("Terminal Faith opportunity census");
    println!(
        "selection: newest {} through oldest {} (requested latest {}; cutoff {})",
        file_label(&paths[0]),
        file_label(paths.last().unwrap()),
        latest,
        through.as_deref().unwrap_or("none"),
    );

    let mut parse_failures = Vec::new();
    let mut games = 0usize;
    let mut victory_mix: BTreeMap<String, usize> = BTreeMap::new();
    let mut seats = Vec::new();
    for path in &paths {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(error) => {
                parse_failures.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        let game: Game = match serde_json::from_str(&raw) {
            Ok(game) => game,
            Err(error) => {
                parse_failures.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        games += 1;
        *victory_mix
            .entry(
                game.victory_type
                    .clone()
                    .unwrap_or_else(|| "none".to_string()),
            )
            .or_default() += 1;
        for pid in 0..game.players.len() {
            let player = &game.players[pid];
            if player.alive && !player.is_minor && !player.is_barbarian && !player.is_free_city {
                seats.push(read_seat(&game, pid));
            }
        }
    }

    let blocked = seats.iter().filter(|seat| seat.blocked).count();
    let unblocked: Vec<&SeatReading> = seats.iter().filter(|seat| !seat.blocked).collect();
    let faith: Vec<f64> = seats.iter().map(|seat| seat.faith).collect();
    let mean = faith.iter().sum::<f64>() / faith.len().max(1) as f64;
    let median = median(&faith);
    let p90 = percentile(&faith, 0.9);
    let at_2k = seats.iter().filter(|seat| seat.faith >= 2_000.0).count();
    let at_5k = seats.iter().filter(|seat| seat.faith >= 5_000.0).count();
    let at_10k = seats.iter().filter(|seat| seat.faith >= 10_000.0).count();
    let winners = seats.iter().filter(|seat| seat.won).count();
    let unblocked_2k = unblocked
        .iter()
        .filter(|seat| seat.faith >= 2_000.0)
        .count();
    let unblocked_5k = unblocked
        .iter()
        .filter(|seat| seat.faith >= 5_000.0)
        .count();

    println!(
        "read: {games}/{} games, {} parse failures; victory mix {:?}",
        paths.len(),
        parse_failures.len(),
        victory_mix
    );
    println!(
        "seats: {} surviving majors, {winners} winners, {blocked} capture-blocked, {} opportunity-eligible",
        seats.len(),
        unblocked.len()
    );
    println!(
        "Faith: mean {mean:.1}, median {median:.1}, p90 {p90:.1}; >=2,000 {at_2k}, >=5,000 {at_5k}, >=10,000 {at_10k}"
    );
    println!();
    println!("class                    offers  seats/all       seats/>=2k      seats/>=5k");

    let mut class_coverage: BTreeMap<FaithClass, Coverage> = BTreeMap::new();
    let mut subtype_coverage: BTreeMap<String, Coverage> = BTreeMap::new();
    for seat in &unblocked {
        for (class, offers) in &seat.offers {
            class_coverage
                .entry(*class)
                .or_default()
                .record(*offers, seat.faith);
        }
        for (subtype, offers) in &seat.subtypes {
            subtype_coverage
                .entry(subtype.clone())
                .or_default()
                .record(*offers, seat.faith);
        }
    }
    for class in FaithClass::ALL {
        let coverage = class_coverage.entry(class).or_default();
        println!(
            "{:<24} {:>6}  {:>4}/{:<4} {:>6.1}%  {:>4}/{:<4} {:>6.1}%  {:>4}/{:<4} {:>6.1}%",
            class.label(),
            coverage.offers,
            coverage.seats,
            unblocked.len(),
            coverage.seats as f64 * 100.0 / unblocked.len().max(1) as f64,
            coverage.seats_2k,
            unblocked_2k,
            coverage.seats_2k as f64 * 100.0 / unblocked_2k.max(1) as f64,
            coverage.seats_5k,
            unblocked_5k,
            coverage.seats_5k as f64 * 100.0 / unblocked_5k.max(1) as f64,
        );
    }

    let no_offer_2k = unblocked
        .iter()
        .filter(|seat| seat.faith >= 2_000.0 && seat.offers.is_empty())
        .count();
    let no_offer_5k = unblocked
        .iter()
        .filter(|seat| seat.faith >= 5_000.0 && seat.offers.is_empty())
        .count();
    println!();
    println!(
        "rich seats with no legal Faith action: >=2,000 {no_offer_2k}/{unblocked_2k}; >=5,000 {no_offer_5k}/{unblocked_5k}"
    );
    println!();
    println!("exact unit / Great Person subtype coverage:");
    for (subtype, coverage) in &subtype_coverage {
        println!(
            "  {subtype:<30} offers {:>5}; seats {:>4}, >=2k {:>4}, >=5k {:>4}",
            coverage.offers, coverage.seats, coverage.seats_2k, coverage.seats_5k
        );
    }

    if !parse_failures.is_empty() {
        eprintln!();
        eprintln!("parse/read failures:");
        for failure in &parse_failures {
            eprintln!("  {failure}");
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civvis::name::Name;
    use std::collections::BTreeSet;

    #[test]
    fn faith_actions_have_one_prospective_class() {
        let game = Game::new(1, 20, 14, 80_001, 100, 0);
        let cases = [
            (
                Action::Buy {
                    city: 1,
                    unit: Name::new("rock_band"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
                Some(FaithClass::CultureAsset),
            ),
            (
                Action::Buy {
                    city: 1,
                    unit: Name::new("missionary"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
                Some(FaithClass::ReligiousUnit),
            ),
            (
                Action::Buy {
                    city: 1,
                    unit: Name::new("warrior"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
                Some(FaithClass::MilitaryUnit),
            ),
            (
                Action::Buy {
                    city: 1,
                    unit: Name::new("builder"),
                    formation: 0,
                    currency: "faith".to_string(),
                },
                Some(FaithClass::OtherUnit),
            ),
            (
                Action::PatronizeGreatPerson {
                    kind: "scientist".to_string(),
                    currency: "faith".to_string(),
                },
                Some(FaithClass::GreatPerson),
            ),
            (
                Action::Buy {
                    city: 1,
                    unit: Name::new("warrior"),
                    formation: 0,
                    currency: "gold".to_string(),
                },
                None,
            ),
        ];
        for (action, expected) in cases {
            assert_eq!(
                classify_faith_action(&game, &action).map(|(class, _)| class),
                expected,
                "{action:?}"
            );
        }
    }

    fn faith_signatures(game: &Game, pid: usize) -> BTreeSet<String> {
        game.legal_actions_within(pid, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE)
            .into_iter()
            .filter(|action| classify_faith_action(game, action).is_some())
            .map(|action| format!("{action:?}"))
            .collect()
    }

    #[test]
    fn clearing_only_terminal_guards_recovers_the_live_faith_action_set() {
        let mut live = Game::new(1, 20, 14, 80_002, 100, 0);
        live.players[0].faith = 10_000.0;
        let expected = faith_signatures(&live, 0);
        assert!(!expected.is_empty(), "fixture needs a legal Faith action");

        let mut terminal = live.clone();
        terminal.winner = Some(0);
        terminal.victory_type = Some("science".to_string());
        assert!(faith_signatures(&terminal, 0).is_empty());

        terminal.winner = None;
        terminal.victory_type = None;
        terminal.current = 0;
        assert_eq!(faith_signatures(&terminal, 0), expected);
    }

    #[test]
    fn median_averages_the_two_middle_seats() {
        assert_eq!(median(&[9.0, 1.0, 5.0]), 5.0);
        assert_eq!(median(&[9.0, 1.0, 5.0, 3.0]), 4.0);
    }
}
