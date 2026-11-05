//! Canals: the grand canals a map script cuts through land.
//!
//! Where the grand canals run (`grand_canals`, `canal_world`), their profile
//! and half width at each point, whether a tile is on one (`canal_at`), the
//! mean width and block count, the tiles they reserve, and the land blocks
//! they cut (`canal_blocks`).
//!
//! Moved verbatim out of `mapgen.rs` (2026-11-05). `mapgen.rs` re-exports
//! everything here, so every path is unchanged; items the rest of `mapgen`
//! uses are `pub(super)`.

use super::*;

/// The six canals of a Grand Canals world, as the tiles they take.
///
/// Two canals circle each of the world's three axes, cut
/// [`CANAL_OFFSET_DEGREES`] to either side of the great circle square to it.
/// Around the polar axis that reads as the parallels 20°N and 20°S, and the
/// other two pairs are the same construction turned a quarter turn: on Earth's
/// longitudes they cross the equator at 20°/160°, 200°/340°, 70°/290° and
/// 110°/250°. A tile belongs to a canal when its angle out of an axis's
/// equatorial plane is within half a canal's width of ±20°, which is a
/// question about where the tile is on the world rather than about the grid it
/// is stored in — so a globe and a flat map are cut by the same six lanes, and
/// every one of them closes on itself on either shape.
///
/// Because no two axes are parallel, every canal crosses all four belonging to
/// the other two axes, twice each: twenty-four junctions, and one connected
/// network rather than six separate rings. That is the point of the world —
/// the ground arrives already divided into blocks, and a ship can reach every
/// one of them.
pub(super) fn grand_canals(wm: &WorldMap) -> BTreeSet<Pos> {
    let half = canal_half_width(wm);
    let offset = CANAL_OFFSET_DEGREES.to_radians();
    wm.tiles
        .keys()
        .copied()
        .filter(|pos| {
            let point = wm.direction(*pos);
            CANAL_AXES.iter().any(|axis| {
                let out_of_plane = trig::asin(dot(point, *axis).clamp(-1.0, 1.0));
                (out_of_plane - offset).abs() <= half || (out_of_plane + offset).abs() <= half
            })
        })
        .collect()
}

/// Half a canal's width, as the angle it subtends at the centre of the world.
///
/// A canal is measured in tiles, because what matters about one is how many
/// ships fit abreast in it, and a tile is what the world is counted in. It is
/// widened with the world so that the six lanes take about the same share of a
/// Duel map as of a Ludicrous one: one tile of width for every
/// [`CANAL_TILES_PER_LAP`] tiles of the lap a canal makes, never fewer than
/// two and never more than six.
pub(super) fn canal_half_width(wm: &WorldMap) -> f64 {
    let step = tile_arc(wm);
    let lap = std::f64::consts::TAU / step;
    let tiles = (lap / CANAL_TILES_PER_LAP)
        .round()
        .clamp(CANAL_MIN_TILES, CANAL_MAX_TILES);
    tiles * step / 2.0
}

/// The land of a Grand Canals world: solid ground, less the six canals, less
/// whatever natural sea the world's land share still leaves room for.
///
/// The canals are cut first because they are geometry rather than luck — the
/// same six lanes on every seed — and what they take is then counted against
/// the water the world type asks for instead of being added on top of it.
/// A world small enough that the canals alone are more water than the share
/// allows gets no natural sea at all rather than a share it cannot honour: on
/// a Duel globe a lap is under sixty tiles, so six two-tile lanes are most of
/// the water there is room for.
///
/// What is held out of the field is what each shape holds out of it elsewhere:
/// a globe keeps its twelve pentagons and, when the world has poles, its caps;
/// a flat map keeps the top and bottom rows, which are its edge rather than
/// its climate.
pub(super) fn canal_world(wm: &WorldMap, poles: MapPoles, rng: &mut Rng) -> BTreeSet<Pos> {
    let canals = grand_canals(wm);
    let pentagons: BTreeSet<Pos> = wm
        .sphere()
        .map(|sphere| sphere.pentagons().into_iter().collect())
        .unwrap_or_default();
    let globe = wm.sphere().is_some();
    let cap = if globe && poles.has_poles() {
        0.93
    } else {
        f64::MAX
    };
    let reserved = |pos: Pos| {
        if pentagons.contains(&pos) || wm.polar_fraction(pos) >= cap {
            return true;
        }
        let (_, row) = hex::axial_to_offset(pos.0, pos.1);
        !globe && (row == 0 || row == wm.height - 1)
    };

    let mut field: BTreeSet<Pos> = wm
        .tiles
        .keys()
        .copied()
        .filter(|pos| !canals.contains(pos) && !reserved(*pos))
        .collect();

    let tiles = wm.tiles.len();
    let wanted_water = tiles * (100 - MapScript::GrandCanals.land_percent() as usize) / 100;
    let spent = tiles - field.len();
    let sea = match wanted_water.checked_sub(spent) {
        Some(remaining) if remaining > 0 => {
            // Few and large. A canal world is read by its lanes, and a scatter
            // of ponds across the blocks between them would make it hard to
            // tell which water was dug and which was always there.
            let seas = (tiles / 900).clamp(2, 12);
            scatter_bodies(wm, &mut field, seas, remaining, rng)
        }
        _ => BTreeSet::new(),
    };

    wm.tiles
        .keys()
        .copied()
        .filter(|pos| !canals.contains(pos) && !sea.contains(pos) && !reserved(*pos))
        .collect()
}

/// The canal two blocks share.
///
/// Which profile it is dug to is settled by the pair of blocks rather than by
/// the tile, so one canal is the same width for the whole of its run and the
/// world reads as dug rather than as eroded. The world's own salt goes into
/// the mix as well, so two worlds that happened on the same blocks would still
/// not dig the same canals between them.
fn canal_profile(one: usize, other: usize, salt: u64) -> CanalProfile {
    let (low, high) = (one.min(other) as u64, one.max(other) as u64);
    let mut mixed =
        salt ^ low.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ high.wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    mixed ^= mixed >> 29;
    mixed = mixed.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed ^= mixed >> 32;
    CANAL_PROFILES[(mixed % CANAL_PROFILES.len() as u64) as usize]
}

/// Whether a tile standing this far from its two nearest blocks has been dug
/// out as canal, and if so to which profile.
///
/// Half the difference of the two spans is how far the tile stands off the
/// line between the blocks, because a step across that line takes one tile off
/// one span and adds it to the other. A canal is dug along the line and half
/// its width to either side of it, so that distance against half the width is
/// the whole question.
fn canal_at(own: (usize, f64), rival: (usize, f64), salt: u64) -> Option<CanalProfile> {
    if rival.0 == usize::MAX {
        return None;
    }
    let profile = canal_profile(own.0, rival.0, salt);
    ((rival.1 - own.1) / 2.0 <= profile.width() / 2.0).then_some(profile)
}

/// How wide a canal runs on average, which is what a block's rim costs it.
fn canal_mean_width() -> f64 {
    CANAL_PROFILES
        .iter()
        .map(|profile| profile.width())
        .sum::<f64>()
        / CANAL_PROFILES.len() as f64
}

/// How many blocks a world with this much open ground is cut into.
///
/// A block of `block` land tiles sits in a cell of `cell` tiles, of which the
/// rim goes to the canals: `block = cell − rim·√cell`. Solve that for the cell
/// and the count is how many of them the world holds.
pub(super) fn canal_block_count(open: usize) -> usize {
    let rim = CANAL_BLOCK_RIM * canal_mean_width();
    let side = (rim + (rim * rim + 4.0 * CANAL_BLOCK_LAND_TILES).sqrt()) / 2.0;
    ((open as f64 / (side * side)).round() as usize).max(CANAL_MIN_BLOCKS)
}

/// The tiles a canal world holds out of its ground whatever else it does: a
/// globe's twelve pentagons and, when the world has poles, its caps; a flat
/// map's top and bottom rows, which are its edge rather than its climate.
fn canal_reserved(wm: &WorldMap, poles: MapPoles) -> BTreeSet<Pos> {
    let globe = wm.sphere().is_some();
    let cap = if globe && poles.has_poles() {
        0.93
    } else {
        f64::MAX
    };
    let pentagons: BTreeSet<Pos> = wm
        .sphere()
        .map(|sphere| sphere.pentagons().into_iter().collect())
        .unwrap_or_default();
    wm.tiles
        .keys()
        .copied()
        .filter(|pos| {
            if pentagons.contains(pos) || wm.polar_fraction(*pos) >= cap {
                return true;
            }
            let (_, row) = hex::axial_to_offset(pos.0, pos.1);
            !globe && (row == 0 || row == wm.height - 1)
        })
        .collect()
}

/// The world of Grand Canals II: ground cut into blocks of about
/// [`CANAL_BLOCK_LAND_TILES`] tiles each, and a canal around every one of
/// them.
///
/// **The blocks come first, and the canals are what is left between them.**
/// A block is grown from a seed, and a tile belongs to whichever seed is
/// fewest tiles away; a tile whose two nearest seeds are near enough to level
/// is on the line between two blocks, and that line is a canal. Because every
/// tile has a nearest seed and every block has a neighbour, the canals arrive
/// as one network rather than as a set of separate ditches — a fleet can leave
/// any block and reach any other.
///
/// **The seeds are spread, and then the blocks are settled.** Spreading takes
/// each new seed as far from the placed ones as the world allows, which gets
/// them apart but not even: the ground one seed happens to be given can be two
/// and a half times another's, and a world read by the size of its blocks
/// cannot afford that. Settling then moves each seed to the middle of its own
/// ground and lengthens the reach of any block left short of its share — which
/// is what evens out the blocks the world runs out under, against a pole or
/// against the edge of a flat map. Both steps ask nothing of the world's shape
/// beyond [`TileFrame`], so the same construction cuts a globe and a flat map,
/// and the blocks come out the same size on either.
///
/// **A canal has three layers**, and they are read off how far into the water
/// a tile is rather than off the geometry that cut it: the first
/// [`CanalProfile::shelf`] tiles in from either bank are shallow, and whatever
/// is left in the middle is deep ocean. Taken that way the shelf is there by
/// construction — no width of canal and no crossing of two of them can leave
/// deep water against a beach — and a junction where several canals meet opens
/// out into a small deep sea, which is what a junction of real canals does.
pub(super) fn canal_blocks(wm: &WorldMap, poles: MapPoles, rng: &mut Rng) -> CanalBlocks {
    let frame = TileFrame::of(wm);
    let reserved = canal_reserved(wm, poles);
    let open: Vec<usize> = (0..frame.tiles.len())
        .filter(|index| !reserved.contains(&frame.tiles[*index]))
        .collect();
    if open.len() < CANAL_MIN_BLOCKS {
        return CanalBlocks::default();
    }

    // The seeds, each one placed as far as it can be from every seed already
    // down. The first is rolled, so a world's blocks are its own.
    let count = canal_block_count(open.len()).min(open.len());
    let mut seeds: Vec<[f64; 3]> = Vec::with_capacity(count);
    seeds.push(frame.points[open[rng.below(open.len())]]);
    let mut nearest = vec![f64::MAX; open.len()];
    while seeds.len() < count {
        let placed = seeds[seeds.len() - 1];
        let mut farthest = 0usize;
        for (slot, index) in open.iter().enumerate() {
            let span = frame.span(frame.points[*index], placed);
            if span < nearest[slot] {
                nearest[slot] = span;
            }
            if nearest[slot] > nearest[farthest] {
                farthest = slot;
            }
        }
        seeds.push(frame.points[open[farthest]]);
    }

    // Spreading alone leaves the blocks uneven: the ground one seed happens to
    // be given can be two and a half times another's, and a world read by the
    // size of its blocks cannot afford that. Each round here does two things
    // to every seed — moves it to the middle of the ground that came to it,
    // which settles a block's shape, and then lengthens or shortens its reach
    // by what it is under or over its share, which settles the size. The
    // second is what the first cannot do: an even spread of seeds still gives
    // an uneven spread of ground wherever the world runs out, against a pole
    // or against the edge of a flat map.
    let salt = rng.next_u64();
    let cell = open.len() as f64 / count as f64;
    let limit = CANAL_BLOCK_SETTLING_LIMIT * cell.sqrt();
    let rim = 2.0 * CANAL_BLOCK_RIM * cell.sqrt();
    let mut reach = vec![0.0f64; count];
    for _ in 0..CANAL_BLOCK_SETTLING_ROUNDS {
        let mut middles = vec![([0.0f64; 3], 0usize); count];
        let mut ground = vec![0usize; count];
        for index in &open {
            let point = frame.points[*index];
            let (own, rival) = frame.nearest_two(&seeds, &reach, point);
            let toward = frame.toward_middle(point);
            let slot = &mut middles[own.0];
            for (axis, component) in toward.iter().enumerate() {
                slot.0[axis] += *component;
            }
            slot.1 += 1;
            if canal_at(own, rival, salt).is_none() {
                ground[own.0] += 1;
            }
        }
        // What is settled is the *ground* a block is left holding, not the
        // room it takes up. Those are not the same number: a block against the
        // edge of a flat map has water there already and pays no canal for it,
        // and a block whose canals happened to be dug wide pays more than one
        // whose canals are narrow. Aiming at the room would leave both of
        // those where they were.
        let held: usize = ground.iter().sum();
        let target = held as f64 / count as f64;
        for (block, (sum, room)) in middles.into_iter().enumerate() {
            if room == 0 {
                // Squeezed out from every side at once. Give it its plain
                // reach back and let the next round find it some ground.
                reach[block] = 0.0;
                continue;
            }
            if let Some(middle) = frame.middle(sum, room) {
                seeds[block] = middle;
            }
            // A block of `cell` tiles has a rim of that many tiles around it,
            // so pushing the rim out by one tile is that much more ground: the
            // reach a shortfall is worth is the shortfall divided by the rim.
            reach[block] = (reach[block]
                + CANAL_BLOCK_SETTLING_RATE * (target - ground[block] as f64) / rim)
                .clamp(-limit, limit);
        }
    }

    // Where the canals ended up, now that the blocks have settled.
    let mut canals: BTreeMap<Pos, CanalProfile> = BTreeMap::new();
    for (index, pos) in frame.tiles.iter().enumerate() {
        if reserved.contains(pos) {
            continue;
        }
        let (own, rival) = frame.nearest_two(&seeds, &reach, frame.points[index]);
        if let Some(profile) = canal_at(own, rival, salt) {
            canals.insert(*pos, profile);
        }
    }

    // The layers. How deep into the water a canal tile lies is one walk in
    // from every bank at once, so a tile knows which layer it is in without
    // anything having to know which way its canal happens to run.
    let mut inward: BTreeMap<Pos, usize> = BTreeMap::new();
    let mut frontier: Vec<Pos> = canals
        .keys()
        .copied()
        .filter(|pos| {
            wm.neighbors(*pos)
                .into_iter()
                .any(|neighbor| !canals.contains_key(&neighbor))
        })
        .collect();
    for pos in &frontier {
        inward.insert(*pos, 1);
    }
    let mut depth = 1usize;
    while !frontier.is_empty() {
        depth += 1;
        let mut next = Vec::new();
        for pos in frontier {
            for neighbor in wm.neighbors(pos) {
                if canals.contains_key(&neighbor) && !inward.contains_key(&neighbor) {
                    inward.insert(neighbor, depth);
                    next.push(neighbor);
                }
            }
        }
        frontier = next;
    }

    let mut plan = CanalBlocks::default();
    for (pos, profile) in &canals {
        if inward.get(pos).copied().unwrap_or(1) <= profile.shelf {
            plan.shelf.insert(*pos);
        } else {
            plan.channel.insert(*pos);
        }
    }

    // What the canals took is counted against the world's water rather than
    // added on top of it, exactly as [`canal_world`] counts its six lanes. A
    // world whose canals have already spent the share gets no natural sea.
    let mut field: BTreeSet<Pos> = frame
        .tiles
        .iter()
        .copied()
        .filter(|pos| !canals.contains_key(pos) && !reserved.contains(pos))
        .collect();
    let tiles = wm.tiles.len();
    let wanted_water = tiles * (100 - MapScript::GrandCanalsTwo.land_percent() as usize) / 100;
    let sea = match wanted_water.checked_sub(tiles - field.len()) {
        Some(remaining) if remaining > 0 => {
            let seas = (tiles / 900).clamp(2, 12);
            scatter_bodies(wm, &mut field, seas, remaining, rng)
        }
        _ => BTreeSet::new(),
    };
    plan.land = frame
        .tiles
        .iter()
        .copied()
        .filter(|pos| !canals.contains_key(pos) && !sea.contains(pos) && !reserved.contains(pos))
        .collect();

    // A block is somewhere a civilization lives. Where a canal has clipped a
    // corner off one — against a pole, or where three of them meet — what is
    // left over is a rock too small to found on, and a world read by the size
    // of its blocks should not be littered with them. They go back to the sea
    // they were cut out of.
    for sliver in connected_components(wm, &plan.land) {
        if sliver.len() < MIN_LANDMASS_FOR_A_START {
            for pos in sliver {
                plan.land.remove(&pos);
            }
        }
    }
    plan
}
