//! The contact posture: a unit that is already inside somebody's reach picks
//! between standing, closing and leaving — by pricing the exchange it is
//! standing in rather than by scoring attacks alone.
//!
//! ★★★★ THE CONTROLLER HAS NEVER HAD A REASON TO *NOT* SWING. Every tactical
//! decision in `advanced_military_step_with_decline` is an attack scan: score
//! each blow this unit can land, take the best if it clears the doctrine
//! threshold, and otherwise fall through to movement. A declined attack
//! therefore produces a *march*, not a stand, and nothing anywhere prices the
//! one line a human plays constantly — dig in on good ground, let the melee
//! come to you, and take the counter-damage trade while the wound closes.
//!
//! Civilization VI pays for that line three separate ways, and the engine
//! here implements all three:
//!
//! - **The counter.** `Game::do_attack` resolves a melee blow *both ways*:
//!   the defender takes `damage(att, def)` and the attacker takes
//!   `damage(def, att)` from the same pair of strengths. An attacker walking
//!   onto a stronger defensive position loses the exchange while initiating
//!   it.
//! - **Fortification and terrain.** `Game::unit_strength(u, true)` adds
//!   `3 × fortify_turns` (capped at two turns, so +6), and `do_attack` adds
//!   `tile_defense_bonus` and `support_bonus` on the defender's side only.
//!   Swinging clears both: `consume_unit_attack` sets `fortified = false` and
//!   `fortify_turns = 0`.
//! - **Healing.** `Game::end_turn` heals a unit only when `!acted`, and
//!   `do_fortify` deliberately does not set `acted`. So standing still is the
//!   only action that recovers 5–20 hit points a turn, and *any* attack or
//!   move forfeits that turn's recovery.
//!
//! ★★★ AND THE ZONE OF CONTROL DECIDES WHO IS EVEN IN THE ARGUMENT. `flow_past`
//! zeroes a unit's remaining movement the instant it enters an enemy zone of
//! control, so a melee body two tiles out cannot step into contact and swing
//! on the same turn: it arrives, and *then* it is adjacent. Its
//! `Game::attack_reach` says so, which is why the covering set below is read
//! off the engine's own envelopes rather than off distance. The consequence is
//! that the stand is only ever offered against a body already in contact —
//! and against those, the whole exchange is the counter, the fortification and
//! the wound closing.
//!
//! ⚠⚠ AND NONE OF IT APPLIES TO A SHOOTER. `do_ranged` deals damage in one
//! direction and there is no second `damage` call in it — the whole "they hurt
//! themselves attacking us" argument is void against ranged, which is exactly
//! why the ranged half of this gene is the *opposite* instruction. A unit
//! standing inside an archer's envelope with no reply of its own is paying
//! rent for nothing: it must either close and take the shooter off the board,
//! or leave the envelope. Hovering just inside it is the one posture that is
//! never right, and it is what the shipped march produces today, because
//! `tactical_step` keeps a unit at attack range from its target and an
//! unanswered shooter is not a target it can attack.
//!
//! ## What the gene actually decides
//!
//! For one land military unit already covered by at least one visible
//! hostile's next-turn attack envelope ([`crate::ai::BasicAi::enemy_attack_envelopes`],
//! the same mobility-true reading `retreat_step` uses), and only for the seat's
//! own major units in a war:
//!
//! 1. **Break or close** when a covering hostile can shoot us and we cannot
//!    answer it where it stands. Close if it is inside [`CHARGE_TURNS`] turns
//!    of movement and we expect to arrive with a fighting body; otherwise
//!    step out of every unanswered shooter's envelope, preferring ground that
//!    heals.
//! 2. **Stand and heal** when every covering hostile has to come to us —
//!    and the arithmetic of it beats the arithmetic of swinging. Both sides
//!    of that comparison are the engine's own
//!    [`crate::game::Game::melee_exchange_strengths`], run once with the
//!    enemy as attacker (what standing pays) and once with us as attacker
//!    (what swinging pays), so the two branches of the decision are priced on
//!    one scale and neither is a re-derivation.
//! 3. **Nothing** otherwise: the ordinary attack scan and march run exactly
//!    as they do today.
//!
//! ## What is deliberately *not* claimed
//!
//! Standing collects its counter-damage only if the enemy actually attacks.
//! Against an opponent that also declines bad trades, the stand wins the heal
//! and nothing else — so the gene's value depends on the tables it is played
//! at, and it is off until a screen prices it. Two known cautions were read
//! before this was written and both are respected: `tactics.rs`'s
//! [`strike_prior`](super::super::tactics) records that folding terrain into a
//! *reply* estimate "mostly licenses braver stands" and measured worse, which
//! is why terrain enters here only on the defender's side of a blow the
//! engine itself computes that way; and `docs/GENOME.md` records that pure
//! valuation tuning has always returned null, which is why this adds a
//! decision the controller could not previously take rather than a weight on
//! one it could.

use super::AdvancedAi;
use crate::game::{expected_damage, Game};
use crate::think;
use crate::Pos;

/// How many turns of closing an unanswered shooter is worth before leaving is
/// the better answer. Two: one to cross the ground a shooter typically keeps
/// between itself and a melee unit, one for the blow.
const CHARGE_TURNS: i32 = 2;

/// The health a charge must expect to arrive with, after paying the shots it
/// will eat on the way in. Below it the unit reaches the shooter as a body
/// that loses the fight it walked into, and leaving is strictly better.
const CHARGE_ARRIVAL_FLOOR: f64 = 25.0;

/// One hostile whose next-turn envelope covers our tile.
struct Covering {
    id: u32,
    pos: Pos,
    /// Damage it does to us where we stand, at the engine's unrandomized
    /// centre.
    incoming: f64,
    /// What it takes back for doing so. Zero for a shot: `do_ranged` has no
    /// second blow.
    returned: f64,
    /// Does it have to enter our reach to hurt us? A melee attacker pays
    /// the counter; a shooter does not.
    melee: bool,
    /// Can it hit us without entering our reach — and can we not answer it?
    unanswered_shooter: bool,
}

impl AdvancedAi {
    /// The gene. `None` leaves this unit to the ordinary attack scan and
    /// march; `Some(acted)` means the posture claimed the turn.
    pub(super) fn contact_posture_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.contact_posture {
            return None;
        }
        // Nothing heals on a Tactics arena (`Game::unit_heal_rate` returns
        // zero there by design), so half this gene's arithmetic is a constant
        // and the other half duplicates the joint search's withdraw lines.
        // `BasicAi::healing_step` stands down there for the same reason.
        if g.is_arena() || self.base.minor || self.base.barb {
            return None;
        }
        // The joint engagement search already weighed this unit's fight
        // against what the rest of the army is doing. Re-deciding it here
        // would let one unit take back a trade the plan made on purpose —
        // the same rule the attack scan follows for `tactics_resolved`.
        if self.tactics_resolved.contains(&uid) || self.tactics_withdrawn.contains(&uid) {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let spec = &g.rules.units[unit.kind];
        if unit.owner != pid
            || spec.class != "military"
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || g.is_embarked(unit)
        {
            return None;
        }
        let hp = f64::from(unit.hp);
        let covering = self.covering_hostiles(g, pid, uid);
        if covering.is_empty() {
            return None;
        }
        // Standing still is the only action that heals, so the heal is the
        // stand's income and every other branch's cost.
        let heal = f64::from(g.unit_heal_rate(uid));

        let unanswered: Vec<&Covering> = covering
            .iter()
            .filter(|threat| threat.unanswered_shooter)
            .collect();
        if !unanswered.is_empty() {
            let shots: f64 = unanswered.iter().map(|threat| threat.incoming).sum();
            // Out-healing the volley is not "being shot at for nothing", and
            // a unit holding ground it is not losing keeps its orders.
            if shots > heal {
                let targets: Vec<(u32, Pos)> = unanswered
                    .iter()
                    .map(|threat| (threat.id, threat.pos))
                    .collect();
                if let Some(acted) = self.close_on_shooter(g, pid, uid, &targets, shots, hp) {
                    return Some(acted);
                }
                if let Some(acted) = self.leave_the_envelope(g, pid, uid, shots) {
                    return Some(acted);
                }
            }
            // Cornered inside the envelope with nothing to charge and nowhere
            // to go. That is the ordinary path's problem, not this one's.
            return None;
        }

        self.stand_and_heal(g, pid, uid, &covering)
    }

    /// Every visible hostile whose next-turn attack envelope covers this
    /// unit's tile, priced both ways.
    fn covering_hostiles(&self, g: &Game, pid: usize, uid: u32) -> Vec<Covering> {
        let envelopes = self.base.enemy_attack_envelopes(g, pid);
        let here = g.units[&uid].pos;
        // The ranged legality frames are hoisted exactly as the attack scan
        // hoists them: `player_vision_now` clones a whole `TileBits`, and
        // nothing here applies an action that could move it.
        let mut frames = None;
        let mut out = Vec::new();
        for (eid, reach) in envelopes.iter() {
            if !reach.contains(&here) {
                continue;
            }
            let Some(enemy) = g.units.get(eid) else {
                continue;
            };
            let enemy_spec = &g.rules.units[enemy.kind];
            let shoots = enemy_spec.has_ranged_attack();
            let (incoming, returned) = if shoots {
                let shot = g
                    .ranged_strike_strengths(*eid, uid, here)
                    .map(|(att, def)| expected_damage(att, def))
                    .unwrap_or(0.0);
                (shot, 0.0)
            } else {
                g.melee_exchange_strengths(*eid, uid)
                    .map(|(att, def)| (expected_damage(att, def), expected_damage(def, att)))
                    .unwrap_or((0.0, 0.0))
            };
            // A shooter we can hit back is not a shooting gallery; it is a
            // target, and the attack scan below owns it.
            let unanswered_shooter = shoots && {
                let frames = frames
                    .get_or_insert_with(|| (g.player_vision_now(pid), g.visibility_viewers(pid)));
                !g.melee_order_is_legal(pid, uid, enemy.pos)
                    && !g.ranged_order_is_legal(pid, uid, enemy.pos, &frames.0, &frames.1)
            };
            out.push(Covering {
                id: *eid,
                pos: enemy.pos,
                incoming,
                returned,
                melee: !shoots,
                unanswered_shooter,
            });
        }
        out
    }

    /// Take the shooter off the board. Only worth starting when the unit
    /// expects to arrive as something that can still fight.
    fn close_on_shooter(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        targets: &[(u32, Pos)],
        shots_per_turn: f64,
        hp: f64,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?;
        if unit.moves_left <= 0.0 {
            return None;
        }
        let here = unit.pos;
        let spec = &g.rules.units[unit.kind];
        let stop = if spec.has_ranged_attack() {
            g.unit_attack_range(uid).max(1)
        } else {
            1
        };
        let pace = g.unit_max_moves(uid).max(1.0);
        // Nearest first: the shortest walk is the one that eats the fewest
        // shots, whatever else is in the volley.
        let mut ordered: Vec<(u32, Pos)> = targets.to_vec();
        ordered.sort_by_key(|(id, pos)| (g.wdist(here, *pos), *id));
        for (id, pos) in ordered {
            let gap = f64::from((g.wdist(here, pos) - stop).max(0));
            let turns = (gap / pace).ceil().max(1.0);
            if turns > f64::from(CHARGE_TURNS) {
                continue;
            }
            if hp - shots_per_turn * turns < CHARGE_ARRIVAL_FLOOR {
                continue;
            }
            if !self.base.step_toward_range(g, pid, uid, pos, stop) {
                continue;
            }
            if self.journal().wants(crate::reasoning::Level::Detail) {
                let kind = g
                    .units
                    .get(&id)
                    .map(|enemy| crate::reasoning::plain(&enemy.kind))
                    .unwrap_or_else(|| "shooter".to_string());
                think!(self.journal(), Military, Detail,
                       "Closing on the {kind}";
                       "it shoots us for {shots_per_turn:.0} a turn and takes nothing back; \
                        {turns:.0} turn(s) to reach it leaves {:.0} health to fight with",
                       hp - shots_per_turn * turns; pos);
            }
            return Some(true);
        }
        None
    }

    /// Step out of every unanswered shooter's envelope. Safety first, then
    /// the ground that heals fastest, then distance from the guns.
    fn leave_the_envelope(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        shots_here: f64,
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?;
        if unit.moves_left <= 0.0 {
            return None;
        }
        let here = unit.pos;
        let envelopes = self.base.enemy_attack_envelopes(g, pid);
        // Only the shooters we could not answer are worth stepping around;
        // walking out of a melee unit's reach is the joint search's withdraw
        // line, priced there and deliberately not duplicated here.
        let mut frames = None;
        let guns: Vec<(Pos, &std::collections::BTreeSet<Pos>)> = envelopes
            .iter()
            .filter_map(|(eid, reach)| {
                let enemy = g.units.get(eid)?;
                if !g.rules.units[enemy.kind].has_ranged_attack() || !reach.contains(&here) {
                    return None;
                }
                let frames = frames
                    .get_or_insert_with(|| (g.player_vision_now(pid), g.visibility_viewers(pid)));
                let answerable = g.melee_order_is_legal(pid, uid, enemy.pos)
                    || g.ranged_order_is_legal(pid, uid, enemy.pos, &frames.0, &frames.1);
                (!answerable).then_some((enemy.pos, reach.as_ref()))
            })
            .collect();
        if guns.is_empty() {
            return None;
        }
        // Fewest guns still covering the tile wins, then the ground that
        // heals fastest, then the most air between us and the guns. `pos`
        // last so the choice is deterministic on a tie.
        let (covered, target) = g
            .approach_reach(uid)
            .into_keys()
            .filter(|pos| *pos != here)
            .map(|pos| {
                let covered = guns
                    .iter()
                    .filter(|(_, reach)| reach.contains(&pos))
                    .count();
                let healing = g.healing_location(pid, pos).rate();
                let spacing = guns
                    .iter()
                    .map(|(gun, _)| g.wdist(pos, *gun))
                    .min()
                    .unwrap_or(0);
                (
                    covered,
                    std::cmp::Reverse(healing),
                    std::cmp::Reverse(spacing),
                    pos,
                )
            })
            .min()
            .map(|(covered, _, _, pos)| (covered, pos))?;
        // Stepping to another tile the same guns cover is motion, not
        // disengagement, and it costs the turn's healing to achieve nothing.
        if covered >= guns.len() {
            return None;
        }
        if !self.base.move_to_evacuation_tile(g, pid, uid, target) {
            return None;
        }
        if self.journal().wants(crate::reasoning::Level::Detail) {
            think!(self.journal(), Military, Detail,
                   "Stepping out of the shooting";
                   "{shots_here:.0} damage a turn from {} unanswerable shooter(s) here, \
                    {covered} of them still covering {target:?}",
                   guns.len(); target);
        }
        Some(true)
    }

    /// Dig in and let them come. Taken only when the standing side of the
    /// exchange beats the swinging side of the same exchange.
    fn stand_and_heal(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        covering: &[Covering],
    ) -> Option<bool> {
        let unit = g.units.get(&uid)?;
        let here = unit.pos;
        let hp = f64::from(unit.hp);
        let heal = f64::from(g.unit_heal_rate(uid));
        // `unit_strength(u, true)` carries `3 × fortify_turns.clamp(0, 2)`,
        // and `do_fortify` raises a standing unit's count to at least one
        // immediately — so the stand is priced with the bonus the stand will
        // actually have when the blow lands, not the one it has now.
        let fortifying = g.unit_can_fortify(unit) && unit.fortify_turns < 1;
        let fortify_gain = if fortifying { 3.0 } else { 0.0 };
        let melee: Vec<&Covering> = covering.iter().filter(|threat| threat.melee).collect();
        if melee.is_empty() {
            return None;
        }
        // Adding to the defence lowers what we take and raises what they take,
        // both by the engine's own curve.
        let incoming: f64 = melee
            .iter()
            .map(|threat| Self::shift_defence(threat.incoming, fortify_gain, false))
            .sum();
        let returned: f64 = melee
            .iter()
            .map(|threat| Self::shift_defence(threat.returned, fortify_gain, true))
            .sum();
        if incoming >= hp {
            // Standing through a round we do not survive is not a posture.
            return None;
        }
        let stand = returned + heal - incoming;

        // The other side of the same exchange: what this unit gets for
        // swinging at a covering threat it can already reach. `do_attack`
        // resolves an attack both ways too, so this is the identical
        // arithmetic with the roles reversed — plus the healing the swing
        // forfeits, which is the term nothing in the shipped scan can see.
        let swing = melee
            .iter()
            .filter(|threat| g.melee_order_is_legal(pid, uid, threat.pos))
            .filter_map(|threat| g.melee_exchange_strengths(uid, threat.id))
            .map(|(att, def)| expected_damage(att, def) - expected_damage(def, att) - heal)
            .fold(f64::NEG_INFINITY, f64::max);
        // A kill ends the exchange, and no arithmetic about future rounds
        // beats taking the unit off the board now.
        let lethal = melee.iter().any(|threat| {
            g.melee_order_is_legal(pid, uid, threat.pos)
                && g.units.get(&threat.id).is_some_and(|enemy| {
                    g.melee_exchange_strengths(uid, threat.id)
                        .is_some_and(|(att, def)| expected_damage(att, def) >= f64::from(enemy.hp))
                })
        });
        if lethal || stand <= swing || stand < 0.0 {
            return None;
        }
        let acted = self.base.fortify_or_stop(g, pid, uid);
        if self.journal().wants(crate::reasoning::Level::Detail) {
            let swing_text = if swing.is_finite() {
                format!("{swing:.0}")
            } else {
                "no blow available".to_string()
            };
            think!(self.journal(), Military, Detail,
                   "Standing to receive {} attacker(s)", melee.len();
                   "holding trades {returned:.0} out for {incoming:.0} in and heals {heal:.0}, \
                    worth {stand:.0} against {swing_text} for attacking, on {hp:.0} health";
                   here);
        }
        Some(acted)
    }

    /// Re-price one blow of a melee exchange for a defence raised by
    /// `gain`. `Game::damage` is `30 · e^((att − def)/25)`, so a defence that
    /// rises by `gain` multiplies the damage taken by `e^(−gain/25)` and the
    /// damage returned by `e^(gain/25)` — exact, except where the engine's own
    /// `1..=100` clamp already bound the blow, which the clamp below restores.
    fn shift_defence(blow: f64, gain: f64, defender_is_striking: bool) -> f64 {
        if gain == 0.0 {
            return blow;
        }
        let exponent = if defender_is_striking {
            gain / 25.0
        } else {
            -gain / 25.0
        };
        (blow * exponent.exp()).clamp(1.0, 100.0)
    }
}
