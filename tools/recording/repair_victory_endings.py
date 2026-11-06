"""Repair recorded terminal presentation, retaining originals and remuxing gameplay.

Requires the installed clip-recorder modules and the two Swift binaries built
from youtube_master.swift and join_victory_tail.swift. Never uploads videos.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
from pathlib import Path


def repair_start(duration, victory_at):
    # Some old masters already held the final turn for more than twelve seconds.
    # Begin before its result so the card is never delayed past the victory.
    return max(0, min(int(duration - 12), int(victory_at) - 2))


def repair(video, renderer, joiner):
    import civvis_video_story as story
    import civvis_video_goal as gate
    from victory_state import completed_frame, local_victory, LANES
    from verify_match_log_master import preview
    from PIL import Image, ImageChops, ImageStat

    receipt_path = video.with_suffix(".quality.json")
    quality = json.loads(receipt_path.read_text())
    if quality.get("terminal_result"):
        print("already repaired", video.name, flush=True)
        return
    gid, final_turn = quality["game_id"], quality["final_turn"]
    first, last, events = story._load(gid)
    outcome = next((e for e in reversed(events) if local_victory(e)), None)
    if not outcome and not last:
        # The run pruner retains the recorder verdict, quality receipt and HUD.
        # Use those three independent surviving records, never a filename.
        import civvis_clip_keeper as keeper
        rows = [json.loads(line) for line in keeper.JOURNAL.read_text().splitlines()]
        verdict = next((r.get("verdict", {}) for r in reversed(rows) if r.get("game_id") == gid), {})
        if verdict.get("won") is True and verdict.get("victory_type") in LANES:
            outcome = dict(kind="victory", won=True, turn=final_turn, local_player=0,
                           victory_type=verdict["victory_type"], source="retained_recorder_verdict_and_quality_receipt")
            archived_hud = json.loads((keeper.CLIPS / "youtube-staging" / f"{gid}.hud.json").read_text())
            frame = dict(turn=final_turn - 1)
            lane = LANES[outcome["victory_type"]]
            prior = next((i["race"] for i in reversed(archived_hud["items"])
                          if " / " in i.get("race", "")), "")
            numbers = re.search(r"(\d+(?:\.\d+)?) / (\d+(?:\.\d+)?)", prior)
            if lane == "science" and numbers:
                frame.update(science_victory_points=float(numbers[1]), science_victory_points_needed=float(numbers[2]))
            elif lane == "culture" and numbers:
                frame.update(foreign_tourists=float(numbers[1]),
                             rivals=[dict(civ="archived_major", player=1, domestic_tourists=float(numbers[2]) - 1)])
            elif lane == "diplomatic" and numbers:
                frame["dvp"] = float(numbers[1])
            if lane in ("science", "culture", "diplomatic") and not numbers:
                raise ValueError(f"{gid}: no retained target; refusing to invent a counter")
            last = {frame["turn"]: frame}
    if not outcome or outcome["turn"] != final_turn:
        raise ValueError(f"{gid}: no matching confirmed victory; refusing to infer a result")
    final = completed_frame(last[max(last)], outcome)
    lane = final["confirmed_victory"].title()
    majors = 1 + len(story._majors(final))
    detail = story._race(lane, final, outcome["local_player"], majors)[0]
    if lane == "Domination":
        detail = "All rival original capitals controlled · victory confirmed"
    elif lane == "Religious":
        detail = "Religion predominant in every civilization"
    elif lane == "Score":
        detail = "Highest score at the turn limit"
    victory_at = float(quality["turn_offsets_seconds"][str(final_turn)])
    info = gate.media_info(video)
    duration = info["duration_seconds"]
    cut = repair_start(duration, victory_at)
    work = Path.home() / "civvis-civ6-runs/clips/repairs/victory-completion" / gid
    work.mkdir(parents=True, exist_ok=True)
    hud = work / "result.hud.json"
    item = dict(t=max(0, victory_at - cut), confirmed_result=True,
                race=f"★ {lane} Victory", result_detail=detail, turn=f"TURN {final_turn}")
    hud.write_text(json.dumps(dict(items=[item], callouts=[])))
    tail, candidate = work / "tail.mp4", work / "repaired.mp4"
    for path in (tail, candidate):
        if path.exists():
            path.unlink()
    print("repairing", video.name, detail, flush=True)
    subprocess.run([str(renderer), "--in", str(video), "--out", str(tail),
                    "--start", str(cut), "--end", str(duration), "--existing-master",
                    "--hold-final", "5", "--confirmed-result-card", "--hud", str(hud)], check=True)
    subprocess.run([str(joiner), str(video), str(tail), str(cut), str(candidate)], check=True)
    result = gate.media_info(candidate)
    if (abs(result["duration_seconds"] - (duration + 5)) > .1
            or (result["width"], result["height"]) != (info["width"], info["height"])
            or abs(result["fps"] - info["fps"]) > .1 or result["codec"] != info["codec"]):
        raise ValueError("repaired format/duration differs from the repair recipe")
    comparisons = []
    for seconds in (min(30, cut / 2), max(0, cut - 2)):
        before, after = work / f"before-{seconds}.png", work / f"after-{seconds}.png"
        preview(video, seconds, before); preview(candidate, seconds, after)
        with Image.open(before) as a, Image.open(after) as b:
            error = ImageStat.Stat(ImageChops.difference(a.convert("RGB"), b.convert("RGB"))).mean
        if max(error) > 1:
            raise ValueError(f"untouched gameplay pixels differ: {seconds}: {error}")
        comparisons.append(dict(seconds=seconds, mean_absolute_rgb_error=error))
    ending = work / "ending.png"
    if ending.exists():
        ending.unlink()
    preview(candidate, result["duration_seconds"] - 1, ending)
    if not ending.exists():
        raise ValueError("repaired ending could not be decoded")
    original = work / video.name
    if original.exists():
        raise ValueError("original backup already exists; refusing to replace it")
    shutil.copy2(receipt_path, work / receipt_path.name)
    text = video.with_suffix(".txt")
    if text.exists():
        shutil.copy2(text, work / text.name)
    original_fingerprint = gate.file_fingerprint(video)
    os.replace(video, original)
    os.replace(candidate, video)
    quality.update(result)
    quality["expected_seconds"] = duration + 5
    quality["terminal_presentation"] = dict(kind="confirmed_victory_card", start_seconds=victory_at, outcome=outcome)
    quality["terminal_result"] = dict(schema="civvis_confirmed_victory_ending/v1", outcome=outcome,
        progress_source="confirmed_result_lower_bound", detail=detail, victory_at_seconds=victory_at,
        original_preserved=str(original), original_file=original_fingerprint,
        unchanged_prefix_seconds=cut, added_final_hold_seconds=5, prefix_pixel_checks=comparisons,
        ending_preview=str(work / "ending.png"))
    quality["master_file"] = gate.file_fingerprint(video)
    gate.atomic_json(receipt_path, quality)
    if text.exists():
        old = text.read_text()
        lines = [line for line in old.splitlines() if not line.startswith("- At the end:")]
        index = next((i for i, line in enumerate(lines) if line.startswith("- Final:")), len(lines))
        lines.insert(index, "- Final victory progress: " + detail + ".")
        text.write_text("\n".join(lines) + "\n")
    tail.unlink()
    print("verified and replaced", video.name, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("videos", nargs="+", type=Path)
    parser.add_argument("--renderer", type=Path, required=True)
    parser.add_argument("--joiner", type=Path, required=True)
    args = parser.parse_args()
    for video in args.videos:
        repair(video, args.renderer, args.joiner)


if __name__ == "__main__":
    main()
