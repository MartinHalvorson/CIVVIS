//! Is an ancient rush geometrically possible in this engine, and against what?
//!
//! The repository has measured, repeatedly, that this agent's wars take
//! nothing: 0.33 cities a game, no capital ever, no peace treaty. Every one of
//! those measurements is of a *late* war. Nothing has asked the prior
//! question — whether a **turn-50 kill** is reachable at all, and what the
//! defender actually looks like when the window is open.
//!
//! Three things decide that, and this measures all three per game:
//!
//! - **Reach.** How far is the nearest rival capital, in turns of marching?
//!   A rush that cannot arrive before turn 50 is not a strategy, it is a wish.
//! - **The wall clock.** `walls` costs 80 production and the AI only
//!   prioritises it once `threatened` fires — hostile units within 6 tiles
//!   *while already at war*. So the question is when walls actually appear,
//!   not when they could.
//! - **The garrison.** `city_strength` is driven by `strongest_unit_built`
//!   and whatever sits on the tile. A capital defended by a warrior is a
//!   different problem from one defended by a spearman on hills.
//!
//! The reading that says "build the rush" is: neighbours inside ~10 turns of
//! march, walls arriving after turn 50, and garrisons at warrior tier. The
//! reading that kills it is walls up by turn 30, or nearest neighbours 20
//! turns away.
//!
//! ```text
//! rush_census --players 6 --maps 12 --turns 120
//! ```
//!
//! Diagnostic only: it never changes a decision, and no agent can name it.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;
use civvis::Pos;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Turns the census reports a snapshot at.
const MARKS: [u32; 6] = [20, 30, 40, 50, 60, 80];

/// A land unit of the rush era covers this much ground a turn, allowing for
/// terrain. Horsemen have 4 movement and chariots 3, but neither walks a
/// straight line across hills and forest.
const MARCH_TILES_PER_TURN: f64 = 1.5;

#[derive(Clone, Default)]
struct Snapshot {
    /// Capitals whose owner has any wall building standing.
    walled_capitals: usize,
    /// Capitals at all, i.e. living majors.
    capitals: usize,
    /// Sum over capitals of `city_strength`.
    capital_strength: f64,
    /// Strongest garrison melee strength found on a capital tile.
    garrison_strength: f64,
    /// Military units per living major.
    military: f64,
    /// Melee-capable units per living major — the only ones that can seal a
    /// siege ring or walk into a depleted city.
    melee: f64,
    /// Melee-capable units standing within 5 tiles of the nearest rival
    /// capital, i.e. the stack the declaration gate actually counts.
    staged: f64,
    /// Melee standing on a ring tile of the nearest rival capital.
    adjacent: f64,
    /// The largest stack any one civilization has on one rival capital's ring.
    /// Averages hide this: only about one seat in six is rushing at a time, so
    /// a per-civilization mean divides a real siege by the five empires not
    /// conducting one.
    max_adjacent: f64,
    /// The same for the 5-tile staging ring.
    max_staged: f64,
    /// Majors at war with another major.
    at_war: usize,
    /// Majors holding each rush technology.
    horseback: usize,
    iron: usize,
    masonry: usize,
    /// Majors holding `craftsmanship` (agoge) and `political_philosophy`
    /// (oligarchy, +4 combat strength).
    craftsmanship: usize,
    political_philosophy: usize,
}

#[derive(Clone, Default)]
struct MapResult {
    /// Per mark, the snapshot.
    marks: Vec<Snapshot>,
    /// Tile distance from each major's capital to the nearest rival capital,
    /// measured on the turn everyone has founded.
    nearest_rival: Vec<i32>,
    /// Turn of the first war between majors, if any. Splits the delay into
    /// build-and-march (before) and siege (after).
    first_war: Option<u32>,
    /// Turn of the first city capture between majors, if any.
    first_capture: Option<u32>,
    /// Turn of the first major eliminated, if any.
    first_elimination: Option<u32>,
    /// Majors still alive at turn 50 and at the end.
    alive_at_50: usize,
    alive_at_end: usize,
    majors: usize,
    /// Per rushing campaign: (declared, first capture from this victim,
    /// victim eliminated). Splits the delay into approach, siege and mop-up
    /// instead of leaving it to a game-level first-war/first-capture pair that
    /// need not even describe the same campaign.
    campaigns: Vec<(u32, Option<u32>, Option<u32>)>,
    /// Blows landed on cities by turn 60 — whether the rush stack, having
    /// declared and stood 3-5 tiles out, ever actually attacks the capital.
    blows_by_60: u64,
    damage_by_60: i64,
}

fn capital_of(g: &Game, pid: usize) -> Option<Pos> {
    g.player_city_ids(pid)
        .into_iter()
        .filter_map(|cid| g.cities.get(&cid))
        .find(|city| city.is_capital)
        .map(|city| city.pos)
}

fn snapshot(g: &Game, majors: &[usize]) -> Snapshot {
    let mut s = Snapshot::default();
    for pid in majors.iter().copied() {
        if !g.players[pid].alive {
            continue;
        }
        s.capitals += 1;
        let player = &g.players[pid];
        if player.techs.contains(&civvis::name!("horseback_riding")) {
            s.horseback += 1;
        }
        if player.techs.contains(&civvis::name!("iron_working")) {
            s.iron += 1;
        }
        if player.techs.contains(&civvis::name!("masonry")) {
            s.masonry += 1;
        }
        if player.civics.contains(&civvis::name!("craftsmanship")) {
            s.craftsmanship += 1;
        }
        if player.civics.contains(&civvis::name!("political_philosophy")) {
            s.political_philosophy += 1;
        }
        s.military += g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                g.units
                    .get(uid)
                    .is_some_and(|u| g.rules.units[u.kind].class == "military")
            })
            .count() as f64;
        let melee: Vec<Pos> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| g.units.get(&uid))
            .filter(|u| g.rules.units[u.kind].is_melee_capable())
            .map(|u| u.pos)
            .collect();
        s.melee += melee.len() as f64;
        if majors.iter().any(|other| *other != pid && g.is_at_war(pid, *other)) {
            s.at_war += 1;
        }
        // Measured against **every** rival major capital, not the nearest.
        // `early_rush_victim` skips friends and walled capitals, so the city
        // the column is actually marching on is frequently not the closest
        // one — scoring only the nearest reports an empty staging ring for an
        // army standing on a different capital entirely.
        let rival_capitals: Vec<Pos> = majors
            .iter()
            .copied()
            .filter(|other| *other != pid)
            .filter(|other| g.players[*other].alive)
            .filter_map(|other| {
                g.player_city_ids(other)
                    .into_iter()
                    .find(|cid| g.cities[cid].is_capital)
                    .map(|cid| g.cities[&cid].pos)
            })
            .collect();
        let nearest_capital = |pos: Pos| {
            rival_capitals
                .iter()
                .map(|capital| g.wdist(pos, *capital))
                .min()
                .unwrap_or(i32::MAX)
        };
        s.staged += melee.iter().filter(|pos| nearest_capital(**pos) <= 5).count() as f64;
        // Per-capital, so a stack split across two objectives is not credited
        // as one siege.
        for capital in rival_capitals.iter() {
            let ring = melee.iter().filter(|pos| g.wdist(**pos, *capital) <= 1).count() as f64;
            let near = melee.iter().filter(|pos| g.wdist(**pos, *capital) <= 5).count() as f64;
            s.max_adjacent = s.max_adjacent.max(ring);
            s.max_staged = s.max_staged.max(near);
        }
        // Adjacency is the one that matters: a melee unit on a ring tile can
        // attack the city this turn and contributes to sealing the siege ring
        // that stops it healing 20 HP a turn.
        s.adjacent += melee.iter().filter(|pos| nearest_capital(**pos) <= 1).count() as f64;

        let Some(cid) = g
            .player_city_ids(pid)
            .into_iter()
            .find(|cid| g.cities[cid].is_capital)
        else {
            continue;
        };
        let city = &g.cities[&cid];
        if city
            .buildings
            .iter()
            .any(|b| g.rules.buildings[b].outer_defense > 0)
        {
            s.walled_capitals += 1;
        }
        s.capital_strength += g.city_strength(cid);
        let garrison = g
            .units_at(city.pos)
            .into_iter()
            .filter_map(|uid| {
                let u = &g.units[&uid];
                (u.owner == pid && g.rules.units[u.kind].class == "military")
                    .then(|| g.rules.units[u.kind].strength)
            })
            .fold(0.0, f64::max);
        s.garrison_strength += garrison;
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 6);
    let maps = number(&args, "--maps", 12);
    let turns = number(&args, "--turns", 120) as u32;
    let width = number(&args, "--width", 74) as i32;
    let height = number(&args, "--height", 46) as i32;
    let seed0 = number(&args, "--seed", 900_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs().min(6));
    // With `--rush` every seat plays the ancient-rush lane, which is what
    // makes the two runs comparable: the same maps, the same seats, one
    // behaviour changed.
    let rush = args.iter().any(|arg| arg == "--rush");

    println!(
        "rush_census: {maps} maps, {players}p {width}x{height}, {turns} turns, seed {seed0}, \
         agent {}",
        if rush { "advanced_rush" } else { "advanced" }
    );

    let per_map = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, 0);
        let stock = Weights::default();
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &stock);
        if rush {
            for ai in fleet.iter_mut() {
                ai.early_rush = true;
            }
        }
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor)
            .collect();
        let mut result = MapResult {
            majors: majors.len(),
            ..Default::default()
        };
        let mut war_seen: Vec<(usize, usize)> = Vec::new();
        let mut prev_dead = 0usize;
        let mut prev_owned: Vec<usize> = majors
            .iter()
            .map(|pid| game.player_city_ids(*pid).len())
            .collect();
        let mut cities_owned: Vec<usize> = majors
            .iter()
            .map(|pid| game.player_city_ids(*pid).len())
            .collect();

        for turn in 0..turns {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                fleet[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }

            // Capital separations, once everyone has actually founded.
            if result.nearest_rival.is_empty()
                && majors.iter().all(|pid| capital_of(&game, *pid).is_some())
            {
                for pid in majors.iter().copied() {
                    let Some(mine) = capital_of(&game, pid) else {
                        continue;
                    };
                    let nearest = majors
                        .iter()
                        .copied()
                        .filter(|other| *other != pid)
                        .filter_map(|other| capital_of(&game, other))
                        .map(|theirs| game.wdist(mine, theirs))
                        .min();
                    if let Some(d) = nearest {
                        result.nearest_rival.push(d);
                    }
                }
            }

            // Open a campaign record the turn a major war appears; close its
            // stages as they happen.
            for (ai, a) in majors.iter().enumerate() {
                for b in majors.iter().skip(ai + 1) {
                    if !game.is_at_war(*a, *b) {
                        continue;
                    }
                    let known = war_seen.iter().any(|(x, y)| x == a && y == b);
                    if !known {
                        war_seen.push((*a, *b));
                        result.campaigns.push((turn, None, None));
                    }
                }
            }
            // A city changed major hands, or a major died: attribute to the
            // most recent open campaign, which for a rush is the only one.
            {
                let owners_now: Vec<usize> = majors
                    .iter()
                    .map(|pid| game.player_city_ids(*pid).len())
                    .collect();
                let lost = owners_now.iter().zip(prev_owned.iter()).any(|(a, b)| a < b);
                let gained = owners_now.iter().zip(prev_owned.iter()).any(|(a, b)| a > b);
                if lost && gained {
                    if let Some(last) = result.campaigns.last_mut() {
                        if last.1.is_none() {
                            last.1 = Some(turn);
                        }
                    }
                }
                prev_owned = owners_now;
                let dead = majors.iter().filter(|pid| !game.players[**pid].alive).count();
                if dead > prev_dead {
                    if let Some(last) = result.campaigns.last_mut() {
                        if last.2.is_none() {
                            last.2 = Some(turn);
                        }
                    }
                    prev_dead = dead;
                }
            }

            if result.first_war.is_none()
                && majors.iter().enumerate().any(|(i, a)| {
                    majors.iter().skip(i + 1).any(|b| game.is_at_war(*a, *b))
                })
            {
                result.first_war = Some(turn);
            }

            // First capture between majors: a major's city count fell while
            // another's rose. Barbarians never hold cities, so a drop that is
            // matched by a rise is a capture.
            let now: Vec<usize> = majors
                .iter()
                .map(|pid| game.player_city_ids(*pid).len())
                .collect();
            if result.first_capture.is_none()
                && now.iter().zip(cities_owned.iter()).any(|(a, b)| a < b)
                && now.iter().zip(cities_owned.iter()).any(|(a, b)| a > b)
            {
                result.first_capture = Some(turn);
            }
            cities_owned = now;

            if result.first_elimination.is_none()
                && majors.iter().any(|pid| !game.players[*pid].alive)
            {
                result.first_elimination = Some(turn);
            }

            if MARKS.contains(&turn) {
                result.marks.push(snapshot(&game, &majors));
            }
            if turn == 50 {
                result.alive_at_50 = majors.iter().filter(|pid| game.players[**pid].alive).count();
            }
            if turn == 60 {
                result.blows_by_60 = game.siege.blows;
                result.damage_by_60 = game.siege.damage;
            }
        }
        result.alive_at_end = majors.iter().filter(|pid| game.players[**pid].alive).count();
        while result.marks.len() < MARKS.len() {
            result.marks.push(Snapshot::default());
        }
        result
    });

    // ---- reach ----
    let mut separations: Vec<i32> = per_map
        .iter()
        .flat_map(|m| m.nearest_rival.iter().copied())
        .collect();
    separations.sort_unstable();
    let pct = |v: &[i32], p: f64| -> i32 {
        if v.is_empty() {
            return 0;
        }
        v[((v.len() as f64 - 1.0) * p).round() as usize]
    };
    println!("\n=== REACH: tiles from a capital to the nearest rival capital ===");
    if separations.is_empty() {
        println!("  no map placed every capital");
    } else {
        let mean = separations.iter().sum::<i32>() as f64 / separations.len() as f64;
        println!(
            "  n={}  min {}  p10 {}  median {}  p90 {}  max {}  mean {mean:.1}",
            separations.len(),
            separations[0],
            pct(&separations, 0.10),
            pct(&separations, 0.50),
            pct(&separations, 0.90),
            separations[separations.len() - 1],
        );
        let march = |tiles: i32| (tiles as f64 / MARCH_TILES_PER_TURN).ceil() as i32;
        println!(
            "  march turns at {MARCH_TILES_PER_TURN} tiles/turn:  median {}  p90 {}",
            march(pct(&separations, 0.50)),
            march(pct(&separations, 0.90)),
        );
        let within = |t: i32| {
            separations.iter().filter(|d| **d <= t).count() as f64 / separations.len() as f64
        };
        for tiles in [8, 12, 16, 20, 25] {
            println!(
                "  seats with a rival capital within {tiles:>2} tiles ({:>2} march turns): {:>5.1}%",
                march(tiles),
                100.0 * within(tiles)
            );
        }
    }

    // ---- the wall clock and the garrison ----
    println!("\n=== THE DEFENDER, BY TURN ===");
    println!(
        "{:>5}{:>9}{:>10}{:>10}{:>9}{:>8}{:>9}{:>8}{:>8}{:>10}{:>8}",
        "turn", "walled%", "cap.str", "garrison", "melee", "staged", "adjac",
        "MAXstg", "MAXadj", "atwar%", "agoge%"
    );
    for (index, mark) in MARKS.iter().enumerate() {
        let mut caps = 0.0;
        let (mut walled, mut strength, mut garrison, mut military) = (0.0, 0.0, 0.0, 0.0);
        let (mut melee, mut staged, mut atwar, mut adjacent) = (0.0, 0.0, 0.0, 0.0);
        let (mut maxadj, mut maxstg) = (0.0_f64, 0.0_f64);
        let (mut iron, mut mason, mut craft) = (0.0, 0.0, 0.0);
        for m in per_map.iter() {
            let s = &m.marks[index];
            caps += s.capitals as f64;
            walled += s.walled_capitals as f64;
            strength += s.capital_strength;
            garrison += s.garrison_strength;
            military += s.military;
            melee += s.melee;
            staged += s.staged;
            adjacent += s.adjacent;
            maxadj = maxadj.max(s.max_adjacent);
            maxstg = maxstg.max(s.max_staged);
            atwar += s.at_war as f64;
            iron += s.iron as f64;
            mason += s.masonry as f64;
            craft += s.craftsmanship as f64;
        }
        if caps == 0.0 {
            continue;
        }
        println!(
            "{mark:>5}{:>8.1}%{:>10.1}{:>10.1}{:>9.1}{:>8.2}{:>8.2}{:>8.0}{:>8.0}{:>7.0}%{:>7.0}%",
            100.0 * walled / caps,
            strength / caps,
            garrison / caps,
            melee / caps,
            staged / caps,
            adjacent / caps,
            maxstg,
            maxadj,
            100.0 * atwar / caps,
            100.0 * craft / caps,
        );
        let _ = (military, mason);
    }

    // ---- what actually happens ----
    let wars: Vec<u32> = per_map.iter().filter_map(|m| m.first_war).collect();
    let captures: Vec<u32> = per_map.iter().filter_map(|m| m.first_capture).collect();
    let elims: Vec<u32> = per_map.iter().filter_map(|m| m.first_elimination).collect();
    let majors = per_map.first().map(|m| m.majors).unwrap_or(0);
    println!("\n=== WHAT THE STOCK AGENT ACTUALLY DOES ===");
    println!(
        "  maps with any war between majors     : {}/{}  {}",
        wars.len(),
        per_map.len(),
        if wars.is_empty() { String::new() } else {
            let mut w = wars.clone(); w.sort_unstable();
            format!("(first war: median turn {})", w[w.len() / 2])
        }
    );
    println!(
        "  maps with any capture between majors : {}/{}  {}",
        captures.len(),
        per_map.len(),
        if captures.is_empty() {
            String::new()
        } else {
            format!(
                "(first capture: median turn {})",
                {
                    let mut c = captures.clone();
                    c.sort_unstable();
                    c[c.len() / 2]
                }
            )
        }
    );
    println!(
        "  maps with any major eliminated       : {}/{}  {}",
        elims.len(),
        per_map.len(),
        if elims.is_empty() {
            String::new()
        } else {
            format!("(first: median turn {})", {
                let mut e = elims.clone();
                e.sort_unstable();
                e[e.len() / 2]
            })
        }
    );
    let alive50: f64 =
        per_map.iter().map(|m| m.alive_at_50 as f64).sum::<f64>() / per_map.len().max(1) as f64;
    let aliveend: f64 =
        per_map.iter().map(|m| m.alive_at_end as f64).sum::<f64>() / per_map.len().max(1) as f64;
    let camps: Vec<(u32, Option<u32>, Option<u32>)> =
        per_map.iter().flat_map(|m| m.campaigns.iter().copied()).collect();
    let med = |mut v: Vec<u32>| -> String {
        if v.is_empty() {
            return "-".to_string();
        }
        v.sort_unstable();
        format!("{}", v[v.len() / 2])
    };
    // Split by when the war opened. A rush is an *early* war; pooling it with
    // the late opportunistic wars that make up most of the sample describes
    // neither.
    for (label, lo, hi) in [("EARLY (declared < t50)", 0u32, 50u32), ("LATE (t50+)", 50, u32::MAX)] {
        let set: Vec<_> = camps.iter().filter(|c| c.0 >= lo && c.0 < hi).copied().collect();
        if set.is_empty() {
            continue;
        }
        println!("\n=== CAMPAIGN CLOCK — {label} ===");
        println!("  wars                                 : {}", set.len());
        println!(
            "  declared (median turn)               : {}",
            med(set.iter().map(|c| c.0).collect())
        );
        println!(
            "  took a city                          : {} of {}  (median turn {})",
            set.iter().filter(|c| c.1.is_some()).count(),
            set.len(),
            med(set.iter().filter_map(|c| c.1).collect())
        );
        println!(
            "  ...turns declaration -> first city   : {}",
            med(set.iter().filter_map(|c| c.1.map(|t| t - c.0)).collect())
        );
        println!(
            "  killed an empire                     : {} of {}  (median turn {})",
            set.iter().filter(|c| c.2.is_some()).count(),
            set.len(),
            med(set.iter().filter_map(|c| c.2).collect())
        );
        println!(
            "  ...turns declaration -> the kill     : {}",
            med(set.iter().filter_map(|c| c.2.map(|t| t - c.0)).collect())
        );
    }
    println!("\n=== THE CAMPAIGN CLOCK (per war, not per game) ===");
    println!("  wars opened                          : {}", camps.len());
    println!(
        "  declared (median turn)               : {}",
        med(camps.iter().map(|c| c.0).collect())
    );
    println!(
        "  first city taken (median turn)       : {}   [{} of {} wars]",
        med(camps.iter().filter_map(|c| c.1).collect()),
        camps.iter().filter(|c| c.1.is_some()).count(),
        camps.len()
    );
    println!(
        "  ...turns from declaration to it      : {}",
        med(camps.iter().filter_map(|c| c.1.map(|t| t - c.0)).collect())
    );
    println!(
        "  an empire died (median turn)         : {}   [{} of {} wars]",
        med(camps.iter().filter_map(|c| c.2).collect()),
        camps.iter().filter(|c| c.2.is_some()).count(),
        camps.len()
    );
    println!(
        "  ...turns from first city to the kill : {}",
        med(camps
            .iter()
            .filter_map(|c| match (c.1, c.2) {
                (Some(a), Some(b)) if b >= a => Some(b - a),
                _ => None,
            })
            .collect())
    );
    let blows: f64 =
        per_map.iter().map(|m| m.blows_by_60 as f64).sum::<f64>() / per_map.len().max(1) as f64;
    let dmg: f64 =
        per_map.iter().map(|m| m.damage_by_60 as f64).sum::<f64>() / per_map.len().max(1) as f64;
    println!("  blows landed on cities by turn 60    : {blows:.1} ({dmg:.0} HP)");
    println!("  majors alive at turn 50              : {alive50:.2} of {majors}");
    println!("  majors alive at the end              : {aliveend:.2} of {majors}");
}
