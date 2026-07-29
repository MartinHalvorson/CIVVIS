//! Interned ruleset identifiers.
//!
//! Everything in this engine is named: a tile's terrain, a unit's kind, a
//! city's buildings, a player's techs, every effect key in the ruleset. Those
//! names were `String`s, which made three costs unavoidable.
//!
//! *Looking one up* hashed the text and then compared it byte for byte against
//! the table's copy — two dependent cache misses and a `memcmp` for a question
//! whose answer is a table slot. *Comparing two* was a `memcmp`. *Copying game
//! state* — which the AI does once per search branch — allocated a fresh heap
//! buffer for every name in every tile, unit and city it copied. Profiling a
//! 400-turn six-player game found `memcmp`, `memmove` and the allocator
//! together holding about **forty percent** of the machine, spread thinly over
//! hundreds of call sites with no single one worth fixing.
//!
//! A [`Name`] is a 32-bit index into a process-wide table of leaked strings. It
//! is `Copy`, so copying game state copies plain data; equality is one integer
//! compare; and a [`SpecMap`](crate::specmap::SpecMap) can answer a lookup by
//! indexing an array instead of hashing.
//!
//! **It still behaves like a string.** `Deref<Target = str>` means every read
//! site — formatting, matching against a literal, passing to a `&str`
//! parameter, `Option::as_deref` — compiles unchanged. It serializes as its
//! text, so saves and observations keep exactly the shape they had.
//!
//! **Ordering is the text's, not the id's.** `BTreeMap`, `BTreeSet` and every
//! `sort` in the engine decide iteration order, and iteration order decides
//! outcomes; ids are handed out in the order names are first seen, which is not
//! alphabetical. So `Ord` compares text. It is still cheap: the table stores
//! each name's first eight bytes as a big-endian word, and identifiers rarely
//! agree that far, so the usual comparison is one integer compare with no
//! memory touched beyond the entry itself. Padding with zero bytes is exact
//! because a ruleset name never contains one.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::ops::Deref;
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// How many distinct names one process can hold. The shipped ruleset interns
/// a few thousand, counting every spec id and every effect key; the ceiling is
/// generous so that a mod, a scenario or a generated identifier cannot quietly
/// run into it. Exceeding it is a panic rather than a fallback, because a
/// silent second identity for the same text would break every equality test in
/// the engine.
const CAPACITY: usize = 1 << 15;

#[derive(Clone, Copy)]
struct Entry {
    text: &'static str,
    /// First eight bytes, big-endian, zero padded — an exact lexicographic
    /// prefix for NUL-free text.
    head: u64,
}

static SLOTS: [OnceLock<Entry>; CAPACITY] = [const { OnceLock::new() }; CAPACITY];

fn registry() -> &'static Mutex<HashMap<&'static str, u32>> {
    static REGISTRY: OnceLock<Mutex<HashMap<&'static str, u32>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn head_of(text: &str) -> u64 {
    let bytes = text.as_bytes();
    let mut head = [0u8; 8];
    let take = bytes.len().min(8);
    head[..take].copy_from_slice(&bytes[..take]);
    u64::from_be_bytes(head)
}

/// An interned ruleset identifier: a name by value, comparable in one
/// instruction and copyable without touching the allocator.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Name(u32);

impl Name {
    /// Intern `text`, returning the same `Name` every time for equal text.
    ///
    /// This takes a lock and hashes, so it belongs at the edges — loading a
    /// ruleset, reading a save, parsing a command line. Hot code should carry
    /// a `Name` it was given, or use [`name!`](crate::name) for a literal,
    /// which interns once and then costs an atomic load.
    pub fn new(text: &str) -> Name {
        let mut registry = registry().lock().expect("the name registry was poisoned");
        if let Some(id) = registry.get(text) {
            return Name(*id);
        }
        let id = registry.len();
        assert!(
            id < CAPACITY,
            "more than {CAPACITY} distinct names were interned; \
             raise name::CAPACITY if a ruleset really is this large"
        );
        let leaked: &'static str = Box::leak(text.to_string().into_boxed_str());
        let entry = Entry {
            text: leaked,
            head: head_of(leaked),
        };
        // Published before the id escapes the lock, so every later reader of
        // this slot sees a complete entry.
        SLOTS[id]
            .set(entry)
            .unwrap_or_else(|_| unreachable!("a name id is handed out once"));
        registry.insert(leaked, id as u32);
        Name(id as u32)
    }

    #[inline]
    fn entry(self) -> &'static Entry {
        SLOTS[self.0 as usize]
            .get()
            .expect("a Name always names an interned slot")
    }

    #[inline]
    pub fn as_str(self) -> &'static str {
        self.entry().text
    }

    /// The table slot behind this name. Callers that keep their own
    /// name-indexed tables use it; nothing about it is stable across runs, so
    /// it must never be serialized or compared for order.
    #[inline]
    pub fn id(self) -> u32 {
        self.0
    }

    /// How many names have been interned so far — the exclusive upper bound on
    /// any live [`Name::id`].
    pub fn interned() -> usize {
        registry().lock().expect("the name registry was poisoned").len()
    }
}

impl Deref for Name {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.entry().text
    }
}

impl AsRef<str> for Name {
    #[inline]
    fn as_ref(&self) -> &str {
        self.entry().text
    }
}

impl PartialOrd for Name {
    #[inline]
    fn partial_cmp(&self, other: &Name) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Name {
    #[inline]
    fn cmp(&self, other: &Name) -> Ordering {
        if self.0 == other.0 {
            return Ordering::Equal;
        }
        let (a, b) = (self.entry(), other.entry());
        match a.head.cmp(&b.head) {
            Ordering::Equal => a.text.cmp(b.text),
            ordered => ordered,
        }
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.entry().text)
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.entry().text, f)
    }
}

impl From<&str> for Name {
    #[inline]
    fn from(text: &str) -> Name {
        Name::new(text)
    }
}

impl From<String> for Name {
    #[inline]
    fn from(text: String) -> Name {
        Name::new(&text)
    }
}

impl From<&String> for Name {
    #[inline]
    fn from(text: &String) -> Name {
        Name::new(text)
    }
}

impl From<&Name> for Name {
    #[inline]
    fn from(name: &Name) -> Name {
        *name
    }
}

impl From<Name> for String {
    #[inline]
    fn from(name: Name) -> String {
        name.as_str().to_string()
    }
}

impl PartialEq<str> for Name {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.entry().text == other
    }
}

impl PartialEq<&str> for Name {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.entry().text == *other
    }
}

impl PartialEq<String> for Name {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.entry().text == other.as_str()
    }
}

impl PartialEq<Name> for str {
    #[inline]
    fn eq(&self, other: &Name) -> bool {
        self == other.entry().text
    }
}

impl PartialEq<Name> for &str {
    #[inline]
    fn eq(&self, other: &Name) -> bool {
        *self == other.entry().text
    }
}

impl PartialEq<Name> for String {
    #[inline]
    fn eq(&self, other: &Name) -> bool {
        self.as_str() == other.entry().text
    }
}

impl Serialize for Name {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.entry().text)
    }
}

impl<'de> Deserialize<'de> for Name {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Name, D::Error> {
        Ok(Name::new(&String::deserialize(deserializer)?))
    }
}

/// Intern a string literal once per call site.
///
/// The first evaluation takes the registry lock; every later one is an atomic
/// load, which is what makes a literal usable in a hot loop.
#[macro_export]
macro_rules! name {
    ($text:literal) => {{
        static ONCE: std::sync::OnceLock<$crate::name::Name> = std::sync::OnceLock::new();
        *ONCE.get_or_init(|| $crate::name::Name::new($text))
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_text_interns_to_one_name() {
        let a = Name::new("granary");
        let b = Name::new(&String::from("granary"));
        assert_eq!(a, b);
        assert_eq!(a.id(), b.id());
        assert_eq!(a.as_str(), "granary");
    }

    #[test]
    fn a_name_reads_as_its_text() {
        let name = Name::new("holy_site");
        assert_eq!(&*name, "holy_site");
        assert_eq!(name, "holy_site");
        assert!(name.starts_with("holy"));
        assert_eq!(format!("{name}"), "holy_site");
        assert_eq!(format!("{name:?}"), "\"holy_site\"");
    }

    /// Iteration order is outcome-bearing, so ordering must be the text's —
    /// never the order names happened to be interned in.
    #[test]
    fn names_order_like_their_text() {
        let mut interned = vec![
            Name::new("zebra_pen"),
            Name::new("aqueduct"),
            Name::new("aqueducts"),
            Name::new("aqueduct_of_a_very_long_name"),
            Name::new("barracks"),
        ];
        interned.sort();
        let mut text: Vec<String> = interned.iter().map(|n| n.to_string()).collect();
        let sorted = {
            let mut sorted = text.clone();
            sorted.sort();
            sorted
        };
        assert_eq!(text, sorted);
        text.clear();
    }

    #[test]
    fn long_names_that_share_a_prefix_still_order_by_text() {
        // The eight-byte head is equal for both, so the comparison has to fall
        // through to the full text.
        let a = Name::new("district_holy_site");
        let b = Name::new("district_campus");
        assert_eq!(a.cmp(&b), "district_holy_site".cmp("district_campus"));
    }

    #[test]
    fn a_name_serializes_as_a_plain_string() {
        let name = Name::new("chariot");
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"chariot\"");
        let back: Name = serde_json::from_str(&json).unwrap();
        assert_eq!(back, name);
    }
}
