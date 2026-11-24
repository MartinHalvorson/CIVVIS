//! Black swans: events nothing in the world saw coming, struck by hand.
//!
//! Operator 2026-11-24: "add a 'Generate Black Swan' - this will be a random,
//! unpredicted event that spawns and changes the game. Could be instant
//! nuclear war. Bio risk could collapse the population of all cities by 50%.
//! Radiation event that starts leaking radiation around the world. Leader
//! collapse. Civ blossoms out into powerful hostile nuclear armed
//! microstates. stuff like that. black swans on a civilizational timeline."
//!
//! No rule of the game rolls one and no agent can order one: a black swan is
//! struck from outside the game, by whoever is studying it
//! (`Game::black_swan`, which Doctor Strange's Civ 6 reaches through the
//! debugger's `black_swan` operation). Which one it is and where it lands are
//! drawn from the seed it is struck with and from nothing else, so the same
//! seed strikes two copies of one world the same way. It writes the world
//! directly, the way a disaster does, with every consequence the rules
//! already have for what it does: a detonation is the blast of
//! `do_wmd_strike` on the strike ledger a client animates, a city that
//! breaks away is a Loyalty revolt into the Free Cities seat
//! (`revolt_to_free_cities`), a fallen leader is Anarchy.
//!
//! Two of the five are not over when they land, so a struck swan is state
//! and not only an account (`Game::black_swans`): a radiation leak spreads a
//! ring a turn from each of its sites, and a microstate that holds a nuclear
//! device may fire it on any turn it is still a Free City. Both are carried
//! on by the turn's `process_black_swans`, which does nothing at all in a
//! world no swan has struck.

use super::*;
use crate::shared_sort::SharedSort;

/// How many swans a game keeps the account of, oldest first.
const SWANS_KEPT: usize = 16;
/// Instant nuclear war: how many rivals each civilization fires on.
pub(super) const WAR_VOLLEYS: usize = 2;
/// Radiation leak: how many sites leak, how far apart they are looked for,
/// how many turns each goes on spreading a ring a turn, and how long ground
/// it reaches stays contaminated.
pub(super) const LEAK_SITES: usize = 4;
const LEAK_SPACING: i32 = 8;
pub(super) const LEAK_TURNS: u32 = 4;
const LEAK_FALLOUT: u32 = 20;
/// What a leak does to whoever stands on the site when it breaks.
const LEAK_SITE_DAMAGE: i32 = 50;
/// Leader collapse: the turns of Anarchy, and the most Loyalty a city of the
/// fallen leader's keeps (its capital keeps twice that).
pub(super) const COLLAPSE_ANARCHY: u32 = 10;
pub(super) const COLLAPSE_LOYALTY: f64 = 25.0;
/// Microstates: the melee units each stands up, the devices each holds, the
/// odds a turn that one holding a device fires it, and for how many turns
/// their arsenal is kept.
pub(super) const MICROSTATE_DEFENDERS: usize = 4;
const MICROSTATE_DEVICES: u8 = 1;
const MICROSTATE_LAUNCH_ODDS: f64 = 0.2;
pub(super) const MICROSTATE_ARMED_TURNS: u32 = 40;

/// The black swans there are.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BlackSwanKind {
    NuclearWar,
    Pandemic,
    RadiationLeak,
    LeaderCollapse,
    Microstates,
}

impl BlackSwanKind {
    pub const ALL: [BlackSwanKind; 5] = [
        BlackSwanKind::NuclearWar,
        BlackSwanKind::Pandemic,
        BlackSwanKind::RadiationLeak,
        BlackSwanKind::LeaderCollapse,
        BlackSwanKind::Microstates,
    ];

    /// The name a request asks for it by.
    pub fn id(self) -> &'static str {
        match self {
            BlackSwanKind::NuclearWar => "nuclear_war",
            BlackSwanKind::Pandemic => "pandemic",
            BlackSwanKind::RadiationLeak => "radiation_leak",
            BlackSwanKind::LeaderCollapse => "leader_collapse",
            BlackSwanKind::Microstates => "microstates",
        }
    }

    /// The name a page shows it by.
    pub fn name(self) -> &'static str {
        match self {
            BlackSwanKind::NuclearWar => "Instant nuclear war",
            BlackSwanKind::Pandemic => "Pandemic",
            BlackSwanKind::RadiationLeak => "Radiation leak",
            BlackSwanKind::LeaderCollapse => "Leader collapse",
            BlackSwanKind::Microstates => "Microstates",
        }
    }

    /// What it does, in a line.
    pub fn about(self) -> &'static str {
        match self {
            BlackSwanKind::NuclearWar => {
                "Every civilization is at war with every other, and each fires nuclear devices on its strongest rivals' largest cities"
            }
            BlackSwanKind::Pandemic => "Every city in the world loses half its people",
            BlackSwanKind::RadiationLeak => {
                "Cities around the world start leaking radiation: fallout spreads a ring a turn from each"
            }
            BlackSwanKind::LeaderCollapse => {
                "The leading civilization's leader falls: Anarchy, and its cities' Loyalty collapses"
            }
            BlackSwanKind::Microstates => {
                "A civilization breaks up: every city but its capital becomes a hostile Free City with an army and a nuclear device"
            }
        }
    }

    pub fn parse(id: &str) -> Option<BlackSwanKind> {
        Self::ALL.into_iter().find(|kind| kind.id() == id)
    }
}

/// One black swan as it struck, and what of it is still going on.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BlackSwan {
    /// The turn it struck on.
    pub turn: u32,
    pub kind: BlackSwanKind,
    /// What it was struck with (`Game::black_swan`).
    pub seed: u64,
    /// What happened, in a line, with its numbers.
    pub text: String,
    /// Where to look: the cities it landed on, or leaks from.
    #[serde(default)]
    pub sites: Vec<Pos>,
    /// The civilization it struck, where it struck one.
    #[serde(default)]
    pub player: Option<usize>,
    /// The last turn its ongoing part runs on: a leak's last ring, the last
    /// turn a microstate may fire. The turn it struck on for one that was
    /// over as it landed.
    #[serde(default)]
    pub until: u32,
    /// Microstates: the devices each city that broke away still holds, by
    /// the city's id.
    #[serde(default)]
    pub armed: BTreeMap<u32, u8>,
}

impl BlackSwan {
    /// Whether some part of it is still to come on a turn after `turn`.
    pub fn ongoing(&self, turn: u32) -> bool {
        self.until > turn
            && match self.kind {
                BlackSwanKind::Microstates => self.armed.values().any(|devices| *devices > 0),
                _ => true,
            }
    }

    /// The account a page is given.
    pub fn json(&self, turn: u32) -> serde_json::Value {
        serde_json::json!({
            "turn": self.turn,
            "kind": self.kind.id(),
            "name": self.kind.name(),
            "text": self.text,
            "sites": self.sites.iter().map(|site| [site.0, site.1]).collect::<Vec<_>>(),
            "player": self.player,
            "until": self.until,
            "ongoing": self.ongoing(turn),
        })
    }
}

impl Game {
    /// The civilizations a swan can strike: every living major with a city.
    fn swan_majors(&self) -> Vec<usize> {
        self.players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .filter(|pid| self.cities.values().any(|city| city.owner == *pid))
            .collect()
    }

    /// A civilization's cities, the most populous first.
    fn swan_cities(&self, pid: usize) -> Vec<u32> {
        let mut cities = self.player_city_ids(pid);
        cities.sort_by_key_shared(|cid| (-self.cities[cid].pop, *cid));
        cities
    }

    /// The city a civilization is governed from: its capital, or its largest
    /// city where its capital is in other hands.
    fn swan_seat_of(&self, pid: usize) -> Option<u32> {
        let cities = self.swan_cities(pid);
        cities
            .iter()
            .copied()
            .find(|cid| self.cities[cid].is_capital && self.cities[cid].original_owner == pid)
            .or_else(|| cities.first().copied())
    }

    /// Whether this world has anything for a kind of swan to strike.
    pub fn black_swan_possible(&self, kind: BlackSwanKind) -> Result<(), String> {
        let majors = self.swan_majors();
        let refused = match kind {
            BlackSwanKind::NuclearWar => (majors.len() < 2)
                .then_some("a nuclear war takes two civilizations with a city each"),
            BlackSwanKind::Pandemic => (!self.cities.values().any(|city| city.pop > 1))
                .then_some("no city has people to lose"),
            BlackSwanKind::RadiationLeak => self
                .cities
                .is_empty()
                .then_some("there is no city to leak from"),
            BlackSwanKind::LeaderCollapse => majors
                .is_empty()
                .then_some("no civilization has a leader to fall"),
            BlackSwanKind::Microstates => (!majors
                .iter()
                .any(|pid| self.player_city_ids(*pid).len() > 1))
            .then_some("no civilization has a second city to break away"),
        };
        refused.map_or(Ok(()), |why| Err(why.to_string()))
    }

    /// Strike a black swan on this world as it stands: `kind`, or one drawn
    /// from those this world has something for when none is named. `seed`
    /// decides which that is and everything about where it lands, so one
    /// seed strikes two copies of a world alike.
    ///
    /// The world's own random stream moves on by one draw: what follows a
    /// black swan is not the game that was going to be played, and two
    /// worlds that differ only by one are told apart by it.
    pub fn black_swan(
        &mut self,
        kind: Option<BlackSwanKind>,
        seed: u64,
    ) -> Result<BlackSwan, String> {
        if self.is_finished() {
            return Err("the game is over".to_string());
        }
        let mut rng = Rng::new(seed);
        let kind = match kind {
            Some(kind) => {
                self.black_swan_possible(kind)?;
                kind
            }
            None => {
                let possible: Vec<BlackSwanKind> = BlackSwanKind::ALL
                    .into_iter()
                    .filter(|kind| self.black_swan_possible(*kind).is_ok())
                    .collect();
                if possible.is_empty() {
                    return Err("this world has nothing for a black swan to strike".to_string());
                }
                possible[rng.below(possible.len())]
            }
        };
        let mut swan = BlackSwan {
            turn: self.turn,
            kind,
            seed,
            text: String::new(),
            sites: Vec::new(),
            player: None,
            until: self.turn,
            armed: BTreeMap::new(),
        };
        match kind {
            BlackSwanKind::NuclearWar => self.swan_nuclear_war(&mut swan, &mut rng),
            BlackSwanKind::Pandemic => self.swan_pandemic(&mut swan),
            BlackSwanKind::RadiationLeak => self.swan_radiation_leak(&mut swan, &mut rng),
            BlackSwanKind::LeaderCollapse => self.swan_leader_collapse(&mut swan),
            BlackSwanKind::Microstates => self.swan_microstates(&mut swan, &mut rng),
        }
        let _ = self.rng.next_u64();
        // Everybody hears of it, whatever the fog says.
        let told: Vec<usize> = self
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        let news = format!("Black swan: {}", swan.text);
        for pid in told {
            self.swan_note(pid, &news, swan.sites.first().copied());
        }
        self.black_swans.push(swan.clone());
        if self.black_swans.len() > SWANS_KEPT {
            let excess = self.black_swans.len() - SWANS_KEPT;
            self.black_swans.drain(..excess);
        }
        // The world was written outside any action, so what is kept from one
        // action to the next was derived from a world that is gone: the
        // production menus and the monopoly census a successful `apply`
        // clears, and the yields `board_written` drops.
        self.query_memo.producible.borrow_mut().clear();
        self.query_memo.monopoly_context.borrow_mut().take();
        self.board_written();
        self.sync_war_log();
        Ok(swan)
    }

    /// News of a swan, as it is and not as something its reader did: the log
    /// names the civilization an entry belongs to in front of one that does
    /// not open with a civilization's name (`attributed`).
    fn swan_note(&mut self, pid: usize, text: &str, at: Option<Pos>) {
        let before = self.chronicle.events.len();
        self.note_important(pid, "World", text, at);
        if self.chronicle.events.len() != before {
            if let Some(event) = self.chronicle.events.last_mut() {
                event.text = text.to_string();
            }
        }
    }

    /// Two civilizations are at war from now on, whatever stood between
    /// them: no declaration was made, so none of a declaration's refusals
    /// (a friendship, a treaty, never having met) is asked.
    fn swan_war(&mut self, first: usize, second: usize, conflict: u32) -> bool {
        if first == second || self.same_team(first, second) || self.is_at_war(first, second) {
            return false;
        }
        self.record_contact(first, second);
        let opened = self.at_war.insert(pair(first, second));
        self.end_bilateral_relations_for_war(first, second);
        if opened {
            self.open_war_front(
                first,
                second,
                WarDeclaration {
                    conflict,
                    declarer: first,
                    target: second,
                    declared_front: true,
                    casus_belli: None,
                    joint_war_until: None,
                },
            );
        }
        opened
    }

    /// A device goes off on `target` with nothing having carried it there:
    /// the blast of `do_wmd_strike` (fallout over the radius, the damage a
    /// ring takes, half of a city's people and its walls), written to the
    /// same ledger and told the same way, without the stockpile, the
    /// platform and the peace that a launched device answers to.
    ///
    /// `attacker` is who it is laid to. A civilization's is its own act of
    /// war, with the grievances and the war moments of one; the Free Cities
    /// seat's has no war to be part of.
    fn swan_detonate(
        &mut self,
        attacker: usize,
        from: Pos,
        target: Pos,
        thermonuclear: bool,
    ) -> NuclearStrike {
        let spec = self.rules.wmds[if thermonuclear {
            "thermonuclear_device"
        } else {
            "nuclear_device"
        }]
        .clone();
        let major = !self.players[attacker].is_barbarian && !self.players[attacker].is_minor;
        let blast: Vec<Pos> = self.wdisk(target, spec.blast_radius);
        let fallout_until = self.turn.saturating_add(spec.fallout_duration);
        let struck_cities: Vec<(usize, String)> = blast
            .iter()
            .filter_map(|position| self.city_at(*position))
            .filter_map(|cid| self.cities.get(&cid))
            .filter(|city| city.owner != attacker)
            .map(|city| (city.owner, city.name.clone()))
            .collect();
        let exposed: Vec<u32> = blast
            .iter()
            .flat_map(|position| self.units_at(*position))
            .collect();
        let mut victims: BTreeSet<usize> = exposed
            .iter()
            .filter_map(|uid| self.units.get(uid))
            .map(|unit| unit.owner)
            .collect();
        for position in blast.iter().copied() {
            if let Some(tile) = self.map.tiles.get_mut(&position) {
                tile.fallout_until = tile.fallout_until.max(fallout_until);
            }
            let ring = self.wdist(position, target);
            let unit_damage = match ring {
                0 => 100,
                1 => 80,
                _ => 60,
            };
            victims.extend(
                self.map
                    .get(position)
                    .and_then(|tile| tile.owner_city)
                    .and_then(|cid| self.cities.get(&cid))
                    .map(|city| city.owner),
            );
            self.damage_tile_area(position, unit_damage, major.then_some(attacker));
            if let Some(struck) = self.city_at(position) {
                let city = self.cities.get_mut(&struck).unwrap();
                city.pop = (city.pop - (city.pop / 2)).max(1);
                city.wall_hp = if ring == 0 { 0 } else { city.wall_hp / 2 };
            }
        }
        victims.retain(|owner| *owner != attacker && !self.players[*owner].is_barbarian);
        let mut struck_names: Vec<String> = Vec::new();
        for (owner, name) in struck_cities {
            struck_names.push(name.clone());
            if major {
                self.record_war_moment(attacker, owner, "nuclear_strike", Some(name), Some(target));
            }
        }
        if major {
            for owner in &victims {
                self.add_grievances(*owner, attacker, 150.0);
            }
        }
        let units_destroyed = exposed
            .into_iter()
            .filter(|uid| !self.units.contains_key(uid))
            .count() as u32;
        let strike = NuclearStrike {
            id: self.allocate_conflict_id(),
            turn: self.turn,
            attacker,
            target,
            thermonuclear,
            platform: "black_swan".to_string(),
            launched_from: from,
            blast_radius: spec.blast_radius,
            fallout_until,
            victims,
            cities: struck_names.clone(),
            units_destroyed,
        };
        self.announce_nuclear_strike(&strike, &struck_names);
        self.chronicle.nuclear_strikes_mut().push(strike.clone());
        // The ledger's own bound (`do_wmd_strike`).
        const STRIKES_KEPT: usize = 32;
        if self.chronicle.nuclear_strikes.len() > STRIKES_KEPT {
            let excess = self.chronicle.nuclear_strikes.len() - STRIKES_KEPT;
            self.chronicle.nuclear_strikes_mut().drain(..excess);
        }
        strike
    }

    /// Instant nuclear war: every civilization is at war with every other,
    /// and each fires on the largest city, not yet struck, of each of its
    /// `WAR_VOLLEYS` strongest rivals. Who fires first is drawn.
    fn swan_nuclear_war(&mut self, swan: &mut BlackSwan, rng: &mut Rng) {
        let majors = self.swan_majors();
        let conflict = self.allocate_conflict_id();
        for (at, first) in majors.iter().enumerate() {
            for second in &majors[at + 1..] {
                self.swan_war(*first, *second, conflict);
            }
        }
        let mut order = majors.clone();
        for at in (1..order.len()).rev() {
            order.swap(at, rng.below(at + 1));
        }
        let scores: BTreeMap<usize, i64> =
            majors.iter().map(|pid| (*pid, self.score(*pid))).collect();
        let mut struck: BTreeSet<u32> = BTreeSet::new();
        let mut devices = 0;
        for attacker in order {
            let Some(from) = self.swan_seat_of(attacker).map(|cid| self.cities[&cid].pos) else {
                continue;
            };
            let mut rivals: Vec<usize> = majors
                .iter()
                .copied()
                .filter(|rival| *rival != attacker && !self.same_team(attacker, *rival))
                .collect();
            rivals.sort_by_key_shared(|rival| (-scores[rival], *rival));
            let thermonuclear = self.players[attacker]
                .techs
                .contains(&Name::new("nuclear_fusion"));
            for rival in rivals.into_iter().take(WAR_VOLLEYS) {
                let cities = self.swan_cities(rival);
                let Some(target) = cities
                    .iter()
                    .copied()
                    .find(|cid| !struck.contains(cid))
                    .or_else(|| cities.first().copied())
                else {
                    continue;
                };
                struck.insert(target);
                let position = self.cities[&target].pos;
                self.swan_detonate(attacker, from, position, thermonuclear);
                devices += 1;
                if !swan.sites.contains(&position) {
                    swan.sites.push(position);
                }
            }
        }
        swan.text = format!(
            "Instant nuclear war. All {} civilizations are at war, and {} nuclear devices fell on {} cities",
            majors.len(),
            devices,
            struck.len()
        );
    }

    /// Pandemic: every city in the world loses half its people, as a city
    /// under a detonation does. A city of one keeps its one.
    fn swan_pandemic(&mut self, swan: &mut BlackSwan) {
        let cities: Vec<u32> = self.cities.keys().copied().collect();
        let (mut before, mut lost, mut struck) = (0i64, 0i64, 0usize);
        let mut largest: Option<(i32, Pos)> = None;
        for cid in cities {
            let city = self.cities.get_mut(&cid).unwrap();
            let dead = city.pop / 2;
            before += i64::from(city.pop);
            if dead == 0 {
                continue;
            }
            if largest.is_none_or(|(pop, _)| city.pop > pop) {
                largest = Some((city.pop, city.pos));
            }
            city.pop -= dead;
            lost += i64::from(dead);
            struck += 1;
        }
        swan.sites.extend(largest.map(|(_, position)| position));
        swan.text = format!(
            "Pandemic. {struck} cities lost half their people: {lost} of the world's {before} citizens"
        );
    }

    /// The ground a leak has reached by its `ring`th turn takes fallout.
    fn swan_contaminate(&mut self, site: Pos, ring: i32) {
        let until = self.turn.saturating_add(LEAK_FALLOUT);
        for position in self.wdisk(site, ring) {
            if let Some(tile) = self.map.tiles.get_mut(&position) {
                tile.fallout_until = tile.fallout_until.max(until);
            }
        }
    }

    /// Radiation leak: `LEAK_SITES` cities around the world, drawn apart from
    /// one another, start leaking. Each site takes a reactor accident's
    /// damage and loses a citizen, the ground a tile around it is
    /// contaminated at once, and the contamination goes on a ring a turn for
    /// `LEAK_TURNS` turns (`process_black_swans`).
    fn swan_radiation_leak(&mut self, swan: &mut BlackSwan, rng: &mut Rng) {
        let mut cities: Vec<u32> = self.cities.keys().copied().collect();
        for at in (1..cities.len()).rev() {
            cities.swap(at, rng.below(at + 1));
        }
        // Apart where the world has room, and wherever it has cities where
        // it does not.
        let mut sites: Vec<u32> = Vec::new();
        for spacing in [LEAK_SPACING, 0] {
            for cid in &cities {
                let position = self.cities[cid].pos;
                let apart = sites
                    .iter()
                    .all(|taken| self.wdist(self.cities[taken].pos, position) >= spacing);
                if sites.len() < LEAK_SITES && !sites.contains(cid) && apart {
                    sites.push(*cid);
                }
            }
        }
        let mut names: Vec<String> = Vec::new();
        for cid in sites {
            let (position, name) = (self.cities[&cid].pos, self.cities[&cid].name.clone());
            self.damage_disaster_tile(position, LEAK_SITE_DAMAGE);
            let city = self.cities.get_mut(&cid).unwrap();
            city.pop = (city.pop - 1).max(1);
            self.swan_contaminate(position, 1);
            swan.sites.push(position);
            names.push(name);
        }
        swan.until = self.turn + LEAK_TURNS;
        swan.text = format!(
            "Radiation leak. {} {} leaking, and the fallout spreads a ring a turn for {} turns",
            names.join(", "),
            if names.len() == 1 { "is" } else { "are" },
            LEAK_TURNS
        );
    }

    /// Leader collapse: the leader of the civilization in the lead falls.
    /// The government it was seated under waits out `COLLAPSE_ANARCHY` turns
    /// of Anarchy, and no city of it keeps more than `COLLAPSE_LOYALTY`
    /// Loyalty (twice that in the capital): which of them hold and which
    /// declare independence is the Loyalty rules' to say from there.
    fn swan_leader_collapse(&mut self, swan: &mut BlackSwan) {
        let mut majors = self.swan_majors();
        majors.sort_by_key_shared(|pid| (-self.score(*pid), *pid));
        let Some(leader) = majors.first().copied() else {
            return;
        };
        let player = &mut self.players[leader];
        if let Some(government) = player.government.take() {
            player.pending_government = Some(government);
        }
        player.anarchy_turns = player.anarchy_turns.max(COLLAPSE_ANARCHY);
        let seat = self.swan_seat_of(leader);
        let cities = self.player_city_ids(leader);
        for cid in &cities {
            let most = if Some(*cid) == seat {
                COLLAPSE_LOYALTY * 2.0
            } else {
                COLLAPSE_LOYALTY
            };
            let city = self.cities.get_mut(cid).unwrap();
            city.loyalty = city.loyalty.min(most);
        }
        swan.player = Some(leader);
        swan.sites.extend(seat.map(|cid| self.cities[&cid].pos));
        swan.until = self.turn + COLLAPSE_ANARCHY;
        let name = self.civ_name(leader);
        swan.text = format!(
            "Leader collapse. {name}, in the lead, has lost its leader: {COLLAPSE_ANARCHY} turns of Anarchy, and the Loyalty of its {} {} has collapsed",
            cities.len(),
            if cities.len() == 1 { "city" } else { "cities" },
        );
    }

    /// Microstates: a civilization with more than one city, drawn among
    /// those there are, breaks up. Every city but the one it is governed
    /// from declares independence into the Free Cities seat, which the whole
    /// world is at war with, stands up `MICROSTATE_DEFENDERS` units and
    /// holds `MICROSTATE_DEVICES` nuclear devices, to be fired by
    /// `process_black_swans`.
    fn swan_microstates(&mut self, swan: &mut BlackSwan, rng: &mut Rng) {
        let broken: Vec<usize> = self
            .swan_majors()
            .into_iter()
            .filter(|pid| self.player_city_ids(*pid).len() > 1)
            .collect();
        if broken.is_empty() {
            return;
        }
        let pid = broken[rng.below(broken.len())];
        let seat = self.swan_seat_of(pid);
        let name = self.civ_name(pid);
        let kept = seat.map_or(String::new(), |cid| self.cities[&cid].name.clone());
        let leaving: Vec<u32> = self
            .swan_cities(pid)
            .into_iter()
            .filter(|cid| Some(*cid) != seat)
            .collect();
        for cid in &leaving {
            swan.sites.push(self.cities[cid].pos);
            self.revolt_to_free_cities(*cid, MICROSTATE_DEFENDERS);
            swan.armed.insert(*cid, MICROSTATE_DEVICES);
        }
        swan.player = Some(pid);
        swan.until = self.turn + MICROSTATE_ARMED_TURNS;
        swan.text = format!(
            "Microstates. {name} has broken up: {} of its cities are hostile Free Cities now, each with an army and a nuclear device, and {name} keeps only {kept}",
            leaving.len()
        );
    }

    /// One world turn of the swans that are not over: a leak spreads its
    /// next ring, and each microstate that still holds a device may fire it.
    /// A world no swan has struck is not touched, and its random stream is
    /// not drawn from.
    pub(super) fn process_black_swans(&mut self) {
        let before = self.turn.saturating_sub(1);
        if !self.black_swans.iter().any(|swan| swan.ongoing(before)) {
            return;
        }
        let turn = self.turn;
        for at in 0..self.black_swans.len() {
            let (kind, struck, until) = {
                let swan = &self.black_swans[at];
                (swan.kind, swan.turn, swan.until)
            };
            if turn <= struck || turn > until {
                continue;
            }
            match kind {
                BlackSwanKind::RadiationLeak => {
                    let ring = 1 + (turn - struck) as i32;
                    for site in self.black_swans[at].sites.clone() {
                        self.swan_contaminate(site, ring);
                    }
                    self.board_written();
                }
                BlackSwanKind::Microstates => self.swan_microstates_fire(at),
                _ => {}
            }
        }
    }

    /// Each city that broke away, is still a Free City and still holds a
    /// device rolls for firing it this turn. It fires on the most populous
    /// city of a living civilization within a device's range.
    fn swan_microstates_fire(&mut self, at: usize) {
        let reach = self.rules.wmds["nuclear_device"].icbm_strike_range;
        let armed: Vec<u32> = self.black_swans[at]
            .armed
            .iter()
            .filter(|(_, devices)| **devices > 0)
            .map(|(cid, _)| *cid)
            .collect();
        for cid in armed {
            let Some((owner, from)) = self.cities.get(&cid).map(|city| (city.owner, city.pos))
            else {
                self.black_swans[at].armed.remove(&cid);
                continue;
            };
            // A microstate that was taken, or came home, has no arsenal.
            if !self.players[owner].is_free_city {
                self.black_swans[at].armed.remove(&cid);
                continue;
            }
            if !self.rng.chance(MICROSTATE_LAUNCH_ODDS) {
                continue;
            }
            let target = self
                .cities
                .values()
                .filter(|city| {
                    let holder = &self.players[city.owner];
                    holder.alive
                        && !holder.is_minor
                        && !holder.is_barbarian
                        && self.wdist(from, city.pos) <= reach
                })
                .max_by_key(|city| (city.pop, std::cmp::Reverse(city.id)))
                .map(|city| city.pos);
            let Some(target) = target else {
                continue;
            };
            self.swan_detonate(owner, from, target, false);
            if let Some(devices) = self.black_swans[at].armed.get_mut(&cid) {
                *devices -= 1;
            }
        }
    }
}
