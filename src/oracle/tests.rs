use super::{Grant, Oracle, CITY_WORK_RADIUS, EXPANSION_TARGET, IDLE_RESERVE_TURNS};
use crate::ai::{AdvancedAi, Ai};
use crate::game::{Game, GovernorState, Item};
use crate::Pos;
use std::collections::BTreeSet;

/// An oracle seat forwards its plan to the dossier; the decision factors
/// under that plan come from the inner agent's journal, so the handle
/// has to reach it the same way `plan_report` does.
#[test]
fn an_attached_journal_reaches_the_wrapped_agent() {
    let journal = crate::reasoning::Journal::recording();
    let mut oracle = Oracle::new(AdvancedAi::new(), Grant::None);
    oracle.attach_journal(journal.handle());
    let mut g = Game::new(2, 24, 16, 8_100, 90, 0);
    let mut other = AdvancedAi::new();
    while g.winner.is_none() && g.turn <= 6 {
        let pid = g.current;
        if pid == 0 {
            oracle.take_turn(&mut g, pid);
        } else {
            other.take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
    }
    assert!(
        journal
            .since(0)
            .thoughts
            .iter()
            .any(|t| t.player == 0 && t.headline.starts_with("Grand strategy:")),
        "the seat run by the oracle wrapper never published its plan to the journal"
    );
}

#[test]
fn grant_ids_round_trip() {
    for grant in Grant::ALL {
        assert_eq!(Grant::from_id(grant.name()), Some(grant));
    }
    assert_eq!(Grant::from_id("nonsense"), None);
}

/// The control must be exactly the wrapped agent: same seed, same game.
/// If it were not, every measured headroom would include the harness.
#[test]
fn the_null_grant_changes_nothing() {
    let play = |grant: Option<Grant>| {
        let mut g = Game::new(2, 24, 16, 8_100, 90, 0);
        let mut plain = AdvancedAi::new();
        let mut wrapped = Oracle::new(AdvancedAi::new(), grant.unwrap_or(Grant::None));
        let mut other = AdvancedAi::new();
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            match (pid, grant) {
                (0, Some(_)) => wrapped.take_turn(&mut g, pid),
                (0, None) => plain.take_turn(&mut g, pid),
                _ => other.take_turn(&mut g, pid),
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        (g.turn, g.winner, g.score(0), g.score(1))
    };
    assert_eq!(
        play(None),
        play(Some(Grant::None)),
        "the null grant must reproduce the unwrapped agent exactly"
    );
}

/// A grant that never fires would measure the stock agent under another
/// name — the failure `elo.rs`'s provenance work exists to prevent. The
/// harness reports the count; this pins that it is not always zero.
///
/// ⚠ Sampled over several seeds rather than pinned to one. A grant fires
/// only if the empire is still holding units it could upgrade, so a single
/// game is really asserting *that empire survived and fell behind*, and any
/// change to the agent reshuffles that. On seed 8101 the agent ends with no
/// cities and no units — in both arms of a research-economy A/B, so the
/// pinned seed was already measuring an unrelated wipeout. What the test
/// means to claim is that the grant is not inert, and that is a claim about
/// ordinary games, not about one of them.
#[test]
fn the_modernity_grant_actually_fires() {
    let mut fired = 0u64;
    let mut surveyed = 0usize;
    for seed in 8_101..8_106u64 {
        let mut g = Game::new(4, 28, 18, seed, 140, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Modernity);
        let mut others = AdvancedAi::fleet(&g);
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            if pid == 0 {
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        surveyed += 1;
        fired += oracle.fired();
    }
    assert!(
        fired > 0,
        "the modernity grant never upgraded anything across {surveyed} games, \
             so a run would have measured the stock agent under an oracle's name"
    );
}

/// The taker grant must fire too. It measures a logistics failure, and a
/// logistics failure that never presents itself is not evidence about
/// logistics.
#[test]
fn the_taker_grant_actually_fires() {
    let mut g = Game::new(4, 28, 18, 8_103, 250, 2);
    let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Taker);
    let mut others = AdvancedAi::fleet(&g);
    while g.winner.is_none() && g.turn <= g.max_turns {
        let pid = g.current;
        if pid == 0 {
            oracle.take_turn(&mut g, pid);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
    }
    assert!(
        oracle.fired() > 0,
        "the taker grant never positioned anybody, so the run would have \
             measured the stock agent under an oracle's name"
    );
}

/// Attrition must fire too, and must grant health and nothing else.
#[test]
fn the_attrition_grant_fires_and_only_heals() {
    let mut g = Game::new(4, 28, 18, 8_104, 200, 2);
    let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Attrition);
    let mut others = AdvancedAi::fleet(&g);
    while g.winner.is_none() && g.turn <= 160 {
        let pid = g.current;
        if pid == 0 {
            let mut probe = g.clone();
            let before: Vec<u32> = probe.player_unit_ids(0);
            let gold = probe.players[0].gold;
            oracle.grant_attrition(&mut probe, 0);
            assert_eq!(
                before,
                probe.player_unit_ids(0),
                "the grant changed the roster"
            );
            assert_eq!(gold, probe.players[0].gold, "the grant moved the treasury");
            assert!(
                probe
                    .player_unit_ids(0)
                    .iter()
                    .all(|uid| probe.units[uid].hp == 100),
                "a healed empire must have no wounded units left"
            );
            oracle.take_turn(&mut g, pid);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
    }
    assert!(
        oracle.fired() > 0,
        "the attrition grant never healed anything"
    );
}

/// The ground grant must actually hand a city ground, must leave every
/// other city's territory alone, and must never reach past the workable
/// ring. A grant that never fires measures the stock agent under another
/// name; a grant that takes a rival's tiles measures conquest.
#[test]
fn the_ground_grant_fires_and_only_takes_neutral_ground() {
    let mut g = Game::new(4, 28, 18, 8_106, 200, 2);
    let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Ground);
    let mut others = AdvancedAi::fleet(&g);
    let mut ever_grew = false;
    while g.winner.is_none() && g.turn <= 120 {
        let pid = g.current;
        if pid == 0 {
            let mut probe = g.clone();
            let before: Vec<(u32, usize)> = probe
                .cities
                .values()
                .map(|city| (city.id, city.owned_tiles.len()))
                .collect();
            // A city can already hold ground outside the workable ring --
            // territory inherited when a neighbour was razed, for one --
            // so the radius assertion below must scope to what the grant
            // itself added, not to everything the city owns.
            let held_before: std::collections::BTreeSet<Pos> = probe
                .player_city_ids(0)
                .into_iter()
                .flat_map(|cid| probe.cities[&cid].owned_tiles.clone())
                .collect();
            let rival_ground: Vec<(Pos, Option<u32>)> = probe
                .map
                .tiles
                .values()
                .filter(|tile| {
                    tile.owner_city
                        .is_some_and(|cid| probe.cities.get(&cid).is_some_and(|c| c.owner != 0))
                })
                .map(|tile| (tile.pos, tile.owner_city))
                .collect();
            let gold = probe.players[0].gold;

            oracle.grant_ground(&mut probe, 0);

            assert_eq!(gold, probe.players[0].gold, "the grant charged for ground");
            for (pos, owner) in &rival_ground {
                assert_eq!(
                    probe.map.tiles[pos].owner_city, *owner,
                    "the grant took ground from another city"
                );
            }
            for cid in probe.player_city_ids(0) {
                let city = &probe.cities[&cid];
                for pos in &city.owned_tiles {
                    if !held_before.contains(pos) {
                        assert!(
                            probe.wdist(city.pos, *pos) <= CITY_WORK_RADIUS as i32,
                            "the grant reached past the workable ring"
                        );
                    }
                    assert_eq!(
                        probe.map.tiles[pos].owner_city,
                        Some(cid),
                        "owned_tiles and owner_city disagree after the grant"
                    );
                }
            }
            let after: Vec<(u32, usize)> = probe
                .cities
                .values()
                .map(|city| (city.id, city.owned_tiles.len()))
                .collect();
            if before != after {
                ever_grew = true;
            }
            oracle.take_turn(&mut g, pid);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
    }
    assert!(oracle.fired() > 0, "the ground grant never claimed a tile");
    assert!(ever_grew, "the grant never enlarged a city's territory");
}

/// The siting grant must actually move foundations, must never lose a
/// city's banked production when it does, must never leave two tiles
/// holding the same foundation, and must only ever improve a site.
#[test]
fn the_siting_grant_fires_and_never_strands_progress() {
    let mut g = Game::new(4, 28, 18, 8_107, 200, 2);
    let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Siting);
    let mut others = AdvancedAi::fleet(&g);
    let mut ever_moved = false;
    while g.winner.is_none() && g.turn <= 140 {
        let pid = g.current;
        if pid == 0 {
            let mut probe = g.clone();
            let banked: f64 = probe
                .player_city_ids(0)
                .into_iter()
                .map(|cid| probe.cities[&cid].production_progress.values().sum::<f64>())
                .sum();
            let before: Vec<(Pos, crate::name::Name)> = foundations_of(&probe, 0);
            let gold = probe.players[0].gold;

            oracle.grant_siting(&mut probe, 0);

            assert_eq!(gold, probe.players[0].gold, "the grant charged for siting");
            let after = foundations_of(&probe, 0);
            assert_eq!(
                before.len(),
                after.len(),
                "the grant created or destroyed a foundation"
            );
            let banked_after: f64 = probe
                .player_city_ids(0)
                .into_iter()
                .map(|cid| probe.cities[&cid].production_progress.values().sum::<f64>())
                .sum();
            assert!(
                (banked - banked_after).abs() < 1e-6,
                "moving a foundation stranded banked production: {banked} -> {banked_after}"
            );
            // Each district that moved must have moved somewhere better,
            // and a queued Item::District must point at the tile that now
            // holds its foundation.
            for cid in probe.player_city_ids(0) {
                for item in &probe.cities[&cid].queue {
                    if let Item::District { district, pos } = item {
                        assert_eq!(
                            probe.map.tiles[pos]
                                .district_foundation
                                .as_ref()
                                .map(|f| f.district.as_str()),
                            Some(district.as_str()),
                            "a queued district points at a tile with no matching foundation"
                        );
                    }
                }
            }
            if before != after {
                ever_moved = true;
            }
            oracle.take_turn(&mut g, pid);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
    }
    assert!(
        oracle.fired() > 0,
        "the siting grant never moved a foundation"
    );
    assert!(ever_moved, "no foundation ever changed tile");
}

fn foundations_of(g: &Game, pid: usize) -> Vec<(Pos, crate::name::Name)> {
    let mut out: Vec<(Pos, crate::name::Name)> = g
        .player_city_ids(pid)
        .into_iter()
        .flat_map(|cid| g.cities[&cid].owned_tiles.clone())
        .filter_map(|pos| {
            g.map.tiles[&pos]
                .district_foundation
                .as_ref()
                .map(|f| (pos, f.district))
        })
        .collect();
    out.sort();
    out
}

/// The expansion grant must actually hand out Settlers, must hand out
/// exactly one at a time, must stop at the target, and must grant nothing
/// else. A grant that quietly also moved the treasury would attribute
/// treasury's headroom to expansion.
#[test]
fn the_expansion_grant_fires_and_grants_only_settlers() {
    let mut g = Game::new(4, 28, 18, 8_108, 200, 2);
    let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Expansion);
    let mut others = AdvancedAi::fleet(&g);
    let mut ever_granted = false;
    while g.winner.is_none() && g.turn <= 140 {
        let pid = g.current;
        if pid == 0 {
            let mut probe = g.clone();
            let gold = probe.players[0].gold;
            let faith = probe.players[0].faith;
            let before: Vec<u32> = probe.player_unit_ids(0);
            let settlers_before = probe
                .player_unit_ids(0)
                .into_iter()
                .filter(|uid| probe.units[uid].kind == "settler")
                .count();
            let cities = probe.player_city_ids(0).len();

            oracle.grant_expansion(&mut probe, 0);

            assert_eq!(gold, probe.players[0].gold, "the grant moved the treasury");
            assert_eq!(faith, probe.players[0].faith, "the grant moved Faith");
            let after: Vec<u32> = probe.player_unit_ids(0);
            let added = after.len() - before.len();
            assert!(added <= 1, "the grant handed out {added} units at once");
            if added == 1 {
                ever_granted = true;
                assert!(
                    settlers_before == 0,
                    "the grant stacked a second settler on a seat that had one"
                );
                assert!(
                    cities < EXPANSION_TARGET,
                    "the grant fired at or above its own target"
                );
                let fresh: Vec<u32> = after
                    .iter()
                    .copied()
                    .filter(|uid| !before.contains(uid))
                    .collect();
                assert_eq!(fresh.len(), 1);
                assert_eq!(
                    probe.units[&fresh[0]].kind, "settler",
                    "the grant handed out something other than a settler"
                );
            }
            oracle.take_turn(&mut g, pid);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
    }
    assert!(
        oracle.fired() > 0,
        "the expansion grant never handed out a settler"
    );
    assert!(ever_granted, "no settler was ever observed being granted");
}

/// The confiscation must actually take Gold, must leave a working buffer
/// rather than emptying the treasury, and must take nothing else.
#[test]
fn the_idle_reserve_ablation_fires_and_leaves_a_buffer() {
    let mut g = Game::new(4, 28, 18, 8_109, 200, 2);
    let mut oracle = Oracle::new(AdvancedAi::new(), Grant::IdleReserve);
    let mut others = AdvancedAi::fleet(&g);
    let mut ever_took = false;
    while g.winner.is_none() && g.turn <= 140 {
        let pid = g.current;
        if pid == 0 {
            let mut probe = g.clone();
            let before = probe.players[0].gold;
            let faith = probe.players[0].faith;
            let units = probe.player_unit_ids(0);
            let cities = probe.player_city_ids(0);
            let income: f64 = cities
                .iter()
                .map(|cid| probe.city_yields(*cid).gold)
                .sum::<f64>()
                .max(1.0);

            oracle.confiscate_idle_reserve(&mut probe, 0);

            let after = probe.players[0].gold;
            assert!(after <= before + 1e-9, "the ablation ADDED Gold");
            assert_eq!(faith, probe.players[0].faith, "the ablation moved Faith");
            assert_eq!(
                units,
                probe.player_unit_ids(0),
                "the ablation changed the roster"
            );
            assert_eq!(
                cities,
                probe.player_city_ids(0),
                "the ablation changed the cities"
            );
            // Whatever it leaves must still be a working buffer.
            assert!(
                after >= (income * IDLE_RESERVE_TURNS).min(before) - 1e-6,
                "the ablation cut into the buffer it is supposed to leave: \
                     {after} against {income}/turn"
            );
            if after < before - 1e-9 {
                ever_took = true;
            }
            oracle.take_turn(&mut g, pid);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
    }
    assert!(
        oracle.fired() > 0,
        "the idle-reserve ablation never took anything"
    );
    assert!(ever_took, "no confiscation was ever observed");
}

#[test]
fn the_suzerain_grant_uses_minimal_raw_stock_under_amani() {
    let game = Game::new_full(2, 26, 16, 8_111, 200, 1, false);
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .unwrap()
        .id;
    let city_state = game.player_city_ids(minor)[0];

    let fixture = |puppeteer: Option<bool>| {
        let mut game = game.clone();
        game.turn = 100;
        game.players[0].met.insert(minor);
        game.players[minor].met.insert(0);
        game.players[1].envoys = vec![(minor, 5)];
        if let Some(puppeteer) = puppeteer {
            game.players[0].governor_roster.insert(
                "amani".to_string(),
                GovernorState {
                    city: Some(city_state),
                    assigned_turn: 0,
                    disabled_until: 0,
                    promotions: if puppeteer {
                        BTreeSet::from(["puppeteer".to_string()])
                    } else {
                        BTreeSet::new()
                    },
                },
            );
        }
        game
    };

    for (puppeteer, expected_raw) in [(None, 6), (Some(false), 4), (Some(true), 1)] {
        let mut game = fixture(puppeteer);
        let free = game.players[0].envoys_free;
        let rival = game.players[1].envoys.clone();
        let gold = game.players[0].gold;
        let faith = game.players[0].faith;
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Suzerain);

        oracle.grant_suzerain(&mut game, 0);

        let raw = game.players[0]
            .envoys
            .iter()
            .find(|(at, _)| *at == minor)
            .map(|(_, count)| *count)
            .unwrap_or(0);
        assert_eq!(raw, expected_raw, "Amani mode {puppeteer:?}");
        assert_eq!(game.envoys_at(0, minor), 6, "Amani mode {puppeteer:?}");
        assert_eq!(game.suzerain_of(minor), Some(0));
        assert_eq!(oracle.fired(), 1);
        assert_eq!(oracle.raw_envoys_granted(), expected_raw as u64);
        assert_eq!(game.players[0].envoys_free, free);
        assert_eq!(game.players[1].envoys, rival);
        assert_eq!(game.players[0].gold, gold);
        assert_eq!(game.players[0].faith, faith);
    }
}

/// The compound suzerainty ceiling must actually take every met state and
/// may create only focal raw Envoys. Moving Gold, Faith, or rival stock
/// would bundle a second resource axis into the result.
#[test]
fn the_suzerain_grant_fires_and_only_moves_its_own_envoys() {
    let mut g = Game::new(4, 28, 18, 8_110, 200, 4);
    let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Suzerain);
    let mut others = AdvancedAi::fleet(&g);
    let mut ever_gained = false;
    while g.winner.is_none() && g.turn <= 140 {
        let pid = g.current;
        if pid == 0 {
            let mut probe = g.clone();
            let gold = probe.players[0].gold;
            let faith = probe.players[0].faith;
            let rival_envoys: Vec<Vec<(usize, i64)>> = probe
                .players
                .iter()
                .filter(|p| p.id != 0)
                .map(|p| p.envoys.clone())
                .collect();
            let met: Vec<usize> = probe
                .players
                .iter()
                .filter(|m| m.is_minor && m.alive && !m.is_barbarian)
                .map(|m| m.id)
                .filter(|m| probe.has_met(0, *m))
                .collect();
            let before = met
                .iter()
                .filter(|m| probe.suzerain_of(**m) == Some(0))
                .count();

            oracle.grant_suzerain(&mut probe, 0);

            assert_eq!(gold, probe.players[0].gold, "the grant moved Gold");
            assert_eq!(faith, probe.players[0].faith, "the grant moved Faith");
            let rivals_after: Vec<Vec<(usize, i64)>> = probe
                .players
                .iter()
                .filter(|p| p.id != 0)
                .map(|p| p.envoys.clone())
                .collect();
            assert_eq!(
                rival_envoys, rivals_after,
                "the grant moved a rival's envoys"
            );
            for minor in &met {
                assert_eq!(
                    probe.suzerain_of(*minor),
                    Some(0),
                    "a met city-state was left unclaimed"
                );
            }
            if !met.is_empty() && before < met.len() {
                ever_gained = true;
            }
            oracle.take_turn(&mut g, pid);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
    }
    assert!(
        oracle.fired() > 0,
        "the suzerain grant never placed an envoy"
    );
    assert!(ever_gained, "no suzerainty was ever actually gained");
}

/// The grant must be modernization and nothing else: no Gold, no health,
/// no extra units. Otherwise a measured headroom would be the bundle's,
/// not the subsystem's.
#[test]
fn the_modernity_grant_changes_only_the_unit_types() {
    let mut g = Game::new(4, 28, 18, 8_102, 200, 2);
    let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Modernity);
    let mut others = AdvancedAi::fleet(&g);
    let mut checked = 0usize;
    while g.winner.is_none() && g.turn <= 160 {
        let pid = g.current;
        if pid == 0 {
            let mut probe = g.clone();
            let before: Vec<(u32, i32)> = probe.units.iter().map(|(id, u)| (*id, u.hp)).collect();
            let gold = probe.players[0].gold;
            oracle.grant_modernity(&mut probe, 0);
            let after: Vec<(u32, i32)> = probe.units.iter().map(|(id, u)| (*id, u.hp)).collect();
            assert_eq!(before, after, "the grant changed unit count or health");
            assert_eq!(gold, probe.players[0].gold, "the grant moved the treasury");
            checked += 1;
            oracle.take_turn(&mut g, pid);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &crate::game::Action::EndTurn);
        }
    }
    assert!(checked > 50, "only {checked} turns exercised");
}
