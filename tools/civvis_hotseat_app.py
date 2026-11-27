#!/usr/bin/env python3
"""The hotseat companion as one program: the harness, its workers, a doctor, a self-check.

    civvis-hotseat [options]          play: tools/civ6_hotseat.py (--help lists every option)
    civvis-hotseat doctor [--json]    what this computer has: the game, its folders, the
                                      control mod, the game process, this bundle
    civvis-hotseat selfcheck          prove the bundle with no game running: install the
                                      mod into a scratch tree, relay a recorded board through
                                      a scratch Automation.log, and have a real decision
                                      worker answer it into SQLite
    civvis-hotseat uninstall          remove the control mod from the game (Civ VI closed)
    civvis-hotseat civ6_brain.py ...  (internal) one CIVVIS seat's decision worker

`tools/package_hotseat.py` freezes this file with PyInstaller. From a checkout it
runs the same way: `python3 tools/civvis_hotseat_app.py doctor`.

Why one program. `tools/civ6_hotseat.py` starts one worker per CIVVIS seat as
`sys.executable civ6_brain.py ...`. In a frozen bundle `sys.executable` is this
program and there is no interpreter to hand a script to, so the harness asks for
`<this program> civ6_brain.py ...` instead (`civ6_hotseat.brain_command`) and this
file routes it. The literal `civ6_brain.py` stays in the worker's command line on
purpose: the stale-worker check and the game lock recognise a worker by it.

Where the bundle's files are. A module frozen into the bundle has its `__file__`
under `sys._MEIPASS` at the same relative path it has under `tools/`, so the
bundle carries the mod at `civ6_control/mod/` and every `Path(__file__)` lookup in
the tools resolves to it unchanged. `doctor` prints the resolved paths and
`selfcheck` installs from them.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import sqlite3
import sys
import tempfile
import time
import traceback
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE / "civ6_control"))

FROZEN = bool(getattr(sys, "frozen", False))
#: Where the bundle's data sits: `sys._MEIPASS` frozen, `tools/` from a checkout.
BUNDLE_ROOT = Path(getattr(sys, "_MEIPASS", HERE))
#: Written by `tools/package_hotseat.py`: what the bundle was built from.
MANIFEST = BUNDLE_ROOT / "bundle.json"
#: A recorded opening board (a real hotseat seat's events up to its first
#: `state`, run tag and orders path scrubbed) for `selfcheck`.
FIXTURE = HERE / "fixtures" / "hotseat_selfcheck_events.jsonl"
#: Must equal `civ6_hotseat.WORKER_COMMAND`; imported lazily there, so a
#: worker start does not pay for the harness (test_civvis_hotseat_app pins it).
WORKER_COMMAND = "civ6_brain.py"
SEAT = 1

WINDOWS_WARNING = (
    "WINDOWS: this bundle builds and passes its self-check on Windows, but nothing "
    "about the game itself has been measured on Windows -- where Civilization VI "
    "looks for mods, whether it writes Logs\\Automation.log, whether the mod's "
    "ATTACH DATABASE works, the Hot Seat menus. Treat a game here as an experiment; "
    "docs/CIV6_HOTSEAT_RELEASE.md lists what to measure.")


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    command = argv[0] if argv else ""
    if command == WORKER_COMMAND:
        return run_worker(argv[1:])
    if command in ("doctor", "--doctor"):
        return doctor(argv[1:])
    if command == "selfcheck":
        return selfcheck(argv[1:])
    if command == "uninstall":
        return uninstall(argv[1:])
    if command in ("-h", "--help"):
        print(__doc__.split("\n\n")[1] + "\n")
    import civ6_hotseat  # noqa: PLC0415 - only the harness pays for its imports
    return civ6_hotseat.main(argv)


def run_worker(argv: list[str]) -> int:
    """`civ6_brain.py`, as the harness started it."""
    import civ6_brain  # noqa: PLC0415
    sys.argv = [WORKER_COMMAND, *argv]
    return civ6_brain.main()


def uninstall(argv: list[str]) -> int:
    """Take the control mod back out of the game. The harness does this itself
    when it ends with Civ VI closed; this is for when it could not."""
    argparse.ArgumentParser(prog="civvis-hotseat uninstall",
                            description=uninstall.__doc__).parse_args(argv)
    import civ6_env as env  # noqa: PLC0415
    from civ6_control import install as modinstall  # noqa: PLC0415

    if env.game_pids():
        print("Civilization VI is running; quit it first (the mod is in use)", file=sys.stderr)
        return 1
    target = modinstall.install_dir()
    print(f"removed {target}" if modinstall.uninstall() else f"not installed ({target})")
    return 0


# ------------------------------------------------------------------ doctor

def bundle_contents() -> dict:
    from civ6_control import install as modinstall  # noqa: PLC0415
    import civ6_hotseat  # noqa: PLC0415

    mod = modinstall.MOD_SOURCE
    shipped = sorted(p.name for p in mod.iterdir()) if mod.is_dir() else []
    orders = civ6_hotseat.BUNDLED_BIN
    manifest = None
    if MANIFEST.is_file():
        try:
            manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        except ValueError:
            manifest = {"error": "bundle.json does not parse"}
    return {
        "frozen": FROZEN,
        "program": sys.executable,
        "root": str(BUNDLE_ROOT),
        "mod_source": str(mod),
        "mod_files": shipped,
        "mod_scripts_present": all(name in shipped for name in modinstall.SCRIPTS),
        "civvis_orders": str(orders) if orders else None,
        "civvis_orders_present": bool(orders and orders.is_file()),
        "selfcheck_fixture": str(FIXTURE) if FIXTURE.is_file() else None,
        "manifest": manifest,
    }


def _file_facts(path: Path) -> dict:
    facts = {"path": str(path), "exists": path.exists()}
    if path.is_file():
        stat = path.stat()
        facts.update(bytes=stat.st_size, modified=datetime.fromtimestamp(
            stat.st_mtime, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"))
    return facts


def doctor_report() -> dict:
    import civ6_env as env  # noqa: PLC0415
    import civ6_hotseat  # noqa: PLC0415
    from civ6_control import install as modinstall  # noqa: PLC0415

    report: dict = {
        "platform": f"{platform.system()} {platform.release()} ({platform.machine()})",
        "python": sys.version.split()[0],
        "utf8_mode": bool(sys.flags.utf8_mode),
        "windows": env.on_windows(),
        "bundle": bundle_contents(),
        "notes": [],
    }
    try:
        report["install_dir"] = str(env.install_dir())
        report["assets_dir"] = str(env.assets_dir())
    except SystemExit as missing:
        report["install_dir"] = None
        report["install_error"] = str(missing)
    if env.on_windows():
        report["steam_libraries"] = [str(library) for root in env.windows_steam_roots()
                                     for library in env.steam_libraries(root)]
        report["user_dir_candidates"] = [str(c) for c in env.windows_user_dir_candidates()]
    user = env.user_dir()
    report["user_dir"] = _file_facts(user)
    report["app_options"] = _file_facts(env.app_options_path())
    report["log_file"] = _file_facts(env.logs_dir() / "Automation.log")
    report["mods_dir"] = _file_facts(env.mods_dir())
    report["mod_locations"] = [{"name": name, "path": str(path) if path else None,
                                "evidence": evidence}
                               for name, path, evidence in env.mod_install_candidates()]
    try:
        target = modinstall.install_dir()
        installed = modinstall.installed_config()
        report["mod"] = {"install_dir": str(target), "installed": target.is_dir(),
                         "run_tag": (installed or {}).get("RunTag"),
                         "hotseat": bool((installed or {}).get("Hotseat"))}
    except SystemExit as missing:
        report["mod"] = {"install_dir": None, "installed": False, "error": str(missing)}
    pids = env.game_pids()
    report["game_running"] = bool(pids)
    report["game_pids"] = pids
    current = civ6_hotseat.read_current()
    report["harness"] = {"runs": str(civ6_hotseat.RUN_ROOT), "current": current,
                         "running": bool(current and civ6_hotseat.harness_alive(
                             current.get("pid", -1)))}
    if not report["utf8_mode"] and env.on_windows():
        report["notes"].append("Python is not in UTF-8 mode: the mod's sources are UTF-8 "
                               "and will not read under the Windows code page "
                               "(set PYTHONUTF8=1, or use the bundle, which sets it)")
    if not env.on_windows() and sys.platform == "darwin" and not shutil.which("swiftc"):
        report["notes"].append("no swiftc on PATH: the automatic, screen-read game setup "
                               "cannot run here; set the game up by hand (--manual-lobby)")
    if "'" in str(civ6_hotseat.RUN_ROOT):
        report["notes"].append(f"the runs folder {civ6_hotseat.RUN_ROOT} holds an apostrophe, "
                               "which the mod strips from the path it attaches: set "
                               "CIVVIS_HOTSEAT_RUNS to a folder without one")
    if env.on_windows():
        report["notes"].append(WINDOWS_WARNING)
    return report


def print_report(report: dict) -> None:
    def line(label: str, value: object) -> None:
        print(f"{label:<18} {value}")

    def facts(label: str, entry: dict) -> None:
        detail = "present" if entry.get("exists") else "MISSING"
        if "bytes" in entry:
            detail += f", {entry['bytes']} bytes, modified {entry['modified']}"
        line(label, f"{entry['path']}  ({detail})")

    if report["windows"]:
        print(f"!! {WINDOWS_WARNING}\n")
    line("platform", f"{report['platform']}, Python {report['python']}, "
                     f"UTF-8 mode {'on' if report['utf8_mode'] else 'off'}")
    bundle = report["bundle"]
    line("program", f"{bundle['program']} ({'bundle' if bundle['frozen'] else 'checkout'})")
    line("game install", report.get("install_dir") or f"NOT FOUND: {report.get('install_error')}")
    for library in report.get("steam_libraries", []):
        line("  steam library", library)
    facts("user dir", report["user_dir"])
    facts("AppOptions.txt", report["app_options"])
    facts("log file", report["log_file"])
    facts("user Mods", report["mods_dir"])
    mod = report["mod"]
    line("control mod", f"{mod.get('install_dir')}  ("
                        + ("installed" + (f", run {mod['run_tag']}" if mod.get("run_tag") else "")
                           if mod.get("installed") else "not installed") + ")")
    for location in report["mod_locations"]:
        line(f"  {location['name']}", f"{location['path']}: {location['evidence']}")
    line("game running", f"yes, pid {report['game_pids']}" if report["game_running"] else "no")
    harness = report["harness"]
    line("harness", ("running: " + str((harness["current"] or {}).get("tag")))
         if harness["running"] else f"not running (runs in {harness['runs']})")
    line("mod source", f"{bundle['mod_source']} ({len(bundle['mod_files'])} files, "
                       f"scripts {'complete' if bundle['mod_scripts_present'] else 'MISSING'})")
    if bundle["frozen"]:
        line("civvis_orders", f"{bundle['civvis_orders']} ("
                              f"{'present' if bundle['civvis_orders_present'] else 'MISSING'})")
    manifest = bundle.get("manifest") or {}
    if manifest:
        line("built", f"{manifest.get('revision')} on {manifest.get('platform')} "
                      f"at {manifest.get('built_utc')} (PyInstaller {manifest.get('pyinstaller')})")
    for note in report["notes"]:
        if note != WINDOWS_WARNING:
            print(f"note: {note}")
    if report["windows"]:
        print(f"\n!! {WINDOWS_WARNING}")


def doctor(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(prog="civvis-hotseat doctor",
                                 description="Print what this computer has for a hotseat game.")
    ap.add_argument("--json", action="store_true", help="machine-readable")
    args = ap.parse_args(argv)
    report = doctor_report()
    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True, default=str))
    else:
        print_report(report)
    return 0


# --------------------------------------------------------------- selfcheck

class Failed(Exception):
    pass


class _Settings(argparse.Namespace):
    """The harness's settings for the self-check's mod config. Anything not set
    here reads as None, so a setting the harness grows later is off, not a crash."""

    def __getattr__(self, name: str):
        if name.startswith("__"):
            raise AttributeError(name)
        return None


def selfcheck(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(
        prog="civvis-hotseat selfcheck",
        description="Prove the bundle end to end without a game: mod install into a "
                    "scratch install tree, the Automation.log relay, a real decision "
                    "worker answering a recorded board into SQLite, worker stop, game "
                    "lock. Touches nothing outside its scratch directory.")
    ap.add_argument("--events", type=Path, default=FIXTURE,
                    help="a seat's events.jsonl from a real game; read up to its newest "
                         "`state` board (default: the recorded fixture)")
    ap.add_argument("--orders-bin", default=None,
                    help="civvis_orders to decide with (default: the bundle's own)")
    ap.add_argument("--dir", type=Path, default=None,
                    help="scratch directory (default: a new temporary one)")
    ap.add_argument("--timeout", type=float, default=180.0,
                    help="seconds the worker gets to answer (default %(default)s)")
    ap.add_argument("--keep", action="store_true", help="keep the scratch directory")
    args = ap.parse_args(argv)

    root = (args.dir or Path(tempfile.mkdtemp(prefix="civvis-selfcheck-"))).absolute()
    root.mkdir(parents=True, exist_ok=True)
    # Every location the harness reads or writes moves under `root` BEFORE the
    # modules that read some of them at import time are imported.
    for name in ("install", "user"):
        (root / name).mkdir(exist_ok=True)
    os.environ.update({
        "CIV6_INSTALL": str(root / "install"),
        "CIV6_USER_DIR": str(root / "user"),
        "CIVVIS_GAME_LOCK_DIR": str(root / "game.lock"),
        "CIVVIS_OPERATOR_HALT_FILE": str(root / "operator-halt.json"),
        "CIVVIS_HOTSEAT_RUNS": str(root / "runs"),
    })
    os.environ.pop("CIV6_DIR", None)
    results: list[tuple[bool, str, str]] = []
    state: dict = {}

    def step(name: str, check) -> None:
        try:
            detail = check() or ""
            results.append((True, name, detail))
            print(f"PASS  {name}: {detail}", flush=True)
        except Exception as error:  # noqa: BLE001 - every step reports, none aborts the rest
            message = f"{type(error).__name__}: {error}"
            results.append((False, name, message))
            print(f"FAIL  {name}: {message}", flush=True)
            if not isinstance(error, Failed):
                traceback.print_exc()

    print(f"selfcheck in {root} ({'bundle' if FROZEN else 'checkout'}, "
          f"{platform.system()} {platform.machine()}, Python {sys.version.split()[0]}, "
          f"UTF-8 mode {'on' if sys.flags.utf8_mode else 'off'})", flush=True)
    step("imports", lambda: _check_imports(state, root))
    step("mod install", lambda: _check_mod_install(state, root))
    step("log relay", lambda: _check_log_relay(state, root, args.events))
    step("decision worker", lambda: _check_worker(state, root, args.orders_bin, args.timeout))
    step("worker stop", lambda: _check_worker_stop(state))
    step("game lock", lambda: _check_lock(state, root))
    step("processes", lambda: _check_processes(state))
    if "brains" in state:
        state["brains"].stop()
    passed = sum(ok for ok, _, _ in results)
    print(f"selfcheck: {passed} of {len(results)} checks passed", flush=True)
    ok = passed == len(results)
    if ok and not args.keep and args.dir is None:
        shutil.rmtree(root, ignore_errors=True)
    else:
        print(f"scratch directory kept: {root}")
    return 0 if ok else 1


def _need(condition: bool, message: str) -> None:
    if not condition:
        raise Failed(message)


def _check_imports(state: dict, root: Path) -> str:
    import civ6_brain  # noqa: F401, PLC0415 - the worker's closure
    import civ6_env as env  # noqa: PLC0415
    import civ6_hotseat  # noqa: PLC0415
    from civ6_control import gamelock, watch  # noqa: F401, PLC0415
    from civ6_control import install as modinstall  # noqa: PLC0415

    state.update(env=env, hotseat=civ6_hotseat, modinstall=modinstall)
    _need(env.install_dir() == root / "install", f"install resolved to {env.install_dir()}")
    _need(env.user_dir() == root / "user", f"user dir resolved to {env.user_dir()}")
    _need(gamelock.LOCK == root / "game.lock", f"game lock is {gamelock.LOCK}")
    shipped = sorted(p.name for p in modinstall.MOD_SOURCE.glob("*"))
    missing = [name for name in modinstall.SCRIPTS if name not in shipped]
    _need(not missing, f"mod source {modinstall.MOD_SOURCE} lacks {missing}")
    return f"mod source {modinstall.MOD_SOURCE} ({len(shipped)} files)"


def _check_mod_install(state: dict, root: Path) -> str:
    hotseat, modinstall = state["hotseat"], state["modinstall"]
    tag = "selfcheck"
    run_dir = root / "runs" / tag
    run_dir.mkdir(parents=True, exist_ok=True)
    settings = _Settings(tag=tag, difficulty="DIFFICULTY_PRINCE", map="Pangaea.lua",
                         map_size="MAPSIZE_DUEL", speed="GAMESPEED_ONLINE", max_turns=250,
                         victory="civvis", leader=None, players=2, auto_human=None,
                         save_every=10)
    orders_db = run_dir / "orders.sqlite"
    config = hotseat.hotseat_config(settings, run_dir, orders_db)
    state.update(tag=tag, run_dir=run_dir, orders_db=orders_db)
    # What the mod's `sqlSafe` makes of the path before it ATTACHes it: every
    # apostrophe and backslash stripped (CivvisControlAgent.lua).
    attached = config["OrdersDb"].replace("'", "").replace("\\", "")
    _need(attached == config["OrdersDb"] and Path(attached) == orders_db,
          f"the mod would attach {attached!r}, not the worker's {str(orders_db)!r}")
    target = modinstall.install(config)
    _need(target.is_relative_to(root), f"installed outside the scratch tree: {target}")
    agent = target / f"{modinstall.HOTSEAT_AGENT.format(seat=SEAT)}.lua"
    _need(agent.is_file(), f"no seat agent {agent.name}")
    _need(f"HotseatSeat = {SEAT}," in agent.read_text(encoding="utf-8"),
          "the seat agent's settings prelude does not name its seat")
    modinfo = (target / "CivvisControl.modinfo").read_text(encoding="utf-8")
    _need(agent.with_suffix(".xml").name in modinfo, "modinfo does not list the seat context")
    installed = modinstall.installed_config() or {}
    _need(installed.get("RunTag") == tag, f"installed run tag {installed.get('RunTag')!r}")
    files = len(list(target.iterdir()))
    size = agent.stat().st_size
    _need(modinstall.uninstall() and not target.exists(), "uninstall left the mod behind")
    return f"{files} files at {target}, seat agent {size} bytes; removed again"


def _check_log_relay(state: dict, root: Path, events_path: Path) -> str:
    env, hotseat = state["env"], state["hotseat"]
    from civ6_control import install as modinstall  # noqa: PLC0415
    from civ6_control import watch  # noqa: PLC0415

    seat_tag = modinstall.hotseat_seat_tag(state["tag"], SEAT)
    events = [json.loads(line) for line in events_path.read_text(encoding="utf-8").splitlines()
              if line.strip()]
    boards = [index for index, event in enumerate(events) if event.get("kind") == "state"]
    _need(bool(boards), f"{events_path} holds no `state` board")
    # Up to the newest board only: a whole game's journal records that board's
    # turn as played, and a worker rightly answers nothing it has played.
    events = events[:boards[-1] + 1]
    log_path = env.logs_dir() / "Automation.log"
    log_path.parent.mkdir(parents=True, exist_ok=True)
    with log_path.open("w", encoding="utf-8", newline="\n") as log:
        for event in events:
            log.write("CIVVISJSON " + json.dumps(dict(event, run=seat_tag), sort_keys=True) + "\n")
    seat_dir = state["run_dir"] / seat_tag
    bridge = watch.EventLogBridge(seat_dir, tag=seat_tag)
    copied = bridge.pump()
    _need(copied == len(events), f"relayed {copied} of {len(events)} events")
    relayed = (seat_dir / "events.jsonl").read_text(encoding="utf-8").count('"kind":"state"')
    _need(relayed >= 1, "the state board did not reach events.jsonl")
    state.update(seat_dir=seat_dir, seat_tag=seat_tag, log=hotseat.Log(root / "selfcheck.log"))
    return f"{copied} events from {log_path} into {seat_dir / 'events.jsonl'}"


def _check_worker(state: dict, root: Path, orders_bin: str | None, timeout_s: float) -> str:
    hotseat = state["hotseat"]
    from civ6_control.orders import reset_orders_db  # noqa: PLC0415

    binary = hotseat.resolve_orders_bin(orders_bin)
    _need(binary is not None, "no civvis_orders (pass --orders-bin)")
    run_dir, orders_db = state["run_dir"], state["orders_db"]
    pinned = hotseat.freeze_binary(binary, run_dir)
    reset_orders_db(orders_db)
    brains = hotseat.Brains(run_dir, state["tag"], [SEAT], orders_db, pinned, "civvis",
                            state["log"])
    state["brains"] = brains
    started = time.monotonic()
    brains.start_all()
    proc = brains.procs[SEAT]
    state["worker_pid"] = proc.pid
    command = hotseat.brain_command(state["seat_dir"], orders_db, pinned, "civvis")
    ready = None
    while time.monotonic() - started < timeout_s:
        if proc.poll() is not None:
            raise Failed(f"the worker exited {proc.returncode}: {_tail(state['seat_dir'])}")
        with sqlite3.connect(str(orders_db), timeout=10) as db:
            row = db.execute("SELECT turn, count, frame FROM ready WHERE run = ?",
                             (state["seat_tag"],)).fetchone()
            if row is not None:
                ready = row
                orders = db.execute("SELECT kind, verb FROM orders WHERE run = ? AND turn = ?",
                                    (state["seat_tag"], row[0])).fetchall()
                break
        time.sleep(0.5)
    _need(ready is not None,
          f"no `ready` row after {timeout_s:.0f}s: {_tail(state['seat_dir'])}")
    _need(ready[1] == len(orders) and orders,
          f"`ready` says {ready[1]} orders, the table holds {len(orders)}")
    kinds = sorted({kind for kind, _ in orders})
    return (f"turn {ready[0]}: {len(orders)} orders ({', '.join(kinds)}) and a `ready` row in "
            f"{time.monotonic() - started:.1f}s from `{' '.join(command[:2])} ...` "
            f"deciding with {pinned.name}")


def _tail(seat_dir: Path, lines: int = 12) -> str:
    try:
        return " | ".join((seat_dir / "brain.log").read_text(
            encoding="utf-8", errors="replace").splitlines()[-lines:])
    except OSError:
        return "(no brain.log)"


def _check_worker_stop(state: dict) -> str:
    env, hotseat = state["env"], state["hotseat"]
    brains, pid = state.get("brains"), state.get("worker_pid")
    _need(brains is not None and pid is not None, "no worker was started")
    stopped = hotseat.stop_stale_worker(state["seat_dir"], grace_s=3.0)
    _need(stopped == pid, f"the stale-worker check did not recognise pid {pid} "
                          f"(command line: {env.process_command(pid)!r})")
    brains.procs[SEAT].wait(timeout=15)
    _need(not env.pid_alive(pid), f"pid {pid} still reads as alive after it exited")
    return f"pid {pid} recognised by its command line, stopped, and reads as gone"


def _check_lock(state: dict, root: Path) -> str:
    from civ6_control import gamelock  # noqa: PLC0415

    tag = state.get("tag", "selfcheck")
    _need(gamelock.acquire(tag, require_verification_intent=False,
                           honour_operator_halt=False), "could not take a free lock")
    held = gamelock.describe()
    _need("running" in held, f"our own lock reads {held!r}")
    gamelock.release()
    _need(not gamelock.LOCK.exists(), "release left the lock")
    dead = state.get("worker_pid")
    _need(dead is not None, "no exited pid to plant")
    gamelock.LOCK.mkdir()
    (gamelock.LOCK / "holder.json").write_text(json.dumps({"pid": dead, "tag": "gone"}))
    _need(gamelock.acquire(tag, require_verification_intent=False, honour_operator_halt=False),
          f"a lock held by exited pid {dead} was not taken over")
    gamelock.release()
    return f"free lock taken and released; a stale one (pid {dead}) taken over at {gamelock.LOCK}"


def _check_processes(state: dict) -> str:
    env, hotseat = state["env"], state["hotseat"]
    _need(env.pid_alive(os.getpid()), "this process reads as not alive")
    details = []
    if env.on_windows():
        names = dict(env.windows_processes())
        mine = names.get(os.getpid())
        _need(mine is not None and mine.lower() == Path(sys.executable).name.lower(),
              f"the process table names this process {mine!r}, not {Path(sys.executable).name}")
        details.append(f"process table names this process {mine}")
    if FROZEN:
        command = env.process_command(os.getpid())
        _need(hotseat.harness_alive(os.getpid()),
              f"--status/--stop would not recognise this program: {command!r}")
        details.append("--status/--stop recognise this program as a harness")
    pids = env.game_pids()
    details.append(f"Civilization VI {'running, pid ' + str(pids) if pids else 'not running'}")
    return "; ".join(details)


if __name__ == "__main__":
    raise SystemExit(main())
