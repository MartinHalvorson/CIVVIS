//! The air surge: beeline Advanced Flight, arm a bomber wing, and take cities
//! with cavalry behind it.
//!
//! ★★★★ THE CONTROLLER NEVER REACHES THE AIR AT ALL. Every road to a late war
//! in this file walks the melee ladder: `choose_war_plan` builds its package
//! out of units that are `is_melee_capable()` and explicitly skips
//! `domain == "air"`, and its unlock ranking sorts by **cheapest remaining
//! research first**, so it can only ever appoint the next tech along — never a
//! three-step beeline. The engine, meanwhile, implements the whole air layer:
//! `Action::AirStrike`, `Action::AirPillage`, `Action::AirRebase`,
//! interception, and `air_slots` on the City Center and the Aerodrome. The
//! tactical layer already flies what it is given (`UnitDoctrine::AirStrike`,
//! `ForceRole::AirStrike`, `air_strike_value`). What is missing is everything
//! upstream of the first Bomber: nothing researches toward it, nothing builds
//! the Aerodrome that trains it, and nothing schedules the attack it enables.
//!
//! The surge supplies exactly that, as one appointment with four phases:
//!
//! | phase | what it owns |
//! |---|---|
//! | `Beeline` | research is forced along `flight → radio → advanced_flight`; the Aerodrome is claimed the turn `flight` lands; cavalry bodies prebuild |
//! | `Arm` | first two Bombers and two cavalry to launch; then the wing grows to [`AIR_SURGE_BOMBERS`] and the escort to [`AIR_SURGE_BODIES`] |
//! | `Strike` | the declaration |
//! | `Exploit` | the war, pressed on the appointed city |
//!
//! **Why this shape and not another tech.** The three techs are a real chain,
//! not a guess: `flight` unlocks the Aerodrome (`data/districts.json`,
//! `air_slots: 2`), `radio` requires `flight` and is what **reveals Aluminum**
//! (`data/resources.json`), and `advanced_flight` requires `radio` and unlocks
//! the Bomber — `bombard_strength` 110 at range 10, `siege: true`, so it
//! damages walls at full rate and never stands in the ZOC that the recorded
//! melee siege spends its life negotiating. The Aluminum the Bomber costs is
//! revealed exactly one tech before the Bomber itself, which is why the
//! appointment cannot price the metal up front and instead abandons the wing
//! if none has arrived [`AIR_SURGE_ALUMINUM_GRACE`] turns after the
//! breakthrough. Mining it needs no new code: `BasicAi::builder_step` already
//! takes an unopened strategic deposit before any other tile, at any distance.
//!
//! **Why cavalry behind it.** A Bomber cannot take a city; it can only empty
//! one. The capture bodies are the light and heavy cavalry lines by
//! preference — `promotion_class` `light_cavalry` / `heavy_cavalry` — because
//! their 4–5 movement is what turns an emptied city into a captured one inside
//! the window the wing keeps it emptied, and because `cavalry: true` means
//! they ignore the zone of control that the melee package negotiates tile by
//! tile. When the empire owns neither Horses nor Iron the surge falls back to
//! the strongest melee body it can actually build, and says so.
//!
//! **Bounded on purpose.** The surge is one appointment at a time, on one
//! objective city, and it stands down for the same reasons the timed war does:
//! the objective changes hands, the target dies, diplomacy makes the war
//! illegal, the launch slips past the endgame reserve, or home Recovery
//! persists. It never opens a second front — [`AdvancedAi::air_surge_open`]
//! refuses while any major war is already on.
//!
//! Off by default: registry row `air-surge`. It reaches the genome
//! unmeasured and stays off until a screen says it helps (`gene_ledger`);
//! `gene_screen --genes air-surge` prices it. ⚠ The domination lane is not a
//! free win here — `docs/eval/` records a pinned `live_target_domination`
//! losing 319 Elo while converting 22 domination victories — so this is
//! deliberately a *capability* the empire can reach, priced on its own, and
//! not a change to which victory the planner aims at.

use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::reasoning::plain;
use crate::think;
use crate::Pos;

/// The technology the whole appointment is built around.
pub(crate) const AIR_SURGE_GOAL_TECH: &str = "advanced_flight";
/// How far out the beeline may open, counted in technologies still missing on
/// the path to [`AIR_SURGE_GOAL_TECH`].
///
/// ★★★ THIS WAS THREE, AND THREE COULD NOT FIRE. The operator's "three tech
/// levels out" describes the *air chain* — `flight -> radio ->
/// advanced_flight` is exactly three — and it is the right description of an
/// empire that already holds the industrial spine beneath it. This one does
/// not. `air_surge_census_at_deployment_scale` on four 300-turn six-player
/// games recorded the closest approach the seat ever made:
///
/// | seed | closest | at turn | still missing |
/// |---|---|---|---|
/// | 941200 | 7 | 258 | flight, radio, industrialization, mass_production, square_rigging, steam_power |
/// | 941201 | 6 | 285 | flight, radio, industrialization, mass_production, steam_power |
/// | 941202 | 9 | 179 | + astronomy, scientific_theory |
/// | 941203 | 15 | 147 | + apprenticeship, banking, cartography, education, … |
///
/// Every seed is short of the same industrial branch, not of the air chain —
/// so a three-technology gate is a gate on a state the seat never reaches, and
/// the gene would have shipped provably inert. Twelve admits the measured
/// window while still refusing an ancient-era empire that would be beelining
/// twenty-five technologies; the binding gate is
/// [`AdvancedAi::air_surge_affordable`], which asks the question that actually
/// matters — whether the chain and the wing fit in the turns that remain.
pub(crate) const AIR_SURGE_TECH_HORIZON: usize = 12;
/// The sustainable wing ceiling. A Bomber consumes one Aluminum each turn, so
/// an empire making four Aluminum per turn can keep four in the air. The
/// actual target is capped by the income available after other Aluminum users;
/// see [`AdvancedAi::air_surge_bomber_goal`].
pub(crate) const AIR_SURGE_BOMBERS: usize = 4;
/// The first two Bombers are enough to empty a defended city in a turn. The
/// other two make the campaign resilient to interception and able to clear a
/// continent, but are not a reason to leave a vulnerable neighbour at peace.
pub(crate) const AIR_SURGE_LAUNCH_BOMBERS: usize = 2;
/// Cavalry bodies that walk in and take the cities the wing empties. Four are
/// the sustained campaign package, so a captured continent can be held.
pub(crate) const AIR_SURGE_BODIES: usize = 4;
/// Two fast capture bodies are enough to begin the campaign. The remaining
/// bodies continue to build behind the opening captures.
pub(crate) const AIR_SURGE_LAUNCH_BODIES: usize = 2;
/// Standard turns that must remain after the estimated launch, or the
/// appointment is not made and a live one stands down. The melee package
/// reserves forty; the wing needs less because it does not march.
pub(crate) const AIR_SURGE_ENDGAME_RESERVE: u32 = 30;
/// Standard turns the appointment waits for Aluminum after the breakthrough
/// before it gives the wing up. `radio` reveals the deposits one tech
/// earlier, so a Builder has had that long to reach one.
pub(crate) const AIR_SURGE_ALUMINUM_GRACE: u32 = 20;
/// How often the appointment re-reads the home situation, in standard turns.
pub(crate) const AIR_SURGE_REVIEW_CADENCE: u32 = 5;
/// After a surge stands down, no new one is appointed for this many standard
/// turns.
///
/// ⚠ Without this the lifecycle re-appoints inside the same call that aborted:
/// `maintain_air_surge` ends by opening a surge whenever none is live, so an
/// abort whose cause is still true — no Aluminum, no base in reach — is undone
/// immediately and re-recorded every turn. The abort census would then count
/// hundreds of stand-downs for one situation, and the empire would hold its
/// research and its queues for a wing it can never build.
pub(crate) const AIR_SURGE_ABORT_COOLDOWN: u32 = 15;
/// Consecutive reviews finding a threatened home city before the appointment
/// stands down.
pub(crate) const AIR_SURGE_RECOVERY_LIMIT: u8 = 2;
/// What a claimed package item is worth against the ordinary production
/// ranking. The melee package prices its breach element at 8_500 and its
/// bodies at 8_000; the wing sits between them so a live timed war keeps
/// first claim on a shared queue.
pub(crate) const AIR_SURGE_AERODROME_VALUE: f64 = 8_400.0;
pub(crate) const AIR_SURGE_BOMBER_VALUE: f64 = 8_200.0;
pub(crate) const AIR_SURGE_BODY_VALUE: f64 = 7_900.0;
/// Each still-missing member of the package is worth this much more than the
/// last, so a city that can finish one now outbids a city that would start a
/// second copy of what is already coming.
pub(crate) const AIR_SURGE_SCARCITY_STEP: f64 = 250.0;

/// Where the appointment has got to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AirSurgePhase {
    /// Researching toward [`AIR_SURGE_GOAL_TECH`].
    Beeline,
    /// The breakthrough is in; the wing and its escort are being built.
    Arm,
    /// The package is ready and the declaration is owed.
    Strike,
    /// The war is on.
    Exploit,
}

impl AirSurgePhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Beeline => "beeline",
            Self::Arm => "arm",
            Self::Strike => "strike",
            Self::Exploit => "exploit",
        }
    }
}

/// The one appointed air surge.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AirSurge {
    pub(crate) target_player: usize,
    /// The current board's ephemeral city id. A fresh live mirror rebuilds
    /// these ids every frame, so it is refreshed from [`Self::objective_pos`]
    /// before the plan is evaluated.
    pub(crate) objective_city: u32,
    /// The durable identity of the objective across a fresh-board rebuild.
    /// City centres do not move, unlike the reconstruction's sequential ids.
    pub(crate) objective_pos: Pos,
    /// The capture body this surge builds and upgrades toward.
    pub(crate) body_unit: Name,
    /// Whether [`Self::body_unit`] is a light or heavy cavalry line. False
    /// means the empire could field no cavalry and the surge fell back to its
    /// strongest melee body.
    pub(crate) body_is_cavalry: bool,
    /// Whether the appointment was made *into* a war already running.
    ///
    /// ★★★ THIS IS THE COUNTER-ATTACK HALF. A Bomber is as useful against a
    /// civilization attacking us as against one we chose, and the empire that
    /// most needs a wing is the one already being invaded — but an
    /// appointment made at war has no declaration to make, so every gate that
    /// reads `declared_turn` would otherwise treat it as a plan whose war
    /// somebody else opened and stand it down on the first pass.
    pub(crate) opened_at_war: bool,
    pub(crate) phase: AirSurgePhase,
    pub(crate) appointed_turn: u32,
    /// The turn [`AIR_SURGE_GOAL_TECH`] landed.
    pub(crate) tech_turn: Option<u32>,
    pub(crate) declared_turn: Option<u32>,
    pub(crate) last_reviewed_turn: u32,
    pub(crate) recovery_assessments: u8,
}

/// What the package looks like on the board right now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct AirSurgeStatus {
    /// Cities holding a finished Aerodrome.
    pub(crate) aerodromes: usize,
    /// Aerodromes standing or in a queue.
    pub(crate) aerodromes_committed: usize,
    pub(crate) bombers: usize,
    pub(crate) bombers_committed: usize,
    pub(crate) bodies: usize,
    pub(crate) bodies_committed: usize,
    /// A Bomber based in one of our cities can reach the objective.
    pub(crate) wing_in_range: bool,
    /// Aluminum can support the two-Bomber launch wing, either sustainably
    /// or from a banked shortfall reserve.
    pub(crate) metal_ready: bool,
}

impl AirSurgeStatus {
    /// A strike wing is built and can reach the city it was built for. The
    /// remaining Bombers are a follow-through package, not a launch delay.
    pub(crate) fn wing_ready(&self) -> bool {
        self.bombers >= AIR_SURGE_LAUNCH_BOMBERS && self.wing_in_range
    }

    /// The initial escort that takes the city the wing empties.
    pub(crate) fn escort_ready(&self) -> bool {
        self.bodies >= AIR_SURGE_LAUNCH_BODIES
    }
}

/// Running account of what the surge did, for the journal and the screens.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AirSurgeCensus {
    pub(crate) appointments: u32,
    pub(crate) breakthroughs: u32,
    pub(crate) declarations: u32,
    pub(crate) objectives_captured: u32,
    pub(crate) aborts: std::collections::BTreeMap<&'static str, u32>,
}

impl AdvancedAi {
    /// Whether an appointment is live. Read by the production reservation and
    /// by the strategy overlay; false whenever the gene is off.
    pub(crate) fn air_surge_active(&self) -> bool {
        self.air_surge_plan.is_some()
    }

    /// Whether any version of the family is on. `air-surge-2` plays INSTEAD
    /// of version one (its enable turns `air_surge` off, the way
    /// `science_victory_drive_2` replaces its v1), so every gate that used to
    /// read the v1 flag reads this instead.
    pub(crate) fn air_surge_enabled(&self) -> bool {
        self.air_surge || self.air_surge_2
    }

    /// `air-surge-2`: the target the diplomacy pass should consult when the
    /// assessment supplies none.
    ///
    /// ★★★ THE FORMAL-WAR CLOCK NEVER RAN ON A LANE SEAT. The Arm-phase
    /// denounce below (`air_surge_opening`) exists so the five-turn Formal War
    /// countdown and the buildout run together — but `advanced_diplomacy`
    /// reaches it through `plan.target_player`, and the strategy overlay only
    /// writes that in Strike/Exploit. A seat on a victory lane assesses no
    /// rival at all, so through Beeline and Arm the opening was never called,
    /// the denounce never happened, and the declaration paid the whole clock
    /// AFTER the wing was ready. Version two hands the diplomacy pass the
    /// surge's own target whenever the assessment names nobody.
    pub(crate) fn air_surge_diplomacy_target(&self) -> Option<usize> {
        if !self.air_surge_2 {
            return None;
        }
        let surge = self.air_surge_plan.as_ref()?;
        // A counter appointed into a running war has nothing to open, so it
        // borrows no decision the assessment did not give it.
        (!surge.opened_at_war).then_some(surge.target_player)
    }

    /// How many technologies still stand between the empire and the Bomber.
    /// Zero means it is already unlocked.
    pub(crate) fn air_surge_missing_techs(g: &Game, pid: usize) -> usize {
        let held = &g.players[pid].techs;
        let mut missing = usize::from(!held.contains(&Name::new(AIR_SURGE_GOAL_TECH)));
        if let Some(ancestors) = g.rules.tech_ancestors.get(AIR_SURGE_GOAL_TECH) {
            missing += ancestors
                .iter()
                .filter(|tech| !held.contains(&Name::new(tech)))
                .count();
        }
        missing
    }

    /// The Bomber this empire would actually train, unique replacements
    /// included. `None` when the ruleset has no such unit at all, which is how
    /// a mod that removes the air layer switches the whole gene off.
    pub(crate) fn air_surge_bomber(g: &Game, pid: usize) -> Option<Name> {
        Self::player_unit_catalog(g, pid).into_iter().find(|unit| {
            let spec = &g.rules.units[*unit];
            spec.domain.as_deref() == Some("air")
                && spec.promotion_class == "air_bomber"
                && spec.tech == Some(Name::new(AIR_SURGE_GOAL_TECH))
        })
    }

    /// The district a Bomber has to be trained in.
    fn air_surge_field(g: &Game, pid: usize) -> Option<Name> {
        let bomber = Self::air_surge_bomber(g, pid)?;
        g.rules.units[bomber].requires_district
    }

    /// The capture body: the strongest cavalry the empire can build today,
    /// else the strongest land melee body it can build today. Returns the
    /// unit and whether it is cavalry, so the journal can say which happened.
    pub(crate) fn air_surge_body(g: &Game, pid: usize) -> Option<(Name, bool)> {
        let buildable = |unit: &Name| {
            let spec = &g.rules.units[*unit];
            spec.class == "military"
                && spec.is_melee_capable()
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && Self::war_unit_unlocked(g, pid, *unit)
        };
        let strongest = |cavalry_only: bool| {
            Self::player_unit_catalog(g, pid)
                .into_iter()
                .filter(buildable)
                .filter(|unit| {
                    !cavalry_only
                        || matches!(
                            g.rules.units[*unit].promotion_class.as_str(),
                            "light_cavalry" | "heavy_cavalry"
                        )
                })
                .max_by(|left, right| {
                    g.rules.units[*left]
                        .strength
                        .total_cmp(&g.rules.units[*right].strength)
                        .then_with(|| right.cmp(left))
                })
        };
        strongest(true)
            .map(|unit| (unit, true))
            .or_else(|| strongest(false).map(|unit| (unit, false)))
    }

    /// Every city of ours that can base an aircraft: the City Center carries
    /// one slot on its own, so a Bomber has somewhere to sit before the
    /// Aerodrome finishes, and the Aerodrome adds two more.
    fn air_surge_bases(g: &Game, pid: usize) -> Vec<Pos> {
        g.player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect()
    }

    /// Whether a Bomber based in one of our cities can strike `objective`.
    fn air_surge_in_range(g: &Game, pid: usize, objective: Pos) -> bool {
        let Some(bomber) = Self::air_surge_bomber(g, pid) else {
            return false;
        };
        let range = g.rules.units[bomber].range;
        Self::air_surge_bases(g, pid)
            .into_iter()
            .any(|base| g.wdist(base, objective) <= range)
    }

    /// The number of Bombers this economy can keep in the air. Four is the
    /// ceiling because a fourth plane is useful on a continent-wide campaign,
    /// while an empire's actual Aluminum income and existing consumers decide
    /// whether that ceiling is affordable.
    ///
    /// A large stockpile can bridge a shortfall for the Aluminum grace period,
    /// but only for the two-plane launch wing. It is not mistaken for a
    /// permanent four-plane income.
    pub(crate) fn air_surge_bomber_goal(g: &Game, pid: usize) -> usize {
        let Some(bomber) = Self::air_surge_bomber(g, pid) else {
            return 0;
        };
        let spec = &g.rules.units[bomber];
        let Some(resource) = spec.requires_resource else {
            return AIR_SURGE_BOMBERS;
        };
        if spec.resource_maintenance <= f64::EPSILON {
            return AIR_SURGE_BOMBERS;
        }

        let other_demand = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind != bomber && !unit.free_upkeep)
            .filter_map(|unit| {
                let unit_spec = &g.rules.units[unit.kind];
                (unit_spec.requires_resource == Some(resource))
                    .then_some(unit_spec.resource_maintenance)
            })
            .sum::<f64>();
        let income = (g.strategic_resource_rate(pid, resource.as_str()) - other_demand).max(0.0);
        let sustainable = (income / spec.resource_maintenance).floor() as usize;
        let sustainable = sustainable.min(AIR_SURGE_BOMBERS);
        if sustainable >= AIR_SURGE_LAUNCH_BOMBERS {
            return sustainable;
        }

        // The deposit may be pillaged, traded away, or still be on a Builder's
        // route. A bank can still pay for an immediate two-Bomber strike, but
        // it must cover their training cost and the whole bounded wait for a
        // replacement source; otherwise this is not a viable wing at all.
        let launch_maintenance = AIR_SURGE_LAUNCH_BOMBERS as f64 * spec.resource_maintenance;
        let grace = g.standard_duration(AIR_SURGE_ALUMINUM_GRACE) as f64;
        let temporary_cost = AIR_SURGE_LAUNCH_BOMBERS as f64 * spec.resource_cost
            + (launch_maintenance - income).max(0.0) * grace;
        if g.strategic_stockpile(pid, resource) + f64::EPSILON >= temporary_cost {
            AIR_SURGE_LAUNCH_BOMBERS
        } else {
            sustainable
        }
    }

    /// Aluminum enough to train and keep the launch wing alive.
    fn air_surge_metal_ready(g: &Game, pid: usize) -> bool {
        Self::air_surge_bomber_goal(g, pid) >= AIR_SURGE_LAUNCH_BOMBERS
    }

    /// The package as the board holds it. Queues count, so the first city that
    /// commits a Bomber removes that vacancy for the next city rather than
    /// every city starting the same wish list.
    pub(crate) fn air_surge_status(&self, g: &Game, pid: usize, plan: &AirSurge) -> AirSurgeStatus {
        let field = Self::air_surge_field(g, pid);
        let bomber = Self::air_surge_bomber(g, pid);
        let mut status = AirSurgeStatus {
            wing_in_range: Self::air_surge_in_range(g, pid, plan.objective_pos),
            metal_ready: Self::air_surge_metal_ready(g, pid),
            ..AirSurgeStatus::default()
        };
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            let built = field.is_some_and(|family| g.city_has_district_family(city, family));
            let queued = field.is_some_and(|family| {
                city.queue.iter().any(|item| {
                    matches!(item, Item::District { district, .. }
                             if g.district_family(*district) == family)
                })
            });
            status.aerodromes += usize::from(built);
            status.aerodromes_committed += usize::from(built || queued);
            for item in &city.queue {
                let Some(unit) = (match item {
                    Item::Unit { unit } => Some(*unit),
                    Item::Formation { unit, .. } => Some(*unit),
                    _ => None,
                }) else {
                    continue;
                };
                if Some(unit) == bomber {
                    status.bombers_committed += 1;
                } else if Self::war_unit_is_at_least(g, pid, unit, plan.body_unit) {
                    status.bodies_committed += 1;
                }
            }
        }
        for uid in g.player_unit_ids(pid) {
            let kind = g.units[&uid].kind;
            if Some(kind) == bomber {
                status.bombers += 1;
            } else if Self::war_unit_is_at_least(g, pid, kind, plan.body_unit) {
                status.bodies += 1;
            }
        }
        status.bombers_committed += status.bombers;
        status.bodies_committed += status.bodies;
        status
    }

    /// Whether the empire may open a surge at all this turn.
    ///
    /// Deliberately the same shape as `may_form_war_plan`: one appointment,
    /// no second front, no surge while the homeland is already in trouble, and
    /// nothing that cannot finish before the endgame reserve.
    pub(crate) fn air_surge_open(&self, g: &Game, pid: usize) -> bool {
        if !self.air_surge_enabled()
            || self.air_surge_plan.is_some()
            // A stand-down whose cause is still true must not be undone by the
            // very call that recorded it. See `AIR_SURGE_ABORT_COOLDOWN`.
            || g.turn < self.air_surge_cooldown_until
            // The melee appointment has the same claim on the empire's idle
            // queues; whichever was appointed first keeps them.
            || self.war_plan.is_some()
            || g.turn < self.peace_until
            || g.player_city_ids(pid).len() < 2
            || self.threatened_city(g, pid).is_some()
            || g.emergency_objective(pid).is_some()
            || Self::air_surge_bomber(g, pid).is_none()
        {
            return false;
        }
        let reserve = g.standard_duration(AIR_SURGE_ENDGAME_RESERVE);
        if g.turn.saturating_add(reserve) >= g.max_turns {
            return false;
        }
        if Self::air_surge_missing_techs(g, pid) > AIR_SURGE_TECH_HORIZON
            || !self.air_surge_affordable(g, pid)
        {
            return false;
        }
        // At most one front. With none, the surge is the elective attack the
        // operator asked for; with exactly one, it is the counter — the wing
        // arms against the civilization already fighting us and needs no
        // declaration at all. Two fronts is a war the empire is losing, and a
        // three-technology beeline is not the answer to it.
        Self::air_surge_fronts(g, pid).len() <= 1
    }

    /// The majors already at war with us.
    fn air_surge_fronts(g: &Game, pid: usize) -> Vec<usize> {
        g.players
            .iter()
            .filter(|player| {
                player.id != pid
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && g.is_at_war(pid, player.id)
            })
            .map(|player| player.id)
            .collect()
    }

    /// Research turns to the breakthrough, and production turns for the
    /// package the empire does not yet hold, at today's science and production.
    ///
    /// Deliberately the same arithmetic the melee appointment uses
    /// (`war_remaining_research_cost` over `war_science_per_turn`), so the two
    /// lanes agree about what "in time" means. The wing does not march, so
    /// there is no third term.
    pub(crate) fn air_surge_launch_estimate(&self, g: &Game, pid: usize) -> (u32, u32) {
        let research = (Self::war_remaining_research_cost(g, pid, Name::new(AIR_SURGE_GOAL_TECH))
            / Self::war_science_per_turn(g, pid))
        .ceil() as u32;
        // `air-surge-2` prices only the package still missing. Version one
        // re-prices the whole wing from scratch every time, which is honest
        // for the first appointment and wrong for every one after it: the
        // empire that just took a city with three Bombers alive is told a
        // second surge costs another full wing, and the endgame reserve
        // refuses the follow-up exactly when it is nearly free. This is the
        // arithmetic that lets the loop repeat.
        let (missing_field, missing_bombers, missing_bodies) = if self.air_surge_2 {
            let standing = self.air_surge_standing_package(g, pid);
            (
                usize::from(standing.0 == 0),
                AIR_SURGE_LAUNCH_BOMBERS.saturating_sub(standing.1),
                AIR_SURGE_LAUNCH_BODIES.saturating_sub(standing.2),
            )
        } else {
            (1, AIR_SURGE_LAUNCH_BOMBERS, AIR_SURGE_LAUNCH_BODIES)
        };
        let field_cost = Self::air_surge_field(g, pid)
            .map(|field| g.rules.districts[field].cost)
            .unwrap_or(0.0)
            * missing_field as f64;
        let wing_cost = Self::air_surge_bomber(g, pid)
            .map(|bomber| g.rules.units[bomber].cost)
            .unwrap_or(0.0)
            * missing_bombers as f64;
        let escort_cost = Self::air_surge_body(g, pid)
            .map(|(body, _)| g.rules.units[body].cost)
            .unwrap_or(0.0)
            * missing_bodies as f64;
        let production = ((field_cost + wing_cost + escort_cost)
            / Self::war_production_per_turn(g, pid))
        .ceil() as u32;
        (research, production)
    }

    /// The package members already standing, counted without an appointment:
    /// `(airfields, bombers, escort bodies)`. The estimate above runs before
    /// any plan exists, so unlike [`Self::air_surge_status`] this cannot read
    /// a plan's chosen body and asks [`Self::air_surge_body`] what the escort
    /// would be today.
    fn air_surge_standing_package(&self, g: &Game, pid: usize) -> (usize, usize, usize) {
        let field = Self::air_surge_field(g, pid);
        let bomber = Self::air_surge_bomber(g, pid);
        let body = Self::air_surge_body(g, pid);
        let airfields = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                field.is_some_and(|family| g.city_has_district_family(&g.cities[cid], family))
            })
            .count();
        let mut bombers = 0;
        let mut bodies = 0;
        for uid in g.player_unit_ids(pid) {
            let kind = g.units[&uid].kind;
            if Some(kind) == bomber {
                bombers += 1;
            } else if body.is_some_and(|(unit, _)| Self::war_unit_is_at_least(g, pid, kind, unit)) {
                bodies += 1;
            }
        }
        (airfields, bombers, bodies)
    }

    /// Whether the whole appointment — the chain and the package — fits in the
    /// turns that remain, with the endgame reserve left over.
    ///
    /// ★★★★ THIS IS THE GATE THAT MATTERS, and the census is why. The seat
    /// reaches its closest approach to Advanced Flight at turn 258 of 300 with
    /// **79 turns of research still to pay** (seed 941200); at turn 285 with
    /// 80 (941201). A gate on the technology count alone would have appointed
    /// both — a beeline that spends the last forty turns of the game
    /// researching a unit it can never build, having given up the lane the
    /// empire was actually winning. Asking whether it fits refuses exactly
    /// those and admits the games where the wing can genuinely fly.
    pub(crate) fn air_surge_affordable(&self, g: &Game, pid: usize) -> bool {
        let (research, production) = self.air_surge_launch_estimate(g, pid);
        let reserve = g.standard_duration(AIR_SURGE_ENDGAME_RESERVE);
        g.turn
            .saturating_add(research)
            .saturating_add(production)
            .saturating_add(reserve)
            < g.max_turns
    }

    /// Pick the target and the objective city: the best campaign-valued city
    /// of a legal major that a Bomber based at home can reach and a land body
    /// can walk to.
    pub(crate) fn choose_air_surge(&self, g: &Game, pid: usize) -> Option<AirSurge> {
        let (body_unit, body_is_cavalry) = Self::air_surge_body(g, pid)?;
        // A running war fixes the target: the counter arms against the
        // civilization already fighting us, never against a third party.
        let front = Self::air_surge_fronts(g, pid).first().copied();
        let mut best: Option<(f64, usize, u32, AirSurge)> = None;
        for target in g.players.iter().filter(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && front.is_none_or(|enemy| player.id == enemy)
                && self.campaign_target_legal(g, pid, player.id)
        }) {
            let mut objectives: Vec<_> = g
                .cities
                .values()
                .filter(|city| city.owner == target.id)
                .collect();
            objectives.sort_by(|left, right| {
                self.campaign_city_value(g, pid, left, GrandStrategy::Conquest)
                    .total_cmp(&self.campaign_city_value(g, pid, right, GrandStrategy::Conquest))
                    .then(left.id.cmp(&right.id))
            });
            for city in objectives {
                // The wing has to be able to hit it from home, and the escort
                // has to be able to walk to it. Either alone is half a surge.
                if !Self::air_surge_in_range(g, pid, city.pos)
                    || self
                        .war_staging_route(g, pid, target.id, city.pos)
                        .is_none()
                {
                    continue;
                }
                let score = self.campaign_city_value(g, pid, city, GrandStrategy::Conquest);
                let plan = AirSurge {
                    target_player: target.id,
                    objective_city: city.id,
                    objective_pos: city.pos,
                    body_unit,
                    body_is_cavalry,
                    opened_at_war: front == Some(target.id),
                    // A counter is fighting already, and production and
                    // diplomacy both read the phase later in this same turn.
                    // The lifecycle recomputes it every turn afterwards.
                    phase: if front == Some(target.id) {
                        AirSurgePhase::Exploit
                    } else {
                        AirSurgePhase::Beeline
                    },
                    appointed_turn: g.turn,
                    tech_turn: None,
                    declared_turn: None,
                    last_reviewed_turn: g.turn,
                    recovery_assessments: 0,
                };
                if best.as_ref().is_none_or(|(old, ot, oc, _)| {
                    score < *old || (score == *old && (target.id, city.id) < (*ot, *oc))
                }) {
                    best = Some((score, target.id, city.id, plan));
                }
                // The objective list is already campaign-ranked; the first
                // reachable city is this target's attack.
                break;
            }
        }
        best.map(|(_, _, _, plan)| plan)
    }

    fn record_air_surge_abort(&mut self, g: &Game, reason: &'static str) {
        *self.air_surge_census.aborts.entry(reason).or_default() += 1;
        self.air_surge_cooldown_until = g
            .turn
            .saturating_add(g.standard_duration(AIR_SURGE_ABORT_COOLDOWN));
        if self.journal().wants(crate::reasoning::Level::Strategy) {
            think!(self.journal(), Military, Strategy, "Standing down the air surge"; "{reason}");
        }
    }

    /// Validate and advance the one appointed surge before any subsystem acts.
    /// This is the lifecycle authority: research, production, diplomacy and
    /// movement only read the resulting phase.
    pub(crate) fn maintain_air_surge(&mut self, g: &Game, pid: usize) {
        if !self.air_surge_enabled() {
            self.air_surge_plan = None;
            self.air_surge_status = AirSurgeStatus::default();
            return;
        }

        if let Some(mut plan) = self.air_surge_plan.take() {
            let target_alive = g
                .players
                .get(plan.target_player)
                .is_some_and(|target| target.alive);
            // Fresh-board live play recreates every city in the order the host
            // happened to export it this frame. That changes `City::id` even
            // while Pella, its owner, and its location are all unchanged.
            // Resolve by the City Center's stable coordinate before treating a
            // missing id as a capture; otherwise a sound bomber campaign dies
            // the turn after it was appointed.
            let objective_city = g.city_at(plan.objective_pos);
            let objective_owner = objective_city
                .and_then(|city| g.cities.get(&city))
                .map(|city| city.owner);
            if objective_owner == Some(plan.target_player) {
                // The owner above could only have come from this current
                // coordinate's city, so the id is safe to carry through the
                // rest of this board's tactical and diplomacy passes.
                plan.objective_city = objective_city.unwrap_or(plan.objective_city);
            }
            let at_war = target_alive && g.is_at_war(pid, plan.target_player);
            let mut ended = false;
            if objective_owner != Some(plan.target_player) {
                if plan.declared_turn.is_some() && objective_owner == Some(pid) {
                    self.air_surge_census.objectives_captured += 1;
                    think!(self.journal(), Military, Strategy,
                           "The air surge has taken its city";
                           "{} turns from appointment, {} from declaration",
                           g.turn.saturating_sub(plan.appointed_turn),
                           plan.declared_turn.map_or(0, |turn| g.turn.saturating_sub(turn)));
                } else {
                    self.record_air_surge_abort(g, "objective changed owner");
                }
                ended = true;
            } else if !target_alive {
                self.record_air_surge_abort(g, "target no longer alive");
                ended = true;
            } else if !at_war && !self.campaign_target_legal(g, pid, plan.target_player) {
                self.record_air_surge_abort(g, "diplomacy made war illegal");
                ended = true;
            } else if at_war && plan.declared_turn.is_none() && !plan.opened_at_war {
                self.record_air_surge_abort(g, "target opened the war first");
                ended = true;
            } else if (plan.declared_turn.is_some() || plan.opened_at_war) && !at_war {
                self.record_air_surge_abort(g, "peace closed the war");
                ended = true;
            }

            if !ended {
                let tech_owned = g.players[pid]
                    .techs
                    .contains(&Name::new(AIR_SURGE_GOAL_TECH));
                if tech_owned && plan.tech_turn.is_none() {
                    plan.tech_turn = Some(g.turn);
                    self.air_surge_census.breakthroughs += 1;
                }
                let status = self.air_surge_status(g, pid, &plan);
                let reserve = g.standard_duration(AIR_SURGE_ENDGAME_RESERVE);
                let grace = g.standard_duration(AIR_SURGE_ALUMINUM_GRACE);
                let fighting = plan.declared_turn.is_some() || plan.opened_at_war;
                if !fighting && g.turn.saturating_add(reserve) >= g.max_turns {
                    self.record_air_surge_abort(g, "the surge slipped past the endgame reserve");
                    ended = true;
                } else if !fighting && !status.wing_in_range {
                    // Cities are lost and founded; a surge whose objective no
                    // longer sits under any of our airfields is not a surge.
                    self.record_air_surge_abort(g, "no base left within the wing's reach");
                    ended = true;
                } else if plan
                    .tech_turn
                    .is_some_and(|turn| g.turn.saturating_sub(turn) >= grace)
                    && status.bombers == 0
                    && !status.metal_ready
                {
                    self.record_air_surge_abort(g, "no Aluminum for the wing");
                    ended = true;
                } else {
                    let cadence = g.standard_duration(AIR_SURGE_REVIEW_CADENCE).max(1);
                    if g.turn.saturating_sub(plan.last_reviewed_turn) >= cadence {
                        plan.last_reviewed_turn = g.turn;
                        if self.threatened_city(g, pid).is_some() {
                            plan.recovery_assessments = plan.recovery_assessments.saturating_add(1);
                        } else {
                            plan.recovery_assessments = 0;
                        }
                    }
                    if !fighting && plan.recovery_assessments >= AIR_SURGE_RECOVERY_LIMIT {
                        self.record_air_surge_abort(
                            g,
                            "home Recovery persisted for two assessments",
                        );
                        ended = true;
                    } else if at_war {
                        plan.phase = AirSurgePhase::Exploit;
                    } else if !tech_owned {
                        plan.phase = AirSurgePhase::Beeline;
                    } else if !(status.wing_ready() && status.escort_ready()) {
                        plan.phase = AirSurgePhase::Arm;
                    } else {
                        plan.phase = AirSurgePhase::Strike;
                    }
                }
                if !ended {
                    self.air_surge_status = status;
                    self.air_surge_plan = Some(plan);
                }
            }
            if ended {
                self.air_surge_plan = None;
                self.air_surge_status = AirSurgeStatus::default();
            }
        }

        if self.air_surge_plan.is_none() && self.air_surge_open(g, pid) {
            if let Some(plan) = self.choose_air_surge(g, pid) {
                self.air_surge_census.appointments += 1;
                if let Some(city) = g.cities.get(&plan.objective_city) {
                    think!(self.journal(), Military, Strategy,
                           "Appointing an air surge against {}", city.name;
                           "beeline {} ({} techs out), raise an {} and {} {}s, take the city with {} {}{}",
                           plain(AIR_SURGE_GOAL_TECH),
                           Self::air_surge_missing_techs(g, pid),
                           Self::air_surge_field(g, pid)
                               .map(|field| plain(field.as_str()))
                               .unwrap_or_else(|| "airfield".to_string()),
                           Self::air_surge_bomber_goal(g, pid),
                           Self::air_surge_bomber(g, pid)
                               .map(|unit| plain(unit.as_str()))
                               .unwrap_or_else(|| "bomber".to_string()),
                           AIR_SURGE_BODIES,
                           plain(plan.body_unit.as_str()),
                           if plan.body_is_cavalry { "" } else { " (no cavalry available)" });
                }
                self.air_surge_status = self.air_surge_status(g, pid, &plan);
                self.air_surge_plan = Some(plan);
            }
        }
    }

    /// The forced research goal while the breakthrough is still missing.
    /// Consumed by `advanced_research`, which walks the cheapest legal step
    /// toward it.
    ///
    /// ⚠ Keyed off the technology, not off [`AirSurgePhase::Beeline`]. A
    /// counter appointed into a running war sits in `Exploit` from its first
    /// turn — and it is precisely the appointment that most needs the beeline,
    /// because the wing is what answers the invasion. The goal retires when
    /// the technology lands, which is the only condition that ever mattered.
    pub(crate) fn air_surge_research_goal(&self, g: &Game, pid: usize) -> Option<&'static str> {
        self.air_surge_plan.as_ref()?;
        (!g.players[pid]
            .techs
            .contains(&Name::new(AIR_SURGE_GOAL_TECH)))
        .then_some(AIR_SURGE_GOAL_TECH)
    }

    /// Once the package is ready the surge owns the grand strategy, exactly
    /// as an appointed timed war does. It deliberately does **not** own it
    /// while the beeline and the buildout run: three techs of Conquest posture
    /// would pay for the wing with the economy that has to build it.
    pub(crate) fn apply_air_surge_to_strategy(&self, plan: &mut StrategicPlan) {
        let Some(surge) = &self.air_surge_plan else {
            return;
        };
        if !matches!(surge.phase, AirSurgePhase::Strike | AirSurgePhase::Exploit) {
            return;
        }
        if plan.strategy != GrandStrategy::Recovery {
            plan.strategy = GrandStrategy::Conquest;
            plan.target_player = Some(surge.target_player);
            plan.target_city = Some(surge.objective_city);
            plan.rush = false;
        }
    }

    /// What one candidate item is worth to the appointed surge, or `None` when
    /// the item is not part of the package. Read by `production_value`, so the
    /// package outbids the ordinary ranking wherever the strategic scorer runs.
    pub(crate) fn air_surge_production_value(
        &self,
        g: &Game,
        pid: usize,
        item: &Item,
        turns: f64,
    ) -> Option<f64> {
        let plan = self.air_surge_plan.as_ref()?;
        let status = self.air_surge_status;
        let bomber_goal = Self::air_surge_bomber_goal(g, pid);
        match item {
            Item::District { district, .. }
                if Self::air_surge_field(g, pid)
                    .is_some_and(|field| g.district_family(*district) == field) =>
            {
                // One airfield is the whole requirement: the Bomber is trained
                // in the city that holds it and rebases from there at twice
                // its movement.
                (status.aerodromes_committed == 0)
                    .then_some(AIR_SURGE_AERODROME_VALUE - turns * 8.0)
            }
            Item::Unit { unit } | Item::Formation { unit, .. } => {
                if Self::air_surge_bomber(g, pid) == Some(*unit) {
                    let missing = bomber_goal.saturating_sub(status.bombers_committed);
                    (missing > 0).then_some(
                        AIR_SURGE_BOMBER_VALUE + AIR_SURGE_SCARCITY_STEP * missing as f64
                            - turns * 8.0,
                    )
                } else if status.metal_ready
                    && Self::war_unit_is_at_least(g, pid, *unit, plan.body_unit)
                {
                    let missing = AIR_SURGE_BODIES.saturating_sub(status.bodies_committed);
                    (missing > 0).then_some(
                        AIR_SURGE_BODY_VALUE + AIR_SURGE_SCARCITY_STEP * missing as f64
                            - turns * 8.0,
                    )
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Claim one idle city queue for the next missing member of the package.
    ///
    /// The adaptive controller hands empty cities to `BasicAi::cities`, which
    /// never consults `production_value` at all — so without this the surge
    /// would price a Bomber nobody ever asked it about. Same claim discipline
    /// as `prioritize_envoy_infrastructure`: one idle, unthreatened queue per
    /// turn, and an exact no-op while the gene is off.
    pub(crate) fn air_surge_production(&mut self, g: &mut Game, pid: usize) -> bool {
        let Some(plan) = self.air_surge_plan.clone() else {
            return false;
        };
        let status = self.air_surge_status;
        let threatened = self.threatened_city(g, pid);
        let field = Self::air_surge_field(g, pid);
        let bomber = Self::air_surge_bomber(g, pid);
        // The airfield first: nothing else in the package can be trained
        // until one city holds it. Then the wing, then the escort that takes
        // the city the wing empties.
        let wants_field = status.aerodromes_committed == 0;
        let bomber_goal = Self::air_surge_bomber_goal(g, pid);
        let wants_bomber = status.bombers_committed < bomber_goal;
        let wants_body = status.metal_ready && status.bodies_committed < AIR_SURGE_BODIES;
        if !wants_field && !wants_bomber && !wants_body {
            return false;
        }
        let remaining = g.max_turns.saturating_sub(g.turn) as f64;
        let mut best: Option<(u8, f64, u32, String, Item)> = None;
        for cid in g.player_city_ids(pid) {
            if !g.cities[&cid].queue.is_empty() || threatened == Some(cid) {
                continue;
            }
            let production = g.city_yields(cid).production.max(0.1);
            for item in g.producible_items(pid, cid) {
                let rank = match &item {
                    Item::District { district, .. }
                        if wants_field
                            && field
                                .is_some_and(|family| g.district_family(*district) == family) =>
                    {
                        0
                    }
                    Item::Unit { unit } if wants_bomber && Some(*unit) == bomber => 1,
                    Item::Unit { unit }
                        if wants_body
                            && Self::war_unit_is_at_least(g, pid, *unit, plan.body_unit) =>
                    {
                        2
                    }
                    _ => continue,
                };
                let build_turns = g.item_remaining_cost_for_city(pid, cid, &item) / production;
                if build_turns > remaining + f64::EPSILON {
                    continue;
                }
                let key = format!("{item:?}");
                let candidate = (rank, build_turns, cid, key, item);
                if best.as_ref().is_none_or(|old| {
                    (candidate.0, candidate.1, candidate.2, &candidate.3)
                        < (old.0, old.1, old.2, &old.3)
                }) {
                    best = Some(candidate);
                }
            }
        }
        let Some((_, build_turns, city, _, item)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_err()
        {
            return false;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let city_name = g.cities[&city].name.clone();
            think!(self.journal(), Military, Decision,
                   "{} starts {} for the air surge", city_name, Self::plain_item(&item);
                   "{:.0} turns; the surge holds {}/{} bombers ({} to launch) and {}/{} {}s ({} to launch) in the {} phase",
                   build_turns,
                   self.air_surge_status.bombers, Self::air_surge_bomber_goal(g, pid),
                   AIR_SURGE_LAUNCH_BOMBERS,
                   self.air_surge_status.bodies, AIR_SURGE_BODIES, AIR_SURGE_LAUNCH_BODIES,
                   plain(plan.body_unit.as_str()),
                   plan.phase.as_str());
        }
        self.air_surge_status = self.air_surge_status(g, pid, &plan);
        true
    }

    /// Own the diplomatic end of the surge. Returning `true` means the
    /// appointment consumed the war-opening decision this turn, even when it
    /// deliberately held.
    pub(crate) fn air_surge_opening(&mut self, g: &mut Game, pid: usize, target: usize) -> bool {
        let Some(plan) = self
            .air_surge_plan
            .as_ref()
            .filter(|plan| plan.target_player == target)
            .cloned()
        else {
            return false;
        };
        if self.urgent_victory_threat(g, target) {
            self.record_air_surge_abort(g, "victory denial superseded the surge");
            self.air_surge_plan = None;
            self.air_surge_status = AirSurgeStatus::default();
            return false;
        }
        if plan.opened_at_war {
            // Nothing to open. The wing arms and fights under the ordinary
            // wartime layers; the surge only supplies the objective.
            return false;
        }
        if plan.phase != AirSurgePhase::Strike {
            // Denounce while the wing is being built, so the Formal War clock
            // and the buildout run together. The casus belli itself is never
            // exercised before the package is ready.
            if plan.phase == AirSurgePhase::Arm {
                if let Some(action @ Action::Denounce { .. }) =
                    self.preferred_war_opening(g, pid, target)
                {
                    let _ = g.apply(pid, &action);
                }
            }
            return true;
        }
        // Phase is recomputed at turn start; a deal or a loss earlier this
        // turn can have changed the board, so the gate is repeated here.
        let status = self.air_surge_status(g, pid, &plan);
        if !(status.wing_ready() && status.escort_ready()) || self.threatened_city(g, pid).is_some()
        {
            return true;
        }
        let Some(action) = self.preferred_war_opening(g, pid, target) else {
            return true;
        };
        let declares = matches!(
            action,
            Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
        );
        if g.apply(pid, &action).is_err() || !declares {
            return true;
        }
        if let Some(active) = self.air_surge_plan.as_mut() {
            active.declared_turn = Some(g.turn);
            active.phase = AirSurgePhase::Exploit;
        }
        self.air_surge_census.declarations += 1;
        if let Some(city) = g.cities.get(&plan.objective_city) {
            think!(self.journal(), Military, Strategy,
                   "The air surge opens on {}", city.name;
                   "{} bombers in range and {} {}s ready, {} turns after the appointment",
                   status.bombers, status.bodies, plain(plan.body_unit.as_str()),
                   g.turn.saturating_sub(plan.appointed_turn));
        }
        true
    }
}
