//! Chokepoint control: five opt-in genes for hard power over the map — the
//! narrow water passage, the strip of land that joins two seas, the mountain
//! pass, and what a city, a border, a district and a soldier each do with one.
//!
//! Operator goal (2026-08-25): *"a gene for considering control over
//! chokepoints — controlling with territory an important narrow water
//! passageway. placing a city on a single wide strip of land to connect 2
//! bodies of water (through the city, for only me), controlling important
//! mountain passageways … we want to exercise this hard power control over
//! the map … add in other considerations too that i did not list."*
//!
//! ## The engine facts every gene here rests on
//!
//! 1. **A city center IS a canal, and only its owner may use it.**
//!    `Game::class_can_traverse` lets a naval traversal class stand on
//!    `water || self.city_at(tile.pos).is_some() || …naval_passage…`, so a
//!    city on a one-tile land bridge joins the two seas it touches. And
//!    `can_enter_past` ends with `if let Some(cid) = self.city_at(pos) { if
//!    self.cities[&cid].owner != u.owner { return false } }` — a foreign hull
//!    may never enter it, at peace **or** at war (it can only attack the
//!    city). That is exactly the operator's "through the city, for only me",
//!    and it is `canal-city` below.
//! 2. **Mountains are the only impassable terrain** (`data/terrains.json`,
//!    plus the `ice`/`volcano`/`impact_zone` features), so a pass is a tile
//!    whose closure lengthens or breaks the land walk around it — the same
//!    fact `pass-picket` reads, priced here in the steps it costs rather than
//!    only found.
//! 3. **Territory is a wall at peace.** `can_enter_past` refuses a tile whose
//!    `territory_owner_at` is a player the mover has no access to
//!    (`unit_has_territory_access`: owner, open borders, war, or one of the
//!    four unit kinds that ignore closed borders). The check reads
//!    `tile.owner_city` and asks nothing about the terrain, so **owning the
//!    water of a strait closes it** to every rival without Open Borders, the
//!    same way owning the land of a pass does. That is what "controlling with
//!    territory" buys, and `chokepoint-claim` is the only lever the seat has
//!    over it: `expand_borders` is the engine's own influence-cost picker and
//!    takes no advice, while `BuyPlot` names a tile (`plot_purchase_cost`:
//!    unowned, explored, adjacent to the city's own ground, ring ≤ 3).
//! 4. **An Encampment is a permanent wall.** The same function ends
//!    `if let Some(cid) = self.encampment_at(pos) { if owner != u.owner {
//!    return false } }`, and unlike the territory arm there is no war,
//!    alliance or Open Borders exception — a foreign unit can never stand on
//!    our unpillaged Encampment. A pass carrying one is shut for the rest of
//!    the game. That is `encampment-seals-the-pass`.
//! 5. **A body on the tile is the wall the army has to break.** The stacking
//!    loop refuses any tile a foreign *military* unit stands on, war or
//!    peace, so one unit in a one-tile pass is a gate: at peace nothing may
//!    pass at all, at war the whole column has to kill it first, and it
//!    fortifies on our own ground where `healing_location` pays the friendly
//!    rate. That is `chokepoint-garrison`.
//!
//! ## Reading a chokepoint: arcs, then the detour it costs
//!
//! A tile is *narrow* in a domain when its neighbours in that domain fall
//! into two or more groups that do not touch each other — the classic
//! isthmus/pass test, asked here as connected components of the ≤ 6
//! neighbours under mutual adjacency, so it costs six distance checks and is
//! independent of ring order (a sphere's pentagon tiles included).
//!
//! Narrow is only a *local* fact, so each pair of groups is then priced by
//! what closing the tile actually costs: a flood of at most [`WINDOW`] tiles
//! from one group to the other with the tile removed. The answer is the
//! extra steps over the two the route through the tile would have taken,
//! capped at [`CUT_DETOUR`] — which is also what a pair that does not
//! reconnect inside the window is worth, because a bounded scan may not call
//! a detour infinite. A group whose own body inside the window is smaller
//! than [`MIN_BODY`] is a pond or a spur and is not a side at all.
//!
//! The same primitive answers all three questions by swapping the domain:
//! the land groups around a land tile are a **pass**, the water groups around
//! a water tile are a **strait**, and the water groups around a *land* tile
//! are a **canal** — the sea detour a city center standing there would save
//! us and no one else.
//!
//! ⚠ **Fog-honest, and passability-honest.** A tile whose own ring is not
//! fully explored is not classified at all: unknown ground reads as a wall,
//! so without that rule every fog edge is a mountain pass. And membership in
//! both domains asks `is_passable`, because `ice` is water the engine refuses
//! to a hull (`data/features.json`) — a sea membership that asked only
//! `is_water` would read the polar cap as open ocean and call every gap in it
//! a strait. The values are cached per turn in [`NarrowsAtlas`]; a controller
//! clone starts empty.
//!
//! ## The five genes
//!
//! - **`chokepoint-siting`** — [`AdvancedAi::chokepoint_site_bonus`]: a
//!   settle site is worth more when the ground its own borders will cover
//!   holds a pass or a strait, weighted by the ring the gate sits in (1.0 out
//!   to ring 1, which foundation grants; 0.6 and 0.35 for the rings a buy can
//!   still reach) and capped so it never outbids food.
//! - **`canal-city`** — [`AdvancedAi::canal_city_bonus`]: fact 1, priced in
//!   the sea detour the city center saves.
//! - **`chokepoint-claim`** — [`AdvancedAi::chokepoint_plot_bonus`]: fact 3,
//!   a term on the Gold plot shortlist for the tile that closes a passage
//!   somebody else could use.
//! - **`encampment-seals-the-pass`** — [`AdvancedAi::encampment_seal_bonus`]:
//!   fact 4, a term on the Encampment's production value at a gate tile, so
//!   the district lands on the pass instead of on the next free plot.
//! - **`chokepoint-garrison`** — [`AdvancedAi::chokepoint_garrison_step`]:
//!   fact 5, a surplus soldier (or a hull, for a strait) holds the gate on
//!   the approach to one of our cities and fortifies there.
//!
//! ⚠ **The garrison holds ground in peace and never in a major war.** A
//! stand-still posture screened NEGATIVELY at 38,160 seats and
//! `advanced/field_craft.rs` records the reason in its own header — "a unit
//! that stands still in a major war is a unit that is not at the siege, and
//! the whole regime is decided by tempo". So the step sits exactly where
//! `pass-picket`'s does, on the peacetime tail of
//! `advanced_military_step_with_decline`, after every raider, camp, village,
//! staging and home-return order: a unit reaches it only when nothing above
//! it wanted it. What it buys there is real anyway — at peace the gate is
//! shut to rival Settlers and armies outright, and a surprise declaration
//! finds it already held. A wartime version of this is the versioned sibling
//! to try next, not this gene.
//!
//! ## What was considered and left out
//!
//! - **The Canal district** (`data/districts.json`: `naval_passage`, Steam
//!   Power) cuts the same passage without a city, and `production_value`
//!   already pays 75–150 for the effect. It arrives around the industrial era
//!   in a 250-turn screen, so the city center — available from turn one — is
//!   where the operator's ask actually pays. `canal-city` leaves the district
//!   arm alone.
//! - **Culture border growth toward a gate.** `Game::expand_borders` is the
//!   shipped `PLOT_INFLUENCE_*` picker and takes no advice from any
//!   controller; the buy is the whole lever, so it is the whole gene.
//! - **River crossings and high ground.** Real chokepoints in Civilization VI,
//!   but the engine prices them as a combat modifier on an ordinary tile, not
//!   as ground that may or may not be entered. They belong to the tactical
//!   arm (`advanced/field_craft.rs`), not to map control.
//! - **Blockading a rival's strait far from home.** Everything here is scoped
//!   to ground our own cities can reach, because the seat cannot hold what it
//!   cannot supply, and because a unit posted across the map is a unit that
//!   is not at home.
//!
//! All five are off in `AdvancedAi::new()` and `legacy()`, `Kind::OptIn` rows
//! in `genes.rs`, and read nothing when off: every entry point returns before
//! it touches the board. Fires probes under `docs/gene_screens/fires/`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::AdvancedAi;
use crate::game::Game;
use crate::think;
use crate::Pos;

/// Every question about a chokepoint is asked inside this many tiles of it.
/// Six is two Settler turns of walking and rather more than the three rings a
/// city can ever own, so a detour longer than the window is one nothing this
/// controller was going to walk around anyway.
const WINDOW: i32 = 6;
/// What a gate is worth when its two sides do not reconnect inside the
/// window: the detour is at least twice the window, and the scale tops out
/// here. ⚠ Deliberately NOT infinity — a bounded scan cannot prove a cut,
/// only that the way round is long.
const CUT_DETOUR: f64 = 14.0;
/// Closing a tile has to cost a mover at least this many extra steps before
/// the tile is a gate at all. Two tiles of detour is a bend in the road.
const MIN_DETOUR: f64 = 3.0;
/// A side smaller than this inside the window is a pond, a spur or a dead
/// end — not somewhere anything comes from, and not worth sealing off.
const MIN_BODY: usize = 8;

/// Rings of a candidate site the siting gene reads for gates.
const SITE_RINGS: i32 = 3;
/// How much of a gate's worth a site collects, by the ring it sits in.
/// Foundation grants ring 1; rings 2 and 3 are what `BuyPlot` can still
/// reach (`Game::plot_purchase_cost`), so they are discounted rather than
/// refused.
const SITE_RING_WEIGHT: [f64; 4] = [1.0, 1.0, 0.6, 0.35];
/// A land pass fully inside the new city's own ground, in site value.
const SITE_LAND_GATE: f64 = 6.0;
/// The same for a strait: worth less because closing it needs the water tile
/// itself to be bought, and because a hull has more ways round than a column.
const SITE_SEA_GATE: f64 = 4.0;
/// How many gates one site may collect. Two, so a genuinely commanding neck
/// scores above a single pass without the disk's every narrow tile adding up.
const SITE_TILES: usize = 2;
/// The most the siting gene may add. The scan admits a site at 12 and a good
/// one runs 30–60, so a gate is a strong tiebreak and never a site on its own.
const SITE_MAX: f64 = 10.0;

/// A city center joining two seas, in site value. Above [`SITE_MAX`] because
/// this one is not a preference over ground the border might cover: the
/// passage exists only if the city center itself stands on that tile.
const CANAL_CITY_VALUE: f64 = 8.0;

/// A gate worth the whole [`CUT_DETOUR`], on the Gold plot shortlist's scale
/// (a Luxury resource is 260, a Natural Wonder 320, and the shortlist buys
/// nothing below 120).
const CLAIM_VALUE: f64 = 320.0;
/// A passage nobody else can reach is scenery. Somebody else's city or camp
/// has to be within this many tiles of the tile before its closure is worth
/// Gold.
const CLAIM_RIVAL_RANGE: i32 = 12;

/// A gate worth the whole [`CUT_DETOUR`], on `production_value`'s district
/// scale (a first Government Plaza is 420, the Conquest Encampment 170).
const ENCAMPMENT_SEAL_VALUE: f64 = 180.0;

/// The garrison reads gates this far from one of our cities: inside the
/// ground the empire can actually supply and reinforce.
const GATE_CITY_RANGE: i32 = 5;
/// Somebody has to be able to come through it, from this far out.
const GATE_SOURCE_RANGE: i32 = 14;
/// Extra steps a gate must cost before a soldier is spent standing on it —
/// higher than [`MIN_DETOUR`], because a body is worth more than a tiebreak.
const GATE_MIN_DETOUR: f64 = 5.0;
/// How many gates the empire mans at once. Two: this is the ground nothing
/// else wanted a unit for, and an empire that garrisons every neck has no
/// field army.
const GATE_POSTS_MAX: usize = 2;
/// A unit standing in one of our cities leaves it for a gate only while the
/// city keeps at least this many military units of its own.
const GATE_MIN_GARRISON: usize = 2;
/// A unit further than this from a gate is somebody else's business.
const GATE_UNIT_RANGE: i32 = 8;
/// Gate candidates kept per post, so an unmannable one does not spend a post.
const GATE_CANDIDATE_SLACK: usize = 3;

/// What one tile is worth as a gate, in each of the three senses. Every field
/// is extra steps, capped at [`CUT_DETOUR`], and zero when the tile is not a
/// gate in that sense at all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Narrows {
    /// A land mover's detour when this land tile is closed to it: the pass.
    pub(super) land: f64,
    /// A hull's detour when this water tile is closed to it: the strait.
    pub(super) sea: f64,
    /// A hull's detour around this LAND tile, which is what a naval passage
    /// standing on it — a city center — would save: the canal.
    pub(super) canal: f64,
}

/// Every chokepoint reading taken this turn.
///
/// Modelled on `SettlementAtlas` and keyed on the turn and the player — but
/// deliberately NOT on `map.tiles.epoch()`, which `SettlementAtlas` does use.
/// That counter rises on every write to any tile, so an atlas keyed on it is
/// cleared several times inside one acting turn, and a reading here is a
/// flood rather than a lookup. Nothing this controller does in a turn can
/// change what a reading depends on: [`AdvancedAi::read_narrows`] asks for
/// terrain passability, water, and what the player has explored, and an
/// improvement, a border, a district or a unit moves none of them. ⚠ The one
/// thing that does — a disaster closing a tile — is therefore seen on the
/// next turn rather than this one, and exploration inside a turn is likewise
/// only picked up next turn, which is conservative in the safe direction (an
/// uncharted ring is refused, never called a pass).
#[derive(Default)]
pub(super) struct NarrowsAtlas {
    turn: Option<u32>,
    pid: usize,
    tiles: BTreeMap<Pos, Narrows>,
}

impl Clone for NarrowsAtlas {
    /// A controller clone is a speculative branch; it rebuilds what it reads.
    fn clone(&self) -> Self {
        Self::default()
    }
}

impl NarrowsAtlas {
    fn matches(&self, g: &Game, pid: usize) -> bool {
        self.turn == Some(g.turn) && self.pid == pid
    }

    fn start(&mut self, g: &Game, pid: usize) {
        self.turn = Some(g.turn);
        self.pid = pid;
        self.tiles.clear();
    }
}

/// One gate the garrison would hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Gate {
    /// The tile held.
    pub(super) at: Pos,
    /// Extra steps its closure costs whoever comes through it.
    pub(super) detour: f64,
    /// A strait, held by a hull, rather than a pass held by a soldier.
    pub(super) sea: bool,
}

/// This turn's gates and who holds them.
#[derive(Clone, Debug, Default)]
pub(super) struct GatePlan {
    /// The turn the plan was drawn for; a step on any other turn has no post.
    turn: Option<u32>,
    /// The gates worth holding, best first.
    gates: Vec<Gate>,
    /// Post assignments by unit.
    posts: BTreeMap<u32, Pos>,
}

impl GatePlan {
    /// The gates drawn this turn, for explainers and tests.
    #[cfg(test)]
    pub(super) fn gates(&self) -> &[Gate] {
        &self.gates
    }

    /// The post a unit was sent to, for explainers and tests.
    #[cfg(test)]
    pub(super) fn post(&self, uid: u32) -> Option<Pos> {
        self.posts.get(&uid).copied()
    }
}

impl AdvancedAi {
    // ------------------------------------------------------------------
    // reading the map
    // ------------------------------------------------------------------

    /// What `pos` is worth as a gate, from this turn's atlas.
    pub(super) fn narrows_at(&self, g: &Game, pid: usize, pos: Pos) -> Narrows {
        {
            let mut atlas = self.narrows_atlas.borrow_mut();
            if !atlas.matches(g, pid) {
                atlas.start(g, pid);
            }
            if let Some(read) = atlas.tiles.get(&pos) {
                return *read;
            }
        }
        let read = Self::read_narrows(g, pid, pos);
        self.narrows_atlas.borrow_mut().tiles.insert(pos, read);
        read
    }

    /// The three readings for one tile. See the module header: land groups
    /// around land are a pass, water groups around water a strait, and water
    /// groups around land a canal.
    fn read_narrows(g: &Game, pid: usize, pos: Pos) -> Narrows {
        let explored = &g.players[pid].explored;
        let Some(tile) = g.map.get(pos) else {
            return Narrows::default();
        };
        if !explored.contains(&pos) {
            return Narrows::default();
        }
        // ⚠ Fog-honest: unknown ground reads as a wall to both floods below,
        // so a tile whose own ring is not charted would be called a pass by
        // the fog alone. Every edge of the explored map is such a tile.
        if !g.nbrs(pos).iter().all(|near| explored.contains(near)) {
            return Narrows::default();
        }
        let land = |at: Pos| {
            explored.contains(&at)
                && g.map
                    .get(at)
                    .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        };
        // ⚠ Water a ship cannot enter is not sea. `data/features.json` marks
        // `ice` impassable and it sits on ocean, so a sea membership that
        // asked only `is_water` would read the polar cap as open water and
        // call every gap in it a strait.
        let sea = |at: Pos| {
            explored.contains(&at)
                && g.map
                    .get(at)
                    .is_some_and(|tile| g.rules.is_water(tile) && g.rules.is_passable(tile))
        };
        if !g.rules.is_passable(tile) {
            // Ground nothing may enter — a mountain, the ice — is what MAKES
            // the narrows beside it. It is never one itself.
            return Narrows::default();
        }
        if g.rules.is_water(tile) {
            return Narrows {
                sea: Self::gate_detour(g, pos, &sea),
                ..Narrows::default()
            };
        }
        Narrows {
            land: Self::gate_detour(g, pos, &land),
            canal: Self::gate_detour(g, pos, &sea),
            sea: 0.0,
        }
    }

    /// Extra steps the worst-hit pair of sides pays when `pos` is closed to
    /// them, capped at [`CUT_DETOUR`]; zero when the tile is not narrow, when
    /// no pair has two real bodies behind it, or when the way round is short.
    fn gate_detour(g: &Game, pos: Pos, member: &dyn Fn(Pos) -> bool) -> f64 {
        let sides = Self::arcs_around(g, pos, member);
        if sides.len() < 2 {
            return 0.0;
        }
        let floods: Vec<(Pos, BTreeMap<Pos, i32>)> = sides
            .into_iter()
            .map(|side| (side, Self::flood(g, side, pos, member)))
            .collect();
        let mut worst = 0.0f64;
        for (index, (_, near)) in floods.iter().enumerate() {
            if near.len() < MIN_BODY {
                continue;
            }
            for (far, other) in floods.iter().skip(index + 1) {
                if other.len() < MIN_BODY {
                    continue;
                }
                // Both ends touch `pos`, so the route through it is two steps.
                let detour = match near.get(far) {
                    Some(steps) => (*steps as f64 - 2.0).max(0.0),
                    None => CUT_DETOUR,
                };
                worst = worst.max(detour.min(CUT_DETOUR));
            }
        }
        if worst < MIN_DETOUR {
            0.0
        } else {
            worst
        }
    }

    /// One representative of each group of `pos`'s neighbours that satisfy
    /// `member` and touch each other. Two or more groups is what "narrow"
    /// means. Grouped by mutual adjacency rather than by ring order, so the
    /// answer is the same on a cylinder, a bounded arena and a globe.
    fn arcs_around(g: &Game, pos: Pos, member: &dyn Fn(Pos) -> bool) -> Vec<Pos> {
        let mut left: BTreeSet<Pos> = g
            .nbrs(pos)
            .into_iter()
            .filter(|near| member(*near))
            .collect();
        let mut sides = Vec::new();
        while let Some(first) = left.iter().next().copied() {
            left.remove(&first);
            let mut group = vec![first];
            let mut queue = vec![first];
            while let Some(current) = queue.pop() {
                let touching: Vec<Pos> = left
                    .iter()
                    .copied()
                    .filter(|other| g.wdist(current, *other) == 1)
                    .collect();
                for other in touching {
                    left.remove(&other);
                    group.push(other);
                    queue.push(other);
                }
            }
            sides.push(group.into_iter().min().expect("a group holds its own seed"));
        }
        sides.sort();
        sides
    }

    /// Steps from `from` to every tile it reaches over `member` ground
    /// without using `avoid`, no further than [`WINDOW`] tiles from `avoid`.
    fn flood(g: &Game, from: Pos, avoid: Pos, member: &dyn Fn(Pos) -> bool) -> BTreeMap<Pos, i32> {
        let mut seen = BTreeMap::new();
        if !member(from) {
            return seen;
        }
        let mut queue = VecDeque::new();
        seen.insert(from, 0);
        queue.push_back(from);
        while let Some(current) = queue.pop_front() {
            let steps = seen[&current];
            for next in g.nbrs(current) {
                if next == avoid
                    || seen.contains_key(&next)
                    || g.wdist(next, avoid) > WINDOW
                    || !member(next)
                {
                    continue;
                }
                seen.insert(next, steps + 1);
                queue.push_back(next);
            }
        }
        seen
    }

    /// Whether anybody else could use a passage here: an explored foreign
    /// city or barbarian camp within [`CLAIM_RIVAL_RANGE`]. Read through our
    /// own exploration, so an unmet empire behind the fog does not make us
    /// spend Gold.
    fn a_rival_can_use(g: &Game, pid: usize, pos: Pos) -> bool {
        let explored = &g.players[pid].explored;
        g.cities.values().any(|city| {
            city.owner != pid
                && explored.contains(&city.pos)
                && g.wdist(city.pos, pos) <= CLAIM_RIVAL_RANGE
        }) || g
            .barb_camps
            .keys()
            .any(|camp| explored.contains(camp) && g.wdist(*camp, pos) <= CLAIM_RIVAL_RANGE)
    }

    // ------------------------------------------------------------------
    // chokepoint-siting
    // ------------------------------------------------------------------

    /// What the gates inside a candidate city's own ground are worth to its
    /// site value. Zero with the gene off, and zero for a gate somebody
    /// already owns — a tile inside a border is already held or already lost.
    pub(super) fn chokepoint_site_bonus(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if !self.chokepoint_siting {
            return 0.0;
        }
        let mut worth: Vec<f64> = Vec::new();
        for tile in g.wdisk(pos, SITE_RINGS) {
            if g.map.get(tile).is_none_or(|tile| tile.owner_city.is_some()) {
                continue;
            }
            let ring = g.wdist(pos, tile).clamp(0, SITE_RINGS) as usize;
            let narrows = self.narrows_at(g, pid, tile);
            let value = (narrows.land / CUT_DETOUR * SITE_LAND_GATE
                + narrows.sea / CUT_DETOUR * SITE_SEA_GATE)
                * SITE_RING_WEIGHT[ring];
            if value > 0.0 {
                worth.push(value);
            }
        }
        worth.sort_by(|left, right| right.total_cmp(left));
        worth
            .into_iter()
            .take(SITE_TILES)
            .sum::<f64>()
            .min(SITE_MAX)
    }

    // ------------------------------------------------------------------
    // canal-city
    // ------------------------------------------------------------------

    /// What a city center standing on this tile would be worth as a naval
    /// passage no rival may use. Zero with the gene off.
    pub(super) fn canal_city_bonus(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if !self.canal_city {
            return 0.0;
        }
        let saved = self.narrows_at(g, pid, pos).canal;
        if saved <= 0.0 {
            return 0.0;
        }
        CANAL_CITY_VALUE * (saved / CUT_DETOUR).min(1.0)
    }

    // ------------------------------------------------------------------
    // chokepoint-claim
    // ------------------------------------------------------------------

    /// What closing this plot with our own border is worth on the Gold
    /// shortlist's scale. Zero with the gene off, for a tile that gates
    /// nothing, and for a passage no one else can reach.
    pub(super) fn chokepoint_plot_bonus(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if !self.chokepoint_claim {
            return 0.0;
        }
        let narrows = self.narrows_at(g, pid, pos);
        let gate = narrows.land.max(narrows.sea);
        if gate <= 0.0 || !Self::a_rival_can_use(g, pid, pos) {
            return 0.0;
        }
        CLAIM_VALUE * (gate / CUT_DETOUR).min(1.0)
    }

    // ------------------------------------------------------------------
    // encampment-seals-the-pass
    // ------------------------------------------------------------------

    /// What an Encampment on this plot is worth as a permanent wall. Zero
    /// with the gene off and for every other district family.
    pub(super) fn encampment_seal_bonus(
        &self,
        g: &Game,
        pid: usize,
        family: &str,
        pos: Pos,
    ) -> f64 {
        if !self.encampment_seals_the_pass || family != "encampment" {
            return 0.0;
        }
        let pass = self.narrows_at(g, pid, pos).land;
        if pass < GATE_MIN_DETOUR || !Self::a_rival_can_use(g, pid, pos) {
            return 0.0;
        }
        ENCAMPMENT_SEAL_VALUE * (pass / CUT_DETOUR).min(1.0)
    }

    // ------------------------------------------------------------------
    // chokepoint-garrison
    // ------------------------------------------------------------------

    /// Draw this turn's gates and their garrisons from the start-of-turn
    /// board, so units planned in parallel agree on them. Nothing is read
    /// with the gene off.
    pub(super) fn chokepoint_gate_plan(&mut self, g: &Game, pid: usize) {
        if !self.chokepoint_garrison {
            return;
        }
        if g.is_arena() || self.base.minor || self.base.barb {
            self.chokepoint_gates = GatePlan::default();
            return;
        }
        let mut plan = GatePlan {
            turn: Some(g.turn),
            ..GatePlan::default()
        };
        self.read_gates(g, pid, &mut plan);
        self.man_the_gates(g, pid, &mut plan);
        self.chokepoint_gates = plan;
    }

    /// The gates on the approaches to our own cities, best first.
    fn read_gates(&self, g: &Game, pid: usize, plan: &mut GatePlan) {
        let explored = &g.players[pid].explored;
        // A city-state counts: `is_at_war` derives a client's side from its
        // suzerain, so the minor on our border is a second army the moment
        // somebody else's envoys take it. A teammate does not.
        let sources: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| {
                city.owner != pid && !g.same_team(pid, city.owner) && explored.contains(&city.pos)
            })
            .map(|city| city.pos)
            .chain(
                g.barb_camps
                    .keys()
                    .copied()
                    .filter(|camp| explored.contains(camp)),
            )
            .collect();
        if sources.is_empty() {
            return;
        }
        let mut gates: BTreeMap<Pos, (Gate, i32)> = BTreeMap::new();
        for cid in g.player_city_ids(pid) {
            let home = g.cities[&cid].pos;
            let near: Vec<Pos> = sources
                .iter()
                .copied()
                .filter(|source| g.wdist(*source, home) <= GATE_SOURCE_RANGE)
                .collect();
            if near.is_empty() {
                continue;
            }
            for at in g.wdisk(home, GATE_CITY_RANGE) {
                if at == home || !Self::gate_stands(g, pid, at) {
                    continue;
                }
                let narrows = self.narrows_at(g, pid, at);
                let sea = narrows.sea > narrows.land;
                let detour = narrows.land.max(narrows.sea);
                if detour < GATE_MIN_DETOUR {
                    continue;
                }
                // On the approach, not behind us: the gate has to be nearer
                // to whoever would come through it than our own city is.
                if !near
                    .iter()
                    .any(|source| g.wdist(at, *source) < g.wdist(home, *source))
                {
                    continue;
                }
                // A corridor is several tiles long and every one of them
                // cuts it equally. Hold the one nearest home: the same wall,
                // inside our own reinforcement and healing. Two of our cities
                // can read the same tile, and the reading itself is a
                // property of the tile — only the distance home differs.
                let reach = g.wdist(at, home);
                let gate = Gate { at, detour, sea };
                gates
                    .entry(at)
                    .and_modify(|(held, held_reach)| {
                        if reach < *held_reach {
                            *held = gate;
                            *held_reach = reach;
                        }
                    })
                    .or_insert((gate, reach));
            }
        }
        let mut ordered: Vec<(Gate, i32)> = gates.into_values().collect();
        ordered.sort_by(|(left, left_reach), (right, right_reach)| {
            right
                .detour
                .total_cmp(&left.detour)
                .then(left_reach.cmp(right_reach))
                .then(left.at.cmp(&right.at))
        });
        // ⚠ Keep more candidates than posts. A strait ranks by its own
        // detour and an empire with no hull in reach cannot hold one, so
        // truncating to the post count here would let an unmannable gate
        // spend a post a land gate could have used. `man_the_gates` stops at
        // the post count instead, once it has bodies for them.
        ordered.truncate(GATE_POSTS_MAX * GATE_CANDIDATE_SLACK);
        plan.gates = ordered.into_iter().map(|(gate, _)| gate).collect();
    }

    /// Whether a gate can be stood on at all: open ground of ours or nobody's,
    /// with no city on it.
    fn gate_stands(g: &Game, pid: usize, at: Pos) -> bool {
        g.city_at(at).is_none()
            && g.map.get(at).is_some_and(|tile| {
                tile.owner_city
                    .and_then(|city| g.cities.get(&city))
                    .is_none_or(|city| city.owner == pid)
            })
    }

    /// Send the nearest unit nothing else has claimed to each gate, best gate
    /// first, and stop once [`GATE_POSTS_MAX`] of them are manned.
    fn man_the_gates(&self, g: &Game, pid: usize, plan: &mut GatePlan) {
        let mut taken: BTreeSet<u32> = BTreeSet::new();
        let mut manned: Vec<Gate> = Vec::new();
        for gate in plan.gates.clone() {
            if manned.len() >= GATE_POSTS_MAX {
                break;
            }
            let best = g
                .player_unit_ids(pid)
                .into_iter()
                .filter(|uid| !taken.contains(uid) && self.gate_unit(g, pid, *uid, gate.sea))
                .map(|uid| (g.wdist(g.units[&uid].pos, gate.at), uid))
                .filter(|(distance, _)| *distance <= GATE_UNIT_RANGE)
                .min();
            if let Some((_, uid)) = best {
                taken.insert(uid);
                plan.posts.insert(uid, gate.at);
                manned.push(gate);
            }
        }
        // The plan keeps the gates it can actually hold: a candidate nothing
        // could reach is not this turn's business.
        if !manned.is_empty() {
            plan.gates = manned;
        }
    }

    /// Whether this unit may be sent to a gate: our own military body of the
    /// gate's domain, not spoken for by the joint plan or a settler escort,
    /// and — if it stands in one of our cities — a surplus over the garrison
    /// that city keeps.
    fn gate_unit(&self, g: &Game, pid: usize, uid: u32, sea: bool) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let spec = &g.rules.units[unit.kind];
        if unit.owner != pid || spec.class != "military" {
            return false;
        }
        let naval = spec.domain.as_deref() == Some("sea");
        if naval != sea || spec.domain.as_deref() == Some("air") {
            return false;
        }
        if !naval && g.is_embarked(unit) {
            return false;
        }
        if self.tactics_resolved.contains(&uid)
            || self.tactics_withdrawn.contains(&uid)
            || self.settler_guards.values().any(|guard| *guard == uid)
        {
            return false;
        }
        // The city keeps its own defenders; only the surplus walks out.
        if g.city_at(unit.pos).is_some() {
            let garrison = g
                .unit_ids_at(unit.pos)
                .iter()
                .filter(|other| {
                    let other = &g.units[*other];
                    other.owner == pid && g.rules.units[other.kind].class == "military"
                })
                .count();
            if garrison <= GATE_MIN_GARRISON {
                return false;
            }
        }
        true
    }

    /// This unit's gate order for the turn. `Some(true)` when it walked,
    /// `Some(false)` when it holds the gate, `None` when it has no post and
    /// the ordinary peacetime step follows.
    pub(super) fn chokepoint_garrison_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.chokepoint_garrison || self.chokepoint_gates.turn != Some(g.turn) {
            return None;
        }
        let post = self.chokepoint_gates.posts.get(&uid).copied()?;
        let here = g.units.get(&uid)?.pos;
        if here == post {
            // Holding: nothing foreign may enter a tile our military body
            // stands on, and the fortify is the engine's own defense bonus
            // for the fight that has to happen if anyone wants through.
            // `fortify_or_stop` reports that no turn was spent, exactly as
            // the picket's own hold does.
            return Some(self.base.fortify_or_stop(g, pid, uid));
        }
        let kind = g.units[&uid].kind;
        let next = g
            .route_step(uid, post, 0)
            .filter(|next| g.can_move(uid, *next))?;
        if !self.base.path_move(g, pid, uid, next) {
            return None;
        }
        think!(self.journal(), Military, Detail,
               "{kind} {uid} walks to the gate at {post:?}";
               "closing that tile costs whoever comes through it {:.0} extra steps, and \
                nothing foreign may enter a tile one of our military units holds",
               self.chokepoint_gates
                   .gates
                   .iter()
                   .find(|gate| gate.at == post)
                   .map(|gate| gate.detour)
                   .unwrap_or_default();
               post);
        Some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::genes::GENES;
    use super::*;
    use crate::game::Game;
    use crate::name;

    fn opt_in_off_in_both_controllers(tag: &str, read: fn(&AdvancedAi) -> bool) {
        assert!(!read(&AdvancedAi::new()), "{tag} must be off in new()");
        assert!(
            !read(&AdvancedAi::legacy()),
            "{tag} must be off in legacy()"
        );
        let gene = GENES
            .iter()
            .find(|gene| gene.tag == tag)
            .expect("the gene is published for gene_screen");
        assert!(gene.opt_in() && gene.screenable() && !gene.live());
        let mut ai = AdvancedAi::new();
        (gene.enable)(&mut ai);
        assert!(read(&ai));
        (gene.disable)(&mut ai);
        assert!(!read(&ai));
    }

    #[test]
    fn chokepoint_siting_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("chokepoint-siting", |ai| ai.chokepoint_siting);
    }

    #[test]
    fn canal_city_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("canal-city", |ai| ai.canal_city);
    }

    #[test]
    fn chokepoint_claim_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("chokepoint-claim", |ai| ai.chokepoint_claim);
    }

    #[test]
    fn encampment_seals_the_pass_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("encampment-seals-the-pass", |ai| {
            ai.encampment_seals_the_pass
        });
    }

    #[test]
    fn chokepoint_garrison_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("chokepoint-garrison", |ai| ai.chokepoint_garrison);
    }

    /// The board every fixture below paints on: two majors who have met, a
    /// flat fully-explored world of grassland, no units and no camps. The
    /// hex metric, the wrap and the map's own neighbour lists are the real
    /// ones — only the terrain is ours.
    fn flat_board(seed: u64) -> Game {
        let mut game = Game::new_full(2, 28, 18, seed, 1_000, 0, false);
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.owner_city = None;
            tile.district = None;
        }
        for pid in 0..2 {
            game.players[pid]
                .explored
                .extend(game.map.tiles.keys().copied());
        }
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        game.turn = 60;
        game.current = 0;
        game
    }

    /// The tile at an offset column and row of the fixture board.
    fn at(game: &Game, col: i32, row: i32) -> Pos {
        let pos = crate::hex::offset_to_axial(col, row);
        assert!(
            game.map.tiles.contains_key(&pos),
            "the fixture board has no tile at column {col}, row {row}"
        );
        pos
    }

    fn paint(game: &mut Game, pos: Pos, terrain: &str) {
        let tile = game.map.tiles.get_mut(&pos).expect("a painted tile exists");
        tile.terrain = crate::name::Name::new(terrain);
        tile.hills = false;
        tile.feature = None;
    }

    /// A wall of mountains down one column of every row, with a single gap:
    /// the mountain pass. Returns the board and the gap.
    fn mountain_wall(seed: u64, col: i32, gap_row: i32) -> (Game, Pos) {
        let mut game = flat_board(seed);
        for row in 0..18 {
            if row == gap_row {
                continue;
            }
            let pos = at(&game, col, row);
            paint(&mut game, pos, "mountain");
        }
        let gap = at(&game, col, gap_row);
        (game, gap)
    }

    /// Two seas the whole width of the world, separated by one row of land:
    /// every tile of that row is a canal site. Returns the board.
    fn two_seas(seed: u64, land_row: i32) -> Game {
        let mut game = flat_board(seed);
        for pos in game.map.tiles.keys().copied().collect::<Vec<_>>() {
            let row = pos.1;
            if row < land_row || (row > land_row && row <= land_row + 6) {
                paint(&mut game, pos, "coast");
            }
        }
        game
    }

    /// Start of turn for one unit: full movement, nothing spent.
    fn fresh(game: &mut Game, uid: u32) {
        let moves = game.unit_max_moves(uid);
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.attacks_left = 1;
        unit.moved = false;
        unit.acted = false;
    }

    // ------------------------------------------------------------------
    // reading the map
    // ------------------------------------------------------------------

    #[test]
    fn the_gap_in_a_mountain_wall_is_a_pass_and_open_ground_is_not() {
        let (game, gap) = mountain_wall(717_001, 14, 9);
        let ai = AdvancedAi::new();
        let pass = ai.narrows_at(&game, 0, gap);
        assert_eq!(
            pass.land, CUT_DETOUR,
            "the two halves do not reconnect inside the window, so the gap is worth the cap"
        );
        assert_eq!(pass.sea, 0.0, "there is no water on this board");
        assert_eq!(pass.canal, 0.0);
        let open = at(&game, 4, 9);
        assert_eq!(
            ai.narrows_at(&game, 0, open),
            Narrows::default(),
            "open ground gates nothing"
        );
        // The wall itself is what MAKES the pass; it is not one.
        let wall = at(&game, 14, 8);
        assert_eq!(ai.narrows_at(&game, 0, wall), Narrows::default());
    }

    #[test]
    fn a_row_of_land_between_two_seas_is_a_canal_and_a_channel_through_it_is_a_strait() {
        let mut game = two_seas(717_002, 6);
        let ai = AdvancedAi::new();
        let bridge = at(&game, 9, 6);
        let canal = ai.narrows_at(&game, 0, bridge);
        assert_eq!(
            canal.canal, CUT_DETOUR,
            "the two seas never meet, so a city center standing here is the only way through"
        );
        assert_eq!(
            canal.land, 0.0,
            "a one-tile land bridge has no body of land on either side inside the window, \
             which is exactly what MIN_BODY refuses"
        );

        // Cut a channel through the bridge: that water tile is now the only
        // way a hull passes, which is the strait.
        let channel = at(&game, 20, 6);
        paint(&mut game, channel, "coast");
        let ai = AdvancedAi::new();
        let strait = ai.narrows_at(&game, 0, channel);
        assert_eq!(strait.sea, CUT_DETOUR);
        assert_eq!(strait.land, 0.0);
        assert_eq!(strait.canal, 0.0);

        // And a land tile far from the channel still reads as a canal site,
        // because the way round is longer than the window.
        let ai = AdvancedAi::new();
        assert_eq!(ai.narrows_at(&game, 0, at(&game, 9, 6)).canal, CUT_DETOUR);
    }

    #[test]
    fn the_ice_is_a_wall_and_not_a_sea() {
        let mut game = two_seas(717_013, 6);
        let bridge = at(&game, 9, 6);
        assert_eq!(
            AdvancedAi::new().narrows_at(&game, 0, bridge).canal,
            CUT_DETOUR
        );
        // Freeze the northern sea. Ice is water the engine refuses to any
        // hull (`data/features.json`), so there is no longer a second sea to
        // join, and the ice tiles themselves gate nothing.
        for pos in game.map.tiles.keys().copied().collect::<Vec<_>>() {
            if pos.1 < 6 {
                game.map.tiles.get_mut(&pos).unwrap().feature = Some(name!("ice"));
            }
        }
        let frozen = AdvancedAi::new();
        assert_eq!(frozen.narrows_at(&game, 0, bridge), Narrows::default());
        assert_eq!(
            frozen.narrows_at(&game, 0, at(&game, 9, 3)),
            Narrows::default(),
            "a tile nothing may enter is never itself a narrows"
        );
    }

    #[test]
    fn a_tile_whose_own_ring_is_not_charted_is_never_called_a_pass() {
        let (mut game, gap) = mountain_wall(717_003, 14, 9);
        let charted = AdvancedAi::new().narrows_at(&game, 0, gap);
        assert_eq!(charted.land, CUT_DETOUR);
        let dark = *game
            .nbrs(gap)
            .first()
            .expect("the gap has neighbours on the fixture board");
        game.players[0].explored.remove(&dark);
        assert_eq!(
            AdvancedAi::new().narrows_at(&game, 0, gap),
            Narrows::default(),
            "unknown ground reads as a wall to the flood, so a fog edge would be a pass"
        );
        assert_eq!(
            AdvancedAi::new().narrows_at(&game, 1, gap).land,
            CUT_DETOUR,
            "the other player has charted it and still sees the pass"
        );
    }

    // ------------------------------------------------------------------
    // the genes
    // ------------------------------------------------------------------

    #[test]
    fn every_bonus_is_zero_with_its_gene_off() {
        let (mut game, gap) = mountain_wall(717_004, 14, 9);
        let beside = at(&game, 13, 9);
        game.found_city_for(0, at(&game, 17, 9), None);
        game.found_city_for(1, at(&game, 8, 9), None);
        let off = AdvancedAi::new();
        assert_eq!(off.chokepoint_site_bonus(&game, 0, beside), 0.0);
        assert_eq!(off.canal_city_bonus(&game, 0, beside), 0.0);
        assert_eq!(off.chokepoint_plot_bonus(&game, 0, gap), 0.0);
        assert_eq!(off.encampment_seal_bonus(&game, 0, "encampment", gap), 0.0);
        let mut ai = AdvancedAi::new();
        ai.chokepoint_gate_plan(&game, 0);
        assert!(ai.chokepoint_gates.gates().is_empty());
        assert_eq!(ai.chokepoint_garrison_step(&mut game, 0, 0), None);
    }

    #[test]
    fn a_site_beside_the_pass_outscores_one_in_open_ground() {
        let (game, gap) = mountain_wall(717_005, 14, 9);
        let mut ai = AdvancedAi::new();
        ai.enable_chokepoint_siting();
        let beside = at(&game, 13, 9);
        let open = at(&game, 5, 3);
        let commanding = ai.chokepoint_site_bonus(&game, 0, beside);
        assert!(
            commanding > 0.0 && commanding <= SITE_MAX,
            "a site whose first ring covers the pass is worth {commanding}"
        );
        assert_eq!(ai.chokepoint_site_bonus(&game, 0, open), 0.0);
        // Ring 3 still counts, at its own weight.
        let far = at(&game, 11, 9);
        let distant = ai.chokepoint_site_bonus(&game, 0, far);
        assert!(
            distant > 0.0 && distant < commanding,
            "ring 3 is worth {distant} against {commanding} at ring 1"
        );
        // A gate already inside somebody's border is not a prize.
        let mut owned = game.clone();
        owned.found_city_for(1, gap, None);
        assert_eq!(ai.chokepoint_site_bonus(&owned, 0, beside), 0.0);
    }

    #[test]
    fn the_canal_bonus_is_paid_on_the_land_bridge_and_nowhere_else() {
        let game = two_seas(717_006, 6);
        let mut ai = AdvancedAi::new();
        ai.enable_canal_city();
        let bridge = at(&game, 9, 6);
        assert_eq!(ai.canal_city_bonus(&game, 0, bridge), CANAL_CITY_VALUE);
        let inland = at(&game, 9, 15);
        assert_eq!(ai.canal_city_bonus(&game, 0, inland), 0.0);
    }

    #[test]
    fn the_site_bonuses_reach_the_settle_value_the_scan_ranks() {
        let (game, _) = mountain_wall(717_007, 14, 9);
        let beside = at(&game, 13, 9);
        let plain = AdvancedAi::new();
        let visible = plain.battlefront_visibility(&game, 0);
        let before = plain.settle_value_visible(&game, 0, beside, &visible);
        let mut ai = AdvancedAi::new();
        ai.enable_chokepoint_siting();
        let after = ai.settle_value_visible(&game, 0, beside, &visible);
        assert!(
            after > before,
            "the gene has to reach the value the site scan ranks: {before} -> {after}"
        );
        assert!((after - before - ai.chokepoint_site_bonus(&game, 0, beside)).abs() < 1e-9);
    }

    #[test]
    fn a_passage_nobody_can_reach_is_not_worth_gold_or_a_district() {
        let (mut game, gap) = mountain_wall(717_008, 14, 9);
        let mut ai = AdvancedAi::new();
        ai.enable_chokepoint_claim();
        ai.enable_encampment_seals_the_pass();
        assert_eq!(
            ai.chokepoint_plot_bonus(&game, 0, gap),
            0.0,
            "an empty world has nobody to keep out"
        );
        assert_eq!(ai.encampment_seal_bonus(&game, 0, "encampment", gap), 0.0);
        game.found_city_for(1, at(&game, 8, 9), None);
        let ai = {
            let mut ai = AdvancedAi::new();
            ai.enable_chokepoint_claim();
            ai.enable_encampment_seals_the_pass();
            ai
        };
        assert_eq!(ai.chokepoint_plot_bonus(&game, 0, gap), CLAIM_VALUE);
        assert_eq!(
            ai.encampment_seal_bonus(&game, 0, "encampment", gap),
            ENCAMPMENT_SEAL_VALUE
        );
        assert_eq!(
            ai.encampment_seal_bonus(&game, 0, "campus", gap),
            0.0,
            "only the district that is a wall is paid for being one"
        );
        assert_eq!(
            ai.chokepoint_plot_bonus(&game, 0, at(&game, 5, 3)),
            0.0,
            "open ground closes nothing"
        );
    }

    #[test]
    fn the_garrison_walks_to_the_gate_and_then_holds_it_fortified() {
        let (mut game, gap) = mountain_wall(717_009, 14, 9);
        let home = at(&game, 17, 9);
        // The tile west of the gap cuts the wall just as the gap does, so
        // the post is decided by the tie-break: the end nearest home.
        let gate = gap;
        assert_eq!(
            AdvancedAi::new()
                .narrows_at(&game, 0, at(&game, 13, 9))
                .land,
            CUT_DETOUR,
            "the corridor is more than one tile long"
        );
        game.found_city_for(0, home, None);
        game.found_city_for(1, at(&game, 8, 9), None);
        let uid = game.spawn_unit("warrior", 0, at(&game, 16, 9));
        let mut ai = AdvancedAi::new();
        ai.enable_chokepoint_garrison();
        ai.chokepoint_gate_plan(&game, 0);
        assert_eq!(
            ai.chokepoint_gates.gates().first().map(|gate| gate.at),
            Some(gate),
            "the pass on the approach from the neighbour is the gate, held at the \
             end of the corridor nearest home"
        );
        assert_eq!(ai.chokepoint_gates.post(uid), Some(gate));

        fresh(&mut game, uid);
        let before = game.wdist(game.units[&uid].pos, gate);
        assert_eq!(ai.chokepoint_garrison_step(&mut game, 0, uid), Some(true));
        assert!(
            game.wdist(game.units[&uid].pos, gate) < before,
            "the walk closes on the gate"
        );

        // Standing on it, the order is to hold and fortify.
        game.remove_unit(uid);
        let holder = game.spawn_unit("warrior", 0, gate);
        fresh(&mut game, holder);
        ai.chokepoint_gate_plan(&game, 0);
        assert_eq!(ai.chokepoint_gates.post(holder), Some(gate));
        assert_eq!(
            ai.chokepoint_garrison_step(&mut game, 0, holder),
            Some(false)
        );
        assert!(
            game.units[&holder].fortified,
            "a body that holds a gate fortifies on it"
        );
    }

    #[test]
    fn a_city_does_not_send_its_own_last_defender_to_a_gate() {
        let (mut game, gap) = mountain_wall(717_010, 14, 9);
        let home = at(&game, 17, 9);
        game.found_city_for(0, home, None);
        game.found_city_for(1, at(&game, 8, 9), None);
        let lone = game.spawn_unit("warrior", 0, home);
        let mut ai = AdvancedAi::new();
        ai.enable_chokepoint_garrison();
        ai.chokepoint_gate_plan(&game, 0);
        assert_eq!(
            ai.chokepoint_gates.gates().first().map(|gate| gate.at),
            Some(gap)
        );
        assert_eq!(
            ai.chokepoint_gates.post(lone),
            None,
            "the city's whole garrison is not a surplus"
        );
        for _ in 0..GATE_MIN_GARRISON {
            game.spawn_unit("warrior", 0, home);
        }
        ai.chokepoint_gate_plan(&game, 0);
        assert!(
            ai.chokepoint_gates.post(lone).is_some()
                || game
                    .player_unit_ids(0)
                    .iter()
                    .any(|uid| ai.chokepoint_gates.post(*uid) == Some(gap)),
            "with a garrison to spare one body walks out to the gate"
        );
    }

    #[test]
    fn a_gate_no_body_can_reach_does_not_spend_a_post() {
        // Two gaps in the same wall, and one soldier who can reach only the
        // southern one. The northern gate sorts FIRST — it is nearer its own
        // city — so a plan that spent a post on every candidate in order
        // would leave the gate we can actually hold unheld.
        let (mut game, south) = mountain_wall(717_012, 14, 9);
        let north = at(&game, 14, 2);
        paint(&mut game, north, "grassland");
        game.found_city_for(0, at(&game, 17, 9), None);
        game.found_city_for(0, at(&game, 16, 2), None);
        game.found_city_for(1, at(&game, 8, 9), None);
        let uid = game.spawn_unit("warrior", 0, at(&game, 18, 12));
        assert!(
            game.wdist(game.units[&uid].pos, north) > GATE_UNIT_RANGE
                && game.wdist(game.units[&uid].pos, south) <= GATE_UNIT_RANGE,
            "the fixture puts exactly one of the two gates in reach"
        );
        let mut ai = AdvancedAi::new();
        ai.enable_chokepoint_garrison();
        ai.chokepoint_gate_plan(&game, 0);
        assert_eq!(ai.chokepoint_gates.post(uid), Some(south));
        assert_eq!(
            ai.chokepoint_gates.gates(),
            &[Gate {
                at: south,
                detour: CUT_DETOUR,
                sea: false,
            }],
            "the plan keeps the gates it can hold and no others"
        );
    }

    #[test]
    fn a_gate_behind_us_is_not_manned() {
        let (mut game, _) = mountain_wall(717_011, 14, 9);
        // Our city sits WEST of the wall and so does the neighbour: the pass
        // is behind us, and nothing comes through it.
        game.found_city_for(0, at(&game, 11, 9), None);
        game.found_city_for(1, at(&game, 6, 9), None);
        game.spawn_unit("warrior", 0, at(&game, 12, 9));
        let mut ai = AdvancedAi::new();
        ai.enable_chokepoint_garrison();
        ai.chokepoint_gate_plan(&game, 0);
        assert!(
            ai.chokepoint_gates.gates().is_empty(),
            "a gate has to be nearer to whoever comes through it than our own city is"
        );
    }
}
