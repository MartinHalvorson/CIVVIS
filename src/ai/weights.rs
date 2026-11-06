//! `Weights`: the strategy weights steering `BasicAi`, and what they default
//! to.
//!
//! The defaults reproduce the original hand-tuned behaviour, and the `evolve`
//! GA searches this space. `DedicationChoice` and `PolicyDeck` are the arms
//! of those two experiments, `OPENING_MENU` is the list the `open*` weights
//! index, and `ADVANCED_D_HOLY` is what the scripted major agent pays for a
//! Holy Site.
//!
//! Moved verbatim out of `ai.rs` (2026-11-05). `ai.rs` re-exports everything
//! here, so every path is unchanged; items the rest of `ai` uses are
//! `pub(super)`.

/// Strategy weights steering BasicAi decisions. Defaults reproduce the
/// original hand-tuned behavior; the `evolve` GA searches this space.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Weights {
    pub city_target: f64,       // stop settling at this many cities+settlers
    pub settler_min_pop: f64,   // city pop needed before making a settler
    pub settler_stop_turn: f64, // no new settlers after this turn
    pub mil_per_city: f64,      // military units to keep per city
    pub builder_per_city: f64,  // builders to keep per city
    pub war_ratio: f64,         // declare war if my power > ratio*theirs+margin
    pub war_margin: f64,
    pub peace_ratio: f64, // sue for peace if my power < ratio*theirs
    pub war_min_turn: f64,
    pub attack_floor: f64,  // minimum exchange score to attack (SEE-style)
    pub kill_bonus: f64,    // exchange bonus for a killing blow
    pub trade_caution: f64, // weight on expected counter-damage
    pub settle_food: f64,   // settle-site yield weights
    pub settle_prod: f64,
    pub settle_gold: f64,
    pub settle_dist: f64, // per-hex penalty on distant settle sites
    pub min_city_dist: f64,
    pub wonder_min_bld: f64, // buildings before a city tries wonders
    pub faith_builder: f64,  // faith reserve before buying a builder
    pub d_campus: f64,       // district build priorities (higher first)
    pub d_commercial: f64,
    pub d_holy: f64,
    pub d_theater: f64,
    // opening book: first four capital builds, indexes into OPENING_MENU
    // (floor; >= menu length = no scripted pick, evaluate normally)
    pub open0: f64,
    pub open1: f64,
    pub open2: f64,
    pub open3: f64,
    // 1-ply tactical movement: candidate tiles scored by progress toward the
    // target plus these positional terms
    pub mv_support: f64, // bonus per adjacent friendly military unit
    pub mv_threat: f64,  // penalty per point of expected incoming damage
    // Hierarchical combat doctrine. AdvancedAi turns these genes into shared
    // army/fleet orders; keeping them in Weights lets self-play evolve economy,
    // grand strategy, and battlefield execution as one genome.
    pub command_radius: f64,     // maximum separation inside one force group
    pub muster_radius: f64,      // distance from group anchor considered ready
    pub muster_readiness: f64,   // fraction assembled before a planned advance
    pub cohesion: f64,           // movement reward for staying with the force
    pub focus_fire: f64,         // attack bonus for the group's shared target
    pub screen: f64,             // penalty for ranged/siege moving ahead of melee
    pub role_spacing: f64,       // reward for each role's preferred engagement depth
    pub objective_progress: f64, // movement reward toward the shared objective
    pub local_superiority: f64,  // caution when local hostile power is greater
    pub withdraw_hp: f64,        // enter persistent recovery at or below this HP
    pub rejoin_hp: f64,          // leave recovery at or above this HP
    // Policy deck appetite. A card is valued by slotting it and asking the
    // engine what changed (`Weights::card_value`), so these are the exchange
    // rates between the things a card can buy -- not a per-card table. The
    // catalogue is 125 cards and grows with any mod, so nothing here names one.
    pub pol_food: f64,
    pub pol_production: f64,
    pub pol_gold: f64,
    pub pol_science: f64,
    pub pol_culture: f64,
    pub pol_faith: f64,
    /// Yield-equivalent of one point of fielded combat strength. Small: a card
    /// worth +5% to ten units moves strength by tens, a yield card by ones.
    pub pol_military: f64,
    /// What one point of influence per turn is worth to the policy valuation,
    /// in the same units as a point of yield.
    ///
    /// **Zero by default, which reproduces the shipped behaviour exactly.**
    /// Influence converts to envoys at the government's `influence_threshold`
    /// and envoys convert to suzerainty at three per city-state, so a point of
    /// influence is worth a fraction of an envoy and an envoy is worth a
    /// fraction of a suzerainty -- but the suzerainty it eventually buys is
    /// measured at a 34-point win-rate swing, so the chain is short and the
    /// prize is large. The right value is not derivable from that and is what
    /// an evaluation is for; this exists so the axis can be measured at all.
    pub pol_influence: f64,
    /// What one point of a city's growth surplus -- its Food past consumption
    /// after the housing, amenity and loyalty throttles
    /// (`Game::city_growth_surplus`) -- is worth to the policy valuation.
    ///
    /// **Zero by default, which reproduces the shipped behaviour exactly.** A
    /// housing card (Insulae, Medina Quarter) or an amenity card (Retainers,
    /// Liberalism) moves no yield the sweep reads: it lifts a growth throttle,
    /// so its value is the growth it releases. `deck-prices-growth` sets it.
    pub pol_growth: f64,
    /// Fraction by which a challenger must beat the incumbent to take its
    /// slot. Zero re-shuffles the deck on noise; one never swaps at all.
    pub pol_swap_margin: f64,
    /// ★★★★★ WHAT EVERY CITY BUILDS, EVERY TURN, WAS OUTSIDE THE SEARCH.
    ///
    /// `AdvancedAi::production_value` is 1,014 lines that rank every candidate
    /// build. Measured on the body of that function: **291 numeric literals,
    /// 93 of them distinct, and zero reads of this struct.** The genome is 40
    /// genes wide and not one of them reached the production decision. So no
    /// run of `evolve` has ever tuned a build priority, the macro search has
    /// never perturbed one, and every change to that ranking has had to be a
    /// hand-picked constant defended in prose.
    ///
    /// The tree already says this about one of the 93. `settler_price` exists
    /// because the settler arm scores `920.0 + site_value * 4.0` and *"920 is
    /// a hardcoded literal: not a gene, so no run of `evolve` has tuned it,
    /// and not a doctrine lever, so the macro search has never perturbed it."*
    /// That complaint is true of ninety-two more constants beside it, and
    /// `docs/AI_GAPS.md` §6 names the general form: the search surface is
    /// narrower than the policy.
    ///
    /// These eight are category multipliers on the final score of each
    /// production arm, in the same currency the arm already returns. They are
    /// deliberately coarse: the goal is to put the *shape* of the build order
    /// inside the search surface, not to expose 93 free parameters to a GA
    /// whose fitness estimate is already noisy.
    ///
    /// ⚠ **1.0 each by default, which is the shipped behaviour exactly.**
    /// Multiplication by 1.0 is exact in IEEE-754 for every finite score, so a
    /// default genome ranks builds bit-identically to before. That matters
    /// more here than usual: `Weights::default()` also seeds `BasicAi::new()`,
    /// which is city-states, barbarians, the `basic` entrant and
    /// `AdvancedAi::legacy()` behind the frozen `advanced_v1` anchor.
    ///
    /// ⚠⚠ A gene that changes nothing is worse than no gene, because it
    /// spends GA budget and reports a tuned parameter that never moved a game.
    /// This repository has shipped silent genes before — §6 again: *"causal
    /// probes have found multiple parameters that do not change the sampled
    /// `AdvancedAi` games"*. Every one of these eight is therefore held to
    /// `production_genes_are_not_silent`, which perturbs each in isolation and
    /// fails if the build order does not move.
    /// Combat units, including escorted `Formation` bodies.
    pub p_military: f64,
    pub p_builder: f64,
    pub p_trader: f64,
    pub p_building: f64,
    pub p_district: f64,
    pub p_wonder: f64,
    pub p_project: f64,
    /// Which deck this strategy holds.
    ///
    /// **Not a gene.** Deliberately absent from `to_vec`/`from_vec`/`bounds`,
    /// so the GA can neither read nor breed it and the genome stays 40 wide.
    /// It rides on `Weights` for one reason: `AdvancedAi::with_weights` already
    /// carries a genome into the inner `BasicAi`, so an eval arm costs no
    /// change to `src/ai/advanced.rs` or `src/elo.rs`. Set it in a harness;
    /// leave it alone in play.
    #[serde(default)]
    pub policy_deck: PolicyDeck,
    /// How this strategy picks its Dedication at an age transition.
    ///
    /// **Not a gene**, for the same reasons as `policy_deck`: absent from
    /// `to_vec`/`from_vec`/`bounds`, so the genome stays 40 wide.
    #[serde(default)]
    pub dedication_choice: DedicationChoice,
}

/// The two arms a Dedication experiment needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DedicationChoice {
    /// The first name `available_dedications` returns, which is a `BTreeMap`
    /// key, which is alphabetical order.
    ///
    /// **This is the agent that plays, because it is the agent that wins.** It
    /// looks arbitrary and it is, but in the Classical era alphabetical order
    /// leads with Exodus of the Evangelists, whose Golden-Age half feeds
    /// missionaries and Great Prophet points — and religion is the lane that
    /// converts in this engine. Measured against `Measured` over 120 mirrored
    /// maps it took **58.8%** of games, 31 map directions to 10, sign
    /// p=0.0015, with the anytime-valid e-process crossing at map 51. See
    /// `docs/AGES.md`. It is no longer the default — `Banking` beat it — but it
    /// remains the frozen control for every age number published before
    /// 2026-07-27.
    Alphabetical,
    /// Ranked by what each Dedication would have paid over the era that just
    /// ended, measured from the civilization's own trigger tally.
    ///
    /// **A recorded negative result, retained as an evaluator arm.** The
    /// projection is the right objective in a Normal or Dark Age, where Era
    /// Score literally buys the next age. In a Golden or Heroic Age it is only
    /// a *correlate* of what the Golden half is worth, and an argmax over a
    /// correlate is the failure mode this repository keeps rediscovering.
    Measured,
    /// `Measured` restricted to the ages where the number it ranks on is the
    /// literal objective: a Normal or Dark Age banks Era Score, so the
    /// Dedication that would have paid most is the one that buys the next age
    /// soonest. A Golden or Heroic Age banks nothing, so that choice is left
    /// exactly as `Alphabetical` makes it.
    ///
    /// This is the repair for `Measured`'s loss, and it is the whole of the
    /// repair — no new signal, just the same signal withdrawn from the half of
    /// the decision where it was never causal.
    ///
    /// **PROMOTED.** Pre-registered at seed 970000, 300 mirrored maps, 600
    /// games: **57.7%**, 67 map directions to 21, sign p=0.0000, Elo **+54**
    /// (CI +14..+93), Wilson **52.0%–63.1%**, e-process 5.72e4 crossing at map
    /// 112 — `promotion gate: PASS` under the unmodified gate. The earlier
    /// disjoint seed 960000 agreed at 56.2%; pooled that is **420 maps, 93 map
    /// directions to 32**.
    #[default]
    Banking,
}

/// The three arms a policy-deck experiment needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PolicyDeck {
    /// Cards valued by slotting them and reading the empire either side.
    ///
    /// Old champion artifacts predate this non-gene field. They deserialize
    /// to Live for compatibility, and a recorded six-seat A/B showed that
    /// changing them to Legacy loses significantly (12 map directions for,
    /// 26 against, p=0.0336). Gene-vector children preserve their template's
    /// non-gene policy through `Weights::from_vec_like`.
    #[default]
    Live,
    /// The pre-2026-07-27 behaviour: the twenty cards of `POLICY_PRIORITY`, in
    /// order, and only while a slot stands empty.
    Legacy,
    /// Slot nothing, ever.
    ///
    /// Not a strategy — an ablation. `Legacy` against `Empty` measures what the
    /// entire card layer is worth, which bounds what *any* card policy can win
    /// and therefore whether choosing well within it deserves more effort. Run
    /// this before optimising a subsystem, not after.
    Empty,
}

pub const OPENING_MENU: [&str; 6] = [
    "scout", "warrior", "builder", "settler", "slinger", "monument",
];

/// What the scripted major agent pays for a Holy Site, against the 2.0 every
/// other holder of these weights keeps.
///
/// ⚠ **Deliberately NOT in `Weights::default()`.** That default also seeds
/// `BasicAi::new()`, which is city-states, barbarians, the `basic` entrant, and
/// `AdvancedAi::legacy()` behind the frozen `advanced_v1` anchor. Moving it
/// there would change populations the gate never measured and would silently
/// redefine a frozen control — so the value lives on the one constructor the
/// evaluated arm actually used.
///
/// The number is read off the shipped league roster rather than tuned: of the
/// bred genomes carrying real 8-player evidence, the top third by outright win
/// rate sit at a games-weighted `d_holy` of 5.6 and the bottom third at exactly
/// the 2.0 default. `advanced_holy_priority` measured it at **+20 Elo, gate
/// PASS** over 1200 paired maps; see `docs/EVAL.md` 2026-08-10.
pub const ADVANCED_D_HOLY: f64 = 5.6;

impl Weights {
    /// The weights the scripted major agent plays, as distinct from the weights
    /// every other holder of this struct gets. One gene apart from
    /// [`Weights::default`]; see [`ADVANCED_D_HOLY`].
    pub fn advanced() -> Weights {
        Weights {
            d_holy: ADVANCED_D_HOLY,
            ..Weights::default()
        }
    }
}

impl Default for Weights {
    fn default() -> Weights {
        Weights {
            city_target: 4.0,
            settler_min_pop: 2.0,
            settler_stop_turn: 150.0,
            mil_per_city: 1.0,
            builder_per_city: 0.5,
            war_ratio: 1.8,
            war_margin: 20.0,
            peace_ratio: 0.6,
            war_min_turn: 40.0,
            attack_floor: 0.0,
            kill_bonus: 25.0,
            trade_caution: 1.0,
            settle_food: 1.2,
            settle_prod: 1.0,
            settle_gold: 0.3,
            settle_dist: 0.4,
            min_city_dist: 4.0,
            wonder_min_bld: 3.0,
            faith_builder: 120.0,
            d_campus: 4.0,
            d_commercial: 3.0,
            d_holy: 2.0,
            d_theater: 1.0,
            open0: 1.0,
            open1: 3.0,
            open2: 2.0,
            open3: 5.0, // warrior settler builder monument
            mv_support: 2.0,
            mv_threat: 0.5,
            command_radius: 6.0,
            muster_radius: 3.0,
            muster_readiness: 0.67,
            cohesion: 3.0,
            focus_fire: 2.5,
            screen: 4.0,
            role_spacing: 2.0,
            objective_progress: 2.5,
            local_superiority: 6.0,
            withdraw_hp: 45.0,
            rejoin_hp: 80.0,
            pol_food: 0.6,
            pol_production: 1.0,
            pol_gold: 0.6,
            pol_science: 1.0,
            pol_culture: 1.0,
            pol_faith: 0.7,
            pol_military: 0.05,
            pol_influence: 0.0,
            pol_growth: 0.0,
            pol_swap_margin: 0.15,
            // Identity. Every production arm is multiplied by its category
            // gene, and multiplying a finite score by exactly 1.0 is exact, so
            // a default genome ranks builds bit-identically to the tree before
            // these existed. `BasicAi::new()` and `AdvancedAi::legacy()` read
            // this default, so anything other than 1.0 here would move a
            // frozen anchor and every minor civilization with it.
            p_military: 1.0,
            p_builder: 1.0,
            p_trader: 1.0,
            p_building: 1.0,
            p_district: 1.0,
            p_wonder: 1.0,
            p_project: 1.0,
            // LEGACY, not Live — **for this default only**. ⚠ The shipped
            // agent does NOT play it: `AdvancedAi::production_weights`
            // overwrites this with `PolicyDeck::Live` for everything
            // `AdvancedAi::new()` builds. The sentence below about "the agent
            // that plays" describes `basic`, `advanced_v1` and any holder of
            // these raw weights, and stopped describing production when that
            // constructor was written. Withhold it with
            // `advanced_legacy_policy_deck` to price the difference.
            //
            // The counterfactual deck is a measured null:
            // 18 map directions to 15, p=0.7283 over 120 mirrored maps, with
            // terminal score also flat. It costs an empire valuation per
            // candidate card per review, so shipping it would buy a real
            // slowdown with no evidence of strength. The mechanism stays for
            // study -- `PolicyDeck::Live` still selects it and the eval arms
            // still work -- but the agent that plays is the one that always
            // played.
            policy_deck: PolicyDeck::Legacy,
            dedication_choice: DedicationChoice::Banking,
        }
    }
}

impl Weights {
    pub fn to_vec(&self) -> Vec<f64> {
        vec![
            self.city_target,
            self.settler_min_pop,
            self.settler_stop_turn,
            self.mil_per_city,
            self.builder_per_city,
            self.war_ratio,
            self.war_margin,
            self.peace_ratio,
            self.war_min_turn,
            self.attack_floor,
            self.kill_bonus,
            self.trade_caution,
            self.settle_food,
            self.settle_prod,
            self.settle_gold,
            self.settle_dist,
            self.min_city_dist,
            self.wonder_min_bld,
            self.faith_builder,
            self.d_campus,
            self.d_commercial,
            self.d_holy,
            self.d_theater,
            self.open0,
            self.open1,
            self.open2,
            self.open3,
            self.mv_support,
            self.mv_threat,
            self.command_radius,
            self.muster_radius,
            self.muster_readiness,
            self.cohesion,
            self.focus_fire,
            self.screen,
            self.role_spacing,
            self.objective_progress,
            self.local_superiority,
            self.withdraw_hp,
            self.rejoin_hp,
            self.p_military,
            self.p_builder,
            self.p_trader,
            self.p_building,
            self.p_district,
            self.p_wonder,
            self.p_project,
        ]
    }

    pub fn from_vec(v: &[f64]) -> Weights {
        Weights {
            city_target: v[0],
            settler_min_pop: v[1],
            settler_stop_turn: v[2],
            mil_per_city: v[3],
            builder_per_city: v[4],
            war_ratio: v[5],
            war_margin: v[6],
            peace_ratio: v[7],
            war_min_turn: v[8],
            attack_floor: v[9],
            kill_bonus: v[10],
            trade_caution: v[11],
            settle_food: v[12],
            settle_prod: v[13],
            settle_gold: v[14],
            settle_dist: v[15],
            min_city_dist: v[16],
            wonder_min_bld: v[17],
            faith_builder: v[18],
            d_campus: v[19],
            d_commercial: v[20],
            d_holy: v[21],
            d_theater: v[22],
            open0: v[23],
            open1: v[24],
            open2: v[25],
            open3: v[26],
            mv_support: v[27],
            mv_threat: v[28],
            command_radius: v[29],
            muster_radius: v[30],
            muster_readiness: v[31],
            cohesion: v[32],
            focus_fire: v[33],
            screen: v[34],
            role_spacing: v[35],
            objective_progress: v[36],
            local_superiority: v[37],
            withdraw_hp: v[38],
            rejoin_hp: v[39],
            p_military: v[40],
            p_builder: v[41],
            p_trader: v[42],
            p_building: v[43],
            p_district: v[44],
            p_wonder: v[45],
            p_project: v[46],
            // The policy appetites are NOT genes. Measured on the statistic
            // that tracks winning, scrambling the whole policy block costs
            // +0.0006 +/- 0.0229 -- flat. Carrying eight worthless dimensions
            // would widen the search for nothing, and it collided with the
            // committed 40-gene champion another agent landed on main.
            // `PolicyDeck::Live` still reads them; the GA does not.
            // `policy_deck` and the appetites take the shipped defaults here.
            ..Weights::default()
        }
    }

    /// Reconstruct the gene vector without changing non-gene policy state.
    ///
    /// `from_vec` intentionally supplies production defaults when no template
    /// exists. Evolution and causal gene interventions do have a template:
    /// the parent or incumbent they are changing. Old champion artifacts can
    /// carry a compatibility policy different from `Weights::default`, so
    /// dropping these fields makes the first child differ before any gene
    /// mutation acts.
    pub fn from_vec_like(v: &[f64], template: &Weights) -> Weights {
        let mut weights = Weights::from_vec(v);
        weights.pol_food = template.pol_food;
        weights.pol_production = template.pol_production;
        weights.pol_gold = template.pol_gold;
        weights.pol_science = template.pol_science;
        weights.pol_culture = template.pol_culture;
        weights.pol_faith = template.pol_faith;
        weights.pol_military = template.pol_military;
        weights.pol_influence = template.pol_influence;
        weights.pol_growth = template.pol_growth;
        weights.pol_swap_margin = template.pol_swap_margin;
        weights.policy_deck = template.policy_deck;
        weights.dedication_choice = template.dedication_choice;
        weights
    }

    /// (lo, hi) clamp per gene, same order as to_vec.
    pub fn bounds() -> [(f64, f64); 47] {
        [
            (2.0, 12.0),
            (1.0, 5.0),
            (60.0, 400.0),
            (0.3, 4.0),
            (0.2, 2.0),
            (0.8, 5.0),
            (-20.0, 80.0),
            (0.2, 1.2),
            (10.0, 200.0),
            (-25.0, 25.0),
            (0.0, 80.0),
            (0.2, 3.0),
            (0.2, 3.0),
            (0.2, 3.0),
            (0.0, 2.0),
            (0.0, 2.0),
            (3.0, 7.0),
            (0.0, 8.0),
            (40.0, 400.0),
            (0.0, 8.0),
            (0.0, 8.0),
            (0.0, 8.0),
            (0.0, 8.0),
            (0.0, 6.99),
            (0.0, 6.99),
            (0.0, 6.99),
            (0.0, 6.99),
            (0.0, 10.0),
            (0.0, 3.0),
            (2.0, 12.0),
            (1.0, 6.0),
            (0.25, 1.0),
            (0.0, 10.0),
            (0.0, 8.0),
            (0.0, 12.0),
            (0.0, 8.0),
            (0.5, 6.0),
            (0.0, 16.0),
            (20.0, 65.0),
            (60.0, 100.0),
            // The eight production category multipliers. The band is
            // deliberately narrow and centred on the identity: these scale a
            // ranking whose arms already differ by thousands, so a wide band
            // would let one category dominate the order outright rather than
            // tilt it, and the shipped constants are a working build order
            // rather than an arbitrary starting point.
            (0.25, 4.0),
            (0.25, 4.0),
            (0.25, 4.0),
            (0.25, 4.0),
            (0.25, 4.0),
            (0.25, 4.0),
            (0.25, 4.0),
        ]
    }

    /// Gene names, same order as `to_vec` and `bounds`.
    ///
    /// A search that reports per-gene results has to name them, and deriving
    /// the name from the index by hand is how a table ends up mislabelled by
    /// one row. `gene_names_match_the_vector` pins the length.
    pub fn gene_names() -> [&'static str; 47] {
        [
            "city_target",
            "settler_min_pop",
            "settler_stop_turn",
            "mil_per_city",
            "builder_per_city",
            "war_ratio",
            "war_margin",
            "peace_ratio",
            "war_min_turn",
            "attack_floor",
            "kill_bonus",
            "trade_caution",
            "settle_food",
            "settle_prod",
            "settle_gold",
            "settle_dist",
            "min_city_dist",
            "wonder_min_bld",
            "faith_builder",
            "d_campus",
            "d_commercial",
            "d_holy",
            "d_theater",
            "open0",
            "open1",
            "open2",
            "open3",
            "mv_support",
            "mv_threat",
            "command_radius",
            "muster_radius",
            "muster_readiness",
            "cohesion",
            "focus_fire",
            "screen",
            "role_spacing",
            "objective_progress",
            "local_superiority",
            "withdraw_hp",
            "rejoin_hp",
            "p_military",
            "p_builder",
            "p_trader",
            "p_building",
            "p_district",
            "p_wonder",
            "p_project",
        ]
    }
}
