//! `decisive-window`: research toward the next military window that is
//! actually decisive against the campaign's target, with the civilization's
//! own unique unit preferred inside it.
//!
//! ## What a decisive window is
//!
//! A city falls when two things are true at once: something can open its
//! walls, and something that can capture beats what defends it. Both sides of
//! that move in tiers, and the tiers are set by a handful of technologies:
//!
//! | rival tech | wall pool | what stops working |
//! |---|---|---|
//! | Masonry | 100 | — (Battering Ram opens it) |
//! | Castles | 200 | Battering Rams (`battering_ram_immunity`) |
//! | Siege Tactics | 300 | Siege Towers (`siege_support_immunity`) |
//! | Steel | 400 everywhere (Urban Defenses) | every support unit |
//!
//! Live King Gran Colombia, 30 games 2026-10-01..04 (state-frame census of
//! our `techs` against each rival's `tech_names`, best rival / rival median):
//! Castles t76/88, Siege Tactics t96/106, Steel t137/142. Ours: Military
//! Engineering (Trebuchet) t105, Metal Casting (Bombard) t139, Steel t176 —
//! **our breaker arrived one full wall tier late, every tier**. Stirrups
//! (Knights) came at t96 although it lies on the Advanced Flight path.
//! Military Science, the Llanero's unlock, came at a median t171 in only 16
//! of 30 games, and not one Llanero was ever trained; the best rival held
//! Military Science at t104. Research was a chain of one-step "modernize the
//! standing army" upgrades plus the lane scorer, so the army was always one
//! generation behind the walls it marched on.
//!
//! ## What this gene does
//!
//! For the campaign target (the plan's, else the nearest legal major) it reads
//! the wall tier the target holds — or the next one, when that technology is
//! already open to it — and the strongest land defender it can train now. It then prices every **package**: one land assault unit this
//! civilization can train that beats that defender by [`DECISIVE_MARGIN`]
//! (anti-cavalry +10 against horse, +5 to melee against spears, as the engine
//! resolves them), plus one breaker that opens those walls inside
//! [`BREACH_TURNS`] with [`BREACH_GUNS`] guns (a ram or tower only while it
//! still works on that tier and the assault is melee or anti-cavalry). The
//! cheapest package by remaining research — a unique unit at
//! [`UNIQUE_COST_SHARE`] of its price and [`UNIQUE_MARGIN`] stronger, for the
//! abilities the strength column omits (the Llanero's adjacency bonus and
//! Gran Colombia's extra move) — becomes the research goal, and a
//! civic-gated unique unit (Samurai, Tagma, Winged Hussar) becomes the civic
//! goal. A package already unlocked means the window is OPEN: no goal, and
//! every other research lane keeps the slot.
//!
//! The margin relaxes with numbers: at [`DOMINANT_POWER`] times the target's
//! military power the assault only has to match its best defender, and at
//! [`OVERWHELMING_POWER`] any assault counts, so the package is the breaker
//! alone.
//!
//! Air units count as breakers on the same damage test, so when the target
//! reaches Urban Defenses this goal and the air surge's bomber beeline agree
//! instead of competing.

use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{expected_damage, Game};
use crate::name::Name;
use crate::rules::UnitSpec;
use std::collections::BTreeSet;

/// Standard-speed turn before which the window never takes the research slot:
/// the ancient opening has its own archery/conquest research.
pub(super) const DECISIVE_WINDOW_OPENS: u32 = 60;
/// Standard-speed turns of research a package may cost and still be a window
/// rather than a wish: 25 Online turns. Frame-0 replays of live King G45-G47
/// priced the packages that matter well inside it — Masonry for rams against
/// Canada at t48 (24 turns), Knights and catapults at t72 (6), Musketmen and
/// Bombards at t90 (5), Line Infantry or Llaneros and Bombards at t100 (19) —
/// while every package past t115 (Tanks and Artillery, Modern Armor and
/// Bombers: 30-52 turns) is a long modernization the air surge prices better.
pub(super) const DECISIVE_RESEARCH_HORIZON: u32 = 50;
/// The assault must out-strength the defender by this much: 30·e^(5/25) ≈ 37
/// damage a blow against ≈ 24 taken — the first margin that wins trades.
pub(super) const DECISIVE_MARGIN: f64 = 5.0;
/// Military power over the target at which the assault only has to match
/// its best defender (margin 0) rather than beat it by [`DECISIVE_MARGIN`].
pub(super) const DOMINANT_POWER: f64 = 2.0;
/// Military power over the target at which numbers carry the assault and the
/// package is a breaker alone: any assault this civilization can field
/// counts. Live King G94 (civvis-20261005T024614Z) fought Portugal at 5-8x
/// its power from t79, and the window read "no package" every turn t73-t135
/// because no unit outclassed Portugal's best defender, so Military
/// Engineering (Trebuchets, the breaker for the 200-HP walls Lisbon raised
/// at t86) waited until t111 while Catapults could not open them.
pub(super) const OVERWHELMING_POWER: f64 = 3.0;
/// `breaker-research-first`: the price premium on a package whose unit needs a
/// strategic resource we cannot see yet. Its revealing technology joins the
/// path; whether we will hold any is unknown, so a breaker that needs no
/// resource wins a close call. Live King G136 (civvis-20261005T143823Z)
/// took Military Engineering at t87 for "bombard opens tier-3 walls" and had
/// no Niter income until t142.
pub(super) const UNREVEALED_RESOURCE_PREMIUM: f64 = 1.5;
/// Strength credited to the civilization's own unique unit for the abilities
/// the strength column omits.
pub(super) const UNIQUE_MARGIN: f64 = 5.0;
/// A unique unit's package is priced at this share of its research.
pub(super) const UNIQUE_COST_SHARE: f64 = 0.75;
/// Guns a land breaker package is sized for. An air breaker is sized as the
/// air surge's wing (`air_surge::AIR_SURGE_BOMBERS`).
pub(super) const BREACH_GUNS: f64 = 2.0;
/// Turns those guns are given to empty the wall pool.
pub(super) const BREACH_TURNS: f64 = 6.0;
const WALL_TIER_HP: i32 = 100;
const ANTI_CAVALRY_BONUS: f64 = 10.0;
const MELEE_VS_SPEAR_BONUS: f64 = 5.0;

/// The civic that unlocks Corps (`corps_fleets`): +10 strength on a merged body.
const CORPS_CIVIC: &str = "nationalism";

/// The technology that raises a civilization to each wall tier.
const WALL_TIER_TECHS: [&str; 4] = ["masonry", "castles", "siege_tactics", "steel"];

/// One priced window: what attacks, what opens the walls, and what is left to
/// research before both are trainable.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DecisiveWindow {
    pub(crate) target: usize,
    pub(crate) assault: Name,
    pub(crate) breaker: Option<Name>,
    /// The wall tier planned against (0-4).
    pub(crate) wall_tier: i32,
    /// The target's strongest defender against this assault.
    pub(crate) defender: f64,
    pub(crate) margin: f64,
    /// Our military power over the target's when the window was priced.
    pub(crate) power_ratio: f64,
    pub(crate) unique: bool,
    /// Turns until both halves are unlocked at the current rates; 0 = open.
    pub(crate) turns: f64,
    pub(crate) tech_goal: Option<Name>,
    pub(crate) civic_goal: Option<Name>,
}

impl DecisiveWindow {
    pub(crate) fn open(&self) -> bool {
        self.tech_goal.is_none() && self.civic_goal.is_none()
    }
}

fn is_cavalry(spec: &UnitSpec) -> bool {
    matches!(
        spec.promotion_class.as_str(),
        "light_cavalry" | "heavy_cavalry"
    )
}

fn land_military(spec: &UnitSpec) -> bool {
    spec.class == "military" && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

/// The tech and civic that unlock `spec`. A unique unit that names neither
/// inherits its base unit's: the Gaesatae is a Warrior and needs nothing,
/// the Tagma names its civic.
fn unlock_of(g: &Game, spec: &UnitSpec) -> (Option<Name>, Option<Name>) {
    if spec.tech.is_none() && spec.civic.is_none() {
        if let Some(base) = spec.replaces.and_then(|base| g.rules.units.get(&base)) {
            return (base.tech, base.civic);
        }
    }
    (spec.tech, spec.civic)
}

/// Whether `pid` holds `tech`, or — with `ahead` — could start researching it
/// now: a wall tier one step ahead is what a rival that out-techs us stands
/// behind by the time our package lands.
fn tech_within_reach(g: &Game, pid: usize, tech: Name, ahead: bool) -> bool {
    let known = &g.players[pid].techs;
    known.contains(&tech)
        || (ahead
            && g.rules
                .techs
                .get(&tech)
                .is_some_and(|spec| spec.requires.iter().all(|req| known.contains(req))))
}

fn civic_within_reach(g: &Game, pid: usize, civic: Name, ahead: bool) -> bool {
    let known = &g.players[pid].civics;
    known.contains(&civic)
        || (ahead
            && g.rules
                .civics
                .get(&civic)
                .is_some_and(|spec| spec.requires.iter().all(|req| known.contains(req))))
}

impl AdvancedAi {
    /// The rival this window is planned against: the plan's target when it is
    /// a legal major, else the nearest legal major by city distance.
    fn decisive_window_target(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> Option<usize> {
        let major = |other: usize| {
            g.players
                .get(other)
                .is_some_and(|p| p.alive && !p.is_minor && !p.is_barbarian)
                && self.campaign_target_legal(g, pid, other)
        };
        if let Some(target) = plan.target_player.filter(|target| major(*target)) {
            return Some(target);
        }
        let ours: Vec<_> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        g.players
            .iter()
            .filter(|other| major(other.id))
            .filter_map(|other| {
                g.player_city_ids(other.id)
                    .into_iter()
                    .flat_map(|cid| {
                        let pos = g.cities[&cid].pos;
                        ours.iter().map(move |home| (*home, pos))
                    })
                    .map(|(home, pos)| g.wdist(home, pos))
                    .min()
                    .map(|distance| (distance, other.id))
            })
            .min()
            .map(|(_, other)| other)
    }

    /// The wall tier `target` holds — from its technologies and from any
    /// walls we have seen — and the tier it reaches next when that wall
    /// technology is already open to it.
    pub(super) fn decisive_wall_tier(&self, g: &Game, target: usize) -> (i32, i32) {
        let from_techs = WALL_TIER_TECHS
            .iter()
            .rposition(|tech| g.players[target].techs.contains(&Name::new(tech)))
            .map_or(0, |index| index as i32 + 1);
        let seen = g
            .player_city_ids(target)
            .into_iter()
            .filter_map(|cid| {
                if self.battlefront_observation {
                    self.remembered_city(cid)
                        .filter(|sighting| sighting.owner == target)
                        .map(|sighting| sighting.wall_hp)
                } else {
                    Some(g.city_max_wall_hp(&g.cities[&cid]))
                }
            })
            .max()
            .unwrap_or(0);
        let held = from_techs.max((seen + WALL_TIER_HP - 1) / WALL_TIER_HP).min(4);
        let next = (held < 4
            && tech_within_reach(g, target, Name::new(WALL_TIER_TECHS[held as usize]), true))
            as i32;
        (held, held + next)
    }

    /// The land designs `target` can train now: its own civilization's
    /// replacements, gated by the technologies and civics it holds. Held
    /// technologies only — a design one step ahead still has to be
    /// researched, built and marched, while a wall tier lands the turn its
    /// technology does — and projecting every rival roster a step ahead
    /// leaves no package decisive against a rival that out-techs us.
    fn decisive_target_roster(g: &Game, target: usize) -> impl Iterator<Item = &UnitSpec> {
        g.rules
            .units
            .iter()
            .filter(move |(kind, spec)| {
                land_military(spec)
                    && spec.buildable
                    && spec.strength > 0.0
                    && g.player_unit_replacement(target, **kind) == **kind
                    && spec
                        .unique_to
                        .as_deref()
                        .is_none_or(|civ| civ == g.players[target].civ)
                    && {
                        let (tech, civic) = unlock_of(g, spec);
                        tech.is_none_or(|tech| tech_within_reach(g, target, tech, false))
                            && civic.is_none_or(|civic| civic_within_reach(g, target, civic, false))
                    }
            })
            .map(|(_, spec)| spec)
    }

    /// The strongest land defender `target` can train now, as it meets
    /// `assault`: anti-cavalry +10 against horse, a spear -5 against melee.
    fn decisive_defender(g: &Game, target: usize, assault: &UnitSpec) -> f64 {
        Self::decisive_target_roster(g, target)
            .filter(|spec| !spec.siege)
            .map(|spec| {
                let mut strength = spec.strength;
                if spec.promotion_class == "anti_cavalry" {
                    if is_cavalry(assault) {
                        strength += ANTI_CAVALRY_BONUS;
                    } else if assault.promotion_class == "melee" {
                        strength -= MELEE_VS_SPEAR_BONUS;
                    }
                }
                strength
            })
            .fold(20.0_f64, f64::max)
    }

    /// The walled City Center strength a breaker shoots at: the strongest one
    /// seen, or the engine's formula from the target's roster — the strongest
    /// unit less 10, +3 per wall level, +3 for the Palace — whichever is
    /// higher.
    fn decisive_city_strength(&self, g: &Game, target: usize, tier: i32) -> f64 {
        let seen = g
            .player_city_ids(target)
            .into_iter()
            .filter_map(|cid| {
                if self.battlefront_observation {
                    self.remembered_city(cid)
                        .filter(|sighting| sighting.owner == target)
                        .map(|sighting| sighting.strength)
                } else {
                    Some(g.city_strength(cid))
                }
            })
            .fold(0.0_f64, f64::max);
        let roster = Self::decisive_target_roster(g, target)
            .map(|spec| spec.strength)
            .fold(20.0_f64, f64::max);
        seen.max(roster - 10.0 + 3.0 * tier as f64 + 3.0)
    }

    /// Whether `pid` can expect to feed `spec`'s strategic resource: a
    /// stockpile, an owned deposit, or a resource it cannot see yet.
    fn decisive_resource_feasible(g: &Game, pid: usize, spec: &UnitSpec) -> bool {
        let Some(resource) = spec.requires_resource else {
            return true;
        };
        !g.resource_visible_to(pid, resource.as_str())
            || g.strategic_stockpile(pid, resource) > 0.0
            || g.player_city_ids(pid).into_iter().any(|cid| {
                g.cities[&cid].owned_tiles.iter().any(|pos| {
                    g.map
                        .tiles
                        .get(pos)
                        .is_some_and(|tile| tile.resource == Some(resource))
                })
            })
    }

    /// `breaker-research-first`: whether `pid` is supplied with `spec`'s
    /// strategic resource now: a stockpile or an income. A deposit without
    /// the improvement that connects it is not supply yet (G136 owned no
    /// Niter income until t142 and built its Bombard at t171), and a resource
    /// still hidden is judged by its revealing technology instead.
    fn decisive_resource_supplied(g: &Game, pid: usize, spec: &UnitSpec) -> bool {
        let Some(resource) = spec.requires_resource else {
            return true;
        };
        !g.resource_visible_to(pid, resource.as_str())
            || g.strategic_stockpile(pid, resource) > 0.0
            || g.strategic_resource_rate(pid, resource.as_str()) > 0.0
    }

    /// `breaker-research-first`: the technology that reveals `spec`'s
    /// strategic resource, while we cannot see it.
    fn decisive_hidden_resource_tech(g: &Game, pid: usize, spec: &UnitSpec) -> Option<Name> {
        let resource = spec.requires_resource?;
        if g.resource_visible_to(pid, resource.as_str()) {
            return None;
        }
        g.rules.resources.get(resource.as_str()).and_then(|spec| spec.tech)
    }

    /// `breaker-research-first`: a siege of ours stands held for a
    /// wall-breaker this turn (`siege-needs-a-breaker`'s wait record).
    fn decisive_siege_held_for_a_breaker(&self, g: &Game) -> bool {
        self.siege_breaker_waits
            .values()
            .any(|wait| g.turn.saturating_sub(wait.last) <= 1)
    }

    /// Whether `tech` unlocks a siege gun, ram or tower this civilization
    /// can train.
    fn decisive_tech_unlocks_a_breaker(g: &Game, pid: usize, tech: Name) -> bool {
        let civ = g.players[pid].civ.as_str();
        g.rules.units.iter().any(|(kind, spec)| {
            spec.tech == Some(tech)
                && spec.buildable
                && g.player_unit_replacement(pid, *kind) == *kind
                && spec.unique_to.as_deref().is_none_or(|owner| owner == civ)
                && ((spec.class == "military" && spec.siege && spec.bombard_strength > 0.0)
                    || matches!(kind.as_str(), "battering_ram" | "siege_tower"))
        })
    }

    /// `breaker-research-first`: while a siege is held for a wall-breaker,
    /// "modernize the standing army" does not spend the breaker's research on
    /// a technology that unlocks none. Live King G136 researched Gunpowder,
    /// Metal Casting's path and Ballistics (Field Cannons: ranged, not
    /// breakers) at t93-t103 while the Victoria siege held 77 turns with
    /// one or two fit guns.
    pub(super) fn modernization_yields_to_the_breaker(
        &self,
        g: &Game,
        pid: usize,
        tech: Name,
    ) -> bool {
        self.breaker_research_first
            && self.decisive_siege_held_for_a_breaker(g)
            && !Self::decisive_tech_unlocks_a_breaker(g, pid, tech)
    }

    /// Unknown technologies and civics on the way to every node in `techs`
    /// and `civics`, priced in turns at the empire's current rates.
    fn decisive_turns(g: &Game, pid: usize, techs: &[Name], civics: &[Name]) -> f64 {
        let player = &g.players[pid];
        let mut tech_path = BTreeSet::new();
        for tech in techs {
            tech_path.insert(tech.to_string());
            if let Some(ancestors) = g.rules.tech_ancestors.get(tech.as_str()) {
                tech_path.extend(ancestors.iter().cloned());
            }
        }
        let tech_cost: f64 = tech_path
            .iter()
            .filter(|tech| !player.techs.contains(&Name::new(tech)))
            .map(|tech| {
                let quoted = g.host_remaining_research_cost(pid, Name::new(tech));
                let mut cost = quoted.unwrap_or_else(|| g.tech_cost(tech));
                if quoted.is_none() && player.research.as_deref() == Some(tech.as_str()) {
                    cost -= player.research_progress.min(cost);
                }
                cost
            })
            .sum();
        let mut civic_path = BTreeSet::new();
        for civic in civics {
            civic_path.insert(civic.to_string());
            if let Some(ancestors) = g.rules.civic_ancestors.get(civic.as_str()) {
                civic_path.extend(ancestors.iter().cloned());
            }
        }
        let civic_cost: f64 = civic_path
            .iter()
            .filter(|civic| !player.civics.contains(&Name::new(civic)))
            .map(|civic| {
                let mut cost = g.civic_cost(civic);
                if player.civic.as_deref() == Some(civic.as_str()) {
                    cost -= player.civic_progress.min(cost);
                }
                cost
            })
            .sum();
        let culture = {
            let _memo = g.query_memo();
            g.player_city_ids(pid)
                .into_iter()
                .map(|cid| g.city_yields(cid).culture)
                .sum::<f64>()
                .max(1.0)
        };
        (tech_cost / Self::war_science_per_turn(g, pid)).max(civic_cost / culture)
    }

    /// The decisive window against the campaign target, or `None` when the
    /// gene is off, the seat is not a conquest seat, the opening still owns
    /// research, or no package inside the horizon is decisive.
    pub(crate) fn decisive_window(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<DecisiveWindow> {
        let horizon = g.standard_duration(DECISIVE_RESEARCH_HORIZON) as f64;
        self.decisive_window_within(g, pid, plan, horizon)
    }

    /// [`Self::decisive_window`] with the research horizon as an argument.
    pub(super) fn decisive_window_within(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        horizon: f64,
    ) -> Option<DecisiveWindow> {
        if !self.decisive_window
            || !(self.active_victory_target(g) == Some(VictoryTarget::Domination)
                || plan.strategy == GrandStrategy::Conquest)
            || g.turn < g.standard_duration(DECISIVE_WINDOW_OPENS)
            || g.player_city_ids(pid).len() < 2
        {
            return None;
        }
        let target = self.decisive_window_target(g, pid, plan)?;
        let power_ratio = g.military_power(pid) / g.military_power(target).max(1.0);
        let required_margin = if power_ratio >= OVERWHELMING_POWER {
            f64::NEG_INFINITY
        } else if power_ratio >= DOMINANT_POWER {
            0.0
        } else {
            DECISIVE_MARGIN
        };
        let (_, tier) = self.decisive_wall_tier(g, target);
        let wall_pool = (tier * WALL_TIER_HP) as f64;
        let city_strength = self.decisive_city_strength(g, target, tier);
        let civ = g.players[pid].civ.as_str();
        let own_design = |kind: Name, spec: &UnitSpec| {
            spec.buildable
                && g.player_unit_replacement(pid, kind) == kind
                && spec.unique_to.as_deref().is_none_or(|owner| owner == civ)
                && spec
                    .obsolete_tech
                    .is_none_or(|tech| !g.players[pid].techs.contains(&tech))
                && if self.breaker_research_first {
                    Self::decisive_resource_supplied(g, pid, spec)
                } else {
                    Self::decisive_resource_feasible(g, pid, spec)
                }
        };
        let unique = |spec: &UnitSpec| !self.civ_blind && spec.unique_to.as_deref() == Some(civ);
        let assaults: Vec<(Name, &UnitSpec)> = g
            .rules
            .units
            .iter()
            .filter(|(kind, spec)| {
                land_military(spec)
                    && !spec.siege
                    && spec.is_melee_capable()
                    && spec.promotion_class != "recon"
                    && own_design(**kind, spec)
            })
            .map(|(kind, spec)| (*kind, spec))
            .collect();
        let breakers: Vec<(Name, &UnitSpec)> = g
            .rules
            .units
            .iter()
            .filter(|(kind, spec)| {
                own_design(**kind, spec)
                    && ((spec.class == "military" && spec.siege && spec.bombard_strength > 0.0)
                        || matches!(kind.as_str(), "battering_ram" | "siege_tower"))
            })
            .map(|(kind, spec)| (*kind, spec))
            .collect();
        let opens = |kind: Name, spec: &UnitSpec, assault: &UnitSpec| -> bool {
            match kind.as_str() {
                "battering_ram" => {
                    tier <= 1 && matches!(assault.promotion_class.as_str(), "melee" | "anti_cavalry")
                }
                "siege_tower" => {
                    tier <= 2 && matches!(assault.promotion_class.as_str(), "melee" | "anti_cavalry")
                }
                _ => {
                    // An air breaker strikes as the air surge's wing, not as a
                    // pair of guns: live King G104 (civvis-20261005T060002Z)
                    // held Bombers from t158, and at t177 the window priced
                    // Jet Bombers (Stealth Technology) because two Bombers
                    // fell short of Sparta's 400-HP walls.
                    let guns = if spec.domain.as_deref() == Some("air") {
                        super::air_surge::AIR_SURGE_BOMBERS as f64
                    } else {
                        BREACH_GUNS
                    };
                    guns * BREACH_TURNS * expected_damage(spec.bombard_strength, city_strength)
                        + f64::EPSILON
                        >= wall_pool
                }
            }
        };
        let mut best: Option<(f64, DecisiveWindow)> = None;
        for (kind, spec) in &assaults {
            let is_unique = unique(spec);
            let defender = Self::decisive_defender(g, target, spec);
            let margin =
                spec.strength + if is_unique { UNIQUE_MARGIN } else { 0.0 } - defender;
            if margin + f64::EPSILON < required_margin {
                continue;
            }
            let (assault_tech, assault_civic) = unlock_of(g, spec);
            let breaker_options: Vec<Option<(Name, Option<Name>, Option<Name>)>> = if tier == 0 {
                vec![None]
            } else {
                breakers
                    .iter()
                    .filter(|(breaker, breaker_spec)| opens(*breaker, breaker_spec, spec))
                    .map(|(breaker, breaker_spec)| {
                        let (tech, civic) = unlock_of(g, breaker_spec);
                        Some((*breaker, tech, civic))
                    })
                    .collect()
            };
            for breaker in breaker_options {
                let mut techs: Vec<Name> = assault_tech.into_iter().collect();
                let mut civics: Vec<Name> = assault_civic.into_iter().collect();
                if let Some((_, tech, civic)) = breaker {
                    techs.extend(tech);
                    civics.extend(civic);
                }
                // `breaker-research-first`: a hidden resource's revealing
                // technology joins the path, and the package pays a premium.
                let hidden: Vec<Name> = if self.breaker_research_first {
                    std::iter::once(*spec)
                        .chain(breaker.map(|(breaker, _, _)| &g.rules.units[breaker]))
                        .filter_map(|unit| Self::decisive_hidden_resource_tech(g, pid, unit))
                        .collect()
                } else {
                    Vec::new()
                };
                techs.extend(hidden.iter().copied());
                let turns = Self::decisive_turns(g, pid, &techs, &civics);
                if turns > horizon {
                    continue;
                }
                let missing = |tech: &Option<Name>| {
                    tech.filter(|tech| !g.players[pid].techs.contains(tech))
                };
                let tech_goal = [missing(&assault_tech), breaker.and_then(|(_, tech, _)| missing(&tech))]
                    .into_iter()
                    .flatten()
                    .chain(hidden.iter().copied().filter(|tech| !g.players[pid].techs.contains(tech)))
                    .min_by(|left, right| {
                        Self::war_remaining_research_cost(g, pid, *left)
                            .total_cmp(&Self::war_remaining_research_cost(g, pid, *right))
                            .then_with(|| left.cmp(right))
                    });
                let civic_goal = [
                    assault_civic.filter(|civic| !g.players[pid].civics.contains(civic)),
                    breaker
                        .and_then(|(_, _, civic)| civic)
                        .filter(|civic| !g.players[pid].civics.contains(civic)),
                ]
                .into_iter()
                .flatten()
                .next();
                let price = turns
                    * if is_unique { UNIQUE_COST_SHARE } else { 1.0 }
                    * if hidden.is_empty() { 1.0 } else { UNREVEALED_RESOURCE_PREMIUM };
                let window = DecisiveWindow {
                    target,
                    assault: *kind,
                    breaker: breaker.map(|(breaker, _, _)| breaker),
                    wall_tier: tier,
                    defender,
                    margin,
                    power_ratio,
                    unique: is_unique,
                    turns,
                    tech_goal,
                    civic_goal,
                };
                let better = best.as_ref().is_none_or(|(best_price, held)| {
                    price
                        .total_cmp(best_price)
                        .then_with(|| held.margin.total_cmp(&window.margin))
                        .then_with(|| window.assault.cmp(&held.assault))
                        .then_with(|| window.breaker.cmp(&held.breaker))
                        .is_lt()
                });
                if better {
                    best = Some((price, window));
                }
            }
        }
        best.map(|(_, window)| window)
    }

    /// The research goal the window hands `advanced_research`: its next
    /// technology, except while `found-against-a-rival-faith`'s Astrology arm
    /// owns the slot (`faith_arm_wants_astrology`). Both arms sit in one
    /// forced-goal match and the faith arm is meant to be the higher one, but
    /// a merge can reorder them; one Ancient node toward a religion of our
    /// own outranks the window either way. Live King G69-G71 were three
    /// straight Religious losses on faithless cities, while a pending window
    /// held research from t69 to t99 in G52.
    pub(super) fn decisive_window_research_goal(
        &self,
        g: &Game,
        pid: usize,
        window: Option<&DecisiveWindow>,
    ) -> Option<Name> {
        window
            .and_then(|window| window.tech_goal)
            .filter(|_| !self.faith_arm_wants_astrology(g, pid))
            .filter(|goal| !self.window_yields_to_the_bombers(g, pid, *goal))
    }

    /// `siege-tier-yields-to-the-bombers`: whether the window's research
    /// `goal` gives way to the air surge's. It does while the surge has a
    /// research goal and `goal` is neither Advanced Flight nor one of its
    /// ancestors. Over the 10-06/07 Emperor runs, cities of a major were
    /// taken 0.20 times per 100 turns between turn 60 and Advanced Flight and
    /// 1.24 times after it, yet the window spent about 1,200 science a game
    /// off that path once the surge's horizon had opened (median turn 95):
    /// the Llanero's Military Science, Steel's Artillery, and on the way the
    /// Bombard tier's Gunpowder and Metal Casting. Advanced Flight landed at a
    /// median turn 147, and the games that reached it by turn 139 took 0.79
    /// major cities between turns 130 and 190 against 0.08 for those past 169.
    pub(crate) fn window_yields_to_the_bombers(&self, g: &Game, pid: usize, goal: Name) -> bool {
        use super::air_surge::AIR_SURGE_GOAL_TECH;
        self.siege_tier_yields_to_the_bombers
            && goal.as_str() != AIR_SURGE_GOAL_TECH
            && !g
                .rules
                .tech_ancestors
                .get(AIR_SURGE_GOAL_TECH)
                .is_some_and(|ancestors| ancestors.contains(goal.as_str()))
            && self.air_surge_research_goal(g, pid).is_some()
    }

    /// The faith arm's own predicate, restated: the veto is due, the Prophet
    /// race is open to us, and Astrology is still unknown. ⚠ `faith_veto_due`
    /// alone stays true for as long as the seat has no religion. Live King
    /// G80 (civvis-20261004T212049Z) researched Astrology at t35, and the
    /// window yielded at t57, t62, t67 and t84 anyway (Military Tactics,
    /// Apprenticeship, Engineering, Military Engineering), with nothing left
    /// for the faith arm to research.
    fn faith_arm_wants_astrology(&self, g: &Game, pid: usize) -> bool {
        self.faith_veto_due(g, pid)
            && self.prophet_race_enterable_for(g, pid, self.victory_target)
            && !g.players[pid].techs.contains(&crate::name!("astrology"))
    }

    /// The Bomber count the Domination air readiness pass builds toward. A
    /// target behind Urban Defenses (tier 4) is the case the window prices
    /// Bombers as the breaker, sized as the air surge's wing; readiness
    /// alone stopped at `AIR_SURGE_LAUNCH_BOMBERS` (live King G104: two
    /// Bombers, Sparta's walls 400 from t196 to t242). Below tier 4 a land
    /// gun is the cheaper breaker and readiness keeps its launch pair.
    pub(crate) fn decisive_air_wing_bombers(&self, g: &Game, pid: usize) -> usize {
        use super::air_surge::{AIR_SURGE_BOMBERS, AIR_SURGE_LAUNCH_BOMBERS};
        if !self.decisive_window {
            return AIR_SURGE_LAUNCH_BOMBERS;
        }
        let Some(plan) = self.plan.as_ref() else {
            return AIR_SURGE_LAUNCH_BOMBERS;
        };
        match self.decisive_window_target(g, pid, plan) {
            Some(target) if self.decisive_wall_tier(g, target).1 >= 4 => AIR_SURGE_BOMBERS,
            _ => AIR_SURGE_LAUNCH_BOMBERS,
        }
    }

    /// The technology the decisive window needs next; `None` once it is open.
    pub(super) fn decisive_window_tech_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<Name> {
        self.decisive_window(g, pid, plan)
            .and_then(|window| window.tech_goal)
    }

    /// The civic the window needs next: a civic-gated window unit's (the
    /// Samurai's Feudalism, the Tagma's Divine Right, the Winged Hussar's
    /// Mercantilism), else Nationalism once it is inside the horizon — Corps
    /// are +10 strength on every body of whatever package the army carries,
    /// the largest single step the civic tree sells, and the live seat forms
    /// them as soon as it can (6-19 FORM_CORPS/FORM_ARMY orders a game) but
    /// reached Nationalism at t150 against the best rival's t106.
    pub(super) fn decisive_window_civic_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<Name> {
        let horizon = g.standard_duration(DECISIVE_RESEARCH_HORIZON) as f64;
        self.decisive_window_civic_goal_within(g, pid, plan, horizon)
    }

    /// [`Self::decisive_window_civic_goal`] with the horizon as an argument.
    pub(super) fn decisive_window_civic_goal_within(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        horizon: f64,
    ) -> Option<Name> {
        let window = self.decisive_window_within(g, pid, plan, horizon)?;
        if window.civic_goal.is_some() {
            return window.civic_goal;
        }
        let corps = Name::new(CORPS_CIVIC);
        (g.rules.civics.contains_key(&corps)
            && !g.players[pid].civics.contains(&corps)
            && Self::decisive_turns(g, pid, &[], &[corps]) <= horizon)
            .then_some(corps)
    }
}

#[cfg(test)]
mod tests;
