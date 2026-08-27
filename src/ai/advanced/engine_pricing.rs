//! Price it like the engine: two opt-in genes that replace a hand-written
//! estimate of a fight with the arithmetic the engine will actually resolve.
//!
//! `Game::melee_exchange_strengths` and `Game::ranged_strike_strengths` are
//! the engine's own strength pair, exposed for exactly this — `do_attack`
//! and `do_ranged` call them, so a controller that prices a fight with them
//! cannot drift from the fight it will get. They carry matchup, flanking,
//! adjacent support, terrain, the river and amphibious penalties,
//! fortification and every promotion. The two places the controller decides
//! a fight both use a strength-only estimate instead, and each is wrong in
//! its own way:
//!
//! - **`exchange_score`** (`BasicAi`, the static exchange evaluation behind
//!   move ordering, the `military_step` accept/decline, and the force's
//!   `focus_target`) reads `unit_strength` bare. So a spearman is priced
//!   against a horseman as if the anti-cavalry bonus did not exist, a unit
//!   attacking across a river pays nothing for the crossing, and a defender
//!   on a hill in a forest behind two friends is priced as if it stood in
//!   the open. `fire-plan` already prices its kills exactly; this makes the
//!   ordinary scan agree with it.
//! - **The threat terms** (`projected_counter_damage`, and the same
//!   arithmetic inline in `coordinated_tactical_step`'s tile score) ask
//!   "what would the enemy do to me if I stood on *that* tile" and then
//!   price the defender **where it is standing now**, with no
//!   `tile_defense_bonus` at all. A unit weighing a hill gets no credit for
//!   the hill; a unit weighing an open tile beside a forest pays nothing for
//!   leaving it. `incoming_damage` — the evacuation path — already does this
//!   correctly, by cloning the unit onto the tile before pricing it; the
//!   movers never learned.
//!
//! Both genes are `Kind::OptIn`, off in `AdvancedAi::new()` and `legacy()`,
//! and byte-identical when off: each is a branch taken only with its flag
//! set, and the flags live on `BasicAi` because both functions do (the
//! `contested-land-first` precedent). Priced first on the arena
//! (`doctrine_arena --a advanced+<gene>`, the curriculum and a captured
//! engagement file, healing off and on) and on `battle_bench`; the
//! whole-game screen is the no-harm check. See `docs/DOCTRINE_ARENA.md`,
//! "The gate for a tactical gene".
//!
//! ## What is deliberately *not* here
//!
//! A Great General's aura. The ranking this work came from asked for one,
//! and the engine has no Great General to give an aura to: Great People are
//! recruited effects rather than board pieces (`data/great_people.json`),
//! `Game`'s `"general"` arm is a one-shot empire-wide `+1 level` on
//! recruitment (`src/game.rs`), and `data/units.json` has no
//! `great_person` class. The only adjacency combat terms the engine has are
//! `flanking_bonus`, `support_bonus` and the one unit that carries
//! `adjacent_combat_strength` — and all three arrive for free with the
//! exact strengths below. Adding a general as a unit is an engine feature
//! and a fidelity question, not a controller gene; `docs/AI_GAPS.md`
//! records it.

#[cfg(test)]
use super::AdvancedAi;
#[cfg(test)]
use crate::game::effective_strength;
use crate::game::{expected_damage, Game};
use crate::{ai::BasicAi, Pos};

impl BasicAi {
    /// The engine's own expected damage for one strike on `pos`, and the
    /// blow that comes back — `None` when the gene is off or the engine
    /// cannot price this pair (a unit gone, a target that is not a unit).
    ///
    /// `(dealt, returned)`. A shot returns nothing, which is the whole
    /// asymmetry `ranged_strike_strengths` exists to state, so `returned`
    /// is zero for a ranged strike.
    pub(crate) fn engine_exchange(
        &self,
        g: &Game,
        uid: u32,
        did: u32,
        pos: Pos,
        ranged: bool,
    ) -> Option<(f64, f64)> {
        if !self.exchange_is_the_engines {
            return None;
        }
        if ranged {
            let (att, def) = g.ranged_strike_strengths(uid, did, pos)?;
            Some((expected_damage(att, def), 0.0))
        } else {
            let (att, def) = g.melee_exchange_strengths(uid, did)?;
            Some((expected_damage(att, def), expected_damage(def, att)))
        }
    }

    /// The defender's strength as the engine would read it **on the tile it
    /// is being asked about**, rather than on the tile it stands on: the
    /// unit moved there, plus that tile's own defence. Returned *before*
    /// `effective_strength`, because each caller folds its own matchup term
    /// in first and then applies the wound penalty — so with the gene off
    /// (`None`) the caller's arithmetic is unchanged to the bit.
    ///
    /// This is `incoming_damage`'s reading, which has always been right on
    /// the evacuation path and has never been the movers'.
    pub(crate) fn defence_base_where_it_would_stand(
        &self,
        g: &Game,
        uid: u32,
        tile: Pos,
    ) -> Option<f64> {
        if !self.defend_where_you_stand {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let mut standing = unit.clone();
        standing.pos = tile;
        Some(g.unit_strength(&standing, true) + g.tile_defense_bonus(tile))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctrine::{build, position};
    use crate::hex;

    fn open_field() -> Game {
        let mut g = build(position("the_reserve").expect("known"), 3).expect("buildable");
        let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
        for uid in seeded {
            g.remove_unit(uid);
        }
        g
    }

    fn at(col: i32, row: i32) -> Pos {
        hex::offset_to_axial(col, row)
    }

    #[test]
    fn the_genes_ship_off_and_are_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.base.exchange_is_the_engines, "an opt-in ships off");
        assert!(!ai.base.defend_where_you_stand, "an opt-in ships off");
        for field in ["exchange_is_the_engines", "defend_where_you_stand"] {
            assert!(
                super::super::GENES
                    .iter()
                    .any(|gene| gene.opt_in() && gene.field == field),
                "{field} is an opt-in gene"
            );
        }
        let mut on = AdvancedAi::new();
        on.enable_exchange_is_the_engines();
        on.enable_defend_where_you_stand();
        assert!(on.base.exchange_is_the_engines && on.base.defend_where_you_stand);
        on.disable_exchange_is_the_engines();
        on.disable_defend_where_you_stand();
        assert!(!on.base.exchange_is_the_engines && !on.base.defend_where_you_stand);
    }

    /// The anti-cavalry matchup is the clearest case the strength-only
    /// estimate cannot see: a spearman defending against a horseman is
    /// stronger than its own combat strength says, and the engine knows it.
    /// With the gene on, the exchange the controller prices is the exchange
    /// the engine resolves.
    #[test]
    fn the_exchange_is_the_one_the_engine_will_resolve() {
        let mut g = open_field();
        let horse = g.spawn_unit("horseman", 0, at(9, 6));
        let spear = g.spawn_unit("spearman", 1, at(10, 6));
        let mut ai = BasicAi::new();
        assert!(
            ai.engine_exchange(&g, horse, spear, at(10, 6), false)
                .is_none(),
            "off, the helper is silent"
        );
        ai.exchange_is_the_engines = true;
        let (dealt, returned) = ai
            .engine_exchange(&g, horse, spear, at(10, 6), false)
            .expect("the engine prices this pair");
        // The engine's own numbers, from the same call `do_attack` makes.
        let (att, def) = g.melee_exchange_strengths(horse, spear).expect("pair");
        assert!((dealt - expected_damage(att, def)).abs() < 1e-9);
        assert!((returned - expected_damage(def, att)).abs() < 1e-9);
        // And the anti-cavalry bonus is in it: the strength-only estimate
        // reads the spearman weaker than the engine does.
        let naive_def = effective_strength(g.unit_strength(&g.units[&spear], true), 100);
        assert!(
            def > naive_def,
            "the matchup the bare strength misses: {def} v {naive_def}"
        );
        // A shot returns nothing.
        let archer = g.spawn_unit("archer", 0, at(8, 6));
        let (shot, back) = ai
            .engine_exchange(&g, archer, spear, at(10, 6), true)
            .expect("the engine prices this shot");
        assert!(shot > 0.0);
        assert_eq!(back, 0.0, "a shot has no second blow");
    }

    /// The defender is priced on the tile it is being asked about. A hill
    /// is worth something before the unit stands on it, not after.
    #[test]
    fn the_defender_is_priced_where_it_would_stand() {
        let mut g = open_field();
        let flat = at(9, 6);
        let hill = at(10, 6);
        g.map.tiles.get_mut(&hill).expect("tile").hills = true;
        let warrior = g.spawn_unit("warrior", 0, flat);
        let mut ai = BasicAi::new();
        assert!(
            ai.defence_base_where_it_would_stand(&g, warrior, hill)
                .is_none(),
            "off, the helper is silent"
        );
        ai.defend_where_you_stand = true;
        let on_flat = ai
            .defence_base_where_it_would_stand(&g, warrior, flat)
            .expect("priced");
        let on_hill = ai
            .defence_base_where_it_would_stand(&g, warrior, hill)
            .expect("priced");
        assert!(
            on_hill > on_flat,
            "the hill is worth something: {on_hill} v {on_flat}"
        );
        assert!(
            (on_hill - on_flat - g.tile_defense_bonus(hill)).abs() < 1e-9,
            "and it is worth exactly the engine's own tile defence"
        );
        // The unit itself has not moved.
        assert_eq!(g.units[&warrior].pos, flat);
    }

    /// The whole point of the second gene, at the level the mover sees it:
    /// the projected counter-damage of standing on a hill is lower than of
    /// standing beside it, and with the gene off the two are equal.
    #[test]
    fn the_threat_term_prefers_the_hill_only_with_the_gene() {
        let mut g = open_field();
        let flat = at(9, 6);
        let hill = at(9, 7);
        g.map.tiles.get_mut(&hill).expect("tile").hills = true;
        let warrior = g.spawn_unit("warrior", 0, at(8, 6));
        let enemy = g.spawn_unit("warrior", 1, at(11, 6));
        let hostiles = vec![enemy];
        let mut ai = BasicAi::new();
        let (off_flat, off_hill) = (
            ai.projected_counter_damage(&g, warrior, flat, &hostiles),
            ai.projected_counter_damage(&g, warrior, hill, &hostiles),
        );
        assert!(
            (off_flat - off_hill).abs() < 1e-9,
            "off, the ground is invisible: {off_flat} v {off_hill}"
        );
        ai.defend_where_you_stand = true;
        let (on_flat, on_hill) = (
            ai.projected_counter_damage(&g, warrior, flat, &hostiles),
            ai.projected_counter_damage(&g, warrior, hill, &hostiles),
        );
        assert!(on_flat > 0.0 && on_hill > 0.0, "both tiles are reachable");
        assert!(
            on_hill < on_flat,
            "on, the hill costs the enemy: {on_hill} v {on_flat}"
        );
    }
}
