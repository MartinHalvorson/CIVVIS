//! Unit upgrades: what a unit upgrades to, what it costs, and doing it.
//!
//! The upgrade graph (`upgrade_costs`) and obsolescence
//! (`unit_is_obsolete`); the price in gold and strategic resources, alone
//! or in a formation, with the bill floor and the host's resource prices
//! where the live game supplies them (`unit_upgrade_price*`,
//! `unit_upgrade_resource_price`); the full offer and why the host refuses
//! one (`unit_gold_upgrade_offer`, `unit_gold_upgrade_detail`); and
//! `do_upgrade_unit`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub fn upgrade_costs(&self, pid: usize, gold: f64, resources: f64) -> (f64, f64) {
        // `MINOR_CIV_GOLD_MILITARY_UPGRADE`: a city-state upgrades for no Gold.
        if self.players[pid].is_minor && !self.players[pid].is_barbarian {
            return (
                0.0,
                resources
                    * (1.0 - self.policy_effect(pid, "upgrade_resource_discount_pct") / 100.0),
            );
        }
        (
            gold * (1.0 - self.policy_effect(pid, "upgrade_gold_discount_pct") / 100.0),
            resources * (1.0 - self.policy_effect(pid, "upgrade_resource_discount_pct") / 100.0),
        )
    }

    /// Whether `kind` has left `pid`'s production menus. The unit stays on the
    /// map; it simply can no longer be trained or purchased.
    ///
    /// ★★★★★ TWO RULES RETIRE A UNIT, AND CIVVIS MODELLED ONLY THE LATER ONE.
    ///
    /// `MandatoryObsoleteTech` is the one the shipped table names, and it fires
    /// late. Civilization VI ALSO drops a unit from the menu the moment its
    /// UPGRADE becomes available — a Catapult disappears when Trebuchets unlock,
    /// not when Steel is researched — and `obsolete_tech` disagrees with the
    /// successor's tech on **54 of the 57 units that have an `upgrade_to`**:
    ///
    /// ```text
    /// unit            obsolete_tech          Civ 6 drops it at
    /// scout           None                   machinery      <- buildable FOREVER
    /// catapult        steel                  military_engineering
    /// trebuchet       steel                  metal_casting
    /// warrior         gunpowder              iron_working
    /// heavy_chariot   combustion             stirrups
    /// ```
    ///
    /// The cost is measured, not theoretical. Live run `civvis-20260803T141442Z`
    /// (turn 241) emitted **123 `civvis_build_unplayable` refusals and 111 of them
    /// were units whose successor's tech was already researched** — 48 Scouts, and
    /// **54 SIEGE UNITS**, 29 Trebuchets and 25 Catapults, every one rejected by the
    /// host. The army was not failing to ask for siege. It asked 54 times and
    /// Civilization VI refused all 54, because the chooser takes the CHEAPEST unit
    /// in a class and the cheapest is exactly the obsolete one.
    ///
    /// Re-asking never stopped because the refusal feeds an EIGHT-TURN cooldown
    /// (`recent_host_item_refusals`) while obsolescence is permanent, so the block
    /// expired and the order came back for the rest of the game.
    ///
    /// `unit_upgrade_target` is the right predicate to reuse rather than a fresh
    /// tech comparison: it already resolves a civilization's unique replacement and
    /// already requires the successor to be `buildable` and unlocked, which is
    /// precisely the host's condition for withdrawing the predecessor.
    pub fn unit_is_obsolete(&self, pid: usize, kind: impl AsName) -> bool {
        let kind = kind.as_name();
        if self
            .rules
            .units
            .get_interned(kind)
            .and_then(|spec| spec.obsolete_tech)
            .is_some_and(|tech| self.players[pid].techs.contains(&tech))
        {
            return true;
        }
        let Some(target) = self.unit_upgrade_target(pid, kind) else {
            return false;
        };
        // ⚠ A SUCCESSOR THAT CANNOT BE BUILT RETIRES NOTHING.
        //
        // Civilization VI keeps the Warrior on the menu for an empire that has no
        // Iron for a Swordsman, and the first version of this rule did not: it
        // retired the predecessor on the tech alone and left such an empire with NO
        // melee unit at all. `military_picker_preserves_city_capturing_melee` caught
        // exactly that, returning a ranged unit where it demanded melee.
        self.rules
            .units
            .get(&target)
            .and_then(|spec| spec.requires_resource)
            .is_none_or(|resource| self.strategic_stockpile(pid, resource) > 0.0)
    }

    /// Every number in the upgrade bill, as a named shipped row.
    ///
    /// All five are `GlobalParameters` rows read from the host's own compiled
    /// database (`Cache/DebugGameplay.sqlite`, table `GlobalParameters`) and
    /// confirmed against `Rules::Units::Instance::GetUpgradeCost` in the
    /// shipped `GameCore_XP2_FinalRelease.dll`, which reads exactly these:
    ///
    /// ```text
    /// UPGRADE_BASE_COST                    10
    /// UPGRADE_MINIMUM_COST                 15
    /// UPGRADE_NET_PRODUCTION_PERCENT_COST 100   Vanilla ships 75; Gathering
    ///                                           Storm replaces it with 100
    /// GOLD_EQUIVALENT_OTHER_YIELDS          2
    /// PURCHASE_DIVISOR                      5
    /// ```
    pub(super) const UPGRADE_BASE_COST: f64 = 10.0;
    pub(super) const UPGRADE_MINIMUM_COST: f64 = 15.0;
    pub(super) const UPGRADE_NET_PRODUCTION_PERCENT_COST: f64 = 100.0;
    pub(super) const GOLD_EQUIVALENT_OTHER_YIELDS: f64 = 2.0;
    pub(super) const PURCHASE_DIVISOR: f64 = 5.0;

    /// Gold and strategic material to upgrade one `kind` into its successor,
    /// priced as a Standard (uncombined) unit.
    ///
    /// ★★★ THE PER-PRODUCTION FACTOR OF TWO IS A SHIPPED ROW, NOT A FUDGE.
    ///
    /// This doc used to say that factor "does not come from GlobalParameters"
    /// and had been inferred from in-game prices being twice the Production
    /// difference. The host disagrees only about where it lives, not about the
    /// number: `GetUpgradeCost` multiplies the net Production difference by
    /// `UPGRADE_NET_PRODUCTION_PERCENT_COST` (100, so all of it) and then by
    /// `GOLD_EQUIVALENT_OTHER_YIELDS` (2), the same Production-to-Gold
    /// conversion the rest of the host uses. Two is the shipped coefficient.
    /// Charging one would halve every upgrade in the game.
    pub fn unit_upgrade_price(&self, pid: usize, kind: &str) -> Option<(Name, f64, f64)> {
        let target = self.unit_upgrade_target(pid, Name::new(kind))?;
        self.unit_upgrade_price_to(pid, Name::new(kind), target)
            .map(|(gold, resources)| (target, gold, resources))
    }

    /// Quote a direct upgrade before its technology is owned.
    ///
    /// A timed campaign must reserve the bill while researching the successor,
    /// earlier than [`Self::unit_upgrade_target`] can expose it as an action.
    /// This keeps the game-speed and policy discounts in the engine and checks
    /// the civilization-specific replacement edge used by the real upgrade.
    pub fn unit_upgrade_price_to(
        &self,
        pid: usize,
        kind: impl AsName,
        target: impl AsName,
    ) -> Option<(f64, f64)> {
        self.unit_upgrade_price_in_formation(pid, kind, target, 0)
    }

    /// The same quote for a unit that is already a Corps or an Army.
    ///
    /// `GetUpgradeCost` multiplies the whole bill by 2 for
    /// `CORPS_MILITARY_FORMATION` and by 3 for `ARMY_MILITARY_FORMATION` --
    /// two or three bodies are being re-equipped -- and it does so *before*
    /// the minimum and the rounding, so a combined unit cannot be priced by
    /// scaling a finished Standard quote.
    pub fn unit_upgrade_price_in_formation(
        &self,
        pid: usize,
        kind: impl AsName,
        target: impl AsName,
        formation: u8,
    ) -> Option<(f64, f64)> {
        let kind = kind.as_name();
        let target = target.as_name();
        let generic = self.rules.units.get_interned(kind)?.upgrade_to?;
        if self.player_unit_replacement(pid, generic) != target {
            return None;
        }
        let from = self.rules.units.get_interned(kind)?.cost;
        let spec = self.rules.units.get_interned(target)?;
        // BASE + NET_PRODUCTION_PERCENT / 100 * GOLD_EQUIVALENT * max(0, dProd).
        // The host differences two already speed-scaled Production costs and
        // speed-scales its base separately; scaling the sum is the same linear
        // map, and `units.json` holds the Standard-speed costs.
        let net = (spec.cost - from).max(0.0)
            * (Self::UPGRADE_NET_PRODUCTION_PERCENT_COST / 100.0)
            * Self::GOLD_EQUIVALENT_OTHER_YIELDS;
        let bodies = match formation {
            0 => 1.0,
            1 => 2.0,
            _ => 3.0,
        };
        let gold = self.game_speed.scale(Self::UPGRADE_BASE_COST + net) * bodies;
        let (gold, resources) = self.upgrade_costs(pid, gold, spec.resource_cost * bodies);
        Some((self.upgrade_bill_floor(gold), resources))
    }

    /// The last two steps of `GetUpgradeCost`, which run *after* the formation
    /// multiplier and the player's upgrade discount, not before them.
    ///
    /// The minimum is asymmetric in the shipped code and that is copied here
    /// deliberately: it *compares* against the raw `UPGRADE_MINIMUM_COST` and
    /// *assigns* the game-speed scaled one, so a cheap upgrade on a fast speed
    /// settles below 15 Gold rather than at it. The bill is then truncated to
    /// whole Gold and rounded DOWN to a multiple of `PURCHASE_DIVISOR`, which
    /// is why the host never quotes 73 Gold for anything.
    fn upgrade_bill_floor(&self, gold: f64) -> f64 {
        let gold = if gold < Self::UPGRADE_MINIMUM_COST {
            self.game_speed.scale(Self::UPGRADE_MINIMUM_COST)
        } else {
            gold
        };
        gold - gold.rem_euclid(Self::PURCHASE_DIVISOR)
    }

    /// Civ VI upgrades happen in friendly territory, before the unit has done
    /// anything else that turn, and cost Gold plus the successor's strategic
    /// material.
    pub fn unit_gold_upgrade_offer(&self, pid: usize, uid: u32) -> Option<(Name, f64, f64)> {
        self.unit_gold_upgrade_detail(pid, uid).ok()
    }

    /// Price this unit's exact successor, including its formation and policy
    /// modifiers. Funding may read a blocked offer; this grants no permission
    /// to upgrade here. Unknown host bills retain the ordinary direct quote.
    pub fn unit_upgrade_resource_price(&self, pid: usize, uid: u32, target: Name) -> Option<f64> {
        let unit = self.units.get(&uid).filter(|unit| unit.owner == pid)?;
        self.host_upgrade_resource_price(uid, target).or_else(|| {
            self.unit_upgrade_price_in_formation(pid, unit.kind, target, unit.formation)
                .map(|(_, resources)| resources)
        })
    }

    fn host_upgrade_resource_price(&self, uid: u32, target: Name) -> Option<f64> {
        let spec = self.rules.units.get_interned(target)?;
        if let Some(bill) = self
            .host_unit_facts
            .get(&uid)
            .and_then(|facts| facts.upgrade.as_ref())
            .filter(|offer| offer.to == Some(target))
            .and_then(|offer| offer.resources.as_ref())
            .filter(|bill| {
                bill.cost.is_finite()
                    && bill.cost >= 0.0
                    && (bill.resource == spec.requires_resource
                        || (bill.resource.is_none() && bill.cost == 0.0))
            })
        {
            return Some(bill.cost);
        }
        None
    }

    /// The same offer, with the first failing precondition named. Diagnostics
    /// and the AI both need to tell "no successor exists" apart from "not
    /// here, not yet, or not affordable".
    pub fn unit_gold_upgrade_detail(
        &self,
        pid: usize,
        uid: u32,
    ) -> Result<(Name, f64, f64), &'static str> {
        let unit = self.units.get(&uid).ok_or("no such unit")?;
        if unit.owner != pid {
            return Err("not owned");
        }
        if unit.acted || unit.moves_left <= 0.0 {
            return Err("already acted");
        }
        // ★★★ THE HOST'S OWN VERDICT COMES FIRST, WHEN IT CROSSED
        // (docs/FIDELITY.md, "The one-to-one map", item 9). Everything below
        // this block is the board's model of `CanStartCommand(UNITCOMMAND_
        // UPGRADE)` — territory, successor, price, material — and the
        // mirrored seat now carries that command's real answer per unit
        // (`UnitPanel.lua:468-483`) with `GetUpgradeCost()` as the bill. A
        // named block is final: the order would be refused on arrival, which
        // is what 933 `UPGRADE` refusals over the 08-04/08-05 runs were. A
        // successor the host names is priced at the host's Gold, checked
        // against the treasury this frame still holds. The native material
        // bill crosses separately from `GetUpgradeResourceCost()`. Absent —
        // an older mod, or a unit with no
        // successor at all — the board's own rules decide, unchanged.
        if let Some(verdict) = self
            .host_unit_facts
            .get(&uid)
            .and_then(|facts| facts.upgrade.as_ref())
        {
            if let Some(reason) = verdict.blocked.as_deref() {
                return Err(Self::host_upgrade_refusal(reason));
            }
            if let Some(target) = verdict.to {
                let quote =
                    self.unit_upgrade_price_in_formation(pid, unit.kind, target, unit.formation);
                let gold = verdict
                    .cost
                    .or_else(|| quote.map(|(gold, _)| gold))
                    .unwrap_or(0.0);
                if self.players[pid].gold + f64::EPSILON < gold {
                    return Err("not enough gold");
                }
                let resources = self
                    .unit_upgrade_resource_price(pid, uid, target)
                    .unwrap_or(0.0);
                // The native permission was observed before this planning
                // turn's earlier upgrades spent material. A known exact bill
                // can check the remaining stock, just as the Gold quote does.
                // Older exports retain their authoritative permission because
                // their modeled bill may disagree with the native one.
                if self.host_upgrade_resource_price(uid, target).is_some() {
                    if let Some(resource) = self.rules.units[target].requires_resource {
                        if self.strategic_stockpile(pid, resource) + f64::EPSILON < resources {
                            return Err("not enough strategic material");
                        }
                    }
                }
                return Ok((target, gold, resources));
            }
        }
        if self.is_embarked(unit) {
            return Err("embarked");
        }
        if self.noncombat_action_blocked_by_zoc(uid) {
            return Err("zone of control");
        }
        let territory = self
            .map
            .tiles
            .get(&unit.pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| self.cities.get(&city))
            .map(|city| city.owner)
            .ok_or("neutral territory")?;
        let friendly = territory == pid
            || self.suzerain_of(territory) == Some(pid)
            || self.players[pid]
                .alliances
                .get(&territory)
                .is_some_and(|alliance| alliance.ends > self.turn);
        if !friendly {
            return Err("foreign territory");
        }
        let target = self
            .unit_upgrade_target(pid, unit.kind)
            .ok_or("no unlocked successor")?;
        // A Corps or an Army is priced as one bill, not as a Standard quote
        // multiplied afterwards: the host applies the body count before the
        // minimum and the `PURCHASE_DIVISOR` rounding.
        let (gold, resources) = self
            .unit_upgrade_price_in_formation(pid, unit.kind, target, unit.formation)
            .ok_or("no unlocked successor")?;
        if self.players[pid].gold + f64::EPSILON < gold {
            return Err("not enough gold");
        }
        if let Some(resource) = self.rules.units[&target].requires_resource.as_deref() {
            if resources > 0.0
                && self.strategic_stockpile(pid, Name::new(resource)) + f64::EPSILON < resources
            {
                return Err("not enough strategic material");
            }
        }
        Ok((target, gold, resources))
    }

    /// The host's first `FAILURE_REASONS` entry for a blocked upgrade, folded
    /// onto the board's own refusal vocabulary so a diagnostic reads the same
    /// whichever side answered. The entries are localisation keys or their
    /// looked-up text; the match is on the word.
    fn host_upgrade_refusal(reason: &str) -> &'static str {
        let reason = reason.to_ascii_uppercase();
        if reason.contains("GOLD") {
            "not enough gold"
        } else if reason.contains("RESOURCE") || reason.contains("STRATEGIC") {
            "not enough strategic material"
        } else if reason.contains("TERRITORY") || reason.contains("BORDER") {
            "foreign territory"
        } else if reason.contains("TECH") || reason.contains("CIVIC") {
            "no unlocked successor"
        } else {
            "the host refuses the upgrade this turn"
        }
    }

    pub(super) fn do_upgrade_unit(&mut self, pid: usize, uid: u32) -> Result<(), String> {
        let unit = self.own_unit(pid, uid)?;
        if unit.acted || unit.moves_left <= 0.0 {
            return Err("unit has already acted this turn".into());
        }
        let (target, gold, resources) = self
            .unit_gold_upgrade_detail(pid, uid)
            .map_err(|why| format!("this unit cannot be upgraded here: {why}"))?;
        let class = self.rules.units[&target].promotion_class.clone();
        let charges = self.rules.units[&target].charges;
        let resource = self.rules.units[&target].requires_resource;
        self.players[pid].gold -= gold;
        if let Some(resource) = resource {
            if resources > 0.0 {
                let held = self.strategic_stockpile(pid, resource);
                self.players[pid]
                    .strategic_resources
                    .insert(Name::new(&resource), (held - resources).max(0.0));
            }
        }
        let keep: Vec<Name> = unit
            .promotions
            .iter()
            .filter(|name| {
                self.rules
                    .promotions
                    .get(name)
                    .is_some_and(|spec| spec.class == class)
            })
            .cloned()
            .collect();
        let from = unit.kind;
        let pos = unit.pos;
        let unit = self.units.get_mut(&uid).unwrap();
        unit.kind = Name::new(&target);
        // Promotions survive within the same promotion class; a successor with
        // its own charge budget starts that budget full.
        unit.promotions = keep.into_iter().collect();
        if charges > 0 {
            unit.charges = charges;
        }
        unit.moves_left = 0.0;
        unit.acted = true;
        unit.moved = true;
        unit.attacks_left = 0;
        unit.fortified = false;
        unit.fortify_turns = 0;
        let target_spec = &self.rules.units[&target];
        if target_spec.class == "military" {
            let best = self.players[pid]
                .counters
                .entry("strongest_unit_built".to_string())
                .or_insert(0);
            *best = (*best).max(target_spec.strength.round() as i64);
            if target_spec.ranged_strength > 0.0 {
                let best_ranged = self.players[pid]
                    .counters
                    .entry("strongest_ranged_built".to_string())
                    .or_insert(0);
                *best_ranged = (*best_ranged).max(target_spec.ranged_strength.round() as i64);
            }
        }
        self.note(
            pid,
            "War",
            format!("upgraded {} to {}", pretty(&from), pretty(&target)),
            Some(pos),
        );
        Ok(())
    }
}
