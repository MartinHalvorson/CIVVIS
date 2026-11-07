//! A Culture seat can fill empty slots from a rival's public deal inventory.
//! Native Pericles 20261106T222235Z finished its first Amphitheater at turn 78
//! but still had no tourism at 119. At 129 its Writer points were 364 against
//! the next Writer's 620-point price. Buying an existing work does not wait
//! for that race. The host supplies the actual tradeable work and price;
//! this policy owns the objective, timing, counterparty and spending ceiling.

use super::*;

impl AdvancedAi {
    /// A native Great Work purchase: counterpart's simulator seat, maximum
    /// Gold-equivalent price, maximum lump Gold, and maximum GPT. `income`
    /// and `cultural_activation_pending` are facts exported by the host. `remaining_gold` reserves other planned purchases;
    /// war and slot facts stay on the observed board.
    pub fn culture_work_purchase_ask(
        &self,
        g: &Game,
        pid: usize,
        remaining_gold: f64,
        income: Option<f64>,
        cultural_activation_pending: bool,
    ) -> Option<(usize, i32, i32, i32)> {
        if self.lane_lost
            || !g.victory_conditions.culture
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || g.turn % 6 != 3
            || cultural_activation_pending
            || self
                .plan
                .as_ref()
                .is_some_and(|plan| plan.threatened_city.is_some())
            || g.cities.values().any(|city| {
                city.owner == pid
                    && city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= 4
            })
        {
            return None;
        }
        let player = g.players.get(pid)?;
        let income = income.filter(|value| value.is_finite())?;
        if !player.gold.is_finite() || !remaining_gold.is_finite() || income <= 5.0 {
            return None;
        }
        if !["writing", "art", "music", "artifact", "relic"]
            .into_iter()
            .any(|kind| g.can_house_additional_great_work(pid, kind))
        {
            return None;
        }
        // Native 012706Z t165 quoted 105 Gold + 12 GPT (405 equivalent)
        // against our 227 ceiling, despite both payment components fitting
        // their reserves. Bound the components independently, then also cap
        // the combined price. The host must enforce all three limits.
        let max_gold =
            (player.gold.min(remaining_gold) - 60.0 - 5.0 * g.player_city_ids(pid).len() as f64)
                .clamp(0.0, 500.0)
                .floor() as i32;
        // Preserve the existing 5 GPT floor and 20% income headroom.
        let max_gpt = ((income - 5.0) * 0.8).clamp(0.0, 20.0).floor() as i32;
        let ceiling = (max_gold + 25 * max_gpt).min(500);
        if ceiling < 80 {
            return None;
        }
        let majors: Vec<_> = g
            .players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !g.same_team(pid, other.id)
            })
            .collect();
        let sellers: Vec<_> = majors
            .into_iter()
            .filter(|other| g.has_met(pid, other.id) && !g.is_at_war(pid, other.id))
            .collect();
        let index = (g.turn / 6) as usize % sellers.len().max(1);
        Some((sellers.get(index)?.id, ceiling, max_gold, max_gpt))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::industrial_zones::tests::producer_empire;

    fn fixture() -> (AdvancedAi, Game) {
        let (mut game, cities) = producer_empire("CULTUREWORKBUY", 472_301, 3);
        game.turn = 63;
        game.players[0].gold = 1_000.0;
        game.players[0].met.extend([1, 2]);
        game.cities
            .get_mut(&cities[0])
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
        (AdvancedAi::targeting(VictoryTarget::Culture), game)
    }

    #[test]
    fn spare_slots_and_money_can_buy_from_each_met_major() {
        let (ai, mut game) = fixture();
        let (first, ceiling, _, _) = ai
            .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), false)
            .unwrap();
        assert_eq!(ceiling, 500);
        game.turn += 6;
        let (second, _, _, _) = ai
            .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), false)
            .unwrap();
        assert_ne!(first, second, "a refusal must not monopolize every ask");
    }

    #[test]
    fn keeps_cash_and_income_reserves_and_does_not_buy_on_unknown_income() {
        let (ai, mut game) = fixture();
        game.players[0].gold = 200.0; // six cities: reserve 90
        let (_, total, cash, gpt) = ai
            .culture_work_purchase_ask(&game, 0, 200.0, Some(30.0), false)
            .unwrap();
        assert_eq!((total, cash, gpt), (500, 110, 20));
        let (_, total, cash, gpt) = ai
            .culture_work_purchase_ask(&game, 0, 200.0, Some(10.0), false)
            .unwrap();
        assert_eq!((total, cash, gpt), (210, 110, 4));
        assert!(ai
            .culture_work_purchase_ask(&game, 0, game.players[0].gold, None, false)
            .is_none());
        assert!(ai
            .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(f64::NAN), false)
            .is_none());
        game.players[0].gold = 50.0;
        let (_, total, cash, gpt) = ai
            .culture_work_purchase_ask(&game, 0, 50.0, Some(10.0), false)
            .unwrap();
        assert_eq!(
            (total, cash, gpt),
            (100, 0, 4),
            "only a GPT-only quote may use this budget"
        );
        assert!(ai
            .culture_work_purchase_ask(&game, 0, 50.0, Some(6.0), false)
            .is_none());
    }

    #[test]
    fn other_objectives_people_and_off_cadence_keep_their_money() {
        let (ai, mut game) = fixture();
        assert!(ai
            .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), true)
            .is_none());
        let science = AdvancedAi::targeting(VictoryTarget::Science);
        assert!(science
            .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), false)
            .is_none());
        game.turn += 1;
        assert!(ai
            .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(30.0), false)
            .is_none());
    }

    #[test]
    fn existing_spending_war_counterparties_and_threatened_cities_preempt_the_purchase() {
        let (ai, mut game) = fixture();
        let (_, _, cash, _) = ai
            .culture_work_purchase_ask(&game, 0, 150.0, Some(30.0), false)
            .unwrap();
        assert_eq!(
            cash, 60,
            "cash committed elsewhere stays unavailable even to a financed quote"
        );
        game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
        assert_eq!(
            ai.culture_work_purchase_ask(&game, 0, 1_000.0, Some(30.0), false)
                .unwrap()
                .0,
            2,
            "a remote war does not forbid trade with the other rival"
        );
        let cid = game.player_city_ids(0)[0];
        game.cities.get_mut(&cid).unwrap().last_attacked = game.turn;
        assert!(ai
            .culture_work_purchase_ask(&game, 0, 1_000.0, Some(30.0), false)
            .is_none());
    }

    #[test]
    fn native_financed_quotes_fit_without_spending_the_cash_reserve() {
        let (ai, mut game) = fixture();
        let reserve = 60.0 + 5.0 * game.player_city_ids(0).len() as f64;
        for (cash_room, income, quoted_gold, quoted_gpt) in
            [(215.0, 48.0039, 118, 14), (227.0, 42.1328, 105, 12)]
        {
            game.players[0].gold = reserve + cash_room;
            let (_, total, cash, gpt) = ai
                .culture_work_purchase_ask(&game, 0, game.players[0].gold, Some(income), false)
                .unwrap();
            assert!(quoted_gold + 25 * quoted_gpt <= total);
            assert!(quoted_gold <= cash && quoted_gpt <= gpt);
            assert!(game.players[0].gold - quoted_gold as f64 >= reserve);
            assert!(income - quoted_gpt as f64 >= 5.0);
        }
    }
}
