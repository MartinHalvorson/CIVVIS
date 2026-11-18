//! `shooters-man-the-capital`: while a war party or a siege engine is in
//! sight of our original capital, our land shooters come home, stand in and
//! beside its City Center, and a siege gun in their reach is their first
//! target.
//!
//! ## Why (live Deity, 6 players, Small Continents, Online; 2026-11-18)
//!
//! Eight of the day's 29 games were retired because the original capital
//! fell, one to eight turns after a rival's declaration. Read from their
//! `events.jsonl` (`~/civvis-score-tools/deity-18/capital-guard/`):
//!
//! - **A siege gun took the walls and we let it stand.** In six of the eight
//!   a Trebuchet, Bombard or Catapult stood two or three tiles from the City
//!   Center for one to four turns before the walls were gone. We owned four
//!   to eight land shooters in each of those wars and they fired on a gun
//!   six times in all, and not once in three of the six.
//! - **The shooters stood elsewhere.** On the turn the gun first stood within
//!   three tiles, one of six, none of six, one of six, one of eight, two of
//!   four and two of four of our shooters stood in or beside the capital.
//!   - `civvis-20261118T093554Z` (China): Hungary declared at 0.81 times our
//!     power. One Trebuchet and two Men-at-Arms at 44 and 48 health took
//!     Beijing on turn 73. Seven or eight Archers of ours stood within six
//!     tiles all eight turns; the Trebuchet stood two or three tiles from the
//!     centre for four of them and took no shot (the logged Archer shots on a
//!     Trebuchet did 14-26, so five or six kill it). The Defend force stood
//!     at Longxi, five tiles away, until Beijing's walls read 21 of 100.
//!   - `civvis-20261118T083517Z` (America): all six shooters, three of them
//!     Crossbowmen, stood five to ten tiles from Washington from Persia's
//!     declaration (turn 85) to the fall (turn 87). Persia's Bombard had been
//!     in sight six tiles out on turns 83 and 84, at peace.
//!   - `civvis-20261118T090535Z` (Kongo): England's Trebuchet stood two tiles
//!     from Mbanza Kongo for three turns and took two Archer shots; one of
//!     four to six shooters stood within one tile.
//!   - `civvis-20261118T104821Z` (Brazil), the opening rush: three Warriors
//!     and a Slinger took an unwalled Rio de Janeiro on turn 30, four turns
//!     after Norway declared. The City Center held nobody on turns 24-28;
//!     our two Archers stood four and five tiles out with the new cities.
//! - **The party was in sight before it declared.** Over the day's 31 game
//!   logs, "two land soldiers of one major that is neither friend nor ally
//!   within six tiles of the capital, or its engine within eight" read at
//!   peace on 266 of the 4,228 turns we held the capital (6%). It read in
//!   the three turns before 14 of the 36 declarations made on us while we
//!   held the capital, and before seven of the eight that took it (not
//!   Brazil's: one Warrior was seen, one turn before).
//!
//! The armed readers did not answer this: `garrison-before-the-surprise`
//! walks one soldier into a city that holds none, at 1.5 times our power;
//! `capital-archers-before-the-surprise-2` keeps one shooter on the centre at
//! 2.5 times, and spends only while the capital has no walls;
//! `siege-at-the-walls-dies-first` and `every-shooter-at-the-walls` price a
//! gun first for whichever shooter already stands in reach of it; and the
//! Anvil rings the city the board ranks first and its one-blow rewrite sends
//! a posted soldier back to a safer tile.
//!
//! ## What the gene does
//!
//! **The watch.** At the start of our unit turn the seat reads its original
//! capital, while it holds it, on the board it sees this turn:
//!
//! - an *engine* within [`MAN_ENGINE_TILES`]: a siege gun, a Battering Ram or
//!   a Siege Tower;
//! - a *party* within [`MAN_PARTY_TILES`]: land soldiers that are not recon.
//!
//! A power at war with us (a major or a city-state) is watched from its first
//! soldier or engine. A major at peace is watched from an engine or from
//! [`MAN_PEACE_PARTY`] soldiers, unless it is our declared friend or ally or
//! `four-faces-bars-the-surprise` rules its surprise out.
//!
//! **The call.** With an engine in sight every free shooter within
//! [`MAN_CALL_RANGE`] tiles of the capital is called. Without one, the
//! largest watched party's size plus one, and at least [`MAN_GUARD_MIN`].
//! The nearest are called first. A free shooter is a land unit with a ranged
//! attack that reaches [`MAN_SHOOTER_REACH`] tiles and is no siege gun, not
//! embarked, not a civilian's guard and not standing on another City Center
//! of ours. A member of a siege of ours is free only while a power at war
//! with us is in sight of the capital: the siege of their city waits for
//! ours.
//!
//! **The orders**, before the battle is planned, nearest shooter first:
//!
//! 1. A siege gun at war with us that it can shoot from where it stands is
//!    shot: the weakest first.
//! 2. Otherwise, when a dry tile within [`MAN_STAND_TILES`] of the City
//!    Center that it can reach this turn has such a gun in reach, it goes
//!    there and fires if it still can. The shooter on the centre keeps the
//!    centre.
//! 3. Otherwise it goes home: onto the City Center when no soldier of ours
//!    holds it, else beside it, as far as this turn's movement takes it over
//!    dry land. There it stays. At war it takes the best shot it has.
//!
//! A called shooter is held for the turn: the battle plan, the Anvil and the
//! per-unit ladder do not order it again.
//!
//! A civilian's guard that stands in or beside the City Center is not called
//! and not held. At war it fires from its tile by the same rule, the gun
//! first: on Beijing's boards the Archer on the centre was a waiting
//! Settler's guard, the one shooter with the Trebuchet in reach.
//!
//! **What it costs, by design.** A shooter beside the centre can be struck;
//! one on it cannot. The gene accepts that for the gun's death: the capital's
//! fall retires the game. At peace the called shooters are off their other
//! work on the turns the watch reads.
//!
//! Stateless: every turn is read from that turn's board, so the live
//! rebuild's unit ids are never remembered. Off, nothing here runs and the
//! controller is byte-identical. Grep the journal for
//! `shooters-man-the-capital:`.

use super::*;

/// How near the original capital an engine is watched: the capital watch's
/// own eight tiles, three turns of a siege gun's march.
pub(crate) const MAN_ENGINE_TILES: i32 = 8;

/// How near a party's soldiers are counted.
pub(crate) const MAN_PARTY_TILES: i32 = 6;

/// Soldiers of one major at peace that make a party. One is a passer-by: the
/// 2026-11-18 census read a single soldier within four tiles as often as two
/// within six, before fewer declarations.
pub(crate) const MAN_PEACE_PARTY: usize = 2;

/// The farthest a shooter is called from: four turns' walk.
pub(crate) const MAN_CALL_RANGE: i32 = 12;

/// The shortest reach that mans the capital: an Archer's. From the centre it
/// reaches a gun shelling the centre from two tiles; a Slinger's does not.
pub(crate) const MAN_SHOOTER_REACH: i32 = 2;

/// The fewest shooters a watched party calls.
pub(crate) const MAN_GUARD_MIN: usize = 2;

/// How far from the City Center a shooter takes a stand to reach a siege
/// gun. The ring beside the centre is six tiles, some of them water and some
/// held by our own soldiers: on Beijing's turn-70 board seven shooters were
/// called and two found a tile beside the centre.
pub(crate) const MAN_STAND_TILES: i32 = 2;

/// What the watch reads at the original capital this turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ManReading {
    /// The original capital's City Center.
    pub(crate) home: Pos,
    /// Siege guns at war with us in sight: every shooter's first target.
    pub(crate) guns: Vec<u32>,
    /// A power at war with us is in sight.
    pub(crate) war: bool,
    /// Engines in sight, of the watched powers.
    pub(crate) engines: usize,
    /// The largest watched party.
    pub(crate) soldiers: usize,
    /// The power that party or those engines belong to.
    pub(crate) owner: usize,
    /// Shooters called.
    pub(crate) wanted: usize,
}

impl AdvancedAi {
    /// A siege gun, a Battering Ram or a Siege Tower.
    fn man_engine(spec: &crate::rules::UnitSpec, kind: &str) -> bool {
        !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            && ((spec.class == "military" && spec.siege)
                || matches!(kind, "battering_ram" | "siege_tower"))
    }

    /// A land soldier that can fight and is not recon.
    fn man_soldier(g: &Game, unit: &crate::game::Unit) -> bool {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            && spec.promotion_class != "recon"
            && (spec.is_melee_capable() || spec.has_ranged_attack())
            && !g.is_embarked(unit)
    }

    /// Whether `other`'s units near the capital are watched: a major or a
    /// city-state at war with us, or a met major at peace that could open a
    /// war on us (no declared friend, no ally, no Four Faces bar).
    fn man_watched(&self, g: &Game, pid: usize, other: usize) -> bool {
        let Some(player) = g.players.get(other) else {
            return false;
        };
        if other == pid || !player.alive || player.is_barbarian || player.is_free_city {
            return false;
        }
        if g.is_at_war(pid, other) {
            return true;
        }
        !player.is_minor
            && g.has_met(pid, other)
            && !g.are_friends(pid, other)
            && !g.are_allied(pid, other)
            && !self.four_faces_spares(g, pid, other)
    }

    /// The watch: see the module. `None` with the gene off, without the
    /// original capital, or with nothing watched in sight.
    pub(super) fn shooters_man_reading(&self, g: &Game, pid: usize) -> Option<ManReading> {
        if !self.shooters_man_the_capital || self.base.minor || self.base.barb {
            return None;
        }
        let capital = Self::capital_watch_city(g, pid)?;
        let home = g.cities[&capital].pos;
        let visible = g.player_vision_frame(pid);
        // By power: its engines, its soldiers, its siege guns at war with us.
        let mut seen: BTreeMap<usize, (usize, usize, Vec<u32>)> = BTreeMap::new();
        for unit in g.units.values() {
            if unit.owner == pid || unit.hp <= 0 {
                continue;
            }
            let distance = g.wdist(unit.pos, home);
            if distance > MAN_ENGINE_TILES {
                continue;
            }
            let spec = &g.rules.units[unit.kind];
            let engine = Self::man_engine(spec, unit.kind.as_str());
            let soldier = !engine && distance <= MAN_PARTY_TILES && Self::man_soldier(g, unit);
            if !(engine || soldier)
                || !self.man_watched(g, pid, unit.owner)
                || !g.sees(&visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
            {
                continue;
            }
            let entry = seen.entry(unit.owner).or_default();
            if engine {
                entry.0 += 1;
                if spec.siege && g.is_at_war(pid, unit.owner) {
                    entry.2.push(unit.id);
                }
            } else {
                entry.1 += 1;
            }
        }
        // A power at war is watched from its first unit; one at peace from an
        // engine or a party.
        seen.retain(|other, (engines, soldiers, _)| {
            *engines > 0 || *soldiers >= MAN_PEACE_PARTY || g.is_at_war(pid, *other)
        });
        let (owner, (_, soldiers, _)) = seen
            .iter()
            .max_by_key(|(other, (engines, soldiers, _))| {
                (
                    g.is_at_war(pid, **other),
                    *engines + *soldiers,
                    std::cmp::Reverse(**other),
                )
            })
            .map(|(other, tally)| (*other, tally.clone()))?;
        let engines: usize = seen.values().map(|(engines, _, _)| *engines).sum();
        let mut guns: Vec<u32> = seen
            .values()
            .flat_map(|(_, _, guns)| guns.iter().copied())
            .collect();
        guns.sort_unstable();
        let wanted = if engines > 0 {
            usize::MAX
        } else {
            (soldiers + 1).max(MAN_GUARD_MIN)
        };
        Some(ManReading {
            home,
            guns,
            war: seen.keys().any(|other| g.is_at_war(pid, *other)),
            engines,
            soldiers,
            owner,
            wanted,
        })
    }

    /// A land shooter of ours that reaches [`MAN_SHOOTER_REACH`] and is no
    /// siege gun.
    fn man_shooter(g: &Game, pid: usize, uid: u32) -> bool {
        g.units.get(&uid).is_some_and(|unit| {
            let spec = &g.rules.units[unit.kind];
            unit.owner == pid
                && unit.hp > 0
                && spec.class == "military"
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && spec.has_ranged_attack()
                && !spec.siege
                && !g.is_embarked(unit)
                && g.unit_attack_range(uid) >= MAN_SHOOTER_REACH
        })
    }

    /// Whether `uid` guards a civilian: linked to one, bound to a Settler or
    /// reserved for a Builder.
    fn man_guard(&self, g: &Game, uid: u32) -> bool {
        g.units[&uid].linked_to.is_some()
            || self.guard_is_bound_to_any_settler(uid)
            || self.guard_is_reserved_for_civilian(uid)
    }

    /// A shooter free to man the capital: see the module. A member of a
    /// siege of ours is free once a power at war with us is in sight of the
    /// capital (`war`).
    fn man_free_shooter(&self, g: &Game, pid: usize, uid: u32, home: Pos, war: bool) -> bool {
        if !Self::man_shooter(g, pid, uid) || self.man_guard(g, uid) {
            return false;
        }
        let unit = &g.units[&uid];
        g.wdist(unit.pos, home) <= MAN_CALL_RANGE
            && (unit.pos == home
                || !g
                    .city_at(unit.pos)
                    .is_some_and(|cid| g.cities[&cid].owner == pid))
            && (war
                || (!self.active_siege_member(g, pid, uid)
                    && !self.staging_siege_member(g, pid, uid)))
    }

    /// A civilian's guard standing in or beside the City Center: it keeps its
    /// charge and its tile, and fires from it.
    fn man_standing_guards(&self, g: &Game, pid: usize, home: Pos) -> Vec<u32> {
        let mut guards: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                Self::man_shooter(g, pid, *uid)
                    && self.man_guard(g, *uid)
                    && g.wdist(g.units[uid].pos, home) <= 1
            })
            .collect();
        guards.sort_unstable();
        guards
    }

    /// The shooters the reading calls, the nearest the capital first (then
    /// the longer reach, the stronger shot, the lower id).
    pub(super) fn shooters_man_called(
        &self,
        g: &Game,
        pid: usize,
        reading: &ManReading,
    ) -> Vec<u32> {
        let mut free: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| self.man_free_shooter(g, pid, *uid, reading.home, reading.war))
            .collect();
        free.sort_by_key(|uid| {
            let unit = &g.units[uid];
            let spec = &g.rules.units[unit.kind];
            (
                g.wdist(unit.pos, reading.home),
                std::cmp::Reverse((
                    g.unit_attack_range(*uid),
                    (spec.ranged_attack_strength() * 100.0) as i64,
                )),
                *uid,
            )
        });
        free.truncate(reading.wanted);
        free
    }

    /// The watched siege gun `uid` can shoot from where it stands: the
    /// weakest, then the nearest the capital.
    fn man_gun_in_reach(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        reading: &ManReading,
    ) -> Option<u32> {
        let frame = g.player_vision_frame(pid);
        let viewers = g.visibility_viewers(pid);
        reading
            .guns
            .iter()
            .copied()
            .filter(|gun| {
                g.units.get(gun).is_some_and(|target| {
                    g.ranged_order_is_legal(pid, uid, target.pos, frame.as_ref(), &viewers)
                })
            })
            .min_by_key(|gun| {
                let target = &g.units[gun];
                (target.hp, g.wdist(target.pos, reading.home), *gun)
            })
    }

    /// A dry tile within [`MAN_STAND_TILES`] of the City Center that `uid`
    /// can reach this turn and that has a watched siege gun in reach: the
    /// nearest the shooter (it is the likeliest to leave it a shot), then the
    /// one with the fewest hostile soldiers beside it, then the nearest the
    /// centre.
    fn man_firing_stand(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        reading: &ManReading,
    ) -> Option<Pos> {
        let unit = g.units.get(&uid)?;
        if unit.moves_left <= 0.0 || reading.guns.is_empty() {
            return None;
        }
        let reach = g.unit_attack_range(uid);
        let from = unit.pos;
        let beside_hostiles = |stand: Pos| {
            g.nbrs(stand)
                .into_iter()
                .filter(|tile| {
                    g.unit_ids_at(*tile).iter().any(|id| {
                        g.units.get(id).is_some_and(|other| {
                            other.owner != pid
                                && g.is_at_war(pid, other.owner)
                                && Self::man_soldier(g, other)
                        })
                    })
                })
                .count()
        };
        g.reachable(uid)
            .into_iter()
            .filter(|stand| {
                g.wdist(*stand, reading.home) <= MAN_STAND_TILES && Self::man_dry(g, *stand)
            })
            .filter(|stand| {
                reading.guns.iter().any(|gun| {
                    g.units.get(gun).is_some_and(|target| {
                        g.wdist(*stand, target.pos) <= reach
                            && g.unit_has_line_of_sight_from(uid, *stand, target.pos)
                    })
                })
            })
            .min_by_key(|stand| {
                (
                    g.wdist(from, *stand),
                    beside_hostiles(*stand),
                    g.wdist(*stand, reading.home),
                    *stand,
                )
            })
    }

    /// Dry land: a shooter that embarks can neither fire nor fortify.
    fn man_dry(g: &Game, tile: Pos) -> bool {
        g.map
            .get(tile)
            .is_some_and(|ground| !g.rules.is_water(ground))
    }

    /// The tile this turn's movement takes `uid` nearest the City Center: the
    /// centre itself only while no soldier of ours holds it.
    fn man_step_home(&self, g: &Game, pid: usize, uid: u32, home: Pos) -> Option<Pos> {
        let unit = g.units.get(&uid)?;
        if unit.moves_left <= 0.0 {
            return None;
        }
        let here = g.wdist(unit.pos, home);
        g.reachable(uid)
            .into_iter()
            .filter(|tile| {
                // Dry land, and another city of ours is that city's to
                // garrison.
                Self::man_dry(g, *tile)
                    && (*tile == home
                        || !g
                            .city_at(*tile)
                            .is_some_and(|cid| g.cities[&cid].owner == pid))
            })
            .map(|tile| (g.wdist(tile, home), g.wdist(unit.pos, tile), tile))
            .filter(|(distance, _, _)| *distance < here)
            .min()
            .map(|(_, _, tile)| tile)
    }

    /// The best shot `uid` has from where it stands: a watched siege gun
    /// first, then a soldier (`centre_keeper_shot`). Fires it. `true` when a
    /// shot was fired.
    fn man_fire(&mut self, g: &mut Game, pid: usize, uid: u32, reading: &ManReading) -> bool {
        if !reading.war {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
            return false;
        }
        let kind = unit.kind;
        let gun = self.man_gun_in_reach(g, pid, uid, reading);
        let target = gun
            .map(|gun| g.units[&gun].pos)
            .or_else(|| self.centre_keeper_shot(g, pid, uid));
        let Some(target) = target else {
            return false;
        };
        let what = gun.map_or("soldier", |gun| g.units[&gun].kind.as_str());
        let what = plain(what);
        if g.apply(pid, &Action::Ranged { unit: uid, target }).is_err() {
            return false;
        }
        self.force_groups_dirty = true;
        think!(self.journal(), Military, Decision,
        "The {} fires on the {} at the capital", plain(kind.as_str()), what;
        "shooters-man-the-capital: {} at {target:?}",
        if gun.is_some() {
            "a siege gun in reach is the first target"
        } else {
            "no siege gun in reach; the best shot from its stand"
        });
        true
    }

    /// One called shooter's orders: see the module.
    fn man_orders(&mut self, g: &mut Game, pid: usize, uid: u32, reading: &ManReading) {
        let Some(unit) = g.units.get(&uid) else {
            return;
        };
        let kind = unit.kind;
        let from = unit.pos;
        let home = reading.home;
        // 1. A gun in reach from where it stands.
        if self.man_gun_in_reach(g, pid, uid, reading).is_some()
            && self.man_fire(g, pid, uid, reading)
        {
            return;
        }
        // 2. A stand near the centre with a gun in reach. The shooter on the
        // centre keeps it: the one tile nothing can strike it on.
        if reading.war && from != home {
            if let Some(stand) = self
                .man_firing_stand(g, pid, uid, reading)
                .filter(|stand| *stand != from)
            {
                if g.apply(
                    pid,
                    &Action::MoveTo {
                        unit: uid,
                        to: stand,
                    },
                )
                .is_ok()
                {
                    self.force_groups_dirty = true;
                    think!(self.journal(), Military, Decision,
                        "The {} takes its stand at the capital", plain(kind.as_str());
                        "shooters-man-the-capital: a siege gun is in reach from {stand:?}, {} \
                         tile(s) from the City Center; {from:?} -> {stand:?}",
                        g.wdist(stand, home));
                    self.man_fire(g, pid, uid, reading);
                    self.man_hold(g, pid, uid);
                    return;
                }
            }
        }
        // 3. Home: on the centre it stays; beside it, it stays unless the
        // centre holds no soldier of ours; otherwise it walks.
        let centre_open = !g.unit_ids_at(home).iter().any(|id| {
            g.units
                .get(id)
                .is_some_and(|other| other.owner == pid && Self::man_soldier(g, other))
        });
        if g.wdist(from, home) > 1 || (from != home && centre_open) {
            let goal = self.man_step_home(g, pid, uid, home);
            let stepped = match goal {
                Some(tile) => g
                    .apply(
                        pid,
                        &Action::MoveTo {
                            unit: uid,
                            to: tile,
                        },
                    )
                    .is_ok(),
                None => false,
            };
            // No tile in this turn's reach is nearer (a lake, a ridge, a
            // column in the way): the pathfinder's step.
            let stepped = stepped
                || (g.units.get(&uid).is_some_and(|unit| unit.moves_left > 0.0)
                    && self.base.step_toward_range(g, pid, uid, home, 1));
            if stepped {
                self.force_groups_dirty = true;
                let to = g.units.get(&uid).map_or(from, |unit| unit.pos);
                think!(self.journal(), Military, Decision,
                    "The {} comes home to the capital", plain(kind.as_str());
                    "shooters-man-the-capital: {} tile(s) out; {from:?} -> {to:?}, bound for {}",
                    g.wdist(from, home),
                    goal.map_or_else(|| "the pathfinder's step".to_string(), |tile| format!("{tile:?}")));
            }
        }
        self.man_fire(g, pid, uid, reading);
        self.man_hold(g, pid, uid);
    }

    /// End a called shooter's turn where it stands.
    fn man_hold(&mut self, g: &mut Game, pid: usize, uid: u32) {
        if g.units.contains_key(&uid) {
            self.base.fortify_or_stop(g, pid, uid);
        }
    }

    /// The pre-pass: read the watch, call the shooters and order each. The
    /// units it returns are held for the turn. Empty with the gene off or
    /// with nothing watched.
    pub(super) fn plan_shooters_man_the_capital(
        &mut self,
        g: &mut Game,
        pid: usize,
    ) -> BTreeSet<u32> {
        let Some(reading) = self.shooters_man_reading(g, pid) else {
            return BTreeSet::new();
        };
        // A civilian's guard in or beside the centre fires from its tile and
        // is not held: it stays its charge's.
        for guard in self.man_standing_guards(g, pid, reading.home) {
            self.man_fire(g, pid, guard, &reading);
        }
        let called = self.shooters_man_called(g, pid, &reading);
        if called.is_empty() {
            return BTreeSet::new();
        }
        let home_now = called
            .iter()
            .filter(|uid| g.wdist(g.units[*uid].pos, reading.home) <= 1)
            .count();
        think!(self.journal(), Military, Decision,
            "Shooters man the capital: {} called, {} in or beside the City Center",
            called.len(), home_now;
            "shooters-man-the-capital: {} engine(s) within {MAN_ENGINE_TILES} tiles and a party \
             of {} within {MAN_PARTY_TILES}, of {} ({}); {} siege gun(s) to shoot first",
            reading.engines, reading.soldiers, g.players[reading.owner].civ,
            if reading.war { "at war with us" } else { "at peace" },
            reading.guns.len());
        for uid in &called {
            self.man_orders(g, pid, *uid, &reading);
        }
        called
            .into_iter()
            .filter(|uid| g.units.contains_key(uid))
            .collect()
    }
}

#[cfg(test)]
mod tests;
