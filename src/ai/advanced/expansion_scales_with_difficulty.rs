//! `expansion-scales-with-difficulty`: the opening band is a King-level
//! measurement, and the rungs above King move the finish line.
//!
//! ## The evidence the current band rests on
//!
//! [`super::expansion_schedule`] carries the corpus: 218 completed live runs
//! (`tools/civ6_run_report.py --aggregate` over `~/civvis-civ6-runs/control`,
//! 2026-08-25), **every one of the nine recorded wins from four to six cities
//! at turn 60**, nothing outside it (0/128, one-sided Fisher
//! *p* = 2.6 × 10⁻⁴). [`super::expansion_schedule::EXPANSION_BAND_FLOOR`] aims
//! the whole opening at the middle of that band, five cities, and
//! `rapid_city_expansion::city_target` composes the same number.
//!
//! ⚠ **That corpus is a King-level field.** `docs/civ6_ladder.json` is the
//! ladder those runs came from, and the first live science win it recorded was
//! a King seat. Five cities is what beat *that* field.
//!
//! ## Why five cities is the wrong number two rungs higher
//!
//! The rung is not flavour, it is arithmetic, and `data/difficulties.json` is
//! where it is written down. Reading the rows above Prince (`order` 3):
//!
//! | rung | order | AI science/culture | AI production/gold | free units per AI |
//! |---|---:|---:|---:|---|
//! | King | 4 | +8% | +20% | warrior, builder |
//! | Emperor | 5 | +16% | +40% | 2 warriors, builder, **settler** |
//! | Immortal | 6 | +24% | +60% | 3 warriors, 2 builders, **settler** |
//! | Deity | 7 | +32% | +80% | 4 warriors, 2 builders, **2 settlers** |
//!
//! Two things scale together, and both of them are city count. Every rival
//! yield rises by a fixed percentage *per city they hold*, so a rival empire
//! out-produces ours by that percentage multiplied by its own width; and from
//! Emperor upward each rival opens with free Settlers, which is the same bonus
//! paid forward as extra cities. A five-city empire against an Emperor field
//! is not slightly behind, it is behind **by construction**: the rivals are
//! wider *and* each of their cities is worth more.
//!
//! The ladder agrees. `docs/EVAL_STATUS.md` and the 2026-09-08 census record
//! the Emperor rung at **0 wins in 111 games**, with nineteen of those games
//! lost outright to a rival science or culture finish — a rival economy
//! arriving first, not a battlefield loss. Human play at these rungs answers
//! it the documented way: eight to twelve cities by turn 100, funded by the
//! expansion policy cards and by Builder chops.
//!
//! ## What this gene changes
//!
//! Four legs, all opt-in, all inert with the gene off.
//!
//! 1. **The target scales with the rung.** [`city_target`] is
//!    [`OPENING_BAND_CITIES`] plus the rung's distance above Prince, capped at
//!    [`WIDE_CAP`]: King 6, Emperor 7, Immortal 8, Deity 9. The opening
//!    deadline extends by [`WIDE_DEADLINE_TURNS_PER_LEVEL`] standard turns per
//!    level past the measured band turn — speed-scaled through
//!    [`Game::standard_duration`], so an Online ladder game and a Standard-speed
//!    game get the same *content*, not the same integer. A second target
//!    [`cities_by_hundred`] = [`city_target`] + [`WIDE_SECOND_TARGET_BONUS`]
//!    keeps the cadence running from the extended deadline to
//!    [`WIDE_SECOND_SHARE`] of the clock (turn 100 of the ladder's 250), which
//!    is the horizon the human answer is quoted at.
//!
//!    ⚠ The band itself is untouched. At Prince and below the level is zero,
//!    so [`city_target`] is exactly [`OPENING_BAND_CITIES`] and the deadline is
//!    exactly the band turn; the only thing the gene still adds there is the
//!    second cadence past the band, and that is deliberate — the corpus says
//!    nothing at all about turns after 60.
//!
//! 2. **The Settler cadence reaches past the capital.** The shipped
//!    reservation in [`super::higher_level_strategy`] (`expansion-best-idle-city`
//!    and its disciplined version two) asks for a Settler in the best idle
//!    city, but only while `counts.settlers == 0` and only when no city in the
//!    empire already has one queued. At a five-city pace that is the right
//!    discipline; at nine it serializes the entire empire behind one walker.
//!    With this gene on, a Settler queued **in the capital** — or a capital
//!    busy with a district, the other thing that occupies the one city that can
//!    afford Settlers early — no longer closes the reservation, up to
//!    [`WIDE_PARALLEL_SETTLERS`] walkers in flight. The selection itself is the
//!    existing one: same debt, same idle-queue rule, same ranking, same
//!    production-value and site-gate admission tests. This gene widens the
//!    condition, it does not fork the chooser.
//!
//!    The reservation is only half of it, because the *pipeline* is what
//!    actually vetoes the second Settler: `settler_in_flight_allowed` answers
//!    **1** for the ordinary empire, and `production_value` returns −10 000 for
//!    a Settler past that width. [`AdvancedAi::expansion_wide_pipeline`] is the
//!    second slot, and it counts founded cities rather than walkers — see its
//!    own note for why that difference is the stall.
//!
//!    ⚠ It is **not** a member of the `expansion-best-idle-city` version
//!    family. It carries its own tag and its own field, and enabling it clears
//!    nothing: version one and version two remain independently screenable,
//!    and either of them may be on beside this gene. What it shares with them
//!    is the code path, which is the point.
//!
//! 3. **Chops: unavailable, and not invented here.** The design called for one
//!    reserved Builder charge per Settler in production, on the best
//!    harvestable tile in the producing city's radius. **This simulator has no
//!    harvest.** `grep -n 'harvest\|chop' src/game.rs` returns three lines and
//!    all three are the reasoning-log sense of the word ("the moment its plan
//!    is harvested"); `data/improvements.json` has no harvest action, and
//!    `Action` has no variant for one. Adding a Feature-removal yield rule to
//!    the engine to serve one opt-in gene would be a rules change wearing a
//!    gene's clothes, so the leg is recorded as unavailable in
//!    `docs/eval/2026-09-09-expansion-scales-with-difficulty.md` and left
//!    unimplemented. The Ancestral Hall's free Builder in every new city (leg
//!    4) is the nearest thing the engine does model.
//!
//! 4. **The cards and the Hall.** `strategic_policies`' timed-economy block
//!    already front-loads Colonization and Expropriation while expansion is
//!    active, but only for `wide-map-capacity` and `rapid-city-expansion-2`;
//!    this gene joins that condition, so the +50% Settler card is slotted the
//!    turn Early Empire lands rather than after the late portfolio has taken
//!    the slot. `production_value`'s `expansion_hall` term — the Ancestral
//!    Hall's `settler_production_pct` and `free_builder_new_city`, priced by
//!    how many seats the empire is short — is likewise gated on `land_grab`
//!    today; this gene is the second condition that opens it. Both terms are
//!    already scaled by the shortfall, so they fade to zero on their own once
//!    the empire reaches its target.
//!
//! 5. **Safety is unchanged, on purpose.** Every existing guard still runs and
//!    none of them is relaxed: the walker-aware site gate
//!    ([`AdvancedAi::settler_site_gate`], which is what pauses the cadence when
//!    no acceptable unclaimed seat lies inside the safe radius), the settler
//!    escort and shelter genes, the housing and amenity floors, the threatened
//!    city and barbarian-alarm skips, and the plan's own `desired_cities`
//!    ceiling — which is still cut down afterwards by `city_target_meets_the_map`
//!    (practical sites) and by the Science lane cap. This gene raises a target
//!    and widens a condition; it never sends a Settler anywhere the shipped
//!    code would refuse to send one.
//!
//! ## Off
//!
//! Off, every function here is bypassed by an `if` on the flag:
//! [`AdvancedAi::expansion_deadline_turn`] returns exactly
//! `AdvancedAi::expansion_band_turn`, [`AdvancedAi::expansion_pace_now`]
//! returns exactly `AdvancedAi::expansion_pace`, the target is not raised, the
//! cadence condition is the shipped conjunction, and both leg-4 gates read
//! their shipped operands. No arithmetic runs that did not run before.

use super::expansion_schedule::EXPANSION_BAND_FLOOR;
use super::AdvancedAi;
use crate::game::{Game, Item};

/// Prince's `order` in `data/difficulties.json`. Prince is the unhandicapped
/// reference rung: no AI yield bonus, no free AI units, no human bonus. Every
/// rung above it hands the rival majors a fixed percentage of every yield and,
/// from Emperor, free Settlers — which is why distance above *this* row, and
/// not the raw order, is what the target scales with.
pub const PRINCE_ORDER: usize = 3;

/// The measured opening band's own target, five cities — the middle of the
/// 4–6 band that every recorded win came from. Kept as an alias of
/// [`EXPANSION_BAND_FLOOR`] rather than a second literal so the two can never
/// drift: this gene adds to the shipped number, it does not replace it.
pub const OPENING_BAND_CITIES: usize = EXPANSION_BAND_FLOOR;

/// The widest empire this gene will ever ask for. Deity is four rungs above
/// Prince and the second target adds two more, so without a cap the ladder's
/// top rung would ask for eleven cities on a map that may hold nine. Ten is
/// the top of the human answer's own quoted range (eight to twelve by turn
/// 100) minus the two seats that range assumes are conquered rather than
/// settled. Every land-aware cap downstream still applies on top of it.
pub const WIDE_CAP: usize = 10;

/// Standard turns the opening deadline gains per rung above Prince.
///
/// One city more to found is one Settler more to build, walk and seat. The
/// live founding cadence quoted in `expansion_schedule` — cities 2/3/4/5/6 at
/// turns 37.0/71.0/89.5/118.7/150.2 — puts a marginal seat at roughly twenty
/// live turns, and the deadline is deliberately *tighter* than that: the
/// cadence has to compress, not merely extend, or the extra cities arrive
/// after the window in which they compound. Speed-scaled at the call site.
pub const WIDE_DEADLINE_TURNS_PER_LEVEL: u32 = 10;

/// Cities the second, post-deadline cadence adds on top of [`city_target`].
pub const WIDE_SECOND_TARGET_BONUS: usize = 2;

/// Where the second target sits on the clock, as a share — turn 100 of the
/// ladder's 250-turn game, the horizon the human answer ("eight to twelve
/// cities by turn 100") is quoted at. Expressed as a share for the same reason
/// [`super::expansion_schedule::EXPANSION_BAND_SHARE`] is: a different turn limit scales with it.
pub const WIDE_SECOND_SHARE: f64 = 0.4;

/// The second target's turn in standard turns, for a game with no turn limit.
/// The same 2:1 ratio to its share that `EXPANSION_BAND_SHARE` 0.24 has to
/// `EXPANSION_BAND_STANDARD` 120, so the two horizons stay in proportion on an
/// unlimited board.
pub const WIDE_SECOND_STANDARD: u32 = 200;

/// The most walkers the widened cadence will have in flight at once.
///
/// Two, not more: the reservation this widens takes an *idle* city, and the
/// second walker exists to stop the empire serializing behind the capital, not
/// to open a settler factory. The pipeline width itself is still capped by
/// `EXPANSION_PIPELINE_CEILING`, and the city target is still the hard cap
/// above both.
pub const WIDE_PARALLEL_SETTLERS: usize = 2;

/// How far above Prince this game's rung sits; zero at Prince and below.
///
/// This reads the *rung*, not this seat's own handicap exemption. `--handicap
/// rivals` exempts the measured seats (`Game::handicap_exempt`) precisely so
/// the rivals keep the rung's bonuses, which is exactly the field this gene
/// exists to answer: the number to beat is set by what the rivals get.
pub(super) fn level_above_prince(g: &Game) -> usize {
    g.difficulty_spec().order.saturating_sub(PRINCE_ORDER)
}

/// The opening target at this rung: the measured band plus one city per rung
/// above Prince, capped at [`WIDE_CAP`].
pub(super) fn city_target(level: usize) -> usize {
    (OPENING_BAND_CITIES + level).min(WIDE_CAP)
}

/// The second target the cadence runs on to after the opening deadline.
/// Capped at [`WIDE_CAP`] as well, so the cap is a ceiling on the gene as a
/// whole rather than on one of its two horizons.
pub(super) fn cities_by_hundred(level: usize) -> usize {
    (city_target(level) + WIDE_SECOND_TARGET_BONUS).min(WIDE_CAP)
}

/// The extended opening deadline: the measured band turn plus
/// [`WIDE_DEADLINE_TURNS_PER_LEVEL`] speed-scaled turns per rung.
pub(super) fn deadline_turn(g: &Game, band: u32, level: usize) -> u32 {
    let extra = WIDE_DEADLINE_TURNS_PER_LEVEL.saturating_mul(level as u32);
    band.saturating_add(g.standard_duration(extra)).max(1)
}

/// The turn the second target is asked for. Never at or before `deadline`, so
/// the ramp between them always has a positive width to divide by.
pub(super) fn second_horizon_turn(g: &Game, deadline: u32) -> u32 {
    g.turn_limit()
        .map(|limit| ((limit as f64) * WIDE_SECOND_SHARE) as u32)
        .unwrap_or_else(|| g.standard_duration(WIDE_SECOND_STANDARD))
        .max(deadline.saturating_add(1))
}

/// How many cities this rung's pace expects by now: one at the start,
/// [`city_target`] by the extended deadline, [`cities_by_hundred`] by the
/// second horizon, and flat after it. Two straight ramps, one per horizon.
pub(super) fn pace(g: &Game, band: u32, level: usize) -> usize {
    let deadline = deadline_turn(g, band, level);
    let opening = city_target(level);
    if g.turn < deadline {
        return 1 + ((opening - 1) * g.turn as usize) / deadline as usize;
    }
    let second = second_horizon_turn(g, deadline);
    let wide = cities_by_hundred(level);
    if g.turn >= second {
        return wide;
    }
    opening + ((wide - opening) * (g.turn - deadline) as usize) / (second - deadline) as usize
}

impl AdvancedAi {
    /// This game's rung above Prince, or `None` while the gene is off. Every
    /// other method here funnels through it, so one `if` decides the whole
    /// gene's participation.
    pub(super) fn expansion_wide_level(&self, g: &Game) -> Option<usize> {
        self.expansion_scales_with_difficulty
            .then(|| level_above_prince(g))
    }

    /// The turn the opening pace aims at. Exactly `expansion_band_turn` with
    /// the gene off, and with it on at Prince.
    pub(super) fn expansion_deadline_turn(&self, g: &Game) -> u32 {
        let band = Self::expansion_band_turn(g);
        match self.expansion_wide_level(g) {
            Some(level) => deadline_turn(g, band, level),
            None => band,
        }
    }

    /// The cities the pace expects by now. Exactly `expansion_pace` with the
    /// gene off.
    pub(super) fn expansion_pace_now(&self, g: &Game) -> usize {
        match self.expansion_wide_level(g) {
            Some(level) => pace(g, Self::expansion_band_turn(g), level),
            None => Self::expansion_pace(g),
        }
    }

    /// The last turn the Settler cadence runs. The opening deadline with the
    /// gene off; the second target's horizon with it on, which is what keeps
    /// the cadence going after the opening band closes.
    pub(super) fn expansion_cadence_horizon(&self, g: &Game) -> u32 {
        let deadline = self.expansion_deadline_turn(g);
        match self.expansion_wide_level(g) {
            Some(_) => second_horizon_turn(g, deadline),
            None => deadline,
        }
    }

    /// The horizon the empire's city target may not fall below, or `None`
    /// while the gene is off.
    ///
    /// This is the *second* target, not the opening one: `desired_cities` is a
    /// horizon the whole game plans against, while the pace above is what the
    /// opening is measured by turn to turn. Every land-aware cap downstream —
    /// `city_target_meets_the_map`'s practical-site room, the Science lane cap
    /// — still applies on top of the number returned here.
    pub(super) fn expansion_wide_city_target(&self, g: &Game) -> Option<usize> {
        self.expansion_wide_level(g).map(cities_by_hundred)
    }

    /// The Settler pipeline width the rung's cadence asks for, or `None` when
    /// it asks for nothing (which is always, with the gene off).
    ///
    /// ⚠ It counts **founded cities**, not walkers.
    /// `AdvancedAi::expansion_pace_shortfall` counts a walker as a city and
    /// `advanced/expansion_schedule.rs` documents what that costs: a live
    /// walker that took twenty-eight turns to seat credited the empire with a
    /// city it did not have for every one of them. At a five-city pace that
    /// conservatism is a deliberate brake. At nine it is the stall itself, so
    /// this arm reads the count that is actually on the board and caps the
    /// total at [`WIDE_PARALLEL_SETTLERS`] — two slots, never a factory. The
    /// city target above it remains the hard cap, exactly as it does for the
    /// schedule and for the land grab.
    pub(super) fn expansion_wide_pipeline(
        &self,
        g: &Game,
        desired_cities: usize,
        city_count: usize,
        settlers: usize,
    ) -> Option<usize> {
        let level = self.expansion_wide_level(g)?;
        if g.turn > self.expansion_cadence_horizon(g)
            || city_count + settlers >= desired_cities
            || city_count >= pace(g, Self::expansion_band_turn(g), level)
        {
            return None;
        }
        Some(WIDE_PARALLEL_SETTLERS.min(desired_cities - city_count))
    }

    /// Whether the capital's queue head is a Settler or a district.
    ///
    /// These are the two things that occupy the one city that can afford
    /// Settlers in the opening, and they are the reason a single-walker
    /// cadence stalls: the capital is committed for many turns and no other
    /// city is allowed to start. A capital that is idle, or building a cheap
    /// unit or building, is not "busy" for this purpose — the shipped
    /// reservation reaches it perfectly well.
    pub(super) fn expansion_wide_capital_is_busy(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            if !city.is_capital {
                return false;
            }
            match city.queue.first() {
                Some(Item::Unit { unit }) => unit == "settler",
                Some(Item::District { .. }) => true,
                _ => false,
            }
        })
    }

    /// Whether the widened cadence admits another Settler reservation while
    /// `settlers` walkers are already in flight.
    ///
    /// Off, and whenever the capital is not the thing holding the cadence up,
    /// this is the shipped rule: no reservation while any walker is alive.
    pub(super) fn expansion_wide_cadence_admits(
        &self,
        g: &Game,
        pid: usize,
        settlers: usize,
    ) -> bool {
        if settlers == 0 {
            return true;
        }
        self.expansion_wide_level(g).is_some()
            && settlers < WIDE_PARALLEL_SETTLERS
            && Self::expansion_wide_capital_is_busy(g, pid)
    }
}

#[cfg(test)]
mod tests;
