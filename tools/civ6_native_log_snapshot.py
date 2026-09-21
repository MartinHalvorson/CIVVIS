"""Bounded evidence capture before a frozen native game is relaunched."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


def snapshot(source: Path, destination: Path, *, file_limit: int = 32 * 1024 * 1024,
             total_limit: int = 128 * 1024 * 1024) -> Path:
    """Preserve direct log files and explain omissions; never replace a snapshot.

    Call after owned-game teardown and before the next launch clears Logs.
    Limits bound reads as well as disk use. A changing source is identified in
    the manifest; hashes describe the copied bytes, not a claimed atomic view.
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
    manifest.write_text(json.dumps({"source": str(source), "copied_bytes": used,
                                    "file_limit": file_limit, "total_limit": total_limit,
                                    "files": entries}, indent=2) + "\n")
    return manifest
