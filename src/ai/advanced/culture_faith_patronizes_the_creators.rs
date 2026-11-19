//! `culture-faith-patronizes-the-creators`: a seat aiming at a Culture
//! victory with no religion of its own buys the cheapest offered Great
//! Writer, Artist or Musician with Faith as soon as the bank carries the
//! price above its reserve and a slot for the work stands open or will
//! stand within [`CULTURE_PATRONAGE_SLOT_ONLINE_TURNS`] Online turns.
//!
//! **Why (live Deity, 6-player Continents, Online, the 53 Culture-seat games
//! of 2026-11-18/19 with a state log).** The Great Person race is what the
//! Culture seats lose: the weak ones took 0-3 Great Writers by t150 against
//! the rivals' 15-24. Yet a Writer, Artist or Musician was Faith-affordable
//! (`patron_faith` within the bank) on 773 game-turns, from about t63-t120,
//! and the seats patronized 59. Counting each individual once, 319 offered
//! creators the seat could pay for went to the rivals instead, about 760
//! Great Works. `advanced_great_people` passed them for four reasons, by
//! affordable turn: no open slot of the work's kind at that moment (407,
//! 53%), the band bank holding the Faith from Nationalism to Cold War (226,
//! 29%), the bank below price plus the ordinary reserve (105, 14%; 700 Faith
//! after Cold War) and, on the rest (35), the one-purchase scoring.
//!
//! **The gene.** While the active victory target is Culture, the lane is
//! not released and the seat has founded no religion (a religion's units keep
//! their Faith), the seat patronizes, with Faith, the cheapest offered
//! Writer, Artist or Musician whose price leaves the reserve in the bank.
//! The reserve is 100 Faith, or 700 once Cold War (a Rock Band) or
//! Conservation (a Naturalist) is known: what one purchasable band or
//! Naturalist costs, so the band bank's pre-Cold-War hoard no longer holds
//! the creators back. The class must be offered now and not exhausted, the
//! person activatable, and a slot of its work's kind (Writing, Art, Music,
//! or an `any` slot) must stand open now or a city of ours must be building a
//! compatible slot building (an Amphitheater, Art Museum or Broadcast Center,
//! or the civilization's version) that finishes within the budget; the
//! creator waits for it as a unit, and `great-works-never-wait` keeps that
//! building claimed. One creator per class a turn. It runs before the
//! ordinary Great Person pass. Off, nothing changes.
//!
//! Grep the journal for `culture-faith-patronizes-the-creators:`.

use super::*;

/// The longest wait, in Online turns, for a slot building already in
/// production before a creator is bought ahead of it.
pub(crate) const CULTURE_PATRONAGE_SLOT_ONLINE_TURNS: f64 = 10.0;

/// The Faith kept in the bank before Cold War or Conservation.
const CULTURE_PATRONAGE_RESERVE: f64 = 100.0;

/// The Faith kept once a Rock Band (Cold War) or a Naturalist (Conservation)
/// can be bought: about one of them, the existing post-Cold-War reserve.
const CULTURE_PATRONAGE_BAND_RESERVE: f64 = 700.0;

impl AdvancedAi {
    /// The Faith a Culture seat keeps before patronizing a creator.
    pub(super) fn culture_patronage_reserve(g: &Game, pid: usize) -> f64 {
        let civics = &g.players[pid].civics;
        if civics.contains(&crate::name!("cold_war"))
            || civics.contains(&crate::name!("conservation"))
        {
            CULTURE_PATRONAGE_BAND_RESERVE
        } else {
            CULTURE_PATRONAGE_RESERVE
        }
    }

    /// Where a creator of `kind` will house its work: `Some("now")` for an
    /// open slot, `Some(city)` for a compatible slot building in production
    /// that finishes within the budget, `None` otherwise.
    pub(super) fn culture_patronage_slot(
        &self,
        g: &Game,
        pid: usize,
        kind: &str,
    ) -> Option<String> {
        let (work, family) = match kind {
            "writer" => ("writing", "amphitheater"),
            "artist" => ("art", "art_museum"),
            "musician" => ("music", "broadcast_center"),
            _ => return None,
        };
        let open = g.players[pid]
            .live_open_great_work_slots
            .as_ref()
            .map_or_else(
                || g.can_house_additional_great_work(pid, work),
                |slots| {
                    slots.contains(work)
                        || slots.contains("any")
                        || (work == "art" && slots.contains("religious_art"))
                },
            );
        if open {
            return Some("now".to_string());
        }
        let budget = CULTURE_PATRONAGE_SLOT_ONLINE_TURNS * g.game_speed.cost_percent() as f64
            / crate::setup::GameSpeed::Online.cost_percent() as f64;
        let family = Name::new(family);
        g.player_city_ids(pid).into_iter().find_map(|cid| {
            let city = g.cities.get(&cid)?;
            let head = city.queue.first()?;
            let Item::Building { building } = head else {
                return None;
            };
            if !g.building_is_family(*building, family) {
                return None;
            }
            let turns = g.host_production_turns(cid, head).unwrap_or_else(|| {
                let remaining =
                    g.item_cost_for_city(pid, cid, head) - g.item_invested_production(cid, head);
                remaining.max(0.0) / g.city_yields(cid).production.max(0.5)
            });
            (turns.is_finite() && turns <= budget).then(|| city.name.clone())
        })
    }

    /// See the module. Exact no-op with the gene off.
    pub(super) fn culture_faith_patronizes_the_creators(&self, g: &mut Game, pid: usize) {
        if !self.culture_faith_patronizes_the_creators
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || g.players[pid].religion.is_some()
        {
            return;
        }
        let reserve = Self::culture_patronage_reserve(g, pid);
        let mut bought: Vec<&str> = Vec::new();
        loop {
            let bank = g.players[pid].faith;
            let choice = ["writer", "artist", "musician"]
                .into_iter()
                .filter(|kind| !bought.contains(kind))
                .filter(|kind| {
                    g.current_great_person(kind).is_some()
                        && g.great_person_class_offered_now(pid, kind)
                        && !(g.players[pid].live_great_person_exhausted.is_some()
                            && !g.great_person_class_earnable(pid, kind))
                        && g.can_activate_current_great_person(pid, kind)
                })
                .filter_map(|kind| {
                    let price = g.great_person_patronage_price(pid, kind, "faith")?;
                    (bank + f64::EPSILON >= price + reserve).then_some((price, kind))
                })
                .filter_map(|(price, kind)| {
                    self.culture_patronage_slot(g, pid, kind)
                        .map(|slot| (price, kind, slot))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(b.1)));
            let Some((price, kind, slot)) = choice else {
                break;
            };
            bought.push(kind);
            let action = Action::PatronizeGreatPerson {
                kind: kind.to_string(),
                currency: "faith".to_string(),
            };
            if g.apply(pid, &action).is_err() {
                continue;
            }
            let housed = if slot == "now" {
                "an open slot takes the work now".to_string()
            } else {
                format!("{slot} is building the slot")
            };
            think!(self.journal(), Economy, Decision,
                   "Patronizes the Great {} with Faith", plain(kind);
                   "culture-faith-patronizes-the-creators: {price:.0} Faith of {bank:.0}, keeping \
                    {reserve:.0}; {housed}");
        }
    }
}

#[cfg(test)]
mod tests;
