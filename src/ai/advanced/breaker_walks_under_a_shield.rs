//! `breaker-walks-under-a-shield`: the early conquest opening's wall breaker
//! walks stacked with a melee body of its strike force, and the force strikes
//! the walls only from beside it.
//!
//! ## Why
//!
//! Live Deity Macedon (4-player Tiny Pangaea, Online, Domination lane): the
//! early conquest opening marches three or four melee bodies, two shooters
//! and one Battering Ram on a city behind Ancient Walls (100), and the strike
//! now reaches its declaration and fails in the assault. `siege_replay` of
//! `civvis-20261111T040704Z` (#5058: 10 seeds x 28 turns from t30, t34, t38
//! and t42, both arms) declared on the Cree in 9-10 of 10 seeds and took
//! Mikisiw-Wacîhk in none of 40. The Ram walked alone from 14 tiles to 5
//! tiles from the city by t52 and died on the first turn of the war;
//! `opening-yields-to-walls` then asked for terms ("the target walled up
//! before the strike landed and no breaker is held"), and the rest of the
//! force never moved the city's health (200 throughout; walls 100 -> 80).
//!
//! The first live city-state war, `civvis-20261111T072641Z`, failed one step
//! earlier. The stone (Auckland, 9 tiles, walls 100, defence 22-28) was
//! declared on at t36 with 4 Warriors, 3 Archers, a Slinger and the Ram, and
//! the siege train took the campaign and staged for 14 turns ("nothing to
//! open the walls, so the train holds outside the city's reach", "the warrior
//! falls back to the muster line ... while the train cannot close") with 0
//! damage, until at t50 the opening yielded. The siege read "0 ram/tower"
//! because from t34 the Ram rode in a formation with an Archer (host unit
//! 851976) that `advanced_formations` had linked on Pella's tile: the Archer
//! stayed home, the pair stood 9-11 tiles from Auckland, outside every
//! breach radius (`breach_reading_within` counts a Ram carried by a melee
//! member, or an unlinked one within the radius), while the opening counted
//! "1 breaker(s) standing" anywhere.
//!
//! ## The rules it leans on
//!
//! - A support unit shares a tile with one land military unit and is not
//!   attacked on its own while it does: the military unit defends. In the
//!   engine the two stand on separate stacking layers (`Game::can_stop`), a
//!   blow at the tile is a blow at the soldier (`do_attack`), and a support
//!   unit is lost only when an enemy enters its tile (`resolve_entered_units`).
//!   Shipped: `Base/Assets/Gameplay/Data/Units.xml:782` gives
//!   `UNIT_BATTERING_RAM` `FormationClass="FORMATION_CLASS_SUPPORT"`. A linked
//!   pair moves as one (`Action::LinkUnits`, the host's
//!   `UNITCOMMAND_ENTER_FORMATION`, already wired and sent by
//!   `advanced_formations`).
//! - A Ram works only from beside the City Center:
//!   `Base/Assets/Text/en_US/Units_Text.xml:270` "When adjacent to a city,
//!   attacking melee units do full damage to the city's Walls."
//!   (`UnitAbilities.xml:124`, `ABILITY_ENABLE_WALL_ATTACK` on
//!   `CLASS_BATTERING_RAM`). Without it a melee blow lands 15% on the walls
//!   (`Game::do_encampment_melee`, `Game::siege_support_effects`).
//!
//! ## What the gene does
//!
//! Only for the early conquest opening (`early-conquest-opening`: the first
//! capital, or `city-state-before-the-first-capital`'s stone) while its city
//! stands with the target, and for one breaker: our Battering Ram or Siege
//! Tower whose effect works on that city's walls, nearest it. Mid- and
//! late-game siege trains are untouched.
//!
//! 1. **March** ([`AdvancedAi::shielded_breaker_step`] for the breaker). The
//!    breaker never ends a move on a tile without a land soldier of ours, and
//!    never walks ahead of the force: it rides linked with its shield, a melee
//!    body of the force or the field army (the one carrying it, else one on
//!    its tile, else the nearest), so the shield's own orders walk the pair
//!    to the rally (`conquest_staging_objective` admits the carrier). A
//!    breaker whose shield is not in reach this turn waits in a city or under
//!    a soldier while the shield walks back for it, and the shield moves first
//!    in the unit order. `advanced_formations` does not link the breaker to
//!    anything but its shield; one already riding with another soldier (a
//!    garrison, a guard) leaves it for the shield.
//! 2. **Arrival.** Once the opening has declared, its force and breaker are
//!    the siege, with any other field-army body within
//!    [`ASSAULT_JOIN_RADIUS`] of the city: the strike is a commitment, and
//!    no stage, muster line,
//!    danger limit or ring-seal hold of the siege train applies to its
//!    bodies ([`AdvancedAi::shielded_breaker_step`] runs ahead of the siege
//!    train in the military step). The shield walks to a tile beside
//!    the City Center (fewest visible enemy soldiers beside it, then not
//!    across a river from the city, then not open flat ground), carrying the
//!    breaker; the other melee bodies take the free tiles beside the city and
//!    the shooters close to range two. While the breaker is further off than
//!    [`SHIELD_HOLD_TURNS`] (still being fetched), the force waits just
//!    outside the city's strike.
//! 3. **Assault order.** While the walls stand and the breaker rides with its
//!    shield within [`SHIELD_HOLD_TURNS`] of the walls, the melee bodies do not
//!    swing at them: they hold beside the city, killing a defender beside
//!    them when the blow kills it and they keep [`BODY_STRIKE_FLOOR`]. Once
//!    the breaker stands beside the city every melee body whose blow it opens
//!    strikes the city each turn while it keeps [`BODY_STRIKE_FLOOR`] (the
//!    shield [`SHIELD_STRIKE_FLOOR`]) or takes the city; shooters fire at a
//!    soldier beside the shield, else at the city. With the walls down none of
//!    the waiting applies: a melee body beside the city strikes at once, the
//!    others close in.
//! 4. **The shipped exits stay.** The release rules are unchanged (the force
//!    gone, the breaker lost under walls -> `opening-yields-to-walls`, the
//!    patience window); a home city under threat takes the turn back; a
//!    wounded body heals on the shipped path. The breaker's loss is
//!    journalled first, with where and to what (`breaker_shield_watch`).
//! 5. **Journal** (grep `Breaker under a shield:`): `the <ram> follows the
//!    <unit> to <city>`, `the <ram> waits ... for its shield`, `the <unit>
//!    holds the assault on <city> until the ram stands beside the walls (N
//!    turn(s))`, `the ram stands beside <city>; N bodies strike the walls`,
//!    `the <unit> strikes <city> beside the ram`, `the <ram> is lost before
//!    <city>`.
//!
//! Off, every entry point returns before it reads the board, so the
//! controller is byte-identical.

use super::*;

/// A shield within this many turns of standing beside the walls with its
/// breaker holds the melee bodies' swings for it.
pub(super) const SHIELD_HOLD_TURNS: i32 = 2;

/// The shield strikes the walls only while it keeps this much health after
/// the reply: the breaker on its tile is lost with it.
pub(super) const SHIELD_STRIKE_FLOOR: i32 = 50;

/// At war, a land body of the field army this close to the city joins the
/// opening's force in the assault: the rally's farthest tile plus the
/// assembly radius around it.
pub(super) const ASSAULT_JOIN_RADIUS: i32 =
    super::early_conquest::CONQUEST_RALLY_MAX + super::early_conquest::CONQUEST_ASSEMBLY_RADIUS;

/// Any other body strikes, or kills a defender, only while it keeps this
/// much health after the reply (`siege_train::ASSAULT_SURVIVOR_HP`).
pub(super) const BODY_STRIKE_FLOOR: i32 = super::siege_train::ASSAULT_SURVIVOR_HP;

/// What the gene remembers between turns: where the breaker last stood and
/// with whom, so its loss can be journalled, and what this turn's assault
/// has journalled. `None` with the gene off.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct BreakerShieldMemory {
    /// The breaker, by this board's unit id; `None` once a live rebuild could
    /// not carry the id across.
    breaker: Option<u32>,
    kind: Name,
    pos: Pos,
    shield: Option<u32>,
    shield_kind: Option<Name>,
    city_pos: Pos,
    /// The turn the hold was last journalled.
    hold_noted: Option<u32>,
    /// The turn and number of strikes made beside the breaker.
    strikes: (u32, u32),
}

/// The opening the gene serves this turn.
#[derive(Clone, Copy, Debug)]
struct ShieldScene {
    city: u32,
    city_pos: Pos,
    /// The opening's war is open.
    declared: bool,
}

impl AdvancedAi {
    /// The early conquest opening while its city stands with its target.
    /// `None` with the gene off.
    fn shield_scene(&self, g: &Game, pid: usize) -> Option<ShieldScene> {
        if !self.breaker_walks_under_a_shield || !self.early_conquest_opening {
            return None;
        }
        let opening = self.conquest_opening.as_ref()?;
        let city = g
            .cities
            .get(&opening.city)
            .filter(|city| city.owner == opening.target)?;
        Some(ShieldScene {
            city: opening.city,
            city_pos: city.pos,
            declared: opening.declared.is_some() && g.is_at_war(pid, opening.target),
        })
    }

    /// The opening's breaker: our Battering Ram or Siege Tower whose effect
    /// works on the city's walls, nearest the city, then the lowest id.
    fn scene_breaker(g: &Game, pid: usize, scene: &ShieldScene) -> Option<u32> {
        g.units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && matches!(unit.kind.as_str(), "battering_ram" | "siege_tower")
                    && !g.is_embarked(unit)
                    && g.city_allows_siege_support(scene.city, unit.kind.as_str())
            })
            .min_by_key(|unit| (g.wdist(unit.pos, scene.city_pos), unit.id))
            .map(|unit| unit.id)
    }

    /// The breaker this gene walks under a shield. `None` with the gene off.
    pub(super) fn shielded_breaker(&self, g: &Game, pid: usize) -> Option<u32> {
        let scene = self.shield_scene(g, pid)?;
        Self::scene_breaker(g, pid, &scene)
    }

    /// `advanced_formations`: the opening's breaker links only to its
    /// shield, in its own step. `false` with the gene off.
    pub(super) fn breaker_waits_for_its_shield(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.shielded_breaker(g, pid) == Some(uid)
    }

    /// The land soldier carrying `breaker` in a formation, if one is.
    fn breaker_carrier(g: &Game, breaker: u32) -> Option<u32> {
        let peer = g.units.get(&breaker)?.linked_to?;
        (super::siege_train::linked_support_carrier(g, peer)
            && g.units
                .get(&peer)
                .is_some_and(|carrier| carrier.linked_to == Some(breaker)))
        .then_some(peer)
    }

    /// The bodies that may carry the breaker: melee bodies of the opening's
    /// force or of the field army, never a civilian's guard or the guard
    /// standing in one of our cities (`first_capital_home_guards`; a body
    /// merely beside a city is no garrison), not embarked.
    fn shield_bodies(&self, g: &Game, pid: usize) -> BTreeSet<u32> {
        let Some(opening) = self.conquest_opening.as_ref() else {
            return BTreeSet::new();
        };
        let guards = self.all_reserved_civilian_guards();
        let home: BTreeSet<u32> = self
            .first_capital_home_guards(g, pid)
            .into_iter()
            .filter(|uid| g.city_at(g.units[uid].pos).is_some())
            .collect();
        let mut pool: BTreeSet<u32> = self.campaign_field_army(g, pid).into_iter().collect();
        pool.extend(opening.force.iter().copied());
        pool.into_iter()
            .filter(|uid| {
                g.units.get(uid).is_some_and(|unit| {
                    unit.owner == pid
                        && !guards.contains(uid)
                        && !home.contains(uid)
                        && Self::conquest_melee_body(&g.rules.units[unit.kind])
                        && !g.is_embarked(unit)
                })
            })
            .collect()
    }

    /// A body that may take the breaker up now: free of any formation and
    /// healthy enough to stand in front of it.
    fn shield_free(g: &Game, uid: u32) -> bool {
        g.units.get(&uid).is_some_and(|unit| {
            unit.linked_to.is_none() && unit.hp >= super::battle_planner::ROTATE_HP
        })
    }

    /// The breaker's shield: the body of [`Self::shield_bodies`] carrying
    /// it, else a free one on its tile, else the free one nearest it (a
    /// Ram's own attackers first, then the lowest id).
    fn breaker_shield(&self, g: &Game, pid: usize, breaker: u32) -> Option<u32> {
        let bodies = self.shield_bodies(g, pid);
        if let Some(carrier) = Self::breaker_carrier(g, breaker).filter(|c| bodies.contains(c)) {
            return Some(carrier);
        }
        let here = g.units.get(&breaker)?.pos;
        bodies
            .into_iter()
            .filter(|uid| Self::shield_free(g, *uid))
            .min_by_key(|uid| {
                let unit = &g.units[uid];
                (
                    g.wdist(unit.pos, here),
                    !crate::ai::siege_support::eligible_attacker(&g.rules.units[unit.kind]),
                    *uid,
                )
            })
    }

    /// `conquest_staging_objective`: whether `uid` carries the opening's
    /// breaker, so it walks to the rally in the force or not. `false` with
    /// the gene off.
    pub(super) fn shield_carries_the_breaker(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.shielded_breaker(g, pid)
            .and_then(|breaker| Self::breaker_carrier(g, breaker))
            == Some(uid)
    }

    /// `advanced_units`: the body that moves first this turn, so the bodies
    /// that hold for the breaker read where it stands. `None` with the gene
    /// off.
    pub(super) fn shield_moves_first(&self, g: &Game, pid: usize) -> Option<u32> {
        let breaker = self.shielded_breaker(g, pid)?;
        self.breaker_shield(g, pid, breaker)
    }

    /// Turns until the shield at `carrier` stands beside the city.
    fn shield_eta(g: &Game, carrier: u32, city: Pos) -> i32 {
        let Some(unit) = g.units.get(&carrier) else {
            return i32::MAX;
        };
        let distance = g.wdist(unit.pos, city);
        if distance <= 1 {
            return 0;
        }
        let moves = g.unit_max_moves(carrier).max(1.0);
        (f64::from(distance - 1) / moves).ceil() as i32
    }

    /// A visible soldier of a rival we are at war with, on `pos`.
    fn shield_enemy_soldier_at(g: &Game, pid: usize, pos: Pos) -> Option<u32> {
        g.unit_ids_at(pos).iter().copied().find(|id| {
            let unit = &g.units[id];
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && g.rules.units[unit.kind].class == "military"
                && g.unit_visible_to(*id, pid)
        })
    }

    /// Whether a land soldier of ours, or a city of ours, holds `pos`.
    fn shield_safe_tile(g: &Game, pid: usize, pos: Pos, except: u32) -> bool {
        g.city_at(pos)
            .is_some_and(|cid| g.cities[&cid].owner == pid)
            || g.unit_ids_at(pos).iter().any(|id| {
                let unit = &g.units[id];
                let spec = &g.rules.units[unit.kind];
                *id != except
                    && unit.owner == pid
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && !g.is_embarked(unit)
            })
    }

    /// The tile beside the City Center the shield walks the breaker to:
    /// standable for the shield, with nothing foreign on it, fewest visible
    /// enemy soldiers beside it (the city's own garrison not counted), then
    /// not across a river from the city, then not open flat ground, then the
    /// nearest. Where it already stands, once it stands beside the city.
    fn shield_arrival_tile(g: &Game, pid: usize, shield: u32, city: Pos) -> Option<Pos> {
        let here = g.units.get(&shield)?.pos;
        if g.wdist(here, city) <= 1 {
            return Some(here);
        }
        g.nbrs(city)
            .into_iter()
            .filter(|pos| {
                *pos != city
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                    && g.unit_can_traverse(shield, *pos)
                    && g.can_stop(shield, *pos)
                    && g.unit_ids_at(*pos)
                        .iter()
                        .all(|id| g.units[id].owner == pid)
            })
            .min_by_key(|pos| {
                let threats = g
                    .nbrs(*pos)
                    .into_iter()
                    .filter(|near| *near != city)
                    .filter(|near| Self::shield_enemy_soldier_at(g, pid, *near).is_some())
                    .count();
                (
                    threats,
                    g.battle_crosses_river(*pos, city),
                    g.tile_defense_bonus(*pos) <= 0.0,
                    g.wdist(here, *pos),
                    *pos,
                )
            })
    }

    /// Link `breaker` to `shield` standing on its tile; journal the march.
    fn shield_link(&mut self, g: &mut Game, pid: usize, shield: u32, breaker: u32, city: u32) {
        let linked = g
            .apply(
                pid,
                &Action::LinkUnits {
                    unit: shield,
                    with: breaker,
                },
            )
            .is_ok();
        self.force_groups_dirty = true;
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let kind = plain(g.units[&breaker].kind.as_str());
            let body = plain(g.units[&shield].kind.as_str());
            let name = g.cities[&city].name.clone();
            let pos = g.units[&shield].pos;
            let distance = g.wdist(pos, g.cities[&city].pos);
            think!(self.journal(), Military, Decision,
                "Breaker under a shield: the {kind} follows the {body} to {name}";
                "stacked at {pos:?}, {distance} tiles from the walls{}; it ends no move without a soldier on its tile",
                if linked { ", in formation" } else { ", the formation refused, so it follows tile for tile" };
                pos);
        }
    }

    /// `breaker-walks-under-a-shield`: the turn of the opening's breaker, of
    /// its shield, and at war of the force's bodies. `None` leaves the unit
    /// to the shipped movers: the gene off, no opening, or a unit and a
    /// moment this gene has no order for. See the module.
    pub(super) fn shielded_breaker_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        // A home city under threat keeps its shipped first claim.
        if plan.threatened_city.is_some() {
            return None;
        }
        let scene = self.shield_scene(g, pid)?;
        let breaker = Self::scene_breaker(g, pid, &scene);
        if breaker == Some(uid) {
            return self.shielded_breaker_turn(g, pid, uid, &scene);
        }
        let shield = breaker.and_then(|breaker| self.breaker_shield(g, pid, breaker));
        if let (Some(breaker), Some(shield)) = (breaker, shield) {
            if shield == uid {
                if let Some(acted) = self.shield_turn(g, pid, uid, breaker, &scene) {
                    return Some(acted);
                }
            }
        }
        if scene.declared {
            return self.shielded_assault_turn(g, pid, uid, breaker, shield, &scene);
        }
        None
    }

    /// Item 1, the breaker: ride with the shield, take one up on its tile or
    /// in reach, or wait where a soldier or a city of ours stands.
    fn shielded_breaker_turn(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        scene: &ShieldScene,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?.clone();
        let bodies = self.shield_bodies(g, pid);
        if let Some(carrier) = Self::breaker_carrier(g, uid) {
            if bodies.contains(&carrier) {
                // The shield's orders move the pair.
                return Some(false);
            }
        }
        // A shield it can take up this turn: on its tile, or in reach.
        let shield = self.breaker_shield(g, pid, uid);
        let reach = g.reachable(uid);
        let take_up = shield.filter(|shield| {
            let at = g.units[shield].pos;
            at == unit.pos || reach.contains(&at)
        });
        if let Some(shield) = take_up {
            if unit.linked_to.is_some() && g.apply(pid, &Action::UnlinkUnits { unit: uid }).is_err()
            {
                return Some(false);
            }
            let to = g.units[&shield].pos;
            if to != unit.pos && !self.base.path_walk_to(g, pid, uid, to) {
                return Some(false);
            }
            if g.units.get(&uid).is_some_and(|ram| ram.pos == to) {
                self.shield_link(g, pid, shield, uid, scene.city);
            }
            return Some(true);
        }
        if Self::breaker_carrier(g, uid).is_some() {
            // Carried by a soldier that is no shield (a garrison, a guard):
            // it rides there until a shield comes for it.
            return Some(false);
        }
        if unit.linked_to.is_some() {
            let _ = g.apply(pid, &Action::UnlinkUnits { unit: uid });
        }
        let here = unit.pos;
        let kind = plain(unit.kind.as_str());
        let name = g.cities[&scene.city].name.clone();
        if Self::shield_safe_tile(g, pid, here, uid) {
            think!(self.journal(), Military, Detail,
                "Breaker under a shield: the {kind} waits at {here:?} for its shield";
                "{}; it walks on to {name} only stacked with a soldier",
                shield.map_or_else(|| "no melee body of the force can carry it".to_string(), |shield| format!(
                    "the {} is {} tiles off and comes back for it",
                    plain(g.units[&shield].kind.as_str()),
                    g.wdist(g.units[&shield].pos, here)
                ));
                here);
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        // Alone in the open: the tile in reach a soldier or a city of ours
        // holds, nearest its shield (else our nearest city); else toward our
        // nearest city.
        let home = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .min_by_key(|pos| (g.wdist(here, *pos), *pos));
        let toward = shield.map(|shield| g.units[&shield].pos).or(home)?;
        if let Some(to) = reach
            .iter()
            .copied()
            .filter(|pos| Self::shield_safe_tile(g, pid, *pos, uid))
            .min_by_key(|pos| (g.wdist(*pos, toward), *pos))
        {
            if self.base.path_walk_to(g, pid, uid, to) {
                return Some(true);
            }
        }
        let home = home?;
        let next = g
            .route_step(uid, home, 0)
            .filter(|next| g.can_move(uid, *next))?;
        Some(self.base.tactical_apply_move(g, pid, uid, next))
    }

    /// Items 1 and 2, the shield: walk back for an uncarried breaker, and at
    /// war carry it to its tile beside the walls and strike from there.
    /// `None` hands the turn on (to the force's orders or the shipped movers):
    /// before the war a carrying shield walks the shipped staging road.
    fn shield_turn(
        &mut self,
        g: &mut Game,
        pid: usize,
        shield: u32,
        breaker: u32,
        scene: &ShieldScene,
    ) -> Option<bool> {
        let carrying = Self::breaker_carrier(g, breaker) == Some(shield);
        let (here, there) = (g.units.get(&shield)?.pos, g.units.get(&breaker)?.pos);
        let walls_stand = g
            .cities
            .get(&scene.city)
            .is_some_and(|city| city.wall_hp > 0 && g.city_max_wall_hp(city) > 0);
        if scene.declared && !walls_stand {
            // Nothing to carry it to: the shield is one more body of the
            // assault.
            return None;
        }
        if !carrying {
            if here == there {
                if g.units[&shield].linked_to.is_none() {
                    self.shield_link(g, pid, shield, breaker, scene.city);
                    return Some(true);
                }
                return None;
            }
            if g.wdist(here, there) <= 1 && !g.can_stop(shield, there) {
                // The breaker steps onto the shield.
                return Some(self.base.fortify_or_stop(g, pid, shield));
            }
            let range = if g.can_stop(shield, there) { 0 } else { 1 };
            let next = g
                .route_step(shield, there, range)
                .filter(|next| g.can_move(shield, *next))?;
            let acted = self.base.tactical_apply_move(g, pid, shield, next);
            if acted {
                let body = plain(g.units[&shield].kind.as_str());
                let kind = plain(g.units[&breaker].kind.as_str());
                think!(self.journal(), Military, Detail,
                    "Breaker under a shield: the {body} walks back for the {kind}";
                    "the {kind} stands at {there:?} with no soldier to carry it; {} tiles to go",
                    g.wdist(next, there);
                    there);
                if g.units.get(&shield).is_some_and(|unit| unit.pos == there) {
                    self.shield_link(g, pid, shield, breaker, scene.city);
                }
            }
            return Some(acted);
        }
        if !scene.declared {
            return None;
        }
        if g.units[&shield].hp < super::battle_planner::ROTATE_HP {
            return None;
        }
        if g.wdist(here, scene.city_pos) <= 1 {
            if super::siege_train::melee_wall_attack_allowed(g, pid, shield, scene.city) {
                if let Some(acted) = self.shield_strike(g, pid, shield, scene, SHIELD_STRIKE_FLOOR)
                {
                    return Some(acted);
                }
            }
            return Some(self.base.fortify_or_stop(g, pid, shield));
        }
        // Every tile beside the city taken or barred: close as far as the
        // road goes, and hold there rather than hand the pair to the staging.
        let arrival = Self::shield_arrival_tile(g, pid, shield, scene.city_pos);
        let Some(next) = arrival
            .and_then(|arrival| g.route_step(shield, arrival, 0))
            .or_else(|| super::siege_train::march_step(g, shield, scene.city_pos, 1))
            .filter(|next| g.can_move(shield, *next))
        else {
            return Some(self.base.fortify_or_stop(g, pid, shield));
        };
        let arrival = arrival.unwrap_or(scene.city_pos);
        let acted = self.base.tactical_apply_move(g, pid, shield, next);
        if acted && self.journal().wants(crate::reasoning::Level::Detail) {
            let body = plain(g.units[&shield].kind.as_str());
            let kind = plain(g.units[&breaker].kind.as_str());
            let name = g.cities[&scene.city].name.clone();
            let (walls, max) = (
                g.cities[&scene.city].wall_hp,
                g.city_max_wall_hp(&g.cities[&scene.city]),
            );
            think!(self.journal(), Military, Detail,
                "Breaker under a shield: the {kind} follows the {body} to {name}";
                "walls {walls}/{max}; it walks to {arrival:?} beside the city, now {} tiles off",
                g.wdist(next, scene.city_pos);
                next);
        }
        Some(acted)
    }

    /// Item 3: one melee blow at the city, kept only when the attacker keeps
    /// `floor` health after the reply or the blow takes the city. A city at 0
    /// health is walked into.
    fn shield_strike(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        scene: &ShieldScene,
        floor: i32,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?.clone();
        if unit.attacks_left <= 0 || unit.moves_left <= 0.0 || g.is_embarked(&unit) {
            return None;
        }
        let city = g.cities.get(&scene.city)?;
        let (hp, walls) = (city.hp, city.wall_hp);
        let action = if hp <= 0 && g.can_move(uid, scene.city_pos) {
            Action::Move {
                unit: uid,
                to: scene.city_pos,
            }
        } else if g.melee_order_is_legal(pid, uid, scene.city_pos) {
            Action::Attack {
                unit: uid,
                target: scene.city_pos,
            }
        } else {
            return None;
        };
        let mut after = g.speculative_clone();
        after.apply(pid, &action).ok()?;
        let captured = after
            .cities
            .get(&scene.city)
            .is_some_and(|city| city.owner == pid);
        if !captured
            && after
                .units
                .get(&uid)
                .is_none_or(|survivor| survivor.hp < floor)
        {
            return None;
        }
        let name = g.cities[&scene.city].name.clone();
        g.apply(pid, &action).ok()?;
        self.force_groups_dirty = true;
        if let Some(memory) = self.breaker_shield_memory.as_mut() {
            if memory.strikes.0 == g.turn {
                memory.strikes.1 += 1;
            } else {
                memory.strikes = (g.turn, 1);
            }
        }
        let (hp_after, walls_after) = g
            .cities
            .get(&scene.city)
            .filter(|city| city.owner != pid)
            .map_or((0, 0), |city| (city.hp, city.wall_hp));
        let body = plain(unit.kind.as_str());
        think!(self.journal(), Military, Detail,
            "Breaker under a shield: the {body} strikes {name} beside the ram";
            "walls {walls} -> {walls_after}, health {hp} -> {hp_after}; the {body} keeps {}; captured {captured}",
            g.units.get(&uid).map_or(0, |unit| unit.hp);
            scene.city_pos);
        Some(true)
    }

    /// Items 2 and 3 for a body of the declared opening's force: the strike
    /// is a commitment, not a staged siege. No stage, muster line, danger
    /// limit or ring-seal hold of the siege train applies to these bodies;
    /// they walk in and strike as the module says. `None` hands the body to
    /// the shipped war: a wounded body (it heals), and walls with no breaker
    /// left (the release rules read that).
    fn shielded_assault_turn(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        breaker: Option<u32>,
        shield: Option<u32>,
        scene: &ShieldScene,
    ) -> Option<bool> {
        if !self.shield_assault_member(g, pid, uid, scene) && Some(uid) != shield {
            return None;
        }
        let unit = g.units.get(&uid)?.clone();
        let spec = g.rules.units[unit.kind].clone();
        let (ranged, melee) = (
            Self::conquest_ranged_body(&spec),
            Self::conquest_melee_body(&spec),
        );
        if g.is_embarked(&unit) || unit.hp < super::battle_planner::ROTATE_HP || !(ranged || melee)
        {
            return None;
        }
        let city = g.cities.get(&scene.city)?;
        let walls_stand = city.wall_hp > 0 && g.city_max_wall_hp(city) > 0;
        let beside = g.wdist(unit.pos, scene.city_pos) <= 1;
        if !walls_stand {
            // No walls to wait for: the melee strike from beside the city,
            // the shooters at the city.
            if ranged {
                return self.shielded_shooter_turn(g, pid, uid, shield, scene, true);
            }
            if beside {
                return Some(
                    self.shield_strike(g, pid, uid, scene, BODY_STRIKE_FLOOR)
                        .unwrap_or_else(|| self.base.fortify_or_stop(g, pid, uid)),
                );
            }
            return self.shield_close_in(g, pid, uid, shield, scene);
        }
        // Walls stand and nothing of ours can open them: the release rules
        // (`opening-yields-to-walls`) and the shipped war read that.
        let breaker = breaker?;
        if Some(uid) == shield {
            // The shield's turn is `shield_turn`'s; a shield it handed back
            // (wounded) is the shipped war's.
            return None;
        }
        let carried = shield.is_some_and(|shield| {
            Self::breaker_carrier(g, breaker) == Some(shield)
                || g.units[&breaker].pos == g.units[&shield].pos
        });
        let ram_beside = carried && g.wdist(g.units[&breaker].pos, scene.city_pos) <= 1;
        let eta = shield.filter(|_| carried).map_or(i32::MAX, |shield| {
            Self::shield_eta(g, shield, scene.city_pos)
        });
        let coming = ram_beside || eta <= SHIELD_HOLD_TURNS;
        if ranged {
            return self.shielded_shooter_turn(g, pid, uid, shield, scene, coming);
        }
        if ram_beside {
            if beside {
                if super::siege_train::melee_wall_attack_allowed(g, pid, uid, scene.city) {
                    if let Some(acted) = self.shield_strike(g, pid, uid, scene, BODY_STRIKE_FLOOR) {
                        return Some(acted);
                    }
                }
                return Some(self.base.fortify_or_stop(g, pid, uid));
            }
            return self.shield_close_in(g, pid, uid, shield, scene);
        }
        self.note_shield_hold(g, uid, eta, scene);
        if !coming {
            // The breaker is further off than the hold: wait just outside the
            // city's strike for it.
            return Some(self.shield_wait_outside(g, pid, uid, scene));
        }
        if beside {
            if let Some(acted) = self.shield_favourable_kill(g, pid, uid, scene) {
                return Some(acted);
            }
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        self.shield_close_in(g, pid, uid, shield, scene)
    }

    /// The army before the walls at war: the opening's force, and any other
    /// land body of the field army within [`ASSAULT_JOIN_RADIUS`] of the city
    /// that is no civilian's guard and stands in no city of ours. Live
    /// `civvis-20261111T072641Z` declared with 8 bodies, 5 of them the force.
    fn shield_assault_member(&self, g: &Game, pid: usize, uid: u32, scene: &ShieldScene) -> bool {
        if self
            .conquest_opening
            .as_ref()
            .is_some_and(|opening| opening.force.contains(&uid))
        {
            return true;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let spec = &g.rules.units[unit.kind];
        unit.owner == pid
            && g.wdist(unit.pos, scene.city_pos) <= ASSAULT_JOIN_RADIUS
            && f64::from(unit.hp) > self.base.w.withdraw_hp
            && (Self::conquest_melee_body(spec) || Self::conquest_ranged_body(spec))
            && g.city_at(unit.pos)
                .is_none_or(|cid| g.cities[&cid].owner != pid)
            && !self.all_reserved_civilian_guards().contains(&uid)
    }

    /// A body of the force waits for a breaker still more than
    /// [`SHIELD_HOLD_TURNS`] off: it closes to just outside the city's strike
    /// and stands there.
    fn shield_wait_outside(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        scene: &ShieldScene,
    ) -> bool {
        let outside = super::siege_train::CITY_STRIKE_RANGE + 1;
        let here = g.units[&uid].pos;
        if g.wdist(here, scene.city_pos) > outside {
            if let Some(next) = super::siege_train::march_step(g, uid, scene.city_pos, outside)
                .filter(|next| g.can_move(uid, *next))
            {
                return self.base.tactical_apply_move(g, pid, uid, next);
            }
        }
        self.base.fortify_or_stop(g, pid, uid)
    }

    /// Journal the hold once a turn.
    fn note_shield_hold(&mut self, g: &Game, uid: u32, eta: i32, scene: &ShieldScene) {
        if let Some(memory) = self.breaker_shield_memory.as_mut() {
            if memory.hold_noted == Some(g.turn) {
                return;
            }
            memory.hold_noted = Some(g.turn);
        }
        let body = plain(g.units[&uid].kind.as_str());
        let name = g.cities[&scene.city].name.clone();
        let (walls, max) = (
            g.cities[&scene.city].wall_hp,
            g.city_max_wall_hp(&g.cities[&scene.city]),
        );
        think!(self.journal(), Military, Decision,
            "Breaker under a shield: the {body} holds the assault on {name} until the ram stands beside the walls ({} turn(s))",
            if eta == i32::MAX { "the ram has no shield yet; more than 2".to_string() } else { eta.to_string() };
            "walls {walls}/{max}: a melee blow without the ram beside the city lands 15% on them and takes the full reply";
            scene.city_pos);
    }

    /// A free tile beside the city for a melee body, the shield's own tile to
    /// come left to it; else the city's range two.
    fn shield_close_in(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        shield: Option<u32>,
        scene: &ShieldScene,
    ) -> Option<bool> {
        let here = g.units.get(&uid)?.pos;
        let reserved = shield
            .filter(|shield| *shield != uid)
            .and_then(|shield| Self::shield_arrival_tile(g, pid, shield, scene.city_pos));
        let goal = g
            .nbrs(scene.city_pos)
            .into_iter()
            .filter(|pos| {
                Some(*pos) != reserved
                    && *pos != scene.city_pos
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                    && g.unit_can_traverse(uid, *pos)
                    && g.can_stop(uid, *pos)
                    && g.unit_ids_at(*pos)
                        .iter()
                        .all(|id| g.units[id].owner == pid)
            })
            .min_by_key(|pos| (g.wdist(here, *pos), g.tile_defense_bonus(*pos) <= 0.0, *pos));
        let next = match goal {
            Some(goal) => g.route_step(uid, goal, 0),
            None if g.wdist(here, scene.city_pos) > 2 => {
                super::siege_train::march_step(g, uid, scene.city_pos, 2)
            }
            None => return Some(self.base.fortify_or_stop(g, pid, uid)),
        }
        .filter(|next| g.can_move(uid, *next))?;
        Some(self.base.tactical_apply_move(g, pid, uid, next))
    }

    /// A melee body holding beside the city kills a defender beside it when
    /// the blow kills and it keeps [`BODY_STRIKE_FLOOR`].
    fn shield_favourable_kill(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        scene: &ShieldScene,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?;
        if unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
            return None;
        }
        let mut targets: Vec<(Pos, u32)> = g
            .nbrs(unit.pos)
            .into_iter()
            .filter(|pos| *pos != scene.city_pos)
            .filter_map(|pos| Self::shield_enemy_soldier_at(g, pid, pos).map(|id| (pos, id)))
            .collect();
        targets.sort_unstable();
        for (pos, defender) in targets {
            if !g.melee_order_is_legal(pid, uid, pos) {
                continue;
            }
            let action = Action::Attack {
                unit: uid,
                target: pos,
            };
            let mut after = g.speculative_clone();
            if after.apply(pid, &action).is_err() {
                continue;
            }
            let killed = !after.units.contains_key(&defender);
            let keeps = after
                .units
                .get(&uid)
                .is_some_and(|survivor| survivor.hp >= BODY_STRIKE_FLOOR);
            if killed && keeps && g.apply(pid, &action).is_ok() {
                self.force_groups_dirty = true;
                return Some(true);
            }
        }
        None
    }

    /// A shooter of the force: a soldier beside the shield first, else the
    /// city, else close to range two (`close`) or wait just outside the
    /// city's strike.
    fn shielded_shooter_turn(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        shield: Option<u32>,
        scene: &ShieldScene,
        close: bool,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?.clone();
        let range = g.unit_attack_range(uid).max(1);
        if unit.attacks_left > 0 && unit.moves_left > 0.0 {
            let shield_pos = shield
                .and_then(|shield| g.units.get(&shield))
                .map(|s| s.pos);
            let mut threats: Vec<(i32, Pos)> = Vec::new();
            if let Some(at) = shield_pos {
                for pos in g.nbrs(at) {
                    if pos == scene.city_pos || g.wdist(unit.pos, pos) > range {
                        continue;
                    }
                    if let Some(id) = Self::shield_enemy_soldier_at(g, pid, pos) {
                        threats.push((g.units[&id].hp, pos));
                    }
                }
            }
            let threat = threats.into_iter().min();
            for target in threat
                .map(|(_, pos)| pos)
                .into_iter()
                .chain((g.wdist(unit.pos, scene.city_pos) <= range).then_some(scene.city_pos))
            {
                if g.apply(pid, &Action::Ranged { unit: uid, target }).is_ok() {
                    self.force_groups_dirty = true;
                    return Some(true);
                }
            }
        }
        if !close {
            return Some(self.shield_wait_outside(g, pid, uid, scene));
        }
        if g.wdist(unit.pos, scene.city_pos) > 2 && unit.moves_left > 0.0 {
            if let Some(next) = super::siege_train::march_step(g, uid, scene.city_pos, 2)
                .filter(|next| g.can_move(uid, *next))
            {
                return Some(self.base.tactical_apply_move(g, pid, uid, next));
            }
        }
        Some(self.base.fortify_or_stop(g, pid, uid))
    }

    // ------------------------------------------------------------------
    // item 4: the watch
    // ------------------------------------------------------------------

    /// `maintain_conquest_opening`, before its release checks: journal the
    /// breaker's loss since the last look, with where and to what, then
    /// look again. Nothing with the gene off.
    pub(super) fn breaker_shield_watch(&mut self, g: &Game, pid: usize) {
        if !self.breaker_walks_under_a_shield {
            return;
        }
        let previous = self.breaker_shield_memory.take();
        if let Some(memory) = previous.as_ref() {
            let alive = memory.breaker.is_some_and(|id| {
                g.units
                    .get(&id)
                    .is_some_and(|unit| unit.owner == pid && unit.kind == memory.kind)
            });
            // A live rebuild that could not carry the id: the same kind of
            // ours still on the tile is the same breaker.
            let still_there = g.unit_ids_at(memory.pos).iter().any(|id| {
                let unit = &g.units[id];
                unit.owner == pid && unit.kind == memory.kind
            });
            if !alive && !still_there {
                self.journal_breaker_loss(g, pid, memory);
            }
        }
        self.breaker_shield_memory = self.breaker_shield_snapshot(g, pid, previous);
    }

    /// `advanced_units`, after the unit loop: where the breaker and its shield
    /// end our turn, and this turn's strikes beside the ram. Nothing with the
    /// gene off.
    pub(super) fn breaker_shield_note(&mut self, g: &Game, pid: usize) {
        if !self.breaker_walks_under_a_shield {
            return;
        }
        let previous = self.breaker_shield_memory.take();
        let strikes = previous
            .as_ref()
            .filter(|memory| memory.strikes.0 == g.turn)
            .map_or(0, |memory| memory.strikes.1);
        self.breaker_shield_memory = self.breaker_shield_snapshot(g, pid, previous);
        let Some(scene) = self.shield_scene(g, pid) else {
            return;
        };
        let Some(breaker) = Self::scene_breaker(g, pid, &scene) else {
            return;
        };
        if strikes == 0 || g.wdist(g.units[&breaker].pos, scene.city_pos) > 1 {
            return;
        }
        let city = &g.cities[&scene.city];
        let name = city.name.clone();
        let (hp, walls, max) = (city.hp, city.wall_hp, g.city_max_wall_hp(city));
        let strength = g.city_strength(scene.city);
        let garrison = g
            .wdisk(scene.city_pos, 1)
            .into_iter()
            .filter_map(|pos| Self::shield_enemy_soldier_at(g, pid, pos))
            .count();
        think!(self.journal(), Military, Decision,
            "Breaker under a shield: the ram stands beside {name}; {strikes} bodies strike the walls";
            "walls {walls}/{max}, health {hp} after the blows; city strength {strength:.0}; {garrison} soldier(s) of theirs on or beside it";
            scene.city_pos);
    }

    /// The memory for this board: the breaker, its tile and its shield.
    fn breaker_shield_snapshot(
        &self,
        g: &Game,
        pid: usize,
        previous: Option<BreakerShieldMemory>,
    ) -> Option<BreakerShieldMemory> {
        let scene = self.shield_scene(g, pid)?;
        let breaker = Self::scene_breaker(g, pid, &scene)?;
        let shield = self.breaker_shield(g, pid, breaker);
        let (hold_noted, strikes) =
            previous.map_or((None, (0, 0)), |memory| (memory.hold_noted, memory.strikes));
        Some(BreakerShieldMemory {
            breaker: Some(breaker),
            kind: g.units[&breaker].kind,
            pos: g.units[&breaker].pos,
            shield,
            shield_kind: shield.map(|shield| g.units[&shield].kind),
            city_pos: scene.city_pos,
            hold_noted,
            strikes,
        })
    }

    /// Journal a lost breaker: captured where a soldier of theirs now stands
    /// on its tile, killed by the soldier of theirs beside it, else lost
    /// unseen.
    fn journal_breaker_loss(&self, g: &Game, pid: usize, memory: &BreakerShieldMemory) {
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let kind = plain(memory.kind.as_str());
        let pos = memory.pos;
        let name = g.city_at(memory.city_pos).map_or_else(
            || "the walls".to_string(),
            |cid| g.cities[&cid].name.clone(),
        );
        let civ = |id: u32| g.players[g.units[&id].owner].civ.clone();
        let on_tile = g.unit_ids_at(pos).iter().copied().find(|id| {
            let unit = &g.units[id];
            unit.owner != pid && g.rules.units[unit.kind].class == "military"
        });
        let beside = g
            .nbrs(pos)
            .into_iter()
            .filter_map(|near| Self::shield_enemy_soldier_at(g, pid, near))
            .min();
        let cause = match (on_tile, beside) {
            (Some(id), _) => format!(
                "captured at {pos:?} by the {} of {}",
                plain(g.units[&id].kind.as_str()),
                civ(id)
            ),
            (None, Some(id)) => format!(
                "killed at {pos:?} by the {} of {} beside it",
                plain(g.units[&id].kind.as_str()),
                civ(id)
            ),
            (None, None) => format!("lost at {pos:?}; no soldier of theirs stands in sight there"),
        };
        let shield = match (memory.shield, memory.shield_kind) {
            (Some(id), Some(body)) => match g.units.get(&id) {
                Some(unit) if unit.owner == pid => format!(
                    "its shield, the {}, stands at {:?} with {} health",
                    plain(body.as_str()),
                    unit.pos,
                    unit.hp
                ),
                _ => format!("its shield, the {}, is gone too", plain(body.as_str())),
            },
            _ => "it had no shield".to_string(),
        };
        think!(self.journal(), Military, Decision,
            "Breaker under a shield: the {kind} is lost before {name}";
            "{cause}; {shield}";
            pos);
    }

    /// Carry the memory through a live board rebuild (`remap_conquest_memory`).
    pub(super) fn remap_breaker_shield_memory(
        &mut self,
        units: &std::collections::BTreeMap<u32, u32>,
    ) {
        if let Some(memory) = self.breaker_shield_memory.as_mut() {
            memory.breaker = memory.breaker.and_then(|id| units.get(&id).copied());
            memory.shield = memory.shield.and_then(|id| units.get(&id).copied());
        }
    }
}

#[cfg(test)]
mod tests;

// TEMPORARY CI scaffold; removed before ship.
#[cfg(test)]
mod replay_scaffold;
