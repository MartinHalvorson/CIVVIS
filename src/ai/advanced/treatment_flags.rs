//! Every `enable_*` / `disable_*` capability toggle on `AdvancedAi`.
//!
//! ★★★ THIS IS THE SECOND OF THE TWO ANCHORS A TREATMENT PR TOUCHES, AND THE
//! ONE THAT HAS ACTUALLY CORRUPTED A MERGE. `treatments.rs` moved the
//! `LIVE_TREATMENTS` table out on 2026-08-18 (#2022) and recorded in its own
//! header that the toggle pair still landed in `advanced.rs`, that two PRs
//! adding a pair at the same anchor had collided there twice on 2026-08-16,
//! and that one of those resolutions swallowed the earlier function's closing
//! brace. It called moving the toggles "its own change with its own risk" and
//! deliberately shipped the small half. This is the other half.
//!
//! `tools/conflict_hotspots.py` measures `src/ai/advanced.rs` at 24% of the
//! last 200 merges, first by a wide margin. `docs/ROADMAP.md` objective 5
//! separates the two reasons a file is contended: **size**, which splitting
//! answers, and **one shared line or list**, which it does not — two PRs
//! appending at the same anchor conflict whatever the file's length. The
//! toggles are the second kind. They are 182 near-identical four-line
//! functions with no readers other than the field they set, so a conflict
//! between two of them is content-free; what made it dangerous was that it
//! happened at line 3,859 of twenty-eight thousand, surrounded by code a
//! careless resolution could take with it.
//!
//! Here, a toggle collision is a collision between two four-line functions in
//! a file that contains nothing else. It is still a conflict. It can no longer
//! swallow anything.
//!
//! ⚠ **The move is only worth anything if it stays moved.** A guard in this
//! file fails when an `enable_*` or `disable_*` method is defined in
//! `advanced.rs` again, and when a function that is *not* a toggle is defined
//! here — so this does not become the next dumping ground. Both run in
//! `cargo test`, which is a required check.
//!
//! Two `impl` blocks. The first is one `toggles!` entry per gene whose pair
//! does nothing but set one flag — most of them. A new gene of that kind adds
//! one entry under the append marker for its flag's first letter: its doc
//! comment, then `flag => enable_flag, disable_flag;`. The second holds every toggle that
//! does more, written out by hand. Until 2026-11-04 the plain pairs were
//! written out too: about 4,500 of the file's 9,600 lines.
//!
//! ⚠ Adding a gene still touches more than this file: the flag field and its
//! constructor default in `advanced.rs` (under their letter's append marker),
//! and the row in the gene registry, `genes.rs`. See
//! `docs/GENE_SCREEN.md`.

use super::AdvancedAi;

/// Writes the plain toggles: `enable_*` sets one flag `true` and `disable_*`
/// sets it `false`, and neither does anything else.
///
/// An entry is `flag => enable_name, disable_name;`, with the gene's doc
/// comment above it; the doc goes on `enable_*`, which is where
/// `tools/genes.py` reads a gene's one-line description from. `flag` may be a
/// path (`base.naval_recon`) for a gene `BasicAi` reads. Both names are
/// spelled out so that a search for either finds its definition. A toggle
/// that does anything more — a family version that clears its siblings, a
/// flag mirrored onto `BasicAi`, a delegation, a bundle — is written by hand
/// in the second `impl` below.
macro_rules! toggles {
    ($(
        $(#[$doc:meta])*
        $($flag:ident).+ => $enable:ident, $disable:ident;
    )*) => {
        $(
            $(#[$doc])*
            pub fn $enable(&mut self) {
                self.$($flag).+ = true;
            }

            #[doc = concat!("The twin of [`Self::", stringify!($enable), "`].")]
            pub fn $disable(&mut self) {
                self.$($flag).+ = false;
            }
        )*
    };
}

impl AdvancedAi {
    // New plain toggles go here, under the marker for the flag's first letter,
    // so that gene pull requests in different ranges do not meet on one line.
    // `tools/test_treatment_append_points.py` holds the block below to its size.
    toggles! {
        // ---- append: a-b --------------------------------------------
        /// Opt-in gene `apostle-evangelizes-unspent`; see
        /// `advanced/apostle_evangelizes_unspent.rs`.
        apostle_evangelizes_unspent => enable_apostle_evangelizes_unspent, disable_apostle_evangelizes_unspent;
        /// `battle-solver`: see `advanced/battle_solver.rs`.
        battle_solver => enable_battle_solver, disable_battle_solver;
        /// `band-plays-its-stage`: Rock Bands take the promotions and tours that
        /// keep them alive, because album sales compound over a band's career.
        base.band_plays_its_stage => enable_band_plays_its_stage, disable_band_plays_its_stage;
        /// `bands-draw-from-the-bar`: see `advanced/bands_draw_from_the_bar.rs`.
        base.bands_draw_from_the_bar => enable_bands_draw_from_the_bar, disable_bands_draw_from_the_bar;
        barbarian_research_prices_the_field => enable_barbarian_research_prices_the_field, disable_barbarian_research_prices_the_field;
        /// `acropolis-before-the-deterrent`: see
        /// `advanced/acropolis_before_the_deterrent.rs`.
        acropolis_before_the_deterrent => enable_acropolis_before_the_deterrent, disable_acropolis_before_the_deterrent;
        /// Opt-in gene `kristina-builds-the-bibliotheque`; see
        /// `advanced/kristina_builds_the_bibliotheque.rs`.
        base.kristina_builds_the_bibliotheque => enable_kristina_builds_the_bibliotheque, disable_kristina_builds_the_bibliotheque;
        /// Opt-in gene `kristina-fills-to-theme`; see
        /// `advanced/kristina_fills_to_theme.rs`.
        base.kristina_fills_to_theme => enable_kristina_fills_to_theme, disable_kristina_fills_to_theme;
        /// Opt-in gene `band-plays-what-it-can-reach`; see
        /// `advanced/band_plays_what_it_can_reach.rs`.
        base.band_plays_what_it_can_reach => enable_band_plays_what_it_can_reach, disable_band_plays_what_it_can_reach;
        /// Opt-in gene `bands-wait-at-the-door`; see
        /// `advanced/bands_wait_at_the_door.rs`.
        base.bands_wait_at_the_door => enable_bands_wait_at_the_door, disable_bands_wait_at_the_door;
        /// Opt-in gene `band-plays-at-level-three`; see
        /// `advanced/band_plays_at_level_three.rs`.
        base.band_plays_at_level_three => enable_band_plays_at_level_three, disable_band_plays_at_level_three;
        // ---- append: c-d --------------------------------------------
        /// Opt-in gene `corps-comes-home-alive`; see
        /// `advanced/apostle_army.rs`.
        corps_comes_home_alive => enable_corps_comes_home_alive, disable_corps_comes_home_alive;
        /// `culture-seat-drama-first`: see `advanced/culture_drama_first.rs`.
        culture_seat_drama_first => enable_culture_seat_drama_first, disable_culture_seat_drama_first;
        /// `culture-seat-drama-before-government`: see
        /// `advanced/culture_drama_first.rs`.
        culture_seat_drama_before_government => enable_culture_seat_drama_before_government, disable_culture_seat_drama_before_government;
        /// `culture-keeps-its-aesthetics`: see `advanced/culture_aesthetics.rs`.
        culture_keeps_its_aesthetics => enable_culture_keeps_its_aesthetics, disable_culture_keeps_its_aesthetics;
        /// `culture-banks-for-the-bands`: see `advanced/culture_band_bank.rs`.
        culture_banks_for_the_bands => enable_culture_banks_for_the_bands, disable_culture_banks_for_the_bands;
        /// `culture-faith-opens-at-the-theaters`: see
        /// `advanced/culture_faith_sources.rs`.
        culture_faith_opens_at_the_theaters => enable_culture_faith_opens_at_the_theaters, disable_culture_faith_opens_at_the_theaters;
        /// `domination-clock-every-rung`: an assigned Domination lane leaves its
        /// development half at 30% of the clock at every difficulty. See
        /// `AdvancedAi::domination_clock_every_rung`.
        domination_clock_every_rung => enable_domination_clock_every_rung, disable_domination_clock_every_rung;
        /// `domination-spends-the-stock`: a Domination seat's delegated governor
        /// takes a resource-priced melee unit whenever a city can start one.
        /// See `AdvancedAi::domination_spends_the_stock`.
        domination_spends_the_stock => enable_domination_spends_the_stock, disable_domination_spends_the_stock;
        /// `chop-feeds-the-long-build`: a Builder clears a forest into a district,
        /// a wonder or a space-race project. See
        /// `advanced/chop_feeds_the_long_build.rs`.
        chop_feeds_the_long_build => enable_chop_feeds_the_long_build, disable_chop_feeds_the_long_build;
        /// `chop-feeds-the-space-race`: the space-race leg of
        /// `chop-feeds-the-long-build` alone. See
        /// `advanced/chop_feeds_the_long_build.rs`.
        chop_feeds_the_space_race => enable_chop_feeds_the_space_race, disable_chop_feeds_the_space_race;
        /// `chop-spares-the-worked-tile`: no chop errand clears a tile a citizen
        /// is working now. See `AdvancedAi::chop_spares`.
        chop_spares_the_worked_tile => enable_chop_spares_the_worked_tile, disable_chop_spares_the_worked_tile;
        /// `cold-war-before-conservation`: see
        /// `advanced/cold_war_before_conservation.rs`.
        cold_war_before_conservation => enable_cold_war_before_conservation, disable_cold_war_before_conservation;
        /// `culture-bank-opens-at-nationalism`: see `advanced/culture_band_bank.rs`.
        culture_bank_opens_at_nationalism => enable_culture_bank_opens_at_nationalism, disable_culture_bank_opens_at_nationalism;
        /// `culture-road-earns-its-inspirations`: see
        /// `advanced/culture_road_inspirations.rs`.
        culture_road_earns_its_inspirations => enable_culture_road_earns_its_inspirations, disable_culture_road_earns_its_inspirations;
        /// `culture-seat-builds-past-the-deterrent`: see
        /// `advanced/culture_past_the_deterrent.rs`.
        culture_seat_builds_past_the_deterrent => enable_culture_seat_builds_past_the_deterrent, disable_culture_seat_builds_past_the_deterrent;
        /// Opt-in gene `culture-digs-every-site`; see
        /// `advanced/culture_digs_every_site.rs`.
        culture_digs_every_site => enable_culture_digs_every_site, disable_culture_digs_every_site;
        /// Opt-in gene `curator-sits-with-the-works`; see
        /// `advanced/curator_sits_with_the_works.rs`.
        curator_sits_with_the_works => enable_curator_sits_with_the_works, disable_curator_sits_with_the_works;
        /// `culture-faith-site-per-theater`: see
        /// `advanced/culture_faith_sources.rs`.
        culture_faith_site_per_theater => enable_culture_faith_site_per_theater, disable_culture_faith_site_per_theater;
        /// `culture-faith-site-per-theater-2`: see
        /// `advanced/culture_faith_sources.rs`.
        culture_faith_site_per_theater_2 => enable_culture_faith_site_per_theater_2, disable_culture_faith_site_per_theater_2;
        /// `culture-road-builds-its-triggers`: see
        /// `advanced/culture_road_triggers.rs`.
        culture_road_builds_its_triggers => enable_culture_road_builds_its_triggers, disable_culture_road_builds_its_triggers;
        /// `culture-bank-holds-the-counterweight`: see
        /// `advanced/culture_band_bank.rs`.
        culture_bank_holds_the_counterweight => enable_culture_bank_holds_the_counterweight, disable_culture_bank_holds_the_counterweight;
        /// `culture-gold-buys-the-shrines`: see
        /// `advanced/culture_faith_sources.rs`.
        culture_gold_buys_the_shrines => enable_culture_gold_buys_the_shrines, disable_culture_gold_buys_the_shrines;
        /// `culture-seat-appoints-pingala-first`: see
        /// `advanced/culture_pingala_first.rs`.
        culture_seat_appoints_pingala_first => enable_culture_seat_appoints_pingala_first, disable_culture_seat_appoints_pingala_first;
        /// `denial-war-needs-the-lane-edge`: see
        /// `advanced/denial_war_needs_the_lane_edge.rs`.
        denial_war_needs_the_lane_edge => enable_denial_war_needs_the_lane_edge, disable_denial_war_needs_the_lane_edge;
        /// `culture-road-takes-the-unique-first`: see
        /// `advanced/culture_unique_first.rs`.
        culture_road_takes_the_unique_first => enable_culture_road_takes_the_unique_first, disable_culture_road_takes_the_unique_first;
        /// Opt-in gene `city-strike-takes-the-gun`; see
        /// `advanced/siege_at_the_walls_dies_first.rs`.
        city_strike_takes_the_gun => enable_city_strike_takes_the_gun, disable_city_strike_takes_the_gun;
        /// `domination-charts-every-capital`: see
        /// `advanced/domination_charts_every_capital.rs`.
        domination_charts_every_capital => enable_domination_charts_every_capital, disable_domination_charts_every_capital;
        /// `domination-target-gets-its-breaker`: see
        /// `advanced/domination_target_gets_its_breaker.rs`.
        domination_target_gets_its_breaker => enable_domination_target_gets_its_breaker, disable_domination_target_gets_its_breaker;
        /// `culture-keeps-pingala-home`: see
        /// `advanced/culture_pingala_first.rs`.
        culture_keeps_pingala_home => enable_culture_keeps_pingala_home, disable_culture_keeps_pingala_home;
        // ---- append: e-f --------------------------------------------
        /// Opt-in `faith-strike-wipes-the-spreaders`; see
        /// `AdvancedAi::faith_strike_wipes_the_spreaders`.
        faith_strike_wipes_the_spreaders => enable_faith_strike_wipes_the_spreaders, disable_faith_strike_wipes_the_spreaders;
        /// Opt-in `faith-strike-at-a-quiet-parity`; see
        /// `AdvancedAi::faith_strike_at_a_quiet_parity`.
        faith_strike_at_a_quiet_parity => enable_faith_strike_at_a_quiet_parity, disable_faith_strike_at_a_quiet_parity;
        /// `flower-power-in-the-dark`: see `advanced/flower_power_in_the_dark.rs`.
        flower_power_in_the_dark => enable_flower_power_in_the_dark, disable_flower_power_in_the_dark;
        /// `first-capital-before-the-walls`: see
        /// `advanced/first_capital_before_the_walls.rs`.
        first_capital_before_the_walls => enable_first_capital_before_the_walls, disable_first_capital_before_the_walls;
        /// Opt-in gene `every-shooter-at-the-walls`; see
        /// `advanced/siege_at_the_walls_dies_first.rs`.
        every_shooter_at_the_walls => enable_every_shooter_at_the_walls, disable_every_shooter_at_the_walls;
        // ---- append: g-k --------------------------------------------
        /// `gutted-rival-passes-the-edge`: a staged declaration by a Domination
        /// lane passes the peak edge against a target at 60% or less of its
        /// 30-turn peak, at 1.5 times what it has left. See
        /// `AdvancedAi::gutted_rival_passes_the_edge`.
        gutted_rival_passes_the_edge => enable_gutted_rival_passes_the_edge, disable_gutted_rival_passes_the_edge;
        /// Opt-in gene `guru-walks-to-the-wounded`; see
        /// `advanced/guru_walks_to_the_wounded.rs`.
        guru_walks_to_the_wounded => enable_guru_walks_to_the_wounded, disable_guru_walks_to_the_wounded;
        /// Opt-in gene `good-land-lifts-the-city-target`; see
        /// `advanced/good_land_lifts_the_city_target.rs`.
        good_land_lifts_the_city_target => enable_good_land_lifts_the_city_target, disable_good_land_lifts_the_city_target;
        /// Opt-in gene `great-works-never-wait`; see
        /// `advanced/great_works_never_wait.rs`.
        great_works_never_wait => enable_great_works_never_wait, disable_great_works_never_wait;
        // ---- append: l-o --------------------------------------------
        /// Opt-in gene `last-charge-joins-the-army`; see
        /// `advanced/apostle_army.rs`.
        last_charge_joins_the_army => enable_last_charge_joins_the_army, disable_last_charge_joins_the_army;
        /// Opt-in `modernization-prices-the-path`; see `modernization_prices_the_path`.
        modernization_prices_the_path => enable_modernization_prices_the_path, disable_modernization_prices_the_path;
        // ---- append: p-r --------------------------------------------
        /// Opt-in `religion-seat-cuts-the-upkeep`; see `advanced/religion_seat_cuts_the_upkeep.rs`.
        religion_seat_cuts_the_upkeep => enable_religion_seat_cuts_the_upkeep, disable_religion_seat_cuts_the_upkeep;
        /// See `advanced/religion_seat_keeps_the_guard.rs`.
        religion_seat_keeps_the_guard => enable_religion_seat_keeps_the_guard, disable_religion_seat_keeps_the_guard;
        /// Opt-in gene `russia-settles-the-tundra`; see
        /// `advanced/russia_tundra.rs`.
        russia_settles_the_tundra => enable_russia_settles_the_tundra, disable_russia_settles_the_tundra;
        /// Opt-in gene `religion-seat-first-shrine`; see
        /// `advanced/religion_seat_first_shrine.rs`.
        religion_seat_first_shrine => enable_religion_seat_first_shrine, disable_religion_seat_first_shrine;
        /// Opt-in gene `religion-seat-guards-the-race`; see
        /// `advanced/religion_seat_guards_the_race.rs`.
        religion_seat_guards_the_race => enable_religion_seat_guards_the_race, disable_religion_seat_guards_the_race;
        /// Opt-in gene `religion-seat-earns-its-prophet`; see
        /// [`Self::religion_seat_earns_its_prophet`].
        religion_seat_earns_its_prophet => enable_religion_seat_earns_its_prophet, disable_religion_seat_earns_its_prophet;
        /// See `advanced/religion_seat_reformed_church.rs`.
        religion_seat_reaches_reformed_church => enable_religion_seat_reaches_reformed_church, disable_religion_seat_reaches_reformed_church;
        /// See `advanced/religion_seat_reveals_the_capitals.rs`.
        religion_seat_reveals_the_capitals => enable_religion_seat_reveals_the_capitals, disable_religion_seat_reveals_the_capitals;
        /// See `advanced/religion_denial_needs_the_edge.rs`.
        religion_denial_needs_the_edge => enable_religion_denial_needs_the_edge, disable_religion_denial_needs_the_edge;
        /// Slot Religious Orders over a lower-ranked ordinary Economic card.
        religion_seat_slots_the_orders => enable_religion_seat_slots_the_orders, disable_religion_seat_slots_the_orders;
        /// See `advanced/religion_seat_flips_the_cheapest.rs`.
        religion_seat_flips_the_cheapest => enable_religion_seat_flips_the_cheapest, disable_religion_seat_flips_the_cheapest;
        /// See `advanced/religion_seat_fields_apostles.rs`.
        religion_seat_builds_holy_sites => enable_religion_seat_builds_holy_sites, disable_religion_seat_builds_holy_sites;
        /// See `advanced/religion_seat_fields_apostles.rs`.
        religion_seat_fields_apostles => enable_religion_seat_fields_apostles, disable_religion_seat_fields_apostles;
        /// See `advanced/religion_seat_fields_apostles.rs`.
        religion_seat_presses_the_campaign => enable_religion_seat_presses_the_campaign, disable_religion_seat_presses_the_campaign;
        /// See `advanced/religion_seat_fields_apostles.rs`.
        religion_seat_races_the_prophet => enable_religion_seat_races_the_prophet, disable_religion_seat_races_the_prophet;
        /// See `advanced/religion_seat_fields_apostles.rs`.
        religion_seat_takes_the_faith_beliefs => enable_religion_seat_takes_the_faith_beliefs, disable_religion_seat_takes_the_faith_beliefs;
        /// Keep a committed Science launch or laser ahead of paused districts.
        paused_district_spares_the_launch => enable_paused_district_spares_the_launch, disable_paused_district_spares_the_launch;
        /// Coordinate a quoted opening Library and backlog Builder.
        parallel_library_backlog => enable_parallel_library_backlog, disable_parallel_library_backlog;
        /// See `advanced/religion_seat_counts_the_pressure.rs`.
        religion_seat_counts_the_pressure => enable_religion_seat_counts_the_pressure, disable_religion_seat_counts_the_pressure;
        /// Opt-in gene `religion-seat-takes-the-debater`; see
        /// `advanced/religion_seat_takes_the_debater.rs`.
        religion_seat_takes_the_debater => enable_religion_seat_takes_the_debater, disable_religion_seat_takes_the_debater;
        /// Opt-in gene `religion-seat-spares-the-launcher`; see
        /// `advanced/religion_seat_spares_the_launcher.rs`.
        religion_seat_spares_the_launcher => enable_religion_seat_spares_the_launcher, disable_religion_seat_spares_the_launcher;
        /// See `advanced/religion_seat_charts_the_rivals.rs`.
        religion_seat_charts_the_rivals => enable_religion_seat_charts_the_rivals, disable_religion_seat_charts_the_rivals;
        /// See `advanced/religion_seat_spends_the_bank.rs`.
        religion_seat_spends_the_bank => enable_religion_seat_spends_the_bank, disable_religion_seat_spends_the_bank;
        // ---- append: s-s --------------------------------------------
        /// Opt-in gene `sweden-builds-open-air-museums`; see
        /// `advanced/sweden_builds_open_air_museums.rs`.
        sweden_builds_open_air_museums => enable_sweden_builds_open_air_museums, disable_sweden_builds_open_air_museums;
        /// Opt-in gene `siege-at-the-walls-dies-first`; see
        /// `advanced/siege_at_the_walls_dies_first.rs`.
        siege_at_the_walls_dies_first => enable_siege_at_the_walls_dies_first, disable_siege_at_the_walls_dies_first;
        /// Opt-in gene `space-denial-slots-the-spy-cards`; see
        /// `advanced/space_denial_spy_cards.rs`.
        space_denial_slots_the_spy_cards => enable_space_denial_slots_the_spy_cards, disable_space_denial_slots_the_spy_cards;
        /// Opt-in gene `space-denial-promotes-the-rocket-scientist`; see
        /// `advanced/space_denial_rocket_scientist.rs`.
        space_denial_promotes_the_rocket_scientist => enable_space_denial_promotes_the_rocket_scientist, disable_space_denial_promotes_the_rocket_scientist;
        // ---- append: t-z --------------------------------------------
        /// Opt-in gene `theology-fights-count-the-host-edge`; see
        /// `advanced/theology_fights.rs`.
        theology_fights_count_the_host_edge => enable_theology_fights_count_the_host_edge, disable_theology_fights_count_the_host_edge;
    }

    toggles! {
        /// Enable `victory-deadline-budget` for measurement.
        victory_deadline_budget => enable_victory_deadline_budget, disable_victory_deadline_budget;
        /// Enable `culture-tourism-payback` for measurement.
        culture_tourism_payback => enable_culture_tourism_payback, disable_culture_tourism_payback;
        /// Enable `siege-positive-damage-budget` for measurement.
        siege_positive_damage_budget => enable_siege_positive_damage_budget, disable_siege_positive_damage_budget;
        /// Enable `culture-faith-reservation` for measurement.
        culture_faith_reservation => enable_culture_faith_reservation, disable_culture_faith_reservation;
        /// Enable `capital-campaign-router` for measurement.
        capital_campaign_router => enable_capital_campaign_router, disable_capital_campaign_router;
        /// Enable `great-work-completion-value` for measurement.
        great_work_completion_value => enable_great_work_completion_value, disable_great_work_completion_value;
        /// Enable `upgrade-window-campaign` for measurement.
        upgrade_window_campaign => enable_upgrade_window_campaign, disable_upgrade_window_campaign;
        /// Enable `tourism-land-reservation` for measurement.
        tourism_land_reservation => enable_tourism_land_reservation, disable_tourism_land_reservation;
        /// Enable `reinforce-before-stall` for measurement.
        reinforce_before_stall => enable_reinforce_before_stall, disable_reinforce_before_stall;
        /// Enable `capture-hold-chain` for measurement.
        capture_hold_chain => enable_capture_hold_chain, disable_capture_hold_chain;
        /// Size the defensive Missionary corps by cities actually under conversion
        /// pressure, up to four, instead of two. Off in production; opted into by
        /// name. See [`AdvancedAi::religious_defence_scales`].
        religious_defence_scales => enable_religious_defence_scales, disable_religious_defence_scales;
        /// Let a wounded spreader in its own Holy Site's heal ring hold and heal
        /// instead of spending a weak charge. Off in production; opted into by
        /// name. See [`AdvancedAi::religious_units_heal_first`].
        religious_units_heal_first => enable_religious_units_heal_first, disable_religious_units_heal_first;
        /// Build a Holy Site in the city losing its religious majority so defenders
        /// can be bought there directly. Put a Holy Site in the city that is
        /// actually losing its majority, so its defender can be bought there
        /// instead of walking from the Holy City. Off in production; opted into by
        /// name. See [`AdvancedAi::holy_site_where_the_threat_is`].
        holy_site_where_the_threat_is => enable_holy_site_where_the_threat_is, disable_holy_site_where_the_threat_is;
        /// Choose the enhancer beliefs that multiply religious spread while the
        /// corps has a job to do. Off in production; opted into by name. See
        /// [`AdvancedAi::enhancer_for_the_corps`].
        enhancer_for_the_corps => enable_enhancer_for_the_corps, disable_enhancer_for_the_corps;
        /// Have a founder outside the Religion lane still build the Shrine and
        /// Temple an Apostle needs. See [`AdvancedAi::founder_temple`]. Opt-in
        /// gene.
        founder_temple => enable_founder_temple, disable_founder_temple;
        /// Credit a Campus building the science its city's multipliers will
        /// actually pay, not its raw spec yield. See
        /// [`AdvancedAi::science_multiplier_payoff`]. Opt-in gene
        /// `science-multiplier-payoff`.
        science_multiplier_payoff => enable_science_multiplier_payoff, disable_science_multiplier_payoff;
        /// Scale a missing Campus building's debt by its own science yield, so
        /// Universities and Labs outrank Libraries. See
        /// [`AdvancedAi::research_tier_premium`]. Opt-in gene
        /// `research-tier-premium`.
        research_tier_premium => enable_research_tier_premium, disable_research_tier_premium;
        /// Credit a power plant the powered yields it switches on, above all the
        /// Research Lab's extra science. See [`AdvancedAi::power_the_laboratory`].
        /// Opt-in gene `power-the-laboratory`.
        power_the_laboratory => enable_power_the_laboratory, disable_power_the_laboratory;
        /// Credit a Campus plot that reaches the Rationalism adjacency threshold
        /// with the science bonus crossing it unlocks. See
        /// [`AdvancedAi::campus_adjacency_threshold`]. Opt-in gene
        /// `campus-adjacency-threshold`.
        campus_adjacency_threshold => enable_campus_adjacency_threshold, disable_campus_adjacency_threshold;
        /// Let a seat with no religion and 600+ banked Faith patronize Great People
        /// whatever the points shortfall. See [`AdvancedAi::idle_faith_patronage`].
        /// Opt-in gene.
        idle_faith_patronage => enable_idle_faith_patronage, disable_idle_faith_patronage;
        /// Reserve a city to build whatever unblocks an earned Great Person,
        /// selling duplicate works to make room. A class earned and blocked
        /// reserves a city for the slot building, district, wonder or soldier that
        /// lifts the block, and a due cultural person sells duplicate works to make
        /// room. See [`AdvancedAi::great_person_housing`]. Production gene.
        great_person_housing => enable_great_person_housing, disable_great_person_housing;
        /// Count a neighbour's unpillaged improvements within reach as raid prizes
        /// and send raiders to pillage them. See
        /// [`AdvancedAi::raid_pillage_prizes`]. Opt-in gene; inert unless
        /// `opportunistic_war` is on.
        raid_pillage_prizes => enable_raid_pillage_prizes, disable_raid_pillage_prizes;
        /// Price the Religion lane's Holy Site at what the Culture lane pays for
        /// its Theater Square. See [`AdvancedAi::holy_lane_parity`]; the evaluator
        /// arm `advanced_holy_lane` sets the field directly, this pair makes it a
        /// native opt-in gene (`PRODUCTION_OPT_INS`).
        holy_lane_parity => enable_holy_lane_parity, disable_holy_lane_parity;
        /// Price the city ceiling off the passable land actually visible, at one
        /// city per 45 tiles, capped at twelve. Native tournament games leave this
        /// off so recorded ladders stay comparable; see `wide_map_capacity` for the
        /// live Settler measurement.
        wide_map_capacity => enable_wide_map_capacity, disable_wide_map_capacity;
        /// Price the live city ceiling off the whole map's land, counting ground
        /// still hidden by fog. See `fog_land_capacity`.
        fog_land_capacity => enable_fog_land_capacity, disable_fog_land_capacity;
        /// Treat a city that is losing hitpoints as besieged even when fog hides
        /// every attacker. See `BasicAi::garrison_under_fire` for the t115
        /// measurement.
        base.garrison_under_fire => enable_garrison_under_fire, disable_garrison_under_fire;
        /// Release a settler's linked escort after two turns without progress so
        /// the settler marches on by itself. See `escort_unstick`.
        escort_unstick => enable_escort_unstick, disable_escort_unstick;
        /// A founder may hold one Guru whenever any of its religious units is
        /// damaged, not only while the home is under conversion pressure, so the
        /// corps out spreading gets the heal too.
        ///
        /// Opt-in gene `guru-heals-the-corps-2`. See
        /// [`AdvancedAi::guru_heals_the_corps_2`].
        guru_heals_the_corps_2 => enable_guru_heals_the_corps_2, disable_guru_heals_the_corps_2;
        /// Have a Religion strategy offer peace to every at-war major so its
        /// missionaries can reach their cities. See `religion_sues_peace` for the
        /// t200 measurement.
        religion_sues_peace => enable_religion_sues_peace, disable_religion_sues_peace;
        /// Let a unit remember its campaign objective, dangerous approaches and a
        /// short retreat commitment across turns. Production Advanced enabled this
        /// by default until 2026-08-14 (the war-half removal; see
        /// `promoted_policy_envoy`); the explicit method keeps focused evaluators
        /// able to opt in deliberately.
        base.unit_objective_memory => enable_unit_objective_memory, disable_unit_objective_memory;
        /// Defer a unit's promotion until it is wounded enough to use the
        /// promotion's heal instead of wasting it.
        promote_when_wounded => enable_promote_when_wounded, disable_promote_when_wounded;
        /// Read Faith purchase prices from the engine at the game's speed and
        /// discounts, instead of a Standard-speed literal.
        ///
        /// `spec.cost * 2.0` is the Faith rate at Standard speed, and `item_cost`
        /// scales every price by `game_speed`. Online — the speed the deployment
        /// profile and the live bridge both play — is 50%, so that literal asks
        /// for **twice** the Faith the engine would take, and the reserve is then
        /// applied on top of a doubled price. Marathon is 300% and it underquotes
        /// by a third, issuing purchases the engine refuses. It also ignores every
        /// discount that moves the number: the founder's belief, Theocracy, the
        /// Holy Site's own purchase discount, a Guru's wonder discount. The same
        /// defect priced Rock Bands, which additionally climb 100 per band already
        /// bought. `unit_purchase_cost` knows all of it and returns None when the
        /// purchase is illegal here, rather than leaving that to a refused order.
        ///
        /// Off by default: it changes play and has not been screened.
        engine_faith_price => enable_engine_faith_price, disable_engine_faith_price;
        /// Raise the wartime army target when the enemy outweighs us, instead of
        /// counting only our own cities. Native tournament games leave this
        /// disabled so their recorded ladders stay comparable.
        army_target_weighs_the_enemy => enable_army_target_weighs_the_enemy, disable_army_target_weighs_the_enemy;
        /// Let the strongest met major raise the army target in peacetime, so
        /// deterrence exists before any declaration. Native tournament games leave
        /// this disabled so their recorded ladders stay comparable.
        peacetime_deterrence => enable_peacetime_deterrence, disable_peacetime_deterrence;
        /// Credit the envoy scorer with the resources, bonuses and points a
        /// suzerainty pays, amortised over envoys still needed. See
        /// [`super::SUZERAIN_PRIZE`]. Off on the anchor, so a comparison against it
        /// measures the term rather than a rename.
        price_the_suzerainty => enable_price_the_suzerainty, disable_price_the_suzerainty;
        envoys_open_the_siege_road => enable_envoys_open_the_siege_road, disable_envoys_open_the_siege_road;
        /// Rebuild the recon arm when every scout is gone and unexplored ground
        /// remains to chart. Native tournament games leave this disabled so their
        /// recorded ladders stay comparable.
        base.recon_replacement => enable_recon_replacement, disable_recon_replacement;
        /// Buy one ship for a fleetless empire with unexplored water off its coast
        /// and send it exploring. See `BasicAi::naval_recon` and `naval_explorer`.
        base.naval_recon => enable_naval_recon, disable_naval_recon;
        /// Keep land units out of the water: no water exploration goals, and
        /// disembark units already at sea. Native tournament games leave this
        /// disabled so their recorded ladders stay comparable.
        base.come_ashore => enable_come_ashore, disable_come_ashore;
        /// Price a fogged objective city from its last sighting instead of treating
        /// unseen ground as empty. Native tournament games leave this disabled so
        /// their recorded ladders stay comparable.
        blind_objective_strength => enable_blind_objective_strength, disable_blind_objective_strength;
        /// Route an adaptive plan that switched to Conquest through the war
        /// production path instead of the basic governor. Native tournament games
        /// leave this disabled so their recorded ladders stay comparable.
        war_economy => enable_war_economy, disable_war_economy;
        /// Keep marching newly built rear units to the campaign objective after war
        /// is declared, not only before. Native tournament games leave this
        /// disabled so their recorded ladders stay comparable.
        war_reinforcement => enable_war_reinforcement, disable_war_reinforcement;
        /// Let a seat with an assigned victory target still counter a rival at
        /// match point. See [`Self::deny_while_targeted`].
        deny_while_targeted => enable_deny_while_targeted, disable_deny_while_targeted;
        /// Leave an ordered Spy alone for the mission's duration instead of
        /// re-sending the order every turn. See [`Self::spy_mission_patience`].
        spy_mission_patience => enable_spy_mission_patience, disable_spy_mission_patience;
        /// Count a stacked guard as protection only when it can hold, and make it
        /// stay with its settler. See [`Self::settler_guard_holds`].
        settler_guard_holds => enable_settler_guard_holds, disable_settler_guard_holds;
        /// Read the Culture and Diplomacy denial alarms off projected pressure,
        /// current value plus recent slope carried forward. See
        /// [`Self::projected_stock_denial`].
        projected_stock_denial => enable_projected_stock_denial, disable_projected_stock_denial;
        /// Count a peacetime major's army massed near one of our cities toward
        /// that city's danger. See [`Self::frontier_massing_alarm`].
        frontier_massing_alarm => enable_frontier_massing_alarm, disable_frontier_massing_alarm;
        /// Test the Rock Band unlock before the Culture lane's stadium and
        /// museum-tourism civics, while retaining Humanism and Conservation first.
        culture_cold_war_window => enable_culture_cold_war_window, disable_culture_cold_war_window;
        /// Read a rival's Science clock from the prerequisite chain it has
        /// climbed, not only from the launches it has made. See
        /// [`Self::science_chain_alarm`].
        science_chain_alarm => enable_science_chain_alarm, disable_science_chain_alarm;
        /// Count a rival's city-state suzerainties toward the Diplomatic threat it
        /// presents. See [`Self::rival_suzerainty_alarm`].
        rival_suzerainty_alarm => enable_rival_suzerainty_alarm, disable_rival_suzerainty_alarm;
        /// Point the three targeted World Congress penalties at the empire the
        /// denial layer names. See [`Self::congress_counter_leader`].
        congress_counter_leader => enable_congress_counter_leader, disable_congress_counter_leader;
        /// Read a rival's conquests from the cities it has taken rather than only
        /// from the capitals. See [`Self::domination_city_count`].
        domination_city_count => enable_domination_city_count, disable_domination_city_count;
        /// Stop a war we did not declare from taking the grand strategy while our
        /// own victory lane is live. See [`Self::unchosen_war_keeps_the_lane`].
        unchosen_war_keeps_the_lane => enable_unchosen_war_keeps_the_lane, disable_unchosen_war_keeps_the_lane;
        /// Measure the elective war against the weakest rival we can reach rather
        /// than the weakest on the board. See [`Self::elective_war_in_reach`].
        elective_war_in_reach => enable_elective_war_in_reach, disable_elective_war_in_reach;
        /// Shut the settler window on whether the city would pay the settler back
        /// before the game ends, rather than on a deadline. See
        /// [`Self::expansion_pays_back`].
        expansion_pays_back => enable_expansion_pays_back, disable_expansion_pays_back;
        /// Stop a war we choose from taking the grand strategy while our own
        /// victory lane is live. See [`Self::elective_war_yields_to_a_lane`].
        elective_war_yields_to_a_lane => enable_elective_war_yields_to_a_lane, disable_elective_war_yields_to_a_lane;
        /// Weigh whether a settle site can be held — barbarian exposure and
        /// distance from our own cities — not only what it yields. See
        /// [`Self::defensible_sites`].
        defensible_sites => enable_defensible_sites, disable_defensible_sites;
        /// Raise the Culture and Diplomacy denial alarms early, since countering an
        /// accumulated stock takes many turns. See
        /// [`Self::stock_denial_lead_time`].
        stock_denial_lead_time => enable_stock_denial_lead_time, disable_stock_denial_lead_time;
        /// Keep the campaign aimed at a city it has already damaged instead of
        /// re-targeting a fresh one each turn. A breach gets an additional value
        /// credit, but even an intact objective needs several turns of marching and
        /// bombardment before that credit exists; changing the objective every
        /// assessment strands the whole army between cities. An emergency, a
        /// changed war target, or capture immediately releases the commitment.
        /// Native tournament games leave this disabled so their recorded ladders
        /// stay comparable.
        siege_commitment => enable_siege_commitment, disable_siege_commitment;
        /// Send a relief force at the besiegers actually damaging the city, not the
        /// enemy nearest the force. Native tournament games leave this disabled so
        /// their recorded ladders stay comparable.
        relief_targets_the_siege => enable_relief_targets_the_siege, disable_relief_targets_the_siege;
        /// Price the enemy units remembered near an unseen objective instead of
        /// reading a fogged approach as empty. Native tournament games leave this
        /// disabled so their recorded ladders stay comparable.
        blind_objective_units => enable_blind_objective_units, disable_blind_objective_units;
        /// Make a Builder retreat from, and never step into, a tile a visible
        /// barbarian can capture next turn. Native opt-in gene
        /// `builder-barbarian-safety`; off in production until its targeted
        /// barbarian screen has priced the safety/tempo trade.
        builder_barbarian_safety => enable_builder_barbarian_safety, disable_builder_barbarian_safety;
        /// Credit strength per production and the civilization's own unique unit
        /// when pricing military production. Evaluator arm
        /// `advanced_unit_efficiency`; off in production.
        unit_cost_efficiency => enable_unit_cost_efficiency, disable_unit_cost_efficiency;
        /// Subtract the unit-maintenance bill inside the policy counterfactual so
        /// maintenance-discount cards score above zero. See
        /// `BasicAi::maintenance_aware_deck`; entrant `advanced_maintenance_deck`.
        base.maintenance_aware_deck => enable_maintenance_aware_deck, disable_maintenance_aware_deck;
        /// Rank each district family by how much of the empire still lacks it, so
        /// Theater Squares get built.
        ///
        /// ⚠ `d_theater` is the lowest of the four district weights in all 51 league
        /// genomes and the picker only skips a family THIS CITY holds, so every city
        /// works down the same constant list and the fourth entry is never reached.
        /// Live run `civvis-20260803T090911Z`: 28 `DISTRICT_CAMPUS` orders, **zero**
        /// `DISTRICT_THEATER`, and no Theatre Square anywhere in a 5-city empire — which
        /// makes the whole culture chain unreachable by construction.
        base.district_coverage => enable_district_coverage, disable_district_coverage;
        /// Break a production cost tie between museums by which great-work slots
        /// the empire can actually fill.
        ///
        /// ⚠ Art and Archaeological Museum are identical in `data/buildings.json` except
        /// for the slot kind and both cost 290, so `sort()` fell through to `Name::cmp`
        /// and the letter 'c' decided it. An Artifact slot needs a 400-production
        /// Archaeologist no live run has ever built; the first run to build a Museum
        /// (`civvis-20260803T082856Z`) took the Artifact one and ended with 0 great works
        /// in 6 slots.
        base.slot_kind_tiebreak => enable_slot_kind_tiebreak, disable_slot_kind_tiebreak;
        /// In a severe empire-wide Amenity crisis, pause one repeatable project for
        /// the amenity repair chain and slot Liberalism. When host-observed Amenity
        /// deficits have crossed a severe empire-wide threshold, pause one
        /// repeatable project for the concrete repair chain and let the policy deck
        /// use its direct empire-wide repair. Frozen controllers leave both
        /// ordinary orderings untouched.
        amenity_project_preemption => enable_amenity_project_preemption, disable_amenity_project_preemption;
        /// Price an amenity district by the building it will hold, and a regional
        /// amenity building by every city it reaches. See `amenity_district_path`.
        amenity_district_path => enable_amenity_district_path, disable_amenity_district_path;
        /// Make the settlement-gap redirect and the Settler ranking read the same
        /// city target as the baseline cascade. See `settlement_target`.
        settlement_gap_reads_city_target => enable_settlement_gap_target, disable_settlement_gap_target;
        /// Let a developed live city build the cheapest legal wonder whatever the
        /// grand strategy says. See `live_wonder_race`: the live seat had never
        /// ordered one, and its games end on the host's score tally.
        live_wonder_race => enable_live_wonder_race, disable_live_wonder_race;
        /// Found the third city before racing for a Great Prophet on the live seat.
        /// See `expansion_before_prophet`.
        expansion_before_prophet => enable_expansion_before_prophet, disable_expansion_before_prophet;
        /// Never open an elective war on the live seat; it has never converted one
        /// into a capture. See `no_elective_war`.
        no_elective_war => enable_no_elective_war, disable_no_elective_war;
        /// Answer a Science or score leader by racing them in that lane instead of
        /// declaring war on them. See `counter_in_lane`.
        counter_in_lane => enable_counter_in_lane, disable_counter_in_lane;
        /// Raise the wanted city count one rung per game era instead of every
        /// ninety standard turns. See `era_paced_expansion`.
        era_paced_expansion => enable_era_paced_expansion, disable_era_paced_expansion;
        /// Price the Ancestral Hall for its Settler production bonus and the free
        /// Builder in every new city.
        expansion_hall => enable_expansion_hall, disable_expansion_hall;
        /// Price a point of culture at what the Science lane pays for a point of
        /// science. See `tally_culture`.
        tally_culture => enable_tally_culture, disable_tally_culture;
        /// Make a Theater Square owe its Amphitheater, Museum and Broadcast Center
        /// the way a Campus owes its buildings. See `culture_building_debt`.
        culture_building_debt => enable_culture_building_debt, disable_culture_building_debt;
        /// Refuse to found a city beyond the empire's Loyalty reach on ground the
        /// seat has not explored. See `frontier_loyalty`.
        frontier_loyalty => enable_frontier_loyalty, disable_frontier_loyalty;
        /// Keep a settler target dropped for danger out of the ranking for several
        /// turns instead of re-picking it immediately. See
        /// `settler_target_hysteresis`.
        settler_target_hysteresis => enable_settler_target_hysteresis, disable_settler_target_hysteresis;
        /// Send a Settler to the best safe alternative site when a visible threat
        /// blocks its route. See `settler_threat_detour`.
        settler_threat_detour => enable_settler_threat_detour, disable_settler_threat_detour;
        /// Price a Settler's walk per turn, rising the longer it has walked, so it
        /// settles sooner. Price a Settler's walk in turns, each turn dearer the
        /// longer the Settler has already been walking, so expansion founds sooner
        /// without giving up a site good enough to pay for its walk. See
        /// `settle_sooner`.
        settle_sooner => enable_settle_sooner, disable_settle_sooner;
        /// Let any civilization build wonders on merit by pricing the fifteen score
        /// points a finished wonder pays. A wonder lane any civilization can reach
        /// on merit: the `Item::Wonder` arm learns the fifteen points
        /// `Game::score_parts` pays for a finished wonder, under a density bar and
        /// the live race's own development guards. See
        /// `AdvancedAi::wonder_score_tally`.
        wonder_score_tally => enable_wonder_score_tally, disable_wonder_score_tally;
        /// Let banked Faith or Gold patronize any affordable Great Person on the
        /// score-tally seat, not only nearly-earned ones. See `tally_great_people`.
        tally_great_people => enable_tally_great_people, disable_tally_great_people;
        /// Make a repeatable district project wait behind the science and
        /// production buildings the city can already build. See
        /// `buildings_before_projects`.
        buildings_before_projects => enable_buildings_before_projects, disable_buildings_before_projects;
        /// Withdraw a unit that one enemy blow could kill to safe healing ground,
        /// and leave when threatened again. See `BasicAi::one_shot_recovery`.
        base.one_shot_recovery => enable_one_shot_recovery, disable_one_shot_recovery;
        /// Stop pricing a barbarian Scout as a threat, since it never attacks or
        /// captures; settlers and scouts ignore it. See
        /// `barbarian_scouts_are_scouts`.
        barbarian_scouts_are_scouts => enable_barbarian_scouts_are_scouts, disable_barbarian_scouts_are_scouts;
        /// Let only one city at a time claim the 3,000-point first Spaceport bonus,
        /// instead of every city at once. See `one_launch_pad`.
        one_launch_pad => enable_one_launch_pad, disable_one_launch_pad;
        /// Rank loyalty emergencies by turns until the city flips rather than by
        /// its current loyalty level. Native tournament games leave this disabled.
        base.loyalty_rate_alarm => enable_loyalty_rate_alarm, disable_loyalty_rate_alarm;
        /// Buy a soldier with Faith only when the treasury can also pay its Gold
        /// upkeep.
        solvent_faith_army => enable_solvent_faith_army, disable_solvent_faith_army;
        /// Choose the pantheon by what it would pay on the tiles the empire owns,
        /// not from a fixed order. Reachable as `advanced_pantheon_board`; see
        /// `BasicAi::pantheon_reads_the_board`.
        base.pantheon_reads_the_board => enable_pantheon_board, disable_pantheon_board;
        /// Rank Great Person classes and project points by the victory the empire
        /// is racing, even during a war. Rank Great Person classes, and the Great
        /// Person points a project earns, by the victory the empire is actually
        /// racing rather than by a war it is fighting. See
        /// `advanced/victory_lane.rs` — this is the one gene there that overrides a
        /// Conquest plan, and the fires-check that chose that scope is in
        /// `docs/VICTORY_GENES.md` §7. `Recovery` still keeps its own strategy. Off
        /// everywhere by default; opt-in gene `lane-great-people`.
        lane_great_people => enable_lane_great_people, disable_lane_great_people;
        /// Choose policy cards for the victory the empire is racing while its plan
        /// is still Expansion. See `advanced/victory_lane.rs`. Off everywhere by
        /// default; opt-in gene `lane-policy-deck`.
        lane_policy_deck => enable_lane_policy_deck, disable_lane_policy_deck;
        /// Run the Culture lane's Faith purchases, Naturalists and Rock Bands, for
        /// an empire racing Culture under an Expansion plan. Run the Culture lane's
        /// Faith pass — the Naturalist that founds a National Park, the touring
        /// Rock Bands — and size its reserve, for an empire racing Culture whose
        /// plan has not named the lane. See `advanced/victory_lane.rs`; `Recovery`
        /// still refuses. Off everywhere by default; opt-in gene
        /// `lane-culture-spending`.
        lane_culture_spending => enable_lane_culture_spending, disable_lane_culture_spending;
        /// Open the Spaceport and launch pass for an empire racing Science even
        /// while its plan is still Expansion. Treat an empire racing Science as a
        /// Science seat throughout the space race: the pad count, the city a launch
        /// project may claim and the city a pad may be sited in all read the race
        /// rather than an explicitly assigned target, and the pass opens at all.
        /// See `advanced/victory_lane.rs`. Off everywhere by default; opt-in gene
        /// `lane-space-race`.
        lane_space_race => enable_lane_space_race, disable_lane_space_race;
        /// Price first place in a scored competition by the Diplomatic Victory
        /// Points it pays. See `advanced/victory_lane.rs`. Off everywhere by
        /// default; opt-in gene `competition-victory-points`.
        competition_victory_points => enable_competition_victory_points, disable_competition_victory_points;
        /// Beeline Advanced Flight, build an Aerodrome and bombers, and take the
        /// appointed city with cavalry behind them. See
        /// [`AdvancedAi::maintain_air_surge`] and `advanced/air_surge.rs`. Off
        /// everywhere by default; opt-in gene `air-surge`.
        air_surge => enable_air_surge, disable_air_surge;
        /// Let a force finish a defender together with a friendly volley, without
        /// reopening the closed war-half bundle. See
        /// [`AdvancedAi::coordinated_finish`].
        ///
        /// ⭐ THIS PAIR EXISTS SO THE FLAG IS COUNTED AT ALL. `docs/EVAL_STATUS.md`
        /// counts a capability toggle by its `enable_*`/`disable_*` pair here, so a
        /// field only ever set directly — `ai.coordinated_finish = true` in
        /// `src/elo.rs`'s `advanced_coordinated_finish` arm — is not merely
        /// unreachable by a screen, it is invisible to the count that measures the
        /// debt. `docs/VICTORY_GENES.md` §9 names it first among the fields in
        /// that position. Off everywhere by default; opt-in gene
        /// `coordinated-finish`.
        coordinated_finish => enable_coordinated_finish, disable_coordinated_finish;
        /// Keep religious units out of every tile a visible barbarian raider can
        /// reach next turn. A religious unit steps out of the tiles a visible
        /// barbarian raider can reach next turn, and never steps into them on the
        /// way to anything, holding when no safe step makes progress. See
        /// [`AdvancedAi::missionary_evades_raiders`]. Opt-in gene
        /// `missionary-evades-raiders`.
        missionary_evades_raiders => enable_missionary_evades_raiders, disable_missionary_evades_raiders;
        /// Scale religious defence with a rival's progress toward religious
        /// victory, and walk the Inquisitor to the heresy. The religious defence
        /// grows with how much of a rival's religious victory is already done —
        /// every civilization is a veto on it — naming and targeting the stakes
        /// faith from half a victory and spending on it from match point; the
        /// Inquisitor walks to the heresy instead of spending its charges where it
        /// was bought. See [`AdvancedAi::religious_veto_defence`]. Opt-in gene
        /// `religious-veto-defence`.
        religious_veto_defence => enable_religious_veto_defence, disable_religious_veto_defence;
        /// Pick the trade quote with the best net value to us instead of the most
        /// balanced exchange. A quote is chosen by our own net value instead of the
        /// most balanced exchange on the board (`min(our gain, their gain)`), which
        /// threw away the ordering `Game::quick_deals` already produced by our
        /// gain. See `BasicAi::deals_for_our_gain`. Opt-in gene
        /// `deals-for-our-gain`.
        base.deals_for_our_gain => enable_deals_for_our_gain, disable_deals_for_our_gain;
        /// Price a trade quote at the counterparty's walk-away point less two Gold,
        /// falling back to the midpoint if refused. The chosen quote's Gold is
        /// moved to the counterparty's walk-away less two Gold — a sale asks for
        /// more, a purchase pays less — where the shipped quote split the surplus
        /// down the middle; the midpoint quote stays the fallback. See
        /// `BasicAi::deals_at_the_ceiling`. Opt-in gene `deals-at-the-ceiling`.
        base.deals_at_the_ceiling => enable_deals_at_the_ceiling, disable_deals_at_the_ceiling;
        /// Stop bundling free one-way Open Borders into friendship and alliance
        /// proposals; sell passage through the quote lane. Friendship and alliance
        /// proposals no longer bundle one-way Open Borders, which every ask handed
        /// out for nothing once Early Empire was in; passage is sold through the
        /// quote lane. See `BasicAi::no_free_passage`. Opt-in gene
        /// `no-free-passage`.
        base.no_free_passage => enable_no_free_passage, disable_no_free_passage;
        /// Fight one campaign front at a time, seeking peace with every other major
        /// and once the front turns against us. Fight one war at a time: keep one
        /// campaign front and sue every other major for peace, hold a fresh
        /// declaration while a war is on, press the front while a city is breaking
        /// or tiles are in reach to pillage, and offer peace once the exchange has
        /// run against us for long enough with nothing left to take. See
        /// [`AdvancedAi::one_war_observe`]. Opt-in gene `one-war-at-a-time`.
        one_war_at_a_time => enable_one_war_at_a_time, disable_one_war_at_a_time;
        /// Let a soldier at war pillage the tile it stands on with movement its
        /// march does not use. A soldier at war standing on a tile it may pillage
        /// spends the movement its march does not use on the pillage — waiting with
        /// its force, unable to move on, or in the siege ring with its blow
        /// declined — and never a tile of advance.
        /// [`AdvancedAi::campaign_pillage`]. Opt-in gene `campaign-pillage`; see
        /// `advanced/city_campaign.rs`.
        campaign_pillage => enable_campaign_pillage, disable_campaign_pillage;
        /// Reserve the first empty trade route slot ahead of ordinary production in
        /// any city that can start a safe route. A barbarian alarm at a remote city
        /// no longer vetoes the whole empire. See
        /// `BasicAi::solvency_first_trade_slot`; production gene
        /// `solvency-first-trade-slot`. Filed here rather than under a marker: the
        /// append-point check reads a method line's first identifier.
        base.solvency_first_trade_slot => enable_solvency_first_trade_slot, disable_solvency_first_trade_slot;
        /// Give early Settler pipeline slots to cities that finish fastest and hold
        /// distinct reachable claim sites. Opt-in gene
        /// `settler-factory-coordination`. Filed here rather than under a marker:
        /// the append-point check reads a method line's first identifier.
        settler_factory_coordination => enable_settler_factory_coordination, disable_settler_factory_coordination;
        /// Ignore nearby barbarian ships that cannot land a meaningful blow, while
        /// still allowing ranged shots at them. Opt-in gene `naval-threat-triage`.
        /// Filed here rather than under a marker: the append-point check reads a
        /// method line's first identifier.
        base.naval_threat_triage => enable_naval_threat_triage, disable_naval_threat_triage;
        /// Convert the first six Standard-speed turns after a surprise war is
        /// declared against us into a bounded defensive mobilization. Off
        /// everywhere by default; opt-in gene `surprise-war-mobilization`. Filed
        /// here rather than under a marker: the append-point check reads a method
        /// line's first identifier.
        surprise_war_mobilization => enable_surprise_war_mobilization, disable_surprise_war_mobilization;
        /// Widen the science lane's land grab, deferring its city cap, while a
        /// founded city can still mature; then revert and grow.
        /// Opt-in gene `science-expansion-phase`; the timing argument is on
        /// `SCIENCE_EXPANSION_CITY_CEILING`. Filed above the markers: the
        /// append-point check reads a method line's first identifier.
        science_expansion_phase => enable_science_expansion_phase, disable_science_expansion_phase;
        /// Compatibility toggle for callers that used the former Science-only
        /// opening band. The shared first-half expansion-and-defense phase now
        /// governs the handoff for every victory lane. Opt-in gene
        /// `science-opening-band`; the append-point check reads a method line's
        /// first identifier.
        science_opening_band => enable_science_opening_band, disable_science_opening_band;
        /// Open the settler pipeline by the shortfall while the opening is behind
        /// the four-cities-by-turn-sixty pace every recorded win came from.
        /// Opt-in gene `expansion-schedule`; see
        /// `advanced/expansion_schedule.rs` for the corpus table. Filed above the
        /// markers: the append-point check reads a method line's first identifier.
        expansion_schedule => enable_expansion_schedule, disable_expansion_schedule;
        /// A Settler out of a city past its deadline founds the best legal site
        /// within reach instead of chasing the ranked one it has not reached.
        /// Opt-in gene `settler-walk-deadline`; see
        /// `advanced/settler_walk_deadline.rs` for the live forensic. Filed above
        /// the markers: the append-point check reads a method line's first
        /// identifier.
        settler_walk_deadline => enable_settler_walk_deadline, disable_settler_walk_deadline;
        /// Work food while the opening is behind the pace and no city has reached
        /// the population a Settler needs. Opt-in gene `growth-to-settle`; see
        /// `advanced/growth_to_settle.rs`.
        growth_to_settle => enable_growth_to_settle, disable_growth_to_settle;
        /// Once the Palace landmass has at most two independent city sites left,
        /// rebuild a naval eye and favor water that can reveal a known foreign
        /// landfall. Opt-in gene `island-exploration`; see `BasicAi`'s
        /// `island_exploration_active` and `exploration_goal`.
        base.island_exploration => enable_island_exploration, disable_island_exploration;
        /// When the main landmass has little room left, route a Settler to the
        /// nearest viable foreign site the player has discovered. Opt-in gene
        /// `overseas-settlement`; see `advanced/island_expansion.rs`.
        overseas_settlement => enable_overseas_settlement, disable_overseas_settlement;
        /// Fall through to the next-best candidate the planner already ranked when
        /// an order is refused, instead of losing the turn. Opt-in gene
        /// `order-retry`; see `advanced/order_retry.rs`.
        order_retry => enable_order_retry, disable_order_retry;
        /// Before the first city, let only a Warrior directly escorting the
        /// Settler reveal terrain first, then reconsider from the ordinary
        /// settlement candidates. Opt-in gene `opening-warrior-recon-2`; see
        /// `advanced/opening_settlement.rs`. Filed above the markers: the
        /// append-point check reads a method line's first identifier.
        opening_warrior_recon_2 => enable_opening_warrior_recon_2, disable_opening_warrior_recon_2;
        /// When the empire leads the field in science, beeline the space-race chain,
        /// build launch-city production and race two pads early.
        /// Opt-in gene `science-victory-drive`; see `advanced/science_victory_drive.rs`
        /// for the live runs that led science and never launched, and what each
        /// lever does. Filed above the markers: the append-point check reads a
        /// method line's first identifier.
        science_victory_drive => enable_science_victory_drive, disable_science_victory_drive;
        /// Let a Builder whose nearest improvable tile cannot be routed to try the
        /// next-nearest instead of standing still for the rest of the game. See
        /// `BasicAi::builder_tries_the_next_tile`; opt-in gene
        /// `builder-tries-the-next-tile`. Filed here rather than under a marker:
        /// the append-point check reads a method line's first identifier.
        base.builder_tries_the_next_tile => enable_builder_tries_the_next_tile, disable_builder_tries_the_next_tile;
        /// Settlers and builders stay out of a barbarian's one-turn reach: flee
        /// it, never step into it alone, and summon a guard onto the settler's
        /// tile when they must cross it. See
        /// [`AdvancedAi::civilian_out_of_reach`]. Opt-in gene
        /// `civilian-out-of-reach`.
        civilian_out_of_reach => enable_civilian_out_of_reach, disable_civilian_out_of_reach;
        /// A unit, building or district that completes an unresearched
        /// technology's or civic's boost is worth the research the boost grants,
        /// spread over the steps the trigger still needs. See
        /// [`AdvancedAi::eureka_production_premium`]. Opt-in gene
        /// `eureka-chasing-production`.
        eureka_chasing_production => enable_eureka_chasing_production, disable_eureka_chasing_production;
        /// Wait only on a two-turn node whose final boost trigger is already at
        /// the front of an owned city queue, using the boost as a light tie-break.
        /// Opt-in gene `boost-wait-research-2`.
        boost_wait_research_2 => enable_boost_wait_research_2, disable_boost_wait_research_2;
        /// Credit a technology or civic with the boosts it makes chaseable by
        /// unlocking what their triggers need. The quarry Masonry's eureka wants
        /// needs Mining first, Machinery's three Archers need Archery, and nothing
        /// ever bought the permission. See [`AdvancedAi::boost_research_value`]. Opt-in gene
        /// `boost-unlock-research`. Filed above the markers: the append-point check
        /// reads a method line's first identifier.
        boost_unlock_research => enable_boost_unlock_research, disable_boost_unlock_research;
        /// Pays the Envoy a city-state's quest promises on the unit or district family it asked for.
        /// The production queue reads the outstanding `train_unit_type` and
        /// `zone_district_type` quests and prices the item the city-state named
        /// at the seat's own Envoy value, scaled by what that city-state's next
        /// Envoy buys. A reorder of the queue, never an addition to it. Opt-in
        /// gene `quest-production`; see `advanced/city_state_quests.rs`.
        /// Operator request, 2026-08-26.
        quest_production => enable_quest_production, disable_quest_production;
        /// Sends the Trader to the city-state that is asking us for a trade route, for the Envoy.
        /// The destination score carries the Envoy an outstanding
        /// `send_trade_route` quest pays, on any of that city-state's cities, so
        /// the ordinary yield terms still choose between them. Opt-in gene
        /// `quest-trade-route`; see `advanced/city_state_quests.rs`. Operator
        /// request, 2026-08-26.
        quest_trade_route => enable_quest_trade_route, disable_quest_trade_route;
        /// Runs the camp errand against the outpost a city-state named, from further out than the usual ring.
        /// A `clear_barbarian_camp` quest names one outpost within five tiles of
        /// the city-state, and clearing any other one does not pay; the errand
        /// prefers the named camp over a nearer unnamed one and admits it from
        /// beyond the home radius it normally stops at. Opt-in gene
        /// `quest-camp-errand`; see `advanced/city_state_quests.rs`. Operator
        /// request, 2026-08-26.
        base.quest_camp_errand => enable_quest_camp_errand, disable_quest_camp_errand;
        /// Prices the Envoy on whatever completes the Eureka or Inspiration a city-state asked for.
        /// A `trigger_tech_boost` or `trigger_civic_boost` quest is paid by the
        /// trigger, so the Envoy rides on the same boost table
        /// `eureka-chasing-production` reads — as the Envoy, beside that gene's
        /// research and independent of it. Opt-in gene `quest-boost`; see
        /// `advanced/city_state_quests.rs`. Operator request, 2026-08-26.
        quest_boost => enable_quest_boost, disable_quest_boost;
        /// Leaves barbarian camps that raid a rival rather than us, and courts the city-states and majors beyond that rival.
        /// A camp is the rival's when a major we are not allied with has a
        /// city strictly nearer it than any of ours and it sits outside our
        /// worked ring; the envoy scorer, the alliance partner score and the
        /// joint-war answer read the far side of every rival. Opt-in gene
        /// `enemy-of-my-enemy`; see `advanced/enemy_of_my_enemy.rs`. Operator
        /// request, 2026-08-25.
        base.enemy_of_my_enemy => enable_enemy_of_my_enemy, disable_enemy_of_my_enemy;
        /// Buy the plot a Barbarian Outpost stands on for the city inside whose
        /// three rings it sits, when being rid of the outpost is worth more than
        /// the plot's quote. The outpost must be one we can see and one a soldier
        /// of ours can reach and disperse — Civilization VI removes an outpost on
        /// entry, never because the ground changed hands. See
        /// [`AdvancedAi::camp_buyout_clearance`]. Opt-in gene `camp-tile-buyout`.
        /// Filed above the markers: the append-point check reads a method line's
        /// first identifier.
        camp_tile_buyout => enable_camp_tile_buyout, disable_camp_tile_buyout;
        /// Price a Gold purchase at the build's card-boosted rate, so items a
        /// slotted card discounts lose purchase priority to items no card touches.
        /// Opt-in gene `buy-what-cards-cannot-boost`; see
        /// `advanced/gold_and_cards.rs`. Filed above the markers: the append-point
        /// check reads a method line's first identifier.
        buy_what_cards_cannot_boost => enable_buy_what_cards_cannot_boost, disable_buy_what_cards_cannot_boost;
        /// Lean the city production governor toward items the slotted policy deck
        /// makes cheap, by half the card's bonus. Opt-in gene
        /// `build-what-cards-boost`; see `advanced/gold_and_cards.rs`. Filed above
        /// the markers: the append-point check reads a method line's first
        /// identifier.
        build_what_cards_boost => enable_build_what_cards_boost, disable_build_what_cards_boost;
        /// Value a settle site by the passes and straits its own borders would
        /// cover. A border refuses entry to anyone without Open Borders, so the
        /// ground a city claims is ground a rival cannot cross. Off in
        /// production; opted into by name. See
        /// [`AdvancedAi::chokepoint_site_bonus`].
        chokepoint_siting => enable_chokepoint_siting, disable_chokepoint_siting;
        /// Value a settle site on a one-tile land bridge by the sea detour its
        /// city center would save. A city center is a naval passage no foreign
        /// hull may enter, at peace or at war. Off in production; opted into by
        /// name. See [`AdvancedAi::canal_city_bonus`].
        canal_city => enable_canal_city, disable_canal_city;
        /// Buy the plot that closes a passage a rival could walk or sail through.
        /// `expand_borders` is the engine's own influence picker and takes no
        /// advice, so the buy is the whole lever. Off in production; opted into by
        /// name. See [`AdvancedAi::chokepoint_plot_bonus`].
        chokepoint_claim => enable_chokepoint_claim, disable_chokepoint_claim;
        /// Site the Encampment on the pass, a tile no foreign unit may ever
        /// enter. `can_enter_past` refuses an unpillaged foreign Encampment with
        /// no war, alliance or Open Borders exception. Off in production; opted
        /// into by name. See [`AdvancedAi::encampment_seal_bonus`].
        encampment_seals_the_pass => enable_encampment_seals_the_pass, disable_encampment_seals_the_pass;
        /// Hold the gate on the approach to one of our cities with a surplus
        /// soldier or hull. Nothing foreign may enter a tile one of our military
        /// units stands on. Off in production; opted into by name. See
        /// [`AdvancedAi::chokepoint_garrison_step`].
        chokepoint_garrison => enable_chokepoint_garrison, disable_chokepoint_garrison;
        /// Price a settle site the way the engine pays it beside a natural
        /// wonder: the wonder's projected yields on every neighbouring work tile
        /// plus a small flat credit per wonder tile in the footprint, capped at a
        /// river's worth.
        ///
        /// Version 2 of `wonder-adjacent-sites`; version 1 left the code on
        /// 2026-09-09. Opt-in gene `wonder-adjacent-sites-2`. See
        /// `AdvancedAi::wonder_adjacent_sites_2` and `advanced/wonder_sites.rs`.
        wonder_adjacent_sites_2 => enable_wonder_adjacent_sites_2, disable_wonder_adjacent_sites_2;
        /// Keep one Builder per city while there is still land to improve, priced
        /// where it can win the queue. See `AdvancedAi::builder_supply_floor`;
        /// opt-in gene `builder-supply-floor`. Filed here rather than under a
        /// marker: the append-point check reads a method line's first identifier.
        builder_supply_floor => enable_builder_supply_floor, disable_builder_supply_floor;
        /// Price the cheap rung of a district chain by payback and leave the whole
        /// district on the clock. See `AdvancedAi::chain_payback_window_2`; opt-in
        /// gene `chain-payback-window-2`, version two of `chain-payback-window`.
        /// Filed here rather than under a marker: the append-point check reads a
        /// method line's first identifier.
        chain_payback_window_2 => enable_chain_payback_window_2, disable_chain_payback_window_2;
        /// Price the science and culture chain debts by whether the building can
        /// still repay, not by how much of the clock is left. See
        /// `AdvancedAi::chain_payback_window`; opt-in gene `chain-payback-window`.
        /// Filed here rather than under a marker: the append-point check reads a
        /// method line's first identifier.
        chain_payback_window => enable_chain_payback_window, disable_chain_payback_window;
        /// Reserve the cheapest Campus building a city owes ahead of ordinary
        /// production. See `AdvancedAi::first_research_building_reserve`; opt-in
        /// gene `first-research-building-reserve`. Filed here rather than under a
        /// marker: the append-point check reads a method line's first identifier.
        first_research_building_reserve => enable_first_research_building_reserve, disable_first_research_building_reserve;
        /// Modernize the standing army before the discretionary purchase pass
        /// spends the treasury, while a major war is being fought. See
        /// `AdvancedAi::upgrade_the_garrison`; opt-in gene `upgrade-the-garrison`.
        /// Filed here rather than under a marker: the append-point check reads a
        /// method line's first identifier.
        upgrade_the_garrison => enable_upgrade_the_garrison, disable_upgrade_the_garrison;
        /// Claim the ground between us and the nearest neighbours first while
        /// the army can hold it, waive the border provocation there, and wall
        /// and garrison the frontier. See `advanced/contested_land.rs`; opt-in
        /// gene `contested-land-first`. The flag lives on `BasicAi`, which owns
        /// the peacetime garrison. Filed here rather than under a marker: the
        /// append-point check reads a method line's first identifier.
        base.contested_land_first => enable_contested_land_first, disable_contested_land_first;
        /// An adaptive seat stops racing for a Great Prophet: the race costs more
        /// science than the religion returns. See
        /// `AdvancedAi::science_building_first`; opt-in gene
        /// `science-building-first`.
        science_building_first => enable_science_building_first, disable_science_building_first;
        /// Cap the city target at the sites the map can actually seat. See
        /// `city_target_meets_the_map`.
        city_target_meets_the_map => enable_city_target_meets_the_map, disable_city_target_meets_the_map;
        /// Let an unlocked government the lane's list never names take the seat
        /// when it carries more policy slots. See `government_capacity_fallback`.
        government_capacity_fallback => enable_government_capacity_fallback, disable_government_capacity_fallback;
        /// Stop paying a lost victory lane's premiums. See
        /// `lane_release_when_hopeless`.
        lane_release_when_hopeless => enable_lane_release_when_hopeless, disable_lane_release_when_hopeless;
        /// Sue for peace when a war has taken nothing and cannot be paid for. See
        /// `peace_when_war_does_not_pay`.
        peace_when_war_does_not_pay => enable_peace_when_war_does_not_pay, disable_peace_when_war_does_not_pay;
        /// Refuse a war the treasury cannot pay for. See `war_needs_a_treasury`.
        war_needs_a_treasury => enable_war_needs_a_treasury, disable_war_needs_a_treasury;
        /// Let the assigned diplomatic lane size its own Congress ballot. See
        /// `lane_votes_its_favor`.
        lane_votes_its_favor => enable_lane_votes_its_favor, disable_lane_votes_its_favor;
        /// Stop paying for a religion race the world has already closed. See
        /// `religion_race_is_closed`.
        religion_race_is_closed => enable_religion_race_is_closed, disable_religion_race_is_closed;
        /// Put Moksha first for a religionless empire under conversion. See
        /// `moksha_defends_the_faithless`.
        moksha_defends_the_faithless => enable_moksha_defends_the_faithless, disable_moksha_defends_the_faithless;
        /// Put a ceiling on how long a settler waits for an escort. See
        /// `escort_patience_runs_out`.
        escort_patience_runs_out => enable_escort_patience_runs_out, disable_escort_patience_runs_out;
        /// See `naval_escort_patience`: an embarked Settler crosses alone on a
        /// quiet sea after [`NAVAL_ESCORT_PATIENCE`] turns held for its escort.
        naval_escort_patience => enable_naval_escort_patience, disable_naval_escort_patience;
        /// See `AdvancedAi::breaker_commitment_holds`.
        breaker_keeps_its_queue => enable_breaker_keeps_its_queue, disable_breaker_keeps_its_queue;
        /// See `AdvancedAi::breaker_to_the_fastest`.
        breaker_to_the_fastest => enable_breaker_to_the_fastest, disable_breaker_to_the_fastest;
        /// The turn's fire is planned once from the engine's own arithmetic —
        /// the kills that can be finished, their shooters first in the unit
        /// order, ranged before the melee finisher, each biased toward its
        /// planned target — without a clone. See `fire_plan`.
        fire_plan => enable_fire_plan, disable_fire_plan;
        /// On an advance, no unit ends the turn more than the body's pace plus
        /// one tile closer to the objective than the force's anchor stood, so a
        /// horseman does not meet the enemy four tiles before the line does.
        /// See `close_as_a_body`.
        close_as_a_body => enable_close_as_a_body, disable_close_as_a_body;
        /// A shooter's tile beside a melee friend that stands nearer the enemy
        /// earns two screen weights — the arena's own definition of screened,
        /// paid to the archer that stays behind the line. See
        /// `screen_the_shooters`.
        screen_the_shooters => enable_screen_the_shooters, disable_screen_the_shooters;
        /// An Archer for every city, the frontier city first, while the world
        /// is Ancient and Classical, and Archery chased until a city can train
        /// one. Filed here rather than under a marker: a whole function under
        /// one reads as an entry. See `early_archers`.
        early_archers => enable_early_archers, disable_early_archers;
        /// Every open settle or improve commitment whose owner the unit pass left
        /// with movement and no order acts on it after the pass — found, improve,
        /// or the safe route step — or is released with a recorded reason. Filed
        /// here rather than under a marker: a whole function under one reads as
        /// an entry. See `commitment_owner_acts` and `advanced/commitments.rs`.
        commitment_owner_acts => enable_commitment_owner_acts, disable_commitment_owner_acts;
        /// `culture-floor`: a culture-yielding building is exempt from the Great
        /// Work veto and the Theatre Square is priced while the empire's culture
        /// a turn trails the strongest major's by the floor ratio. See
        /// `advanced/yield_floors.rs`.
        culture_floor => enable_culture_floor, disable_culture_floor;
        /// `gold-income-floor`: Markets, Lighthouses, gold-yielding buildings and
        /// the Commercial Hub or Harbor are priced by the income the empire is
        /// short of two Gold a turn per city. See `advanced/yield_floors.rs`.
        gold_income_floor => enable_gold_income_floor, disable_gold_income_floor;
        /// A settler in the barbarians' hands is taken back: exempt from the
        /// duplicate-settler guard, first among adjacent captures, pursued out to
        /// `BARBARIAN_SETTLER_PURSUIT_RADIUS`. Gene `barbarian-settler-capture`;
        /// the flag lives on `BasicAi`, whose `military_step` and
        /// `capture_adjacent_civilian` read it.
        base.barbarian_settler_capture => enable_barbarian_settler_capture, disable_barbarian_settler_capture;
        /// Price a city site's Harbor-eligible coast in the final settlement
        /// score. See `advanced/coastal_sites.rs`.
        coastal_city_sites => enable_coastal_city_sites, disable_coastal_city_sites;
        /// Price a strike with the two strengths the engine will resolve it
        /// with, so matchup, flanking, adjacent support, ground and the river
        /// reach the exchange evaluation. See `exchange_is_the_engines`.
        base.exchange_is_the_engines => enable_exchange_is_the_engines, disable_exchange_is_the_engines;
        /// Price the defender on the tile it is being asked about, with that
        /// tile's own defence, so a unit weighing a hill is credited for the
        /// hill. See `defend_where_you_stand`.
        base.defend_where_you_stand => enable_defend_where_you_stand, disable_defend_where_you_stand;
        /// A force group beyond `THREAT_RELIEF_RADIUS` of the threatened city is
        /// a relief column: it advances on the siege when locally superior,
        /// holds at the city when too weak, and while a city is threatened
        /// focuses only on combatants and cities. See `relief_column_marches`.
        relief_column_marches => enable_relief_column_marches, disable_relief_column_marches;
        /// A Settler always has somewhere to go: exhaustion asks wider questions
        /// instead of holding, and a watchdog bounds every other hold. Filed
        /// above the marker run like `enable_early_archers`: a whole function
        /// under a marker reads as an entry. See `settler_never_idles` and
        /// `advanced/settler_never_idles.rs`.
        settler_never_idles => enable_settler_never_idles, disable_settler_never_idles;
        /// The host's barbarian scouts capture civilians, so a barbarian recon unit counts in every capture-reach model.
        ///
        /// Host-only `live-barbarian-scouts-capture`. Run civvis-20260828T122324Z
        /// lost four settlers to one scout that `barbarian_scouts_are_scouts`
        /// exempted from every capture model; the native barbarian seat's recon
        /// really cannot capture, so the treatment is inert on a native board.
        /// See `advanced/civilian_safety.rs`.
        live_barbarian_scouts_capture => enable_live_barbarian_scouts_capture, disable_live_barbarian_scouts_capture;
        /// A lost settler retires its ground for every settler; fleeing prefers a friendly stack; guards stay near known camps; early settlers stay within a six-tile capital corridor.
        ///
        /// Host-only `live-settler-capture-lessons`: what twenty-four live captures
        /// on 2026-08-28 taught. A settler that leaves the board with no city
        /// within two tiles of where it stood was taken, and every site within
        /// three tiles of that ground is dead for every settler for thirty
        /// standard turns; a civilian fleeing with no safe tile prefers a tile one
        /// of our units holds to the least exposed bare one and never holds beside
        /// a raider while a farther tile exists; the strongest guard that can reach
        /// the settler this turn is summoned, not the nearest; a guard is not
        /// released while a known barbarian camp is within eight tiles; a Settler
        /// first seen during the one-city opening stays within six tiles of its
        /// capital and returns if an emergency flee pushes it farther. See
        /// `advanced/civilian_safety.rs`.
        live_settler_capture_lessons => enable_live_settler_capture_lessons, disable_live_settler_capture_lessons;
        /// A stranded Settler's exhaustion search sets aside a site beside an
        /// unresolved rival border — the one fog guess the preferred search made
        /// that names a city the forecast cannot see — and its nearest-legal
        /// tier runs the same concrete-revolt forecast the ranked tier does. See
        /// `AdvancedAi::exhaustion_site_unpriceable`.
        exhaustion_loyalty_guard => enable_exhaustion_loyalty_guard, disable_exhaustion_loyalty_guard;
        /// A wonder within twelve turns of done in one of the empire's strongest
        /// cities opens the live wonder race without the three-city and
        /// three-building guards, with a bonus that scales with how quickly it
        /// finishes; a wonder further than twenty-five turns from done never
        /// opens the ordinary race. See `AdvancedAi::wonder_bargain_city`.
        cheapest_wonder_first => enable_cheapest_wonder_first, disable_cheapest_wonder_first;
        first_district_first => enable_first_district_first, disable_first_district_first;
        walls_after_districts => enable_walls_after_districts, disable_walls_after_districts;
        /// The two-turn escort cap releases the settler on schedule instead of
        /// being suspended by a predicate that reads only the visible frame, and
        /// a settler already outside its own city with no guard on its tile
        /// marches on a zero risk reading rather than fortifying bare. See
        /// `AdvancedAi::stacked_escort_pace`.
        escort_cap_holds => enable_escort_cap_holds, disable_escort_cap_holds;
        first_luxury_first => enable_first_luxury_first, disable_first_luxury_first;
        /// `spaceport-surplus-veto` (OptIn): the Science strategy's flat per-pad
        /// district bonus stops paying once the empire already holds as many
        /// Spaceports as the current race stage can use
        /// (`science_drive_desired_pads`). Measured motive: live Emperor game
        /// 20260901T132005Z built nine pads and was still ordering more at t213
        /// with the race already unwinnable.
        spaceport_surplus_veto => enable_spaceport_surplus_veto, disable_spaceport_surplus_veto;
        /// `settler-site-gate`: a city starts a Settler only while an acceptable,
        /// unclaimed site worth founding exists. See `advanced/settler_site_gate.rs`.
        settler_site_gate => enable_settler_site_gate, disable_settler_site_gate;
        /// `campus-through-expansion` (OptIn): a Science seat prices the Campus
        /// and its buildings in the Science lane while its plan still reads
        /// Expansion, and no city's first specialty district may be an
        /// Entertainment Complex. Measured motive: live Emperor game
        /// 20260901T175154Z held four cities and zero districts at turn 64 —
        /// under Expansion the Campus had no strategic arm and a beaker weighed
        /// 1.2, so the Plaza, walls, the repair project and an Arena-credited
        /// Entertainment Complex took every queue.
        campus_through_expansion => enable_campus_through_expansion, disable_campus_through_expansion;
        /// `campus-before-halfway` (OptIn): a Science seat's Campus keeps the
        /// lane's 170-point arm before the halfway clock instead of pricing at
        /// zero, and past the half-empire coverage cliff the Campus keeps asking
        /// in any city of `CAMPUS_EVERY_CITY_POP_FLOOR` (four) — the floor the
        /// 2026-08-19 repair named and left as a literal `false`. Measured
        /// motive (2026-09-08): September Emperor ladder games were lost to a
        /// rival's science win at t182–224; #3124 reserved the first half of the
        /// clock for expansion and defence, which also zeroed the Campus's lane
        /// arm for 125 turns of a 250-turn game.
        campus_before_halfway => enable_campus_before_halfway, disable_campus_before_halfway;
        /// `trade-route-network` (OptIn): a Commercial Hub (or a Harbor where no
        /// Hub stands) beside a standing Campus escapes the Science contract and
        /// is priced as trade capacity; a Market or Lighthouse is worth the
        /// route it adds. Measured motive: zero Commercial Hubs in the last 22
        /// live runs and a trade capacity of one for the whole game — thirteen
        /// cities on one Trade Route for 216 turns in 20260901T132005Z.
        trade_route_network => enable_trade_route_network, disable_trade_route_network;
        /// `industrial-chain-debt` (OptIn): an Industrial Zone owes its
        /// Workshop, Factory and plant the same flat debt a Campus owes its
        /// Library, a regional building is worth the production it reaches, and
        /// the Factory and plants join the buildings a repeatable project waits
        /// behind. Measured motive: nine Industrial Zones, four Workshops, one
        /// Factory and no plant at turn 216 of 20260901T132005Z.
        industrial_chain_debt => enable_industrial_chain_debt, disable_industrial_chain_debt;
        /// Version 2 of `skip-the-prophet-race`: leave the Prophet race only at
        /// the narrower last call. Version 1's unconditional withdrawal left the
        /// code on 2026-09-09. See `AdvancedAi::skip_the_prophet_race_2`.
        skip_the_prophet_race_2 => enable_skip_the_prophet_race_2, disable_skip_the_prophet_race_2;
        /// `guard-breaks-the-pin`: a Settler's stacked guard strikes the raider
        /// whose zone of control pins the pair when the trade is worth it. See
        /// `advanced/siege_response.rs`.
        guard_breaks_the_pin => enable_guard_breaks_the_pin, disable_guard_breaks_the_pin;
        /// `settler-target-floor`: a Settler is never sent to a site not worth the
        /// walk. See `advanced/settler_target_floor.rs`.
        settler_target_floor => enable_settler_target_floor, disable_settler_target_floor;
        /// The force's turn is planned jointly — the danger field, a
        /// beam-searched kill sequence verified on one clone, and a heal
        /// rotation — ahead of the per-unit ladder, which leaves the planned
        /// units alone. See `battle_planner`.
        battle_planner => enable_battle_planner, disable_battle_planner;
        /// A force whose objective is an enemy city plays the siege as a state
        /// machine — stage on the ring out of the city's reach until the bill is
        /// met, seal the ring spread-first, reduce walls before the garrison,
        /// reserve one taker and walk in when the city is within its blow. See
        /// `siege_train`.
        siege_train => enable_siege_train, disable_siege_train;
        /// The land group nearest a threatened city of ours holds it as a
        /// formation — a shooter on the centre, melee on the front tiles that
        /// face the enemy, the rest within two, the wounded rotated into the
        /// city — in place of the relief hold point. See `siege_train`.
        anvil => enable_anvil, disable_anvil;
        /// Upgrade the standing army before the purchase pass, at strategic
        /// moments. See `AdvancedAi::modernize_before_spending`; default-on gene
        /// `modernize-before-spending`.
        modernize_before_spending => enable_modernize_before_spending, disable_modernize_before_spending;
        /// A promoted unit keeps hit points in hand before it withdraws. See
        /// `BasicAi::veteran_retreat_margin`; default-on gene
        /// `veterans-withdraw-early`.
        base.veteran_retreat_margin => enable_veteran_retreat_margin, disable_veteran_retreat_margin;
        /// A ranged unit keeps `RANGED_HP_RESERVE` in hand against the lethal
        /// pool. See `BasicAi::ranged_hp_reserve`; opt-in gene `ranged-hp-reserve`.
        base.ranged_hp_reserve => enable_ranged_hp_reserve, disable_ranged_hp_reserve;
        /// `chop-for-expansion`: a Builder turns a forest into a Settler. See
        /// `advanced/chop_for_expansion.rs`.
        chop_for_expansion => enable_chop_for_expansion, disable_chop_for_expansion;
        /// `conquest-takes-the-soft-city`: rank the early conquest target by what
        /// can be taken before what is worth most. See `advanced/early_conquest.rs`.
        conquest_takes_the_soft_city => enable_conquest_takes_the_soft_city, disable_conquest_takes_the_soft_city;
        /// `counter-culture-by-conquest`: answer a culture leader with war aimed
        /// at its Great Works. See `advanced/victory_heuristics.rs`.
        counter_culture_by_conquest => enable_counter_culture_by_conquest, disable_counter_culture_by_conquest;
        /// A rival close to winning is answered before the lane's expansion rule.
        /// See the `denial_outranks_expansion` field; opt-in gene
        /// `denial-outranks-expansion`.
        denial_outranks_expansion => enable_denial_outranks_expansion, disable_denial_outranks_expansion;
        /// The Domination lane hands over to Conquest at `DOMINATION_HANDOVER_CITIES`
        /// instead of waiting for a growing city target. Opt-in gene
        /// `domination-lane-hands-over`.
        domination_lane_hands_over => enable_domination_lane_hands_over, disable_domination_lane_hands_over;
        /// `boost-planner-builds`: the boost planner serves `building:` triggers.
        /// See `advanced/boost_planner.rs`.
        boost_planner_builds => enable_boost_planner_builds, disable_boost_planner_builds;
        /// The army's turn planned from a ranked Objective Board — rows valued in
        /// hammers with a requirement and a deadline — and served by persistent
        /// task forces, in place of proximity force groups and the posture
        /// ladder; `force_groups` is built from the forces. See `objective_board`.
        objective_board => enable_objective_board, disable_objective_board;
        /// `war-policy-via-board`: target feasibility, the declaration and the
        /// peace term read off the board. See `advanced/war_policy.rs`.
        war_policy_via_board => enable_war_policy_via_board, disable_war_policy_via_board;
        /// `strike-reach`: the battle planner's danger field reads every hostile's
        /// reach the way the engine resolves a blow — a unit that stops in our
        /// zone of control keeps its movement for the strike, so a melee unit
        /// two tiles off can close and hit — and on the mirrored board a ranged
        /// hostile needs no line of sight of its own. See `battle_planner`.
        strike_reach => enable_strike_reach, disable_strike_reach;
        /// `safest-stand`: when the battle planner's heal rotation finds no tile
        /// the danger field reads as zero, the wounded or exposed unit takes the
        /// least dangerous tile in reach — its own if that is the least — and
        /// fortifies, instead of being left to the ladder. See `battle_planner`.
        safest_stand => enable_safest_stand, disable_safest_stand;
        /// A route step that is not legal this turn holds the decision instead
        /// of releasing it, bounded by `COMMITMENT_PATIENCE` consecutive
        /// forgotten turns. See the field doc on `AdvancedAi`.
        route_block_is_a_wait => enable_route_block_is_a_wait, disable_route_block_is_a_wait;
        /// A safe-step guard that would not move at all steps to the safest
        /// reachable neighbour that is strictly safer than standing still. See
        /// the field doc on `AdvancedAi`.
        standing_still_is_a_risk => enable_standing_still_is_a_risk, disable_standing_still_is_a_risk;
        /// A threat detour refuses a fallback worth less than
        /// `SETTLER_DETOUR_VALUE_FLOOR` of the site it defers. See the field doc
        /// on `AdvancedAi`.
        detour_keeps_the_site_worth => enable_detour_keeps_the_site_worth, disable_detour_keeps_the_site_worth;
        envoy_building_dividends => enable_envoy_building_dividends, disable_envoy_building_dividends;
        /// Arm the culture defence at 30 percent of the victory bar instead of
        /// version one's 50, refuse every sale to the threatening rival, and
        /// denounce it. See `advanced/culture_strategy.rs`. Opt-in gene
        /// `culture-threat-early`. Filed here rather than under a marker: the
        /// append-point check reads a method line's first identifier.
        culture_threat_early => enable_culture_threat_early, disable_culture_threat_early;
        /// Plan the next six technologies' and four civics' boosts: classify each
        /// trigger by what it costs the plan and turn the cheap ones into at most
        /// three deadlined side objectives, each a share-of-value premium on the
        /// one production or Builder choice that fires it. See
        /// `advanced/boost_planner.rs`. Opt-in gene `boost-planner`. Filed here
        /// rather than under a marker: the append-point check reads a method
        /// line's first identifier.
        boost_planner => enable_boost_planner, disable_boost_planner;
        /// Take a small neighbour's city in the opening: a met rival's known
        /// city within twelve tiles of the capital, the capital's production
        /// reserved for three shooters and two melee bodies ahead of the second
        /// Settler, the war declared once the force is assembled and the bill
        /// covered, and no strike-force body ending its move beside unseen
        /// ground. See `advanced/early_conquest.rs`. Opt-in gene
        /// `early-conquest-opening`; off in production, opted into by name.
        /// Filed here rather than under a marker: the append-point check reads a
        /// method line's first identifier.
        early_conquest_opening => enable_early_conquest_opening, disable_early_conquest_opening;
        /// Scale the opening city target, the opening deadline, the Settler
        /// cadence and the expansion cards with the difficulty rung: the measured
        /// 4-6 band is a King-level reading and every rung above it widens the
        /// rival field. See `advanced/expansion_scales_with_difficulty.rs`.
        /// Opt-in gene `expansion-scales-with-difficulty`. Filed above the
        /// markers: the append-point check reads a method line's first identifier.
        expansion_scales_with_difficulty => enable_expansion_scales_with_difficulty, disable_expansion_scales_with_difficulty;
        /// The stock alliance desk asks for a Research Alliance on any turn,
        /// ranks the partner by science, holds the slot for it while Scientific
        /// Theory is still short, and pays a premium for the first route to that
        /// ally while its level still climbs. See
        /// `advanced/research_alliance.rs`. Opt-in gene
        /// `research-alliance-first`. Filed here rather than under a marker: the
        /// append-point check reads a method line's first identifier.
        research_alliance_first => enable_research_alliance_first, disable_research_alliance_first;
        /// Rank required capitals within the selected domination campaign front.
        domination_capital_focus => enable_domination_capital_focus, disable_domination_capital_focus;
        /// See `AdvancedAi::expansion_hall_district`.
        expansion_hall_district => enable_expansion_hall_district, disable_expansion_hall_district;
        builder_charge_window => enable_builder_charge_window, disable_builder_charge_window;
        trade_growth_to_district => enable_trade_growth_to_district, disable_trade_growth_to_district;
        /// `builders-work-through-raiders`: the live capture lessons' reach holds
        /// keep Settlers only. See `AdvancedAi::builder_reach_safety_on`.
        builders_work_through_raiders => enable_builders_work_through_raiders, disable_builders_work_through_raiders;
        /// `settler-detour-stays-near`: a threat detour's fallback must lie about
        /// as close as the site it leaves. See `SETTLER_DETOUR_NEAR_SLACK`.
        settler_detour_stays_near => enable_settler_detour_stays_near, disable_settler_detour_stays_near;
        /// `beeline-orders-by-value`: a forced research or civic goal walks its
        /// prerequisites by value, not by printed price. See
        /// `AdvancedAi::beeline_step`.
        beeline_orders_by_value => enable_beeline_orders_by_value, disable_beeline_orders_by_value;
        /// `befriend-the-strongest`: offer a friendship-only deal to the
        /// strongest neighbour at peace. See `advanced/protective_friendship.rs`.
        befriend_the_strongest => enable_befriend_the_strongest, disable_befriend_the_strongest;
        /// `domination-ignores-city-states`: an assigned Domination seat keeps
        /// city-states out of its campaign's fallback ranking. See
        /// `AdvancedAi::conquest_campaign_considers_city_states`.
        domination_ignores_city_states => enable_domination_ignores_city_states, disable_domination_ignores_city_states;
        /// `domination-specializes-earlier`: an assigned Domination lane leaves
        /// its development half at 40% of the clock. See
        /// `AdvancedAi::domination_specializes_earlier`.
        domination_specializes_earlier => enable_domination_specializes_earlier, disable_domination_specializes_earlier;
        /// See `gold_and_cards::UPKEEP_RESERVE_TURNS`: upgrade passes keep one and
        /// a half turns of the army's bill in the treasury.
        upkeep_reserve => enable_upkeep_reserve, disable_upkeep_reserve;
        /// Opt-in gene `counter-war-needs-parity`; see
        /// [`Self::counter_war_needs_parity`].
        counter_war_needs_parity => enable_counter_war_needs_parity, disable_counter_war_needs_parity;
        /// Opt-in gene `shared-danger`; see [`Self::shared_danger`].
        shared_danger => enable_shared_danger, disable_shared_danger;
        /// Opt-in gene `strategic-deposit-prey`; see [`Self::strategic_deposit_prey`].
        strategic_deposit_prey => enable_strategic_deposit_prey, disable_strategic_deposit_prey;
        /// Opt-in gene `denial-nearest-finish`; see [`Self::denial_nearest_finish`].
        denial_nearest_finish => enable_denial_nearest_finish, disable_denial_nearest_finish;
        /// Opt-in gene `siege-train-scales-with-walls`; see
        /// [`Self::siege_train_scales_with_walls`].
        siege_train_scales_with_walls => enable_siege_train_scales_with_walls, disable_siege_train_scales_with_walls;
        /// Opt-in gene `siege-rally-holds`; see [`Self::siege_rally_holds`].
        siege_rally_holds => enable_siege_rally_holds, disable_siege_rally_holds;
        /// Opt-in gene `raze-a-doomed-capture`; see [`Self::raze_doomed_capture`].
        raze_doomed_capture => enable_raze_doomed_capture, disable_raze_doomed_capture;
        /// Opt-in gene `siege-holds-a-breach`; see [`Self::siege_holds_a_breach`].
        siege_holds_a_breach => enable_siege_holds_a_breach, disable_siege_holds_a_breach;
        /// Opt-in gene `stall-rebases-on-new-walls`; see
        /// [`Self::stall_rebases_on_new_walls`].
        stall_rebases_on_new_walls => enable_stall_rebases_on_new_walls, disable_stall_rebases_on_new_walls;
        /// Opt-in gene `siege-ranged-floor`; see [`Self::siege_ranged_floor`].
        siege_ranged_floor => enable_siege_ranged_floor, disable_siege_ranged_floor;
        /// Opt-in gene `denial-needs-a-road`; see [`Self::denial_needs_a_road`].
        denial_needs_a_road => enable_denial_needs_a_road, disable_denial_needs_a_road;
        /// Opt-in gene `siege-budget-counts-what-fires`; see
        /// [`Self::siege_budget_counts_what_fires`].
        siege_budget_counts_what_fires => enable_siege_budget_counts_what_fires, disable_siege_budget_counts_what_fires;
        /// Opt-in gene `siege-needs-a-breaker`; see
        /// [`Self::siege_needs_a_breaker`].
        siege_needs_a_breaker => enable_siege_needs_a_breaker, disable_siege_needs_a_breaker;
        /// Opt-in gene `siege-force-keeps-its-members`; see
        /// [`Self::siege_force_keeps_its_members`].
        siege_force_keeps_its_members => enable_siege_force_keeps_its_members, disable_siege_force_keeps_its_members;
        /// Opt-in gene `improvement-upgrades-count`; see
        /// [`Self::improvement_upgrades_count`].
        improvement_upgrades_count => enable_improvement_upgrades_count, disable_improvement_upgrades_count;
        /// Opt-in gene `staging-gun-trusts-its-escort`; see
        /// [`Self::staging_gun_trusts_its_escort`].
        staging_gun_trusts_its_escort => enable_staging_gun_trusts_its_escort, disable_staging_gun_trusts_its_escort;
        /// Opt-in gene `opening-force-keeps-its-members`; see
        /// [`Self::opening_force_keeps_its_members`].
        opening_force_keeps_its_members => enable_opening_force_keeps_its_members, disable_opening_force_keeps_its_members;
        /// Opt-in gene `one-sanctuary`; see [`Self::one_sanctuary`].
        one_sanctuary => enable_one_sanctuary, disable_one_sanctuary;
        /// Opt-in gene `runaway-expander-counter`; see
        /// [`Self::runaway_expander_counter`].
        runaway_expander_counter => enable_runaway_expander_counter, disable_runaway_expander_counter;
        /// Opt-in gene `decisive-window`; see [`Self::decisive_window`].
        decisive_window => enable_decisive_window, disable_decisive_window;
        /// Opt-in gene `unique-unit-preference`; see
        /// `BasicAi::unique_unit_preference`.
        base.unique_unit_preference => enable_unique_unit_preference, disable_unique_unit_preference;
        /// Opt-in gene `raids-cut-tourism`; see [`Self::raids_cut_tourism`].
        raids_cut_tourism => enable_raids_cut_tourism, disable_raids_cut_tourism;
        /// Opt-in gene `breaker-before-the-war`; see
        /// [`Self::breaker_before_the_war`].
        breaker_before_the_war => enable_breaker_before_the_war, disable_breaker_before_the_war;
        /// Opt-in gene `breach-assault-closes-in`; see
        /// [`Self::breach_assault_closes_in`]. Acts only beside `breach-assault`.
        breach_assault_closes_in => enable_breach_assault_closes_in, disable_breach_assault_closes_in;
        /// Opt-in gene `second-front-kept-when-winning`; see
        /// [`Self::second_front_kept_when_winning`].
        second_front_kept_when_winning => enable_second_front_kept_when_winning, disable_second_front_kept_when_winning;
        /// Opt-in gene `second-front-kept-when-winning-2`; see
        /// [`Self::second_front_kept_when_winning_2`]. Version one's terms stay
        /// with their own flag.
        second_front_kept_when_winning_2 => enable_second_front_kept_when_winning_2, disable_second_front_kept_when_winning_2;
        /// Opt-in gene `peace-waits-for-unseen-prey`; see
        /// [`Self::peace_waits_for_unseen_prey`].
        peace_waits_for_unseen_prey => enable_peace_waits_for_unseen_prey, disable_peace_waits_for_unseen_prey;
        /// Opt-in gene `front-finishes-its-siege`; see
        /// [`Self::front_finishes_its_siege`].
        front_finishes_its_siege => enable_front_finishes_its_siege, disable_front_finishes_its_siege;
        /// Opt-in gene `peace-waits-for-the-foothold`; see
        /// [`Self::peace_waits_for_the_foothold`].
        peace_waits_for_the_foothold => enable_peace_waits_for_the_foothold, disable_peace_waits_for_the_foothold;
        /// Opt-in gene `sanctuary-yields-a-held-queue`; see
        /// [`Self::sanctuary_yields_a_held_queue`].
        sanctuary_yields_a_held_queue => enable_sanctuary_yields_a_held_queue, disable_sanctuary_yields_a_held_queue;
        /// Opt-in gene `found-against-a-rival-faith`; see
        /// [`Self::found_against_a_rival_faith`].
        found_against_a_rival_faith => enable_found_against_a_rival_faith, disable_found_against_a_rival_faith;
        /// Opt-in gene `domination-strikes-when-staged`; see
        /// [`Self::domination_strikes_when_staged`].
        domination_strikes_when_staged => enable_domination_strikes_when_staged, disable_domination_strikes_when_staged;
        /// Opt-in gene `capture-waits-on-the-march`; see
        /// [`Self::capture_waits_on_the_march`].
        capture_waits_on_the_march => enable_capture_waits_on_the_march, disable_capture_waits_on_the_march;
        /// Opt-in gene `recovery-keeps-a-winning-war`; see
        /// [`Self::recovery_keeps_a_winning_war`].
        recovery_keeps_a_winning_war => enable_recovery_keeps_a_winning_war, disable_recovery_keeps_a_winning_war;
        /// Opt-in gene `one-war-swaps-a-stalled-front`; see
        /// [`Self::one_war_swaps_a_stalled_front`].
        one_war_swaps_a_stalled_front => enable_one_war_swaps_a_stalled_front, disable_one_war_swaps_a_stalled_front;
        /// Opt-in gene `rout-spares-a-stronger-army`; see
        /// [`Self::rout_spares_a_stronger_army`].
        rout_spares_a_stronger_army => enable_rout_spares_a_stronger_army, disable_rout_spares_a_stronger_army;
        /// Opt-in gene `culture-counter-declares`; see
        /// [`Self::culture_counter_declares`].
        culture_counter_declares => enable_culture_counter_declares, disable_culture_counter_declares;
        /// Opt-in gene `last-capital-war-kept`; see
        /// [`Self::last_capital_war_kept`].
        last_capital_war_kept => enable_last_capital_war_kept, disable_last_capital_war_kept;
        /// Opt-in gene `formations-heed-refusals`; see
        /// [`Self::formations_heed_refusals`].
        formations_heed_refusals => enable_formations_heed_refusals, disable_formations_heed_refusals;
        /// Opt-in gene `diplomatic-contender-kept`; see
        /// [`Self::diplomatic_contender_kept`].
        diplomatic_contender_kept => enable_diplomatic_contender_kept, disable_diplomatic_contender_kept;
        /// Opt-in gene `diplomatic-contender-kept-2`; see
        /// [`Self::diplomatic_contender_kept_2`]. Version one's terms stay with
        /// their own flag.
        diplomatic_contender_kept_2 => enable_diplomatic_contender_kept_2, disable_diplomatic_contender_kept_2;
        /// Opt-in gene `breaker-supply-scales-2`; see
        /// [`Self::breaker_supply_scales_2`].
        breaker_supply_scales_2 => enable_breaker_supply_scales_2, disable_breaker_supply_scales_2;
        /// Opt-in gene `breaker-supply-scales`; see
        /// [`Self::breaker_supply_scales`].
        breaker_supply_scales => enable_breaker_supply_scales, disable_breaker_supply_scales;
        /// Opt-in gene `breach-assault`; see [`Self::breach_assault`].
        breach_assault => enable_breach_assault, disable_breach_assault;
        /// Opt-in gene `siege-counts-posted-shooters`; see
        /// [`Self::siege_counts_posted_shooters`].
        siege_counts_posted_shooters => enable_siege_counts_posted_shooters, disable_siege_counts_posted_shooters;
        /// Opt-in gene `capital-defense-holds`; see
        /// [`Self::capital_defense_contact`].
        capital_defense_holds => enable_capital_defense_holds, disable_capital_defense_holds;
        /// Opt-in gene `staging-column-passes-through`; see
        /// [`Self::staging_column_passes_through`].
        staging_column_passes_through => enable_staging_column_passes_through, disable_staging_column_passes_through;
        /// Opt-in gene `breakers-stay-with-the-siege`; see
        /// [`Self::breakers_stay_with_the_siege`].
        breakers_stay_with_the_siege => enable_breakers_stay_with_the_siege, disable_breakers_stay_with_the_siege;
        /// Opt-in gene `stalled-peace-spares-the-counter`; see
        /// [`Self::stalled_peace_spares_the_counter`].
        stalled_peace_spares_the_counter => enable_stalled_peace_spares_the_counter, disable_stalled_peace_spares_the_counter;
        /// Opt-in gene `staging-gun-remembers-hostiles`; see
        /// [`Self::staging_gun_remembers_hostiles`].
        staging_gun_remembers_hostiles => enable_staging_gun_remembers_hostiles, disable_staging_gun_remembers_hostiles;
        /// Opt-in gene `conquest-opening-needs-the-production`; see
        /// [`Self::conquest_force_estimate`].
        conquest_opening_needs_the_production => enable_conquest_opening_needs_the_production, disable_conquest_opening_needs_the_production;
        /// Opt-in gene `war-bill-prices-the-tier-gap`; see
        /// [`Self::war_bill_prices_the_tier_gap`].
        war_bill_prices_the_tier_gap => enable_war_bill_prices_the_tier_gap, disable_war_bill_prices_the_tier_gap;
        /// Opt-in gene `dialogue-never-declares-war`; see
        /// [`Self::dialogue_never_declares_war`].
        dialogue_never_declares_war => enable_dialogue_never_declares_war, disable_dialogue_never_declares_war;
        /// Opt-in gene `declaration-waits-for-the-breaker`; see
        /// [`Self::declaration_waits_for_the_breaker`].
        declaration_waits_for_the_breaker => enable_declaration_waits_for_the_breaker, disable_declaration_waits_for_the_breaker;
        /// Opt-in gene `capital-prey-opens-a-front`; see
        /// [`Self::capital_prey_opens_a_front`].
        capital_prey_opens_a_front => enable_capital_prey_opens_a_front, disable_capital_prey_opens_a_front;
        /// Opt-in gene `front-needs-a-declarable-rival`; see
        /// [`Self::front_needs_a_declarable_rival`].
        front_needs_a_declarable_rival => enable_front_needs_a_declarable_rival, disable_front_needs_a_declarable_rival;
        /// Opt-in gene `front-finishes-its-capital`; see
        /// [`Self::front_finishes_its_capital`].
        front_finishes_its_capital => enable_front_finishes_its_capital, disable_front_finishes_its_capital;
        /// Opt-in gene `religious-threat-spares-the-front`; see
        /// [`Self::religious_threat_spares_the_front`].
        religious_threat_spares_the_front => enable_religious_threat_spares_the_front, disable_religious_threat_spares_the_front;
        /// Opt-in gene `culture-finish-at-the-observed-bar`; see
        /// [`Self::culture_finish_at_the_observed_bar`].
        culture_finish_at_the_observed_bar => enable_culture_finish_at_the_observed_bar, disable_culture_finish_at_the_observed_bar;
        /// Opt-in gene `stage-march-keeps-to-land`; see
        /// [`Self::stage_march_keeps_to_land`].
        stage_march_keeps_to_land => enable_stage_march_keeps_to_land, disable_stage_march_keeps_to_land;
        /// Opt-in gene `breach-counts-nearby-guns`; see
        /// [`Self::breach_counts_nearby_guns`].
        breach_counts_nearby_guns => enable_breach_counts_nearby_guns, disable_breach_counts_nearby_guns;
        /// Opt-in gene `peace-asks-a-city`; see [`Self::peace_asks_a_city`].
        peace_asks_a_city => enable_peace_asks_a_city, disable_peace_asks_a_city;
        /// Opt-in gene `capital-prey-opens-a-front-2`; see
        /// [`Self::capital_prey_opens_a_front_2`].
        capital_prey_opens_a_front_2 => enable_capital_prey_opens_a_front_2, disable_capital_prey_opens_a_front_2;
        /// Opt-in gene `air-surge-strikes-through-urgency`; see
        /// [`Self::air_surge_strikes_through_urgency`].
        air_surge_strikes_through_urgency => enable_air_surge_strikes_through_urgency, disable_air_surge_strikes_through_urgency;
        /// Opt-in gene `liberation-funds-the-congress`; see
        /// [`Self::liberation_funds_the_congress`].
        liberation_funds_the_congress => enable_liberation_funds_the_congress, disable_liberation_funds_the_congress;
        /// Opt-in gene `guns-enter-together`; see
        /// [`Self::guns_enter_together`].
        guns_enter_together => enable_guns_enter_together, disable_guns_enter_together;
        /// Opt-in gene `melee-storms-an-open-city`; see
        /// [`Self::melee_storms_an_open_city`].
        melee_storms_an_open_city => enable_melee_storms_an_open_city, disable_melee_storms_an_open_city;
        /// Opt-in gene `capture-holds-the-ring`; see
        /// [`Self::capture_holds_the_ring`].
        capture_holds_the_ring => enable_capture_holds_the_ring, disable_capture_holds_the_ring;
        /// Opt-in gene `breach-reads-the-air`; see
        /// [`Self::breach_reads_the_air`].
        breach_reads_the_air => enable_breach_reads_the_air, disable_breach_reads_the_air;
        /// Opt-in gene `culture-reads-the-engine-clock`; see
        /// [`Self::culture_reads_the_engine_clock`].
        culture_reads_the_engine_clock => enable_culture_reads_the_engine_clock, disable_culture_reads_the_engine_clock;
        /// Opt-in gene `rout-spares-the-counter`; see
        /// [`Self::rout_spares_the_counter`].
        rout_spares_the_counter => enable_rout_spares_the_counter, disable_rout_spares_the_counter;
        /// Opt-in gene `age-closer-spends-the-reserve`; see
        /// [`Self::age_closer_spends_the_reserve`].
        age_closer_spends_the_reserve => enable_age_closer_spends_the_reserve, disable_age_closer_spends_the_reserve;
        /// Opt-in gene `find-the-capital`; see [`Self::find_the_capital`].
        find_the_capital => enable_find_the_capital, disable_find_the_capital;
        /// Opt-in gene `air-volley-needs-a-road`; see
        /// [`Self::air_volley_needs_a_road`].
        air_volley_needs_a_road => enable_air_volley_needs_a_road, disable_air_volley_needs_a_road;
        /// Opt-in gene `guns-stay-out-of-reach`; see
        /// [`Self::guns_stay_out_of_reach`].
        guns_stay_out_of_reach => enable_guns_stay_out_of_reach, disable_guns_stay_out_of_reach;
        /// Opt-in gene `capital-prey-scales-the-walls`; see
        /// [`Self::capital_prey_scales_the_walls`].
        capital_prey_scales_the_walls => enable_capital_prey_scales_the_walls, disable_capital_prey_scales_the_walls;
        /// Opt-in gene `faith-counter-waits-for-match-point`; see
        /// [`Self::faith_counter_waits_for_match_point`].
        faith_counter_waits_for_match_point => enable_faith_counter_waits_for_match_point, disable_faith_counter_waits_for_match_point;
        /// Opt-in gene `prey-reads-a-steady-power`; see
        /// [`Self::prey_reads_a_steady_power`].
        prey_reads_a_steady_power => enable_prey_reads_a_steady_power, disable_prey_reads_a_steady_power;
        /// Opt-in gene `second-front-keeps-its-war`; see
        /// [`Self::second_front_keeps_its_war`].
        second_front_keeps_its_war => enable_second_front_keeps_its_war, disable_second_front_keeps_its_war;
        /// Opt-in gene `counterweight-faith-is-no-threat`; see
        /// [`Self::counterweight_faith_is_no_threat`].
        counterweight_faith_is_no_threat => enable_counterweight_faith_is_no_threat, disable_counterweight_faith_is_no_threat;
        /// Opt-in gene `ranged-before-melee`; see
        /// [`Self::ranged_before_melee`].
        ranged_before_melee => enable_ranged_before_melee, disable_ranged_before_melee;
        /// Opt-in gene `tower-assault`; see
        /// [`Self::tower_assault`].
        tower_assault => enable_tower_assault, disable_tower_assault;
        /// Opt-in gene `ring-fires-on-the-city`; see
        /// [`Self::ring_fires_on_the_city`].
        ring_fires_on_the_city => enable_ring_fires_on_the_city, disable_ring_fires_on_the_city;
        /// Opt-in gene `recovery-peace-waits`; see
        /// [`Self::recovery_peace_waits`].
        recovery_peace_waits => enable_recovery_peace_waits, disable_recovery_peace_waits;
        /// Opt-in gene `favor-bought-before-congress`; see
        /// [`Self::favor_bought_before_congress`].
        favor_bought_before_congress => enable_favor_bought_before_congress, disable_favor_bought_before_congress;
        /// Opt-in gene `favor-spares-the-surprise-war`; see
        /// [`Self::favor_spares_the_surprise_war`].
        favor_spares_the_surprise_war => enable_favor_spares_the_surprise_war, disable_favor_spares_the_surprise_war;
        /// Opt-in gene `conquest-opening-stays-near`; see
        /// [`Self::conquest_opening_stays_near`].
        conquest_opening_stays_near => enable_conquest_opening_stays_near, disable_conquest_opening_stays_near;
        /// Opt-in gene `science-ladder-reads-the-clock`; see
        /// [`Self::science_ladder_reads_the_clock`].
        science_ladder_reads_the_clock => enable_science_ladder_reads_the_clock, disable_science_ladder_reads_the_clock;
        /// Opt-in gene `contender-at-peace-is-the-target`; see
        /// [`Self::contender_at_peace_is_the_target`].
        contender_at_peace_is_the_target => enable_contender_at_peace_is_the_target, disable_contender_at_peace_is_the_target;
        /// Opt-in gene `campaign-weighs-the-tourism-leader`; see
        /// [`Self::campaign_weighs_the_tourism_leader`].
        campaign_weighs_the_tourism_leader => enable_campaign_weighs_the_tourism_leader, disable_campaign_weighs_the_tourism_leader;
        /// Opt-in gene `recovery-needs-the-deficit`; see
        /// [`Self::recovery_needs_the_deficit`].
        recovery_needs_the_deficit => enable_recovery_needs_the_deficit, disable_recovery_needs_the_deficit;
        /// Opt-in gene `domination-finish-holds-the-front`; see
        /// [`Self::domination_finish_holds_the_front`].
        domination_finish_holds_the_front => enable_domination_finish_holds_the_front, disable_domination_finish_holds_the_front;
        /// Opt-in gene `declaration-needs-the-edge`; see
        /// [`Self::declaration_needs_the_edge`].
        declaration_needs_the_edge => enable_declaration_needs_the_edge, disable_declaration_needs_the_edge;
        /// Opt-in gene `diplomatic-contender-eliminated`; see
        /// [`Self::diplomatic_contender_eliminated`].
        diplomatic_contender_eliminated => enable_diplomatic_contender_eliminated, disable_diplomatic_contender_eliminated;
        /// Opt-in gene `breaker-reads-the-march`; see
        /// [`Self::breaker_reads_the_march`].
        breaker_reads_the_march => enable_breaker_reads_the_march, disable_breaker_reads_the_march;
        /// Opt-in gene `science-denounce-waits-for-the-race`; see
        /// [`Self::science_denounce_waits_for_the_race`].
        science_denounce_waits_for_the_race => enable_science_denounce_waits_for_the_race, disable_science_denounce_waits_for_the_race;
        /// Opt-in gene `luxury-buy-asks`; see [`Self::luxury_buy_asks`].
        luxury_buy_asks => enable_luxury_buy_asks, disable_luxury_buy_asks;
        /// Opt-in gene `overwhelming-power-declares`; see
        /// [`Self::overwhelming_power_declares`].
        overwhelming_power_declares => enable_overwhelming_power_declares, disable_overwhelming_power_declares;
        /// Opt-in gene `stale-swap-reads-the-march`; see
        /// [`Self::stale_swap_reads_the_march`].
        stale_swap_reads_the_march => enable_stale_swap_reads_the_march, disable_stale_swap_reads_the_march;
        /// Opt-in gene `march-uses-its-moves`; see
        /// [`Self::march_uses_its_moves`].
        march_uses_its_moves => enable_march_uses_its_moves, disable_march_uses_its_moves;
        /// Opt-in gene `siege-members-use-their-moves`; see
        /// [`Self::siege_members_use_their_moves`].
        siege_members_use_their_moves => enable_siege_members_use_their_moves, disable_siege_members_use_their_moves;
        /// Opt-in gene `guns-grind-the-walls`; see
        /// [`Self::guns_grind_the_walls`].
        guns_grind_the_walls => enable_guns_grind_the_walls, disable_guns_grind_the_walls;
        /// Opt-in gene `tier-gap-priced-once`; see
        /// [`Self::tier_gap_priced_once`].
        tier_gap_priced_once => enable_tier_gap_priced_once, disable_tier_gap_priced_once;
        /// Opt-in gene `breaker-research-first`; see
        /// [`Self::breaker_research_first`].
        breaker_research_first => enable_breaker_research_first, disable_breaker_research_first;
        /// Opt-in gene `breach-support-reads-the-wall-tier`; see
        /// [`Self::breach_support_reads_the_wall_tier`].
        breach_support_reads_the_wall_tier => enable_breach_support_reads_the_wall_tier, disable_breach_support_reads_the_wall_tier;
        /// Opt-in gene `second-front-waits-for-its-war`; see
        /// [`Self::second_front_waits_for_its_war`].
        second_front_waits_for_its_war => enable_second_front_waits_for_its_war, disable_second_front_waits_for_its_war;
        /// Opt-in gene `guns-post-for-a-near-breach`; see
        /// [`Self::guns_post_for_a_near_breach`].
        guns_post_for_a_near_breach => enable_guns_post_for_a_near_breach, disable_guns_post_for_a_near_breach;
        /// Opt-in gene `golden-dedication-serves-the-conquest`; see
        /// [`Self::golden_dedication_serves_the_conquest`].
        golden_dedication_serves_the_conquest => enable_golden_dedication_serves_the_conquest, disable_golden_dedication_serves_the_conquest;
        /// Opt-in gene `prophet-race-earns-its-points`; see
        /// [`Self::reserve_race_shrine`] and [`Self::race_wants_revelation`].
        prophet_race_earns_its_points => enable_prophet_race_earns_its_points, disable_prophet_race_earns_its_points;
        /// Opt-in gene `faith-counter-needs-the-edge`; see
        /// `AdvancedAi::faith_counter_has_the_edge`.
        faith_counter_needs_the_edge => enable_faith_counter_needs_the_edge, disable_faith_counter_needs_the_edge;
        /// Opt-in gene `stall-waits-for-the-breach`; see
        /// [`Self::stall_waits_for_the_breach`].
        stall_waits_for_the_breach => enable_stall_waits_for_the_breach, disable_stall_waits_for_the_breach;
        /// Opt-in gene `counterfaith-leaves-two-holdouts`; see
        /// [`Self::counterfaith_is_safe`].
        counterfaith_leaves_two_holdouts => enable_counterfaith_leaves_two_holdouts, disable_counterfaith_leaves_two_holdouts;
        /// Opt-in gene `flipped-capital-finishes`; see
        /// [`Self::flipped_capital_finishes`].
        flipped_capital_finishes => enable_flipped_capital_finishes, disable_flipped_capital_finishes;
        /// Opt-in gene `siege-target-needs-a-road`; see
        /// [`Self::reconcile_siege_roads`].
        siege_target_needs_a_road => enable_siege_target_needs_a_road, disable_siege_target_needs_a_road;
        /// Opt-in gene `opening-yields-to-walls`; see
        /// [`Self::opening_yields_to_walls`].
        opening_yields_to_walls => enable_opening_yields_to_walls, disable_opening_yields_to_walls;
        /// Opt-in gene `stranded-settler-leaves-the-corridor`; see
        /// [`Self::stranded_settler_leaves_the_corridor`].
        stranded_settler_leaves_the_corridor => enable_stranded_settler_leaves_the_corridor, disable_stranded_settler_leaves_the_corridor;
        /// Opt-in gene `blocker-becomes-the-target`; see
        /// [`Self::road_blocker_front`].
        blocker_becomes_the_target => enable_blocker_becomes_the_target, disable_blocker_becomes_the_target;
        /// Opt-in gene `stage-musters-out-of-reach`; see
        /// [`Self::stage_musters_out_of_reach`].
        stage_musters_out_of_reach => enable_stage_musters_out_of_reach, disable_stage_musters_out_of_reach;
        /// Opt-in gene `staging-gun-reads-the-shared-danger`; see
        /// [`Self::staging_gun_reads_the_shared_danger`].
        staging_gun_reads_the_shared_danger => enable_staging_gun_reads_the_shared_danger, disable_staging_gun_reads_the_shared_danger;
        /// Opt-in gene `urgent-denial-needs-the-edge`; see
        /// `AdvancedAi::urgent_denial_has_the_edge`.
        urgent_denial_needs_the_edge => enable_urgent_denial_needs_the_edge, disable_urgent_denial_needs_the_edge;
        /// Opt-in gene `bombers-open-the-siege-walls`; see
        /// [`Self::bombers_open_the_siege_walls`].
        bombers_open_the_siege_walls => enable_bombers_open_the_siege_walls, disable_bombers_open_the_siege_walls;
        /// Opt-in gene `air-fire-counts-on-the-city`; see
        /// [`Self::air_fire_counts_on_the_city`].
        air_fire_counts_on_the_city => enable_air_fire_counts_on_the_city, disable_air_fire_counts_on_the_city;
        /// Opt-in gene `capital-taken-moves-on`; see
        /// `AdvancedAi::capital_moves_on_next`.
        capital_taken_moves_on => enable_capital_taken_moves_on, disable_capital_taken_moves_on;
        /// Opt-in gene `tier-gap-reads-the-fielded-line`; see
        /// [`Self::tier_gap_reads_the_fielded_line`].
        tier_gap_reads_the_fielded_line => enable_tier_gap_reads_the_fielded_line, disable_tier_gap_reads_the_fielded_line;
        /// Opt-in gene `unwalled-target-declares-into-the-strike`; see
        /// [`Self::unwalled_target_declares_into_the_strike`].
        unwalled_target_declares_into_the_strike => enable_unwalled_target_declares_into_the_strike, disable_unwalled_target_declares_into_the_strike;
        /// Opt-in gene `unwalled-city-takes-the-swarm`; see
        /// [`Self::unwalled_city_takes_the_swarm`].
        unwalled_city_takes_the_swarm => enable_unwalled_city_takes_the_swarm, disable_unwalled_city_takes_the_swarm;
        /// Opt-in gene `religious-match-point-defence`; see
        /// `AdvancedAi::match_point_defence_has_the_edge`.
        religious_match_point_defence => enable_religious_match_point_defence, disable_religious_match_point_defence;
        /// Opt-in gene `founder-spreads-only-its-faith`; see
        /// `advanced/founder_faith.rs`.
        founder_spreads_only_its_faith => enable_founder_spreads_only_its_faith, disable_founder_spreads_only_its_faith;
        /// Opt-in gene `science-leader-is-the-target`; see
        /// [`Self::science_leader_is_the_target`].
        science_leader_is_the_target => enable_science_leader_is_the_target, disable_science_leader_is_the_target;
        /// Opt-in gene `counter-war-needs-the-emperor-edge`; see
        /// [`Self::counter_war_has_the_emperor_edge`].
        counter_war_needs_the_emperor_edge => enable_counter_war_needs_the_emperor_edge, disable_counter_war_needs_the_emperor_edge;
        /// Opt-in gene `declaration-waits-for-the-breach`; see
        /// [`Self::declaration_waits_for_the_breach`].
        declaration_waits_for_the_breach => enable_declaration_waits_for_the_breach, disable_declaration_waits_for_the_breach;
        /// Opt-in gene `counterweight-spends-the-bank`; see
        /// `advanced/counterweight_bank.rs`.
        counterweight_spends_the_bank => enable_counterweight_spends_the_bank, disable_counterweight_spends_the_bank;
        /// Opt-in gene `declaration-needs-production-parity`; see
        /// [`Self::declaration_has_production_parity`].
        declaration_needs_production_parity => enable_declaration_needs_production_parity, disable_declaration_needs_production_parity;
        /// Opt-in gene `parity-reads-the-front`; see
        /// [`Self::rival_front_production`]. Only matters while
        /// `declaration-needs-production-parity` is on.
        parity_reads_the_front => enable_parity_reads_the_front, disable_parity_reads_the_front;
        /// Opt-in gene `founder-keeps-two-sources`; see
        /// `advanced/second_faith_source.rs`.
        founder_keeps_two_sources => enable_founder_keeps_two_sources, disable_founder_keeps_two_sources;
        /// Opt-in gene `invest-keeps-its-cavalry`; see
        /// [`Self::invest_keeps_its_cavalry`].
        invest_keeps_its_cavalry => enable_invest_keeps_its_cavalry, disable_invest_keeps_its_cavalry;
        /// Opt-in gene `founder-funds-the-inquisition`; see
        /// `advanced/inquisition_first.rs`.
        founder_funds_the_inquisition => enable_founder_funds_the_inquisition, disable_founder_funds_the_inquisition;
        /// Opt-in gene `theater-keeps-its-amphitheater`; see
        /// `AdvancedAi::theater_keeps_its_amphitheater`.
        theater_keeps_its_amphitheater => enable_theater_keeps_its_amphitheater, disable_theater_keeps_its_amphitheater;
        /// Opt-in gene `siege-train-upgrades-first`; see
        /// [`Self::siege_train_upgrades_first`].
        siege_train_upgrades_first => enable_siege_train_upgrades_first, disable_siege_train_upgrades_first;
        /// Opt-in gene `wounded-taker-finishes-the-breach`; see
        /// [`Self::wounded_taker_finishes_the_breach`].
        wounded_taker_finishes_the_breach => enable_wounded_taker_finishes_the_breach, disable_wounded_taker_finishes_the_breach;
        /// Opt-in gene `urban-planning-fills-the-slot`; see
        /// `AdvancedAi::urban_planning_fills_the_slot`.
        urban_planning_fills_the_slot => enable_urban_planning_fills_the_slot, disable_urban_planning_fills_the_slot;
        /// Opt-in gene `growth-prices-the-farm`; see
        /// `AdvancedAi::growth_prices_the_farm`.
        growth_prices_the_farm => enable_growth_prices_the_farm, disable_growth_prices_the_farm;
        /// Opt-in gene `siege-buys-the-passage`; see
        /// [`Self::siege_buys_the_passage`].
        siege_buys_the_passage => enable_siege_buys_the_passage, disable_siege_buys_the_passage;
        /// Opt-in gene `weak-target-skips-the-muster`; see
        /// [`Self::weak_target_skips_the_muster`].
        weak_target_skips_the_muster => enable_weak_target_skips_the_muster, disable_weak_target_skips_the_muster;
        /// Opt-in gene `breakers-match-the-walls`; see
        /// [`Self::breakers_match_the_walls`].
        breakers_match_the_walls => enable_breakers_match_the_walls, disable_breakers_match_the_walls;
        /// Opt-in gene `siege-buys-the-gun-resource`; see
        /// [`Self::siege_buys_the_gun_resource`].
        siege_buys_the_gun_resource => enable_siege_buys_the_gun_resource, disable_siege_buys_the_gun_resource;
        /// Opt-in `army-upgrade-materials`; see `army_upgrade_materials`.
        army_upgrade_materials => enable_army_upgrade_materials, disable_army_upgrade_materials;
        /// Opt-in gene `siege-tier-yields-to-the-bombers`; see
        /// [`Self::siege_tier_yields_to_the_bombers`].
        siege_tier_yields_to_the_bombers => enable_siege_tier_yields_to_the_bombers, disable_siege_tier_yields_to_the_bombers;
        /// Opt-in gene `surge-fields-the-bombers`; see
        /// [`Self::surge_fields_the_bombers`].
        surge_fields_the_bombers => enable_surge_fields_the_bombers, disable_surge_fields_the_bombers;
        /// Opt-in gene `prophet-builds-its-site`; see
        /// [`Self::reserve_prophet_site`] and [`Self::prophet_race_exhausted`].
        prophet_builds_its_site => enable_prophet_builds_its_site, disable_prophet_builds_its_site;
        /// Opt-in gene `counterweight-from-the-first-convert`; see
        /// [`Self::first_convert_threat`] and [`Self::counterweight_need_for`].
        counterweight_from_the_first_convert => enable_counterweight_from_the_first_convert, disable_counterweight_from_the_first_convert;
        /// Opt-in gene `long-road-names-the-blocker`; see
        /// [`Self::long_road_names_the_blocker`].
        long_road_names_the_blocker => enable_long_road_names_the_blocker, disable_long_road_names_the_blocker;
        /// Opt-in gene `stalled-settler-takes-a-safe-site`; see
        /// `advanced/stalled_settler_site.rs`.
        stalled_settler_takes_a_safe_site => enable_stalled_settler_takes_a_safe_site, disable_stalled_settler_takes_a_safe_site;
        /// Opt-in gene `gun-queues-behind-the-column`; see
        /// [`Self::gun_queues_behind_the_column`].
        gun_queues_behind_the_column => enable_gun_queues_behind_the_column, disable_gun_queues_behind_the_column;
        /// Opt-in gene `builders-improve-the-worked-for-production`; see
        /// `advanced/worked_production.rs`.
        builders_improve_the_worked_for_production => enable_builders_improve_the_worked_for_production, disable_builders_improve_the_worked_for_production;
        /// Opt-in gene `founder-defends-its-cities`; see
        /// [`Self::founder_defence_live`].
        founder_defends_its_cities => enable_founder_defends_its_cities, disable_founder_defends_its_cities;
        /// Opt-in gene `luxury-swap-asks`; see [`Self::luxury_swap_asks`].
        luxury_swap_asks => enable_luxury_swap_asks, disable_luxury_swap_asks;
        /// Opt-in gene `staging-reaches-the-border`; see
        /// [`Self::staging_reaches_the_border`].
        staging_reaches_the_border => enable_staging_reaches_the_border, disable_staging_reaches_the_border;
        /// Enable `science-denial-every-pad`.
        science_denial_every_pad => enable_science_denial_every_pad, disable_science_denial_every_pad;
        /// Opt-in gene `dvp-leader-is-the-front`; see
        /// [`Self::dvp_leader_is_the_front`].
        dvp_leader_is_the_front => enable_dvp_leader_is_the_front, disable_dvp_leader_is_the_front;
        /// Opt-in gene `congress-guards-the-leader`; see
        /// [`Self::congress_guards_the_leader`].
        congress_guards_the_leader => enable_congress_guards_the_leader, disable_congress_guards_the_leader;
        /// Opt-in gene `counter-out-of-reach-takes-the-weak`; see
        /// [`Self::counter_out_of_reach_takes_the_weak`].
        counter_out_of_reach_takes_the_weak => enable_counter_out_of_reach_takes_the_weak, disable_counter_out_of_reach_takes_the_weak;
        /// Enable `science-denial-trains-spies`.
        science_denial_trains_spies => enable_science_denial_trains_spies, disable_science_denial_trains_spies;
        /// Enable `science-suppression-hits-the-pads`.
        science_suppression_hits_the_pads => enable_science_suppression_hits_the_pads, disable_science_suppression_hits_the_pads;
        /// Enable `culture-denial-heist`.
        culture_denial_heist => enable_culture_denial_heist, disable_culture_denial_heist;
        /// Enable `war-kills-the-bands`.
        war_kills_the_bands => enable_war_kills_the_bands, disable_war_kills_the_bands;
        /// Enable `war-raids-the-pads`.
        war_raids_the_pads => enable_war_raids_the_pads, disable_war_raids_the_pads;
        /// Enable `no-peace-with-a-launcher`.
        no_peace_with_a_launcher => enable_no_peace_with_a_launcher, disable_no_peace_with_a_launcher;
        /// Enable `launcher-war-ignores-the-edge`.
        launcher_war_ignores_the_edge => enable_launcher_war_ignores_the_edge, disable_launcher_war_ignores_the_edge;
        /// Opt-in gene `defeated-majors-leave-the-board`; see
        /// [`Self::retire_defeated_majors`].
        defeated_majors_leave_the_board => enable_defeated_majors_leave_the_board, disable_defeated_majors_leave_the_board;
        /// Opt-in gene `elimination-waits-on-the-clock`; see
        /// [`Self::elimination_waits_on_the_clock`].
        elimination_waits_on_the_clock => enable_elimination_waits_on_the_clock, disable_elimination_waits_on_the_clock;
        /// Enable `early-settler-floor`.
        early_settler_floor => enable_early_settler_floor, disable_early_settler_floor;
        /// Enable `war-kills-the-bands-2`.
        war_kills_the_bands_2 => enable_war_kills_the_bands_2, disable_war_kills_the_bands_2;
        /// Opt-in gene `match-point-interception-ignores-power`; see
        /// [`Self::last_holdout_interception`].
        match_point_interception_ignores_power => enable_match_point_interception_ignores_power, disable_match_point_interception_ignores_power;
        /// Opt-in gene `counterweight-finishes-one-shrine`; see
        /// [`Self::counterweight_shrine_city`] and [`Self::counterweight_bank_held`].
        counterweight_finishes_one_shrine => enable_counterweight_finishes_one_shrine, disable_counterweight_finishes_one_shrine;
        /// Opt-in gene `falling-city-outranks-the-heal`; see
        /// [`Self::falling_city_outranks_the_heal`].
        falling_city_outranks_the_heal => enable_falling_city_outranks_the_heal, disable_falling_city_outranks_the_heal;
        /// Enable `near-rival-deterrence`.
        near_rival_deterrence => enable_near_rival_deterrence, disable_near_rival_deterrence;
        /// Opt-in gene `wall-sortie-skips-the-encampment`; see
        /// [`Self::wall_sortie_skips_the_encampment`].
        wall_sortie_skips_the_encampment => enable_wall_sortie_skips_the_encampment, disable_wall_sortie_skips_the_encampment;
        /// Opt-in gene `counterweight-flips-the-small-towns`; see
        /// [`Self::small_town_threat`] and [`Self::small_town_target_adjustment`].
        counterweight_flips_the_small_towns => enable_counterweight_flips_the_small_towns, disable_counterweight_flips_the_small_towns;
        /// Opt-in gene `muster-walks-the-road`; see
        /// [`Self::muster_walks_the_road`].
        muster_walks_the_road => enable_muster_walks_the_road, disable_muster_walks_the_road;
        unseen_capital_is_unseen => enable_unseen_capital_is_unseen, disable_unseen_capital_is_unseen;
        /// Opt-in gene `peacetime-classical-republic`; see
        /// [`Self::peacetime_republic_choice`].
        peacetime_classical_republic => enable_peacetime_classical_republic, disable_peacetime_classical_republic;
        /// Opt-in gene `bombers-fly-on-a-small-stock`; see
        /// [`Self::bombers_fly_on_a_small_stock`].
        bombers_fly_on_a_small_stock => enable_bombers_fly_on_a_small_stock, disable_bombers_fly_on_a_small_stock;
        /// Opt-in gene `settler-walks-to-the-better-site`; see
        /// [`Self::better_site_beyond_the_turn`].
        settler_walks_to_the_better_site => enable_settler_walks_to_the_better_site, disable_settler_walks_to_the_better_site;
        /// Opt-in gene `early-conquest-stands-down`; see
        /// [`Self::conquest_stands_down`].
        early_conquest_stands_down => enable_early_conquest_stands_down, disable_early_conquest_stands_down;
        /// Opt-in gene `granary-claims-the-bound-queue`; see
        /// `advanced/granary_claims_the_bound_queue.rs`.
        granary_claims_the_bound_queue => enable_granary_claims_the_bound_queue, disable_granary_claims_the_bound_queue;
        /// Opt-in gene `capital-campus-before-the-plaza`; see
        /// `advanced/capital_campus_claim.rs`.
        capital_campus_before_the_plaza => enable_capital_campus_before_the_plaza, disable_capital_campus_before_the_plaza;
        /// Opt-in gene `gold-buys-the-settler`; see
        /// `advanced/gold_buys_the_settler.rs`.
        gold_buys_the_settler => enable_gold_buys_the_settler, disable_gold_buys_the_settler;
        /// Opt-in gene `trader-fills-the-idle-route`; see
        /// `advanced/trader_fills_the_idle_route.rs`.
        trader_fills_the_idle_route => enable_trader_fills_the_idle_route, disable_trader_fills_the_idle_route;
        /// Opt-in gene `civic-awaits-its-inspiration`; see
        /// `advanced/civic_inspiration_wait.rs`.
        civic_awaits_its_inspiration => enable_civic_awaits_its_inspiration, disable_civic_awaits_its_inspiration;
        /// Opt-in gene `backlog-builder-takes-the-queue`; see
        /// `advanced/backlog_builder_takes_the_queue.rs`.
        backlog_builder_takes_the_queue => enable_backlog_builder_takes_the_queue, disable_backlog_builder_takes_the_queue;
        /// Opt-in gene `deterrence-spares-the-faith-defence`; see
        /// `deterrence_may_displace` in `advanced/near_rival_deterrence.rs`.
        deterrence_spares_the_faith_defence => enable_deterrence_spares_the_faith_defence, disable_deterrence_spares_the_faith_defence;
        /// Opt-in gene `falling-city-calls-the-takers`; see
        /// `advanced/falling_city_takers.rs`.
        falling_city_calls_the_takers => enable_falling_city_calls_the_takers, disable_falling_city_calls_the_takers;
        /// Opt-in gene `space-denial-spies-from-rocketry`; see
        /// `denial_spy_racers` in `advanced/science_denial_every_pad.rs`.
        space_denial_spies_from_rocketry => enable_space_denial_spies_from_rocketry, disable_space_denial_spies_from_rocketry;
        /// `BasicAi::opening_defense_preempts_once`; opt-in gene
        /// `opening-defense-preempts-once`.
        base.opening_defense_preempts_once => enable_opening_defense_preempts_once, disable_opening_defense_preempts_once;
        /// Opt-in gene `first-luxury-first-2`; see `advanced/first_luxury.rs`.
        first_luxury_first_2 => enable_first_luxury_first_2, disable_first_luxury_first_2;
        /// Opt-in gene `theology-fights-only-to-win`; see
        /// `advanced/theology_fights.rs`.
        theology_fights_only_to_win => enable_theology_fights_only_to_win, disable_theology_fights_only_to_win;
        /// Opt-in gene `founder-holds-the-holy-city`; see
        /// `advanced/holy_city_guard.rs`.
        founder_holds_the_holy_city => enable_founder_holds_the_holy_city, disable_founder_holds_the_holy_city;
        /// Opt-in gene `founder-converts-its-own-cities`; see
        /// [`Self::founder_converts_its_own_cities`].
        founder_converts_its_own_cities => enable_founder_converts_its_own_cities, disable_founder_converts_its_own_cities;
        /// Opt-in gene `spreader-steps-out-of-the-apostle`; see
        /// `advanced/apostle_evasion.rs`.
        spreader_steps_out_of_the_apostle => enable_spreader_steps_out_of_the_apostle, disable_spreader_steps_out_of_the_apostle;
        /// Opt-in gene `crushing-steady-passes-the-edge`; see
        /// `DeclarationEdge::crushing_steady`.
        crushing_steady_passes_the_edge => enable_crushing_steady_passes_the_edge, disable_crushing_steady_passes_the_edge;
        /// Opt-in gene `settler-marches-alone-when-clear`; see
        /// `AdvancedAi::settler_path_clear`.
        settler_marches_alone_when_clear => enable_settler_marches_alone_when_clear, disable_settler_marches_alone_when_clear;
        /// Opt-in gene `aqueduct-claims-the-capped-queue`; see
        /// `advanced/aqueduct_claims_the_capped_queue.rs`.
        aqueduct_claims_the_capped_queue => enable_aqueduct_claims_the_capped_queue, disable_aqueduct_claims_the_capped_queue;
        /// Opt-in gene `engineering-for-capped-fresh-water`; see
        /// `advanced/engineering_for_capped_fresh_water.rs`.
        engineering_for_capped_fresh_water => enable_engineering_for_capped_fresh_water, disable_engineering_for_capped_fresh_water;
        /// Finish a paused district foundation the host quotes at two turns or
        /// less while the city is at full health with no attack in reach. Opt-in
        /// gene `paused-district-finishes`; see
        /// `AdvancedAi::paused_district_finish_item`.
        paused_district_finishes => enable_paused_district_finishes, disable_paused_district_finishes;
        /// Once Apprenticeship is known, the producer step's Industrial Zone (or
        /// the zone's Workshop) claims its city's queue over a deferrable head.
        /// Opt-in gene `industrial-zone-claims-the-producer-queue`; see
        /// `AdvancedAi::claim_producer_zone`.
        industrial_zone_claims_the_producer_queue => enable_industrial_zone_claims_the_producer_queue, disable_industrial_zone_claims_the_producer_queue;
        /// Opt-in Science University priority before industrial claims.
        universities_before_the_forge => enable_universities_before_the_forge, disable_universities_before_the_forge;
        /// Move a one-war front cut off from every land road to an enemy the
        /// army can walk to. Opt-in gene `sea-locked-front-moves-on`; see
        /// `advanced/sea_locked_front.rs`.
        sea_locked_front_moves_on => enable_sea_locked_front_moves_on, disable_sea_locked_front_moves_on;
        /// Opt-in gene `siege-tier-before-the-beeline`; see
        /// `advanced/siege_tier_before_the_beeline.rs`.
        siege_tier_before_the_beeline => enable_siege_tier_before_the_beeline, disable_siege_tier_before_the_beeline;
        /// A standing Campus's next building (Library, else University, else
        /// Research Lab) claims its city's queue over a deferrable head. Opt-in
        /// gene `campus-building-claims-the-queue`; see
        /// `AdvancedAi::claim_campus_building`.
        campus_building_claims_the_queue => enable_campus_building_claims_the_queue, disable_campus_building_claims_the_queue;
        /// A weak third party whose closed borders hold a siege's march three
        /// turns is declared on. Opt-in gene `closed-border-blocker-declares`;
        /// see [`Self::closed_border_front`].
        closed_border_blocker_declares => enable_closed_border_blocker_declares, disable_closed_border_blocker_declares;
        /// Opt-in gene `growth-prices-the-farm-2`; see
        /// `AdvancedAi::growth_prices_the_farm_2`.
        growth_prices_the_farm_2 => enable_growth_prices_the_farm_2, disable_growth_prices_the_farm_2;
        /// Once Industrialization is known, the zone city whose Factory reaches
        /// the most new cities (then, with Coal, the Factory city whose plant
        /// powers the most Factories) claims its queue. Opt-in gene
        /// `industrial-zone-claims-the-factory`; see `AdvancedAi::claim_factory`.
        industrial_zone_claims_the_factory => enable_industrial_zone_claims_the_factory, disable_industrial_zone_claims_the_factory;
        /// A march whose dry road exists but cannot start this turn waits
        /// without reading as a water hold. Opt-in gene
        /// `dry-road-outlasts-a-blocked-step`; see
        /// `AdvancedAi::dry_road_blocked_holds`.
        dry_road_outlasts_a_blocked_step => enable_dry_road_outlasts_a_blocked_step, disable_dry_road_outlasts_a_blocked_step;
        /// Opt-in gene `luxury-swap-asks-2`; see
        /// `AdvancedAi::luxury_swap_asks_2`.
        luxury_swap_asks_2 => enable_luxury_swap_asks_2, disable_luxury_swap_asks_2;
        /// Opt-in gene `purchases-keep-the-upkeep-reserve`; see
        /// `AdvancedAi::purchases_keep_the_upkeep_reserve`.
        purchases_keep_the_upkeep_reserve => enable_purchases_keep_the_upkeep_reserve, disable_purchases_keep_the_upkeep_reserve;
        /// Opt-in gene `unwalled-city-spares-the-train`; see
        /// `AdvancedAi::unwalled_city_spares_the_train`.
        unwalled_city_spares_the_train => enable_unwalled_city_spares_the_train, disable_unwalled_city_spares_the_train;
        /// Opt-in gene `siege-budget-projects-the-seal`; see
        /// `AdvancedAi::siege_budget_projects_the_seal`.
        siege_budget_projects_the_seal => enable_siege_budget_projects_the_seal, disable_siege_budget_projects_the_seal;
        /// Opt-in gene `elimination-yields-to-a-launch`; see
        /// `AdvancedAi::elimination_yields_to_a_launch`.
        elimination_yields_to_a_launch => enable_elimination_yields_to_a_launch, disable_elimination_yields_to_a_launch;
        /// Opt-in gene `builders-route-around-mountains`; see
        /// `AdvancedAi::builders_route_around_mountains`.
        builders_route_around_mountains => enable_builders_route_around_mountains, disable_builders_route_around_mountains;
        /// Opt-in gene `war-target-prices-the-walls`; see
        /// `AdvancedAi::war_target_prices_the_walls`.
        war_target_prices_the_walls => enable_war_target_prices_the_walls, disable_war_target_prices_the_walls;
        /// Opt-in gene `embassy-finds-the-capital`; see
        /// `AdvancedAi::embassy_finds_the_capital`.
        embassy_finds_the_capital => enable_embassy_finds_the_capital, disable_embassy_finds_the_capital;
        /// Opt-in gene `embassy-finds-the-capital-2`; see
        /// `advanced/embassy_capital.rs`.
        embassy_finds_the_capital_2 => enable_embassy_finds_the_capital_2, disable_embassy_finds_the_capital_2;
        /// Opt-in gene `aluminum-buys-the-tile`; see
        /// `AdvancedAi::aluminum_buys_the_tile`.
        aluminum_buys_the_tile => enable_aluminum_buys_the_tile, disable_aluminum_buys_the_tile;
        /// Opt-in gene `aluminum-ask-tops-up-gold`; see
        /// `AdvancedAi::aluminum_ask_tops_up_gold`.
        aluminum_ask_tops_up_gold => enable_aluminum_ask_tops_up_gold, disable_aluminum_ask_tops_up_gold;
        /// Opt-in gene `racer-keeps-the-spies`; see
        /// `AdvancedAi::racer_keeps_the_spies`.
        racer_keeps_the_spies => enable_racer_keeps_the_spies, disable_racer_keeps_the_spies;
        /// Opt-in gene `capital-walls-before-the-surprise`; see
        /// `advanced/capital_walls_before_the_surprise.rs`.
        capital_walls_before_the_surprise => enable_capital_walls_before_the_surprise, disable_capital_walls_before_the_surprise;
        /// Opt-in gene `gun-queue-follows-the-road`; see
        /// [`Self::gun_queue_follows_the_road`].
        gun_queue_follows_the_road => enable_gun_queue_follows_the_road, disable_gun_queue_follows_the_road;
        /// Opt-in gene `phantom-capture-keeps-the-siege`; see
        /// `advanced/phantom_capture.rs`.
        phantom_capture_keeps_the_siege => enable_phantom_capture_keeps_the_siege, disable_phantom_capture_keeps_the_siege;
        /// Opt-in gene `holdout-keeps-the-faith-war`; see
        /// `advanced/holdout_faith_war.rs`.
        holdout_keeps_the_faith_war => enable_holdout_keeps_the_faith_war, disable_holdout_keeps_the_faith_war;
        /// Opt-in gene `holdout-keeps-the-faith-war-2`; see
        /// `advanced/holdout_faith_war.rs`.
        holdout_keeps_the_faith_war_2 => enable_holdout_keeps_the_faith_war_2, disable_holdout_keeps_the_faith_war_2;
        /// Opt-in gene `launched-racer-yields-the-capital`; see
        /// `advanced/launched_racer.rs`.
        launched_racer_yields_the_capital => enable_launched_racer_yields_the_capital, disable_launched_racer_yields_the_capital;
        /// Opt-in gene `racer-blocker-declares`; see `advanced/silent_counter.rs`.
        racer_blocker_declares => enable_racer_blocker_declares, disable_racer_blocker_declares;
        /// Opt-in gene `astrology-before-the-second-city`; see
        /// `advanced/astrology_before_the_second_city.rs`.
        astrology_before_the_second_city => enable_astrology_before_the_second_city, disable_astrology_before_the_second_city;
        /// Opt-in gene `prophet-race-takes-divine-spark`; see
        /// `advanced/astrology_before_the_second_city.rs`.
        prophet_race_takes_divine_spark => enable_prophet_race_takes_divine_spark, disable_prophet_race_takes_divine_spark;
        /// Opt-in gene `city-under-fire-ranks-first`; see
        /// `advanced/objective_board.rs`.
        city_under_fire_ranks_first => enable_city_under_fire_ranks_first, disable_city_under_fire_ranks_first;
        /// Opt-in gene `faith-war-before-the-flip`; see
        /// `advanced/faith_war_before_the_flip.rs`.
        faith_war_before_the_flip => enable_faith_war_before_the_flip, disable_faith_war_before_the_flip;
        /// Opt-in gene `open-city-draws-the-front`; see
        /// `advanced/open_city_front.rs`.
        open_city_draws_the_front => enable_open_city_draws_the_front, disable_open_city_draws_the_front;
        /// Opt-in gene `faith-war-before-the-flip-2`; see
        /// `advanced/faith_war_before_the_flip.rs`.
        faith_war_before_the_flip_2 => enable_faith_war_before_the_flip_2, disable_faith_war_before_the_flip_2;
        /// Opt-in gene `medina-quarter-houses-the-districts`; see
        /// `advanced/medina_quarter.rs`.
        medina_quarter_houses_the_districts => enable_medina_quarter_houses_the_districts, disable_medina_quarter_houses_the_districts;
        /// Opt-in gene `solvent-or-hold`; see `advanced/solvency.rs`.
        solvent_or_hold => enable_solvent_or_hold, disable_solvent_or_hold;
        /// Opt-in gene `bankruptcy-exits-the-drain`; see `advanced/solvency.rs`.
        bankruptcy_exits_the_drain => enable_bankruptcy_exits_the_drain, disable_bankruptcy_exits_the_drain;
        /// Opt-in gene `neighborhood-houses-the-capped-city`; see
        /// `advanced/neighborhood_houses_the_capped_city.rs`.
        neighborhood_houses_the_capped_city => enable_neighborhood_houses_the_capped_city, disable_neighborhood_houses_the_capped_city;
        /// Prepare a watched frontier city's first fortifications before war.
        frontier_walls_before_the_surprise => enable_frontier_walls_before_the_surprise, disable_frontier_walls_before_the_surprise;
        /// Prefer the wall branch for siege guns in a walled major-war campaign.
        siege_promotions_open_the_walls => enable_siege_promotions_open_the_walls, disable_siege_promotions_open_the_walls;
        /// Price a train-held Ram or Tower in the siege damage budget. See
        /// [`Self::siege_budget_counts_the_ram`].
        siege_budget_counts_the_ram => enable_siege_budget_counts_the_ram, disable_siege_budget_counts_the_ram;
        /// Claim a free district slot for a first Campus. See
        /// [`Self::campus_claims_the_free_slot`].
        campus_claims_the_free_slot => enable_campus_claims_the_free_slot, disable_campus_claims_the_free_slot;
        /// Version 2 of the free-slot Campus claim. See
        /// [`Self::campus_claims_the_free_slot_2`].
        campus_claims_the_free_slot_2 => enable_campus_claims_the_free_slot_2, disable_campus_claims_the_free_slot_2;
        /// Buy the wall-breaker a held siege waits for. See
        /// [`Self::breaker_bought_for_the_siege`].
        breaker_bought_for_the_siege => enable_breaker_bought_for_the_siege, disable_breaker_bought_for_the_siege;
        /// Buy an empty route slot's Trader with the idle bank. See
        /// [`Self::gold_buys_the_idle_trader`].
        gold_buys_the_idle_trader => enable_gold_buys_the_idle_trader, disable_gold_buys_the_idle_trader;
        /// Buy the threatened city's defender while its menu sells one. See
        /// [`Self::threatened_city_buys_its_defender`].
        threatened_city_buys_its_defender => enable_threatened_city_buys_its_defender, disable_threatened_city_buys_its_defender;
        /// Grow the Bomber wing past four while the Aluminum sustains it. See
        /// [`Self::air_wing_grows_with_the_aluminum`].
        air_wing_grows_with_the_aluminum => enable_air_wing_grows_with_the_aluminum, disable_air_wing_grows_with_the_aluminum;
        /// Lead the running wars with the rival the counter names. See
        /// [`Self::denial_target_leads_the_fronts`].
        denial_target_leads_the_fronts => enable_denial_target_leads_the_fronts, disable_denial_target_leads_the_fronts;
        /// Let a friendship lapse once the friend is racing to win. See
        /// [`Self::friendship_ends_at_the_race`].
        friendship_ends_at_the_race => enable_friendship_ends_at_the_race, disable_friendship_ends_at_the_race;
        /// Offer a friendship only to a rival at least as strong as us. See
        /// [`Self::friendship_only_with_the_stronger`].
        friendship_only_with_the_stronger => enable_friendship_only_with_the_stronger, disable_friendship_only_with_the_stronger;
        /// Aim a Domination conquest at a foothold within Bomber range of a rival
        /// capital. See [`Self::foothold_faces_the_capital`].
        foothold_faces_the_capital => enable_foothold_faces_the_capital, disable_foothold_faces_the_capital;
        /// Send a diplomatic mission to every rival we have met. See
        /// [`Self::delegations_reach_every_rival`].
        delegations_reach_every_rival => enable_delegations_reach_every_rival, disable_delegations_reach_every_rival;
        /// Keep a faith war from choosing the campaign front. See
        /// [`Self::faith_war_is_no_front`].
        faith_war_is_no_front => enable_faith_war_is_no_front, disable_faith_war_is_no_front;
        /// Version 2 of `faith-war-is-no-front`. See
        /// [`Self::faith_war_is_no_front_2`].
        faith_war_is_no_front_2 => enable_faith_war_is_no_front_2, disable_faith_war_is_no_front_2;
        /// Offer no friendship to the campaign's target. See
        /// [`Self::friendship_spares_the_target`].
        friendship_spares_the_target => enable_friendship_spares_the_target, disable_friendship_spares_the_target;
        /// Let a mustered siege count the guns it mustered when it stages. See
        /// [`Self::stage_reads_the_muster_breakers`].
        stage_reads_the_muster_breakers => enable_stage_reads_the_muster_breakers, disable_stage_reads_the_muster_breakers;
        /// Step melee onto the open ring of an unwalled city. See
        /// [`Self::unwalled_ring_steps_in`].
        unwalled_ring_steps_in => enable_unwalled_ring_steps_in, disable_unwalled_ring_steps_in;
        /// Offer no friendship to a rival weak enough to be prey. See
        /// [`Self::friendship_spares_the_prey`].
        friendship_spares_the_prey => enable_friendship_spares_the_prey, disable_friendship_spares_the_prey;
        /// Unlock the Campus ahead of the luxury and Astrology detours. See
        /// [`Self::campus_unlock_before_the_luxury`].
        campus_unlock_before_the_luxury => enable_campus_unlock_before_the_luxury, disable_campus_unlock_before_the_luxury;
        /// Declare on prey without the staged ring or a home Defend row. See
        /// [`Self::prey_is_declared_on`].
        prey_is_declared_on => enable_prey_is_declared_on, disable_prey_is_declared_on;
        /// Buy the threatened city a defender of its era, not the cheapest
        /// body. See [`Self::threatened_city_buys_its_defender_2`].
        threatened_city_buys_its_defender_2 => enable_threatened_city_buys_its_defender_2, disable_threatened_city_buys_its_defender_2;
        /// Keep a city-state war from becoming the Domination front. See
        /// [`Self::city_state_war_is_no_front`].
        city_state_war_is_no_front => enable_city_state_war_is_no_front, disable_city_state_war_is_no_front;
        /// Opt-in gene `staging-gun-trusts-its-escort-2`; see
        /// [`Self::staging_gun_trusts_its_escort_2`].
        staging_gun_trusts_its_escort_2 => enable_staging_gun_trusts_its_escort_2, disable_staging_gun_trusts_its_escort_2;
        /// Opt-in gene `prey-war-is-kept`; see [`Self::prey_war_is_kept`].
        prey_war_is_kept => enable_prey_war_is_kept, disable_prey_war_is_kept;
        /// Opt-in gene `defenders-hold-the-walls`; see
        /// [`Self::defenders_hold_the_walls`].
        defenders_hold_the_walls => enable_defenders_hold_the_walls, disable_defenders_hold_the_walls;
        /// Opt-in gene `war-faith-buys-the-army`; see
        /// [`Self::war_faith_buys_the_army`].
        war_faith_buys_the_army => enable_war_faith_buys_the_army, disable_war_faith_buys_the_army;
        /// Opt-in gene `faith-war-needs-the-edge`; see
        /// [`Self::faith_war_needs_the_edge`].
        faith_war_needs_the_edge => enable_faith_war_needs_the_edge, disable_faith_war_needs_the_edge;
        /// Opt-in `wing-capital-spotting`; see `advanced/wing_capital_spotting.rs`.
        wing_capital_spotting => enable_wing_capital_spotting, disable_wing_capital_spotting;
        /// Opt-in gene `the-giant-gets-the-peace-offer`; see
        /// [`Self::the_giant_gets_the_peace_offer`].
        the_giant_gets_the_peace_offer => enable_the_giant_gets_the_peace_offer, disable_the_giant_gets_the_peace_offer;
        /// Opt-in gene `the-last-two-capitals-come-first`; see
        /// [`Self::the_last_two_capitals_come_first`].
        the_last_two_capitals_come_first => enable_the_last_two_capitals_come_first, disable_the_last_two_capitals_come_first;
        /// Opt-in gene `launcher-war-skips-the-reserve`; see
        /// [`Self::launcher_war_skips_the_reserve`].
        launcher_war_skips_the_reserve => enable_launcher_war_skips_the_reserve, disable_launcher_war_skips_the_reserve;
        /// Opt-in gene `aluminum-tile-spends-the-war-chest`; see
        /// [`Self::aluminum_tile_spends_the_war_chest`].
        aluminum_tile_spends_the_war_chest => enable_aluminum_tile_spends_the_war_chest, disable_aluminum_tile_spends_the_war_chest;
        /// Offer no friendship to the founder whose faith we must fight. See
        /// [`Self::friendship_spares_the_converter`].
        friendship_spares_the_converter => enable_friendship_spares_the_converter, disable_friendship_spares_the_converter;
        /// Opt-in gene `siege-tier-yields-to-the-bombers-2`; see
        /// [`Self::siege_tier_yields_to_the_bombers_2`].
        siege_tier_yields_to_the_bombers_2 => enable_siege_tier_yields_to_the_bombers_2, disable_siege_tier_yields_to_the_bombers_2;
        /// Let a Spy in the space racer's city disrupt before it trains on Gain
        /// Sources. See [`Self::disrupt_before_the_sources`].
        disrupt_before_the_sources => enable_disrupt_before_the_sources, disable_disrupt_before_the_sources;
        /// Opt-in gene `disrupt-waits-for-the-match-point`; see
        /// `advanced/disrupt_waits_for_the_match_point.rs`.
        disrupt_waits_for_the_match_point => enable_disrupt_waits_for_the_match_point, disable_disrupt_waits_for_the_match_point;
        /// Opt-in gene `siege-moves-under-the-wing`; see
        /// [`Self::siege_moves_under_the_wing`].
        siege_moves_under_the_wing => enable_siege_moves_under_the_wing, disable_siege_moves_under_the_wing;
        /// Opt-in gene `unchosen-war-takes-the-first-peace`: see
        /// [`Self::unchosen_war_takes_the_first_peace`].
        unchosen_war_takes_the_first_peace => enable_unchosen_war_takes_the_first_peace, disable_unchosen_war_takes_the_first_peace;
        /// Opt-in gene `prey-waits-for-the-march`; see
        /// [`Self::prey_waits_for_the_march`].
        prey_waits_for_the_march => enable_prey_waits_for_the_march, disable_prey_waits_for_the_march;
        /// Opt-in gene `prey-waits-for-the-march-2`; see
        /// [`Self::prey_waits_for_the_march_2`].
        prey_waits_for_the_march_2 => enable_prey_waits_for_the_march_2, disable_prey_waits_for_the_march_2;
        /// Opt-in gene `upkeep-waits-for-the-income`; see
        /// [`Self::upkeep_waits_for_the_income`].
        upkeep_waits_for_the_income => enable_upkeep_waits_for_the_income, disable_upkeep_waits_for_the_income;
        /// Opt-in gene `food-focus-grows-the-small-city`; see
        /// [`Self::food_focus_grows_the_small_city`].
        food_focus_grows_the_small_city => enable_food_focus_grows_the_small_city, disable_food_focus_grows_the_small_city;
        /// Opt-in gene `capital-monument-after-the-second-city`; see
        /// [`Self::capital_monument_after_the_second_city`].
        capital_monument_after_the_second_city => enable_capital_monument_after_the_second_city, disable_capital_monument_after_the_second_city;
        /// `diplo-seat-wonder-claim`.
        diplo_seat_wonder_claim => enable_diplo_seat_wonder_claim, disable_diplo_seat_wonder_claim;
        /// `science-drive-answers-the-launch`.
        science_drive_answers_the_launch => enable_science_drive_answers_the_launch, disable_science_drive_answers_the_launch;
        /// `inquisition-defends-the-diplomat`.
        inquisition_defends_the_diplomat => enable_inquisition_defends_the_diplomat, disable_inquisition_defends_the_diplomat;
        /// Opt-in gene `culture-seat-monuments`; see
        /// [`Self::culture_seat_monuments`].
        culture_seat_monuments => enable_culture_seat_monuments, disable_culture_seat_monuments;
        /// `diplo-match-point-beelines-the-points`.
        diplo_match_point_beelines => enable_diplo_match_point_beelines_the_points, disable_diplo_match_point_beelines_the_points;
        /// `inquisition-answers-a-rival-majority`.
        inquisition_answers_a_rival_majority => enable_inquisition_answers_a_rival_majority, disable_inquisition_answers_a_rival_majority;
        /// Opt-in gene `diplo-seat-statue-harbor`; see
        /// [`Self::diplo_seat_statue_harbor`].
        diplo_seat_statue_harbor => enable_diplo_seat_statue_harbor, disable_diplo_seat_statue_harbor;
        /// Opt-in gene `hidden-border-yields-to-our-pressure`; see
        /// [`Self::hidden_border_yields_to_our_pressure`].
        hidden_border_yields_to_our_pressure => enable_hidden_border_yields_to_our_pressure, disable_hidden_border_yields_to_our_pressure;
        /// `diplomat-saves-for-the-apostle`.
        diplomat_saves_for_the_apostle => enable_diplomat_saves_for_the_apostle, disable_diplomat_saves_for_the_apostle;
        /// Turn on `culture-routes-reach-every-rival`. See
        /// [`Self::culture_routes_reach_every_rival`].
        culture_routes_reach_every_rival => enable_culture_routes_reach_every_rival, disable_culture_routes_reach_every_rival;
        /// Turn on `trade-quotes-pay-the-lump`. See
        /// [`Self::trade_quotes_pay_the_lump`].
        trade_quotes_pay_the_lump => enable_trade_quotes_pay_the_lump, disable_trade_quotes_pay_the_lump;
        /// Opt-in gene `counterweight-defends-the-diplomat`; see
        /// [`Self::counterweight_defends_the_diplomat`].
        counterweight_defends_the_diplomat => enable_counterweight_defends_the_diplomat, disable_counterweight_defends_the_diplomat;
        /// Opt-in gene `faith-war-defends-the-diplomat`; see
        /// [`Self::faith_war_defends_the_diplomat`].
        faith_war_defends_the_diplomat => enable_faith_war_defends_the_diplomat, disable_faith_war_defends_the_diplomat;
        /// Opt-in gene `counterweight-builds-its-source`; see
        /// [`Self::counterweight_builds_its_source`].
        counterweight_builds_its_source => enable_counterweight_builds_its_source, disable_counterweight_builds_its_source;
        /// Opt-in gene `culture-recovery-spends-the-bank`; see
        /// [`Self::culture_recovery_spends_the_bank`].
        culture_recovery_spends_the_bank => enable_culture_recovery_spends_the_bank, disable_culture_recovery_spends_the_bank;
        /// Opt-in gene `racer-spies-search-for-the-pad`; see
        /// [`Self::racer_spies_search_for_the_pad`].
        racer_spies_search_for_the_pad => enable_racer_spies_search_for_the_pad, disable_racer_spies_search_for_the_pad;
        /// Opt-in gene `launcher-war-serves-every-lane`; see
        /// [`Self::launcher_war_serves_every_lane`].
        launcher_war_serves_every_lane => enable_launcher_war_serves_every_lane, disable_launcher_war_serves_every_lane;
        /// Opt-in gene `culture-seat-allies`; see [`Self::culture_seat_allies`].
        culture_seat_allies => enable_culture_seat_allies, disable_culture_seat_allies;
        /// Opt-in gene `surprise-watch-holds-its-ratio`; see
        /// [`Self::surprise_watch_holds_its_ratio`].
        surprise_watch_holds_its_ratio => enable_surprise_watch_holds_its_ratio, disable_surprise_watch_holds_its_ratio;
        /// Opt-in gene `settler-founds-under-raiders`; see
        /// [`Self::settler_founds_under_raiders`].
        settler_founds_under_raiders => enable_settler_founds_under_raiders, disable_settler_founds_under_raiders;
        /// While a living major is unmet or unlocated, or a city-state is unmet,
        /// the queued sea scout keeps its queue, ships route toward foreign
        /// landfall, and the faster hull is preferred. Opt-in gene
        /// `contact-fleet`; see `advanced/contact_fleet.rs`.
        base.contact_fleet => enable_contact_fleet, disable_contact_fleet;
        /// Opt-in gene `immortal-skips-the-prophet-race`; see
        /// [`Self::immortal_skips_the_prophet_race`].
        immortal_skips_the_prophet_race => enable_immortal_skips_the_prophet_race, disable_immortal_skips_the_prophet_race;
        /// Opt-in gene `religion-lane-spreads-from-founding`; see
        /// [`Self::religion_lane_spreads_from_founding`].
        religion_lane_spreads_from_founding => enable_religion_lane_spreads_from_founding, disable_religion_lane_spreads_from_founding;
        /// Opt-in `religion-finishes-majorities`; see `religion_majorities.rs`.
        religion_finishes_majorities => enable_religion_finishes_majorities, disable_religion_finishes_majorities;
        /// Opt-in gene `board-ignores-the-faith-war`; see
        /// [`Self::board_ignores_the_faith_war`].
        board_ignores_the_faith_war => enable_board_ignores_the_faith_war, disable_board_ignores_the_faith_war;
        /// Opt-in gene `naval-crossing-keeps-going`; see
        /// [`Self::naval_crossing_keeps_going`].
        naval_crossing_keeps_going => enable_naval_crossing_keeps_going, disable_naval_crossing_keeps_going;
        /// Opt-in gene `builder-prices-travel-in-turns`; see
        /// [`Self::builder_prices_travel_in_turns`].
        builder_prices_travel_in_turns => enable_builder_prices_travel_in_turns, disable_builder_prices_travel_in_turns;
        /// Opt-in gene `religion-lane-hands-over`; see
        /// [`Self::religion_lane_hands_over`].
        religion_lane_hands_over => enable_religion_lane_hands_over, disable_religion_lane_hands_over;
        /// Opt-in gene `early-route-feeds-production`; see
        /// [`Self::early_route_feeds_production`].
        early_route_feeds_production => enable_early_route_feeds_production, disable_early_route_feeds_production;
        /// Opt-in gene `religion-race-keeps-expanding`; see
        /// [`Self::religion_race_keeps_expanding`].
        religion_race_keeps_expanding => enable_religion_race_keeps_expanding, disable_religion_race_keeps_expanding;
        /// Opt-in gene `domination-targets-land-first`; see
        /// [`Self::domination_targets_land_first`].
        domination_targets_land_first => enable_domination_targets_land_first, disable_domination_targets_land_first;
        /// Opt-in gene `stalled-spreader-braves-the-march`; see
        /// [`Self::stalled_spreader_braves_the_march`].
        stalled_spreader_braves_the_march => enable_stalled_spreader_braves_the_march, disable_stalled_spreader_braves_the_march;
        /// Opt-in gene `anvil-garrisons-its-melee`; see
        /// [`Self::anvil_garrisons_its_melee`].
        anvil_garrisons_its_melee => enable_anvil_garrisons_its_melee, disable_anvil_garrisons_its_melee;
        /// Opt-in gene `defence-holds-under-fire`; see
        /// [`Self::defence_holds_under_fire`].
        defence_holds_under_fire => enable_defence_holds_under_fire, disable_defence_holds_under_fire;
        /// Opt-in gene `land-grab-outlasts-the-opening`; see
        /// [`Self::land_grab_outlasts_the_opening`].
        land_grab_outlasts_the_opening => enable_land_grab_outlasts_the_opening, disable_land_grab_outlasts_the_opening;
        /// Opt-in gene `science-seat-allies`; see [`Self::science_seat_allies`].
        science_seat_allies => enable_science_seat_allies, disable_science_seat_allies;
        /// Opt-in gene `science-seat-allies-2`; see
        /// `advanced/culture_seat_allies.rs`.
        science_seat_allies_2 => enable_science_seat_allies_2, disable_science_seat_allies_2;
        /// Opt-in gene `science-seat-builds-the-statue`; see
        /// `advanced/diplo_statue_harbor.rs`.
        science_seat_builds_the_statue => enable_science_seat_builds_the_statue,
            disable_science_seat_builds_the_statue;
        /// Opt-in gene `science-seat-races-the-congress`: a live-only ballot
        /// change; see `CivvisCongressRacer` in `CivvisControlAgent.lua`.
        science_seat_races_the_congress => enable_science_seat_races_the_congress,
            disable_science_seat_races_the_congress;
        /// Opt-in gene `capital-walls-at-any-ratio`; see
        /// [`Self::capital_walls_at_any_ratio`].
        capital_walls_at_any_ratio => enable_capital_walls_at_any_ratio, disable_capital_walls_at_any_ratio;
        /// Opt-in gene `religion-seat-defends-its-faith`; see
        /// [`Self::religion_seat_defends_its_faith`].
        religion_seat_defends_its_faith => enable_religion_seat_defends_its_faith, disable_religion_seat_defends_its_faith;
        /// Opt-in gene `religion-seat-founds-first`; see
        /// [`Self::religion_seat_founds_first`].
        religion_seat_founds_first => enable_religion_seat_founds_first, disable_religion_seat_founds_first;
        /// Opt-in gene `science-denounce-names-the-leader`; see
        /// [`Self::science_denounce_names_the_leader`].
        science_denounce_names_the_leader => enable_science_denounce_names_the_leader, disable_science_denounce_names_the_leader;
        /// Opt-in gene `friendship-keeps-the-peace`; see
        /// [`Self::friendship_keeps_the_peace`].
        friendship_keeps_the_peace => enable_friendship_keeps_the_peace, disable_friendship_keeps_the_peace;
        /// Opt-in gene `pillaged-districts-repair-first`; see
        /// [`Self::pillaged_districts_repair_first`].
        pillaged_districts_repair_first => enable_pillaged_districts_repair_first, disable_pillaged_districts_repair_first;
        /// Opt-in gene `launcher-war-fights-the-faster-racer`; see
        /// [`Self::launcher_war_fights_the_faster_racer`].
        launcher_war_fights_the_faster_racer => enable_launcher_war_fights_the_faster_racer, disable_launcher_war_fights_the_faster_racer;
        /// Opt-in gene `apostle-transit-keeps-the-cover`; see
        /// [`Self::apostle_transit_keeps_the_cover`].
        apostle_transit_keeps_the_cover => enable_apostle_transit_keeps_the_cover, disable_apostle_transit_keeps_the_cover;
        /// Opt-in gene `fog-yields-to-a-stranded-settler`; see
        /// [`Self::fog_yields_to_a_stranded_settler`].
        fog_yields_to_a_stranded_settler => enable_fog_yields_to_a_stranded_settler, disable_fog_yields_to_a_stranded_settler;
        /// Opt-in gene `science-drive-answers-from-the-pad`; see
        /// [`Self::science_drive_answers_from_the_pad`].
        science_drive_answers_from_the_pad => enable_science_drive_answers_from_the_pad, disable_science_drive_answers_from_the_pad;
        /// Turn on `culture-digs-one-museum`. See [`Self::culture_digs_one_museum`].
        culture_digs_one_museum => enable_culture_digs_one_museum, disable_culture_digs_one_museum;
        /// Opt-in gene `science-drive-answers-from-the-pad-2`; see
        /// [`Self::science_drive_answers_from_the_pad_2`].
        science_drive_answers_from_the_pad_2 => enable_science_drive_answers_from_the_pad_2, disable_science_drive_answers_from_the_pad_2;
        /// Opt-in gene `launch-pad-holds-its-queue`; see
        /// [`Self::launch_pad_holds_its_queue`].
        launch_pad_holds_its_queue => enable_launch_pad_holds_its_queue, disable_launch_pad_holds_its_queue;
        /// Opt-in gene `launch-commits-the-endgame`; see
        /// [`Self::launch_commits_the_endgame`].
        launch_commits_the_endgame => enable_launch_commits_the_endgame, disable_launch_commits_the_endgame;
        /// Opt-in gene `domination-joins-the-race`; see
        /// [`Self::domination_joins_the_race`].
        domination_joins_the_race => enable_domination_joins_the_race, disable_domination_joins_the_race;
        /// Opt-in gene `alliance-remembers-the-refusal`; see
        /// [`Self::alliance_remembers_the_refusal`].
        alliance_remembers_the_refusal => enable_alliance_remembers_the_refusal, disable_alliance_remembers_the_refusal;
        /// Opt-in gene `science-drive-leads-beside-the-lane`; see
        /// [`Self::science_drive_leads_beside_the_lane`].
        science_drive_leads_beside_the_lane => enable_science_drive_leads_beside_the_lane, disable_science_drive_leads_beside_the_lane;
        /// Opt-in gene `friendship-refuses-the-racer`; see
        /// [`Self::friendship_refuses_the_racer`].
        friendship_refuses_the_racer => enable_friendship_refuses_the_racer, disable_friendship_refuses_the_racer;
        /// Opt-in gene `deterrence-trusts-a-friend`; see
        /// [`Self::deterrence_trusts_a_friend`].
        deterrence_trusts_a_friend => enable_deterrence_trusts_a_friend, disable_deterrence_trusts_a_friend;
        /// Opt-in gene `campus-unlock-every-lane`; see
        /// [`Self::campus_unlock_every_lane`].
        campus_unlock_every_lane => enable_campus_unlock_every_lane, disable_campus_unlock_every_lane;
        /// Opt-in gene `gold-buys-the-opening-builder`; see
        /// [`Self::gold_buys_the_opening_builder`].
        gold_buys_the_opening_builder => enable_gold_buys_the_opening_builder, disable_gold_buys_the_opening_builder;
        /// Opt-in gene `satellite-holds-the-pad`; see
        /// [`Self::satellite_holds_the_pad`].
        satellite_holds_the_pad => enable_satellite_holds_the_pad, disable_satellite_holds_the_pad;
        /// Opt-in gene `stalled-conquest-takes-science`; see
        /// [`Self::stalled_conquest_takes_science`].
        stalled_conquest_takes_science => enable_stalled_conquest_takes_science, disable_stalled_conquest_takes_science;
        /// Keep a committed first Science launch on its research path.
        first_launch_keeps_research => enable_first_launch_keeps_research, disable_first_launch_keeps_research;
        /// Prioritize usable labs and nearby Rocketry during a Science posture.
        science_foundation_before_backlog => enable_science_foundation_before_backlog, disable_science_foundation_before_backlog;
        science_chain_crosses_window => enable_science_chain_crosses_window, disable_science_chain_crosses_window;
        science_campus_before_repeat_settler => enable_science_campus_before_repeat_settler, disable_science_campus_before_repeat_settler;
        /// Replace an obsolete native research path with a revealed launch milestone.
        revealed_launch_tech_takes_the_slot => enable_revealed_launch_tech_takes_the_slot, disable_revealed_launch_tech_takes_the_slot;
        /// Ask the native market for Coal serving ready research power.
        science_power_buys_fuel => enable_science_power_buys_fuel, disable_science_power_buys_fuel;
        /// Prefer a legal non-lowland site when starting a native Spaceport.
        spaceport_above_the_sea => enable_spaceport_above_the_sea, disable_spaceport_above_the_sea;
        /// Opt-in gene `old-techs-before-the-beeline`; see
        /// `advanced/old_techs_before_the_beeline.rs`.
        old_techs_before_the_beeline => enable_old_techs_before_the_beeline, disable_old_techs_before_the_beeline;
        /// Opt-in gene `saved-work-is-seen`; see `advanced/saved_work_is_seen.rs`.
        saved_work_is_seen => enable_saved_work_is_seen, disable_saved_work_is_seen;
        /// Opt-in gene `gold-buys-the-backlog-builder`; see
        /// `advanced/gold_buys_the_backlog_builder.rs`.
        gold_buys_the_backlog_builder => enable_gold_buys_the_backlog_builder, disable_gold_buys_the_backlog_builder;
        /// Opt-in gene `science-specializes-earlier`; see
        /// [`Self::science_specializes_earlier`].
        science_specializes_earlier => enable_science_specializes_earlier, disable_science_specializes_earlier;
        /// Opt-in Science Writing priority before optional research detours.
        science_writing_before_the_luxury => enable_science_writing_before_the_luxury, disable_science_writing_before_the_luxury;
        /// Opt-in gene `near-done-work-finishes`; see
        /// `advanced/near_done_work_finishes.rs`.
        near_done_work_finishes => enable_near_done_work_finishes, disable_near_done_work_finishes;
        /// Opt-in gene `founding-voyage`; see [`Self::founding_voyage`].
        founding_voyage => enable_founding_voyage, disable_founding_voyage;
        /// Opt-in gene `outclassed-siege-stands-down`; see
        /// [`Self::outclassed_siege_stands_down`].
        outclassed_siege_stands_down => enable_outclassed_siege_stands_down, disable_outclassed_siege_stands_down;
        /// Opt-in gene `ranged-finish-on-the-mean`; see
        /// [`Self::ranged_finish_on_the_mean`].
        ranged_finish_on_the_mean => enable_ranged_finish_on_the_mean, disable_ranged_finish_on_the_mean;
        /// Opt-in gene `generals-lead-the-army`; see
        /// [`Self::generals_lead_the_army`].
        generals_lead_the_army => enable_generals_lead_the_army, disable_generals_lead_the_army;
        /// Opt-in gene `capital-leaves-the-frost`; see
        /// [`Self::capital_leaves_the_frost`].
        capital_leaves_the_frost => enable_capital_leaves_the_frost, disable_capital_leaves_the_frost;
        /// Opt-in gene `gold-buys-the-production-tile`; see
        /// [`Self::gold_buys_the_production_tile`].
        gold_buys_the_production_tile => enable_gold_buys_the_production_tile, disable_gold_buys_the_production_tile;
        /// Prefer a timely anti-cavalry defender against a visible mounted threat.
        spears_answer_the_horse => enable_spears_answer_the_horse, disable_spears_answer_the_horse;
        /// Opt-in gene `slots-survive-the-roll`; see
        /// [`Self::slots_survive_the_roll`].
        slots_survive_the_roll => enable_slots_survive_the_roll, disable_slots_survive_the_roll;
        /// Scale the free-slot Campus time budget relative to Online costs.
        free_slot_campus_clock => enable_free_slot_campus_clock, disable_free_slot_campus_clock;
        /// Opt-in gene `camps-close-the-age`; see [`Self::camps_close_the_age`].
        camps_close_the_age => enable_camps_close_the_age, disable_camps_close_the_age;
        /// Opt-in gene `hint-runs-the-claims`; see
        /// [`Self::hint_runs_the_claims`].
        hint_runs_the_claims => enable_hint_runs_the_claims, disable_hint_runs_the_claims;
        /// Opt-in gene `luxury-asks-pay-in-strategics`; see
        /// `advanced/luxury_asks_pay_in_strategics.rs`.
        luxury_asks_pay_in_strategics => enable_luxury_asks_pay_in_strategics, disable_luxury_asks_pay_in_strategics;
        /// Opt-in gene `luxury-asks-pay-in-strategics-2`; see
        /// `advanced/luxury_asks_pay_in_strategics.rs`.
        luxury_asks_pay_in_strategics_2 => enable_luxury_asks_pay_in_strategics_2, disable_luxury_asks_pay_in_strategics_2;
        /// Opt-in gene `military-alliance-shield`; see
        /// `advanced/military_alliance_shield.rs`.
        military_alliance_shield => enable_military_alliance_shield, disable_military_alliance_shield;
        /// Price the Science lost when a new district crowds an owned Seowon.
        districts_protect_seowon_science => enable_districts_protect_seowon_science, disable_districts_protect_seowon_science;
        /// Opt-in gene `early-cities-keep-their-distance`; see
        /// [`Self::early_cities_keep_their_distance`].
        early_cities_keep_their_distance => enable_early_cities_keep_their_distance, disable_early_cities_keep_their_distance;
        /// Opt-in gene `denial-spies-count-their-losses`; see
        /// `advanced/denial_spies_count_their_losses.rs`.
        denial_spies_count_their_losses => enable_denial_spies_count_their_losses, disable_denial_spies_count_their_losses;
        /// Opt-in gene `spies-steal-the-lead`; see
        /// `advanced/spies_steal_the_lead.rs`.
        spies_steal_the_lead => enable_spies_steal_the_lead, disable_spies_steal_the_lead;
        /// Opt-in gene `spy-civics-come-first`; see
        /// `advanced/spy_civics_come_first.rs`.
        spy_civics_come_first => enable_spy_civics_come_first, disable_spy_civics_come_first;
        /// Opt-in gene `holdable-land-keeps-settling`; see
        /// `advanced/holdable_land_keeps_settling.rs`.
        holdable_land_keeps_settling => enable_holdable_land_keeps_settling, disable_holdable_land_keeps_settling;
        /// Opt-in gene `medina-yields-to-the-amenities`; see
        /// `advanced/medina_quarter.rs`.
        medina_yields_to_the_amenities => enable_medina_yields_to_the_amenities, disable_medina_yields_to_the_amenities;
        /// Opt-in gene `war-defense-spends-the-runway`; see
        /// `advanced/war_defense_spends_the_runway.rs`.
        war_defense_spends_the_runway => enable_war_defense_spends_the_runway, disable_war_defense_spends_the_runway;
        /// Opt-in gene `plaza-serves-the-scientist`; see
        /// `BasicAi::plaza_building_item_for`.
        plaza_serves_the_scientist => enable_plaza_serves_the_scientist, disable_plaza_serves_the_scientist;
        /// Opt-in gene `parity-upgrades-before-it-buys`; see
        /// `advanced/parity_upgrades_before_it_buys.rs`.
        parity_upgrades_before_it_buys => enable_parity_upgrades_before_it_buys, disable_parity_upgrades_before_it_buys;
        /// Opt-in gene `parity-upgrades-before-it-buys-2`; see
        /// `advanced/parity_upgrades_before_it_buys.rs`.
        parity_upgrades_before_it_buys_2 => enable_parity_upgrades_before_it_buys_2, disable_parity_upgrades_before_it_buys_2;
        /// Opt-in gene `parity-upgrades-before-it-buys-3`; see
        /// `advanced/parity_upgrades_before_it_buys.rs`.
        parity_upgrades_before_it_buys_3 => enable_parity_upgrades_before_it_buys_3, disable_parity_upgrades_before_it_buys_3;
        /// Opt-in gene `settler-shelters-from-the-war`; see
        /// `advanced/settler_shelters_from_the_war.rs`.
        settler_shelters_from_the_war => enable_settler_shelters_from_the_war, disable_settler_shelters_from_the_war;
        /// Opt-in gene `threatened-lane-lends-the-army`; see
        /// `advanced/threatened_lane_lends_the_army.rs`.
        threatened_lane_lends_the_army => enable_threatened_lane_lends_the_army, disable_threatened_lane_lends_the_army;
        /// Opt-in gene `prod-tiles-call-the-builders`; acts through
        /// `builders-cover-the-worked-backlog`'s step. See
        /// `ai/prod_tiles_call_the_builders.rs`.
        base.prod_tiles_call_the_builders => enable_prod_tiles_call_the_builders, disable_prod_tiles_call_the_builders;
        /// Opt-in gene `outmatched-war-prices-the-army`; see
        /// `advanced/outmatched_war_prices_the_army.rs`.
        outmatched_war_prices_the_army => enable_outmatched_war_prices_the_army, disable_outmatched_war_prices_the_army;
        /// Opt-in gene `threatened-war-holds-the-claims`; see
        /// `advanced/threatened_war_holds_the_claims.rs`.
        threatened_war_holds_the_claims => enable_threatened_war_holds_the_claims, disable_threatened_war_holds_the_claims;
        /// Opt-in gene `korea-mines-beside-the-seowon`; see
        /// `advanced/korea_mines_beside_the_seowon.rs`.
        korea_mines_beside_the_seowon => enable_korea_mines_beside_the_seowon, disable_korea_mines_beside_the_seowon;
        /// Opt-in gene `captives-come-home`; see
        /// `advanced/captives_come_home.rs`.
        captives_come_home => enable_captives_come_home, disable_captives_come_home;
        /// Opt-in gene `captives-come-home-2` (version 2, carrying version
        /// 1); see `advanced/captives_come_home.rs`.
        captives_come_home_2 => enable_captives_come_home_2, disable_captives_come_home_2;
        /// Opt-in gene `plaza-serves-the-scientist-2` (version 2 of
        /// `plaza-serves-the-scientist`, which it carries); see
        /// `advanced/plaza_serves_the_scientist_2.rs`.
        plaza_serves_the_scientist_2 => enable_plaza_serves_the_scientist_2, disable_plaza_serves_the_scientist_2;
        /// Opt-in gene `science-heads-keep-their-queue`; see
        /// `advanced/science_heads_keep_their_queue.rs`.
        science_heads_keep_their_queue => enable_science_heads_keep_their_queue, disable_science_heads_keep_their_queue;
        /// Opt-in gene `denounce-only-to-declare`; see
        /// `advanced/denounce_only_to_declare.rs`.
        denounce_only_to_declare => enable_denounce_only_to_declare, disable_denounce_only_to_declare;
        /// Opt-in gene `capture-scar-reads-the-new-city`; see
        /// `advanced/civilian_safety.rs` (`resolve_vanished_settlers`).
        capture_scar_reads_the_new_city => enable_capture_scar_reads_the_new_city, disable_capture_scar_reads_the_new_city;
        /// Opt-in gene `districts-price-the-improvement`; see
        /// [`Self::districts_price_the_improvement`].
        districts_price_the_improvement => enable_districts_price_the_improvement, disable_districts_price_the_improvement;
        /// Opt-in gene `tribute-buys-the-friendship`; see
        /// `advanced/tribute_buys_the_friendship.rs`.
        tribute_buys_the_friendship => enable_tribute_buys_the_friendship, disable_tribute_buys_the_friendship;
        /// Opt-in gene `world-religion-condemns-the-faith`; see
        /// `advanced/world_religion_condemns.rs`.
        world_religion_condemns_the_faith => enable_world_religion_condemns_the_faith, disable_world_religion_condemns_the_faith;
        /// Opt-in gene `government-reach-pays-the-switch`; see
        /// `advanced/government_reach.rs`.
        government_reach_pays_the_switch => enable_government_reach_pays_the_switch, disable_government_reach_pays_the_switch;
        /// Opt-in gene `plaza-serves-the-scientist-3` (version 3, carrying 1
        /// and 2); see `advanced/plaza_serves_the_scientist_2.rs`.
        plaza_serves_the_scientist_3 => enable_plaza_serves_the_scientist_3, disable_plaza_serves_the_scientist_3;
        /// Opt-in gene `space-people-call-the-projects`; see
        /// `advanced/space_people_call_the_projects.rs`.
        /// Opt-in gene `sea-scout-claims-the-queue-2` (version 2, carrying 1);
        /// see `advanced/contact_fleet.rs`.
        sea_scout_claims_the_queue_2 => enable_sea_scout_claims_the_queue_2, disable_sea_scout_claims_the_queue_2;
        space_people_call_the_projects => enable_space_people_call_the_projects, disable_space_people_call_the_projects;
        /// Prefer a safe legal site for a new native Science Campus.
        campus_above_the_sea => enable_campus_above_the_sea, disable_campus_above_the_sea;
        /// Opt-in gene `fresh-water-prices-the-site`; see
        /// `advanced/fresh_water_site.rs`.
        fresh_water_prices_the_site => enable_fresh_water_prices_the_site, disable_fresh_water_prices_the_site;
        /// Keep defensive formations near their assigned threatened city.
        defenders_keep_the_city_in_reach => enable_defenders_keep_the_city_in_reach, disable_defenders_keep_the_city_in_reach;
        /// Opt-in gene `plant-reads-the-grid`; see
        /// `advanced/plant_reads_the_grid.rs`.
        plant_reads_the_grid => enable_plant_reads_the_grid, disable_plant_reads_the_grid;
        /// Opt-in gene `sea-scout-claims-the-queue`; see
        /// `advanced/contact_fleet.rs`.
        sea_scout_claims_the_queue => enable_sea_scout_claims_the_queue, disable_sea_scout_claims_the_queue;
        /// Opt-in gene `wounded-ship-sails-on`; see
        /// `advanced/wounded_ship_sails_on.rs`.
        wounded_ship_sails_on => enable_wounded_ship_sails_on, disable_wounded_ship_sails_on;
        /// Opt-in gene `nusantara-settles-the-shore`; see
        /// `advanced/nusantara_settles_the_shore.rs`.
        nusantara_settles_the_shore => enable_nusantara_settles_the_shore, disable_nusantara_settles_the_shore;
        /// Rank Spy producers by native turns, sparing science on ties.
        spies_price_the_science_head => enable_spies_price_the_science_head, disable_spies_price_the_science_head;
    }
}

impl AdvancedAi {
    /// Pingala version two retains ordinary dividends and joins a committed
    /// launch producer. Independently off and unpriced; one family version plays.
    pub fn enable_pingala_follows_research_2(&mut self) {
        self.pingala_follows_research = false;
        self.pingala_follows_research_2 = true;
    }

    /// Withhold the independent Pingala launch-job version.
    pub fn disable_pingala_follows_research_2(&mut self) {
        self.pingala_follows_research_2 = false;
    }

    /// Promote an Apostle for the job the empire needs rather than the largest
    /// number on the card. Off in production; opted into by name (`victory_eval
    /// --with apostle-promotion-by-role`, `gene_screen`). See
    /// [`crate::ai::BasicAi::apostle_promotion_by_role`] for the units mismatch
    /// that makes the shipped ranking a constant.
    pub fn enable_apostle_promotion_by_role(&mut self) {
        self.base.apostle_promotion_by_role = true;
    }

    /// The twin of `enable_apostle_promotion_by_role`, so an arm that opted in
    /// can put it back.
    pub fn disable_apostle_promotion_by_role(&mut self) {
        self.base.apostle_promotion_by_role = false;
    }

    /// Adapt a live Civilization VI Trader, which cannot walk, to its distinct
    /// route-start action. Native tournament games leave this disabled.
    pub fn enable_live_trader_route_adapter(&mut self) {
        self.live_trader_route_adapter = true;
    }

    /// Withholding twin for `enable_live_trader_route_adapter`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_live_trader_route_adapter(&mut self) {
        self.live_trader_route_adapter = false;
    }

    /// Buy the second and third Scout early, before Early Empire closes borders
    /// and city-states become unreachable. Buy the second and third Scout while
    /// the world's borders are still open — after Early Empire a city-state
    /// cannot be met by land at all. See [`AdvancedAi::early_contact_window`].
    pub fn enable_early_contact_window(&mut self) {
        self.early_contact_window = true;
        self.early_contact_window_2 = false;
    }

    /// The twin of `enable_early_contact_window`.
    pub fn disable_early_contact_window(&mut self) {
        self.early_contact_window = false;
    }

    /// Spread early Scouts across nearby land frontiers until first major
    /// contact, retaining the original city-state production incentive.
    pub fn enable_early_contact_window_2(&mut self) {
        self.early_contact_window_2 = true;
        self.early_contact_window = false;
    }

    pub fn disable_early_contact_window_2(&mut self) {
        self.early_contact_window_2 = false;
    }

    /// Open a surprise war on a neighbour whose Settlers, Builders or tiles lie
    /// exposed nearby, then sue for peace. Open a surprise war on a neighbour
    /// whose unescorted Settlers, Builders or unpillaged tiles lie within a
    /// short march of our soldiers, take them, and sue for peace. See
    /// [`AdvancedAi::opportunistic_war`]. Production gene.
    pub fn enable_opportunistic_war(&mut self) {
        self.opportunistic_war = true;
        self.opportunistic_war_2 = false;
    }

    /// The twin of `enable_opportunistic_war`.
    pub fn disable_opportunistic_war(&mut self) {
        self.opportunistic_war = false;
        if !self.opportunistic_war_2 {
            self.raid_war = None;
        }
    }

    /// Price only prizes with a short route after declaration. Version two
    /// of `opportunistic-war`; selecting either version clears its sibling.
    pub fn enable_opportunistic_war_2(&mut self) {
        self.opportunistic_war_2 = true;
        self.opportunistic_war = false;
    }

    pub fn disable_opportunistic_war_2(&mut self) {
        self.opportunistic_war_2 = false;
        if !self.opportunistic_war {
            self.raid_war = None;
        }
    }

    /// Enforce Civilization VI's rule that a purchased religious unit inherits
    /// its city's majority religion. Native tournament games leave this
    /// disabled.
    pub fn enable_live_religious_purchase_guard(&mut self) {
        self.base.live_religious_purchase_guard = true;
    }

    /// Withholding twin for `enable_live_religious_purchase_guard`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_live_religious_purchase_guard(&mut self) {
        self.base.live_religious_purchase_guard = false;
    }
    /// Discount a Settler that has stopped walking from the expansion gate, and
    /// found where it stands when stalled. Native tournament games leave this
    /// disabled so their recorded ladders replay the historical controller move
    /// for move.
    pub fn enable_stranded_settler_discount(&mut self) {
        self.base.settler_strand_discount = true;
        // Discounting the stuck body lets another Settler enter the pipeline,
        // but does not convert the production already standing on legal, safe
        // ground. Once the bounded stall counter expires, finish that city
        // instead of beginning another target cycle.
        self.settler_founds_when_stalled = true;
    }

    /// Record each tactical step so a unit moved twice in one turn cannot
    /// return to the tile it just left. Native tournament games leave this
    /// disabled so their recorded ladders replay move-for-move.
    pub fn enable_recorded_tactical_step(&mut self) {
        self.base.recorded_tactical_step = true;
    }

    /// Count a unit's livelock history once per Civilization VI turn, not once
    /// per live replan frame. Keep that bridge-only accounting behind a
    /// separately withholdable treatment.
    pub fn enable_live_motion_turn_accounting(&mut self) {
        self.base.enable_live_motion_turn_accounting();
    }

    /// Withholding twin for `enable_live_motion_turn_accounting`.
    pub fn disable_live_motion_turn_accounting(&mut self) {
        self.base.disable_live_motion_turn_accounting();
    }

    /// Refuse any step onto a tile the unit already stood on this turn, closing
    /// three-hop loops too.
    ///
    /// `enable_recorded_tactical_step` remembers one tile, which refuses
    /// `A -> B -> A` and lets `A -> B -> C -> A` through. See
    /// `BasicAi::whole_turn_backtrack_guard` for what that costs.
    pub fn enable_whole_turn_backtrack_guard(&mut self) {
        self.base.whole_turn_backtrack_guard = true;
    }

    /// Withholding twin for `enable_whole_turn_backtrack_guard`. See
    /// `LIVE_TREATMENTS`.
    pub fn disable_whole_turn_backtrack_guard(&mut self) {
        self.base.whole_turn_backtrack_guard = false;
    }

    /// Withholding twin for `enable_recorded_tactical_step`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_recorded_tactical_step(&mut self) {
        self.base.recorded_tactical_step = false;
    }

    /// Let more than one Settler exist at a time, up to the shortfall against
    /// the city target. Set by the Civilization VI bridge only; native
    /// constructors and the frozen anchor keep the one-at-a-time gate in both
    /// settler routes.
    pub fn enable_parallel_settlers(&mut self) {
        self.parallel_settlers = true;
        self.base.parallel_settlers = true;
    }

    /// Withhold the live seat's second Settler pipeline so the evaluator can
    /// price it against the same otherwise-complete deployment bundle.
    pub fn disable_parallel_settlers(&mut self) {
        self.parallel_settlers = false;
        self.base.parallel_settlers = false;
    }

    /// Start a Settler at Civilization VI's own population floor of 2 instead
    /// of the genome's higher figure. Set by the Civilization VI bridge only;
    /// native constructors and the frozen anchor keep the genome's figure.
    pub fn enable_host_settler_pop(&mut self) {
        self.base.enable_host_settler_pop();
    }

    /// Withhold the host population floor from one live evaluator arm.
    pub fn disable_host_settler_pop(&mut self) {
        self.base.host_settler_pop = false;
    }

    /// Give up an exploration target the host accepts but never actually moves
    /// the unit toward. Set by the Civilization VI bridge only.
    pub fn enable_explore_dead_targets(&mut self) {
        self.base.enable_explore_dead_targets();
    }

    /// Withhold target retirement from one live evaluator arm.
    pub fn disable_explore_dead_targets(&mut self) {
        self.base.explore_dead_targets = false;
    }

    /// Hold an exploration goal until reached and sweep outward from home
    /// instead of pacing the same box. Set by the Civilization VI bridge only.
    pub fn enable_explore_commit(&mut self) {
        self.base.enable_explore_commit();
    }

    /// An amenity crisis repair is bought with Gold when the treasury covers
    /// it, so the Science plan's repeatable project keeps its queue; only a
    /// district or an unaffordable building still pauses the project.
    ///
    /// Version 2 of `amenity_project_preemption`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `amenity-project-preemption-2`. See `AdvancedAi::amenity_project_preemption_2`.
    pub fn enable_amenity_project_preemption_2(&mut self) {
        self.amenity_project_preemption = false;
        self.amenity_project_preemption_2 = true;
    }

    pub fn disable_amenity_project_preemption_2(&mut self) {
        self.amenity_project_preemption_2 = false;
    }

    /// A settle site with a plot in its first three rings that could host a
    /// Campus at raw Science adjacency 4 is worth 15% more, so the
    /// multiplier's threshold is bought where it is decided — at city siting
    /// — and not only priced at the district.
    ///
    /// Version 2 of `campus_adjacency_threshold`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `campus-adjacency-threshold-2`. See `AdvancedAi::campus_adjacency_threshold_2`.
    pub fn enable_campus_adjacency_threshold_2(&mut self) {
        self.campus_adjacency_threshold = false;
        self.campus_adjacency_threshold_2 = true;
    }

    pub fn disable_campus_adjacency_threshold_2(&mut self) {
        self.campus_adjacency_threshold_2 = false;
    }

    /// The district coverage term falls to a quarter of the bred weight for a
    /// family every city already holds, not half, pushing the lever further
    /// in the direction that paid score share.
    ///
    /// Version 2 of `district_coverage`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `district-coverage-2`. See `BasicAi::district_coverage_2`.
    pub fn enable_district_coverage_2(&mut self) {
        self.base.district_coverage = false;
        self.base.district_coverage_2 = true;
    }

    pub fn disable_district_coverage_2(&mut self) {
        self.base.district_coverage_2 = false;
    }

    /// A slipping city with no Holy Site claims one only when Gold can buy
    /// it; the district is never put at the front of the city's own queue.
    ///
    /// Version 2 of `holy_site_where_the_threat_is`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `holy-site-where-the-threat-is-2`. See `AdvancedAi::holy_site_where_the_threat_is_2`.
    pub fn enable_holy_site_where_the_threat_is_2(&mut self) {
        self.holy_site_where_the_threat_is = false;
        self.holy_site_where_the_threat_is_2 = true;
    }

    pub fn disable_holy_site_where_the_threat_is_2(&mut self) {
        self.holy_site_where_the_threat_is_2 = false;
    }

    /// Two peacetime naval eyes instead of one while unseen water remains, so
    /// the second coast is charted while the first Galley is still out.
    ///
    /// Version 2 of `naval_recon`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `naval-recon-2`. See `BasicAi::naval_recon_2`.
    pub fn enable_naval_recon_2(&mut self) {
        self.base.naval_recon = false;
        self.base.naval_recon_2 = true;
    }

    pub fn disable_naval_recon_2(&mut self) {
        self.base.naval_recon_2 = false;
    }

    /// Version 3 of `naval-recon`: let a simultaneously missing land scout
    /// take the idle queue before the one peacetime sea scout. One version per
    /// family is active in a screen.
    pub fn enable_naval_recon_3(&mut self) {
        self.base.naval_recon = false;
        self.base.naval_recon_2 = false;
        self.base.naval_recon_3 = true;
    }

    /// The twin of `enable_naval_recon_3`.
    pub fn disable_naval_recon_3(&mut self) {
        self.base.naval_recon_3 = false;
    }

    /// A building whose powered half would be switched on the day it stands —
    /// the city is powered and stays powered with the building's own demand —
    /// is priced with that half, so the Lab, Stock Exchange and Factory in
    /// already-powered cities stop being bought without it.
    ///
    /// Version 2 of `power_the_laboratory`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `power-the-laboratory-2`. See `AdvancedAi::power_the_laboratory_2`.
    pub fn enable_power_the_laboratory_2(&mut self) {
        self.power_the_laboratory = false;
        self.power_the_laboratory_2 = true;
    }

    pub fn disable_power_the_laboratory_2(&mut self) {
        self.power_the_laboratory_2 = false;
    }

    /// A settler's bound guard is no protection when two visible hostiles
    /// that can reach the tile each match its strength, not only when one is
    /// 1.5× it.
    ///
    /// Version 2 of `settler_guard_holds`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `settler-guard-holds-2`. See `AdvancedAi::settler_guard_holds_2`.
    pub fn enable_settler_guard_holds_2(&mut self) {
        self.settler_guard_holds = false;
        self.settler_guard_holds_2 = true;
    }

    pub fn disable_settler_guard_holds_2(&mut self) {
        self.settler_guard_holds_2 = false;
    }

    /// A settle site one settler drops for an empire-wide invalidation is set
    /// aside for every own settler for the same window. Route safety remains
    /// local, so an escorted Settler can still use a site an unguarded one
    /// cannot approach.
    ///
    /// Version 2 of `settler_target_hysteresis`; one version of a family plays, so this
    /// turns version 1 off. Opt-in gene `settler-target-hysteresis-2`. See `AdvancedAi::settler_target_hysteresis_2`.
    pub fn enable_settler_target_hysteresis_2(&mut self) {
        self.settler_target_hysteresis = false;
        self.settler_target_hysteresis_2 = true;
    }

    pub fn disable_settler_target_hysteresis_2(&mut self) {
        self.settler_target_hysteresis_2 = false;
    }

    /// The first turn an own land unit stands within two tiles of an at-war
    /// city resets the war-fatigue clock once for that city, so a campaign
    /// still walking to its target is not offered away as stalled.
    ///
    /// Opt-in gene `siege-is-progress-2`. See `AdvancedAi::siege_is_progress_2`.
    pub fn enable_siege_is_progress_2(&mut self) {
        self.siege_is_progress_3 = false;
        self.siege_is_progress_2 = true;
    }

    pub fn disable_siege_is_progress_2(&mut self) {
        self.siege_is_progress_2 = false;
    }

    /// Reset war fatigue once per enemy city only after a nearby own land
    /// force is observed reducing its wall or city health. Version 3 replaces
    /// v2's proximity-only reset and remains independently reversible.
    pub fn enable_siege_is_progress_3(&mut self) {
        self.siege_is_progress_2 = false;
        self.siege_is_progress_3 = true;
    }

    /// The twin of `enable_siege_is_progress_3`.
    pub fn disable_siege_is_progress_3(&mut self) {
        self.siege_is_progress_3 = false;
    }
    /// Escort live settlers by shadowing with ordinary moves instead of
    /// Civilization VI's formation channel, which stalls.
    ///
    /// The live bridge recorded formations failing to advance; a guard that
    /// shadows with normal movement is the already-tested host-safe path.
    /// This is therefore a live fidelity gate, not a re-promotion of the
    /// native gene that the ledger correctly holds off.
    pub fn enable_live_formationless_settler_shadow(&mut self) {
        self.live_formationless_settler_shadow = true;
    }

    /// The twin of [`Self::enable_live_formationless_settler_shadow`].
    pub fn disable_live_formationless_settler_shadow(&mut self) {
        self.live_formationless_settler_shadow = false;
        self.settler_guards.clear();
        self.settler_sea_guards.clear();
        self.settler_escort_journeys.clear();
        self.guard_wait.clear();
    }
    /// In peacetime let the whole army answer home threats, ranking a nearby
    /// barbarian camp above countryside raiders. See `BasicAi::camp_party`.
    pub fn enable_camp_party(&mut self) {
        self.base.enable_camp_party();
    }

    pub fn disable_camp_party(&mut self) {
        self.base.disable_camp_party();
    }

    /// Withhold the committed exploration goal that production Advanced
    /// carries by default (see `BasicAi::explore_commit`), so the evaluator
    /// arm `advanced_without_explore_commit` can price it.
    pub fn disable_explore_commit(&mut self) {
        self.base.explore_commit = false;
    }

    /// Let the defensive Recovery posture expire after a turn limit instead of
    /// trapping the empire in it permanently.
    ///
    /// Production leaves this measured-null repair off. The live bridge and
    /// explicit evaluator bundles call this method when they need the repair;
    /// keeping the switch here makes that opt-in auditable.
    pub fn enable_bounded_recovery(&mut self) {
        self.bounded_recovery = true;
    }

    /// Credit a movement tile for the attack it opens next, not only charge it
    /// for the threat it accepts. Native tournament games leave this disabled
    /// so their recorded ladders stay comparable.
    pub fn enable_strike_opening(&mut self) {
        self.strike_opening = true;
    }

    /// Withholding twin for `enable_strike_opening`, so the live bundle can be
    /// priced by taking this one treatment out of it. See `LIVE_TREATMENTS`.
    pub fn disable_strike_opening(&mut self) {
        self.strike_opening = false;
    }

    /// Score the Diplomacy lane by when twenty Diplomatic Victory Points
    /// arrive along the Congress calendar, not by how many are banked. See
    /// [`Self::diplomatic_lane_forecast`].
    pub fn enable_diplomatic_lane_forecast(&mut self) {
        self.diplomatic_lane_forecast_2 = false;
        self.diplomatic_lane_forecast = true;
    }

    pub fn disable_diplomatic_lane_forecast(&mut self) {
        self.diplomatic_lane_forecast = false;
    }

    /// Version 2 waits until a current suzerainty or already-earned Diplomatic
    /// Victory Point proves a real foothold before projecting Congress turns.
    /// One version per family is active in a screen.
    pub fn enable_diplomatic_lane_forecast_2(&mut self) {
        self.diplomatic_lane_forecast = false;
        self.diplomatic_lane_forecast_2 = true;
    }

    /// The twin of `enable_diplomatic_lane_forecast_2`.
    pub fn disable_diplomatic_lane_forecast_2(&mut self) {
        self.diplomatic_lane_forecast_2 = false;
    }

    /// Read a rival's religious clock from the cities it has converted rather
    /// than from whole civilizations already lost. See
    /// [`Self::conversion_majority_alarm`].
    pub fn enable_conversion_majority_alarm(&mut self) {
        self.conversion_majority_alarm = true;
        self.conversion_majority_alarm_2 = false;
    }

    pub fn disable_conversion_majority_alarm(&mut self) {
        self.conversion_majority_alarm = false;
    }

    /// Read each rival civilization's progress toward its own majority, then
    /// use the least-converted holdout as the conjunctive victory clock.
    /// Version 2 of `conversion-majority-alarm`; enabling it selects this
    /// family version.
    pub fn enable_conversion_majority_alarm_2(&mut self) {
        self.conversion_majority_alarm = false;
        self.conversion_majority_alarm_2 = true;
    }

    /// The twin of `enable_conversion_majority_alarm_2`.
    pub fn disable_conversion_majority_alarm_2(&mut self) {
        self.conversion_majority_alarm_2 = false;
    }

    /// Score the Culture lane by where the two tourist curves are when the
    /// clock stops. See [`Self::culture_lane_forecast`].
    pub fn enable_culture_lane_forecast(&mut self) {
        self.culture_lane_forecast = true;
        self.culture_lane_forecast_2 = false;
    }

    pub fn disable_culture_lane_forecast(&mut self) {
        self.culture_lane_forecast = false;
    }

    pub fn enable_culture_lane_forecast_2(&mut self) {
        self.culture_lane_forecast = false;
        self.culture_lane_forecast_2 = true;
    }

    pub fn disable_culture_lane_forecast_2(&mut self) {
        self.culture_lane_forecast_2 = false;
    }

    /// Measure the Recovery power gap against the war we are actually
    /// fighting. See [`Self::recovery_reads_the_war`].
    pub fn enable_recovery_reads_the_war(&mut self) {
        self.recovery_reads_the_war_2 = false;
        self.recovery_reads_the_war = true;
    }

    pub fn disable_recovery_reads_the_war(&mut self) {
        self.recovery_reads_the_war = false;
    }

    /// Measure Recovery against the strongest current major opponent within
    /// the campaign planner's reach. Enabling version two turns version one
    /// off; immediate city threats remain a separate trigger. See
    /// [`Self::recovery_reads_the_war_2`].
    pub fn enable_recovery_reads_the_war_2(&mut self) {
        self.recovery_reads_the_war = false;
        self.recovery_reads_the_war_2 = true;
    }

    /// The twin of `enable_recovery_reads_the_war_2`.
    pub fn disable_recovery_reads_the_war_2(&mut self) {
        self.recovery_reads_the_war_2 = false;
    }

    /// ★★★★★ EVERY LIVE-BRIDGE REPAIR, IN ONE PLACE THAT THE MEASUREMENT CAN PLAY.
    ///
    /// These eight flags are the difference between the frozen tournament
    /// controller and the agent that actually plays Civilization VI. They were
    /// set one by one inside `civvis_orders::decide`, which is a binary — so no
    /// headless arm could construct the deployed agent, and **not one of them
    /// has ever been measured on an outcome.** #930, #933, #955, #957 and #962
    /// all shipped on reasoning plus a live anecdote.
    ///
    /// Collecting them here gives `builtin_ai` a `live` controller to play, so
    /// `civvis tournament --ais live,live_without_<flag>` can price any one of
    /// them in cities and score instead of in order counts. The bridge calls
    /// this same function, so the measured agent and the deployed agent cannot
    /// drift apart.
    ///
    /// ⚠ ADD NEW BRIDGE FLAGS TO `enable_live_bridge_universe`, not in the
    /// binary, or the arm silently stops matching the deployment — the exact
    /// shape of `civvis-the-runner-tree-was-the-broken-link`.
    ///
    /// ★★★★ AND WHAT SHIPS IS THE UNIVERSE MINUS WHAT THE LEDGER HOLDS OFF.
    /// Operator directive 2026-08-20: the defaults reflect the best genome.
    /// Since the directive of 2026-08-22 the ledger reads that off the
    /// ranking's two win columns — a gene is on when both its last and prior
    /// native screens are positive, or when their average clears +15 with
    /// neither below −10. `apply_gene_ledger` (`advanced/gene_ledger.rs`)
    /// ends this helper: a treatment the ledger does not default on is
    /// withheld, an opt-in it defaults on is enabled, a flag no native screen
    /// can price (Firaxis-only) stays as the universe set it. A new treatment
    /// therefore ships OFF until two screens agree; `gene_screen --list` shows
    /// each gene's verdict and default.
    pub fn enable_live_bridge(&mut self) {
        self.enable_live_bridge_universe();
        self.apply_gene_ledger();
    }

    /// Every live treatment on, the ledger NOT applied: the genome's universe.
    /// `gene_screen` starts here and sets each gene to its drawn state; the
    /// membership tests read this body. Deployment is `enable_live_bridge`.
    pub fn enable_live_bridge_universe(&mut self) {
        self.observed_player = true;
        // Every `live()` gene in the registry: the repairs and the host-only
        // adapters. The reason each exists is on its row in `genes.rs`.
        for gene in super::GENES.iter().filter(|gene| gene.live()) {
            (gene.enable)(self);
        }
        self.apply_shared_city_target_contract();
    }
    /// The deployment player under native execution. This is intentionally
    /// the same fixed policy bundle and ledger as `enable_live_bridge`.
    /// Historically the host-only flags were omitted here, including several
    /// strategic safeguards; that made a tournament genome a different player.
    /// True host representation adapters remain inert without their host facts.
    pub fn enable_engine_repairs(&mut self) {
        self.enable_engine_repairs_universe();
        self.apply_gene_ledger();
    }

    /// The deployment genome for a board CIVVIS itself owns: the same
    /// universe and ledger as [`AdvancedAi::enable_engine_repairs`], planning
    /// on the authoritative board rather than on a fog-redacted view.
    ///
    /// This is the player civvis.ai seats (`Session::ai_fleet`). The ledger
    /// is what makes it the deployment genome — the stock `AdvancedAi::new()`
    /// the site seated before carried only the production genes, and lost
    /// head to head: as one major against three of these at Prince (Tiny
    /// Pangaea, Online, seeds 52000000+) it won 2 of 34 games, and one of
    /// these against three stock agents won 24 of 48, where parity is one in
    /// four. Planning on the authoritative board keeps the information
    /// contract the site's agents always had and costs a third of the
    /// fog-honest planner's replanned frames; the view-and-replay executor
    /// stays with the seats that need it (`gene_screen`, the live bridge).
    pub fn enable_native_deployment(&mut self) {
        self.enable_engine_repairs();
        self.observed_player = false;
    }

    /// Every native repair on, the ledger NOT applied — the two halves plus
    /// the fixed shared city horizon. `gene_screen` draws only the registered
    /// repairs, so this production-profile policy remains fixed across every
    /// tournament genome. See `enable_live_bridge_universe`.
    pub fn enable_engine_repairs_universe(&mut self) {
        self.enable_live_bridge_universe();
    }

    /// Let a stalled settler found where it stands. Evaluator arm
    /// `advanced_settler_founds_when_stalled`; off in the native production
    /// controller.
    ///
    /// ⚠ **Not off in the live bridge.** `enable_stranded_settler_discount`
    /// sets the same flag as part of its bundle, and `enable_live_bridge`
    /// calls that — so a live seat DOES found where it stands, and this
    /// explicit enabler is only how an evaluator seats the behaviour on a
    /// native arm. Reading "off in production" here and stopping cost a study
    /// on 2026-08-19 a false conclusion: `founds_where_it_stands` opens with
    /// `if !self.settler_founds_when_stalled`, so the flag looks like the
    /// reason that path fires rarely on the live ladder. It is not — the flag
    /// is on, and `g.can_found_city` refusing the tile a walker happens to be
    /// standing on is what usually ends it.
    pub fn enable_settler_founds_when_stalled(&mut self) {
        self.settler_founds_when_stalled = true;
    }

    /// Build a ranged defender, not a melee one, when the barbarian ring around
    /// a city is mostly shooters. See `BasicAi::barbarian_ranged_answer`;
    /// withheld by `barbarian-ranged-answer`.
    pub fn enable_barbarian_ranged_answer(&mut self) {
        self.base.enable_barbarian_ranged_answer();
    }

    pub fn disable_barbarian_ranged_answer(&mut self) {
        self.base.disable_barbarian_ranged_answer();
    }

    /// Price a fight against a barbarian below a fight against a major, since
    /// barbarians carry no war costs. See `BasicAi::barbarian_bargain`;
    /// withheld by `barbarian-bargain`.
    pub fn enable_barbarian_bargain(&mut self) {
        self.base.enable_barbarian_bargain();
    }

    pub fn disable_barbarian_bargain(&mut self) {
        self.base.disable_barbarian_bargain();
    }

    /// Deliberate camp clearing as a peacetime errand. See
    /// `BasicAi::camp_bounty`; entrant `advanced_camp_bounty`.
    pub fn enable_camp_bounty(&mut self) {
        self.base.camp_bounty = true;
    }

    /// Hold the stranded-Settler discount off, for the controlled arm.
    ///
    /// ⚠ Every `enable_*` in `enable_live_bridge` needs this counterpart or the
    /// treatment cannot be ablated: `--without stranded-settler-discount` exits
    /// 2 on an unknown name, so the one arm that would measure the repair
    /// against the deployed configuration does not exist. It shipped without
    /// one; this is that omission.
    pub fn disable_stranded_settler_discount(&mut self) {
        self.base.settler_strand_discount = false;
        self.settler_founds_when_stalled = false;
    }

    /// Expand to all the land the empire can hold at pipeline pace, instead of
    /// on an era clock. Sets both halves: the strategic governor's target,
    /// window and pipeline, and `BasicAi::pick_item`'s pipeline and window.
    pub fn enable_land_grab(&mut self) {
        self.land_grab = true;
        self.base.land_grab = true;
    }

    pub fn disable_land_grab(&mut self) {
        self.land_grab = false;
        self.base.land_grab = false;
    }

    /// Reserve the safe capital's first build for its first Scout.
    pub fn enable_scout_first_opening(&mut self) {
        self.scout_first_opening = true;
        self.base.scout_first_opening = true;
    }

    /// Restore the configured opening without changing its progress.
    pub fn disable_scout_first_opening(&mut self) {
        self.scout_first_opening = false;
        self.base.scout_first_opening = false;
    }

    /// Replace the capital's first opening slot with legal ranged support.
    pub fn enable_opening_ranged_support(&mut self) {
        self.opening_ranged_support = true;
        self.base.opening_ranged_support = true;
    }

    /// Restore the ordinary opener without changing later recon settings.
    pub fn disable_opening_ranged_support(&mut self) {
        self.opening_ranged_support = false;
        self.base.opening_ranged_support = false;
    }

    /// After its current production completes, let a population-two capital
    /// start the next legal Settler before ordinary production ranking. The
    /// baseline governor retains the city-target, site, and emergency gates.
    pub fn enable_capital_settler_after_completion(&mut self) {
        self.capital_settler_after_completion = true;
        self.base.enable_capital_settler_after_completion();
    }

    /// The twin of [`AdvancedAi::enable_capital_settler_after_completion`].
    pub fn disable_capital_settler_after_completion(&mut self) {
        self.capital_settler_after_completion = false;
        self.base.disable_capital_settler_after_completion();
    }

    /// Take the pantheon that founds cities and keep the God-King card until
    /// its Faith is paid. Sets both halves: the strategic portfolio's God-King
    /// want, and `BasicAi`'s pantheon prefix.
    pub fn enable_expansion_pantheon(&mut self) {
        self.expansion_pantheon = true;
        self.base.expansion_pantheon = true;
    }

    pub fn disable_expansion_pantheon(&mut self) {
        self.expansion_pantheon = false;
        self.base.expansion_pantheon = false;
    }

    /// Hold the opening book's Settler slot until the host's population floor
    /// instead of burning it.
    pub fn enable_opening_settler_waits(&mut self) {
        self.opening_settler_waits = true;
        self.base.opening_settler_waits = true;
    }

    pub fn disable_opening_settler_waits(&mut self) {
        self.opening_settler_waits = false;
        self.base.opening_settler_waits = false;
    }

    /// Bank an envoy instead of spending it on a placement whose score is
    /// negative. See `bank_envoys`.
    pub fn enable_bank_envoys(&mut self) {
        self.bank_envoys = true;
        self.base.enable_bank_envoys();
    }

    pub fn disable_bank_envoys(&mut self) {
        self.bank_envoys = false;
        self.base.disable_bank_envoys();
    }

    /// Keep the hostile-envelope table across this seat's own unit moves —
    /// evaluator arm `advanced_envelope_own_moves`. See
    /// `BasicAi::envelope_cache_across_own_moves`.
    pub fn enable_envelope_cache_across_own_moves(&mut self) {
        self.base.enable_envelope_cache_across_own_moves();
    }

    /// Price a wonder's effects in the victory lane's currency and build the
    /// ones that lane needs. See `AdvancedAi::strategic_wonder_value`.
    pub fn enable_strategic_wonders(&mut self) {
        self.strategic_wonders = true;
    }

    /// Withholding twin for `enable_strategic_wonders`, so the arm can be
    /// priced by taking this one treatment out. See `LIVE_TREATMENTS`.
    pub fn disable_strategic_wonders(&mut self) {
        self.strategic_wonders = false;
    }

    /// Aim research at the technology that raises the housing ceiling while
    /// housing is throttling growth.
    pub fn enable_housing_research(&mut self) {
        self.housing_research = true;
        self.housing_research_2 = false;
    }

    pub fn disable_housing_research(&mut self) {
        self.housing_research = false;
    }

    /// Research housing that the capped cities can actually build.
    pub fn enable_housing_research_2(&mut self) {
        self.housing_research = false;
        self.housing_research_2 = true;
    }

    pub fn disable_housing_research_2(&mut self) {
        self.housing_research_2 = false;
    }

    /// Hold one more live-bridge flag off so it can be priced. Every flag in
    /// `enable_live_bridge` needs one of these or it ships unmeasured — which is
    /// how five repairs reached deployment without a single outcome number.
    pub fn disable_bounded_recovery(&mut self) {
        self.bounded_recovery = false;
    }

    /// Plan a city's districts, their plots and tile purchases jointly,
    /// reserving plots in rings one to three. The city plans its districts,
    /// sites and tile buys together: wished districts get jointly assigned,
    /// reserved plots over rings 1-3, and the tile a very valuable site needs
    /// is bought. See [`AdvancedAi::district_planning`]. Opt-in gene
    /// `district-planning`. (Filed here rather than under a marker: the
    /// append-point check reads a line's first identifier, which for any `pub
    /// fn` is `pub`.)
    pub fn enable_district_planning(&mut self) {
        self.district_planning_3 = false;
        self.district_planning = true;
    }

    /// The twin of `enable_district_planning`.
    pub fn disable_district_planning(&mut self) {
        self.district_planning = false;
    }

    /// Let a Missionary on its last charge explore nearby fog for a few turns
    /// before spending it. A Missionary on its last charge explores the fog
    /// within ten tiles for up to twelve turns before spending it, unless a
    /// city of ours is slipping or an untouched city stands beside it. See
    /// [`AdvancedAi::missionary_last_charge_explores`]. Opt-in gene
    /// `missionary-last-charge-explores`. (Filed here rather than under a
    /// marker: the append-point check reads a line's first identifier.)
    pub fn enable_missionary_last_charge_explores(&mut self) {
        self.missionary_last_charge_explores_2 = false;
        self.missionary_last_charge_explores = true;
        self.missionary_explore.borrow_mut().clear();
    }

    /// The twin of `enable_missionary_last_charge_explores`.
    pub fn disable_missionary_last_charge_explores(&mut self) {
        self.missionary_last_charge_explores = false;
    }

    /// Send a last-charge Missionary on a long, routeable expedition before it
    /// spends the charge: up to thirty-six tiles, through borders religious
    /// units may cross, while still yielding to an endangered own city or an
    /// untouched city beside it. This is version two of the local ten-tile
    /// policy, so it is mutually exclusive with it. Opt-in gene
    /// `missionary-last-charge-explores-2`; see
    /// [`AdvancedAi::missionary_last_charge_explores_2`].
    pub fn enable_missionary_last_charge_explores_2(&mut self) {
        self.missionary_last_charge_explores = false;
        self.missionary_last_charge_explores_2 = true;
        self.missionary_explore.borrow_mut().clear();
    }

    /// The twin of `enable_missionary_last_charge_explores_2`.
    pub fn disable_missionary_last_charge_explores_2(&mut self) {
        self.missionary_last_charge_explores_2 = false;
    }

    /// Appraise weaker neighbours, plan to take one to three holdable cities
    /// the army can afford, and launch when staged. Appraise the neighbours on
    /// public military power and tech count, plan the holdable city — or two,
    /// or three — of a weaker one that the field army can take with units to
    /// spare, and launch when the staging ring carries that bill. The plan aims
    /// the campaign, replaces the empire ratio at the declaration with the
    /// city's own requirement, and offers peace once every planned city is
    /// taken. [`AdvancedAi::city_campaign`]. Opt-in gene `city-campaign`; see
    /// `advanced/city_campaign.rs`.
    pub fn enable_city_campaign(&mut self) {
        self.city_campaign = true;
        self.city_campaign_2 = false;
    }

    /// The twin of `enable_city_campaign`.
    pub fn disable_city_campaign(&mut self) {
        self.city_campaign = false;
    }

    /// Station an idle recon unit on the chokepoint tile of the land route
    /// toward a neighbour, or watch their border. A recon unit with nothing
    /// left to explore holds the pass toward a neighbour — the first tile
    /// outside their borders whose removal cuts the land walk between the two
    /// capitals — or, when no single tile cuts it, watches the border tile that
    /// walk leaves their ground by. [`AdvancedAi::pass_picket`]. Opt-in gene
    /// `pass-picket`; see `advanced/recon_disruption.rs`.
    pub fn enable_pass_picket(&mut self) {
        self.pass_picket = true;
        self.pass_picket_2 = false;
    }

    /// The twin of `enable_pass_picket`.
    pub fn disable_pass_picket(&mut self) {
        self.pass_picket = false;
    }

    /// Hold a pass against a visible approaching Settler before exploring.
    pub fn enable_pass_picket_2(&mut self) {
        self.pass_picket_2 = true;
        self.pass_picket = false;
    }

    pub fn disable_pass_picket_2(&mut self) {
        self.pass_picket_2 = false;
    }

    /// Version 2 of `science_victory_drive`: the original planner remains a
    /// separately measurable family member, while this continuation uses a
    /// meaningful lead, legal launch sites, a research funnel, and an estimate
    /// that can keep a live chain moving. One version of the family plays, so
    /// this turns version 1 off. Opt-in gene `science-victory-drive-2`.
    pub fn enable_science_victory_drive_2(&mut self) {
        self.science_victory_drive = false;
        self.science_victory_drive_2 = true;
    }

    /// The twin of `enable_science_victory_drive_2`.
    pub fn disable_science_victory_drive_2(&mut self) {
        self.science_victory_drive_2 = false;
    }

    /// In the opening, ordinary repeatable Great-Person projects wait for
    /// city development; an open Prophet race and a clutch for an exceptional
    /// Scientist remain forcing. Opt-in gene `early-project-restraint`.
    pub fn enable_early_project_restraint(&mut self) {
        self.early_project_restraint_2 = false;
        self.early_project_restraint = true;
    }

    /// The twin of `enable_early_project_restraint`.
    pub fn disable_early_project_restraint(&mut self) {
        self.early_project_restraint = false;
    }

    /// A unit the next blow could remove leaves the reach of whatever can
    /// strike it — the roll-top total of visible attackers, or a raider
    /// remembered in the fog — and a shooter or scout does not end the
    /// turn inside a raider's reach without a melee unit beside it. See
    /// `advanced/wounded_out_of_reach.rs`. Opt-in gene `wounded-out-of-reach`.
    pub fn enable_wounded_out_of_reach(&mut self) {
        self.wounded_out_of_reach_2 = false;
        self.wounded_out_of_reach = true;
    }

    /// The twin of `enable_wounded_out_of_reach`.
    pub fn disable_wounded_out_of_reach(&mut self) {
        self.wounded_out_of_reach = false;
    }

    /// Version two retains the original withdrawal and also considers a
    /// nominal lethal shot from a recently observed gun. One version plays.
    pub fn enable_wounded_out_of_reach_2(&mut self) {
        self.wounded_out_of_reach = false;
        self.wounded_out_of_reach_2 = true;
    }

    pub fn disable_wounded_out_of_reach_2(&mut self) {
        self.wounded_out_of_reach_2 = false;
    }

    /// An improvement that completes an unresearched technology's or civic's
    /// boost is worth the research the boost grants, spread over the steps
    /// the trigger still needs. See [`AdvancedAi::eureka_builder_premium`].
    /// Opt-in gene `eureka-chasing-builder`.
    pub fn enable_eureka_chasing_builder(&mut self) {
        self.eureka_chasing_builder_2 = false;
        self.eureka_chasing_builder = true;
    }

    /// The twin of `enable_eureka_chasing_builder`.
    pub fn disable_eureka_chasing_builder(&mut self) {
        self.eureka_chasing_builder = false;
    }

    /// Version two rewards only a final, immediately usable Builder eureka
    /// step for the technology or civic currently in progress. It replaces
    /// the broad v1 premium for this seat. Opt-in gene
    /// `eureka-chasing-builder-2`.
    pub fn enable_eureka_chasing_builder_2(&mut self) {
        self.eureka_chasing_builder = false;
        self.eureka_chasing_builder_2 = true;
    }

    /// The twin of `enable_eureka_chasing_builder_2`.
    pub fn disable_eureka_chasing_builder_2(&mut self) {
        self.eureka_chasing_builder_2 = false;
    }

    /// Recruit the target's neighbours before an elective war: alliances,
    /// envoys to its city-states and joint-war invitations at the strike. From
    /// the turn the war desk holds a target we are at peace with, one alliance
    /// a turn is proposed to the target's best neighbour (`military` when free
    /// on both sides), the envoy scorer gains a term for a city-state near the
    /// target — most for one the target holds — and the turn the desk would
    /// declare, every eligible neighbour is sent `ProposeJointWar` and the
    /// declaration is held while an answer is due. See
    /// [`AdvancedAi::coalition_observe`]. Opt-in gene `coalition-before-war`.
    /// Filed above the markers: the append-point check reads a method line's
    /// first identifier.
    pub fn enable_coalition_before_war(&mut self) {
        self.coalition_before_war = true;
        self.coalition_before_war_2 = false;
        self.coalition_before_war_3 = false;
    }

    /// The twin of `enable_coalition_before_war`.
    pub fn disable_coalition_before_war(&mut self) {
        self.coalition_before_war = false;
    }

    /// Scale a node whose boost is already in hand by what the discount buys
    /// under the value function's own cost divisor. `tech_value` and
    /// `civic_value` both end `(value + k) / cost.sqrt()`, so a node costing
    /// `1 - frac` of its printed price scores `1 / (1 - frac).sqrt()` times as
    /// much — 1.29 at the shipped 40%, capped. See
    /// [`AdvancedAi::boost_in_hand_scale`]. Opt-in gene
    /// `boost-first-research`. Filed above the markers: the append-point check
    /// reads a method line's first identifier.
    pub fn enable_boost_first_research(&mut self) {
        self.boost_first_research = true;
        self.boost_first_research_2 = false;
    }

    /// The twin of `enable_boost_first_research`.
    pub fn disable_boost_first_research(&mut self) {
        self.boost_first_research = false;
    }

    /// Let a boost in hand break ties among comparable research candidates
    /// only: the same discount scale as version one, applied in the argmax
    /// after every forced lane goal has stood down, to a boosted node whose
    /// unscaled score is within `BOOST_TIEBREAK_BAND` of the ordinary
    /// winner's. See [`AdvancedAi::boost_tiebreak_pick`]. Opt-in gene
    /// `boost-first-research-2`; a seat plays at most one version.
    pub fn enable_boost_first_research_2(&mut self) {
        self.boost_first_research_2 = true;
        self.boost_first_research = false;
    }

    /// The twin of `enable_boost_first_research_2`.
    pub fn disable_boost_first_research_2(&mut self) {
        self.boost_first_research_2 = false;
    }

    /// Version one hunts every Eureka and Inspiration through a global union
    /// of research, production, Builder, and combat hooks. It stays intact as
    /// the broad family control. See `advanced/chase_every_boost.rs`. Opt-in
    /// gene `chase-every-boost`.
    /// Filed above the markers: the append-point check reads a method line's
    /// first identifier.
    pub fn enable_chase_every_boost(&mut self) {
        self.chase_every_boost_2 = false;
        self.chase_every_boost = true;
    }

    /// The twin of `enable_chase_every_boost`.
    pub fn disable_chase_every_boost(&mut self) {
        self.chase_every_boost = false;
    }

    /// Version two only lets a city finish the final production trigger for
    /// the technology or civic already in progress, before that study ends.
    /// Enabling the successor turns the broad version one off, so one family
    /// version plays. See `advanced/chase_every_boost.rs`. Opt-in gene
    /// `chase-every-boost-2`.
    pub fn enable_chase_every_boost_2(&mut self) {
        self.chase_every_boost = false;
        self.chase_every_boost_2 = true;
    }

    /// The twin of `enable_chase_every_boost_2`.
    pub fn disable_chase_every_boost_2(&mut self) {
        self.chase_every_boost_2 = false;
    }

    /// Version one buys Walls or a land defender for a city struck within four
    /// turns, retaining its broad damage-memory signal for comparison. Opt-in
    /// gene `native-emergency-purchase`; see
    /// `advanced/gold_and_cards.rs`. Filed above the markers: the append-point
    /// check reads a method line's first identifier.
    pub fn enable_native_emergency_purchase(&mut self) {
        self.native_emergency_purchase_2 = false;
        self.native_emergency_purchase = true;
    }

    /// The twin of `enable_native_emergency_purchase`.
    pub fn disable_native_emergency_purchase(&mut self) {
        self.native_emergency_purchase = false;
    }

    /// Version two buys the same local answer on a host-confirmed hit in the
    /// current turn, or on a one-turn-old damage mark that a visible at-war
    /// military unit can legally follow with a City Center attack. One family
    /// version plays, so enabling this successor turns version one off. Opt-in
    /// gene `native-emergency-purchase-2`; see `advanced/gold_and_cards.rs`.
    pub fn enable_native_emergency_purchase_2(&mut self) {
        self.native_emergency_purchase = false;
        self.native_emergency_purchase_2 = true;
    }

    /// The twin of `enable_native_emergency_purchase_2`.
    pub fn disable_native_emergency_purchase_2(&mut self) {
        self.native_emergency_purchase_2 = false;
    }

    /// Send an explorer to the unseen ring of a natural wonder within
    /// settling range of an own city before it picks a frontier, so a site
    /// beside the wonder exists to be priced. Opt-in gene
    /// `wonder-ring-recon`; see `advanced/wonder_sites.rs` and
    /// `BasicAi::wonder_ring_recon`.
    pub fn enable_wonder_ring_recon(&mut self) {
        self.base.wonder_ring_recon_2 = false;
        self.base.wonder_ring_recon = true;
    }

    /// The twin of `enable_wonder_ring_recon`.
    pub fn disable_wonder_ring_recon(&mut self) {
        self.base.wonder_ring_recon = false;
    }

    /// Preserve every nearby natural-wonder pocket V1 scouts, but spend at
    /// most one extra tile of travel to expose more of its footprint. This is
    /// version two of `wonder-ring-recon`, so enabling it turns V1 off.
    pub fn enable_wonder_ring_recon_2(&mut self) {
        self.base.wonder_ring_recon = false;
        self.base.wonder_ring_recon_2 = true;
    }

    /// The twin of `enable_wonder_ring_recon_2`.
    pub fn disable_wonder_ring_recon_2(&mut self) {
        self.base.wonder_ring_recon_2 = false;
    }

    /// Reserve the first Builder ahead of ordinary production, the way
    /// `solvency-first-trade-slot` reserves the first trade slot. See
    /// `AdvancedAi::first_builder_reserve`; opt-in gene `first-builder-reserve`.
    /// Filed here rather than under a marker: the append-point check reads a
    /// method line's first identifier.
    pub fn enable_first_builder_reserve(&mut self) {
        self.first_builder_reserve = true;
        self.first_builder_reserve_2 = false;
    }

    /// The twin of `enable_first_builder_reserve`.
    pub fn disable_first_builder_reserve(&mut self) {
        self.first_builder_reserve = false;
    }

    /// Reserve one Builder for an immediately connectable first-copy luxury
    /// when Amenities are short and expansion is covered, retaining a
    /// lifetime receipt. Opt-in gene `first-builder-reserve-2`.
    pub fn enable_first_builder_reserve_2(&mut self) {
        self.first_builder_reserve_2 = true;
        self.first_builder_reserve = false;
    }

    /// The twin of `enable_first_builder_reserve_2`.
    pub fn disable_first_builder_reserve_2(&mut self) {
        self.first_builder_reserve_2 = false;
    }

    /// Climb to a tier-2 government once Political Philosophy lands, instead of
    /// playing the whole game on four policy slots. See
    /// `AdvancedAi::government_ladder`; opt-in gene `government-ladder`. Filed
    /// here rather than under a marker: the append-point check reads a method
    /// line's first identifier.
    pub fn enable_government_ladder(&mut self) {
        self.government_ladder = true;
        self.government_ladder_3 = false;
    }

    /// The twin of `enable_government_ladder`.
    pub fn disable_government_ladder(&mut self) {
        self.government_ladder = false;
    }
    /// Fill an idle turn with something that is not a soldier, or leave it
    /// idle. See `AdvancedAi::never_an_empty_queue_2`; opt-in gene
    /// `never-an-empty-queue-2`, version two of `never-an-empty-queue`. Filed
    /// here rather than under a marker: the append-point check reads a method
    /// line's first identifier.
    pub fn enable_never_an_empty_queue_2(&mut self) {
        self.never_an_empty_queue = false;
        self.never_an_empty_queue_2 = true;
        self.never_an_empty_queue_3 = false;
    }

    /// The twin of `enable_never_an_empty_queue_2`.
    pub fn disable_never_an_empty_queue_2(&mut self) {
        self.never_an_empty_queue_2 = false;
    }
    /// Build the best real candidate instead of standing idle when nothing
    /// clears the ordinary production bar. See
    /// `AdvancedAi::never_an_empty_queue`; opt-in gene `never-an-empty-queue`.
    /// Filed here rather than under a marker: the append-point check reads a
    /// method line's first identifier.
    pub fn enable_never_an_empty_queue(&mut self) {
        self.never_an_empty_queue = true;
        self.never_an_empty_queue_2 = false;
        self.never_an_empty_queue_3 = false;
    }

    /// The twin of `enable_never_an_empty_queue`.
    pub fn disable_never_an_empty_queue(&mut self) {
        self.never_an_empty_queue = false;
    }

    /// Climb past tier two, and keep climbing while the field out-slots us.
    /// See `government_ladder_rung`.
    pub fn enable_government_ladder_2(&mut self) {
        self.government_ladder_2 = true;
        self.government_ladder_3 = false;
    }

    /// The twin of `enable_government_ladder_2`.
    pub fn disable_government_ladder_2(&mut self) {
        self.government_ladder_2 = false;
    }

    /// See `BasicAi::front_weighted_floor`: the military floor builds where
    /// the unit reaches the campaign's target soonest.
    pub fn enable_front_weighted_floor(&mut self) {
        self.front_weighted_floor = true;
        self.base.front_weighted_floor = true;
    }

    /// The twin of `enable_front_weighted_floor`.
    pub fn disable_front_weighted_floor(&mut self) {
        self.front_weighted_floor = false;
        self.base.front_weighted_floor = false;
        self.base.front_objective = None;
    }

    /// See `BasicAi::front_weighted_floor_2`: a city near the campaign's
    /// target builds the floor's unit ahead of its economy steps.
    pub fn enable_front_weighted_floor_2(&mut self) {
        self.front_weighted_floor_2 = true;
        self.base.front_weighted_floor_2 = true;
    }

    /// The twin of `enable_front_weighted_floor_2`.
    pub fn disable_front_weighted_floor_2(&mut self) {
        self.front_weighted_floor_2 = false;
        self.base.front_weighted_floor_2 = false;
    }

    /// See `BasicAi::activation_resume_waits`: an activation path waits for a
    /// queued district that already holds production.
    pub fn enable_activation_resume_waits(&mut self) {
        self.activation_resume_waits = true;
        self.base.activation_resume_waits = true;
    }

    /// The twin of `enable_activation_resume_waits`.
    pub fn disable_activation_resume_waits(&mut self) {
        self.activation_resume_waits = false;
        self.base.activation_resume_waits = false;
    }

    /// See `BasicAi::activation_keeps_its_building`: an activation path keeps
    /// a queued rung of any waiting person's slot chain.
    pub fn enable_activation_keeps_its_building(&mut self) {
        self.activation_keeps_its_building = true;
        self.base.activation_keeps_its_building = true;
    }

    /// The twin of `enable_activation_keeps_its_building`.
    pub fn disable_activation_keeps_its_building(&mut self) {
        self.activation_keeps_its_building = false;
        self.base.activation_keeps_its_building = false;
    }

    /// One digger for an owned person's missing Artifact, before more Museums.
    pub fn enable_artifact_before_more_museums(&mut self) {
        self.artifact_before_more_museums = true;
        self.base.artifact_before_more_museums = true;
    }

    /// Restore the existing activation capacity path.
    pub fn disable_artifact_before_more_museums(&mut self) {
        self.artifact_before_more_museums = false;
        self.base.artifact_before_more_museums = false;
    }

    /// The working reserve, and the first Builder or a missing Monument
    /// bought ahead of the purchase argmax. See `treasury_at_work_2`.
    pub fn enable_treasury_at_work_2(&mut self) {
        self.treasury_at_work_2 = true;
        self.treasury_at_work_2_2 = false;
    }

    /// The twin of `enable_treasury_at_work_2`.
    pub fn disable_treasury_at_work_2(&mut self) {
        self.treasury_at_work_2 = false;
    }

    /// Keep the working reserve and buy the first Builder near local work.
    pub fn enable_treasury_at_work_2_2(&mut self) {
        self.treasury_at_work_2 = false;
        self.treasury_at_work_2_2 = true;
    }

    pub fn disable_treasury_at_work_2_2(&mut self) {
        self.treasury_at_work_2_2 = false;
    }

    /// A declared war's objective nobody of ours has been near for
    /// `CAPTURE_GO_TURNS` consecutive turns is stood down explicitly and the
    /// strategy re-assessed, instead of being held unprosecuted. Filed here
    /// rather than under a marker: a whole function under one reads as an
    /// entry. See `capture_go_or_stand_down` and `advanced/commitments.rs`.
    pub fn enable_capture_go_or_stand_down(&mut self) {
        self.capture_go_or_stand_down = true;
    }

    /// The twin of `enable_capture_go_or_stand_down`.
    pub fn disable_capture_go_or_stand_down(&mut self) {
        self.capture_go_or_stand_down = false;
        self.capture_stood_down.clear();
    }

    /// Version 2 of `capture_go_or_stand_down`: also stands down a siege
    /// whose bodies are at the objective but have not pushed the city to a
    /// new low for `CAPTURE_STALL_TURNS` stalled readings. One version of a
    /// family plays, so this turns version 1 off. Opt-in gene
    /// `capture-go-or-stand-down-2`. Filed here rather than under a marker.
    pub fn enable_capture_go_or_stand_down_2(&mut self) {
        self.capture_go_or_stand_down = false;
        self.capture_go_or_stand_down_2 = true;
    }

    /// The twin of `enable_capture_go_or_stand_down_2`.
    pub fn disable_capture_go_or_stand_down_2(&mut self) {
        self.capture_go_or_stand_down_2 = false;
        self.capture_stood_down.clear();
    }

    /// A settle or improve target survives a passing threat, and the
    /// commitment ledger retires it after `COMMITMENT_PATIENCE` consecutive
    /// forgotten turns, parking the site. Filed here rather than under a
    /// marker: a whole function under one reads as an entry. See
    /// `commitment_patience` and `advanced/commitments.rs`.
    pub fn enable_commitment_patience(&mut self) {
        self.commitment_patience = true;
    }

    /// The twin of `enable_commitment_patience`.
    pub fn disable_commitment_patience(&mut self) {
        self.commitment_patience = false;
        self.builder_avoid.clear();
    }

    /// Version 2 keeps the coast baseline and additionally prices the best
    /// water-resource adjacency around a prospective Harbor.
    pub fn enable_coastal_city_sites_2(&mut self) {
        self.coastal_city_sites = false;
        self.coastal_city_sites_2 = true;
    }

    /// The twin of `enable_coastal_city_sites_2`.
    pub fn disable_coastal_city_sites_2(&mut self) {
        self.coastal_city_sites_2 = false;
    }

    /// A wounded unit holding a front trades places with the fresh unit
    /// behind it — the engine's own `Action::Swap`, which no controller has
    /// ever chosen — so the line does not open when it leaves. See
    /// `swap_rotation`.
    pub fn enable_swap_rotation(&mut self) {
        self.swap_rotation = true;
        self.swap_rotation_2 = false;
    }

    /// The twin of `enable_swap_rotation`.
    pub fn disable_swap_rotation(&mut self) {
        self.swap_rotation = false;
    }

    /// While a city of ours is threatened or bleeding, every ordinary Gold
    /// purchase leaves one emergency defender's price in the treasury. See
    /// `threatened_city_reserve`.
    pub fn enable_threatened_city_reserve(&mut self) {
        self.threatened_city_reserve = true;
        self.threatened_city_reserve_2 = false;
    }

    /// The twin of `enable_threatened_city_reserve`.
    pub fn disable_threatened_city_reserve(&mut self) {
        self.threatened_city_reserve = false;
    }

    pub fn enable_threatened_city_reserve_2(&mut self) {
        self.threatened_city_reserve = false;
        self.threatened_city_reserve_2 = true;
    }

    pub fn disable_threatened_city_reserve_2(&mut self) {
        self.threatened_city_reserve_2 = false;
    }

    /// Once the empire holds three cities, no Settler starts while an owned
    /// Settler has stood on one tile six turns; the same brake on the
    /// `BasicAi` pipeline. See `AdvancedAi::settler_in_flight_allowed`.
    pub fn enable_settler_backlog_brake(&mut self) {
        self.settler_backlog_brake_2 = false;
        self.settler_backlog_brake = true;
        self.base.settler_backlog_brake = true;
        self.base.settler_backlog_brake_2 = false;
    }

    /// The twin of `enable_settler_backlog_brake`.
    pub fn disable_settler_backlog_brake(&mut self) {
        self.settler_backlog_brake = false;
        self.base.settler_backlog_brake = false;
    }

    /// Preserve V1's full stop for a mature empire of six or more cities, but
    /// leave the ordinary pipeline alone during the high-leverage
    /// three-to-five-city growth wave. Version two of
    /// `settler-backlog-brake`, so enabling it turns V1 off.
    pub fn enable_settler_backlog_brake_2(&mut self) {
        self.settler_backlog_brake = false;
        self.base.settler_backlog_brake = false;
        self.settler_backlog_brake_2 = true;
        self.base.settler_backlog_brake_2 = true;
    }

    /// The twin of `enable_settler_backlog_brake_2`.
    pub fn disable_settler_backlog_brake_2(&mut self) {
        self.settler_backlog_brake_2 = false;
        self.base.settler_backlog_brake_2 = false;
    }

    /// A city grown to its housing (pop + 1 ≥ housing) with no Granary starts
    /// one ahead of the argmax. See `AdvancedAi::first_granary_reserve`.
    pub fn enable_first_granary_reserve(&mut self) {
        self.first_granary_reserve = true;
        self.first_granary_reserve_2 = false;
        self.first_granary_reserve_3 = false;
        self.base.housing_reserve = false;
    }

    /// The twin of `enable_first_granary_reserve`.
    pub fn disable_first_granary_reserve(&mut self) {
        self.first_granary_reserve = false;
    }

    pub fn enable_first_granary_reserve_2(&mut self) {
        self.first_granary_reserve = false;
        self.first_granary_reserve_2 = true;
        self.first_granary_reserve_3 = false;
        self.base.housing_reserve = false;
    }

    pub fn disable_first_granary_reserve_2(&mut self) {
        self.first_granary_reserve_2 = false;
    }

    /// Research the cheapest technology that connects an owned, unimproved
    /// luxury (Irrigation, Sailing) ahead of the lane's beeline, once the
    /// opening techs are in. See `AdvancedAi::unconnected_luxury_tech`.
    pub fn enable_connect_the_luxury(&mut self) {
        self.connect_the_luxury = true;
        self.connect_the_luxury_2 = false;
    }

    /// The twin of `enable_connect_the_luxury`.
    pub fn disable_connect_the_luxury(&mut self) {
        self.connect_the_luxury = false;
    }

    /// In peacetime, keep the seat's military power at four fifths of the
    /// strongest bordering major's by buying the contact city's ranged
    /// defender with Gold above a reserve. See
    /// `AdvancedAi::border_parity_purchase`.
    pub fn enable_border_parity(&mut self) {
        self.border_parity = true;
        self.border_parity_2 = false;
        self.border_parity_3 = false;
    }

    /// The twin of `enable_border_parity`.
    pub fn disable_border_parity(&mut self) {
        self.border_parity = false;
    }

    /// One to four era points short of a Normal Age, patronize any Great
    /// Person the bank can carry for its Historic Moment. See
    /// `AdvancedAi::era_points_short`.
    pub fn enable_age_closer(&mut self) {
        self.age_closer = true;
        self.age_closer_2 = false;
    }

    /// The twin of `enable_age_closer`.
    pub fn disable_age_closer(&mut self) {
        self.age_closer = false;
    }

    /// Research a prerequisite-met technology whose Eureka is in hand and
    /// whose remaining cost is at most two turns of science before the lane's
    /// beeline resumes. See `AdvancedAi::boosted_bargain_tech`.
    pub fn enable_boosted_bargain_first(&mut self) {
        self.boosted_bargain_first = true;
        self.boosted_bargain_first_2 = false;
        self.boosted_bargain_first_3 = false;
    }

    /// The twin of `enable_boosted_bargain_first`.
    pub fn disable_boosted_bargain_first(&mut self) {
        self.boosted_bargain_first = false;
    }

    /// Let a one-turn boosted technology break a close ordinary fallback
    /// decision after every forced lane goal stands down. See
    /// `AdvancedAi::boosted_bargain_tech_2`.
    pub fn enable_boosted_bargain_first_2(&mut self) {
        self.boosted_bargain_first_2 = true;
        self.boosted_bargain_first = false;
        self.boosted_bargain_first_3 = false;
    }

    /// The twin of `enable_boosted_bargain_first_2`.
    pub fn disable_boosted_bargain_first_2(&mut self) {
        self.boosted_bargain_first_2 = false;
    }

    /// `boosted-bargain-first-3`: a one-turn boosted technology may
    /// interrupt a peaceful lane beeline that is not about to land its
    /// target; the other versions stand down. See
    /// `AdvancedAi::boosted_bargain_tech_3`.
    pub fn enable_boosted_bargain_first_3(&mut self) {
        self.boosted_bargain_first_3 = true;
        self.boosted_bargain_first = false;
        self.boosted_bargain_first_2 = false;
    }

    /// The twin of `enable_boosted_bargain_first_3`.
    pub fn disable_boosted_bargain_first_3(&mut self) {
        self.boosted_bargain_first_3 = false;
    }

    /// Version two of `border-parity`: the same target and Gold purchase,
    /// and when the treasury cannot pay, the contact city's idle queue starts
    /// the defender. See `AdvancedAi::border_parity_target`.
    pub fn enable_border_parity_2(&mut self) {
        self.border_parity = false;
        self.border_parity_2 = true;
        self.border_parity_3 = false;
    }

    /// The twin of `enable_border_parity_2`.
    pub fn disable_border_parity_2(&mut self) {
        self.border_parity_2 = false;
    }

    /// The civilian capture envelope counts every at-war owner and keeps
    /// pricing a hostile the seat has seen for a few turns after it walks
    /// back into the fog. See `AdvancedAi::barbarian_reach`.
    pub fn enable_hostile_memory(&mut self) {
        self.hostile_memory = true;
        self.hostile_memory_2 = false;
        self.hostile_memory_3 = false;
    }

    /// Version two retains civilian memory and prices a land escort's embarkation.
    pub fn enable_hostile_memory_2(&mut self) {
        self.hostile_memory = false;
        self.hostile_memory_2 = true;
        self.hostile_memory_3 = false;
    }

    /// Version three also revises stale sightings when their forecast area
    /// is fully visible, using only the unit facts recorded at observation.
    pub fn enable_hostile_memory_3(&mut self) {
        self.hostile_memory = false;
        self.hostile_memory_2 = false;
        self.hostile_memory_3 = true;
    }

    pub fn disable_hostile_memory_3(&mut self) {
        self.hostile_memory_3 = false;
    }

    pub fn disable_hostile_memory_2(&mut self) {
        self.hostile_memory_2 = false;
    }

    /// The twin of `enable_hostile_memory`.
    pub fn disable_hostile_memory(&mut self) {
        self.hostile_memory = false;
    }

    /// `live-move-refusal-break` (HostOnly): stop re-issuing a move the host
    /// keeps refusing. `BasicAi` records the first pathed step issued to each
    /// unit per turn; a unit seen on the same tile two judged turns running
    /// with the same issued step gets that step barred for eight standard
    /// turns (`judge_move_refusals`), and a frozen Settler additionally has
    /// its destination retired through the dead-site machinery. Measured
    /// motive: 13.9% of all orders were `did_not_move`, with one settler
    /// re-ordered to the identical tile eleven straight turns before capture.
    pub fn enable_live_move_refusal_break(&mut self) {
        self.live_move_refusal_break = true;
        self.base.move_refusal_break = true;
    }

    /// Withholding twin for `enable_live_move_refusal_break`.
    pub fn disable_live_move_refusal_break(&mut self) {
        self.live_move_refusal_break = false;
        self.base.move_refusal_break = false;
    }

    /// `district-planning-3`: retain the joint district-site plan, but make
    /// a Gold purchase only for its highest-value unowned site when that city
    /// is idle and can start the district next. The full working reserve stays
    /// intact. One family version plays at a time.
    pub fn enable_district_planning_3(&mut self) {
        self.district_planning = false;
        self.district_planning_3 = true;
    }

    /// The twin of `enable_district_planning_3`.
    pub fn disable_district_planning_3(&mut self) {
        self.district_planning_3 = false;
    }

    /// Version 2 of `air_surge`: the science–domination loop. The original
    /// one-appointment surge remains a separately measurable family member;
    /// this continuation lets the Formal-War clock run through the buildout,
    /// prices a follow-up surge by the package still missing so the loop
    /// repeats, and keeps the Campus priced under Conquest. One version of
    /// the family plays, so this turns version 1 off. Opt-in gene
    /// `air-surge-2`. (Filed above the letter markers like every toggle pair
    /// here — the append-point guard reads a method line's first identifier.)
    pub fn enable_air_surge_2(&mut self) {
        self.air_surge = false;
        self.air_surge_2 = true;
    }

    /// The twin of `enable_air_surge_2`.
    pub fn disable_air_surge_2(&mut self) {
        self.air_surge_2 = false;
    }

    /// `siege-preempts-the-queue`: a raider on a city's doorstep is answered
    /// with a body before anything else is built, bought when no defender
    /// exists, and a recon unit is not a defender. A `BasicAi` flag; see
    /// `advanced/siege_response.rs`.
    pub fn enable_siege_preempts_the_queue(&mut self) {
        self.base.enable_siege_preempts_the_queue();
    }

    /// The twin of `enable_siege_preempts_the_queue`.
    pub fn disable_siege_preempts_the_queue(&mut self) {
        self.base.disable_siege_preempts_the_queue();
    }

    /// Version two of rapid city expansion: aim at the measured five-city
    /// opening band without the culled version one's immediate fifteen-city
    /// order, non-empty queue preemption, closest-site override,
    /// founding-pantheon override, or automatic conquest pivot.
    pub fn enable_rapid_city_expansion_2(&mut self) {
        self.rapid_city_expansion_2 = true;
        self.base.enable_rapid_city_expansion_2();
    }

    /// The twin of `enable_rapid_city_expansion_2`.
    pub fn disable_rapid_city_expansion_2(&mut self) {
        self.rapid_city_expansion_2 = false;
        self.base.disable_rapid_city_expansion_2();
    }

    /// Version three of `never-an-empty-queue`: tolerate one transient empty
    /// turn, then recover a persistent stall with a civilian candidate above
    /// the hard veto. Enabling it selects this family version exclusively.
    pub fn enable_never_an_empty_queue_3(&mut self) {
        self.never_an_empty_queue = false;
        self.never_an_empty_queue_2 = false;
        self.never_an_empty_queue_3 = true;
    }

    /// The twin of `enable_never_an_empty_queue_3`.
    pub fn disable_never_an_empty_queue_3(&mut self) {
        self.never_an_empty_queue_3 = false;
    }

    /// At the ready strike, invite one credible neighbour to a joint war and
    /// wait no more than one turn. Credible means the target is close to a
    /// victory or the partner has the grievance and combined power that make
    /// the Basic controller accept. Before the strike it asks that partner
    /// only for a military alliance and spends Envoy score only to unseat the
    /// target from a nearby client; it never retries. Opt-in gene
    /// `coalition-before-war-2`.
    pub fn enable_coalition_before_war_2(&mut self) {
        self.coalition_before_war_2 = true;
        self.coalition_before_war = false;
        self.coalition_before_war_3 = false;
    }

    /// The twin of `enable_coalition_before_war_2`.
    pub fn disable_coalition_before_war_2(&mut self) {
        self.coalition_before_war_2 = false;
    }

    /// Recruit only a target neighbour already fighting it. The accepted
    /// military alliance makes that real second front an immediate combat
    /// bonus when we declare; this version neither diverts Envoys nor holds
    /// a ready declaration for a speculative joint-war answer. Opt-in gene
    /// `coalition-before-war-3`.
    pub fn enable_coalition_before_war_3(&mut self) {
        self.coalition_before_war_3 = true;
        self.coalition_before_war = false;
        self.coalition_before_war_2 = false;
    }

    /// The twin of `enable_coalition_before_war_3`.
    pub fn disable_coalition_before_war_3(&mut self) {
        self.coalition_before_war_3 = false;
    }

    /// Restrain an off-lane repeatable Great-Person project only while its
    /// district owes a first building this city can start now. Lane projects
    /// and immediate race swings remain available. Version 2 of
    /// `early-project-restraint`; one family member plays at a time.
    pub fn enable_early_project_restraint_2(&mut self) {
        self.early_project_restraint = false;
        self.early_project_restraint_2 = true;
    }

    /// The twin of `enable_early_project_restraint_2`.
    pub fn disable_early_project_restraint_2(&mut self) {
        self.early_project_restraint_2 = false;
    }

    /// Version three of `border-parity`: fill one local garrison debt only
    /// when two visible non-recon land bodies from the same peaceful major
    /// are staged beside the city. Buy one answer without touching production.
    /// Enabling it selects this family version.
    pub fn enable_border_parity_3(&mut self) {
        self.border_parity = false;
        self.border_parity_2 = false;
        self.border_parity_3 = true;
    }

    /// The twin of `enable_border_parity_3`.
    pub fn disable_border_parity_3(&mut self) {
        self.border_parity_3 = false;
    }

    /// Version two of `battle_planner`: the positions plan — slots laid
    /// against the enemy contact and the objective, a minimum-cost
    /// assignment with reservations, moves front to rear, pacing on the
    /// approach — joins the kill plan and the heal rotation. One version of
    /// a family plays, so this turns version one off. Opt-in gene
    /// `battle-planner-2`. See `battle_planner`.
    pub fn enable_battle_planner_2(&mut self) {
        self.battle_planner = false;
        self.battle_planner_2 = true;
    }

    /// The twin of `enable_battle_planner_2`.
    pub fn disable_battle_planner_2(&mut self) {
        self.battle_planner_2 = false;
    }

    /// `battle-planner-3`: version three of `battle_planner` — the siege's
    /// taker left to the siege, the host's strike preview over the closed
    /// form, and the previews asked for. One version of a family plays, so
    /// this turns versions one and two off. See `battle_planner`.
    pub fn enable_battle_planner_3(&mut self) {
        self.battle_planner = false;
        self.battle_planner_2 = false;
        self.battle_planner_3 = true;
    }

    /// The twin of `enable_battle_planner_3`.
    pub fn disable_battle_planner_3(&mut self) {
        self.battle_planner_3 = false;
    }

    /// Enter the secondary Great Prophet race only when the board-aware
    /// religious-opening rank admits this seat: two cities, a real Holy Site
    /// path, and one of the remaining global slots. Astrology after the
    /// opening techs, the empire's first Holy Site at the front of the
    /// district order, the Prophet priced as a lane great person, and
    /// `pursue_religion` for the prize; an explicit Science lane stays on its
    /// pure beeline. Version 2 of `enter-the-prophet-race`; version 1 left
    /// the code on 2026-09-09. See `enter_the_prophet_race_2`.
    pub fn enable_enter_the_prophet_race_2(&mut self) {
        self.enter_the_prophet_race_2 = true;
    }

    /// The twin of `enable_enter_the_prophet_race_2`.
    pub fn disable_enter_the_prophet_race_2(&mut self) {
        self.enter_the_prophet_race_2 = false;
        self.base.enter_prophet_race = false;
    }

    /// Until the development half ends, an assigned lane's cities take the
    /// unassigned seat's production dispatch. Opt-in gene
    /// `lane-delegates-production`.
    pub fn enable_lane_delegates_production(&mut self) {
        self.lane_delegates_production = true;
        self.lane_delegates_production_2 = false;
    }
    /// The twin of `enable_lane_delegates_production`.
    pub fn disable_lane_delegates_production(&mut self) {
        self.lane_delegates_production = false;
    }
    /// Version two: an assigned lane's cities take the unassigned seat's
    /// production dispatch for the whole game. Opt-in gene
    /// `lane-delegates-production-2`.
    pub fn enable_lane_delegates_production_2(&mut self) {
        self.lane_delegates_production = false;
        self.lane_delegates_production_2 = true;
    }
    /// The twin of `enable_lane_delegates_production_2`.
    pub fn disable_lane_delegates_production_2(&mut self) {
        self.lane_delegates_production_2 = false;
    }

    /// Version two keeps V1's field-front swap and adds a roll-top lethal
    /// rotation when the wounded destination is safer and the relief survives.
    /// One family version plays at a time.
    pub fn enable_swap_rotation_2(&mut self) {
        self.swap_rotation = false;
        self.swap_rotation_2 = true;
    }

    /// The twin of `enable_swap_rotation_2`.
    pub fn disable_swap_rotation_2(&mut self) {
        self.swap_rotation_2 = false;
    }

    /// Turn an existing Conquest plan into one nearby, holdable city only when
    /// its complete spare-included bill is already staged. This successor
    /// replaces v1's multi-city expedition, retains a single objective while
    /// it stays feasible, and abandons an unlaunched opportunity quickly.
    /// Opt-in gene `city-campaign-2`; see `advanced/city_campaign.rs`.
    pub fn enable_city_campaign_2(&mut self) {
        self.city_campaign = false;
        self.city_campaign_2 = true;
    }

    /// The twin of `enable_city_campaign_2`.
    pub fn disable_city_campaign_2(&mut self) {
        self.city_campaign_2 = false;
    }

    /// `doomed-blow-veto`: a unit whose every blow this turn would leave it
    /// dead on the enemy's next turn — the return damage plus the danger
    /// field at the stand against its hit points — is taken from the ladder,
    /// which would attack with it, and given to the heal rotation as an
    /// exposed unit. See `battle_planner`.
    pub fn enable_doomed_blow_veto(&mut self) {
        self.doomed_blow_veto = true;
        self.doomed_blow_veto_2 = false;
    }

    /// Price survival after the complete friendly strike sequence.
    pub fn enable_doomed_blow_veto_2(&mut self) {
        self.doomed_blow_veto = false;
        self.doomed_blow_veto_2 = true;
    }

    /// Disable version two without enabling another family version.
    pub fn disable_doomed_blow_veto_2(&mut self) {
        self.doomed_blow_veto_2 = false;
    }

    /// The twin of `enable_doomed_blow_veto`.
    pub fn disable_doomed_blow_veto(&mut self) {
        self.doomed_blow_veto = false;
    }

    /// Deny a rival the science victory rather than only race it: the
    /// diplomatic refusals, the denunciation and one spy's disruption of the
    /// launch pad. See `advanced/science_threat_denial.rs`. Opt-in gene
    /// `science-threat-denial`. Filed here rather than under a marker: the
    /// append-point check reads a method line's first identifier.
    pub fn enable_science_threat_denial(&mut self) {
        self.science_threat_denial = true;
    }

    /// The twin of `enable_science_threat_denial`. The war rung's own flag
    /// is left as it is; it is inert without this one.
    pub fn disable_science_threat_denial(&mut self) {
        self.science_threat_denial = false;
    }

    /// The pad raid and the bounded war that opens it, on top of the base
    /// denial. Opt-in gene `science-denial-war`; it REQUIRES
    /// `science-threat-denial` and arms it, so the tag fires on its own and a
    /// seat that drew the war without the base still plays the whole ladder.
    /// See `advanced/science_threat_denial.rs`.
    pub fn enable_science_denial_war(&mut self) {
        self.science_threat_denial = true;
        self.science_denial_war = true;
    }

    /// The twin of `enable_science_denial_war`: the base stays as it was.
    pub fn disable_science_denial_war(&mut self) {
        self.science_denial_war = false;
    }

    /// Unlock the first land siege capability for a walled Conquest objective.
    pub fn enable_domination_siege_research(&mut self) {
        self.domination_siege_research = true;
        self.domination_siege_research_2 = false;
    }
    pub fn disable_domination_siege_research(&mut self) {
        self.domination_siege_research = false;
    }

    /// Version one, and the next stronger buildable land siege design once
    /// the target's walls reach the Medieval tier.
    pub fn enable_domination_siege_research_2(&mut self) {
        self.domination_siege_research = false;
        self.domination_siege_research_2 = true;
    }
    /// The twin of `enable_domination_siege_research_2`.
    pub fn disable_domination_siege_research_2(&mut self) {
        self.domination_siege_research_2 = false;
    }

    /// See `BasicAi::district_buildings_first`: a standing district's first
    /// building before the delegated city governor opens another district.
    pub fn enable_district_buildings_first(&mut self) {
        self.district_buildings_first = true;
        self.district_buildings_first_2 = false;
        self.base.district_buildings_first = true;
        self.base.capital_library_first = false;
    }
    /// Enable combat preservation and full-health recovery.
    pub fn enable_unit_preservation(&mut self) {
        self.unit_preservation = true;
    }
    /// Withhold combat preservation for paired evaluation.
    pub fn disable_unit_preservation(&mut self) {
        self.unit_preservation = false;
    }

    /// The twin of `enable_district_buildings_first`.
    pub fn disable_district_buildings_first(&mut self) {
        self.district_buildings_first = false;
        self.base.district_buildings_first = self.district_buildings_first_2;
    }

    /// See `BasicAi::capital_library_first`: version one, and the capital's
    /// Library ahead of its next Settler once it holds a Campus.
    pub fn enable_district_buildings_first_2(&mut self) {
        self.district_buildings_first = false;
        self.district_buildings_first_2 = true;
        self.base.district_buildings_first = true;
        self.base.capital_library_first = true;
    }

    /// The twin of `enable_district_buildings_first_2`.
    pub fn disable_district_buildings_first_2(&mut self) {
        self.district_buildings_first_2 = false;
        self.base.district_buildings_first = self.district_buildings_first;
        self.base.capital_library_first = false;
    }

    /// See `BasicAi::monument_first`: a city's Monument ahead of the military
    /// floor and the Settler step in the delegated city governor.
    pub fn enable_monument_first(&mut self) {
        self.monument_first = true;
        self.base.monument_first = true;
    }

    /// The twin of `enable_monument_first`.
    pub fn disable_monument_first(&mut self) {
        self.monument_first = false;
        self.base.monument_first = false;
    }

    /// See `BasicAi::culture_defense_theater`: a Theater Square while the
    /// empire's Culture trails the strongest rival's.
    pub fn enable_culture_defense_theater(&mut self) {
        self.culture_defense_theater = true;
        self.base.culture_defense_theater = true;
    }

    /// The twin of `enable_culture_defense_theater`.
    pub fn disable_culture_defense_theater(&mut self) {
        self.culture_defense_theater = false;
        self.base.culture_defense_theater = self.culture_defense_theater_2;
    }

    /// See `AdvancedAi::culture_defense_theater_2`: version 1, but a
    /// housing-bound city builds its Granary before the reservation's Theater.
    pub fn enable_culture_defense_theater_2(&mut self) {
        self.culture_defense_theater_2 = true;
        self.base.culture_defense_theater = true;
    }

    /// The twin of `enable_culture_defense_theater_2`.
    pub fn disable_culture_defense_theater_2(&mut self) {
        self.culture_defense_theater_2 = false;
        self.base.culture_defense_theater = self.culture_defense_theater;
    }

    /// See `BasicAi::campus_before_harbor`: a city's first Campus before its
    /// Harbor in the delegated city governor.
    pub fn enable_campus_before_harbor(&mut self) {
        self.campus_before_harbor = true;
        self.campus_before_harbor_2 = false;
        self.base.campus_before_harbor = true;
        self.base.capital_campus_first = false;
    }

    /// The twin of `enable_campus_before_harbor`.
    pub fn disable_campus_before_harbor(&mut self) {
        self.campus_before_harbor = false;
        self.base.campus_before_harbor = self.campus_before_harbor_2;
    }

    /// See `BasicAi::capital_campus_first`: version one, and the capital's
    /// first Campus before its next Settler.
    pub fn enable_campus_before_harbor_2(&mut self) {
        self.campus_before_harbor = false;
        self.campus_before_harbor_2 = true;
        self.base.campus_before_harbor = true;
        self.base.capital_campus_first = true;
    }

    /// The twin of `enable_campus_before_harbor_2`.
    pub fn disable_campus_before_harbor_2(&mut self) {
        self.campus_before_harbor_2 = false;
        self.base.campus_before_harbor = self.campus_before_harbor;
        self.base.capital_campus_first = false;
    }

    /// See `AdvancedAi::first_granary_reserve_3`: a housing-bound city's
    /// Granary, then its Aqueduct, in both production governors.
    pub fn enable_first_granary_reserve_3(&mut self) {
        self.first_granary_reserve = false;
        self.first_granary_reserve_2 = false;
        self.first_granary_reserve_3 = true;
        self.base.housing_reserve = true;
    }

    /// The twin of `enable_first_granary_reserve_3`.
    pub fn disable_first_granary_reserve_3(&mut self) {
        self.first_granary_reserve_3 = false;
        self.base.housing_reserve = false;
    }

    /// See `AdvancedAi::maintenance_relief_held` and
    /// `AdvancedAi::liberalism_repair_bar`: the deck keeps its maintenance
    /// relief and its Amenity repair card through the turn's reshuffles.
    pub fn enable_policy_deck_hysteresis(&mut self) {
        self.policy_deck_hysteresis = true;
        self.policy_deck_hysteresis_2 = false;
    }

    /// The twin of `enable_policy_deck_hysteresis`.
    pub fn disable_policy_deck_hysteresis(&mut self) {
        self.policy_deck_hysteresis = false;
    }

    /// See `AdvancedAi::relief_outruns_income`: version 1, and the relief
    /// holds while the income does not cover it and may evict a lower-ranked
    /// wanted military card. Exclusive with version 1.
    pub fn enable_policy_deck_hysteresis_2(&mut self) {
        self.policy_deck_hysteresis = false;
        self.policy_deck_hysteresis_2 = true;
    }

    /// The twin of `enable_policy_deck_hysteresis_2`.
    pub fn disable_policy_deck_hysteresis_2(&mut self) {
        self.policy_deck_hysteresis_2 = false;
    }

    /// See `BasicAi::builder_before_the_army`: the delegated city governor's
    /// Builder quota ahead of the Monument, the Settler and the military floor.
    pub fn enable_builder_before_the_army(&mut self) {
        self.disable_builder_before_the_army_2();
        self.disable_builder_before_the_army_3();
        self.builder_before_the_army = true;
        self.base.builder_before_the_army = true;
    }

    /// The twin of `enable_builder_before_the_army`.
    pub fn disable_builder_before_the_army(&mut self) {
        self.builder_before_the_army = false;
        self.base.builder_before_the_army = false;
    }

    /// See `BasicAi::builder_before_the_army_2`: version 1's step, only while
    /// the empire has no Builder standing or queued. Exclusive with version 1.
    pub fn enable_builder_before_the_army_2(&mut self) {
        self.disable_builder_before_the_army();
        self.disable_builder_before_the_army_3();
        self.builder_before_the_army_2 = true;
        self.base.builder_before_the_army_2 = true;
    }

    /// The twin of `enable_builder_before_the_army_2`.
    pub fn disable_builder_before_the_army_2(&mut self) {
        self.builder_before_the_army_2 = false;
        self.base.builder_before_the_army_2 = false;
    }

    /// See `BasicAi::campus_before_the_army`: a city's first Campus ahead of
    /// the military floor and the capital Settler.
    pub fn enable_campus_before_the_army(&mut self) {
        self.disable_campus_before_the_army_2();
        self.disable_campus_before_the_army_3();
        self.campus_before_the_army = true;
        self.base.campus_before_the_army = true;
    }

    /// The twin of `enable_campus_before_the_army`.
    pub fn disable_campus_before_the_army(&mut self) {
        self.campus_before_the_army = false;
        self.base.campus_before_the_army = false;
    }

    /// See `BasicAi::campus_before_the_army_2`: version 1's Campus and Library
    /// behind the Monument and the capital Settler, ahead of the military
    /// floor only. Exclusive with version 1.
    pub fn enable_campus_before_the_army_2(&mut self) {
        self.disable_campus_before_the_army();
        self.disable_campus_before_the_army_3();
        self.campus_before_the_army_2 = true;
        self.base.campus_before_the_army_2 = true;
    }

    /// The twin of `enable_campus_before_the_army_2`.
    pub fn disable_campus_before_the_army_2(&mut self) {
        self.campus_before_the_army_2 = false;
        self.base.campus_before_the_army_2 = false;
    }

    /// See `BasicAi::campus_before_the_army_3`: version 2, on through the
    /// University and the Research Lab. Exclusive with versions 1 and 2.
    pub fn enable_campus_before_the_army_3(&mut self) {
        self.disable_campus_before_the_army();
        self.disable_campus_before_the_army_2();
        self.campus_before_the_army_3 = true;
        self.base.campus_before_the_army_3 = true;
    }

    /// The twin of `enable_campus_before_the_army_3`.
    pub fn disable_campus_before_the_army_3(&mut self) {
        self.campus_before_the_army_3 = false;
        self.base.campus_before_the_army_3 = false;
    }

    /// See `BasicAi::settler_before_the_navy`: the navy step yields to a due
    /// Settler.
    pub fn enable_settler_before_the_navy(&mut self) {
        self.settler_before_the_navy = true;
        self.base.settler_before_the_navy = true;
    }

    /// The twin of `enable_settler_before_the_navy`.
    pub fn disable_settler_before_the_navy(&mut self) {
        self.settler_before_the_navy = false;
        self.base.settler_before_the_navy = false;
    }

    /// See `BasicAi::granary_before_the_army`: a housing-bound city's Granary,
    /// else its Aqueduct, ahead of the military floor.
    pub fn enable_granary_before_the_army(&mut self) {
        self.disable_granary_before_the_army_2();
        self.granary_before_the_army = true;
        self.base.granary_before_the_army = true;
    }

    /// The twin of `enable_granary_before_the_army`.
    pub fn disable_granary_before_the_army(&mut self) {
        self.granary_before_the_army = false;
        self.base.granary_before_the_army = false;
    }

    /// See `BasicAi::industry_before_the_army`: the Industrial Zone, its
    /// Workshop and its Factory ahead of the military floor.
    pub fn enable_industry_before_the_army(&mut self) {
        self.disable_industry_before_the_army_2();
        self.disable_industry_before_the_army_3();
        self.industry_before_the_army = true;
        self.base.industry_before_the_army = true;
    }

    /// The twin of `enable_industry_before_the_army`.
    pub fn disable_industry_before_the_army(&mut self) {
        self.industry_before_the_army = false;
        self.base.industry_before_the_army = false;
    }

    /// See `BasicAi::industry_before_the_army_2`: version 1 keeping a slot
    /// for the Theater and five Gold a turn. Exclusive with version 1.
    pub fn enable_industry_before_the_army_2(&mut self) {
        self.disable_industry_before_the_army();
        self.disable_industry_before_the_army_3();
        self.industry_before_the_army_2 = true;
        self.base.industry_before_the_army_2 = true;
    }

    /// The twin of `enable_industry_before_the_army_2`.
    pub fn disable_industry_before_the_army_2(&mut self) {
        self.industry_before_the_army_2 = false;
        self.base.industry_before_the_army_2 = false;
    }

    /// See `BasicAi::industry_in_the_district_list`.
    pub fn enable_industry_in_the_district_list(&mut self) {
        self.industry_in_the_district_list = true;
        self.base.industry_in_the_district_list = true;
    }

    /// The twin of `enable_industry_in_the_district_list`.
    pub fn disable_industry_in_the_district_list(&mut self) {
        self.industry_in_the_district_list = false;
        self.base.industry_in_the_district_list = false;
    }

    /// See `BasicAi::builder_before_the_army_3`: version 2's first Builder,
    /// then one per three unimproved worked tiles behind the Campus step and
    /// a due Settler. Exclusive with versions 1 and 2.
    pub fn enable_builder_before_the_army_3(&mut self) {
        self.disable_builder_before_the_army();
        self.disable_builder_before_the_army_2();
        self.builder_before_the_army_3 = true;
        self.base.builder_before_the_army_3 = true;
    }

    /// The twin of `enable_builder_before_the_army_3`.
    pub fn disable_builder_before_the_army_3(&mut self) {
        self.builder_before_the_army_3 = false;
        self.base.builder_before_the_army_3 = false;
    }

    /// See `BasicAi::industry_before_the_army_3`: version 2 only while the
    /// army meets its floor. Exclusive with versions 1 and 2.
    pub fn enable_industry_before_the_army_3(&mut self) {
        self.disable_industry_before_the_army();
        self.disable_industry_before_the_army_2();
        self.industry_before_the_army_3 = true;
        self.base.industry_before_the_army_3 = true;
    }

    /// The twin of `enable_industry_before_the_army_3`.
    pub fn disable_industry_before_the_army_3(&mut self) {
        self.industry_before_the_army_3 = false;
        self.base.industry_before_the_army_3 = false;
    }

    /// See `BasicAi::industrial_hub`.
    pub fn enable_industrial_hub(&mut self) {
        self.industrial_hub = true;
        self.base.industrial_hub = true;
    }

    /// The twin of `enable_industrial_hub`.
    pub fn disable_industrial_hub(&mut self) {
        self.industrial_hub = false;
        self.base.industrial_hub = false;
    }

    /// Opt-in gene `colonization-earns-its-slot-2`; see
    /// [`Self::colonization_earns_its_slot_2`]. Exclusive with version 1.
    pub fn enable_colonization_earns_its_slot_2(&mut self) {
        self.disable_colonization_earns_its_slot();
        self.colonization_earns_its_slot_2 = true;
    }

    /// The twin of `enable_colonization_earns_its_slot_2`.
    pub fn disable_colonization_earns_its_slot_2(&mut self) {
        self.colonization_earns_its_slot_2 = false;
    }

    /// See `BasicAi::granary_before_the_army_2`: version 1's housing reserve
    /// behind the Campus step and the Builder backlog. Exclusive with
    /// version 1.
    pub fn enable_granary_before_the_army_2(&mut self) {
        self.disable_granary_before_the_army();
        self.granary_before_the_army_2 = true;
        self.base.granary_before_the_army_2 = true;
    }

    /// The twin of `enable_granary_before_the_army_2`.
    pub fn disable_granary_before_the_army_2(&mut self) {
        self.granary_before_the_army_2 = false;
        self.base.granary_before_the_army_2 = false;
    }

    /// Opt-in gene `colonization-earns-its-slot`; see
    /// [`Self::colonization_earns_its_slot`].
    pub fn enable_colonization_earns_its_slot(&mut self) {
        self.disable_colonization_earns_its_slot_2();
        self.colonization_earns_its_slot = true;
    }

    /// The twin of `enable_colonization_earns_its_slot`.
    pub fn disable_colonization_earns_its_slot(&mut self) {
        self.colonization_earns_its_slot = false;
    }

    /// Opt-in gene `prophet-race-takes-a-district-slot`; see
    /// [`Self::prophet_race_takes_a_district_slot`].
    pub fn enable_prophet_race_takes_a_district_slot(&mut self) {
        self.prophet_race_takes_a_district_slot = true;
        self.base.prophet_race_takes_a_district_slot = true;
    }

    /// The twin of `enable_prophet_race_takes_a_district_slot`.
    pub fn disable_prophet_race_takes_a_district_slot(&mut self) {
        self.prophet_race_takes_a_district_slot = false;
        self.base.prophet_race_takes_a_district_slot = false;
    }

    /// Opt-in gene `prophet-race-takes-a-district-slot-2`; see
    /// [`Self::prophet_race_takes_a_district_slot_2`].
    pub fn enable_prophet_race_takes_a_district_slot_2(&mut self) {
        self.prophet_race_takes_a_district_slot_2 = true;
        self.base.prophet_race_takes_a_district_slot_2 = true;
    }

    /// The twin of `enable_prophet_race_takes_a_district_slot_2`.
    pub fn disable_prophet_race_takes_a_district_slot_2(&mut self) {
        self.prophet_race_takes_a_district_slot_2 = false;
        self.base.prophet_race_takes_a_district_slot_2 = false;
    }

    /// Opt-in gene `denial-keeps-its-rival`; see
    /// [`Self::denial_keeps_its_rival`].
    pub fn enable_denial_keeps_its_rival(&mut self) {
        self.denial_keeps_its_rival = true;
    }

    /// The twin of `enable_denial_keeps_its_rival`.
    pub fn disable_denial_keeps_its_rival(&mut self) {
        self.denial_keeps_its_rival = false;
        self.denial_incumbent = None;
    }

    /// Opt-in gene `own-column-is-not-a-refusal`; see
    /// [`Self::own_column_is_not_a_refusal`].
    pub fn enable_own_column_is_not_a_refusal(&mut self) {
        self.own_column_is_not_a_refusal = true;
        self.base.own_column_is_not_a_refusal = true;
    }

    /// The twin of `enable_own_column_is_not_a_refusal`.
    pub fn disable_own_column_is_not_a_refusal(&mut self) {
        self.own_column_is_not_a_refusal = false;
        self.base.own_column_is_not_a_refusal = false;
    }

    /// Opt-in gene `bleeding-capital-loyalty`; see
    /// [`Self::bleeding_capital_loyalty`].
    pub fn enable_bleeding_capital_loyalty(&mut self) {
        self.bleeding_capital_loyalty = true;
        self.base.victor_first_for_a_short_runway = true;
    }

    /// The twin of `enable_bleeding_capital_loyalty`.
    pub fn disable_bleeding_capital_loyalty(&mut self) {
        self.bleeding_capital_loyalty = false;
        self.base.victor_first_for_a_short_runway = false;
    }

    /// Opt-in gene `siege-keeps-a-shooter`; see
    /// [`Self::siege_keeps_a_shooter`].
    pub fn enable_siege_keeps_a_shooter(&mut self) {
        self.siege_keeps_a_shooter = true;
    }

    /// The twin of `enable_siege_keeps_a_shooter`.
    pub fn disable_siege_keeps_a_shooter(&mut self) {
        self.siege_keeps_a_shooter = false;
        self.siege_wall_keepers.clear();
    }

    /// See `BasicAi::plaza_in_the_district_list`.
    pub fn enable_plaza_in_the_district_list(&mut self) {
        self.plaza_in_the_district_list = true;
        self.base.plaza_in_the_district_list = true;
    }

    /// The twin of `enable_plaza_in_the_district_list`.
    pub fn disable_plaza_in_the_district_list(&mut self) {
        self.plaza_in_the_district_list = false;
        self.base.plaza_in_the_district_list = false;
    }

    /// Version 2 of `declaration_needs_the_edge`; one version of a family
    /// plays, so this turns version 1 off. Opt-in gene
    /// `declaration-needs-the-edge-2`. See
    /// `AdvancedAi::declaration_needs_the_edge_2`.
    pub fn enable_declaration_needs_the_edge_2(&mut self) {
        self.declaration_needs_the_edge = false;
        self.declaration_needs_the_edge_2 = true;
    }

    /// The twin of `enable_declaration_needs_the_edge_2`.
    pub fn disable_declaration_needs_the_edge_2(&mut self) {
        self.declaration_needs_the_edge_2 = false;
    }

    /// Opt-in gene `builders-before-the-lent-floor`; see
    /// [`Self::builders_before_the_lent_floor`].
    pub fn enable_builders_before_the_lent_floor(&mut self) {
        self.builders_before_the_lent_floor = true;
        self.base.builders_before_the_lent_floor = true;
    }

    /// The twin of `enable_builders_before_the_lent_floor`.
    pub fn disable_builders_before_the_lent_floor(&mut self) {
        self.builders_before_the_lent_floor = false;
        self.base.builders_before_the_lent_floor = false;
    }

    /// Opt-in gene `housing-bound-city-builds-its-granary`; see
    /// `BasicAi::housing_bound_city_builds_its_granary`.
    pub fn enable_housing_bound_city_builds_its_granary(&mut self) {
        self.housing_bound_city_builds_its_granary = true;
        self.base.housing_bound_city_builds_its_granary = true;
    }

    /// The twin of `enable_housing_bound_city_builds_its_granary`.
    pub fn disable_housing_bound_city_builds_its_granary(&mut self) {
        self.housing_bound_city_builds_its_granary = false;
        self.base.housing_bound_city_builds_its_granary = false;
    }

    /// Opt-in gene `commercial-hub-and-traders`; see
    /// `BasicAi::commercial_hub_and_traders` and
    /// [`Self::international_gold_route_premium`].
    pub fn enable_commercial_hub_and_traders(&mut self) {
        self.commercial_hub_and_traders = true;
        self.base.commercial_hub_and_traders = true;
    }

    /// The twin of `enable_commercial_hub_and_traders`.
    pub fn disable_commercial_hub_and_traders(&mut self) {
        self.commercial_hub_and_traders = false;
        self.base.commercial_hub_and_traders = false;
    }

    /// Opt-in gene `campus-buildings-first`; see
    /// `BasicAi::campus_buildings_first` and
    /// [`Self::reserve_culture_defense_theater`].
    pub fn enable_campus_buildings_first(&mut self) {
        self.campus_buildings_first = true;
        self.base.campus_buildings_first = true;
    }

    /// The twin of `enable_campus_buildings_first`.
    pub fn disable_campus_buildings_first(&mut self) {
        self.campus_buildings_first = false;
        self.base.campus_buildings_first = false;
    }

    /// Opt-in gene `industrial-zone-in-the-producers`; see
    /// `BasicAi::industrial_zone_in_the_producers`.
    pub fn enable_industrial_zone_in_the_producers(&mut self) {
        self.industrial_zone_in_the_producers = true;
        self.base.industrial_zone_in_the_producers = true;
    }

    /// The twin of `enable_industrial_zone_in_the_producers`.
    pub fn disable_industrial_zone_in_the_producers(&mut self) {
        self.industrial_zone_in_the_producers = false;
        self.base.industrial_zone_in_the_producers = false;
    }

    /// Opt-in gene `science-denial-spy-reads-the-leader`, which requires and
    /// arms `science-threat-denial`; see `advanced/science_threat_denial.rs`.
    pub fn enable_science_denial_spy_reads_the_leader(&mut self) {
        self.science_threat_denial = true;
        self.science_denial_spy_reads_the_leader = true;
    }

    /// The twin of `enable_science_denial_spy_reads_the_leader`: the base
    /// stays as it was.
    pub fn disable_science_denial_spy_reads_the_leader(&mut self) {
        self.science_denial_spy_reads_the_leader = false;
    }

    /// Opt-in gene `commercial-hub-in-the-strategic-queue`, which requires and
    /// arms `commercial-hub-and-traders`; see `BasicAi::commercial_hub_step`.
    pub fn enable_commercial_hub_in_the_strategic_queue(&mut self) {
        self.commercial_hub_and_traders = true;
        self.base.commercial_hub_and_traders = true;
        self.commercial_hub_in_the_strategic_queue = true;
    }

    /// The twin of `enable_commercial_hub_in_the_strategic_queue`: the base
    /// stays as it was.
    pub fn disable_commercial_hub_in_the_strategic_queue(&mut self) {
        self.commercial_hub_in_the_strategic_queue = false;
    }

    /// Opt-in gene `housing-cap-builds-the-aqueduct`; see
    /// `BasicAi::housing_cap_builds_the_aqueduct`.
    pub fn enable_housing_cap_builds_the_aqueduct(&mut self) {
        self.housing_cap_builds_the_aqueduct = true;
        self.base.housing_cap_builds_the_aqueduct = true;
    }

    /// The twin of `enable_housing_cap_builds_the_aqueduct`.
    pub fn disable_housing_cap_builds_the_aqueduct(&mut self) {
        self.housing_cap_builds_the_aqueduct = false;
        self.base.housing_cap_builds_the_aqueduct = false;
    }

    /// Opt-in gene `builders-cover-the-worked-backlog`; see
    /// `BasicAi::builders_cover_the_worked_backlog`.
    pub fn enable_builders_cover_the_worked_backlog(&mut self) {
        self.builders_cover_the_worked_backlog = true;
        self.base.builders_cover_the_worked_backlog = true;
    }

    /// The twin of `enable_builders_cover_the_worked_backlog`.
    pub fn disable_builders_cover_the_worked_backlog(&mut self) {
        self.builders_cover_the_worked_backlog = false;
        self.base.builders_cover_the_worked_backlog = false;
    }

    /// A water-locked Domination siege objective's guns and escorts cross as
    /// one group. Opt-in gene `amphibious-siege-train`; see
    /// `AdvancedAi::amphibious_hold_step`.
    pub fn enable_amphibious_siege_train(&mut self) {
        self.amphibious_siege_train = true;
    }

    /// The twin of `enable_amphibious_siege_train`; a crossing in hand is
    /// dropped unless version two still runs.
    pub fn disable_amphibious_siege_train(&mut self) {
        self.amphibious_siege_train = false;
        if !self.amphibious_siege_train_2 {
            self.amphibious_train = None;
        }
    }

    /// A water-locked siege's crossing fords a strait of one or two water
    /// tiles, locks its members, sails on water only and lands on a wider
    /// beach. Opt-in gene `amphibious-siege-train-2`, a superset of
    /// `amphibious-siege-train`, which it turns off so a seat plays one
    /// version; see `AdvancedAi::amphibious_hold_step`.
    pub fn enable_amphibious_siege_train_2(&mut self) {
        self.amphibious_siege_train = false;
        self.amphibious_siege_train_2 = true;
    }

    /// The twin of `enable_amphibious_siege_train_2`; a crossing in hand is
    /// dropped unless version one still runs.
    pub fn disable_amphibious_siege_train_2(&mut self) {
        self.amphibious_siege_train_2 = false;
        if !self.amphibious_siege_train {
            self.amphibious_train = None;
        }
    }

    /// Enable the separately measured staged Culture air-interception version.
    pub fn enable_elimination_yields_to_a_launch_2(&mut self) {
        self.elimination_yields_to_a_launch_2 = true;
    }

    /// Withhold the version-two interception path, retaining independent v1.
    pub fn disable_elimination_yields_to_a_launch_2(&mut self) {
        self.elimination_yields_to_a_launch_2 = false;
    }

    /// Opt-in gene `merchant-confederation-earns-its-slot`; the deck reading
    /// lives on `BasicAi`, the portfolio insert on `AdvancedAi`.
    pub fn enable_merchant_confederation_earns_its_slot(&mut self) {
        self.merchant_confederation_earns_its_slot = true;
        self.base.deck_reads_player_income = true;
    }

    /// The twin of `enable_merchant_confederation_earns_its_slot`.
    pub fn disable_merchant_confederation_earns_its_slot(&mut self) {
        self.merchant_confederation_earns_its_slot = false;
        self.base.deck_reads_player_income = false;
    }

    /// Opt-in gene `merchant-confederation-earns-its-slot-2`; version one
    /// plus the seat it may take. See `merchant_confederation_pays`.
    pub fn enable_merchant_confederation_earns_its_slot_2(&mut self) {
        self.merchant_confederation_earns_its_slot_2 = true;
        self.base.deck_reads_player_income = true;
    }

    /// The twin of `enable_merchant_confederation_earns_its_slot_2`.
    pub fn disable_merchant_confederation_earns_its_slot_2(&mut self) {
        self.merchant_confederation_earns_its_slot_2 = false;
        self.base.deck_reads_player_income = false;
    }

    /// Opt-in `capital-watch-arms-the-slinger`; see `advanced/capital_watch_slinger.rs`.
    pub fn enable_capital_watch_arms_the_slinger(&mut self) {
        self.capital_watch_arms_the_slinger = true;
    }

    /// The twin of `enable_capital_watch_arms_the_slinger`.
    pub fn disable_capital_watch_arms_the_slinger(&mut self) {
        self.capital_watch_arms_the_slinger = false;
        self.capital_watch_slinger_turn = None;
    }

    /// Opt-in gene `nuclear-goal-needs-a-launcher`; see
    /// [`Self::nuclear_goal_needs_a_launcher`].
    pub fn enable_nuclear_goal_needs_a_launcher(&mut self) {
        self.nuclear_goal_needs_a_launcher = true;
        self.base.nuclear_goal_needs_a_launcher = true;
    }

    /// The twin of `enable_nuclear_goal_needs_a_launcher`.
    pub fn disable_nuclear_goal_needs_a_launcher(&mut self) {
        self.nuclear_goal_needs_a_launcher = false;
        self.base.nuclear_goal_needs_a_launcher = false;
    }

    /// Measure the default-off forward airfield preparation project.
    pub fn enable_forward_airfield_project(&mut self) {
        self.forward_airfield_project = true;
    }

    /// Withhold the project and release its private reservation.
    pub fn disable_forward_airfield_project(&mut self) {
        self.forward_airfield_project = false;
        self.forward_airfield_project_state = None;
    }

    /// Let a siege short of its bill take the repeatable-project queues. See
    /// [`Self::siege_shortfall_claims_the_projects`].
    pub fn enable_siege_shortfall_claims_the_projects(&mut self) {
        self.siege_shortfall_claims_the_projects = true;
    }

    /// The twin of `enable_siege_shortfall_claims_the_projects`.
    pub fn disable_siege_shortfall_claims_the_projects(&mut self) {
        self.siege_shortfall_claims_the_projects = false;
        self.siege_force_short.clear();
    }

    /// Opt-in gene `culture-seat-theater-first`; see
    /// [`Self::culture_seat_theater_first`].
    pub fn enable_culture_seat_theater_first(&mut self) {
        self.culture_seat_theater_first = true;
        self.culture_seat_theater_first_2 = false;
    }

    /// The twin of `enable_culture_seat_theater_first`.
    pub fn disable_culture_seat_theater_first(&mut self) {
        self.culture_seat_theater_first = false;
    }

    /// Turn on `culture-keeps-every-border-open`. See
    /// [`Self::culture_keeps_every_border_open`].
    pub fn enable_culture_keeps_every_border_open(&mut self) {
        self.culture_keeps_every_border_open = true;
        self.culture_keeps_every_border_open_2 = false;
    }

    /// The twin of `enable_culture_keeps_every_border_open`.
    pub fn disable_culture_keeps_every_border_open(&mut self) {
        self.culture_keeps_every_border_open = false;
    }

    /// Opt-in gene `opening-builder-after-the-first-settler`; see
    /// `BasicAi::opening_builder_after_the_first_settler`.
    pub fn enable_opening_builder_after_the_first_settler(&mut self) {
        self.opening_builder_after_the_first_settler = true;
        self.base.enable_opening_builder_after_the_first_settler();
    }

    /// The twin of `enable_opening_builder_after_the_first_settler`.
    pub fn disable_opening_builder_after_the_first_settler(&mut self) {
        self.opening_builder_after_the_first_settler = false;
        self.base.disable_opening_builder_after_the_first_settler();
    }

    /// Opt-in gene `deck-prices-growth`; see
    /// `BasicAi::deck_prices_growth`.
    pub fn enable_deck_prices_growth(&mut self) {
        self.deck_prices_growth = true;
        self.base.enable_deck_prices_growth();
    }

    /// The twin of `enable_deck_prices_growth`.
    pub fn disable_deck_prices_growth(&mut self) {
        self.deck_prices_growth = false;
        self.base.disable_deck_prices_growth();
    }

    /// Opt-in gene `retainers-garrison-the-short-cities`; see
    /// `BasicAi::retainers_garrison_the_short_cities`.
    pub fn enable_retainers_garrison_the_short_cities(&mut self) {
        self.retainers_garrison_the_short_cities = true;
        self.base.enable_retainers_garrison_the_short_cities();
    }

    /// The twin of `enable_retainers_garrison_the_short_cities`.
    pub fn disable_retainers_garrison_the_short_cities(&mut self) {
        self.retainers_garrison_the_short_cities = false;
        self.base.disable_retainers_garrison_the_short_cities();
    }

    /// Turn on `culture-keeps-every-border-open-2`. See
    /// [`Self::culture_keeps_every_border_open_2`].
    pub fn enable_culture_keeps_every_border_open_2(&mut self) {
        self.culture_keeps_every_border_open = false;
        self.culture_keeps_every_border_open_2 = true;
    }

    /// The twin of `enable_culture_keeps_every_border_open_2`.
    pub fn disable_culture_keeps_every_border_open_2(&mut self) {
        self.culture_keeps_every_border_open_2 = false;
    }

    /// Opt-in gene `launch-pad-keeps-its-rung`; see
    /// [`Self::launch_pad_keeps_its_rung`].
    pub fn enable_launch_pad_keeps_its_rung(&mut self) {
        self.launch_pad_keeps_its_rung = true;
        self.base.enable_launch_pad_resumes_its_foundation();
    }

    /// The twin of `enable_launch_pad_keeps_its_rung`.
    pub fn disable_launch_pad_keeps_its_rung(&mut self) {
        self.launch_pad_keeps_its_rung = false;
        self.base.disable_launch_pad_resumes_its_foundation();
    }

    /// Turn on `culture-seat-theater-first-2`. See
    /// [`Self::culture_seat_theater_first_2`].
    pub fn enable_culture_seat_theater_first_2(&mut self) {
        self.culture_seat_theater_first = false;
        self.culture_seat_theater_first_2 = true;
    }

    /// The twin of `enable_culture_seat_theater_first_2`.
    pub fn disable_culture_seat_theater_first_2(&mut self) {
        self.culture_seat_theater_first_2 = false;
    }

    /// Opt-in gene `first-pad-in-the-forge`; see
    /// [`Self::first_pad_in_the_forge`].
    pub fn enable_first_pad_in_the_forge(&mut self) {
        self.first_pad_in_the_forge = true;
        self.base.enable_first_pad_in_the_forge();
    }

    /// The twin of `enable_first_pad_in_the_forge`.
    pub fn disable_first_pad_in_the_forge(&mut self) {
        self.first_pad_in_the_forge = false;
        self.base.disable_first_pad_in_the_forge();
    }

    /// Opt-in gene `deck-follows-the-queue`; the behaviour lives on
    /// `BasicAi::deck_follows_the_queue`, see `ai/deck_queue.rs`.
    pub fn enable_deck_follows_the_queue(&mut self) {
        self.deck_follows_the_queue = true;
        self.base.enable_deck_follows_the_queue();
    }

    /// The twin of `enable_deck_follows_the_queue`.
    pub fn disable_deck_follows_the_queue(&mut self) {
        self.deck_follows_the_queue = false;
        self.base.disable_deck_follows_the_queue();
    }

    /// Aim a bounded Conquest window at a soft city-state city beside the
    /// empire. See [`Self::city_state_harvest`].
    pub fn enable_city_state_harvest(&mut self) {
        self.city_state_harvest = true;
    }

    /// The twin of `enable_city_state_harvest`.
    pub fn disable_city_state_harvest(&mut self) {
        self.city_state_harvest = false;
        self.city_state_harvest_target = None;
    }

    /// Enable measured routing to the board's assigned Defend city.
    pub fn enable_anvil_follows_the_defense(&mut self) {
        self.anvil_follows_the_defense = true;
    }
    /// Preserve the original global threat-city routing.
    pub fn disable_anvil_follows_the_defense(&mut self) {
        self.anvil_follows_the_defense = false;
    }

    /// Opt-in gene `guns-stay-out-of-reach-2`; version two of the family, so
    /// version one is turned off. See [`Self::guns_stay_out_of_reach_2`].
    pub fn enable_guns_stay_out_of_reach_2(&mut self) {
        self.guns_stay_out_of_reach = false;
        self.guns_stay_out_of_reach_3 = false;
        self.guns_stay_out_of_reach_2 = true;
    }

    /// The twin of `enable_guns_stay_out_of_reach_2`.
    pub fn disable_guns_stay_out_of_reach_2(&mut self) {
        self.guns_stay_out_of_reach_2 = false;
    }

    /// Opt-in gene `guns-stay-out-of-reach-3`; version three of the
    /// family, so versions one and two are turned off. See
    /// [`Self::guns_stay_out_of_reach_3`].
    pub fn enable_guns_stay_out_of_reach_3(&mut self) {
        self.guns_stay_out_of_reach = false;
        self.guns_stay_out_of_reach_2 = false;
        self.guns_stay_out_of_reach_3 = true;
    }

    /// The twin of `enable_guns_stay_out_of_reach_3`.
    pub fn disable_guns_stay_out_of_reach_3(&mut self) {
        self.guns_stay_out_of_reach_3 = false;
    }

    /// Opening defence priority for the threatened sole capital.
    pub fn enable_first_capital_before_the_camp(&mut self) {
        self.first_capital_before_the_camp = true;
    }

    /// Restore the existing objective-board priorities.
    pub fn disable_first_capital_before_the_camp(&mut self) {
        self.first_capital_before_the_camp = false;
    }

    /// Opt-in gene `garrison-before-the-surprise`; see
    /// `BasicAi::garrison_before_the_surprise`.
    pub fn enable_garrison_before_the_surprise(&mut self) {
        self.garrison_before_the_surprise = true;
        self.base.enable_garrison_before_the_surprise();
    }

    /// The twin of `enable_garrison_before_the_surprise`.
    pub fn disable_garrison_before_the_surprise(&mut self) {
        self.garrison_before_the_surprise = false;
        self.base.disable_garrison_before_the_surprise();
    }

    /// Opt-in gene `camps-close-the-age-2`; see
    /// [`Self::camps_close_the_age_2`]. One version of the family plays.
    pub fn enable_camps_close_the_age_2(&mut self) {
        self.camps_close_the_age = false;
        self.camps_close_the_age_2 = true;
    }

    /// The twin of `enable_camps_close_the_age_2`.
    pub fn disable_camps_close_the_age_2(&mut self) {
        self.camps_close_the_age_2 = false;
    }

    /// Opt-in gene `capital-keeps-its-defender`; see
    /// [`Self::capital_keeps_its_defender`].
    pub fn enable_capital_keeps_its_defender(&mut self) {
        self.capital_keeps_its_defender = true;
    }

    /// The twin of `enable_capital_keeps_its_defender`.
    pub fn disable_capital_keeps_its_defender(&mut self) {
        self.capital_keeps_its_defender = false;
    }

    /// Opt-in gene `capital-keeps-its-defender-2`; see
    /// [`Self::capital_keeps_its_defender_2`]. One version of the family
    /// plays.
    pub fn enable_capital_keeps_its_defender_2(&mut self) {
        self.capital_keeps_its_defender = false;
        self.capital_keeps_its_defender_2 = true;
    }

    /// The twin of `enable_capital_keeps_its_defender_2`.
    pub fn disable_capital_keeps_its_defender_2(&mut self) {
        self.capital_keeps_its_defender_2 = false;
    }

    /// Keep a Governor whose departure would leave our original capital losing Loyalty.
    pub fn enable_capital_keeps_its_governor(&mut self) {
        self.capital_keeps_its_governor = true;
        self.base.capital_keeps_its_governor = true;
    }

    /// Withhold the independently screenable capital Governor guard.
    pub fn disable_capital_keeps_its_governor(&mut self) {
        self.capital_keeps_its_governor = false;
        self.base.capital_keeps_its_governor = false;
    }

    /// Opt-in gene `breaker-spares-the-backlog-builder`; see
    /// [`Self::breaker_spares_the_backlog_builder`].
    pub fn enable_breaker_spares_the_backlog_builder(&mut self) {
        self.breaker_spares_the_backlog_builder = true;
    }

    /// The twin of `enable_breaker_spares_the_backlog_builder`.
    pub fn disable_breaker_spares_the_backlog_builder(&mut self) {
        self.breaker_spares_the_backlog_builder = false;
    }

    /// Opt-in gene `camps-close-the-age-3`; see
    /// [`Self::camps_close_the_age_3`]. One version of the family plays.
    pub fn enable_camps_close_the_age_3(&mut self) {
        self.camps_close_the_age = false;
        self.camps_close_the_age_2 = false;
        self.camps_close_the_age_3 = true;
    }

    /// The twin of `enable_camps_close_the_age_3`.
    pub fn disable_camps_close_the_age_3(&mut self) {
        self.camps_close_the_age_3 = false;
    }

    /// Opt-in gene `capital-keeps-its-defender-3`; see
    /// [`Self::capital_keeps_its_defender_3`]. One version of the family
    /// plays.
    pub fn enable_capital_keeps_its_defender_3(&mut self) {
        self.capital_keeps_its_defender = false;
        self.capital_keeps_its_defender_2 = false;
        self.capital_keeps_its_defender_3 = true;
    }

    /// The twin of `enable_capital_keeps_its_defender_3`.
    pub fn disable_capital_keeps_its_defender_3(&mut self) {
        self.capital_keeps_its_defender_3 = false;
    }

    /// Opt-in gene `walls-release-the-one-blow`; see
    /// [`Self::walls_release_the_one_blow`].
    pub fn enable_walls_release_the_one_blow(&mut self) {
        self.walls_release_the_one_blow = true;
    }

    /// The twin of `enable_walls_release_the_one_blow`.
    pub fn disable_walls_release_the_one_blow(&mut self) {
        self.walls_release_the_one_blow = false;
    }

    /// Opt-in gene `anvil-spares-the-one-blow`; see
    /// [`Self::anvil_spares_the_one_blow`].
    pub fn enable_anvil_spares_the_one_blow(&mut self) {
        self.anvil_spares_the_one_blow = true;
    }

    /// The twin of `enable_anvil_spares_the_one_blow`.
    pub fn disable_anvil_spares_the_one_blow(&mut self) {
        self.anvil_spares_the_one_blow = false;
    }

    /// Opt-in gene `ranged-kills-are-free`; see
    /// [`Self::ranged_kills_are_free`].
    pub fn enable_ranged_kills_are_free(&mut self) {
        self.ranged_kills_are_free = true;
    }

    /// The twin of `enable_ranged_kills_are_free`.
    pub fn disable_ranged_kills_are_free(&mut self) {
        self.ranged_kills_are_free = false;
    }

    /// Opt-in gene `bankruptcy-exits-the-drain-2`, version two of
    /// `bankruptcy-exits-the-drain`; see `advanced/solvency.rs`. A seat plays
    /// one version, so this turns version one off.
    pub fn enable_bankruptcy_exits_the_drain_2(&mut self) {
        self.bankruptcy_exits_the_drain_2 = true;
        self.bankruptcy_exits_the_drain = false;
    }

    /// The twin of `enable_bankruptcy_exits_the_drain_2`.
    pub fn disable_bankruptcy_exits_the_drain_2(&mut self) {
        self.bankruptcy_exits_the_drain_2 = false;
    }

    /// Opt-in gene `settler-shelters-from-the-war-2`, version two of
    /// `settler-shelters-from-the-war`; see
    /// `advanced/settler_shelters_from_the_war.rs`. A seat plays one version,
    /// so this turns version one off.
    pub fn enable_settler_shelters_from_the_war_2(&mut self) {
        self.settler_shelters_from_the_war_2 = true;
        self.settler_shelters_from_the_war = false;
    }

    /// The twin of `enable_settler_shelters_from_the_war_2`.
    pub fn disable_settler_shelters_from_the_war_2(&mut self) {
        self.settler_shelters_from_the_war_2 = false;
    }

    /// Opt-in gene `military-alliance-shield-2`, version two of
    /// `military-alliance-shield`: only Friendly rivals are asked, and a
    /// Military kind taken or refused falls through to another kind; see
    /// `advanced/military_alliance_shield.rs`. A seat plays one version, so
    /// this turns version one off.
    pub fn enable_military_alliance_shield_2(&mut self) {
        self.military_alliance_shield_2 = true;
        self.military_alliance_shield = false;
    }

    /// The twin of `enable_military_alliance_shield_2`.
    pub fn disable_military_alliance_shield_2(&mut self) {
        self.military_alliance_shield_2 = false;
    }

    /// Opt-in gene `group-steps-read-the-field`; see
    /// [`Self::group_steps_read_the_field`].
    pub fn enable_group_steps_read_the_field(&mut self) {
        self.group_steps_read_the_field = true;
    }

    /// The twin of `enable_group_steps_read_the_field`.
    pub fn disable_group_steps_read_the_field(&mut self) {
        self.group_steps_read_the_field = false;
    }

    /// Opt-in gene `nusantara-reads-the-coast`; see
    /// [`Self::nusantara_reads_the_coast`].
    pub fn enable_nusantara_reads_the_coast(&mut self) {
        self.nusantara_reads_the_coast = true;
    }

    /// The twin of `enable_nusantara_reads_the_coast`.
    pub fn disable_nusantara_reads_the_coast(&mut self) {
        self.nusantara_reads_the_coast = false;
    }

    /// Opt-in committed launch relocation; see `governor_dividends/pingala_launch`.
    pub fn enable_pingala_earns_the_launch(&mut self) {
        self.pingala_earns_the_launch = true;
    }

    /// Withhold the committed launch relocation.
    pub fn disable_pingala_earns_the_launch(&mut self) {
        self.pingala_earns_the_launch = false;
    }

    /// Opt-in gene `field-reads-the-tile-it-leaves`; see
    /// [`Self::field_reads_the_tile_it_leaves`].
    pub fn enable_field_reads_the_tile_it_leaves(&mut self) {
        self.field_reads_the_tile_it_leaves = true;
    }

    /// The twin of `enable_field_reads_the_tile_it_leaves`.
    pub fn disable_field_reads_the_tile_it_leaves(&mut self) {
        self.field_reads_the_tile_it_leaves = false;
    }

    /// Opt-in gene `exalted-goddess-buys-the-coastal-guard`; see
    /// [`Self::exalted_goddess_buys_the_coastal_guard`].
    pub fn enable_exalted_goddess_buys_the_coastal_guard(&mut self) {
        self.exalted_goddess_buys_the_coastal_guard = true;
    }

    /// The twin of `enable_exalted_goddess_buys_the_coastal_guard`.
    pub fn disable_exalted_goddess_buys_the_coastal_guard(&mut self) {
        self.exalted_goddess_buys_the_coastal_guard = false;
    }

    // ---- append: a-b ------------------------------------------------
    // ---- append: c-d ------------------------------------------------
    // ---- append: e-f ------------------------------------------------
    // ---- append: g-k ------------------------------------------------
    // ---- append: l-o ------------------------------------------------
    // ---- append: p-r ------------------------------------------------
    // ---- append: s-s ------------------------------------------------
    // ---- append: t-z ------------------------------------------------
}

#[cfg(test)]
mod native_deployment_tests;

#[cfg(test)]
mod guard {
    /// The name of a method defined at `impl` depth on `line`, if there is one.
    ///
    /// Deliberately textual: the guards below are about where a function is
    /// written, which no reflection over the compiled crate can see.
    fn method_name(line: &str) -> Option<&str> {
        let body = line.strip_prefix("    ")?;
        if body.starts_with(' ') {
            return None; // deeper than `impl` depth: a nested item or a body line
        }
        let body = match body.strip_prefix("pub") {
            Some(rest) => match rest.strip_prefix('(') {
                Some(scoped) => scoped.split_once(") ")?.1,
                None => rest.strip_prefix(' ')?,
            },
            None => body,
        };
        let body = body.strip_prefix("const ").unwrap_or(body);
        let rest = body.strip_prefix("fn ")?;
        Some(&rest[..rest.find(|c: char| !c.is_alphanumeric() && c != '_')?])
    }

    fn is_toggle(name: &str) -> bool {
        name.starts_with("enable_") || name.starts_with("disable_")
    }

    /// The two names a `toggles!` entry (`flag => enable_x, disable_x;`)
    /// defines, if `line` is one.
    fn entry_names(line: &str) -> Option<(&str, &str)> {
        let body = line.strip_prefix("        ")?;
        // Deeper lines, comments, and the macro's own `$flag => $enable`
        // pattern are not entries.
        if body.starts_with(' ') || body.starts_with("//") || body.starts_with('$') {
            return None;
        }
        let (_flag, names) = body.split_once(" => ")?;
        let (enable, disable) = names.strip_suffix(';')?.split_once(", ")?;
        Some((enable, disable))
    }

    /// Every toggle name this file defines: hand-written methods and both
    /// halves of every `toggles!` entry.
    fn defined_names() -> Vec<&'static str> {
        let mut names = Vec::new();
        for line in toggles_only().lines() {
            if let Some(name) = method_name(line) {
                names.push(name);
            } else if let Some((enable, disable)) = entry_names(line) {
                names.extend([enable, disable]);
            }
        }
        names
    }

    /// This file up to where this module starts.
    ///
    /// The guards below are themselves functions at `impl` depth in this
    /// file, so a scan of the whole text reports them as strays.
    fn toggles_only() -> &'static str {
        let source = include_str!("treatment_flags.rs");
        let marker = "\n#[cfg(test)]\nmod guard {";
        &source[..source
            .find(marker)
            .expect("the guard module moved or was renamed")]
    }

    /// The toggles stay out of the hotspot.
    ///
    /// Without this, the next treatment PR adds its pair back at the anchor
    /// this change exists to empty, and the move is undone by the following
    /// morning. `AGENTS.md` states the rule this implements: a sentence that
    /// states a fact is enforced by a test, or it is a guess with good
    /// posture.
    #[test]
    fn advanced_rs_defines_no_capability_toggles() {
        let offenders: Vec<&str> = include_str!("../advanced.rs")
            .lines()
            .filter(|line| method_name(line).is_some_and(is_toggle))
            .collect();
        assert!(
            offenders.is_empty(),
            "capability toggles belong in src/ai/advanced/treatment_flags.rs, not in \
             src/ai/advanced.rs — the file tools/conflict_hotspots.py measures as the most \
             contended in the repository. Move these there:\n{}",
            offenders.join("\n")
        );
    }

    /// And nothing else moves in here.
    ///
    /// A file that holds only short setters is safe to conflict in. That
    /// property is the whole point of the move, and it is lost the first time
    /// something with real logic is parked here because the file was quiet.
    #[test]
    fn treatment_flags_holds_only_toggles() {
        let mut strays: Vec<&str> = toggles_only()
            .lines()
            .filter(|line| method_name(line).is_some_and(|name| !is_toggle(name)))
            .collect();
        strays.extend(toggles_only().lines().filter(|line| {
            entry_names(line).is_some_and(|(enable, disable)| {
                !enable.starts_with("enable_") || !disable.starts_with("disable_")
            })
        }));
        assert!(
            strays.is_empty(),
            "src/ai/advanced/treatment_flags.rs holds capability toggles and nothing else; \
             these belong in src/ai/advanced.rs:\n{}",
            strays.join("\n")
        );
    }

    /// The guards can see a toggle at all.
    ///
    /// Both assertions above pass trivially if `method_name` stops matching —
    /// a rustfmt change to how a signature wraps, a visibility spelling it
    /// does not know. This one fails instead of going quiet.
    #[test]
    fn the_guards_can_see_the_toggles_they_guard() {
        let names = defined_names();
        let found = names.iter().filter(|name| is_toggle(name)).count();
        assert!(
            found > 1500,
            "the guards found only {found} toggles in a file that holds ~1,570 of them; \
             the guards in this module are no longer reading the source they claim to check"
        );
        let entries = toggles_only().lines().filter_map(entry_names).count();
        assert!(
            entries > 500,
            "entry_names() found only {entries} `toggles!` entries of ~580; the entry \
             check above is no longer reading the block it guards"
        );
    }

    /// No toggle is defined twice. Two hand-written definitions are a compile
    /// error; a `toggles!` entry and a hand-written pair for the same name are
    /// too, but only this test names the line to delete.
    #[test]
    fn every_toggle_is_defined_once() {
        let mut names = defined_names();
        names.sort_unstable();
        let repeats: Vec<&&str> = names
            .windows(2)
            .filter(|w| w[0] == w[1])
            .map(|w| &w[0])
            .collect();
        assert!(repeats.is_empty(), "toggles defined twice: {repeats:?}");
    }
}
