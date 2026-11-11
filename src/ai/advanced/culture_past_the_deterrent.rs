//! `culture-seat-builds-past-the-deterrent`: at peace, a Culture seat's
//! culture claims (its first Theater Squares, its Amphitheaters and first Art
//! Museum, its Monuments, the Faith sources opened at two Theaters) go ahead of a `near-rival-deterrence` unit at the
//! head of a queue, and the deterrent no longer takes a queue back from them.
//!
//! **Why (live Deity Online Kristina, 2026-11-10, the first three Sweden
//! culture seats).** Pericles on the same lane and genes stood at a median 28
//! domestic tourists at t100; Kristina at 13, 13 and 17. Culture a turn was
//! 8 at t40, 13 at t70 and 42 at t100 in civvis-20261110T140731Z, a game at
//! peace until t139. Its `why.log` shows the deterrent holding Stockholm from
//! t39 to t69 (warriors, archers, horsemen against Vietnam's 1.2-1.6 times
//! our power: never level), and then taking the empire's first Amphitheater
//! back eleven times between t69 and t92 ("trains a deterrent ... amphitheater
//! keeps its progress"): the deterrent runs before the culture claims, may
//! displace any building, and the culture claims then yield to the unit it
//! queued. The first Amphitheater stood at t92 and the first Art Museum at
//! t115; Göteborg's Monument, claimed at t25, lost its queue to an archer at
//! t30 and was finished at t80.
//!
//! `acropolis-before-the-deterrent` lets only a first Theater Square of four
//! Online turns or less pass, which is Greece's Acropolis (27 production) in
//! most cities and an ordinary Theater Square (48-55) only in the capital.
//! The Amphitheater and Monument claims had no pass at all.
//!
//! **What the gene does.** While no major is at war with us, on a Culture
//! seat: the Theater, Amphitheater, Art Museum, Monument and Faith source
//! claims treat a deterrent unit at the head as deferrable (it keeps its
//! progress and returns behind the culture build), and the deterrent passes
//! over a city whose head is an item those claims hold. Every other hold (the early
//! Settler floor, granaries, the backlog Builder, walls, the faith defence,
//! anything finishing this turn) is unchanged, and the deterrent still trains
//! in the cities that are not building culture. Off, or for another victory
//! target, nothing changes.
//!
//! **`culture-pass-keeps-half-the-army`** (2026-11-10, the bound on the
//! above). The pass holds only while our military power is at least
//! [`CULTURE_PASS_ARMY_FLOOR`] of the near neighbour the deterrent answers
//! (`deterrence_threat`); below it the deterrent keeps its old priority.
//! Between half and parity the culture builds still go first, which covers the
//! case this gene was written for: 140731Z stood at 0.6-0.8 of Vietnam's power.
//!
//! Why: 37 live Deity Online culture games of 11-10 (10 before this gene was
//! armed at 17:06Z, 27 after). A game collapsed (eliminated by t110, or two
//! cities or fewer at t100) in 0 of 18 where every rival city stood more than
//! ten tiles from our capital at t40, and in 9 of 19 where one stood within
//! ten: those are the only games the deterrent acts in. Among them, 1 of 5
//! collapsed before the gene and 8 of 13 after. In the same near games the
//! deterrent's claims fell from 14 to 6 a game (t30-t70), our military at
//! t60 from a median 238 to 141, and our land soldiers from 9 to 6. Every
//! loss came from a major's surprise war, and at peace with a major holding a
//! city within ten tiles of ours the declaration rate per 100 rival-turns was
//! 1.46 below 0.45 of its power (18 of 1,234), 0.62 at 0.45-0.7 (3 of 480)
//! and 0.14 at 0.7 or more (1 of 720). The capital was the first city lost in
//! 14 of the 19 wars that cost one, a median 6 turns after the declaration,
//! while the host signs no peace before 8 Online turns. Other culture genes
//! were armed in the same window, so the before/after contrast is not this
//! gene's alone.

use super::*;

/// `culture-pass-keeps-half-the-army`: our power over the near neighbour's
/// below which the culture claims no longer pass the deterrent.
pub(super) const CULTURE_PASS_ARMY_FLOOR: f64 = 0.5;

impl AdvancedAi {
    /// Whether the culture claims pass the deterrent this turn: the gene is
    /// on, the seat aims at Culture, no major is at war with us, and (under
    /// `culture-pass-keeps-half-the-army`) we field at least half the near
    /// neighbour's power.
    pub(super) fn culture_builds_past_the_deterrent(&self, g: &Game, pid: usize) -> bool {
        self.culture_seat_builds_past_the_deterrent
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
            && !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_barbarian
                    && !other.is_minor
                    && g.is_at_war(pid, other.id)
            })
            && !self.culture_pass_under_the_army_floor(g, pid)
    }

    /// `culture-pass-keeps-half-the-army`: a near neighbour the deterrent
    /// answers fields more than twice our power. False with the gene off.
    pub(super) fn culture_pass_under_the_army_floor(&self, g: &Game, pid: usize) -> bool {
        self.culture_pass_keeps_half_the_army
            && self
                .deterrence_threat(g, pid)
                .is_some_and(|(_, theirs)| g.military_power(pid) < CULTURE_PASS_ARMY_FLOOR * theirs)
    }

    /// Whether the deterrent unit `item` at a queue head still holds it
    /// against a culture claim. False under this gene at peace.
    pub(super) fn deterrent_holds_against_culture(
        &self,
        g: &Game,
        pid: usize,
        item: &Item,
    ) -> bool {
        self.deterrence_unit_holds(g, pid, item) && !self.culture_builds_past_the_deterrent(g, pid)
    }

    /// Whether the deterrent leaves `cid` alone because its head is a
    /// culture claim's item. False with the gene off.
    pub(super) fn deterrent_spares_the_culture_build(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.culture_builds_past_the_deterrent(g, pid) {
            return false;
        }
        let Some(head) = g.cities.get(&cid).and_then(|city| city.queue.first()) else {
            return false;
        };
        self.culture_theater_holds(g, pid, cid, plan, head)
            || self.culture_monument_holds(g, pid, cid, plan, head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> Game {
        Game::new_full(2, 30, 20, 79_208, 250, 0, false)
    }

    #[test]
    fn off_or_another_target_never_passes() {
        let g = game();
        let ai = AdvancedAi::targeting(VictoryTarget::Culture);
        assert!(!ai.culture_builds_past_the_deterrent(&g, 0));
        let mut science = AdvancedAi::targeting(VictoryTarget::Science);
        science.enable_culture_seat_builds_past_the_deterrent();
        assert!(!science.culture_builds_past_the_deterrent(&g, 0));
    }

    /// Our capital, a met neighbour within the deterrence radius with twice
    /// our power, inside the window; the capital's queue holds a warrior.
    /// Returns the game and our capital.
    fn out_gunned_culture_board() -> Option<(Game, u32)> {
        use super::super::near_rival_deterrence::DETERRENCE_START_STANDARD;
        let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")?;
        game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
        for pid in [0, 1] {
            for uid in game.player_unit_ids(pid) {
                game.remove_unit(uid);
            }
        }
        let ours = game.player_city_ids(0)[0];
        let home = game.cities[&ours].pos;
        let mut sites: Vec<Pos> = game
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                (5..=8).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
            })
            .map(|(pos, _)| *pos)
            .collect();
        sites.sort();
        game.place_city(1, *sites.first()?, None);
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        game.players[0].techs.insert(crate::name!("bronze_working"));
        let observed = std::sync::Arc::make_mut(&mut game.observed_military_power);
        observed.insert(0, 100.0);
        observed.insert(1, 200.0);
        game.turn = game.standard_duration(DETERRENCE_START_STANDARD) + 5;
        let city = game.cities.get_mut(&ours).unwrap();
        city.pop = 4;
        city.queue.clear();
        city.queue.push(Item::Unit {
            unit: crate::name!("warrior"),
        });
        Some((game, ours))
    }

    fn culture_seat(on: bool) -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_near_rival_deterrence();
        ai.enable_culture_seat_monuments();
        if on {
            ai.enable_culture_seat_builds_past_the_deterrent();
        }
        ai
    }

    /// The live case: the deterrent's warrior holds the culture claims with
    /// the gene off and not with it on; at war the hold returns. The claims'
    /// own end-to-end change on captured live boards is the fires artifact
    /// (`docs/gene_screens/fires/culture-seat-builds-past-the-deterrent.action-pairs.json`).
    #[test]
    fn the_deterrents_unit_holds_the_culture_claims_only_with_the_gene_off() {
        let Some((game, ours)) = out_gunned_culture_board() else {
            panic!("fixture: our capital beside a neighbour");
        };
        let warrior = Item::Unit {
            unit: crate::name!("warrior"),
        };
        let off = culture_seat(false);
        assert!(
            off.deterrence_unit_holds(&game, 0, &warrior),
            "fixture: the neighbour out-guns us and the warrior is the deterrent's"
        );
        assert!(off.deterrent_holds_against_culture(&game, 0, &warrior));
        let plan = off.assess(&game, 0);
        assert!(!off.deterrent_spares_the_culture_build(&game, 0, ours, &plan));

        let on = culture_seat(true);
        assert!(on.deterrence_unit_holds(&game, 0, &warrior));
        assert!(!on.deterrent_holds_against_culture(&game, 0, &warrior));

        let mut war = game.clone();
        war.at_war.insert((0, 1));
        assert!(!on.culture_builds_past_the_deterrent(&war, 0));
    }

    /// `culture-pass-keeps-half-the-army`: at a third of the neighbour's
    /// power the deterrent's warrior holds the culture claims again; at half
    /// or more the pass stands. Off, the pass stands at any power.
    #[test]
    fn the_pass_keeps_half_the_army() {
        let Some((mut game, ours)) = out_gunned_culture_board() else {
            panic!("fixture: our capital beside a neighbour");
        };
        let warrior = Item::Unit {
            unit: crate::name!("warrior"),
        };
        std::sync::Arc::make_mut(&mut game.observed_military_power).insert(1, 300.0);
        let pass = culture_seat(true);
        assert!(pass.culture_builds_past_the_deterrent(&game, 0));
        assert!(!pass.culture_pass_under_the_army_floor(&game, 0));

        let mut floor = culture_seat(true);
        floor.enable_culture_pass_keeps_half_the_army();
        assert!(floor.deterrence_unit_holds(&game, 0, &warrior));
        assert!(floor.culture_pass_under_the_army_floor(&game, 0));
        assert!(!floor.culture_builds_past_the_deterrent(&game, 0));
        assert!(floor.deterrent_holds_against_culture(&game, 0, &warrior));
        let plan = floor.assess(&game, 0);
        assert!(!floor.deterrent_spares_the_culture_build(&game, 0, ours, &plan));

        std::sync::Arc::make_mut(&mut game.observed_military_power).insert(1, 200.0);
        assert!(!floor.culture_pass_under_the_army_floor(&game, 0));
        assert!(floor.culture_builds_past_the_deterrent(&game, 0));
        assert!(!floor.deterrent_holds_against_culture(&game, 0, &warrior));
    }

    #[test]
    fn a_culture_seat_at_peace_passes_and_at_war_does_not() {
        let mut g = game();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_culture_seat_builds_past_the_deterrent();
        assert!(ai.culture_builds_past_the_deterrent(&g, 0));
        g.at_war.insert((0, 1));
        assert!(!ai.culture_builds_past_the_deterrent(&g, 0));
    }
}
