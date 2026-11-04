"""Bounded evidence capture before a frozen or crashed native game is relaunched."""

from __future__ import annotations

import hashlib
import json
import shutil
import time
from pathlib import Path


def snapshot(source: Path, destination: Path, *, file_limit: int = 32 * 1024 * 1024,
             total_limit: int = 128 * 1024 * 1024) -> Path:
    """Preserve direct log files and explain omissions; never replace a snapshot.

    Call after owned-game teardown and before the next launch clears Logs.
    Limits bound reads as well as disk use. A changing source is identified in
    the manifest; hashes describe the copied bytes, not a claimed atomic view.
    Whole files take priority. Remaining space preserves tails of oversized
    files; their source size and byte offset identify the truncated capture.
    A tail may begin partway through a record.
    Directory/manifest errors propagate so the caller can report them without
    preventing recovery. Individual file failures are recorded and skipped.
    """
    manifest = destination / "manifest.json"
    if destination.exists():
        if not manifest.is_file():
            raise OSError(f"incomplete snapshot already exists: {destination}")
        return manifest
    paths = sorted(source.iterdir())
    destination.mkdir(parents=True)
    entries = []
    tails = []
    used = 0
    for path in paths:
        entry = {"name": path.name}
        try:
            if path.name == manifest.name:
                entry["skipped"] = "reserved_manifest_name"
            elif path.is_symlink() or not path.is_file():
                entry["skipped"] = "not_regular_file"
            else:
                before = path.stat()
                limit = min(file_limit, max(0, total_limit - used))
                if before.st_size > limit:
                    entry.update(skipped="byte_limit", source_bytes=before.st_size)
                    tails.append((path, entry))
                else:
                    with path.open("rb") as stream:
                        data = stream.read(limit + 1)
                    after = path.stat()
                    if len(data) > limit:
                        entry["skipped"] = "grew_past_byte_limit"
                    else:
                        (destination / path.name).write_bytes(data)
                        used += len(data)
                        entry.update(bytes=len(data), sha256=hashlib.sha256(data).hexdigest(),
                                     source_mtime_ns=before.st_mtime_ns,
                                     changed_during_read=(before.st_size != after.st_size
                                                         or before.st_mtime_ns != after.st_mtime_ns))
        except OSError as error:
            entry["error"] = str(error)
            # A failed write can leave a partial file. It is not evidence and
            # must not accumulate outside the successful-copy byte budget.
            (destination / path.name).unlink(missing_ok=True)
        entries.append(entry)
    # Do not let an early large file displace smaller complete logs. Revisit
    # omitted files only after all whole-file candidates have had a chance.
    for path, entry in tails:
        limit = min(file_limit, max(0, total_limit - used))
        if limit <= 0:
            break
        try:
            if path.is_symlink() or not path.is_file():
                entry["skipped"] = "not_regular_file"
                continue
            entry.pop("skipped", None)
            before = path.stat()
            offset = max(0, before.st_size - limit)
            with path.open("rb") as stream:
                stream.seek(offset)
                data = stream.read(limit)
            after = path.stat()
            (destination / path.name).write_bytes(data)
            used += len(data)
            entry.update(source_bytes=before.st_size, start_offset=offset,
                         truncated=offset > 0 or len(data) < before.st_size,
                         bytes=len(data), sha256=hashlib.sha256(data).hexdigest(),
                         source_mtime_ns=before.st_mtime_ns,
                         changed_during_read=(before.st_size != after.st_size
                                             or before.st_mtime_ns != after.st_mtime_ns))
        except OSError as error:
            entry["error"] = str(error)
            (destination / path.name).unlink(missing_ok=True)
    manifest.write_text(json.dumps({"source": str(source), "copied_bytes": used,
                                    "file_limit": file_limit, "total_limit": total_limit,
                                    "files": entries}, indent=2) + "\n")
    return manifest


def copy_crash_reports(reports: Path, destination: Path, *, since: float,
                       prefix: str = "Civ6_Exe_Child-", wait_s: float = 0.0,
                       poll_s: float = 0.5, limit: int = 4,
                       clock=None) -> list[str]:
    """Copy macOS crash reports written for this game segment; return their names.

    ★ A NATIVE CRASH IS NAMED BY ITS STACK, AND macOS KEEPS THE STACK. Live
    civvis-20261104T152818Z died at t280 with a SIGSEGV on Civ's own "Game
    Core" thread (a null node in a list walk under `GameCore RECV`), and the
    only copy of that stack was the `.ips` in the user's DiagnosticReports,
    pruned by the OS on its own schedule. ReportCrash writes it a moment
    after the process dies, so the caller may give it ``wait_s`` to land.
    Only reports modified at or after ``since`` (the segment's launch) are
    taken, newest first, at most ``limit``. Existing copies are never
    replaced. Errors on individual files are skipped; a missing reports
    directory yields an empty list.
    """
    clock = clock or time

    def candidates() -> list[Path]:
        try:
            found = [path for path in reports.iterdir()
                     if path.name.startswith(prefix) and path.suffix == ".ips"
                     and path.is_file() and path.stat().st_mtime >= since]
        except OSError:
            return []
        return sorted(found, key=lambda path: path.stat().st_mtime, reverse=True)[:limit]

    deadline = clock.time() + max(0.0, wait_s)
    found = candidates()
    while not found and clock.time() < deadline:
        clock.sleep(poll_s)
        found = candidates()
    copied = []
    for path in found:
        target = destination / path.name
        if target.exists():
            copied.append(path.name)
            continue
        try:
            destination.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, target)
        except OSError:
            # A failed copy can leave a partial file; it is not evidence.
            target.unlink(missing_ok=True)
            continue
        copied.append(path.name)
    return copied
