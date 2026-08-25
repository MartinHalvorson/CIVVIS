//! Gene screen: price EVERY gene from ONE batch of random-genome games.
//!
//! A gene is one boolean behaviour flag on `AdvancedAi`; the genome a seat
//! plays is the set of genes it has on. The question asked of every gene is
//! the same: *does a seat win more with this gene on than off?*
//!
//! The design is deliberately the simplest one that answers it cleanly. Every
//! game seats `players` majors, and EVERY major seat draws its own genome
//! independently of every other seat and every other game: each screened gene
//! is on with probability one half, or three quarters for a gene the
//! deployment genome already ships on (`P_ON`, `P_DEFAULT_ON`). Nothing is
//! paired, mirrored or complemented — the operator asked for the randomness,
//! because a gene priced against a structured background can ride on the
//! genes it happens to be drawn beside, and here it cannot: averaged over
//! thousands of random backgrounds, each gene holds its own. Every game is
//! written as one row per major seat — genome beside outcome — so nothing is
//! ever replayed to ask a new question of it.
//!
//! What it prices per gene, with intervals, **per seat**:
//!
//! - win rate of the seats with the gene on against the seats with it off
//!   (the seat winning by any victory), Δ in points with a 95% CI and z. The
//!   seats of one game share a winner, so every error is clustered by game;
//! - the same for **score share** (a seat's score over all majors' scores), a
//!   continuous outcome that resolves an edge at a fraction of the seats a
//!   win/loss count needs;
//! - an OLS-adjusted Δ that regresses the seat outcome on every screened
//!   gene at once (plus an intercept), so a gene is not credited with the
//!   chance imbalance of its neighbours (printed once the seat count can
//!   support it);
//! - three newest-first, non-overlapping replication tranches of about
//!   `REPRO_WINDOW_SEATS` seats, whole games only, so an apparent win or loss
//!   is auditable for replication before it changes the genome;
//! - the incremental simulation cost of enabling the gene on one major seat.
//!
//! ⚠ It is a SCREEN. A hundred genes at |z| ≥ 2 flag ~4.5 of them by chance
//! alone; the table prints that number, the family-wise |z| bar, and the
//! smallest Δ the run could resolve at 80% power, so a `~` row is read as
//! "unresolved at this size" and never as "no effect". Two-factor interactions
//! are a separate scan (`--analyze --interactions`), estimated from the same
//! rows.
//!
//! ⚠ The genome carries the NATIVE bundle only: the engine repairs minus the
//! Firaxis-only flags (which read host state a CIVVIS board does not have and
//! are inert here), plus the production treatments and opt-ins. A host-only
//! flag screened here would measure noise and be reported as noise; it is
//! excluded rather than measured.
//!
//! Every batch stamps the binary that played it — the commit, whether that
//! tree was dirty, a sha256 of the executable, and a sha256 of the gene set
//! compiled into it — and pre-registers the size it was launched to play
//! (`Build`, `Batch`): `tools/genes.py` refuses a source whose gene set
//! does not match the code at the commit it names, and `--analyze` reports
//! actual against intended so a run that stopped early cannot read as a
//! finished screen.
//!
//! Usage:
//!   gene_screen [--games N] [--start-seed N] [--jobs N] [--out PATH]
//!               [--genes tag,tag,...] [--target-games N] [--append] [--quiet]
//!               [--p-on 0.25] [--p-default-on 0.75]
//!               PROBE ONLY, and a batch using any of them is not a ledger
//!               source: [--contested] [--contested-field lane,lane]
//!               [--contested-field-genes lanes|tag,tag]
//!               [--native-competitions] [--no-native-competitions]
//!               [--players N] [--turns N] [--width N] [--height N]
//!               [--city-states N] [--speed ID] [--map ID] [--victories a,b]
//!               [--victory-mask rotate:N] [--difficulty RUNG]
//!               [--difficulty-rotate king:1,emperor:2,immortal:1] [--rivals firaxis-mix]
//!               [--stock-civs]
//!   gene_screen --analyze PATH [PATH ...] [--json OUT] [--interactions]
//!               [--denial] [--top N] [--by-civ TAG]
//!   gene_screen --list
//!
//! ⭐ THE CONTESTED FIELD (`--contested`, 2026-08-24) is an ADDED MODE, never a
//! redefinition. The standard screen above is untouched and every recorded
//! column keeps comparing; a contested batch changes two legs of the header
//! (`contested_field`, `native_competitions`), `shape_of` reads it as `legacy`,
//! and `tools/genes.py` refuses it as a ledger source. What it adds is an
//! opponent: some major seats are PINNED to pursue a victory lane and are
//! not measured, the rest draw genomes as usual, and a drawn seat that
//! does not deny the pursuer loses to it. It exists because the fieldless
//! screen ends 0-1% of its games diplomatically while the live seat loses
//! 19.6% of its games that way — so every denial gene in the tables has been
//! priced against a field that never threatens the thing the gene denies.
//! `--analyze --denial` is the axis those genes are actually read on.
//!
//! ⭐ ONE SCREEN, and the bare defaults are it: six majors on 74x46 Continents
//! with nine city-states, Online speed to its own 250-turn clock, all six
//! victory lanes, every seat carrying its own drawn genome, civilizations
//! shuffled per map (`SCREEN_PLAYERS` and friends below). That is
//! Civilization VI's own six-player map row and the deployment shape, so the
//! ledger is read from the games the agent actually plays.
//! `gene_screen --games N --out rows.jsonl` is a screen; anything that moves
//! a leg of the profile is a probe, and `tools/genes.py` refuses it as
//! a source rather than mixing shapes.
//!
//! Files written by the earlier paired designs (every header before this one
//! says `foldover` or `prior`) still analyse: their rows are seats with a
//! genome and an outcome like any other, and the estimator here never needed
//! the pairing. Only the sampling changed.
use civvis::ai::{run_game, AdvancedAi, VictoryTarget};
use civvis::game::{Game, GameOptions, DIPLOMATIC_VICTORY_POINTS};
use civvis::rng::Rng;
use civvis::setup::{GameSpeed, MapScript};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::time::Instant;

/// ⭐ THE SCREEN. One shape, so two batches are comparable without an argument
/// about which regime read which gene (operator, 2026-08-22). Six majors on
/// Civilization VI's own six-player map row — `CIV6_MAP_SIZES` "small", 74x46
/// with nine city-states and three continents — at Online speed to its own
/// 250-turn clock, every victory lane live, every seat carrying its own drawn
/// genome.
///
/// Continents rather than Pangaea because Pangaea's religion wins were 48% of
/// all endings and drowned everything else; the same seeds on continents read
/// 28% religious, 18% culture and 52% at the clock, for +11.9% wall per game
/// (`docs/GENE_SCREEN.md`, "Why continents"). A batch that changes any leg of
/// this is a probe: `tools/genes.py` refuses it as a ledger source.
const SCREEN_PLAYERS: usize = 6;
const SCREEN_WIDTH: i32 = 74;
const SCREEN_HEIGHT: i32 = 46;
const SCREEN_CITY_STATES: usize = 9;
const SCREEN_MAP: MapScript = MapScript::Continents;

/// The five conditions a victory mask may close. Score is never among them:
/// it is the clock, the ending that turns a game the 250-turn limit reaches
/// into a decided one rather than a truncation.
const MASKABLE_LANES: [&str; 5] = [
    "science",
    "culture",
    "religious",
    "diplomatic",
    "domination",
];

/// ⭐ THE ROTATING VICTORY MASK (`--victory-mask rotate:N`, 2026-08-25).
///
/// The standard screen leaves all six lanes live in every game, and on this
/// board science and diplomatic victories land past the clock while religious
/// conversion decides most of the games that end early — so a gene for a lane
/// nobody finishes is priced on a board where its lane never decides
/// anything, and a gene for the lane that does decide is priced against a
/// board where that lane is always open. The live Civilization VI ladder
/// loses diplomatic 32 : culture 27 : religious 8 : science 4 : domination 1.
///
/// `rotate:N` closes N of the five real conditions per game, deterministically
/// from the game's seed: the C(5,N) N-subsets of the maskable lanes in one
/// fixed order, indexed by `seed % count`, so a consecutive seed window plays
/// every mask an equal number of times and every lane is closed in exactly
/// N/5 of the games. Score stays on in every game. Across the batch every
/// lane is live, which is why a rotating batch keeps the standard shape:
/// `victories` in the header is the batch-level set (all six), `victory_mask`
/// names the rotation, and each row carries the lanes its own game closed
/// (`victories_off`) so `--analyze` can read a lane gene with its lane open
/// against closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VictoryMask {
    /// How many real conditions each game closes.
    rotate: usize,
}

impl VictoryMask {
    fn parse(text: &str) -> Result<VictoryMask, String> {
        let Some(n) = text.strip_prefix("rotate:") else {
            return Err(format!(
                "unknown victory mask {text:?}; the form is rotate:N"
            ));
        };
        let rotate: usize = n
            .trim()
            .parse()
            .map_err(|_| format!("rotate:{n}: N must be a whole number"))?;
        if rotate == 0 {
            return Err("rotate:0 closes nothing; leave --victory-mask off instead".to_string());
        }
        Ok(VictoryMask { rotate })
    }

    fn id(self) -> String {
        format!("rotate:{}", self.rotate)
    }

    /// The lanes the rotation draws from: the maskable lanes `victories`
    /// leaves enabled at the batch level.
    fn lanes(self, victories: civvis::game::VictoryConditions) -> Vec<&'static str> {
        MASKABLE_LANES
            .iter()
            .copied()
            .filter(|lane| victories.is_enabled(lane))
            .collect()
    }

    /// Every mask the rotation cycles through, in one fixed order: the
    /// N-subsets of [`VictoryMask::lanes`], lexicographic by lane position,
    /// each subset sorted by name. Empty when fewer than N lanes are enabled.
    fn masks(self, victories: civvis::game::VictoryConditions) -> Vec<Vec<&'static str>> {
        combinations(&self.lanes(victories), self.rotate)
            .into_iter()
            .map(|mut mask| {
                mask.sort_unstable();
                mask
            })
            .collect()
    }

    /// The lanes closed in the game played on `seed`, sorted by name. The
    /// seed modulo the mask count: exactly balanced over any seed window
    /// that is a multiple of the count, never more than one game apart
    /// otherwise, and the same game reproduces the same mask.
    fn closed(self, seed: u64, victories: civvis::game::VictoryConditions) -> Vec<&'static str> {
        let masks = self.masks(victories);
        if masks.is_empty() {
            return Vec::new();
        }
        masks[(seed % masks.len() as u64) as usize].clone()
    }

    /// The conditions the game on `seed` is played with. Score is on
    /// whatever the batch-level set said: the mask must never leave a game
    /// nobody can win.
    fn apply(
        self,
        seed: u64,
        victories: civvis::game::VictoryConditions,
    ) -> civvis::game::VictoryConditions {
        let mut conditions = victories;
        for lane in self.closed(seed, victories) {
            match lane {
                "science" => conditions.science = false,
                "culture" => conditions.culture = false,
                "religious" => conditions.religious = false,
                "diplomatic" => conditions.diplomatic = false,
                "domination" => conditions.domination = false,
                _ => {}
            }
        }
        conditions.score = true;
        conditions
    }

    /// Games per mask over `games` consecutive seeds from `start_seed`, keyed
    /// by the closed lanes joined with `+` — what the header pre-registers.
    fn games_by_mask(
        self,
        start_seed: u64,
        games: usize,
        victories: civvis::game::VictoryConditions,
    ) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for game in 0..games {
            let key = mask_key(&self.closed(start_seed + game as u64, victories));
            *counts.entry(key).or_default() += 1;
        }
        counts
    }
}

/// The k-subsets of `items` in lexicographic order of position.
fn combinations(items: &[&'static str], k: usize) -> Vec<Vec<&'static str>> {
    if k == 0 {
        return vec![Vec::new()];
    }
    if items.len() < k {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, &first) in items.iter().enumerate() {
        for mut rest in combinations(&items[i + 1..], k - 1) {
            rest.insert(0, first);
            out.push(rest);
        }
    }
    out
}

/// ⭐ THE MAJORS' RUNG ROTATION (`--difficulty-rotate king:1,emperor:2,immortal:1`,
/// 2026-08-25).
///
/// The difficulty is the AI handicap every major seat plays with — the yield,
/// combat, experience and era-boost bonuses of `data/difficulties.json` —
/// and the live Civilization VI verification ladder plays Emperor and above,
/// while every screen so far played at the Prince default. A weighted list of
/// rungs is drawn per game from the seed: the weights are laid end to end
/// and the game on `seed` takes the rung at `seed % total`, so a consecutive
/// seed window plays each rung in exactly its share. The barbarian seat keeps
/// its own rung (`default_barbarian_difficulty`, Immortal) whatever the
/// majors draw. Rows carry the rung their game played, so `--analyze` can
/// read a gene per rung.
#[derive(Clone, Debug, PartialEq, Eq)]
struct DifficultyRotation {
    /// Rung and weight, in the order given.
    rungs: Vec<(String, usize)>,
}

impl DifficultyRotation {
    /// `king:1,emperor:2,immortal:1`; a rung without a weight counts once.
    fn parse(text: &str, known: &[&str]) -> Result<DifficultyRotation, String> {
        let mut rungs = Vec::new();
        for entry in text.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let (rung, weight) = match entry.split_once(':') {
                Some((rung, weight)) => (
                    rung.trim(),
                    weight
                        .trim()
                        .parse::<usize>()
                        .map_err(|_| format!("{entry}: the weight must be a whole number"))?,
                ),
                None => (entry, 1),
            };
            if !known.contains(&rung) {
                return Err(format!(
                    "unknown difficulty {rung:?}; choose from {known:?}"
                ));
            }
            if weight == 0 {
                return Err(format!(
                    "{entry}: a rung with weight 0 is never played; leave it out"
                ));
            }
            if rungs.iter().any(|(seen, _): &(String, usize)| seen == rung) {
                return Err(format!("{rung} is named twice"));
            }
            rungs.push((rung.to_string(), weight));
        }
        if rungs.len() < 2 {
            return Err(
                "a rotation names at least two rungs (a single rung is --difficulty)".to_string(),
            );
        }
        Ok(DifficultyRotation { rungs })
    }

    fn id(&self) -> String {
        self.rungs
            .iter()
            .map(|(rung, weight)| format!("{rung}:{weight}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn total(&self) -> usize {
        self.rungs.iter().map(|(_, weight)| weight).sum()
    }

    /// The rung the game on `seed` plays.
    fn rung(&self, seed: u64) -> &str {
        let mut slot = (seed % self.total() as u64) as usize;
        for (rung, weight) in &self.rungs {
            if slot < *weight {
                return rung;
            }
            slot -= weight;
        }
        &self.rungs[0].0
    }

    /// Games per rung over `games` consecutive seeds from `start_seed` — what
    /// the header pre-registers.
    fn games_by_rung(&self, start_seed: u64, games: usize) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for game in 0..games {
            *counts
                .entry(self.rung(start_seed + game as u64).to_string())
                .or_default() += 1;
        }
        counts
    }
}

/// One mask's name: its closed lanes joined with `+`, or `none`.
fn mask_key<S: AsRef<str>>(closed: &[S]) -> String {
    if closed.is_empty() {
        "none".to_string()
    } else {
        closed
            .iter()
            .map(AsRef::as_ref)
            .collect::<Vec<_>>()
            .join("+")
    }
}

/// ⭐ THE DRAW (operator, 2026-08-24). Every tournament genome STARTS FROM
/// THE DEFAULT GENOME: a gene the deployment ships on stays on with
/// probability three quarters (a one-in-four chance of turning off), and a
/// gene it ships off stays off with probability three quarters (a one-in-four
/// chance of turning on). The batch is deliberately biased toward the genome
/// people actually get — *"we want high level tournament competition and want
/// to select for genes that improve upon this performance, not some baseline
/// performance"* — while every gene still has both arms populated. A gene
/// that is on then picks its version: the top version 60% of the time, one of
/// the others 40% (`BEST_VERSION_SHARE`). Each seat's genome is drawn
/// independently of every other seat and every other game; nothing is paired
/// or complemented. (Until 2026-08-24 a default-off gene was on at one half.)
/// Percentage points per unit of standard error at 80% power, α = 0.05,
/// two-sided: 2.8 standard errors, and 100 to carry a proportion into points.
/// See [`resolving_power`].
const POWER_FACTOR: f64 = 280.0;

const P_ON: f64 = 0.25;
const P_DEFAULT_ON: f64 = 0.75;

/// Genes the screen holds at their default unless `--genes` asks for them by
/// name. This is a COST list, not a verdict: `joint-tactics` costs +27.3%
/// compute per enabled major seat (P10, 17,574 seat pairs) against ±1.6% for
/// every other gene, and screening it multiplies the whole batch by 2.52x —
/// a 10,000-game screen goes from 8.8 hours to 22.2.
///
/// ⚠ Do not read this as "it does nothing". Its win columns are +3/-4, inside
/// any band this instrument has printed, but P10 reads `share HELPS **` at
/// share z +3.84 — past that screen's family-wise bar. What it is owed is a
/// deliberate run (`--genes joint-tactics`), not a seat on every batch at
/// 2.5x the bill.
const HELD_UNLESS_ASKED: &[&str] = &["joint-tactics"];

/// ⭐ THE CONTESTED FIELD, and why it exists (2026-08-24).
///
/// The standard screen draws every seat's genome from one controller and reads
/// a gene as seats-on against seats-off. That is a good instrument for what it
/// measures and structurally blind to what actually beats us:
///
/// | ending | the standard screen | the live seat, against Firaxis' AI |
/// |---|---:|---:|
/// | diplomatic | 0–1% | **32 of 74 rival wins** — 19.6% of terminal games |
/// | culture | 11–18% | **27 of 74** |
/// | religious | 28–48% | 8 |
///
/// Diplomatic and culture are **83% of every early loss on the live seat** and
/// barely happen here, so every DENIAL gene in the tables has been priced
/// against a field that never threatens the thing the gene denies. That is not
/// a hypothesis: `congress_counter_leader`'s own field doc declines the
/// `world_leader` veto because the census found *"no diplomatic victory in 40
/// games. There is no headroom there to take"* — a census taken in a regime
/// where diplomatic victories do not happen at all.
///
/// The contested field is the answer: `field.len()` major seats are PINNED to
/// pursue a lane with [`AdvancedAi::retarget`], the drawn seats play the same
/// game, and a seat that does not deny the pursuer loses to it. The default
/// field is one of each lane the live seat actually loses to.
const CONTESTED_FIELD: &[&str] = &["diplomatic", "culture"];

/// ⭐ THE FIVE VICTORY-LANE OPT-INS, offered to a field seat by
/// `--contested-field-genes lanes` — and NOT the default, because it was tried
/// and measured worse.
///
/// The reasoning was good and the measurement disagreed.
/// `docs/VICTORY_GENES.md`'s four `lane-*` genes and
/// `competition-victory-points` are precisely the deciders that read the raced
/// lane — Great Person patronage, the policy deck, the Naturalist and the Rock
/// Bands, the space race, and the Diplomatic Victory Points a scored
/// competition pays — and all five ship **off**, so a pursuer seated with the
/// deployment genome alone looked like a pursuer with its lane behaviour
/// switched off.
///
/// Both fields were then run on the same board and the same seeds (92000000+,
/// one diplomatic and one culture pursuer, native competitions on; the
/// artifacts are `docs/gene_screens/2026-08-24-contested-field-*.json`):
///
/// | the field's own genome | games | held the board's top DVP | the most visiting tourists | won |
/// |---|---:|---:|---:|---:|
/// | the deployment genome | 27 | 8 | 7 | **0** |
/// | plus these five | 35 | 4 | 6 | **0** |
///
/// The lane genes made the pursuers hold their own lane's lead **less** often,
/// not more, and neither field ever converted. Two of the five are among the
/// genes this same batch priced on its measured seats, at −17.0 pp ± 6.9 and
/// −15.9 pp ± 7.0 (27 games, 108 seats) — a discovery-sized reading on a small
/// batch, but pointing the other way from the change. So the default stays the
/// deployment genome, which is also the rival the agent actually meets, and the
/// five are kept behind a flag for whoever wants to try again with n behind
/// them.
const CONTESTED_FIELD_GENES: &[&str] = &[
    "lane-great-people",
    "lane-policy-deck",
    "lane-culture-spending",
    "lane-space-race",
    "competition-victory-points",
];

/// One boolean treatment flag read as a gene.
///
/// `after_setup_on` is the flag's state after the seat is built (stock
/// production plus `enable_engine_repairs_universe`), `stock_on` its state on
/// the production agent alone, and `flip` the toggle that moves it away from
/// `after_setup_on`.
struct Gene {
    field: &'static str,
    tag: &'static str,
    /// On after `enable_engine_repairs_universe` — the genome's universe.
    after_setup_on: bool,
    stock_on: bool,
    /// On in the explicit deployment genome, or the universe state for a tag
    /// the native screen cannot price. See `gene_ledger.rs`.
    default_on: bool,
    /// The ledger's pooled on−off win difference in points — the gene's
    /// tracked wins over every screen that priced it (`win_diff_pp`).
    /// `None` for an unmeasured gene. Decides which version of a family is
    /// the best; see `best_version`.
    tracked_wins: Option<f64>,
    flip: fn(&mut AdvancedAi),
}

/// The deployment default for a tag: the ledger's say, else the universe.
fn ledger_default(tag: &str, universe_on: bool) -> bool {
    civvis::ai::ledger_default_on(tag).unwrap_or(universe_on)
}

/// Every gene this screen can vary, in the order the genome bits are written.
///
/// ⚠ Discovered from the registry (`src/ai/advanced/genes.rs`), never listed
/// by hand: every `screenable()` row — the engine repairs, the production
/// genes and the opt-ins — in registry order. A host-only gene reads Civ 6
/// state a native board does not have and is excluded rather than measured.
fn gene_table() -> Vec<Gene> {
    civvis::ai::screenable_genes()
        .into_iter()
        .map(|gene| Gene {
            field: gene.field,
            tag: gene.tag,
            after_setup_on: gene.universe_on(),
            stock_on: gene.stock_on(),
            default_on: ledger_default(gene.tag, gene.universe_on()),
            tracked_wins: civvis::ai::ledger_verdict(gene.tag).and_then(|row| row.win_diff_pp),
            flip: if gene.universe_on() {
                gene.disable
            } else {
                gene.enable
            },
        })
        .collect()
}

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

fn is_zero_u8(value: &u8) -> bool {
    *value == 0
}

/// ⭐ THE RIVAL MIX (`--rivals firaxis-mix`, 2026-08-25).
///
/// The standard screen's opposition is the other drawn genomes: every effect
/// is averaged over random opposing genomes drawn from the same controller,
/// which is the right instrument for "does this gene help against the
/// ecosystem" and a blind one for "does it help against a rival that is not
/// us". With the mix, ONE major seat per game — its chair rotating with the
/// game index like a contested pin, so no position is always the rival —
/// plays a fixed opponent instead of a drawn genome, and the kind of opponent
/// rotates per game from the seed:
///
/// - `legacy` — [`AdvancedAi::legacy`], the frozen anchor;
/// - `firaxis-mix` — the deployment genome, [`AdvancedAi::new`], retargeted
///   at one victory lane drawn in the shares the live Civilization VI ladder
///   actually loses to: diplomatic 32 : culture 27 : religious 8 : science 4 :
///   domination 1 ([`FIRAXIS_MIX_LANES`], the Hall of Fame census in
///   `docs/eval/2026-08-18-we-screen-against-a-religion-game-and-lose-a-diplomacy-game.md`);
/// - `random` — a genome with every screened gene on at one half, drawn like
///   any seat's.
///
/// The rival seat is NOT measured: its row is written with `kind: "rival"`
/// so every estimator — which reads `kind == "game"` — skips it, and every
/// measured row of the game says `rival_mix: "measured"`. `--analyze` then
/// reads every gene past the family-wise bar on the three kinds apart and
/// says whether its sign agrees.
const RIVAL_KINDS: [&str; 3] = ["legacy", "firaxis-mix", "random"];

/// The lanes a `firaxis-mix` rival pursues, weighted by the live ladder's
/// losses (diplomatic 32 : culture 27 : religious 8 : science 4 : domination 1).
const FIRAXIS_MIX_LANES: [(VictoryTarget, u64); 5] = [
    (VictoryTarget::Diplomacy, 32),
    (VictoryTarget::Culture, 27),
    (VictoryTarget::Religion, 8),
    (VictoryTarget::Science, 4),
    (VictoryTarget::Domination, 1),
];

/// The kind of rival the game on `seed` seats: the three kinds in turn.
fn rival_kind(seed: u64) -> &'static str {
    RIVAL_KINDS[(seed % RIVAL_KINDS.len() as u64) as usize]
}

/// The major (by index among the majors) that plays the rival in `game`:
/// rotates with the game index, for the reason [`pinned_seats`] gives.
fn rival_index(players: usize, game: usize) -> usize {
    if players == 0 {
        0
    } else {
        game % players
    }
}

/// The lane a `firaxis-mix` rival on `seed` pursues, drawn from
/// [`FIRAXIS_MIX_LANES`] in their weights: the weights laid end to end and
/// indexed by `seed / 3` (the kind took `seed % 3`), so 72 consecutive
/// firaxis-mix games play each lane exactly its share.
fn firaxis_mix_target(seed: u64) -> VictoryTarget {
    let total: u64 = FIRAXIS_MIX_LANES.iter().map(|(_, weight)| weight).sum();
    let mut slot = (seed / RIVAL_KINDS.len() as u64) % total;
    for &(target, weight) in &FIRAXIS_MIX_LANES {
        if slot < weight {
            return target;
        }
        slot -= weight;
    }
    FIRAXIS_MIX_LANES[0].0
}

/// Games per rival kind over `games` consecutive seeds from `start_seed`.
fn rival_games(start_seed: u64, games: usize) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for game in 0..games {
        *counts
            .entry(rival_kind(start_seed + game as u64).to_string())
            .or_default() += 1;
    }
    counts
}

/// The rival seat itself. `random_genome` is the drawn genome a `random`
/// rival plays; the other kinds ignore it.
fn rival_seat(kind: &str, seed: u64, genes: &[Gene], random_genome: &[bool]) -> AdvancedAi {
    match kind {
        "legacy" => AdvancedAi::legacy(),
        "firaxis-mix" => {
            let mut ai = AdvancedAi::new();
            ai.retarget(firaxis_mix_target(seed));
            ai
        }
        _ => seat_with_genome(genes, random_genome),
    }
}

/// One seat of one screened game, written to the JSONL file and read back by
/// `--analyze`. A game yields one row per major seat.
#[derive(Clone, Serialize, Deserialize, Debug)]
struct Row {
    /// `game` for a screened seat. (Files written by the earlier paired designs
    /// also hold `anchor` rows; those are skipped by every estimate here.)
    kind: String,
    /// The game's index in its batch: seed `start_seed + game`.
    #[serde(default)]
    game: usize,
    /// ⚠ Legacy only. Files written by the paired designs name a row by the
    /// map pair it came from and the arm it played; seats of one game share
    /// `(seed, arm)`. New files write neither.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pair: usize,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    arm: u8,
    seed: u64,
    seat: usize,
    /// One char per gene in header order: `1` on, `0` off.
    genome: String,
    win: bool,
    winner: Option<usize>,
    victory: String,
    turn: u32,
    score: i64,
    /// This seat's score over the sum of every major's score.
    score_share: f64,
    /// 1 = highest score among majors.
    rank: usize,
    cities: usize,
    alive: bool,
    /// Whole-game wall seconds; every seat of one game carries the same value.
    secs: f64,
    /// ★ WHY THE SEAT LOST, NOT ONLY THAT IT DID. The first native runs said
    /// two thirds of games ended by RELIGIOUS conversion at a median of turn
    /// 149 — the single largest failure mode on the board — and the rows could
    /// not say one thing about how the losing seat stood in that race. These
    /// fields are the cheapest possible answer, all end-of-game reads, and
    /// they turn "lost to religion" into a diagnosis: did it found a faith at
    /// all, how many of its own cities were flying a foreign one at the end,
    /// was the faith banked rather than spent, and had it ever unlocked the
    /// Inquisitor.
    ///
    /// `#[serde(default)]` on every one of them, so a file written before they
    /// existed still analyses.
    #[serde(default)]
    founded_religion: bool,
    /// Our cities whose MAJORITY religion is somebody else's faith.
    #[serde(default)]
    foreign_faith_cities: usize,
    /// Faith still in the bank at the end. A seat losing the conversion race
    /// with a thousand faith unspent is a spending defect, not a race it could
    /// not have run.
    #[serde(default)]
    faith: f64,
    /// Whether an Inquisition was ever launched — the gate that has to fall
    /// before an Inquisitor can be bought at all.
    #[serde(default)]
    inquisition: bool,
    #[serde(default)]
    techs: usize,
    /// ★ Wonders standing in this seat's cities at the end. `Game::score_parts`
    /// awards **15 points a wonder** — the densest line of a score tally that
    /// decides three quarters of the games this screen plays — and the
    /// `Item::Wonder` arm of `production_value` refuses every wonder outside a
    /// Culture plan, a Score target or an untargeted Egypt or China. Whether
    /// that refusal actually costs the agent a wonder was argued from prose for
    /// months and never read out of a batch; this field is the reading. ⚠ It is
    /// a census, not a lever: within one arm wonders track score share and so
    /// do cities, and only an on−off contrast says which way it runs.
    #[serde(default)]
    wonders: usize,
    #[serde(default)]
    military: f64,
    /// The civilization this seat played. Empty in files written before the
    /// field existed.
    #[serde(default)]
    civ: String,
    /// The war this seat chose and what it took: surprise wars the
    /// `opportunistic-war` gene opened (`raid_wars`), Settlers and Builders
    /// captured by entering their tile (`captured:*`), tiles and district
    /// layers pillaged (`pillages`). A gene that fires in no game measures
    /// nothing — these say whether it fired.
    #[serde(default)]
    raid_wars: i64,
    /// The `city-campaign` gene's plans drawn, wars found open under a plan,
    /// and planned cities taken, and the `campaign-pillage` gene's pillages
    /// (`campaign:*`, 2026-08-24).
    #[serde(default)]
    campaign_plans: i64,
    #[serde(default)]
    campaign_wars: i64,
    #[serde(default)]
    campaign_captures: i64,
    #[serde(default)]
    campaign_pillages: i64,
    #[serde(default)]
    settlers_captured: i64,
    #[serde(default)]
    builders_captured: i64,
    /// Religious units of this seat condemned by the barbarian seat
    /// (`religious_lost_to_barbarians`, 2026-08-24): what the barbarian
    /// heretic hunt takes, and what `missionary-evades-raiders` keeps.
    #[serde(default)]
    religious_lost: i64,
    #[serde(default)]
    pillages: i64,
    /// Settlers counted as prizes at the raids' declarations.
    #[serde(default)]
    raid_settler_prizes: i64,
    /// ⭐ WHERE THIS SEAT STOOD IN THE TWO LANES THAT ACTUALLY BEAT US, and
    /// where the best rival stood.
    ///
    /// 83% of every early loss on the live Civilization VI seat is diplomatic
    /// or culture (`docs/FIDELITY.md`), and until the contested field existed
    /// the screen could not say whether anybody on the board was even
    /// *running* those races. `dvp` is Diplomatic Victory Points against the
    /// twenty a diplomatic victory needs; `tourists` is the visiting tourists
    /// a culture victory is decided on. The `rival_` pair is the highest
    /// either reached on any OTHER major seat, so a measured seat's row proves
    /// the pursuit was real without the pursuer's own row having to exist.
    ///
    /// `#[serde(default)]`, so a file written before the contested field still
    /// analyses; zero is exactly what its absence meant.
    #[serde(default)]
    dvp: i64,
    #[serde(default)]
    rival_dvp: i64,
    #[serde(default)]
    tourists: i64,
    #[serde(default)]
    rival_tourists: i64,
    /// This seat's DOMESTIC tourists — the bar a culture pursuer has to clear,
    /// because `check_culture_victory` asks for more visiting tourists than the
    /// best rival's domestic total and there is no fixed threshold to quote.
    #[serde(default)]
    domestic: i64,
    /// ⭐ The victory lanes the rotating mask CLOSED in this seat's game,
    /// sorted by name (`--victory-mask rotate:N`). Every seat of one game
    /// carries the same list. Empty in an unmasked game and in every file
    /// written before the mask existed, and not written at all then, so a
    /// standard row is byte for byte what it was.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    victories_off: Vec<String>,
    /// ⭐ The difficulty rung the majors played in this seat's game
    /// (`--difficulty`, or the draw of `--difficulty-rotate`). Every seat of
    /// one game carries the same rung. Empty in every file written before
    /// 2026-08-25, which means the Prince default those batches played.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    difficulty: String,
    /// ⭐ THE RIVAL MIX: `measured` on a measured seat's row; on the fixed
    /// opponent's own row (`kind: "rival"`, skipped by every estimator) the
    /// kind it played — `legacy`, `firaxis-mix` or `random`. Empty in a batch
    /// without the mix and in every file written before 2026-08-25.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    rival_mix: String,
    /// The lane a `firaxis-mix` rival pursued, on its own row only.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    rival_target: String,
}

/// The game a row belongs to. Seats of one game share a winner and a timing,
/// so every standard error here is clustered on this key. A screened game is
/// its seed; a legacy paired file plays two games on one seed and tells them
/// apart by arm.
type GameKey = (u64, u8);

impl Row {
    fn game_key(&self) -> GameKey {
        (self.seed, self.arm)
    }

    fn bits(&self) -> Vec<bool> {
        self.genome.chars().map(|c| c == '1').collect()
    }
}

/// ⭐ THE BINARY A SCREEN WAS PLAYED BY, stamped into the batch's own header.
///
/// ⚠⚠ NOTHING CHECKED THIS, AND IT HAS COST THE PROJECT THREE TIMES.
/// `docs/gene_ledger.json` records where a column came from; it never recorded
/// which binary produced it, so a screen could price code that no longer
/// existed and read as current:
///
/// - **P10, 2026-08-22.** #2266 culled ten genes. P10's simulation binary was
///   built 1h43m before that merge, so the batch was already in flight and
///   published a **+63** column for `holy-lane-parity` after the gene's code
///   was gone. The reading was real; the gene came back (#2299) and confirmed
///   directly at +99, z +4.05 (#2307). The project got the right answer from a
///   careful reader, not from a gate.
/// - **#2307's own write-up** had to state its source commit and its binary's
///   SHA-256 in prose, because the artefact had nowhere to put them.
/// - **2026-08-23.** The first standard-shape screen re-priced `barbarian-hunt`
///   from the legacy −1.73 pp to +0.20 pp (z +0.65) while a sibling change was
///   minutes from deleting that gene on the legacy reading — which would have
///   made a brand-new screen a source pricing a gene the code no longer had.
///
/// `genes_sha256` is the load-bearing field, and it is the one that cannot go
/// stale: it is hashed from `gene_table()` **as compiled into this binary**,
/// never read back from a file, an environment variable, or a working tree.
/// A commit can be misreported; the gene set of the running code cannot. So
/// `tools/genes.py` re-derives the gene tags at the commit a source
/// claims and refuses that source when the two disagree **in either
/// direction** — a gene priced here and absent there, or a gene present there
/// and never compiled in here. The second is what an unmeasured gene quietly
/// looks like.
#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq, Eq)]
struct Build {
    /// The commit this binary was built from, or empty when nothing could say
    /// so honestly. Empty is a refusal at the ledger, never a guess.
    commit: String,
    /// Where `commit` came from, so a weak answer cannot pass for a strong
    /// one: `env` (`CIVVIS_COMMIT`, what every supervisor already sets),
    /// `binary-name` (a promoted `civvis-<40-hex>` executable), `build-tree`
    /// (Git in the directory this crate was compiled from), or `unstamped`.
    commit_source: String,
    /// Whether the tree that answered had uncommitted changes to anything the
    /// games are played out of. A dirty tree is not a commit, so the ledger
    /// refuses it: the code that played the games is not recoverable from any
    /// revision.
    ///
    /// Scoped to `BUILD_INPUTS` rather than the whole worktree, and untracked
    /// files are not counted. Both narrowings are the same argument: an
    /// edited analysis tool or an untracked scratch file cannot change how a
    /// game plays out, while anything that can — a source file, the manifest,
    /// the lockfile, a data table — is a tracked change under one of those
    /// paths. A guard that fired on every open editor buffer would be turned
    /// off, and a guard that is turned off measures nothing.
    dirty: bool,
    /// ⭐ The gene set, hashed: sha256 over the tags this binary can vary, in
    /// header order, one per line. See `gene_set_fingerprint`.
    genes_sha256: String,
    /// sha256 of this executable's own bytes — the exact artefact. This is
    /// what `docs/eval/2026-08-22-standard-gene-screen-23622-paired-seats.md`
    /// had to write out by hand as "release binary SHA-256".
    binary_sha256: String,
}

/// ⭐ WHAT THE BATCH WAS LAUNCHED TO PLAY, declared before its first game.
///
/// P10 "ended early at the operator's request" at 5,858 of a planned 10,000
/// games. Stopping early is legitimate and stays legitimate — the operator
/// does it deliberately — but with no pre-registered target the analysis
/// tranche is chosen after seeing the data, and the artefact cannot tell a
/// completed screen from a truncated one. The target is therefore written into
/// the header before the first game finishes, and every `--analyze` reports
/// actual against intended whether or not anyone asks.
#[derive(Clone, Serialize, Deserialize, Debug, Default, PartialEq, Eq)]
struct Batch {
    /// Games this segment was launched to play — `--target-games`, else
    /// `--games`. Zero in a file written before pre-registration existed.
    #[serde(default)]
    target_games: usize,
    /// The seat observations that target implies: `target_games × players`.
    /// This is the unit the analysis counts, so intended and actual are the
    /// same currency and their ratio means something.
    #[serde(default)]
    target_seats: usize,
    /// ⚠ Legacy only: the paired designs pre-registered map PAIRS and the
    /// matched seat COMPARISONS (one on-seat against one off-seat) they
    /// implied. Read back so an old file still reports actual against
    /// intended, in seats: a comparison is two seats. New files write neither.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    target_pairs: usize,
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    target_comparisons: usize,
    /// The seed window the target reserves, inclusive. A run that stops early
    /// leaves the tail of this window unplayed, which is exactly the gap the
    /// analysis prints.
    seed_first: u64,
    seed_last: u64,
}

impl Batch {
    /// The seats this batch was launched to observe, in whichever currency the
    /// file pre-registered. Zero when nothing was pre-registered.
    fn intended_seats(&self) -> usize {
        if self.target_seats > 0 {
            self.target_seats
        } else {
            self.target_comparisons * 2
        }
    }

    fn pre_registered(&self) -> bool {
        self.intended_seats() > 0
    }
}

/// The fingerprint of the gene set a binary can vary: sha256 over the tags in
/// header order, one per line, each newline-terminated.
///
/// `tools/genes.py::gene_set_fingerprint` computes the same string from
/// the registry, `src/ai/advanced/genes.rs` — every `screenable()` row in
/// order — **at any commit** (and from the three tables that preceded it at
/// older commits), which is how a screen is checked against the code it
/// claims to have played.
/// `the_gene_table_is_exactly_what_the_ledger_re_derives_from_the_tables`
/// holds the two rules together against the compiled table itself.
fn gene_set_fingerprint(genes: &[Gene]) -> String {
    let mut text = String::new();
    for gene in genes {
        text.push_str(gene.tag);
        text.push('\n');
    }
    sha256_hex(text.as_bytes())
}

/// Run `git` in `dir`, or `None` when it cannot answer.
fn git(dir: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// A promoted `civvis-<40-hex-sha>` executable names its own revision, exactly
/// as `server.rs` reads it. Kept in step with that reader by
/// `a_promoted_binary_names_its_own_revision`.
fn promoted_binary_commit(name: &str) -> Option<String> {
    let commit = name.strip_prefix("civvis-")?;
    (commit.len() == 40 && commit.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| commit.to_owned())
}

/// Seconds since the epoch that this executable's bytes were last written.
fn binary_mtime_secs(path: &std::path::Path) -> Option<u64> {
    path.metadata()
        .ok()?
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|since| since.as_secs())
}

/// Everything a played game comes out of: the crate's sources, what pins its
/// build, and the tables it reads. A tracked change under any of these makes
/// the revision a lie; a change anywhere else — an analysis tool, a document —
/// cannot reach a game.
const BUILD_INPUTS: &[&str] = &["src", "Cargo.toml", "Cargo.lock", "build.rs", "data"];

/// Stamp the running binary: its gene set, its own bytes, and the revision it
/// was built from.
///
/// ⚠ THE ORDER IS THE POINT. `CIVVIS_COMMIT` is what every supervisor in this
/// repository already sets (`server.rs`, `spectator_supervisor.py`,
/// `simloop/iterate.sh`) and is the launcher's own word; a promoted
/// `civvis-<sha>` name is the artefact's word; the build tree is a *guess*,
/// because a worktree moves on after a binary is built. That guess is refused
/// outright when the tree's HEAD is newer than this executable's bytes — a
/// stale revision reported confidently is the failure this whole struct
/// exists to stop, and `build.rs` deliberately keeps the revision out of
/// Cargo's inputs (#892) so it cannot be baked in at compile time instead.
fn stamp_build(genes: &[Gene]) -> Build {
    let executable = std::env::current_exe().ok();
    let binary_sha256 = executable
        .as_deref()
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| sha256_hex(&bytes))
        .unwrap_or_default();
    let mut build = Build {
        commit: String::new(),
        commit_source: "unstamped".to_string(),
        dirty: false,
        genes_sha256: gene_set_fingerprint(genes),
        binary_sha256,
    };
    if let Some(commit) = std::env::var("CIVVIS_COMMIT")
        .ok()
        .filter(|commit| !commit.is_empty())
    {
        build.commit = commit;
        build.commit_source = "env".to_string();
        return build;
    }
    if let Some(commit) = executable
        .as_deref()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .and_then(promoted_binary_commit)
    {
        build.commit = commit;
        build.commit_source = "binary-name".to_string();
        return build;
    }
    let tree = env!("CARGO_MANIFEST_DIR");
    let Some(head) = git(tree, &["rev-parse", "HEAD"]) else {
        return build;
    };
    // ⚠ Against the last commit that touched a BUILD INPUT, not against HEAD.
    // This fleet lands a hundred merges a day and most of them are tools and
    // documents; comparing with HEAD would call a perfectly good binary stale
    // every time somebody committed a Python file, and a guard that cries wolf
    // is a guard that gets waved through. What matters is whether the tree
    // changed the code the games are played out of since this executable was
    // linked.
    let mut last_touch = vec!["log", "-1", "--format=%ct", "--"];
    last_touch.extend_from_slice(BUILD_INPUTS);
    let code_time: Option<u64> = git(tree, &last_touch).and_then(|at| at.parse().ok());
    let built = executable.as_deref().and_then(binary_mtime_secs);
    if let (Some(code_time), Some(built)) = (code_time, built) {
        if built < code_time {
            // The tree changed the engine after this binary was linked, so its
            // HEAD is not what played the games. Say nothing rather than
            // something false: the ledger refuses an unstamped source, and the
            // fix is a rebuild or an explicit `CIVVIS_COMMIT`.
            //
            // ⚠ The residual this cannot see is a checkout *backwards* — a
            // bisect that moves the tree to an older commit whose code the
            // binary does not have. The gene-set fingerprint is what catches
            // that, whenever the gene set is one of the things that differ.
            build.commit_source = "unstamped-tree-moved".to_string();
            return build;
        }
    }
    build.commit = head;
    build.commit_source = "build-tree".to_string();
    let mut status = vec!["status", "--porcelain", "--untracked-files=no", "--"];
    status.extend_from_slice(BUILD_INPUTS);
    build.dirty = git(tree, &status)
        .map(|changed| !changed.is_empty())
        .unwrap_or(true);
    build
}

/// SHA-256 (FIPS 180-4) of `bytes`, lowercase hex.
///
/// Written out rather than pulled in: the crate's dependency list is three
/// entries wide on purpose, and a hash the fleet's Python side already has in
/// `hashlib` does not justify a fourth. `sha256_matches_the_published_vectors`
/// pins it against the standard's own test vectors, and the Python side is
/// held to the same answers.
fn sha256_hex(bytes: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = bytes.to_vec();
    let bit_length = (bytes.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());
    // The padding above makes the length an exact multiple of 64, so both
    // remainders are empty by construction.
    let (blocks, _) = message.as_chunks::<64>();
    for chunk in blocks {
        let mut w = [0u32; 64];
        let (words, _) = chunk.as_chunks::<4>();
        for (index, word) in words.iter().enumerate() {
            w[index] = u32::from_be_bytes(*word);
        }
        for index in 16..64 {
            let s0 = w[index - 15].rotate_right(7)
                ^ w[index - 15].rotate_right(18)
                ^ (w[index - 15] >> 3);
            let s1 = w[index - 2].rotate_right(17)
                ^ w[index - 2].rotate_right(19)
                ^ (w[index - 2] >> 10);
            w[index] = w[index - 16]
                .wrapping_add(s0)
                .wrapping_add(w[index - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(w[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }
    state.iter().map(|word| format!("{word:08x}")).collect()
}

/// The first line of the JSONL file: the gene order every genome string is
/// written in, and the profile the games were played at.
#[derive(Clone, Serialize, Deserialize, Debug)]
struct Header {
    kind: String,
    genes: Vec<String>,
    screened: Vec<String>,
    players: usize,
    width: i32,
    height: i32,
    turns: u32,
    city_states: usize,
    speed: String,
    map: String,
    /// What an un-screened gene is held at. Always `best` — the deployment
    /// genome — for a screen; the earlier designs also had `repairs` and
    /// `stock` probes, which old files may still say.
    baseline: String,
    start_seed: u64,
    /// Whether every seat's civilization was shuffled per map instead of the
    /// stock order (Rome, Egypt, Greece, China, … by seat). Absent in files
    /// written before the flag existed, which means `false`.
    #[serde(default)]
    randomize_civs: bool,
    /// The victory lanes left enabled, comma separated. Absent in files written
    /// before the flag existed, which means all six.
    #[serde(default)]
    victories: String,
    /// Every major seat carries its own drawn genome, so each game yields one
    /// observation per major. Always true for a screen; the classic
    /// one-treated-seat files say `false` (or nothing, which is the same).
    #[serde(default)]
    all_seats: bool,
    /// `independent` — every seat's genome drawn on its own. Files written by
    /// the earlier paired designs say `foldover` or `prior`; absent means
    /// `foldover`. They analyse the same way: rows are rows.
    #[serde(default = "foldover")]
    design: String,
    /// Each gene's on-probability in header order: `P_ON`, `P_DEFAULT_ON`, or
    /// 0/1 for a gene held at its default. A legacy foldover file leaves it
    /// empty, which the analysis reads as one half for every screened gene.
    #[serde(default)]
    prior: Vec<f64>,
    /// The two draw probabilities the batch ran with. Zero in a legacy file.
    #[serde(default)]
    p_on: f64,
    #[serde(default)]
    p_default_on: f64,
    /// ⭐ The gene FAMILIES: every versioned gene with its versions, in
    /// version order, base first (`war-economy`, `war-economy-2`, …). A seat
    /// plays at most one version of a family, so the screen can say whether
    /// the improvement improved. Re-derived from the tags by `--analyze`;
    /// recorded so a reader of the file sees it too. Empty when no gene is
    /// versioned, and in every legacy file.
    #[serde(default)]
    families: Vec<Vec<String>>,
    /// ⭐ THE CONTESTED FIELD: the victory lanes rival seats were PINNED to
    /// pursue, comma separated, or empty for the standard fieldless screen.
    /// One major seat is pinned per entry and the pinned positions rotate with
    /// the game index.
    ///
    /// A batch with a field is a PROBE by construction: `shape_of` reads it as
    /// `legacy` and `tools/genes.py` refuses it as a ledger source. That is
    /// the whole safety property — a gene priced against a board racing for a
    /// diplomatic victory is not priced against the board every recorded
    /// column was taken on, and the two must never pool.
    ///
    /// ⚠ NAMED `contested_field`, not `field`. Every header the retired paired
    /// designs wrote already carries a `field` — the name of the agent the
    /// treated seat played against (`"advanced"`) — and reusing it would have
    /// made nine historical ledger records read as contested boards. The
    /// collision was caught by `tools/genes.py check` reporting drift on the
    /// ledger's own history, which is exactly what that gate is for.
    #[serde(default)]
    contested_field: String,
    /// Whether this batch ran CIVVIS' own scored competitions
    /// (`Game::native_competitions`). Off in the standard screen and in every
    /// file written before 2026-08-24. It is the only native route to
    /// Diplomatic Victory Points that recurs through the second half of a
    /// game, so the contested field turns it on — which makes it another leg
    /// of the shape and another reason such a batch is not a ledger source.
    #[serde(default)]
    native_competitions: bool,
    /// The genes a FIELD seat played on top of the deployment genome, comma
    /// separated (`CONTESTED_FIELD_GENES`). Provenance, not a shape leg — a
    /// batch that has a field is already refused as a source by
    /// `contested_field` — but a reader of the file cannot reconstruct what the
    /// pursuers were without it. Empty in a fieldless batch and in every file
    /// written before 2026-08-24.
    #[serde(default)]
    contested_field_genes: String,
    /// ⭐ THE ROTATING VICTORY MASK: `rotate:N` when each game closed N of
    /// the five real conditions from its seed ([`VictoryMask`]), empty when
    /// every game played the batch-level set. NOT a shape leg: `victories`
    /// above stays the batch-level set and every lane is live across a
    /// rotating batch, so `shape_of` reads it as the standard screen. Absent
    /// in every file written before 2026-08-25.
    #[serde(default)]
    victory_mask: String,
    /// The games this segment pre-registered per mask, keyed by the closed
    /// lanes joined with `+` — derived from the seed window before the first
    /// game, so a stopped batch's intended split is on record. Empty when
    /// there is no mask.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    victory_mask_games: BTreeMap<String, usize>,
    /// ⭐ The majors' difficulty rung when every game played one
    /// (`--difficulty`), or empty under a rotation. Absent in every file
    /// written before 2026-08-25, which means the Prince default. Provenance,
    /// not a shape leg: `tools/genes.py` records it on the source.
    #[serde(default)]
    difficulty: String,
    /// ⭐ THE RUNG ROTATION, `king:1,emperor:2,immortal:1`
    /// ([`DifficultyRotation`]), or empty when every game played `difficulty`.
    #[serde(default)]
    difficulty_rotate: String,
    /// The games this segment pre-registered per rung, from its seed window,
    /// before the first game. Empty without a rotation.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    difficulty_games: BTreeMap<String, usize>,
    /// ⭐ THE RIVAL MIX: `firaxis-mix` when one major seat per game played a
    /// fixed opponent ([`RIVAL_KINDS`], rotating per game from the seed),
    /// empty when every major drew a genome. Provenance on the source, not a
    /// shape leg. Absent in every file written before 2026-08-25.
    #[serde(default)]
    rivals: String,
    /// The games this segment pre-registered per rival kind. Empty without
    /// the mix.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    rival_games: BTreeMap<String, usize>,
    /// ⭐ The binary that played these games. Absent in files written before
    /// 2026-08-23, which `tools/genes.py` grandfathers as history and
    /// marks `pre-fingerprint` rather than accepting silently.
    #[serde(default)]
    build: Build,
    /// ⭐ What the batch was launched to play, declared before its first game.
    /// Absent in files written before 2026-08-23, where actual cannot be read
    /// against intended at all.
    #[serde(default)]
    batch: Batch,
}

fn foldover() -> String {
    "foldover".to_string()
}

/// ⭐ VERSIONED GENES. An improvement to a gene is a NEW gene, `<base>-<n>`
/// (`war-economy-2`), with its own flag, toggles and code path, screened
/// beside the original under the same rules. The original keeps its tag and
/// its history and is version one. Versions form a FAMILY: a seat plays at
/// most one of them — off, the original, or one improvement — so every
/// version is priced against the same "off" and, head to head, against the
/// version it claims to improve (operator, 2026-08-23).
///
/// Discovered from the tags, never listed: a tag `<base>-<n>` with `n ≥ 2`
/// whose `<base>` is itself a gene is that gene's version `n`. Returned as
/// header indices, one list per family, base first then ascending versions.
fn families_of(tags: &[String]) -> Vec<Vec<usize>> {
    let index_of = |tag: &str| tags.iter().position(|candidate| candidate == tag);
    let mut found: BTreeMap<usize, Vec<(u32, usize)>> = BTreeMap::new();
    for (i, tag) in tags.iter().enumerate() {
        let Some((base, version)) = tag.rsplit_once('-') else {
            continue;
        };
        let Ok(version) = version.parse::<u32>() else {
            continue;
        };
        if version < 2 {
            continue;
        }
        if let Some(base_index) = index_of(base) {
            found.entry(base_index).or_default().push((version, i));
        }
    }
    found
        .into_iter()
        .map(|(base_index, mut versions)| {
            versions.sort_unstable();
            std::iter::once(base_index)
                .chain(versions.into_iter().map(|(_, index)| index))
                .collect()
        })
        .collect()
}

impl Header {
    /// The families among this file's genes, as header indices.
    fn families(&self) -> Vec<Vec<usize>> {
        families_of(&self.genes)
    }

    /// Which header genes were screened, as a mask over the gene order.
    fn screened_mask(&self) -> Vec<bool> {
        self.genes
            .iter()
            .map(|gene| self.screened.contains(gene))
            .collect()
    }

    /// The on-probability each gene was drawn with, in header order. A legacy
    /// file without the field drew every screened gene at one half.
    fn on_probabilities(&self) -> Vec<f64> {
        if self.prior.len() == self.genes.len() {
            return self.prior.clone();
        }
        self.screened_mask()
            .into_iter()
            .map(|screened| if screened { 0.5 } else { 0.0 })
            .collect()
    }
}

fn number(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn real(args: &[String], flag: &str, default: f64) -> f64 {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn present(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

/// ⭐ THE VERSION PICK (operator, 2026-08-24). A gene that is on plays its
/// TOP version 60% of the time and one of its other versions — drawn
/// uniformly among the rest — the other 40%; a gene with one version plays
/// that version. So the batch mostly plays what would ship, while every
/// challenger is still priced against off and against the best on every
/// screen it sits in. At two versions the head-to-head loses ~4% of its
/// precision against an equal split (error² ∝ 1/p_a + 1/p_b: 4.17/p against
/// 4/p). A family with no measured top version (see `best_version`) shares
/// equally instead.
const BEST_VERSION_SHARE: f64 = 0.6;

/// The share of a family's on-probability each drawable version takes:
/// `BEST_VERSION_SHARE` to the best, the remainder split evenly among the
/// rest; a lone version takes everything; with no best known, even shares.
fn version_shares(candidates: &[usize], best: Option<usize>) -> Vec<f64> {
    let count = candidates.len();
    match best {
        Some(best) if count > 1 => candidates
            .iter()
            .map(|&i| {
                if i == best {
                    BEST_VERSION_SHARE
                } else {
                    (1.0 - BEST_VERSION_SHARE) / (count - 1) as f64
                }
            })
            .collect(),
        _ => vec![1.0 / count as f64; count],
    }
}

/// ⭐ THE BEST VERSION of a family, among the given drawable members: the
/// version the pinned ledger ships if any, else the priced
/// version with the highest tracked wins (the ledger's pooled on−off win
/// difference), ties to the higher version; `None` when nothing is priced,
/// so an unmeasured family shares its probability equally. The same reading
/// ranks a screen's display when no version is pinned. `tools/genes.py`
/// validates the explicit deployment selection separately: it permits at most
/// one selected family member and never changes that selection from scores.
fn best_version(genes: &[Gene], candidates: &[usize]) -> Option<usize> {
    if let Some(&shipping) = candidates.iter().find(|&&i| genes[i].default_on) {
        return Some(shipping);
    }
    candidates
        .iter()
        .copied()
        .filter(|&i| genes[i].tracked_wins.is_some())
        .max_by(|&a, &b| {
            genes[a]
                .tracked_wins
                .partial_cmp(&genes[b].tracked_wins)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.cmp(&b))
        })
}

/// The on-probability of every gene in header order: `p_default_on` for a
/// screened gene the deployment genome ships on, `p_on` for any other screened
/// gene, and 0 or 1 for a gene held at its default.
///
/// A FAMILY is drawn as one level — off, or exactly one of its versions — so
/// its members' probabilities here are MARGINALS: the family is on with the
/// probability its deployment state says (`p_default_on` if any version ships
/// on, else `p_on`), shared among the screened versions with the BEST version
/// taking `BEST_VERSION_SHARE` of it and the rest splitting the remainder
/// evenly (`version_shares`; operator, 2026-08-24: *"a 60% chance of using the
/// top version of the gene and a 40% chance of using a different gene
/// version (randomly pick among the rest)"* — see `best_version`). A version
/// held ON at its default forces its siblings off; a version held off simply
/// takes no share. The draw reads the family back off these marginals.
fn on_probabilities(
    genes: &[Gene],
    screened: &[bool],
    p_on: f64,
    p_default_on: f64,
    families: &[Vec<usize>],
) -> Vec<f64> {
    let mut probabilities: Vec<f64> = genes
        .iter()
        .zip(screened)
        .map(
            |(gene, &is_screened)| match (is_screened, gene.default_on) {
                (true, true) => p_default_on,
                (true, false) => p_on,
                (false, true) => 1.0,
                (false, false) => 0.0,
            },
        )
        .collect();
    for family in families {
        let candidates: Vec<usize> = family
            .iter()
            .copied()
            .filter(|&i| probabilities[i] > 0.0 && probabilities[i] < 1.0)
            .collect();
        let forced_on = family.iter().any(|&i| probabilities[i] >= 1.0);
        if forced_on {
            for &i in &candidates {
                probabilities[i] = 0.0;
            }
            continue;
        }
        if candidates.is_empty() {
            continue;
        }
        let family_p = candidates
            .iter()
            .map(|&i| probabilities[i])
            .fold(0.0, f64::max);
        let best = best_version(genes, &candidates);
        for (&i, share) in candidates.iter().zip(version_shares(&candidates, best)) {
            probabilities[i] = family_p * share;
        }
    }
    probabilities
}

/// Draw one seat's genome: gene `i` on with probability `probabilities[i]`,
/// seeded from the screen's start seed and the seat's position in the batch
/// (`game × players + seat`), so a run reproduces exactly and two runs on
/// disjoint seed windows draw disjoint genomes. Every seat of every game is
/// an independent draw — nothing is paired, mirrored or complemented.
///
/// A family is then drawn as ONE level over its drawable versions: on with
/// the family's probability (the sum of its members' marginals), and if on,
/// one version in proportion to the marginals — the best version takes
/// `BEST_VERSION_SHARE`, the rest split the remainder — never two versions on
/// one seat.
fn draw_genome(
    start_seed: u64,
    game: usize,
    players: usize,
    seat: usize,
    probabilities: &[f64],
    families: &[Vec<usize>],
) -> Vec<bool> {
    let mut rng = Rng::new(
        start_seed
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add((game * players + seat) as u64)
            .wrapping_add(0x5EED_6E4E),
    );
    let mut genome: Vec<bool> = probabilities
        .iter()
        .map(|&p| {
            if p >= 1.0 {
                true
            } else if p <= 0.0 {
                false
            } else {
                rng.chance(p)
            }
        })
        .collect();
    for family in families {
        let candidates: Vec<usize> = family
            .iter()
            .copied()
            .filter(|&i| probabilities[i] > 0.0 && probabilities[i] < 1.0)
            .collect();
        if candidates.is_empty() {
            continue;
        }
        for &i in &candidates {
            genome[i] = false;
        }
        let family_p: f64 = candidates.iter().map(|&i| probabilities[i]).sum();
        if rng.chance(family_p.min(1.0)) {
            let mut roll = rng.f64() * family_p;
            let mut pick = candidates[candidates.len() - 1];
            for &i in &candidates {
                if roll < probabilities[i] {
                    pick = i;
                    break;
                }
                roll -= probabilities[i];
            }
            genome[pick] = true;
        }
    }
    genome
}

fn genome_string(genome: &[bool]) -> String {
    genome
        .iter()
        .map(|&on| if on { '1' } else { '0' })
        .collect()
}

/// Build a seat: production plus the whole repair universe, then every gene
/// set to the genome bit. The genome string carries every gene, held ones
/// included, so a seat's row says exactly what it played.
fn seat_with_genome(genes: &[Gene], genome: &[bool]) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    // The universe, not the deployment genome: every gene is set explicitly
    // below, and `after_setup_on` describes the universe.
    ai.enable_engine_repairs_universe();
    for (gene, &on) in genes.iter().zip(genome) {
        if on != gene.after_setup_on {
            (gene.flip)(&mut ai);
        }
    }
    ai
}

struct Profile {
    players: usize,
    width: i32,
    height: i32,
    turns: u32,
    city_states: usize,
    speed: GameSpeed,
    map: MapScript,
    randomize_civs: bool,
    victories: civvis::game::VictoryConditions,
    /// ⭐ THE CONTESTED FIELD. One major seat is pinned to each lane named
    /// here; the rest draw genomes and are the seats the screen measures.
    /// Empty is the standard fieldless screen.
    field: Vec<VictoryTarget>,
    /// `Game::native_competitions`.
    native_competitions: bool,
    /// Genes switched on for a field seat over the deployment genome
    /// (`CONTESTED_FIELD_GENES`). Never applied to a measured seat.
    field_genes: Vec<String>,
    /// ⭐ The rotating victory mask, `None` for the plain batch-level set.
    victory_mask: Option<VictoryMask>,
    /// The majors' rung when every game plays one.
    difficulty: String,
    /// ⭐ The majors' rung rotation, `None` when every game plays `difficulty`.
    difficulty_rotate: Option<DifficultyRotation>,
    /// ⭐ Whether one major seat per game plays a fixed rival ([`RIVAL_KINDS`]).
    rivals: bool,
}

/// ⭐ WHICH MAJOR SEATS THE FIELD PINS, AND TO WHAT — one entry per major
/// seat, in seat order, `None` for a seat that draws a genome and is measured.
///
/// ⚠ The pinned positions ROTATE with the game index. Seat position is not
/// neutral on this board — the note on `--stock-civs` records seats 0 and 2
/// winning twice as often as seat 3 *whoever sat there* — so pinning a fixed
/// position would confound the field with the seat and hand every measured
/// gene the leftovers of one particular chair. Rotating gives every position
/// an equal share of both roles across the batch.
fn pinned_seats(
    players: usize,
    game: usize,
    field: &[VictoryTarget],
) -> Vec<Option<VictoryTarget>> {
    let mut pinned = vec![None; players];
    if players == 0 {
        return pinned;
    }
    for (offset, &target) in field.iter().enumerate() {
        pinned[(game + offset) % players] = Some(target);
    }
    pinned
}

/// A field seat: the deployment genome plus `lane_genes`, pinned to one victory
/// lane.
///
/// Not a drawn genome and not the repair universe. It starts from
/// `AdvancedAi::new()` — exactly the genome the ledger ships, the rival the
/// agent actually meets — and is held CONSTANT across the batch, so it
/// contributes no variance of its own to any gene's contrast. `retarget` is the
/// same call the rollout planner and the retired `live_target_<lane>` arms
/// used, so the pursuit is the controller's real victory-lane behaviour rather
/// than a label: `victory_focus` resolves to the assigned lane, and the ballot,
/// the Great Person race, the policy deck, the culture spending pass and the
/// space race all read it. `lane_genes` is empty by default — the deployment
/// genome is the whole of a pursuer — and `--contested-field-genes lanes`
/// switches on the seven lane opt-ins instead; see `CONTESTED_FIELD_GENES` for
/// the 62 games that chose between them.
fn field_seat(target: VictoryTarget, lane_genes: &[String]) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    for gene in civvis::ai::screenable_genes() {
        if lane_genes.iter().any(|tag| tag == gene.tag) {
            (gene.enable)(&mut ai);
        }
    }
    ai.retarget(target);
    ai
}

/// Play one game in which every DRAWN major seat carries its own genome, and
/// report one row per drawn major. Minor and barbarian seats stay stock.
///
/// Fieldless — the standard screen — every major is drawn, so the opposition
/// is the other drawn majors: effects are averaged over random opposing
/// genomes rather than measured against a fixed production field, which is
/// the point: a flag that only pays against untreated opponents is a flag the
/// mixed ecosystem does not have.
///
/// ⭐ With a CONTESTED FIELD, `profile.field.len()` of the majors are instead
/// pinned pursuers ([`field_seat`]) and are NOT measured: no row is written
/// for them. They are the threat, not the observation. Their score still
/// counts in every measured seat's `score_share`, and their Diplomatic
/// Victory Points and tourists are what `rival_dvp` / `rival_tourists`
/// report, so a measured row can say how close the pursuer came.
fn play_game(
    profile: &Profile,
    genes: &[Gene],
    game: usize,
    seed: u64,
    genomes: &[Vec<bool>],
) -> Vec<Row> {
    let started = Instant::now();
    // ⭐ THE MASK IS PER GAME, FROM THE SEED: the batch-level set minus the
    // lanes this game's mask closes, score always on.
    let closed: Vec<String> = profile
        .victory_mask
        .map(|mask| {
            mask.closed(seed, profile.victories)
                .into_iter()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let victories = profile.victory_mask.map_or(profile.victories, |mask| {
        mask.apply(seed, profile.victories)
    });
    // ⭐ THE MAJORS' RUNG, per game from the seed under a rotation. The
    // barbarian seat keeps its own rung: `GameOptions::new` sets it.
    let difficulty = profile
        .difficulty_rotate
        .as_ref()
        .map_or(profile.difficulty.as_str(), |rotation| rotation.rung(seed))
        .to_string();
    let mut world = Game::new_with(GameOptions {
        speed: profile.speed.id().to_string(),
        map_script: profile.map,
        randomize_civs: profile.randomize_civs,
        victory_conditions: victories,
        difficulty: difficulty.clone(),
        ..GameOptions::new(
            profile.players,
            profile.width,
            profile.height,
            seed,
            profile.turns,
            profile.city_states,
        )
    });
    // ⚠ Set on the world rather than through `GameOptions`, which has no such
    // field: `civvis simulate --native-competitions` reaches it exactly the
    // same way. Nothing in the engine changes for this — the flag has shipped
    // since the competitions landed, off by default and waiting for an
    // instrument that could price it.
    world.native_competitions = profile.native_competitions;
    let pinned = pinned_seats(profile.players, game, &profile.field);
    // ⭐ THE RIVAL MIX: one major plays a fixed opponent of a kind drawn from
    // the seed, in a chair that rotates with the game index.
    let rival = profile
        .rivals
        .then(|| (rival_index(profile.players, game), rival_kind(seed)));
    let mut majors = Vec::new();
    let mut ais: Vec<AdvancedAi> = (0..world.players.len())
        .map(|pid| {
            if world.players[pid].is_minor || world.players[pid].is_barbarian {
                AdvancedAi::new()
            } else {
                let index = majors.len();
                majors.push(pid);
                match (pinned.get(index).copied().flatten(), rival) {
                    (Some(target), _) => field_seat(target, &profile.field_genes),
                    (None, Some((rival_at, kind))) if rival_at == index => {
                        rival_seat(kind, seed, genes, &genomes[index])
                    }
                    (None, _) => seat_with_genome(genes, &genomes[index]),
                }
            }
        })
        .collect();
    run_game(&mut world, &mut ais);
    let secs = started.elapsed().as_secs_f64();
    majors
        .iter()
        .enumerate()
        .filter(|(index, _)| pinned.get(*index).copied().flatten().is_none())
        .map(|(index, &seat)| {
            let this_rival = rival.filter(|(rival_at, _)| *rival_at == index);
            let mut row = row_for_seat(
                &world,
                game,
                seed,
                seat,
                match this_rival {
                    // Only a random rival has a genome in header order.
                    Some((_, "random")) | None => genome_string(&genomes[index]),
                    Some(_) => String::new(),
                },
                secs,
            );
            row.victories_off = closed.clone();
            row.difficulty = difficulty.clone();
            match this_rival {
                Some((_, kind)) => {
                    // ⭐ NOT a measured seat: `kind` is what every estimator
                    // filters on, so the fixed opponent prices no gene.
                    row.kind = "rival".to_string();
                    row.rival_mix = kind.to_string();
                    if kind == "firaxis-mix" {
                        row.rival_target = firaxis_mix_target(seed).as_str().to_string();
                    }
                }
                None if rival.is_some() => row.rival_mix = "measured".to_string(),
                None => {}
            }
            row
        })
        .collect()
}

/// One finished game read from one seat's point of view.
fn row_for_seat(
    game: &Game,
    index: usize,
    seed: u64,
    seat: usize,
    genome: String,
    secs: f64,
) -> Row {
    let majors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    let scores: BTreeMap<usize, i64> = majors.iter().map(|&pid| (pid, game.score(pid))).collect();
    let total: i64 = scores.values().sum();
    let score = scores.get(&seat).copied().unwrap_or(0);
    let rank = 1 + scores.values().filter(|&&other| other > score).count();
    let religion = game.players[seat].religion.clone();
    let foreign_faith_cities = game
        .player_city_ids(seat)
        .iter()
        .filter_map(|cid| game.cities.get(cid))
        .filter(|city| {
            game.city_religion(city)
                .is_some_and(|faith| Some(faith) != religion.as_deref())
        })
        .count();
    let counter = |key: &str| game.players[seat].counters.get(key).copied().unwrap_or(0);
    // ⭐ The two lanes the live seat actually loses to, read off the finished
    // world: this seat's standing and the best OTHER major's. `rival_dvp` is
    // what says a pinned diplomatic pursuer was a real threat — or that it was
    // not, which is a finding and not a bug.
    let rival = |value: &dyn Fn(usize) -> i64| {
        majors
            .iter()
            .filter(|&&pid| pid != seat)
            .map(|&pid| value(pid))
            .max()
            .unwrap_or(0)
    };
    Row {
        kind: "game".to_string(),
        game: index,
        pair: 0,
        arm: 0,
        seed,
        seat,
        genome,
        win: game.winner == Some(seat),
        winner: game.winner,
        victory: game.victory_type.clone().unwrap_or_default(),
        turn: game.reported_turn(),
        score,
        score_share: if total > 0 {
            score as f64 / total as f64
        } else {
            0.0
        },
        rank,
        cities: game.player_city_ids(seat).len(),
        alive: game.players[seat].alive,
        secs,
        founded_religion: religion.is_some(),
        foreign_faith_cities,
        faith: game.players[seat].faith,
        inquisition: game.players[seat]
            .counters
            .get("inquisition")
            .is_some_and(|launched| *launched > 0),
        techs: game.players[seat].techs.len(),
        wonders: game
            .player_city_ids(seat)
            .iter()
            .filter_map(|cid| game.cities.get(cid))
            .map(|city| city.wonders.len())
            .sum(),
        military: game.military_power(seat),
        civ: game.players[seat].civ.clone(),
        raid_wars: counter("raid_wars"),
        campaign_plans: counter("campaign:planned"),
        campaign_wars: counter("campaign:declared"),
        campaign_captures: counter("campaign:taken"),
        campaign_pillages: counter("campaign:pillaged"),
        settlers_captured: counter("captured:settler"),
        builders_captured: counter("captured:builder"),
        religious_lost: counter("religious_lost_to_barbarians"),
        pillages: counter("pillages"),
        raid_settler_prizes: counter("raid_prize:settler"),
        dvp: game.players[seat].dvp,
        rival_dvp: rival(&|pid| game.players[pid].dvp),
        tourists: game.foreign_tourists(seat),
        rival_tourists: rival(&|pid| game.foreign_tourists(pid)),
        domestic: game.domestic_tourists(seat),
        // Filled in by `play_game`, which knows the game's mask and rung.
        victories_off: Vec::new(),
        difficulty: String::new(),
        rival_mix: String::new(),
        rival_target: String::new(),
    }
}

// ----------------------------------------------------------------- statistics

/// Standard normal CDF (Abramowitz & Stegun 7.1.26, |error| < 1.5e-7).
fn normal_cdf(z: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.3275911 * z.abs() / std::f64::consts::SQRT_2);
    let poly = t
        * (0.254829592
            + t * (-0.284496736 + t * (1.421413741 + t * (-1.453152027 + t * 1.061405429))));
    let erf = 1.0 - poly * (-(z * z) / 2.0).exp();
    0.5 * (1.0 + if z >= 0.0 { erf } else { -erf })
}

/// Upper-tail quantile: the `z` with `P(Z > z) = p`, by bisection.
fn normal_quantile_upper(p: f64) -> f64 {
    let (mut lo, mut hi) = (0.0f64, 40.0f64);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if 1.0 - normal_cdf(mid) > p {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Solve `A x = b` in place by Gaussian elimination with partial pivoting.
/// Returns `None` when the matrix is singular to working precision.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-9 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            if factor == 0.0 {
                continue;
            }
            let pivot_row = a[col].clone();
            for (entry, pivot_entry) in a[row].iter_mut().zip(&pivot_row).skip(col) {
                *entry -= factor * pivot_entry;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut sum = b[row];
        for k in row + 1..n {
            sum -= a[row][k] * x[k];
        }
        x[row] = sum / a[row][row];
    }
    Some(x)
}

/// Invert a symmetric positive matrix column by column; `None` if singular.
fn invert(a: &[Vec<f64>]) -> Option<Vec<Vec<f64>>> {
    let n = a.len();
    let mut columns = Vec::with_capacity(n);
    for j in 0..n {
        let mut e = vec![0.0; n];
        e[j] = 1.0;
        columns.push(solve(a.to_vec(), e)?);
    }
    // columns[j][i] is entry (i, j) of the inverse.
    Some(
        (0..n)
            .map(|i| (0..n).map(|j| columns[j][i]).collect())
            .collect(),
    )
}

/// OLS of `y` on `design` — rows that already carry their intercept column —
/// with cluster-robust (sandwich) standard errors: the seats of one game share
/// a winner, so their residuals are correlated and a per-seat error would be
/// too small. Each cluster's score vector `Σᵢ xᵢeᵢ` is summed whole before it
/// is squared: `(XᵀX)⁻¹ (Σ_g s_g s_gᵀ) (XᵀX)⁻¹`, scaled by `G/(G−1)`.
///
/// Returns `(coefficient, se)` per column, or `None` when the design cannot
/// support it (fewer rows than columns plus ten, fewer than two clusters, or a
/// singular matrix — a gene that never varied, for instance).
fn ols_clustered(design: &[Vec<f64>], y: &[f64], clusters: &[GameKey]) -> Option<Vec<(f64, f64)>> {
    let n = y.len();
    let k = design.first()?.len();
    if k == 0 || n < k + 10 {
        return None;
    }
    let mut xtx = vec![vec![0.0; k]; k];
    let mut xty = vec![0.0; k];
    for (row, &value) in design.iter().zip(y) {
        for i in 0..k {
            xty[i] += row[i] * value;
            for j in i..k {
                xtx[i][j] += row[i] * row[j];
            }
        }
    }
    // Mirror the upper triangle accumulated above into the lower one.
    for i in 1..k {
        let (above, from_i) = xtx.split_at_mut(i);
        for (j, row) in above.iter().enumerate() {
            from_i[0][j] = row[i];
        }
    }
    let inverse = invert(&xtx)?;
    let beta: Vec<f64> = (0..k)
        .map(|i| (0..k).map(|j| inverse[i][j] * xty[j]).sum())
        .collect();
    let mut scores: BTreeMap<GameKey, Vec<f64>> = BTreeMap::new();
    for ((row, &value), key) in design.iter().zip(y).zip(clusters) {
        let fitted: f64 = row.iter().zip(&beta).map(|(x, b)| x * b).sum();
        let residual = value - fitted;
        let score = scores.entry(*key).or_insert_with(|| vec![0.0; k]);
        for i in 0..k {
            score[i] += row[i] * residual;
        }
    }
    let groups = scores.len();
    if groups < 2 {
        return None;
    }
    let mut meat = vec![vec![0.0; k]; k];
    for score in scores.values() {
        for i in 0..k {
            for j in 0..k {
                meat[i][j] += score[i] * score[j];
            }
        }
    }
    let small_sample = groups as f64 / (groups - 1) as f64;
    Some(
        (0..k)
            .map(|i| {
                let variance: f64 = (0..k)
                    .map(|a| {
                        (0..k)
                            .map(|b| inverse[i][a] * meat[a][b] * inverse[b][i])
                            .sum::<f64>()
                    })
                    .sum::<f64>()
                    * small_sample;
                (beta[i], variance.max(0.0).sqrt())
            })
            .collect(),
    )
}

/// The screened rows of a file as the analysis sees them: one seat per row
/// with its ±1 sign on every screened gene, its game, and its outcomes.
struct Seats<'a> {
    rows: Vec<&'a Row>,
    /// Header-order index of each screened gene, in column order.
    columns: Vec<usize>,
    /// ±1 per screened column, per row.
    signs: Vec<Vec<f64>>,
    clusters: Vec<GameKey>,
    wins: Vec<f64>,
    shares: Vec<f64>,
}

impl<'a> Seats<'a> {
    fn of(header: &Header, rows: &'a [Row]) -> Self {
        let k = header.genes.len();
        let screened = header.screened_mask();
        let columns: Vec<usize> = (0..k).filter(|&i| screened[i]).collect();
        let rows: Vec<&Row> = rows
            .iter()
            .filter(|row| row.kind == "game" && row.genome.chars().count() == k)
            .collect();
        let signs = rows
            .iter()
            .map(|row| {
                let bits = row.bits();
                columns
                    .iter()
                    .map(|&i| if bits[i] { 1.0 } else { -1.0 })
                    .collect()
            })
            .collect();
        let clusters = rows.iter().map(|row| row.game_key()).collect();
        let wins = rows
            .iter()
            .map(|row| f64::from(u8::from(row.win)))
            .collect();
        let shares = rows.iter().map(|row| row.score_share).collect();
        Seats {
            rows,
            columns,
            signs,
            clusters,
            wins,
            shares,
        }
    }

    fn games(&self) -> usize {
        let mut keys: Vec<GameKey> = self.clusters.clone();
        keys.sort_unstable();
        keys.dedup();
        keys.len()
    }

    /// One gene's on−off difference on an outcome, with its clustered error:
    /// the regression of the outcome on `[1, sign]`, whose slope is half the
    /// on−off difference exactly.
    fn contrast(&self, column: usize, outcome: &[f64]) -> (f64, f64) {
        let design: Vec<Vec<f64>> = self
            .signs
            .iter()
            .map(|row| vec![1.0, row[column]])
            .collect();
        match ols_clustered(&design, outcome, &self.clusters) {
            Some(fit) => {
                let (delta, se) = (2.0 * fit[1].0, 2.0 * fit[1].1);
                (delta, se.max(self.empty_arm_floor(column, outcome)))
            }
            None => {
                // Too few seats for an error: report the raw difference and no
                // precision, so the row reads as unresolved rather than exact.
                let (mut on, mut n_on, mut off, mut n_off) = (0.0, 0usize, 0.0, 0usize);
                for (row, &value) in self.signs.iter().zip(outcome) {
                    if row[column] > 0.0 {
                        on += value;
                        n_on += 1;
                    } else {
                        off += value;
                        n_off += 1;
                    }
                }
                let mean = |sum: f64, n: usize| if n > 0 { sum / n as f64 } else { 0.0 };
                (mean(on, n_on) - mean(off, n_off), f64::INFINITY)
            }
        }
    }

    /// A floor under the clustered error for a binary outcome one of whose
    /// arms has NO EVENTS AT ALL.
    ///
    /// ⚠⚠ A GENE THAT CANNOT FIRE ONCE PRINTED `HURTS **` AT z -18.21. With
    /// `--genes <tag>` only that gene varies, so a gene that does not fire
    /// plays both arms as the same game; each game still has exactly one
    /// winner, and whether that winner drew the gene is chance. When none of
    /// them did, the sandwich estimator answers a two-point interval instead
    /// of refusing. Reproduced on 2026-08-25 by editing a gene's predicate to
    /// `return false` unconditionally:
    ///
    /// ```text
    /// gene_screen --games 12 --jobs 6 --genes <tag> --start-seed 99000000
    ///   16/56  0.0%  21.4%  -21.4pp z-18.21  [-23.7, -19.1]  HURTS **
    /// ```
    ///
    /// The same block had already reported that identical number for two real
    /// implementations with OPPOSITE semantics, which is what exposed it.
    ///
    /// The difference is still real and is still reported; what is not to be
    /// believed is its precision. So this widens rather than refuses, and
    /// widens to a figure that is defensible on its own: the ordinary
    /// difference-of-proportions error, multiplied by the design effect of the
    /// seats sharing a game (mean cluster size), which is exactly the
    /// correction clustering exists to apply. It can only ever make a row less
    /// significant, never more.
    ///
    /// Deliberately not a refusal, and deliberately not gated on the event
    /// count: a gene that wins every game it is drawn into ALSO empties an
    /// arm, and that one is real. On the planted fixture in this file it keeps
    /// |z| above ten; on the block above it takes |z| below two.
    fn empty_arm_floor(&self, column: usize, outcome: &[f64]) -> f64 {
        if !outcome.iter().all(|value| *value == 0.0 || *value == 1.0) {
            return 0.0;
        }
        let arm = |on: bool| {
            let mut events = 0.0;
            let mut seats = 0.0;
            for (row, value) in self.signs.iter().zip(outcome) {
                if (row[column] > 0.0) == on {
                    seats += 1.0;
                    events += *value;
                }
            }
            (events, seats)
        };
        let (on_events, on_seats) = arm(true);
        let (off_events, off_seats) = arm(false);
        if on_seats <= 0.0 || off_seats <= 0.0 {
            return 0.0;
        }
        if on_events > 0.0 && off_events > 0.0 {
            return 0.0;
        }
        let variance = |events: f64, seats: f64| {
            let rate = events / seats;
            rate * (1.0 - rate) / seats
        };
        let clusters = self
            .clusters
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            .max(1) as f64;
        let design_effect = (on_seats + off_seats) / clusters;
        (variance(on_events, on_seats) + variance(off_events, off_seats))
            .max(0.0)
            .sqrt()
            * design_effect.max(1.0).sqrt()
    }

    /// Every gene's win Δ at once: the regression of the win on an intercept
    /// and every screened sign, so a gene is not credited with the chance
    /// imbalance of its neighbours. `None` until the seats can support it.
    fn adjusted(&self, outcome: &[f64]) -> Option<Vec<(f64, f64)>> {
        let design: Vec<Vec<f64>> = self
            .signs
            .iter()
            .map(|row| std::iter::once(1.0).chain(row.iter().copied()).collect())
            .collect();
        let fit = ols_clustered(&design, outcome, &self.clusters)?;
        Some(
            fit.into_iter()
                .skip(1)
                .enumerate()
                .map(|(column, (b, se))| {
                    // The adjusted column is the same estimate from a wider
                    // design and inherits the same defect, so it takes the
                    // same floor. See `empty_arm_floor`.
                    (
                        2.0 * b,
                        (2.0 * se).max(self.empty_arm_floor(column, outcome)),
                    )
                })
                .collect(),
        )
    }
}

/// One gene's estimates from the seats.
#[derive(Clone, Debug)]
struct GeneEstimate {
    tag: String,
    /// Seats with the gene on / off behind `win_on` / `win_off`.
    n_on: usize,
    n_off: usize,
    win_on: f64,
    win_off: f64,
    /// Win-rate Δ (on − off) with its standard error.
    win_delta: f64,
    win_se: f64,
    share_delta: f64,
    share_se: f64,
    adjusted: Option<(f64, f64)>,
}

impl GeneEstimate {
    fn seats(&self) -> usize {
        self.n_on + self.n_off
    }
    fn win_z(&self) -> f64 {
        if self.win_se > 0.0 && self.win_se.is_finite() {
            self.win_delta / self.win_se
        } else {
            0.0
        }
    }
    fn share_z(&self) -> f64 {
        if self.share_se > 0.0 && self.share_se.is_finite() {
            self.share_delta / self.share_se
        } else {
            0.0
        }
    }
}

/// The whole table's worth of estimates, plus what they rest on.
struct Estimates {
    genes: Vec<GeneEstimate>,
    games: usize,
    seats: usize,
    overall_win: f64,
    overall_share: f64,
}

fn estimate(header: &Header, rows: &[Row]) -> Estimates {
    let seats = Seats::of(header, rows);
    let n = seats.rows.len();
    let overall_win = if n > 0 {
        seats.wins.iter().sum::<f64>() / n as f64
    } else {
        0.0
    };
    let overall_share = if n > 0 {
        seats.shares.iter().sum::<f64>() / n as f64
    } else {
        0.0
    };
    let adjusted = seats.adjusted(&seats.wins);
    let genes = seats
        .columns
        .iter()
        .enumerate()
        .map(|(column, &index)| {
            let (mut wins_on, mut n_on, mut wins_off, mut n_off) = (0usize, 0usize, 0usize, 0usize);
            for (row, seat) in seats.signs.iter().zip(&seats.rows) {
                if row[column] > 0.0 {
                    n_on += 1;
                    wins_on += usize::from(seat.win);
                } else {
                    n_off += 1;
                    wins_off += usize::from(seat.win);
                }
            }
            let (win_delta, win_se) = seats.contrast(column, &seats.wins);
            let (share_delta, share_se) = seats.contrast(column, &seats.shares);
            let rate = |wins: usize, n: usize| if n > 0 { wins as f64 / n as f64 } else { 0.0 };
            GeneEstimate {
                tag: header.genes[index].clone(),
                n_on,
                n_off,
                win_on: rate(wins_on, n_on),
                win_off: rate(wins_off, n_off),
                win_delta,
                win_se,
                share_delta,
                share_se,
                adjusted: adjusted.as_ref().map(|all| all[column]),
            }
        })
        .collect();
    Estimates {
        genes,
        games: seats.games(),
        seats: n,
        overall_win,
        overall_share,
    }
}

/// One cell of a family table: the seats that played one level of the
/// family — `off`, or one of its versions — and how they did.
#[derive(Clone, Debug)]
struct FamilyCell {
    label: String,
    seats: usize,
    win: f64,
    share: f64,
}

/// One head-to-head inside a family: `b` against `a`, on seats that played
/// exactly one of the two, errors clustered by game.
#[derive(Clone, Debug)]
struct FamilyContrast {
    a: String,
    b: String,
    win_delta: f64,
    win_se: f64,
    share_delta: f64,
    share_se: f64,
}

impl FamilyContrast {
    fn win_z(&self) -> f64 {
        if self.win_se > 0.0 && self.win_se.is_finite() {
            self.win_delta / self.win_se
        } else {
            0.0
        }
    }
    fn share_z(&self) -> f64 {
        if self.share_se > 0.0 && self.share_se.is_finite() {
            self.share_delta / self.share_se
        } else {
            0.0
        }
    }
}

/// A versioned gene read as one family: what each level did, and whether
/// each version beats `off` and beats the version before it.
#[derive(Clone, Debug)]
struct FamilyEstimate {
    base: String,
    versions: Vec<String>,
    cells: Vec<FamilyCell>,
    contrasts: Vec<FamilyContrast>,
}

/// Every family's cells and contrasts. A seat that somehow played two
/// versions (impossible under the screen's draw; possible in a hand-built
/// file) is left out of that family rather than filed under either.
fn estimate_families(header: &Header, rows: &[Row]) -> Vec<FamilyEstimate> {
    let seats = Seats::of(header, rows);
    header
        .families()
        .into_iter()
        .filter(|family| family.len() >= 2)
        .map(|family| {
            let labels: Vec<String> = std::iter::once("off".to_string())
                .chain(family.iter().map(|&i| header.genes[i].clone()))
                .collect();
            // Level per seat: 0 = off, k = version k (1-based in `family`).
            let levels: Vec<Option<usize>> = seats
                .rows
                .iter()
                .map(|row| {
                    let bits = row.bits();
                    let on: Vec<usize> = family
                        .iter()
                        .enumerate()
                        .filter(|(_, &i)| bits[i])
                        .map(|(k, _)| k + 1)
                        .collect();
                    match on.as_slice() {
                        [] => Some(0),
                        [one] => Some(*one),
                        _ => None,
                    }
                })
                .collect();
            let cells: Vec<FamilyCell> = (0..labels.len())
                .map(|level| {
                    let members: Vec<usize> = levels
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| **l == Some(level))
                        .map(|(r, _)| r)
                        .collect();
                    let n = members.len();
                    let mean = |values: &[f64]| {
                        if n == 0 {
                            0.0
                        } else {
                            members.iter().map(|&r| values[r]).sum::<f64>() / n as f64
                        }
                    };
                    FamilyCell {
                        label: labels[level].clone(),
                        seats: n,
                        win: mean(&seats.wins),
                        share: mean(&seats.shares),
                    }
                })
                .collect();
            let contrast = |a: usize, b: usize| -> FamilyContrast {
                let picked: Vec<usize> = levels
                    .iter()
                    .enumerate()
                    .filter(|(_, l)| **l == Some(a) || **l == Some(b))
                    .map(|(r, _)| r)
                    .collect();
                let design: Vec<Vec<f64>> = picked
                    .iter()
                    .map(|&r| vec![1.0, if levels[r] == Some(b) { 1.0 } else { 0.0 }])
                    .collect();
                let clusters: Vec<GameKey> = picked.iter().map(|&r| seats.clusters[r]).collect();
                let fit = |outcome: &[f64]| -> (f64, f64) {
                    let y: Vec<f64> = picked.iter().map(|&r| outcome[r]).collect();
                    match ols_clustered(&design, &y, &clusters) {
                        Some(fit) => (fit[1].0, fit[1].1),
                        None => (cells[b].win - cells[a].win, f64::INFINITY),
                    }
                };
                let (win_delta, win_se) = fit(&seats.wins);
                let (share_delta, share_se) = fit(&seats.shares);
                FamilyContrast {
                    a: labels[a].clone(),
                    b: labels[b].clone(),
                    win_delta,
                    win_se,
                    share_delta,
                    share_se,
                }
            };
            let mut contrasts = Vec::new();
            for version in 1..labels.len() {
                contrasts.push(contrast(0, version));
            }
            for version in 2..labels.len() {
                contrasts.push(contrast(version - 1, version));
            }
            FamilyEstimate {
                base: header.genes[family[0]].clone(),
                versions: family.iter().map(|&i| header.genes[i].clone()).collect(),
                cells,
                contrasts,
            }
        })
        .collect()
}

/// One gene's causal simulation-cost estimate. Effects stay on the log scale
/// internally: a coefficient of `ln(1.10)` means enabling the gene for one
/// major seat makes the measured quantity 10% larger. Log ratios are the
/// right unit here because a 100 ms delay means something very different in a
/// two-second game and a two-minute game, and they keep a temporarily slow
/// worker from dominating the estimate.
#[derive(Clone, Copy, Debug)]
struct CostEstimate {
    /// Games with usable seconds and turn counts behind the fit.
    games: usize,
    /// Change in wall seconds per completed game turn, per enabled major seat.
    compute_log_delta: f64,
    compute_log_se: f64,
    /// Change in whole-game wall seconds, per enabled major seat.
    time_log_delta: f64,
    time_log_se: f64,
}

impl CostEstimate {
    fn percent(log_delta: f64) -> f64 {
        100.0 * log_delta.exp_m1()
    }

    /// Delta-method standard error after changing from log units to percent.
    fn percent_se(log_delta: f64, log_se: f64) -> f64 {
        100.0 * log_delta.exp() * log_se
    }

    fn compute_pct(&self) -> f64 {
        Self::percent(self.compute_log_delta)
    }

    fn compute_se_pct(&self) -> f64 {
        Self::percent_se(self.compute_log_delta, self.compute_log_se)
    }

    fn time_pct(&self) -> f64 {
        Self::percent(self.time_log_delta)
    }

    fn time_se_pct(&self) -> f64 {
        Self::percent_se(self.time_log_delta, self.time_log_se)
    }
}

/// OLS with HC1 heteroskedasticity-robust errors. Timing variance grows with a
/// game's size and length even after taking logs, so a homoskedastic error
/// would be optimistic here. Rows are one independent game each; no further
/// clustering is required.
fn robust_cost_regression(design: &[Vec<f64>], y: &[f64]) -> Option<Vec<(f64, f64)>> {
    let n = y.len();
    let k = design.first()?.len();
    if k == 0 || n < 2 * k + 10 {
        return None;
    }
    let mut xtx = vec![vec![0.0; k]; k];
    let mut xty = vec![0.0; k];
    for (row, &d) in design.iter().zip(y) {
        for i in 0..k {
            xty[i] += row[i] * d;
            for j in 0..k {
                xtx[i][j] += row[i] * row[j];
            }
        }
    }
    let inverse = invert(&xtx)?;
    let beta: Vec<f64> = (0..k)
        .map(|i| (0..k).map(|j| inverse[i][j] * xty[j]).sum())
        .collect();
    let mut meat = vec![vec![0.0; k]; k];
    for (row, &d) in design.iter().zip(y) {
        let fitted: f64 = row.iter().zip(&beta).map(|(x, b)| x * b).sum();
        let residual2 = (d - fitted).powi(2);
        for i in 0..k {
            for j in 0..k {
                meat[i][j] += row[i] * row[j] * residual2;
            }
        }
    }
    let hc1 = n as f64 / (n - k) as f64;
    Some(
        (0..k)
            .map(|i| {
                let variance: f64 = (0..k)
                    .map(|a| {
                        (0..k)
                            .map(|b| inverse[i][a] * meat[a][b] * inverse[b][i])
                            .sum::<f64>()
                    })
                    .sum::<f64>()
                    * hc1;
                (beta[i], variance.max(0.0).sqrt())
            })
            .collect(),
    )
}

/// Fit the game timings on an intercept and every screened gene at once. The
/// intercept absorbs machine-load drift; the gene coefficients therefore
/// cannot inherit a small chance imbalance in which genomes happened to run
/// first. When a short pilot cannot support the full matrix, fit each gene
/// with the same intercept rather than publishing no measurement at all.
fn adjusted_cost_effects(counts: &[Vec<f64>], y: &[f64]) -> Vec<Option<(f64, f64)>> {
    let Some(k) = counts.first().map(Vec::len) else {
        return Vec::new();
    };
    let with_intercept: Vec<Vec<f64>> = counts
        .iter()
        .map(|row| std::iter::once(1.0).chain(row.iter().copied()).collect())
        .collect();
    if let Some(all) = robust_cost_regression(&with_intercept, y) {
        return all.into_iter().skip(1).map(Some).collect();
    }
    (0..k)
        .map(|column| {
            let one: Vec<Vec<f64>> = counts.iter().map(|row| vec![1.0, row[column]]).collect();
            robust_cost_regression(&one, y).and_then(|fit| fit.get(1).copied())
        })
        .collect()
}

/// Estimate the incremental runtime cost of enabling every screened gene from
/// the screen rows that already exist. No heuristic is timed in its hot path
/// and no profiling-only game is required.
///
/// A game's timing is regressed on how many of its major seats had each gene
/// on, so the coefficient is the cost of enabling the gene for one major
/// seat, with one independent observation per game rather than `players`
/// duplicate timings.
///
/// `compute` is seconds per completed turn: it answers whether the simulation
/// does more wall-clock work at the same game length. `time` is whole-game
/// seconds: it is the throughput impact operators actually pay, including a
/// gene that reaches the same victory in more or fewer turns.
fn estimate_costs(header: &Header, rows: &[Row]) -> BTreeMap<String, CostEstimate> {
    let seats = Seats::of(header, rows);
    let width = seats.columns.len();
    // key -> (count of seats with each gene on, compute log, time log)
    let mut games: BTreeMap<GameKey, (Vec<f64>, f64, f64)> = BTreeMap::new();
    for ((row, signs), key) in seats.rows.iter().zip(&seats.signs).zip(&seats.clusters) {
        if !(row.secs.is_finite() && row.secs > 0.0) || row.turn == 0 {
            continue;
        }
        let game = games.entry(*key).or_insert_with(|| {
            (
                vec![0.0; width],
                (row.secs / f64::from(row.turn)).ln(),
                row.secs.ln(),
            )
        });
        for (count, sign) in game.0.iter_mut().zip(signs) {
            if *sign > 0.0 {
                *count += 1.0;
            }
        }
    }
    if games.is_empty() {
        return BTreeMap::new();
    }
    let counts: Vec<Vec<f64>> = games.values().map(|game| game.0.clone()).collect();
    let compute_y: Vec<f64> = games.values().map(|game| game.1).collect();
    let time_y: Vec<f64> = games.values().map(|game| game.2).collect();
    let compute = adjusted_cost_effects(&counts, &compute_y);
    let time = adjusted_cost_effects(&counts, &time_y);
    seats
        .columns
        .iter()
        .enumerate()
        .filter_map(|(column, &index)| {
            let ((compute_log_delta, compute_log_se), (time_log_delta, time_log_se)) = (
                compute.get(column).copied().flatten()?,
                time.get(column).copied().flatten()?,
            );
            (compute_log_delta.is_finite()
                && compute_log_se.is_finite()
                && time_log_delta.is_finite()
                && time_log_se.is_finite())
            .then(|| {
                (
                    header.genes[index].clone(),
                    CostEstimate {
                        games: games.len(),
                        compute_log_delta,
                        compute_log_se,
                        time_log_delta,
                        time_log_se,
                    },
                )
            })
        })
        .collect()
}

/// A chronological replication tranche holds about this many seats — whole
/// games only, since the seats of one game share a winner and splitting them
/// across tranches would make the purported replications correlated. Twenty
/// thousand seats is the precision the old design's 10,000 seat pairs bought.
const REPRO_WINDOW_SEATS: usize = 20_000;
const REPRO_WINDOW_COUNT: usize = 3;

/// One newest-first, non-overlapping replication tranche.
#[derive(Clone, Debug)]
struct ReproTranche {
    seats: usize,
    games: usize,
    estimates: Vec<GeneEstimate>,
}

/// Split the newest data into non-overlapping, chronological replication
/// tranches. "Newest" is input order, deliberately, rather than the seed:
/// appended runs can use any disjoint seed range, while input order is what
/// "latest" means. A whole game moves as one unit, so the requested width is
/// approximate only when it would otherwise split a game.
fn reproducibility_tranches(
    header: &Header,
    rows: &[Row],
    window_seats: usize,
    window_count: usize,
) -> Vec<ReproTranche> {
    let k = header.genes.len();
    let mut grouped: BTreeMap<GameKey, (usize, Vec<&Row>)> = BTreeMap::new();
    for (ordinal, row) in rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.kind == "game" && row.genome.chars().count() == k)
    {
        let game = grouped.entry(row.game_key()).or_insert((0, Vec::new()));
        game.0 = game.0.max(ordinal);
        game.1.push(row);
    }
    let mut games: Vec<(usize, Vec<&Row>)> = grouped.into_values().collect();
    games.sort_by_key(|(ordinal, _)| *ordinal);

    let target = window_seats.max(1);
    let mut end = games.len();
    let mut tranches = Vec::new();
    while end > 0 && tranches.len() < window_count {
        let mut start = end;
        let mut count = 0usize;
        while start > 0 && count < target {
            start -= 1;
            count += games[start].1.len();
        }
        // Choose the closest boundary, but never create an empty window.
        if count > target {
            let without_first = count - games[start].1.len();
            if without_first > 0 && target - without_first < count - target {
                start += 1;
            }
        }
        let tranche_rows: Vec<Row> = games[start..end]
            .iter()
            .flat_map(|(_, seats)| seats.iter().map(|row| (*row).clone()))
            .collect();
        let estimates = estimate(header, &tranche_rows);
        tranches.push(ReproTranche {
            seats: estimates.seats,
            games: estimates.games,
            estimates: estimates.genes,
        });
        end = start;
    }
    tranches
}

fn tranche_estimate<'a>(tranche: &'a ReproTranche, tag: &str) -> Option<&'a GeneEstimate> {
    tranche
        .estimates
        .iter()
        .find(|estimate| estimate.tag == tag)
}

/// A compact table cell for one chronological win-rate replication tranche.
/// The effect is in percentage points and the z retains the window's
/// cluster-aware uncertainty rather than pretending the raw seat rows are
/// independent.
fn tranche_cell(tranche: Option<&ReproTranche>, tag: &str) -> String {
    match tranche.and_then(|tranche| tranche_estimate(tranche, tag)) {
        Some(estimate) => format!(
            "{:+.1}pp z{:+.2}",
            100.0 * estimate.win_delta,
            estimate.win_z()
        ),
        None => "—".to_string(),
    }
}

/// The family-wise 5% bar for `k` genes (Bonferroni, two-sided).
fn family_wise_z(k: usize) -> f64 {
    normal_quantile_upper(0.025 / k.max(1) as f64)
}

/// The |z| a reading needs before the run that produced it was powered to
/// find an effect that size.
///
/// [`POWER_FACTOR`] is 2.8 standard errors, so "the smallest Δ this run
/// resolves at 80% power" and "|z| = 2.8" are the same statement about the
/// same row. Anything below it is a difference the run could have missed.
const POWERED_Z: f64 = 2.8;

/// ⚠⚠ A VERDICT MUST NOT OUTRUN THE POWER THAT PRODUCED IT.
///
/// The bars below are significance bars: |z| ≥ 2 for a flag, the family-wise
/// bar for a starred one, both at α = 0.05. The run's own resolving power is a
/// different quantity — [`POWERED_Z`] standard errors — and it is the LARGER
/// of the two whenever a single gene is screened, because the family-wise bar
/// for one gene is 1.96.
///
/// So a row could read `HELPS **` while sitting below the smallest effect its
/// run was powered to detect, which is the regime where a significant estimate
/// is most likely to be an overestimate. That is not hypothetical:
/// `docs/gene_screens/fires/defensible-sites.json` was written on 2026-08-25
/// reading
///
/// ```text
/// win_delta_pp +42.9   win_resolves_pp 57.1   read "HELPS **"
/// ```
///
/// a forty-three point reading, a family-wise verdict, and a run that cannot
/// resolve anything under fifty-seven. #2465 taught the artifact to carry its
/// resolving power; this teaches the verdict to consult it.
///
/// Underpowered readings keep their flag and gain a word: `helps * (thin)`.
/// Nothing is suppressed — the difference is still real and still reported —
/// but no row can now assert a starred verdict the run could not support, and
/// `**` is reserved for readings that clear both bars.
fn read_column(win_z: f64, share_z: f64, family_z: f64) -> String {
    let word = |z: f64| -> Option<String> {
        let thin = z.abs() < POWERED_Z;
        let verdict = if z.abs() >= family_z && !thin {
            if z > 0.0 {
                "HELPS **"
            } else {
                "HURTS **"
            }
        } else if z.abs() >= 2.0 {
            if z > 0.0 {
                "helps *"
            } else {
                "hurts *"
            }
        } else {
            return None;
        };
        Some(if thin {
            format!("{verdict} (thin)")
        } else {
            verdict.to_string()
        })
    };
    match (word(win_z), word(share_z)) {
        (None, None) => "~".to_string(),
        (Some(win), None) => win,
        (None, Some(share)) => format!("share {share}"),
        (Some(win), Some(share)) => format!("{win} · share {share}"),
    }
}

/// One two-factor interaction between screened genes.
#[derive(Clone, Debug)]
struct Interaction {
    a: usize,
    b: usize,
    /// How much more gene `b` is worth when gene `a` is on (and symmetrically),
    /// on the outcome's own scale. `4γ` in the centred ±1 parameterisation.
    synergy: f64,
    se: f64,
}

impl Interaction {
    fn z(&self) -> f64 {
        if self.se > 0.0 && self.se.is_finite() {
            self.synergy / self.se
        } else {
            0.0
        }
    }
}

/// Every two-factor interaction, estimated marginally from the seats.
///
/// Write the outcome as `y = μ + Σβᵢzᵢ + Σγᵢⱼzᵢzⱼ` with `zᵢ` the ±1 gene sign
/// CENTRED on its draw probability (`sᵢ − (2pᵢ − 1)`), so that under the
/// independent draw every product `zᵢzⱼ` is uncorrelated with every main
/// effect and every other product in expectation. Each `γᵢⱼ` is then the
/// marginal regression of the centred outcome on `zᵢzⱼ`, errors clustered by
/// game. A hundred genes have 4,950 two-factor terms and no affordable run
/// fits them all at once, so the marginal estimate is what is affordable; what
/// it pays is variance, since every other interaction and the game's own
/// difficulty sit in the residual. The reported figure is `4γ`: **how much
/// more one gene is worth when the other is on**.
///
/// ⚠ Far noisier than the main effects from the same run. Read the
/// multiplicity bar, not the top row.
fn interactions(
    header: &Header,
    rows: &[Row],
    outcome: fn(&Row) -> f64,
) -> (Vec<Interaction>, usize) {
    let seats = Seats::of(header, rows);
    let n = seats.rows.len();
    // A product of two centred signs needs all four on/off cells populated
    // before its ratio estimate means anything; below a few dozen seats the
    // scan prints arithmetic, not evidence.
    if n < 30 {
        return (Vec::new(), n);
    }
    let probabilities = header.on_probabilities();
    let centres: Vec<f64> = seats
        .columns
        .iter()
        .map(|&i| 2.0 * probabilities[i] - 1.0)
        .collect();
    let y: Vec<f64> = seats.rows.iter().map(|row| outcome(row)).collect();
    let mean = y.iter().sum::<f64>() / n as f64;
    let centred: Vec<f64> = y.iter().map(|value| value - mean).collect();
    let width = seats.columns.len();
    let mut found = Vec::with_capacity(width * width / 2);
    for a in 0..width {
        for b in a + 1..width {
            let w: Vec<f64> = seats
                .signs
                .iter()
                .map(|row| (row[a] - centres[a]) * (row[b] - centres[b]))
                .collect();
            let ww: f64 = w.iter().map(|x| x * x).sum();
            if ww <= 0.0 {
                continue;
            }
            let gamma = w.iter().zip(&centred).map(|(x, y)| x * y).sum::<f64>() / ww;
            // Cluster-robust error of the ratio estimator: each game's score
            // summed whole before squaring.
            let mut scores: BTreeMap<GameKey, f64> = BTreeMap::new();
            for ((x, y), key) in w.iter().zip(&centred).zip(&seats.clusters) {
                *scores.entry(*key).or_insert(0.0) += x * (y - gamma * x);
            }
            let se = scores.values().map(|s| s * s).sum::<f64>().sqrt() / ww;
            found.push(Interaction {
                a,
                b,
                synergy: 4.0 * gamma,
                se: 4.0 * se,
            });
        }
    }
    (found, n)
}

/// Print the strongest two-factor interactions on one outcome.
fn print_interactions(
    header: &Header,
    rows: &[Row],
    label: &str,
    scale: f64,
    unit: &str,
    outcome: fn(&Row) -> f64,
    top: usize,
) {
    let (mut found, seats) = interactions(header, rows, outcome);
    if found.is_empty() {
        println!("\ninteractions ({label}): not enough seats");
        return;
    }
    let screened = header.screened_mask();
    let names: Vec<&String> = header
        .genes
        .iter()
        .zip(screened)
        .filter(|(_, screened)| *screened)
        .map(|(gene, _)| gene)
        .collect();
    let tests = found.len();
    let family_z = normal_quantile_upper(0.025 / tests as f64);
    found.sort_by(|a, b| b.z().abs().total_cmp(&a.z().abs()));
    let flagged = found.iter().filter(|row| row.z().abs() >= family_z).count();
    // ⚠ THE COUNT AT |z|≥2 IS THE ONLY HONEST HEADLINE HERE, and it is a
    // count against an expectation, not a list of exciting rows. 4,950 tests
    // throw ~225 flags at |z|≥2 with nothing whatever going on, so a table
    // that printed its top twelve without this line would read as twelve
    // findings every single time it was run — including on pure noise.
    let loose = found.iter().filter(|row| row.z().abs() >= 2.0).count();
    let expected_loose = tests as f64 * 0.0455;
    println!(
        "\ntwo-factor interactions on {label} · {seats} seats · {tests} gene pairs tested · \
         {loose} at |z|≥2 against {expected_loose:.0} expected by chance · \
         {flagged} past the family-wise bar |z|≥{family_z:.2} against {:.2} expected{}",
        0.05,
        if loose as f64 <= expected_loose && flagged == 0 {
            " ⇒ THIS LAYER IS INDISTINGUISHABLE FROM NOISE at this size; the rows below are the loudest noise, not findings"
        } else {
            ""
        }
    );
    println!(
        "estimated marginally from the same seats as the main table; the figure is how much more \
         one gene is worth when the other is on"
    );
    println!(
        "\n{:<28} {:<28} {:>10} {:>8} {:>7}  read",
        "gene", "with", "synergy", "±se", "z"
    );
    for row in found.iter().take(top) {
        let z = row.z();
        let read = if z.abs() >= family_z {
            if z > 0.0 {
                "SYNERGY **"
            } else {
                "ANTAGONISM **"
            }
        } else if z.abs() >= 2.0 {
            if z > 0.0 {
                "synergy *"
            } else {
                "antagonism *"
            }
        } else {
            "~"
        };
        println!(
            "{:<28} {:<28} {:>+9.2}{unit} {:>7.2} {:>+7.2}  {}",
            names[row.a],
            names[row.b],
            scale * row.synergy,
            scale * row.se,
            z,
            read
        );
    }
    if tests > top {
        println!("… {} weaker pairs not shown (--top N)", tests - top);
    }
}

fn print_table(header: &Header, rows: &[Row]) {
    let estimates = estimate(header, rows);
    let costs = estimate_costs(header, rows);
    println!(
        "\ngene screen · {} seats in {} games · {}p {}x{} {} · {} · {} turns · {} city-states · {} · {} · design {}",
        estimates.seats,
        estimates.games,
        header.players,
        header.width,
        header.height,
        header.map,
        header.speed,
        header.turns,
        header.city_states,
        if header.randomize_civs {
            "shuffled civs"
        } else {
            "stock-seated civs"
        },
        if header.victories.is_empty()
            || header.victories.split(',').count() == civvis::game::VictoryConditions::NAMES.len()
        {
            "all lanes".to_string()
        } else {
            format!("lanes {}", header.victories)
        },
        header.design
    );
    println!("{}", build_line(header));
    println!("{}", field_line(header));
    println!("{}", mask_line(header));
    println!("{}", difficulty_line(header));
    println!("{}", rivals_line(header));
    println!("{}", completeness_line(header, estimates.seats));
    println!(
        "a seat overall: win {:.1}% (chance {:.1}%) · score share {:.1}% (equal share {:.1}%)",
        100.0 * estimates.overall_win,
        100.0 / header.players as f64,
        100.0 * estimates.overall_share,
        100.0 / header.players as f64
    );
    if header.design == "independent" {
        let screened = header.screened_mask();
        let mut by_p: BTreeMap<String, usize> = BTreeMap::new();
        for (p, &s) in header.prior.iter().zip(&screened) {
            if s {
                *by_p.entry(format!("{:.2}", p)).or_default() += 1;
            }
        }
        println!(
            "draw: every seat's genome independent, genes on at p = {}; Δ is the seats-on against \
             seats-off difference, errors clustered by game; adjΔpp is the OLS over every screened gene",
            by_p
                .iter()
                .map(|(p, n)| format!("{p}×{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    {
        // The regime, so a table is never read without knowing what decided
        // its games: two thirds of native 4p games end by conversion before
        // a siege can matter, and that is visible only here.
        let screened_rows: Vec<&Row> = rows.iter().filter(|row| row.kind == "game").collect();
        let mut census: BTreeMap<&str, (usize, Vec<u32>)> = BTreeMap::new();
        for row in &screened_rows {
            let entry = census
                .entry(row.victory.as_str())
                .or_insert((0, Vec::new()));
            entry.0 += 1;
            entry.1.push(row.turn);
        }
        let mut kinds: Vec<_> = census.into_iter().collect();
        kinds.sort_by_key(|kind| std::cmp::Reverse(kind.1 .0));
        let parts: Vec<String> = kinds
            .iter()
            .map(|(kind, (count, turns))| {
                let mut turns = turns.clone();
                turns.sort_unstable();
                format!(
                    "{} {} ({:.0}%, median t{})",
                    if kind.is_empty() { "unfinished" } else { kind },
                    count,
                    100.0 * *count as f64 / screened_rows.len().max(1) as f64,
                    turns[turns.len() / 2]
                )
            })
            .collect();
        if !parts.is_empty() {
            println!("how the games ended (by seat): {}", parts.join(" · "));
        }
        print_drift_meter(rows);
        // ⭐ THE TWO LANES THE LIVE SEAT ACTUALLY LOSES TO. Printed whenever
        // the rows carry the fields, fieldless or contested, so the standard
        // screen's own answer to "was anybody even racing?" is visible beside
        // the contested one instead of having to be argued.
        let instrumented = screened_rows
            .iter()
            .any(|row| row.dvp > 0 || row.rival_dvp > 0 || row.tourists > 0 || row.domestic > 0);
        if instrumented && !screened_rows.is_empty() {
            let n = screened_rows.len() as f64;
            let mean =
                |f: fn(&Row) -> i64| screened_rows.iter().map(|row| f(row) as f64).sum::<f64>() / n;
            let best =
                |f: fn(&Row) -> i64| screened_rows.iter().map(|row| f(row)).max().unwrap_or(0);
            let share = |f: fn(&Row) -> bool| {
                100.0 * screened_rows.iter().filter(|row| f(row)).count() as f64 / n
            };
            println!(
                "the contested lanes: DVP own {:.1} (best {}) · best rival {:.1} (best {}) of the {} a \
                 diplomatic victory needs — somebody held all {} in {:.0}% of seats · visiting \
                 tourists own {:.0}, best rival {:.0}, against this seat's own {:.0} domestic \
                 (the bar a culture victory has to clear)",
                mean(|row| row.dvp),
                best(|row| row.dvp),
                mean(|row| row.rival_dvp),
                best(|row| row.rival_dvp),
                DIPLOMATIC_VICTORY_POINTS,
                DIPLOMATIC_VICTORY_POINTS,
                share(|row| row.dvp >= DIPLOMATIC_VICTORY_POINTS
                    || row.rival_dvp >= DIPLOMATIC_VICTORY_POINTS),
                mean(|row| row.tourists),
                mean(|row| row.rival_tourists),
                mean(|row| row.domestic),
            );
            println!(
                "  what a denial gene has to reduce: {:.1}% of measured seats lost to a rival's \
                 DIPLOMATIC victory, {:.1}% to a rival's CULTURE victory, {:.1}% to a rival's \
                 religion, and {:.1}% won something themselves",
                share(|row| lost_to(row, "diplomatic")),
                share(|row| lost_to(row, "culture")),
                share(|row| lost_to(row, "religious")),
                share(|row| row.win),
            );
        }
    }
    {
        // The religion census: how a seat stood in the race that decides so
        // many of these games. Printed only when the rows carry it, so a file
        // written before the instrumentation still analyses.
        let played: Vec<&Row> = rows.iter().filter(|row| row.kind == "game").collect();
        let instrumented = played
            .iter()
            .any(|row| row.founded_religion || row.faith > 0.0 || row.techs > 0);
        if instrumented && !played.is_empty() {
            let n = played.len() as f64;
            let founded = played.iter().filter(|row| row.founded_religion).count();
            let lost_to_faith: Vec<&&Row> = played
                .iter()
                .filter(|row| !row.win && row.victory == "religious")
                .collect();
            let mean = |rows: &[&&Row], f: fn(&Row) -> f64| {
                if rows.is_empty() {
                    0.0
                } else {
                    rows.iter().map(|row| f(row)).sum::<f64>() / rows.len() as f64
                }
            };
            println!(
                "religion census: founded a faith in {:.0}% of seats · inquisition launched in {:.0}% · \
                 own cities under a foreign faith at the end {:.1} of {:.1} · faith left banked {:.0}",
                100.0 * founded as f64 / n,
                100.0 * played.iter().filter(|row| row.inquisition).count() as f64 / n,
                played.iter().map(|row| row.foreign_faith_cities as f64).sum::<f64>() / n,
                played.iter().map(|row| row.cities as f64).sum::<f64>() / n,
                played.iter().map(|row| row.faith).sum::<f64>() / n,
            );
            if !lost_to_faith.is_empty() {
                println!(
                    "  of the {} seats lost to a rival's religion: {:.0}% had founded one of our own, \
                     {:.1} of {:.1} cities had flipped, {:.0} faith was still banked",
                    lost_to_faith.len(),
                    100.0
                        * lost_to_faith
                            .iter()
                            .filter(|row| row.founded_religion)
                            .count() as f64
                        / lost_to_faith.len() as f64,
                    mean(&lost_to_faith, |row| row.foreign_faith_cities as f64),
                    mean(&lost_to_faith, |row| row.cities as f64),
                    mean(&lost_to_faith, |row| row.faith),
                );
            }
        }
    }
    {
        // ★ The wonder census. `Game::score_parts` pays 15 points a wonder, the
        // densest line of a tally that decides three quarters of these games,
        // and the `Item::Wonder` arm refuses one unless a narrow set of gates
        // opens. This says whether a seat ever built one — the actuation
        // question — and is printed only when the rows carry the field.
        let played: Vec<&Row> = rows.iter().filter(|row| row.kind == "game").collect();
        if played.iter().any(|row| row.wonders > 0) {
            let n = played.len() as f64;
            let built = played.iter().filter(|row| row.wonders > 0).count();
            let total: usize = played.iter().map(|row| row.wonders).sum();
            let mut by_civ: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
            for row in &played {
                let entry = by_civ.entry(row.civ.as_str()).or_default();
                entry.0 += 1;
                entry.1 += row.wonders;
            }
            let civs = by_civ
                .iter()
                .filter(|(civ, _)| !civ.is_empty())
                .map(|(civ, (seats, wonders))| {
                    format!("{civ} {:.2}", *wonders as f64 / (*seats).max(1) as f64)
                })
                .collect::<Vec<_>>()
                .join(" · ");
            println!(
                "wonder census: {:.1}% of seats finished a wonder · {:.2} a seat · {:.0} tally points a seat",
                100.0 * built as f64 / n.max(1.0),
                total as f64 / n.max(1.0),
                15.0 * total as f64 / n.max(1.0),
            );
            if !civs.is_empty() {
                println!("  wonders per seat by civilization: {civs}");
            }
        }
    }
    let mut genes = estimates.genes;
    if genes.is_empty() {
        println!("no screened genes with seats");
        return;
    }
    let tranches = reproducibility_tranches(header, rows, REPRO_WINDOW_SEATS, REPRO_WINDOW_COUNT);
    let tranche_sizes = tranches
        .iter()
        .enumerate()
        .map(|(index, tranche)| {
            let label = match index {
                0 => "latest",
                1 => "previous",
                _ => "earlier",
            };
            format!("{label}={} seats/{} games", tranche.seats, tranche.games)
        })
        .collect::<Vec<_>>()
        .join(", ");
    println!(
        "reproducibility windows (newest first): {tranche_sizes}; target {REPRO_WINDOW_SEATS} seats each, \
         rounded only to keep every game whole"
    );
    let k = genes.len();
    let family_z = family_wise_z(k);
    let median_se = {
        let mut ses: Vec<f64> = genes
            .iter()
            .map(|e| e.win_se)
            .filter(|se| se.is_finite())
            .collect();
        ses.sort_by(|a, b| a.total_cmp(b));
        ses.get(ses.len() / 2).copied().unwrap_or(f64::INFINITY)
    };
    let median_share_se = {
        let mut ses: Vec<f64> = genes
            .iter()
            .map(|e| e.share_se)
            .filter(|se| se.is_finite())
            .collect();
        ses.sort_by(|a, b| a.total_cmp(b));
        ses.get(ses.len() / 2).copied().unwrap_or(f64::INFINITY)
    };
    println!(
        "resolution: {} genes; this run resolves a win Δ of ±{:.1} pp (share Δ ±{:.2} pp) at 80% power; \
         |z|≥2 flags ~{:.1} genes by chance, family-wise 5% bar is |z|≥{:.2}",
        k,
        POWER_FACTOR * median_se,
        POWER_FACTOR * median_share_se,
        k as f64 * 0.0455,
        family_z
    );
    let adjusted_shown = genes.iter().any(|e| e.adjusted.is_some());
    if !adjusted_shown {
        println!(
            "adjusted column needs at least {} seats (genes+11) — showing marginal estimates only",
            k + 11
        );
    }
    genes.sort_by(|a, b| b.win_z().total_cmp(&a.win_z()));
    println!(
        "\n{:<28} {:>13} {:>6} {:>6} {:>16} {:>16} {:>16} {:>15} {:>6}  {:>8} {:>6}  {:>9}  {:>16} {:>16}  read",
        "gene",
        "on n/off n",
        "on%",
        "off%",
        "latest 20k",
        "previous 20k",
        "earlier 20k",
        "all 95% CI",
        "z",
        "shareΔ",
        "z",
        "adjΔpp",
        "compute cost",
        "time cost"
    );
    for e in &genes {
        let z = e.win_z();
        let read = read_column(z, e.share_z(), family_z);
        let adjusted = match e.adjusted {
            Some((effect, se)) => format!("{:+.1}±{:.1}", 100.0 * effect, 100.0 * se),
            None => "-".to_string(),
        };
        let count = format!("{}/{}", e.n_on, e.n_off);
        let latest = tranche_cell(tranches.first(), &e.tag);
        let previous = tranche_cell(tranches.get(1), &e.tag);
        let earlier = tranche_cell(tranches.get(2), &e.tag);
        let (compute_cost, time_cost) = match costs.get(&e.tag) {
            Some(cost) => (
                format!("{:+.2}±{:.2}%", cost.compute_pct(), cost.compute_se_pct()),
                format!("{:+.2}±{:.2}%", cost.time_pct(), cost.time_se_pct()),
            ),
            None => ("-".to_string(), "-".to_string()),
        };
        println!(
            "{:<28} {:>13} {:>5.1}% {:>5.1}% {:>16} {:>16} {:>16} [{:>+6.1},{:>+6.1}] {:>+6.2}  {:>+7.2}pp {:>+6.2}  {:>9}  {:>16} {:>16}  {}",
            e.tag,
            count,
            100.0 * e.win_on,
            100.0 * e.win_off,
            latest,
            previous,
            earlier,
            100.0 * (e.win_delta - 1.96 * e.win_se),
            100.0 * (e.win_delta + 1.96 * e.win_se),
            z,
            100.0 * e.share_delta,
            e.share_z(),
            adjusted,
            compute_cost,
            time_cost,
            read
        );
    }
    println!(
        "\n`*` = |z|≥2 (a screen flag, ~1 in 22 by chance); `**` = past the family-wise bar; the read \
         column names the win Δ first and the score-share Δ when it says more. `~` = unresolved at \
         this size, NOT no effect. Each 20k cell is that window's win Δpp / clustered z; on% minus \
         off% is the win Δ. shareΔ is the score-share Δ in points; adjΔpp is the OLS win Δ over \
         every screened gene at once. Cost cells are percent change per enabled major seat ± one \
         standard error: compute is wall seconds per completed turn, time is whole-game wall \
         seconds, and positive is slower."
    );
    print_families(header, rows);
    print_victory_masks(header, rows);
    print_difficulty_rungs(header, rows);
    print_rival_mix(header, rows);
}

/// The family tables: for every versioned gene, what each level did and
/// whether each version beats `off` and the version before it.
fn print_families(header: &Header, rows: &[Row]) {
    let families = estimate_families(header, rows);
    if families.is_empty() {
        return;
    }
    println!(
        "\nversioned genes · a seat plays at most one version of a family; every contrast is \
         seats-on-b against seats-on-a, errors clustered by game"
    );
    for family in &families {
        let cells = family
            .cells
            .iter()
            .map(|cell| {
                format!(
                    "{} {} seats win {:.1}% share {:.1}%",
                    cell.label,
                    cell.seats,
                    100.0 * cell.win,
                    100.0 * cell.share
                )
            })
            .collect::<Vec<_>>()
            .join(" · ");
        println!("\n{}: {}", family.base, cells);
        for contrast in &family.contrasts {
            println!(
                "  {} − {}: win {:+.1} pp ± {:.1} (z {:+.2}) · share {:+.2} pp (z {:+.2})",
                contrast.b,
                contrast.a,
                100.0 * contrast.win_delta,
                100.0 * contrast.win_se,
                contrast.win_z(),
                100.0 * contrast.share_delta,
                contrast.share_z()
            );
        }
    }
    println!(
        "\nan improvement improves when its \"against the version before it\" contrast is \
         positive on the win axis and it also beats off; the ledger keeps one version of a \
         family on — the best by tracked wins — and never two, and the draw plays that best \
         version {:.0}% of the time the family is on, the rest sharing the remaining {:.0}%",
        100.0 * BEST_VERSION_SHARE,
        100.0 * (1.0 - BEST_VERSION_SHARE)
    );
}

/// One gene's contrast split by the civilization the seat played — the
/// subgroup the marginal table averages away: a flag can be worth nothing on
/// average and still be a real strategy for one civilization, or the reverse.
/// Errors cluster by game as everywhere else.
fn print_by_civ(header: &Header, rows: &[Row], tag: &str) {
    let Some(index) = header.genes.iter().position(|gene| gene == tag) else {
        println!("\nby-civ: {tag:?} is not a gene in this file's header");
        return;
    };
    if !header.screened.iter().any(|gene| gene == tag) {
        println!("\nby-civ: {tag:?} was not screened in this file");
        return;
    }
    let k = header.genes.len();
    let mut by_civ: BTreeMap<&str, Vec<&Row>> = BTreeMap::new();
    let mut unlabelled = 0usize;
    for row in rows
        .iter()
        .filter(|row| row.kind == "game" && row.genome.chars().count() == k)
    {
        if row.civ.is_empty() {
            unlabelled += 1;
            continue;
        }
        by_civ.entry(row.civ.as_str()).or_default().push(row);
    }
    if unlabelled > 0 {
        println!(
            "\nby-civ: {unlabelled} seats have no civ on their rows (written before the field existed) and are left out"
        );
    }
    if by_civ.is_empty() {
        println!("\nby-civ: no labelled seats");
        return;
    }
    let civs = by_civ.len();
    let family_z = normal_quantile_upper(0.025 / civs as f64);
    println!(
        "\n{tag} by civilization · {} civs · family-wise 5% bar |z|≥{family_z:.2} (a subgroup scan: treat a flag as where to point a run, not a finding)",
        civs
    );
    println!(
        "{:<16} {:>6} {:>7} {:>15} {:>6}  {:>8} {:>6}  read",
        "civ", "seats", "Δpp", "95% CI", "z", "shareΔ", "z"
    );
    let mut table: Vec<(&str, usize, f64, f64, f64, f64)> = by_civ
        .iter()
        .map(|(civ, seats)| {
            let design: Vec<Vec<f64>> = seats
                .iter()
                .map(|row| vec![1.0, if row.bits()[index] { 1.0 } else { -1.0 }])
                .collect();
            let clusters: Vec<GameKey> = seats.iter().map(|row| row.game_key()).collect();
            let wins: Vec<f64> = seats
                .iter()
                .map(|row| f64::from(u8::from(row.win)))
                .collect();
            let shares: Vec<f64> = seats.iter().map(|row| row.score_share).collect();
            let contrast = |y: &[f64]| match ols_clustered(&design, y, &clusters) {
                Some(fit) => (2.0 * fit[1].0, 2.0 * fit[1].1),
                None => (0.0, f64::INFINITY),
            };
            let (wd, wse) = contrast(&wins);
            let (sd, sse) = contrast(&shares);
            (
                *civ,
                seats.len(),
                wd,
                wse,
                sd,
                if sse > 0.0 && sse.is_finite() {
                    sd / sse
                } else {
                    0.0
                },
            )
        })
        .collect();
    table.sort_by(|a, b| {
        let za = if a.3 > 0.0 && a.3.is_finite() {
            a.2 / a.3
        } else {
            0.0
        };
        let zb = if b.3 > 0.0 && b.3.is_finite() {
            b.2 / b.3
        } else {
            0.0
        };
        zb.total_cmp(&za)
    });
    for (civ, seats, wd, wse, sd, sz) in table {
        let z = if wse > 0.0 && wse.is_finite() {
            wd / wse
        } else {
            0.0
        };
        println!(
            "{:<16} {:>6} {:>+7.1} [{:>+6.1},{:>+6.1}] {:>+6.2}  {:>+7.2}pp {:>+6.2}  {}",
            civ,
            seats,
            100.0 * wd,
            100.0 * (wd - 1.96 * wse),
            100.0 * (wd + 1.96 * wse),
            z,
            100.0 * sd,
            sz,
            read_column(z, sz, family_z)
        );
    }
}

/// The analysis as data: one object per screened gene with the numbers the
/// table prints, plus the profile. `tools/genes.py` reads this to
/// build `docs/gene_ledger.json` and the generated Rust table, so the
/// deployment genome is derived from the screens rather than typed in.
/// Two-sided 80% power at α = 0.05 needs about 2.8 standard errors, and the
/// estimates here are proportions, so a percentage-point figure is 280 × SE.
///
/// ⚠⚠ THIS NUMBER IS WHY A SIX-GAME PROBE CANNOT PRICE A GENE, AND UNTIL
/// 2026-08-25 IT LIVED ONLY IN THE TERMINAL. A twelve-game single-gene probe
/// resolves a win Δ of about **±28.6 pp**; the ninety-game nine-gene screen
/// beside it resolves **±10.3 pp**. Nine genes probed at twelve games read
/// between +22.2 and −21.1 pp and every one of those readings was inside its
/// own run's noise. Re-measured together at 540 seats, eight of the nine came
/// back indistinguishable from zero — `conversion-majority-alarm` from +22.2
/// to +0.2, `diplomatic-lane-forecast` from +18.5 to −0.8.
///
/// `docs/gene_screens/fires/*.json` recorded the point estimates and not this,
/// so a committed probe carried a Δ with nothing beside it saying whether the
/// run could have resolved it. Now it carries both.
fn resolving_power(standard_error: f64) -> Option<f64> {
    standard_error
        .is_finite()
        .then_some(POWER_FACTOR * standard_error)
}

/// The median standard error across the screened genes, which is what the
/// printed `resolution:` line reports for the run as a whole.
fn median_win_se(genes: &[GeneEstimate]) -> f64 {
    median_se(genes.iter().map(|gene| gene.win_se))
}

fn median_share_se(genes: &[GeneEstimate]) -> f64 {
    median_se(genes.iter().map(|gene| gene.share_se))
}

fn median_se(values: impl Iterator<Item = f64>) -> f64 {
    let mut finite: Vec<f64> = values.filter(|se| se.is_finite()).collect();
    finite.sort_by(|left, right| left.total_cmp(right));
    finite
        .get(finite.len() / 2)
        .copied()
        .unwrap_or(f64::INFINITY)
}

fn write_json_summary(path: &str, header: &Header, rows: &[Row]) {
    let estimates = estimate(header, rows);
    let costs = estimate_costs(header, rows);
    let tranches = reproducibility_tranches(header, rows, REPRO_WINDOW_SEATS, REPRO_WINDOW_COUNT);
    let family_z = family_wise_z(estimates.genes.len());
    let genes: Vec<serde_json::Value> = estimates
        .genes
        .iter()
        .map(|e| {
            let cost = costs.get(&e.tag);
            let win_tranches: Vec<serde_json::Value> = tranches
                .iter()
                .enumerate()
                .filter_map(|(index, tranche)| {
                    let estimate = tranche_estimate(tranche, &e.tag)?;
                    Some(serde_json::json!({
                        "position": match index {
                            0 => "latest",
                            1 => "previous",
                            _ => "earlier",
                        },
                        "seats": estimate.seats(),
                        "games": tranche.games,
                        "win_delta_pp": 100.0 * estimate.win_delta,
                        "win_se_pp": 100.0 * estimate.win_se,
                        "win_z": estimate.win_z(),
                    }))
                })
                .collect();
            serde_json::json!({
                "tag": e.tag,
                "seats": e.seats(),
                "n_on": e.n_on,
                "n_off": e.n_off,
                "win_on": e.win_on,
                "win_off": e.win_off,
                "win_delta_pp": 100.0 * e.win_delta,
                "win_se_pp": 100.0 * e.win_se,
                "win_z": e.win_z(),
                // ⚠⚠ THE POINT ESTIMATE ABOVE MUST NOT TRAVEL WITHOUT THIS.
                // The smallest win Δ this row could have resolved at 80%
                // power. A `win_delta_pp` smaller than its own
                // `win_resolves_pp` is inside the noise of the run that
                // produced it, whatever its sign and however many blocks
                // agree on that sign. See `POWER_FACTOR`.
                "win_resolves_pp": resolving_power(e.win_se),
                "share_delta_pp": 100.0 * e.share_delta,
                "share_se_pp": 100.0 * e.share_se,
                "share_z": e.share_z(),
                "share_resolves_pp": resolving_power(e.share_se),
                "adjusted_pp": e.adjusted.map(|(b, _)| 100.0 * b),
                "adjusted_se_pp": e.adjusted.map(|(_, se)| 100.0 * se),
                "read": read_column(e.win_z(), e.share_z(), family_z),
                "win_tranches": win_tranches,
                "cost_games": cost.map(|c| c.games),
                "compute_cost_pct": cost.map(CostEstimate::compute_pct),
                "compute_cost_se_pct": cost.map(CostEstimate::compute_se_pct),
                "time_cost_pct": cost.map(CostEstimate::time_pct),
                "time_cost_se_pct": cost.map(CostEstimate::time_se_pct),
            })
        })
        .collect();
    let intended = header.batch.intended_seats();
    let summary = serde_json::json!({
        "kind": "gene_screen_analysis",
        "profile": header,
        // ⭐ `standard` is THE screen; anything else is a probe, and
        // `tools/genes.py` refuses it as a source.
        "shape": shape_of(header),
        // ⭐ INTENDED AGAINST ACTUAL. The target was written into the header
        // before the first game finished, so a batch that stopped early is a
        // partial screen in its own artefact rather than an unmarked one. A
        // file written before 2026-08-23 carries nulls here, which is the
        // honest answer: nothing was pre-registered, so nothing can be read
        // against it.
        "batch": {
            "target_games": (header.batch.target_games > 0).then_some(header.batch.target_games),
            "target_seats": (intended > 0).then_some(intended),
            "complete_seats": estimates.seats,
            "complete_games": estimates.games,
            "completion": (intended > 0).then(|| estimates.seats as f64 / intended as f64),
            "partial": (intended > 0).then_some(estimates.seats < intended),
            "seed_window": header.batch.pre_registered()
                .then_some([header.batch.seed_first, header.batch.seed_last]),
            "seed_window_played": played_seed_window(rows).map(|(low, high)| [low, high]),
            "read": completeness_line(header, estimates.seats),
        },
        "seats": estimates.seats,
        "games": estimates.games,
        "endings": ending_census(rows),
        "drift": drift_meter(rows).map(|meter| meter.json()),
        "victory_masks": mask_report(header, rows).map(|report| report.json()),
        "difficulty": rung_report(header, rows).map(|report| report.json()),
        "rival_mix": rival_report(header, rows).map(|report| report.json()),
        "overall_win": estimates.overall_win,
        "overall_share": estimates.overall_share,
        "family_wise_z": family_z,
        // The `resolution:` line this run prints, kept rather than left in the
        // terminal. Everything an artifact needs to be read honestly on its
        // own is now in the artifact.
        "resolution": {
            "genes": estimates.genes.len(),
            "win_pp": resolving_power(median_win_se(&estimates.genes)),
            "share_pp": resolving_power(median_share_se(&estimates.genes)),
            "power": 0.8,
            "alpha": 0.05,
            "expected_flags_by_chance": estimates.genes.len() as f64 * 0.0455,
            "read": "the smallest Δ this run could resolve at 80% power; a Δ \
                     below it is inside the run's own noise",
        },
        "reproducibility": {
            "unit": "seats",
            "target_seats_per_window": REPRO_WINDOW_SEATS,
            "windows": "newest first; whole games only",
        },
        "families": estimate_families(header, rows)
            .iter()
            .map(|family| {
                serde_json::json!({
                    "base": family.base,
                    "versions": family.versions,
                    "cells": family.cells.iter().map(|cell| serde_json::json!({
                        "label": cell.label,
                        "seats": cell.seats,
                        "win": cell.win,
                        "share": cell.share,
                    })).collect::<Vec<_>>(),
                    "contrasts": family.contrasts.iter().map(|c| serde_json::json!({
                        "a": c.a,
                        "b": c.b,
                        "win_delta_pp": 100.0 * c.win_delta,
                        "win_se_pp": 100.0 * c.win_se,
                        "win_z": c.win_z(),
                        "share_delta_pp": 100.0 * c.share_delta,
                        "share_se_pp": 100.0 * c.share_se,
                        "share_z": c.share_z(),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>(),
        "cost_method": {
            "unit": "percent change per enabled major seat; positive is slower",
            "compute": "log of wall seconds per completed turn, per game",
            "time": "log of whole-game wall seconds, per game",
            "fit": "OLS on how many seats had each screened gene on, with an intercept; one observation per game with HC1 robust errors",
        },
        "genes": genes,
    });
    std::fs::write(
        path,
        serde_json::to_string_pretty(&summary).expect("summary serializes"),
    )
    .unwrap_or_else(|error| {
        eprintln!("cannot write {path}: {error}");
        std::process::exit(2);
    });
    println!("analysis written to {path}");
}

fn read_rows(paths: &[String]) -> (Header, Vec<Row>) {
    let mut header: Option<Header> = None;
    // Pre-registration is per segment, keyed by the seed window it reserved:
    // an `--append` session declares its own share of the screen on its own
    // disjoint start seed, and the screen's intended size is their sum. A
    // header rewritten over the same window — the same segment restarted —
    // counts once rather than twice.
    let mut targets: BTreeMap<u64, Batch> = BTreeMap::new();
    let mut rows = Vec::new();
    for path in paths {
        let file = std::fs::File::open(path).unwrap_or_else(|error| {
            eprintln!("cannot open {path}: {error}");
            std::process::exit(2);
        });
        for (line_no, line) in std::io::BufReader::new(file).lines().enumerate() {
            let line = line.unwrap_or_else(|error| {
                eprintln!("{path}:{}: {error}", line_no + 1);
                std::process::exit(2);
            });
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(found) = serde_json::from_str::<Header>(&line) {
                if found.kind == "header" {
                    if found.batch.pre_registered() {
                        targets.insert(found.batch.seed_first, found.batch.clone());
                    }
                    match &header {
                        None => header = Some(found),
                        Some(first) => {
                            if first.genes != found.genes {
                                eprintln!(
                                    "{path} was written with a different gene order than {}; \
                                     it cannot be merged (regenerate both with the same build)",
                                    paths[0]
                                );
                                std::process::exit(2);
                            }
                            if first.players != found.players
                                || first.width != found.width
                                || first.height != found.height
                                || first.turns != found.turns
                                || first.speed != found.speed
                                || first.map != found.map
                                || first.baseline != found.baseline
                                || first.randomize_civs != found.randomize_civs
                                || first.victories != found.victories
                                || first.victory_mask != found.victory_mask
                                || first.difficulty != found.difficulty
                                || first.difficulty_rotate != found.difficulty_rotate
                                || first.rivals != found.rivals
                                || first.all_seats != found.all_seats
                                || first.design != found.design
                                || first.prior != found.prior
                            {
                                eprintln!(
                                    "{path} was played at a different profile or draw than {}; a merged \
                                     table would mix two experiments",
                                    paths[0]
                                );
                                std::process::exit(2);
                            }
                        }
                    }
                    continue;
                }
            }
            match serde_json::from_str::<Row>(&line) {
                Ok(row) => rows.push(row),
                Err(error) => {
                    eprintln!("{path}:{}: not a row: {error}", line_no + 1);
                    std::process::exit(2);
                }
            }
        }
    }
    let Some(mut header) = header else {
        eprintln!("no header line found; was this file written by gene_screen?");
        std::process::exit(2);
    };
    header.batch = merged_target(&targets);
    (header, rows)
}

/// The whole screen's pre-registration, from every segment that declared one.
/// Empty when no merged file carried a target, which is what a file written
/// before 2026-08-23 looks like.
fn merged_target(targets: &BTreeMap<u64, Batch>) -> Batch {
    Batch {
        target_games: targets.values().map(|batch| batch.target_games).sum(),
        target_seats: targets.values().map(|batch| batch.target_seats).sum(),
        target_pairs: targets.values().map(|batch| batch.target_pairs).sum(),
        target_comparisons: targets.values().map(|batch| batch.target_comparisons).sum(),
        seed_first: targets
            .values()
            .map(|batch| batch.seed_first)
            .min()
            .unwrap_or(0),
        seed_last: targets
            .values()
            .map(|batch| batch.seed_last)
            .max()
            .unwrap_or(0),
    }
}

/// The seed window a file's finished games actually cover, or `None` when it
/// holds no game rows.
fn played_seed_window(rows: &[Row]) -> Option<(u64, u64)> {
    let mut seeds = rows
        .iter()
        .filter(|row| row.kind == "game")
        .map(|row| row.seed);
    let first = seeds.next()?;
    Some(seeds.fold((first, first), |(low, high), seed| {
        (low.min(seed), high.max(seed))
    }))
}

/// Whether an analysis was played at the screen or is a probe. Every leg is
/// checked, because any one of them changes what a column means: the lanes
/// decide which genes can act, the map decides how the game ends, and the
/// player count decides the chance base the column is measured against. The
/// draw design is NOT a leg: a foldover file at this shape and an independent
/// one price the same genes on the same board, and the estimator reads both.
///
/// ⭐ Neither is a ROTATING VICTORY MASK (`victory_mask: "rotate:N"`). The
/// `victories` leg is the batch-level set and a rotating batch leaves all six
/// live across the batch — every lane open in (5−N)/5 of its games, every game
/// still ending on score at the clock — so it is accepted as the standard
/// screen; `tools/genes.py` records the mask on the source and reads the same
/// way. A `--victories` restriction is a second regime and stays a probe.
fn shape_of(header: &Header) -> &'static str {
    let all_lanes = header.victories.is_empty()
        || header.victories.split(',').count() == civvis::game::VictoryConditions::NAMES.len();
    let standard = header.players == SCREEN_PLAYERS
        && header.width == SCREEN_WIDTH
        && header.height == SCREEN_HEIGHT
        && header.city_states == SCREEN_CITY_STATES
        && header.map == SCREEN_MAP.id()
        && header.speed == GameSpeed::Online.id()
        && header.turns == GameSpeed::Online.turn_limit()
        && all_lanes
        && header.all_seats
        && header.randomize_civs
        // ⚠ THE CONTESTED FIELD IS A DIFFERENT WORLD, and the reason this
        // check exists at all is that a batch which looks standard in every
        // other leg would otherwise pool with the standard screen and
        // re-price a hundred genes against a board they were never measured
        // on. Both legs default to the fieldless values, so every file
        // written before 2026-08-24 keeps the shape it always had.
        && header.contested_field.is_empty()
        && !header.native_competitions
        && header.baseline == "best";
    if standard {
        "standard"
    } else {
        "legacy"
    }
}

/// ⭐ HOW THE GAMES ENDED, INTO THE ARTEFACT AND NOT ONLY THE TERMINAL.
///
/// A gene column is a claim about code and the header already proves which
/// code. The census is a claim about the BOARD, and until now it existed only
/// in a run's stdout: `docs/gene_screens/*.json` carried no record of what
/// decided its games. That is exactly the evidence a contested batch stands on
/// — "the diplomatic lane now completes" is a count, and a count belongs in the
/// file — so it is written for every batch, contested or not.
///
/// `won_by_field` is the count of games whose winner is not one of the measured
/// seats. Fieldless that is always zero; contested it is how often the pinned
/// pursuer converted, which is the difference between a threat and a label.
fn ending_census(rows: &[Row]) -> serde_json::Value {
    let played: Vec<&Row> = rows.iter().filter(|row| row.kind == "game").collect();
    if played.is_empty() {
        return serde_json::Value::Null;
    }
    let mut seats_of_game: BTreeMap<GameKey, Vec<&Row>> = BTreeMap::new();
    for row in &played {
        seats_of_game.entry(row.game_key()).or_default().push(row);
    }
    let games = seats_of_game.len();
    let mut endings: BTreeMap<&str, (usize, usize, usize, Vec<u32>)> = BTreeMap::new();
    for seats in seats_of_game.values() {
        let first = seats[0];
        let kind = if first.victory.is_empty() {
            "unfinished"
        } else {
            first.victory.as_str()
        };
        let measured: Vec<usize> = seats.iter().map(|row| row.seat).collect();
        let entry = endings.entry(kind).or_insert((0, 0, 0, Vec::new()));
        entry.0 += 1;
        entry.1 += seats.len();
        if first
            .winner
            .is_some_and(|winner| !measured.contains(&winner))
        {
            entry.2 += 1;
        }
        entry.3.push(first.turn);
    }
    let by_kind: serde_json::Map<String, serde_json::Value> = endings
        .into_iter()
        .map(|(kind, (game_count, seat_count, field_wins, mut turns))| {
            turns.sort_unstable();
            (
                kind.to_string(),
                serde_json::json!({
                    "games": game_count,
                    "seats": seat_count,
                    "share": game_count as f64 / games as f64,
                    "median_turn": turns[turns.len() / 2],
                    "won_by_field": field_wins,
                }),
            )
        })
        .collect();
    let n = played.len() as f64;
    let mean = |f: fn(&Row) -> i64| played.iter().map(|row| f(row) as f64).sum::<f64>() / n;
    let share = |f: fn(&Row) -> bool| played.iter().filter(|row| f(row)).count() as f64 / n;
    serde_json::json!({
        "unit": "games, and the seats of those games",
        "games": games,
        "by_kind": by_kind,
        "won_by_field": played
            .iter()
            .filter(|row| row.winner.is_some_and(|winner| winner != row.seat))
            .count(),
        "lanes": {
            "dvp_mean": mean(|row| row.dvp),
            "rival_dvp_mean": mean(|row| row.rival_dvp),
            "dvp_required": DIPLOMATIC_VICTORY_POINTS,
            "seats_where_somebody_reached_the_threshold": share(|row| {
                row.dvp >= DIPLOMATIC_VICTORY_POINTS || row.rival_dvp >= DIPLOMATIC_VICTORY_POINTS
            }),
            "tourists_mean": mean(|row| row.tourists),
            "rival_tourists_mean": mean(|row| row.rival_tourists),
            "domestic_mean": mean(|row| row.domestic),
        },
        "lost_to": {
            "diplomatic": share(|row| lost_to(row, "diplomatic")),
            "culture": share(|row| lost_to(row, "culture")),
            "religious": share(|row| lost_to(row, "religious")),
            "science": share(|row| lost_to(row, "science")),
        },
    })
}

/// ⭐ THE DENIAL TABLE: per gene, the change in how often this seat LOST the
/// game to a rival's victory of each kind.
///
/// The win column answers "does this seat win more". A denial gene is not for
/// winning more; it is for stopping somebody else winning, and on a six-player
/// board those are different numbers — a denial that works hands the game to
/// one of the four empires that are not us about four times out of five, and
/// the win column cannot see the difference between that and nothing
/// happening. `docs/FIDELITY.md` records the live seat losing 107 of 299
/// terminal games to a rival's victory, **15 of them while leading on score**;
/// this is the axis those games live on.
///
/// Estimated exactly like the win column — the seats-on minus seats-off
/// difference from a regression on `[1, sign]`, errors clustered by game — so
/// it is read with the same bars, and the same warning applies twice over: a
/// hundred genes at |z| >= 2 flag about 4.5 of them by chance.
fn print_denial(header: &Header, rows: &[Row], top: Option<usize>) {
    let seats = Seats::of(header, rows);
    if seats.rows.len() < 2 {
        println!("denial: too few seats to price anything");
        return;
    }
    // The lanes worth a column are the ones that actually ended games here.
    // Reading a fixed list would print four empty columns on a board where
    // nothing but religion happens, which is how a table teaches somebody the
    // wrong thing about their own instrument.
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for row in &seats.rows {
        if !row.win && !row.victory.is_empty() {
            *counts.entry(row.victory.as_str()).or_default() += 1;
        }
    }
    let mut lanes: Vec<(&str, usize)> = counts.into_iter().collect();
    lanes.sort_by_key(|(lane, count)| (std::cmp::Reverse(*count), *lane));
    lanes.retain(|(_, count)| *count > 0);
    lanes.truncate(4);
    if lanes.is_empty() {
        println!("denial: no seat in this batch lost to a rival's victory — nothing to deny");
        return;
    }
    let n = seats.rows.len() as f64;
    println!(
        "\ndenial axis — Δ in the rate this seat LOST to a rival's victory, on minus off (negative \
         is denial). Base rates over {} seats: {}",
        seats.rows.len(),
        lanes
            .iter()
            .map(|(lane, count)| format!("{lane} {:.1}%", 100.0 * *count as f64 / n))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    let outcomes: Vec<Vec<f64>> = lanes
        .iter()
        .map(|(lane, _)| {
            seats
                .rows
                .iter()
                .map(|row| f64::from(u8::from(lost_to(row, lane))))
                .collect()
        })
        .collect();
    let mut table: Vec<(String, Vec<(f64, f64)>)> = Vec::new();
    for (column, &index) in seats.columns.iter().enumerate() {
        let cells: Vec<(f64, f64)> = outcomes
            .iter()
            .map(|outcome| {
                let (delta, se) = seats.contrast(column, outcome);
                (100.0 * delta, 100.0 * se)
            })
            .collect();
        table.push((header.genes[index].clone(), cells));
    }
    // Most denial first, by the leading lane's z. A gene that reduces the
    // biggest killer is the one the reader is looking for.
    table.sort_by(|a, b| {
        let z = |cells: &Vec<(f64, f64)>| {
            let (delta, se) = cells[0];
            if se > 0.0 && se.is_finite() {
                delta / se
            } else {
                0.0
            }
        };
        z(&a.1)
            .partial_cmp(&z(&b.1))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    if let Some(top) = top {
        table.truncate(top.max(1));
    }
    let header_cells: Vec<String> = lanes
        .iter()
        .map(|(lane, _)| format!("{:>18}", format!("lost to {lane}")))
        .collect();
    println!("{:<30}{}", "gene", header_cells.join(""));
    for (tag, cells) in &table {
        let rendered: Vec<String> = cells
            .iter()
            .map(|&(delta, se)| {
                if se.is_finite() && se > 0.0 {
                    format!("{:>18}", format!("{delta:+.2} (z {:+.2})", delta / se))
                } else {
                    format!("{:>18}", format!("{delta:+.2} (z   —)"))
                }
            })
            .collect();
        println!("{tag:<30}{}", rendered.join(""));
    }
    println!(
        "⚠ These are the SAME seats as the win table, read on a different outcome. A gene may deny \
         a lane and still lose the game, and a gene that denies nothing here has no headroom on \
         this board whatever its win column says."
    );
}

/// Whether this seat lost the game to a rival's victory of one kind. The
/// denial axis: a denial gene's job is not to win more games, it is to stop
/// somebody else winning one, and those are different numbers whenever the
/// board has more than two empires on it.
fn lost_to(row: &Row, kind: &str) -> bool {
    !row.win && row.victory == kind
}

/// One line naming the board a batch was played on: fieldless, or the lanes
/// its pinned pursuers were racing.
///
/// Printed beside the build line, because "which genes" and "which board" are
/// the two things a column cannot be read without, and this project has
/// already published a column whose board was typed by hand and differed from
/// the intended one in four axes
/// (`docs/eval/2026-08-23-the-congress-purchase-verdict-and-a-name-for-the-contested-b.md`).
fn field_line(header: &Header) -> String {
    if header.contested_field.is_empty() {
        return format!(
            "field: none — the standard fieldless screen{}",
            if header.native_competitions {
                " · ⚠ native competitions ON (a probe: this is not the standard screen)"
            } else {
                ""
            }
        );
    }
    format!(
        "field: ⭐ CONTESTED — {} pinned to pursue, {} drawn seats measured · native competitions {}",
        header
            .contested_field
            .split(',')
            .collect::<Vec<_>>()
            .join(" + "),
        header
            .players
            .saturating_sub(header.contested_field.split(',').filter(|lane| !lane.is_empty()).count()),
        if header.native_competitions {
            "on"
        } else {
            "⚠ OFF — the diplomatic lane has no recurring route to 20 points"
        }
    ) + &if header.contested_field_genes.is_empty() {
        " · pursuers play the deployment genome alone".to_string()
    } else {
        format!(
            " · pursuers also play {}",
            header.contested_field_genes.replace(',', ", ")
        )
    }
}

/// One line saying what the rotating victory mask did, or that there was none.
fn mask_line(header: &Header) -> String {
    if header.victory_mask.is_empty() {
        return "victory mask: none — every game plays the batch-level lanes".to_string();
    }
    format!(
        "victory mask: ⭐ {} — each game closes {} of the five real conditions from its seed, score \
         always on; {} masks pre-registered ({}) · still the standard shape: every lane is live across \
         the batch",
        header.victory_mask,
        header
            .victory_mask
            .strip_prefix("rotate:")
            .unwrap_or("?"),
        header.victory_mask_games.len(),
        header
            .victory_mask_games
            .iter()
            .map(|(mask, games)| format!("{mask}×{games}"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// ⭐ WHICH LANE A LANE GENE IS FOR, so the mask can read it with that lane
/// open against closed. The registry's own words (`src/ai/advanced/genes.rs`):
/// `competition-victory-points` prices the points a scored competition pays,
/// the culture spending pass and the space race are their lanes by name, and
/// `holy-lane-parity` is the religion race. The remaining `lane-*` genes —
/// great people, the policy deck, the commit — substitute WHICHEVER lane the
/// seat is racing at one decider, so they belong to no single lane and are read
/// on all five.
const LANE_GENE_LANES: [(&str, &str); 4] = [
    ("competition-victory-points", "diplomatic"),
    ("lane-culture-spending", "culture"),
    ("lane-space-race", "science"),
    ("holy-lane-parity", "religious"),
];

/// The lanes a gene is read on by the mask split: one for a gene of one lane,
/// all five for a `lane-*` gene of no single lane, none for every other gene.
/// A version (`lane-space-race-2`) is read as its base.
fn lane_gene_lanes(tag: &str) -> Option<Vec<&'static str>> {
    let base = match tag.rsplit_once('-') {
        Some((base, version)) if version.parse::<usize>().is_ok() => base,
        _ => tag,
    };
    if let Some((_, lane)) = LANE_GENE_LANES.iter().find(|(gene, _)| *gene == base) {
        return Some(vec![lane]);
    }
    if base.starts_with("lane-") {
        return Some(MASKABLE_LANES.to_vec());
    }
    None
}

/// One lane gene's win Δ with one lane open against closed.
struct MaskSplit {
    tag: String,
    lane: &'static str,
    /// (Δ, standard error, seats) with the lane open.
    open: (f64, f64, usize),
    /// The same with the lane closed.
    closed: (f64, f64, usize),
}

impl MaskSplit {
    /// Open minus closed, with the error of a difference of two independent
    /// estimates — the two subsets share no game.
    fn difference(&self) -> (f64, f64) {
        let se = (self.open.1 * self.open.1 + self.closed.1 * self.closed.1).sqrt();
        (self.open.0 - self.closed.0, se)
    }
}

/// ⭐ THE VICTORY MASK READ BACK: how many games each mask took, how often
/// each lane was open, and every lane gene with its lane open against closed.
struct MaskReport {
    mask: String,
    games: usize,
    by_mask: BTreeMap<String, usize>,
    /// Per lane: games with it open, games with it closed.
    lanes: Vec<(&'static str, usize, usize)>,
    splits: Vec<MaskSplit>,
}

impl MaskReport {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "mask": self.mask,
            "games": self.games,
            "by_mask": self.by_mask,
            "lanes": self.lanes.iter().map(|(lane, open, closed)| {
                serde_json::json!({"lane": lane, "open_games": open, "closed_games": closed})
            }).collect::<Vec<_>>(),
            "lane_genes": self.splits.iter().map(|split| {
                let (delta, se) = split.difference();
                serde_json::json!({
                    "tag": split.tag,
                    "lane": split.lane,
                    "open_seats": split.open.2,
                    "open_win_delta_pp": 100.0 * split.open.0,
                    "open_win_se_pp": 100.0 * split.open.1,
                    "closed_seats": split.closed.2,
                    "closed_win_delta_pp": 100.0 * split.closed.0,
                    "closed_win_se_pp": 100.0 * split.closed.1,
                    "open_minus_closed_pp": 100.0 * delta,
                    "open_minus_closed_se_pp": 100.0 * se,
                })
            }).collect::<Vec<_>>(),
        })
    }
}

/// A gene's win Δ on the seats `keep` selects, with its clustered error and
/// the seat count. `None` when the gene is not screened in this header.
fn subset_win_contrast(
    header: &Header,
    rows: &[Row],
    gene: usize,
    keep: impl Fn(&Row) -> bool,
) -> Option<(f64, f64, usize)> {
    let subset: Vec<Row> = rows.iter().filter(|row| keep(row)).cloned().collect();
    let seats = Seats::of(header, &subset);
    let column = seats.columns.iter().position(|&index| index == gene)?;
    let n = seats.rows.len();
    if n == 0 {
        return Some((0.0, f64::INFINITY, 0));
    }
    let (delta, se) = seats.contrast(column, &seats.wins);
    Some((delta, se, n))
}

/// The report, or `None` for a batch no mask touched (no header mask and no
/// row with a closed lane).
fn mask_report(header: &Header, rows: &[Row]) -> Option<MaskReport> {
    let played: Vec<&Row> = rows.iter().filter(|row| row.kind == "game").collect();
    if header.victory_mask.is_empty() && played.iter().all(|row| row.victories_off.is_empty()) {
        return None;
    }
    let mut first_of_game: BTreeMap<GameKey, &Row> = BTreeMap::new();
    for row in &played {
        first_of_game.entry(row.game_key()).or_insert(row);
    }
    let mut by_mask: BTreeMap<String, usize> = BTreeMap::new();
    for row in first_of_game.values() {
        *by_mask.entry(mask_key(&row.victories_off)).or_default() += 1;
    }
    let lanes = MASKABLE_LANES
        .iter()
        .map(|&lane| {
            let closed = first_of_game
                .values()
                .filter(|row| row.victories_off.iter().any(|off| off == lane))
                .count();
            (lane, first_of_game.len() - closed, closed)
        })
        .collect();
    let mut splits = Vec::new();
    for (index, tag) in header.genes.iter().enumerate() {
        if !header.screened.iter().any(|screened| screened == tag) {
            continue;
        }
        let Some(gene_lanes) = lane_gene_lanes(tag) else {
            continue;
        };
        for lane in gene_lanes {
            let open = subset_win_contrast(header, rows, index, |row| {
                !row.victories_off.iter().any(|off| off == lane)
            });
            let closed = subset_win_contrast(header, rows, index, |row| {
                row.victories_off.iter().any(|off| off == lane)
            });
            if let (Some(open), Some(closed)) = (open, closed) {
                splits.push(MaskSplit {
                    tag: tag.clone(),
                    lane,
                    open,
                    closed,
                });
            }
        }
    }
    Some(MaskReport {
        mask: header.victory_mask.clone(),
        games: first_of_game.len(),
        by_mask,
        lanes,
        splits,
    })
}

/// The "Victory masks" section of `--analyze`: mask counts, lane open/closed
/// counts, and every lane gene read with its own lane open against closed.
fn print_victory_masks(header: &Header, rows: &[Row]) {
    let Some(report) = mask_report(header, rows) else {
        return;
    };
    println!(
        "\nVictory masks · {} · {} games",
        if report.mask.is_empty() {
            "(rows carry closed lanes, header names no mask)".to_string()
        } else {
            report.mask.clone()
        },
        report.games
    );
    println!(
        "  masks: {}",
        report
            .by_mask
            .iter()
            .map(|(mask, games)| format!("{mask}×{games}"))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    println!(
        "  lanes: {}",
        report
            .lanes
            .iter()
            .map(|(lane, open, closed)| format!("{lane} open {open} / closed {closed}"))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    if report.splits.is_empty() {
        println!("  no lane gene screened, so no open/closed split");
        return;
    }
    println!(
        "  {:<28} {:<11} {:>22} {:>22} {:>16}",
        "lane gene", "lane", "lane OPEN winΔ (seats)", "lane CLOSED winΔ (seats)", "open−closed"
    );
    let cell = |(delta, se, seats): (f64, f64, usize)| {
        if se.is_finite() {
            format!("{:+.1}±{:.1}pp ({seats})", 100.0 * delta, 100.0 * se)
        } else {
            format!("{:+.1}pp ({seats})", 100.0 * delta)
        }
    };
    for split in &report.splits {
        let (delta, se) = split.difference();
        println!(
            "  {:<28} {:<11} {:>22} {:>22} {:>16}",
            split.tag,
            split.lane,
            cell(split.open),
            cell(split.closed),
            if se.is_finite() {
                format!("{:+.1}±{:.1}pp", 100.0 * delta, 100.0 * se)
            } else {
                format!("{:+.1}pp", 100.0 * delta)
            }
        );
    }
    println!(
        "  a lane gene pays through its lane: a Δ that is larger with the lane open than closed is \
         the lane paying, one that is the same either way is the gene paying through score share or \
         not at all. Errors are clustered by game; the two subsets share no game."
    );
}

/// One line saying what rung the majors played, or how they rotated.
fn difficulty_line(header: &Header) -> String {
    if !header.difficulty_rotate.is_empty() {
        return format!(
            "difficulty: ⭐ majors rotate {} per game from the seed ({}) · barbarians at their own rung ({})",
            header.difficulty_rotate,
            header
                .difficulty_games
                .iter()
                .map(|(rung, games)| format!("{rung}×{games}"))
                .collect::<Vec<_>>()
                .join(" "),
            civvis::game::default_barbarian_difficulty()
        );
    }
    format!(
        "difficulty: majors at {} in every game · barbarians at their own rung ({})",
        if header.difficulty.is_empty() {
            format!(
                "{} (the default, unrecorded)",
                civvis::game::default_difficulty()
            )
        } else {
            header.difficulty.clone()
        },
        civvis::game::default_barbarian_difficulty()
    )
}

/// The rung a row's game played, with the pre-2026-08-25 default filled in.
fn row_rung(row: &Row) -> String {
    if row.difficulty.is_empty() {
        civvis::game::default_difficulty()
    } else {
        row.difficulty.clone()
    }
}

/// The ruleset's rungs in ladder order, Settler first and Deity last.
fn difficulty_ladder(rules: &civvis::rules::Rules) -> Vec<&str> {
    let mut names: Vec<&str> = rules.difficulties.keys().map(|k| k.as_str()).collect();
    names.sort_by_key(|name| rules.difficulties[*name].order);
    names
}

/// A gene's (win Δ, standard error, seats) on each subset of a batch, in the
/// report's subset order — one cell per rung, per rival kind, and so on.
type SubsetCells = Vec<(f64, f64, usize)>;

/// ⭐ THE RUNG ROTATION READ BACK: games per rung, and the top genes by |win Δ|
/// read on every rung separately.
struct RungReport {
    rotation: String,
    /// Rung, games — in ladder order.
    games: Vec<(String, usize)>,
    /// Per gene: tag, whole-batch win Δ, and (Δ, se, seats) per rung in
    /// `games` order.
    genes: Vec<(String, f64, SubsetCells)>,
}

impl RungReport {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "rotation": self.rotation,
            "games": self.games.iter().map(|(rung, games)| {
                serde_json::json!({"rung": rung, "games": games})
            }).collect::<Vec<_>>(),
            "top_genes": self.genes.iter().map(|(tag, delta, per_rung)| {
                serde_json::json!({
                    "tag": tag,
                    "win_delta_pp": 100.0 * delta,
                    "by_rung": self.games.iter().zip(per_rung).map(|((rung, _), (d, se, n))| {
                        serde_json::json!({
                            "rung": rung,
                            "seats": n,
                            "win_delta_pp": 100.0 * d,
                            "win_se_pp": 100.0 * se,
                        })
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        })
    }
}

/// How many genes the rung table shows: the top by |win Δ| over the batch.
const RUNG_TABLE_GENES: usize = 10;

/// The report, or `None` when every game played one rung and no rotation was
/// declared.
fn rung_report(header: &Header, rows: &[Row]) -> Option<RungReport> {
    let played: Vec<&Row> = rows.iter().filter(|row| row.kind == "game").collect();
    let mut first_of_game: BTreeMap<GameKey, &Row> = BTreeMap::new();
    for row in &played {
        first_of_game.entry(row.game_key()).or_insert(row);
    }
    let mut by_rung: BTreeMap<String, usize> = BTreeMap::new();
    for row in first_of_game.values() {
        *by_rung.entry(row_rung(row)).or_default() += 1;
    }
    if header.difficulty_rotate.is_empty() && by_rung.len() < 2 {
        return None;
    }
    // Ladder order, not alphabetical: settler first, deity last.
    let rules = civvis::rules::Rules::embedded();
    let ladder = difficulty_ladder(&rules);
    let mut games: Vec<(String, usize)> = ladder
        .iter()
        .filter_map(|rung| by_rung.get(*rung).map(|&n| ((*rung).to_string(), n)))
        .collect();
    for (rung, n) in &by_rung {
        if !ladder.contains(&rung.as_str()) {
            games.push((rung.clone(), *n));
        }
    }
    let estimates = estimate(header, rows);
    let mut top: Vec<&GeneEstimate> = estimates.genes.iter().collect();
    top.sort_by(|a, b| {
        b.win_delta
            .abs()
            .partial_cmp(&a.win_delta.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let genes = top
        .iter()
        .take(RUNG_TABLE_GENES)
        .filter_map(|estimate| {
            let index = header.genes.iter().position(|tag| *tag == estimate.tag)?;
            let per_rung = games
                .iter()
                .map(|(rung, _)| {
                    subset_win_contrast(header, rows, index, |row| row_rung(row) == *rung)
                        .unwrap_or((0.0, f64::INFINITY, 0))
                })
                .collect();
            Some((estimate.tag.clone(), estimate.win_delta, per_rung))
        })
        .collect();
    Some(RungReport {
        rotation: header.difficulty_rotate.clone(),
        games,
        genes,
    })
}

/// The "Difficulty rungs" section of `--analyze`: games per rung and the top
/// genes by |win Δ| read on every rung.
fn print_difficulty_rungs(header: &Header, rows: &[Row]) {
    let Some(report) = rung_report(header, rows) else {
        return;
    };
    println!(
        "\nDifficulty rungs · {} · {}",
        if report.rotation.is_empty() {
            "(rows carry more than one rung, header names no rotation)".to_string()
        } else {
            report.rotation.clone()
        },
        report
            .games
            .iter()
            .map(|(rung, games)| format!("{rung}×{games} games"))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    if report.genes.is_empty() {
        return;
    }
    let cell = |(delta, se, seats): (f64, f64, usize)| {
        if se.is_finite() {
            format!("{:+.1}±{:.1} ({seats})", 100.0 * delta, 100.0 * se)
        } else {
            format!("{:+.1} ({seats})", 100.0 * delta)
        }
    };
    let head: Vec<String> = report
        .games
        .iter()
        .map(|(rung, _)| format!("{rung:>20}"))
        .collect();
    println!(
        "  {:<28} {:>9} {}",
        format!("top {} by |winΔ|", report.genes.len()),
        "all pp",
        head.join(" ")
    );
    for (tag, delta, per_rung) in &report.genes {
        println!(
            "  {:<28} {:>+9.1} {}",
            tag,
            100.0 * delta,
            per_rung
                .iter()
                .map(|&cell_value| format!("{:>20}", cell(cell_value)))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    println!(
        "  win Δpp ± one clustered standard error (seats) per rung; a gene whose sign holds on every \
         rung pays at every handicap, one that flips is a gene for one rung. The barbarian seat \
         plays its own rung throughout."
    );
}

/// One line saying whether a fixed rival sat in every game.
fn rivals_line(header: &Header) -> String {
    if header.rivals.is_empty() {
        return "rivals: none — every major draws a genome and is measured".to_string();
    }
    format!(
        "rivals: ⭐ {} — one chair per game plays a fixed opponent, rotating {} from the seed ({}); \
         that seat is not measured",
        header.rivals,
        RIVAL_KINDS.join(" / "),
        header
            .rival_games
            .iter()
            .map(|(kind, games)| format!("{kind}×{games}"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

/// ⭐ THE RIVAL MIX READ BACK: games and rival wins per kind, and every gene
/// past the family-wise bar read against each kind of rival apart.
struct RivalReport {
    rivals: String,
    /// Kind, games, games the rival itself won — in [`RIVAL_KINDS`] order.
    kinds: Vec<(String, usize, usize)>,
    /// Per gene past the bar: tag, whole-batch win Δ, (Δ, se, seats) per
    /// kind in `kinds` order, and whether every kind's sign agrees with the
    /// whole batch's.
    genes: Vec<(String, f64, SubsetCells, bool)>,
    family_z: f64,
}

impl RivalReport {
    fn json(&self) -> serde_json::Value {
        serde_json::json!({
            "rivals": self.rivals,
            "family_wise_z": self.family_z,
            "kinds": self.kinds.iter().map(|(kind, games, won)| {
                serde_json::json!({"kind": kind, "games": games, "rival_won": won})
            }).collect::<Vec<_>>(),
            "genes_past_the_bar": self.genes.iter().map(|(tag, delta, per_kind, agree)| {
                serde_json::json!({
                    "tag": tag,
                    "win_delta_pp": 100.0 * delta,
                    "signs_agree": agree,
                    "by_kind": self.kinds.iter().zip(per_kind).map(|((kind, _, _), (d, se, n))| {
                        serde_json::json!({
                            "kind": kind,
                            "seats": n,
                            "win_delta_pp": 100.0 * d,
                            "win_se_pp": 100.0 * se,
                        })
                    }).collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        })
    }
}

/// The report, or `None` for a batch with no rival rows and no mix declared.
fn rival_report(header: &Header, rows: &[Row]) -> Option<RivalReport> {
    let mut kind_of_game: BTreeMap<GameKey, (&str, bool)> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.kind == "rival") {
        kind_of_game.insert(row.game_key(), (row.rival_mix.as_str(), row.win));
    }
    if header.rivals.is_empty() && kind_of_game.is_empty() {
        return None;
    }
    let kinds: Vec<(String, usize, usize)> = RIVAL_KINDS
        .iter()
        .map(|&kind| {
            let games = kind_of_game.values().filter(|(k, _)| *k == kind).count();
            let won = kind_of_game
                .values()
                .filter(|(k, w)| *k == kind && *w)
                .count();
            (kind.to_string(), games, won)
        })
        .collect();
    let estimates = estimate(header, rows);
    let family_z = family_wise_z(estimates.genes.len());
    let genes = estimates
        .genes
        .iter()
        .filter(|estimate| estimate.win_z().abs() >= family_z)
        .filter_map(|estimate| {
            let index = header.genes.iter().position(|tag| *tag == estimate.tag)?;
            let per_kind: SubsetCells = kinds
                .iter()
                .map(|(kind, _, _)| {
                    subset_win_contrast(header, rows, index, |row| {
                        kind_of_game
                            .get(&row.game_key())
                            .is_some_and(|(k, _)| *k == kind.as_str())
                    })
                    .unwrap_or((0.0, f64::INFINITY, 0))
                })
                .collect();
            let agree = per_kind
                .iter()
                .filter(|(_, _, n)| *n > 0)
                .all(|(delta, _, _)| delta.signum() == estimate.win_delta.signum());
            Some((estimate.tag.clone(), estimate.win_delta, per_kind, agree))
        })
        .collect();
    Some(RivalReport {
        rivals: header.rivals.clone(),
        kinds,
        genes,
        family_z,
    })
}

/// The "Rival mix" section of `--analyze`: games and rival wins per kind, and
/// every gene past the family-wise bar read per kind with its sign agreement.
fn print_rival_mix(header: &Header, rows: &[Row]) {
    let Some(report) = rival_report(header, rows) else {
        return;
    };
    println!(
        "\nRival mix · {} · {}",
        if report.rivals.is_empty() {
            "(rows carry rival seats, header names no mix)".to_string()
        } else {
            report.rivals.clone()
        },
        report
            .kinds
            .iter()
            .map(|(kind, games, won)| format!("{kind}×{games} games (rival won {won})"))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    if report.genes.is_empty() {
        println!(
            "  no gene past the family-wise bar |z|≥{:.2}, so no sign agreement to read",
            report.family_z
        );
        return;
    }
    let cell = |(delta, se, seats): (f64, f64, usize)| {
        if se.is_finite() {
            format!("{:+.1}±{:.1} ({seats})", 100.0 * delta, 100.0 * se)
        } else {
            format!("{:+.1} ({seats})", 100.0 * delta)
        }
    };
    println!(
        "  {:<28} {:>9} {} {:>8}",
        format!("past |z|≥{:.2}", report.family_z),
        "all pp",
        report
            .kinds
            .iter()
            .map(|(kind, _, _)| format!("{kind:>20}"))
            .collect::<Vec<_>>()
            .join(" "),
        "signs"
    );
    for (tag, delta, per_kind, agree) in &report.genes {
        println!(
            "  {:<28} {:>+9.1} {} {:>8}",
            tag,
            100.0 * delta,
            per_kind
                .iter()
                .map(|&value| format!("{:>20}", cell(value)))
                .collect::<Vec<_>>()
                .join(" "),
            if *agree { "agree" } else { "SPLIT" }
        );
    }
    println!(
        "  win Δpp ± one clustered standard error (measured seats) in the games whose fixed rival was \
         of each kind; `agree` = every kind's sign is the whole batch's, `SPLIT` = a gene that pays \
         against one rival and not another. The rival seat itself prices no gene."
    );
}

/// ⭐ THE LIVE LOSS CENSUS the screen is read against: how the live
/// Civilization VI verification seat's games ended when a rival won, from
/// the Hall of Fame census in
/// `docs/eval/2026-08-18-we-screen-against-a-religion-game-and-lose-a-diplomacy-game.md`
/// — diplomatic 32 : culture 27 : religious 8 : science 4 : domination 1.
/// The board the screen plays is not that board (science and diplomatic land
/// past its clock; conversion decides most of its early endings), and the
/// drift meter says by how much.
const LIVE_LOSS_CENSUS: [(&str, usize); 5] = [
    ("diplomatic", 32),
    ("culture", 27),
    ("religious", 8),
    ("science", 4),
    ("domination", 1),
];
const LIVE_LOSS_CENSUS_SOURCE: &str =
    "docs/eval/2026-08-18-we-screen-against-a-religion-game-and-lose-a-diplomacy-game.md";

/// Where the live ladder's own record is read from when `--analyze` runs in
/// a checkout: its `attempts[].victory_type` are the endings of every
/// finished live game, ours or a rival's.
const LIVE_LADDER_JSON: &str = "docs/civ6_ladder.json";

/// A live `victory_type` as the screen names the ending, or `None` for a
/// value that is not an ending (`VICTORY_DEFAULT`, an unfinished game).
fn live_ending(victory_type: &str) -> Option<&'static str> {
    match victory_type {
        "VICTORY_SCORE" => Some("score"),
        "VICTORY_DIPLOMATIC" => Some("diplomatic"),
        "VICTORY_CULTURE" => Some("culture"),
        "VICTORY_RELIGIOUS" => Some("religious"),
        "VICTORY_TECHNOLOGY" => Some("science"),
        "VICTORY_CONQUEST" => Some("domination"),
        _ => None,
    }
}

/// The live ladder's endings from `docs/civ6_ladder.json`, if the file is
/// readable here and carries any: ending → games.
fn live_ladder_endings(path: &str) -> Option<BTreeMap<&'static str, usize>> {
    let text = std::fs::read_to_string(path).ok()?;
    let ladder: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut endings: BTreeMap<&'static str, usize> = BTreeMap::new();
    for attempt in ladder.get("attempts")?.as_array()? {
        if let Some(ending) = attempt
            .get("victory_type")
            .and_then(serde_json::Value::as_str)
            .and_then(live_ending)
        {
            *endings.entry(ending).or_default() += 1;
        }
    }
    (!endings.is_empty()).then_some(endings)
}

/// ⭐ THE DRIFT METER: the batch's share of games ended by each condition
/// beside the live seat's, so a reader sees which lanes the screen is not
/// exercising before reading a lane gene's column.
struct DriftMeter {
    /// Games in the batch, and per ending its games.
    games: usize,
    batch: BTreeMap<&'static str, usize>,
    /// The live ladder's own endings, when `LIVE_LADDER_JSON` was readable.
    ladder: Option<BTreeMap<&'static str, usize>>,
}

/// The six endings in the lobby's order, then `unfinished`.
const DRIFT_ENDINGS: [&str; 7] = [
    "science",
    "culture",
    "religious",
    "diplomatic",
    "domination",
    "score",
    "unfinished",
];

impl DriftMeter {
    fn share(count: usize, total: usize) -> f64 {
        if total == 0 {
            0.0
        } else {
            count as f64 / total as f64
        }
    }

    fn json(&self) -> serde_json::Value {
        let ladder_total: usize = self.ladder.iter().flat_map(|l| l.values()).sum();
        let census_total: usize = LIVE_LOSS_CENSUS.iter().map(|(_, n)| n).sum();
        serde_json::json!({
            "unit": "share of games ended by each condition",
            "games": self.games,
            "batch": DRIFT_ENDINGS.iter().map(|ending| {
                let games = self.batch.get(ending).copied().unwrap_or(0);
                (ending.to_string(), serde_json::json!(Self::share(games, self.games)))
            }).collect::<serde_json::Map<_, _>>(),
            "live_ladder": self.ladder.as_ref().map(|ladder| serde_json::json!({
                "source": LIVE_LADDER_JSON,
                "games": ladder_total,
                "shares": ladder.iter().map(|(ending, n)| {
                    (ending.to_string(), serde_json::json!(Self::share(*n, ladder_total)))
                }).collect::<serde_json::Map<_, _>>(),
            })),
            "live_loss_census": {
                "source": LIVE_LOSS_CENSUS_SOURCE,
                "games": census_total,
                "shares": LIVE_LOSS_CENSUS.iter().map(|(ending, n)| {
                    (ending.to_string(), serde_json::json!(Self::share(*n, census_total)))
                }).collect::<serde_json::Map<_, _>>(),
            },
        })
    }
}

/// The meter over the batch's games, or `None` for a batch with no games.
fn drift_meter(rows: &[Row]) -> Option<DriftMeter> {
    drift_meter_with(rows, LIVE_LADDER_JSON)
}

fn drift_meter_with(rows: &[Row], ladder_path: &str) -> Option<DriftMeter> {
    let mut first_of_game: BTreeMap<GameKey, &Row> = BTreeMap::new();
    for row in rows.iter().filter(|row| row.kind == "game") {
        first_of_game.entry(row.game_key()).or_insert(row);
    }
    if first_of_game.is_empty() {
        return None;
    }
    let mut batch: BTreeMap<&'static str, usize> = BTreeMap::new();
    for row in first_of_game.values() {
        let ending = DRIFT_ENDINGS
            .iter()
            .copied()
            .find(|ending| *ending == row.victory)
            .unwrap_or("unfinished");
        *batch.entry(ending).or_default() += 1;
    }
    Some(DriftMeter {
        games: first_of_game.len(),
        batch,
        ladder: live_ladder_endings(ladder_path),
    })
}

/// The drift table: one row per ending, the batch's share beside the live
/// ladder's (when its JSON is readable here) and the live loss census.
fn print_drift_meter(rows: &[Row]) {
    let Some(meter) = drift_meter(rows) else {
        return;
    };
    let ladder_total: usize = meter.ladder.iter().flat_map(|l| l.values()).sum();
    let census_total: usize = LIVE_LOSS_CENSUS.iter().map(|(_, n)| n).sum();
    println!(
        "drift · share of games ended by each condition · {} batch games · live loss census {} rival \
         wins ({LIVE_LOSS_CENSUS_SOURCE}){}",
        meter.games,
        census_total,
        match &meter.ladder {
            Some(_) => format!(" · live ladder {ladder_total} finished games ({LIVE_LADDER_JSON})"),
            None => format!(" · {LIVE_LADDER_JSON} not readable here, ladder column omitted"),
        }
    );
    println!(
        "  {:<11} {:>9} {:>13}{}",
        "ending",
        "batch",
        "live losses",
        if meter.ladder.is_some() {
            format!(" {:>12}", "live ladder")
        } else {
            String::new()
        }
    );
    for ending in DRIFT_ENDINGS {
        let batch = DriftMeter::share(meter.batch.get(ending).copied().unwrap_or(0), meter.games);
        let census = LIVE_LOSS_CENSUS
            .iter()
            .find(|(name, _)| *name == ending)
            .map(|(_, n)| format!("{:>12.0}%", 100.0 * DriftMeter::share(*n, census_total)))
            .unwrap_or_else(|| format!("{:>13}", "-"));
        let ladder = meter
            .ladder
            .as_ref()
            .map(|ladder| {
                format!(
                    " {:>11.0}%",
                    100.0
                        * DriftMeter::share(ladder.get(ending).copied().unwrap_or(0), ladder_total)
                )
            })
            .unwrap_or_default();
        println!("  {ending:<11} {:>8.0}% {census}{ladder}", 100.0 * batch);
    }
    println!(
        "  the live loss census is how the live seat's games ended when a RIVAL won; the ladder column \
         is every finished live game, ours included. A lane the batch never ends on is a lane its \
         genes cannot pay through on the win axis — read their share column."
    );
}

/// One line naming the binary a batch was played by, printed before the first
/// game and again by `--analyze`.
///
/// An unstamped or dirty build is called out here rather than left for the
/// ledger to discover hours later: `tools/genes.py` refuses both, and
/// the cheapest moment to learn that is before the batch starts.
fn build_line(header: &Header) -> String {
    let build = &header.build;
    if build.genes_sha256.is_empty() {
        return "build: pre-fingerprint — this file predates the build stamp (2026-08-23)"
            .to_string();
    }
    let short = |sha: &str| sha.chars().take(12).collect::<String>();
    let revision = if build.commit.is_empty() {
        format!(
            "⚠ UNSTAMPED ({}) — set CIVVIS_COMMIT, or the ledger will refuse this batch",
            build.commit_source
        )
    } else {
        format!(
            "{} ({}{})",
            short(&build.commit),
            build.commit_source,
            if build.dirty {
                ", ⚠ DIRTY TREE — the ledger will refuse this batch"
            } else {
                ""
            }
        )
    };
    format!(
        "build: {revision} · {} genes sha {} · binary sha {}",
        header.genes.len(),
        short(&build.genes_sha256),
        if build.binary_sha256.is_empty() {
            "unknown".to_string()
        } else {
            short(&build.binary_sha256)
        }
    )
}

/// Actual against intended, in seats.
///
/// ⚠ THE ANALYSIS MUST NOT BE ABLE TO PRESENT A TRUNCATED RUN AS A COMPLETED
/// ONE. P10 stopped at 5,858 of a planned 10,000 games at the operator's
/// request — a legitimate decision that left an artefact which read like a
/// finished screen. Every printed table now says which it is, and a file
/// written before pre-registration says that instead of guessing.
fn completeness_line(header: &Header, seats: usize) -> String {
    let target = header.batch.intended_seats();
    if target == 0 {
        return "⚠ batch size was not pre-registered: this file predates pre-registration \
                (2026-08-23), so actual cannot be read against intended"
            .to_string();
    }
    let window = format!(
        "seeds {}..{} reserved",
        header.batch.seed_first, header.batch.seed_last
    );
    let percent = 100.0 * seats as f64 / target as f64;
    if seats < target {
        format!(
            "⚠⚠ PARTIAL SCREEN · {seats} of {target} intended seats \
             ({percent:.1}%) · {window} — a truncated run, not a completed one"
        )
    } else {
        format!("screen complete · {seats} of {target} intended seats ({percent:.1}%) · {window}")
    }
}

fn usage() -> ! {
    eprintln!(
        "the screen: gene_screen [--games N] [--start-seed N] [--jobs N] [--genes tag,tag,...] \
         [--target-games N] [--out PATH] [--append] [--quiet] [--p-on 0.25] [--p-default-on 0.75]\n       \
         (6 majors, 74x46 continents, 9 city-states, online/250, all six lanes, every seat its own \
         random genome, shuffled civs — the one shape the ledger accepts)\n       \
         probe only, NOT a ledger source: [--contested] [--contested-field lane,lane] \
         [--contested-field-genes lanes|tag,tag] [--native-competitions] [--no-native-competitions] \
         [--players N] [--turns N] [--width N] \
         [--height N] [--city-states N] [--speed ID] [--map ID] [--victories a,b,...] [--stock-civs]\n       \
         still the screen (all lanes live across the batch): [--victory-mask rotate:N] closes N of the \
         five real conditions per game from its seed, score always on, C(5,N) masks at equal shares\n       \
         the majors' rung: [--difficulty RUNG] (default {}) or [--difficulty-rotate king:1,emperor:2,immortal:1] \
         drawn per game from the seed in those shares; barbarians stay at {}\n       \
         the rival mix: [--rivals firaxis-mix] seats one fixed opponent per game — legacy anchor / \
         deployment genome retargeted at a live-census lane / random genome, in turn — that is not measured\n       \
         (--contested pins one rival seat per lane to actually pursue it — {} by default — and turns \
         on native scored competitions, the only recurring native route to the {DIPLOMATIC_VICTORY_POINTS} \
         Diplomatic Victory Points that lane needs)\n       \
         gene_screen --analyze PATH [PATH ...] [--json OUT] [--interactions] [--denial] [--top N] [--by-civ TAG]\n       \
         gene_screen --list",
        civvis::game::default_difficulty(),
        civvis::game::default_barbarian_difficulty(),
        CONTESTED_FIELD.join("+")
    );
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if present(&args, "--help") || present(&args, "-h") {
        usage();
    }
    let genes = gene_table();

    if present(&args, "--list") {
        println!(
            "{} genes (bit order) · default = the deployment genome (docs/gene_ledger.json) · \
             p = on-probability in the screen ({P_ON} / {P_DEFAULT_ON} for a default-on gene) · \
             HELD = out of the default screened set on cost, ask for it by name",
            genes.len()
        );
        let tags: Vec<String> = genes.iter().map(|gene| gene.tag.to_string()).collect();
        for family in families_of(&tags) {
            println!(
                "family {}: {} — a seat plays at most one version",
                tags[family[0]],
                family
                    .iter()
                    .map(|&i| tags[i].as_str())
                    .collect::<Vec<_>>()
                    .join(" · ")
            );
        }
        for (i, gene) in genes.iter().enumerate() {
            let verdict = civvis::ai::ledger_verdict(gene.tag)
                .map(|row| row.verdict.as_str())
                .unwrap_or("unmeasured");
            println!(
                "{i:>3}  {:<28} {:<32} universe:{} stock:{} default:{} ledger:{:<10} p:{:.2}{}",
                gene.tag,
                gene.field,
                if gene.after_setup_on { "on " } else { "off" },
                if gene.stock_on { "on " } else { "off" },
                if gene.default_on { "on " } else { "off" },
                verdict,
                if gene.default_on { P_DEFAULT_ON } else { P_ON },
                if HELD_UNLESS_ASKED.contains(&gene.tag) {
                    "  HELD"
                } else {
                    ""
                }
            );
        }
        return;
    }

    if let Some(index) = args.iter().position(|arg| arg == "--analyze") {
        let paths: Vec<String> = args[index + 1..]
            .iter()
            .take_while(|arg| !arg.starts_with("--"))
            .cloned()
            .collect();
        if paths.is_empty() {
            eprintln!("--analyze needs at least one JSONL path");
            usage();
        }
        let (header, rows) = read_rows(&paths);
        print_table(&header, &rows);
        if let Some(path) = text(&args, "--json") {
            write_json_summary(&path, &header, &rows);
        }
        if let Some(tag) = text(&args, "--by-civ") {
            print_by_civ(&header, &rows, &tag);
        }
        if present(&args, "--denial") {
            let top = args
                .iter()
                .any(|arg| arg == "--top")
                .then(|| number(&args, "--top", 20).max(1) as usize);
            print_denial(&header, &rows, top);
        }
        if present(&args, "--interactions") {
            let top = number(&args, "--top", 20).max(1) as usize;
            print_interactions(
                &header,
                &rows,
                "win rate",
                100.0,
                "pp",
                |row| f64::from(u8::from(row.win)),
                top,
            );
            print_interactions(
                &header,
                &rows,
                "score share",
                100.0,
                "pp",
                |row| row.score_share,
                top,
            );
        }
        return;
    }

    for retired in [
        "--pairs",
        "--target-pairs",
        "--anchor-pairs",
        "--design",
        "--single-seat",
        "--field",
        "--baseline",
        "--p-helps",
        "--p-hurts",
        "--p-unresolved",
    ] {
        if present(&args, retired) {
            eprintln!(
                "{retired} belonged to the paired designs, which are gone: every seat now draws its own \
                 genome independently. Size the batch with --games N (one game = {SCREEN_PLAYERS} seats at \
                 the screen) and tune the draw with --p-on / --p-default-on."
            );
            std::process::exit(2);
        }
    }

    let games_to_play = number(&args, "--games", 200).max(1) as usize;
    let start_seed = number(&args, "--start-seed", 26_081_900) as u64;
    // ⭐ PRE-REGISTER THE SIZE. A single-session batch declares it by playing
    // it, so `--games` is the default target and no operator has to remember a
    // second flag. `--target-games` is for the screen deliberately split over
    // `--append` sessions: each segment declares its share of the whole on its
    // own disjoint start seed, and `--analyze` sums them. Either way the
    // header carries the intention before the first game finishes, so a run
    // that stops early is visibly partial instead of quietly complete.
    let target_games = number(&args, "--target-games", games_to_play as i64).max(1) as usize;
    // ⚠ THE SCREEN IS ONE SHAPE (operator, 2026-08-22). Every default below
    // is a leg of it, and a batch that changes one is a probe, not the screen:
    // `tools/genes.py` refuses a source that does not match. The shape is
    // Civilization VI's own six-player row (`CIV6_MAP_SIZES`, "small": 74x46,
    // nine city-states, three continents) so the games the ledger is read from
    // are the games the deployment shape plays, not a cheaper stand-in.
    let players = number(&args, "--players", SCREEN_PLAYERS as i64).max(2) as usize;
    let width = number(&args, "--width", SCREEN_WIDTH as i64) as i32;
    let height = number(&args, "--height", SCREEN_HEIGHT as i64) as i32;
    let city_states = number(&args, "--city-states", SCREEN_CITY_STATES as i64).max(0) as usize;
    let jobs = number(&args, "--jobs", civvis::parallel::default_jobs() as i64).max(1) as usize;
    let quiet = present(&args, "--quiet");
    // ⚠ Stock seating is a FIXED civ per seat (Rome, Egypt, Greece, China…),
    // and on the first 250-pair run seats 0 and 2 won twice as often as seat 3
    // whoever sat there. Shuffling per map is a leg of the screen;
    // `--stock-civs` is the probe escape.
    let randomize_civs = !present(&args, "--stock-civs");
    // ⚠ THE LANES DECIDE WHICH GENES CAN EVEN ACT, so the screen leaves all six
    // live and the ledger reads one world. Restricting them (`--victories
    // domination,score`) once gave the war and siege genes a game that did not
    // end by conversion at turn 149; that was a second regime, its columns were
    // never comparable with the six-lane ones, and it is now a probe rather
    // than a ledger source. Continents is what de-biases the religion lane
    // now: 48% of Pangaea endings were conversions against 28% here.
    let victories = match text(&args, "--victories") {
        None => civvis::game::VictoryConditions::default(),
        Some(list) => civvis::game::VictoryConditions::parse(&list).unwrap_or_else(|why| {
            eprintln!(
                "--victories: {why}; choose from {:?}",
                civvis::game::VictoryConditions::NAMES
            );
            std::process::exit(2);
        }),
    };
    // ⭐ THE ROTATING VICTORY MASK. Per game, from the seed, N of the five
    // real conditions are closed and score stays on; every lane is live
    // across the batch, so this is still the standard shape (`shape_of`).
    let victory_mask = text(&args, "--victory-mask").map(|spec| {
        let mask = VictoryMask::parse(&spec).unwrap_or_else(|why| {
            eprintln!("--victory-mask: {why}");
            std::process::exit(2);
        });
        let lanes = mask.lanes(victories);
        if lanes.len() <= mask.rotate {
            eprintln!(
                "--victory-mask {spec}: only {} real conditions are enabled ({}); a rotation must \
                 leave at least one open every game (a fixed restriction is --victories)",
                lanes.len(),
                lanes.join(",")
            );
            std::process::exit(2);
        }
        mask
    });
    // ⭐ THE MAJORS' RUNG. One rung for every game, or a weighted rotation
    // drawn per game from the seed. Provenance in the header, not a shape
    // leg; the barbarian seat keeps its own rung either way.
    let rules = civvis::rules::Rules::embedded();
    let known_rungs = difficulty_ladder(&rules);
    let difficulty = text(&args, "--difficulty").unwrap_or_else(civvis::game::default_difficulty);
    if !known_rungs.contains(&difficulty.as_str()) {
        eprintln!("unknown --difficulty {difficulty:?}; choose one of {known_rungs:?}");
        std::process::exit(2);
    }
    let difficulty_rotate = text(&args, "--difficulty-rotate").map(|spec| {
        DifficultyRotation::parse(&spec, &known_rungs).unwrap_or_else(|why| {
            eprintln!("--difficulty-rotate: {why}");
            std::process::exit(2);
        })
    });
    if difficulty_rotate.is_some() && present(&args, "--difficulty") {
        eprintln!("--difficulty and --difficulty-rotate name the majors' rung two ways; pass one");
        std::process::exit(2);
    }
    // ⭐ THE RIVAL MIX. One chair per game plays a fixed opponent that is
    // never measured; see `RIVAL_KINDS`.
    let rivals = match text(&args, "--rivals").as_deref() {
        None | Some("none") => false,
        Some("firaxis-mix") => true,
        Some(other) => {
            eprintln!("unknown --rivals {other:?}; the mix is firaxis-mix (or none)");
            std::process::exit(2);
        }
    };
    let speed = match text(&args, "--speed") {
        None => GameSpeed::Online,
        Some(id) => GameSpeed::from_id(&id).unwrap_or_else(|| {
            eprintln!("unknown --speed {id:?}; use online|quick|standard|epic|marathon");
            std::process::exit(2);
        }),
    };
    let turns = if present(&args, "--turns") {
        number(&args, "--turns", 250).max(1) as u32
    } else {
        speed.turn_limit()
    };
    let map = match text(&args, "--map") {
        None => SCREEN_MAP,
        Some(id) => MapScript::from_id(&id).unwrap_or_else(|| {
            eprintln!("unknown --map {id:?}");
            std::process::exit(2);
        }),
    };
    // ⭐ THE CONTESTED FIELD. `--contested` is the default field — one
    // diplomatic pursuer and one culture pursuer, the two lanes that take 83%
    // of the live seat's early losses. `--contested-field a,b` names it
    // exactly. Both make the batch a probe: it is measuring denial against a
    // board that threatens something, which is a different question from the
    // one the ledger's columns answer, and `shape_of` says so.
    let field: Vec<VictoryTarget> = match (
        present(&args, "--contested"),
        text(&args, "--contested-field"),
    ) {
        (false, None) => Vec::new(),
        (_, Some(list)) => list
            .split(',')
            .map(str::trim)
            .filter(|lane| !lane.is_empty())
            .map(|lane| {
                lane.parse::<VictoryTarget>().unwrap_or_else(|why| {
                    eprintln!(
                        "--contested-field: {why}; lanes are {}",
                        VictoryTarget::ALL
                            .iter()
                            .map(|target| target.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    std::process::exit(2);
                })
            })
            .collect(),
        (true, None) => CONTESTED_FIELD
            .iter()
            .map(|lane| lane.parse::<VictoryTarget>().expect("a known lane"))
            .collect(),
    };
    // ⚠ A field needs somebody to measure. Two, in fact: with one drawn seat
    // every gene's on-arm and off-arm would be different GAMES rather than
    // different seats of the same game, and the clustered error this screen is
    // built on would have nothing left to cluster.
    if field.len() + 2 > players {
        eprintln!(
            "--contested-field pins {} of {players} seats and the screen measures the rest;              leave at least two drawn seats (raise --players, or name fewer lanes)",
            field.len()
        );
        std::process::exit(2);
    }
    for lane in &field {
        if !victories.is_enabled(lane.as_str()) {
            eprintln!(
                "--contested-field {}: that lane is disabled by --victories, so the pinned seat                  would pursue a victory the game cannot award",
                lane.as_str()
            );
            std::process::exit(2);
        }
    }
    if victory_mask.is_some() && !field.is_empty() {
        eprintln!(
            "--victory-mask cannot be combined with a contested field: the mask would close the \
             pinned pursuer's lane in some games and it would chase a victory the game cannot award"
        );
        std::process::exit(2);
    }
    if rivals && !field.is_empty() {
        eprintln!("--rivals cannot be combined with a contested field: the field already pins rival seats");
        std::process::exit(2);
    }
    if rivals && players < 3 {
        eprintln!("--rivals seats one fixed opponent per game and measures the rest; needs --players 3 or more");
        std::process::exit(2);
    }
    // The pursuers' own genome over the deployment one. `none` reproduces the
    // weaker deployment-genome-only field the constant's doc measured.
    let field_genes: Vec<String> = if field.is_empty() {
        Vec::new()
    } else {
        match text(&args, "--contested-field-genes").as_deref() {
            // The default and `none` are the same thing: the deployment genome,
            // for the reason `CONTESTED_FIELD_GENES` records.
            None | Some("none") => Vec::new(),
            Some("lanes") => CONTESTED_FIELD_GENES
                .iter()
                .map(|tag| (*tag).to_string())
                .collect(),
            Some(list) => list
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(|tag| {
                    if !genes.iter().any(|gene| gene.tag == tag) {
                        eprintln!(
                            "--contested-field-genes: unknown gene {tag:?}; `gene_screen --list` names them"
                        );
                        std::process::exit(2);
                    }
                    tag.to_string()
                })
                .collect(),
        }
    };
    // ⚠ THE DIPLOMATIC LANE NEEDS A ROUTE TO 20 POINTS TO EXIST AT ALL.
    // `docs/FIDELITY.md` is blunt about it: the competition sources that pay
    // Diplomatic Victory Points through the whole second half of a real game
    // are the difference between the live board and this one, and
    // `Game::native_competitions` — which runs the two of them CIVVIS models —
    // ships OFF. A contested field turns it on, because pinning a seat to a
    // lane it cannot finish is the cosmetic version of this feature. It stays
    // off for the standard screen, where it would move every recorded column.
    let native_competitions = if present(&args, "--no-native-competitions") {
        false
    } else {
        present(&args, "--native-competitions") || !field.is_empty()
    };
    let drawn = players - field.len() - usize::from(rivals);
    let p_on = real(&args, "--p-on", P_ON);
    let p_default_on = real(&args, "--p-default-on", P_DEFAULT_ON);
    for (name, p) in [("--p-on", p_on), ("--p-default-on", p_default_on)] {
        if !(p > 0.0 && p < 1.0) {
            eprintln!("{name} must be strictly between 0 and 1 (both arms need seats), got {p}");
            std::process::exit(2);
        }
    }
    let screened: Vec<bool> = match text(&args, "--genes") {
        // Everything but the cost list, which `--genes` can still ask for.
        None => genes
            .iter()
            .map(|gene| !HELD_UNLESS_ASKED.contains(&gene.tag))
            .collect(),
        Some(list) => {
            let wanted: Vec<&str> = list
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            for name in &wanted {
                if !genes
                    .iter()
                    .any(|gene| gene.tag == *name || gene.field == *name)
                {
                    eprintln!("unknown gene {name:?}; `gene_screen --list` names them");
                    std::process::exit(2);
                }
            }
            genes
                .iter()
                .map(|gene| {
                    wanted
                        .iter()
                        .any(|name| gene.tag == *name || gene.field == *name)
                })
                .collect()
        }
    };
    let screened_count = screened.iter().filter(|&&s| s).count();
    if screened_count == 0 {
        eprintln!("nothing to screen");
        std::process::exit(2);
    }
    let tags: Vec<String> = genes.iter().map(|gene| gene.tag.to_string()).collect();
    let families = families_of(&tags);
    // ⚠ A version screened without its family measures nothing: a sibling
    // held ON at its default forces every screened version off (the family
    // is one level per seat), so the row comes back +0.0 and reads as inert.
    // Versions are priced in the standard batch beside everything else; a
    // probe that names one must name them all.
    for family in &families {
        let named: Vec<&str> = family
            .iter()
            .filter(|&&i| screened[i])
            .map(|&i| genes[i].tag)
            .collect();
        let held_on: Vec<&str> = family
            .iter()
            .filter(|&&i| !screened[i] && genes[i].default_on)
            .map(|&i| genes[i].tag)
            .collect();
        if !named.is_empty() && !held_on.is_empty() {
            let whole: Vec<&str> = family.iter().map(|&i| genes[i].tag).collect();
            eprintln!(
                "{} is a version of {}, and {} is held on at its default: a held-on version \
                 forces its siblings off, so screen the family together — --genes {}",
                named.join(","),
                genes[family[0]].tag,
                held_on.join(","),
                whole.join(",")
            );
            std::process::exit(2);
        }
    }
    let probabilities = on_probabilities(&genes, &screened, p_on, p_default_on, &families);

    let out_path =
        text(&args, "--out").unwrap_or_else(|| format!("gene_screen-{start_seed}.jsonl"));
    let append = present(&args, "--append");
    if !append
        && std::fs::metadata(&out_path)
            .map(|meta| meta.len() > 0)
            .unwrap_or(false)
    {
        eprintln!(
            "{out_path} already holds rows; pass --append to add to it (with a disjoint --start-seed) or --out for a new file"
        );
        std::process::exit(2);
    }
    let mut out = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&out_path)
        .unwrap_or_else(|error| {
            eprintln!("cannot open {out_path}: {error}");
            std::process::exit(2);
        });
    let header = Header {
        kind: "header".to_string(),
        genes: genes.iter().map(|gene| gene.tag.to_string()).collect(),
        screened: genes
            .iter()
            .zip(&screened)
            .filter(|(_, &s)| s)
            .map(|(gene, _)| gene.tag.to_string())
            .collect(),
        players,
        width,
        height,
        turns,
        city_states,
        speed: speed.id().to_string(),
        map: map.id().to_string(),
        baseline: "best".to_string(),
        start_seed,
        randomize_civs,
        victories: civvis::game::VictoryConditions::NAMES
            .iter()
            .filter(|name| victories.is_enabled(name))
            .copied()
            .collect::<Vec<_>>()
            .join(","),
        all_seats: true,
        contested_field: field
            .iter()
            .map(|target| target.as_str())
            .collect::<Vec<_>>()
            .join(","),
        native_competitions,
        contested_field_genes: field_genes.join(","),
        victory_mask: victory_mask.map(VictoryMask::id).unwrap_or_default(),
        victory_mask_games: victory_mask
            .map(|mask| mask.games_by_mask(start_seed, games_to_play, victories))
            .unwrap_or_default(),
        difficulty: if difficulty_rotate.is_some() {
            String::new()
        } else {
            difficulty.clone()
        },
        difficulty_rotate: difficulty_rotate
            .as_ref()
            .map(DifficultyRotation::id)
            .unwrap_or_default(),
        difficulty_games: difficulty_rotate
            .as_ref()
            .map(|rotation| rotation.games_by_rung(start_seed, games_to_play))
            .unwrap_or_default(),
        rivals: if rivals {
            "firaxis-mix".to_string()
        } else {
            String::new()
        },
        rival_games: if rivals {
            rival_games(start_seed, games_to_play)
        } else {
            BTreeMap::new()
        },
        design: "independent".to_string(),
        prior: probabilities.clone(),
        families: families
            .iter()
            .map(|family| family.iter().map(|&i| tags[i].clone()).collect())
            .collect(),
        p_on,
        p_default_on,
        build: stamp_build(&genes),
        batch: Batch {
            target_games,
            // ⚠ MEASURED seats, not chairs. A pinned pursuer plays the game and
            // is never observed, so counting it here would report a screen as
            // complete a third before it was.
            target_seats: target_games * drawn,
            target_pairs: 0,
            target_comparisons: 0,
            seed_first: start_seed,
            seed_last: start_seed + target_games as u64 - 1,
        },
    };
    // ⚠ PRINTED BEFORE THE BATCH, not after it. An eight-hour screen whose
    // build could not be identified is eight hours the ledger will refuse, and
    // the operator can see that in the first line instead of at the end.
    println!("{}", build_line(&header));
    println!("{}", field_line(&header));
    println!("{}", mask_line(&header));
    println!("{}", difficulty_line(&header));
    println!("{}", rivals_line(&header));
    writeln!(
        out,
        "{}",
        serde_json::to_string(&header).expect("header serializes")
    )
    .expect("write header");

    let profile = Profile {
        players,
        width,
        height,
        turns,
        city_states,
        speed,
        map,
        randomize_civs,
        victories,
        field: field.clone(),
        native_competitions,
        field_genes: field_genes.clone(),
        victory_mask,
        difficulty,
        difficulty_rotate,
        rivals,
    };
    println!(
        "gene screen: {games_to_play} games ({} measured seats of {} chairs, every drawn seat its own genome, on at p={p_on} / {p_default_on} default-on) · {} of {} genes screened · {players}p {width}x{height} {} · {} · {turns} turns · {city_states} city-states · {} civs · seeds {start_seed}..{} · {jobs} jobs · rows → {out_path}",
        games_to_play * drawn,
        games_to_play * players,
        screened_count,
        genes.len(),
        map.id(),
        speed.id(),
        if randomize_civs { "shuffled" } else { "stock-seated" },
        start_seed + games_to_play as u64 - 1
    );

    let started = Instant::now();
    let done = std::sync::atomic::AtomicUsize::new(0);
    let out = std::sync::Mutex::new(out);
    let uniform: Vec<f64> = probabilities
        .iter()
        .zip(&screened)
        .map(|(&p, &s)| if s { 0.5 } else { p })
        .collect();
    let played: Vec<Vec<Row>> = civvis::parallel::map_reporting(
        games_to_play,
        jobs,
        |game| {
            let seed = start_seed + game as u64;
            let genomes: Vec<Vec<bool>> = (0..players)
                .map(|seat| {
                    // ⭐ A `random` rival plays every screened gene at one
                    // half — the draw's own machinery, a flat prior.
                    if rivals && seat == rival_index(players, game) && rival_kind(seed) == "random"
                    {
                        draw_genome(start_seed, game, players, seat, &uniform, &families)
                    } else {
                        draw_genome(start_seed, game, players, seat, &probabilities, &families)
                    }
                })
                .collect();
            play_game(&profile, &genes, game, seed, &genomes)
        },
        |_game, game_rows| {
            let mut out = out.lock().expect("row writer");
            for row in game_rows {
                writeln!(
                    out,
                    "{}",
                    serde_json::to_string(row).expect("row serializes")
                )
                .expect("write row");
            }
            out.flush().expect("flush rows");
            let finished = done.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            if !quiet {
                let elapsed = started.elapsed().as_secs_f64();
                let row = &game_rows[0];
                println!(
                    "[{finished:>5}/{games_to_play}] game {} seed {} ({} seats) · {} · t{} · {} · {:.0}s ({:.2} games/s, ~{:.0}s left)",
                    row.game,
                    row.seed,
                    game_rows.len(),
                    if row.victory.is_empty() { "-" } else { &row.victory },
                    row.turn,
                    match game_rows.iter().find(|r| r.win) {
                        Some(winner) => format!("winner seat {} ({})", winner.seat, winner.civ),
                        None => "no winner".to_string(),
                    },
                    row.secs,
                    finished as f64 / elapsed.max(1e-9),
                    elapsed / finished as f64 * (games_to_play - finished) as f64
                );
            }
        },
    );
    let rows: Vec<Row> = played.into_iter().flatten().collect();
    println!(
        "\n{games_to_play} games ({} seats) in {:.0}s ({:.2} games/s)",
        rows.len(),
        started.elapsed().as_secs_f64(),
        games_to_play as f64 / started.elapsed().as_secs_f64().max(1e-9),
    );
    print_table(&header, &rows);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_header(genes: &[&str]) -> Header {
        Header {
            kind: "header".into(),
            genes: genes.iter().map(|gene| (*gene).to_string()).collect(),
            screened: genes.iter().map(|gene| (*gene).to_string()).collect(),
            players: 3,
            width: 1,
            height: 1,
            turns: 1,
            city_states: 0,
            speed: "online".into(),
            map: "pangaea".into(),
            baseline: "best".into(),
            start_seed: 1,
            randomize_civs: false,
            victories: String::new(),
            all_seats: true,
            contested_field: String::new(),
            native_competitions: false,
            contested_field_genes: String::new(),
            victory_mask: String::new(),
            victory_mask_games: BTreeMap::new(),
            difficulty: String::new(),
            difficulty_rotate: String::new(),
            difficulty_games: BTreeMap::new(),
            rivals: String::new(),
            rival_games: BTreeMap::new(),
            design: "independent".into(),
            prior: vec![0.5; genes.len()],
            p_on: 0.5,
            p_default_on: 0.75,
            families: Vec::new(),
            build: Build::default(),
            batch: Batch::default(),
        }
    }

    fn test_row(game: usize, seat: usize, genome: &str, win: bool) -> Row {
        Row {
            kind: "game".into(),
            game,
            pair: 0,
            arm: 0,
            seed: game as u64,
            seat,
            genome: genome.into(),
            win,
            winner: None,
            victory: String::new(),
            turn: 1,
            score: 0,
            score_share: if win { 0.4 } else { 0.2 },
            rank: 1,
            cities: 0,
            alive: true,
            secs: 0.0,
            founded_religion: false,
            foreign_faith_cities: 0,
            faith: 0.0,
            inquisition: false,
            techs: 0,
            wonders: 0,
            military: 0.0,
            civ: String::new(),
            raid_wars: 0,
            campaign_plans: 0,
            campaign_wars: 0,
            campaign_captures: 0,
            campaign_pillages: 0,
            settlers_captured: 0,
            builders_captured: 0,
            religious_lost: 0,
            pillages: 0,
            raid_settler_prizes: 0,
            dvp: 0,
            rival_dvp: 0,
            tourists: 0,
            rival_tourists: 0,
            domestic: 0,
            victories_off: Vec::new(),
            difficulty: String::new(),
            rival_mix: String::new(),
            rival_target: String::new(),
        }
    }

    /// A synthetic screen: `games` games of three seats over `k` genes, every
    /// seat's genome drawn by the real draw, outcomes supplied by `outcome`
    /// from the seat's bits.
    fn synthetic(
        k: usize,
        games: usize,
        outcome: impl Fn(&[bool], usize) -> (bool, f64),
    ) -> (Header, Vec<Row>) {
        let names: Vec<String> = (0..k).map(|i| format!("g{i}")).collect();
        let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let header = test_header(&name_refs);
        let probabilities = vec![0.5; k];
        let mut rows = Vec::new();
        for game in 0..games {
            for seat in 0..3 {
                let bits = draw_genome(11, game, 3, seat, &probabilities, &[]);
                let (win, share) = outcome(&bits, seat);
                let mut row = test_row(game, seat, &genome_string(&bits), win);
                row.score_share = share;
                rows.push(row);
            }
        }
        (header, rows)
    }

    /// The gene table is discovered from the repository's tables and every
    /// row can actually be flipped on a real controller.
    #[test]
    fn every_gene_is_a_real_flag_with_a_toggle() {
        let genes = gene_table();
        assert_eq!(
            genes.len(),
            civvis::ai::GENES.iter().filter(|g| g.screenable()).count()
        );
        let mut tags: Vec<&str> = genes.iter().map(|g| g.tag).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), genes.len(), "a gene tag is repeated");
        // Host-only flags are excluded by construction, not by luck — unless
        // the registry says the flag also acts on a native board
        // (`Kind::HostOnlyOptIn`: `joint-tactics`, whose search runs
        // headless). A plain `HostOnly` row never reaches the genome.
        for gene in &genes {
            let row = civvis::ai::gene(gene.tag).expect("registered");
            assert!(
                !row.host_only() || row.opt_in(),
                "{} is host-only and would screen as noise",
                gene.tag
            );
        }
        // Flipping every gene on a live controller must not panic.
        let mut ai = AdvancedAi::new();
        ai.enable_engine_repairs();
        for gene in &genes {
            (gene.flip)(&mut ai);
        }
    }

    /// The same seed, game and seat draw the same genome; different seats of
    /// one game are independent draws; a held gene never moves.
    #[test]
    fn a_seat_reproduces_from_its_seed_and_seats_are_independent() {
        let probabilities = [0.5, 0.75, 1.0, 0.0, 0.5];
        let a = draw_genome(7, 3, 6, 2, &probabilities, &[]);
        assert_eq!(a, draw_genome(7, 3, 6, 2, &probabilities, &[]));
        assert!(a[2], "a gene held on stays on");
        assert!(!a[3], "a gene held off stays off");
        let mut identical = 0;
        for game in 0..200 {
            let seat0 = draw_genome(7, game, 6, 0, &probabilities, &[]);
            let seat1 = draw_genome(7, game, 6, 1, &probabilities, &[]);
            if seat0 == seat1 {
                identical += 1;
            }
            // No seat is ever another seat's complement by construction: the
            // held genes agree, and the drawn ones are independent coins.
            assert_eq!(seat0[2], seat1[2]);
        }
        assert!(
            identical < 60,
            "{identical} of 200 seat pairs drew the same genome: the seats are not independent"
        );
        assert_eq!(genome_string(&[true, false, true]), "101");
    }

    /// Over many seats a gene is on about as often as its probability says —
    /// one half for an ordinary gene, three quarters for a default-on one.
    #[test]
    fn genes_follow_their_draw_probabilities() {
        let probabilities = [0.5, 0.75, 0.5, 0.75];
        let mut on = [0usize; 4];
        let seats = 4000;
        for index in 0..seats {
            for (i, &bit) in draw_genome(99, index / 6, 6, index % 6, &probabilities, &[])
                .iter()
                .enumerate()
            {
                on[i] += usize::from(bit);
            }
        }
        for (i, count) in on.iter().enumerate() {
            let rate = *count as f64 / seats as f64;
            let want = probabilities[i];
            assert!(
                (rate - want).abs() < 0.04,
                "gene {i} on-rate {rate} is not near {want}"
            );
        }
    }

    /// The draw probability of every gene in header order: the deployment's
    /// default-on genes at `p_default_on`, the rest at `p_on`, and a gene held
    /// out of the screen pinned to its default.
    #[test]
    fn on_probabilities_follow_the_deployment_default() {
        let genes = gene_table();
        let mut screened = vec![true; genes.len()];
        screened[0] = false;
        let p = on_probabilities(&genes, &screened, 0.5, 0.75, &[]);
        assert_eq!(p[0], if genes[0].default_on { 1.0 } else { 0.0 });
        for (gene, (&p, &s)) in genes.iter().zip(p.iter().zip(&screened)).skip(1) {
            assert!(s);
            assert_eq!(p, if gene.default_on { 0.75 } else { 0.5 }, "{}", gene.tag);
        }
        assert!(
            genes.iter().any(|gene| gene.default_on) && genes.iter().any(|gene| !gene.default_on),
            "the table should hold both default-on and default-off genes"
        );
    }

    /// A seat plays exactly the genome it was drawn: the bit wins over the
    /// universe and over the ledger's default.
    #[test]
    fn a_seat_plays_exactly_its_genome() {
        // Nothing observable is exposed for most flags, so this test pins the
        // logic on the one flag that is public: `siege_is_progress` is an
        // engine repair, on after setup, off on stock.
        let genes = gene_table();
        let index = genes
            .iter()
            .position(|g| g.tag == "siege-is-progress")
            .expect("siege-is-progress is an engine repair");
        let mut on = vec![false; genes.len()];
        on[index] = true;
        assert!(seat_with_genome(&genes, &on).siege_is_progress);
        let off = vec![false; genes.len()];
        assert!(!seat_with_genome(&genes, &off).siege_is_progress);
        let universe: Vec<bool> = genes.iter().map(|gene| gene.after_setup_on).collect();
        assert!(seat_with_genome(&genes, &universe).siege_is_progress);
    }

    #[test]
    fn the_read_column_names_both_axes() {
        assert_eq!(read_column(0.5, -0.3, 3.33), "~");
        assert_eq!(read_column(-0.8, -7.2, 3.33), "share HURTS **");
        assert_eq!(read_column(-3.5, -1.0, 3.33), "HURTS **");
        // ⚠ 2.0 <= |z| < 2.8 is a reading the run was not powered to find.
        // It keeps its flag and says so; see `POWERED_Z`.
        assert_eq!(read_column(2.4, 0.1, 3.33), "helps * (thin)");
        assert_eq!(
            read_column(2.1, 4.9, 3.33),
            "helps * (thin) · share HELPS **"
        );
    }

    /// ⚠⚠ THE ROW THAT PROMPTED THIS. `defensible-sites`' fires artifact was
    /// written on 2026-08-25 reading `win_delta_pp +42.9` beside
    /// `win_resolves_pp 57.1` — a forty-three point difference from a run that
    /// cannot resolve anything under fifty-seven — and the verdict column
    /// printed `HELPS **` on it, because the family-wise bar for a single gene
    /// is 1.96 while the run's own 80%-power threshold is `POWERED_Z`.
    #[test]
    fn a_starred_verdict_needs_the_power_to_back_it() {
        // The artifact's own numbers: Δ 42.9 pp against a 57.1 pp resolution
        // is z = 42.9 / (57.1 / 2.8) ≈ 2.10, and one gene bars at 1.96.
        let z = 42.9 / (57.1 / POWERED_Z);
        assert!((2.0..POWERED_Z).contains(&z), "the artifact's z is {z}");
        assert_eq!(
            read_column(z, 0.0, 1.96),
            "helps * (thin)",
            "a run that cannot resolve the difference must not star it"
        );

        // Clear of the power bar, the same single-gene screen stars it.
        assert_eq!(read_column(3.0, 0.0, 1.96), "HELPS **");
        // And the thin band still reports the difference — nothing is hidden.
        assert!(read_column(-2.5, 0.0, 1.96).starts_with("hurts *"));
        assert!(read_column(-2.5, 0.0, 1.96).ends_with("(thin)"));
    }

    #[test]
    fn the_normal_tables_are_right() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);
        assert!((normal_cdf(1.96) - 0.975).abs() < 1e-4);
        assert!((normal_quantile_upper(0.025) - 1.96).abs() < 1e-3);
        assert!((normal_quantile_upper(0.025 / 57.0) - 3.33).abs() < 0.02);
    }

    /// Noise-free planted on−off differences come back exactly from the
    /// all-genes regression, and the marginal contrast of each gene agrees
    /// with it up to the neighbours' chance imbalance.
    #[test]
    fn ols_recovers_planted_effects() {
        let planted = [0.3, -0.2, 0.0, 0.1, 0.05];
        let (header, rows) = synthetic(5, 400, |bits, _| {
            let share = 0.2
                + bits
                    .iter()
                    .zip(&planted)
                    .map(|(&b, d)| if b { d / 2.0 } else { -d / 2.0 })
                    .sum::<f64>();
            (false, share)
        });
        let seats = Seats::of(&header, &rows);
        let fitted = seats
            .adjusted(&seats.shares)
            .expect("1,200 seats support 5 genes");
        for (i, (effect, se)) in fitted.iter().enumerate() {
            assert!(
                (effect - planted[i]).abs() < 1e-9,
                "gene {i}: {effect} vs {}",
                planted[i]
            );
            assert!(
                se.abs() < 1e-6,
                "noise-free fit should carry no error, got {se}"
            );
        }
        // The marginal contrast on a single gene is the adjusted figure up to
        // the other genes' imbalance, which 1,200 seats leave small.
        let (marginal, _) = seats.contrast(0, &seats.shares);
        assert!((marginal - planted[0]).abs() < 0.03, "marginal {marginal}");
    }

    /// A gene that decides the outcome reads as such; a gene that does nothing
    /// reads as nothing — each holds its own against the other's randomness.
    #[test]
    fn estimate_reads_a_planted_gene_and_clears_an_inert_one() {
        // Gene 0 wins the seat unless it sits in chair 2, so the outcome is
        // decided by the gene and still carries variance for an error.
        let (header, rows) = synthetic(3, 300, |bits, seat| {
            (bits[0] && seat < 2, if bits[0] { 0.4 } else { 0.2 })
        });
        let estimates = estimate(&header, &rows);
        assert_eq!(estimates.seats, 900);
        assert_eq!(estimates.games, 300);
        let planted = &estimates.genes[0];
        assert_eq!(planted.tag, "g0");
        assert!(
            (planted.win_on - 2.0 / 3.0).abs() < 0.06,
            "on {}",
            planted.win_on
        );
        assert!(planted.win_off.abs() < 1e-9);
        assert!(
            (planted.win_delta - 2.0 / 3.0).abs() < 0.06,
            "Δ {}",
            planted.win_delta
        );
        assert!(planted.win_z() > 10.0, "z {}", planted.win_z());
        assert!(
            planted.n_on > 350 && planted.n_off > 350,
            "{}/{}",
            planted.n_on,
            planted.n_off
        );
        let inert = &estimates.genes[1];
        assert!(inert.win_z().abs() < 3.0, "inert gene z {}", inert.win_z());
        assert!(
            inert.win_delta.abs() < 0.12,
            "inert gene Δ {}",
            inert.win_delta
        );
        assert!(inert.adjusted.is_some());
    }

    /// The seats of one game share a winner, so the error is clustered on the
    /// game: a file whose seats are all in one game cannot carry an error at
    /// all, and perfectly duplicated seats do not shrink it.
    #[test]
    fn errors_cluster_by_game() {
        let design: Vec<Vec<f64>> = (0..40)
            .map(|i| vec![1.0, if i % 2 == 0 { 1.0 } else { -1.0 }])
            .collect();
        let y: Vec<f64> = (0..40)
            .map(|i| if i % 2 == 0 { 1.0 } else { 0.0 } + (i % 5) as f64 * 0.1)
            .collect();
        let one_game: Vec<GameKey> = vec![(1, 0); 40];
        assert!(ols_clustered(&design, &y, &one_game).is_none());
        let own_games: Vec<GameKey> = (0..40).map(|i| (i as u64, 0)).collect();
        let (_, se_independent) = ols_clustered(&design, &y, &own_games).unwrap()[1];
        // Duplicate every row inside its own game: the seat count doubles,
        // the information does not.
        let mut design2 = design.clone();
        design2.extend(design.iter().cloned());
        let mut y2 = y.clone();
        y2.extend(y.iter().copied());
        let mut games2 = own_games.clone();
        games2.extend(own_games.iter().copied());
        let (_, se_clustered) = ols_clustered(&design2, &y2, &games2).unwrap()[1];
        assert!(
            (se_clustered - se_independent).abs() < 0.2 * se_independent,
            "clustered {se_clustered} vs {se_independent}: duplicates inside a game must not count twice"
        );
        // Treated as independent seats, the duplicates would halve the variance.
        let naive: Vec<GameKey> = (0..80).map(|i| (i as u64, 0)).collect();
        let (_, se_naive) = ols_clustered(&design2, &y2, &naive).unwrap()[1];
        assert!(
            se_naive < 0.8 * se_clustered,
            "naive {se_naive} vs clustered {se_clustered}"
        );
    }

    /// Tranches are newest first (input order), never split a game, and
    /// their seats add up to what was analysed.
    #[test]
    fn reproducibility_tranches_are_newest_first_and_whole_game() {
        let (header, rows) = synthetic(2, 10, |bits, _| (bits[0], 0.3));
        let tranches = reproducibility_tranches(&header, &rows, 9, 3);
        assert_eq!(tranches.len(), 3);
        // 30 seats in games of 3: a 9-seat target is exactly 3 games.
        assert!(
            tranches.iter().all(|t| t.seats % 3 == 0),
            "{:?}",
            tranches.iter().map(|t| t.seats).collect::<Vec<_>>()
        );
        assert_eq!(tranches[0].seats, 9);
        assert_eq!(tranches[0].games, 3);
        // Re-ordering the input re-orders the tranches: the newest games are
        // the last written, not the highest seed.
        let mut reversed = rows.clone();
        reversed.reverse();
        let again = reproducibility_tranches(&header, &reversed, 9, 1);
        assert_eq!(again[0].seats, 9);
        // Everything analysed lands in some tranche when the count allows.
        let all = reproducibility_tranches(&header, &rows, 9, 10);
        assert_eq!(all.iter().map(|t| t.seats).sum::<usize>(), 30);
    }

    /// A planted two-factor synergy is recovered with its sign and size, and
    /// a pure main effect does not masquerade as one.
    #[test]
    fn interactions_recover_a_planted_synergy() {
        let (header, rows) = synthetic(3, 1500, |bits, _| {
            let s = |b: bool| if b { 1.0 } else { -1.0 };
            // y = 0.5 + 0.1·s0 + 0.05·(s1·s2): a main effect and one synergy.
            (
                false,
                0.5 + 0.1 * s(bits[0]) + 0.05 * s(bits[1]) * s(bits[2]),
            )
        });
        let (found, seats) = interactions(&header, &rows, |row| row.score_share);
        assert_eq!(seats, 4500);
        let pair = |a: usize, b: usize| found.iter().find(|i| i.a == a && i.b == b).expect("pair");
        let synergy = pair(1, 2);
        // Marginal, so the other terms' finite-sample correlation leaks in a
        // little; 4,500 seats hold it to the second decimal.
        assert!(
            (synergy.synergy - 0.2).abs() < 0.03,
            "4γ should be near 0.2, got {}",
            synergy.synergy
        );
        assert!(synergy.z() > 10.0, "z {}", synergy.z());
        let none = pair(0, 1);
        assert!(
            none.synergy.abs() < 0.03,
            "a main effect is not a synergy: {}",
            none.synergy
        );
    }

    /// The cost column is the cost of enabling a gene on ONE seat: a game's
    /// seconds per turn grow with how many of its seats carry the gene.
    #[test]
    fn timing_cost_is_per_enabled_seat() {
        let (header, mut rows) = synthetic(2, 400, |bits, _| (bits[0], 0.3));
        let mut on_count: BTreeMap<GameKey, usize> = BTreeMap::new();
        for row in &rows {
            *on_count.entry(row.game_key()).or_default() += usize::from(row.bits()[0]);
        }
        for row in rows.iter_mut() {
            let n = on_count[&row.game_key()] as f64;
            row.turn = 100;
            // +10% compute per enabled seat on gene 0; gene 1 costs nothing.
            row.secs = 50.0 * 1.10f64.powf(n);
        }
        let costs = estimate_costs(&header, &rows);
        let g0 = costs.get("g0").expect("gene 0 priced");
        assert!(
            (g0.compute_pct() - 10.0).abs() < 0.05,
            "{}",
            g0.compute_pct()
        );
        assert!((g0.time_pct() - 10.0).abs() < 0.05, "{}", g0.time_pct());
        let g1 = costs.get("g1").expect("gene 1 priced");
        assert!(g1.compute_pct().abs() < 0.05, "{}", g1.compute_pct());
        assert_eq!(g0.games, 400);
    }

    /// Rows without a timing publish no cost guess.
    #[test]
    fn rows_without_timings_publish_no_cost() {
        let (header, rows) = synthetic(2, 50, |bits, _| (bits[0], 0.3));
        assert!(
            estimate_costs(&header, &rows).is_empty(),
            "secs are 0.0 in test rows"
        );
    }

    #[test]
    fn rows_round_trip_through_json_and_a_legacy_row_still_reads() {
        let mut row = test_row(3, 2, "0110", true);
        row.seed = 42;
        row.winner = Some(2);
        row.victory = "science".into();
        row.civ = "rome".into();
        let text = serde_json::to_string(&row).unwrap();
        assert!(
            !text.contains("\"pair\""),
            "a new row carries no pair: {text}"
        );
        assert!(
            !text.contains("\"arm\""),
            "a new row carries no arm: {text}"
        );
        let back: Row = serde_json::from_str(&text).unwrap();
        assert_eq!(back.genome, "0110");
        assert_eq!(back.winner, Some(2));
        assert_eq!(back.game_key(), (42, 0));
        assert!(serde_json::from_str::<Header>(&text)
            .map(|h| h.kind != "header")
            .unwrap_or(true));
        // A paired-design row: the two arms of one pair share a seed and are
        // two games, which the game key tells apart.
        let legacy = r#"{"kind":"game","pair":3,"arm":1,"seed":42,"seat":2,"genome":"0110","win":true,
            "winner":2,"victory":"science","turn":210,"score":1234,"score_share":0.31,"rank":1,
            "cities":9,"alive":true,"secs":12.5}"#;
        let old: Row = serde_json::from_str(legacy).unwrap();
        assert_eq!(old.game_key(), (42, 1));
        assert_eq!(old.game, 0);
    }

    /// A file written by the paired designs still analyses: its rows are
    /// seats with genomes, its draw reads as one half, and its pre-registered
    /// comparisons read back as seats.
    #[test]
    fn a_legacy_file_still_analyses() {
        let mut header = test_header(&["a", "b"]);
        header.design = "foldover".into();
        header.prior = Vec::new();
        header.p_on = 0.0;
        header.p_default_on = 0.0;
        assert_eq!(header.on_probabilities(), vec![0.5, 0.5]);
        header.batch = Batch {
            target_games: 0,
            target_seats: 0,
            target_pairs: 100,
            target_comparisons: 600,
            seed_first: 1,
            seed_last: 100,
        };
        assert_eq!(header.batch.intended_seats(), 1_200);
        assert!(header.batch.pre_registered());
        let mut rows = Vec::new();
        for pair in 0..60u64 {
            for arm in 0..2u8 {
                for seat in 0..3 {
                    let bit = (pair as usize + seat).is_multiple_of(2) == (arm == 0);
                    let genome = if bit { "10" } else { "01" };
                    let mut row = test_row(0, seat, genome, bit);
                    row.seed = pair;
                    row.arm = arm;
                    row.pair = pair as usize;
                    rows.push(row);
                }
            }
        }
        let estimates = estimate(&header, &rows);
        assert_eq!(estimates.seats, 360);
        assert_eq!(estimates.games, 120, "two arms on one seed are two games");
        assert!((estimates.genes[0].win_delta - 1.0).abs() < 1e-9);
        assert!((estimates.genes[1].win_delta + 1.0).abs() < 1e-9);
    }

    /// The one-shape rule: the draw design is not a leg of the shape, so a
    /// foldover file at the screen's shape and an independent one both read
    /// `standard`; moving any map leg makes a probe.
    #[test]
    fn the_shape_ignores_the_draw_and_checks_every_map_leg() {
        let mut header = test_header(&["a"]);
        header.players = SCREEN_PLAYERS;
        header.width = SCREEN_WIDTH;
        header.height = SCREEN_HEIGHT;
        header.city_states = SCREEN_CITY_STATES;
        header.map = SCREEN_MAP.id().to_string();
        header.speed = GameSpeed::Online.id().to_string();
        header.turns = GameSpeed::Online.turn_limit();
        header.randomize_civs = true;
        assert_eq!(shape_of(&header), "standard");
        header.design = "foldover".into();
        assert_eq!(shape_of(&header), "standard");
        header.map = "pangaea".into();
        assert_eq!(shape_of(&header), "legacy");
    }

    /// ⚠⚠ THE CONTESTED FIELD MUST NEVER POOL WITH THE STANDARD SCREEN. Every
    /// recorded gene column was taken fieldless, and a contested batch differs
    /// in no map leg at all — same players, map, size, city-states, speed,
    /// clock, lanes and civ shuffle — so without these two legs it would read
    /// `standard` and re-price a hundred genes against a board they were never
    /// measured on. That is the whole safety property of the mode, and it is
    /// the one thing here worth a test of its own.
    #[test]
    fn a_contested_batch_is_never_the_standard_screen() {
        let mut header = test_header(&["a"]);
        header.players = SCREEN_PLAYERS;
        header.width = SCREEN_WIDTH;
        header.height = SCREEN_HEIGHT;
        header.city_states = SCREEN_CITY_STATES;
        header.map = SCREEN_MAP.id().to_string();
        header.speed = GameSpeed::Online.id().to_string();
        header.turns = GameSpeed::Online.turn_limit();
        header.randomize_civs = true;
        assert_eq!(
            shape_of(&header),
            "standard",
            "the fieldless control is the screen"
        );

        header.contested_field = "diplomatic,culture".into();
        assert_eq!(
            shape_of(&header),
            "legacy",
            "a pinned field is a different board"
        );
        header.contested_field = String::new();

        // And the competitions on their own, because a fieldless batch that
        // seats scored competitions is also not the world the ledger holds.
        header.native_competitions = true;
        assert_eq!(shape_of(&header), "legacy");
        header.native_competitions = false;
        assert_eq!(shape_of(&header), "standard");
    }

    /// Both legs default to the fieldless values, so every file written before
    /// the contested field keeps exactly the shape it always had.
    #[test]
    fn a_file_written_before_the_contested_field_is_still_the_screen() {
        let before = r#"{"kind":"header","genes":["a"],"screened":["a"],"players":6,"width":74,
            "height":46,"turns":250,"city_states":9,"speed":"online","map":"continents",
            "baseline":"best","start_seed":1,"randomize_civs":true,"all_seats":true,
            "design":"independent","prior":[0.5],"p_on":0.5,"p_default_on":0.75}"#
            .replace(['\n', ' '], "");
        let header: Header = serde_json::from_str(&before).expect("a pre-field header parses");
        assert!(header.contested_field.is_empty());
        assert!(!header.native_competitions);
        assert_eq!(shape_of(&header), "standard");
    }

    fn screen_header(genes: &[&str]) -> Header {
        let mut header = test_header(genes);
        header.players = SCREEN_PLAYERS;
        header.width = SCREEN_WIDTH;
        header.height = SCREEN_HEIGHT;
        header.city_states = SCREEN_CITY_STATES;
        header.map = SCREEN_MAP.id().to_string();
        header.speed = GameSpeed::Online.id().to_string();
        header.turns = GameSpeed::Online.turn_limit();
        header.randomize_civs = true;
        header
    }

    /// ⭐ `rotate:2` is ten masks, each seed always the same one, every mask
    /// exactly a tenth of a thousand consecutive seeds, every lane closed in
    /// exactly two fifths of them — and score never.
    #[test]
    fn the_victory_mask_is_deterministic_and_balanced() {
        let mask = VictoryMask::parse("rotate:2").expect("parses");
        let all = civvis::game::VictoryConditions::default();
        assert_eq!(mask.masks(all).len(), 10, "C(5,2)");
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut closed_per_lane: BTreeMap<&str, usize> = BTreeMap::new();
        for seed in 26_081_900u64..26_082_900 {
            let closed = mask.closed(seed, all);
            assert_eq!(
                closed,
                mask.closed(seed, all),
                "the same seed, the same mask"
            );
            assert_eq!(closed.len(), 2);
            let mut sorted = closed.clone();
            sorted.sort_unstable();
            assert_eq!(closed, sorted, "closed lanes are sorted");
            assert!(
                !closed.contains(&"score"),
                "score is the clock and never closes"
            );
            for lane in &closed {
                *closed_per_lane.entry(lane).or_default() += 1;
            }
            *counts.entry(mask_key(&closed)).or_default() += 1;
            let applied = mask.apply(seed, all);
            assert!(applied.score);
            for lane in MASKABLE_LANES {
                assert_eq!(
                    applied.is_enabled(lane),
                    !closed.contains(&lane),
                    "{lane} at {seed}"
                );
            }
        }
        assert_eq!(counts.len(), 10);
        assert!(counts.values().all(|&n| n == 100), "{counts:?}");
        assert_eq!(closed_per_lane.len(), 5);
        assert!(
            closed_per_lane.values().all(|&n| n == 400),
            "{closed_per_lane:?}"
        );
        assert_eq!(
            mask.games_by_mask(26_081_900, 1000, all),
            counts,
            "the header pre-registers exactly what the seeds play"
        );
    }

    /// Score stays on for every N, and a rotation must leave a real lane open.
    #[test]
    fn the_victory_mask_never_closes_score_at_any_width() {
        let all = civvis::game::VictoryConditions::default();
        for n in 1..=4 {
            let mask = VictoryMask::parse(&format!("rotate:{n}")).expect("parses");
            for seed in 0..64u64 {
                assert!(mask.apply(seed, all).score);
                assert!(!mask.closed(seed, all).contains(&"score"));
            }
        }
        // A batch-level restriction narrows what the rotation draws from.
        let two = civvis::game::VictoryConditions::parse("science,culture,score").expect("parses");
        let mask = VictoryMask::parse("rotate:1").expect("parses");
        assert_eq!(mask.lanes(two), vec!["science", "culture"]);
        assert_eq!(mask.masks(two), vec![vec!["science"], vec!["culture"]]);
        assert!(VictoryMask::parse("rotate:0").is_err());
        assert!(VictoryMask::parse("random:2").is_err());
        assert!(VictoryMask::parse("rotate:x").is_err());
    }

    /// A row carries the lanes its game closed, sorted; an unmasked row writes
    /// nothing and a file written before the field reads as unmasked.
    #[test]
    fn rows_carry_the_lanes_their_game_closed() {
        let mut row = test_row(3, 1, "01", false);
        let text = serde_json::to_string(&row).expect("serializes");
        assert!(
            !text.contains("victories_off"),
            "an unmasked row is unchanged: {text}"
        );
        let back: Row = serde_json::from_str(&text).expect("parses");
        assert!(back.victories_off.is_empty());
        row.victories_off = vec!["culture".into(), "science".into()];
        let text = serde_json::to_string(&row).expect("serializes");
        assert!(
            text.contains(r#""victories_off":["culture","science"]"#),
            "{text}"
        );
        let back: Row = serde_json::from_str(&text).expect("parses");
        assert_eq!(back.victories_off, row.victories_off);
    }

    /// ⭐ A rotating batch IS the standard screen: `victories` is the batch-level
    /// set and every lane is live across the batch. A `--victories`
    /// restriction stays a probe.
    #[test]
    fn a_rotating_mask_batch_is_the_standard_screen() {
        let mut header = screen_header(&["a"]);
        assert_eq!(shape_of(&header), "standard");
        header.victory_mask = "rotate:2".into();
        header.victory_mask_games = BTreeMap::from([("culture+science".to_string(), 1)]);
        assert_eq!(
            shape_of(&header),
            "standard",
            "every lane is live across the batch"
        );
        header.victories = "domination,score".into();
        assert_eq!(
            shape_of(&header),
            "legacy",
            "a restricted batch-level set is a probe"
        );
    }

    /// The analyze split on a synthetic masked batch: a lane gene that pays
    /// only when its lane is open reads larger open than closed, the mask
    /// counts come back from the rows, and the report lands in the JSON.
    #[test]
    fn the_mask_split_reads_a_lane_gene_open_against_closed() {
        let mask = VictoryMask::parse("rotate:2").expect("parses");
        let all = civvis::game::VictoryConditions::default();
        let mut header = screen_header(&["lane-space-race", "lane-policy-deck", "settler-guard"]);
        header.victory_mask = mask.id();
        let probabilities = vec![0.5; 3];
        let mut rows = Vec::new();
        let games = 400;
        for game in 0..games {
            let seed = 500_000 + game as u64;
            let closed: Vec<String> = mask
                .closed(seed, all)
                .iter()
                .map(|s| s.to_string())
                .collect();
            let science_open = !closed.iter().any(|lane| lane == "science");
            // Six seats; the winner is the lowest seat with the space-race gene
            // on when science is open, else a fixed rotation independent of it.
            let genomes: Vec<Vec<bool>> = (0..6)
                .map(|seat| draw_genome(7, game, 6, seat, &probabilities, &[]))
                .collect();
            let winner = if science_open {
                genomes.iter().position(|bits| bits[0]).unwrap_or(game % 6)
            } else {
                game % 6
            };
            for (seat, bits) in genomes.iter().enumerate() {
                let mut row = test_row(game, seat, &genome_string(bits), seat == winner);
                row.seed = seed;
                row.victories_off = closed.clone();
                rows.push(row);
            }
        }
        let report = mask_report(&header, &rows).expect("a masked batch reports");
        assert_eq!(report.games, games);
        assert_eq!(report.by_mask.len(), 10);
        assert!(
            report.by_mask.values().all(|&n| n == 40),
            "{:?}",
            report.by_mask
        );
        let science = report
            .lanes
            .iter()
            .find(|(lane, _, _)| *lane == "science")
            .expect("science");
        assert_eq!((science.1, science.2), (240, 160));
        // One split for the space race (science), five for the policy deck,
        // none for settler-guard.
        assert_eq!(
            report.splits.len(),
            6,
            "{:?}",
            report
                .splits
                .iter()
                .map(|s| (&s.tag, s.lane))
                .collect::<Vec<_>>()
        );
        let race = report
            .splits
            .iter()
            .find(|s| s.tag == "lane-space-race")
            .expect("space race");
        assert_eq!(race.lane, "science");
        assert!(race.open.0 > 0.3, "open Δ {:+.3}", race.open.0);
        assert!(race.closed.0.abs() < 0.1, "closed Δ {:+.3}", race.closed.0);
        assert!(race.difference().0 > 0.3);
        assert!(report.splits.iter().all(|s| s.tag != "settler-guard"));
        let json = report.json();
        assert_eq!(json["mask"], "rotate:2");
        assert_eq!(json["lane_genes"].as_array().expect("array").len(), 6);
        // And an unmasked batch reports nothing at all.
        let plain = screen_header(&["lane-space-race"]);
        let plain_rows = vec![test_row(0, 0, "1", true), test_row(0, 1, "0", false)];
        assert!(mask_report(&plain, &plain_rows).is_none());
        print_victory_masks(&header, &rows);
    }

    /// ⭐ `king:1,emperor:2,immortal:1` over a thousand consecutive seeds is
    /// 250 / 500 / 250 exactly, the same seed always the same rung, and the
    /// header pre-registers what the seeds play.
    #[test]
    fn the_rung_rotation_is_deterministic_and_takes_its_shares() {
        let known = ["prince", "king", "emperor", "immortal", "deity"];
        let rotation =
            DifficultyRotation::parse("king:1,emperor:2,immortal:1", &known).expect("parses");
        assert_eq!(rotation.id(), "king:1,emperor:2,immortal:1");
        assert_eq!(rotation.total(), 4);
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for seed in 26_081_900u64..26_082_900 {
            let rung = rotation.rung(seed);
            assert_eq!(rung, rotation.rung(seed));
            *counts.entry(rung.to_string()).or_default() += 1;
        }
        assert_eq!(
            counts,
            BTreeMap::from([
                ("king".to_string(), 250),
                ("emperor".to_string(), 500),
                ("immortal".to_string(), 250)
            ])
        );
        assert_eq!(rotation.games_by_rung(26_081_900, 1000), counts);
        // A bare rung counts once; the refusals.
        let bare = DifficultyRotation::parse("king,emperor", &known).expect("parses");
        assert_eq!(bare.id(), "king:1,emperor:1");
        assert!(
            DifficultyRotation::parse("king:1", &known).is_err(),
            "one rung is --difficulty"
        );
        assert!(DifficultyRotation::parse("king:0,emperor:1", &known).is_err());
        assert!(DifficultyRotation::parse("king:1,king:1", &known).is_err());
        assert!(DifficultyRotation::parse("king:1,godlike:1", &known).is_err());
        assert!(DifficultyRotation::parse("king:x,emperor:1", &known).is_err());
    }

    /// A row carries its game's rung; an old row reads as the Prince default.
    #[test]
    fn rows_carry_the_rung_their_game_played() {
        let mut row = test_row(1, 0, "1", true);
        let text = serde_json::to_string(&row).expect("serializes");
        assert!(!text.contains("difficulty"), "{text}");
        assert_eq!(
            row_rung(&serde_json::from_str::<Row>(&text).expect("parses")),
            "prince"
        );
        row.difficulty = "emperor".into();
        let back: Row = serde_json::from_str(&serde_json::to_string(&row).expect("serializes"))
            .expect("parses");
        assert_eq!(back.difficulty, "emperor");
        assert_eq!(row_rung(&back), "emperor");
    }

    /// A rotating batch at the screen's shape is the standard screen: the
    /// rung is provenance, not a leg.
    #[test]
    fn a_rung_rotation_batch_is_the_standard_screen() {
        let mut header = screen_header(&["a"]);
        header.difficulty = "emperor".into();
        assert_eq!(shape_of(&header), "standard");
        header.difficulty = String::new();
        header.difficulty_rotate = "king:1,emperor:2,immortal:1".into();
        assert_eq!(shape_of(&header), "standard");
    }

    /// The per-rung table on a synthetic rotating batch: games per rung in
    /// ladder order, the top genes by |win Δ|, and a gene that pays on one
    /// rung only reads that way.
    #[test]
    fn the_rung_table_reads_a_gene_per_rung() {
        let known = ["king", "emperor", "immortal"];
        let rotation =
            DifficultyRotation::parse("king:1,emperor:2,immortal:1", &known).expect("parses");
        let mut header = screen_header(&["only-on-immortal", "everywhere", "inert"]);
        header.difficulty_rotate = rotation.id();
        let probabilities = vec![0.5; 3];
        let mut rows = Vec::new();
        let games = 400;
        for game in 0..games {
            let seed = 700_000 + game as u64;
            let rung = rotation.rung(seed).to_string();
            let genomes: Vec<Vec<bool>> = (0..6)
                .map(|seat| draw_genome(9, game, 6, seat, &probabilities, &[]))
                .collect();
            // `everywhere` wins on every rung; `only-on-immortal` decides
            // only immortal games, where it outranks everything.
            let winner = if rung == "immortal" {
                genomes.iter().position(|bits| bits[0]).unwrap_or(game % 6)
            } else {
                genomes.iter().position(|bits| bits[1]).unwrap_or(game % 6)
            };
            for (seat, bits) in genomes.iter().enumerate() {
                let mut row = test_row(game, seat, &genome_string(bits), seat == winner);
                row.seed = seed;
                row.difficulty = rung.clone();
                rows.push(row);
            }
        }
        let report = rung_report(&header, &rows).expect("a rotating batch reports");
        assert_eq!(
            report.games,
            vec![
                ("king".to_string(), 100),
                ("emperor".to_string(), 200),
                ("immortal".to_string(), 100)
            ],
            "ladder order, exact shares"
        );
        assert_eq!(report.genes.len(), 3);
        let immortal_only = report
            .genes
            .iter()
            .find(|(tag, _, _)| tag == "only-on-immortal")
            .expect("in the table");
        let per_rung = &immortal_only.2;
        assert!(per_rung[2].0 > 0.3, "immortal Δ {:+.3}", per_rung[2].0);
        assert!(per_rung[0].0.abs() < 0.15, "king Δ {:+.3}", per_rung[0].0);
        assert_eq!(per_rung[0].2 + per_rung[1].2 + per_rung[2].2, 2400);
        let json = report.json();
        assert_eq!(json["rotation"], rotation.id());
        assert_eq!(json["top_genes"].as_array().expect("array").len(), 3);
        // One rung throughout, no rotation declared: nothing to split.
        let plain = screen_header(&["a"]);
        let plain_rows = vec![test_row(0, 0, "1", true), test_row(0, 1, "0", false)];
        assert!(rung_report(&plain, &plain_rows).is_none());
        print_difficulty_rungs(&header, &rows);
    }

    /// ⭐ The rival kind rotates through the three kinds exactly, the chair
    /// rotates with the game, and a firaxis-mix rival's lane comes in the
    /// live census's shares over a window of 72 firaxis-mix games.
    #[test]
    fn the_rival_mix_rotates_kind_chair_and_lane_deterministically() {
        let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
        for seed in 26_081_900u64..26_082_899 {
            assert_eq!(rival_kind(seed), rival_kind(seed));
            *kinds.entry(rival_kind(seed)).or_default() += 1;
        }
        assert_eq!(
            kinds.values().copied().collect::<Vec<_>>(),
            vec![333, 333, 333]
        );
        assert_eq!(
            rival_games(26_081_900, 999)
                .values()
                .copied()
                .collect::<Vec<_>>(),
            vec![333, 333, 333]
        );
        assert_eq!(
            (0..12).map(|game| rival_index(6, game)).collect::<Vec<_>>(),
            [0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5]
        );
        // Every third seed is a firaxis-mix game; 72 of them play each lane
        // in its weight exactly.
        let mut lanes: BTreeMap<&str, usize> = BTreeMap::new();
        let mut seen = 0;
        let mut seed = 0u64;
        while seen < 72 {
            if rival_kind(seed) == "firaxis-mix" {
                *lanes.entry(firaxis_mix_target(seed).as_str()).or_default() += 1;
                seen += 1;
            }
            seed += 1;
        }
        assert_eq!(
            lanes,
            BTreeMap::from([
                ("diplomatic", 32),
                ("culture", 27),
                ("religious", 8),
                ("science", 4),
                ("domination", 1)
            ])
        );
        // The three seats build.
        let genes = gene_table();
        let genome = vec![false; genes.len()];
        assert!(rival_seat("legacy", 1, &genes, &genome)
            .victory_target()
            .is_none());
        assert_eq!(
            rival_seat("firaxis-mix", 1, &genes, &genome).victory_target(),
            Some(firaxis_mix_target(1))
        );
        assert!(rival_seat("random", 1, &genes, &genome)
            .victory_target()
            .is_none());
    }

    /// A rival's row is `kind: "rival"` and prices nothing; the measured rows
    /// say `measured`; the split reads a gene per rival kind.
    #[test]
    fn the_rival_seat_prices_no_gene_and_the_split_reads_per_kind() {
        let mut header = screen_header(&["pays-vs-legacy", "everywhere"]);
        header.rivals = "firaxis-mix".into();
        let probabilities = vec![0.5; 2];
        let mut rows = Vec::new();
        let games = 600;
        for game in 0..games {
            let seed = 900_000 + game as u64;
            let kind = rival_kind(seed);
            let at = rival_index(6, game);
            let genomes: Vec<Vec<bool>> = (0..6)
                .map(|seat| draw_genome(3, game, 6, seat, &probabilities, &[]))
                .collect();
            // `everywhere` wins whatever the rival; against the legacy anchor
            // a seat with `pays-vs-legacy` as well takes precedence.
            let measured = |wanted: &dyn Fn(&[bool]) -> bool| {
                genomes
                    .iter()
                    .enumerate()
                    .find(|(seat, bits)| *seat != at && wanted(bits))
                    .map(|(seat, _)| seat)
            };
            let winner = (kind == "legacy")
                .then(|| measured(&|bits| bits[0] && bits[1]))
                .flatten()
                .or_else(|| measured(&|bits| bits[1]))
                .unwrap_or(at);
            for (seat, bits) in genomes.iter().enumerate() {
                let mut row = test_row(game, seat, &genome_string(bits), seat == winner);
                row.seed = seed;
                if seat == at {
                    row.kind = "rival".into();
                    row.rival_mix = kind.into();
                    row.genome = String::new();
                } else {
                    row.rival_mix = "measured".into();
                }
                rows.push(row);
            }
        }
        let estimates = estimate(&header, &rows);
        assert_eq!(estimates.seats, games * 5, "the rival seat is not a seat");
        let report = rival_report(&header, &rows).expect("a mixed batch reports");
        assert_eq!(
            report
                .kinds
                .iter()
                .map(|(_, games, _)| *games)
                .collect::<Vec<_>>(),
            vec![200, 200, 200]
        );
        assert!(report
            .genes
            .iter()
            .any(|(tag, _, _, _)| tag == "everywhere"));
        let everywhere = report
            .genes
            .iter()
            .find(|(tag, _, _, _)| tag == "everywhere")
            .expect("past the bar");
        assert!(everywhere.3, "pays against every kind: {:?}", everywhere.2);
        assert_eq!(
            everywhere.2.iter().map(|(_, _, n)| *n).sum::<usize>(),
            games * 5
        );
        if let Some(legacy_only) = report
            .genes
            .iter()
            .find(|(tag, _, _, _)| tag == "pays-vs-legacy")
        {
            assert!(
                legacy_only.2[0].0 > legacy_only.2[1].0,
                "{:?}",
                legacy_only.2
            );
        }
        let json = report.json();
        assert_eq!(json["rivals"], "firaxis-mix");
        assert_eq!(json["kinds"].as_array().expect("array").len(), 3);
        // Round trip: the rival row keeps its kind and mix.
        let rival = rows
            .iter()
            .find(|row| row.kind == "rival")
            .expect("a rival row");
        let back: Row = serde_json::from_str(&serde_json::to_string(rival).expect("serializes"))
            .expect("parses");
        assert_eq!(
            (back.kind.as_str(), back.rival_mix.as_str()),
            ("rival", rival.rival_mix.as_str())
        );
        // No mix, no report; and the mix is not a shape leg.
        let plain = screen_header(&["a"]);
        assert!(rival_report(&plain, &[test_row(0, 0, "1", true)]).is_none());
        assert_eq!(shape_of(&header), "standard");
        print_rival_mix(&header, &rows);
    }

    /// ⭐ The drift meter reads the batch by GAME, maps the live ladder's own
    /// `victory_type`s, and keeps the loss census beside them.
    #[test]
    fn the_drift_meter_reads_the_batch_beside_the_live_census() {
        let mut rows = Vec::new();
        for game in 0..10 {
            for seat in 0..3 {
                let mut row = test_row(game, seat, "1", seat == 0);
                row.victory = match game % 4 {
                    0 | 1 => "score".into(),
                    2 => "religious".into(),
                    _ => "culture".into(),
                };
                rows.push(row);
            }
        }
        let dir = std::env::temp_dir().join(format!("civvis-drift-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let ladder = dir.join("ladder.json");
        std::fs::write(
            &ladder,
            r#"{"attempts":[{"victory_type":"VICTORY_SCORE"},{"victory_type":"VICTORY_DIPLOMATIC"},
                {"victory_type":"VICTORY_DIPLOMATIC"},{"victory_type":"VICTORY_DEFAULT"},{"won":false}]}"#,
        )
        .expect("write");
        let meter = drift_meter_with(&rows, ladder.to_str().expect("path")).expect("games");
        assert_eq!(meter.games, 10, "by game, not by seat");
        assert_eq!(
            meter.batch,
            BTreeMap::from([("score", 6), ("religious", 2), ("culture", 2)])
        );
        assert_eq!(
            meter.ladder,
            Some(BTreeMap::from([("score", 1), ("diplomatic", 2)])),
            "VICTORY_DEFAULT and an unfinished attempt are not endings"
        );
        let json = meter.json();
        assert!((json["batch"]["score"].as_f64().expect("share") - 0.6).abs() < 1e-9);
        assert_eq!(json["live_ladder"]["games"], 3);
        assert_eq!(json["live_loss_census"]["games"], 72);
        assert_eq!(json["live_loss_census"]["source"], LIVE_LOSS_CENSUS_SOURCE);
        let _ = std::fs::remove_dir_all(&dir);
        // Unreadable ladder: the column is omitted, the meter still reads.
        let without = drift_meter_with(&rows, "/nonexistent/ladder.json").expect("games");
        assert!(without.ladder.is_none());
        assert!(drift_meter_with(&[], "/nonexistent").is_none());
        print_drift_meter(&rows);
        // The real ledger's record, when this test runs in a checkout, maps.
        if let Some(live) = live_ladder_endings(LIVE_LADDER_JSON) {
            assert!(live.values().sum::<usize>() > 0);
        }
        assert_eq!(live_ending("VICTORY_TECHNOLOGY"), Some("science"));
        assert_eq!(live_ending("VICTORY_CONQUEST"), Some("domination"));
        assert_eq!(live_ending("VICTORY_DEFAULT"), None);
    }

    /// A lane gene's lane is read off the registry's own words; a version is
    /// read as its base; a gene of no lane has no split.
    #[test]
    fn lane_genes_map_to_their_lanes() {
        assert_eq!(lane_gene_lanes("lane-space-race"), Some(vec!["science"]));
        assert_eq!(
            lane_gene_lanes("lane-culture-spending-2"),
            Some(vec!["culture"])
        );
        assert_eq!(lane_gene_lanes("holy-lane-parity"), Some(vec!["religious"]));
        assert_eq!(
            lane_gene_lanes("competition-victory-points"),
            Some(vec!["diplomatic"])
        );
        assert_eq!(
            lane_gene_lanes("lane-policy-deck"),
            Some(MASKABLE_LANES.to_vec())
        );
        assert_eq!(
            lane_gene_lanes("lane-commit"),
            Some(MASKABLE_LANES.to_vec())
        );
        assert_eq!(lane_gene_lanes("settler-guard"), None);
    }

    /// The pinned positions rotate with the game index, because seat position
    /// is not neutral on this board: a fixed pin would hand every measured
    /// gene the leftovers of one particular chair for the whole batch.
    #[test]
    fn the_field_rotates_so_no_seat_is_always_pinned() {
        let field = [VictoryTarget::Diplomacy, VictoryTarget::Culture];
        let mut pinned_count = [0usize; 6];
        let mut measured_count = [0usize; 6];
        for game in 0..60 {
            let pinned = pinned_seats(6, game, &field);
            assert_eq!(pinned.iter().filter(|seat| seat.is_some()).count(), 2);
            assert_eq!(
                pinned.iter().filter(|seat| seat.is_none()).count(),
                4,
                "four drawn seats are measured"
            );
            for (seat, target) in pinned.iter().enumerate() {
                match target {
                    Some(_) => pinned_count[seat] += 1,
                    None => measured_count[seat] += 1,
                }
            }
        }
        assert!(
            pinned_count.iter().all(|&count| count == 20),
            "every seat is pinned equally often: {pinned_count:?}"
        );
        assert!(measured_count.iter().all(|&count| count == 40));
        // The lanes keep their identity: seat `game` is always the first lane.
        assert_eq!(
            pinned_seats(6, 7, &field)[1],
            Some(VictoryTarget::Diplomacy)
        );
        assert_eq!(pinned_seats(6, 7, &field)[2], Some(VictoryTarget::Culture));
    }

    /// A fieldless batch pins nobody, which is what keeps the standard screen
    /// byte-for-byte the game it always played.
    #[test]
    fn no_field_pins_no_seat() {
        assert_eq!(pinned_seats(6, 3, &[]), vec![None; 6]);
    }

    /// A field seat is pinned to its lane; a drawn seat is ADAPTIVE, whatever
    /// genome it drew. That separation is the mode: the pursuit belongs to the
    /// field and never leaks into the seats being measured.
    #[test]
    fn only_the_field_carries_a_pinned_lane() {
        for target in VictoryTarget::ALL {
            assert_eq!(field_seat(target, &[]).victory_target(), Some(target));
        }
        let genes = gene_table();
        for bits in [vec![false; genes.len()], vec![true; genes.len()]] {
            assert_eq!(
                seat_with_genome(&genes, &bits).victory_target(),
                None,
                "a drawn seat plays the adaptive victory planner, on any genome"
            );
        }
    }

    /// ⭐ A PURSUER PLAYS ITS LANE'S GENES; A MEASURED SEAT NEVER DOES. The
    /// seven `lane-*`/`competition-victory-points` opt-ins are the deciders
    /// that read the raced lane and they all ship off, so a field seated with
    /// the deployment genome alone races with its lane behaviour switched off —
    /// and the seven that read it are kept behind a flag because seating them
    /// measured WORSE — see `CONTESTED_FIELD_GENES`.
    #[test]
    fn the_field_genes_are_real_genes_and_reach_only_the_field() {
        let tags: Vec<&str> = civvis::ai::screenable_genes()
            .into_iter()
            .map(|gene| gene.tag)
            .collect();
        for tag in CONTESTED_FIELD_GENES {
            assert!(tags.contains(tag), "{tag} is not a registered gene");
        }
        let lane_genes: Vec<String> = CONTESTED_FIELD_GENES
            .iter()
            .map(|tag| (*tag).to_string())
            .collect();
        // The pursuit survives the extra genes, and the bare form is still
        // reachable for the comparison the constant's doc records.
        assert_eq!(
            field_seat(VictoryTarget::Diplomacy, &lane_genes).victory_target(),
            Some(VictoryTarget::Diplomacy)
        );
        assert_eq!(
            field_seat(VictoryTarget::Culture, &[]).victory_target(),
            Some(VictoryTarget::Culture)
        );
    }

    /// The denial axis reads the outcome the win column cannot see: losing the
    /// game to somebody else's victory of a named kind.
    #[test]
    fn denial_reads_a_loss_to_a_named_rival_victory() {
        let mut row = test_row(0, 0, "1", false);
        row.victory = "diplomatic".into();
        assert!(lost_to(&row, "diplomatic"));
        assert!(!lost_to(&row, "culture"));
        row.win = true;
        assert!(
            !lost_to(&row, "diplomatic"),
            "winning the diplomatic victory is not losing to one"
        );
        let unfinished = test_row(0, 0, "1", false);
        assert!(!lost_to(&unfinished, "diplomatic"), "no ending, no denial");
    }

    /// The board a batch played is printed beside the binary it played with,
    /// and a fieldless line says so rather than saying nothing.
    #[test]
    fn the_field_line_names_the_board() {
        let mut header = test_header(&["a"]);
        header.players = 6;
        assert!(field_line(&header).contains("none"));
        header.contested_field = "diplomatic,culture".into();
        header.native_competitions = true;
        let line = field_line(&header);
        assert!(line.contains("CONTESTED"), "{line}");
        assert!(line.contains("diplomatic + culture"), "{line}");
        assert!(line.contains("4 drawn seats"), "{line}");
        header.native_competitions = false;
        assert!(
            field_line(&header).contains("OFF"),
            "a contested field with no route to 20 points says so"
        );
    }

    /// A tag `<base>-<n>` whose base is a gene is that gene's version `n`;
    /// anything else is just a name.
    #[test]
    fn tags_form_families_only_when_the_base_is_a_gene() {
        let tags: Vec<String> = [
            "war-economy",
            "war-economy-2",
            "war-economy-3",
            "one-launch-pad",
            "search-cadence-20",
            "war-economy-1",
        ]
        .iter()
        .map(|t| t.to_string())
        .collect();
        let families = families_of(&tags);
        assert_eq!(families, vec![vec![0, 1, 2]], "{families:?}");
        assert!(families_of(&["a".to_string(), "b-2".to_string()]).is_empty());
    }

    /// A family is one level per seat: off, or exactly one version. Over many
    /// seats the family is on as often as its probability says and each
    /// version takes an equal share of it.
    #[test]
    fn a_family_is_drawn_one_version_at_a_time() {
        // genes: g0 plain, g1 = base, g2 = base-2, g3 = base-3; family p = 0.75
        let probabilities = [0.5, 0.25, 0.25, 0.25];
        let families = vec![vec![1, 2, 3]];
        let seats = 6000;
        let mut on = [0usize; 4];
        let mut family_on = 0usize;
        for index in 0..seats {
            let genome = draw_genome(5, index / 6, 6, index % 6, &probabilities, &families);
            let versions_on = genome[1..].iter().filter(|&&b| b).count();
            assert!(
                versions_on <= 1,
                "seat {index} played two versions: {genome:?}"
            );
            family_on += versions_on;
            for (i, &bit) in genome.iter().enumerate() {
                on[i] += usize::from(bit);
            }
        }
        let rate = |count: usize| count as f64 / seats as f64;
        assert!(
            (rate(family_on) - 0.75).abs() < 0.03,
            "family on {}",
            rate(family_on)
        );
        for (i, &count) in on.iter().enumerate().skip(1) {
            assert!(
                (rate(count) - 0.25).abs() < 0.03,
                "version {i} on {}",
                rate(count)
            );
        }
        assert!((rate(on[0]) - 0.5).abs() < 0.03);
        // Reproducible from the seed.
        assert_eq!(
            draw_genome(5, 7, 6, 2, &probabilities, &families),
            draw_genome(5, 7, 6, 2, &probabilities, &families)
        );
    }

    /// The best version plays most: with marginals 0.30 / 0.45 the family is
    /// on three quarters of the time and version 2 takes 60% of that.
    #[test]
    fn the_best_version_is_drawn_sixty_percent_of_the_time() {
        let probabilities = [0.5, 0.30, 0.45];
        let families = vec![vec![1, 2]];
        let seats = 6000;
        let mut on = [0usize; 3];
        for index in 0..seats {
            let genome = draw_genome(9, index / 6, 6, index % 6, &probabilities, &families);
            assert!(
                !(genome[1] && genome[2]),
                "seat {index} played two versions"
            );
            for (i, &bit) in genome.iter().enumerate() {
                on[i] += usize::from(bit);
            }
        }
        let rate = |count: usize| count as f64 / seats as f64;
        assert!(
            (rate(on[1]) - 0.30).abs() < 0.03,
            "version 1 on {}",
            rate(on[1])
        );
        assert!(
            (rate(on[2]) - 0.45).abs() < 0.03,
            "version 2 on {}",
            rate(on[2])
        );
        assert!((rate(on[1] + on[2]) - 0.75).abs() < 0.03);
    }

    /// ⭐ The tournament draw starts from the default genome: a default-on
    /// gene turns off one time in four, a default-off gene turns on one time
    /// in four, and a gene that is on plays its top version 60% of the time,
    /// the rest sharing the other 40% evenly. One version takes everything.
    #[test]
    fn the_draw_starts_from_the_default_genome() {
        assert_eq!(P_DEFAULT_ON, 0.75);
        assert_eq!(P_ON, 0.25);
        assert_eq!(BEST_VERSION_SHARE, 0.6);
        let close = |p: &[f64], want: &[f64]| {
            assert!(
                p.len() == want.len() && p.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-12),
                "{p:?} != {want:?}"
            );
        };
        close(&version_shares(&[4], Some(4)), &[1.0]);
        close(&version_shares(&[4], None), &[1.0]);
        close(&version_shares(&[4, 5], Some(5)), &[0.4, 0.6]);
        close(&version_shares(&[4, 5, 6], Some(4)), &[0.6, 0.2, 0.2]);
        close(
            &version_shares(&[4, 5, 6, 7], Some(6)),
            &[0.4 / 3.0, 0.4 / 3.0, 0.6, 0.4 / 3.0],
        );
        close(&version_shares(&[4, 5, 6], None), &[1.0 / 3.0; 3]);
        // Over the real gene table at the defaults: every screened gene sits at
        // exactly one of the two levels, or is a family member whose family
        // sums to one of them.
        let genes = gene_table();
        let tags: Vec<String> = genes.iter().map(|gene| gene.tag.to_string()).collect();
        let families = families_of(&tags);
        let screened = vec![true; genes.len()];
        let p = on_probabilities(&genes, &screened, P_ON, P_DEFAULT_ON, &families);
        let mut in_family = vec![false; genes.len()];
        for family in &families {
            let family_p: f64 = family.iter().map(|&i| p[i]).sum();
            let ships = family.iter().any(|&i| genes[i].default_on);
            let want = if ships { P_DEFAULT_ON } else { P_ON };
            assert!(
                (family_p - want).abs() < 1e-9,
                "{}: family on at {family_p}, want {want}",
                genes[family[0]].tag
            );
            for &i in family {
                in_family[i] = true;
            }
        }
        for (i, gene) in genes.iter().enumerate() {
            if in_family[i] {
                continue;
            }
            let want = if gene.default_on { P_DEFAULT_ON } else { P_ON };
            assert_eq!(p[i], want, "{}", gene.tag);
        }
    }

    /// The family's marginals: the family probability shared among the
    /// screened versions, 60% to the best version and the rest split evenly;
    /// a version held on forces its siblings off.
    #[test]
    fn family_marginals_share_the_family_probability() {
        let synthetic = |tag: &'static str, default_on: bool, tracked_wins: Option<f64>| Gene {
            field: tag,
            tag,
            after_setup_on: false,
            stock_on: false,
            default_on,
            tracked_wins,
            flip: |_| {},
        };
        let close = |p: &[f64], want: &[f64]| {
            assert!(
                p.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-12),
                "{p:?} != {want:?}"
            );
        };
        // An unmeasured family shares equally.
        let genes = vec![
            synthetic("g", false, None),
            synthetic("g-2", false, None),
            synthetic("g-3", false, None),
        ];
        let families = vec![vec![0, 1, 2]];
        let p = on_probabilities(&genes, &[true, true, true], 0.5, 0.75, &families);
        close(&p, &[0.5 / 3.0, 0.5 / 3.0, 0.5 / 3.0]);
        // The shipping version is the best and takes 60%; the family is on
        // at p_default_on because a version ships.
        let genes = vec![
            synthetic("g", true, Some(0.4)),
            synthetic("g-2", false, Some(0.9)),
        ];
        let families = vec![vec![0, 1]];
        let p = on_probabilities(&genes, &[true, true], 0.5, 0.75, &families);
        close(&p, &[0.45, 0.30]);
        // With nothing shipping, tracked wins decide, ties to the higher
        // version; the other two split the remaining 40% evenly.
        let genes = vec![
            synthetic("g", false, Some(0.4)),
            synthetic("g-2", false, Some(0.9)),
            synthetic("g-3", false, Some(0.9)),
        ];
        let p = on_probabilities(&genes, &[true, true, true], 0.5, 0.75, &[vec![0, 1, 2]]);
        close(&p, &[0.1, 0.1, 0.3]);
        // Hold a version at its default: on → its siblings are forced off;
        // off → it takes no share and the rest of the family carries on.
        let genes = vec![
            synthetic("g", true, Some(0.4)),
            synthetic("g-2", false, Some(0.9)),
        ];
        let held = on_probabilities(&genes, &[false, true], 0.5, 0.75, &[vec![0, 1]]);
        assert_eq!(held, vec![1.0, 0.0]);
        let held = on_probabilities(&genes, &[true, false], 0.5, 0.75, &[vec![0, 1]]);
        assert_eq!(held, vec![0.75, 0.0]);
    }

    /// A planted improvement reads as one: version 2 beats off and beats
    /// version 1; version 1 beats off; the cells carry the seats.
    #[test]
    fn family_contrasts_read_a_planted_improvement() {
        let names = ["g", "g-2"];
        let header = {
            let mut h = test_header(&names);
            h.families = vec![vec!["g".into(), "g-2".into()]];
            h.prior = vec![0.375, 0.375];
            h
        };
        let families = vec![vec![0, 1]];
        let mut rows = Vec::new();
        for game in 0..1500 {
            for seat in 0..3 {
                let bits = draw_genome(3, game, 3, seat, &[0.375, 0.375], &families);
                // off wins 20%, v1 30%, v2 50%, decided by a hash of the seat.
                let roll = ((game * 7 + seat * 13) % 10) as f64 / 10.0;
                let threshold = if bits[1] {
                    0.5
                } else if bits[0] {
                    0.3
                } else {
                    0.2
                };
                let win = roll < threshold;
                let mut row = test_row(game, seat, &genome_string(&bits), win);
                row.score_share = threshold;
                rows.push(row);
            }
        }
        assert_eq!(header.families(), vec![vec![0, 1]]);
        let families = estimate_families(&header, &rows);
        assert_eq!(families.len(), 1);
        let family = &families[0];
        assert_eq!(family.base, "g");
        assert_eq!(
            family
                .cells
                .iter()
                .map(|c| c.label.as_str())
                .collect::<Vec<_>>(),
            ["off", "g", "g-2"]
        );
        assert_eq!(family.cells.iter().map(|c| c.seats).sum::<usize>(), 4500);
        let by = |a: &str, b: &str| {
            family
                .contrasts
                .iter()
                .find(|c| c.a == a && c.b == b)
                .unwrap_or_else(|| panic!("contrast {b} − {a}"))
        };
        let v1 = by("off", "g");
        let v2 = by("off", "g-2");
        let step = by("g", "g-2");
        assert!(
            (v1.win_delta - 0.10).abs() < 0.05,
            "v1 − off {}",
            v1.win_delta
        );
        assert!(
            (v2.win_delta - 0.30).abs() < 0.05,
            "v2 − off {}",
            v2.win_delta
        );
        assert!(
            (step.win_delta - 0.20).abs() < 0.05,
            "v2 − v1 {}",
            step.win_delta
        );
        assert!(step.win_z() > 4.0, "z {}", step.win_z());
        // The share axis was planted noise-free, so it is exact.
        assert!((step.share_delta - 0.20).abs() < 1e-9);
    }

    // ─── provenance: the binary a screen was played by ────────────────────

    /// Every double-quoted string in `text`, with `//` and `/* */` comments
    /// removed first so a tag named inside a comment cannot join the table.
    ///
    /// ⚠ THIS IS `tools/genes.py::_quoted` IN THE OTHER LANGUAGE, and
    /// the rule is deliberately the simplest one that both can state without
    /// argument: strip comments, take the quoted strings in order.
    fn quoted(text: &str) -> Vec<String> {
        let mut stripped = String::with_capacity(text.len());
        let mut rest = text;
        loop {
            let line = rest.find("//");
            let block = rest.find("/*");
            match (line, block) {
                (None, None) => {
                    stripped.push_str(rest);
                    break;
                }
                (Some(at), None) => {
                    stripped.push_str(&rest[..at]);
                    rest = rest[at..].find('\n').map_or("", |end| &rest[at + end..]);
                }
                (None, Some(at)) => {
                    stripped.push_str(&rest[..at]);
                    rest = rest[at..]
                        .find("*/")
                        .map_or("", |end| &rest[at + end + 2..]);
                }
                (Some(a), Some(b)) if a < b => {
                    stripped.push_str(&rest[..a]);
                    rest = rest[a..].find('\n').map_or("", |end| &rest[a + end..]);
                }
                (Some(_), Some(b)) => {
                    stripped.push_str(&rest[..b]);
                    rest = rest[b..].find("*/").map_or("", |end| &rest[b + end + 2..]);
                }
            }
        }
        stripped
            .split('"')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect()
    }

    /// The body of `pub const <name>: … = &[ … ];`, brackets balanced.
    fn table_body<'a>(text: &'a str, name: &str) -> &'a str {
        let start = text
            .find(&format!("pub const {name}"))
            .unwrap_or_else(|| panic!("{name} is not declared"));
        let open = text[start..].find("= &[").expect("a slice literal") + start + 4;
        let mut depth = 1usize;
        for (offset, byte) in text[open..].bytes().enumerate() {
            match byte {
                b'[' => depth += 1,
                b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return &text[open..open + offset];
                    }
                }
                _ => {}
            }
        }
        panic!("{name} is not closed");
    }

    /// The gene tags a reader gets from the registry's text alone, in the
    /// order `gene_table()` builds them: every `Gene { tag: "…", field: "…",
    /// kind: Kind::… }` row of `GENES` whose kind is screenable — anything but
    /// a plain `Kind::HostOnly` — in order. `tools/genes.py` implements
    /// the same reading (and the older three-table one for older commits).
    fn tags_from_source_tables(root: &std::path::Path) -> Vec<String> {
        let registry = std::fs::read_to_string(root.join("src/ai/advanced/genes.rs"))
            .expect("src/ai/advanced/genes.rs");
        let body = table_body(&registry, "GENES");
        let mut tags = Vec::new();
        for row in body.split("Gene {").skip(1) {
            let strings = quoted(row);
            let tag = strings.first().expect("a row names its tag").clone();
            let kind = row
                .split("kind:")
                .nth(1)
                .and_then(|rest| rest.split(',').next())
                .map(str::trim)
                .expect("a row names its kind");
            if kind != "Kind::HostOnly" {
                tags.push(tag);
            }
        }
        tags
    }

    /// ⭐ THE GUARD'S FOUNDATION. `tools/genes.py` recomputes a screen's
    /// gene-set fingerprint from these two files at the commit the screen
    /// claims, by exactly this rule, and refuses the screen when the answer
    /// differs. If the text rule and the compiled table ever disagreed, that
    /// guard would refuse every honest screen instead of the dishonest one —
    /// so the rule is pinned here, against the table the binary actually
    /// varies, in the same change that adds the guard.
    #[test]
    fn the_gene_table_is_exactly_what_the_ledger_re_derives_from_the_tables() {
        let compiled: Vec<String> = gene_table()
            .iter()
            .map(|gene| gene.tag.to_string())
            .collect();
        let parsed = tags_from_source_tables(std::path::Path::new(env!("CARGO_MANIFEST_DIR")));
        assert_eq!(
            parsed, compiled,
            "the source tables and the compiled gene table disagree; \
             tools/genes.py reads the registry"
        );
        assert!(compiled.len() > 50, "the tables scrape found too few genes");
    }

    /// The fingerprint is the tags, newline-terminated, hashed — the exact
    /// string `tools/genes.py::gene_set_fingerprint` builds. Pinned on a
    /// literal so a change to either side is a failure and not a surprise.
    #[test]
    fn the_gene_set_fingerprint_is_the_tags_newline_terminated() {
        let genes = gene_table();
        let expected = sha256_hex(
            genes
                .iter()
                .map(|gene| format!("{}\n", gene.tag))
                .collect::<String>()
                .as_bytes(),
        );
        assert_eq!(gene_set_fingerprint(&genes), expected);
        // Two tags, so the constant below can be checked by hand against any
        // sha256 tool: printf 'a\nb\n' | shasum -a 256
        let two = [
            Gene {
                field: "a",
                tag: "a",
                after_setup_on: false,
                stock_on: false,
                default_on: false,
                tracked_wins: None,
                flip: |_| {},
            },
            Gene {
                field: "b",
                tag: "b",
                after_setup_on: false,
                stock_on: false,
                default_on: false,
                tracked_wins: None,
                flip: |_| {},
            },
        ];
        assert_eq!(
            gene_set_fingerprint(&two),
            "911169ddaaf146aff539f58c26c489af3b892dff0fe283c1c264c65ae5aa59a2"
        );
    }

    /// FIPS 180-4's own vectors, plus the length-block boundary a hand-written
    /// implementation gets wrong.
    #[test]
    fn sha256_matches_the_published_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // 55, 56 and 64 bytes: either side of the padding boundary and exactly
        // one block, where an off-by-one loses or doubles a chunk.
        assert_eq!(
            sha256_hex(&[b'a'; 55]),
            "9f4390f8d30c2dd92ec9f095b65e2b9ae9b0a925a5258e241c9f1e910f734318"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 56]),
            "b35439a4ac6f0948b6d6f9e3c6af0f5f590ce20f1bde7090ef7970686ec6738a"
        );
        assert_eq!(
            sha256_hex(&[b'a'; 64]),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
        );
    }

    /// The promoted-executable fallback reads the same names `server.rs` does.
    #[test]
    fn a_promoted_binary_names_its_own_revision() {
        let sha = "d23f92d944cd889aa4c9dfe58c37aceb8e55eabd";
        assert_eq!(
            promoted_binary_commit(&format!("civvis-{sha}")).as_deref(),
            Some(sha)
        );
        assert_eq!(promoted_binary_commit("gene_screen"), None);
        assert_eq!(promoted_binary_commit("civvis-not-a-sha"), None);
        assert_eq!(
            promoted_binary_commit(&format!("civvis-{}", &sha[..39])),
            None
        );
    }

    /// The operator learns before the batch, not after it.
    #[test]
    fn the_build_line_calls_out_an_unstamped_or_dirty_build() {
        let mut header = test_header(&["a", "b"]);
        header.build = Build {
            commit: "d23f92d944cd889aa4c9dfe58c37aceb8e55eabd".into(),
            commit_source: "env".into(),
            dirty: false,
            genes_sha256: "0".repeat(64),
            binary_sha256: "1".repeat(64),
        };
        let clean = build_line(&header);
        assert!(clean.contains("d23f92d944cd"), "{clean}");
        assert!(clean.contains("2 genes"), "{clean}");
        assert!(!clean.contains("⚠"), "{clean}");

        header.build.dirty = true;
        assert!(build_line(&header).contains("DIRTY TREE"));

        header.build.commit = String::new();
        header.build.commit_source = "unstamped-tree-moved".into();
        let unstamped = build_line(&header);
        assert!(unstamped.contains("UNSTAMPED"), "{unstamped}");
        assert!(unstamped.contains("unstamped-tree-moved"), "{unstamped}");

        header.build = Build::default();
        assert!(build_line(&header).contains("pre-fingerprint"));
    }

    /// A header written before 2026-08-23 still parses, and reads as
    /// unstamped rather than as a build that could not be identified.
    #[test]
    fn a_pre_fingerprint_header_still_reads() {
        let legacy = r#"{"kind":"header","genes":["a"],"screened":["a"],"players":6,
            "width":60,"height":38,"turns":250,"city_states":6,"speed":"online",
            "map":"pangaea","baseline":"best","field":"advanced","start_seed":1}"#;
        let header: Header = serde_json::from_str(legacy).expect("legacy header parses");
        assert_eq!(header.build, Build::default());
        assert_eq!(header.batch, Batch::default());
        assert!(header.build.genes_sha256.is_empty());
        assert_eq!(header.batch.target_comparisons, 0);
    }

    /// ⚠ A TRUNCATED RUN MUST NOT READ AS A COMPLETED ONE. P10 stopped at
    /// 5,858 of a planned 10,000 games and its artefact said nothing about it.
    #[test]
    fn a_partial_run_says_partial_and_a_complete_one_says_complete() {
        let mut header = test_header(&["a"]);
        header.batch = Batch {
            target_games: 10_000,
            target_seats: 60_000,
            target_pairs: 0,
            target_comparisons: 0,
            seed_first: 100_000_000,
            seed_last: 100_009_999,
        };
        let partial = completeness_line(&header, 17_574);
        assert!(partial.contains("PARTIAL SCREEN"), "{partial}");
        assert!(partial.contains("17574 of 60000"), "{partial}");
        assert!(partial.contains("29.3%"), "{partial}");
        assert!(partial.contains("100000000..100009999"), "{partial}");

        let done = completeness_line(&header, 60_000);
        assert!(done.contains("screen complete"), "{done}");
        assert!(!done.contains("PARTIAL"), "{done}");

        // A file written before pre-registration says so rather than guessing.
        header.batch = Batch::default();
        let unknown = completeness_line(&header, 17_574);
        assert!(unknown.contains("not pre-registered"), "{unknown}");
        assert!(!unknown.contains("complete"), "{unknown}");
    }

    /// Segments of one screen sum; the same segment written twice counts once.
    #[test]
    fn merged_targets_sum_segments_and_count_a_rewritten_header_once() {
        let segment = |first: u64, games: usize| Batch {
            target_games: games,
            target_seats: games * 6,
            target_pairs: 0,
            target_comparisons: 0,
            seed_first: first,
            seed_last: first + games as u64 - 1,
        };
        let mut targets = BTreeMap::new();
        for batch in [
            segment(1_000, 400),
            segment(2_000, 600),
            // the first segment's header, written again on a restart
            segment(1_000, 400),
        ] {
            targets.insert(batch.seed_first, batch);
        }
        let merged = merged_target(&targets);
        assert_eq!(merged.target_games, 1_000);
        assert_eq!(merged.target_seats, 6_000);
        assert_eq!((merged.seed_first, merged.seed_last), (1_000, 2_599));
        assert_eq!(merged_target(&BTreeMap::new()), Batch::default());
    }

    /// The seed window the finished games actually cover, which is the half of
    /// "actual against intended" the rows know.
    /// ⚠⚠ THE BLOCK THAT PRINTED `HURTS **` FOR A GENE THAT COULD NOT FIRE.
    ///
    /// See [`Seats::empty_arm_floor`]. Both shapes are held here, because the
    /// fix is only correct if it separates them: an empty arm with a handful
    /// of events must lose its precision, and an empty arm under an
    /// overwhelming effect must keep it.
    #[test]
    fn an_empty_arm_loses_its_precision_unless_the_effect_is_overwhelming() {
        let header = test_header(&["alpha"]);

        // The artifact: twelve games, six seats, one winner each, and no
        // winner ever drew the gene. The difference is real; the confidence
        // is not.
        let mut rows = Vec::new();
        for game in 0..12 {
            for seat in 0..6 {
                rows.push(test_row(
                    game,
                    seat,
                    if seat < 2 { "1" } else { "0" },
                    seat == 5,
                ));
            }
        }
        let seats = Seats::of(&header, &rows);
        let (delta, se) = seats.contrast(0, &seats.wins);
        assert!(delta < -0.1, "the difference is reported ({delta})");
        assert!(
            (delta / se).abs() < 2.0,
            "but it must not read significant (z {})",
            delta / se
        );

        // The real thing: the same empty arm, but the gene wins every game it
        // is drawn into. That effect is overwhelming and keeps its error.
        let mut planted = Vec::new();
        for game in 0..12 {
            for seat in 0..6 {
                planted.push(test_row(
                    game,
                    seat,
                    if seat < 2 { "1" } else { "0" },
                    seat == 0,
                ));
            }
        }
        let seats = Seats::of(&header, &planted);
        let (delta, se) = seats.contrast(0, &seats.wins);
        assert!(delta > 0.1, "the planted effect is reported ({delta})");
        assert!(
            (delta / se).abs() > 2.0,
            "and it must still read significant (z {})",
            delta / se
        );
    }

    /// ⚠⚠ A POINT ESTIMATE MUST NOT TRAVEL WITHOUT THE POWER THAT PRODUCED IT.
    ///
    /// `resolving_power` is the figure the `resolution:` line prints, and the
    /// two must be the same arithmetic or an artifact and its terminal output
    /// disagree about whether a reading means anything. See `POWER_FACTOR`.
    #[test]
    fn a_reading_carries_the_smallest_delta_its_run_could_resolve() {
        // A twelve-game single-gene probe's standard error, in proportion
        // units: the `+2.7 pp [-17.3, +22.7]` row this was taken from.
        let probe_se = 0.102;
        assert!(
            (resolving_power(probe_se).expect("finite") - 28.6).abs() < 0.1,
            "a twelve-game probe resolves about ±28.6 pp, not {:?}",
            resolving_power(probe_se)
        );
        // Which is the whole point: that run's own +2.7 pp reading is inside
        // its own noise, and so was every probe reading in this series.
        assert!(2.7 < resolving_power(probe_se).expect("finite"));
        assert!(22.2 < resolving_power(probe_se).expect("finite"));

        // A ninety-game nine-gene screen resolves about ±10.3 pp.
        let screen_se = 0.0368;
        assert!(
            (resolving_power(screen_se).expect("finite") - 10.3).abs() < 0.1,
            "{:?}",
            resolving_power(screen_se)
        );

        // An infinite error -- the empty-arm case #2452 widened -- has no
        // resolving power to report, and says so rather than inventing one.
        assert_eq!(resolving_power(f64::INFINITY), None);

        // The median is what the run-level figure reports, so a single
        // un-resolvable row cannot drag the whole run's number to infinity.
        let se = |win: f64| GeneEstimate {
            tag: "t".into(),
            n_on: 1,
            n_off: 1,
            win_on: 0.0,
            win_off: 0.0,
            win_delta: 0.0,
            win_se: win,
            share_delta: 0.0,
            share_se: win,
            adjusted: None,
        };
        let genes = vec![se(0.05), se(0.10), se(f64::INFINITY)];
        assert!(
            (median_win_se(&genes) - 0.10).abs() < 1e-9,
            "{}",
            median_win_se(&genes)
        );
    }

    #[test]
    fn the_played_seed_window_is_read_from_the_game_rows() {
        let mut rows = vec![
            test_row(0, 0, "1", true),
            test_row(0, 1, "0", false),
            test_row(7, 0, "1", true),
        ];
        rows[0].seed = 141_000_000;
        rows[1].seed = 141_000_000;
        rows[2].seed = 141_000_006;
        let mut anchor = test_row(9, 0, "1", true);
        anchor.kind = "anchor".into();
        anchor.seed = 999;
        rows.push(anchor);
        assert_eq!(
            played_seed_window(&rows),
            Some((141_000_000, 141_000_006)),
            "legacy anchors are not screened games and do not widen the window"
        );
        assert_eq!(played_seed_window(&[]), None);
    }
}
