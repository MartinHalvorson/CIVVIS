//! Which city-states a world seats, and in what order.
//!
//! Seating used to be the roster's own order, taken from the top: `wanted`
//! seats meant Kabul, Geneva, Hattusa, Mohenjo-Daro, … every game, on every
//! map, for ever. Nothing rolled, so nothing varied — and because
//! `data/city_states.json` groups the shipped forty-eight by the order
//! Civilization VI lists them and then runs through Magna Graecia, the Levant,
//! the Aegean, Etruria and the Hansa in blocks, taking the top of a longer
//! roster would have been worse than arbitrary: a twelve-seat world would have
//! drawn eleven of its twelve city-states from the Mediterranean.
//!
//! Two properties decide the draw instead:
//!
//! * **Type balance.** The six Suzerain types are dealt round-robin, so a
//!   twelve-seat world gets two of each and a fourteen-seat world gets two of
//!   each plus two more, rather than whatever the roster's order happened to
//!   put at the top. Which two types get the odd seats rotates with the seed.
//! * **Spread.** Within a type, the seat taken is the one whose real site is
//!   farthest from every site already taken — ordinary farthest-point
//!   dispersion over the globe. This is what stops six Sicilian towns and
//!   four Syrian tells from crowding into one world while whole continents go
//!   unrepresented.
//!
//! Both run on every script, not only True Start Earth. The positions only
//! *place* anything on a true-start map, but a world whose city-states are
//! drawn from six continents and six types reads differently from one that
//! drew the first N of a list wherever they end up standing.
//!
//! The shipped forty-eight are still exhausted before any other identity is
//! reached, so an ordinary game seats only city-states the real game could
//! have seated. That was true of the old order and stays true of this one.

use crate::rng::Rng;
use crate::rules::CityStateSpec;
use crate::sphere::trig;

/// The six Suzerain types, in the order the roster declares them.
const TYPES: [&str; 6] = [
    "militaristic",
    "scientific",
    "cultural",
    "religious",
    "trade",
    "industrial",
];

/// A site as a unit vector on the sphere, so "far apart" is one dot product
/// rather than a great-circle formula with a pole special case.
fn direction(latitude: f64, longitude: f64) -> [f64; 3] {
    let (lat, lon) = (latitude.to_radians(), longitude.to_radians());
    [
        trig::cos(lat) * trig::cos(lon),
        trig::cos(lat) * trig::sin(lon),
        trig::sin(lat),
    ]
}

/// How far apart two sites are, as `1 - cos(angle)`: 0 for the same place and
/// 2 for the far side of the world. Monotone in the true distance, which is
/// all a farthest-point rule needs.
fn separation(a: [f64; 3], b: [f64; 3]) -> f64 {
    2.0 - (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) - 1.0
}

/// The roster indices to seat, in seating order.
///
/// Deterministic in `rng`, so the same seed lays out the same world; the
/// caller passes a stream of its own so drawing city-states cannot shift the
/// numbers the map generator would otherwise have rolled.
pub fn seat_selection(roster: &[CityStateSpec], wanted: usize, rng: &mut Rng) -> Vec<usize> {
    let wanted = wanted.min(roster.len());
    if wanted == 0 {
        return Vec::new();
    }
    // Tier 1 is the shipped forty-eight, tier 2 everything else. A tier is
    // emptied before the next is touched.
    let mut tiers: Vec<Vec<usize>> = vec![
        (0..roster.len()).filter(|&i| roster[i].shipped).collect(),
        (0..roster.len()).filter(|&i| !roster[i].shipped).collect(),
    ];
    tiers.retain(|tier| !tier.is_empty());

    // Whose turn it is to be dealt first rotates, so the type that gets the
    // odd seat in a world that does not divide by six is not always the same
    // one. The order within the rotation stays the roster's.
    let offset = (rng.next_u64() % TYPES.len() as u64) as usize;

    let mut chosen: Vec<usize> = Vec::with_capacity(wanted);
    let mut placed: Vec<[f64; 3]> = Vec::with_capacity(wanted);
    let mut turn = 0usize;
    while chosen.len() < wanted {
        let before = chosen.len();
        for step in 0..TYPES.len() {
            if chosen.len() == wanted {
                break;
            }
            let kind = TYPES[(offset + turn + step) % TYPES.len()];
            let Some(tier) = tiers
                .iter_mut()
                .find(|tier| tier.iter().any(|&index| roster[index].kind == kind))
            else {
                continue; // no identity of this type is left anywhere
            };
            let pick = farthest(roster, tier, kind, &placed, rng);
            let index = tier.swap_remove(pick);
            if let Some(site) = roster[index].site() {
                placed.push(direction(site.latitude, site.longitude));
            }
            chosen.push(index);
        }
        tiers.retain(|tier| !tier.is_empty());
        turn += 1;
        // Every type is exhausted in every tier: the roster simply has no more
        // identities, and a seat without one cannot be filled.
        if chosen.len() == before {
            break;
        }
    }
    chosen
}

/// The position within `tier` of the `kind` candidate farthest from everything
/// already placed.
///
/// Ties are broken by reservoir sampling rather than by taking the first,
/// because the very first pick of a game ties every candidate at once — there
/// is nothing placed to be far from — and always answering "the first
/// militaristic city-state in the roster" would put Kabul in every world just
/// as surely as the old order did.
fn farthest(
    roster: &[CityStateSpec],
    tier: &[usize],
    kind: &str,
    placed: &[[f64; 3]],
    rng: &mut Rng,
) -> usize {
    let mut best = f64::NEG_INFINITY;
    let mut seen = 0u64;
    let mut pick = 0usize;
    for (position, &index) in tier.iter().enumerate() {
        if roster[index].kind != kind {
            continue;
        }
        // An identity with no coordinates cannot be spread, so it sorts below
        // every located one and is reached only when they run out. Nothing in
        // the shipped roster is in this state; a mod overlay's rows can be.
        let score = match roster[index].site() {
            Some(site) => {
                let direction = direction(site.latitude, site.longitude);
                placed
                    .iter()
                    .map(|other| separation(direction, *other))
                    .fold(f64::INFINITY, f64::min)
            }
            None => f64::NEG_INFINITY,
        };
        if score > best + f64::EPSILON {
            best = score;
            seen = 1;
            pick = position;
        } else if (score - best).abs() <= f64::EPSILON {
            seen += 1;
            if rng.next_u64() % seen == 0 {
                pick = position;
            }
        }
    }
    pick
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::Rules;

    fn roster() -> Vec<CityStateSpec> {
        Rules::shipped().city_states.roster.clone()
    }

    #[test]
    fn every_shipped_city_state_carries_a_real_site() {
        for seat in roster() {
            let site = seat
                .site()
                .unwrap_or_else(|| panic!("{} has no coordinates", seat.name));
            assert!(
                (-90.0..=90.0).contains(&site.latitude),
                "{} latitude {}",
                seat.name,
                site.latitude
            );
            assert!(
                (-180.0..=180.0).contains(&site.longitude),
                "{} longitude {}",
                seat.name,
                site.longitude
            );
            // 0,0 is the Gulf of Guinea and the value a forgotten field would
            // take. No city-state in this roster stood within a degree of it.
            assert!(
                site.latitude.abs() > 1.0 || site.longitude.abs() > 1.0,
                "{} looks like an unset coordinate",
                seat.name
            );
        }
    }

    #[test]
    fn the_six_types_are_dealt_evenly() {
        let roster = roster();
        for wanted in [1usize, 6, 12, 13, 24, 48] {
            let mut rng = Rng::new(7 * wanted as u64 + 1);
            let chosen = seat_selection(&roster, wanted, &mut rng);
            assert_eq!(chosen.len(), wanted);
            let mut counts = std::collections::BTreeMap::new();
            for index in &chosen {
                *counts.entry(roster[*index].kind.clone()).or_insert(0usize) += 1;
            }
            let high = counts.values().copied().max().unwrap_or(0);
            let low = if counts.len() < TYPES.len() {
                0
            } else {
                counts.values().copied().min().unwrap_or(0)
            };
            assert!(
                high - low <= 1,
                "{wanted} seats split {counts:?}, which is not a round-robin"
            );
        }
    }

    #[test]
    fn a_seat_is_never_handed_out_twice() {
        let roster = roster();
        let mut rng = Rng::new(99);
        let chosen = seat_selection(&roster, roster.len(), &mut rng);
        let unique: std::collections::BTreeSet<usize> = chosen.iter().copied().collect();
        assert_eq!(unique.len(), chosen.len());
        assert_eq!(chosen.len(), roster.len());
    }

    #[test]
    fn the_shipped_forty_eight_are_exhausted_before_any_other_identity() {
        let roster = roster();
        let shipped = roster.iter().filter(|seat| seat.shipped).count();
        let mut rng = Rng::new(4);
        for index in seat_selection(&roster, shipped, &mut rng) {
            assert!(
                roster[index].shipped,
                "{} is not one of the shipped forty-eight",
                roster[index].name
            );
        }
    }

    #[test]
    fn the_draw_is_spread_across_the_world_rather_than_one_region() {
        // The old order — the first twelve of the roster — is the control.
        // Both are scored the same way: the mean distance from the group's own
        // centre, in units where 2 is the far side of the world.
        let roster = roster();
        let spread = |indices: &[usize]| -> f64 {
            let points: Vec<[f64; 3]> = indices
                .iter()
                .filter_map(|&i| roster[i].site())
                .map(|site| direction(site.latitude, site.longitude))
                .collect();
            let mut total = 0.0;
            let mut pairs = 0.0;
            for first in 0..points.len() {
                for second in (first + 1)..points.len() {
                    total += separation(points[first], points[second]);
                    pairs += 1.0;
                }
            }
            total / pairs
        };
        let old: Vec<usize> = (0..12).collect();
        for seed in 0..8u64 {
            let mut rng = Rng::new(seed);
            let chosen = seat_selection(&roster, 12, &mut rng);
            assert!(
                spread(&chosen) > spread(&old),
                "seed {seed} drew a tighter cluster than the roster's own first twelve"
            );
        }
    }

    #[test]
    fn two_names_for_one_place_are_not_both_seated_while_the_world_has_room() {
        // Visby/Wisby, Turku City/Abo, Bandar Brunei/Brunei and
        // Ayutthaya/Ayutthaya City are the same site twice. A farthest-point
        // draw should never take the second of a pair while anywhere else is
        // still free, and this is the check that says so.
        let roster = roster();
        let mut sites: std::collections::BTreeMap<String, Vec<usize>> = Default::default();
        for (index, seat) in roster.iter().enumerate() {
            if let Some(site) = seat.site() {
                sites
                    .entry(format!("{:.2},{:.2}", site.latitude, site.longitude))
                    .or_default()
                    .push(index);
            }
        }
        let twinned: Vec<Vec<usize>> = sites.into_values().filter(|at| at.len() > 1).collect();
        assert_eq!(
            twinned.len(),
            4,
            "the roster should hold four twinned sites"
        );
        for seed in 0..8u64 {
            let mut rng = Rng::new(seed);
            let chosen: std::collections::BTreeSet<usize> =
                seat_selection(&roster, 24, &mut rng).into_iter().collect();
            for pair in &twinned {
                let both = pair.iter().filter(|index| chosen.contains(index)).count();
                assert!(
                    both <= 1,
                    "seed {seed} seated one site under both its names"
                );
            }
        }
    }

    #[test]
    fn the_same_seed_lays_out_the_same_world() {
        let roster = roster();
        let draw = |seed: u64| seat_selection(&roster, 16, &mut Rng::new(seed));
        assert_eq!(draw(2024), draw(2024));
        assert_ne!(draw(1), draw(2), "different seeds should draw differently");
    }
}
