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
/// rather than a wish: 40 Online turns, which at the live seat's measured
/// t100 science (46 a turn) is Siege Tactics, Military Science and Metal
/// Casting from a Medieval tree with room to spare.
pub(super) const DECISIVE_RESEARCH_HORIZON: u32 = 80;
/// The assault must out-strength the defender by this much: 30·e^(5/25) ≈ 37
/// damage a blow against ≈ 24 taken — the first margin that wins trades.
pub(super) const DECISIVE_MARGIN: f64 = 5.0;
/// Strength credited to the civilization's own unique unit for the abilities
/// the strength column omits.
pub(super) const UNIQUE_MARGIN: f64 = 5.0;
/// A unique unit's package is priced at this share of its research.
pub(super) const UNIQUE_COST_SHARE: f64 = 0.75;
/// Guns a breaker package is sized for.
pub(super) const BREACH_GUNS: f64 = 2.0;
/// Turns those guns are given to empty the wall pool.
pub(super) const BREACH_TURNS: f64 = 6.0;
const WALL_TIER_HP: i32 = 100;
const ANTI_CAVALRY_BONUS: f64 = 10.0;
const MELEE_VS_SPEAR_BONUS: f64 = 5.0;

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
                && Self::decisive_resource_feasible(g, pid, spec)
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
                    BREACH_GUNS
                        * BREACH_TURNS
                        * expected_damage(spec.bombard_strength, city_strength)
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
            if margin + f64::EPSILON < DECISIVE_MARGIN {
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
                let price = turns * if is_unique { UNIQUE_COST_SHARE } else { 1.0 };
                let window = DecisiveWindow {
                    target,
                    assault: *kind,
                    breaker: breaker.map(|(breaker, _, _)| breaker),
                    wall_tier: tier,
                    defender,
                    margin,
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

    /// The civic a civic-gated window unit needs next (the Samurai's
    /// Feudalism, the Tagma's Divine Right, the Winged Hussar's Mercantilism).
    pub(super) fn decisive_window_civic_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<Name> {
        self.decisive_window(g, pid, plan)
            .and_then(|window| window.civic_goal)
    }
}

#[cfg(test)]
mod tests;
