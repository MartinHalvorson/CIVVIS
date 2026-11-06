"""Verify complete real-time victory masters before advancing the live difficulty."""
import bisect
import fcntl
import json
import os
import re
import subprocess
from datetime import datetime, timezone
from pathlib import Path

import civvis_clip_keeper as K


GOAL = K.HOME / ".civvis-domination-video-goal.json"
POLICY = K.HOME / ".civvis-verification-policy"
STATUS = K.CLIPS / "domination-video-goal.json"
QUALITY_SCHEMA = "civvis_full_game_video/v2"
PANEL_PROBE = K.HOME / ".local/bin/civvis-video-panels"
PANEL_SCHEMA = "civvis_dual_panel_picture/v1"
# Operator 2026-10-30: the Lv 7 Diplomatic master (204643Z) sat 2 min 47 s on one frame
# (the Animal Husbandry screen, turn 6): the recorder could not film a slot, held its last
# frames and paused the live game. A held game plays nothing, so the master skips it: every
# span between the control mod's operator_paused and operator_resumed events (the recorder's
# off-screen pause and the HUD Pause button alike) is cut, keeping HOLD_MARGIN of the held
# board at each edge. Nothing near a turn instant is ever cut, so every turn keeps its
# picture, and the kept footage still plays at 1x.
HELD_RECIPE = "unscaled_time_ranges_minus_held_game"
HOLD_MARGIN = 0.5
HOLD_MIN_CUT = 2.0
TURN_GUARD = (0.5, 2.0)  # seconds before / after a turn instant that are never cut
# 11-02 (civvis-20261102T063640Z, King Science t221): the capture-viewer guard can (re)open the
# CIVVIS viewer window only once the :8610 mirror answers, i.e. at the turn-1 board. After the lane
# had been off and the window was gone, the CIVVIS picture joined 5.5 s into the window. Turns last
# ~1 s there, so turns 1-4 were filmed with only the Civ VI board. One narrow exception, opening
# only: the CIVVIS panel may join late if it joins within OPENING_GRACE_SECONDS of the window start,
# only turns 1..OPENING_GRACE_TURNS lack it, the Civ VI board is filmed at every one of those turns
# and at every opening sample, and both pictures cover >= OPENING_GRACE_SHARE of the whole window.
# Those turns stay as filmed (Civ VI board, black CIVVIS slot): nothing is padded or back-filled.
# The receipt records it under "opening_grace"; the publisher writes a review note. 0 disables.
OPENING_GRACE_SECONDS = 10.0
OPENING_GRACE_TURNS = 5
OPENING_GRACE_SHARE = 0.99
# 11-02 (civvis-20261102T084807Z, King Culture t244): Civ VI drew its own window black for 2.9 s
# when a RockBandMoviePopup (the rock-band concert movie) opened over the diplomacy screen; the
# harness closed it and the map came back. The capture ran throughout: the recorder logged nothing
# for the Civ slot and the CIVVIS panel stayed live. So the Firaxis picture was present, and the
# game was showing black. A Firaxis-only dark run is excused as the game's own screen only when:
#   * it starts within GAME_DARK_SPAN seconds after a full-screen movie popup the game reported
#     (event_lock/autoclose screen ending in "MoviePopup");
#   * it lasts at most GAME_DARK_MAX one-second samples;
#   * the CIVVIS panel is visible at every one of those samples;
#   * the recorder logged no Civ-slot capture event (civ:/away:/stream died/stopping) around it.
# Turns still need a dual-panel sample, and the share thresholds are unchanged. Nothing is cut or
# replaced. The receipt lists them under "game_dark_spans"; the publisher writes a review note.
GAME_DARK_MAX = 4
GAME_DARK_SPAN = 6.0
GAME_DARK_SCREEN_SUFFIX = "MoviePopup"
CAPTURE_EVENT = re.compile(r"^(\d{4}-\d\d-\d\dT[\d:.]+Z) \[recorder \d+\] "
                           r"(?:civ: |away: |a stream died|stopping: )")


class QualityError(ValueError):
    pass


def read_json(path):
    return json.loads(Path(path).read_text())


def atomic_json(path, value):
    path = Path(path)
    temporary = path.with_name(path.name + f".{os.getpid()}.tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    os.replace(temporary, path)


def media_info(path):
    result = subprocess.run([str(K.RECORDER), "info", str(Path(path).resolve())],
                            capture_output=True, text=True, timeout=60, check=True)
    match = re.search(r"duration=([\d.]+)s size=(\d+)x(\d+) fps=([\d.]+) codec=(\w+)",
                      result.stdout)
    if not match:
        raise QualityError(f"unreadable video: {path}")
    return dict(duration_seconds=float(match[1]), width=int(match[2]),
                height=int(match[3]), fps=float(match[4]), codec=match[5])


def scan_panels(path, times):
    result = subprocess.run(["/usr/bin/nice", "-n", "15", str(PANEL_PROBE),
                             str(Path(path).resolve())], input=json.dumps({"times": times}),
                            capture_output=True, text=True, timeout=1200, check=True)
    return json.loads(result.stdout)["samples"]


def panel_evidence(path, start, duration, turn_offsets, game_dark=None):
    """game_dark: [[a, b, screen], ...] seconds from `start` after a game movie popup in which a
    short Firaxis-only dark run is the game's own screen (see GAME_DARK_MAX)."""
    end = start + duration - 0.1
    if duration < 1 or not turn_offsets:
        raise QualityError("no game-turn picture sample plan")
    regular = [round(min(start + 0.5 + index, end), 3)
               for index in range(max(1, int(duration)))]
    turn_samples = {turn: sorted({round(min(end, max(start, start + offset + delay)), 3)
                                  for delay in (0.5, 1.0, 1.5)})
                    for turn, offset in turn_offsets.items()}
    requested = sorted(set(regular).union(*(set(times) for times in turn_samples.values())))
    samples = {round(sample["time"], 3): sample for sample in scan_panels(path, requested)}

    def seen(time, panel):
        return samples.get(time, {}).get(panel) is True

    def longest_gap(filled):
        longest = consecutive = 0
        for visible in filled:
            consecutive = 0 if visible else consecutive + 1
            longest = max(longest, consecutive)
        return longest

    shares = {}
    blackouts = {}
    for panel in ("left", "right"):
        filled = [seen(time, panel) for time in regular]
        shares[panel] = sum(filled) / len(filled)
        blackouts[panel] = longest_gap(filled)
    missing_turns = [turn for turn, times in turn_samples.items()
                     if not any(seen(time, "left") and seen(time, "right") for time in times)]
    dark_spans = []
    if game_dark and GAME_DARK_MAX > 0:
        # Firaxis-only dark runs that the game's own movie screen explains are not a lost picture.
        runs, run = [], []
        for time in regular + [None]:
            if time is not None and not seen(time, "left"):
                run.append(time)
            elif run:
                runs.append(run)
                run = []
        unexplained = []
        for run in runs:
            screen = next((name for a, b, name in game_dark
                           if all(start + a <= time <= start + b for time in run)), None)
            if (len(run) > 1 and screen and len(run) <= GAME_DARK_MAX
                    and all(seen(time, "right") for time in run)):
                dark_spans.append({"from_seconds": round(run[0] - start, 3),
                                   "to_seconds": round(run[-1] - start + 1, 3),
                                   "dark_samples": len(run), "screen": screen})
            else:
                unexplained.append(len(run))
        if dark_spans:
            blackouts["left"] = max(unexplained, default=0)
    grace = opening_grace(requested, regular, turn_samples, missing_turns, shares, start, seen)
    if grace:
        # The CIVVIS panel's leading stretch is the grace; any later gap still counts.
        blackouts["right"] = longest_gap([seen(time, "right") for time in regular
                                          if time >= start + grace["joins_at_seconds"]])
        missing_turns = []
    if (min(shares.values()) < 0.98 or max(blackouts.values()) > 2 or missing_turns):
        raise QualityError("both pictures must be present: "
                           f"Firaxis {shares['left']:.1%}, CIVVIS {shares['right']:.1%}; "
                           f"missing dual-panel turns {missing_turns[:10]}"
                           + (f"; longest blackout {max(blackouts.values())} s"
                              if max(blackouts.values()) > 2 else ""))
    evidence = {"schema": PANEL_SCHEMA, "both_visible": True, "samples": len(requested),
                "left_visible_share": shares["left"], "right_visible_share": shares["right"],
                "max_blackout_seconds": max(blackouts.values()), "turn_samples": len(turn_samples),
                "missing_turns": []}
    if grace:
        evidence["opening_grace"] = grace
    if dark_spans:
        evidence["game_dark_spans"] = dark_spans
    return evidence


def opening_grace(requested, regular, turn_samples, missing_turns, shares, start, seen):
    """The OPENING_GRACE record when the only gap is the CIVVIS picture joining late at the
    opening (see OPENING_GRACE_SECONDS), else None."""
    if not missing_turns or OPENING_GRACE_SECONDS <= 0:
        return None
    joins = next((time for time in requested if seen(time, "right")), None)
    if joins is None or joins - start > OPENING_GRACE_SECONDS:
        return None
    if any(not turn.isdigit() or not 1 <= int(turn) <= OPENING_GRACE_TURNS for turn in missing_turns):
        return None
    # Civ VI's board is filmed through the whole opening, and at each grace turn.
    if not all(seen(time, "left") for time in requested if time < joins):
        return None
    if not all(any(seen(time, "left") for time in turn_samples[turn]) for turn in missing_turns):
        return None
    # The grace turns are the leading stretch, never a gap after the CIVVIS picture joined.
    if any(time >= joins for turn in missing_turns for time in turn_samples[turn]
           if not (seen(time, "left") and seen(time, "right"))):
        return None
    if min(shares.values()) < OPENING_GRACE_SHARE:
        return None
    return {"panel": "CIVVIS", "joins_at_seconds": round(joins - start, 3),
            "civ_vi_only_turns": sorted(missing_turns, key=int),
            "leading_blackout_samples": sum(1 for time in regular if time < joins)}


def game_dark_within_bounds(evidence):
    spans = evidence.get("game_dark_spans")
    if spans is None:
        return True
    return bool(GAME_DARK_MAX > 0 and isinstance(spans, list) and spans and all(
        isinstance(span, dict) and 2 <= span.get("dark_samples", 0) <= GAME_DARK_MAX
        and str(span.get("screen", "")).endswith(GAME_DARK_SCREEN_SUFFIX) for span in spans))


def grace_within_bounds(evidence):
    grace = evidence.get("opening_grace")
    if grace is None:
        return True
    turns = grace.get("civ_vi_only_turns") or []
    return bool(OPENING_GRACE_SECONDS > 0 and grace.get("panel") == "CIVVIS" and turns
            and 0 <= grace.get("joins_at_seconds", float("inf")) <= OPENING_GRACE_SECONDS
            and all(str(turn).isdigit() and 1 <= int(turn) <= OPENING_GRACE_TURNS for turn in turns)
            and evidence.get("left_visible_share", 0) >= OPENING_GRACE_SHARE
            and evidence.get("right_visible_share", 0) >= OPENING_GRACE_SHARE)


def panel_verified(evidence, turns):
    return (evidence.get("schema") == PANEL_SCHEMA and evidence.get("both_visible") is True
            and evidence.get("samples", 0) > 0 and evidence.get("turn_samples") == turns
            and evidence.get("left_visible_share", 0) >= 0.98
            and evidence.get("right_visible_share", 0) >= 0.98
            and evidence.get("max_blackout_seconds", float("inf")) <= 2
            and evidence.get("missing_turns") == []
            and grace_within_bounds(evidence)
            and game_dark_within_bounds(evidence))


def event_instant(event):
    """An event's epoch seconds: its millisecond `utc` stamp, else its whole-second `t`."""
    stamp = event.get("utc")
    if isinstance(stamp, str):
        try:
            return datetime.fromisoformat(stamp.replace("Z", "+00:00")).timestamp()
        except ValueError:
            pass
    t = event.get("t")
    return float(t) if isinstance(t, (int, float)) else None


def held_cuts(holds, guards, duration):
    """Spans of the window (seconds from its start) to drop: each held game minus its
    margins and every turn guard, sorted, at least HOLD_MIN_CUT long."""
    cuts = []
    for paused, resumed in holds:
        pieces = [(max(0.0, paused + HOLD_MARGIN), min(duration, resumed - HOLD_MARGIN))]
        for guard in guards:
            low, high = guard - TURN_GUARD[0], guard + TURN_GUARD[1]
            pieces = [piece for a, b in pieces for piece in ((a, min(b, low)), (max(a, high), b))
                      if piece[1] > piece[0]]
        cuts += [(a, b) for a, b in pieces if b - a >= HOLD_MIN_CUT]
    cuts.sort()
    merged = []
    for a, b in cuts:
        if merged and a <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(merged[-1][1], b))
        else:
            merged.append((a, b))
    return merged


def file_fingerprint(path):
    info = Path(path).stat()
    return {"size_bytes": info.st_size, "mtime_ns": info.st_mtime_ns}


def source_evidence(gid, row, raw, window, state=None, control=None, recorder_log=None):
    if not window:
        raise QualityError("no verified first-turn-to-victory time window")
    game = (read_json(K.STATE) if state is None else state).get("games", {}).get(gid, {})
    segments = sorted(game.get("segments", []), key=lambda segment: segment.get("started") or 0)
    if game.get("partial") or not segments or len(segments) != row.get("segments"):
        raise QualityError("partial recording or missing stitched segments")
    if any(segment.get("rc") != 0 or segment.get("ended") is None for segment in segments):
        raise QualityError("a recording segment did not close cleanly")
    verdict = row.get("verdict") or {}
    if not verdict.get("won"):
        raise QualityError("no confirmed victory")
    control = K.CONTROL if control is None else Path(control)
    turns = set()
    turn_instants = {}
    victory_instants = {}
    victories = []
    holds = []  # (paused, resumed) epochs of the live game held on its own turn
    instants = []  # every state/victory epoch: footage around them is never cut
    movies = []  # (epoch, screen) of each full-screen movie popup the game reported
    for tag in game.get("tags", {}):
        path = control / tag / "events.jsonl"
        if not path.exists():
            raise QualityError(f"missing turn telemetry for {tag}")
        paused_at = None
        with path.open() as events:
            for line in events:
                event = json.loads(line)
                if event.get("kind") == "operator_paused" and paused_at is None:
                    paused_at = event_instant(event)
                elif event.get("kind") == "operator_resumed" and paused_at is not None:
                    resumed_at = event_instant(event)
                    if resumed_at is not None and resumed_at > paused_at:
                        holds.append((paused_at, resumed_at))
                    paused_at = None
                if (event.get("kind") in ("event_lock", "autoclose")
                        and str(event.get("screen") or "").endswith(GAME_DARK_SCREEN_SUFFIX)):
                    instant = event_instant(event)
                    if instant is not None:
                        movies.append((instant, event["screen"]))
                if event.get("kind") not in ("state", "victory"):
                    continue
                if isinstance(event.get("t"), (int, float)):
                    instants.append(float(event["t"]))
                turn = event.get("turn")
                if not isinstance(turn, int) or turn < 1:
                    continue
                timestamp = event.get("t")
                if not isinstance(timestamp, (int, float)) or not any(
                        segment["started"] - 1 <= timestamp <= segment["ended"] + 1
                        for segment in segments):
                    raise QualityError(f"turn {turn} was not covered by its recording")
                if event["kind"] == "state":
                    turns.add(turn)
                    turn_instants.setdefault(turn, timestamp)
                else:
                    victories.append(turn)
                    victory_instants.setdefault(turn, timestamp)
    if not victories:
        raise QualityError("missing actual victory event")
    final_turn = max(victories)
    # A victory decided in the turn change (204643Z: state t194, then the t195 victory 18 s
    # later) has no state export for its own turn; its filmed victory event is that turn's
    # instant. Only the final turn, right after a recorded one, is covered this way.
    if final_turn not in turns and final_turn - 1 in turns:
        turns.add(final_turn)
        turn_instants.setdefault(final_turn, victory_instants[final_turn])
    missing = sorted(set(range(1, final_turn + 1)) - turns)
    if missing:
        raise QualityError(f"missing recorded turns: {missing[:10]}")
    log_text = ((K.CLIPS / "recorder.log").read_text(errors="replace")
                if recorder_log is None else recorder_log)
    trims = {}
    for line in log_text.splitlines():
        match = re.search(r"concat: \+ (\S+\.mp4) \([\d.]+ min(?:, setup (\d+) s trimmed)?", line)
        if match:
            trims[match[1]] = float(match[2] or 0)
    expected_raw = 0.0
    stitched_segments = []
    for segment in segments:
        name = Path(segment["path"]).name
        if name not in trims:
            raise QualityError(f"no successful concat receipt for {name}")
        stitched_segments.append((segment, trims[name], expected_raw))
        expected_raw += segment["ended"] - segment["started"] - trims[name]
    actual_raw = media_info(raw)["duration_seconds"]
    if abs(actual_raw - expected_raw) > max(4, len(segments) * 3):
        raise QualityError("source duration does not match real-time recording coverage")
    start, duration = window
    if start < 0 or duration <= 0 or start + duration > actual_raw + 2:
        raise QualityError("turn window is outside the source recording")
    def window_offset(timestamp):
        """(segment index, seconds from the window start) of an epoch, or None."""
        for index, (segment, trim, cursor) in enumerate(stitched_segments):
            if segment["started"] + trim - 1 <= timestamp <= segment["ended"] + 1:
                return index, cursor + timestamp - segment["started"] - trim - start
        return None

    turn_offsets = {}
    for turn, timestamp in turn_instants.items():
        if turn > final_turn:
            continue
        placed = window_offset(timestamp)
        if placed:
            turn_offsets[str(turn)] = max(0, placed[1])
    if len(turn_offsets) != final_turn:
        raise QualityError("not every turn maps to a filmed picture")
    # Windows after each game movie popup in which a short Firaxis-only dark run is the game's own
    # screen; none where the recorder logged a Civ-slot capture event (GAME_DARK_MAX).
    capture_events = sorted(datetime.fromisoformat(match[1].replace("Z", "+00:00")).timestamp()
                            for match in map(CAPTURE_EVENT.match, log_text.splitlines()) if match)
    game_dark = []
    for instant, screen in sorted(movies):
        placed = window_offset(instant)
        low = bisect.bisect_left(capture_events, instant - 1)
        if not placed or (low < len(capture_events)
                          and capture_events[low] <= instant + GAME_DARK_SPAN + 1):
            continue
        game_dark.append([round(placed[1], 3), round(placed[1] + GAME_DARK_SPAN, 3), screen])
    panels = panel_evidence(raw, start, duration, turn_offsets, game_dark=game_dark)
    window_holds = []
    for paused, resumed in holds:
        a, b = window_offset(paused), window_offset(resumed)
        if a and b and a[0] == b[0]:  # a hold across a reload is left in
            window_holds.append((a[1], b[1]))
    guards = [placed[1] for placed in map(window_offset, instants) if placed]
    guards += [float(offset) for offset in turn_offsets.values()]
    cuts = held_cuts(window_holds, guards, duration)
    held = sum(b - a for a, b in cuts)

    def master_offset(offset):
        return round(offset - sum(b - a for a, b in cuts if b <= offset), 3)

    proof = {"schema": QUALITY_SCHEMA, "game_id": gid, "all_turns": True,
             "first_turn": 1, "final_turn": final_turn, "covered_turns": final_turn,
             "playback_rate": 1.0, "segments": len(segments),
             "expected_seconds": duration - held, "source_seconds": actual_raw,
             "window_start_seconds": start, "export_recipe": "unscaled_time_range",
             "turn_offsets_seconds": {turn: master_offset(offset) for turn, offset in turn_offsets.items()},
             "dual_panels": {"source": panels}}
    spans = panels.get("game_dark_spans") or []
    used = [window for window in game_dark
            if any(window[0] <= span["from_seconds"] and span["to_seconds"] - 1 <= window[1] for span in spans)]
    if used:
        proof["game_dark_windows_seconds"] = [[master_offset(a), master_offset(b), screen]
                                              for a, b, screen in used]
    if cuts:
        proof.update(export_recipe=HELD_RECIPE, window_seconds=duration,
                     held_seconds=round(held, 3),
                     held_cuts_seconds=[[round(start + a, 3), round(start + b, 3)] for a, b in cuts])
    return proof


def held_cuts_consistent(evidence):
    """One unscaled time range, or one minus held-game cuts that account exactly for
    the shorter master and sit inside the window."""
    cuts = evidence.get("held_cuts_seconds")
    if evidence.get("export_recipe") == "unscaled_time_range":
        return not cuts
    if evidence.get("export_recipe") != HELD_RECIPE or not cuts:
        return False
    start = evidence.get("window_start_seconds", 0)
    window = evidence.get("window_seconds", 0)
    previous = start
    for a, b in cuts:
        if not (previous <= a and b - a >= HOLD_MIN_CUT - 0.01 and b <= start + window):
            return False
        previous = b
    held = sum(b - a for a, b in cuts)
    return (abs(held - evidence.get("held_seconds", -1)) < 0.05
            and abs(window - held - evidence.get("expected_seconds", 0)) < 0.05)


def terminal_presentation(evidence):
    """Only the confirmed last turn may use a result card instead of two panels."""
    terminal = evidence.get("terminal_presentation")
    if terminal is None:
        return None
    from victory_state import local_victory
    final = evidence.get("final_turn", 0)
    outcome = terminal.get("outcome") or {}
    start = terminal.get("start_seconds")
    offset = evidence.get("turn_offsets_seconds", {}).get(str(final))
    if (terminal.get("kind") != "confirmed_victory_card" or not local_victory(outcome)
            or outcome["turn"] != final or not isinstance(start, (int, float))
            or offset is None or abs(start - float(offset)) > 0.05
            or not 1 <= start < evidence.get("expected_seconds", 0)
            or evidence["expected_seconds"] - start > 15):
        raise QualityError("unverified or mistimed terminal result card")
    return float(start), final - 1


def validate_master(path, evidence):
    if evidence.get("schema") != QUALITY_SCHEMA:
        raise QualityError("no verified dual-panel pictures; the earlier receipt only checked timing")
    if (not evidence.get("all_turns")
            or evidence.get("first_turn") != 1 or evidence.get("final_turn", 0) < 1
            or evidence.get("covered_turns") != evidence.get("final_turn")
            or evidence.get("playback_rate") != 1.0
            or not held_cuts_consistent(evidence)):
        raise QualityError("no complete real-time export proof")
    info = media_info(path)
    if info["width"] < 3840 or info["height"] < 2160 or info["fps"] < 59:
        raise QualityError("master is not 4K/60 fps")
    if abs(info["duration_seconds"] - evidence["expected_seconds"]) > 1:
        raise QualityError("master duration changed: possible time scaling or missing turns")
    panels = evidence.get("dual_panels", {})
    if not panel_verified(panels.get("source", {}), evidence["final_turn"]):
        raise QualityError("source has no verified Firaxis and CIVVIS pictures")
    fingerprint = file_fingerprint(path)
    master = panels.get("master", {})
    terminal = terminal_presentation(evidence)
    panel_duration, panel_turns = terminal or (info["duration_seconds"], evidence["final_turn"])
    offsets = {t: v for t, v in evidence["turn_offsets_seconds"].items() if int(t) <= panel_turns}
    if evidence.get("master_file") != fingerprint or not panel_verified(master, panel_turns):
        master = panel_evidence(path, 0, panel_duration, offsets,
                                game_dark=evidence.get("game_dark_windows_seconds"))
    if terminal:
        master["scope"] = "gameplay before confirmed final result card"
    return dict(evidence, **info, dual_panels={"source": panels["source"], "master": master},
                master_file=fingerprint)


def update_policy(level, path=None):
    path = POLICY if path is None else Path(path)
    desired = {"CIVVIS_DIFFICULTY": f"DIFFICULTY_{'EMPEROR' if level == 6 else 'IMMORTAL'}",
               "CIVVIS_VICTORY": "domination", "CIVVIS_RESTART_BELOW_PRODUCTION_RANK": "0",
               "CIVVIS_RESTART_BELOW_LEADER_RATIO": "0"}
    with path.with_name(path.name + ".lock").open("a+") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        previous = path.read_text()
        lines = []
        seen = set()
        for line in previous.splitlines():
            key = line.split("#", 1)[0].partition("=")[0].strip()
            if key in desired:
                if key not in seen:
                    lines.append(f"{key}={desired[key]}")
                    seen.add(key)
            else:
                lines.append(line)
        lines.extend(f"{key}={value}" for key, value in desired.items() if key not in seen)
        updated = "\n".join(lines) + "\n"
        if updated != previous:
            temporary = path.with_name(path.name + f".{os.getpid()}.tmp")
            temporary.write_text(updated)
            temporary.chmod(path.stat().st_mode & 0o777)
            os.replace(temporary, path)


def reconcile(rows, dry_run=False):
    if not GOAL.exists():
        return None
    goal = read_json(GOAL)
    if not goal.get("enabled"):
        return None
    if (goal.get("from_level"), goal.get("to_level"), goal.get("required_videos")) != (6, 7, 5):
        raise QualityError("unsupported video milestone configuration")
    qualified = {}
    rejected = {}
    for row in rows:
        gid = row.get("game_id")
        if not gid or (row.get("level"), row.get("civ"), row.get("victory")) != (
                6, goal["civ"], "Domination") or not row.get("master"):
            continue
        path = Path(row["master"])
        try:
            if path.parent.resolve() != K.DESKTOP.resolve() or not path.is_file():
                raise QualityError("master is not saved on the Desktop")
            evidence = read_json(path.with_suffix(".quality.json"))
            if evidence.get("game_id") != gid or evidence != row.get("quality"):
                raise QualityError("quality receipt does not match the published master")
            validate_master(path, evidence)
            qualified[gid] = str(path)
        except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
            rejected[gid] = str(error)
    level = 7 if len(qualified) >= goal["required_videos"] else 6
    status = {"updated_at": datetime.now(timezone.utc).isoformat(), "required_videos": 5,
              "qualified_videos": len(qualified), "selected_level": level,
              "qualified": qualified, "unqualified": rejected,
              "promotion": "ready" if level == 7 else "waiting_for_verified_videos"}
    if not dry_run:
        update_policy(level)
        atomic_json(STATUS, status)
    return status
