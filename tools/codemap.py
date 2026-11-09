#!/usr/bin/env python3
"""Where does this live? A map of the code, read out of the code.

This repository has 600 Rust modules, 900 genes and most of a million lines, so
a question like "what decides where a Campus goes" usually starts with a long
search. This answers it from what the code says about itself, and writes no
file: a committed index would change with every gene and collide with every
open PR, and it would be stale whenever anybody forgot to refresh it.

Usage::

    python3 tools/codemap.py settle site    # modules, tools and docs matching every word
    python3 tools/codemap.py --gene anvil   # one gene: flag, reads, armed live or not
    python3 tools/codemap.py --turn         # the order one AI seat plays a turn
    python3 tools/codemap.py --check        # every shipping Rust module says what it is for

``test_codemap.py`` holds the tree to ``--check``.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent

GENES = "src/ai/advanced/genes.rs"
TURN_FILE = "src/ai/advanced.rs"
TURN_FN = "fn take_turn_inner"
LIVE_GENES = "deploy/live-force-on.txt"
LEDGER = "docs/gene_ledger.json"

_TEST_MODULE = re.compile(
    r"#\[cfg\(test\)\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([a-z]+\))?\s+)?mod\s+(\w+)\s*;"
)
_INLINE_TEST = re.compile(r"#\[cfg\(test\)\]\s*(?:pub(?:\([a-z]+\))?\s+)?mod\s+\w+\s*\{")
_INCLUDED = re.compile(r'\binclude!\(\s*"([^"]+\.rs)"\s*\)')
_RUST_FN = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?fn\s+([a-z_][a-z0-9_]*)", re.M)
_PY_FN = re.compile(r"^\s*def\s+([a-zA-Z_][a-zA-Z0-9_]*)", re.M)
_LUA_FN = re.compile(r"^\s*(?:local\s+)?function\s+([A-Za-z_][A-Za-z0-9_.:]*)", re.M)
_JS_FN = re.compile(r"^\s*(?:async\s+)?function\s+([A-Za-z_][A-Za-z0-9_]*)", re.M)
_SENTENCE_END = re.compile(r"(?<=[.!?])\s")
_GENE_ROW = re.compile(r'Gene \{ tag: "([^"]+)", field: "([^"]+)", kind: Kind::(\w+)')


@dataclass
class Entry:
    path: str
    lines: int
    summary: str
    functions: list[tuple[str, int]] = field(default_factory=list)


def _one_sentence(text: str, limit: int = 170) -> str:
    text = " ".join(text.split())
    text = _SENTENCE_END.split(text, maxsplit=1)[0]
    return text if len(text) <= limit else text[: limit - 1].rstrip() + "…"


def _rust_summary(text: str) -> str:
    """The first paragraph of the ``//!`` header, as one sentence."""
    collected: list[str] = []
    for line in text.split("\n"):
        stripped = line.strip()
        if stripped.startswith("//!"):
            body = stripped[3:].strip()
            if not body and collected:
                break
            collected.append(body)
        elif collected or (stripped and not stripped.startswith("//")):
            break
    return _one_sentence(" ".join(collected))


def _python_summary(text: str) -> str:
    match = re.search(r'^(?:#![^\n]*\n)?(?:#[^\n]*\n)*\s*[rR]?"""(.*?)"""', text, re.S)
    if not match:
        return ""
    return _one_sentence(match.group(1).strip().split("\n\n")[0])


def _comment_summary(text: str, marker: str) -> str:
    collected: list[str] = []
    for line in text.split("\n")[:30]:
        stripped = line.strip()
        if stripped.startswith(marker):
            collected.append(stripped[len(marker) :].strip("-= ").strip())
        elif collected:
            break
    return _one_sentence(" ".join(item for item in collected if item))


def _markdown_summary(text: str) -> str:
    title = ""
    for line in text.split("\n"):
        stripped = line.strip()
        if stripped.startswith("# ") and not title:
            title = stripped[2:].strip()
        elif title and stripped and not stripped.startswith(("#", "⚠", "|", "<")):
            return _one_sentence(f"{title}: {stripped}")
    return title


def _functions(pattern: re.Pattern, text: str) -> list[tuple[str, int]]:
    return [(m.group(1), text.count("\n", 0, m.start(1)) + 1) for m in pattern.finditer(text)]


def _relative(path: Path, repo: Path) -> str:
    return path.relative_to(repo).as_posix()


def _test_modules(files: dict[Path, str]) -> set[str]:
    return {name for text in files.values() for name in _TEST_MODULE.findall(text)}


def _is_test_file(path: Path, test_modules: set[str]) -> bool:
    return (
        path.stem == "tests"
        or path.stem.endswith("_tests")
        or "tests" in path.parts
        or path.stem in test_modules
    )


def shipping_rust(repo: Path = REPO) -> dict[str, str]:
    """Every Rust module that ships, repo-relative path to source."""
    files = {
        path: path.read_text(encoding="utf-8", errors="replace")
        for path in sorted((repo / "src").rglob("*.rs"))
    }
    tests = _test_modules(files)
    return {_relative(p, repo): t for p, t in files.items() if not _is_test_file(p, tests)}


def undocumented(repo: Path = REPO) -> list[str]:
    """Shipping modules that do not open with a ``//!`` header.

    A file pulled in with ``include!`` cannot carry inner doc comments, so it
    is held to nothing here.
    """
    sources = shipping_rust(repo)
    included = {name for text in sources.values() for name in _INCLUDED.findall(text)}
    missing = []
    for path, text in sources.items():
        if Path(path).name in {Path(name).name for name in included}:
            continue
        if not re.search(r"^\s*//!\s*\S", text, re.M):
            missing.append(path)
    return missing


def index(repo: Path = REPO) -> list[Entry]:
    entries = []
    for path, text in shipping_rust(repo).items():
        inline = _INLINE_TEST.search(text)
        body = text[: inline.start()] if inline else text
        entries.append(
            Entry(path, text.count("\n") + 1, _rust_summary(text), _functions(_RUST_FN, body))
        )
    for path in sorted((repo / "tools").rglob("*.py")):
        if path.name.startswith("test_"):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        entries.append(
            Entry(
                _relative(path, repo),
                text.count("\n") + 1,
                _python_summary(text),
                _functions(_PY_FN, text),
            )
        )
    for path in sorted((repo / "tools/civ6_control/mod").glob("*.lua")):
        if path.name.endswith("_test.lua"):
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        entries.append(
            Entry(
                _relative(path, repo),
                text.count("\n") + 1,
                _comment_summary(text, "--"),
                _functions(_LUA_FN, text),
            )
        )
    for path in sorted((repo / "web/assets").glob("*.js")):
        text = path.read_text(encoding="utf-8", errors="replace")
        entries.append(
            Entry(
                _relative(path, repo),
                text.count("\n") + 1,
                _comment_summary(text, "//"),
                _functions(_JS_FN, text),
            )
        )
    for path in sorted((repo / "docs").glob("*.md")):
        text = path.read_text(encoding="utf-8", errors="replace")
        entries.append(Entry(_relative(path, repo), text.count("\n") + 1, _markdown_summary(text)))
    return entries


def gene_rows(repo: Path = REPO) -> dict[str, tuple[str, str]]:
    """Gene tag to (flag, kind), from the registry."""
    return {
        tag: (flag, kind)
        for tag, flag, kind in _GENE_ROW.findall((repo / GENES).read_text(encoding="utf-8"))
    }


def search(words: list[str], entries: list[Entry], limit: int = 14) -> list[tuple[int, Entry, list[tuple[str, int]]]]:
    """Entries matching every word, best first.

    A word in the file name counts most, then in the module's own description,
    then in the names of its functions.
    """
    words = [word.lower() for word in words]
    ranked = []
    for entry in entries:
        score = 0
        matched: list[tuple[str, int]] = []
        for word in words:
            in_path = word in Path(entry.path).stem.lower() or word in entry.path.lower()
            in_summary = word in entry.summary.lower()
            hits = [item for item in entry.functions if word in item[0].lower()]
            if not in_path and not in_summary and not hits:
                score = -1
                break
            score += (6 if in_path else 0) + (3 if in_summary else 0) + min(3, len(hits))
            matched += [item for item in hits if item not in matched]
        if score > 0:
            ranked.append((score, entry, matched))
    ranked.sort(key=lambda row: (-row[0], row[1].lines, row[1].path))
    return ranked[:limit]


def print_search(words: list[str], repo: Path = REPO) -> int:
    ranked = search(words, index(repo))
    needle = [word.lower() for word in words]
    genes = [
        (tag, flag)
        for tag, (flag, _) in gene_rows(repo).items()
        if all(word in tag for word in needle)
    ]
    if not ranked and not genes:
        print(f"nothing matches every one of: {' '.join(words)}. Try fewer words.")
        return 1
    for _, entry, matched in ranked:
        print(f"{entry.path}  ({entry.lines} lines)")
        if entry.summary:
            print(f"    {entry.summary}")
        for name, line in matched[:4]:
            print(f"    {name}  :{line}")
    if genes:
        print(f"\ngenes ({len(genes)}; use --gene TAG for one):")
        for tag, _ in genes[:20]:
            print(f"    {tag}")
    return 0


def _flag_reads(flag: str, repo: Path) -> list[tuple[str, int]]:
    read = re.compile(rf"\.{re.escape(flag)}\b(?!\s*=(?![=>]))")
    sites = []
    for path, text in shipping_rust(repo).items():
        inline = _INLINE_TEST.search(text)
        body = text[: inline.start()] if inline else text
        if path.endswith("/treatment_flags.rs"):
            continue
        for match in read.finditer(body):
            line_start = body.rfind("\n", 0, match.start()) + 1
            if body[line_start:match.start()].lstrip().startswith("//"):
                continue
            sites.append((path, body.count("\n", 0, match.start()) + 1))
    return sites


def print_gene(tag: str, repo: Path = REPO) -> int:
    rows = gene_rows(repo)
    if tag not in rows:
        close = sorted(name for name in rows if tag.lower() in name)[:10]
        print(f"no gene is tagged {tag!r}." + (f" Close: {', '.join(close)}" if close else ""))
        return 1
    flag, kind = rows[tag]
    live_path = repo / LIVE_GENES
    armed = tag in re.split(r"[,\s]+", live_path.read_text(encoding="utf-8")) if live_path.exists() else None
    default_on = None
    ledger_path = repo / LEDGER
    if ledger_path.exists():
        for row in json.loads(ledger_path.read_text(encoding="utf-8")).get("genes", []):
            if row.get("tag") == tag:
                default_on = row.get("default_on")
                break
    print(f"gene     {tag}")
    print(f"flag     {flag}")
    print(f"kind     {kind}")
    print(f"armed in {LIVE_GENES}: {'yes' if armed else 'no' if armed is not None else 'unknown'}")
    print(f"ledger default: {'on' if default_on else 'off' if default_on is not None else 'not ranked'}")
    module = repo / "src/ai/advanced" / f"{flag}.rs"
    if module.exists():
        text = module.read_text(encoding="utf-8", errors="replace")
        print(f"module   {_relative(module, repo)}  ({text.count(chr(10)) + 1} lines)")
        summary = _rust_summary(text)
        if summary:
            print(f"         {summary}")
    sites = _flag_reads(flag, repo)
    print(f"\n{len(sites)} read(s) of the flag:")
    for path, line in sites[:30]:
        print(f"    {path}:{line}")
    if not sites:
        outside = []
        for root in ("tools", "tools/civ6_control/mod"):
            for path in sorted((repo / root).glob("*")):
                if path.suffix in {".py", ".lua"} and not path.name.startswith("test_"):
                    if tag in path.read_text(encoding="utf-8", errors="replace"):
                        outside.append(_relative(path, repo))
        if outside:
            print("  no Rust reads the flag; the tag is read by:")
            for path in outside[:10]:
                print(f"    {path}")
    return 0


def _body_of(text: str, signature: str) -> tuple[int, str]:
    """The braced body of the function ``signature`` opens, and its first line."""
    start = text.find(signature)
    if start < 0:
        raise SystemExit(f"codemap: {signature!r} is gone from {TURN_FILE}; point TURN_FN at its replacement")
    opening = text.index("{", start)
    depth = 0
    index_ = opening
    while index_ < len(text):
        char = text[index_]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text.count("\n", 0, opening) + 1, text[opening : index_ + 1]
        index_ += 1
    raise SystemExit(f"codemap: unbalanced braces after {signature!r}")


def turn_order(repo: Path = REPO) -> list[tuple[int, str, str]]:
    """Each step ``take_turn_inner`` calls, in source order: line, method, home."""
    text = (repo / TURN_FILE).read_text(encoding="utf-8", errors="replace")
    first_line, body = _body_of(text, TURN_FN)
    homes: dict[str, str] = {}
    for path, source in shipping_rust(repo).items():
        if not path.startswith("src/ai"):
            continue
        for name, line in _functions(_RUST_FN, source):
            homes.setdefault(name, f"{path}:{line}")
    steps = []
    seen = set()
    for match in re.finditer(r"\bself\.([a-z_][a-z0-9_]*)\(\s*(?:g\b|&\*g|&mut \*g)", body):
        name = match.group(1)
        if name in seen:
            continue
        seen.add(name)
        steps.append((first_line + body.count("\n", 0, match.start()), name, homes.get(name, "?")))
    return steps


def print_turn(repo: Path = REPO) -> int:
    steps = turn_order(repo)
    print(f"{len(steps)} steps of {TURN_FN} in {TURN_FILE}, in source order")
    print("(many sit behind a condition; open the line to see which)\n")
    for line, name, home in steps:
        print(f"{line:6}  {name:48} {home}")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("words", nargs="*", help="match every word against names and descriptions")
    parser.add_argument("--gene", help="one gene's flag, reads and live status")
    parser.add_argument("--turn", action="store_true", help="the order an AI seat plays a turn")
    parser.add_argument("--check", action="store_true", help="fail on an undescribed module")
    args = parser.parse_args()
    if args.check:
        missing = undocumented()
        print(f"{len(shipping_rust())} shipping Rust modules, {len(missing)} without a //! header")
        for path in missing:
            print(f"  {path}")
        return 1 if missing else 0
    if args.gene:
        return print_gene(args.gene)
    if args.turn:
        return print_turn()
    if args.words:
        return print_search(args.words)
    parser.print_help()
    return 0


if __name__ == "__main__":
    sys.exit(main())
