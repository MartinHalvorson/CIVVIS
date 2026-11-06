"""The on-screen story of a won game for its YouTube master.

Operator 2026-10-30: "Polish production videos ready for youtube please. Do what
you can to make the videos great and entertaining to watch, but also clearly
communicating the essential information."

From the run's state frames and event log this builds, in master time (the
quality receipt's ``turn_offsets_seconds``, which already count from the
master's first frame):

* ``items``: one scoreboard state per turn for the bottom band: the turn, the
  world era, our city count and the victory race (tourists against the target
  for Culture, Diplomatic Victory Points out of 20, rival capitals held for
  Domination) with a progress bar.
* ``callouts``: short milestone captions (eras, wonders, golden ages, wars,
  World Congress points, culture overtaking a rival, cities founded/captured).
* ``chapters``: YouTube chapter lines ("0:00 Turn 1 - Greece begins").
* ``story``: plain-language lines for the description.

The master's timing is untouched (real time, turn 1 to the final turn); all of
this is drawn in the black bands, off both game panels.
"""
import json
import re

import civvis_clip_keeper as K
from victory_state import completed_frame, local_victory

ERAS = ["Ancient", "Classical", "Medieval", "Renaissance", "Industrial", "Modern",
        "Atomic", "Information", "Future"]
DVP_TO_WIN = 20
CALLOUT_SECONDS = 7.0
CALLOUT_GAP = 0.8
CHAPTER_GAP = 12.0
MAX_CHAPTERS = 16


# Display names where the game id loses punctuation or diacritics.
NAMES = {"STATUE_LIBERTY": "Statue of Liberty", "ST_BASILS_CATHEDRAL": "St. Basil's Cathedral",
         "ORSZAGHAZ": "Országház", "HALICARNASSUS_MAUSOLEUM": "Mausoleum at Halicarnassus",
         "CHICHEN_ITZA": "Chichén Itzá", "MONT_ST_MICHEL": "Mont St. Michel", "MAHABODHI_TEMPLE": "Mahabodhi Temple",
         "HUEY_TEOCALLI": "Huey Teocalli", "MEENAKSHI_TEMPLE": "Meenakshi Temple", "ST_PETERS_BASILICA": "St. Peter's Basilica",
         "TERRACOTTA_ARMY": "Terracotta Army", "OXFORD_UNIVERSITY": "Oxford University", "MACHU_PICCHU": "Machu Picchu",
         "HANGING_GARDENS": "Hanging Gardens", "GREAT_LIBRARY": "Great Library", "GREAT_LIGHTHOUSE": "Great Lighthouse",
         "GREAT_ZIMBABWE": "Great Zimbabwe", "TEMPLE_ARTEMIS": "Temple of Artemis", "STATUE_OF_ZEUS": "Statue of Zeus",
         "APADANA": "Apadana", "BOLSHOI_THEATRE": "Bolshoi Theatre", "BIG_BEN": "Big Ben", "EIFFEL_TOWER": "Eiffel Tower",
         "CRISTO_REDENTOR": "Cristo Redentor", "SYDNEY_OPERA_HOUSE": "Sydney Opera House", "GOLDEN_GATE_BRIDGE": "Golden Gate Bridge",
         "ESTADIO_DO_MARACANA": "Estádio do Maracanã", "BROADWAY": "Broadway", "AMUNDSEN_SCOTT_RESEARCH_STATION":
         "Amundsen-Scott Research Station", "KILWA_KISIWANI": "Kilwa Kisiwani", "UNIVERSITY_SANKORE": "University of Sankore",
         "CASA_DE_CONTRATACION": "Casa de Contratación", "ALHAMBRA": "Alhambra", "ANGKOR_WAT": "Angkor Wat",
         "TAJ_MAHAL": "Taj Mahal", "VENETIAN_ARSENAL": "Venetian Arsenal", "TORRE_DE_BELEM": "Torre de Belém"}


def _pretty(ident, prefixes=("BUILDING_", "GOVERNMENT_", "LOC_", "DISTRICT_", "WONDER_")):
    s = str(ident or "")
    key = re.sub(r"^(BUILDING_|WONDER_|LOC_BUILDING_)", "", s.upper())
    key = re.sub(r"_NAME$", "", key)
    if key in NAMES:
        return NAMES[key]
    for p in prefixes:
        if s.upper().startswith(p):
            s = s[len(p):]
    s = re.sub(r"_NAME$", "", s)
    return s.replace("_", " ").title().replace(" Of ", " of ").replace(" The ", " the ")


def _city(name):
    name = str(name or "")
    if name.upper().startswith("LOC_"):
        name = re.sub(r"^LOC_CITY_(NAME_)?", "", name, flags=re.I).replace("_", " ").title()
    return name


def stamp(seconds):
    s = int(max(0, seconds))
    h, m, sec = s // 3600, (s % 3600) // 60, s % 60
    return f"{h}:{m:02d}:{sec:02d}" if h else f"{m}:{sec:02d}"


def _load(gid):
    """First and last state frame per turn, plus the non-state events we narrate."""
    first, last, events = {}, {}, []
    path = K.CONTROL / gid / "events.jsonl"
    if not path.exists():
        return first, last, events
    with open(path, errors="replace") as fh:
        for line in fh:
            if '"kind": "state"' in line:
                try:
                    d = json.loads(line)
                except ValueError:
                    continue
                t = d.get("turn")
                if isinstance(t, int):
                    first.setdefault(t, d)
                    last[t] = d
            elif '"kind": "wc_outcome"' in line or '"kind": "victory"' in line:
                try:
                    events.append(json.loads(line))
                except ValueError:
                    pass
    return first, last, events


def _majors(frame):
    return [r for r in frame.get("rivals") or []
            if r.get("civ") and "CITY_STATE" not in str(r.get("civ")) and r.get("player") is not None]


def _race(victory, frame, me, majors_total):
    """(text, progress 0..1 or None) for the scoreboard."""
    rivals = _majors(frame)
    confirmed = bool(frame.get("confirmed_victory"))
    if confirmed and victory in ("Domination", "Religious", "Score"):
        detail = {"Domination": "All rival original capitals controlled",
                  "Religious": "Religion predominant in every civilization",
                  "Score": "Highest score at the turn limit"}
        return detail[victory], 1.0
    if victory == "Culture":
        ours = frame.get("foreign_tourists") or 0
        dom = [(r.get("domestic_tourists") or 0) for r in rivals]
        if dom:
            need = max(dom) + 1
            return f"Visiting tourists {'≥' if frame.get('confirmed_victory') else ''}{ours:g} / {need:g} to win", min(1.0, ours / need)
        return ("Culture victory threshold reached", 1.0) if confirmed else (f"Visiting tourists {ours:g}", None)
    if victory == "Diplomatic":
        dvp = frame.get("dvp") or 0
        best = max([(r.get("dvp") or 0) for r in rivals] or [0])
        return f"Diplomatic Victory Points {'≥' if frame.get('confirmed_victory') else ''}{dvp:g} / {DVP_TO_WIN}   (best rival {best})", min(1.0, dvp / DVP_TO_WIN)
    if victory == "Domination":
        held = sum(1 for c in frame.get("cities") or []
                   if c.get("original_capital") and c.get("original_owner") not in (None, me))
        n = max(1, majors_total - 1)
        return f"Rival capitals held {held} / {n}", min(1.0, held / n)
    if victory == "Science":
        pts = frame.get("science_victory_points") or 0
        need = frame.get("science_victory_points_needed") or 0
        if need:
            return f"Science victory points {'≥' if frame.get('confirmed_victory') else ''}{pts:g} / {need:g}", min(1.0, pts / need)
        if confirmed:
            return "Science victory threshold reached", 1.0
    return "", None


def build(gid, victory, civ, turn_offsets, final_turn, duration):
    first, last, wc = _load(gid)
    offsets = {int(k): float(v) for k, v in (turn_offsets or {}).items()}
    if not first or not offsets:
        return None
    outcome = next((e for e in reversed(wc) if local_victory(e)), None)
    terminal = None
    if outcome and outcome["turn"] == final_turn:
        terminal = completed_frame(last[max(last)], outcome)
    me = next(iter(first.values())).get("local_player", 0)
    majors_total = 1 + len({r.get("player") for f in last.values() for r in _majors(f)})

    def at(turn):
        return offsets.get(turn)

    items = []
    for turn in sorted(t for t in first if t in offsets and t <= final_turn):
        f = first[turn]
        era = f.get("world_era")
        info = []
        if isinstance(era, int) and 0 <= era < len(ERAS):
            info.append(f"{ERAS[era]} Era")
        n = len(f.get("cities") or [])
        if n:
            info.append(f"{n} {'city' if n == 1 else 'cities'}")
        if f.get("golden_age") or f.get("heroic_golden_age"):
            info.append("★ Golden Age")
        elif f.get("dark_age"):
            info.append("Dark Age")
        race, progress = _race(victory, f, me, majors_total)
        item = {"t": round(at(turn), 2), "turn": f"TURN {turn}", "info": "   ·   ".join(info), "race": race}
        if progress is not None:
            item["progress"] = round(progress, 3)
        items.append(item)
    # The victory turn closes the scoreboard with the result (204643Z won in the turn change,
    # so its t195 had no state of its own and the bar ended on t194's 19 / 20).
    if items and at(final_turn) is not None and round(at(final_turn), 2) >= items[-1]["t"]:
        if items[-1]["turn"] == f"TURN {final_turn}":
            items.pop()
        info = items[-1]["info"] if items else ""
        items.append({"t": round(at(final_turn), 2), "turn": f"TURN {final_turn}", "info": info,
                      "race": f"★ {victory} Victory" + (" · " + _race(victory, terminal, me, majors_total)[0]
                                                       if terminal else ""), "progress": 1.0,
                      "confirmed_result": bool(terminal),
                      "result_detail": (_race(victory, terminal, me, majors_total)[0] if terminal else ""),
                      "result_note": "Victory confirmed · counters show the winning minimum"})

    # Milestones: (turn, priority, callout text, chapter title or None). Priority 0 = always a chapter.
    marks = []
    prev = None
    wonders_seen, cities_seen, captured_seen = set(), set(), set()
    at_war, dominated, gov = {}, set(), None
    dvp_marks = set()
    names = {}
    golden = dark = False
    story = {"wonders": [], "wars": [], "golden": [], "dvp": [], "culture": []}
    for turn in sorted(last):
        if turn > final_turn:
            break
        f = last[turn]
        for r in _majors(f):
            names.setdefault(r["player"], K.civ_name(r.get("civ"), r.get("leader")))
        era = f.get("world_era")
        if turn == 1 or prev is None:
            marks.append((1, 0, None, f"Turn 1 - {civ} begins"))
        elif isinstance(era, int) and era != prev.get("world_era") and 0 <= era < len(ERAS):
            marks.append((turn, 0, f"Turn {turn}  ·  The world enters the {ERAS[era]} Era", f"Turn {turn} - {ERAS[era]} Era"))
        g = bool(f.get("golden_age") or f.get("heroic_golden_age"))
        if g and not golden:
            kind = "Heroic Age" if f.get("heroic_golden_age") else "Golden Age"
            marks.append((turn, 2, f"Turn {turn}  ·  {civ} enters a {kind}", None))
            story["golden"].append(turn)
        golden = g
        dk = bool(f.get("dark_age"))
        if dk and not dark:
            marks.append((turn, 3, f"Turn {turn}  ·  {civ} slips into a Dark Age", None))
        dark = dk
        gv = f.get("government")
        if gv and gov and gv != gov:
            marks.append((turn, 3, f"Turn {turn}  ·  New government: {_pretty(gv)}", None))
        gov = gv or gov
        for c in f.get("cities") or []:
            cname = _city(c.get("name"))
            for w in c.get("wonders") or []:
                wname = _pretty(w if isinstance(w, str) else (w.get("type") or w.get("name")))
                if wname and wname not in wonders_seen:
                    wonders_seen.add(wname)
                    if prev is not None and victory == "Diplomatic" and wname == "Statue of Liberty":
                        # The Statue's +4 is the largest point source outside the Congress.
                        marks.append((turn, 1, f"Turn {turn}  ·  Statue of Liberty built in {cname}: "
                                               f"+4 Diplomatic Victory Points",
                                      f"Turn {turn} - Statue of Liberty (+4 DVP)"))
                    elif prev is not None:
                        marks.append((turn, 2 if victory == "Diplomatic" else 1,
                                      f"Turn {turn}  ·  Wonder built: {wname} in {cname}",
                                      f"Turn {turn} - {wname}"))
                        story["wonders"].append(f"{wname} ({cname}, turn {turn})")
            if cname and cname not in cities_seen:
                cities_seen.add(cname)
                if prev is not None:
                    if c.get("original_owner") not in (None, me) and cname not in captured_seen:
                        captured_seen.add(cname)
                        owner = names.get(c.get("original_owner"), "a rival")
                        cap = " (their capital)" if c.get("original_capital") else ""
                        marks.append((turn, 1, f"Turn {turn}  ·  {civ} captures {cname} from {owner}{cap}",
                                      f"Turn {turn} - {cname} captured" if cap else None))
                    else:
                        marks.append((turn, 4, f"Turn {turn}  ·  {cname} founded", None))
        for r in _majors(f):
            pid, war = r["player"], bool(r.get("at_war"))
            if war and not at_war.get(pid):
                marks.append((turn, 1, f"Turn {turn}  ·  War with {names[pid]}", f"Turn {turn} - War with {names[pid]}"))
                story["wars"].append(f"turn {turn}: war with {names[pid]}")
            elif not war and at_war.get(pid):
                marks.append((turn, 3, f"Turn {turn}  ·  Peace with {names[pid]}", None))
            at_war[pid] = war
            if (f.get("foreign_tourists") or 0) > (r.get("domestic_tourists") or 0) > 0 and pid not in dominated:
                dominated.add(pid)
                marks.append((turn, 1, f"Turn {turn}  ·  {civ}'s culture now dominates {names[pid]}",
                              f"Turn {turn} - Culture dominates {names[pid]}"))
                story["culture"].append(f"turn {turn}: culture dominance over {names[pid]}")
        dvp = f.get("dvp") or 0
        for m in (5, 10, 15):
            if dvp >= m and m not in dvp_marks:
                dvp_marks.add(m)
                if victory == "Diplomatic" or m >= 10:
                    marks.append((turn, 1 if victory == "Diplomatic" else 3,
                                  f"Turn {turn}  ·  {civ} reaches {m} Diplomatic Victory Points",
                                  None))  # the World Congress chapters carry the DVP story
                story["dvp"].append(f"{m} by turn {turn}")
        prev = f
    # wc_outcome "dvp" carries each player's running TOTAL, not the session's gain. The gain is
    # the total against our points in the last state exported before the session: points won
    # between sessions (the Statue of Liberty, techs, civics) are not the Congress's (204643Z
    # showed "+7" at t175 for a +3 session that followed the Statue's +4).
    have = 0
    timeline = sorted((fr.get("t"), fr.get("dvp") or 0) for frames in (first, last) for fr in frames.values()
                      if isinstance(fr.get("t"), (int, float)))
    for e in sorted(wc, key=lambda e: e.get("turn") or 0):
        if e.get("kind") != "wc_outcome":
            continue
        turn = e.get("turn")
        total = max([p.get("points") or 0 for p in e.get("dvp") or [] if p.get("player") == me] or [have])
        before = [dvp for t, dvp in timeline if isinstance(e.get("t"), (int, float)) and t < e["t"]]
        gain, have = total - (before[-1] if before else have), max(have, total)
        if isinstance(turn, int) and turn <= final_turn and gain > 0:
            # In a Diplomatic game the Congress sessions ARE the story: chapter each gain.
            diplo = victory == "Diplomatic"
            marks.append((turn, 1 if diplo else 2,
                          f"Turn {turn}  ·  World Congress: +{gain} Diplomatic Victory "
                          f"Point{'s' if gain != 1 else ''} for {civ} ({total} total)",
                          f"Turn {turn} - World Congress: +{gain} ({total} DVP)" if diplo and gain >= 2 else None))
    marks.sort(key=lambda m: (m[0], m[1]))

    callouts, cursor = [], 0.0
    crowded = sum(1 for m in marks if m[2]) * (CALLOUT_SECONDS + CALLOUT_GAP) > 0.5 * max(1.0, duration)
    for turn, pri, text, _ in marks:
        if not text or at(turn) is None or (crowded and pri >= 4):
            continue
        t = max(at(turn) + 0.6, cursor)
        if t - at(turn) > 25 or t + 2 > duration - 8:   # stale, or under the end card
            continue
        callouts.append({"t": round(t, 2), "dur": CALLOUT_SECONDS, "text": text})
        cursor = t + CALLOUT_SECONDS + CALLOUT_GAP

    chapters = [(0.0, f"Turn 1 - {civ} begins")]
    used = set()
    cands = [(at(t), pri, title) for t, pri, _, title in marks if title and t > 1 and at(t) is not None]
    vt = at(final_turn)
    end_title = f"Turn {final_turn} - {victory} Victory!"
    for pri_cap in (0, 1, 2):
        for t, pri, title in sorted(cands):
            if pri > pri_cap or title in used:
                continue
            if len(chapters) >= MAX_CHAPTERS - 1:
                break
            if all(abs(t - c[0]) >= CHAPTER_GAP for c in chapters) and (vt is None or vt - t >= CHAPTER_GAP):
                chapters.append((t, title))
                used.add(title)
            elif pri <= 1 and (vt is None or vt - t >= CHAPTER_GAP):
                # Too close to a chapter already taken (e.g. a Congress claim a turn before
                # a new era): share its line rather than vanish from the chapter list.
                i = min(range(len(chapters)), key=lambda k: abs(chapters[k][0] - t))
                ct, ctitle = chapters[i]
                if i > 0 and abs(ct - t) < CHAPTER_GAP:
                    head, rest = (ctitle, title) if ct <= t else (title, ctitle)
                    merged = head + "; " + re.sub(r"^Turn \d+ - ", "", rest)
                    if len(merged) <= 90:
                        chapters[i] = (min(ct, t), merged)
                        used.add(title)
    chapters.sort()
    if vt is not None and vt - chapters[-1][0] >= CHAPTER_GAP:
        chapters.append((vt, end_title))
    chapter_lines = [f"{stamp(t)} {title}" for t, title in chapters] if len(chapters) >= 3 else []

    lines = []
    if story["wonders"]:
        lines.append("Wonders built: " + ", ".join(story["wonders"]) + ".")
    if story["golden"]:
        lines.append("Golden Ages from turn " + ", ".join(str(t) for t in story["golden"]) + ".")
    if story["dvp"]:
        lines.append("Diplomatic Victory Points: " + ", ".join(story["dvp"]) + ".")
    if story["culture"]:
        lines.append("Culture: " + "; ".join(story["culture"]) + ".")
    f = terminal or last.get(final_turn) or last[max(last)]
    if victory == "Culture":
        dom = [(names.get(r["player"]), r.get("domestic_tourists") or 0) for r in _majors(f)]
        if dom:
            lines.append(f"At the end: {f.get('foreign_tourists') or 0} visiting tourists against the best rival's "
                         f"{max(d[1] for d in dom)} domestic tourists.")
    if victory == "Diplomatic":
        lines.append(f"At the end: at least {f.get('dvp') or 0} Diplomatic Victory Points (victory confirmed).")
    return {"items": items, "callouts": callouts, "chapters": chapter_lines, "story": lines}
