//! Placing a new world's resources: a port of the shipped
//! `ResourceGenerator.lua`.
//!
//! Luxuries and strategics are dealt one continent at a time, so a landmass
//! holds a handful of luxury kinds that no other land has. Sea luxuries and
//! offshore Oil go by latitude band, and bonus resources across the whole map.
//! Standard Resources aims to fill 30 percent of the plots it can use.

use super::*;
use crate::rules::ResourceSpec;

/// `iTargetPercentage` on the default Standard Resources setting.
const TARGET_PERCENT: f64 = 30.0;
/// `iLuxuryPercentage`: the luxuries' share of that target.
const LUXURY_PERCENT: f64 = 19.33;
/// `iStrategicPercentage`: the strategics' share of that target.
const STRATEGIC_PERCENT: f64 = 21.7;
/// `iLuxuriesPerRegion`: land luxury kinds dealt to each continent.
const LUXURIES_PER_CONTINENT: usize = 4;
/// `Resource_SeaLuxuries` after Rise and Fall's updates. Rows are `iWaterLux`
/// from 1, columns Duel through Huge.
const SEA_LUXURIES: [[usize; 6]; 4] = [
    [0, 0, 0, 0, 0, 0],
    [1, 1, 1, 2, 2, 2],
    [1, 2, 2, 2, 3, 3],
    [2, 2, 3, 3, 4, 4],
];
/// `Resource_SeaStrategics` by `iWaterLux`: the same at every map size.
const SEA_STRATEGICS: [usize; 4] = [0, 1, 1, 1];
/// Gathering Storm's `Resource_Distribution` has one row for each continent
/// count through this many.
const DISTRIBUTION_ROWS: usize = 8;
/// Each of those rows holds seven Average entries, read one per continent. A
/// continent past the seventh, or on a world with no row, is left with no
/// weight and takes the one deposit of each strategic that a zero rounds up
/// to.
const DISTRIBUTION_ENTRIES: usize = 7;

/// What one kind of sea resource needs from `deal_sea_kinds`.
struct SeaDeal {
    class: &'static str,
    /// Kinds the size table asks for.
    wanted: usize,
    percent: f64,
    divisor: f64,
    /// Sea luxuries scale their roll and their cap by `SeaFrequency`, and Oil
    /// does not.
    scales_with_frequency: bool,
}

/// Deal a new world its luxuries, strategics and bonus resources.
pub(super) fn place_resources(
    rules: &Rules,
    wm: &mut WorldMap,
    script: MapScript,
    size: &MapSize,
    rng: &mut Rng,
) {
    let (water_row, water_bonus) = match script {
        MapScript::Pangaea => (2, 1.25),
        MapScript::InlandSea => (1, 1.0),
        MapScript::Islands => (4, 1.25),
        _ => (3, 1.25),
    };
    let column = match size.id {
        "duel" => 0,
        "tiny" => 1,
        "small" => 2,
        "standard" => 3,
        "large" => 4,
        _ => 5,
    };
    let plots = plots_in_column_order(wm);
    let continents = continent_plots(rules, wm, &plots);

    deal_land_luxuries(rules, wm, &continents, water_row, rng);
    deal_sea_kinds(
        rules,
        wm,
        &plots,
        SeaDeal {
            class: "luxury",
            wanted: SEA_LUXURIES[water_row - 1][column],
            percent: LUXURY_PERCENT,
            divisor: 2.0,
            scales_with_frequency: true,
        },
        rng,
    );
    deal_land_strategics(rules, wm, &continents, rng);
    deal_sea_kinds(
        rules,
        wm,
        &plots,
        SeaDeal {
            class: "strategic",
            wanted: SEA_STRATEGICS[water_row - 1],
            percent: STRATEGIC_PERCENT,
            divisor: 12.0,
            scales_with_frequency: false,
        },
        rng,
    );
    deal_bonus(rules, wm, &plots, |spec| spec.frequency, 1.0, rng);
    deal_bonus(
        rules,
        wm,
        &plots,
        |spec| spec.sea_frequency,
        water_bonus,
        rng,
    );
    thin_crowded_bonus(rules, wm, &plots);
}

/// `ResourceBuilder.CanHaveResource`, read from where the shipped game puts
/// resources: a plot with a feature is decided by that feature alone, whatever
/// the terrain beneath it, so Coal lies in flat Woods although it needs hills
/// in the open.
pub(super) fn can_have_resource(rules: &Rules, wm: &WorldMap, pos: Pos, resource: Name) -> bool {
    let tile = &wm.tiles[&pos];
    let spec = &rules.resources[&resource];
    if tile.resource.is_some() {
        return false;
    }
    if let Some(feature) = &tile.feature {
        return spec.feature.contains(feature);
    }
    if !spec.takes_terrain(tile.terrain, tile.hills) {
        return false;
    }
    !spec.adjacent_to_land
        || !rules.is_water(tile)
        || wm
            .neighbors(pos)
            .into_iter()
            .any(|neighbor| wm.tiles.get(&neighbor).is_some_and(|t| !rules.is_water(t)))
}

/// Every plot in the order the shipped script scans them, column by column.
fn plots_in_column_order(wm: &WorldMap) -> Vec<Pos> {
    let mut plots: Vec<Pos> = wm.tiles.keys().copied().collect();
    plots.sort_by_key(|&(q, r)| hex::axial_to_offset(q, r));
    plots
}

/// Each continent's land plots. Land no continent claims counts as one more,
/// so a small test board still receives resources.
fn continent_plots(rules: &Rules, wm: &WorldMap, plots: &[Pos]) -> Vec<Vec<Pos>> {
    let mut continents: BTreeMap<usize, Vec<Pos>> = BTreeMap::new();
    for pos in plots {
        let tile = &wm.tiles[pos];
        if !rules.is_water(tile) {
            continents
                .entry(tile.continent.unwrap_or(usize::MAX))
                .or_default()
                .push(*pos);
        }
    }
    continents.into_values().collect()
}

fn kinds_of(rules: &Rules, class: &str, weight: impl Fn(&ResourceSpec) -> u32) -> Vec<Name> {
    rules
        .resources
        .iter()
        .filter(|(_, spec)| spec.class == class && weight(spec) > 0)
        .map(|(name, _)| *name)
        .collect()
}

fn shuffle<T>(items: &mut [T], rng: &mut Rng) {
    for last in (1..items.len()).rev() {
        items.swap(last, rng.below(last + 1));
    }
}

fn adjacent_resources(wm: &WorldMap, pos: Pos) -> usize {
    wm.neighbors(pos)
        .into_iter()
        .filter(|neighbor| {
            wm.tiles
                .get(neighbor)
                .is_some_and(|tile| tile.resource.is_some())
        })
        .count()
}

fn adjacent_to_ice(wm: &WorldMap, pos: Pos) -> bool {
    wm.neighbors(pos).into_iter().any(|neighbor| {
        wm.tiles
            .get(&neighbor)
            .is_some_and(|tile| tile.feature.as_deref() == Some("ice"))
    })
}

/// A site's score: open ground first, with a roll to spread the choice.
fn site_score(wm: &WorldMap, pos: Pos, crowding: f64, rng: &mut Rng) -> f64 {
    500.0 / ((adjacent_resources(wm, pos) + 1) as f64 * crowding) + rng.below(100) as f64
}

/// Best site first. Lua's `table.sort` leaves equal scores in no particular
/// order, so ties are shuffled rather than falling to the western columns.
fn ranked(mut sites: Vec<(f64, Pos)>, rng: &mut Rng) -> Vec<Pos> {
    shuffle(&mut sites, rng);
    sites.sort_by(|a, b| b.0.total_cmp(&a.0));
    sites.into_iter().map(|(_, pos)| pos).collect()
}

fn set_resource(wm: &mut WorldMap, sites: &[Pos], count: usize, resource: Name) {
    for pos in sites.iter().take(count) {
        wm.tiles.get_mut(pos).unwrap().resource = Some(resource);
    }
}

/// The plots of a continent that can take `resource` with at most one
/// resource beside them, best first.
fn continent_sites(
    rules: &Rules,
    wm: &WorldMap,
    plots: &[Pos],
    resource: Name,
    crowding: f64,
    shuns_ice: bool,
    rng: &mut Rng,
) -> Vec<Pos> {
    let mut sites = Vec::new();
    for &pos in plots {
        if can_have_resource(rules, wm, pos, resource)
            && !(shuns_ice && adjacent_to_ice(wm, pos))
            && adjacent_resources(wm, pos) <= 1
        {
            sites.push((site_score(wm, pos, crowding, rng), pos));
        }
    }
    ranked(sites, rng)
}

/// Four luxury kinds to each continent from one shuffled deck, so a kind lies
/// on a single continent. A kind drawn where nothing can hold it is spent.
fn deal_land_luxuries(
    rules: &Rules,
    wm: &mut WorldMap,
    continents: &[Vec<Pos>],
    water_row: usize,
    rng: &mut Rng,
) {
    let mut deck = kinds_of(rules, "luxury", |spec| spec.frequency);
    shuffle(&mut deck, rng);
    for plots in continents {
        // The script halves the count on its land-heavy map.
        let counted = plots.len() as f64 / if water_row == 1 { 2.0 } else { 1.0 };
        let each = TARGET_PERCENT / 100.0 * counted * LUXURY_PERCENT
            / 100.0
            / LUXURIES_PER_CONTINENT as f64;
        let each = if each > 1.0 { each as usize } else { 1 };
        let (mut dealt, mut index, mut failed) = (0, 0, 0);
        while dealt < LUXURIES_PER_CONTINENT && failed < 2 {
            let drawn = deck.get(index).copied();
            if let Some(luxury) = drawn {
                deck.remove(index);
                let sites = continent_sites(rules, wm, plots, luxury, 3.75, true, rng);
                set_resource(wm, &sites, each, luxury);
                index += 1;
                dealt += 1;
                failed = 0;
            }
            if index >= deck.len() {
                index = 0;
                failed += 1;
            } else if drawn.is_none() {
                failed += 1;
            }
        }
    }
}

/// Each strategic on each continent in proportion to its `Frequency`, and at
/// least one deposit of each where the land allows it.
fn deal_land_strategics(rules: &Rules, wm: &mut WorldMap, continents: &[Vec<Pos>], rng: &mut Rng) {
    let mut kinds = kinds_of(rules, "strategic", |spec| spec.frequency);
    let total: f64 = kinds
        .iter()
        .map(|kind| rules.resources[kind].frequency as f64)
        .sum();
    for (index, plots) in continents.iter().enumerate() {
        let weight = if continents.len() <= DISTRIBUTION_ROWS && index < DISTRIBUTION_ENTRIES {
            1.0
        } else {
            0.0
        };
        let occurrences = plots.len() as f64 * TARGET_PERCENT / 100.0 * STRATEGIC_PERCENT / 100.0;
        // The script sorts these by a count that is zero for every strategic,
        // which leaves them in their shuffled order.
        shuffle(&mut kinds, rng);
        for &kind in &kinds {
            let frequency = rules.resources[&kind].frequency as f64;
            let share = occurrences * frequency / total * weight;
            let count = if frequency > 1.0 && share < 1.0 {
                1
            } else {
                share as usize
            };
            let sites = continent_sites(rules, wm, plots, kind, 15.0, false, rng);
            set_resource(wm, &sites, count, kind);
        }
    }
}

/// Sea luxuries or offshore Oil: every water plot rolls in turn, west to east,
/// through the polar band and then the equatorial one, until the kind's cap.
fn deal_sea_kinds(rules: &Rules, wm: &mut WorldMap, plots: &[Pos], deal: SeaDeal, rng: &mut Rng) {
    let mut deck = kinds_of(rules, deal.class, |spec| spec.sea_frequency);
    shuffle(&mut deck, rng);
    if deal.wanted == 0 {
        return;
    }
    let mut odd = deal.wanted % 2 == 1;
    let per_band = deal.wanted / 2 + usize::from(odd);
    let water: Vec<Pos> = plots
        .iter()
        .copied()
        .filter(|pos| rules.is_water(&wm.tiles[pos]) && !adjacent_to_ice(wm, *pos))
        .collect();
    let shallow = water
        .iter()
        .filter(|pos| matches!(wm.tiles[*pos].terrain.as_str(), "coast" | "lake"))
        .count();
    let occurrences = TARGET_PERCENT / 100.0 * shallow as f64 * deal.percent
        / 100.0
        / per_band as f64
        / deal.divisor;
    let half_height = wm.height as f64 / 2.0;
    for (highest, lowest) in [(100.0, 35.1), (35.0, 0.0)] {
        for index in 0..per_band {
            let Some(&chosen) = deck.get(index) else {
                return;
            };
            let frequency = if deal.scales_with_frequency {
                rules.resources[&chosen].sea_frequency as f64
            } else {
                1.0
            };
            let cap = occurrences * frequency;
            let (mut placed, mut first, mut odd_switch) = (0usize, true, false);
            for &pos in &water {
                let row = hex::axial_to_offset(pos.0, pos.1).1 as f64;
                let latitude = (half_height - row).abs() / half_height * 100.0;
                if latitude < highest
                    && latitude > lowest
                    && placed as f64 <= cap
                    && sea_plot_takes(rules, wm, pos, chosen, occurrences, frequency, rng)
                {
                    if first {
                        if odd {
                            odd_switch = true;
                        } else {
                            deck.remove(0);
                        }
                        first = false;
                    }
                    placed += 1;
                }
                if odd_switch {
                    odd = false;
                }
            }
        }
    }
}

/// One water plot's roll for a sea resource. A neighbouring resource divides
/// the roll, so these seldom touch.
fn sea_plot_takes(
    rules: &Rules,
    wm: &mut WorldMap,
    pos: Pos,
    resource: Name,
    occurrences: f64,
    frequency: f64,
    rng: &mut Rng,
) -> bool {
    if !can_have_resource(rules, wm, pos, resource) {
        return false;
    }
    let die = (15.0 * occurrences + 300.0) as usize;
    let score = rng.below(die) as f64 / ((adjacent_resources(wm, pos) + 1) as f64 * 3.0);
    if score * frequency < 85.0 + 5.0 * occurrences {
        return false;
    }
    wm.tiles.get_mut(&pos).unwrap().resource = Some(resource);
    true
}

/// Bonus resources across the whole map, in proportion to their weights.
fn deal_bonus(
    rules: &Rules,
    wm: &mut WorldMap,
    plots: &[Pos],
    weight: impl Fn(&ResourceSpec) -> u32,
    multiplier: f64,
    rng: &mut Rng,
) {
    let mut kinds = kinds_of(rules, "bonus", &weight);
    shuffle(&mut kinds, rng);
    let total: f64 = kinds
        .iter()
        .map(|kind| weight(&rules.resources[kind]) as f64)
        .sum();
    if total == 0.0 {
        return;
    }
    let valid = plots
        .iter()
        .filter(|pos| {
            kinds
                .iter()
                .any(|kind| can_have_resource(rules, wm, **pos, *kind))
        })
        .count();
    let per_weight =
        TARGET_PERCENT / 100.0 * valid as f64 * (100.0 - STRATEGIC_PERCENT - LUXURY_PERCENT)
            / 100.0
            / total
            * multiplier;
    for kind in kinds {
        let mut sites = Vec::new();
        for &pos in plots {
            if can_have_resource(rules, wm, pos, kind) {
                sites.push((site_score(wm, pos, 1.1, rng), pos));
            }
        }
        let sites = ranked(sites, rng);
        let count = (per_weight * weight(&rules.resources[&kind]) as f64) as usize;
        set_resource(wm, &sites, count, kind);
    }
}

/// A bonus resource with two or more of its own kind beside it is removed.
pub(super) fn thin_crowded_bonus(rules: &Rules, wm: &mut WorldMap, plots: &[Pos]) {
    for pos in plots {
        let Some(resource) = wm.tiles[pos].resource else {
            continue;
        };
        if rules.resources[&resource].class != "bonus" {
            continue;
        }
        let same = wm
            .neighbors(*pos)
            .into_iter()
            .filter(|neighbor| {
                wm.tiles
                    .get(neighbor)
                    .is_some_and(|tile| tile.resource == Some(resource))
            })
            .count();
        if same >= 2 {
            wm.tiles.get_mut(pos).unwrap().resource = None;
        }
    }
}
