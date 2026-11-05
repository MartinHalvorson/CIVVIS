//! The Moon: its deposits, and the mass drivers that bring them home.
//!
//! Rolling what the Moon is made of (`roll_moon_deposits`), a deposit and
//! its ores (`moon_deposit`, `moon_ores`), a player's mass drivers, drawing
//! from the Moon, aiming a mass driver (`do_aim_mass_driver`), and the
//! turn's `process_mass_drivers`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Roll what the Moon is made of.
    ///
    /// There is one Moon, so this runs once at setup rather than once per
    /// civilization, and what a mass driver takes out of a pile is gone for
    /// everybody. A ruleset that puts nothing up there — every classic game —
    /// rolls nothing and consumes no randomness at all, so a seed still makes
    /// exactly the world it always made.
    pub(super) fn roll_moon_deposits(rules: &Rules, rng: &mut Rng) -> BTreeMap<Name, f64> {
        rules
            .resources
            .iter()
            .filter_map(|(ore, spec)| spec.lunar.map(|lunar| (*ore, lunar)))
            .map(|(ore, lunar)| {
                let low = lunar.min.min(lunar.max).max(0.0);
                let high = lunar.max.max(lunar.min).max(low);
                (ore, rng.uniform(low, high).round())
            })
            .collect()
    }

    /// What is left of one of the Moon's piles. Zero for an ore the Moon was
    /// never rolled with, which is all of them in a classic game.
    pub fn moon_deposit(&self, ore: impl AsName) -> f64 {
        self.moon_deposits
            .get(&ore.as_name())
            .copied()
            .unwrap_or(0.0)
    }

    /// The ores this world's Moon was rolled with, in ruleset order, whether
    /// or not any of each is left.
    pub fn moon_ores(&self) -> Vec<Name> {
        self.moon_deposits.keys().copied().collect()
    }

    /// How many mass drivers this civilization has standing on the Moon.
    pub fn mass_drivers(&self, pid: usize) -> i64 {
        self.players[pid]
            .counters
            .get("project_effect:mass_drivers")
            .copied()
            .unwrap_or(0)
    }

    /// Take up to `wanted` units out of one lunar pile and report what the
    /// Moon actually had. The piles are shared, so this is the single place
    /// they are drawn down.
    fn draw_from_moon(&mut self, ore: Name, wanted: f64) -> f64 {
        let Some(pile) = self.moon_deposits.get_mut(&ore) else {
            return 0.0;
        };
        let drawn = pile.min(wanted).max(0.0).floor();
        *pile -= drawn;
        drawn
    }

    /// Land this turn's cargo.
    ///
    /// Every driver throws one unit of the aimed ore down onto the aimed site
    /// and the Moon is one unit lighter for it — the whole of the mechanic is
    /// that a pile somewhere else pays for a stockpile here. Nothing is drawn
    /// that the stockpile has no room to hold: the ore is finite and shared,
    /// and spilling a rival's share onto a full warehouse is not worth
    /// modelling.
    ///
    /// A driver that fired at something during the previous turn still
    /// delivers now. Firing already cost a unit of the same ore out of the
    /// stockpile — the slug *is* the metal — so a strike is a turn's cargo
    /// spent on a target rather than a separate magazine.
    pub(super) fn process_mass_drivers(&mut self, pid: usize) {
        self.players[pid].mass_driver_shots = 0;
        let drivers = self.mass_drivers(pid);
        if drivers <= 0 || self.players[pid].is_barbarian {
            return;
        }
        let (Some(ore), Some(site)) = (
            self.players[pid].mass_driver_ore,
            self.players[pid].mass_driver_site,
        ) else {
            return;
        };
        // Ground that has been taken, or lost with the city that held it,
        // catches nothing until the drivers are aimed somewhere else.
        if self.tile_owner(site) != Some(pid) {
            return;
        }
        let room =
            (self.strategic_stockpile_capacity(pid) - self.strategic_stockpile(pid, ore)).max(0.0);
        let landed = self.draw_from_moon(ore, (drivers as f64).min(room.floor()));
        if landed <= 0.0 {
            return;
        }
        let stock = self.strategic_stockpile(pid, ore) + landed;
        self.players[pid].strategic_resources.insert(ore, stock);
        *self.players[pid]
            .counters
            .entry("mass_driver_landings".to_string())
            .or_insert(0) += landed as i64;
    }

    /// Aim every mass driver at one ore and one landing site.
    ///
    /// A standing order rather than a shot: it holds until it is given again,
    /// which is what makes the drivers infrastructure instead of an action to
    /// repeat every turn. Until one is given, nothing falls.
    pub(super) fn do_aim_mass_driver(
        &mut self,
        pid: usize,
        site: Pos,
        ore: Name,
    ) -> Result<(), String> {
        if self.mass_drivers(pid) <= 0 {
            return Err("no mass driver on the Moon".into());
        }
        if self.moon_deposit(ore) <= 0.0 {
            return Err("the Moon has none of that ore left".into());
        }
        if !self.resource_visible_to(pid, ore.as_str()) {
            return Err("that resource is not known to this civilization".into());
        }
        if self.tile_owner(site) != Some(pid) {
            return Err("the landing site must be inside your own borders".into());
        }
        self.players[pid].mass_driver_site = Some(site);
        self.players[pid].mass_driver_ore = Some(ore);
        Ok(())
    }
}
