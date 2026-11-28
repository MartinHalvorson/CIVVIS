//! The hotseat deal board (`state.deals`, contract "deals v1" in
//! `~/civvis-civ6-runs/hotseat/findings/trade-deals-design.md` section 4)
//! and the item grammar of the `deal` order.
//!
//! The mod writes, on a CIVVIS seat's frame-0 board in a hotseat game, the
//! deals waiting for this seat (`incoming`), the ones it has waiting with
//! others (`outgoing`), how its recent proposals ended (`outcomes`), which
//! the person refused (`refused_by_person`) and who each counterpart is
//! (`counterparts`: `person`, `civvis` or `ai`).
//!
//! ⚠ READ LENIENTLY, ON PURPOSE. A new typed field that fails to deserialize
//! loses the WHOLE board (the warning on `StateRival::techs`), and a Lua empty
//! table reaches JSON as `[]` whether it was a list or a map. So the board
//! keeps `deals` as raw JSON (`StateSnapshot::deals`) and this module reads it
//! field by field: anything it cannot read is left out, never fatal.

use super::{Deal, Item};
use serde_json::Value;
use std::collections::BTreeMap;

/// The contract version this reader understands.
pub const DEALS_VERSION: i64 = 1;

/// One deal item as the board carries it (`{"type":"GOLD","amount":4,
/// "duration":30}`): the host's `DealItemTypes` name, its subtype or
/// resource name, amount, duration (0: one-time) and, for a Great Work, its
/// instance and object class.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardItem {
    pub kind: String,
    pub name: Option<String>,
    pub amount: i64,
    pub duration: i64,
    pub instance: Option<i64>,
    /// `GREATWORKOBJECT_*` of a Great Work (decider need 2 in the contract).
    pub object: Option<String>,
    /// `luxury` / `strategic` of a resource (decider need 1).
    pub class: Option<String>,
}

/// Both sides of a deal from this seat's side, as board items.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Terms {
    /// From this seat.
    pub give: Vec<BoardItem>,
    /// From the other side.
    pub get: Vec<BoardItem>,
}

/// A deal another seat has proposed to this one.
#[derive(Clone, Debug, PartialEq)]
pub struct IncomingDeal {
    pub from: usize,
    /// `person`, `civvis` or `ai`.
    pub from_kind: String,
    pub id: i64,
    pub pending_turns: i64,
    pub terms: Terms,
    /// Our proposal this deal answers, when the mod knows it (decider need 4).
    pub counter_of: Option<i64>,
    /// Deals crossed in this exchange, both ways, from 1 (decider need 4).
    pub round: Option<i64>,
}

/// A deal this seat has proposed and that is still waiting.
#[derive(Clone, Debug, PartialEq)]
pub struct OutgoingDeal {
    pub to: usize,
    pub id: i64,
    pub proposed_turn: i64,
    pub terms: Terms,
}

/// How one of this seat's proposals ended. `terms` is empty when the board
/// did not carry them (decider need 3).
#[derive(Clone, Debug, PartialEq)]
pub struct DealOutcome {
    pub with: usize,
    pub id: i64,
    pub turn: i64,
    /// `accepted`, `rejected`, `countered`, `withdrawn`, `expired`, `unknown`.
    pub outcome: String,
    pub terms: Terms,
}

/// The `deals` board of one seat's frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DealBoard {
    pub incoming: Vec<IncomingDeal>,
    pub outgoing: Vec<OutgoingDeal>,
    pub outcomes: Vec<DealOutcome>,
    /// Ids of our proposals the person refused, with the turn and (when the
    /// board carries them) the terms.
    pub refused_by_person: Vec<(i64, i64, Terms)>,
    /// Who each counterpart is, by host player id.
    pub counterparts: BTreeMap<usize, String>,
    /// Free Great Work slots by the object class they take (decider need 5).
    pub great_work_room: BTreeMap<String, i64>,
    /// Each strategic resource's stockpile cap (decider need 6).
    pub strategic_caps: BTreeMap<String, f64>,
}

fn int(value: &Value) -> Option<i64> {
    value.as_i64().or_else(|| {
        value
            .as_f64()
            .filter(|v| v.is_finite())
            .map(|v| v.round() as i64)
    })
}

fn int_at(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(int)
}

fn text_at(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_string)
}

fn player_at(value: &Value, key: &str) -> Option<usize> {
    int_at(value, key).and_then(|player| usize::try_from(player).ok())
}

/// The items of a list, each read on its own; an unreadable one is dropped.
fn items_at(value: &Value, key: &str) -> Vec<BoardItem> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(BoardItem::from_json).collect())
        .unwrap_or_default()
}

fn terms_at(value: &Value) -> Terms {
    Terms {
        give: items_at(value, "give"),
        get: items_at(value, "get"),
    }
}

/// A `{name: number}` map; anything else (a Lua empty table is `[]`) is empty.
fn numbers_at(value: &Value, key: &str) -> BTreeMap<String, f64> {
    value
        .get(key)
        .and_then(Value::as_object)
        .map(|map| {
            map.iter()
                .filter_map(|(name, number)| {
                    number
                        .as_f64()
                        .filter(|n| n.is_finite())
                        .map(|n| (name.clone(), n))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn list_at<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

impl DealBoard {
    /// The board's `deals`, `None` when absent or not a version this reader
    /// understands.
    pub fn from_json(deals: &Value) -> Option<DealBoard> {
        if int_at(deals, "v") != Some(DEALS_VERSION) {
            return None;
        }
        let incoming = list_at(deals, "incoming")
            .filter_map(|deal| {
                Some(IncomingDeal {
                    from: player_at(deal, "from")?,
                    from_kind: text_at(deal, "from_kind").unwrap_or_default(),
                    id: int_at(deal, "id")?,
                    pending_turns: int_at(deal, "pending_turns").unwrap_or(0),
                    terms: terms_at(deal),
                    counter_of: int_at(deal, "counter_of"),
                    round: int_at(deal, "round"),
                })
            })
            .collect();
        let outgoing = list_at(deals, "outgoing")
            .filter_map(|deal| {
                Some(OutgoingDeal {
                    to: player_at(deal, "to")?,
                    id: int_at(deal, "id")?,
                    proposed_turn: int_at(deal, "proposed_turn").unwrap_or(-1),
                    terms: terms_at(deal),
                })
            })
            .collect();
        let outcomes = list_at(deals, "outcomes")
            .filter_map(|outcome| {
                Some(DealOutcome {
                    with: player_at(outcome, "with")?,
                    id: int_at(outcome, "id")?,
                    turn: int_at(outcome, "turn").unwrap_or(-1),
                    outcome: text_at(outcome, "outcome").unwrap_or_default(),
                    terms: terms_at(outcome),
                })
            })
            .collect();
        let refused_by_person = list_at(deals, "refused_by_person")
            .filter_map(|refused| {
                Some((
                    int_at(refused, "id")?,
                    int_at(refused, "turn").unwrap_or(-1),
                    terms_at(refused),
                ))
            })
            .collect();
        let counterparts = list_at(deals, "counterparts")
            .filter_map(|who| Some((player_at(who, "player")?, text_at(who, "kind")?)))
            .collect();
        Some(DealBoard {
            incoming,
            outgoing,
            outcomes,
            refused_by_person,
            counterparts,
            great_work_room: numbers_at(deals, "great_work_room")
                .into_iter()
                .map(|(object, slots)| (object, slots.floor() as i64))
                .collect(),
            strategic_caps: numbers_at(deals, "strategic_caps"),
        })
    }

    /// Whether host player `player` is the person.
    pub fn is_person(&self, player: usize) -> bool {
        self.counterparts.get(&player).map(String::as_str) == Some("person")
            || self
                .incoming
                .iter()
                .any(|deal| deal.from == player && deal.from_kind == "person")
    }
}

impl BoardItem {
    /// One item of the board, `None` when it has no type.
    pub fn from_json(value: &Value) -> Option<BoardItem> {
        Some(BoardItem {
            kind: text_at(value, "type")?,
            name: text_at(value, "name"),
            amount: int_at(value, "amount").unwrap_or(0),
            duration: int_at(value, "duration").unwrap_or(0),
            instance: int_at(value, "instance"),
            object: text_at(value, "object"),
            class: text_at(value, "class"),
        })
    }

    pub fn gold(amount: i64) -> BoardItem {
        BoardItem {
            kind: "GOLD".to_string(),
            amount,
            ..BoardItem::default()
        }
    }

    pub fn gold_per_turn(amount: i64) -> BoardItem {
        BoardItem {
            kind: "GOLD".to_string(),
            amount,
            duration: i64::from(super::value::DEAL_TURNS),
            ..BoardItem::default()
        }
    }

    /// Whether this is lump Gold (`Some(false)`), Gold a turn
    /// (`Some(true)`), or not Gold at all.
    pub fn gold_kind(&self) -> Option<bool> {
        (self.kind == "GOLD").then_some(self.duration > 0)
    }

    /// Whether a resource item is a luxury copy: the board's class, else a
    /// timed item (a luxury is lent for the deal's turns; a strategic lump is
    /// handed over at once).
    fn is_luxury(&self) -> bool {
        match self.class.as_deref() {
            Some(class) => class.eq_ignore_ascii_case("luxury"),
            None => self.duration > 0,
        }
    }

    /// The valuation's items for this board item; `None` when the valuation
    /// cannot price it (a city, a captive, a war, an alliance, a Great Work of
    /// ours it cannot find). `ours` is whether this seat gives it, and
    /// `own_works` maps our Great Work type names to their object class.
    pub fn to_items(&self, ours: bool, own_works: &BTreeMap<String, String>) -> Option<Vec<Item>> {
        let amount = self.amount;
        match self.kind.as_str() {
            "GOLD" if self.duration > 0 => Some(vec![Item::GoldPerTurn(amount)]),
            "GOLD" => Some(vec![Item::Gold(amount)]),
            "FAVOR" => Some(vec![Item::Favor(amount)]),
            "RESOURCE" => {
                let name = self.name.clone()?;
                if self.is_luxury() {
                    let copies = usize::try_from(amount.max(1)).ok()?;
                    Some(vec![Item::Luxury(name); copies])
                } else {
                    Some(vec![Item::Strategic(name, amount)])
                }
            }
            "AGREEMENT" if self.name.as_deref() == Some("OPEN_BORDERS") => {
                Some(vec![Item::OpenBorders])
            }
            "GREATWORK" => {
                let object = match (&self.object, ours) {
                    (Some(object), _) => object.clone(),
                    (None, true) => own_works.get(self.name.as_deref()?)?.clone(),
                    // A work whose class the board does not say is worth
                    // nothing we can count on.
                    (None, false) => String::new(),
                };
                Some(vec![Item::GreatWork(object)])
            }
            _ => None,
        }
    }

    /// The item in the `deal` order's grammar (`GOLD=50`, `GOLD=4@30`,
    /// `FAVOR=10`, `RESOURCE_DYES=1@30`, `RESOURCE_IRON=10`, `OPEN_BORDERS@30`,
    /// `GREATWORK#12`); `None` for an item the order cannot carry.
    pub fn render(&self) -> Option<String> {
        let timed = |head: String| {
            if self.duration > 0 {
                format!("{head}@{}", self.duration)
            } else {
                head
            }
        };
        match self.kind.as_str() {
            "GOLD" => Some(timed(format!("GOLD={}", self.amount))),
            "FAVOR" => Some(format!("FAVOR={}", self.amount)),
            "RESOURCE" => Some(timed(format!("{}={}", self.name.as_deref()?, self.amount))),
            "AGREEMENT" if self.name.as_deref() == Some("OPEN_BORDERS") => Some(format!(
                "OPEN_BORDERS@{}",
                if self.duration > 0 {
                    self.duration
                } else {
                    i64::from(super::value::DEAL_TURNS)
                }
            )),
            "GREATWORK" => Some(format!("GREATWORK#{}", self.instance?)),
            _ => None,
        }
    }
}

impl Terms {
    /// The valuation's deal for these terms; `None` when any item cannot be
    /// priced.
    pub fn deal(&self, own_works: &BTreeMap<String, String>) -> Option<Deal> {
        let side = |items: &[BoardItem], ours: bool| -> Option<Vec<Item>> {
            let mut out = Vec::new();
            for item in items {
                out.extend(item.to_items(ours, own_works)?);
            }
            Some(out)
        };
        Some(Deal {
            give: side(&self.give, true)?,
            get: side(&self.get, false)?,
        })
    }

    /// `give=<items>;get=<items>` for a `deal` order; `None` when an item
    /// cannot be carried.
    pub fn render(&self) -> Option<String> {
        let side = |items: &[BoardItem]| -> Option<String> {
            Some(
                items
                    .iter()
                    .map(BoardItem::render)
                    .collect::<Option<Vec<String>>>()?
                    .join(","),
            )
        };
        Some(format!(
            "give={};get={}",
            side(&self.give)?,
            side(&self.get)?
        ))
    }

    /// The terms in a canonical form, items sorted on each side, so two sets
    /// of terms that move the same things compare equal.
    pub fn key(&self) -> Option<String> {
        let side = |items: &[BoardItem]| -> Option<String> {
            let mut rendered: Vec<String> = items
                .iter()
                .map(BoardItem::render)
                .collect::<Option<Vec<String>>>()?;
            rendered.sort();
            Some(rendered.join(","))
        };
        Some(format!(
            "give={};get={}",
            side(&self.give)?,
            side(&self.get)?
        ))
    }

    /// The terms with `extra` more Gold from the other side, of the kind
    /// asked (a turn, or lump): added to an item of theirs of that kind if
    /// there is one, so the deal keeps one Gold item a kind a side.
    pub fn with_more_gold_from_them(&self, extra: i64, per_turn: bool) -> Terms {
        let mut terms = self.clone();
        match terms
            .get
            .iter_mut()
            .find(|item| item.gold_kind() == Some(per_turn))
        {
            Some(item) => item.amount += extra,
            None => terms.get.push(if per_turn {
                BoardItem::gold_per_turn(extra)
            } else {
                BoardItem::gold(extra)
            }),
        }
        terms
    }

    /// The terms with `less` less lump Gold from us (the item removed when it
    /// reaches zero).
    pub fn with_less_gold_from_us(&self, less: i64) -> Terms {
        let mut terms = self.clone();
        if let Some(index) = terms
            .give
            .iter()
            .position(|item| item.gold_kind() == Some(false))
        {
            terms.give[index].amount -= less;
            if terms.give[index].amount <= 0 {
                terms.give.remove(index);
            }
        }
        terms
    }

    /// The Gold, of the kind asked, the other side gives in these terms.
    pub fn gold_from_them(&self, per_turn: bool) -> i64 {
        self.get
            .iter()
            .filter(|item| item.gold_kind() == Some(per_turn))
            .map(|item| item.amount)
            .sum()
    }

    /// The lump Gold we give in these terms.
    pub fn lump_gold_from_us(&self) -> i64 {
        self.give
            .iter()
            .filter(|item| item.gold_kind() == Some(false))
            .map(|item| item.amount)
            .sum()
    }
}
