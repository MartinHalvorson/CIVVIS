//! `denial-spies-fill-every-slot` (opt-in): a Culture or Domination seat
//! trains a Spy for every free slot from the moment it has slots, in the
//! cities that finish one soonest, instead of one at a time after a rival
//! takes Rocketry.
//!
//! ## Why
//!
//! Live Deity Pericles and Kristina, 11-10 21Z to 11-11 01Z: 15 culture-lane
//! games, 7 lost to a rival Science Victory at t168-202. The space denial
//! (`racer-keeps-the-spies`, `science-denial-trains-spies`,
//! `space-denial-spies-from-rocketry`, the spy cards and Rocket Scientist)
//! was armed in all of them, yet the network stood half empty. Over the 12
//! games that had Spy slots, 2,202 slot-turns after the first rival's
//! Rocketry: a Spy in place 41%, in transit 25%, in training 14%, held by a
//! captor 14%, and 11% with nothing in the slot at all. Before that Rocketry
//! (260 slot-turns, capacity from t83-128) 61% of the slots stood empty:
//! nothing trains a Spy on this lane until a rival holds Rocketry. The first
//! Spy came 6-42 turns after the first slot (median 18).
//!
//! `science-denial-trains-spies` claims one queue at a time ("the host
//! trains them one by one") and waits for that Spy before the next. The host
//! does not: Civilization VI counts a Spy in production against the
//! capacity, and two cities trained Spies side by side in
//! civvis-20261110T232642Z (Pharsalos and Knossos, t98-107). The claim
//! also sat behind Spies the stock valuation had started in the weakest
//! cities: 232642Z's first Spy took 18 turns in Knossos at 6 Production a
//! turn while Athens made 33; 010629Z's took 16 in Olympia at 19 against 48.
//!
//! Spies cannot be bought: `UNIT_SPY` has no `PurchaseYield`
//! (`GameplayDB.Units`: Cost 225, `COST_PROGRESSION_PREVIOUS_COPIES` 75, so
//! 112, 150, 187... Production at Online speed), and the city purchase
//! menus never list it. Production is the only supply.
//!
//! ## What the gene does
//!
//! Inert with the flag off, off a Culture or Domination aim, with the
//! Science Victory off, or with `science-denial-trains-spies` off (it
//! reshapes that claim). On:
//!
//! 1. **From the first slot.** With Spy capacity and no rival yet on the
//!    denial board, the met rival making the most Science a turn stands in
//!    as the racer the claim trains against, so the slots fill before the
//!    first Rocketry instead of after it. The posting is unchanged: until a
//!    rival is a racer the Spies take the plan's ordinary posts.
//! 2. **To capacity, side by side.** The claim trains to our whole Spy
//!    capacity and does not wait for a Spy already in training: each pass
//!    claims one more queue while Spies held and in training stay under
//!    capacity and a city may still train one. On the live board that last
//!    test is the host's own menu (`BuildQueue:CanProduce` per city, which
//!    already counts Spies in training, in transit and held by captors), so
//!    no order goes past the host's capacity; one claim per pass keeps every
//!    order inside the menu the pass read. A native board claims the whole
//!    shortfall at once.
//! 3. **Soonest first.** When the host quotes the Spy in every eligible
//!    city, the claim takes the fewest turns, then the most Production.
//! 4. **Culture first on the Culture aim.** A Spy never displaces a Theater
//!    Square district, a building of one, or a building with Great Work
//!    slots. Replayed on captured live boards, the unguarded claim replaced
//!    an Art Museum in Sparta (civvis-20261111T002909Z t100).
//!
//! The eligibility, displacement, siege, threat and launch-pad rules of the
//! claim are unchanged, and the governor keeps a Spy it trains for the gene
//! as it keeps the denial's own.
//!
//! **`denial-spies-fill-every-slot-2`.** Version one, and on a Score aim as
//! well. Operator 2026-11-15: the live focus is a Score win (Rome, Level 6,
//! a 180-turn cap), and a rival's Science Victory ends most Score games
//! before the cap. A seat plays one version: enabling version two turns
//! version one off.
//!
//! **`denial-spies-fill-every-slot-3`.** Version two, and on a Science,
//! Religion or Diplomacy aim as well, so on every aim. Live Deity
//! 2026-11-19: version two was forced in every game, yet 17 of the last 21
//! were Science-lane seats, where it was inert. Their slots stood emptier
//! than the Culture seats': Spies on the map against capacity at t130 /
//! t150 / t170, 1 of 2.5, 1 of 3, 2 of 4 against 2 of 2, 2 of 3, 2.5 of 4,
//! and slots with neither a Spy nor one in training 0.69 / 0.88 / 1.38
//! against 0.26 / 0.51 / 1.14. 12 of the last 14 tournament losses were
//! rival Science Victories at t157-192. Nothing in versions one and two
//! barred a Science seat for a reason: they were measured on Culture
//! games and scoped to them.
//!
//! What a Science seat must keep is its own launch, so on the three new aims
//! the claim takes no city that holds, has placed or queues a Spaceport, or
//! queues a space project (the Spy trains in the next-best city, as under
//! `launch-pad-holds-its-queue`), and displaces no Builder at the head of a
//! queue; on the Religion aim no religious unit either, as the Culture aim
//! keeps its culture heads. Everything else is version two's claim: the
//! threatened and recently attacked cities, soldiers, Settlers, walls,
//! Wonders, projects and repairs were already never displaced, and the
//! ranking (`spies-price-the-science-head`, then the host's soonest quote)
//! is unchanged. The Culture, Domination and Score aims read exactly version
//! two. Enabling version three turns versions one and two off.

use super::*;
use crate::shared_sort::SharedSort;

impl AdvancedAi {
    /// Whether the gene reshapes the Spy claim on this board: the flag, the
    /// denial claim it reshapes, the Science Victory, and a Culture or
    /// Domination aim (or, under `denial-spies-fill-every-slot-2`, a Score
    /// aim too; under `denial-spies-fill-every-slot-3`, any aim).
    pub(super) fn denial_spies_fill_every_slot_on(&self, g: &Game) -> bool {
        (self.denial_spies_fill_every_slot
            || self.denial_spies_fill_every_slot_2
            || self.denial_spies_fill_every_slot_3)
            && self.science_denial_trains_spies
            && g.victory_conditions.science
            && match self.active_victory_target(g) {
                Some(VictoryTarget::Culture | VictoryTarget::Domination) => true,
                Some(VictoryTarget::Score) => {
                    self.denial_spies_fill_every_slot_2 || self.denial_spies_fill_every_slot_3
                }
                Some(
                    VictoryTarget::Science | VictoryTarget::Religion | VictoryTarget::Diplomacy,
                ) => self.denial_spies_fill_every_slot_3,
                None => false,
            }
    }

    /// Under version three on a Science, Religion or Diplomacy aim (the aims
    /// versions one and two leave alone), whether the claim leaves `cid` to
    /// the seat's own work: a city that holds, has placed or queues a
    /// Spaceport, or queues a space project; a queue opening with a Builder;
    /// on the Religion aim, one opening with a religious unit. False off
    /// version three and on the older versions' aims.
    pub(super) fn denial_fill_keeps_the_launch(&self, g: &Game, cid: u32) -> bool {
        let target = self.active_victory_target(g);
        if !self.denial_spies_fill_every_slot_3
            || !matches!(
                target,
                Some(VictoryTarget::Science | VictoryTarget::Religion | VictoryTarget::Diplomacy)
            )
            || !self.denial_spies_fill_every_slot_on(g)
        {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let space_project = city.queue.iter().any(
            |item| matches!(item, Item::Project { project } if Game::is_space_project(project)),
        );
        let kept_head = city.queue.first().is_some_and(|item| match item {
            Item::Unit { unit } => {
                *unit == "builder"
                    || (target == Some(VictoryTarget::Religion)
                        && g.rules
                            .units
                            .get(unit)
                            .is_some_and(|spec| spec.class == "religious"))
            }
            _ => false,
        });
        space_project || kept_head || Self::city_has_spaceport_commitment(g, cid)
    }

    /// The rivals the Spy claim trains against: the denial board
    /// ([`Self::denial_spy_racers`]), or under the gene, while that board is
    /// empty and we hold Spy capacity, the met living rival major making the
    /// most Science a turn (the lowest seat on a tie).
    pub(super) fn denial_training_racers(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        let racers = self.denial_spy_racers(g, pid);
        if !racers.is_empty() || !self.denial_spies_fill_every_slot_on(g) {
            return racers;
        }
        if g.spy_capacity(pid) <= 0 {
            return racers;
        }
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
            })
            .map(|rival| (Self::seat_science_per_turn(g, rival.id), rival.id))
            .max_by(|(left, left_id), (right, right_id)| {
                left.total_cmp(right).then(right_id.cmp(left_id))
            })
            .map(|(_, rival)| rival)
            .into_iter()
            .collect()
    }

    /// The Spies the claim trains to: our whole capacity under the gene,
    /// else [`Self::denial_spies_wanted`].
    pub(super) fn denial_training_target(
        &self,
        g: &Game,
        pid: usize,
        racers: &BTreeSet<usize>,
    ) -> usize {
        if self.denial_spies_fill_every_slot_on(g) {
            g.spy_capacity(pid).max(0) as usize
        } else {
            self.denial_spies_wanted(g, pid, racers)
        }
    }

    /// How many queues one pass of the claim may take, given `held` Spies,
    /// `queued` in training and `needed`: one while a Spy trains (the old
    /// rule) and none once it does; under the gene, the shortfall, but one
    /// per pass on a board with host menus, whose answer is read before the
    /// pass places any order.
    pub(super) fn denial_training_claims(
        &self,
        g: &Game,
        held: usize,
        queued: usize,
        needed: usize,
    ) -> usize {
        if !self.denial_spies_fill_every_slot_on(g) {
            return usize::from(queued == 0 && held < needed);
        }
        let shortfall = needed.saturating_sub(held + queued);
        if g.host_buildable.is_empty() {
            shortfall
        } else {
            shortfall.min(1)
        }
    }

    /// Under the gene on a Culture aim, whether `cid` opens its queue with
    /// culture a Spy must not displace: a Theater Square district, a building
    /// of one, or any building with Great Work slots. False off the gene or
    /// off the Culture aim (the old displacement rule).
    pub(super) fn denial_fill_keeps_the_culture_head(&self, g: &Game, cid: u32) -> bool {
        if !self.denial_spies_fill_every_slot_on(g)
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return false;
        }
        g.cities
            .get(&cid)
            .and_then(|city| city.queue.first())
            .is_some_and(|item| match item {
                Item::District { district, .. } => g.district_family(*district) == "theater_square",
                Item::Building { building } => {
                    g.rules.buildings.get(building).is_some_and(|spec| {
                        !spec.great_work_slots.is_empty()
                            || spec.district.is_some_and(|district| {
                                g.district_family(district) == "theater_square"
                            })
                    })
                }
                _ => false,
            })
    }

    /// Under the gene, order the claim's candidates by the host's quote for
    /// a Spy when every one carries a finite positive quote: fewest turns,
    /// then most Production, then the lowest id. Leaves them alone
    /// otherwise.
    pub(super) fn denial_fill_rank_by_quote(&self, g: &Game, candidates: &mut [(f64, u32)]) {
        if !self.denial_spies_fill_every_slot_on(g) {
            return;
        }
        let spy = Item::Unit {
            unit: crate::name!("spy"),
        };
        let quote = |cid: u32| {
            g.host_production_turns(cid, &spy)
                .filter(|turns| turns.is_finite() && *turns > 0.0)
        };
        if !candidates.iter().all(|(_, cid)| quote(*cid).is_some()) {
            return;
        }
        candidates.sort_by_shared(|(left, left_city), (right, right_city)| {
            quote(*left_city)
                .unwrap()
                .total_cmp(&quote(*right_city).unwrap())
                .then_with(|| right.total_cmp(left))
                .then(left_city.cmp(right_city))
        });
    }
}

#[cfg(test)]
mod tests;
