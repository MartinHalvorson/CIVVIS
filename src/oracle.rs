//! Oracle ablation: find which subsystem actually limits the agent.
//!
//! Every attempt to strengthen this AI has failed one of two ways. The first
//! is documented in `docs/AI_GAPS.md`: the evaluator could not see the
//! decision it was ranking, so no amount of training could rank it. The
//! second is the same error one level up — the change addressed a constraint
//! that was not binding. `advanced_relief_scoped` is the worked example: it
//! cut the force groups frozen far from an emergency from 19.0% of
//! force-group turns to 10.4%, exactly as designed, and measured
//! Elo-equivalent -6 over 120 mirrored maps.
//!
//! Both mistakes are avoidable by asking a cheaper question first. For a
//! subsystem S, what would this agent's win rate be if S simply could not
//! fail? The gap between the stock agent and the S-oracle is the *headroom*
//! in S: an upper bound on everything any amount of work on S could ever be
//! worth. A subsystem whose oracle wins nothing is a settled question, and
//! settling one costs a batch of games instead of a design, an
//! implementation and a pre-registered run.
//!
//! These grants cheat, deliberately and visibly. They are diagnostics, never
//! entrants: [`Oracle`] is constructed only by `src/bin/ablate.rs`, and
//! nothing in `elo.rs` can name one, so an oracle result can never be
//! recorded as an agent's rating.
//!
//! Each grant is applied at the start of the seat's turn, before the wrapped
//! agent plays, because `AdvancedAi::take_turn` ends its own turn.
use crate::ai::{Ai, PlanReport};
use std::collections::BTreeSet;
use crate::game::Game;
use crate::Pos;

/// The engine's workable ring. `plot_purchase_cost` prices rings one through
/// three and nothing beyond, so this is exactly the ground a citizen could
/// ever be assigned to.
const CITY_WORK_RADIUS: usize = 3;

/// A capability granted to the wrapped agent for free.
///
/// Each one is chosen to bound one measured failure, so a null result closes
/// that question rather than leaving it open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Grant {
    /// Nothing. The control: an ablation run must measure this at parity, or
    /// the harness is reporting its own noise as headroom.
    None,
    /// Every unit that has an upgrade takes it, free, every turn.
    ///
    /// Bounds the standing-army problem. Measured over 48 six-player games,
    /// 81% of the 4,912 military units alive at the end were three or more
    /// eras behind the world era, and 15% had a researched upgrade they had
    /// never taken. This asks what perfect modernization would be worth
    /// without asking anyone to pay for it.
    Modernity,
    /// Whenever an enemy city is standing open, put a melee unit next to it.
    ///
    /// Bounds the siege-conversion problem. Over the same games the AI left
    /// 210 cities at zero garrison and had a melee unit adjacent with
    /// movement in hand on 46 of them — 22%. The engine's own note says the
    /// two candidate explanations are "declines a capture it could make" and
    /// "never has anyone there to make it". This grant removes the second
    /// one by teleporting the nearest melee unit into position, so what the
    /// win rate does next distinguishes them.
    Taker,
    /// Every unit starts every turn at full health.
    ///
    /// Bounds combat micro — `AI_GAPS.md` item 4, which its own re-sequencing
    /// leaves explicitly unmeasured: "treat it as unknown rather than cheap".
    /// Retreat-and-heal cycling, refusing an unfavourable trade, and pulling a
    /// wounded unit out before it is killed all cash out as the same thing:
    /// health a better player would still have. This grants the outcome
    /// without granting the skill, so the win rate says what the skill is
    /// worth at most.
    ///
    /// It does not make units immortal. A blow large enough to kill still
    /// kills; what disappears is accumulated damage.
    Attrition,
    /// A large, unearned pile of Gold and Faith every turn.
    ///
    /// Not a subsystem. This is the instrument's calibration: it grants an
    /// advantage nobody would argue is small, so a run can establish that the
    /// harness detects an advantage at all. Without it a null from any other
    /// grant is ambiguous between "this subsystem does not limit the agent"
    /// and "this design cannot resolve anything", and those call for opposite
    /// next steps.
    ///
    /// Deliberately crude and deliberately huge — a stock empire finishes
    /// these games with a few hundred Gold, so this is worth orders of
    /// magnitude more than any honest improvement to any subsystem. If it
    /// does not register, nothing else measured here means anything.
    Treasury,
    /// Every city instantly owns every unclaimed tile inside its workable
    /// three-ring radius.
    ///
    /// Bounds the *ceiling* that #532's saturation result was conditional on.
    /// `city_decision_census` measured the citizen governor claiming 89.3% of
    /// its city's food ceiling and 99.5% of its production ceiling — but that
    /// ceiling is computed over the tiles the city **already owns**, so a
    /// governor allocating 89% of a poor endowment still reads as saturated.
    /// Five scripted city-strategy arms measured null against that ceiling;
    /// this asks whether the ceiling itself is the thing that binds.
    ///
    /// Border growth is paid for in accumulated Culture and plot purchase in
    /// Gold, and both are slow. This grants the outcome — the ground — without
    /// granting the Culture or the Gold, so the win rate says what perfect
    /// territorial acquisition is worth at most.
    ///
    /// It deliberately never takes a tile another city already owns. That
    /// would grant conquest and the measured headroom would belong to the
    /// bundle rather than to border growth.
    Ground,
}

impl Grant {
    pub const ALL: [Grant; 6] = [
        Grant::None,
        Grant::Modernity,
        Grant::Taker,
        Grant::Attrition,
        Grant::Treasury,
        Grant::Ground,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Grant::None => "none",
            Grant::Modernity => "modernity",
            Grant::Taker => "taker",
            Grant::Attrition => "attrition",
            Grant::Treasury => "treasury",
            Grant::Ground => "ground",
        }
    }

    pub fn from_id(id: &str) -> Option<Grant> {
        Self::ALL.into_iter().find(|grant| grant.name() == id)
    }
}

/// One wrapped agent plus the capability it is being handed.
pub struct Oracle<A: Ai> {
    inner: A,
    grant: Grant,
    /// How many times the grant actually did something. A grant that never
    /// fires measures the stock agent under another name, which is the exact
    /// failure the provenance work in `elo.rs` exists to prevent — so the
    /// harness reports it rather than letting a null be ambiguous.
    fired: u64,
}

impl<A: Ai> Oracle<A> {
    pub fn new(inner: A, grant: Grant) -> Oracle<A> {
        Oracle {
            inner,
            grant,
            fired: 0,
        }
    }

    /// Times the grant changed the position.
    pub fn fired(&self) -> u64 {
        self.fired
    }

    /// Walk every unit to the top of its unlocked upgrade chain, free, with
    /// none of the frictions a real upgrade pays.
    ///
    /// Deliberately *not* routed through `Action::UpgradeUnit`. That path
    /// requires the unit to be in friendly territory with movement in hand
    /// and outside a zone of control — faithful to Civ 6, and exactly the
    /// friction a field army never satisfies, which is a large part of why
    /// 81% of the standing army finishes three or more eras stale. An oracle
    /// that respected those preconditions would measure "free upgrades for
    /// the garrison" and report a null for the wrong reason: the first
    /// version of this grant did precisely that and fired zero times, which
    /// `the_modernity_grant_actually_fires` caught.
    ///
    /// Strength, range and every other combat property are read from
    /// `rules.units[kind]` at query time, so rewriting `kind` is what
    /// modernization *is* here. HP is a 0..100 condition independent of the
    /// unit type and is deliberately preserved: this grants a better army,
    /// not a healed one.
    fn grant_modernity(&mut self, g: &mut Game, pid: usize) {
        for uid in g.player_unit_ids(pid) {
            // Bounded by the length of an upgrade chain; the guard only stops
            // a cycle in a malformed ruleset.
            for _ in 0..16 {
                let kind = g.units[&uid].kind.clone();
                let Some(target) = g.unit_upgrade_target(pid, &kind) else {
                    break;
                };
                if target == kind {
                    break;
                }
                if let Some(unit) = g.units.get_mut(&uid) {
                    unit.kind = target;
                }
                self.fired += 1;
            }
        }
    }

    /// Hand over Gold and Faith at a rate no economy in these games reaches.
    /// Hand every city the unclaimed ground inside its workable radius.
    ///
    /// Radius three is the engine's own workable ring — `plot_purchase_cost`
    /// prices rings one through three and nothing beyond — so this grants
    /// exactly the tiles a citizen could ever be assigned to and no more.
    /// Reached by three rounds of neighbour expansion rather than by scanning
    /// the map, because this runs once per city per turn.
    fn grant_ground(&mut self, g: &mut Game, pid: usize) {
        for cid in g.player_city_ids(pid) {
            let Some(city) = g.cities.get(&cid) else {
                continue;
            };
            let mut frontier = vec![city.pos];
            let mut seen: BTreeSet<Pos> = frontier.iter().copied().collect();
            for _ in 0..CITY_WORK_RADIUS {
                let mut next = Vec::new();
                for pos in frontier.drain(..) {
                    for neighbor in g.nbrs(pos) {
                        if seen.insert(neighbor) {
                            next.push(neighbor);
                        }
                    }
                }
                frontier = next;
            }
            for pos in seen {
                // Never take ground another city holds: that would be a
                // conquest grant, and the headroom would belong to the bundle.
                let unclaimed = g
                    .map
                    .tiles
                    .get(&pos)
                    .is_some_and(|tile| tile.owner_city.is_none());
                if !unclaimed {
                    continue;
                }
                if let Some(tile) = g.map.tiles.get_mut(&pos) {
                    tile.owner_city = Some(cid);
                }
                if let Some(city) = g.cities.get_mut(&cid) {
                    if !city.owned_tiles.contains(&pos) {
                        city.owned_tiles.push(pos);
                    }
                }
                self.fired += 1;
            }
        }
    }

    fn grant_treasury(&mut self, g: &mut Game, pid: usize) {
        g.players[pid].gold += 200.0;
        g.players[pid].faith += 100.0;
        self.fired += 1;
    }

    /// Restore every unit to full health.
    ///
    /// Deliberately health only: no movement refresh, no extra attacks, no
    /// promotions. Those would grant tempo and experience alongside
    /// preservation and the measured headroom would belong to the bundle.
    fn grant_attrition(&mut self, g: &mut Game, pid: usize) {
        for uid in g.player_unit_ids(pid) {
            if let Some(unit) = g.units.get_mut(&uid) {
                if unit.hp < 100 {
                    unit.hp = 100;
                    self.fired += 1;
                }
            }
        }
    }

    /// Every enemy city this empire is actually reducing gets the nearest
    /// melee unit placed beside it.
    ///
    /// The trigger is *at war with its owner*, not *already open* and not
    /// *currently under siege*. Both narrower versions were tried and fired
    /// zero times: a spent garrison heals before the grant comes round again,
    /// and conditioning on an active siege only positions a taker where the
    /// agent already chose to attack, which is the case it least needs help
    /// with.
    ///
    /// What is being bounded is not "walk into the open city" — the attack
    /// evaluator already pays 520+ for a capture. It is the logistics
    /// failure underneath: over 48 six-player games the AI left 210 cities at
    /// zero garrison and had a melee unit adjacent with movement in hand on
    /// 46 of them, 22%. So this keeps one melee unit standing at every enemy
    /// city, permanently, and lets the agent's own evaluator do the rest.
    ///
    /// It may well *lose*. A lone unit parked beside a defended city dies,
    /// and the grant takes no view on whether being there is wise. That is a
    /// real outcome for an upper bound to have, and the harness reports it as
    /// HARMFUL rather than as evidence that logistics are fine.
    fn grant_taker(&mut self, g: &mut Game, pid: usize) {
        let open: Vec<(u32, Pos)> = g
            .cities
            .values()
            .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner))
            .map(|city| (city.id, city.pos))
            .collect();
        for (_, city_pos) in open {
            // Somewhere legal to stand: an adjacent land tile with nobody on
            // it. Sorted so the grant is deterministic across runs.
            let mut approaches: Vec<Pos> = g
                .nbrs(city_pos)
                .into_iter()
                .filter(|position| {
                    g.map.get(*position).is_some_and(|tile| {
                        g.rules.is_passable(tile) && !g.rules.is_water(tile)
                    }) && g.units_at(*position).is_empty()
                })
                .collect();
            approaches.sort_unstable();
            let Some(landing) = approaches.first().copied() else {
                continue;
            };
            // The nearest melee unit that is not already in contact. One that
            // is already adjacent needs no help, and moving it would be the
            // harness playing the game rather than removing a constraint.
            let mut candidates: Vec<(i32, u32)> = g
                .units
                .values()
                .filter(|unit| unit.owner == pid)
                .filter(|unit| {
                    let spec = &g.rules.units[unit.kind.as_str()];
                    spec.class == "military" && spec.is_melee_capable()
                })
                .filter(|unit| g.wdist(unit.pos, city_pos) > 1)
                .map(|unit| (g.wdist(unit.pos, city_pos), unit.id))
                .collect();
            candidates.sort_unstable();
            let Some((_, uid)) = candidates.first().copied() else {
                continue;
            };
            // Placed, not marched, and through `relocate` rather than by
            // writing `pos`. The engine keeps a tile->units occupancy index;
            // a bare `pos` write leaves the unit listed at its old tile, and
            // `units_at` then returns an id `units` no longer holds. That
            // panics, but only much later and in unrelated code — combat
            // resolution, disaster damage, support auras — which is how the
            // first version of this grant crashed eight worker threads in
            // four different files.
            //
            // Movement for the turn is already refreshed at this point, so
            // the unit arrives able to act. That is the whole point: the
            // constraint being removed is "the piece that could take the city
            // is not there, or is already spent".
            g.relocate(uid, landing);
            self.fired += 1;
        }
    }
}

impl<A: Ai> Ai for Oracle<A> {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        if g.winner.is_none() && !g.players[pid].is_barbarian && !g.players[pid].is_minor {
            match self.grant {
                Grant::None => {}
                Grant::Modernity => self.grant_modernity(g, pid),
                Grant::Taker => self.grant_taker(g, pid),
                Grant::Attrition => self.grant_attrition(g, pid),
                Grant::Treasury => self.grant_treasury(g, pid),
                Grant::Ground => self.grant_ground(g, pid),
            }
        }
        self.inner.take_turn(g, pid);
    }

    fn strategy_label(&self) -> Option<&'static str> {
        self.inner.strategy_label()
    }

    fn plan_report(&self) -> Option<PlanReport> {
        self.inner.plan_report()
    }

    fn review_census(&self) -> Option<crate::strategic::ReviewCensus> {
        self.inner.review_census()
    }
}

#[cfg(test)]
mod tests {
    use super::{Grant, Oracle, CITY_WORK_RADIUS};
    use crate::ai::{AdvancedAi, Ai};
    use crate::game::Game;
    use crate::Pos;

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
    #[test]
    fn the_modernity_grant_actually_fires() {
        let mut g = Game::new(4, 28, 18, 8_101, 140, 2);
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
        assert!(
            oracle.fired() > 0,
            "the modernity grant never upgraded anything, so the run would \
             have measured the stock agent under an oracle's name"
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
                assert_eq!(before, probe.player_unit_ids(0), "the grant changed the roster");
                assert_eq!(gold, probe.players[0].gold, "the grant moved the treasury");
                assert!(
                    probe.player_unit_ids(0).iter().all(|uid| probe.units[uid].hp == 100),
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
        assert!(oracle.fired() > 0, "the attrition grant never healed anything");
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
                let before: Vec<(u32, i32)> =
                    probe.units.iter().map(|(id, u)| (*id, u.hp)).collect();
                let gold = probe.players[0].gold;
                oracle.grant_modernity(&mut probe, 0);
                let after: Vec<(u32, i32)> =
                    probe.units.iter().map(|(id, u)| (*id, u.hp)).collect();
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
}
