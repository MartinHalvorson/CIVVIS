//! One seat's own price, in Gold now, for every item it can trade, read from
//! its own board at this moment.
//!
//! ★ THE UNIT IS GOLD IN HAND THIS TURN. Lump Gold is worth its face value;
//! everything that pays over time (Gold per turn, a luxury's Amenities, a
//! Great Work's yields) is the sum of what it pays each turn, discounted by
//! [`DISCOUNT_PER_TURN_STANDARD`] scaled to the game speed, over the turns it
//! lasts. A timed deal item lasts [`DEAL_TURNS`].
//!
//! ★ GIVING AND RECEIVING ARE PRICED SEPARATELY, from the same position.
//! Receiving a luxury we lack is worth the Amenities it brings; receiving a
//! duplicate is worth nothing. Giving a spare copy costs almost nothing;
//! giving the only copy costs the Amenities our cities lose with it. A deal
//! is priced as a whole ([`Position::net`]): two new luxuries reaching the same
//! three cities are worth less than twice one, because Amenities work in
//! steps (the happiness table below).
//!
//! ★ THE VICTORY LANE CHANGES THE NUMBERS. A Culture seat pays for Tourism, a
//! Science seat for Science, a Religion seat for Faith, a Domination seat for
//! Production and for the strategic resources its army needs, a Diplomacy
//! seat for Favor (its votes).
//!
//! Every constant marked GUESS is one: a number chosen for a stated reason,
//! not measured. Each is a named constant so that a measurement can replace it
//! in one place. Nothing here reads another seat's valuation; what a partner
//! will take is found by offer and counter-offer, never computed.

use super::{Deal, Item};
use crate::ai::VictoryTarget;
use crate::mirror::{StateCity, StateRival, StateSnapshot};
use std::collections::BTreeMap;

// ---- the clock -------------------------------------------------------------

/// Turns a timed deal item (Gold per turn, a luxury copy, Open Borders) lasts:
/// the shipped deal screen's default for every timed item,
/// `Base/Assets/UI/DiplomacyDealView.lua:84`
/// (`local ms_DefaultMultiTurnGoldDuration = 30;`) and `:1340`
/// (`pDealItem:SetDuration(30);	-- Default to this many turns`).
pub const DEAL_TURNS: u32 = 30;

/// GUESS. What a turn's delay costs at Standard speed: 1.5% a turn, so 30
/// turns of a payment are worth about 24 turns of it in hand, close to the
/// 25-turn factor the engine's own book puts on Gold per turn
/// (`Game::receive_items_value`). A faster game makes each turn a larger
/// share of the game, so the rate scales with [`speed_cost_percent`]: 3% a
/// turn at Online, where 30 turns are worth about 19.6.
pub const DISCOUNT_PER_TURN_STANDARD: f64 = 0.015;

/// The shipped `GameSpeeds.CostMultiplier` of each speed
/// (`Base/Assets/Gameplay/Data/GameSpeeds.xml`: Marathon 300, Epic 150,
/// Standard 100, Quick 67, Online 50). An unknown name reads as Standard.
pub fn speed_cost_percent(speed: &str) -> f64 {
    match speed {
        "GAMESPEED_ONLINE" => 50.0,
        "GAMESPEED_QUICK" => 67.0,
        "GAMESPEED_EPIC" => 150.0,
        "GAMESPEED_MARATHON" => 300.0,
        _ => 100.0,
    }
}

/// GUESS. Turns a permanent asset (a Great Work) is valued over, at most: past
/// this the discount and the game's end make the rest small.
pub const PERMANENT_HORIZON_TURNS: u32 = 60;

/// Turns left in a game whose board does not say (`seat.max_turns` unset).
/// GUESS: a mid-game seat.
pub const UNKNOWN_TURNS_LEFT: u32 = 100;

// ---- what a yield point is worth -------------------------------------------

/// GUESS. Gold a point of Food is worth: it is what Growth is made of, and a
/// citizen pays back more than a Gold over the horizon, but only after it is
/// grown.
pub const FOOD_GOLD: f64 = 1.0;
/// Gold the host asks for a point of Production when nothing on the board says
/// (the live table of 2026-11-27, Online speed: a 25-Production Builder priced
/// at 105 Gold, `hotseat-20261128T031456Z` seats p3 and p5). The board's own
/// purchase menu replaces it when it can be read ([`board_purchase_rate`]).
pub const GOLD_PER_PRODUCTION_PURCHASE: f64 = 4.0;
/// GUESS. Share of the purchase price a point of Production we make ourselves
/// is worth: Gold buys only what is worth buying, so a point of our own
/// Production is worth about half of buying one.
pub const PRODUCTION_SHARE_OF_PURCHASE: f64 = 0.5;
/// GUESS. Gold a point of Science is worth to a seat not racing Science.
pub const SCIENCE_GOLD: f64 = 1.5;
/// GUESS. Gold a point of Culture is worth to a seat not racing Culture.
pub const CULTURE_GOLD: f64 = 1.5;
/// GUESS. Gold a point of Faith is worth to a seat not racing Religion.
pub const FAITH_GOLD: f64 = 0.75;
/// GUESS. Gold a point of Tourism a turn is worth to a seat not racing
/// Culture: it is not their victory, but it is still a lead in the race.
pub const TOURISM_GOLD: f64 = 0.25;
/// GUESS. Gold a point of Tourism a turn is worth to a Culture seat, whose
/// victory it is.
pub const CULTURE_SEAT_TOURISM_GOLD: f64 = 3.0;
/// GUESS. How much more a seat values the yield its victory is made of.
pub const LANE_YIELD_PREMIUM: f64 = 1.5;

/// Gold each kind of yield is worth to this seat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct YieldGold {
    pub food: f64,
    pub production: f64,
    pub gold: f64,
    pub science: f64,
    pub culture: f64,
    pub faith: f64,
    pub tourism: f64,
}

impl YieldGold {
    /// The weights for a victory lane, with Production at
    /// `purchase_rate × PRODUCTION_SHARE_OF_PURCHASE`.
    pub fn for_lane(lane: Option<VictoryTarget>, purchase_rate: f64) -> YieldGold {
        let mut weights = YieldGold {
            food: FOOD_GOLD,
            production: purchase_rate * PRODUCTION_SHARE_OF_PURCHASE,
            gold: 1.0,
            science: SCIENCE_GOLD,
            culture: CULTURE_GOLD,
            faith: FAITH_GOLD,
            tourism: TOURISM_GOLD,
        };
        match lane {
            Some(VictoryTarget::Science) => weights.science *= LANE_YIELD_PREMIUM,
            Some(VictoryTarget::Culture) => {
                weights.culture *= LANE_YIELD_PREMIUM;
                weights.tourism = CULTURE_SEAT_TOURISM_GOLD;
            }
            Some(VictoryTarget::Religion) => weights.faith *= LANE_YIELD_PREMIUM,
            Some(VictoryTarget::Domination) => weights.production *= LANE_YIELD_PREMIUM,
            Some(VictoryTarget::Diplomacy) | Some(VictoryTarget::Score) | None => {}
        }
        weights
    }
}

/// The seat's own Gold price for a point of Production: the median, over its
/// cities, of the host's purchase price over the production cost of the same
/// item, between 1 and 8; [`GOLD_PER_PRODUCTION_PURCHASE`] when the board
/// pairs none.
pub fn board_purchase_rate(state: &StateSnapshot) -> f64 {
    let mut rates: Vec<f64> = Vec::new();
    for city in &state.cities {
        let (Some(menu), Some(shop)) = (city.buildable.as_ref(), city.purchasable.as_ref()) else {
            continue;
        };
        for item in shop {
            let Some(gold) = item.g else { continue };
            let Some(row) = menu.iter().find(|row| row.t == item.t) else {
                continue;
            };
            if gold.is_finite() && gold > 0.0 && row.c.is_finite() && row.c > 0.0 {
                rates.push(gold / row.c);
            }
        }
    }
    if rates.is_empty() {
        return GOLD_PER_PRODUCTION_PURCHASE;
    }
    rates.sort_by(f64::total_cmp);
    rates[rates.len() / 2].clamp(1.0, 8.0)
}

// ---- Amenities --------------------------------------------------------------

/// What an Amenity score does to a city, by the happiness table in force with
/// Gathering Storm: `Base/Assets/Gameplay/Data/Happinesses.xml:17-23` with the
/// updates of `DLC/Expansion2/Data/Expansion2_Buildings.xml:463-515` (Content
/// 0 to 2, Happy 3 to 4, Ecstatic 5 and up; yields -40/-30/-20/-10/0/+10/+20)
/// and the loyalty rows of `DLC/Expansion2/Data/Expansion1_Happinesses.xml:5-11`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mood {
    /// `GrowthModifier`, percent.
    pub growth_pct: f64,
    /// `NonFoodYieldModifier`, percent.
    pub yield_pct: f64,
    /// `IdentityPerTurnChange`: loyalty a turn.
    pub identity: f64,
}

/// The [`Mood`] of a city whose Amenities less those it needs are `score`.
pub fn mood(score: i32) -> Mood {
    let (growth_pct, yield_pct, identity) = match score {
        i32::MIN..=-7 => (-100.0, -40.0, -6.0),
        -6..=-5 => (-100.0, -30.0, -6.0),
        -4..=-3 => (-30.0, -20.0, -6.0),
        -2..=-1 => (-15.0, -5.0, -3.0),
        0..=2 => (0.0, 0.0, 0.0),
        3..=4 => (10.0, 10.0, 3.0),
        _ => (20.0, 20.0, 6.0),
    };
    Mood {
        growth_pct,
        yield_pct,
        identity,
    }
}

/// Cities one luxury type serves, one Amenity each (the engine's
/// `Game::primary_resource_value`: "One unique Luxury can serve four cities").
pub const CITIES_PER_LUXURY: usize = 4;
/// `CITY_POP_PER_AMENITY` (`Base/Assets/Gameplay/Data/GlobalParameters.xml:104`):
/// a city needs one Amenity per two citizens, rounded up (the live boards:
/// population 1-2 need 1, 3-4 need 2).
pub const POP_PER_AMENITY: i32 = 2;
/// GUESS. Gold a point of loyalty a turn is worth to a city that is losing
/// loyalty; nothing to a city that is not.
pub const LOYALTY_POINT_GOLD: f64 = 1.0;
/// GUESS. What letting go of a spare luxury copy costs: nothing in Amenities
/// (one copy of a type serves as well as five), but the copy could be sold to
/// someone else, so it is never given for nothing.
pub const LUXURY_SPARE_GOLD: f64 = 5.0;

/// Amenities a city of `pop` citizens needs.
pub fn amenities_needed(pop: i32) -> i32 {
    (pop.max(0) + POP_PER_AMENITY - 1) / POP_PER_AMENITY
}

/// One city's Amenity position, as the valuation reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct CityMood {
    pub name: String,
    pub pop: i32,
    /// Growth is projected up to this population (the city's Housing): past it
    /// growth slows to a crawl.
    pub pop_cap: i32,
    /// Amenities less Amenities needed, now.
    pub score: i32,
    /// Luxury Amenities the city receives now: a lost luxury type can only take
    /// these. `None` when the board did not say.
    pub luxury_amenities: Option<i32>,
    /// Turns to the next citizen; growth is not projected when it is not a
    /// positive number.
    pub growth_turns: f64,
    pub food_surplus: f64,
    /// The city's non-Food yields a turn in Gold, before the happiness
    /// modifier the host applies now.
    pub base_nonfood_gold: f64,
    pub loyalty_falling: bool,
}

impl CityMood {
    /// The city as the board reports it, `None` when the host did not report
    /// its Amenities (`NaN` or `-1`, see `StateCity::amenities`).
    pub fn from_city(city: &StateCity, weights: &YieldGold) -> Option<CityMood> {
        if !(city.amenities >= 0.0 && city.amenities_needed >= 0.0) {
            return None;
        }
        let score = (city.amenities - city.amenities_needed).round() as i32;
        let observed = city.yields.as_ref().map_or(0.0, |y| {
            y.production * weights.production
                + y.gold * weights.gold
                + y.science * weights.science
                + y.culture * weights.culture
                + y.faith * weights.faith
        });
        // The host's yields already carry the happiness modifier: undo it, so
        // every level is priced from the same base.
        let applied = if city.happiness_yield_mult.is_finite() {
            city.happiness_yield_mult
        } else {
            mood(score).yield_pct
        };
        let base_nonfood_gold = observed;
        let housing = city.housing.filter(|h| h.is_finite()).unwrap_or(0.0);
        Some(CityMood {
            name: city.name.clone(),
            pop: city.pop.max(0),
            pop_cap: city.pop.max(housing.floor() as i32),
            score,
            luxury_amenities: if city.amenities_luxuries >= 0.0 {
                Some(city.amenities_luxuries.round() as i32)
            } else {
                None
            },
            growth_turns: city.growth_turns,
            food_surplus: if city.food_surplus.is_finite() {
                city.food_surplus
            } else {
                0.0
            },
            base_nonfood_gold: if base_nonfood_gold.is_finite() {
                base_nonfood_gold.max(0.0)
            } else {
                0.0
            },
            loyalty_falling: city.loyalty_per_turn.is_finite() && city.loyalty_per_turn < 0.0,
        })
    }
}

// ---- strategic resources ----------------------------------------------------

/// The engine's base strategic stockpile cap (`Game::strategic_stockpile_capacity`:
/// 50 plus building effects). The board does not carry the cap, so a caller
/// that knows it (the mirrored board) sets [`Position::strategic_cap`].
pub const STRATEGIC_CAP_DEFAULT: f64 = 50.0;
/// GUESS. Gold a strategic unit is worth while the seat needs it for units it
/// means to build or keep supplied over the deal's horizon.
pub const STRATEGIC_NEEDED_GOLD: f64 = 8.0;
/// GUESS. Gold a strategic unit is worth that the seat does not need: it may
/// need it later, or sell it.
pub const STRATEGIC_IDLE_GOLD: f64 = 1.0;
/// GUESS. Giving away units the seat needs costs more than buying them: the
/// army waits while they are replaced.
pub const STRATEGIC_NEED_GIVE_PREMIUM: f64 = 1.25;
/// GUESS. Strategic units a Domination seat expects to spend over a deal's
/// horizon on each resource its era's army uses, when no caller says.
pub const DOMINATION_STRATEGIC_NEED: f64 = 20.0;
/// GUESS. The same for every other seat: a few defenders' worth.
pub const DEFENCE_STRATEGIC_NEED: f64 = 5.0;

/// GUESS. The strategic resources the army of a world era is built from
/// (Ancient = 0): Horses and Iron early, Niter from the Medieval, Coal and Oil
/// from the Industrial, Aluminum and Uranium last.
pub fn era_strategics(era: i64) -> &'static [&'static str] {
    match era {
        i64::MIN..=1 => &["RESOURCE_HORSES", "RESOURCE_IRON"],
        2..=3 => &["RESOURCE_HORSES", "RESOURCE_IRON", "RESOURCE_NITER"],
        4 => &["RESOURCE_HORSES", "RESOURCE_NITER", "RESOURCE_COAL"],
        5..=6 => &["RESOURCE_COAL", "RESOURCE_OIL", "RESOURCE_ALUMINUM"],
        _ => &["RESOURCE_OIL", "RESOURCE_ALUMINUM", "RESOURCE_URANIUM"],
    }
}

// ---- Favor, Great Works, agreements ----------------------------------------

/// Gold a point of Diplomatic Favor is worth: what rivals paid us for it, the
/// median of forty live favor sales (`favor-bought-before-congress` note in
/// `civvis_orders`, "about 5.5 Gold a point").
pub const FAVOR_GOLD_PER_POINT: f64 = 5.5;
/// GUESS. A Diplomacy seat's Favor is its votes: it prices a point at twice
/// the market.
pub const DIPLOMACY_FAVOR_PREMIUM: f64 = 2.0;

/// The engine's book for passage (`Game::open_borders_receive_value`): 28 Gold
/// plus 0.35 a point of the receiver's Tourism a turn, Tourism capped at 80.
pub const OPEN_BORDERS_BASE_GOLD: f64 = 28.0;
pub const OPEN_BORDERS_TOURISM_SHARE: f64 = 0.35;
pub const OPEN_BORDERS_TOURISM_CAP: f64 = 80.0;
/// GUESS. Passage is worth more to a Culture seat (Tourism to a civilization
/// with open borders) and to a Domination seat (its armies march through).
pub const OPEN_BORDERS_LANE_PREMIUM: f64 = 1.5;
/// The engine's book for granting passage (`Game::open_borders_give_cost`):
/// 9 Gold plus 8 for each multiple of our military the receiver fields, the
/// ratio capped at 3.
pub const OPEN_BORDERS_GIVE_BASE_GOLD: f64 = 9.0;
pub const OPEN_BORDERS_GIVE_THREAT_GOLD: f64 = 8.0;

/// The host's fees for a Delegation and a Resident Embassy, paid by the side
/// that sends it (`Base/Assets/Gameplay/Data/DiplomaticActions.xml:68`
/// `Cost="25"`, `:60` `Cost="50"`).
pub const DELEGATION_FEE: f64 = 25.0;
pub const EMBASSY_FEE: f64 = 50.0;
/// GUESS. What our Delegation and Embassy at a rival's court are worth to us
/// (a level of access each, and its opinion of us), a little above the fee:
/// Firaxis's own leaders put them at 25 and 50 when Friendly
/// (`DiplomaticActions.xml:169`, `:164`, `Worth`).
pub const DELEGATION_GOLD: f64 = 35.0;
pub const EMBASSY_GOLD: f64 = 70.0;
/// GUESS. What hosting a rival's Delegation or Embassy costs us (what it lets
/// them see) and earns us (their goodwill): Firaxis's own leaders put the cost
/// at 5 when Friendly (`DiplomaticActions.xml:169`, `Cost`).
pub const HOSTING_COST_GOLD: f64 = 5.0;
pub const HOSTING_GOODWILL_GOLD: f64 = 10.0;
/// Turns after a denunciation, either way, in which a seat hosts nothing from
/// that rival (the luxury swap's own window, `LUXURY_SWAP_DENOUNCE_TURNS`).
pub const DENOUNCED_TURNS: i64 = 30;

/// GUESS. What a Declaration of Friendship is worth on its own: no surprise
/// war from that seat while it lasts.
pub const FRIENDSHIP_GOLD: f64 = 30.0;
/// GUESS. And how much more it is worth for each multiple of our military the
/// friend fields (a stronger friend is a war we do not have to fight),
/// counted up to one multiple.
pub const FRIENDSHIP_SECURITY_GOLD: f64 = 60.0;
/// GUESS. What friendship costs a Domination seat with a rival it outguns by
/// [`DOMINATION_EDGE`]: the war it may not declare while the friendship lasts.
pub const FRIENDSHIP_TIES_THE_ARMY_GOLD: f64 = 150.0;
/// The power edge at which a Domination seat sees a rival as its target: the
/// planner's own recovery threshold (`my_power * 1.25 < opponent`,
/// `assessment.rs`), turned around.
pub const DOMINATION_EDGE: f64 = 1.25;

/// A Great Work class's Culture, Faith and Tourism a turn: the engine's
/// numbers (`Game::great_work_receive_value` and `Game::great_work_tourism`:
/// writing 2/0/2, art 3/0/2, artifact 3/0/3, music 4/0/4, relic 0/4/8), by
/// the host's object class (`Base/Assets/Gameplay/Data/GreatWorks.xml:208-215`).
pub fn great_work_yields(object: &str) -> Option<(f64, f64, f64)> {
    match object {
        "GREATWORKOBJECT_WRITING" => Some((2.0, 0.0, 2.0)),
        "GREATWORKOBJECT_SCULPTURE"
        | "GREATWORKOBJECT_PORTRAIT"
        | "GREATWORKOBJECT_LANDSCAPE"
        | "GREATWORKOBJECT_RELIGIOUS" => Some((3.0, 0.0, 2.0)),
        "GREATWORKOBJECT_ARTIFACT" => Some((3.0, 0.0, 3.0)),
        "GREATWORKOBJECT_MUSIC" => Some((4.0, 0.0, 4.0)),
        "GREATWORKOBJECT_RELIC" => Some((0.0, 4.0, 8.0)),
        _ => None,
    }
}
/// The engine's scarcity rule for letting a Great Work go
/// (`Game::great_work_give_cost`): the last of its class costs 10% more than
/// its yields, one of several 10% less.
pub const LAST_GREAT_WORK_PREMIUM: f64 = 1.10;
pub const SPARE_GREAT_WORK_DISCOUNT: f64 = 0.90;

// ---- the position ------------------------------------------------------------

/// Everything the valuation reads about this seat, at this moment. Built from
/// the board ([`Position::from_board`]); every field is public so that a caller
/// that knows more (the stockpile cap, the units it means to build, the free
/// Great Work slots) can say so.
#[derive(Clone, Debug, PartialEq)]
pub struct Position {
    pub turn: u32,
    pub lane: Option<VictoryTarget>,
    /// Discount a turn at this game speed.
    pub rate: f64,
    pub turns_left: u32,
    pub gold: f64,
    pub gold_per_turn: f64,
    pub favor: f64,
    pub military: f64,
    pub tourism: f64,
    pub weights: YieldGold,
    /// Luxury copies held, by host type name.
    pub luxuries: BTreeMap<String, i64>,
    /// Whether [`Self::luxuries`] can be trusted: the host's count was
    /// exported, or no city receives a luxury Amenity (so none is held).
    pub luxuries_known: bool,
    pub strategics: BTreeMap<String, f64>,
    pub strategic_income: BTreeMap<String, f64>,
    pub strategic_cap: f64,
    /// Strategic units the seat expects to spend over a deal's horizon.
    pub strategic_needs: BTreeMap<String, f64>,
    /// Great Works held, by host object class.
    pub great_works: BTreeMap<String, i64>,
    /// Free Great Work slots, by the object class they take. Empty unless a
    /// caller fills it: a work is worth nothing to a seat with nowhere to put
    /// it, and the board does not list free slots.
    pub great_work_room: BTreeMap<String, i64>,
    pub cities: Vec<CityMood>,
}

impl Position {
    /// This seat's position from its own board, for a seat playing `lane`
    /// (`None`: a general seat).
    pub fn from_board(state: &StateSnapshot, lane: Option<VictoryTarget>) -> Position {
        let weights = YieldGold::for_lane(lane, board_purchase_rate(state));
        let finite = |value: f64| if value.is_finite() { value } else { 0.0 };
        let cities: Vec<CityMood> = state
            .cities
            .iter()
            .filter_map(|city| CityMood::from_city(city, &weights))
            .collect();
        let no_luxury_amenity = state
            .cities
            .iter()
            .all(|city| city.amenities_luxuries == 0.0);
        let luxuries: BTreeMap<String, i64> = state
            .luxury_counts
            .as_ref()
            .map(|counts| {
                counts
                    .iter()
                    .filter(|(_, count)| count.is_finite() && **count >= 1.0)
                    .map(|(name, count)| (name.clone(), count.floor() as i64))
                    .collect()
            })
            .unwrap_or_default();
        let mut great_works: BTreeMap<String, i64> = BTreeMap::new();
        for city in &state.cities {
            for work in city.great_works.iter().flatten() {
                *great_works.entry(work.object.clone()).or_insert(0) += 1;
            }
        }
        let need = if lane == Some(VictoryTarget::Domination) {
            DOMINATION_STRATEGIC_NEED
        } else {
            DEFENCE_STRATEGIC_NEED
        };
        let strategic_needs = era_strategics(state.world_era.unwrap_or(0))
            .iter()
            .map(|name| (name.to_string(), need))
            .collect();
        let max_turns = u32::try_from(state.seat.max_turns).unwrap_or(0);
        Position {
            turn: state.turn,
            lane,
            rate: DISCOUNT_PER_TURN_STANDARD,
            turns_left: if max_turns > state.turn {
                max_turns - state.turn
            } else {
                UNKNOWN_TURNS_LEFT
            },
            gold: (state.gold as f64).max(0.0),
            gold_per_turn: finite(state.gold_per_turn.unwrap_or(0.0)),
            favor: finite(state.favor.unwrap_or(0.0)).max(0.0),
            military: finite(state.military).max(0.0),
            tourism: finite(state.tourism_per_turn.unwrap_or(0.0)).max(0.0),
            weights,
            luxuries,
            luxuries_known: state.luxury_counts.is_some() || no_luxury_amenity,
            strategics: state.strategic_resources.clone().unwrap_or_default(),
            strategic_income: state.strategic_resource_income.clone().unwrap_or_default(),
            strategic_cap: STRATEGIC_CAP_DEFAULT,
            strategic_needs,
            great_works,
            great_work_room: BTreeMap::new(),
            cities,
        }
    }

    /// What a Gold `turns` turns from now is worth now.
    pub fn discount(&self, turns: u32) -> f64 {
        (1.0 + self.rate).powi(-(turns.min(i32::MAX as u32) as i32))
    }

    /// What one Gold a turn for `turns` turns, the first paid next turn, is
    /// worth now.
    pub fn annuity(&self, turns: u32) -> f64 {
        (1..=turns).map(|turn| self.discount(turn)).sum()
    }

    /// The city's population `turns` from now: one citizen every
    /// `growth_turns`, up to its Housing.
    fn projected_pop(city: &CityMood, turns: u32) -> i32 {
        if !(city.growth_turns >= 1.0 && city.food_surplus > 0.0) {
            return city.pop;
        }
        let grown = (f64::from(turns) / city.growth_turns).floor() as i32;
        (city.pop + grown).min(city.pop_cap)
    }

    /// The Gold one turn at Amenity score `score` adds to the city, against a
    /// Content city: the yield and growth modifiers of [`mood`], and loyalty
    /// when the city is losing it.
    fn turn_gold(&self, city: &CityMood, score: i32) -> f64 {
        let mood = mood(score);
        let loyalty = if city.loyalty_falling {
            mood.identity * LOYALTY_POINT_GOLD
        } else {
            0.0
        };
        mood.yield_pct / 100.0 * city.base_nonfood_gold
            + mood.growth_pct / 100.0 * city.food_surplus.max(0.0) * self.weights.food
            + loyalty
    }

    /// The city's Gold over a deal's horizon with `delta` Amenities more than
    /// it has, its need growing with its projected population.
    fn horizon_gold(&self, city: &CityMood, delta: i32) -> f64 {
        let needed_now = amenities_needed(city.pop);
        (1..=DEAL_TURNS)
            .map(|turn| {
                let grown_need = amenities_needed(Self::projected_pop(city, turn)) - needed_now;
                self.discount(turn) * self.turn_gold(city, city.score - grown_need + delta)
            })
            .sum()
    }

    /// The Gold the empire gains over a deal's horizon from holding `gained`
    /// more luxury types and `lost` fewer. Each type moves one Amenity in up to
    /// [`CITIES_PER_LUXURY`] cities. A lost type is taken from the cities that
    /// lose most among those that receive a luxury Amenity now; a gained type
    /// goes where it gains most. Both are greedy, one type at a time, so the
    /// second type in a city is priced after the first.
    pub fn luxury_types_value(&self, gained: usize, lost: usize) -> f64 {
        let count = self.cities.len();
        let mut delta = vec![0i32; count];
        let mut luxury_left: Vec<Option<i32>> = self
            .cities
            .iter()
            .map(|city| city.luxury_amenities)
            .collect();
        let step = |index: usize, at: i32, by: i32| {
            let city = &self.cities[index];
            self.horizon_gold(city, at + by) - self.horizon_gold(city, at)
        };
        for _ in 0..lost {
            let mut order: Vec<(f64, usize)> = (0..count)
                .filter(|index| luxury_left[*index].is_none_or(|left| left > 0))
                .map(|index| (step(index, delta[index], -1), index))
                .collect();
            order.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            for (_, index) in order.into_iter().take(CITIES_PER_LUXURY) {
                delta[index] -= 1;
                if let Some(left) = luxury_left[index].as_mut() {
                    *left -= 1;
                }
            }
        }
        for _ in 0..gained {
            let mut order: Vec<(f64, usize)> = (0..count)
                .map(|index| (step(index, delta[index], 1), index))
                .collect();
            order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            for (_, index) in order.into_iter().take(CITIES_PER_LUXURY) {
                delta[index] += 1;
            }
        }
        (0..count)
            .map(|index| {
                let city = &self.cities[index];
                self.horizon_gold(city, delta[index]) - self.horizon_gold(city, 0)
            })
            .sum()
    }

    /// The Gold a change of luxury copies is worth (negative: what it costs),
    /// by type: `changes` maps a host type name to copies received (positive)
    /// or given (negative). `NEG_INFINITY` when a copy we do not hold is
    /// given, or any copy is given while the board cannot say what we hold.
    fn luxury_change_value(&self, changes: &BTreeMap<&str, i64>) -> f64 {
        let mut gained = 0usize;
        let mut lost = 0usize;
        let mut spares_given = 0i64;
        for (name, change) in changes {
            if *change == 0 {
                continue;
            }
            if *change < 0 && !self.luxuries_known {
                return f64::NEG_INFINITY;
            }
            if *change > 0 && !self.luxuries_known {
                // A first copy cannot be told from a duplicate: worth nothing
                // for certain.
                continue;
            }
            let held = self.luxuries.get(*name).copied().unwrap_or(0);
            let after = held + change;
            if after < 0 {
                return f64::NEG_INFINITY;
            }
            if after > 0 {
                gained += 1;
            }
            if held > 0 && after == 0 {
                lost += 1;
            }
            if *change < 0 {
                spares_given += 0;
            }
        }
        self.luxury_types_value(gained, lost) - spares_given as f64 * LUXURY_SPARE_GOLD
    }

    /// The Gold a change of one strategic stockpile is worth (negative: what
    /// it costs). Units received fill the seat's shortfall over the horizon at
    /// [`STRATEGIC_NEEDED_GOLD`], the rest at [`STRATEGIC_IDLE_GOLD`], none past
    /// the cap. Units given come first from what the cap would waste anyway
    /// (free), then from the spare (idle price), last from what the seat needs
    /// (the need price and [`STRATEGIC_NEED_GIVE_PREMIUM`]).
    fn strategic_change_value(&self, resource: &str, change: i64) -> f64 {
        let stock = self
            .strategics
            .get(resource)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let income = self
            .strategic_income
            .get(resource)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let need = self
            .strategic_needs
            .get(resource)
            .copied()
            .unwrap_or(0.0)
            .max(0.0);
        let accrued = income * f64::from(DEAL_TURNS);
        if change > 0 {
            let room = (self.strategic_cap - stock).max(0.0);
            let usable = (change as f64).min(room);
            let short = (need - stock - accrued).max(0.0);
            let fill = usable.min(short);
            usable * STRATEGIC_IDLE_GOLD
        } else if change < 0 {
            let given = (-change) as f64;
            if given > stock + 1e-9 {
                return f64::NEG_INFINITY;
            }
            let needed_from_stock = (need - accrued).max(0.0);
            let spare = (stock - needed_from_stock).max(0.0);
            let wasted = (stock + accrued - need - self.strategic_cap)
                .max(0.0)
                .min(spare);
            let free = given.min(wasted);
            let idle = (given - free).min(spare - free);
            let needed = given - free - idle;
            -(idle * STRATEGIC_IDLE_GOLD
                + needed * STRATEGIC_NEEDED_GOLD * STRATEGIC_NEED_GIVE_PREMIUM)
        } else {
            0.0
        }
    }

    /// The Gold a Great Work of `object` class pays this seat over the rest of
    /// the game (at most [`PERMANENT_HORIZON_TURNS`]).
    fn great_work_stream(&self, object: &str) -> f64 {
        let Some((culture, faith, tourism)) = great_work_yields(object) else {
            return 0.0;
        };
        let per_turn = culture * self.weights.culture
            + faith * self.weights.faith
            + tourism * self.weights.tourism;
        per_turn * self.annuity(self.turns_left.min(PERMANENT_HORIZON_TURNS))
    }

    /// The Gold a change of Great Works of one class is worth (negative: what
    /// it costs). Works received beyond the free slots are worth nothing.
    fn great_work_change_value(&self, object: &str, change: i64) -> f64 {
        let stream = self.great_work_stream(object);
        if change > 0 {
            let room = self
                .great_work_room
                .get(object)
                .copied()
                .unwrap_or(0)
                .max(0);
            change as f64 * stream
        } else if change < 0 {
            let held = self.great_works.get(object).copied().unwrap_or(0);
            if -change > held {
                return f64::NEG_INFINITY;
            }
            let scarcity = if -change >= held {
                LAST_GREAT_WORK_PREMIUM
            } else {
                SPARE_GREAT_WORK_DISCOUNT
            };
            change as f64 * stream * scarcity
        } else {
            0.0
        }
    }

    /// Gold a point of Favor is worth to this seat.
    pub fn favor_point_gold(&self) -> f64 {
        if self.lane == Some(VictoryTarget::Diplomacy) {
            FAVOR_GOLD_PER_POINT
        } else {
            FAVOR_GOLD_PER_POINT
        }
    }

    /// Whether we denounced them, or they us, within [`DENOUNCED_TURNS`].
    fn denounced(&self, partner: &StateRival) -> bool {
        let recent = |turn: Option<i64>| {
            turn.is_some_and(|at| at >= 0 && i64::from(self.turn) - at < DENOUNCED_TURNS)
        };
        recent(partner.our_denounce_turn) || recent(partner.their_denounce_turn)
    }

    /// Their military over ours, from 0 to 3; 1 when the partner is unknown.
    fn threat(&self, partner: Option<&StateRival>) -> f64 {
        partner.map_or(1.0, |rival| {
            let theirs = if rival.military.is_finite() {
                rival.military.max(0.0)
            } else {
                self.military
            };
            (theirs / self.military.max(1.0)).clamp(0.0, 3.0)
        })
    }

    /// What a Declaration of Friendship with `partner` is worth to us; it can
    /// be negative (a Domination seat befriending its target). `NEG_INFINITY`
    /// at war: there is no friendship to declare.
    pub fn friendship_value(&self, partner: Option<&StateRival>) -> f64 {
        if partner.is_some_and(|rival| rival.at_war) {
            return f64::NEG_INFINITY;
        }
        let threat = self.threat(partner);
        let mut value = FRIENDSHIP_GOLD + FRIENDSHIP_SECURITY_GOLD * (threat - 1.0).clamp(0.0, 1.0);
        if self.lane == Some(VictoryTarget::Domination)
            && partner.is_some()
            && threat * DOMINATION_EDGE <= 1.0
        {
            value -= 0.0;
        }
        value
    }

    /// What receiving one item that is priced on its own is worth.
    fn get_value(&self, item: &Item) -> f64 {
        match item {
            Item::OpenBorders => {
                let premium = match self.lane {
                    Some(VictoryTarget::Culture) | Some(VictoryTarget::Domination) => {
                        OPEN_BORDERS_LANE_PREMIUM
                    }
                    _ => 1.0,
                };
                (OPEN_BORDERS_BASE_GOLD
                    + OPEN_BORDERS_TOURISM_SHARE * self.tourism.min(OPEN_BORDERS_TOURISM_CAP))
                    * premium
            }
            Item::Delegation => DELEGATION_GOLD - DELEGATION_FEE,
            Item::Embassy => EMBASSY_GOLD - EMBASSY_FEE,
            // Lump and per-turn Gold, Favor, and the items priced as a whole in
            // `net`, never reach here.
            _ => 0.0,
        }
    }

    /// What giving one item that is priced on its own costs; `INFINITY` when
    /// we cannot or will not give it.
    fn give_cost(&self, item: &Item, partner: Option<&StateRival>) -> f64 {
        match item {
            Item::OpenBorders => {
                OPEN_BORDERS_GIVE_BASE_GOLD + OPEN_BORDERS_GIVE_THREAT_GOLD * self.threat(partner)
            }
            Item::Delegation | Item::Embassy => {
                if partner.is_some_and(|rival| rival.at_war) {
                    f64::INFINITY
                } else {
                    HOSTING_COST_GOLD - HOSTING_GOODWILL_GOLD
                }
            }
            _ => 0.0,
        }
    }

    /// ★ THE PRICE OF A WHOLE DEAL TO THIS SEAT: what it receives less what it
    /// gives, in Gold now. `NEG_INFINITY` when the deal asks for something the
    /// seat does not have, cannot promise (Gold per turn beyond its income) or
    /// will not give (hosting a seat it is at war with), or carries a negative
    /// amount. `partner` is the other side as our board sees it, for the items
    /// whose price depends on who they are (passage, hosting, friendship).
    pub fn net(&self, deal: &Deal, partner: Option<&StateRival>) -> f64 {
        let mut total = 0.0;
        let mut gold = 0i64;
        let mut gold_per_turn = 0i64;
        let mut favor = 0i64;
        let mut luxuries: BTreeMap<&str, i64> = BTreeMap::new();
        let mut strategics: BTreeMap<&str, i64> = BTreeMap::new();
        let mut works: BTreeMap<&str, i64> = BTreeMap::new();
        let mut friendship = false;
        for (sign, items) in [(1i64, &deal.get), (-1i64, &deal.give)] {
            for item in items {
                match item {
                    Item::Gold(amount)
                    | Item::GoldPerTurn(amount)
                    | Item::Favor(amount)
                    | Item::Strategic(_, amount)
                        if *amount < 0 =>
                    {
                        return f64::NEG_INFINITY;
                    }
                    Item::Gold(amount) => gold += sign * amount,
                    Item::GoldPerTurn(amount) => gold_per_turn += sign * amount,
                    Item::Favor(amount) => favor += sign * amount,
                    Item::Luxury(name) => *luxuries.entry(name.as_str()).or_insert(0) += sign,
                    Item::Strategic(name, amount) => {
                        *strategics.entry(name.as_str()).or_insert(0) += sign * amount
                    }
                    Item::GreatWork(object) => *works.entry(object.as_str()).or_insert(0) += sign,
                    Item::Friendship => friendship = true,
                    other if sign > 0 => total += self.get_value(other),
                    other => total -= self.give_cost(other, partner),
                }
            }
        }
        if -gold > self.gold.floor() as i64
            || false
            || -favor > self.favor.floor() as i64
        {
            return f64::NEG_INFINITY;
        }
        total += gold as f64;
        total += gold_per_turn as f64 * self.annuity(DEAL_TURNS);
        total += favor as f64 * self.favor_point_gold();
        total += self.luxury_change_value(&luxuries);
        for (name, change) in strategics {
            total += self.strategic_change_value(name, change);
        }
        for (object, change) in works {
            total += self.great_work_change_value(object, change);
        }
        if friendship {
            total += self.friendship_value(partner);
        }
        if total.is_nan() {
            f64::NEG_INFINITY
        } else {
            total
        }
    }

    /// What receiving `item` alone is worth to this seat.
    pub fn value_of_getting(&self, item: &Item, partner: Option<&StateRival>) -> f64 {
        self.net(
            &Deal {
                give: Vec::new(),
                get: vec![item.clone()],
            },
            partner,
        )
    }

    /// What giving `item` alone costs this seat (`INFINITY`: it cannot).
    pub fn cost_of_giving(&self, item: &Item, partner: Option<&StateRival>) -> f64 {
        -self.net(
            &Deal {
                give: vec![item.clone()],
                get: Vec::new(),
            },
            partner,
        )
    }
}

#[cfg(test)]
mod tests;
