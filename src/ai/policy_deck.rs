//! The policy deck and Dedications: which cards to hold and which Dedications
//! to take.
//!
//! A deck is judged by what the empire is worth holding it, as the engine
//! computes it: `empire_reading` values the empire for a set of weights, the
//! difference across a candidate card is that card's whole effect
//! (`policy_card_score`), `reading_with_policy_trace` keeps the unchanged
//! baseline each candidate is compared against, and `revise_policy_deck`
//! holds or changes the deck. Dedications are chosen by `choose_dedications`
//! and, for `golden-dedication-serves-the-conquest`, by
//! `choose_conquest_dedications`.
//!
//! Moved verbatim out of `ai.rs` (2026-11-05). `ai.rs` imports everything
//! here, so every path is unchanged; items the rest of `ai` uses are
//! `pub(super)`.

use super::*;

pub(super) const POLICY_PRIORITY: [&str; 20] = [
    "urban_planning",
    "colonization",
    "ilkum",
    "feudal_contract",
    "agoge",
    "discipline",
    "god_king",
    "insulae",
    "meritocracy",
    "serfdom",
    "conscription",
    "bastions",
    "retainers",
    "town_charters",
    "craftsmen",
    "maritime_industries",
    "maneuver",
    "limes",
    "survey",
    "strategos",
];

/// Turns between re-examinations of a deck that is already full. Each review
/// costs one empire valuation per candidate card, and the answer moves on the
/// scale of a civic unlocking, not of a turn passing.
pub(super) const POLICY_REVIEW_EVERY: u32 = 8;

/// `deck-prices-growth`: a point of growth surplus is priced at this share of
/// the Food weight, on top of the Food itself. Live Immortal 2026-10-30/31
/// (seats alive past t120): 32-42% of our cities grew at half or a quarter
/// for housing and 22-28% were short of Amenities at t80-t140, our citizens
/// per city stood at 0.68-0.77 of the production leader's, and the deck
/// slotted Insulae in 1 of the 99 t80 seats that held it and Retainers in 18
/// of 89.
pub(super) const DECK_GROWTH_FOOD_SHARE: f64 = 1.0;

/// A policy review has only a few dozen independent cards, while each active
/// worker needs a full private game snapshot. Four branches are the measured
/// knee on the deployment-shaped single-game workload; use the rest of the
/// persistent pool for wider unit, tactical, purchase, and visibility work.
const POLICY_SCORE_MAX_WORKERS: usize = 4;

/// What `pid`'s empire is worth to `w` right now, in yield-equivalent points.
///
/// Read either side of a candidate card, the difference is that card's entire
/// effect **as the engine computes it** — district adjacency percentages,
/// unit-era windows, per-city production multipliers and all. Nothing here
/// names an effect key, so the 125-card catalogue and every card a mod adds
/// are covered by construction, and a card whose effect the engine does not
/// implement scores exactly zero instead of scoring its own documentation.
///
/// This is a counterfactual, which is the only kind of estimate this
/// repository has ever got value from: the card is applied and the result
/// measured, rather than a regression predicting what cards tend to accompany
/// winning. `docs/SUPERHUMAN.md` §0 is the evidence for preferring the former.
pub(super) fn empire_reading(
    g: &Game,
    pid: usize,
    w: &Weights,
    net_maintenance: bool,
    player_income: bool,
) -> f64 {
    // A card counterfactual is a read-only whole-empire sweep. Keep one memo
    // scope over it so the city-yield and ownership derivations shared by its
    // cities are answered once, then drop it before the caller changes cards.
    let _memo = g.query_memo();
    let mut value = 0.0;
    for cid in g.player_city_ids(pid) {
        let y = g.city_yields(cid);
        // Production is read as production *toward what this city is actually
        // building*. `city_yields` carries flat adders -- Urban Planning's
        // `city_production` -- but the whole `*_production_pct` family reaches
        // the game only through `item_prod_mult`, so without this factor Agoge,
        // Colonization, Ilkum, Maritime Industries, Limes and
        // Maneuver all score exactly 0.0 and lose every tie to a card worth a
        // rounding error of gold. That is the failure `PolicyAi` was retired
        // for, one layer up.
        let queued = g.cities.get(&cid).and_then(|city| city.queue.first());
        let toward_item = g.item_prod_mult(pid, cid, queued);
        value += w.pol_food * y.food
            + w.pol_production * y.production * toward_item
            + w.pol_gold * y.gold
            + w.pol_science * y.science
            + w.pol_culture * y.culture
            + w.pol_faith * y.faith;
        // `deck-prices-growth`: the Food that reaches the growth bank after
        // the housing, amenity and loyalty throttles. Skipped at zero, so the
        // shipped reading is untouched.
        if w.pol_growth != 0.0 {
            if let Some(city) = g.cities.get(&cid) {
                let surplus = g.city_growth_surplus(
                    pid,
                    cid,
                    y.food,
                    g.city_housing(city),
                    g.city_amenity_surplus(city),
                );
                value += w.pol_growth * surplus.max(0.0);
            }
        }
    }
    if let Some(y) = g.observed_yield_adjustments.get(&pid) {
        value += w.pol_food * y.food
            + w.pol_production * y.production
            + w.pol_gold * y.gold
            + w.pol_science * y.science
            + w.pol_culture * y.culture
            + w.pol_faith * y.faith;
    }
    // Combat cards move no yield. Ask the units themselves instead: a card
    // worth +5 strength to a standing army is visible here and nowhere else.
    let mut strength = 0.0;
    for uid in g.player_unit_ids(pid) {
        if let Some(unit) = g.units.get(&uid) {
            strength += g.unit_strength(unit, false) + g.unit_strength(unit, true);
        }
    }
    // Influence is neither a city yield nor unit strength, so a card whose
    // whole effect is `influence_per_turn` reads bit-identical either side of
    // the counterfactual and scores exactly 0.0 -- the same failure the
    // `*_production_pct` family had before `item_prod_mult` was folded in
    // above, and it silences an entire slot category. Ten of the seventeen
    // diplomatic cards name no yield in any effect key, `charismatic_leader`
    // (+2 influence) and `gunboat_diplomacy` (+4) among them.
    //
    // It is worth seeing because influence is the whole envoy economy. #602
    // measured suzerainty of every met city-state at **56.7% against a 22.7%
    // control**, p=0.0000 over 400 maps -- the largest ceiling in this
    // repository. #608 found the envoy pool at 0.00 unspent on every sample
    // and the agent suzerain of 3% of the city-states it meets, so the gap is
    // income, not placement. #612 found `charismatic_leader` unlocked on 63.7%
    // of turns with an open diplomatic slot on 49.1% -- and slotted on 0.0%.
    //
    // Read as a rate, because that is what a card changes. The stock is
    // `player.influence`, which the counterfactual cannot move in one turn.
    //
    // Only the policy term appears. `city_state_influence` sums three sources
    // -- the government's `influence_per_turn`, this, and the `influence_points`
    // buildings -- but slotting a card moves neither of the other two, so they
    // are identical either side of the counterfactual and cancel exactly in
    // the difference. Including them would add a large constant to both
    // readings and change no ranking.
    // Maintenance is gold the empire pays, not gold a city yields, so a
    // card whose whole effect is `unit_maintenance_discount` — Conscription,
    // Levee en Masse — reads identical either side of the counterfactual and
    // scores exactly 0.0, the same silence `influence_per_turn` had above.
    // (The `item_prod_mult` comment above used to claim Conscription too;
    // that was wrong — the discount never touches production.) Subtract the
    // empire's own unit bill at the gold weight: for every card that does not
    // move maintenance the bill is identical either side and cancels exactly,
    // so no other ranking changes. Behind `maintenance_aware_deck` so the
    // entrant `advanced_maintenance_deck` can price it first.
    let maintenance = if net_maintenance {
        w.pol_gold * g.unit_gold_maintenance(pid)
    } else {
        0.0
    };
    // `merchant-confederation-earns-its-slot`: policy income paid to the
    // PLAYER is no line of any city's ledger (`Game::player_policy_yields`),
    // so the sweep above read Merchant Confederation's Gold per Envoy and
    // Raj's yields per tributary as nothing, the policy trace marked both
    // cards inert, and they scored exactly 0.0 -- the silence influence and
    // maintenance had above. 165 live runs from 10-13: Merchant
    // Confederation available in 137, slotted in none, 7,300 turns at 15 or
    // more placed Envoys (about 216,000 Gold) behind Charismatic Leader's
    // unweighted influence. For every card that pays no player income the
    // term is identical either side and cancels exactly.
    let income = if player_income {
        let y = g.player_policy_yields(pid);
        w.pol_food * y.food
            + w.pol_production * y.production
            + w.pol_gold * y.gold
            + w.pol_science * y.science
            + w.pol_culture * y.culture
            + w.pol_faith * y.faith
    } else {
        0.0
    };
    value + w.pol_military * strength + w.pol_influence * g.policy_effect(pid, "influence_per_turn")
        - maintenance
        + income
}

/// Take the Dedications this age offers, best first.
///
/// Both AI tiers used to take `available_dedications(pid).next()` — the first
/// name in a `BTreeMap`, so every civilization in every game dedicated
/// alphabetically. In the Classical era that is Exodus of the Evangelists,
/// chosen by civilizations that have not founded a religion and never will.
///
/// The ranking is the civilization's own record. `projected_dedication_score`
/// asks what each Dedication *would have paid* over the era that just ended,
/// from a tally of trigger firings the engine keeps whether or not the trigger
/// was dedicated. That single number ranks both halves of the choice, because
/// a Dedication's two halves name the same activity: Free Inquiry counts your
/// Eurekas and then makes Eurekas worth more, To Arms counts your Corps kills
/// and then makes Corps cheaper. So the civilization that has been doing a
/// thing is the one both halves pay.
///
/// Which half is live still changes what the number *means*, and the engine
/// settles that: a Golden or Heroic Age normally banks no Era Score, so there
/// the tally is read purely as "which lane am I in". In a Normal or Dark Age it
/// is read literally, as the score that buys the next age. Georgia's Strength
/// in Unity is the explicit exception and keeps the literal ranking in every
/// age.
///
/// Ties — including the all-zero tie of a civilization whose first age arrives
/// before it has done anything the table counts — fall back to the alphabetical
/// order this code has always used, so the choice only moves where there is
/// evidence to move it.
pub(crate) fn choose_dedications(g: &mut Game, pid: usize, choice: DedicationChoice) {
    loop {
        let mut offered = g.available_dedications(pid);
        if offered.is_empty() {
            return;
        }
        // A Golden or Heroic Age normally banks no Era Score, so there the
        // projection is only a correlate of what the Golden half is worth —
        // and ranking on it is what lost the first gate. `Banking` keeps the
        // measured number where it is the literal objective and leaves the
        // rest alone. Georgia is the shipped exception: Strength in Unity also
        // pays the Normal-Age half during Golden and Heroic Ages.
        let banking = dedication_banks(g, pid);
        let rank = match choice {
            DedicationChoice::Alphabetical => false,
            DedicationChoice::Measured => true,
            DedicationChoice::Banking => banking,
        };
        if rank {
            offered.sort_by(|left, right| {
                g.projected_dedication_score(pid, right)
                    .cmp(&g.projected_dedication_score(pid, left))
                    .then(left.cmp(right))
            });
        }
        let mut progressed = false;
        for dedication in offered {
            if g.apply(
                pid,
                &Action::ChooseDedication {
                    dedication: Name::new(&dedication),
                },
            )
            .is_ok()
            {
                progressed = true;
                break;
            }
        }
        if !progressed {
            return;
        }
    }
}

/// Whether this age's Dedication is read as the Era Score it banks: a Normal
/// or Dark Age, or Georgia's Strength in Unity in any age. See
/// [`choose_dedications`].
fn dedication_banks(g: &Game, pid: usize) -> bool {
    !matches!(g.players[pid].age.as_str(), "golden" | "heroic")
        || g.civ_effect(pid, "golden_dedication_era_score") > 0.0
}

/// `golden-dedication-serves-the-conquest`: the Golden-half order a
/// domination seat takes its Golden and Heroic Age Dedications in.
///
/// Under `Banking` a Golden or Heroic Age does not rank, so the choice falls
/// to the alphabetical tie-break, which leads with Exodus of the Evangelists.
/// Over the 10-04/05 live King control runs (88 games) Exodus took 57 of 81
/// Medieval Golden Ages, and the Heroic Ages took Exodus, Free Inquiry and
/// Monumentality in that order. Exodus's Golden half moves Missionaries and
/// pays Great Prophet points, which a seat with no religion never spends.
/// The order puts first the halves a conquest spends: To Arms! (+25%
/// Production toward military units, +50% combat experience), Sky and Stars
/// (air experience and the flight Eurekas the air surge rides), Monumentality
/// (Settlers and Builders 30% cheaper and bought with Faith, Builders +2
/// Movement), then the economy, with Exodus last.
pub(crate) const CONQUEST_GOLDEN_DEDICATIONS: [&str; 12] = [
    "to_arms",
    "sky_and_stars",
    "monumentality",
    "free_inquiry",
    "reform_the_coinage",
    "heartbeat_of_steam",
    "automaton_warfare",
    "pen_brush_and_voice",
    "hic_sunt_dracones",
    "bodyguard_of_lies",
    "wish_you_were_here",
    "exodus_of_the_evangelists",
];

/// `golden-dedication-serves-the-conquest`: in a Golden or Heroic Age, take
/// the offered Dedications in [`CONQUEST_GOLDEN_DEDICATIONS`] order (a name
/// the list does not carry goes after it, alphabetically). In an age that
/// banks Era Score, defer to [`choose_dedications`] with `fallback`, so that
/// choice is unchanged. Returns the Dedications this call chose by the
/// conquest order.
pub(crate) fn choose_conquest_dedications(
    g: &mut Game,
    pid: usize,
    fallback: DedicationChoice,
) -> Vec<Name> {
    if pid >= g.players.len() || dedication_banks(g, pid) {
        choose_dedications(g, pid, fallback);
        return Vec::new();
    }
    let place = |name: &Name| {
        CONQUEST_GOLDEN_DEDICATIONS
            .iter()
            .position(|preferred| *preferred == name.as_str())
            .unwrap_or(CONQUEST_GOLDEN_DEDICATIONS.len())
    };
    let mut chosen = Vec::new();
    loop {
        let mut offered = g.available_dedications(pid);
        if offered.is_empty() {
            return chosen;
        }
        offered.sort_by(|left, right| place(left).cmp(&place(right)).then(left.cmp(right)));
        let Some(dedication) = offered.into_iter().find(|dedication| {
            g.apply(
                pid,
                &Action::ChooseDedication {
                    dedication: *dedication,
                },
            )
            .is_ok()
        }) else {
            return chosen;
        };
        chosen.push(dedication);
    }
}

/// Hold the deck the empire is worth most with, and change it when that
/// changes.
///
/// The predecessor of this function tried twenty hard-coded cards in a fixed
/// order, and only while a slot stood empty — so a deck filled once in the
/// Ancient era and `policies_fit` refused every later card for the rest of the
/// game. Measured over 64 seat-games (`src/bin/policy_census.rs`), an average
/// seat unlocked 42.0 cards and played 7.3 of them.
///
/// Ordering falls back to `POLICY_PRIORITY` on ties, so wherever the
/// counterfactual is silent — a card the engine gives no effect — the choice
/// is exactly the one this code has always made.
/// Score one candidate against the policy slate the review started with.
///
/// The caller restores the exact initial slate before the next score, making
/// candidate values independent. That lets a headless controller evaluate
/// this expensive counterfactual batch on worker-private game snapshots while
/// retaining the serial candidate order and deck commit below.
#[allow(clippy::too_many_arguments)]
pub(super) fn policy_card_score(
    g: &mut Game,
    pid: usize,
    w: &Weights,
    candidate: &(usize, String, Name),
    current_reading: f64,
    reads: &PolicyReadSet,
    net_maintenance: bool,
    player_income: bool,
) -> (f64, usize, String, Name) {
    let (priority, slot, card) = candidate;
    let incumbent = g.players[pid].policies.contains(card);
    // Every candidate shares one side of the counterfactual: an available
    // card is compared with the current slate, while an incumbent is compared
    // with the current slate after removing that card.  The old scorer walked
    // the whole empire for both sides on every card, repeating the unchanged
    // side dozens of times during one review.  Carrying the current reading
    // into the batch makes the score incremental: one whole-empire sweep for a
    // challenger, and one removal sweep for an incumbent.  The authoritative
    // candidate order and the exact `empire_reading` arithmetic are unchanged.
    // The other side of the counterfactual is also already known whenever the
    // card cannot reach anything `empire_reading` reads. `reads` is the record
    // of every question the unchanged sweep asked the deck; a card naming none
    // of those keys and none of those card names leaves every lookup returning
    // the value it already returned, so the two runs stay in lockstep and the
    // sweep lands on the same bits. Ten Great-Person, envoy and espionage
    // cards move only quantities this reading never looks at, and the review
    // used to pay a whole-empire sweep to rediscover that once per card, every
    // eight turns, for the rest of the game.
    let (without, with) = if reads.card_is_inert(&g.rules, *card) {
        (current_reading, current_reading)
    } else if incumbent {
        // `board_written` on both sides of each what-if: this seat's own
        // cities are never kept, but an ally's or a trade partner's are, and
        // nothing read under the rearranged deck may answer for the real one.
        g.players[pid].policies.remove(card);
        g.board_written();
        let without = empire_reading(g, pid, w, net_maintenance, player_income);
        g.players[pid].policies.insert(*card);
        g.board_written();
        (without, current_reading)
    } else {
        g.players[pid].policies.insert(*card);
        g.board_written();
        let with = empire_reading(g, pid, w, net_maintenance, player_income);
        g.players[pid].policies.remove(card);
        g.board_written();
        (current_reading, with)
    };
    let gain = with - without;
    // A sitting card keeps its slot unless beaten by the margin. Without
    // this the deck reshuffles on arithmetic noise every review.
    let hysteresis = if incumbent {
        w.pol_swap_margin * gain.abs()
    } else {
        0.0
    };
    (gain + hysteresis, *priority, slot.clone(), *card)
}

/// The unchanged reading every candidate is compared against, together with
/// the record of what it asked `pid`'s policy deck.
///
/// The trace costs one interned-name or one string comparison per policy
/// question of one sweep the review runs anyway, and it is what lets
/// [`policy_card_score`] answer a card without running a second one.
pub(super) fn reading_with_policy_trace(
    g: &Game,
    pid: usize,
    w: &Weights,
    net_maintenance: bool,
    player_income: bool,
) -> (f64, PolicyReadSet) {
    let trace = g.trace_policy_reads(pid);
    let reading = empire_reading(g, pid, w, net_maintenance, player_income);
    let reads = trace.finish();
    (reading, reads)
}

pub(super) fn revise_policy_deck(
    g: &mut Game,
    pid: usize,
    w: &Weights,
    pool: Option<&WorkPool>,
    net_maintenance: bool,
    player_income: bool,
    force_review: bool,
) {
    if g.host_policy_change_blocked(pid) {
        return;
    }
    let force_review = force_review || g.host_policy_change_window(pid) == Some(true);
    let slots = g.gov_slots(pid);
    let total = slots.military + slots.economic + slots.diplomatic + slots.wildcard;
    if total <= 0 {
        return;
    }
    if w.policy_deck == PolicyDeck::Empty {
        return;
    }
    if w.policy_deck == PolicyDeck::Legacy {
        if (g.players[pid].policies.len() as i64) < total {
            for card in POLICY_PRIORITY {
                let _ = g.apply(
                    pid,
                    &Action::SlotPolicy {
                        policy: Name::new(card),
                    },
                );
            }
        }
        return;
    }
    let held: Vec<Name> = g.players[pid].policies.iter().cloned().collect();
    if held.len() as i64 >= total && !g.turn.is_multiple_of(POLICY_REVIEW_EVERY) && !force_review {
        return;
    }

    let mut candidates = g.available_policies(pid);
    candidates.extend(held.iter().cloned());
    candidates.sort();
    candidates.dedup();

    let rank = |card: &str| {
        POLICY_PRIORITY
            .iter()
            .position(|entry| *entry == card)
            .unwrap_or(POLICY_PRIORITY.len())
    };

    let candidates: Vec<(usize, String, Name)> = candidates
        .into_iter()
        .filter_map(|card| {
            g.rules
                .policies
                .get(&card)
                .map(|spec| (rank(&card), spec.slot.clone(), card))
        })
        .collect();
    let mut scored: Vec<(f64, usize, String, Name)> =
        match pool.filter(|pool| pool.threads() > 1 && candidates.len() > 1) {
            Some(pool) => {
                // `Game` has worker-local RefCell caches and cannot be shared.
                // One snapshot per active worker keeps those caches private while
                // the pool's shared cursor balances uneven candidate costs.
                let active = pool
                    .threads()
                    .min(candidates.len())
                    .min(POLICY_SCORE_MAX_WORKERS);
                let states = (0..active).map(|_| g.clone()).collect::<Vec<_>>();
                let weights = w.clone();
                pool.map_stateful_limited(
                    candidates.len(),
                    POLICY_SCORE_MAX_WORKERS,
                    states,
                    move |mut branch, indices| {
                        // The branch is reused for all indices claimed by this
                        // worker.  Compute the unchanged slate once, before the
                        // candidate mutations begin, rather than once per card.
                        // The trace is taken on the branch, so each worker records
                        // its own and nothing crosses a thread.
                        let (current_reading, reads) = reading_with_policy_trace(
                            &branch,
                            pid,
                            &weights,
                            net_maintenance,
                            player_income,
                        );
                        indices
                            .map(|index| {
                                (
                                    index,
                                    policy_card_score(
                                        &mut branch,
                                        pid,
                                        &weights,
                                        &candidates[index],
                                        current_reading,
                                        &reads,
                                        net_maintenance,
                                        player_income,
                                    ),
                                )
                            })
                            .collect()
                    },
                )
            }
            None => {
                let (current_reading, reads) =
                    reading_with_policy_trace(g, pid, w, net_maintenance, player_income);
                candidates
                    .iter()
                    .map(|candidate| {
                        policy_card_score(
                            g,
                            pid,
                            w,
                            candidate,
                            current_reading,
                            &reads,
                            net_maintenance,
                            player_income,
                        )
                    })
                    .collect()
            }
        };

    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.1.cmp(&b.1))
            .then(a.3.cmp(&b.3))
    });

    let mut room = [slots.military, slots.economic, slots.diplomatic];
    let mut wildcard = slots.wildcard;
    let mut target: std::collections::BTreeSet<Name> = Default::default();
    for (_, _, slot, card) in &scored {
        let kind = match slot.as_str() {
            "military" => 0,
            "economic" => 1,
            "diplomatic" => 2,
            _ => 3, // a Wildcard card fits only a Wildcard slot
        };
        if kind < 3 && room[kind] > 0 {
            room[kind] -= 1;
            target.insert(*card);
        } else if wildcard > 0 {
            wildcard -= 1;
            target.insert(*card);
        }
    }

    for card in held {
        if !target.contains(&card) {
            let _ = g.apply(pid, &Action::UnslotPolicy { policy: card });
        }
    }
    for card in &target {
        if !g.players[pid].policies.contains(card) {
            let _ = g.apply(pid, &Action::SlotPolicy { policy: *card });
        }
    }
    // Floor: whatever the assignment above left empty, fill the way this code
    // always has. An empty slot is strictly worse than a card of unknown worth,
    // and this guarantees the change can never reduce occupancy.
    if (g.players[pid].policies.len() as i64) < total {
        for card in POLICY_PRIORITY {
            let _ = g.apply(
                pid,
                &Action::SlotPolicy {
                    policy: Name::new(card),
                },
            );
        }
    }
}
