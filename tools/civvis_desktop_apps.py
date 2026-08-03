#!/usr/bin/env python3
"""Build, install, and verify the local Rust and WASM CIVVIS macOS apps.

Both apps are cut from one pinned Git revision, rendered from one launcher
template, signed before installation, and swapped under one process lock.  The
previous bundles are archived instead of overwritten.
"""

import argparse
import contextlib
import dataclasses
import datetime as dt
import fcntl
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
FULL_COMMIT = re.compile(r"^[0-9a-f]{40}$")
TEMPLATE_TOKENS = ("MODE", "LABEL", "COMMIT", "COMMIT_TIME", "BUILT_AT")
DEFAULT_PRESET = (
    "AI simulation; Small 74x46 flat Continents; 6 majors;\n"
    "9 city-states; free for all; Ancient; Online/250 turns; Blitz/1000 ms;\n"
    "hot equator and cold poles; science, culture, religious, diplomatic,\n"
    "domination, and score victories enabled."
)


@dataclasses.dataclass(frozen=True)
class AppSpec:
    mode: str
    label: str
    bundle_id: str
    port: int

    @property
    def bundle_name(self) -> str:
        return self.label + ".app"

    @property
    def executable_name(self) -> str:
        return self.label + " Launcher"


APPS = (
    AppSpec("rust", "Rust CIVVIS", "ai.civvis.rust.desktop", 8785),
    AppSpec("wasm", "WASM CIVVIS", "ai.civvis.wasm.desktop", 8790),
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
    version: str


@dataclasses.dataclass(frozen=True)
class InstalledSwap:
    archives: Tuple[Tuple[pathlib.Path, pathlib.Path], ...]
    targets: Tuple[pathlib.Path, ...]


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
        short=commit[:7],
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


def verify_default_contract(source: pathlib.Path, template: pathlib.Path) -> None:
    launcher = template.read_text(encoding="utf-8")
    rust_fragments = (
        "--players 6 --width 74 --height 46 --city-states 9",
        "--speed online --map continents --shape flat --poles poles",
        "--start-era ancient --spectate",
        "--victories science,culture,religious,diplomatic,domination,score",
    )
    wasm = (source / "src/wasm.rs").read_text(encoding="utf-8")
    wasm_fragments = (
        "static PACE: Cell<u64> = const { Cell::new(1_000) };",
        "num_players: 6,",
        "map_script: MapScript::Continents,",
        "map_topology: MapTopology::Flat,",
        "map_poles: MapPoles::Poles,",
        "game_speed: GameSpeed::Online,",
        "start_era: 0,",
        "science: true,",
        "culture: true,",
        "religious: true,",
        "diplomatic: true,",
        "domination: true,",
        "score: true,",
        "spectate: true,",
        "teams: Vec::new(),",
    )
    missing = [fragment for fragment in rust_fragments if fragment not in launcher]
    missing += [fragment for fragment in wasm_fragments if fragment not in wasm]
    if missing:
        raise DesktopAppError("desktop default contract drifted: " + ", ".join(missing))


def build_artifacts(
    repo: pathlib.Path,
    revision: Revision,
    state_dir: pathlib.Path,
    template: pathlib.Path,
) -> BuildArtifacts:
    build_root = state_dir / ("build-" + revision.short + "-" + compact_stamp())
    source = build_root / "source"
    native_target = build_root / "native-target"
    wasm_target = build_root / "wasm-target"
    wasm_site = build_root / "wasm-site"
    build_root.mkdir(parents=True)
    run(("git", "worktree", "add", "--detach", str(source), revision.commit), cwd=repo)
    try:
        verify_default_contract(source, template)
        env = build_environment(os.environ)
        native_built_at = utc_now()
        native_env = dict(env)
        native_env.update(
            {
                "CARGO_BUILD_JOBS": "2",
                "CARGO_TARGET_DIR": str(native_target),
                "CIVVIS_COMMIT": revision.commit,
                "CIVVIS_COMMIT_TIME": revision.committed_at,
                "CIVVIS_BUILT_AT": native_built_at,
            }
        )
        run(("cargo", "build", "--release", "--locked", "--bin", "civvis"), cwd=source, env=native_env)
        native_binary = native_target / "release/civvis"
        if not native_binary.is_file():
            raise DesktopAppError("native build did not produce civvis")

        wasm_env = dict(env)
        wasm_env.update({"CARGO_BUILD_JOBS": "2", "CARGO_TARGET_DIR": str(wasm_target)})
        run(("./beta/publish.sh", "--out", str(wasm_site)), cwd=source, env=wasm_env)
        manifest = json.loads((wasm_site / "beta/build.json").read_text(encoding="utf-8"))
        if manifest.get("commit") != revision.commit:
            raise DesktopAppError("WASM manifest names a different revision")
        if not (wasm_site / "beta/civvis.wasm").is_file():
            raise DesktopAppError("WASM build did not produce civvis.wasm")
        serve_script = build_root / "serve.py"
        shutil.copy2(source / "beta/serve.py", serve_script)
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
            version=version_match.group(1),
        )
    finally:
        if source.exists():
            run(("git", "worktree", "remove", str(source)), cwd=repo)


def render_launcher(template: str, spec: AppSpec, revision: Revision, built_at: str) -> str:
    values = {
        "MODE": spec.mode,
        "LABEL": spec.label,
        "COMMIT": revision.commit,
        "COMMIT_TIME": revision.committed_at,
        "BUILT_AT": built_at,
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
        launcher = macos / spec.executable_name
        launcher.write_text(render_launcher(template, spec, artifacts.revision, built_at), encoding="utf-8")
        launcher.chmod(0o755)
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
        if spec.mode == "rust":
            binary = resources / ("civvis-" + artifacts.revision.commit)
            shutil.copy2(artifacts.native_binary, binary)
            binary.chmod(0o755)
        else:
            shutil.copytree(artifacts.wasm_site, resources / "site")
            shutil.copy2(artifacts.serve_script, resources / "serve.py")
            (resources / "serve.py").chmod(0o755)

        run(("/usr/bin/xattr", "-cr", str(app)))
        run(("/usr/bin/codesign", "--force", "--deep", "--sign", "-", str(app)))
        run(("/usr/bin/codesign", "--verify", "--deep", "--strict", "--verbose=2", str(app)))
        run(("/bin/zsh", "-n", str(launcher)))
        run(("/usr/bin/plutil", "-lint", str(app / "Contents/Info.plist")))

    if (apps_root / "WASM CIVVIS.app/Contents/Resources/serve.py").read_bytes() != artifacts.serve_script.read_bytes():
        raise DesktopAppError("the staged WASM server differs from the selected source")
    return apps_root


def commit_from_build_note(app: pathlib.Path) -> str:
    note = app / "Contents/Resources/BUILD.txt"
    if not note.is_file():
        return "unknown"
    match = re.search(r"^Commit: ([0-9a-f]{40})$", note.read_text(encoding="utf-8"), re.MULTILINE)
    return match.group(1)[:7] if match else "unknown"


def unused_archive(previous: pathlib.Path, spec: AppSpec, short: str, stamp: str) -> pathlib.Path:
    stem = spec.label.replace(" ", "-") + "-" + short + "-" + stamp
    candidate = previous / (stem + ".app")
    suffix = 2
    while candidate.exists():
        candidate = previous / (stem + "-" + str(suffix) + ".app")
        suffix += 1
    return candidate


def install_apps(apps_root: pathlib.Path, desktop: pathlib.Path, state_dir: pathlib.Path) -> InstalledSwap:
    desktop.mkdir(parents=True, exist_ok=True)
    previous = state_dir / "previous"
    previous.mkdir(parents=True, exist_ok=True)
    stamp = compact_stamp()
    incoming: List[Tuple[pathlib.Path, pathlib.Path]] = []
    archived: List[Tuple[pathlib.Path, pathlib.Path]] = []
    installed: List[Tuple[pathlib.Path, pathlib.Path]] = []
    try:
        for spec in APPS:
            staged = apps_root / spec.bundle_name
            temporary = desktop / ("." + spec.bundle_name + ".incoming-" + str(os.getpid()))
            if temporary.exists():
                raise DesktopAppError("stale incoming bundle exists: " + str(temporary))
            shutil.copytree(staged, temporary, copy_function=shutil.copy2)
            incoming.append((temporary, staged))

        for spec in APPS:
            target = desktop / spec.bundle_name
            if target.exists():
                archive = unused_archive(previous, spec, commit_from_build_note(target), stamp)
                shutil.move(str(target), str(archive))
                archived.append((archive, target))

        for spec, (temporary, staged) in zip(APPS, incoming):
            target = desktop / spec.bundle_name
            shutil.move(str(temporary), str(target))
            installed.append((target, staged))
    except Exception:
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
    )


def rollback_install(swap: InstalledSwap, relaunch: bool = False) -> None:
    # The generated source bundles remain in the build directory, so removing
    # a failed installed copy loses nothing. Restore every previous bundle to
    # its original Desktop name before asking it to launch again.
    for target in reversed(swap.targets):
        if target.exists():
            shutil.rmtree(target)
    for archive, target in reversed(swap.archives):
        if archive.exists() and not target.exists():
            shutil.move(str(archive), str(target))
    if relaunch and swap.targets:
        for spec in APPS:
            target = swap.targets[0].parent / spec.bundle_name
            if target.exists():
                subprocess.run(("/usr/bin/open", "-n", str(target)), check=False)


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


def launcher_metadata(app: pathlib.Path, executable: str) -> dict:
    text = (app / "Contents/MacOS" / executable).read_text(encoding="utf-8")
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
        run(("/bin/zsh", "-n", str(app / "Contents/MacOS" / spec.executable_name)))
        metadata[spec.mode] = launcher_metadata(app, spec.executable_name)

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

    manifest = json.loads(
        (desktop / "WASM CIVVIS.app/Contents/Resources/site/beta/build.json").read_text(encoding="utf-8")
    )
    if manifest.get("commit") != wasm["commit"] or manifest.get("built_at") != wasm["built_at"]:
        raise DesktopAppError("WASM launcher and manifest provenance differ")
    selected_serve = subprocess.check_output(
        ("git", "show", rust["commit"] + ":beta/serve.py"), cwd=str(repo)
    )
    if (desktop / "WASM CIVVIS.app/Contents/Resources/serve.py").read_bytes() != selected_serve:
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
        rust_listener = listener(repo, 8785)
        wasm_listener = listener(repo, 8790)
        if rust_listener["ppid"] != 1 or wasm_listener["ppid"] != 1:
            raise DesktopAppError("a desktop listener is still attached to its launcher")
        report.update(
            {
                "live_rust": live_rust,
                "live_wasm": live_wasm,
                "listeners": {"rust": rust_listener, "wasm": wasm_listener},
                "routes": {"rust": rust_routes, "wasm": wasm_routes},
            }
        )
    return report


def parse_args(argv: Optional[Sequence[str]] = None) -> argparse.Namespace:
    script = pathlib.Path(__file__)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("build", "install", "verify"))
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
    parser.add_argument("--max-build-age-minutes", type=int, default=30)
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
            print(json.dumps(report, indent=2, sort_keys=True))
            return 0

        with install_lock(state_dir):
            revision = resolve_revision(repo, args.ref, not args.no_fetch)
            artifacts = build_artifacts(repo, revision, state_dir, template)
            apps_root = stage_apps(repo, artifacts, template)
            print("staged {} and {} from {} at {}".format(APPS[0].bundle_name, APPS[1].bundle_name, revision.short, apps_root))
            if args.action == "build":
                return 0
            swap = install_apps(apps_root, desktop, state_dir)
            for path, _ in swap.archives:
                print("archived", path)
            try:
                if not args.no_launch:
                    launch_apps(desktop, artifacts)
                report = verify_installed(
                    repo,
                    desktop,
                    revision,
                    args.max_build_age_minutes,
                    not args.no_launch,
                )
            except Exception:
                print("new desktop apps failed verification; restoring the archived pair", file=sys.stderr)
                rollback_install(swap, relaunch=not args.no_launch)
                raise
            print(json.dumps(report, indent=2, sort_keys=True))
            return 0
    except (DesktopAppError, subprocess.CalledProcessError, OSError, ValueError) as error:
        print("desktop app error:", error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
