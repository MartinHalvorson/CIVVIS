#!/usr/bin/env python3
"""Build, refresh, install, and verify local Rust and WASM CIVVIS macOS apps.

Both apps are cut from one pinned Git revision, rendered from one launcher
template, signed before installation, and swapped under one process lock. The
private installed bundles are exposed through stable Desktop links so a
background launch agent never needs write access to macOS's protected Desktop
folder. Previous bundles are archived instead of overwritten.
"""

import argparse
import contextlib
import dataclasses
import datetime as dt
import fcntl
import hashlib
import json
import os
import pathlib
import plistlib
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.request
from typing import Dict, Iterator, List, Mapping, Optional, Sequence, Tuple


REPOSITORY_URL = "https://github.com/MartinHalvorson/CIVVIS"
# Where `beta/publish.sh` writes the viewer inside its --out directory. The
# publisher emits one complete LANE per revision with the viewer at the lane
# ROOT — "." rather than a subdirectory (it was `test/` when the publisher
# emitted a half-assembled site; the deploy workflow now stacks two whole
# lanes itself). It is NOT `beta/`: that is only the source directory the
# publisher is invoked from. Reading the wrong place raised FileNotFoundError
# after the build had already succeeded, so every desktop app silently
# stopped rebuilding while CI stayed green — nothing here is exercised by the
# suite unless a test names this constant.
# `viewer_lane_matches_the_publisher` pins it against publish.sh itself.
VIEWER_LANE = "."
FULL_COMMIT = re.compile(r"^[0-9a-f]{40}$")
GENERATED_BUILD_NAME = re.compile(r"^build-[0-9a-f]+-\d{8}T\d{6}Z$")
TEMPLATE_TOKENS = ("MODE", "LABEL", "COMMIT", "COMMIT_TIME", "BUILT_AT", "REPO")
DEFAULT_PRESET = (
    "AI simulation; Small 74x46 flat Continents; 6 majors;\n"
    "9 city-states; free for all; Ancient; Online/250 turns; Blitz/500 ms;\n"
    "hot equator and cold poles; science, culture, religious, diplomatic,\n"
    "domination, and score victories enabled."
)


@dataclasses.dataclass(frozen=True)
class AppSpec:
    mode: str
    label: str
    bundle_id: str
    port: int
    legacy_labels: Tuple[str, ...] = ()

    @property
    def bundle_name(self) -> str:
        return self.label + ".app"

    @property
    def executable_name(self) -> str:
        return "CIVVISLauncher"

    @property
    def launcher_script_name(self) -> str:
        return "CIVVIS Launcher.zsh"


APPS = (
    AppSpec("rust", "CIVVIS Rust", "ai.civvis.rust.desktop", 8785, ("Rust CIVVIS",)),
    AppSpec("wasm", "CIVVIS Wasm", "ai.civvis.wasm.desktop", 8790, ("WASM CIVVIS",)),
)


@dataclasses.dataclass(frozen=True)
class Revision:
    commit: str
    short: str
    committed_at: str
    title: str


@dataclasses.dataclass(frozen=True)
class BuildArtifacts:
    root: pathlib.Path
    revision: Revision
    native_binary: pathlib.Path
    native_built_at: str
    wasm_site: pathlib.Path
    wasm_built_at: str
    wasm_bytes: int
    bundle_bytes: int
    serve_script: pathlib.Path
    supervisor_script: pathlib.Path
    source_snapshot: str
    version: str


@dataclasses.dataclass(frozen=True)
class InstalledSwap:
    archives: Tuple[Tuple[pathlib.Path, pathlib.Path], ...]
    targets: Tuple[pathlib.Path, ...]
    links: Tuple[Tuple[pathlib.Path, Optional[str]], ...]
    desktop: pathlib.Path


class DesktopAppError(RuntimeError):
    pass


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def compact_stamp() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def run(
    command: Sequence[str],
    *,
    cwd: Optional[pathlib.Path] = None,
    env: Optional[Mapping[str, str]] = None,
    capture: bool = False,
) -> subprocess.CompletedProcess:
    print("+", " ".join(str(part) for part in command), flush=True)
    return subprocess.run(
        [str(part) for part in command],
        cwd=str(cwd) if cwd else None,
        env=dict(env) if env else None,
        check=True,
        text=True,
        stdout=subprocess.PIPE if capture else None,
        stderr=subprocess.PIPE if capture else None,
    )


def git(repo: pathlib.Path, *args: str) -> str:
    result = run(("git", *args), cwd=repo, capture=True)
    return result.stdout.strip()


def repository_root(script: pathlib.Path) -> pathlib.Path:
    return script.resolve().parents[1]


def resolve_revision(repo: pathlib.Path, ref: str, fetch: bool) -> Revision:
    if fetch:
        run(("git", "fetch", "--prune", "origin", "main"), cwd=repo)
    commit = git(repo, "rev-parse", ref + "^{commit}")
    if not FULL_COMMIT.fullmatch(commit):
        raise DesktopAppError("git did not resolve a full commit for " + ref)
    return Revision(
        commit=commit,
        short=git(repo, "rev-parse", "--short", commit),
        committed_at=git(repo, "show", "-s", "--format=%cI", commit),
        title=git(repo, "show", "-s", "--format=%s", commit),
    )


@contextlib.contextmanager
def install_lock(state_dir: pathlib.Path) -> Iterator[None]:
    state_dir.mkdir(parents=True, exist_ok=True)
    lock_path = state_dir / "desktop-apps.lock"
    with lock_path.open("a+") as held:
        try:
            fcntl.flock(held.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise DesktopAppError("another CIVVIS desktop build or install is active") from error
        held.seek(0)
        held.truncate()
        held.write(str(os.getpid()) + "\n")
        held.flush()
        try:
            yield
        finally:
            held.seek(0)
            held.truncate()
            held.flush()
            fcntl.flock(held.fileno(), fcntl.LOCK_UN)


def build_environment(base: Mapping[str, str]) -> Dict[str, str]:
    env = dict(base)
    candidates = (
        pathlib.Path.home() / ".local/share/civvis-wasm-tools/binaryen-version_131/bin",
        pathlib.Path.home() / ".local/share/civvis-wasm-tools/venv/bin",
        pathlib.Path.home() / ".cargo/bin",
        pathlib.Path("/opt/homebrew/bin"),
    )
    additions = [str(path) for path in candidates if path.is_dir()]
    env["PATH"] = os.pathsep.join(additions + [env.get("PATH", "")])
    if shutil.which("cargo", path=env["PATH"]) is None:
        raise DesktopAppError("cargo is not available")
    if shutil.which("wasm-opt", path=env["PATH"]) is None:
        raise DesktopAppError("wasm-opt is required for the optimized WASM desktop build")
    return env


# The engine's stock opening world, as named by `stock_opening_params` in
# `src/server.rs`. These are the field names to read out of it, mapped to the
# supervisor option each one governs.
STOCK_WORLD_FIELDS = {
    "num_players": "players",
    "map_script": "map",
    "map_topology": "shape",
    "map_poles": "poles",
    "game_speed": "speed",
}
# Rust spellings of the values, as the id strings every other surface uses.
# A spelling the stock world stops naming is kept rather than pruned: this is a
# translation table, not a second statement of which world is stock, and the
# reader above already refuses a spelling it does not know.
STOCK_WORLD_IDS = {
    "MapScript::Lakes": "lakes",
    "MapScript::TeninsBall": "tenins_ball",
    "MapTopology::Planet": "planet",
    "MapPoles::Poles": "poles",
    "MapPoles::Randomized": "randomized",
    "GameSpeed::Online": "online",
}


def read_stock_opening_world(source: pathlib.Path) -> Dict[str, str]:
    """The stock opening world, read from the engine that defines it.

    `stock_opening_params` in `src/server.rs` is the single description of the
    world a first visit opens on; `wasm::opening_params` and `/rules`
    `default_setup` are both it. Nothing else may restate those values, so this
    reads them rather than keeping a second copy that can rot — which is
    exactly what the fragment list here used to be, and why it went on passing
    a world nobody shipped until it stopped matching any file at all.
    """
    server = (source / "src/server.rs").read_text(encoding="utf-8")
    start = server.find("fn stock_opening_params(")
    if start < 0:
        raise DesktopAppError(
            "src/server.rs no longer defines stock_opening_params; the desktop "
            "default contract has lost the engine it reads from"
        )
    body = server[start : server.find("\n}", start)]
    world: Dict[str, str] = {}
    for field, option in STOCK_WORLD_FIELDS.items():
        # Either `field: Value,` or Rust's shorthand `field,` with the value
        # bound just above as `let field = Value;`.
        found = re.search(rf"^\s*{field}: ([^,\n]+),", body, re.MULTILINE) or re.search(
            rf"^\s*let {field} = ([^;\n]+);", body, re.MULTILINE
        )
        if not found:
            raise DesktopAppError(
                f"stock_opening_params no longer names {field}; the desktop "
                "default contract cannot be checked against the engine"
            )
        value = found.group(1).strip()
        if "::" in value and value not in STOCK_WORLD_IDS:
            raise DesktopAppError(
                f"stock_opening_params names {field} as {value}, whose id string "
                f"this contract does not know; add it to STOCK_WORLD_IDS"
            )
        world[option] = STOCK_WORLD_IDS.get(value, value)
    return world


def supervisor_option_defaults(supervisor: str) -> Dict[str, str]:
    """Each `--option` the supervisor declares, mapped to its argparse default.

    Parsed per `add_argument` call rather than by searching the whole file:
    the option strings also appear in the `civvis play` argument list, and an
    option that lost its default must not silently borrow the next one's.
    """
    defaults: Dict[str, str] = {}
    calls = [found.start() for found in re.finditer(r"parser\.add_argument\(", supervisor)]
    for index, at in enumerate(calls):
        call = supervisor[at : calls[index + 1] if index + 1 < len(calls) else len(supervisor)]
        name = re.search(r'"--([\w-]+)"', call)
        value = re.search(r'default=("?)([\w.-]+)\1', call)
        if name and value:
            defaults[name.group(1)] = value.group(2)
    return defaults


def verify_default_contract(source: pathlib.Path, template: pathlib.Path) -> None:
    launcher = template.read_text(encoding="utf-8")
    supervisor = (source / "tools/spectator_supervisor.py").read_text(encoding="utf-8")
    world = read_stock_opening_world(source)

    missing: List[str] = []
    # The desktop channels must not restate the world at all. The launcher
    # passes operational flags only, so the supervisor's own defaults — checked
    # against the engine below — are what both channels open on. A world flag
    # here would be a third copy, and the one that silently diverged before.
    for option in ("--players", "--width", "--height", "--city-states", "--turns",
                   "--map", "--shape", "--poles", "--speed"):
        if option in launcher:
            missing.append(f"launcher pins {option}; the engine's stock world decides it")
    for fragment in ("--fixed-setup --source-check-interval 1200",):
        if fragment not in launcher:
            missing.append(fragment)

    # The supervisor is the one place a Python surface names the world, and it
    # must name the engine's.
    declared = supervisor_option_defaults(supervisor)
    for option, value in world.items():
        if option not in declared:
            missing.append(f"supervisor no longer declares a --{option} default")
        elif declared[option] != value:
            missing.append(
                f"supervisor --{option} default is {declared[option]!r}, "
                f"the engine's stock world is {value!r}"
            )
    supervisor_fragments = (
        'SIMULATION_SPEED = "online"',
        'SIMULATION_START_ERA = "ancient"',
        '"--spectate",',
        '"--supervised",',
    )
    missing += [fragment for fragment in supervisor_fragments if fragment not in supervisor]
    if missing:
        raise DesktopAppError("desktop default contract drifted: " + ", ".join(missing))


def runtime_source_snapshot(source: pathlib.Path) -> str:
    """Hash the inputs the native supervisor uses to validate a promoted build."""
    files: List[pathlib.Path] = []
    for relative in ("Cargo.toml", "Cargo.lock", "build.rs", "src", "data", "web"):
        path = source / relative
        if path.is_file():
            files.append(path)
        elif path.is_dir():
            files.extend(candidate for candidate in path.rglob("*") if candidate.is_file())
    digest = hashlib.sha256()
    for path in sorted(files, key=lambda candidate: candidate.relative_to(source).as_posix()):
        relative = path.relative_to(source).as_posix().encode()
        digest.update(len(relative).to_bytes(4, "big"))
        digest.update(relative)
        digest.update(path.read_bytes())
    return digest.hexdigest()


def reusable_cargo_targets(state_dir: pathlib.Path) -> Tuple[pathlib.Path, pathlib.Path]:
    cargo_cache = state_dir / "cargo-cache"
    cargo_cache.mkdir(parents=True, exist_ok=True)
    builds = sorted(
        (
            path
            for path in state_dir.iterdir()
            if path.is_dir() and GENERATED_BUILD_NAME.fullmatch(path.name)
        ),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    targets = (cargo_cache / "native-target", cargo_cache / "wasm-target")
    for target in targets:
        if target.exists():
            continue
        for build in builds:
            legacy = build / target.name
            if legacy.is_dir():
                shutil.move(str(legacy), str(target))
                break
    return targets


def build_artifacts(
    repo: pathlib.Path,
    revision: Revision,
    state_dir: pathlib.Path,
    template: pathlib.Path,
) -> BuildArtifacts:
    build_root = state_dir / ("build-" + revision.short + "-" + compact_stamp())
    source = build_root / "source"
    # Cargo outputs are safe to reuse across pinned detached worktrees and make
    # the perpetual refresh cadence practical. The immutable artifacts copied
    # into each app still live under this build's timestamped root.
    native_target, wasm_target = reusable_cargo_targets(state_dir)
    wasm_site = build_root / "wasm-site"
    build_root.mkdir(parents=True)
    run(("git", "worktree", "add", "--detach", str(source), revision.commit), cwd=repo)
    try:
        verify_default_contract(source, template)
        env = build_environment(os.environ)
        native_env = dict(env)
        native_env.update(
            {
                "CARGO_BUILD_JOBS": "2",
                "CARGO_TARGET_DIR": str(native_target),
            }
        )
        run(("cargo", "build", "--release", "--locked", "--bin", "civvis"), cwd=source, env=native_env)
        native_binary = native_target / "release/civvis"
        if not native_binary.is_file():
            raise DesktopAppError("native build did not produce civvis")
        # Native provenance is supplied by spectator-build.json when the
        # promoted binary launches; unlike WASM, it is not compiled into the
        # artifact. Stamp the completed binary here, not before Cargo starts.
        # A low-priority native compile can take most of the freshness window,
        # and counting that work as artifact age made a fresh paired build
        # reject itself after the subsequent WASM build.
        native_built_at = utc_now()

        wasm_env = dict(env)
        wasm_env.update({"CARGO_BUILD_JOBS": "2", "CARGO_TARGET_DIR": str(wasm_target)})
        run(("./beta/publish.sh", "--out", str(wasm_site)), cwd=source, env=wasm_env)
        manifest = json.loads(
            (wasm_site / VIEWER_LANE / "build.json").read_text(encoding="utf-8")
        )
        if manifest.get("commit") != revision.commit:
            raise DesktopAppError("WASM manifest names a different revision")
        if not (wasm_site / VIEWER_LANE / "civvis.wasm").is_file():
            raise DesktopAppError("WASM build did not produce civvis.wasm")
        serve_script = build_root / "serve.py"
        shutil.copy2(source / "beta/serve.py", serve_script)
        supervisor_script = build_root / "spectator_supervisor.py"
        shutil.copy2(source / "tools/spectator_supervisor.py", supervisor_script)
        cargo_manifest = (source / "Cargo.toml").read_text(encoding="utf-8")
        version_match = re.search(r'^version\s*=\s*"([^"]+)"', cargo_manifest, re.MULTILINE)
        if not version_match:
            raise DesktopAppError("Cargo.toml does not declare a package version")

        record = {
            "commit": revision.commit,
            "commit_time": revision.committed_at,
            "native_built_at": native_built_at,
            "wasm_built_at": manifest["built_at"],
            "wasm_bytes": manifest["wasm_bytes"],
            "bundle_bytes": manifest["bundle_bytes"],
        }
        (build_root / "build-record.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
        return BuildArtifacts(
            root=build_root,
            revision=revision,
            native_binary=native_binary,
            native_built_at=native_built_at,
            wasm_site=wasm_site,
            wasm_built_at=manifest["built_at"],
            wasm_bytes=int(manifest["wasm_bytes"]),
            bundle_bytes=int(manifest["bundle_bytes"]),
            serve_script=serve_script,
            supervisor_script=supervisor_script,
            source_snapshot=runtime_source_snapshot(source),
            version=version_match.group(1),
        )
    finally:
        if source.exists():
            run(("git", "worktree", "remove", str(source)), cwd=repo)


def render_launcher(
    template: str,
    spec: AppSpec,
    revision: Revision,
    built_at: str,
    repo: pathlib.Path,
) -> str:
    values = {
        "MODE": spec.mode,
        "LABEL": spec.label,
        "COMMIT": revision.commit,
        "COMMIT_TIME": revision.committed_at,
        "BUILT_AT": built_at,
        "REPO": str(repo),
    }
    rendered = template
    for token in TEMPLATE_TOKENS:
        rendered = rendered.replace("@@" + token + "@@", values[token])
    leftovers = re.findall(r"@@[A-Z_]+@@", rendered)
    if leftovers:
        raise DesktopAppError("unrendered launcher tokens: " + ", ".join(leftovers))
    return rendered


def write_info_plist(path: pathlib.Path, spec: AppSpec, short: str, version: str = "0.6.0") -> None:
    payload = {
        "CFBundleDevelopmentRegion": "en",
        "CFBundleDisplayName": spec.label,
        "CFBundleExecutable": spec.executable_name,
        "CFBundleIconFile": "CIVVIS.icns",
        "CFBundleIdentifier": spec.bundle_id,
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleName": spec.label,
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": short,
        "LSMinimumSystemVersion": "13.0",
        "LSUIElement": True,
        "NSHighResolutionCapable": True,
    }
    with path.open("wb") as plist:
        plistlib.dump(payload, plist, sort_keys=True)


def build_note(spec: AppSpec, artifacts: BuildArtifacts, built_at: str) -> str:
    revision = artifacts.revision
    if spec.mode == "rust":
        profile = "Build profile: cargo build --release --locked --bin civvis\nOptimization: opt-level 3, thin LTO, one codegen unit"
    else:
        percent = artifacts.bundle_bytes * 100 // (25 * 1024 * 1024)
        profile = (
            "Build profile: wasm32-unknown-unknown release, Binaryen wasm-opt -O3\n"
            "Engine: {:,} bytes\nComplete local site: {:,} bytes ({}% of the 25 MiB budget)".format(
                artifacts.wasm_bytes, artifacts.bundle_bytes, percent
            )
        )
    return (
        "{} desktop build\n\nRepository: {}\nCommit: {}\nCommitted: {}\nBuilt: {}\nTitle: {}\n{}\n\n"
        "Opening preset: {}\n".format(
            spec.label,
            REPOSITORY_URL,
            revision.commit,
            revision.committed_at,
            built_at,
            revision.title,
            profile,
            DEFAULT_PRESET,
        )
    )


def stage_apps(repo: pathlib.Path, artifacts: BuildArtifacts, template_path: pathlib.Path) -> pathlib.Path:
    apps_root = artifacts.root / "apps"
    apps_root.mkdir()
    template = template_path.read_text(encoding="utf-8")
    watcher_template = template_path.with_name("CIVVIS Tab Watcher.zsh.in")
    if not watcher_template.is_file():
        raise DesktopAppError("desktop tab watcher template is missing")
    native_launcher_source = template_path.with_name("CIVVIS Launcher.c")
    if not native_launcher_source.is_file():
        raise DesktopAppError("native desktop launcher source is missing")
    system_icon = pathlib.Path(
        "/System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/GenericApplicationIcon.icns"
    )
    if not system_icon.is_file():
        raise DesktopAppError("macOS generic application icon is missing")

    for spec in APPS:
        app = apps_root / spec.bundle_name
        macos = app / "Contents/MacOS"
        resources = app / "Contents/Resources"
        macos.mkdir(parents=True)
        resources.mkdir()
        built_at = artifacts.native_built_at if spec.mode == "rust" else artifacts.wasm_built_at
        launcher_script = resources / spec.launcher_script_name
        launcher_script.write_text(
            render_launcher(template, spec, artifacts.revision, built_at, repo),
            encoding="utf-8",
        )
        # The native wrapper passes this resource to /bin/zsh; only the Mach-O
        # wrapper belongs in Contents/MacOS. Files there are always treated as
        # nested code, whose script-signature xattrs shutil cannot preserve.
        launcher_script.chmod(0o644)
        native_launcher = macos / spec.executable_name
        run(
            (
                "/usr/bin/xcrun",
                "--sdk",
                "macosx",
                "clang",
                "-Os",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-mmacosx-version-min=13.0",
                str(native_launcher_source),
                "-o",
                str(native_launcher),
            )
        )
        write_info_plist(
            app / "Contents/Info.plist",
            spec,
            artifacts.revision.short,
            artifacts.version,
        )
        # The system icon is root-owned and may carry filesystem flags a user
        # cannot reproduce. Its bytes are the asset; its protected metadata is
        # not part of either app bundle.
        shutil.copyfile(system_icon, resources / "CIVVIS.icns")
        (resources / "BUILD.txt").write_text(build_note(spec, artifacts, built_at), encoding="utf-8")
        watcher = resources / "CIVVIS Tab Watcher.zsh"
        shutil.copy2(watcher_template, watcher)
        watcher.chmod(0o755)
        if spec.mode == "rust":
            binary = resources / ("civvis-" + artifacts.revision.commit)
            shutil.copy2(artifacts.native_binary, binary)
            binary.chmod(0o755)
            shutil.copy2(artifacts.supervisor_script, resources / "spectator_supervisor.py")
            (resources / "spectator_supervisor.py").chmod(0o755)
            runtime = {
                "revision": artifacts.revision.commit,
                "embedded_revision": artifacts.revision.short,
                "commit_time": artifacts.revision.committed_at,
                "dirty": False,
                "source_snapshot": artifacts.source_snapshot,
                "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "built_at": artifacts.native_built_at,
            }
            (resources / "spectator-build.json").write_text(
                json.dumps(runtime, indent=2) + "\n", encoding="utf-8"
            )
        else:
            shutil.copytree(artifacts.wasm_site, resources / "site")
            shutil.copy2(artifacts.serve_script, resources / "serve.py")
            (resources / "serve.py").chmod(0o755)

        run(("/usr/bin/xattr", "-cr", str(app)))
        run(("/usr/bin/codesign", "--force", "--deep", "--sign", "-", str(app)))
        run(("/usr/bin/codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app)))
        run(("/bin/zsh", "-n", str(launcher_script)))
        run(("/bin/zsh", "-n", str(watcher)))
        run(("/usr/bin/plutil", "-lint", str(app / "Contents/Info.plist")))

    if (apps_root / "CIVVIS Wasm.app/Contents/Resources/serve.py").read_bytes() != artifacts.serve_script.read_bytes():
        raise DesktopAppError("the staged WASM server differs from the selected source")
    return apps_root


def commit_from_build_note(app: pathlib.Path) -> str:
    note = app / "Contents/Resources/BUILD.txt"
    if not note.is_file():
        return "unknown"
    match = re.search(r"^Commit: ([0-9a-f]{40})$", note.read_text(encoding="utf-8"), re.MULTILINE)
    return match.group(1)[:7] if match else "unknown"


def unused_archive(previous: pathlib.Path, label: str, short: str, stamp: str) -> pathlib.Path:
    stem = label.replace(" ", "-") + "-" + short + "-" + stamp
    candidate = previous / (stem + ".app")
    suffix = 2
    while candidate.exists():
        candidate = previous / (stem + "-" + str(suffix) + ".app")
        suffix += 1
    return candidate


def symlink_points_to(link: pathlib.Path, target: pathlib.Path) -> bool:
    if not link.is_symlink():
        return False
    held = pathlib.Path(os.readlink(link))
    if not held.is_absolute():
        held = link.parent / held
    return held.resolve(strict=False) == target.resolve(strict=False)


def restore_links(links: Sequence[Tuple[pathlib.Path, Optional[str]]]) -> None:
    for link, previous_target in reversed(links):
        if link.is_symlink():
            link.unlink()
        if previous_target is not None:
            link.symlink_to(previous_target, target_is_directory=True)


def install_apps(
    apps_root: pathlib.Path, desktop: pathlib.Path, state_dir: pathlib.Path
) -> InstalledSwap:
    if not desktop.is_dir():
        desktop.mkdir(parents=True)
    previous = state_dir / "previous"
    previous.mkdir(parents=True, exist_ok=True)
    installed_root = state_dir / "installed"
    installed_root.mkdir(parents=True, exist_ok=True)
    stamp = compact_stamp()
    incoming: List[Tuple[pathlib.Path, pathlib.Path]] = []
    archived: List[Tuple[pathlib.Path, pathlib.Path]] = []
    installed: List[Tuple[pathlib.Path, pathlib.Path]] = []
    links: List[Tuple[pathlib.Path, Optional[str]]] = []
    try:
        for spec in APPS:
            staged = apps_root / spec.bundle_name
            temporary = installed_root / (
                "." + spec.bundle_name + ".incoming-" + str(os.getpid())
            )
            if temporary.exists() or temporary.is_symlink():
                raise DesktopAppError("stale incoming bundle exists: " + str(temporary))
            shutil.copytree(staged, temporary, copy_function=shutil.copy2)
            incoming.append((temporary, staged))

        for spec in APPS:
            target = installed_root / spec.bundle_name
            if target.is_symlink():
                raise DesktopAppError("installed bundle must not be a symlink: " + str(target))
            if target.exists():
                archive = unused_archive(
                    previous, spec.label, commit_from_build_note(target), stamp
                )
                shutil.move(str(target), str(archive))
                archived.append((archive, target))

        for spec, (temporary, staged) in zip(APPS, incoming):
            target = installed_root / spec.bundle_name
            shutil.move(str(temporary), str(target))
            installed.append((target, staged))

        for spec in APPS:
            target = desktop / spec.bundle_name
            installed_target = installed_root / spec.bundle_name
            if not symlink_points_to(target, installed_target):
                previous_target = None
                if target.is_symlink():
                    previous_target = os.readlink(target)
                    target.unlink()
                elif target.exists():
                    archive = unused_archive(
                        previous, spec.label, commit_from_build_note(target), stamp
                    )
                    shutil.move(str(target), str(archive))
                    archived.append((archive, target))
                temporary_link = desktop / (
                    "." + spec.bundle_name + ".link-" + str(os.getpid())
                )
                if temporary_link.exists() or temporary_link.is_symlink():
                    raise DesktopAppError("stale Desktop link exists: " + str(temporary_link))
                links.append((target, previous_target))
                try:
                    temporary_link.symlink_to(installed_target, target_is_directory=True)
                    os.replace(temporary_link, target)
                finally:
                    if temporary_link.is_symlink():
                        temporary_link.unlink()

            for legacy_label in spec.legacy_labels:
                legacy = desktop / (legacy_label + ".app")
                if legacy.is_symlink():
                    links.append((legacy, os.readlink(legacy)))
                    legacy.unlink()
                elif legacy.exists():
                    archive = unused_archive(
                        previous,
                        legacy_label,
                        commit_from_build_note(legacy),
                        stamp,
                    )
                    shutil.move(str(legacy), str(archive))
                    archived.append((archive, legacy))
    except Exception:
        restore_links(links)
        for target, staged in reversed(installed):
            if target.exists():
                shutil.rmtree(target)
        for archive, target in reversed(archived):
            if archive.exists() and not target.exists():
                shutil.move(str(archive), str(target))
        for temporary, _ in incoming:
            if temporary.exists():
                shutil.rmtree(temporary)
        raise
    return InstalledSwap(
        archives=tuple(archived),
        targets=tuple(target for target, _ in installed),
        links=tuple(links),
        desktop=desktop,
    )


def rollback_install(swap: InstalledSwap, relaunch: bool = False) -> None:
    # The generated source bundles remain in the build directory, so removing
    # a failed installed copy loses nothing. Restore every previous private or
    # Desktop bundle before asking the stable links to launch again.
    restore_links(swap.links)
    for target in reversed(swap.targets):
        if target.exists():
            shutil.rmtree(target)
    for archive, target in reversed(swap.archives):
        if archive.exists() and not target.exists():
            shutil.move(str(archive), str(target))
    if relaunch and swap.targets:
        for spec in APPS:
            target = swap.desktop / spec.bundle_name
            if target.exists():
                subprocess.run(("/usr/bin/open", "-n", str(target)), check=False)


REFRESH_AGENT_LABEL = "ai.civvis.desktop-refresh"
REFRESH_INTERVAL_SECONDS = 60
REFRESH_REBUILD_AGE_MINUTES = 10
MAX_BUILD_AGE_MINUTES = 20


def refresh_agent_path() -> pathlib.Path:
    return pathlib.Path.home() / "Library/LaunchAgents" / (REFRESH_AGENT_LABEL + ".plist")


def refresh_agent_payload(
    repo: pathlib.Path, desktop: pathlib.Path, state_dir: pathlib.Path
) -> dict:
    home = pathlib.Path.home()
    log = home / "Library/Logs/CIVVIS Desktop Refresh.log"
    return {
        "Label": REFRESH_AGENT_LABEL,
        "ProgramArguments": [
            "/usr/bin/python3",
            str(repo / "tools/civvis_desktop_apps.py"),
            "refresh",
            "--repo",
            str(repo),
            "--desktop",
            str(desktop),
            "--state-dir",
            str(state_dir),
            "--max-build-age-minutes",
            str(MAX_BUILD_AGE_MINUTES),
            "--rebuild-age-minutes",
            str(REFRESH_REBUILD_AGE_MINUTES),
            "--no-launch",
        ],
        "StartInterval": REFRESH_INTERVAL_SECONDS,
        "ProcessType": "Background",
        "LowPriorityIO": True,
        "Nice": 10,
        "StandardOutPath": str(log),
        "StandardErrorPath": str(log),
        "EnvironmentVariables": {
            "HOME": str(home),
            "PATH": ":".join(
                (
                    str(home / ".cargo/bin"),
                    "/opt/homebrew/bin",
                    "/usr/local/bin",
                    "/usr/bin",
                    "/bin",
                    "/usr/sbin",
                    "/sbin",
                )
            ),
        },
    }


def install_refresh_agent(
    repo: pathlib.Path, desktop: pathlib.Path, state_dir: pathlib.Path
) -> pathlib.Path:
    path = refresh_agent_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    (pathlib.Path.home() / "Library/Logs").mkdir(parents=True, exist_ok=True)
    staged = path.with_suffix(".plist.new")
    with staged.open("wb") as output:
        plistlib.dump(refresh_agent_payload(repo, desktop, state_dir), output, sort_keys=True)
    os.replace(staged, path)
    domain = "gui/{}".format(os.getuid())
    subprocess.run(
        ("/bin/launchctl", "bootout", domain + "/" + REFRESH_AGENT_LABEL),
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    run(("/bin/launchctl", "bootstrap", domain, str(path)))
    return path


def verify_refresh_agent(
    repo: pathlib.Path, desktop: pathlib.Path, state_dir: pathlib.Path
) -> dict:
    path = refresh_agent_path()
    if not path.is_file():
        raise DesktopAppError("missing recurring refresh agent " + str(path))
    with path.open("rb") as source:
        installed = plistlib.load(source)
    expected = refresh_agent_payload(repo, desktop, state_dir)
    if installed != expected:
        raise DesktopAppError("the recurring desktop refresh agent is out of date")
    domain = "gui/{}/{}".format(os.getuid(), REFRESH_AGENT_LABEL)
    loaded = subprocess.run(
        ("/bin/launchctl", "print", domain),
        check=False,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )
    if loaded.returncode != 0:
        raise DesktopAppError("the recurring desktop refresh agent is not loaded")
    return {
        "label": REFRESH_AGENT_LABEL,
        "loaded": True,
        "interval_seconds": installed["StartInterval"],
        "rebuild_age_minutes": REFRESH_REBUILD_AGE_MINUTES,
        "max_build_age_minutes": MAX_BUILD_AGE_MINUTES,
        "path": str(path),
    }


def prune_generated_state(
    state_dir: pathlib.Path, keep_builds: int = 2, keep_archives: int = 4
) -> None:
    """Bound disk use for an updater designed to run indefinitely."""
    builds = sorted(
        (
            path
            for path in state_dir.iterdir()
            if path.is_dir() and GENERATED_BUILD_NAME.fullmatch(path.name)
        ),
        key=lambda path: path.stat().st_mtime,
        reverse=True,
    )
    previous = state_dir / "previous"
    archives = (
        sorted(
            (path for path in previous.glob("*.app") if path.is_dir()),
            key=lambda path: path.stat().st_mtime,
            reverse=True,
        )
        if previous.is_dir()
        else []
    )
    for path in builds[max(0, keep_builds) :] + archives[max(0, keep_archives) :]:
        shutil.rmtree(path)


def json_url(url: str) -> dict:
    with urllib.request.urlopen(url, timeout=2) as response:
        return json.loads(response.read())


def wait_for_live(revision: Revision, native_built_at: str, wasm_built_at: str, seconds: int = 90) -> None:
    deadline = time.time() + seconds
    last_error: Optional[Exception] = None
    while time.time() < deadline:
        try:
            rust = json_url("http://127.0.0.1:8785/runtime")
            wasm = json_url("http://127.0.0.1:8790/wasm/build.json")
            if (
                rust.get("commit") == revision.commit
                and rust.get("built_at") == native_built_at
                and wasm.get("commit") == revision.commit
                and wasm.get("built_at") == wasm_built_at
            ):
                return
        except (OSError, ValueError, urllib.error.URLError) as error:
            last_error = error
        time.sleep(1)
    raise DesktopAppError("desktop apps did not become live: " + str(last_error or "metadata mismatch"))


def launch_apps(desktop: pathlib.Path, artifacts: BuildArtifacts) -> None:
    for spec in APPS:
        run(("/usr/bin/open", "-n", str(desktop / spec.bundle_name)))
    wait_for_live(
        artifacts.revision,
        artifacts.native_built_at,
        artifacts.wasm_built_at,
    )


def launcher_metadata(app: pathlib.Path, launcher_script: str) -> dict:
    text = (app / "Contents/Resources" / launcher_script).read_text(encoding="utf-8")
    values = {}
    for name in ("mode", "commit", "commit_time", "built_at"):
        match = re.search(r'^readonly civvis_{}="([^"]+)"$'.format(name), text, re.MULTILINE)
        if not match:
            raise DesktopAppError("launcher omits civvis_" + name + ": " + str(app))
        values[name] = match.group(1)
    return values


def age_minutes(timestamp: str) -> int:
    parsed = dt.datetime.fromisoformat(timestamp.replace("Z", "+00:00"))
    return int((dt.datetime.now(dt.timezone.utc) - parsed).total_seconds() // 60)


def installed_pair_needs_refresh(
    desktop: pathlib.Path,
    revision: Revision,
    max_build_age_minutes: int,
) -> bool:
    """Cheaply decide whether a click should rebuild the installed pair."""
    metadata = []
    for spec in APPS:
        app = desktop / spec.bundle_name
        launcher = app / "Contents/Resources" / spec.launcher_script_name
        if not launcher.is_file():
            return True
        try:
            held = launcher_metadata(app, spec.launcher_script_name)
            if age_minutes(held["built_at"]) > max_build_age_minutes:
                return True
        except (DesktopAppError, OSError, ValueError):
            return True
        metadata.append(held)
    return any(held["commit"] != revision.commit for held in metadata)


def route(url: str) -> Tuple[int, str, str, str]:
    opener = urllib.request.build_opener(urllib.request.HTTPRedirectHandler())
    with opener.open(url, timeout=3) as response:
        return (
            response.status,
            response.headers.get_content_type(),
            response.geturl(),
            response.headers.get("Cache-Control", ""),
        )


def listener(repo: pathlib.Path, port: int) -> dict:
    result = run(("/usr/sbin/lsof", "-nP", "-t", "-iTCP:" + str(port), "-sTCP:LISTEN"), capture=True)
    pids = [line for line in result.stdout.splitlines() if line]
    if len(pids) != 1:
        raise DesktopAppError("expected one listener on port " + str(port))
    pid = pids[0]
    process = run(("/bin/ps", "-p", pid, "-o", "ppid=,command="), capture=True).stdout.strip()
    parent, command = process.split(None, 1)
    return {"pid": int(pid), "ppid": int(parent), "command": command}


def wait_for_detached_listeners(repo: pathlib.Path, seconds: int = 30) -> dict:
    """Wait until both ports belong to the installed long-lived runtimes."""
    deadline = time.time() + seconds
    held = {}
    last_error: Optional[Exception] = None
    while time.time() < deadline:
        try:
            held = {
                "rust": listener(repo, 8785),
                "wasm": listener(repo, 8790),
            }
            if (
                "/rust-runtime/target/spectator/civvis play" in held["rust"]["command"]
                and "/CIVVIS Wasm.app/Contents/Resources/serve.py "
                in held["wasm"]["command"]
            ):
                return held
        except DesktopAppError as error:
            last_error = error
        time.sleep(0.25)
    if held:
        detail = ", ".join(
            "{} pid {} ppid {}".format(mode, process["pid"], process["ppid"])
            for mode, process in held.items()
        )
    else:
        detail = str(last_error or "listeners unavailable")
    raise DesktopAppError("desktop listeners did not detach from their launchers: " + detail)


def verify_installed(
    repo: pathlib.Path,
    desktop: pathlib.Path,
    expected: Optional[Revision],
    max_build_age_minutes: int,
    require_live: bool,
) -> dict:
    metadata = {}
    for spec in APPS:
        app = desktop / spec.bundle_name
        if not app.is_dir():
            raise DesktopAppError("missing " + str(app))
        run(("/usr/bin/codesign", "--verify", "--deep", "--verbose=2", str(app)))
        run(("/usr/bin/plutil", "-lint", str(app / "Contents/Info.plist")))
        native_launcher = app / "Contents/MacOS" / spec.executable_name
        if not native_launcher.is_file() or not os.access(native_launcher, os.X_OK):
            raise DesktopAppError("missing native bundle executable " + str(native_launcher))
        run(("/usr/bin/codesign", "--verify", "--verbose=2", str(native_launcher)))
        run(("/bin/zsh", "-n", str(app / "Contents/Resources" / spec.launcher_script_name)))
        metadata[spec.mode] = launcher_metadata(app, spec.launcher_script_name)

    rust, wasm = metadata["rust"], metadata["wasm"]
    if rust["commit"] != wasm["commit"] or rust["commit_time"] != wasm["commit_time"]:
        raise DesktopAppError("Rust and WASM launchers name different source revisions")
    if expected and rust["commit"] != expected.commit:
        raise DesktopAppError("installed launchers are not built from " + expected.commit)
    for mode, held in metadata.items():
        age = age_minutes(held["built_at"])
        held["build_age_minutes"] = age
        if age > max_build_age_minutes:
            raise DesktopAppError("{} build is {} minutes old".format(mode, age))

    wasm_app = desktop / "CIVVIS Wasm.app"
    manifest = json.loads(
        (wasm_app / "Contents/Resources/site" / VIEWER_LANE / "build.json").read_text(
            encoding="utf-8"
        )
    )
    if manifest.get("commit") != wasm["commit"] or manifest.get("built_at") != wasm["built_at"]:
        raise DesktopAppError("WASM launcher and manifest provenance differ")
    selected_serve = subprocess.check_output(
        ("git", "show", rust["commit"] + ":beta/serve.py"), cwd=str(repo)
    )
    if (wasm_app / "Contents/Resources/serve.py").read_bytes() != selected_serve:
        raise DesktopAppError("installed WASM server differs from current source")

    report = {"commit": rust["commit"], "rust": rust, "wasm": wasm, "routes": {}}
    if require_live:
        live_rust = json_url("http://127.0.0.1:8785/runtime")
        live_wasm = json_url("http://127.0.0.1:8790/wasm/build.json")
        if live_rust.get("commit") != rust["commit"] or live_rust.get("built_at") != rust["built_at"]:
            raise DesktopAppError("live Rust metadata differs from its launcher")
        if live_wasm.get("commit") != wasm["commit"] or live_wasm.get("built_at") != wasm["built_at"]:
            raise DesktopAppError("live WASM metadata differs from its launcher")
        rust_routes = (
            route("http://127.0.0.1:8785/"),
            route("http://127.0.0.1:8785/rust"),
            route("http://127.0.0.1:8785/rust/"),
        )
        wasm_routes = (
            route("http://127.0.0.1:8790/wasm"),
            route("http://127.0.0.1:8790/wasm/?game=17"),
            route("http://127.0.0.1:8790/wasm/civvis.wasm"),
            route("http://127.0.0.1:8790/wasm/build.json"),
        )
        if any(status != 200 or mime != "text/html" for status, mime, _, _ in rust_routes):
            raise DesktopAppError("one or more native routes failed")
        if wasm_routes[0][0] != 200 or not wasm_routes[0][2].endswith("/wasm/"):
            raise DesktopAppError("the local /wasm alias did not redirect")
        if wasm_routes[1][0:2] != (200, "text/html"):
            raise DesktopAppError("the local WASM page failed")
        if wasm_routes[2][0:2] != (200, "application/wasm"):
            raise DesktopAppError("the local WASM module MIME type failed")
        if wasm_routes[3][0:2] != (200, "application/json"):
            raise DesktopAppError("the local WASM manifest failed")
        if not wasm_routes[1][2].endswith("/wasm/?game=17"):
            raise DesktopAppError("the local WASM channel lost its query string")
        if any("no-store" not in route_result[3] for route_result in rust_routes + wasm_routes):
            raise DesktopAppError("one or more local channel responses are cacheable")
        listeners = wait_for_detached_listeners(repo)
        report.update(
            {
                "live_rust": live_rust,
                "live_wasm": live_wasm,
                "listeners": listeners,
                "routes": {"rust": rust_routes, "wasm": wasm_routes},
            }
        )
    return report


def parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    script = pathlib.Path(__file__)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("build", "install", "refresh", "verify"))
    parser.add_argument("--repo", type=pathlib.Path, default=repository_root(script))
    parser.add_argument("--ref", default="origin/main")
    parser.add_argument("--desktop", type=pathlib.Path, default=pathlib.Path.home() / "Desktop")
    parser.add_argument(
        "--state-dir",
        type=pathlib.Path,
        default=pathlib.Path.home() / ".local/share/civvis-desktop",
    )
    parser.add_argument("--no-fetch", action="store_true")
    parser.add_argument("--no-launch", action="store_true")
    parser.add_argument(
        "--max-build-age-minutes", type=int, default=MAX_BUILD_AGE_MINUTES
    )
    parser.add_argument(
        "--rebuild-age-minutes", type=int, default=REFRESH_REBUILD_AGE_MINUTES
    )
    return parser.parse_args(argv)


def main(argv: Optional[Sequence[str]] = None) -> int:
    args = parse_args(argv)
    if sys.platform != "darwin":
        raise DesktopAppError("CIVVIS desktop app installation currently requires macOS")
    repo = args.repo.resolve()
    desktop = args.desktop.expanduser().resolve()
    state_dir = args.state_dir.expanduser().resolve()
    template = pathlib.Path(__file__).resolve().parent / "desktop/CIVVIS Launcher.zsh.in"

    try:
        if args.action == "verify":
            revision = resolve_revision(repo, args.ref, not args.no_fetch)
            report = verify_installed(
                repo,
                desktop,
                revision,
                args.max_build_age_minutes,
                not args.no_launch,
            )
            report["refresh_agent"] = verify_refresh_agent(repo, desktop, state_dir)
            print(json.dumps(report, indent=2, sort_keys=True))
            return 0

        with install_lock(state_dir):
            revision = resolve_revision(repo, args.ref, not args.no_fetch)
            if args.action == "refresh" and not installed_pair_needs_refresh(
                desktop, revision, args.rebuild_age_minutes
            ):
                print("desktop apps already match {} and are fresh".format(revision.short))
                return 0
            artifacts = build_artifacts(repo, revision, state_dir, template)
            apps_root = stage_apps(repo, artifacts, template)
            print("staged {} and {} from {} at {}".format(APPS[0].bundle_name, APPS[1].bundle_name, revision.short, apps_root))
            if args.action == "build":
                return 0
            swap = install_apps(apps_root, desktop, state_dir)
            for path, _ in swap.archives:
                print("archived", path)
            should_launch = args.action == "install" and not args.no_launch
            try:
                if should_launch:
                    launch_apps(desktop, artifacts)
                if args.action == "install":
                    install_refresh_agent(repo, desktop, state_dir)
                report = verify_installed(
                    repo,
                    desktop,
                    revision,
                    args.max_build_age_minutes,
                    should_launch,
                )
                if args.action == "install":
                    report["refresh_agent"] = verify_refresh_agent(
                        repo, desktop, state_dir
                    )
            except Exception:
                print("new desktop apps failed verification; restoring the archived pair", file=sys.stderr)
                rollback_install(swap, relaunch=should_launch)
                raise
            try:
                prune_generated_state(state_dir)
            except OSError as error:
                print("warning: could not prune old desktop artifacts:", error, file=sys.stderr)
            print(json.dumps(report, indent=2, sort_keys=True))
            return 0
    except (DesktopAppError, subprocess.CalledProcessError, OSError, ValueError) as error:
        print("desktop app error:", error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
