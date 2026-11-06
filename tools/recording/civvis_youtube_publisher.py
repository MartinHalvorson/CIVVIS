#!/usr/bin/env python3
"""Turn CIVVIS win clips into YouTube masters on the Desktop.

Operator 2026-10-14: "record domination victories and post youtube quality
videos saved to desktop/. post videos up to the latest 3 wins per civ per
victory type per level." The operator reviews and approves every video before
anything is posted to YouTube; this script never uploads.

One pass (the clip keeper runs it as a child every few minutes, so it inherits
Terminal's Desktop access; a plain launchd job can write to the Desktop but not
list or read it):

1. Each won game in the keeper's journal (clips.jsonl) whose raw clip still sits
   where the keeper saved it becomes a master: civvis-youtube-master renders a
   3840x2160 60 fps HEVC file with a title band, saved as
   ``~/Desktop/lv <N> <Civ> <Victory> <YYYY-MM-DD>.mp4`` plus a ``.txt`` with a
   suggested YouTube title and description. The raw clip then moves to
   ``clips/raw-wins/``. A raw clip the operator moved or deleted is left alone.
   One master per pass (a 20-minute game renders in about 12 minutes).
2. Per (level, civ, victory type) the newest five masters stay on the
   Desktop; older ones MOVE (never deleted) to ``~/Desktop/Old Recordings/``.
3. Full-turn coverage, clean recording segments, real-time duration and 4K/60
   output are verified before a master is saved. Spans where the live game was
   held (the control mod's operator_paused..operator_resumed: the recorder's
   off-screen pause or the HUD Pause button) are cut from the master
   (civvis_video_goal.HELD_RECIPE); everything kept plays at 1x. Five verified level-six
   domination masters on the Desktop unlock level seven at a game boundary.

usage: civvis_youtube_publisher.py [--dry-run]
"""
import fcntl
import json
import os
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civvis_clip_keeper as K  # noqa: E402  (naming helpers and paths)
import civvis_video_goal as video_goal
import civvis_video_story as video_story
import civvis_pericles_ladder as pericles_ladder

HOME = Path.home()
MASTER_BIN = Path(os.environ.get("CIVVIS_YT_MASTER", HOME / ".local/bin/civvis-youtube-master"))
YT_JOURNAL = K.CLIPS / "youtube.jsonl"
YT_STAGING = K.CLIPS / "youtube-staging"
RAW_ARCHIVE = K.CLIPS / "raw-wins"
OLD = K.DESKTOP / "Old Recordings"
LOCK = K.CLIPS / "youtube.lock"
LOGFILE = HOME / "Library/Logs/civvis-youtube-publisher.log"
PLAY_LOGS = HOME / "civvis-climb-logs"
KEEP = 5
# A source the full-game gate refused is not re-gated while it is unchanged (same size and mtime)
# for REFUSAL_TTL: each re-gate decodes the whole 4K clip beside the live game, and three refused
# sources re-gated every pass held a new win behind them (10-30).
GATE_REFUSALS = K.CLIPS / "gate-refusals.json"
# Raw win clips leave the Desktop as soon as a pass sees them, and are rendered from here: only
# a finished master with its title, settings line, scoreboard and call-outs belongs on the
# Desktop (operator 2026-10-31: "please make sure videos are marked with our markup stuff -
# narration, game settings, etc"). The keeper still saves the raw to the Desktop first.
RAW_PENDING = K.CLIPS / "raw-pending"


def pending_path(gid, clip):
    return RAW_PENDING / f"{gid} {Path(clip).name}"


def sweep_raw_from_desktop(dry_run):
    """Move every unpublished raw win clip off the Desktop into RAW_PENDING."""
    done = {r["game_id"] for r in read_jsonl(YT_JOURNAL) if r.get("master")}
    # Newest row first: the keeper reuses a label ("lv 5 Greece Science.mp4"), so a raw on the Desktop belongs to the
    # NEWEST row naming it. Oldest-first filed 11-01 110755Z's raw under 10-30 051958Z, whose gate then refused it.
    for row in reversed(read_jsonl(K.JOURNAL)):
        gid, clip = row.get("game_id"), row.get("clip")
        if not (gid and clip and (row.get("verdict") or {}).get("won")) or gid in done:
            continue
        raw = Path(clip)
        if raw.parent == K.DESKTOP and raw.exists() and not dry_run:
            RAW_PENDING.mkdir(parents=True, exist_ok=True)
            shutil.move(str(raw), str(pending_path(gid, clip)))
            log(f"{gid}: raw clip moved off the Desktop to {pending_path(gid, clip)}")
REFUSAL_TTL = 6 * 3600

LEVEL_NAMES = {1: "Settler", 2: "Chieftain", 3: "Warlord", 4: "Prince", 5: "King",
               6: "Emperor", 7: "Immortal", 8: "Deity"}
LEADER_NAMES = {"LEADER_SIMON_BOLIVAR": "Simón Bolívar", "LEADER_PERICLES": "Pericles", "LEADER_GORGO": "Gorgo"}
VICTORY_PHRASE = {"Domination": "wins by Domination", "Score": "wins on Score",
                  "Culture": "wins a Culture victory", "Diplomatic": "wins a Diplomatic victory"}
MAP_SIZES = {"MAPSIZE_DUEL": "Duel", "MAPSIZE_TINY": "Tiny", "MAPSIZE_SMALL": "Small",
             "MAPSIZE_STANDARD": "Standard", "MAPSIZE_LARGE": "Large", "MAPSIZE_HUGE": "Huge"}


def log(msg):
    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    with open(LOGFILE, "a") as fh:
        fh.write(f"{stamp} [publisher] {msg}\n")


def read_jsonl(path):
    rows = []
    if path.exists():
        for line in path.read_text(errors="replace").splitlines():
            try:
                rows.append(json.loads(line))
            except ValueError:
                pass
    return rows


def win_time(gid):
    """The game's start stamp from its id (civvis-YYYYMMDDTHHMMSSZ)."""
    m = re.search(r"(\d{8}T\d{6}Z)", gid)
    return datetime.strptime(m.group(1), "%Y%m%dT%H%M%SZ").replace(tzinfo=timezone.utc) if m else None


def game_facts(gid):
    """Victory turn, map and player count from the run's play log(s)."""
    facts = {}
    for log_path in sorted(PLAY_LOGS.glob(f"{gid}*-play.log")):
        text = log_path.read_text(errors="replace")
        m = re.search(r"seat 0 \S+ difficulty=\S+ size=(\S+) .*? players=(\d+)", text)
        if m:
            facts["map"] = MAP_SIZES.get(m.group(1), m.group(1))
            facts["players"] = int(m.group(2))
        i = text.find("\n{")
        if i >= 0:
            try:
                summary = json.loads(text[i:text.rfind("}") + 1])
                out = summary.get("outcome")
                if isinstance(out, dict) and out.get("turn"):
                    facts["turn"] = out["turn"]
                # The seat event is the game's own report; the top-level key is absent on live runs,
                # so every master read "Pangaea" whatever was played (10-31: Continents shown as Pangaea).
                game_map = summary.get("map") or (summary.get("seat") or {}).get("map")
                if game_map:
                    facts["map_script"] = str(game_map).replace(".lua", "").replace("_", " ")
            except ValueError:
                pass
    return facts


def clip_window(gid):
    """(start, duration) in seconds of the stitched raw clip covering exactly the
    turn-1 board through the final turn (the victory event), or None.

    Operator 2026-10-14: "from turn 1 to the final turn but not more". The keeper's
    concat cuts each segment's head to 3 s before its run's first state event (the
    turn-1 board) and logs "setup N s trimmed"; the play log records the victory
    event's epoch `t`. Segments come from the keeper's state.json.
    """
    try:
        state = json.loads(K.STATE.read_text())
    except (OSError, ValueError):
        return None
    g = (state.get("games") or {}).get(gid) or {}
    segs = sorted(g.get("segments") or [], key=lambda s: s.get("started") or 0)
    if not segs:
        return None
    # The setup each segment lost to the concat's trim, from the recorder log.
    trims = {}
    try:
        for line in (K.CLIPS / "recorder.log").read_text(errors="replace").splitlines():
            m = re.search(r"concat: \+ (\S+\.mp4) \(([\d.]+) min(?:, setup (\d+) s trimmed)?", line)
            if m:
                trims[m.group(1)] = float(m.group(3) or 0)
    except OSError:
        pass
    victory_t = None
    for log_path in sorted(PLAY_LOGS.glob(f"{gid}*-play.log")):
        for line in log_path.read_text(errors="replace").splitlines():
            if line.startswith("[victory] "):
                try:
                    victory_t = float(json.loads(line[len("[victory] "):]).get("t"))
                except (ValueError, TypeError):
                    pass
    if victory_t is None:
        return None
    offset = 0.0
    for i, seg in enumerate(segs):
        name = Path(seg.get("path", "")).name
        trim = trims.get(name, 0.0)
        started, ended = seg.get("started"), seg.get("ended")
        if started is None or ended is None:
            return None
        if i == len(segs) - 1 or victory_t <= ended:
            end = offset + (victory_t - started - trim) + 7.0
            first_trim = trims.get(Path(segs[0].get("path", "")).name, 0.0)
            start = 3.0 if first_trim > 0 else 0.0
            if end <= start + 60:
                return None
            return (start, end - start)
        offset += max(0.0, (ended - started) - trim)
    return None


def _city_label(name):
    """'LOC_CITY_NAME_ADDIS_ABABA' -> 'Addis Ababa'; a localized name passes through."""
    name = str(name or "")
    if name.upper().startswith("LOC_"):
        name = re.sub(r"^LOC_CITY_(NAME_)?", "", name, flags=re.I).replace("_", " ").title()
    return name


def game_story(gid):
    """Plain-language timeline from the run's state frames: rivals, wars,
    captures by rival (capitals marked), eliminations and the final tally.
    Uses the last state frame of each turn; [] when the run has none."""
    path = K.CONTROL / gid / "events.jsonl"
    if not path.exists():
        return []
    frames = {}
    try:
        with open(path, errors="replace") as fh:
            for line in fh:
                if '"kind": "state"' not in line:
                    continue
                try:
                    d = json.loads(line)
                except ValueError:
                    continue
                if isinstance(d.get("turn"), int):
                    frames[d["turn"]] = d  # the last frame of each turn wins
    except OSError:
        return []
    if not frames:
        return []
    me = None
    rivals, war_from, eliminated, capitals, last_seen_with_cities = {}, {}, {}, {}, {}
    captured, ours_first, lost = [], set(), set()
    seen_ours = set()
    for turn in sorted(frames):
        d = frames[turn]
        me = d.get("local_player", 0) if me is None else me
        now = set()
        for r in d.get("rivals") or []:
            pid = r.get("player")
            if pid is None or not r.get("civ") or "CITY_STATE" in str(r.get("civ")):
                continue
            rivals.setdefault(pid, K.civ_name(r.get("civ"), r.get("leader")))
            cities = r.get("cities") or []
            if cities:
                now.add(pid)
            for c in cities:
                # The ORIGINAL capital only: the first capital we see for this rival.
                if c.get("capital") and pid not in capitals.values():
                    capitals[_city_label(c.get("name"))] = pid
            if cities:
                last_seen_with_cities[pid] = turn
            if r.get("at_war") and pid not in war_from:
                war_from[pid] = turn
        for c in d.get("cities") or []:
            name = _city_label(c.get("name"))
            if c.get("original_owner") == me:
                ours_first.add(name)
            elif name not in seen_ours and c.get("original_owner") is not None:
                captured.append((turn, name, c.get("original_owner")))
            seen_ours.add(name)
        current = {_city_label(c.get("name")) for c in d.get("cities") or []}
        lost |= {n for n in ours_first if n not in current}
    last = frames[max(frames)]
    # Eliminated: had cities once, has none (or is gone) in the final frame.
    final_with_cities = {r.get("player") for r in last.get("rivals") or [] if r.get("cities")}
    for pid, seen in last_seen_with_cities.items():
        if pid not in final_with_cities:
            eliminated[pid] = seen + 1
    lines = []
    names = [rivals[p] for p in sorted(rivals)]
    if names:
        lines.append("Rivals: " + (", ".join(names[:-1]) + " and " + names[-1] if len(names) > 1 else names[0]) + ".")
    for pid in sorted(war_from, key=war_from.get):
        taken = [(t, n) for t, n, o in captured if o == pid]
        bits = [f"{n} (capital, turn {t})" if capitals.get(n) == pid else f"{n} (turn {t})" for t, n in taken]
        line = f"Turn {war_from[pid]}: war with {rivals[pid]}."
        if bits:
            line += " Captured " + ", ".join(bits) + "."
        if pid in eliminated:
            line += f" {rivals[pid]} eliminated by turn {eliminated[pid]}."
        lines.append(line)
    others = [(t, n) for t, n, o in captured if o not in rivals]
    if others:
        lines.append("City-states taken: " + ", ".join(f"{n} (turn {t})" for t, n in others) + ".")
    kept = len(last.get("cities") or [])
    lines.append(f"Final: {kept} cities, {len(captured)} captured, {len(lost)} lost.")
    return lines


def leader_label(leader_id):
    """Display name for a leader id. Random leaders (operator 2026-10-31) reach far past
    LEADER_NAMES, so unknown ids fall back to the id itself: LEADER_HOJO -> "Hojo"."""
    leader_id = leader_id or ""
    if leader_id in LEADER_NAMES:
        return LEADER_NAMES[leader_id]
    words = leader_id.removeprefix("LEADER_").replace("_", " ").strip()
    return " ".join(w if w in ("II", "III", "VII") else w.capitalize() for w in words.split())


def texts(verdict, gid, story=None):
    level = K.level_of(verdict.get("difficulty"))
    civ = K.civ_name(verdict.get("civ"), verdict.get("leader"))
    victory = K.victory_name(verdict.get("victory_type"))
    leader = leader_label(verdict.get("leader"))
    facts = game_facts(gid)
    when = win_time(gid)
    date = when.strftime("%Y-%m-%d") if when else "undated"
    title = "CIVVIS AI " + VICTORY_PHRASE.get(victory, f"wins a {victory} victory")
    parts = [f"{civ} ({leader})" if leader else civ]
    if level:
        parts.append(f"{LEVEL_NAMES.get(level, 'Level')} (Lv {level})")
    if facts.get("players") and facts.get("map"):
        parts.append(f"{facts['players']}-player {facts['map']} {facts.get('map_script', 'Pangaea')}")
    if facts.get("turn"):
        parts.append(f"turn {facts['turn']}")
    parts.append(date)
    subtitle = " · ".join(parts)
    yt_title = (f"CIVVIS AI: {civ}" + (f" ({leader})" if leader else "")
                + f" {victory} Victory on {LEVEL_NAMES.get(level, '?')} (Lv {level})"
                + (f", turn {facts['turn']}" if facts.get("turn") else ""))
    story = story or {}
    story_lines = story.get("story") or []
    description = "\n".join([
        yt_title,
        "",
        f"The CIVVIS AI plays {civ}" + (f" ({leader})" if leader else "") + f" in Civilization VI at "
        f"{LEVEL_NAMES.get(level, '?')} difficulty and wins a {victory} victory"
        + (f" on turn {facts['turn']}." if facts.get("turn") else "."),
        "Left: the live Civilization VI game. Right: CIVVIS, the AI's own view of the same game.",
        "The bar along the bottom tracks the turn, the era and the race to victory; gold captions mark"
        " the turning points.",
        "",
        *((["Chapters:"] + story["chapters"] + [""]) if story.get("chapters") else []),
        *((["How the game went:"] + [f"- {line}" for line in war_story[:-1]]
           + ([f"- Turn {facts['turn']}: {victory} victory."] if facts.get("turn") else [])
           + [f"- {line}" for line in (story_lines or [])]
           + [f"- {war_story[-1]}", ""])
          if (war_story := game_story(gid)) else []),
        f"Game: {gid} · recorded {date}",
    ])
    banner = f"{victory.upper()} VICTORY" + (f" · TURN {facts['turn']}" if facts.get("turn") else "")
    return {"label": f"{K.clip_label(verdict)} {date}", "title": title, "subtitle": subtitle,
            "banner": banner,
            "yt_title": yt_title, "description": description, "level": level, "civ": civ,
            "victory": victory, "when": when.isoformat() if when else None}


def clock(seconds):
    return f"{int(seconds // 60)}:{seconds % 60:04.1f}"


def grace_note(evidence):
    """Review notes (not for YouTube) for what the gate let through: the CIVVIS picture joining
    late at the opening (civvis_video_goal.OPENING_GRACE_SECONDS) and Civ VI's own black movie
    screens (GAME_DARK_MAX); '' otherwise."""
    source = (evidence.get("dual_panels") or {}).get("source") or {}
    notes = []
    grace = source.get("opening_grace")
    if grace:
        turns = grace.get("civ_vi_only_turns") or []
        notes.append(f"the CIVVIS picture joins at {clock(grace.get('joins_at_seconds', 0))}; turn(s) "
                     f"{', '.join(turns)} show the Civ VI board only. The CIVVIS viewer window opened after "
                     "the game had started (opening grace in the quality gate). Nothing was padded or back-filled.")
    start = evidence.get("window_start_seconds", 0)
    for span in source.get("game_dark_spans") or []:
        raw_at = start + span["from_seconds"]  # spans are window seconds; held-game cuts shorten the master
        at = span["from_seconds"] - sum(b - a for a, b in evidence.get("held_cuts_seconds") or [] if b <= raw_at)
        notes.append(f"at about {clock(at)} the Civ VI picture is black for ~{span.get('dark_samples', 0)} s: "
                     f"the game's own {span.get('screen')} screen (the capture ran on and the CIVVIS panel "
                     "stays live). Nothing was cut or replaced.")
    return "".join(f"\nREVIEW NOTE (not for YouTube): {note}\n" for note in notes)


def held_cut_args(evidence):
    """--cut A:B (raw-clip seconds) for each span the quality receipt cut as a held game."""
    return [arg for a, b in evidence.get("held_cuts_seconds") or [] for arg in ("--cut", f"{a:.3f}:{b:.3f}")]


def journal(entry):
    with open(YT_JOURNAL, "a") as fh:
        fh.write(json.dumps(dict(entry, utc=datetime.now(timezone.utc).isoformat())) + "\n")


def publish_one(dry_run):
    done = {r["game_id"] for r in read_jsonl(YT_JOURNAL) if r.get("master")}
    try:
        refusals = json.loads(GATE_REFUSALS.read_text())
    except (OSError, ValueError):
        refusals = {}
    now = datetime.now(timezone.utc).timestamp()
    for row in reversed(read_jsonl(K.JOURNAL)):  # newest win first
        gid, clip, verdict = row.get("game_id"), row.get("clip"), row.get("verdict") or {}
        if not (gid and clip and verdict.get("won")) or gid in done:
            continue
        raw = Path(clip)
        if not raw.exists():
            raw = pending_path(gid, clip)
        if not raw.exists():
            continue  # the operator moved or deleted it: leave their choice alone
        st = raw.stat()
        key = f"{gid}|{st.st_size}|{int(st.st_mtime)}"
        if now - (refusals.get(key) or {}).get("at", 0) < REFUSAL_TTL:
            continue
        t = texts(verdict, gid)
        window = clip_window(gid)
        try:
            evidence = video_goal.source_evidence(gid, row, raw, window)
        except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
            log(f"{gid}: full-game quality gate: {error}; preserving the source")
            if not dry_run:
                refusals[key] = {"at": now, "error": str(error)[:300]}
                video_goal.atomic_json(GATE_REFUSALS, refusals)
            continue
        grace = ((evidence.get("dual_panels") or {}).get("source") or {}).get("opening_grace")
        if grace:
            log(f"{gid}: quality gate OPENING GRACE: the CIVVIS picture joins {grace['joins_at_seconds']:.1f}s into "
                f"the window; Civ VI board only for turn(s) {grace['civ_vi_only_turns']} (review note added)")
        for span in ((evidence.get("dual_panels") or {}).get("source") or {}).get("game_dark_spans") or []:
            log(f"{gid}: quality gate GAME DARK SCREEN: Civ VI picture black {span['dark_samples']} s from "
                f"{span['from_seconds']:.1f}s into the window, the game's own {span['screen']}; capture ran on "
                "(review note added)")
        hud = None
        try:
            story = video_story.build(gid, t["victory"], t["civ"], evidence.get("turn_offsets_seconds"),
                                      evidence.get("final_turn"), evidence.get("expected_seconds") or 0)
        except Exception as error:  # the story is polish: a failure renders the plain master
            log(f"{gid}: on-screen story failed ({error!r}); rendering without it")
            story = None
        if story:
            final_item = story["items"][-1] if story["items"] else {}
            if final_item.get("confirmed_result"):
                import victory_state
                _, _, events = video_story._load(gid)
                outcome = next(e for e in reversed(events) if victory_state.local_victory(e))
                evidence["terminal_presentation"] = dict(kind="confirmed_victory_card",
                                                         start_seconds=final_item["t"], outcome=outcome)
            t = texts(verdict, gid, story)
            YT_STAGING.mkdir(parents=True, exist_ok=True)
            hud = YT_STAGING / f"{gid}.hud.json"
            hud.write_text(json.dumps({"items": story["items"], "callouts": story["callouts"]}))
            log(f"{gid}: story: {len(story['items'])} scoreboard turns, {len(story['callouts'])} call-outs, "
                f"{len(story['chapters'])} chapters")
        log(f"{gid}: rendering a master of {raw.name} as '{t['label']}'")
        if dry_run:
            print(json.dumps(t, indent=2))
            return True
        YT_STAGING.mkdir(parents=True, exist_ok=True)
        tmp = YT_STAGING / f"{gid}.mp4"
        cmd = ["/usr/bin/nice", "-n", "15", str(MASTER_BIN), "--in", str(raw), "--out", str(tmp),
               "--title", t["title"], "--subtitle", t["subtitle"], "--banner", t["banner"]]
        cmd += ["--start", f"{window[0]:.2f}", "--end", f"{window[0] + window[1]:.2f}"]
        cmd += held_cut_args(evidence)
        if hud:
            cmd += ["--hud", str(hud)]
        held = evidence.get("held_seconds")
        log(f"{gid}: window {window[0]:.1f}s + {window[1]:.1f}s (every turn, 1× speed"
            + (f"; {held:.1f}s of held game cut in {len(evidence['held_cuts_seconds'])} place(s))" if held else ")"))
        with open(LOGFILE, "a") as out:
            rc = subprocess.call(cmd, stdout=out, stderr=out)
        if rc != 0 or not tmp.exists():
            log(f"{gid}: render FAILED rc={rc}; the raw clip stays where it is")
            journal({"game_id": gid, "failed": rc, "raw": str(raw)})
            return True
        try:
            quality = video_goal.validate_master(tmp, evidence)
        except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
            log(f"{gid}: rendered master failed quality validation: {error}")
            journal({"game_id": gid, "quality_failed": str(error), "raw": str(raw)})
            return True
        dest = K.free_name(K.DESKTOP, t["label"])
        shutil.move(str(tmp), str(dest))
        video_goal.atomic_json(dest.with_suffix(".quality.json"), quality)
        dest.with_suffix(".txt").write_text(
            f"TITLE\n{t['yt_title']}\n\nDESCRIPTION\n{t['description']}\n\n"
            "STATUS: awaiting operator review (not posted)\n" + grace_note(quality))
        RAW_ARCHIVE.mkdir(parents=True, exist_ok=True)
        archived = RAW_ARCHIVE / f"{gid} {raw.name}"
        shutil.move(str(raw), str(archived))
        log(f"{gid}: saved {dest.name} ({dest.stat().st_size / 1e9:.2f} GB); raw → {archived}")
        journal({"game_id": gid, "master": str(dest), "raw_archived": str(archived),
                 "level": t["level"], "civ": t["civ"], "victory": t["victory"], "when": t["when"],
                 "yt_title": t["yt_title"], "quality": quality})
        K.notify("CIVVIS YouTube master ready for review", dest.name)
        return True
    return False


def retire_old(dry_run):
    """Keep five masters per group without deleting older recordings."""
    groups = {}
    for r in read_jsonl(YT_JOURNAL):
        if r.get("master") and not r.get("retired_to") and Path(r["master"]).exists():
            groups.setdefault((r.get("level"), r.get("civ"), r.get("victory")), []).append(r)
    retired = {r["game_id"] for r in read_jsonl(YT_JOURNAL) if r.get("retired_to")}
    for key, rows in groups.items():
        rows = [r for r in rows if r["game_id"] not in retired]
        rows.sort(key=lambda r: r.get("when") or "", reverse=True)
        for r in rows[KEEP:]:
            src = Path(r["master"])
            log(f"{r['game_id']}: {key} has more than {KEEP} masters; retiring {src.name}")
            if dry_run:
                continue
            OLD.mkdir(parents=True, exist_ok=True)
            dest = K.free_name(OLD, src.stem)
            shutil.move(str(src), str(dest))
            side = src.with_suffix(".txt")
            if side.exists():
                shutil.move(str(side), str(dest.with_suffix(".txt")))
            receipt = src.with_suffix(".quality.json")
            if receipt.exists():
                shutil.move(str(receipt), str(dest.with_suffix(".quality.json")))
            journal({"game_id": r["game_id"], "retired_to": str(dest)})


def main():
    dry_run = "--dry-run" in sys.argv
    K.CLIPS.mkdir(parents=True, exist_ok=True)
    lock_fh = open(LOCK, "a+")
    try:
        fcntl.flock(lock_fh, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except OSError:
        return 0  # a render is still running
    if not MASTER_BIN.exists():
        log(f"{MASTER_BIN} missing; nothing rendered")
        return 1
    video_goal.reconcile(read_jsonl(YT_JOURNAL), dry_run=dry_run)
    try:
        sweep_raw_from_desktop(dry_run)
    except OSError as error:
        log(f"raw sweep: {error!r}")
    if "--reconcile-only" not in sys.argv:
        publish_one(dry_run)
    retire_old(dry_run)
    video_goal.reconcile(read_jsonl(YT_JOURNAL), dry_run=dry_run)
    try:
        pericles_ladder.reconcile(read_jsonl(YT_JOURNAL), dry_run=dry_run)
    except Exception as error:  # the ladder must never stop masters from being saved
        log(f"pericles ladder: {error!r}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
