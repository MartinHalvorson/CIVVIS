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
//! (`Build`, `Batch`): `tools/gene_ledger.py` refuses a source whose gene set
//! does not match the code at the commit it names, and `--analyze` reports
//! actual against intended so a run that stopped early cannot read as a
//! finished screen.
//!
//! Usage:
//!   gene_screen [--games N] [--start-seed N] [--jobs N] [--out PATH]
//!               [--genes tag,tag,...] [--target-games N] [--append] [--quiet]
//!               [--p-on 0.5] [--p-default-on 0.75]
//!               PROBE ONLY, and a batch using any of them is not a ledger
//!               source: [--players N] [--turns N] [--width N] [--height N]
//!               [--city-states N] [--speed ID] [--map ID] [--victories a,b]
//!               [--stock-civs]
//!   gene_screen --analyze PATH [PATH ...] [--json OUT] [--interactions]
//!               [--top N] [--by-civ TAG]
//!   gene_screen --list
//!
//! ⭐ ONE SCREEN, and the bare defaults are it: six majors on 74x46 Continents
//! with nine city-states, Online speed to its own 250-turn clock, all six
//! victory lanes, every seat carrying its own drawn genome, civilizations
//! shuffled per map (`SCREEN_PLAYERS` and friends below). That is
//! Civilization VI's own six-player map row and the deployment shape, so the
//! ledger is read from the games the agent actually plays.
//! `gene_screen --games N --out rows.jsonl` is a screen; anything that moves
//! a leg of the profile is a probe, and `tools/gene_ledger.py` refuses it as
//! a source rather than mixing shapes.
//!
//! Files written by the earlier paired designs (every header before this one
//! says `foldover` or `prior`) still analyse: their rows are seats with a
//! genome and an outcome like any other, and the estimator here never needed
//! the pairing. Only the sampling changed.
use civvis::ai::{run_game, AdvancedAi, LiveTreatment};
use civvis::game::{Game, GameOptions};
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
/// this is a probe: `tools/gene_ledger.py` refuses it as a ledger source.
const SCREEN_PLAYERS: usize = 6;
const SCREEN_WIDTH: i32 = 74;
const SCREEN_HEIGHT: i32 = 46;
const SCREEN_CITY_STATES: usize = 9;
const SCREEN_MAP: MapScript = MapScript::Continents;

/// ⭐ THE DRAW (operator, 2026-08-23). Every screened gene is on with
/// probability one half — except a gene the deployment genome already ships
/// on, which is on three quarters of the time, so the batch plays mostly the
/// genome people actually get while every gene still has both arms well
/// populated. Each seat's genome is drawn independently of every other seat
/// and every other game; nothing is paired or complemented.
const P_ON: f64 = 0.5;
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
    /// On in the deployment genome: the ledger's `helps`, or — for a gene the
    /// ledger has not measured — the universe state. See `gene_ledger.rs`.
    default_on: bool,
    flip: fn(&mut AdvancedAi),
}

/// The deployment default for a tag: the ledger's say, else the universe.
fn ledger_default(tag: &str, universe_on: bool) -> bool {
    civvis::ai::ledger_default_on(tag).unwrap_or(universe_on)
}

/// Every gene this screen can vary, in the order the genome bits are written.
///
/// ⚠ Discovered from the repository's own tables, never listed by hand: a
/// treatment added to `ENGINE_REPAIR_TREATMENTS`, `PRODUCTION_TREATMENTS` or
/// `PRODUCTION_OPT_INS` reaches the genome without touching this file. An
/// engine-repair tag with no `LIVE_TREATMENTS` row is a panic, not a silent
/// omission — the elo tests already hold the two tables in step and this
/// binary trusts that contract loudly.
fn gene_table() -> Vec<Gene> {
    let mut genes = Vec::new();
    for repair in civvis::elo::ENGINE_REPAIR_TREATMENTS {
        let &(field, tag, disable): &LiveTreatment = civvis::ai::LIVE_TREATMENTS
            .iter()
            .find(|(_, row_tag, _)| row_tag == repair)
            .unwrap_or_else(|| {
                panic!(
                    "engine repair {repair} has no LIVE_TREATMENTS row, so it cannot be withheld"
                )
            });
        genes.push(Gene {
            field,
            tag,
            after_setup_on: true,
            stock_on: false,
            default_on: ledger_default(tag, true),
            flip: disable,
        });
    }
    for &(field, tag, disable) in civvis::ai::PRODUCTION_TREATMENTS {
        genes.push(Gene {
            field,
            tag,
            after_setup_on: true,
            stock_on: true,
            default_on: ledger_default(tag, true),
            flip: disable,
        });
    }
    for &(field, tag, enable) in civvis::ai::PRODUCTION_OPT_INS {
        genes.push(Gene {
            field,
            tag,
            after_setup_on: false,
            stock_on: false,
            default_on: ledger_default(tag, false),
            flip: enable,
        });
    }
    genes
}

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

fn is_zero_u8(value: &u8) -> bool {
    *value == 0
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
    #[serde(default)]
    settlers_captured: i64,
    #[serde(default)]
    builders_captured: i64,
    #[serde(default)]
    pillages: i64,
    /// Settlers counted as prizes at the raids' declarations.
    #[serde(default)]
    raid_settler_prizes: i64,
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
/// `tools/gene_ledger.py` re-derives the gene tags at the commit a source
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
/// `tools/gene_ledger.py::gene_set_fingerprint` computes the same string from
/// `ENGINE_REPAIR_TREATMENTS` in `src/elo.rs` and `PRODUCTION_TREATMENTS` plus
/// `PRODUCTION_OPT_INS` in `src/ai/advanced/treatments.rs` **at any commit**,
/// which is how a screen is checked against the code it claims to have played.
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
    /// ⭐ The binary that played these games. Absent in files written before
    /// 2026-08-23, which `tools/gene_ledger.py` grandfathers as history and
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

/// The on-probability of every gene in header order: `p_default_on` for a
/// screened gene the deployment genome ships on, `p_on` for any other screened
/// gene, and 0 or 1 for a gene held at its default.
///
/// A FAMILY is drawn as one level — off, or exactly one of its versions — so
/// its members' probabilities here are MARGINALS: the family is on with the
/// probability its deployment state says (`p_default_on` if any version ships
/// on, else `p_on`), shared equally among the screened versions. A version
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
        let share = family_p / candidates.len() as f64;
        for &i in &candidates {
            probabilities[i] = share;
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
/// one version uniformly at random — never two versions on one seat.
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
            let pick = (rng.f64() * candidates.len() as f64) as usize;
            genome[candidates[pick.min(candidates.len() - 1)]] = true;
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
}

/// Play one game in which EVERY major seat carries its own drawn genome, and
/// report one row per major. Minor and barbarian seats stay stock. The field
/// is the other drawn majors — effects are averaged over random opposing
/// genomes rather than measured against a fixed production field, which is
/// the point: a flag that only pays against untreated opponents is a flag
/// the mixed ecosystem does not have.
fn play_game(
    profile: &Profile,
    genes: &[Gene],
    game: usize,
    seed: u64,
    genomes: &[Vec<bool>],
) -> Vec<Row> {
    let started = Instant::now();
    let mut world = Game::new_with(GameOptions {
        speed: profile.speed.id().to_string(),
        map_script: profile.map,
        randomize_civs: profile.randomize_civs,
        victory_conditions: profile.victories,
        ..GameOptions::new(
            profile.players,
            profile.width,
            profile.height,
            seed,
            profile.turns,
            profile.city_states,
        )
    });
    let mut majors = Vec::new();
    let mut ais: Vec<AdvancedAi> = (0..world.players.len())
        .map(|pid| {
            if world.players[pid].is_minor || world.players[pid].is_barbarian {
                AdvancedAi::new()
            } else {
                majors.push(pid);
                seat_with_genome(genes, &genomes[majors.len() - 1])
            }
        })
        .collect();
    run_game(&mut world, &mut ais);
    let secs = started.elapsed().as_secs_f64();
    majors
        .iter()
        .enumerate()
        .map(|(index, &seat)| {
            row_for_seat(
                &world,
                game,
                seed,
                seat,
                genome_string(&genomes[index]),
                secs,
            )
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
        military: game.military_power(seat),
        civ: game.players[seat].civ.clone(),
        raid_wars: counter("raid_wars"),
        settlers_captured: counter("captured:settler"),
        builders_captured: counter("captured:builder"),
        pillages: counter("pillages"),
        raid_settler_prizes: counter("raid_prize:settler"),
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
            Some(fit) => (2.0 * fit[1].0, 2.0 * fit[1].1),
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
                .map(|(b, se)| (2.0 * b, 2.0 * se))
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

fn read_column(win_z: f64, share_z: f64, family_z: f64) -> String {
    let word = |z: f64| -> Option<&'static str> {
        if z.abs() >= family_z {
            Some(if z > 0.0 { "HELPS **" } else { "HURTS **" })
        } else if z.abs() >= 2.0 {
            Some(if z > 0.0 { "helps *" } else { "hurts *" })
        } else {
            None
        }
    };
    match (word(win_z), word(share_z)) {
        (None, None) => "~".to_string(),
        (Some(win), None) => win.to_string(),
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
        280.0 * median_se,
        280.0 * median_share_se,
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
         family on — the best one — and never two"
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
/// table prints, plus the profile. `tools/gene_ledger.py` reads this to
/// build `docs/gene_ledger.json` and the generated Rust table, so the
/// deployment genome is derived from the screens rather than typed in.
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
                "share_delta_pp": 100.0 * e.share_delta,
                "share_se_pp": 100.0 * e.share_se,
                "share_z": e.share_z(),
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
        // `tools/gene_ledger.py` refuses it as a source.
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
        "overall_win": estimates.overall_win,
        "overall_share": estimates.overall_share,
        "family_wise_z": family_z,
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
        && header.baseline == "best";
    if standard {
        "standard"
    } else {
        "legacy"
    }
}

/// One line naming the binary a batch was played by, printed before the first
/// game and again by `--analyze`.
///
/// An unstamped or dirty build is called out here rather than left for the
/// ledger to discover hours later: `tools/gene_ledger.py` refuses both, and
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
         [--target-games N] [--out PATH] [--append] [--quiet] [--p-on 0.5] [--p-default-on 0.75]\n       \
         (6 majors, 74x46 continents, 9 city-states, online/250, all six lanes, every seat its own \
         random genome, shuffled civs — the one shape the ledger accepts)\n       \
         probe only, NOT a ledger source: [--players N] [--turns N] [--width N] [--height N] \
         [--city-states N] [--speed ID] [--map ID] [--victories a,b,...] [--stock-civs]\n       \
         gene_screen --analyze PATH [PATH ...] [--json OUT] [--interactions] [--top N] [--by-civ TAG]\n       \
         gene_screen --list"
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
    // `tools/gene_ledger.py` refuses a source that does not match. The shape is
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
            target_seats: target_games * players,
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
    };
    println!(
        "gene screen: {games_to_play} games ({} seats, every seat its own genome, on at p={p_on} / {p_default_on} default-on) · {} of {} genes screened · {players}p {width}x{height} {} · {} · {turns} turns · {city_states} city-states · {} civs · seeds {start_seed}..{} · {jobs} jobs · rows → {out_path}",
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
    let played: Vec<Vec<Row>> = civvis::parallel::map_reporting(
        games_to_play,
        jobs,
        |game| {
            let seed = start_seed + game as u64;
            let genomes: Vec<Vec<bool>> = (0..players)
                .map(|seat| draw_genome(start_seed, game, players, seat, &probabilities, &families))
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
            military: 0.0,
            civ: String::new(),
            raid_wars: 0,
            settlers_captured: 0,
            builders_captured: 0,
            pillages: 0,
            raid_settler_prizes: 0,
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
            civvis::elo::ENGINE_REPAIR_TREATMENTS.len()
                + civvis::ai::PRODUCTION_TREATMENTS.len()
                + civvis::ai::PRODUCTION_OPT_INS.len()
        );
        let mut tags: Vec<&str> = genes.iter().map(|g| g.tag).collect();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), genes.len(), "a gene tag is repeated");
        // Firaxis-only flags are excluded by construction, not by luck —
        // unless a `PRODUCTION_OPT_INS` row names one on purpose. That table
        // is an author's statement that the flag acts on a native board
        // (`joint-tactics`: the bridge turns the joint search on, but
        // `advanced_joint_tactics` is production plus that flag and the
        // arena runs it every day); a repair reaches the genome only through
        // `ENGINE_REPAIR_TREATMENTS`, which still excludes every host-only tag.
        let opted_in: Vec<&str> = civvis::ai::PRODUCTION_OPT_INS
            .iter()
            .map(|(_, tag, _)| *tag)
            .collect();
        for gene in &genes {
            assert!(
                !civvis::elo::FIRAXIS_ONLY_TREATMENTS.contains(&gene.tag)
                    || opted_in.contains(&gene.tag),
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

    /// The harmful `governor-every-lane` composite is deliberately split in
    /// the controller. These two rows make that split a real genome choice:
    /// a seat can carry either established predicate while its sibling and
    /// the historical composite stay off.
    #[test]
    fn governor_halves_are_independent_opt_in_genes() {
        let genes = gene_table();
        let seat = |tag: &str| {
            let index = genes
                .iter()
                .position(|gene| gene.tag == tag)
                .unwrap_or_else(|| panic!("{tag} is a screenable gene"));
            let mut genome = vec![false; genes.len()];
            genome[index] = true;
            seat_with_genome(&genes, &genome)
        };

        let victory = seat("governor-victory-lanes");
        assert!(victory.governor_victory_lanes);
        assert!(!victory.governor_expansion_lane);
        assert!(!victory.governor_every_lane);

        let expansion = seat("governor-expansion-lane");
        assert!(!expansion.governor_victory_lanes);
        assert!(expansion.governor_expansion_lane);
        assert!(!expansion.governor_every_lane);
    }

    #[test]
    fn the_read_column_names_both_axes() {
        assert_eq!(read_column(0.5, -0.3, 3.33), "~");
        assert_eq!(read_column(2.4, 0.1, 3.33), "helps *");
        assert_eq!(read_column(-0.8, -7.2, 3.33), "share HURTS **");
        assert_eq!(read_column(2.1, 4.9, 3.33), "helps * · share HELPS **");
        assert_eq!(read_column(-3.5, -1.0, 3.33), "HURTS **");
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

    /// The family's marginals: the family probability shared among the
    /// screened versions; a version held on forces its siblings off.
    #[test]
    fn family_marginals_share_the_family_probability() {
        let genes = gene_table();
        let mut screened = vec![true; genes.len()];
        let families = vec![vec![0, 1, 2]];
        let p = on_probabilities(&genes, &screened, 0.5, 0.75, &families);
        let family_p = if genes[..3].iter().any(|g| g.default_on) {
            0.75
        } else {
            0.5
        };
        for i in 0..3 {
            assert!(
                (p[i] - family_p / 3.0).abs() < 1e-12,
                "{} {}",
                genes[i].tag,
                p[i]
            );
        }
        // Hold gene 1 at its default: on → siblings are forced off; off → the
        // other two share the family.
        screened[1] = false;
        let held = on_probabilities(&genes, &screened, 0.5, 0.75, &families);
        if genes[1].default_on {
            assert_eq!(held[1], 1.0);
            assert_eq!((held[0], held[2]), (0.0, 0.0));
        } else {
            assert_eq!(held[1], 0.0);
            let family_p = if genes[0].default_on || genes[2].default_on {
                0.75
            } else {
                0.5
            };
            assert!((held[0] - family_p / 2.0).abs() < 1e-12);
            assert!((held[2] - family_p / 2.0).abs() < 1e-12);
        }
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
    /// ⚠ THIS IS `tools/gene_ledger.py::_quoted` IN THE OTHER LANGUAGE, and
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

    /// The gene tags a reader gets from the source tables alone, in the order
    /// `gene_table()` builds them: every tag of `ENGINE_REPAIR_TREATMENTS`,
    /// then the `(field, tag, toggle)` rows of `PRODUCTION_TREATMENTS` and
    /// `PRODUCTION_OPT_INS`, whose tag is the second string of each row.
    fn tags_from_source_tables(root: &std::path::Path) -> Vec<String> {
        let elo = std::fs::read_to_string(root.join("src/elo.rs")).expect("src/elo.rs");
        let treatments =
            std::fs::read_to_string(root.join("src/ai/advanced/treatments.rs")).expect("tables");
        let mut tags = quoted(table_body(&elo, "ENGINE_REPAIR_TREATMENTS"));
        for name in ["PRODUCTION_TREATMENTS", "PRODUCTION_OPT_INS"] {
            tags.extend(
                quoted(table_body(&treatments, name))
                    .into_iter()
                    .skip(1)
                    .step_by(2),
            );
        }
        tags
    }

    /// ⭐ THE GUARD'S FOUNDATION. `tools/gene_ledger.py` recomputes a screen's
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
             tools/gene_ledger.py reads the tables"
        );
        assert!(compiled.len() > 50, "the tables scrape found too few genes");
    }

    /// The fingerprint is the tags, newline-terminated, hashed — the exact
    /// string `tools/gene_ledger.py::gene_set_fingerprint` builds. Pinned on a
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
                flip: |_| {},
            },
            Gene {
                field: "b",
                tag: "b",
                after_setup_on: false,
                stock_on: false,
                default_on: false,
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
