//! `settler-pump-leans-on-production`: on the native Civ VI seat,
//! `civvis_orders` asks the host's city governor to favour Production in a
//! city training a Settler on a Food surplus of
//! `SETTLER_PUMP_MIN_FOOD` (4) or more and a Production of
//! `SETTLER_PUMP_MAX_PRODUCTION` (5) or less, and releases that preference
//! once the Settler is done (constants in
//! `src/bin/civvis_orders/citizen_growth.rs`).
//!
//! **Why (hotseat `hotseat-20261128T031456Z`, 2026-11-27, Online speed).**
//! Seat p3 (Cree, Science target) started its first Settler in its capital
//! on turn 7 and was still training it on turn 16, the last turn played:
//! ten turns at 3-4 Production on a Food surplus of 4-8, with no citizen
//! preference set (`food_favored`, `food_disfavored` and
//! `food_focus_managed` all false on every `state` frame). The capital grew
//! from 2 to 5 citizens in those turns while it made 4 Production. Seat p2
//! (Poland) met the same test on turns 8 and 11 (Food +4, Production 4-5).
//! No other seat did: p1, p4 and p5 trained their Settlers at 7-11
//! Production on +2-3 Food.
//!
//! **What the host offers.** The city panel's yield focus is one command
//! for every yield: `Base/Assets/UI/Panels/CityPanel.lua:445-458`
//! (`SetYieldFocus(yieldType)`: `PARAM_FLAGS = 0`, `PARAM_YIELD_TYPE =
//! yieldType`, `PARAM_DATA0 = 1/0`, then
//! `CityManager.RequestCommand(g_pCity, CityCommandTypes.SET_FOCUS, ...)`).
//! The host still allocates the citizens; the preference only leans it.
//!
//! **What the agent mod takes today.** Only a Food focus: its `city_focus`
//! handler refuses any other verb as `unknown_food_focus`
//! (`tools/civ6_control/mod/CivvisControlAgent.lua`, `if verb ~=
//! "FAVOR_FOOD" and verb ~= "RELEASE_FOOD" then return false,
//! "unknown_food_focus"; end`), `CivvisCitizenGrowth.command` sets
//! `PARAM_YIELD_TYPE = YieldTypes.FOOD`, and the export reads back only the
//! Food preference. So until the mod takes `FAVOR_PRODUCTION` /
//! `RELEASE_PRODUCTION` and exports `production_favored` /
//! `production_focus_managed`, this gene's request is refused by the mod, its
//! verdict is `production_focus_not_exported`, and the three-strike rule
//! (`ORDER_REFUSAL_STRIKES`) withholds it for ten turns at a time. It
//! releases only a preference the export says is ours, so it never releases
//! anything today.
//!
//! **The rule.** A city whose queue head is a Settler, with a finite Food
//! surplus of at least 4 and a finite Production of at most 5, no Food
//! preference set and no Production preference set, is asked to favour
//! Production. While the export says the Production preference is ours, it
//! stays until the Settler leaves the queue or the city's Food surplus goes
//! negative, and is then released. A preference set elsewhere is never
//! released.
//!
//! Off, `civvis_orders` asks exactly what it did before. The simulator issues
//! no city-focus orders, so the gene acts only on the native seat.

use super::*;

impl AdvancedAi {
    /// `settler-pump-leans-on-production`: whether `civvis_orders` asks the
    /// host for a Production focus in a slow settler pump. See the module
    /// notes.
    pub fn settler_pump_leans_on_production(&self) -> bool {
        self.settler_pump_leans_on_production
    }
}
