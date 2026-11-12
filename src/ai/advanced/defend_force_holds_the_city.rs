//! `defend-force-holds-the-city`: every threatened city of ours is held on
//! posts. A shooter takes the City Center, melee soldiers block the ring
//! tiles the enemy's melee can reach, the other shooters stand where that
//! melee cannot reach, and every soldier strikes from its post.
//!
//! **Why (live Deity Gran Colombia, 62 games of 2026-11-12, 100 cities lost;
//! the census of #5198).**
//! - At the fall, the Defend force was in Engage rather than Hold at 53 of
//!   the 62 lost cities that had one. Its objective is the nearest hostile
//!   within eight tiles of the city (`project_forces`), so it marched out,
//!   and its readiness was 40% or less on 42.
//! - 9 cities fell with plenty of force nearby but not at the walls. In
//!   civvis-20261112T070705Z, 23 soldiers were within 8 tiles of Maracaibo
//!   and one Trebuchet was in the city.
//! - 16 fell with the centre held and a comparable local force.
//! - The Anvil forms only around the single `plan.threatened_city`. Its
//!   spared posts lay 4+ tiles from the city on 357 of 446.
//! - #5198 (`centre-keeps-its-garrison`) kept the centre but forbade its
//!   melee every blow. Its paired 60-seed read was neutral (whole-game
//!   cities lost 1.42 -> 1.80, z +1.2), and on posed boards enemy kills fell
//!   65 -> 50.
//!
//! **Scope.** A city of ours is held when it has a Defend row on the
//! objective board, or when a visible enemy land soldier at war stands within
//! [`CENTRE_THREAT_REACH`] of it (`centre_threatened`). The cities are taken
//! nearest-threat first. Each takes as its candidates the land members of
//! its Defend and Relieve task forces, and our other land soldiers within
//! [`HOLD_RADIUS`]. Excluded are soldiers the battle planner ordered this
//! turn, siege members, a settler's guard, a civilian's escort, and any unit
//! another pass of this turn claimed (air assault, falling-city takers,
//! raiders, hunters, the pre-kill withdrawal). A unit holds one city at most.
//!
//! **Posts,** drawn once a turn just before the unit loop, after every other
//! claim:
//! 1. **Centre.** A shooter keeps it. A melee occupant swaps with an adjacent
//!    shooter (`Action::Swap`). An empty centre takes the nearest shooter that
//!    can reach it this turn, else the strongest melee that can.
//! 2. **Ring.** An enemy takes a city only with a melee unit beside the
//!    centre. So every ring tile that a visible enemy melee unit can reach
//!    next turn (`threat_reach`), or already stands beside, gets a melee or
//!    anti-cavalry soldier, nearest-threat first, the strongest that can reach
//!    it. Cavalry fills in after them.
//!    A post's expected cost is the sum, over the enemy melee that reach
//!    it, of the fitted host's chance to strike
//!    (`battle_solver::host_enemy::strike_probability`, #5197: by its class,
//!    owner, health, home ground, the strength edge over the post's holder,
//!    and whether it strikes without moving, which alone multiplies the odds
//!    about six-fold) times the expected blow. A soldier is not posted where
//!    that sum meets its health.
//! 3. **Shooters.** The others stand within two tiles of the centre, on the
//!    tile with the least such cost: off the enemy melee's reach first, then
//!    nearest the city.
//! 4. **The rest** march to the city: candidates out of reach of every post
//!    go to the cheapest free tile within two, instead of the Engage march.
//!
//! **Blows,** in each unit's own step, from its post. Targets beside the city
//! come first, then a kill, then the most damage.
//! - A shooter fires from where it stands.
//! - A ring soldier strikes an adjacent enemy when it survives the reply at
//!   the top of the roll. A kill carries it forward, which the doctrine allows
//!   because the blow kills and it lives.
//! - The centre's melee strikes only a blow that cannot kill even at the top
//!   of the roll (no advance) and that it survives, or a surviving kill when
//!   another candidate of the same city can step into the emptied centre this
//!   turn. That candidate is moved in at once.
//! - A marching soldier strikes only a kill it survives.
//!
//! **Who wins where.**
//! - A posted unit's step returns before `centre-keeps-its-garrison`, the
//!   siege step (and the Anvil within it), the attack scan and the Engage
//!   march. So the hold decides each posted unit once.
//! - The Anvil stands down at a held city (`anvil_defense_city` returns
//!   `None`), and its add-ons (`anvil-spares-the-one-blow`,
//!   `anvil-garrisons-its-melee`) with it.
//! - The battle planner and the kill prepass run first. Their units are not
//!   posted, and `centre-keeps-its-garrison`'s prepass veto on a centre's
//!   melee still applies there. The hold's own step then grants the centre
//!   the blows described above.
//! - Unposted soldiers keep their ordinary step, including
//!   `centre-keeps-its-garrison`'s fill.
//!
//! One journal line a city a turn names the posts. Off, nothing changes: the
//! plan is cleared and every reading returns before it looks at the board.
//!
//! **Version two (`defend-force-holds-the-city-2`).** Version one read
//! neutral to worse in the paired 60-seed whole-game sim:
//! - cities lost 1.42 -> 1.62, units lost 20.95 -> 23.20, lost to barbarians
//!   3.13 -> 3.90, civilians lost 3.10 -> 3.78, kill:loss 5.86 -> 3.51;
//! - 491 lost units were last ordered by its posts, 435 of them within two
//!   tiles of a hostile;
//! - the battle planner's positions plan crossed its posts 250 times.
//!
//! Version two carries version one's reads, and the versions exclude each
//! other. It changes five things:
//! 1. **Capturers only.** Barbarians and the Free Cities form no hold. A city
//!    is held for a visible land soldier of a major or a city-state at war
//!    within [`CENTRE_THREAT_REACH`], or for a Defend row with one within
//!    [`HOLD_ENEMY_RADIUS`]. Where no capturer can strike the city or its
//!    ring next turn (the plan's `quiet` cities), a posted soldier off the
//!    centre may leave its post for a raider it kills and survives
//!    (`hold_raider_kill`). A soldier stacked with one of our civilians off a
//!    city keeps guarding it and is not posted.
//! 2. **The ring only when blocking matters:** a melee capturer beside the
//!    centre, or the capturers' blows that can land on the city over
//!    [`CAPTURE_TURNS`] enemy turns meeting its health and walls.
//!    Otherwise melee and the extra shooters stand ready within two tiles on
//!    the tile with the lowest death chance before our next turn
//!    (`priced_risk::death_chance`, each visible enemy that can strike the
//!    tile landing at the fitted host's odds). The centre rule is version
//!    one's.
//! 3. **Outclassed:** when the strongest visible enemy within
//!    [`OUTCLASSED_RADIUS`] is at least [`OUTCLASSED_EDGE`] stronger than
//!    our strongest candidate, a ring post needs a death chance under
//!    [`OUTCLASSED_RING_DEATH_LIMIT`]; otherwise under [`RING_DEATH_LIMIT`].
//!    A soldier that dies to one blow blocks nothing.
//! 4. **The positions plan leaves the posts alone.** The soldiers this
//!    turn's posts may take are read before the battle planner
//!    (`preview_hold_members`) and left out of its positions plan.
//! 5. Its journal line says whether the city can fall, whether we are
//!    outclassed, and whether it is quiet.

use super::battle_solver::host_enemy::{strike_probability, HostClass, Owner, StrikeFeatures};
use super::centre_keeps_its_garrison::CENTRE_THREAT_REACH;
use super::objective_board::ObjectiveKey;
use super::*;
use crate::game::expected_damage;

/// Our land soldiers this close to a held city join its posts (its Defend
/// and Relieve task force members join from anywhere).
pub(crate) const HOLD_RADIUS: i32 = 3;
/// Visible enemy soldiers this close to a held city shape its ring.
pub(crate) const HOLD_ENEMY_RADIUS: i32 = 6;
/// Version two: the ring is not posted where the post's death chance before
/// our next turn (`priced_risk::death_chance`) reaches this.
pub(crate) const RING_DEATH_LIMIT: f64 = 0.5;
/// Version two: the same limit when the defenders are outclassed.
pub(crate) const OUTCLASSED_RING_DEATH_LIMIT: f64 = 0.15;
/// Version two: the defenders are outclassed when the strongest visible
/// enemy within [`OUTCLASSED_RADIUS`] of the city is at least this much
/// stronger than our strongest candidate.
pub(crate) const OUTCLASSED_EDGE: f64 = 15.0;
pub(crate) const OUTCLASSED_RADIUS: i32 = 4;
/// Version two: a city is about to fall when the visible capturers' blows
/// that can land on it over this many enemy turns meet its health and walls.
pub(crate) const CAPTURE_TURNS: f64 = 2.0;

/// What a post is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum HoldRole {
    Centre,
    Ring,
    Shooter,
    March,
}

/// One unit's post this turn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HoldPost {
    pub city: u32,
    pub at: Pos,
    pub role: HoldRole,
}

/// This turn's posts, drawn once before the unit loop.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct HoldPlan {
    pub turn: u32,
    pub posts: BTreeMap<u32, HoldPost>,
    /// The held cities, nearest threat first.
    pub cities: Vec<u32>,
    /// Version two: held cities no capturer can reach next turn, where a
    /// posted soldier may leave its post to kill a raider it survives.
    pub quiet: BTreeSet<u32>,
}

impl HoldPlan {
    pub(crate) fn holds(&self, cid: u32) -> bool {
        self.cities.contains(&cid)
    }
}

fn is_shooter(g: &Game, uid: u32) -> bool {
    g.units
        .get(&uid)
        .is_some_and(|unit| g.rules.units[unit.kind].has_ranged_attack())
}

/// A ring soldier: melee-capable without a ranged attack; melee and
/// anti-cavalry before cavalry.
fn ring_rank(g: &Game, uid: u32) -> Option<u8> {
    let unit = g.units.get(&uid)?;
    let spec = &g.rules.units[unit.kind];
    if !spec.is_melee_capable() || spec.has_ranged_attack() {
        return None;
    }
    Some(match spec.promotion_class.as_str() {
        "melee" | "anti_cavalry" => 0,
        _ => 1,
    })
}

/// A tile one of our soldiers may be posted on: passable land, no enemy on
/// it, and none of our soldiers but `uid`.
fn open_for(g: &Game, pid: usize, uid: u32, pos: Pos) -> bool {
    g.map
        .get(pos)
        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        && g.unit_ids_at(pos).iter().all(|id| {
            let other = &g.units[id];
            *id == uid || (other.owner == pid && g.rules.units[other.kind].class != "military")
        })
}

fn can_reach(g: &Game, uid: u32, pos: Pos) -> bool {
    g.units.get(&uid).is_some_and(|unit| unit.pos == pos) || g.reachable(uid).contains(&pos)
}

/// How a blow from a post ranks: beside the city, kills, damage, then the
/// tile.
type BlowKey = (bool, bool, i64, std::cmp::Reverse<Pos>);

/// A visible enemy soldier near a held city, and its reach next turn.
struct Threat {
    uid: u32,
    pos: Pos,
    melee: bool,
    strength: f64,
    reach: BTreeSet<Pos>,
    /// What the fitted host's strike decision reads about the unit.
    hp: i32,
    class: HostClass,
    owner: Owner,
    home: bool,
    /// Version two: a civilization that can take the city (a major or a
    /// city-state), not barbarians or the Free Cities.
    capturer: bool,
    /// Its blow: melee strength, or its ranged attack for a shooter.
    attack: f64,
    /// Its strike range: 1 for melee.
    range: i32,
}

impl AdvancedAi {
    /// The held cities' posts, read by the unit step. Empty with the gene off.
    pub(super) fn defend_hold_holds(&self, cid: u32) -> bool {
        self.defend_hold_on() && self.defend_hold_plan.holds(cid)
    }

    /// Either version of the family plays (version two carries version
    /// one's reads).
    pub(super) fn defend_hold_on(&self) -> bool {
        self.defend_force_holds_the_city || self.defend_force_holds_the_city_2
    }

    /// Visible enemy soldiers at war within [`HOLD_ENEMY_RADIUS`] of `at`.
    /// `all_reach` (version two) reads every one's reach, not only melee's.
    fn hold_threats(&self, g: &Game, pid: usize, at: Pos, all_reach: bool) -> Vec<Threat> {
        let mut out = Vec::new();
        for tile in g.wdisk(at, HOLD_ENEMY_RADIUS) {
            for id in g.unit_ids_at(tile) {
                let Some(enemy) = g.units.get(id) else {
                    continue;
                };
                let spec = &g.rules.units[enemy.kind];
                if enemy.owner == pid
                    || enemy.hp <= 0
                    || spec.class != "military"
                    || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    || !g.is_at_war(pid, enemy.owner)
                    || !self.battlefront_unit_visible(g, pid, *id)
                {
                    continue;
                }
                let melee = spec.is_melee_capable() && !spec.has_ranged_attack();
                out.push(Threat {
                    uid: *id,
                    pos: enemy.pos,
                    melee,
                    strength: g.unit_strength(enemy, false),
                    hp: enemy.hp,
                    class: match spec.promotion_class.as_str() {
                        "light_cavalry" | "heavy_cavalry" => HostClass::Cavalry,
                        _ if spec.siege => HostClass::Siege,
                        _ if spec.has_ranged_attack() => HostClass::Ranged,
                        _ => HostClass::Melee,
                    },
                    owner: {
                        // As the solver reads it (`battle_solver/extract.rs`).
                        let player = &g.players[enemy.owner];
                        if player.is_minor || player.is_free_city {
                            Owner::CityState
                        } else if player.is_barbarian {
                            Owner::Barbarian
                        } else {
                            Owner::Major
                        }
                    },
                    home: std::iter::once(enemy.pos)
                        .chain(g.nbrs(enemy.pos))
                        .any(|tile| {
                            g.city_at(tile)
                                .is_some_and(|cid| g.cities[&cid].owner == enemy.owner)
                        }),
                    reach: if melee || all_reach {
                        g.threat_reach(*id).into_iter().collect()
                    } else {
                        BTreeSet::new()
                    },
                    capturer: {
                        let player = &g.players[enemy.owner];
                        !player.is_barbarian && !player.is_free_city
                    },
                    attack: if spec.has_ranged_attack() {
                        spec.ranged_attack_strength().max(spec.bombard_strength)
                    } else {
                        g.unit_strength(enemy, false)
                    },
                    range: if spec.has_ranged_attack() {
                        g.unit_attack_range(*id).max(1)
                    } else {
                        1
                    },
                });
            }
        }
        out.sort_by_key(|threat| threat.uid);
        out
    }

    /// What a post at `tile` is expected to cost `uid` in health this coming
    /// enemy turn: each enemy melee that reaches it, at the fitted host's
    /// odds, times its expected blow.
    fn hold_post_cost(g: &Game, uid: u32, tile: Pos, threats: &[Threat]) -> f64 {
        let Some(unit) = g.units.get(&uid) else {
            return f64::INFINITY;
        };
        let defence = g.unit_strength(unit, true);
        let held = crate::game::effective_strength(defence, unit.hp);
        threats
            .iter()
            .filter(|threat| threat.melee)
            .filter_map(|threat| {
                let still = g.wdist(threat.pos, tile) <= 1;
                if !still
                    && !threat
                        .reach
                        .iter()
                        .any(|reached| g.wdist(*reached, tile) <= 1)
                {
                    return None;
                }
                let edge = crate::game::effective_strength(threat.strength, threat.hp) - held;
                let odds = f64::from(strike_probability(&StrikeFeatures {
                    class: threat.class,
                    owner: threat.owner,
                    hp: threat.hp.clamp(0, 100) as i16,
                    home: threat.home,
                    edge: edge as f32,
                    targets: 1,
                    still,
                }));
                Some(odds * expected_damage(threat.strength, defence))
            })
            .sum()
    }

    /// A land soldier this turn's other claims leave to the hold.
    fn hold_candidate(&self, g: &Game, pid: usize, uid: u32, claimed: &BTreeSet<u32>) -> bool {
        Self::centre_soldier(g, pid, uid)
            && !claimed.contains(&uid)
            && !self.battle_planner_claims(uid)
            && !self.builder_guard_reserved(uid)
            && !self.guard_is_bound_to_any_settler(uid)
            && !self.guard_is_reserved_for_civilian(uid)
            && !self.unit_is_reserved(uid)
            && !self.active_siege_member(g, pid, uid)
            && !self.staging_siege_member(g, pid, uid)
    }

    /// Draw and walk this turn's posts: see the module. `claimed` holds the
    /// units this turn's other passes took. Nothing with the gene off.
    pub(super) fn hold_the_cities(&mut self, g: &mut Game, pid: usize, claimed: &BTreeSet<u32>) {
        self.defend_hold_plan = HoldPlan::default();
        if !self.defend_hold_on() {
            return;
        }
        self.defend_hold_plan.turn = g.turn;
        if self.defend_force_holds_the_city_2 {
            self.hold_the_cities_v2(g, pid, claimed);
            return;
        }
        let board_rows: BTreeSet<u32> = if self.objective_board {
            self.objective_board_state
                .rows
                .iter()
                .filter_map(|row| match row.key {
                    ObjectiveKey::Defend(cid) => Some(cid),
                    _ => None,
                })
                .collect()
        } else {
            BTreeSet::new()
        };
        let mut cities: Vec<(i32, u32, Pos)> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let at = g.cities[&cid].pos;
                if !board_rows.contains(&cid) && !self.centre_threatened(g, pid, at) {
                    return None;
                }
                let nearest = g
                    .wdisk(at, HOLD_ENEMY_RADIUS)
                    .into_iter()
                    .filter(|tile| {
                        g.unit_ids_at(*tile).iter().any(|id| {
                            let enemy = &g.units[id];
                            enemy.owner != pid
                                && g.rules.units[enemy.kind].class == "military"
                                && g.is_at_war(pid, enemy.owner)
                                && self.battlefront_unit_visible(g, pid, *id)
                        })
                    })
                    .map(|tile| g.wdist(tile, at))
                    .min()
                    .unwrap_or(HOLD_ENEMY_RADIUS + 1);
                Some((nearest, cid, at))
            })
            .collect();
        cities.sort();
        let mut taken: BTreeSet<u32> = BTreeSet::new();
        for (_, cid, at) in cities {
            self.defend_hold_plan.cities.push(cid);
            self.hold_one_city(g, pid, cid, at, claimed, &mut taken);
        }
    }

    /// The centre post (step 1 of the module), pushed onto `posts`: a
    /// shooter keeps it, a melee occupant swaps with an adjacent shooter, an
    /// empty centre takes the nearest shooter in reach, else the strongest
    /// melee.
    fn hold_the_centre(
        g: &mut Game,
        pid: usize,
        cid: u32,
        at: Pos,
        candidates: &[u32],
        posts: &mut Vec<(u32, HoldPost)>,
    ) {
        let post = |at_tile: Pos, role: HoldRole| HoldPost {
            city: cid,
            at: at_tile,
            role,
        };
        let occupant = g.unit_ids_at(at).iter().copied().find(|id| {
            let unit = &g.units[id];
            let spec = &g.rules.units[unit.kind];
            unit.owner == pid
                && spec.class == "military"
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        });
        let occupant_ours = occupant.filter(|id| candidates.contains(id));
        match occupant {
            Some(holder) if is_shooter(g, holder) => {
                if occupant_ours.is_some() {
                    posts.push((holder, post(at, HoldRole::Centre)));
                }
            }
            _ => {
                let shooter = candidates
                    .iter()
                    .copied()
                    .filter(|uid| Some(*uid) != occupant && is_shooter(g, *uid))
                    .filter(|uid| {
                        let pos = g.units[uid].pos;
                        match occupant {
                            Some(_) => occupant_ours.is_some() && g.wdist(pos, at) == 1,
                            None => can_reach(g, *uid, at),
                        }
                    })
                    .min_by_key(|uid| {
                        let unit = &g.units[uid];
                        (
                            g.wdist(unit.pos, at),
                            g.rules.units[unit.kind].siege,
                            std::cmp::Reverse((g.unit_strength(unit, true) * 100.0) as i64),
                            *uid,
                        )
                    });
                let mut filled = false;
                if let Some(shooter) = shooter {
                    let from = g.units[&shooter].pos;
                    match occupant_ours {
                        Some(melee) => {
                            if g.apply(
                                pid,
                                &Action::Swap {
                                    unit: shooter,
                                    other: melee,
                                },
                            )
                            .is_ok()
                                && g.units.get(&shooter).is_some_and(|unit| unit.pos == at)
                            {
                                posts.push((shooter, post(at, HoldRole::Centre)));
                                posts.push((melee, post(from, HoldRole::Ring)));
                                filled = true;
                            }
                        }
                        None => {
                            if from == at
                                || (g
                                    .apply(
                                        pid,
                                        &Action::MoveTo {
                                            unit: shooter,
                                            to: at,
                                        },
                                    )
                                    .is_ok()
                                    && g.units.get(&shooter).is_some_and(|unit| unit.pos == at))
                            {
                                posts.push((shooter, post(at, HoldRole::Centre)));
                                filled = true;
                            }
                        }
                    }
                }
                if !filled {
                    if let Some(holder) = occupant_ours {
                        posts.push((holder, post(at, HoldRole::Centre)));
                    } else if occupant.is_none() {
                        let melee = candidates
                            .iter()
                            .copied()
                            .filter(|uid| ring_rank(g, *uid).is_some() && can_reach(g, *uid, at))
                            .max_by_key(|uid| {
                                let unit = &g.units[uid];
                                (
                                    (crate::game::effective_strength(
                                        g.unit_strength(unit, true),
                                        unit.hp,
                                    ) * 100.0) as i64,
                                    std::cmp::Reverse(*uid),
                                )
                            });
                        if let Some(melee) = melee {
                            let arrived = g.units[&melee].pos == at
                                || (g
                                    .apply(
                                        pid,
                                        &Action::MoveTo {
                                            unit: melee,
                                            to: at,
                                        },
                                    )
                                    .is_ok()
                                    && g.units.get(&melee).is_some_and(|unit| unit.pos == at));
                            if arrived {
                                posts.push((melee, post(at, HoldRole::Centre)));
                            }
                        }
                    }
                }
            }
        }
    }

    fn hold_one_city(
        &mut self,
        g: &mut Game,
        pid: usize,
        cid: u32,
        at: Pos,
        claimed: &BTreeSet<u32>,
        taken: &mut BTreeSet<u32>,
    ) {
        let threats = self.hold_threats(g, pid, at, false);
        let members: BTreeSet<u32> = self
            .objective_board_state
            .forces
            .iter()
            .filter(|force| {
                force.domain == ForceDomain::Land
                    && matches!(
                        force.objective_key,
                        ObjectiveKey::Defend(id) | ObjectiveKey::Relieve(id) if id == cid
                    )
            })
            .flat_map(|force| force.units.iter().copied())
            .collect();
        let mut candidates: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| !taken.contains(uid))
            .filter(|uid| {
                members.contains(uid)
                    || g.units
                        .get(uid)
                        .is_some_and(|unit| g.wdist(unit.pos, at) <= HOLD_RADIUS)
            })
            .filter(|uid| self.hold_candidate(g, pid, *uid, claimed))
            .collect();
        candidates.sort_unstable();
        let mut posts: Vec<(u32, HoldPost)> = Vec::new();
        let post = |at_tile: Pos, role: HoldRole| HoldPost {
            city: cid,
            at: at_tile,
            role,
        };

        // 1. The centre.
        Self::hold_the_centre(g, pid, cid, at, &candidates, &mut posts);
        let mut posted: BTreeSet<u32> = posts.iter().map(|(uid, _)| *uid).collect();

        // 2. The ring tiles the enemy melee can take the city from.
        let mut ring: Vec<(i32, Pos)> = g
            .nbrs(at)
            .into_iter()
            .filter(|tile| {
                g.map
                    .get(*tile)
                    .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
                    && !g
                        .unit_ids_at(*tile)
                        .iter()
                        .any(|id| g.units[id].owner != pid)
            })
            .filter_map(|tile| {
                let nearest = threats
                    .iter()
                    .filter(|threat| {
                        threat.melee
                            && (g.wdist(threat.pos, tile) <= 1 || threat.reach.contains(&tile))
                    })
                    .map(|threat| g.wdist(threat.pos, tile))
                    .min()?;
                Some((nearest, tile))
            })
            .collect();
        ring.sort();
        let threatened = ring.len();
        let mut blocked = 0usize;
        for (_, tile) in ring {
            // Already held by a soldier of ours: a candidate keeps it as its
            // post; anyone else blocks it anyway.
            if let Some(holder) = g.unit_ids_at(tile).iter().copied().find(|id| {
                let unit = &g.units[id];
                unit.owner == pid && g.rules.units[unit.kind].class == "military"
            }) {
                blocked += 1;
                if candidates.contains(&holder) && !posted.contains(&holder) {
                    posted.insert(holder);
                    posts.push((holder, post(tile, HoldRole::Ring)));
                }
                continue;
            }
            let pick = candidates
                .iter()
                .copied()
                .filter(|uid| !posted.contains(uid))
                .filter_map(|uid| ring_rank(g, uid).map(|rank| (uid, rank)))
                .filter(|(uid, _)| {
                    f64::from(g.units[uid].hp) > Self::hold_post_cost(g, *uid, tile, &threats)
                })
                .filter(|(uid, _)| open_for(g, pid, *uid, tile) && can_reach(g, *uid, tile))
                .min_by_key(|(uid, rank)| {
                    let unit = &g.units[uid];
                    (
                        *rank,
                        std::cmp::Reverse(
                            (crate::game::effective_strength(g.unit_strength(unit, true), unit.hp)
                                * 100.0) as i64,
                        ),
                        g.wdist(unit.pos, tile),
                        *uid,
                    )
                })
                .map(|(uid, _)| uid);
            let Some(pick) = pick else {
                continue;
            };
            let arrived = g.units[&pick].pos == tile
                || (g
                    .apply(
                        pid,
                        &Action::MoveTo {
                            unit: pick,
                            to: tile,
                        },
                    )
                    .is_ok()
                    && g.units.get(&pick).is_some_and(|unit| unit.pos == tile));
            if arrived {
                blocked += 1;
                posted.insert(pick);
                posts.push((pick, post(tile, HoldRole::Ring)));
            }
        }

        // 3. Shooters, and 4. everyone else, on the cheapest free tile within
        //    two of the centre.
        let rest: Vec<u32> = candidates
            .iter()
            .copied()
            .filter(|uid| !posted.contains(uid))
            .collect();
        for uid in rest {
            let shooter = is_shooter(g, uid);
            let role = if shooter {
                HoldRole::Shooter
            } else {
                HoldRole::March
            };
            let here = g.units[&uid].pos;
            let reach: BTreeSet<Pos> = g.reachable(uid).into_iter().collect();
            let melee_reach = |tile: Pos| {
                threats.iter().any(|threat| {
                    threat.melee
                        && (g.wdist(threat.pos, tile) <= 1
                            || threat
                                .reach
                                .iter()
                                .any(|reached| g.wdist(*reached, tile) <= 1))
                })
            };
            let range = if shooter {
                g.unit_attack_range(uid).max(1)
            } else {
                1
            };
            // A shooter's tile covers the approach: an enemy soldier within a
            // step of its range.
            let covers = |tile: Pos| {
                !shooter
                    || threats
                        .iter()
                        .any(|threat| g.wdist(threat.pos, tile) <= range + 1)
            };
            let best = g
                .wdisk(at, 2)
                .into_iter()
                .filter(|tile| *tile != at && open_for(g, pid, uid, *tile))
                .filter(|tile| *tile == here || reach.contains(tile))
                .min_by(|a, b| {
                    let key = |tile: &Pos| {
                        (
                            melee_reach(*tile),
                            !covers(*tile),
                            Self::hold_post_cost(g, uid, *tile, &threats),
                            g.wdist(*tile, at),
                            g.wdist(*tile, here),
                            *tile,
                        )
                    };
                    let (ka, kb) = (key(a), key(b));
                    ka.0.cmp(&kb.0)
                        .then(ka.1.cmp(&kb.1))
                        .then(ka.2.total_cmp(&kb.2))
                        .then(ka.3.cmp(&kb.3))
                        .then(ka.4.cmp(&kb.4))
                        .then(ka.5.cmp(&kb.5))
                });
            match best {
                Some(tile) => {
                    if tile != here {
                        let _ = g.apply(
                            pid,
                            &Action::MoveTo {
                                unit: uid,
                                to: tile,
                            },
                        );
                    }
                    let now = g.units.get(&uid).map(|unit| unit.pos).unwrap_or(here);
                    posts.push((uid, post(now, role)));
                }
                None => {
                    // Out of reach of every tile near the city this turn:
                    // march on the nearest open one.
                    if g.wdist(here, at) > 2 {
                        let goal = g
                            .wdisk(at, 2)
                            .into_iter()
                            .filter(|tile| *tile != at && open_for(g, pid, uid, *tile))
                            .min_by_key(|tile| (g.wdist(*tile, here), *tile));
                        if let Some(goal) = goal {
                            let _ = g.apply(
                                pid,
                                &Action::MoveTo {
                                    unit: uid,
                                    to: goal,
                                },
                            );
                        }
                    }
                    let now = g.units.get(&uid).map(|unit| unit.pos).unwrap_or(here);
                    posts.push((uid, post(now, HoldRole::March)));
                }
            }
            posted.insert(uid);
        }
        if !posts.is_empty() {
            self.force_groups_dirty = true;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let name = g.cities[&cid].name.clone();
            let of = |role: HoldRole| -> Vec<String> {
                posts
                    .iter()
                    .filter(|(_, p)| p.role == role)
                    .filter_map(|(uid, p)| {
                        g.units
                            .get(uid)
                            .map(|unit| format!("{} {:?}", plain(unit.kind.as_str()), p.at))
                    })
                    .collect()
            };
            let centre = of(HoldRole::Centre);
            let ring_posts = of(HoldRole::Ring);
            let shooters = of(HoldRole::Shooter);
            let marching = of(HoldRole::March);
            think!(self.journal(), Military, Decision,
                "Hold at {}: centre {}, ring {} of {} threatened tile(s) blocked",
                name,
                if centre.is_empty() { "nobody".to_string() } else { centre.join(", ") },
                blocked,
                threatened;
                "defend-force-holds-the-city: ring [{}]; shooters [{}]; marching [{}]; {} visible enemy soldier(s) within {}",
                ring_posts.join(", "),
                shooters.join(", "),
                marching.join(", "),
                threats.len(),
                HOLD_ENEMY_RADIUS);
        }
        for (uid, p) in posts {
            taken.insert(uid);
            self.defend_hold_plan.posts.insert(uid, p);
        }
    }

    /// A posted unit's step: walk on to its post if it is not there, strike
    /// from it (see the module), fortify. `None` with the gene off, or for a
    /// unit with no post this turn.
    pub(super) fn defend_hold_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        if !self.defend_hold_on() || self.defend_hold_plan.turn != g.turn {
            return None;
        }
        let post = self.defend_hold_plan.posts.get(&uid)?.clone();
        let unit = g.units.get(&uid)?;
        let centre = g
            .cities
            .get(&post.city)
            .filter(|city| city.owner == pid)?
            .pos;
        // A unit standing in the centre holds it, whatever it was posted as
        // (a refill moved it in).
        let role = if unit.pos == centre {
            HoldRole::Centre
        } else {
            post.role
        };
        // Walk on to the post, but never back to it after a blow carried the
        // unit forward.
        if role != HoldRole::Centre
            && unit.pos != post.at
            && unit.moves_left > 0.0
            && unit.attacks_left > 0
        {
            let _ = g.apply(
                pid,
                &Action::MoveTo {
                    unit: uid,
                    to: post.at,
                },
            );
        }
        if let Some(acted) = self.hold_blow(g, pid, uid, role, &post, centre) {
            return Some(acted);
        }
        // Version two: with no capturer able to reach the city next turn, a
        // soldier off the centre may leave its post for a raider it kills
        // and survives.
        if self.defend_force_holds_the_city_2
            && role != HoldRole::Centre
            && self.defend_hold_plan.quiet.contains(&post.city)
            && self.hold_raider_kill(g, pid, uid)
        {
            return Some(true);
        }
        if g.units.contains_key(&uid) {
            self.base.fortify_or_stop(g, pid, uid);
        }
        Some(false)
    }

    /// One blow from the post, if the doctrine allows one; `Some(true)` when
    /// it landed.
    fn hold_blow(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        role: HoldRole,
        post: &HoldPost,
        centre: Pos,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?;
        if unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
            return None;
        }
        let spec = &g.rules.units[unit.kind];
        let here = unit.pos;
        let hp = f64::from(unit.hp);
        let roll = crate::ai::COMBAT_ROLL_MAX;
        let enemy_on = |tile: Pos| -> Option<u32> {
            g.unit_ids_at(tile)
                .iter()
                .copied()
                .filter(|id| {
                    let enemy = &g.units[id];
                    let espec = &g.rules.units[enemy.kind];
                    enemy.owner != pid
                        && g.is_at_war(pid, enemy.owner)
                        && espec.class == "military"
                        && espec.domain.as_deref() != Some("air")
                })
                .max_by(|a, b| {
                    let reading = |id: &u32| {
                        let enemy = &g.units[id];
                        crate::game::effective_strength(g.unit_strength(enemy, true), enemy.hp)
                    };
                    reading(a).total_cmp(&reading(b))
                })
        };
        // (beside the city, kills, damage, tile) -> the order.
        let mut best: Option<(BlowKey, Action, bool)> = None;
        let mut consider = |key: BlowKey, action: Action, advances: bool| {
            if best.as_ref().is_none_or(|(k, _, _)| key > *k) {
                best = Some((key, action, advances));
            }
        };
        if spec.has_ranged_attack() {
            let frame = g.player_vision_frame(pid);
            let viewers = g.visibility_viewers(pid);
            for tile in g.wdisk(here, g.unit_attack_range(uid)) {
                if g.city_at(tile).is_some()
                    || !g.ranged_order_is_legal(pid, uid, tile, frame.as_ref(), &viewers)
                {
                    continue;
                }
                let Some(did) = enemy_on(tile) else {
                    continue;
                };
                let Some((att, def)) = g.ranged_strike_strengths(uid, did, tile) else {
                    continue;
                };
                let dealt = expected_damage(att, def);
                let kills = dealt >= f64::from(g.units[&did].hp);
                consider(
                    (
                        g.wdist(tile, centre) <= 1,
                        kills,
                        (dealt * 100.0) as i64,
                        std::cmp::Reverse(tile),
                    ),
                    Action::Ranged {
                        unit: uid,
                        target: tile,
                    },
                    false,
                );
            }
        } else if spec.is_melee_capable() {
            for tile in g.nbrs(here) {
                if g.city_at(tile).is_some() || !g.melee_order_is_legal(pid, uid, tile) {
                    continue;
                }
                let Some(did) = enemy_on(tile) else {
                    continue;
                };
                let Some((att, def)) = g.melee_exchange_strengths(uid, did) else {
                    continue;
                };
                let target_hp = f64::from(g.units[&did].hp);
                let dealt = expected_damage(att, def);
                let taken = expected_damage(def, att);
                let survives = taken * roll < hp;
                let kills = dealt >= target_hp;
                let may_kill = dealt * roll >= target_hp;
                let allowed = match role {
                    // The centre's melee: a blow that cannot carry it out,
                    // or a kill with the centre refilled (checked below).
                    HoldRole::Centre => survives,
                    HoldRole::Ring => survives,
                    HoldRole::Shooter | HoldRole::March => survives && kills,
                };
                if !allowed {
                    continue;
                }
                consider(
                    (
                        g.wdist(tile, centre) <= 1,
                        kills,
                        (dealt * 100.0) as i64,
                        std::cmp::Reverse(tile),
                    ),
                    Action::Attack {
                        unit: uid,
                        target: tile,
                    },
                    may_kill,
                );
            }
        }
        let (_, action, may_advance) = best?;
        // The centre's melee leaves the centre only when another candidate
        // of this city steps into it this same turn.
        let refill = if role == HoldRole::Centre && may_advance {
            let mut after = g.speculative_clone();
            if after.apply(pid, &action).is_err() {
                return None;
            }
            let left = after.units.get(&uid).is_none_or(|unit| unit.pos != centre);
            if left {
                Some(
                    self.defend_hold_plan
                        .posts
                        .iter()
                        .filter(|(other, other_post)| {
                            **other != uid
                                && other_post.city == post.city
                                && matches!(other_post.role, HoldRole::Shooter | HoldRole::March)
                        })
                        .map(|(other, _)| *other)
                        .find(|other| {
                            after
                                .units
                                .get(other)
                                .is_some_and(|unit| unit.moves_left > 0.0 && unit.attacks_left > 0)
                                && after.reachable(*other).contains(&centre)
                        })?,
                )
            } else {
                None
            }
        } else {
            None
        };
        if g.apply(pid, &action).is_err() {
            return None;
        }
        self.force_groups_dirty = true;
        if let Some(other) = refill {
            if g.units.get(&uid).is_none_or(|unit| unit.pos != centre) {
                let _ = g.apply(
                    pid,
                    &Action::MoveTo {
                        unit: other,
                        to: centre,
                    },
                );
            }
        }
        Some(true)
    }
}

/// Whether `threat` can land a blow on `tile` next turn, and whether without
/// moving.
fn threat_strikes(g: &Game, threat: &Threat, tile: Pos) -> Option<bool> {
    let still = g.wdist(threat.pos, tile) <= threat.range;
    (still
        || threat
            .reach
            .iter()
            .any(|reached| g.wdist(*reached, tile) <= threat.range))
    .then_some(still)
}

/// Version two: the chance `uid` standing on `tile` is dead before our next
/// turn, each visible enemy that can strike it landing its blow at the fitted
/// host's odds (`priced_risk::death_chance`).
fn post_death_chance(g: &Game, uid: u32, tile: Pos, threats: &[Threat]) -> f64 {
    let Some(unit) = g.units.get(&uid) else {
        return 1.0;
    };
    let defence = g.unit_strength(unit, true);
    let held = crate::game::effective_strength(defence, unit.hp);
    let blows: Vec<(f32, f32, f64)> = threats
        .iter()
        .filter_map(|threat| {
            let still = threat_strikes(g, threat, tile)?;
            let edge = crate::game::effective_strength(threat.attack, threat.hp) - held;
            let p = f64::from(strike_probability(&StrikeFeatures {
                class: threat.class,
                owner: threat.owner,
                hp: threat.hp.clamp(0, 100) as i16,
                home: threat.home,
                edge: edge as f32,
                targets: 1,
                still,
            }));
            Some((threat.attack as f32, defence as f32, p))
        })
        .collect();
    super::priced_risk::death_chance(unit.hp, 0.0, &blows)
}

impl AdvancedAi {
    /// Version two: a soldier stacked with one of our civilians off a city
    /// keeps protecting it.
    fn guards_a_civilian(g: &Game, pid: usize, uid: u32) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        g.city_at(unit.pos).is_none()
            && g.unit_ids_at(unit.pos).iter().any(|id| {
                let other = &g.units[id];
                *id != uid && other.owner == pid && g.rules.units[other.kind].class != "military"
            })
    }

    /// Version two: a visible land soldier of a civilization that can take
    /// the city (a major or a city-state at war with us) within `radius`.
    fn capturer_within(&self, g: &Game, pid: usize, at: Pos, radius: i32) -> bool {
        g.wdisk(at, radius).into_iter().any(|tile| {
            g.unit_ids_at(tile).iter().any(|id| {
                g.units.get(id).is_some_and(|enemy| {
                    let spec = &g.rules.units[enemy.kind];
                    let player = &g.players[enemy.owner];
                    enemy.owner != pid
                        && enemy.hp > 0
                        && spec.class == "military"
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                        && !player.is_barbarian
                        && !player.is_free_city
                        && g.is_at_war(pid, enemy.owner)
                        && self.battlefront_unit_visible(g, pid, *id)
                })
            })
        })
    }

    /// Version two's held cities: a capturer within [`CENTRE_THREAT_REACH`],
    /// or a Defend row with a capturer within [`HOLD_ENEMY_RADIUS`]. Nearest
    /// capturer first.
    fn held_cities_v2(&self, g: &Game, pid: usize) -> Vec<(u32, Pos)> {
        let board_rows: BTreeSet<u32> = if self.objective_board {
            self.objective_board_state
                .rows
                .iter()
                .filter_map(|row| match row.key {
                    ObjectiveKey::Defend(cid) => Some(cid),
                    _ => None,
                })
                .collect()
        } else {
            BTreeSet::new()
        };
        let mut cities: Vec<(i32, u32, Pos)> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let at = g.cities[&cid].pos;
                let near = self.capturer_within(g, pid, at, CENTRE_THREAT_REACH);
                if !near
                    && !(board_rows.contains(&cid)
                        && self.capturer_within(g, pid, at, HOLD_ENEMY_RADIUS))
                {
                    return None;
                }
                let nearest = (1..=HOLD_ENEMY_RADIUS)
                    .find(|radius| self.capturer_within(g, pid, at, *radius))
                    .unwrap_or(HOLD_ENEMY_RADIUS + 1);
                Some((nearest, cid, at))
            })
            .collect();
        cities.sort();
        cities.into_iter().map(|(_, cid, at)| (cid, at)).collect()
    }

    /// The land members of `cid`'s Defend and Relieve task forces.
    fn hold_force_members(&self, cid: u32) -> BTreeSet<u32> {
        self.objective_board_state
            .forces
            .iter()
            .filter(|force| {
                force.domain == ForceDomain::Land
                    && matches!(
                        force.objective_key,
                        ObjectiveKey::Defend(id) | ObjectiveKey::Relieve(id) if id == cid
                    )
            })
            .flat_map(|force| force.units.iter().copied())
            .collect()
    }

    /// Version two, before the battle planner: the soldiers this turn's
    /// posts may take, so the positions plan leaves them alone. Empty with
    /// version two off.
    pub(super) fn preview_hold_members(&mut self, g: &Game, pid: usize) {
        self.defend_hold_members.clear();
        if !self.defend_force_holds_the_city_2 {
            return;
        }
        let none = BTreeSet::new();
        for (cid, at) in self.held_cities_v2(g, pid) {
            let members = self.hold_force_members(cid);
            for uid in g.player_unit_ids(pid) {
                let near = members.contains(&uid)
                    || g.units
                        .get(&uid)
                        .is_some_and(|unit| g.wdist(unit.pos, at) <= HOLD_RADIUS);
                if near
                    && self.hold_candidate(g, pid, uid, &none)
                    && !Self::guards_a_civilian(g, pid, uid)
                {
                    self.defend_hold_members.insert(uid);
                }
            }
        }
    }

    /// Version two: a soldier the positions plan leaves to this turn's posts.
    /// Always false with version two off.
    pub(super) fn defend_hold_member_v2(&self, uid: u32) -> bool {
        self.defend_force_holds_the_city_2 && self.defend_hold_members.contains(&uid)
    }

    fn hold_the_cities_v2(&mut self, g: &mut Game, pid: usize, claimed: &BTreeSet<u32>) {
        let mut taken: BTreeSet<u32> = BTreeSet::new();
        for (cid, at) in self.held_cities_v2(g, pid) {
            self.defend_hold_plan.cities.push(cid);
            self.hold_one_city_v2(g, pid, cid, at, claimed, &mut taken);
        }
    }

    /// Version two: whether the visible capturers can take the city within
    /// [`CAPTURE_TURNS`] enemy turns: a melee capturer already beside the
    /// centre, or their blows that can land on it meeting its health and
    /// walls (and a melee capturer near enough to walk in).
    fn capture_soon(g: &Game, cid: u32, at: Pos, threats: &[Threat]) -> bool {
        let capturers: Vec<&Threat> = threats.iter().filter(|threat| threat.capturer).collect();
        if capturers
            .iter()
            .any(|threat| threat.melee && g.wdist(threat.pos, at) <= 1)
        {
            return true;
        }
        if !capturers.iter().any(|threat| threat.melee) {
            return false;
        }
        let city = &g.cities[&cid];
        let defence = g.city_strength(cid);
        let per_turn: f64 = capturers
            .iter()
            .filter(|threat| threat_strikes(g, threat, at).is_some())
            .map(|threat| expected_damage(threat.attack, defence))
            .sum();
        CAPTURE_TURNS * per_turn >= f64::from(city.hp.max(0) + city.wall_hp.max(0))
    }

    fn hold_one_city_v2(
        &mut self,
        g: &mut Game,
        pid: usize,
        cid: u32,
        at: Pos,
        claimed: &BTreeSet<u32>,
        taken: &mut BTreeSet<u32>,
    ) {
        let threats = self.hold_threats(g, pid, at, true);
        let members = self.hold_force_members(cid);
        let mut candidates: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| !taken.contains(uid))
            .filter(|uid| {
                members.contains(uid)
                    || g.units
                        .get(uid)
                        .is_some_and(|unit| g.wdist(unit.pos, at) <= HOLD_RADIUS)
            })
            .filter(|uid| self.hold_candidate(g, pid, *uid, claimed))
            .filter(|uid| !Self::guards_a_civilian(g, pid, *uid))
            .collect();
        candidates.sort_unstable();
        let mut posts: Vec<(u32, HoldPost)> = Vec::new();
        let post = |at_tile: Pos, role: HoldRole| HoldPost {
            city: cid,
            at: at_tile,
            role,
        };
        // 1. The centre, as version one.
        Self::hold_the_centre(g, pid, cid, at, &candidates, &mut posts);
        let mut posted: BTreeSet<u32> = posts.iter().map(|(uid, _)| *uid).collect();

        // Whether blocking matters, and whether we are outclassed.
        let soon = Self::capture_soon(g, cid, at, &threats);
        let ring_tiles: Vec<Pos> = g.nbrs(at).into_iter().collect();
        let quiet = !threats.iter().any(|threat| {
            threat.capturer
                && std::iter::once(at)
                    .chain(ring_tiles.iter().copied())
                    .any(|tile| threat_strikes(g, threat, tile).is_some())
        });
        if quiet {
            self.defend_hold_plan.quiet.insert(cid);
        }
        let strongest = |uid: &u32| -> f64 {
            g.units.get(uid).map_or(0.0, |unit| {
                let spec = &g.rules.units[unit.kind];
                g.unit_strength(unit, true)
                    .max(spec.ranged_attack_strength())
            })
        };
        let ours = candidates.iter().map(strongest).fold(0.0, f64::max);
        let theirs = threats
            .iter()
            .filter(|threat| g.wdist(threat.pos, at) <= OUTCLASSED_RADIUS)
            .map(|threat| threat.attack)
            .fold(0.0, f64::max);
        let outclassed = theirs >= ours + OUTCLASSED_EDGE;
        let limit = if outclassed {
            OUTCLASSED_RING_DEATH_LIMIT
        } else {
            RING_DEATH_LIMIT
        };

        // 2. The ring, only when blocking matters.
        let mut threatened = 0usize;
        let mut blocked = 0usize;
        if soon {
            let mut ring: Vec<(i32, Pos)> = ring_tiles
                .iter()
                .copied()
                .filter(|tile| {
                    g.map
                        .get(*tile)
                        .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
                        && !g
                            .unit_ids_at(*tile)
                            .iter()
                            .any(|id| g.units[id].owner != pid)
                })
                .filter_map(|tile| {
                    let nearest = threats
                        .iter()
                        .filter(|threat| {
                            threat.capturer
                                && threat.melee
                                && (g.wdist(threat.pos, tile) <= 1 || threat.reach.contains(&tile))
                        })
                        .map(|threat| g.wdist(threat.pos, tile))
                        .min()?;
                    Some((nearest, tile))
                })
                .collect();
            ring.sort();
            threatened = ring.len();
            for (_, tile) in ring {
                if let Some(holder) = g.unit_ids_at(tile).iter().copied().find(|id| {
                    let unit = &g.units[id];
                    unit.owner == pid && g.rules.units[unit.kind].class == "military"
                }) {
                    // A candidate already on the tile holds it only under the
                    // death-chance limit; otherwise it stands back with the
                    // rest. A soldier not ours to post blocks it anyway.
                    if !candidates.contains(&holder) || posted.contains(&holder) {
                        blocked += 1;
                    } else if ring_rank(g, holder).is_some()
                        && post_death_chance(g, holder, tile, &threats) < limit
                    {
                        blocked += 1;
                        posted.insert(holder);
                        posts.push((holder, post(tile, HoldRole::Ring)));
                    }
                    continue;
                }
                let pick = candidates
                    .iter()
                    .copied()
                    .filter(|uid| !posted.contains(uid))
                    .filter_map(|uid| ring_rank(g, uid).map(|rank| (uid, rank)))
                    .filter(|(uid, _)| open_for(g, pid, *uid, tile) && can_reach(g, *uid, tile))
                    .filter(|(uid, _)| post_death_chance(g, *uid, tile, &threats) < limit)
                    .min_by_key(|(uid, rank)| {
                        let unit = &g.units[uid];
                        (
                            *rank,
                            std::cmp::Reverse(
                                (crate::game::effective_strength(
                                    g.unit_strength(unit, true),
                                    unit.hp,
                                ) * 100.0) as i64,
                            ),
                            g.wdist(unit.pos, tile),
                            *uid,
                        )
                    })
                    .map(|(uid, _)| uid);
                let Some(pick) = pick else {
                    continue;
                };
                let arrived = g.units[&pick].pos == tile
                    || (g
                        .apply(
                            pid,
                            &Action::MoveTo {
                                unit: pick,
                                to: tile,
                            },
                        )
                        .is_ok()
                        && g.units.get(&pick).is_some_and(|unit| unit.pos == tile));
                if arrived {
                    blocked += 1;
                    posted.insert(pick);
                    posts.push((pick, post(tile, HoldRole::Ring)));
                }
            }
        }

        // 3. Everyone else stands ready within two tiles, where the enemy
        //    cannot strike next turn if such a tile is in reach.
        let rest: Vec<u32> = candidates
            .iter()
            .copied()
            .filter(|uid| !posted.contains(uid))
            .collect();
        for uid in rest {
            let shooter = is_shooter(g, uid);
            let role = if shooter {
                HoldRole::Shooter
            } else {
                HoldRole::March
            };
            let here = g.units[&uid].pos;
            let reach: BTreeSet<Pos> = g.reachable(uid).into_iter().collect();
            let range = if shooter {
                g.unit_attack_range(uid).max(1)
            } else {
                1
            };
            let covers = |tile: Pos| {
                !shooter
                    || threats
                        .iter()
                        .any(|threat| g.wdist(threat.pos, tile) <= range + 1)
            };
            let best = g
                .wdisk(at, 2)
                .into_iter()
                .filter(|tile| *tile != at && open_for(g, pid, uid, *tile))
                .filter(|tile| *tile == here || reach.contains(tile))
                .map(|tile| (tile, post_death_chance(g, uid, tile, &threats)))
                .min_by(|(a, da), (b, db)| {
                    da.total_cmp(db)
                        .then((!covers(*a)).cmp(&!covers(*b)))
                        .then(g.wdist(*a, at).cmp(&g.wdist(*b, at)))
                        .then(g.wdist(*a, here).cmp(&g.wdist(*b, here)))
                        .then(a.cmp(b))
                })
                .map(|(tile, _)| tile);
            match best {
                Some(tile) => {
                    if tile != here {
                        let _ = g.apply(
                            pid,
                            &Action::MoveTo {
                                unit: uid,
                                to: tile,
                            },
                        );
                    }
                    let now = g.units.get(&uid).map(|unit| unit.pos).unwrap_or(here);
                    posts.push((uid, post(now, role)));
                }
                None => {
                    if g.wdist(here, at) > 2 {
                        let goal = g
                            .wdisk(at, 2)
                            .into_iter()
                            .filter(|tile| *tile != at && open_for(g, pid, uid, *tile))
                            .min_by_key(|tile| (g.wdist(*tile, here), *tile));
                        if let Some(goal) = goal {
                            let _ = g.apply(
                                pid,
                                &Action::MoveTo {
                                    unit: uid,
                                    to: goal,
                                },
                            );
                        }
                    }
                    let now = g.units.get(&uid).map(|unit| unit.pos).unwrap_or(here);
                    posts.push((uid, post(now, HoldRole::March)));
                }
            }
            posted.insert(uid);
        }
        if !posts.is_empty() {
            self.force_groups_dirty = true;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let name = g.cities[&cid].name.clone();
            let of = |role: HoldRole| -> Vec<String> {
                posts
                    .iter()
                    .filter(|(_, p)| p.role == role)
                    .filter_map(|(uid, p)| {
                        g.units
                            .get(uid)
                            .map(|unit| format!("{} {:?}", plain(unit.kind.as_str()), p.at))
                    })
                    .collect()
            };
            let centre = of(HoldRole::Centre);
            think!(self.journal(), Military, Decision,
                "Hold at {}: centre {}, ring {} of {} threatened tile(s) blocked",
                name,
                if centre.is_empty() { "nobody".to_string() } else { centre.join(", ") },
                blocked,
                threatened;
                "defend-force-holds-the-city-2: {}; {}{}; ring [{}]; ready [{}]; marching [{}]",
                if soon { "the city can fall within two enemy turns" } else { "no fall within two enemy turns, so no ring posts" },
                if outclassed { "outclassed, " } else { "" },
                if quiet { "no capturer reaches the city next turn" } else { "a capturer reaches the city next turn" },
                of(HoldRole::Ring).join(", "),
                of(HoldRole::Shooter).join(", "),
                of(HoldRole::March).join(", "));
        }
        for (uid, p) in posts {
            taken.insert(uid);
            self.defend_hold_plan.posts.insert(uid, p);
        }
    }

    /// Version two: a raider (barbarian or Free Cities) `uid` can kill this
    /// turn and survive, struck from where it stands or after a step: tried
    /// on an exact clone, then played. `true` when it struck.
    fn hold_raider_kill(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
            return false;
        }
        let here = unit.pos;
        let spec = &g.rules.units[unit.kind];
        let range = if spec.has_ranged_attack() {
            g.unit_attack_range(uid).max(1)
        } else {
            1
        };
        let raiders: Vec<(u32, Pos)> = g
            .wdisk(here, 3 + range)
            .into_iter()
            .flat_map(|tile| g.unit_ids_at(tile).to_vec())
            .filter_map(|id| {
                let enemy = g.units.get(&id)?;
                let player = &g.players[enemy.owner];
                (enemy.owner != pid
                    && (player.is_barbarian || player.is_free_city)
                    && g.is_at_war(pid, enemy.owner)
                    && g.rules.units[enemy.kind].class == "military"
                    && self.battlefront_unit_visible(g, pid, id))
                .then_some((id, enemy.pos))
            })
            .collect();
        if raiders.is_empty() {
            return false;
        }
        let mut stands: Vec<Pos> = vec![here];
        stands.extend(g.reachable(uid));
        let ranged = spec.has_ranged_attack();
        let mut best: Option<(i32, Pos, Pos)> = None;
        for (raider, target) in raiders {
            for stand in stands.iter().copied() {
                if g.wdist(stand, target) > range
                    || (stand != here && !open_for(g, pid, uid, stand))
                {
                    continue;
                }
                let mut after = g.speculative_clone();
                if stand != here
                    && (after
                        .apply(
                            pid,
                            &Action::MoveTo {
                                unit: uid,
                                to: stand,
                            },
                        )
                        .is_err()
                        || after.units.get(&uid).is_none_or(|moved| moved.pos != stand))
                {
                    continue;
                }
                let blow = if ranged {
                    Action::Ranged { unit: uid, target }
                } else {
                    Action::Attack { unit: uid, target }
                };
                if after.apply(pid, &blow).is_err() || after.units.contains_key(&raider) {
                    continue;
                }
                let Some(left) = after.units.get(&uid).map(|survivor| survivor.hp) else {
                    continue;
                };
                if best.is_none_or(|(hp, _, _)| left > hp) {
                    best = Some((left, stand, target));
                }
            }
        }
        let Some((_, stand, target)) = best else {
            return false;
        };
        if stand != here {
            let _ = g.apply(
                pid,
                &Action::MoveTo {
                    unit: uid,
                    to: stand,
                },
            );
        }
        if g.units.get(&uid).is_none_or(|moved| moved.pos != stand) {
            return false;
        }
        let blow = if ranged {
            Action::Ranged { unit: uid, target }
        } else {
            Action::Attack { unit: uid, target }
        };
        let struck = g.apply(pid, &blow).is_ok();
        if struck {
            self.force_groups_dirty = true;
        }
        struck
    }
}

#[cfg(test)]
mod tests;
