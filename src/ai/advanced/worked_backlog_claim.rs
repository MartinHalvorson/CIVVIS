//! `builders-cover-the-worked-backlog`: the backlog's Builder claims one idle
//! queue a turn ahead of the controller's routine idle-queue claims, or is
//! bought with Gold when no idle city can take it.
//!
//! **Why (live G383, civvis-20261008T102717Z, pin cbc51e678).** The gene's
//! step in `BasicAi::pick_item` fired three times by turn 100 while the
//! unimproved worked tiles grew from 11 at t63 to 25 at t97 against 0-8
//! Builder charges. Every turn from t58 to t100 the empire-level gate wanted a
//! Builder; what stopped it was the idle queue never reaching `pick_item`.
//! Of the ~20 idle queues from t63 to t97, four went to a due Settler (by
//! design) and most of the rest to the controller's own claims, which run
//! before the delegated governor: a Scout and a Skirmisher for the land
//! frontier (t70, t94), the Entertainment Complex (t86), the Government Plaza's
//! Grand Master's Chapel (t88, t96), the development shortfall's Campus (t82),
//! the religious defence's Shrine (t69) and the siege's Catapults (t77-t84).
//! Barinas, Cartagena and Maracaibo at 3-4 Production also needed 7-10 Online
//! turns for a Builder against the step's six.
//!
//! So this pass runs after the defence, religion, settlement and siege claims
//! and before the Plaza, the development shortfall, the Entertainment Complex
//! and the recon claims, and takes the idle city that trains the Builder
//! soonest. Off, nothing runs.

use super::*;

impl AdvancedAi {
    /// See the module: one idle, unthreatened queue for the worked backlog's
    /// Builder, else a Gold purchase while the backlog runs away.
    pub(super) fn claim_idle_queue_for_worked_backlog(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.base.builders_cover_the_worked_backlog {
            return;
        }
        let counts = self.counts(g, pid);
        let city_ids = g.player_city_ids(pid);
        let n_cities = city_ids.len();
        let mut best: Option<(f64, u32)> = None;
        for &city in &city_ids {
            if !g.cities[&city].queue.is_empty() || plan.threatened_city == Some(city) {
                continue;
            }
            let Some(turns) = self.base.worked_backlog_claim_turns(
                g,
                pid,
                city,
                n_cities,
                counts.settlers,
                counts.builders,
                counts.military,
            ) else {
                continue;
            };
            if best.is_none_or(|(old, old_city)| {
                turns + 1e-9 < old || ((turns - old).abs() <= 1e-9 && city < old_city)
            }) {
                best = Some((turns, city));
            }
        }
        let Some((turns, city)) = best else {
            self.base
                .buy_worked_backlog_builder(g, pid, &city_ids, counts.builders);
            return;
        };
        let item = Item::Unit {
            unit: crate::name!("builder"),
        };
        let city_name = g.cities[&city].name.clone();
        if g.apply(pid, &Action::Produce { city, item }).is_ok() {
            think!(self.journal(), Economy, Decision,
                   "{city_name} starts a Builder for the worked backlog";
                   "the idle queue that trains one soonest ({turns:.1} turns), ahead of the \
                    routine idle-queue claims");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A founded capital and a second city four or more tiles away, both
    /// idle, with Mining, every owned plot a bare grassland hill, a Settler
    /// already walking, and no Gold unless a test grants it.
    fn two_idle_cities() -> (Game, u32, u32) {
        let mut game = Game::new_full(1, 24, 16, 936_221, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("the player opens with a settler");
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        for uid in game.player_unit_ids(0) {
            game.remove_unit(uid);
        }
        let capital = game.player_city_ids(0)[0];
        let home = game.cities[&capital].pos;
        let sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
        let mut second = None;
        for site in sites {
            if game.wdist(site, home) < 4 {
                continue;
            }
            let settler = game.spawn_unit("settler", 0, site);
            if game.apply(0, &Action::FoundCity { unit: settler }).is_ok() {
                second = game
                    .player_city_ids(0)
                    .into_iter()
                    .find(|cid| game.cities[cid].pos == site);
                break;
            }
            game.remove_unit(settler);
        }
        let second = second.expect("a legal second site");
        // A Settler already walking, so no city is due another one.
        game.spawn_unit("settler", 0, home);
        game.players[0].techs.insert(crate::name!("mining"));
        game.players[0].gold = 0.0;
        game.players[0].gold_per_turn = 5.0;
        for cid in [capital, second] {
            let center = game.cities[&cid].pos;
            let owned: Vec<Pos> = game.cities[&cid].owned_tiles.to_vec();
            for position in owned {
                if position == center || game.map.tiles[&position].district.is_some() {
                    continue;
                }
                let tile = game.map.tiles.get_mut(&position).unwrap();
                tile.terrain = crate::name!("grassland");
                tile.hills = true;
                tile.feature = None;
                tile.resource = None;
                tile.improvement = None;
            }
            let city = game.cities.get_mut(&cid).unwrap();
            city.pop = 4;
            city.queue.clear();
        }
        (game, capital, second)
    }

    fn armed() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_builders_cover_the_worked_backlog();
        ai
    }

    fn plan(ai: &AdvancedAi, game: &Game) -> StrategicPlan {
        ai.assess(game, 0)
    }

    fn builders_queued(game: &Game) -> usize {
        game.player_city_ids(0)
            .into_iter()
            .filter(|cid| {
                matches!(
                    game.cities[cid].queue.first(),
                    Some(Item::Unit { unit }) if *unit == "builder"
                )
            })
            .count()
    }

    fn builders_alive(game: &Game) -> usize {
        game.player_unit_ids(0)
            .into_iter()
            .filter(|uid| game.units[uid].kind == "builder")
            .count()
    }

    /// The blocked case: an idle city a routine claim would otherwise take
    /// gets the backlog's Builder first, one a turn; off, nothing moves.
    #[test]
    fn an_idle_queue_trains_the_backlog_builder_before_routine_claims() {
        let (game, _capital, _second) = two_idle_cities();
        let backlog = BasicAi::unimproved_worked_tiles(&game, 0);
        assert!(backlog >= 4, "fixture backlog {backlog}");
        let mut off = game.clone();
        let stock = AdvancedAi::new();
        stock.claim_idle_queue_for_worked_backlog(&mut off, 0, &plan(&stock, &game));
        assert_eq!(builders_queued(&off), 0, "off: no claim");
        let mut on = game.clone();
        let ai = armed();
        ai.claim_idle_queue_for_worked_backlog(&mut on, 0, &plan(&ai, &game));
        assert_eq!(builders_queued(&on), 1, "one idle queue takes the Builder");
        // A city under siege or a due Settler keeps its queue: with every
        // idle city threatened, nothing is claimed.
        let mut threatened = game.clone();
        for cid in threatened.player_city_ids(0) {
            threatened.cities.get_mut(&cid).unwrap().last_attacked = threatened.turn.max(1);
        }
        let p = plan(&ai, &threatened);
        ai.claim_idle_queue_for_worked_backlog(&mut threatened, 0, &p);
        assert_eq!(builders_queued(&threatened), 0, "threatened cities keep their queues");
    }

    /// The slow-city half of the blocked case: a Builder that takes longer
    /// than `WORKED_BACKLOG_BUILDER_MAX_TURNS` is still trained while the
    /// backlog runs away from the charges, and not once the charges cover
    /// half of it.
    #[test]
    fn a_slow_city_trains_the_builder_while_the_backlog_runs_away() {
        let (mut game, capital, second) = two_idle_cities();
        // Six citizens a city, so the 50%-60% window holds a whole charge.
        for cid in [capital, second] {
            game.cities.get_mut(&cid).unwrap().pop = 6;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        let cost = game.item_cost_for_city(0, capital, &builder);
        let base = game.city_yields(capital).production;
        // Production for a Builder in twelve standard turns.
        let wanted = cost / game.standard_duration(12) as f64;
        std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            capital,
            crate::rules::Yields {
                production: wanted - base,
                ..Default::default()
            },
        );
        let turns = BasicAi::unit_build_turns(&game, 0, capital, "builder");
        assert!(
            turns > game.standard_duration(8) as f64 && turns <= game.standard_duration(15) as f64,
            "fixture: a slow Builder ({turns:.1} turns)"
        );
        let ai = armed();
        let n = game.player_city_ids(0).len();
        assert!(
            ai.base
                .worked_backlog_claim_turns(&game, 0, capital, n, 1, 0, 2)
                .is_some(),
            "no charges against the backlog: the slow city trains it"
        );
        let backlog = BasicAi::unimproved_worked_tiles(&game, 0) as i32;
        let mut covered = game.clone();
        let center = covered.cities[&capital].pos;
        let uid = covered.spawn_unit("builder", 0, center);
        // Over half the backlog yet under the 60% cover: not running away.
        covered.units.get_mut(&uid).unwrap().charges = (f64::from(backlog) * 0.6).ceil() as i32 - 1;
        let cover = BasicAi::builder_charge_cover(&covered, 0);
        assert!(
            f64::from(cover) < backlog as f64 * 0.6 && f64::from(cover) * 2.0 > backlog as f64,
            "fixture: cover {cover} against backlog {backlog}"
        );
        assert_eq!(
            ai.base
                .worked_backlog_claim_turns(&covered, 0, capital, n, 1, 1, 2),
            None,
            "a backlog that is not running away keeps the eight-turn cap"
        );
    }

    /// With no idle queue, a runaway backlog buys its Builder while the
    /// treasury keeps `spend_gold`'s reserve, and not below it.
    #[test]
    fn a_runaway_backlog_buys_its_builder_above_the_reserve() {
        let (mut game, capital, second) = two_idle_cities();
        for cid in [capital, second] {
            game.cities.get_mut(&cid).unwrap().queue.push(Item::Building {
                building: crate::name!("monument"),
            });
        }
        let ai = armed();
        let price = [capital, second]
            .iter()
            .filter_map(|cid| game.unit_purchase_cost(0, *cid, "builder", "gold"))
            .fold(f64::INFINITY, f64::min);
        assert!(price.is_finite(), "fixture: a Builder can be bought");
        let reserve = 100.0 + 25.0 * 2.0;
        let mut poor = game.clone();
        poor.players[0].gold = price + reserve - 1.0;
        let p = plan(&ai, &poor);
        ai.claim_idle_queue_for_worked_backlog(&mut poor, 0, &p);
        assert_eq!(builders_alive(&poor), 0, "below the reserve: no purchase");
        let mut rich = game.clone();
        rich.players[0].gold = price + reserve + 1.0;
        let p = plan(&ai, &rich);
        ai.claim_idle_queue_for_worked_backlog(&mut rich, 0, &p);
        assert_eq!(builders_alive(&rich), 1, "above the reserve: one Builder bought");
        let mut off = game.clone();
        off.players[0].gold = price + reserve + 1.0;
        let stock = AdvancedAi::new();
        let p = plan(&stock, &off);
        stock.claim_idle_queue_for_worked_backlog(&mut off, 0, &p);
        assert_eq!(builders_alive(&off), 0, "off: no purchase");
    }
}
