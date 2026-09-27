"""Preserve the exact autosave selected for a native-game continuation."""

from __future__ import annotations

from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import stat


def _identity(info: os.stat_result) -> tuple[int, ...]:
    return (info.st_dev, info.st_ino, info.st_size, info.st_mtime_ns,
            info.st_ctime_ns)


def snapshot(source: Path, destination: Path, *, root_tag: str, frozen_tag: str,
             continuation_tag: str, last_turn: int | None,
             file_limit: int = 64 * 1024 * 1024) -> Path:
    """Copy a complete, stable save after teardown and before launching recovery.

    A manifest is published only after the archived bytes pass a checksum
    readback. Existing destinations are never replaced. Oversized, empty,
    nonregular or changing inputs fail explicitly rather than leaving a
    truncated save that looks replayable. Failures remove only files created
    by this call; the climb reports the error and retains its original reload.
    ``last_turn`` is the last observed game turn, not the save's numeric suffix.
    """
    if source.suffix.lower() != ".civ6save":
        raise OSError(f"not a Civ6 save: {source}")
    before = source.lstat()
    if not stat.S_ISREG(before.st_mode):
        raise OSError(f"not a regular save: {source}")
    if not 0 < before.st_size <= file_limit:
        raise OSError(f"save size {before.st_size} outside 1..{file_limit}: {source}")
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    with os.fdopen(os.open(source, flags), "rb") as stream:
        if _identity(os.fstat(stream.fileno())) != _identity(before):
            raise OSError(f"save changed before copy: {source}")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.mkdir()  # Exclusive ownership; never overwrite earlier evidence.
        archived = destination / source.name
        manifest = destination / "manifest.json"
        try:
            digest = hashlib.sha256()
            copied = 0
            with archived.open("xb") as output:
                while copied < before.st_size:
                    chunk = stream.read(min(1024 * 1024, before.st_size - copied))
                    if not chunk:
                        raise OSError(f"save truncated during copy: {source}")
                    output.write(chunk)
                    digest.update(chunk)
                    copied += len(chunk)
                if stream.read(1):
                    raise OSError(f"save grew during copy: {source}")
                if (_identity(os.fstat(stream.fileno())) != _identity(before)
                        or _identity(source.lstat()) != _identity(before)):
                    raise OSError(f"save changed during copy: {source}")
                output.flush()
                os.fsync(output.fileno())
            check = hashlib.sha256()
            with archived.open("rb") as output:
                remaining = copied
                while remaining:
                    chunk = output.read(min(1024 * 1024, remaining))
                    if not chunk:
                        break
                    check.update(chunk)
                    remaining -= len(chunk)
                if remaining or output.read(1):
                    raise OSError(f"save checksum readback failed: {archived}")
            if check.digest() != digest.digest():
                raise OSError(f"save checksum readback failed: {archived}")
            report = {
                "schema": 1,
                "captured_at": datetime.now(timezone.utc).isoformat(),
                "root_tag": root_tag,
                "frozen_tag": frozen_tag,
                "continuation_tag": continuation_tag,
                "last_observed_turn": last_turn,
                "source": str(source.absolute()),
                "source_mtime_ns": before.st_mtime_ns,
                "source_ctime_ns": before.st_ctime_ns,
                "archive": source.name,
                "bytes": copied,
                "sha256": digest.hexdigest(),
            }
            with manifest.open("x") as output:
                output.write(json.dumps(report, indent=2) + "\n")
                output.flush()
                os.fsync(output.fileno())
        except OSError:
            for path in (manifest, archived):
                try:
                    path.unlink(missing_ok=True)
                except OSError:
                    pass
            try:
                destination.rmdir()
            except OSError:
                pass
            raise
    return manifest
