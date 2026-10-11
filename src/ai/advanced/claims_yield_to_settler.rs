//! The queue claims (`aqueduct-claims-the-capped-queue`,
//! `industrial-zone-claims-the-producer-queue`,
//! `campus-building-claims-the-queue`) yield a city whose next Settler the
//! strategic governor would train.
//!
//! **Why (live Immortal, 10-11).** The claims run in the controller ahead of
//! the strategic production governor, which is where every post-opening
//! Settler is chosen ("starts settler | … the empire holds N military for N
//! cities against a target of N each"). Their never-displace lists keep a
//! Settler already in a queue, and their idle branch asked the basic step's
//! `settler_due`, whose single-walker gate and basic city target are not the
//! governor's pipeline width and settlement target. So once the early city
//! floor closed (~live turn 60), a city the governor would have handed a
//! Settler was claimed first. Settler starts over live turns 60-100 fell from
//! a median 3-4.5 a game on the pins before the Industrial Zone claim (39
//! games) to 0-2.5 after it (28 games), and the city count at turn 100 with
//! it.
//!
//! [`AdvancedAi::settler_due_in`] asks the governor's own Settler admission
//! — room under the settlement target, a free pipeline slot, population two,
//! the city's expansion window open, the factory coordination's pick when
//! that is on, and a practical site — so a claim steps aside exactly where
//! the governor would bid a Settler. Each claim asks it only behind its own
//! gene flag.

use super::*;

impl AdvancedAi {
    /// Whether the strategic governor's Settler arm would admit `cid` this
    /// turn: the empire is short of its settlement target counting Settlers
    /// in flight, the pipeline has a free slot, the city has the population
    /// and an open expansion window, it is the coordinated factory when
    /// `settler_factory_coordination` is on, and a practical site exists.
    /// The cheap gates go first; the site scan last.
    pub(super) fn settler_due_in(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
    ) -> bool {
        let settler = Item::Unit {
            unit: crate::name!("settler"),
        };
        let city_count = g.player_city_ids(pid).len();
        let desired = self.settlement_target(plan);
        if city_count + counts.settlers >= desired {
            return false;
        }
        let allowed = self.settler_pipeline_width(g, pid, desired, city_count, counts.settlers);
        if counts.settlers >= allowed {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.pop < 2 || !g.can_produce(pid, cid, &settler) {
            return false;
        }
        if !self.settler_expansion_window_open(g, pid, cid) {
            return false;
        }
        if self.settler_factory_coordination
            && !self.settler_factory_is_competitive(g, pid, cid, plan, counts)
        {
            return false;
        }
        self.base.has_practical_settle_site(g, pid)
    }
}
