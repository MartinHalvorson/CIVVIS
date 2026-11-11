//! `generals-lead-the-army`: the live bridge keeps a Great General (and Gran
//! Colombia's Comandante General) with each front of our land army instead of
//! retiring it the turn it arrives.
//!
//! ## Why
//!
//! A Great General and a Comandante General give land units within two tiles
//! +5 Combat Strength and +1 Movement for as long as they stand on the board
//! (`GranColombia_Maya_GreatPeople.xml`: `GREATPERSON_COMANDANTE_STRENGTH_AOE_LAND`,
//! amount 5; the Comandante carries `CLASS_ALL_ERAS`, so its aura never
//! expires). Gran Colombia receives one free every era. The engine does not
//! model Great People as board pieces (`advanced/engine_pricing.rs`), so the
//! decider never moves them; the bridge's `great_person_orders` activated
//! every person whose host command was open, and for a General that command
//! is RETIRE. On the live Gran Colombia domination seat (Lv 7 Standard,
//! 2026-11-03/04) every Comandante was retired on its first turn (t53-55,
//! t93-98, t133-138, t174-180), for a random civic, a Trader, one free unit
//! or a promotion, while the army fought at a kill/loss ratio near 1:1.
//! +5 strength is a factor of about e^(5/25) = 1.22 on damage dealt and its
//! inverse on damage taken in every exchange inside the aura.
//!
//! ## What the gene does
//!
//! Live bridge only (`src/bin/civvis_orders/generals_lead_the_army.rs`): each
//! frame the bridge picks posts, one per front, at our land soldiers that lead
//! the most other land soldiers within two tiles, weighted toward visible
//! enemies within four tiles, and walks the nearest General to each post. A
//! General past the number of posts is retired as before, except that the
//! first two Comandantes wait with our largest group of soldiers when no post
//! is worth one (a scattered army gathers again, and that aura never
//! expires; live `civvis-20261111T172329Z` retired all six, the first on the
//! frame its escort dispersed). A General cannot be
//! captured (`CanRetreatWhenCaptured`) and shares a tile with a soldier, so
//! standing with the army risks nothing. `Kind::OptIn`; with the gene off the
//! bridge retires every General exactly as before.
use super::AdvancedAi;

impl AdvancedAi {
    /// Whether the live bridge keeps Generals with the army instead of
    /// retiring them. See `advanced/generals_lead_the_army.rs`.
    pub fn generals_lead_the_army_enabled(&self) -> bool {
        self.generals_lead_the_army
    }
}
