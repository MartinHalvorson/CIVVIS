//! `camp-tile-buyout`: buy the ground a Barbarian Outpost stands on when
//! being rid of it is worth more than the plot's price.
//!
//! Operator request, 2026-08-25: *"buy a barbarian camp tile that is adjacent
//! to our borders if we care about clearing it more than the monetary cost."*
//! "Adjacent to our borders" is exactly what a `BuyPlot` already means —
//! `Game::plot_purchase_cost` sells a neutral plot only when it touches this
//! city's own territory and stands inside its three-ring radius — so the gene
//! is a price and a decision, not a new action.
//!
//! ⚠ WHAT THE GOLD BUYS, AND WHAT IT DOES NOT. Civilization VI disperses an
//! outpost on ENTRY and by nothing else. `Improvements.xml` files it as
//! `<Row ImprovementType="IMPROVEMENT_BARBARIAN_CAMP" … BarbarianCamp="true"
//! RemoveOnEntry="true" DispersalGold="50" TraitType="TRAIT_BARBARIAN"
//! Appeal="-1"/>`, and the Civilopedia's own account of the world says the
//! same in words — "A civilization will earn a [ICON_Gold] Gold reward for
//! dispersing a Barbarian outpost − in addition to the benefit of stopping it
//! from spawning more Barbarian units"
//! (`Base/Assets/Text/en_US/Civilopedia_Concepts_Text.xml`,
//! `LOC_PEDIA_CONCEPTS_PAGE_WORLD_6_CHAPTER_CONTENT_PARA_1`). No column and no
//! line in the shipped rules dissolves an outpost because the plot changed
//! hands, and CIVVIS does not invent one: `Game::do_buy_plot` moves the deed
//! and leaves the camp standing, exactly as the shipped game does.
//!
//! So a purchase here is the *second half* of a clearance, never the
//! clearance itself. What it is worth is the ground:
//!
//! - **Culture will not take it.** `border_influence_cost` charges a camp
//!   plot the shipped `PLOT_INFLUENCE_IMPROVEMENT_COST` of +100 — a whole
//!   ring's worth of distance — so a city grows *around* an outpost and
//!   leaves the hole behind for a long time after a soldier has walked in.
//!   Gold is the only prompt way to hold that hex.
//! - **The hex is the outpost's hex.** It is by construction inside the
//!   city's own three rings, next to ground we already work, which is why
//!   the raiders it releases are on our improvements the turn they spawn.
//!
//! ★★★★ AND A CAMP DOES STAND. `civvis-20260816T200454Z` kept one for **130
//! turns** inside a capital's own six-tile ring (see
//! `in_peacetime_the_whole_field_army_answers_and_the_camp_outranks_the_countryside`):
//! the home guard's recall budget was half the army with the garrison charged
//! against it, so a three-unit peacetime army had a cap of zero and nobody
//! went. That is the empire this gene is for — one that lives beside an
//! outpost for a hundred turns and never takes the hex it sits on.
//!
//! And so the gene refuses to buy ground it has no way to make use of: the
//! outpost must be one we can see, and one a soldier of ours can actually
//! reach and disperse ([`AdvancedAi::camp_buyout_clearer`]). Buying the deed
//! to a camp no unit of ours can walk into is paying for a hole in the
//! border.
//!
//! **Why it is a pass of its own** rather than another candidate in
//! `advanced_gold_spending`'s ranking: the two scales do not meet. A unit or
//! building is scored `production_value × (7 + turns)` and reaches the
//! thousands, while every plot in the game is a *surplus* purchase scored
//! near its 120 floor and considered only when nothing else qualifies at all.
//! A threat priced into that ranking would never be bought, which is the same
//! as not writing the gene. The operator's rule is a threshold, so it is
//! implemented as one: when the outpost costs us more than the quote, the hex
//! is bought, at most one a turn, and never out of the reserve. It does not
//! end the turn's spending the way an emergency defence does — a hex bought
//! out of the surplus above the reserve should not also cost the city the
//! Settler it was saving for — and a city under fire still outranks it,
//! because `emergency_city_defense_purchase` runs first.
//!
//! Off, `advanced_gold_spending` is byte-identical: one flag is read, and no
//! outpost is ever priced.

use super::AdvancedAi;
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;

/// What an outpost one ring off the City Center costs the city that has to
/// live beside it, in Gold. An outpost pins a garrison, pillages the
/// improvements around it and keeps releasing raiders for as long as it
/// stands; against the shipped ring-1 quote of 50 Gold that is four rings'
/// worth of ground, and the quote climbs to five times that as the game
/// runs on (`tile_purchase_cost`), which is where the rule starts refusing.
pub(super) const CAMP_STANDING_COST: f64 = 240.0;

/// The share of that a ring-one, ring-two and ring-three outpost carries.
/// Distance is priced rather than gated, the way `border_influence_cost`
/// prices it: the far corner of the third ring is half the nuisance the hex
/// against the City Center is.
pub(super) const CAMP_RING_SHARE: [f64; 3] = [1.0, 0.75, 0.5];

/// A Barbarian standing this close to the outpost is part of what the
/// outpost is about to do to us.
pub(super) const CAMP_PARTY_RADIUS: i32 = 2;

/// What each of those we can see is worth on top of the standing cost …
pub(super) const CAMP_PARTY_UNIT: f64 = 60.0;

/// … up to a raid party's worth of them. Past this the answer is soldiers,
/// not a deed.
pub(super) const CAMP_PARTY_CAP: usize = 3;

/// How far a soldier may be from the outpost and still count as able to
/// disperse it: the camp errand's own home-ground radius
/// (`crate::ai::HOME_THREAT_RADIUS`), so the two agree about which outposts
/// are ours to deal with.
pub(super) const CAMP_CLEAR_REACH: i32 = crate::ai::HOME_THREAT_RADIUS;

/// The headroom every Gold purchase in `advanced_gold_spending` leaves above
/// the strategy's reserve, matched here so a buyout cannot empty a treasury
/// the ordinary spender was protecting.
pub(super) const CAMP_BUY_HEADROOM: f64 = 200.0;

impl AdvancedAi {
    /// Whether a unit of ours could walk into the outpost at `pos` and
    /// disperse it. Clearance is a move, so the question is a soldier's
    /// class and its distance, not an attack.
    ///
    /// Recon is excluded for the reason the camp errand excludes it: a Scout
    /// prices an empty camp above its own threshold and would answer this
    /// question for a party it cannot survive.
    pub(super) fn camp_buyout_clearer(&self, g: &Game, pid: usize, pos: Pos) -> bool {
        g.units.values().any(|unit| {
            let spec = &g.rules.units[unit.kind];
            unit.owner == pid
                && spec.class == "military"
                && spec.promotion_class != "recon"
                && g.wdist(unit.pos, pos) <= CAMP_CLEAR_REACH
        })
    }

    /// What being rid of the outpost at `pos` is worth to `city`, in the Gold
    /// the quote is denominated in, or `None` when this is not an outpost we
    /// would buy the ground from at any price.
    pub(super) fn camp_buyout_clearance(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        pos: Pos,
    ) -> Option<f64> {
        if !g.barb_camps.contains_key(&pos) {
            return None;
        }
        let ring = g.wdist(g.cities.get(&city)?.pos, pos);
        let share = *CAMP_RING_SHARE.get(usize::try_from(ring).ok()?.checked_sub(1)?)?;
        // One frame for the whole appraisal. `player_can_see` re-derives the
        // vision input stamp on every call, and the party count below asks it
        // once per Barbarian on the map.
        let visible = g.player_vision_frame(pid);
        // `plot_purchase_cost` sells an EXPLORED plot, and an outpost
        // remembered from thirty turns ago may have been dispersed by
        // somebody else since. Buy only ground we are looking at.
        if !g.sees(&visible, pos) {
            return None;
        }
        // Ground under an outpost nobody of ours can reach is a hole in the
        // border with a deed attached.
        if !self.camp_buyout_clearer(g, pid, pos) {
            return None;
        }
        let party = g
            .units
            .values()
            .filter(|unit| {
                Some(unit.owner) == g.barb_pid
                    && g.wdist(unit.pos, pos) <= CAMP_PARTY_RADIUS
                    && g.sees(&visible, unit.pos)
            })
            .count()
            .min(CAMP_PARTY_CAP) as f64
            * CAMP_PARTY_UNIT;
        Some(CAMP_STANDING_COST * share + party)
    }

    /// Buy out the one outpost worth the most over its quote, if any is.
    /// At most one plot a turn, never below `reserve` plus the headroom the
    /// ordinary spender keeps.
    ///
    /// Cities are the outer loop and the map's outposts the inner one:
    /// `barb_camps` holds a couple of dozen entries on a full world, so this
    /// asks `plot_purchase_cost` a few hundred times at worst and never
    /// enumerates the legal-action space.
    pub(super) fn camp_tile_buyout_purchase(&self, g: &mut Game, pid: usize, reserve: f64) -> bool {
        let bank = g.players[pid].gold;
        let camps: Vec<Pos> = g.barb_camps.keys().copied().collect();
        let mut best: Option<(f64, f64, u32, Pos)> = None;
        for city in g.player_city_ids(pid) {
            for pos in &camps {
                let Some(cost) = g.plot_purchase_cost(pid, city, *pos) else {
                    continue;
                };
                if bank + f64::EPSILON < reserve + CAMP_BUY_HEADROOM + cost {
                    continue;
                }
                let Some(clearance) = self.camp_buyout_clearance(g, pid, city, *pos) else {
                    continue;
                };
                // The operator's rule, and the whole gene: pay only when
                // being rid of the outpost is worth more than the quote.
                if clearance + f64::EPSILON < cost {
                    continue;
                }
                let margin = clearance - cost;
                if best.is_none_or(|(old, _, old_city, old_pos)| {
                    (margin, *pos, city) > (old, old_pos, old_city)
                }) {
                    best = Some((margin, cost, city, *pos));
                }
            }
        }
        let Some((margin, cost, city, pos)) = best else {
            return false;
        };
        if g.apply(pid, &Action::BuyPlot { city, pos, cost }).is_err() {
            return false;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let name = g
                .cities
                .get(&city)
                .map(|city| city.name.clone())
                .unwrap_or_else(|| "the empire".to_string());
            think!(self.journal(), Economy, Decision,
                "Buying the ground a Barbarian Outpost stands on for {name}";
                "{cost:.0} Gold for the hex at {pos:?}, {margin:.0} less than living \
                 beside the outpost costs; the camp itself comes off when a soldier \
                 walks in");
        }
        true
    }
}
