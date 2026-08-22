//! The religious corps: four opt-in genes for the units a founder buys with
//! Faith and what it does with them afterwards.
//!
//! ★★★★ THE FOUNDER LOSES ITS OWN CITIES AND DIES WITH THE BANK FULL. The 6p
//! 60k screen (13,446 seat-pairs, 2026-08-20) that
//! [`AdvancedAi::inquisition_on_threat`] was built from measured a founder
//! that kept its cities at **52.5% wins** and a founder that ended with three
//! or more cities under a rival faith at **3.0%** — the same as never
//! founding at all — and that was **46% of every founder**. Of the seats lost
//! to a rival's religion, **58% died with 300+ Faith banked**.
//!
//! `inquisition_on_threat` opened the one lock that file names: outside an
//! offensive posture `apostle_cap` was 0, so the unit that launches the
//! Inquisition was never bought and `inquisitor_cap` could never leave 0.
//! These four are the rest of the same chain, and each is a thing the
//! controller does **not do at all** rather than a number it sets badly —
//! `docs/GENOME.md` is unambiguous that re-pricing the second is the
//! perpetual null and repairing the first is where its one gate-passing gain
//! came from.
//!
//! ## What the engine pays for, and what the controller never collects
//!
//! - **A wounded spreader is a weaker spreader.** `Game::do_spread` adds
//!   `spread × foreign_multiplier × hp/100` pressure. A Missionary at 40 hit
//!   points spends a whole charge for 40% of a charge's pressure. Nothing in
//!   the controller reads a religious unit's health before spending one.
//! - **Standing still is the only way a religious unit heals.** `end_turn`
//!   heals a unit only when `!acted`, and `do_move` sets `acted` on every
//!   step — while `do_fortify`, the military escape hatch, refuses a
//!   civilian. So a religious unit heals only on a turn its controller gives
//!   it nothing to do, and `advanced_religious_step` always finds something:
//!   its last leg walks toward the nearest rival religious unit, or the Holy
//!   City, from anywhere on the map. The corps therefore decays monotonically
//!   from its first theological exchange to its last charge.
//! - **The heal only exists at home.** `unit_heal_rate` gives a religious
//!   unit `3 × faith` of an adjacent friendly Holy Site — the Monastery that
//!   would heal it in the field is `unique_to` Armagh — so the hold this
//!   module adds can only ever fire where the unit is standing in its own
//!   ring, which is where leaving costs nothing.
//! - **The Guru is the only field heal a corps has**, +40 hit points to every
//!   adjacent religious unit of its faith, and `guru_cap` is
//!   `offensive && apostles > 0`. A founder purely on defence — the 46% —
//!   cannot buy one, so its damaged corps has no heal at all beyond standing
//!   in the ring.
//! - **A military unit may condemn a heretic at peace when the World Congress
//!   says so.** `do_condemn_heretic` accepts the blow when
//!   `is_at_war(pid, target.owner) || congress_effect_active("world_religion",
//!   "B", religion)`; `condemn_step` asks only the first half, so the
//!   Congress licence has never once been used. Interdiction is the cheap
//!   half of the counter — measured 08-17 over 39 live games, we field 590
//!   religious units against rivals' 12,708, and 41% of rival sightings
//!   already have one of our military units within two tiles.
//!
//! ## What is deliberately NOT here
//!
//! No gene raises `d_holy` or otherwise re-ranks the Holy Site district.
//! #1491 measured that at +20 Elo on a 24x16 board and **+2 (CI −46..+50)**
//! at the deployment shape, where it trades science victories for religious
//! ones about 1:1. The district ranking is a valuation, and it is closed.
//!
//! All four are off everywhere by default, as an unmeasured screenable gene
//! is: `gene_ledger::ledger_default_on` returns `false` for a tag no screen
//! has priced. `docs/GENE_SCREEN.md` documents the instrument that prices
//! them.

use super::AdvancedAi;
use crate::game::{Action, Game, Item};
use crate::Pos;

/// Below this share of full health a spreader is spending a whole charge for
/// a fraction of a charge's pressure, because `do_spread` scales the pressure
/// it adds by `hp/100`. An adjacent Holy Site heals `3 × faith` a turn — 12
/// to 18 with a Shrine and a Temple standing — so the wait is two or three
/// turns against a third of every remaining charge.
const SPREADER_HEALTH_FLOOR: i32 = 70;

/// The defensive Missionary corps may not exceed this, however many cities
/// are slipping. The cap exists so a founder under pressure everywhere cannot
/// convert its whole bank into spreaders and starve the Apostle that launches
/// the Inquisition — the failure the first `inquisition_on_threat` cut
/// measured at −8.2 pp by starving the Missionaries the other way.
const DEFENSIVE_MISSIONARY_CEILING: usize = 4;

impl AdvancedAi {
    /// `religious_defence_scales`: how many Missionaries a founder that is not
    /// on the offensive may hold.
    ///
    /// The shipped rule is `(1 + defensive_targets.div_ceil(2)).min(2)` — a
    /// constant 2 from the second threatened city onward, which is the same
    /// answer to three cities slipping as to one. With the gene on the corps
    /// is one spreader per city actually under pressure, bounded by
    /// [`DEFENSIVE_MISSIONARY_CEILING`].
    ///
    /// Returns the shipped value untouched when the gene is off, so this is
    /// an exact no-op in production.
    pub(super) fn defensive_missionary_cap(
        &self,
        defensive_targets: usize,
        shipped: usize,
    ) -> usize {
        if !self.religious_defence_scales {
            return shipped;
        }
        (1 + defensive_targets)
            .min(DEFENSIVE_MISSIONARY_CEILING)
            .max(shipped)
    }

    /// `guru_defends_the_corps`: whether a founder that is not on the
    /// offensive may hold one Guru.
    ///
    /// The Guru is the corps' only field heal and `guru_cap` reads
    /// `offensive && apostles > 0`, so the founder whose whole religious
    /// effort is defending its own cities can never buy one. This says yes
    /// only when there is something for it to heal: the home is under
    /// conversion pressure and a religious unit is already damaged.
    pub(super) fn guru_defends_the_corps(
        &self,
        g: &Game,
        pid: usize,
        home_under_pressure: bool,
    ) -> bool {
        if !self.guru_heals_the_corps || !home_under_pressure {
            return false;
        }
        g.units.values().any(|unit| {
            unit.owner == pid
                && unit.hp < 100
                && g.rules.units[unit.kind].class == "religious"
                && unit.kind != "guru"
        })
    }

    /// `religious_units_heal_first`: whether this religious unit should spend
    /// the turn standing still so the engine heals it, instead of spending a
    /// charge at a fraction of its strength.
    ///
    /// Three conditions, all of them narrow on purpose:
    ///
    /// - it is below [`SPREADER_HEALTH_FLOOR`], so a charge spent now is
    ///   measurably discounted;
    /// - `Game::unit_heal_rate` is positive **here**, which for a religious
    ///   unit means it is already standing in its own Holy Site's ring — the
    ///   hold can never strand a unit somewhere it would not recover;
    /// - and no city within reach of this turn's Spread is one it is needed
    ///   in. A city already losing its majority cannot wait three turns, and
    ///   the defensive conversion is the whole point of holding a corps.
    ///
    /// The caller returns `false` from its step, which is how this controller
    /// says "no action": `plan_general_unit_turn` breaks its loop, the unit
    /// never sets `acted`, and `end_turn` heals it.
    pub(super) fn religious_unit_holds_to_heal(&self, g: &Game, pid: usize, uid: u32) -> bool {
        if !self.religious_units_heal_first {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.hp >= SPREADER_HEALTH_FLOOR || g.unit_heal_rate(uid) <= 0 {
            return false;
        }
        let Some(religion) = unit
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone())
        else {
            return false;
        };
        // `do_spread` accepts a city on this tile or an adjacent one, so this
        // is exactly the set the unit could convert without moving.
        let reachable_now = std::iter::once(unit.pos).chain(g.nbrs(unit.pos));
        !reachable_now
            .filter_map(|position| g.city_at(position))
            .filter_map(|cid| g.cities.get(&cid))
            .any(|city| Self::city_needs_religious_support(g, pid, city, &religion))
    }

    /// `spread_campaign_persists`: whether a founder outside the Religion lane
    /// is on the offensive.
    ///
    /// ★★★ THE CAMPAIGN CANNOT SURVIVE ITS OWN LAST CHARGE. The shipped test
    /// is `active_campaign || faith >= 2_000×speed`, and `active_campaign`
    /// means *this instant* holding a spreader with a charge left. A
    /// Missionary is removed by `do_spread` on its last charge, so the turn a
    /// wave finishes the posture drops: `missionary_cap` falls from six to
    /// two, `apostle_cap` from two to zero, and the campaign that was
    /// converting cities stops until the bank climbs back to two thousand.
    /// The empire then re-buys, re-walks and re-converts ground it had
    /// already taken. It is the same shape as `siege_commitment` and
    /// `settler_target_hysteresis`: a live commitment that nothing keeps
    /// alive between waves.
    ///
    /// With the gene on, a seat that has **already converted a foreign city**
    /// to its religion stays on the offensive while an unconverted foreign
    /// target remains — the reserve its caller passes still decides what it
    /// can afford, so this opens the posture, never the purse.
    ///
    /// ⚠ It is a genuine risk and the screen must price it, not this comment.
    /// #1491 measured the Holy Site re-ranking trading science victories for
    /// religious ones about 1:1 at the deployment shape, and a campaign that
    /// persists spends more Faith on spreaders. `docs/GENOME.md` is why this
    /// ships off: reachability is not leverage.
    pub(super) fn spread_campaign_persists(&self, g: &Game, pid: usize, religion: &str) -> bool {
        if !self.spread_campaign_persists {
            return false;
        }
        g.cities.values().any(|city| {
            city.owner != pid
                && !g.players[city.owner].is_barbarian
                && g.city_religion(city) == Some(religion)
        })
    }

    /// `holy_site_where_the_threat_is`: put a Holy Site in the city that is
    /// actually slipping, so its defender can be bought there.
    ///
    /// ★★★ THE DEFENCE CANNOT BE BOUGHT WHERE IT IS NEEDED.
    /// `Game::unit_purchase_cost` refuses a religious unit unless **that
    /// city** holds a Holy Site district and has a religion; the Faith bank is
    /// empire-wide but the counter is not. A founder whose only Holy Site is
    /// its Holy City therefore answers a city flipping on the far side of the
    /// empire by buying at home and walking — and `do_move` sets `acted`, so
    /// the walk is also the turns the unit does not heal and does not spread.
    /// `founder_temple` fills in the Shrine and the Temple, but only in a city
    /// that already has the district.
    ///
    /// With the gene on, a city under conversion pressure with no Holy Site
    /// claims one, bought with Gold when the treasury covers it and otherwise
    /// put at the front of that city's own queue.
    ///
    /// ⚠ ADJACENT TO A CLOSED QUESTION, AND DELIBERATELY NARROWER THAN IT.
    /// #1491 measured raising `d_holy` — the district's standing rank against
    /// the Campus and the Commercial Hub — at +20 Elo on a 24x16 board and
    /// **+2 (CI −46..+50)** at the deployment shape, where it trades science
    /// victories for religious ones about 1:1, and reverted it. This changes
    /// no ranking: it is a conditional claim in a city that is losing its
    /// majority right now, which is the case that ranking never priced.
    pub(super) fn holy_site_where_the_threat_is(&self, g: &mut Game, pid: usize) {
        if !self.holy_site_where_the_threat_is {
            return;
        }
        let Some(religion) = g.players[pid].religion.clone() else {
            return;
        };
        let family = crate::name!("holy_site");
        let mut slipping: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                !g.city_has_district_family(&g.cities[cid], family)
                    && Self::city_needs_religious_support(g, pid, &g.cities[cid], &religion)
            })
            .collect();
        // The biggest city first: pressure scales with followers, and a
        // stable id breaks the tie so two runs of the same game agree.
        slipping.sort_by_key(|cid| (std::cmp::Reverse(g.cities[cid].pop), *cid));
        for cid in slipping {
            let district = crate::ai::BasicAi::civ_district(g, pid, "holy_site");
            let Some(item) = g
                .district_sites(cid, district)
                .into_iter()
                .max_by(|left, right| {
                    g.district_yields(district, *left)
                        .total()
                        .partial_cmp(&g.district_yields(district, *right).total())
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(left.cmp(right))
                })
                .map(|pos| Item::District { district, pos })
                .filter(|item| g.can_produce(pid, cid, item))
            else {
                continue;
            };
            // The engine prices the purchase and refuses it when the
            // treasury cannot cover it, so ask it rather than quoting a
            // second price here.
            if let Item::District { district, pos } = item {
                if g.apply(
                    pid,
                    &Action::BuyDistrict {
                        city: cid,
                        district,
                        pos,
                        currency: "gold".to_string(),
                    },
                )
                .is_ok()
                {
                    return;
                }
            }
            // A city losing its majority has nothing in its queue worth more
            // than the counter it cannot otherwise buy. `Produce` keeps the
            // displaced item's progress.
            if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
                return;
            }
        }
    }

    /// `enhancer_for_the_corps`: score an Apostle's belief by what it does for
    /// the corps when the corps has a job.
    ///
    /// ★★★ THE BELIEFS THAT MULTIPLY A CHARGE ARE SCORED AS "ANYTHING ELSE".
    /// `advanced_religious_step`'s `EvangelizeBelief` table scores by victory
    /// lane — `wat` for Science, `cathedral` for Culture, `holy_order` for
    /// Religion — and gives everything it does not name **100**, the same
    /// number for `scripture` (+25% to the pressure every charge adds) as for
    /// a worship building this empire has no particular use for. So the ties
    /// are broken by name. The five enhancers below are the corps' only
    /// multipliers in the whole belief catalogue:
    ///
    /// | belief | effect |
    /// |---|---|
    /// | `holy_order` | religious units 30% cheaper in Faith — more corps per bank |
    /// | `scripture` | +25% pressure strength — every charge is worth more |
    /// | `itinerant_preachers` | +30% pressure range |
    /// | `missionary_zeal` | +1 flat movement — fewer turns walking, more healing |
    /// | `defender_of_the_faith` | +5 combat strength in friendly cities |
    ///
    /// This is the same shape as the shipped `apostle_promotion_by_role`
    /// gene: promote — here, evangelize — for the job the empire actually has
    /// rather than for the number the lane table happens to carry. It returns
    /// `None` unless the corps has a job (a city under conversion pressure, or
    /// a spread campaign in the field), so a comfortable founder keeps the
    /// lane's worship pick exactly as shipped.
    pub(super) fn corps_enhancer_score(&self, g: &Game, pid: usize, belief: &str) -> Option<i32> {
        if !self.enhancer_for_the_corps {
            return None;
        }
        // Above the lane table's own maximum of 300, but only for a corps
        // with work in front of it.
        let score = match belief {
            "holy_order" => 400,
            "scripture" => 380,
            "itinerant_preachers" => 340,
            "missionary_zeal" => 320,
            "defender_of_the_faith" => 300,
            _ => return None,
        };
        let religion = g.players[pid].religion.as_deref()?;
        let defending = g
            .player_city_ids(pid)
            .into_iter()
            .any(|cid| Self::city_needs_religious_support(g, pid, &g.cities[&cid], religion));
        let campaigning = g.units.values().any(|unit| {
            unit.owner == pid && unit.charges > 0 && g.rules.units[unit.kind].religious_spread > 0.0
        });
        (defending || campaigning).then_some(score)
    }

    /// `condemn_under_congress`: the enemy religious unit on this tile a
    /// military unit may remove, whether by war or by the World Congress.
    ///
    /// `do_condemn_heretic` licenses the blow when either
    /// `is_at_war(pid, target.owner)` **or** the Congress has condemned the
    /// target's religion (`world_religion`, option B). `condemn_step` has only
    /// ever asked the first, so a resolution this seat may itself have voted
    /// for has never removed a single missionary. With the gene off this is
    /// the war test alone, unchanged.
    pub(super) fn condemnable_heretic(&self, g: &Game, pid: usize, at: Pos) -> Option<u32> {
        g.units_at(at).into_iter().find(|target| {
            let target = &g.units[target];
            if target.owner == pid || g.rules.units[target.kind].class != "religious" {
                return false;
            }
            if g.is_at_war(pid, target.owner) {
                return true;
            }
            self.condemn_under_congress
                && target.religion.as_deref().is_some_and(|religion| {
                    g.congress_effect_active("world_religion", "B", religion)
                })
        })
    }
}
