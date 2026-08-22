#!/usr/bin/env python3
"""Turn one iteration's output into a row, and say what is worth reading in it.

Both arms are driven by `sim.mjs`, so both report the same JSON; what differs
is only which build answered. That is the whole point — the same seed, fully
named, played by the native binary and by the wasm32 module, has to come out
the same game.

The row carries the notes: the things a green exit status hides. A game nobody
won, a civilization eliminated, a build that grew warnings, an arm that lost
throughput against its own history — and a pair of arms that disagreed.
"""

import argparse
import json
import pathlib


def parse_report(text):
    """Read the harness's own JSON. Both arms are driven by it, so both speak it."""
    start = text.find("{")
    if start == -1:
        return {}
    try:
        got = json.loads(text[start:])
    except json.JSONDecodeError:
        return {}
    out = {
        "turn": got.get("turn"),
        "winner": got.get("winner"),
        "victory": got.get("victory"),
        "turn_limit": got.get("turn_limit"),
        "engine_seconds": got.get("seconds"),
        "scores": got.get("scores") or [],
        "requests": got.get("requests"),
        "map": got.get("map"),
        "map_digest": got.get("map_digest"),
        "starts_digest": got.get("starts_digest"),
        "commit": got.get("commit"),
        "seat_strategy": got.get("seat_strategy"),
    }
    for key in ("error", "panic", "wasm_bytes", "peak_wasm_mib", "cpu_seconds"):
        if got.get(key):
            out[key] = got[key]
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--result", required=True)
    ap.add_argument("--ledger", required=True)
    ap.add_argument("--iteration", type=int, required=True)
    ap.add_argument("--arm", required=True)
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--sha", required=True)
    ap.add_argument("--stamp", required=True)
    ap.add_argument("--build-seconds", type=int, default=0)
    ap.add_argument("--build-warnings", type=int, default=0)
    ap.add_argument("--sim-seconds", type=int, default=0)
    ap.add_argument("--sim-status", type=int, default=0)
    ap.add_argument("--raw-wasm-bytes", type=int, default=0)
    ap.add_argument("--config", default="")
    ap.add_argument("--cpu-seconds", type=float, default=0.0)
    ap.add_argument("--load", type=float, default=0.0)
    ap.add_argument("--repeat-log")
    ap.add_argument("--publish", default="")
    ap.add_argument("--publish-bytes", type=int, default=0)
    ap.add_argument("--saveload", default="")
    ap.add_argument("--saveload-log")
    ap.add_argument("--sim-log")
    ap.add_argument("--build-log")
    ap.add_argument("--failed")
    # Tolerant on purpose. This loop edits its own scripts while running, and
    # `iterate.sh` is re-read every pass while a half-updated `record.py` is
    # not — so a flag can arrive here before the argument that accepts it does.
    # That happened: iteration 170 played a full game and then lost it, because
    # an unrecognised `--saveload` made argparse exit(2). The game is the
    # expensive part and it had already been played; refusing to write it down
    # over an argument name is the worst possible trade.
    #
    # ⚠ The ordering rule this cost: when changing what passes between the two,
    # teach the *callee* first and only then the caller.
    args, unknown = ap.parse_known_args()
    if unknown:
        print(f"   (ignoring unrecognised argument(s): {' '.join(unknown)})")

    row = {
        "iteration": args.iteration,
        "arm": args.arm,
        "seed": args.seed,
        "sha": args.sha,
        "at": args.stamp,
        "build_seconds": args.build_seconds,
        "build_warnings": args.build_warnings,
        "sim_seconds": args.sim_seconds,
        "config": args.config,
        "load": args.load,
        "sim_status": args.sim_status,
        "ok": args.failed is None and args.sim_status == 0,
    }
    notes = []

    if args.failed:
        row["failed"] = args.failed
        if args.build_log:
            errors = [
                line.rstrip()
                for line in pathlib.Path(args.build_log).read_text(errors="replace").splitlines()
                if line.startswith("error")
            ]
            row["build_errors"] = errors[:20]
        notes.append(args.failed)
    elif args.sim_log:
        text = pathlib.Path(args.sim_log).read_text(errors="replace")
        row.update(parse_report(text))

    turn = row.get("turn") or 0
    seconds = row.get("engine_seconds") or 0
    if turn and seconds:
        row["turns_per_second"] = round(turn / seconds, 3)
    # The rust arm's engine is the server, whose CPU the caller reads from `ps`;
    # the wasm arm's engine is the harness process, which reports its own. Only
    # one of the two will be set on any given row.
    cpu = args.cpu_seconds or row.get("cpu_seconds") or 0
    if cpu:
        row["cpu_seconds"] = round(cpu, 3)
        if turn:
            row["turns_per_cpu_second"] = round(turn / cpu, 3)

    # The same build, the same seed, played twice. If this ever disagrees,
    # every parity conclusion in this ledger needs re-reading: a divergence
    # could be one build failing to reproduce itself rather than the two builds
    # differing, and a matching pair could be coincidence.
    if args.repeat_log and pathlib.Path(args.repeat_log).exists():
        again = parse_report(pathlib.Path(args.repeat_log).read_text(errors="replace"))
        compared = ("map_digest", "starts_digest", "turn", "winner", "victory", "scores")
        differing = [f for f in compared if again.get(f) != row.get(f)]
        row["self_consistent"] = not differing
        if differing:
            notes.append(
                "THIS BUILD DID NOT REPRODUCE ITSELF on seed "
                f"{args.seed}: {', '.join(differing)} changed between two runs of the "
                "same binary. Every parity result here assumes it would."
            )

    # Whether a saved world comes back the same world. Only the world: a
    # reloaded game deliberately gets a fresh AI fleet, so its *continuation*
    # differs by design and is not what this asks about.
    if args.saveload:
        row["saveload"] = args.saveload
        got = {}
        log = pathlib.Path(args.saveload_log) if args.saveload_log else None
        if log and log.exists():
            text = log.read_text(errors="replace")
            try:
                got = json.loads(text[text.find("{"):])
            except (ValueError, json.JSONDecodeError):
                got = {}
        # Read whether it passed or failed, so a run of green cross-build
        # checks is countable rather than only visible when one breaks.
        if got.get("cross_build"):
            row["cross_build_save"] = got["cross_build"]
        if got.get("save_bytes"):
            row["save_bytes"] = got["save_bytes"]

        crossed = str(got.get("cross_build") or "")
        if crossed and crossed != "ok" and not crossed.startswith("skipped"):
            notes.append(
                "THE OTHER BUILD'S SAVE DID NOT LOAD THE SAME: " + crossed
                + (f" ({', '.join(got['cross_build_differing'])})"
                   if got.get("cross_build_differing") else "")
                + (f" — {got['cross_build_error']}" if got.get("cross_build_error") else "")
            )
        elif args.saveload != "ok":
            detail = ""
            if got.get("differing"):
                detail = f": {', '.join(got['differing'])} changed"
            elif got.get("error"):
                detail = f": {got['error']}"
            notes.append(f"A SAVED WORLD DID NOT COME BACK THE SAME{detail}")

    # Whether civvis.ai could still be assembled from this revision. Nothing
    # in CI does this — the site workflow is manual on purpose — so a break in
    # the viewer's asset rewrites or a bundle over budget would otherwise
    # surface at a publish rather than at the commit that caused it.
    if args.publish:
        row["publish"] = args.publish
        if args.publish_bytes:
            row["publish_bytes"] = args.publish_bytes
            budget = 26214400
            row["publish_pct_of_budget"] = round(100 * args.publish_bytes / budget, 1)
            if args.publish_bytes > budget * 0.9:
                notes.append(
                    f"the published bundle is {args.publish_bytes:,} bytes, "
                    f"{row['publish_pct_of_budget']}% of the 25 MiB budget that fails the build"
                )
        if args.publish != "ok":
            notes.append(
                "THE SITE NO LONGER BUILDS: `beta/publish.sh` failed on this revision. "
                "Nothing in CI runs it, so this would next have been seen at a publish."
            )

    # What `wasm-opt` bought. The published bundle has a hard byte budget, so
    # the shrink ratio is worth a number rather than a shrug.
    if args.raw_wasm_bytes and row.get("wasm_bytes"):
        row["raw_wasm_bytes"] = args.raw_wasm_bytes
        row["shrunk_pct"] = round(100 - 100 * row["wasm_bytes"] / args.raw_wasm_bytes, 1)
        # `beta/publish.sh` fails the build over a 25 MiB bundle, and the module
        # is the part of it that grows every week. Noticing here costs nothing
        # and is a great deal earlier than noticing at a publish.
        previous = None
        if pathlib.Path(args.ledger).exists():
            for line in reversed(pathlib.Path(args.ledger).read_text(errors="replace").splitlines()):
                try:
                    past = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if past.get("wasm_bytes"):
                    previous = past
                    break
        if previous:
            grew = row["wasm_bytes"] - previous["wasm_bytes"]
            if abs(grew) > previous["wasm_bytes"] * 0.02:
                notes.append(
                    f"the module {'grew' if grew > 0 else 'shrank'} "
                    f"{abs(grew):,} bytes to {row['wasm_bytes']:,} "
                    f"(was {previous['wasm_bytes']:,} at {previous.get('sha')})"
                )

    # What a green exit status hides.
    if row.get("panic"):
        notes.append(f"the module died: {row['panic']}")
    if row.get("error"):
        notes.append(row["error"])
    dead = [s["civ"] for s in row.get("scores", []) if s.get("eliminated")]
    if dead:
        notes.append(f"eliminated: {', '.join(dead)}")
    if args.build_warnings:
        notes.append(f"{args.build_warnings} build warning(s)")

    # Throughput against this arm's own history — the comparison that holds,
    # because the two arms are not the same machine code and never will be.
    ledger = pathlib.Path(args.ledger)
    history = []
    if ledger.exists():
        for line in ledger.read_text(errors="replace").splitlines():
            try:
                past = json.loads(line)
            except json.JSONDecodeError:
                continue
            # Same arm *and* same board. Configurations differ in how much work
            # a turn is — ten majors on `crowded` move far more than six on
            # `baseline` — so a best-of-all-configs baseline flagged every
            # heavy board as a regression. Six of eight lines in the report
            # were that, and noise at that rate is how a real one gets skipped.
            if (
                past.get("arm") == args.arm
                and past.get("config", "") == args.config
                and past.get("turns_per_second")
            ):
                history.append(past)
    if history and row.get("turns_per_second"):
        row["best_turns_per_second"] = max(p["turns_per_second"] for p in history)
        # Against the *same seed*, because different seeds are different
        # workloads. Seeds 1002 and 1003 differ by a fifth on the same binary
        # simply because one game has more units to move, and comparing across
        # them reported a regression on a build that had not changed a line.
        # Same seed, different revision, is the comparison that means the
        # program got slower.
        same_seed = [p for p in history if p.get("seed") == args.seed]
        # On CPU time, not the wall clock. This box runs two live Civ 6 games
        # and several agents, and load swings between about 6 and 25; seed 1036
        # produced the *identical* game twice and read 4.294 turns/s at the low
        # end and 2.972 at the high one. Both arms fell together, which is a
        # busy machine and not a revision. CPU seconds do not move with load.
        # Same seed and same board are not enough: an engine change can alter
        # the game itself, and seed 1032 went from turn 185 by science to turn
        # 134 by religion between two revisions. Comparing those two is
        # comparing different amounts of work. Only games that came out the
        # same are comparable.
        past_cpu = [
            p for p in same_seed
            if p.get("turns_per_cpu_second")
            and p.get("turn") == row.get("turn")
            and p.get("scores") == row.get("scores")
        ]
        if past_cpu and row.get("turns_per_cpu_second"):
            was = max(p["turns_per_cpu_second"] for p in past_cpu)
            row["same_seed_best_cpu"] = was
            # 25%, because the observed floor is not far off 11%. Two runs of
            # the *identical* game on this machine measured 13.241 and 11.785
            # turns/cpu-s. CPU time is far better than the wall clock here, but
            # it is not load-proof either: these cores are not all the same
            # kind, and a run pushed onto the efficiency cores spends more CPU
            # seconds doing exactly the same work. A threshold under the noise
            # floor is a generator of false alarms, and this loop has produced
            # enough of those already.
            if row["turns_per_cpu_second"] < was * 0.75:
                # Even CPU seconds are not load-proof on this machine: the cores
                # are not all the same kind, and a run pushed onto the
                # efficiency cores spends more of them doing identical work.
                # Seed 1036 read 6.828/cpu-s at load 2.5 and 4.486 at load 35 —
                # same revision, same agent, same turn 214 — which is a 34%
                # "regression" that is entirely the box. So the loads have to be
                # in the same country before a claim is worth making.
                best_row = max(past_cpu, key=lambda p: p["turns_per_cpu_second"])
                then, now = best_row.get("load") or 0, args.load or 0
                comparable = not (then and now) or now <= then * 2.5
                fell = round(100 - 100 * row["turns_per_cpu_second"] / was)
                if comparable:
                    notes.append(
                        f"CPU throughput on seed {args.seed} fell to "
                        f"{row['turns_per_cpu_second']}/cpu-s from {was}/cpu-s "
                        f"({fell}% down) — same seed, same board, same game, and "
                        f"the box was no busier (load {now} vs {then})"
                    )
                else:
                    row["throughput_not_compared"] = (
                        f"{fell}% under best, but load was {now} against {then}"
                    )
        elif same_seed:
            row["same_seed_best"] = max(p["turns_per_second"] for p in same_seed)
        elif row.get("turns_per_cpu_second"):
            # No same-seed history yet, but the board matches. In CPU seconds
            # like the check above — a fallback left on the wall clock is the
            # half of the comparison load can still move, which is the whole
            # reason the primary one moved off it.
            board_cpu = [p["turns_per_cpu_second"] for p in history
                         if p.get("turns_per_cpu_second")]
            # A "best" drawn from one or two runs is not a best, it is the only
            # number there is — and a board's seeds vary by more than 2x anyway.
            # `true-start-earth` fired this on its second seed ever, against a
            # best set by its first. Wait until the board has some history.
            if len(board_cpu) >= 4 and row["turns_per_cpu_second"] < max(board_cpu) * 0.5:
                notes.append(
                    f"CPU throughput {row['turns_per_cpu_second']}/cpu-s is less than half "
                    f"the best {max(board_cpu)}/cpu-s seen on "
                    f"{args.config or 'this board'} (different seed — may just be the game)"
                )

    # Peak wasm memory, on the same board, same seed, and the same game — the
    # only comparison that means anything, exactly as for throughput. The
    # module's memory only ever grows, so this reading is both peak and final.
    #
    # Never checked until now, and the ledger had something to say: identical
    # games gained 8-17% across iterations ~250-690, then went flat for the 850
    # after. A one-time step rather than a leak, and 94 MiB against wasm32's
    # 4 GiB is not close to anything — but nothing would have noticed either
    # way, which is the part worth fixing.
    if row.get("peak_wasm_mib") and row.get("scores"):
        alike = [
            p for p in history
            if p.get("peak_wasm_mib")
            and p.get("seed") == args.seed
            and p.get("turn") == row.get("turn")
            and p.get("scores") == row.get("scores")
        ]
        if len(alike) >= 4:
            was = min(p["peak_wasm_mib"] for p in alike)
            row["same_game_min_mib"] = was
            if row["peak_wasm_mib"] > was * 1.25:
                notes.append(
                    f"the module now needs {row['peak_wasm_mib']:.1f} MiB for a game that "
                    f"used {was:.1f} — same board, same seed, same result, so the world is "
                    f"the same size ({round(100 * row['peak_wasm_mib'] / was - 100)}% more)"
                )

    # The paired arm, same seed: two builds of one program asked for one game.
    if ledger.exists():
        for line in reversed(ledger.read_text(errors="replace").splitlines()):
            try:
                past = json.loads(line)
            except json.JSONDecodeError:
                continue
            # Same seed, other arm, and the *same revision* — two builds of one
            # program. A pair spanning a merge is two programs, and reporting
            # that as the builds disagreeing would be the loop's own fault.
            if (
                past.get("seed") == args.seed
                and past.get("arm") != args.arm
                and past.get("sha") == args.sha
                and past.get("config", "") == args.config
                and past.get("ok")
            ):
                # Which agent sat down has to match before a difference in
                # *play* can be blamed on the build. Since #1094 it often does
                # not: the module seats from a shipped league and the native
                # binary seats `advanced`, and there is no way through the API
                # to align them — `/host-league` refuses an empty roster and
                # `/new` does not take a strategy.
                #
                # Going blind would be the wrong answer. The map and the
                # starting positions are built before any agent acts, so they
                # are still exactly comparable, and mapgen is where the one
                # standing engine divergence lives (#1061). Compare what is
                # still valid and say plainly what was not.
                seats_agree = past.get("seat_strategy") == row.get("seat_strategy")
                if not seats_agree:
                    row["seats_differed"] = [row.get("seat_strategy"), past.get("seat_strategy")]
                row["paired_with"] = {
                    "arm": past["arm"],
                    "iteration": past["iteration"],
                    "turn": past.get("turn"),
                    "winner": past.get("winner"),
                }
                if not row["ok"]:
                    break
                # Both arms are handed the same fully-named setup and driven
                # through the same routes, so the same seed has to be the same
                # game. Anything below is wasm32 itself — 32-bit `usize`, a
                # different allocator, a different float lowering — and that is
                # exactly what alternating the builds is for.
                divergence = []
                if not seats_agree:
                    # Only the world before anybody played it.
                    if row.get("map_digest") and past.get("map_digest"):
                        if row["map_digest"] != past["map_digest"]:
                            divergence.append("THE MAP ITSELF differs, before a turn is played")
                        elif row.get("starts_digest") != past.get("starts_digest"):
                            divergence.append("same map, but the empires were placed differently")
                    if divergence:
                        row["diverged_from_pair"] = divergence
                        notes.append(
                            f"DIVERGED from {past['arm']} on seed {args.seed}: "
                            + "; ".join(divergence)
                        )
                    else:
                        notes.append(
                            "map and starts match; the game itself was NOT COMPARED because the "
                            f"builds seated different agents ({row.get('seat_strategy')} vs "
                            f"{past.get('seat_strategy')})"
                        )
                    break
                for field in ("turn", "winner", "victory"):
                    if past.get(field) != row.get(field):
                        divergence.append(f"{field} {row.get(field)} vs {past.get(field)}")
                here = {s.get("civ"): s.get("score") for s in row.get("scores", [])}
                there = {s.get("civ"): s.get("score") for s in past.get("scores", [])}
                if here and there and here != there:
                    disagreed = sorted(set(here) | set(there))
                    shown = [
                        f"{civ} {here.get(civ)}/{there.get(civ)}"
                        for civ in disagreed
                        if here.get(civ) != there.get(civ)
                    ]
                    divergence.append("scores " + ", ".join(shown[:6]))
                # Where it happened — but only once something has *actually*
                # diverged. Written above the field checks this said "same map
                # and same starts" on every identical pair, and since it was
                # appended to the same list that decides whether to report at
                # all, eight consecutive matching pairs were filed as
                # divergences in the tier that is meant to hold only real ones.
                # The location is a description of a divergence, so it cannot
                # be what creates one.
                if divergence:
                    if row.get("map_digest") and past.get("map_digest"):
                        if row["map_digest"] != past["map_digest"]:
                            where = "THE MAP ITSELF differs, before a turn is played"
                        elif row.get("starts_digest") != past.get("starts_digest"):
                            where = "same map, but the empires were placed differently"
                        else:
                            where = "same map and same starts — the difference is in the simulation"
                        divergence.insert(0, where)
                    row["diverged_from_pair"] = divergence
                    notes.append(
                        f"DIVERGED from {past['arm']} on seed {args.seed}: " + "; ".join(divergence)
                    )
                break

    row["notes"] = notes
    pathlib.Path(args.result).write_text(json.dumps(row, indent=2) + "\n")
    with ledger.open("a") as out:
        out.write(json.dumps(row) + "\n")

    print(f"   -> {args.result}")
    for note in notes:
        print(f"   ! {note}")


if __name__ == "__main__":
    main()
