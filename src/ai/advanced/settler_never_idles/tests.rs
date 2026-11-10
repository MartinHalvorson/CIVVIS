use super::super::AdvancedAi;
use super::*;
use crate::ai::Ai;
use crate::game::Game;
use crate::reasoning::Journal;

/// The gene ships off in both stock controllers; the toggles are twins.
#[test]
fn the_gene_is_off_in_both_controllers_and_the_toggles_are_twins() {
    let fresh = AdvancedAi::new();
    let legacy = AdvancedAi::legacy();
    assert!(!fresh.settler_never_idles);
    assert!(!legacy.settler_never_idles);
    let mut opted = AdvancedAi::new();
    opted.enable_settler_never_idles();
    assert!(opted.settler_never_idles);
    opted.disable_settler_never_idles();
    assert!(!opted.settler_never_idles);
}

/// The guard ships off in both stock controllers; the toggles are twins.
#[test]
fn the_guard_is_off_in_both_controllers_and_the_toggles_are_twins() {
    let fresh = AdvancedAi::new();
    let legacy = AdvancedAi::legacy();
    assert!(!fresh.exhaustion_loyalty_guard);
    assert!(!legacy.exhaustion_loyalty_guard);
    let mut opted = AdvancedAi::new();
    opted.enable_exhaustion_loyalty_guard();
    assert!(opted.exhaustion_loyalty_guard);
    opted.disable_exhaustion_loyalty_guard();
    assert!(!opted.exhaustion_loyalty_guard);
}

/// `civvis-20260929T002742Z` founded Maracaibo on an exhaustion site
/// beside a hidden Kongo city after the preferred search rejected nearby
/// sites for that very border. It lost about 20 Loyalty a turn and flipped
/// eleven turns later. The live frontier guard must apply in both fallback
/// tiers and on arrival even when the broader exhaustion gene is off.
#[test]
fn frontier_guard_covers_exhaustion_and_relaxed_arrival() {
    for (frontier, guard) in [(false, false), (true, false), (true, true)] {
        let mut g = Game::new_full(1, 20, 12, 91_306, 120, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        g.units.get_mut(&settler).expect("settler").moves_left = 20.0;
        let explored: Vec<Pos> = g.map.tiles.keys().copied().collect();
        g.players[0].explored.extend(explored.iter().copied());
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        ai.enable_settler_never_idles();
        if frontier {
            ai.enable_frontier_loyalty();
        } else {
            ai.disable_frontier_loyalty();
        }
        if guard {
            ai.enable_exhaustion_loyalty_guard();
        } else {
            ai.disable_exhaustion_loyalty_guard();
        }
        ai.attach_journal(Journal::recording());
        let reachable = ai
            .settler_exhaustion_target(&g, 0, settler)
            .expect("fixture: the open board offers an exhaustion site");
        // A met major's border the mirror could not attribute to a seen
        // city, on every plot: the whole board is five tiles from one.
        g.unseen_major_borders.extend(explored.iter().copied());
        let picked = ai.settler_exhaustion_target(&g, 0, settler);
        if frontier {
            assert_eq!(picked, None, "every site lies beside an unresolved border");
            assert!(
                ai.relaxed_arrival_verdict(&g, 0, reachable).is_some(),
                "a border revealed during the walk must be checked on arrival"
            );
            assert!(
                !ai.settler_stranded(&mut g, 0, settler),
                "the stranded Settler does not found on unpriceable ground"
            );
            assert!(g.cities.is_empty(), "no city was founded");
            assert!(
                ai.settler_stranded_at.contains_key(&settler),
                "the hold is named, not silent"
            );
        } else {
            assert!(
                picked.is_some(),
                "without the frontier guard, the search is unchanged"
            );
        }
    }
}

/// The rival-sphere sieve on the doomed-target board: the plot four tiles
/// from a pop-12 rival capital is inside the sphere, a plot beside our
/// own capital is not, and with both plots the only ones left the guard's
/// exhaustion search answers the home plot instead of spending its
/// forecast retries on the doomed one and returning nothing.
#[test]
fn the_sieve_keeps_the_forecast_for_plots_outside_the_rivals_sphere() {
    let mut g = Game::new_full(2, 40, 24, crate::rng::fixture_seed("AUTO_11", 91_777), 250, 0, false);
    g.current = 0;
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .expect("a starting settler");
        let pos = g.units[&settler].pos;
        g.remove_unit(settler);
        g.found_city_for(pid, pos, None);
    }
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    let ours = g.player_city_ids(0)[0];
    let theirs = g.player_city_ids(1)[0];
    let home = g.cities[&ours].pos;
    let rival = g.cities[&theirs].pos;
    assert!(g.wdist(home, rival) >= 12, "fixture needs a distant rival");
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for position in &positions {
        let tile = g.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        g.players[0].explored.insert(*position);
    }
    g.cities.get_mut(&theirs).unwrap().pop = 12;
    g.cities.get_mut(&ours).unwrap().pop = 6;
    let mut beside_rival: Vec<Pos> = positions
        .iter()
        .copied()
        .filter(|pos| g.wdist(*pos, rival) == 4 && g.wdist(*pos, home) >= 4)
        .collect();
    beside_rival.sort_unstable();
    let doomed = beside_rival[0];
    // Beside home, out of the rival's reach, and inside the stranded
    // search radius from where the Settler will stand.
    let mut beside_home: Vec<Pos> = positions
        .iter()
        .copied()
        .filter(|pos| {
            g.wdist(*pos, home) <= 4
                && g.wdist(*pos, rival) >= 8
                && g.wdist(*pos, doomed) < STRANDED_SITE_RADIUS
        })
        .collect();
    beside_home.sort_by_key(|pos| (g.wdist(*pos, doomed), *pos));
    let mut ai = AdvancedAi::new();
    ai.enable_engine_repairs();
    ai.enable_settler_never_idles();
    ai.enable_exhaustion_loyalty_guard();
    ai.attach_journal(Journal::recording());
    let safe = beside_home
        .iter()
        .copied()
        .find(|pos| ai.base.valid_settle_site(&g, 0, *pos))
        .expect("fixture: a legal plot beside home");
    assert!(
        AdvancedAi::inside_rival_sphere(&g, 0, doomed),
        "four tiles from a rival capital"
    );
    assert!(
        !AdvancedAi::inside_rival_sphere(&g, 0, safe),
        "beside our own capital"
    );
    // The Settler stands beside the doomed plot, the nearest legal one.
    let start = g
        .nbrs(doomed)
        .into_iter()
        .find(|pos| g.map.tiles.contains_key(pos) && g.wdist(*pos, rival) >= 4)
        .expect("a neighbour to stand on");
    let settler = g.spawn_test_unit("settler", 0, start);
    g.units.get_mut(&settler).expect("settler").moves_left = 40.0;
    let retired = ai.settler_dead_sites.entry(settler).or_default();
    for pos in &positions {
        if *pos != doomed && *pos != safe {
            retired.insert(*pos, g.turn + 1000);
        }
    }
    assert_eq!(
        ai.settler_exhaustion_target(&g, 0, settler),
        Some(safe),
        "the sieve drops the doomed plot and the forecast passes the home plot"
    );
}

/// Arretium in `civvis-20260829T090147Z`: a plot four tiles from a small
/// visible rival city and five from our own capital passes the mirror's
/// forecast (the rival's unseen cities are not on the board) and revolted
/// in peacetime. Under the guard the preferred search's verdict refuses it
/// for standing inside the rival's sphere; off, the forecast alone speaks.
#[test]
fn the_preferred_verdict_refuses_a_plot_inside_a_rivals_sphere_under_the_guard() {
    let mut g = Game::new_full(2, 40, 24, 91_775, 250, 0, false);
    g.current = 0;
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .expect("a starting settler");
        let pos = g.units[&settler].pos;
        g.remove_unit(settler);
        g.found_city_for(pid, pos, None);
    }
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    let ours = g.player_city_ids(0)[0];
    let theirs = g.player_city_ids(1)[0];
    let home = g.cities[&ours].pos;
    let rival = g.cities[&theirs].pos;
    assert!(g.wdist(home, rival) >= 12, "fixture needs a distant rival");
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for position in &positions {
        let tile = g.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        g.players[0].explored.insert(*position);
    }
    // A small rival city: the forecast reads its two citizens as harmless
    // once a city of ours stands five tiles from the plot — Arretium's
    // board: Rome five away, Aksu (pop 2) four away.
    g.cities.get_mut(&theirs).unwrap().pop = 2;
    g.cities.get_mut(&ours).unwrap().pop = 6;
    let mut beside_rival: Vec<Pos> = positions
        .iter()
        .copied()
        .filter(|pos| g.wdist(*pos, rival) == 4 && g.wdist(*pos, home) > 4)
        .collect();
    beside_rival.sort_unstable();
    let plot = beside_rival[0];
    let mut near_plot: Vec<Pos> = positions
        .iter()
        .copied()
        .filter(|pos| g.wdist(*pos, plot) == 5 && g.wdist(*pos, rival) >= 6)
        .collect();
    near_plot.sort_unstable();
    let second = g.found_city_for(0, near_plot[0], None);
    g.cities.get_mut(&second).unwrap().pop = 6;
    assert!(
        AdvancedAi::settle_site_forecast_revolt(&g, 0, plot).is_none(),
        "fixture: the forecast passes a plot beside a pop-2 rival city"
    );
    assert!(AdvancedAi::inside_rival_sphere(&g, 0, plot));
    let mut ai = AdvancedAi::new();
    ai.enable_engine_repairs();
    // This fixture exercises the guard's off-to-on transition. The
    // deployment ledger may select it from its batch evidence, so hold it
    // off explicitly before proving the unguarded forecast path.
    ai.disable_exhaustion_loyalty_guard();
    assert!(!ai.exhaustion_loyalty_guard);
    assert_eq!(
        ai.settle_site_loyalty_verdict(&g, 0, plot),
        None,
        "off, the forecast alone speaks and passes it"
    );
    ai.enable_exhaustion_loyalty_guard();
    let why = ai
        .settle_site_loyalty_verdict(&g, 0, plot)
        .expect("under the guard the preferred verdict refuses the plot");
    assert!(why.contains("rival major's city"), "{why}");
}

/// The nearest-legal tier founded with no forecast at all. The board of
/// `research_probe`'s doomed-target tests: a pop-12 rival capital and a
/// plot four tiles from it where a new city loses twenty Loyalty a turn.
/// Every other plot is retired for this Settler, so the ranked tier has
/// nothing and the nearest tier answers. Off, it takes the doomed plot;
/// under the guard it forecasts, refuses, and returns nothing.
#[test]
fn the_nearest_legal_tier_forecasts_under_the_guard() {
    for guard in [false, true] {
        let mut g = Game::new_full(2, 40, 24, 91_775, 250, 0, false);
        g.current = 0;
        for pid in 0..2 {
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| g.units[unit].kind == "settler")
                .expect("a starting settler");
            let pos = g.units[&settler].pos;
            g.remove_unit(settler);
            g.found_city_for(pid, pos, None);
        }
        for unit in g.player_unit_ids(0) {
            g.remove_unit(unit);
        }
        let ours = g.player_city_ids(0)[0];
        let theirs = g.player_city_ids(1)[0];
        let home = g.cities[&ours].pos;
        let rival = g.cities[&theirs].pos;
        assert!(g.wdist(home, rival) >= 12, "fixture needs a distant rival");
        let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
        for position in &positions {
            let tile = g.map.tiles.get_mut(position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            g.players[0].explored.insert(*position);
        }
        g.cities.get_mut(&theirs).unwrap().pop = 12;
        g.cities.get_mut(&ours).unwrap().pop = 6;
        let mut beside_rival: Vec<Pos> = positions
            .iter()
            .copied()
            .filter(|pos| g.wdist(*pos, rival) == 4 && g.wdist(*pos, home) >= 4)
            .collect();
        beside_rival.sort_unstable();
        let doomed = beside_rival[0];
        let start = g
            .nbrs(doomed)
            .into_iter()
            .find(|pos| g.map.tiles.contains_key(pos) && g.wdist(*pos, rival) >= 4)
            .expect("a neighbour to stand on");
        let settler = g.spawn_test_unit("settler", 0, start);
        g.units.get_mut(&settler).expect("settler").moves_left = 20.0;
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        ai.enable_settler_never_idles();
        if guard {
            ai.enable_exhaustion_loyalty_guard();
        } else {
            ai.disable_exhaustion_loyalty_guard();
        }
        ai.attach_journal(Journal::recording());
        // Isolate this optional guard from the fixed production safety
        // pass, which now also runs in native tournaments.
        ai.disable_live_settler_capture_lessons();
        assert!(
            ai.base.valid_settle_site(&g, 0, doomed),
            "fixture: the doomed plot is a legal site"
        );
        let (per_turn, turns) = AdvancedAi::settle_site_forecast_revolt(&g, 0, doomed)
            .expect("fixture: the forecast dooms the plot");
        assert!(
            turns < STRANDED_SITE_MIN_HOLD_TURNS,
            "fixture: revolt in {turns:.0} turns at {per_turn:.1} a turn"
        );
        // Retire every other plot for this Settler, so only the nearest
        // tier can answer.
        let retired = ai.settler_dead_sites.entry(settler).or_default();
        for pos in &positions {
            if *pos != doomed {
                retired.insert(*pos, g.turn + 1000);
            }
        }
        let picked = ai.settler_exhaustion_target(&g, 0, settler);
        if guard {
            assert_eq!(
                picked, None,
                "the guard forecasts the nearest legal site and refuses the revolt"
            );
        } else {
            assert_eq!(
                picked,
                Some(doomed),
                "off, the nearest tier takes the doomed plot unasked"
            );
        }
    }
}

/// Science cannot use the never-idles tier's twenty-turn compromise. A
/// colony forecast to revolt in the ordinary growth horizon is still a
/// lost science city, even when it would survive longer than the generic
/// exhaustion floor. The cached watchdog path must honor the same rule.
#[test]
fn an_explicit_science_lane_rejects_a_slow_exhaustion_revolt() {
    let mut g = Game::new_full(2, 40, 24, 91_775, 250, 0, false);
    g.current = 0;
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .expect("a starting settler");
        let pos = g.units[&settler].pos;
        g.remove_unit(settler);
        g.found_city_for(pid, pos, None);
    }
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    let ours = g.player_city_ids(0)[0];
    let theirs = g.player_city_ids(1)[0];
    let home = g.cities[&ours].pos;
    let rival = g.cities[&theirs].pos;
    assert!(g.wdist(home, rival) >= 12, "fixture needs a distant rival");
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for position in &positions {
        let tile = g.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        g.players[0].explored.insert(*position);
    }
    g.cities.get_mut(&ours).unwrap().pop = 6;
    let probe = AdvancedAi::new();
    let mut moderate = None;
    for rival_pop in 2..=12 {
        g.cities.get_mut(&theirs).unwrap().pop = rival_pop;
        for pos in &positions {
            if g.wdist(*pos, home) > STRANDED_SITE_RADIUS
                || !probe.base.valid_settle_site(&g, 0, *pos)
            {
                continue;
            }
            if let Some((per_turn, turns)) = AdvancedAi::settle_site_forecast_revolt(&g, 0, *pos) {
                if turns >= STRANDED_SITE_MIN_HOLD_TURNS {
                    moderate = Some((*pos, rival_pop, per_turn, turns));
                    break;
                }
            }
        }
        if moderate.is_some() {
            break;
        }
    }
    let (site, rival_pop, per_turn, turns) =
        moderate.expect("fixture needs a revolt forecast between twenty and forty turns");
    g.cities.get_mut(&theirs).unwrap().pop = rival_pop;
    assert!(per_turn < 0.0);
    assert!(turns >= STRANDED_SITE_MIN_HOLD_TURNS);

    let start = g
        .nbrs(site)
        .into_iter()
        .find(|pos| g.map.tiles.contains_key(pos) && g.wdist(*pos, rival) >= 4)
        .expect("a reachable tile beside the test site");
    let settler = g.spawn_test_unit("settler", 0, start);
    g.units.get_mut(&settler).expect("settler").moves_left = 20.0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_engine_repairs();
    ai.enable_settler_never_idles();
    ai.attach_journal(Journal::recording());
    let retired = ai.settler_dead_sites.entry(settler).or_default();
    for pos in &positions {
        if *pos != site {
            retired.insert(*pos, g.turn + 1000);
        }
    }
    assert_eq!(
        ai.settler_exhaustion_target(&g, 0, settler),
        None,
        "Science rejects a revolt inside the normal growth horizon"
    );

    g.units.get_mut(&settler).expect("settler").pos = site;
    g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
    ai.settler_targets.insert(settler, site);
    assert!(!ai.settler_watchdog_step(&mut g, 0, settler));
    assert_eq!(
        g.player_city_ids(0).len(),
        1,
        "the Science watchdog does not found the doomed cached site"
    );
}

/// A Settler that never moves counts up; one that moves resets; a
/// second look in the same turn does not count twice.
#[test]
fn the_idle_streak_counts_turns_on_one_tile_once_each() {
    let mut g = Game::new_full(1, 12, 8, 91_301, 60, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .expect("a starting settler");
    let mut ai = AdvancedAi::new();
    ai.enable_settler_never_idles();
    assert_eq!(ai.note_settler_idle(&g, settler), 0);
    assert_eq!(
        ai.note_settler_idle(&g, settler),
        0,
        "same turn, same answer"
    );
    g.turn += 1;
    assert_eq!(ai.note_settler_idle(&g, settler), 1);
    g.turn += 1;
    assert_eq!(ai.note_settler_idle(&g, settler), 2);
    let here = g.units[&settler].pos;
    let step = g
        .nbrs(here)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
        .expect("a land neighbour");
    g.units.get_mut(&settler).expect("settler").pos = step;
    g.turn += 1;
    assert_eq!(
        ai.note_settler_idle(&g, settler),
        0,
        "moving resets the streak"
    );
}

/// On a board where every site is explicitly retired, neither controller
/// resurrects one. The live safety gene still names the stranded hold
/// instead of wandering back through the same rejected corridor.
///
/// The board: one city, one Settler standing in it, every plot within
/// three tiles of the city legal-but-excluded by spacing, the rest of the
/// map retired for this Settler as `settler_dead_sites` — exactly the
/// state thirty-turn retirements leave a live seat in after a handful of
/// forecast refusals.
#[test]
fn a_settler_whose_every_site_was_retired_walks_under_the_gene_and_holds_without_it() {
    for gene in [false, true] {
        let mut g = Game::new_full(1, 16, 10, 91_302, 120, 0, false);
        let founding = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        g.apply(0, &Action::FoundCity { unit: founding })
            .expect("the starting settler founds");
        let home = g.cities.values().next().expect("a city").pos;
        let settler = g.spawn_test_unit("settler", 0, home);
        g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        ai.enable_loyalty_rate_alarm();
        // The ledger pins the gene on; the off arm withholds it by name.
        if gene {
            ai.enable_settler_never_idles();
        } else {
            ai.disable_settler_never_idles();
        }
        ai.attach_journal(Journal::recording());
        // Retire every plot for this Settler: the state the live seat
        // reaches after its forecast refusals compound.
        let all: Vec<Pos> = g.map.tiles.keys().copied().collect();
        let retired = ai.settler_dead_sites.entry(settler).or_default();
        for pos in all {
            retired.insert(pos, g.turn + 1000);
        }
        let before = g.units[&settler].pos;
        let acted = ai.advanced_settler_step(&mut g, 0, settler);
        let after = g.units[&settler].pos;
        assert!(!acted, "all explicitly retired sites remain retired");
        assert_eq!(after, before, "the settler does not wander to a dead site");
        assert!(
            !ai.settler_targets.contains_key(&settler),
            "the stranded settler carries no resurrected target"
        );
        if gene {
            assert!(
                ai.settler_stranded_at.contains_key(&settler),
                "the safety gene records the stranded hold"
            );
        }
    }
}

/// A no-target Settler must still be able to learn the ground that made
/// every known city site unavailable. The old outer watchdog returned
/// before this point because `settler_stranded` had removed its target;
/// after patience the live gene now takes one safe step toward unseen land.
#[test]
fn a_stranded_settler_reveals_an_unexplored_land_frontier_after_patience() {
    let mut g = Game::new_full(1, 16, 10, 91_307, 120, 0, false);
    let founding = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .expect("a starting Settler");
    g.apply(0, &Action::FoundCity { unit: founding })
        .expect("the starting Settler founds");
    let home = g.cities.values().next().expect("a capital").pos;
    let settler = g.spawn_test_unit("settler", 0, home);
    g.units
        .get_mut(&settler)
        .expect("the test Settler")
        .moves_left = 2.0;

    // The mirror knows only the capital tile. The adjacent hidden land is
    // deliberately not itself a city site (the capital's spacing ring),
    // so the exhaustion search has no answer until the Settler reveals it.
    g.players[0].explored.clear();
    g.players[0].explored.insert(home);
    let mut ai = AdvancedAi::new();
    ai.enable_engine_repairs();
    ai.enable_settler_never_idles();
    ai.attach_journal(Journal::recording());

    let all: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pos in all {
        if ai.base.valid_settle_site(&g, 0, pos) {
            ai.settler_dead_sites
                .entry(settler)
                .or_default()
                .insert(pos, g.turn + 1000);
        }
    }
    let frontier = g
        .nbrs(home)
        .into_iter()
        .find(|pos| {
            !g.players[0].explored.contains(pos)
                && !g.blocked_city_sites.contains(pos)
                && !ai.settler_site_is_dead(settler, *pos)
                && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
                && g.unit_can_traverse(settler, *pos)
        })
        .expect("the fixture leaves an adjacent unexplored land tile");
    assert_eq!(
        ai.settler_exhaustion_target(&g, 0, settler),
        None,
        "all known city sites are retired before the reveal"
    );

    for turn in 0..=SETTLER_IDLE_PATIENCE {
        g.turn = turn;
        g.units
            .get_mut(&settler)
            .expect("the test Settler")
            .moves_left = 2.0;
        let acted = ai.advanced_settler_step(&mut g, 0, settler);
        if turn < SETTLER_IDLE_PATIENCE {
            assert!(!acted, "patience does not fire early at turn {turn}");
            assert_eq!(g.units[&settler].pos, home);
        } else {
            assert!(acted, "the watchdog takes the frontier recovery step");
        }
    }
    assert_ne!(
        g.units[&settler].pos, home,
        "the no-target Settler is no longer parked at the capital"
    );
    assert!(
        g.wdist(g.units[&settler].pos, frontier) <= 1,
        "the recovery step heads toward the hidden land frontier"
    );
}

/// Exhaustion is a relaxed search, not permission to resurrect a target
/// another branch deliberately retired.  This is the live parallel-settler
/// failure: a hysteresis retirement was recorded for a peer, then the
/// never-idles fallback ignored it and sent both Settlers back to the same
/// site.
#[test]
fn exhaustion_skips_retired_and_peer_reserved_sites() {
    let mut g = Game::new_full(1, 20, 12, 91_306, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .expect("a starting settler");
    let home = g.units[&settler].pos;
    let peer = g.spawn_test_unit("settler", 0, home);
    // Give the fixture's probe Settlers enough movement to make the
    // one-turn reachability test independent of the random starting ring.
    g.units.get_mut(&settler).expect("settler").moves_left = 20.0;
    g.units.get_mut(&peer).expect("peer").moves_left = 20.0;
    // The production scan intentionally refuses unobserved opening
    // footprints. This regression is about retirement/reservation, so
    // make the fixture's map fully observed first.
    let explored: Vec<Pos> = g.map.tiles.keys().copied().collect();
    g.players[0].explored.extend(explored);

    let mut ai = AdvancedAi::new();
    ai.enable_engine_repairs();
    ai.enable_settler_never_idles();
    let reachable: Vec<Pos> = ai
        .settle_sites_with_limit(&g, 0, home, 14, Some(SETTLEMENT_GLOBAL_PREFILTER_LIMIT))
        .into_iter()
        .map(|(pos, _)| pos)
        .filter(|pos| g.path_to(settler, *pos).is_some())
        .take(4)
        .collect();
    let direct_valid = g
        .wdisk(home, 14)
        .into_iter()
        .filter(|pos| ai.base.valid_settle_site(&g, 0, *pos))
        .count();
    assert!(
        reachable.len() >= 3,
        "fixture needs three reachable city sites, got {reachable:?} (direct valid={direct_valid})"
    );
    let retired = reachable[0];
    let reserved = reachable[1];
    let avoided = reachable[2];
    ai.settler_dead_sites
        .entry(settler)
        .or_default()
        .insert(retired, g.turn + 1000);
    ai.settler_targets.insert(peer, reserved);
    ai.settler_avoid.insert(settler, (avoided, g.turn + 1000));

    let picked = ai.settler_exhaustion_target(&g, 0, settler);
    assert!(
        picked.is_some(),
        "the fixture leaves a fourth reachable site"
    );
    assert_ne!(
        picked,
        Some(retired),
        "the fallback resurrected a dead site"
    );
    assert_ne!(
        picked,
        Some(reserved),
        "the fallback duplicated a peer target"
    );
    assert_ne!(
        picked,
        Some(avoided),
        "the fallback resurrected the target in this Settler's hysteresis cooldown"
    );
}

/// A live fallback must not answer "never idle" by selecting a plot a
/// visible hostile can take on the next turn.  The historical controller
/// still asks the nearest legal site; the bridge's capture lessons reject
/// that sole exposed target and let the emergency safety pass/stranded
/// watchdog hold or retreat instead.
#[test]
fn live_exhaustion_fallback_drops_an_exposed_legal_site() {
    let mut g = Game::new_full(2, 32, 20, 91_319, 120, 0, false);
    g.current = 0;
    let founding = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .expect("a starting settler");
    let home = g.units[&founding].pos;
    g.apply(0, &crate::game::Action::FoundCity { unit: founding })
        .expect("the starting settler founds");
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for position in &positions {
        let tile = g.map.tiles.get_mut(position).expect("map tile");
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        g.players[0].explored.insert(*position);
    }
    let probe = AdvancedAi::new();
    let danger = positions
        .iter()
        .copied()
        .filter(|pos| {
            g.wdist(home, *pos) >= 6
                && g.wdist(home, *pos) <= 12
                && probe.base.valid_settle_site(&g, 0, *pos)
        })
        .min()
        .expect("a legal frontier site");
    let start = g
        .nbrs(danger)
        .into_iter()
        .find(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .expect("a passable neighbour for the Settler");
    let hostile_tile = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            g.wdist(danger, *pos) == 1
                && *pos != start
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .expect("a visible hostile post beside the danger site");
    let settler = g.spawn_test_unit("settler", 0, start);
    g.units.get_mut(&settler).expect("settler").moves_left = 20.0;
    let hostile = g.spawn_test_unit("warrior", 1, hostile_tile);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.players[0].explored.insert(hostile_tile);

    let mut plain = AdvancedAi::new();
    plain.enable_settler_never_idles();
    plain.settler_dead_sites.entry(settler).or_default().extend(
        g.map
            .tiles
            .keys()
            .copied()
            .filter(|pos| *pos != danger)
            .map(|pos| (pos, g.turn + 1000)),
    );
    assert_eq!(
        plain.settler_exhaustion_target(&g, 0, settler),
        Some(danger),
        "the historical nearest-legal fallback sees the fixture site"
    );

    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    live.enable_settler_never_idles();
    live.settler_dead_sites.entry(settler).or_default().extend(
        g.map
            .tiles
            .keys()
            .copied()
            .filter(|pos| *pos != danger)
            .map(|pos| (pos, g.turn + 1000)),
    );
    let visible = live.battlefront_visibility(&g, 0);
    assert!(g.unit_visible_to(hostile, 0) && g.sees(&visible, hostile_tile));
    assert!(
        live.settlement_tile_risk(&g, 0, Some(settler), danger, &visible)
            > super::super::SETTLER_STEP_RISK_LIMIT,
        "the fixture danger site is visibly capturable"
    );
    assert_eq!(
        live.settler_exhaustion_target(&g, 0, settler),
        None,
        "the live fallback refuses the exposed sole site"
    );
}

/// The watchdog respects the one rule it keeps: a tile a visible raider
/// can reach next turn is not taken, however long the Settler has waited.
#[test]
fn the_watchdog_will_not_step_into_a_raiders_reach() {
    let mut g = Game::new_full(2, 16, 10, 91_303, 120, 0, false);
    let founding = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .expect("a starting settler");
    g.apply(0, &Action::FoundCity { unit: founding })
        .expect("the starting settler founds");
    let home = g
        .cities
        .values()
        .find(|city| city.owner == 0)
        .expect("a city")
        .pos;
    let settler = g.spawn_test_unit("settler", 0, home);
    g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
    let Some(barbarian) = g.barb_pid else {
        return; // no barbarian seat on this fixture
    };
    // A warrior on every land tile two steps out: every neighbour of the
    // city is inside its reach.
    let ring: Vec<Pos> = g
        .wdisk(home, 2)
        .into_iter()
        .filter(|pos| g.wdist(*pos, home) == 2)
        .filter(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile)))
        .collect();
    for pos in &ring {
        g.spawn_test_unit("warrior", barbarian, *pos);
    }
    let mut ai = AdvancedAi::new();
    ai.enable_engine_repairs();
    ai.enable_settler_never_idles();
    ai.attach_journal(Journal::recording());
    let target = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.wdist(*pos, home) == 6 && ai.base.valid_settle_site(&g, 0, *pos))
        .expect("a legal site six tiles out");
    ai.settler_targets.insert(settler, target);
    ai.settler_idle_streak
        .insert(settler, (home, g.turn, SETTLER_IDLE_PATIENCE));
    let reach = ai.barbarian_reach(&g, 0, home, REACH_SCAN_RADIUS);
    let every_neighbour_covered = g
        .nbrs(home)
        .into_iter()
        .filter(|pos| g.can_move(settler, *pos))
        .all(|pos| reach.covers(&g, pos));
    if !every_neighbour_covered {
        return; // the ring did not close on this fixture; nothing to prove
    }
    assert!(!ai.settler_watchdog_step(&mut g, 0, settler));
    assert_eq!(g.units[&settler].pos, home, "it stayed in the city");
}

/// A site the exhaustion search chose is founded on arrival even where
/// the strict verdict's fog guess would refuse it; the same board with
/// the site chosen the ordinary way is still refused. The engine's own
/// forecast is the one thing both rules keep.
#[test]
fn an_exhaustion_site_is_founded_on_arrival_where_the_fog_guess_would_refuse_it() {
    for relaxed in [true, false] {
        let mut g = Game::new_full(1, 20, 12, 91_305, 120, 0, false);
        let founding = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        g.apply(0, &Action::FoundCity { unit: founding })
            .expect("the starting settler founds");
        let home = g.cities.values().next().expect("a city").pos;
        // Explore only the home halo so a far site is a fogged frontier.
        let halo: Vec<Pos> = g.wdisk(home, 5);
        g.players[0].explored.clear();
        g.players[0].explored.extend(halo);
        let far: Vec<Pos> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| g.wdist(*pos, home) >= 9)
            .collect();
        let mut site = None;
        for pos in far {
            let probe = g.spawn_test_unit("settler", 0, pos);
            let legal = g.can_found_city(probe) && AdvancedAi::beyond_loyalty_reach(&g, 0, pos);
            g.remove_unit(probe);
            if legal {
                site = Some(pos);
                break;
            }
        }
        let site = site.expect("fixture needs a legal fogged frontier site");
        let settler = g.spawn_test_unit("settler", 0, site);
        g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge();
        ai.enable_loyalty_rate_alarm();
        ai.enable_settler_never_idles();
        assert!(ai.frontier_loyalty, "the live seat carries the fog guess");
        ai.settler_targets.insert(settler, site);
        if relaxed {
            ai.settler_relaxed_targets.insert(settler, site);
        }
        let cities_before = g.player_city_ids(0).len();
        let acted = ai.advanced_settler_step(&mut g, 0, settler);
        let founded = g.player_city_ids(0).len() > cities_before;
        if relaxed {
            assert!(acted && founded, "the relaxed arrival founds the colony");
        } else {
            assert!(
                !founded,
                "the strict arrival still refuses the fogged frontier"
            );
        }
    }
}

/// A Settler with no legal site anywhere but the one underfoot founds
/// there; one with none at all says it is stranded rather than holding
/// silently.
#[test]
fn a_stranded_settler_founds_where_it_stands_or_says_so() {
    let mut g = Game::new_full(1, 12, 8, 91_304, 60, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .expect("a starting settler");
    let here = g.units[&settler].pos;
    // Block every plot but the one underfoot.
    for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
        if pos != here {
            std::sync::Arc::make_mut(&mut g.blocked_city_sites).insert(pos);
        }
    }
    let mut ai = AdvancedAi::new();
    ai.enable_settler_never_idles();
    let journal = Journal::recording();
    ai.attach_journal(journal.handle());
    assert_eq!(
        ai.settler_exhaustion_target(&g, 0, settler),
        Some(here),
        "the tile underfoot is the one legal site"
    );
    assert!(g.can_found_city(settler));
    assert!(ai.settler_stranded(&mut g, 0, settler));
    assert_eq!(
        g.player_city_ids(0).len(),
        1,
        "the capital stands where it stood"
    );
    // A second Settler in that city cannot found (spacing) and has no
    // site anywhere: it reports itself stranded rather than holding
    // silently.
    let second = g.spawn_test_unit("settler", 0, here);
    assert!(ai.settler_exhaustion_target(&g, 0, second).is_none());
    assert!(!ai.settler_stranded(&mut g, 0, second));
    let delta = journal.since(0);
    assert!(
        delta
            .thoughts
            .iter()
            .any(|thought| thought.headline.starts_with("Settler is stranded")),
        "the hold is named"
    );
}

/// `stranded-settler-leaves-the-corridor`: an early Settler stranded inside
/// its home corridor drops the corridor under the gene and keeps it
/// otherwise. Live King game 160 held one stranded six tiles from Bogotá
/// from turn 29 to 66.
#[test]
fn a_stranded_early_settler_leaves_its_corridor_only_under_the_gene() {
    for gene in [false, true] {
        let mut g = Game::new_full(1, 12, 8, 91_304, 60, 0, false);
        let founder = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("a starting settler");
        let here = g.units[&founder].pos;
        g.apply(0, &Action::FoundCity { unit: founder }).unwrap();
        for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
            std::sync::Arc::make_mut(&mut g.blocked_city_sites).insert(pos);
        }
        let settler = g.spawn_test_unit("settler", 0, here);
        let mut ai = AdvancedAi::new();
        ai.enable_settler_never_idles();
        ai.enable_live_settler_capture_lessons();
        if gene {
            ai.enable_stranded_settler_leaves_the_corridor();
        }
        ai.early_settler_homes.insert(settler, here);
        assert_eq!(
            ai.early_settler_home(settler),
            Some(here),
            "fixture: an early home"
        );
        let journal = Journal::recording();
        ai.attach_journal(journal.handle());
        assert!(!ai.settler_stranded(&mut g, 0, settler));
        assert_eq!(ai.early_settler_home(settler).is_none(), gene);
        let named = journal.since(0).thoughts.iter().any(|thought| {
            thought
                .headline
                .starts_with("Settler leaves its early corridor")
        });
        assert_eq!(named, gene);
    }
}

fn protected_retreat_board(city: bool) -> (Game, AdvancedAi, u32, Pos, u32) {
    let mut g = Game::new_full(2, 28, 18, 91_337, 120, 0, true);
    g.current = 0;
    let founding = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let here = g.units[&founding].pos;
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pos in &positions {
        let tile = g.map.tiles.get_mut(pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.players[0].explored.extend(positions);
    if city {
        g.apply(0, &Action::FoundCity { unit: founding }).unwrap();
    }
    let units: Vec<u32> = g.units.keys().copied().collect();
    for uid in units {
        g.remove_unit(uid);
    }
    let settler = g.spawn_test_unit("settler", 0, here);
    let raider_at = g
        .nbrs(here)
        .into_iter()
        .find(|p| g.map.get(*p).is_some())
        .unwrap();
    let raider = g.spawn_test_unit("warrior", g.barb_pid.unwrap(), raider_at);
    let mut ai = AdvancedAi::new();
    ai.enable_settler_guard_holds();
    ai.enable_live_settler_capture_lessons();
    (g, ai, settler, here, raider)
}

#[test]
fn protected_city_refuses_cornered_retreat_but_still_releases_a_safe_march() {
    let (mut g, mut ai, settler, here, raider) = protected_retreat_board(true);
    let reach = ai.barbarian_reach(&g, 0, here, REACH_SCAN_RADIUS);
    assert!(
        reach.raiders_covering(&g, here) > 0,
        "the city is inside hostile reach"
    );
    assert!(ai.civilian_safe_at(&g, 0, settler, here, &reach));
    assert!(
        ai.cornered_retreats(&g, settler, here, &reach).is_empty(),
        "raw raider coverage must not send a protected settler out of its city"
    );
    let target = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|p| {
            g.wdist(here, *p) == 4
                && ai.base.valid_settle_site(&g, 0, *p)
                && g.route_step(settler, *p, 0).is_some()
        })
        .unwrap();
    ai.settler_targets.insert(settler, target);
    g.remove_unit(raider);
    assert!(
        ai.settler_watchdog_step(&mut g, 0, settler),
        "city protection must not prevent a safe departure when the threat clears"
    );
    assert_ne!(g.units[&settler].pos, here);
    assert!(g.wdist(g.units[&settler].pos, target) < g.wdist(here, target));
}

#[test]
fn protected_city_can_depart_safely_while_a_raider_still_covers_the_city() {
    let (mut g, mut ai, settler, here, raider) = protected_retreat_board(true);
    g.remove_unit(raider);
    let hostile = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|p| g.wdist(here, *p) == 2)
        .unwrap();
    g.spawn_test_unit("warrior", g.barb_pid.unwrap(), hostile);
    let reach = ai.barbarian_reach(&g, 0, here, REACH_SCAN_RADIUS);
    assert!(reach.raiders_covering(&g, here) > 0);
    let target = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|p| {
            g.wdist(here, *p) == 4
                && ai.base.valid_settle_site(&g, 0, *p)
                && g.route_step(settler, *p, 0)
                    .is_some_and(|next| ai.watchdog_tile_is_safe(&g, 0, settler, next, &reach))
        })
        .expect("a safe departure away from the raider");
    ai.settler_targets.insert(settler, target);
    assert!(ai.settler_watchdog_step(&mut g, 0, settler));
    let arrived = g.units[&settler].pos;
    assert_ne!(arrived, here);
    assert!(g.wdist(arrived, target) < g.wdist(here, target));
    assert!(
        !reach.covers(&g, arrived),
        "the accepted step is outside capture reach"
    );
}

#[test]
fn protected_guard_blocks_retreat_only_while_it_really_protects_the_settler() {
    let (mut g, mut ai, settler, here, _) = protected_retreat_board(false);
    let guard = g.spawn_test_unit("warrior", 0, here);
    ai.settler_guards.insert(settler, guard);
    let reach = ai.barbarian_reach(&g, 0, here, REACH_SCAN_RADIUS);
    assert!(reach.raiders_covering(&g, here) > 0);
    assert!(ai.civilian_safe_at(&g, 0, settler, here, &reach));
    assert!(
        ai.cornered_retreats(&g, settler, here, &reach).is_empty(),
        "a surviving bound guard is protection, not an emergency retreat trigger"
    );
    g.units.get_mut(&guard).unwrap().hp = 20;
    assert!(!ai.civilian_safe_at(&g, 0, settler, here, &reach));
    assert!(
        !ai.cornered_retreats(&g, settler, here, &reach).is_empty(),
        "a wounded guard must not suppress a useful escape"
    );
    g.units.get_mut(&guard).unwrap().hp = 100;
    ai.settler_guards.remove(&settler);
    assert!(
        !ai.cornered_retreats(&g, settler, here, &reach).is_empty(),
        "an unbound bystander may leave and must not suppress escape"
    );
}

/// ⚠⚠⚠ A HOLD IS NOT SAFETY WHEN THE TILE UNDER THE SETTLER IS COVERED.
///
/// Every candidate the watchdog considers must make PROGRESS and pass the
/// absolute `watchdog_tile_is_safe`. A settler raiders have surrounded has
/// no such tile, so it refused "to walk it into a capture" while standing
/// in one. Live, `civvis-20260830T095742Z` settler 589826 held from t34 to
/// t47 — its own journal reading "3 raider(s) could take (14, 9) and no
/// reachable tile is better" — and a horseman took it. That game built
/// EIGHT settlers and ended turn 150 with ONE city at 0.277 of the leader.
#[test]
fn a_cornered_settler_gives_ground_rather_than_wait_to_be_taken() {
    let mut g = Game::new_full(2, 20, 12, 91_306, 120, 0, true);
    let explored: Vec<Pos> = g.map.tiles.keys().copied().collect();
    g.players[0].explored.extend(explored.iter().copied());
    // ⚠ `players.iter().position(|p| p.is_barbarian)` is NOT the same seat:
    // it finds an earlier slot and the spawned raiders end up owned by a
    // minor, so the reach never sees them. `barb_pid` is the one the reach
    // itself asks for.
    let barb = g
        .barb_pid
        .expect("new_full's last argument seats the barbarians");
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .expect("a starting settler");
    let here = g.units[&settler].pos;
    g.units.get_mut(&settler).expect("settler").moves_left = 2.0;
    let ai = AdvancedAi::new();

    // Nothing is hunting it: holding is not the losing move, so the
    // watchdog is offered no reason to give ground.
    let quiet = ai.barbarian_reach(&g, 0, here, REACH_SCAN_RADIUS);
    assert!(
        ai.cornered_retreats(&g, settler, here, &quiet).is_empty(),
        "an unthreatened settler is never told to retreat"
    );

    // Now put raiders on it. Two neighbours are enough to cover the tile.
    for next in g.nbrs(here).into_iter().take(2) {
        g.spawn_unit("warrior", barb, next);
    }
    let hunted = ai.barbarian_reach(&g, 0, here, REACH_SCAN_RADIUS);
    let here_covering = hunted.raiders_covering(&g, here);
    let seen: Vec<(u32, bool)> = g
        .units
        .values()
        .filter(|u| Some(u.owner) == g.barb_pid)
        .map(|u| (u.id, g.unit_visible_to(u.id, 0)))
        .collect();
    assert!(
        here_covering > 0,
        "the raiders cover the settler's own tile: {seen:?}"
    );

    let retreats = ai.cornered_retreats(&g, settler, here, &hunted);
    // The contract, whatever this map happens to offer: never the tile it
    // already stands on, every choice STRICTLY better than staying, and
    // ordered so the least-covered is taken first.
    assert!(
        !retreats.contains(&here),
        "retreating to here is not retreating"
    );
    let mut previous = 0usize;
    for next in &retreats {
        let covering = hunted.raiders_covering(&g, *next);
        assert!(
            covering < here_covering
                || (covering == here_covering
                    && hunted.nearest(&g, *next) > hunted.nearest(&g, here)),
            "{next:?} is covered by {covering} against {here_covering} here — \
                 it is not strictly better than holding"
        );
        assert!(
            covering >= previous,
            "the least-covered tile is offered first"
        );
        previous = covering;
    }
}
