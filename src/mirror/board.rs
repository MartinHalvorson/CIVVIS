//! The board: writing what the seat sees of the map into the rebuilt game.
//!
//! `apply_terrain` and `apply_landmass` lay out the land, `apply_rivers`
//! the rivers, and `apply_territory` the borders. `apply_finished_improvements`
//! places the improvements already built, `apply_observed_plot_yields` sets
//! the host's own plot yields, and `apply_tile_memory` keeps what the seat
//! remembers under fog.
//!
//! Moved verbatim out of `mirror.rs` (2026-11-05). `mirror.rs` re-exports
//! everything here, so every path is unchanged. Items the rest of `mirror`
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

/// Fold `improved` events onto the assembled map, so a finished improvement is
/// on the board before the next sweep repeats it.
///
/// ★★★★★ The sweep runs every few turns and until it does, the mirror shows the
/// ground bare — so CIVVIS re-orders what it has just built. Measured on run
/// `civvis-20260811T163652Z`: 23 duplicate improvement orders, every refusal 1–3
/// turns after a sweep, the ledger reading `IMPROVE:MINE` succeeded at t18 and
/// refused at t19 against `existing=IMPROVEMENT_MINE`.
///
/// Three rules make this safe, and each one is load-bearing:
///
/// 1. **Only the `im` field is touched.** [`Snapshot::from_chunks`] REPLACES a
///    plot (`revealed.insert(pos, plot.clone())`), so folding a partial plot in
///    as a one-plot chunk — the obvious cheap version — would strip that tile's
///    terrain, owner and resource. Mutating the one field cannot.
/// 2. **Only a plot the seat has already revealed.** An improvement on ground
///    never seen is not evidence the ground exists, and inventing a plot here
///    would hand the simulator information the seat does not have.
/// 3. **Only events at or after the newest chunk.** An older event cannot
///    override a fresher sweep, which is what keeps a removed improvement from
///    coming back.
///
/// ⚠ It does open a narrow window in the other direction: a tile improved and
/// then PILLAGED before the next sweep reads as improved until that sweep
/// corrects it. Pillaging is far rarer than building, the window is the same few
/// turns, and the sweep is authoritative either way — so this trades a common
/// error for a rare one rather than removing error altogether.
pub(super) fn apply_finished_improvements(
    raw: &str,
    turn: Option<u32>,
    state_start: Option<usize>,
    snapshot: &mut Snapshot,
) {
    for (start, end) in line_ranges_containing(raw, "\"improved\"") {
        if state_start.is_some_and(|limit| start > limit) {
            break;
        }
        let line = &raw[start..end];
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|k| k.as_str()) != Some("improved") {
            continue;
        }
        let at = event.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if turn.is_some_and(|limit| at > limit) || at < snapshot.turn {
            continue;
        }
        let (Some(x), Some(y), Some(im)) = (
            event.get("x").and_then(|v| v.as_i64()),
            event.get("y").and_then(|v| v.as_i64()),
            event.get("im").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        if let Some(plot) = snapshot.revealed.get_mut(&(x as i32, y as i32)) {
            if plot.im.as_deref() != Some(im) {
                plot.io = None;
            }
            plot.im = Some(im.to_string());
            // The plot's exported yields were read before this improvement
            // existed; see `Snapshot::set_improvement` for why they cannot
            // survive it.
            plot.yl = None;
        }
    }
}

/// Write every plot the seat has seen onto the map, and explicit unknowns elsewhere.
///
/// Shared by the one-shot rebuild and by [`LiveMirror::sync`], which has to re-apply
/// it as ground is revealed. An unresolved terrain name is unknown too: retaining the
/// generated tile below it would turn a translation failure into plausible fiction.
pub(crate) fn apply_terrain(game: &mut crate::game::Game, snapshot: &Snapshot) {
    let vocab = Vocabulary::embedded();
    let unknown = Name::new("unknown");
    let visible_resources: std::collections::BTreeSet<crate::name::Name> = game
        .rules
        .resources
        .keys()
        .filter(|name| game.resource_visible_to(0, name.as_str()))
        .cloned()
        .collect();
    let width = snapshot.width.max(1);
    let height = snapshot.height.max(1);
    // ⚠ THE REAL IMPROVEMENT SET, taken from the ruleset ONCE before the loop.
    //
    // This used to be a hardcoded list of sixteen names, because checking the ruleset
    // inside the loop needs a second borrow of `game` while `game.map.tiles` is held
    // mutably. The list then drifted: CIVVIS models **36** improvements and the list
    // named 16, so TWENTY modelled improvements read as unimproved ground — including
    // `barbarian_camp`, `goody_hut` and `meteor_goody`, which are precisely the three
    // `AdvancedAi` looks for when deciding whether a move invalidates a plan
    // (`advanced.rs`, `invalidates_followers`).
    //
    // Found on live run `civvis-20260801T141601Z`: the mirror check reported
    // `IMPROVEMENT_BARBARIAN_CAMP` among "improvements CIVVIS does not model", which
    // contradicted the ruleset. Hoisting the lookup costs one allocation per rebuild
    // and cannot drift again.
    let modelled_improvements: std::collections::BTreeSet<String> = game
        .rules
        .improvements
        .keys()
        .map(|name| name.as_str().to_string())
        .collect();
    let mut unknown_positions = std::collections::BTreeSet::new();
    for y in 0..height {
        for x in 0..width {
            let pos = crate::hex::offset_to_axial(x, y);
            let Some(tile) = game.map.tiles.get_mut(&pos) else {
                continue;
            };
            let Some(plot) = snapshot.plot((x, y)) else {
                unknown_positions.insert(pos);
                // Keep `assumed_traversable`: `grow_frontier` owns that planning
                // prior and recalculates it after this pass. Everything else came
                // from the generated placeholder world and must be erased.
                tile.terrain = unknown;
                tile.hills = false;
                tile.feature = None;
                tile.resource = None;
                tile.improvement = None;
                tile.improvement_owner = None;
                tile.pillaged = false;
                tile.district = None;
                tile.district_foundation = None;
                tile.wonder = None;
                tile.owner_city = None;
                tile.road = 0;
                tile.continent = None;
                tile.coastal_lowland = 0;
                tile.flooded = false;
                tile.submerged = false;
                tile.drought = false;
                tile.storm = None;
                tile.fallout_until = 0;
                tile.disaster_faith = 0.0;
                tile.disaster_food = 0.0;
                tile.disaster_production = 0.0;
                continue;
            };
            tile.assumed_traversable = false;
            tile.assumed_navigable = false;
            let resolved = plot
                .t
                .as_deref()
                .and_then(|name| match vocab.terrain(name) {
                    Resolved::Known(value) => Some(value),
                    Resolved::Excluded(_) | Resolved::Unknown(_) => None,
                });
            let (terrain, hills) = resolved.unwrap_or((unknown, false));
            tile.terrain =
                if plot.w && plot.lk == Some(true) && matches!(terrain.as_str(), "coast" | "ocean")
                {
                    crate::name!("lake")
                } else {
                    terrain
                };
            tile.hills = hills;
            tile.feature = plot.f.as_ref().and_then(|name| match vocab.feature(name) {
                Resolved::Known(value) => Some(value),
                _ => None,
            });
            tile.resource = plot
                .r
                .as_ref()
                .and_then(|name| match vocab.resource(name) {
                    Resolved::Known(value) => Some(value),
                    _ => None,
                })
                .filter(|resource| visible_resources.contains(resource));
            // ★★★ WHAT IS ALREADY IMPROVED. An unimproved-looking world makes CIVVIS
            // order builders forever: 19 of them for one city in one measured run.
            // ⚠ Mapped by name with no vocabulary, so a Civ 6 improvement CIVVIS does
            // not know becomes None rather than a wrong improvement — the tile then
            // reads unimproved, which is the honest direction for a name we cannot
            // translate.
            tile.improvement = plot.im.as_ref().and_then(|name| {
                let short = civvis_improvement_name(name);
                if modelled_improvements.contains(&short) {
                    Some(Name::new(&short))
                } else {
                    None
                }
            });
            // Ownership is assigned after the native-player map is known.
            // Never retain an owner from a previous improvement or export.
            tile.improvement_owner = None;
            // Own district pillage comes from the city record; foreign district
            // pillage is applied with its infrastructure after city ownership.
            // Neither should be overwritten by the improvement path here.
            if tile.improvement.is_some() {
                tile.pillaged = plot.p;
            }
            // A National Park is not an improvement in Civilization VI — the
            // plot answers `IsNationalPark` and `GetImprovementType` -1 — and
            // it IS one on this board (`national_park` in the improvement
            // rules, read by `established_national_parks`), so the flag lands
            // as the improvement and the Amenities, the Tourism and the
            // unworkable ground follow.
            if plot.np {
                tile.improvement = Some(Name::new("national_park"));
                tile.pillaged = false;
            }
            // The host's road, on the engine's own ladder. An older export
            // carries no `rt` and reads 0, exactly what the mirror wrote before.
            tile.road = route_level(plot.rt.as_deref(), plot.rp);
            // ★★★★ AND NO SIMULATED WEATHER ON MIRRORED GROUND.
            //
            // The same defect as `apply_rivers`, `apply_landmass` and the
            // cliffs, one group of fields over: a field this pass does not
            // write keeps whatever `Game::new`'s generated world put there.
            // These eight are the engine's own disaster bookkeeping. Exact
            // flooding is restored after the climate phase in step_host_climate;
            // the remaining weather reaches the board through `Plot:GetYield`
            // instead (a flooded or
            // irradiated plot reads zero there, which is exactly what it pays).
            // So the honest value is nothing at all: a modelled eruption on a
            // mirrored board would be invented weather, and it would now be
            // counted TWICE, once as the model's own fertility and once inside
            // the host correction derived against it. Cleared unconditionally,
            // like `Tile::continent` above, because the point is to lose the
            // generated value rather than to keep it.
            tile.flooded = false;
            tile.submerged = false;
            tile.drought = false;
            tile.storm = None;
            tile.fallout_until = 0;
            tile.disaster_faith = 0.0;
            tile.disaster_food = 0.0;
            tile.disaster_production = 0.0;
        }
    }
    // `place_city` initially claims its complete first ring. When part of that
    // ring is undisclosed, clearing `owner_city` above must clear the city's
    // reverse index too or the two halves of ownership disagree.
    for city in game.cities.values_mut() {
        city.owned_tiles
            .retain(|pos| !unknown_positions.contains(pos));
    }
    // ★★★★ THE HOST'S BARBARIAN CAMPS REACH THE BOARD'S CAMP REGISTER.
    //
    // The tile above carries `barbarian_camp` as an improvement, and that is all
    // it carried: `game.barb_camps` — the register the engine's own barbarian
    // seat fills when it plants a camp — stayed EMPTY on every mirrored board.
    // Four readers depend on it and every one of them read nothing: the home
    // guard's threat list ranks a camp within `HOME_THREAT_RADIUS` "just under
    // a live raider" and sends a unit to clear it (`ai.rs`, the local-threat
    // scan); `barbarian_presence_at_home` counts a camp near home as a reason
    // to treat the barbarian seat as an enemy; `settlement_tile_risk` prices a
    // visible camp within three tiles of a site; `defensibility` discounts a
    // site by its distance to the nearest camp. Run civvis-20260816T155856Z:
    // two camps SEVEN tiles from Rome for the whole game, upgrading warriors
    // into musketmen; 121 attacks on the raiders they sent, not one on either
    // camp; eight of fourteen Settlers captured; five cities at turn 147.
    // Rebuilt from the tiles on every apply — a camp the host cleared is gone
    // on the next export, and the value is the turn it was seen, which is what
    // the engine's own register holds.
    let camps: Vec<crate::Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| tile.improvement.as_deref() == Some("barbarian_camp"))
        .map(|(pos, _)| *pos)
        .collect();
    game.barb_camps.clear();
    for camp in camps {
        game.barb_camps.insert(camp, game.turn);
    }
    // Rivers before the memory below, so what the seat remembers is the mirrored
    // network and not the generated one.
    apply_rivers(game, snapshot);
    apply_landmass(game, snapshot);
    // Called from here, and never separately, because a map and an explored set that
    // disagree is the defect this pair repairs — see `apply_explored`.
    apply_explored(game, snapshot);
    // And what that ground LOOKED like, which is a different set — see
    // `apply_tile_memory`. Explored ground with no memory is not drawn dim, it is
    // not drawn at all.
    apply_tile_memory(game, snapshot);
}

/// Put Civilization VI's rivers on the board, and take the invented ones off it.
///
/// ★★★★★ **THE BOARD WAS NEVER RIVER-LESS, WHICH IS WHY THIS SURVIVED.** Nothing in
/// this bridge ever wrote a river — `grep -c river src/mirror.rs` answered **0** — and
/// the honest conclusion from that alone would have been "the mirror has no rivers".
/// It had 33 of them. `rebuild_game` starts from `Game::new`, which generates an
/// ordinary CIVVIS map complete with a river network, and `apply_terrain` overwrites
/// terrain, feature, resource and improvement while leaving `river_edges` untouched.
/// The generated world's rivers therefore showed through on every mirrored game ever
/// played, in places the real game has no river at all.
///
/// The same trap as `explored` (35 invented plots) and `remembered_tiles` (33 tiles of
/// a different world): **a populated field is not a mirrored one**, and the check that
/// would have caught it is agreement against the export, never "is it non-empty".
///
/// **The control that settles it.** A Civilization VI river plot is fresh water *by
/// definition*, and `fw` (`IsFreshWater`) has been exported all along. Measured on the
/// live run `civvis-20260731T235836Z` at turn 112, before this fix:
///
/// | | |
/// |---|---|
/// | revealed plots paired with the board | 513 |
/// | plots Civilization VI calls fresh water | 132 (25.7%) |
/// | CIVVIS tiles carrying a river | 33 |
/// | ...of those, fresh water in the export | **12 (36.4%)** |
///
/// 36.4% against a 25.7% base rate is chance. Real rivers would read ~100%.
///
/// ⚠ **Clears before it writes.** `WorldMap::clear_rivers` runs first for the same
/// reason `apply_explored` replaces rather than extends: leaving the generated network
/// in place and adding to it keeps every invented river forever, and they are the whole
/// defect.
///
/// ## The edge mapping
///
/// Civilization VI holds a river on three of a plot's six edges — W, NW, NE — and the
/// other three are the same segments seen from the neighbours, so exporting three per
/// plot carries the whole network. In this reconstruction `r` **is** Civilization VI's
/// `y`, and Civ 6's `y` grows north, so the axial directions in [`crate::hex::DIRS`]
/// read as E, SE, SW, W, NW, NE and the three flags land on indices 3, 4 and 5.
///
/// ⚠ Written through `set_river_edge`, which sets the reciprocal edge on the neighbour
/// too, so the two tiles cannot disagree about a segment they share. A river whose far
/// side is not revealed is simply not written — that is the part of the network the
/// seat has not seen, and inventing it is what this function exists to stop.
pub(crate) fn apply_rivers(game: &mut crate::game::Game, snapshot: &Snapshot) {
    // ⚠⚠ `IsWOfRiver` MEANS "THIS PLOT IS WEST **OF** THE RIVER" — the river is on the
    // plot's EAST edge. Not "there is a river on the west edge", which is how this was
    // first written, and the two put the segment on opposite sides of the hex.
    //
    // Caught on live run civvis-20260801T011451Z at turn 1, by the one check that can
    // see it: a Civilization VI river plot is fresh water by definition, so every tile
    // CIVVIS calls riverside must be `fw` in the export. Four were not — (18,21)
    // through (18,24), a contiguous column, all reporting `rv = 0`, `ri = false`,
    // `fw = false`. Civilization VI had no river there at all.
    //
    // The export said where it really was. The river runs down the x=19/20 boundary:
    //
    //     (19,22) rv = 1 (W)      and Civ 6 marks (20,22) riverside — its EAST neighbour
    //     (19,25) rv = 4 (NE)     and Civ 6 marks (19,24) riverside — its SOUTH-WEST one
    //     (20,22) rv = 4 (NE)     and Civ 6 marks (19,21) riverside — likewise
    //
    // So each flag names the plot's own position relative to the river, and the edge
    // carrying it is the OPPOSITE one:
    //
    //     IsWOfRiver  -> river on the EAST edge        DIRS[0]
    //     IsNWOfRiver -> river on the SOUTH-EAST edge  DIRS[1]
    //     IsNEOfRiver -> river on the SOUTH-WEST edge  DIRS[2]
    //
    // ⚠ Why the original passed its unit test anyway: `set_river_edge` marks BOTH
    // tiles sharing the segment, so the plot that reported the flag came out riverside
    // either way. Only the NEIGHBOUR differed — the segment sat on the wrong side of
    // the hex, which is invisible to "is this tile riverside" and decides every river
    // crossing and every district's river adjacency. A fixture that exports one plot
    // and checks that plot cannot see it; the test below now pins the neighbour.
    //
    // Directions in this reconstruction's frame, where `r` IS Civilization VI's `y`
    // and Civ 6's `y` grows north: DIRS[0] E, [1] SE, [2] SW, [3] W, [4] NW, [5] NE.
    game.map.clear_rivers();
    for (x, y) in snapshot.revealed_positions() {
        let Some(plot) = snapshot.plot((x, y)) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(x, y);
        // `ri` is `Plot:IsRiver()`, the host's own answer to "is this plot riverside".
        // `rv` carries the six edges, but a segment whose Firaxis holder is an
        // unrevealed neighbour reads back as 0 here while the plot is riverside — the
        // Lua says so where it writes `ri` — and until 2026-08-26 nothing read the
        // bit, so housing, fresh water and river adjacency on such a plot were all
        // dry. The flag marks the tile riverside without inventing a crossing.
        if plot.ri {
            if let Some(tile) = game.map.tiles.get_mut(&pos) {
                tile.riverside = true;
            }
        }
        if plot.rv == 0 {
            continue;
        }
        // Bits 8/16/32 carry W/NW/NE edges read from the neighbouring Firaxis
        // holders. The exporter includes them even when that neighbour is hidden:
        // the segment on this revealed plot is itself known. North-up staging can
        // also map any of the six directions onto any other one.
        for (bit, direction) in [(1u8, 0usize), (2, 1), (4, 2), (8, 3), (16, 4), (32, 5)] {
            if plot.rv & bit == 0 {
                continue;
            }
            let delta = crate::hex::DIRS[direction];
            let neighbour = (pos.0 + delta.0, pos.1 + delta.1);
            if !game.map.set_river_edge(pos, neighbour, true) {
                // A river edge on a revealed boundary tile is visible even when
                // the tile across it is not. Preserve that one-sided boundary
                // fact instead of turning a known riverside plot into dry land.
                if let Some(tile) = game.map.tiles.get_mut(&pos) {
                    tile.river_edges[direction] = true;
                }
            }
        }
    }
}

/// Which landmass each plot belongs to, how low it lies — and no invented cliffs.
///
/// The third sweep of the same defect as [`apply_rivers`]: a field `apply_terrain` does
/// not write keeps whatever the map `Game::new` generated put there. Measured on the
/// live run `civvis-20260731T235836Z` at turn 207, against a board of 776 tiles:
///
/// | field | populated | by |
/// |---|---|---|
/// | `continent` | 200 | the generator |
/// | `cliff_edges` | 66 | the generator |
/// | `coastal_lowland` | 24 | the generator |
///
/// ⚠ `continent` was not merely wrong, it was **incoherent**: 200 tiles carried a
/// region and 576 carried none, on a board where every land plot has a continent in the
/// real game. "Another continent" is load-bearing in this ruleset, so a seat that
/// cannot tell one landmass from another cannot reason about overseas settling or
/// invasion at all.
///
/// ## Continent indices are assigned, not translated
///
/// `Tile::continent` is a zero-based region id whose only job is to say *same landmass
/// or not*. Civilization VI names them (`CONTINENT_AFRICA`), so the names present in
/// the snapshot are sorted and numbered. Sorting rather than first-seen order keeps the
/// assignment a pure function of the snapshot, so two syncs over the same revealed set
/// agree. ⚠ A newly revealed continent can therefore renumber the others — that is
/// safe here because every consumer compares ids **within one reconstruction**, and the
/// whole map is rebuilt from the snapshot on each sync anyway.
///
/// ## Cliffs are cleared, because they cannot be mirrored
///
/// ⚠ **There is no gameplay Lua accessor for cliffs.** `IsCliff` exists in this
/// install only inside art definitions (`<m_ParamName text="IsCliff"/>`), never as a
/// plot method, so unlike rivers there is nothing to export.
///
/// That leaves a choice between two fictions, and it is the same one `apply_terrain`
/// faced over unseen ground: **the expensive fiction is the one that stops movement.**
/// `Game::crosses_cliff` fires only on a land/water boundary and
/// `unit_can_cross_cliff` gates movement on it, so an invented cliff **blocks
/// embarkation** at a shoreline the real game lets a unit leave from. That is the
/// precise shape of a failure already on the books — a seat whose world ends at the
/// water, with `met` stuck at zero. A missing cliff, by contrast, only means CIVVIS may
/// plan a crossing Civilization VI refuses, and the mod honours refusal, so it costs a
/// turn rather than a permanently walled-in empire.
///
/// So they are cleared, and this comment is the record that it is a known gap rather
/// than an oversight. If a cliff accessor ever appears, mirror them the way rivers are
/// mirrored and delete this paragraph.
pub(crate) fn apply_landmass(game: &mut crate::game::Game, snapshot: &Snapshot) {
    // Sorted so the numbering is a function of the snapshot and nothing else.
    let mut names: Vec<&str> = snapshot
        .revealed_positions()
        .filter_map(|pos| snapshot.plot(pos)?.ct.as_deref())
        .collect();
    names.sort_unstable();
    names.dedup();
    let index_of: BTreeMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(index, name)| (*name, index))
        .collect();

    // Rebuilt from the plots on every apply, like the landmass: a plot whose
    // reading lapsed keeps none.
    Arc::make_mut(&mut game.observed_appeal).clear();
    Arc::make_mut(&mut game.observed_fresh_water).clear();
    for (x, y) in snapshot.revealed_positions() {
        let Some(plot) = snapshot.plot((x, y)) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(x, y);
        let Some(tile) = game.map.tiles.get_mut(&pos) else {
            continue;
        };
        // ⚠ Assigned unconditionally, including the `None` case. A plot the export says
        // has no continent must LOSE the generated one rather than keep it, which is
        // the whole defect.
        tile.continent = plot
            .ct
            .as_deref()
            .and_then(|name| index_of.get(name).copied());
        tile.coastal_lowland = plot.cl.clamp(0, 3) as u8;
        // The host's own appeal, which `Game::tile_appeal` prefers to its
        // derivation from the six neighbours it can see.
        if let Some(appeal) = plot.ap {
            Arc::make_mut(&mut game.observed_appeal).insert(pos, appeal);
        }
        // The host's fresh-water answer, which `Game::city_water` prefers.
        if let Some(fresh) = plot.fw {
            Arc::make_mut(&mut game.observed_fresh_water).insert(pos, fresh);
        }
    }

    // Nothing can mirror these, so nothing may keep them. See above.
    for tile in game.map.tiles.values_mut() {
        tile.cliff_edges = [false; 6];
    }
}

/// Tell the seat what the ground it has seen actually LOOKED like.
///
/// ★★★★★ **THE SEAT REMEMBERED NOTHING, SO THE MIRROR DREW ONLY WHAT IT COULD SEE THIS
/// INSTANT** — a live vision cone beside a Civilization VI screen showing a continent.
/// The operator reported it as *"civvis sometimes only shows current visibility… you can
/// see from the map now that area has been explored that isn't in civvis map."*
///
/// `explored` and `remembered_tiles` are two different sets and this bridge only ever
/// wrote the first. `grep -c remembered_tiles src/mirror.rs` answered **0**.
///
/// ⚠⚠ **AND THE MEMORY WAS NOT EMPTY — IT WAS A DIFFERENT WORLD.** Exactly the trap
/// [[civvis-civ6-explored-was-a-fiction]] describes, one field over. Measured by
/// disabling this function on the test below: the seat held **33 remembered tiles**
/// against **6 charted plots**, because `Game::new` generates a CIVVIS map and reveals a
/// start on it. Overlap: **2**. So the board rendered **2 of 6** charted plots, and both
/// survivors were coincidences of the generated map rather than ground the real
/// Civilization VI seat had ever stood on. Every "is the memory populated" check said
/// yes, which is why this outlived the `explored` fix that shipped beside it.
///
/// What that costs: `obs.rs` builds the board a *seated* viewer receives by walking
/// `explored`, and for a tile that is not currently visible it reads the freshest
/// viewer's memory of it —
///
/// ```ignore
/// let tiles: Vec<Value> = explored.iter().filter_map(|pos| {
///     let live = omniscient || vis.contains(pos);
///     let (tile, owner) = if live { … } else {
///         let (memory, _) = viewers.iter().filter_map(…).max_by_key(…)?;  // <-- HERE
///         (&memory.tile, memory.owner)
///     };
/// ```
///
/// That `?` sits inside a `filter_map`, so a tile with no memory is **dropped from the
/// board entirely** — not fogged, not stale, absent. The mirror window attaches with
/// `POST /view {"player": 0}`, so it is exactly the seated, non-omniscient path, and
/// every explored plot outside the seat's current sight radius disappeared from it.
///
/// ⚠ This is a *display and knowledge* defect, not only a cosmetic one: the same
/// observation feeds anything reading the board through `obs`, so ground the seat has
/// charted reads as ground nobody has ever been to.
///
/// ⚠ **Replaces, for the same reason [`apply_explored`] replaces.** `rebuild_game`
/// starts from `Game::new`, which generates an ordinary CIVVIS map and reveals a start
/// on it; any memory of *that* world describes a place the real seat has never been
/// ([[civvis-civ6-explored-was-a-fiction]]). Rebuilding from the snapshot keeps the
/// invariant that matters — **memory is exactly the explored set, never more and never
/// less** — so the two can no longer disagree. It stays monotonic in practice because
/// `Snapshot::revealed` only ever accumulates.
///
/// ⚠ The frontier is deliberately excluded: this keys strictly off
/// `Snapshot::revealed_positions`, so a provisionally traversable unknown is never
/// remembered as though the seat had seen it.
///
/// Idempotent.
pub(crate) fn apply_tile_memory(game: &mut crate::game::Game, snapshot: &Snapshot) {
    let turn = snapshot.turn.max(1);
    // Decided first, applied second: reading the map and the owning city needs `game`
    // immutably while writing the seat's memory needs it mutably.
    let remembered: Vec<(crate::Pos, crate::world::RememberedTile)> = snapshot
        .revealed_positions()
        .filter_map(|(x, y)| {
            let pos = crate::hex::offset_to_axial(x, y);
            let tile = game.map.get(pos)?;
            // The same derivation the live branch of `obs.rs` uses, so a tile does not
            // change owner merely by passing under fog. `apply_territory` has already
            // written `owner_city` from Civilization VI's own `GetOwner` on the rebuild
            // path; where it has not run, this is `None`, which is what the live branch
            // would report for the same tile.
            let owner = tile
                .owner_city
                .and_then(|city| game.cities.get(&city))
                .map(|city| city.owner);
            Some((
                pos,
                crate::world::RememberedTile {
                    tile: tile.clone(),
                    owner,
                    seen_turn: turn,
                },
            ))
        })
        .collect();
    let Some(seat) = game.players.get_mut(0) else {
        return;
    };
    seat.remembered_tiles.forget_all();
    for (pos, memory) in remembered {
        seat.remembered_tiles.remember(pos, memory, turn);
    }
}

/// ★★★★★ THE HOST'S OWN READING OF EVERY PLOT IT EXPORTED, not only the ones a
/// city happens to be working this turn.
///
/// The per-plot correction below this in `apply_observed_city_economy` has
/// always been derived from `state.cities[].worked[].yields` — six or eight
/// plots per city, the ones the host's citizen manager assigned. Every OTHER
/// plot on the board was paid CIVVIS's own catalogue sum, and that sum is short
/// by whatever the ground holds that no row of the ruleset names.
///
/// Disaster fertility is the whole of it. Gathering Storm stores an eruption's
/// or a flood's permanent +1 Food / +1 Production ON THE PLOT
/// (`RandomEvent_Yields`), and Volcanic Soil itself has not one
/// `Feature_YieldChanges` row — so a mirror that reads the feature name sees
/// bare Grassland where the game shows 3 Food 3 Production. The operator
/// reported exactly that tile on the live board.
///
/// The set that was wrong is the set that matters: a plot nobody works yet is
/// precisely what a Builder is choosing to improve, what the citizen governor
/// is choosing to grow onto, and what a Settler is choosing to found beside.
///
/// Four rules keep this a correction rather than an override:
///
/// 1. **Deltas, never absolutes**, like every other correction here — a
///    counterfactual Farm still moves the tile by its modeled amount.
/// 2. **Never over a fresher reading.** The worked and centre plots are
///    corrected first, from THIS turn's state export; this fills in around them
///    with `or_insert` and cannot displace them.
/// 3. **Only a current record.** `Snapshot::revealed` accumulates forever, so a
///    plot the newest sweep missed still sits in it looking authoritative;
///    [`Snapshot::is_current`] refuses those.
/// 4. **Not on a district, foundation, wonder or city centre.** Those pay
///    through their own readers, and a plot's `GetYield` is the ground under
///    them.
pub(super) fn apply_observed_plot_yields(game: &mut crate::game::Game, snapshot: &Snapshot) {
    for (x, y) in snapshot.revealed_positions().collect::<Vec<_>>() {
        if !snapshot.is_current((x, y)) {
            continue;
        }
        let Some(host) = snapshot.plot((x, y)).and_then(Plot::host_yields) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(x, y);
        let Some(tile) = game.map.get(pos) else {
            continue;
        };
        if tile.district.is_some()
            || tile.district_foundation.is_some()
            || tile.wonder.is_some()
            || game.city_at(pos).is_some()
        {
            continue;
        }
        if game.observed_tile_yield_adjustments.contains_key(&pos) {
            continue;
        }
        let model = game.modeled_tile_yields(pos);
        let delta = crate::rules::Yields {
            food: host.food - model.food,
            production: host.production - model.production,
            gold: host.gold - model.gold,
            science: host.science - model.science,
            culture: host.culture - model.culture,
            faith: host.faith - model.faith,
        };
        // ⚠ ONLY WHERE THE MODEL IS ACTUALLY WRONG. Most of a map is ordinary
        // ground the catalogue prices exactly, and a zero correction is a
        // no-op that would still be cloned into every planning copy of the
        // board and written into every saved mirror. Recording only the
        // disagreements keeps this map the size of the problem — and makes it
        // readable as "the plots the host and CIVVIS do not agree on", which is
        // what `tools/civ6_yield_drift.py` wants from it.
        if [
            delta.food,
            delta.production,
            delta.gold,
            delta.science,
            delta.culture,
            delta.faith,
        ]
        .iter()
        .all(|value| value.abs() < 1e-9)
        {
            continue;
        }
        Arc::make_mut(&mut game.observed_tile_yield_adjustments).insert(pos, delta);
    }
}

pub(super) fn apply_territory(
    game: &mut crate::game::Game,
    snapshot: &Snapshot,
    state: &StateSnapshot,
) {
    // Civ 6 player id -> CIVVIS seat. The rival array is met-gated, so this
    // must use the stable host-id map shared by the war and city loops.
    let mut seat_of: std::collections::BTreeMap<i32, usize> =
        host_major_seat_map(state, modeled_major_player_count(game))
            .into_iter()
            .filter_map(|(host, seat)| i32::try_from(host).ok().map(|host| (host, seat)))
            .collect();
    for (minor, seat) in minor_actor_assignments(game, state) {
        seat_of.insert(minor.player as i32, seat);
    }
    for city in &state.cities {
        if let Some(plot) = snapshot.plot((city.x, city.y)) {
            if plot.o >= 0 && !seat_of.contains_key(&plot.o) {
                seat_of.insert(plot.o, 0);
            }
        }
    }
    let mut centres: std::collections::BTreeMap<usize, Vec<(u32, crate::Pos)>> = Default::default();
    for (cid, city) in &game.cities {
        centres
            .entry(city.owner)
            .or_default()
            .push((*cid, city.pos));
    }
    // Decided first, applied second: the nearest-city lookup needs `game` immutably
    // while the assignment needs it mutably.
    let mut assign: Vec<(crate::Pos, Option<u32>)> = Vec::new();
    // Ground somebody else holds that we cannot attribute to a mirrored seat.
    let mut blocked: std::collections::BTreeSet<crate::Pos> = Default::default();
    // The same ground, minus whatever this seat may already walk through. See
    // [`crate::game::Game::closed_borders`]: `blocked` answers "cannot found
    // here", which is not the same question as "cannot enter here" — war and
    // open borders open the second while leaving the first shut, and a settler
    // barred by loyalty is barred from founding on ground it may cross freely.
    let mut sealed: std::collections::BTreeSet<crate::Pos> = Default::default();
    // Ground a MET major owns whose city attribution is unresolved: either the
    // seat holds no city on this board at all, or its nearest known city lies
    // on or beyond the fifth ownership ring. At five, the host cannot tell that
    // city from a nearer one still in fog; beyond five, the visible city cannot
    // own it at all. See [`crate::game::Game::unseen_major_borders`].
    let mut unseen_major: std::collections::BTreeSet<crate::Pos> = Default::default();
    // Who seals how much, majors only — the passage-purchase lane's shopping
    // list. See [`crate::game::Game::sealed_border_owners`].
    let mut sealed_by: std::collections::BTreeMap<usize, u32> = Default::default();
    let is_major = |seat: usize| {
        game.players
            .get(seat)
            .is_some_and(|p| seat != 0 && !p.is_minor && !p.is_barbarian)
    };
    for y in 0..snapshot.height.max(1) {
        for x in 0..snapshot.width.max(1) {
            let Some(plot) = snapshot.plot((x, y)) else {
                continue;
            };
            let pos = crate::hex::offset_to_axial(x, y);
            if !game.map.tiles.contains_key(&pos) {
                continue;
            }
            let improvement_owner = plot.io.and_then(|owner| seat_of.get(&owner)).copied();
            if let Some(tile) = game.map.tiles.get_mut(&pos) {
                tile.improvement_owner = tile.improvement.and(improvement_owner);
            }
            let Some(&seat) = seat_of.get(&plot.o) else {
                // `o = -1` is nobody, and Civilization VI is authoritative about that
                // too: a tile CIVVIS thinks it owns and the game says is neutral is
                // the same class of error in the other direction.
                if plot.o < 0 {
                    assign.push((pos, None));
                    continue;
                }
                // ★★★★★ SOMEBODY OWNS IT AND WE CANNOT NAME THEM — USUALLY A
                // CITY-STATE. `state.rivals` carries the MAJOR civilizations this seat
                // has met, so a minor's territory maps to no seat at all and used to
                // arrive as free land. It is not free: Civilization VI will not let a
                // settler found there, and CIVVIS will happily pick it because on its
                // board the tile is unowned, high-yield and often already improved.
                //
                // Measured on run `civvis-20260731T052021Z`, which is the whole 53-turn
                // stall in one line: CIVVIS chose offset (15,11) — plains hills, worth
                // 99.6, with a MINE at (14,11) and a FARM at (16,11) beside it, all
                // three exported as `o: 6`. That is a city-state's improved land. The
                // settler walked to the border, could not take the last step, and
                // bounced between two tiles for the rest of the game while the empire
                // held one city.
                //
                // Recorded through `blocked_city_sites`, the channel the host's own
                // `found` refusals already use, rather than as a new kind of fact: it
                // means the same thing — ground this seat cannot found on — and it is
                // read by the same planner. We do not invent a city to own it, because
                // we do not know where their city is and a phantom owner is worse than
                // a known prohibition.
                blocked.insert(pos);
                // Nobody we can name holds it, so there is no diplomacy to
                // consult and no war that could have opened it. A city-state's
                // land is shut to us until we are its Suzerain, which is the
                // conservative reading and the one that matches the settler
                // stall this arm was written for.
                sealed.insert(pos);
                // ⚠ And it is certainly not OURS. `place_city` hands a mirrored
                // city its whole first ring, so a rival or minor plot that falls
                // inside one reads back as our own territory — worse than the
                // "reads unowned" error this function was written to fix, because
                // it inflates the yields and workable tiles we plan on. Strip it.
                assign.push((pos, None));
                continue;
            };
            // Legacy recordings and ordinary foreign ground name only the
            // player. Explicit purchase/district parents override proximity.
            let nearest = centres.get(&seat).and_then(|list| {
                list.iter()
                    .min_by_key(|(cid, centre)| (game.wdist(pos, *centre), *cid))
                    .map(|(cid, centre)| (*cid, game.wdist(pos, *centre)))
            });
            // City IDs belong to native players, not a global namespace.
            // An observed district may belong to a farther city; assigning it
            // by proximity can collapse two cities' Encampments into one roster.
            // Never redirect an explicit but unresolved parent to another city.
            let owner = if plot.oc.is_some() {
                let native_city = if seat == 0 {
                    state.cities.iter().find(|city| Some(city.id) == plot.oc)
                } else {
                    state
                        .rivals
                        .iter()
                        .filter(|rival| rival.player == plot.o as usize)
                        .flat_map(|rival| &rival.cities)
                        .chain(
                            state
                                .minors
                                .iter()
                                .filter(|minor| minor.player == plot.o as usize)
                                .flat_map(|minor| &minor.cities),
                        )
                        .find(|city| Some(city.id) == plot.oc)
                };
                native_city
                    .and_then(|city| game.city_at(crate::hex::offset_to_axial(city.x, city.y)))
                    .filter(|cid| game.cities.get(cid).is_some_and(|city| city.owner == seat))
            } else {
                nearest.map(|(cid, _)| cid)
            };
            if owner.is_some() {
                // A plot on the outermost known-city ownership ring is
                // ambiguous: the export names only a player, so it may instead
                // belong to a nearer city this seat has not seen. Beyond that
                // ring it must be unseen. Mark either case for the live
                // settlement Loyalty guard.
                if is_major(seat)
                    && plot.oc.is_none()
                    && nearest.is_some_and(|(_, distance)| distance >= CIV6_CITY_OWNERSHIP_REACH)
                {
                    unseen_major.insert(pos);
                }
                // Attributed ground is gated by `can_enter` through
                // `has_open_borders`, which now reads the host's own Early
                // Empire answer (`Player::borders_enforced`) — no seal needed.
                // The passage-purchase lane still has to know WHO shuts how
                // much: a major whose border we can see and cannot cross is
                // exactly the seat Open Borders is bought from, and until now
                // only fogged ground made that list.
                if is_major(seat) && !game.is_at_war(0, seat) && game.enforces_borders(seat) {
                    let granted = game
                        .players
                        .get(seat)
                        .and_then(|p| p.open_borders_until.get(&0))
                        .is_some_and(|until| *until > game.turn);
                    if !granted {
                        *sealed_by.entry(seat).or_insert(0) += 1;
                    }
                }
                assign.push((pos, owner));
            } else if seat == 0 {
                if plot.oc.is_some() {
                    assign.push((pos, None));
                }
            } else {
                if is_major(seat) {
                    unseen_major.insert(pos);
                }
                // A seat we can NAME but that holds no city on this board —
                // their centre is unrevealed, or refused planting. Their
                // ground is still not ours to found on; leaving it unassigned
                // would re-open the exact hole the unattributable arm above
                // closes.
                blocked.insert(pos);
                // ...and not ours to walk into either, unless we are at war.
                // Asking here rather than in `can_enter` is what keeps a
                // declaration of war from sealing our own invasion out: the seat
                // is named, so `at_war` is answerable from the export even
                // though their cities are unseen.
                //
                // ⚠ `has_open_borders` is deliberately NOT consulted. It returns
                // TRUE whenever the owner has no `open_borders` tree effect —
                // the correct Civ 6 rule that a civ without Early Empire has no
                // enforced border — and this mirror does not model a RIVAL's
                // civics, so it answers "free passage" for every rival by
                // default. Consulting it would defeat the seal in exactly the
                // live case it exists for. Sealing ground we could in truth have
                // crossed costs a peacetime shortcut; not sealing it cost 74
                // turns of a scout re-sending one blocked step.
                //
                // An EXPLICIT grant is a different fact from a default: when
                // the export says this rival granted us Open Borders (the
                // import above wrote it onto the seat), the host itself will
                // let our units through, so sealing would waste exactly the
                // passage the buy lane just paid for.
                let granted = game
                    .players
                    .get(seat)
                    .and_then(|p| p.open_borders_until.get(&0))
                    .is_some_and(|until| *until > game.turn);
                if !game.is_at_war(0, seat) && !granted {
                    sealed.insert(pos);
                    if is_major(seat) {
                        *sealed_by.entry(seat).or_insert(0) += 1;
                    }
                }
                // Nor ours to count as territory — see the arm above.
                assign.push((pos, None));
            }
        }
    }
    // State can report a city before the terrain export includes its centre.
    // `plant_city` correctly refuses to invent that missing terrain, but the
    // host still enforces the city's four-tile settlement floor on every
    // revealed neighbour. Reserve the visible portion of that floor without
    // inventing a city, its yields, ownership, or diplomacy.
    for city in state
        .cities
        .iter()
        .chain(state.rivals.iter().flat_map(|rival| rival.cities.iter()))
        .chain(state.minors.iter().flat_map(|minor| minor.cities.iter()))
    {
        let centre = crate::hex::offset_to_axial(city.x, city.y);
        if game.city_at(centre).is_some() {
            continue;
        }
        for site in game.wdisk(centre, 3) {
            if game.map.tiles.contains_key(&site) {
                blocked.insert(site);
            }
        }
    }
    Arc::make_mut(&mut game.blocked_city_sites).extend(blocked);
    // Assigned, not extended: recomputed from the export every turn, so ground
    // that opens — a war declared, borders granted, or simply their city coming
    // into view so the ordinary gate takes over — stops being sealed next turn.
    game.closed_borders = sealed;
    game.unseen_major_borders = unseen_major;
    game.sealed_border_owners = sealed_by;
    for (pos, owner) in assign {
        let previous = game.map.tiles.get(&pos).and_then(|tile| tile.owner_city);
        if previous == owner {
            continue;
        }
        if let Some(old) = previous.and_then(|cid| game.cities.get_mut(&cid)) {
            old.owned_tiles.retain(|held| *held != pos);
        }
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            tile.owner_city = owner;
        }
        if let Some(new) = owner.and_then(|cid| game.cities.get_mut(&cid)) {
            if !new.owned_tiles.contains(&pos) {
                new.owned_tiles.push(pos);
            }
        }
    }
    apply_foreign_infrastructure(game, snapshot);
}

pub(crate) fn apply_explored(game: &mut crate::game::Game, snapshot: &Snapshot) {
    // The seat is player 0 throughout this bridge: `plant_unit(&mut game, 0, …)` for
    // our own units, `game.players[0]` for our civics and techs.
    let Some(seat) = game.players.get_mut(0) else {
        return;
    };
    seat.explored = snapshot
        .revealed_positions()
        .map(|(x, y)| crate::hex::offset_to_axial(x, y))
        .collect();
}
