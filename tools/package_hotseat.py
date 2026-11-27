#!/usr/bin/env python3
"""Build the hotseat companion: one folder a player downloads and runs.

    python3 tools/package_hotseat.py --orders-bin target/release/civvis_orders
    python3 tools/package_hotseat.py --build-orders              # cargo build it first
    python3 tools/package_hotseat.py --orders-bin PATH --archive  # and zip the folder

The folder is target/hotseat-bundle/dist/civvis-hotseat/: the program
`civvis-hotseat` (`.exe` on Windows), its `_internal/` runtime, and README.md
(docs/CIV6_HOTSEAT_RELEASE.md). `--archive` also writes
target/hotseat-bundle/civvis-hotseat-<os>-<arch>.zip. The workflow
`.github/workflows/hotseat-bundle.yml` builds it on macOS and Windows and runs it.

What goes in, each found rather than listed:

* The Python: `tools/civvis_hotseat_app.py` and everything it imports, found by
  PyInstaller's analysis over `tools/` and `tools/civ6_control/` (the harness,
  `civ6_brain.py` and their whole closure). Nothing outside the standard
  library: Pillow, which only the screen-reading setup uses, is not installed
  in the build environment and so is not collected.
* The control mod: every file in `tools/civ6_control/mod/` except `*_test.lua`
  (those run under lua5.1 in the test workflow, never in the game). The build
  stops if the scripts `install.py` writes are not among them.
* `civvis_orders`: the binary given. It reads nothing at run time but the seat's
  run directory: the rules, the leader roster and the champion genome are
  compiled in (`include_str!`), and its one working-directory read (`evolved/`,
  then `data/evolved/`) falls back to the compiled-in champion. Measured
  2026-11-27 on the recorded board: byte-identical orders from an empty working
  directory and from a checkout. The harness starts workers with
  `--github-refresh-seconds 0`, so `civ6_brain.py`'s GitHub refresh never runs
  and the bundle needs no network.
* The self-check's recorded board, `tools/fixtures/hotseat_selfcheck_events.jsonl`.
* `bundle.json`: revision, platform, tool versions and every input's sha256.

PyInstaller is pinned below and installed into a private virtual environment
under the work directory, so it never becomes a requirement of the repository.
The program runs in Python's UTF-8 mode (`X utf8`): the mod's sources are UTF-8,
and under the Windows code page (cp1252) CivvisControlAgent.lua does not even
decode (byte 0x90), so `install.py` would fail on its first read.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

PYINSTALLER = "6.22.3"  # 2026-09-12
REPO = Path(__file__).resolve().parent.parent
TOOLS = REPO / "tools"
MOD = TOOLS / "civ6_control" / "mod"
APP = TOOLS / "civvis_hotseat_app.py"
FIXTURE = TOOLS / "fixtures" / "hotseat_selfcheck_events.jsonl"
GUIDE = REPO / "docs" / "CIV6_HOTSEAT_RELEASE.md"
NAME = "civvis-hotseat"
DEFAULT_WORK = REPO / "target" / "hotseat-bundle"
ORDERS_NAME = "civvis_orders.exe" if os.name == "nt" else "civvis_orders"


def mod_files() -> list[Path]:
    """The mod as the game sees it: everything but the Lua test files."""
    sys.path.insert(0, str(TOOLS))
    from civ6_control import install  # noqa: PLC0415

    files = sorted(path for path in MOD.iterdir()
                   if path.is_file() and not path.name.endswith("_test.lua"))
    names = {path.name for path in files}
    missing = [name for name in (*install.SCRIPTS, "CivvisControl.modinfo") if name not in names]
    if missing:
        raise SystemExit(f"{MOD} lacks {missing}; the bundle would install a broken mod")
    return files


def platform_tag() -> str:
    system = {"darwin": "macos", "win32": "windows"}.get(sys.platform, sys.platform)
    machine = platform.machine().lower()
    machine = {"amd64": "x86_64", "aarch64": "arm64"}.get(machine, machine)
    return f"{system}-{machine}"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_revision() -> str:
    try:
        head = subprocess.run(["git", "-C", str(REPO), "rev-parse", "HEAD"], capture_output=True,
                              text=True, timeout=30).stdout.strip()
        dirty = subprocess.run(["git", "-C", str(REPO), "status", "--porcelain", "--",
                                "tools", "docs/CIV6_HOTSEAT_RELEASE.md"],
                               capture_output=True, text=True, timeout=30).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        return "unknown"
    return (head or "unknown") + ("-dirty" if dirty else "")


def build_orders() -> Path:
    """`cargo build --release --locked --bin civvis_orders` in this checkout."""
    subprocess.run(["cargo", "build", "--release", "--locked", "--bin", "civvis_orders"],
                   cwd=REPO, check=True)
    return REPO / "target" / "release" / ORDERS_NAME


def pyinstaller_python(work: Path) -> Path:
    """A private virtual environment holding exactly the pinned PyInstaller."""
    venv = work / "venv"
    python = venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
    if not python.is_file():
        subprocess.run([sys.executable, "-m", "venv", str(venv)], check=True)
    shown = subprocess.run([str(python), "-m", "pip", "show", "pyinstaller"],
                           capture_output=True, text=True).stdout
    if f"Version: {PYINSTALLER}" not in shown:
        subprocess.run([str(python), "-m", "pip", "install", "--disable-pip-version-check",
                        "--quiet", f"pyinstaller=={PYINSTALLER}"], check=True)
    return python


def folder_bytes(folder: Path) -> int:
    return sum(path.stat().st_size for path in folder.rglob("*")
               if path.is_file() and not path.is_symlink())


def build(orders: Path, work: Path, archive: bool) -> Path:
    if not orders.is_file():
        raise SystemExit(f"no civvis_orders at {orders}")
    mod = mod_files()
    work.mkdir(parents=True, exist_ok=True)
    stage, dist, scratch = work / "stage", work / "dist", work / "pyinstaller"
    shutil.rmtree(stage, ignore_errors=True)
    shutil.rmtree(dist / NAME, ignore_errors=True)
    stage.mkdir()
    pinned = stage / ORDERS_NAME
    shutil.copy2(orders, pinned)
    python = pyinstaller_python(work)
    manifest = {
        "name": NAME,
        "revision": git_revision(),
        "built_utc": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "platform": platform_tag(),
        "python": platform.python_version(),
        "pyinstaller": PYINSTALLER,
        "civvis_orders": {"name": ORDERS_NAME, "bytes": pinned.stat().st_size,
                          "sha256": sha256(pinned), "built_from": str(orders)},
        "mod": {path.name: sha256(path) for path in mod},
        "fixture": sha256(FIXTURE),
    }
    (stage / "bundle.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n",
                                       encoding="utf-8")
    sep = os.pathsep
    command = [str(python), "-m", "PyInstaller", "--noconfirm", "--clean", "--onedir",
               "--console", "--name", NAME, "--distpath", str(dist), "--workpath",
               str(scratch), "--specpath", str(scratch),
               "--paths", str(TOOLS), "--paths", str(TOOLS / "civ6_control"),
               "--python-option", "X utf8",
               "--hidden-import", "civ6_brain", "--exclude-module", "tkinter",
               "--add-binary", f"{pinned}{sep}.",
               "--add-data", f"{stage / 'bundle.json'}{sep}.",
               "--add-data", f"{FIXTURE}{sep}fixtures"]
    for path in mod:
        command += ["--add-data", f"{path}{sep}civ6_control/mod"]
    subprocess.run([*command, str(APP)], check=True, cwd=work)
    folder = dist / NAME
    program = folder / (NAME + (".exe" if os.name == "nt" else ""))
    if not program.is_file():
        raise SystemExit(f"PyInstaller finished without {program}")
    shutil.copy2(GUIDE, folder / "README.md")
    print(f"bundle: {folder} ({folder_bytes(folder) / 1e6:.1f} MB; program {program.name}, "
          f"civvis_orders {manifest['civvis_orders']['bytes'] / 1e6:.1f} MB, "
          f"{len(mod)} mod files)", flush=True)
    if archive:
        made = make_archive(folder, work / f"{NAME}-{platform_tag()}.zip")
        print(f"archive: {made} ({made.stat().st_size / 1e6:.1f} MB)", flush=True)
    return folder


def make_archive(folder: Path, target: Path) -> Path:
    """Zip the folder. On macOS with `ditto`, as Finder would: the bundled
    Python.framework is held together by symlinks, which `shutil`'s zip turns
    into empty directories and duplicated files."""
    target.unlink(missing_ok=True)
    if sys.platform == "darwin":
        subprocess.run(["ditto", "-c", "-k", "--keepParent", str(folder), str(target)],
                       check=True)
        return target
    return Path(shutil.make_archive(str(target.with_suffix("")), "zip",
                                    root_dir=folder.parent, base_dir=folder.name))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    source = ap.add_mutually_exclusive_group(required=True)
    source.add_argument("--orders-bin", type=Path, help="the civvis_orders binary to ship")
    source.add_argument("--build-orders", action="store_true",
                        help="cargo build --release --locked --bin civvis_orders first")
    ap.add_argument("--work", type=Path, default=DEFAULT_WORK,
                    help="build directory (default %(default)s)")
    ap.add_argument("--archive", action="store_true", help="also zip the folder")
    args = ap.parse_args(argv)
    orders = build_orders() if args.build_orders else args.orders_bin.expanduser()
    build(orders.resolve(), args.work.resolve(), args.archive)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
