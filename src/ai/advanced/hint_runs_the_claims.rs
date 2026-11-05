//! `hint-runs-the-claims`: the live production hint asks the same claim chain
//! the main production path asks.
//!
//! **Why (live Lv 8 Deity Korea, 2026-11-05).** When a live city's item
//! finishes, the bridge answers Civilization VI's production prompt with a
//! deferred `produce_next` hint (`civvis_orders`, `emit_deferred_production_hints`).
//! The hint comes from [`AdvancedAi::preview_live_production`], which clears
//! that city's queue on a throwaway board and runs only `advanced_production`.
//! The idle-queue claims in `take_turn_inner` never see that queue. Those are
//! the early Settler floor, the holdable-land Settler, granaries, the
//! deterrent, the Trader, the backlog Builder, the Aqueduct, the
//! Neighborhood, the Industrial Zone and Factory, the Campus claims, and the
//! culture and diplomacy claims. By the next frame the host holds the hinted
//! item, so an idle-queue claim finds no idle queue.
//!
//! In live games the hints outnumber direct `produce` orders about two to one
//! (`orders.sqlite`: 480 vs 230 in civvis-20261105T072215Z, 343 vs 192 in
//! 062514Z, 440 vs 226 in 052244Z). Two thirds of live production choices
//! therefore skip every claim gene. The native board has no hint, so every
//! empty queue there goes through the chain.
//!
//! That is the live/sim gap the state census found. A Korea-seat sim and
//! the live games agree on tile economics: the improved share of worked tiles
//! and the production per worked tile match within a few percent. But the sim
//! seat holds 7.5 cities at t100 against 5 live, and 3 Industrial Zones at
//! t80 against 0.5 (`claim_producer_zone`).
//!
//! **What it does.** [`AdvancedAi::run_idle_queue_claims`] is the claim chain,
//! moved verbatim out of `take_turn_inner`, which still calls it in the same
//! place: off, nothing changes. With the gene on and the opening book done,
//! the preview runs it on its throwaway board before `advanced_production`.
//! The finishing city's hint is then what the main path would have chosen
//! for an empty queue. After the governor it also runs the defensive writers
//! that follow the governors on the main path
//! ([`AdvancedAi::run_hint_defence_writers`]: the capital's and the frontier's
//! Walls, the surprise-war wave, the threatened city's defender), so a claim
//! never takes a hint the main path would give to Walls. The preview is a clone
//! of the agent, so its journal is silent (`Journal::clone`) and none of its
//! claims reach the live `why.log`.

use super::*;

/// Turns after a hit on a city during which its hint keeps the governor's
/// own choice. Matches the production scorer's four-turn `threatened` window.
pub(crate) const HINT_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The idle-queue claims, in the order `take_turn_inner` has always run
    /// them. Each is an exact no-op while its own gene is off. Returns
    /// `threatened-war-holds-the-claims`' hold, which the later claims in
    /// `take_turn_inner` also read.
    pub(super) fn run_idle_queue_claims(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        // `forward-airfield-project`: a project already in flight takes
        // its producer's idle queue for the next stage before the routine
        // claims below refill it. Exact no-op while the gene is off or no
        // project is in flight. See `claim_forward_airfield_stage_in_flight`.
        // `threatened-war-holds-the-claims`: while a war with a major
        // threatens a home city, the economy's idle-queue claims below
        // stand aside so those queues reach the war target. Exact no-op
        // while the gene is off. See
        // `advanced/threatened_war_holds_the_claims.rs`.
        let war_holds = self.threatened_war_holds_the_claims_holds(g, pid, plan);
        self.claim_forward_airfield_stage_in_flight(g, pid, plan);
        // `early-settler-floor`: a Settler for the opening's band floor
        // goes first, ahead of the Prophet race and the routine claims
        // below. Exact no-op while the gene is off. See
        // `advanced/early_settler_floor.rs`.
        self.claim_early_settler_floor(g, pid, plan);
        // `holdable-land-keeps-settling`: past the opening floor, a
        // Settler for a holdable seat, ahead of the routine claims below.
        // Exact no-op while the gene is off. See
        // `advanced/holdable_land_keeps_settling.rs`.
        self.claim_holdable_land_settler(g, pid, plan);
        // `capital-monument-after-the-second-city`: the capital's Monument,
        // once the second city stands, after the floor's Settler and ahead
        // of the routine claims. Exact no-op while the gene is off. See
        // `advanced/capital_monument.rs`.
        if !war_holds {
            self.claim_capital_monument(g, pid, plan);
        }
        // `granary-claims-the-bound-queue`: a housing-bound city's Granary
        // ahead of the deterrent and the routine claims below; a due
        // Settler and the faith defence above keep their order. See
        // `advanced/granary_claims_the_bound_queue.rs`.
        if !war_holds {
            self.claim_bound_granaries(g, pid, plan);
        }
        // `near-rival-deterrence`: a land unit while a neighbour at peace
        // out-guns us, after the floor's Settler and ahead of the Prophet
        // race and the routine claims. Exact no-op while the gene is off.
        // See `advanced/near_rival_deterrence.rs`.
        self.claim_deterrence_unit(g, pid, plan);
        // `trader-fills-the-idle-route`: an empty trade route slot's
        // Trader, after the floor's Settler, the Granary and the deterrent
        // and ahead of the Prophet race and the routine claims. Exact
        // no-op while the gene is off. See
        // `advanced/trader_fills_the_idle_route.rs`.
        if !war_holds {
            self.claim_trader_for_idle_route(g, pid, plan);
        }
        // `backlog-builder-takes-the-queue`: the worked backlog's Builder,
        // after the Trader and ahead of the Prophet race and the routine
        // claims. Exact no-op while the gene is off. See
        // `advanced/backlog_builder_takes_the_queue.rs`.
        if !war_holds {
            self.claim_backlog_builder(g, pid, plan);
        }
        // `aqueduct-claims-the-capped-queue`: the most capped city's legal
        // Aqueduct, after the backlog Builder and ahead of the Prophet race
        // and the routine claims. Exact no-op while the gene is off. See
        // `advanced/aqueduct_claims_the_capped_queue.rs`.
        if !war_holds {
            self.claim_capped_aqueduct(g, pid, plan);
        }
        // `neighborhood-houses-the-capped-city`: after the Aqueduct, the
        // most capped city's Neighborhood. Exact no-op while the gene is
        // off. See `advanced/neighborhood_houses_the_capped_city.rs`.
        if !war_holds {
            self.claim_capped_neighborhood(g, pid, plan);
        }
        // `sewer-claims-the-capped-queue`: the last of the housing claims.
        // Exact no-op while the gene is off. See
        // `advanced/sewer_claims_the_capped_queue.rs`.
        if !war_holds {
            self.claim_capped_sewer(g, pid, plan);
        }
        // `industrial-zone-claims-the-producer-queue`: the producer step's
        // Industrial Zone or Workshop, after the Aqueduct and ahead of the
        // Prophet race and the routine claims. Exact no-op while the gene
        // is off. See `advanced/industrial_zone_claims_the_producer_queue.rs`.
        if !war_holds {
            self.claim_producer_zone(g, pid, plan);
        }
        // `industrial-zone-claims-the-factory`: the Factory that reaches
        // the most new cities, or the plant that powers the most Factories,
        // after the Industrial Zone and ahead of the Campus building claim.
        // Exact no-op while the gene is off. See
        // `advanced/industrial_zone_claims_the_factory.rs`.
        if !war_holds {
            self.claim_factory(g, pid, plan);
        }
        // `campus-building-claims-the-queue`: a standing Campus's next
        // building, after the Industrial Zone and ahead of the Prophet race
        // and the routine claims. Exact no-op while the gene is off. See
        // `advanced/campus_building_claims_the_queue.rs`.
        if !war_holds {
            self.claim_campus_building(g, pid, plan);
        }
        // `campus-claims-the-free-slot`: a free district slot's first
        // Campus, after the Campus building claim and ahead of the Prophet
        // race and the routine claims. Exact no-op while the gene is off.
        // See `advanced/campus_claims_the_free_slot.rs`.
        if !war_holds {
            self.claim_free_slot_campus(g, pid, plan);
        }
        // `culture-seat-theater-first`: a Culture seat's first Theater
        // Squares, after the Campus claims (which yield those cities) and
        // ahead of the Prophet race and the routine claims. Exact no-op
        // while the gene is off. See `advanced/culture_theater_first.rs`.
        if !war_holds {
            self.claim_culture_theaters(g, pid, plan);
        }
        // `culture-seat-monuments`: a Culture seat's Monuments, after the
        // Theater claim (whose heads they leave alone). Exact no-op while
        // the gene is off. See `advanced/culture_monuments.rs`.
        if !war_holds {
            self.claim_culture_monuments(g, pid, plan);
        }
        // `diplo-seat-statue-harbor`: the Statue of Liberty's Harbor on a
        // Diplomacy seat. Exact no-op while the gene is off. See
        // `advanced/diplo_statue_harbor.rs`.
        if !war_holds {
            self.claim_statue_harbor(g, pid, plan);
        }
        // `diplo-seat-wonder-claim`: one Diplomatic-point wonder in the
        // strongest city of a Diplomatic seat, after the Culture claims
        // and ahead of the Prophet race and the routine claims. Exact
        // no-op while the gene is off. See `advanced/diplo_wonder_claim.rs`.
        if !war_holds {
            self.claim_diplo_wonder(g, pid, plan);
        }
        // `prophet-builds-its-site`: the Holy Site a Prophet founds on
        // goes first. Exact no-op while the gene is off.
        self.reserve_prophet_site(g, pid, plan);
        // `prophet-race-earns-its-points`: the race's Holy Site city
        // starts its Shrine. Exact no-op while the gene is off.
        self.reserve_race_shrine(g, pid, plan);
        // `pillaged-districts-repair-first`: a pillaged district's repair,
        // after every claim above so none of them takes the queue back.
        // Exact no-op while the gene is off. See
        // `advanced/pillaged_repairs_first.rs`.
        if !war_holds {
            self.claim_pillaged_repairs(g, pid);
        }
        war_holds
    }

    /// The defensive writers that `take_turn_inner` runs after the governors,
    /// in its order, restricted to those that need no turn-start state: the
    /// capital's and the frontier's Walls before a surprise war, the surprise
    /// war's defensive wave, and the threatened city's defender. Under
    /// `hint-runs-the-claims` the preview runs them after `advanced_production`,
    /// so a claim never takes a hint the main path would have given to Walls.
    /// Live replay (civvis-20261105T133505Z t80/t90) and -78's 188-board census
    /// showed the claims alone displacing Walls on hint turns.
    pub(super) fn run_hint_defence_writers(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        self.claim_capital_walls_before_the_surprise(g, pid);
        self.claim_frontier_walls_before_the_surprise(g, pid);
        self.mobilize_surprise_defense_production(g, pid);
        self.redirect_unsafe_city_queue_for_defense(g, pid, plan.threatened_city);
    }

    /// Whether the city a hint is for stands in the war's reach: hit within
    /// [`HINT_ATTACK_MEMORY`] turns, or under visible or remembered pressure
    /// from a hostile military unit inside the threat radius (six tiles,
    /// barbarians included). Such a city keeps the governor's own choice, and
    /// neither the claims nor the extra writers run for its hint.
    ///
    /// -78's census of 523 live boards (every third turn t60-150, 28 Deity
    /// Korea games, ± the gene with #4565 in) found 10 of the 12 lost Walls
    /// hints in cities with a hostile within six tiles.
    pub(super) fn hint_city_in_reach_of_war(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= HINT_ATTACK_MEMORY
        {
            return true;
        }
        let visible = g.player_vision_frame(pid);
        self.city_pressure_with_belief(g, pid, cid, &visible) > 0.0
    }
}

#[cfg(test)]
mod tests;
