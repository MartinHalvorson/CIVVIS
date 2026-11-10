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

use super::*;

impl AdvancedAi {
    /// Whether the culture claims pass the deterrent this turn: the gene is
    /// on, the seat aims at Culture and no major is at war with us.
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

    /// Our capital and a second city, a met neighbour within the deterrence
    /// radius with twice our power, inside the window; the capital's queue
    /// holds a warrior. Returns the game and our capital.
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
        let theirs = *sites.first()?;
        let second = *sites
            .iter()
            .rev()
            .find(|pos| game.wdist(**pos, theirs) >= 4)?;
        game.place_city(1, theirs, None);
        game.place_city(0, second, None);
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        game.players[0].techs.insert(crate::name!("bronze_working"));
        let observed = std::sync::Arc::make_mut(&mut game.observed_military_power);
        observed.insert(0, 100.0);
        observed.insert(1, 200.0);
        game.turn = game.standard_duration(DETERRENCE_START_STANDARD) + 5;
        // A Monument one turn of work from done, so the claim's build-time
        // bound holds whatever the bare capital makes.
        let monument = Item::Building {
            building: crate::name!("monument"),
        };
        let nearly = game.item_cost_for_city(0, ours, &monument) - 0.5;
        let city = game.cities.get_mut(&ours).unwrap();
        city.pop = 4;
        city.queue.clear();
        city.queue.push(Item::Unit {
            unit: crate::name!("warrior"),
        });
        city.production_progress
            .insert(Game::item_progress_key(&monument), nearly);
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

    /// The live case: the Monument claim yields to the deterrent's warrior
    /// with the gene off, and takes the queue with it on; the deterrent then
    /// leaves the claimed Monument alone.
    #[test]
    fn the_monument_claim_passes_the_deterrent_and_keeps_its_queue() {
        let Some((game, ours)) = out_gunned_culture_board() else {
            panic!("fixture: two cities of ours beside a neighbour");
        };
        let monument = Item::Building {
            building: crate::name!("monument"),
        };

        let off = culture_seat(false);
        assert!(
            off.deterrence_threat(&game, 0).is_some(),
            "fixture: the neighbour out-guns us"
        );
        let mut held = game.clone();
        let plan = off.assess(&held, 0);
        off.claim_culture_monuments(&mut held, 0, &plan);
        assert_ne!(
            held.cities[&ours].queue.first(),
            Some(&monument),
            "off: the warrior keeps the capital"
        );

        let on = culture_seat(true);
        let mut passed = game.clone();
        let plan = on.assess(&passed, 0);
        assert!(
            on.culture_monument_item(&passed, 0, ours, &plan).is_some(),
            "fixture: the capital's Monument is claimable"
        );
        assert!(
            on.deterrence_unit_holds(
                &passed,
                0,
                &Item::Unit {
                    unit: crate::name!("warrior"),
                }
            ),
            "fixture: the warrior is the deterrent's"
        );
        assert!(on.culture_builds_past_the_deterrent(&passed, 0));
        on.claim_culture_monuments(&mut passed, 0, &plan);
        assert_eq!(
            passed.cities[&ours].queue.first(),
            Some(&monument),
            "on: the Monument goes ahead of the warrior"
        );
        assert!(on.deterrent_spares_the_culture_build(&passed, 0, ours, &plan));
        let before = passed.cities[&ours].queue.first().cloned();
        on.claim_deterrence_unit(&mut passed, 0, &plan);
        assert_eq!(
            passed.cities[&ours].queue.first().cloned(),
            before,
            "on: the deterrent leaves the claimed Monument alone"
        );
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
