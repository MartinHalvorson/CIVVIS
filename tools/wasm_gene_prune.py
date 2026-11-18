#!/usr/bin/env python3
"""Compile the browser engine without genes no browser seat can turn on.

`beta/publish.sh` runs this over a scratch copy of the source before building
the WebAssembly engine. Every opt-in gene is a `bool` on `AdvancedAi` or
`BasicAi` read with `self.flag`, and the gene registry carries 980-odd of
them; civvis.ai seats the deployment genome and its site experiments, so most
are false in every browser game, yet each one's code ships. Rewriting the
reads of those flags to `false` lets the compiler delete that code. Measured
2026-11-18 at `241674c`: the module went 18.37 -> 13.95 MB (3.97 -> 3.03 MB
brotli) with identical trace and save hashes on full games and no change in
engine time.

The rewrite is only sound for a flag that can never be true in the browser,
so the analysis keeps any gene it cannot prove off, and fails safe:

* only `Kind::OptIn` rows are candidates — every other kind can be on;
* a tag in `DEPLOYMENT_GENOME` or `SITE_EXPERIMENT_GENES`, or named by a
  string literal anywhere else in shipping code or embedded data (roster
  rows, rated agents), is kept: something may enable it by name;
* a flag declared on any struct other than `AdvancedAi`/`BasicAi`, written
  with anything but `false` outside its own toggle, borrowed `&mut`, assigned
  through a tuple, initialised to anything but `false`, or whose enable
  function is called anywhere but its own registry row, is kept — that is how
  `science_building_first` is switched on mid-turn for a science seat.

Nothing is written until the whole analysis has run; a registry this cannot
read leaves the tree untouched and exits 0 with the reason, so a publish
still produces a correct (larger) engine. Test code (`tests.rs`, test
directories, every `#[cfg(test)]` item) and `src/bin/` are never rewritten.

    tools/wasm_gene_prune.py <source-tree> [--report report.json] [--dry-run]
"""

from __future__ import annotations

import argparse
import bisect
import json
import pathlib
import re
import sys

PLAIN_FIELD = re.compile(r"^[a-z_][a-z0-9_]*$")
SEAT_STRUCTS = {"AdvancedAi", "BasicAi"}


# --------------------------------------------------------------- the scanner


def literal_end(text: str, i: int) -> int | None:
    """Index just past a comment, string or char literal starting at `i`."""
    if text.startswith("//", i):
        end = text.find("\n", i)
        return len(text) if end < 0 else end
    if text.startswith("/*", i):
        depth, j = 0, i
        while j < len(text):
            if text.startswith("/*", j):
                depth, j = depth + 1, j + 2
            elif text.startswith("*/", j):
                depth, j = depth - 1, j + 2
                if depth == 0:
                    return j
            else:
                j += 1
        return j
    word_before = i > 0 and (text[i - 1].isalnum() or text[i - 1] == "_")
    raw = re.match(r'b?r(#*)"', text[i:i + 24])
    if raw and not word_before:
        close = '"' + raw.group(1)
        end = text.find(close, i + len(raw.group(0)))
        return len(text) if end < 0 else end + len(close)
    if text[i] == '"' or (text.startswith('b"', i) and not word_before):
        j = i + (2 if text[i] == "b" else 1)
        while j < len(text):
            if text[j] == "\\":
                j += 2
            elif text[j] == '"':
                return j + 1
            else:
                j += 1
        return j
    if text[i] == "'":
        char = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.[^']{0,7}|[^\\'])'", text[i:i + 12])
        if char:
            return i + len(char.group(0))
    return None


# Where a comment or literal may begin; `literal_end` decides whether it does.
LITERAL_START = re.compile(r"""//|/\*|(?<!\w)b?r#*"|(?<!\w)b"|"|'""")
NOT_NEWLINE = re.compile(r"[^\n]")


def blank(text: str) -> str:
    """`text` with comments and literals blanked to spaces, newlines kept."""
    out, kept, i = [], 0, 0
    while True:
        found = LITERAL_START.search(text, i)
        if not found:
            break
        end = literal_end(text, found.start())
        if end is None:
            i = found.start() + 1
            continue
        out.append(text[kept:found.start()])
        out.append(NOT_NEWLINE.sub(" ", text[found.start():end]))
        kept = i = end
    out.append(text[kept:])
    return "".join(out)


def test_spans(code: str) -> list[tuple[int, int]]:
    """Spans of every `#[cfg(test)]` item in already-blanked code."""
    spans = []
    for found in re.finditer(r"#\[cfg\(test\)\]", code):
        depth, j = 0, found.end()
        while j < len(code):
            if code[j] == ";" and depth == 0:
                break
            if code[j] == "{":
                depth += 1
            elif code[j] == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        spans.append((found.start(), j + 1))
    return spans


def shipping(code: str) -> str:
    """Blanked code with its test items blanked too."""
    chars = list(code)
    for start, end in test_spans(code):
        for k in range(start, min(end, len(chars))):
            if chars[k] != "\n":
                chars[k] = " "
    return "".join(chars)


def is_test_path(relative: pathlib.Path) -> bool:
    name = relative.name
    return (name == "tests.rs" or name.endswith("_tests.rs") or name.endswith("_test.rs")
            or "tests" in relative.parts)


# -------------------------------------------------------------- the analysis


class Source:
    """The crate's shipping Rust, as written and with literals blanked."""

    def __init__(self, root: pathlib.Path):
        self.root = root
        self.files: dict[pathlib.Path, str] = {}
        self.code: dict[pathlib.Path, str] = {}
        for path in sorted((root / "src").rglob("*.rs")):
            relative = path.relative_to(root / "src")
            if relative.parts[0] == "bin" or is_test_path(relative):
                continue
            text = path.read_text(encoding="utf-8")
            self.files[path] = text
            self.code[path] = shipping(blank(text))


def registry(root: pathlib.Path) -> list[tuple[str, str, str, str]]:
    """(tag, field, kind, enable fn) for every row of the gene registry."""
    text = (root / "src/ai/advanced/genes.rs").read_text(encoding="utf-8")
    return re.findall(
        r'Gene \{ tag: "([^"]+)", field: "([^"]+)", kind: Kind::(\w+)(?:\([^)]*\))?, '
        r"enable: AdvancedAi::(\w+),", text)


def string_list(text: str, name: str) -> set[str]:
    found = re.search(name + r"\s*:\s*&\[&str\]\s*=\s*&\[(.*?)\];", text, re.S)
    if not found:
        raise ValueError(f"{name} is not where it was")
    return set(re.findall(r'"([^"]+)"', found.group(1)))


def struct_spans(code: str) -> list[tuple[str, int, int]]:
    """(name, body start, body end) of every braced struct in blanked code."""
    spans = []
    for found in re.finditer(r"\bstruct\s+(\w+)[^;{(]*\{", code):
        depth, j = 1, found.end()
        while j < len(code) and depth:
            if code[j] == "{":
                depth += 1
            elif code[j] == "}":
                depth -= 1
            j += 1
        spans.append((found.group(1), found.end(), j))
    return spans


def owner_at(spans: list[tuple[str, int, int]], at: int) -> str | None:
    inside = [(start, name) for name, start, end in spans if start <= at < end]
    return max(inside)[1] if inside else None


def enclosing_paren_assigned(code: str, at: int) -> bool:
    """Whether `at` sits inside a parenthesised/bracketed group assigned to."""
    depth, j = 0, at
    while j < len(code):
        ch = code[j]
        if ch in "([{":
            depth += 1
        elif ch in ")]}":
            if depth == 0:
                if ch == "}":
                    return False
                return re.match(r"\s*=(?![=>])", code[j + 1:j + 8]) is not None
            depth -= 1
        elif ch == ";" and depth == 0:
            return False
        j += 1
    return False


def analyse(root: pathlib.Path) -> dict:
    rows = registry(root)
    if len(rows) < 100:
        raise ValueError(f"read only {len(rows)} gene registry rows")
    genes_text = (root / "src/ai/advanced/genes.rs").read_text(encoding="utf-8")
    site_text = (root / "src/ai/advanced/site_deployment.rs").read_text(encoding="utf-8")
    deployed = string_list(genes_text, "DEPLOYMENT_GENOME")
    site = string_list(site_text, "SITE_EXPERIMENT_GENES")

    source = Source(root)
    genes_path = root / "src/ai/advanced/genes.rs"
    named = set()
    for path, text in source.files.items():
        if path == genes_path:
            continue
        code = source.code[path]
        # A string literal is blank in `code`; read it from the text at the
        # same offsets so only literals in shipping code count.
        for found in re.finditer(r'"([a-z0-9][a-z0-9-]*)"', text):
            if code[found.start()] == " " and code[found.end() - 1] == " ":
                named.add(found.group(1))
    for folder in ("data", "mods"):
        for path in sorted((root / folder).rglob("*.json")):
            named |= set(re.findall(r'"([a-z0-9][a-z0-9-]*)"', path.read_text(encoding="utf-8")))

    kept: dict[str, str] = {}
    candidates: dict[str, tuple[str, str]] = {}
    for tag, field, kind, enable in rows:
        reason = None
        if kind != "OptIn":
            reason = f"kind {kind}"
        elif tag in deployed:
            reason = "in DEPLOYMENT_GENOME"
        elif tag in site:
            reason = "in SITE_EXPERIMENT_GENES"
        elif tag in named:
            reason = "named elsewhere in shipping code or data"
        elif not PLAIN_FIELD.match(field):
            reason = "not a plain field"
        if reason:
            kept[field] = reason
        elif field not in kept:
            candidates[field] = (tag, enable)
    for field in list(candidates):
        if field in kept:
            del candidates[field]

    if not candidates:
        return {"rows": len(rows), "pruned": {}, "kept": kept}
    # Generic identifiers checked against the candidates: a 700-way regex
    # alternation is what Python's `re` is slowest at.
    access = re.compile(r"\.([a-z_][a-z0-9_]*)\b")
    declared = re.compile(r"(?<![\w.])([a-z_][a-z0-9_]*)\s*:(?!:)")

    def keep(field: str, reason: str) -> None:
        if field in candidates:
            del candidates[field]
            kept[field] = reason

    for path, code in source.code.items():
        spans = struct_spans(code)
        fns = [(found.start(), found.group(1)) for found in re.finditer(r"\bfn\s+(\w+)", code)]
        fn_starts = [start for start, _ in fns]
        fn_names = [name for _, name in fns]
        for found in declared.finditer(code):
            field = found.group(1)
            if field not in candidates:
                continue
            line_start = code.rfind("\n", 0, found.start()) + 1
            prefix = code[line_start:found.start()].strip()
            after = code[found.end():found.end() + 120].lstrip()
            owner = owner_at(spans, found.start())
            if owner is not None and re.fullmatch(r"(pub(\([a-z]+\))?)?", prefix):
                # A field declaration. Only the two seat agents may carry it,
                # or a read of `.field` could be some other struct's.
                if owner not in SEAT_STRUCTS:
                    keep(field, f"also a field of {owner}")
                elif not after.startswith("bool"):
                    keep(field, f"not a bool on {owner}")
                continue
            # A struct literal, pattern or binding: `field: value`.
            value = re.match(r"([^,}\n]*)", after).group(1).strip()
            if value != "false":
                keep(field, f"`{field}: {value[:40]}` in {path.name}")
        for found in access.finditer(code):
            field = found.group(1)
            if field not in candidates:
                continue
            tail = code[found.end():found.end() + 12]
            head = code[max(0, found.start() - 160):found.start()]
            written = re.match(r"\s*(?:[|&^+\-*/]?=)(?![=>])", tail)
            if written:
                at = bisect.bisect_right(fn_starts, found.start()) - 1
                own = at >= 0 and fn_names[at] == candidates[field][1]
                rhs = code[found.end():code.find(";", found.end())].split("=", 1)[-1].strip()
                if not (own or rhs == "false"):
                    keep(field, f"written outside its toggle in {path.name}")
                continue
            if re.search(r"&\s*mut\s+[\w.]*$", head):
                keep(field, f"borrowed mutably in {path.name}")
            elif enclosing_paren_assigned(code, found.end()):
                keep(field, f"assigned through a tuple in {path.name}")
    enables = {enable: field for field, (_, enable) in candidates.items()}
    name = re.compile(r"\b(enable_[a-z0-9_]+)\b")
    for path, code in source.code.items():
        if path == genes_path:
            continue
        for found in name.finditer(code):
            if found.group(1) not in enables:
                continue
            # Its definition: `fn enable_x` or a `toggles!` entry `flag => enable_x`.
            if re.search(r"(fn\s+|=>\s*)$", code[max(0, found.start() - 8):found.start()]):
                continue
            keep(enables[found.group(1)], f"{found.group(1)} is called outside the registry")

    return {"rows": len(rows), "pruned": candidates, "kept": kept}


# -------------------------------------------------------------- the rewrite


def rewrite(root: pathlib.Path, pruned: dict) -> tuple[dict[pathlib.Path, str], int]:
    """Every shipping file with reads of a pruned flag replaced by `false`."""
    if not pruned:
        return {}, 0
    source = Source(root)
    read = re.compile(
        r"(?<![\w.])[a-z_][a-z0-9_]*(?:\.[a-z_][a-z0-9_]*)*\.([a-z_][a-z0-9_]*)\b"
        # Not a write, a call, a path, a macro, or a `toggles!` entry
        # (`base.flag => enable_flag, disable_flag;`).
        r"(?!\s*(?:[|&^+\-*/]?=(?!=)|\(|::|!))")
    changed, reads = {}, 0
    for path, text in source.files.items():
        code = source.code[path]
        pieces, last, count = [], 0, 0
        for found in read.finditer(code):
            if found.group(1) not in pruned:
                continue
            # The blanked code and the text share offsets; only rewrite what
            # is code in both.
            if text[found.start():found.end()] != code[found.start():found.end()]:
                continue
            pieces.append(text[last:found.start()])
            pieces.append("false")
            last = found.end()
            count += 1
        if count:
            pieces.append(text[last:])
            changed[path] = "".join(pieces)
            reads += count
    return changed, reads


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("tree", type=pathlib.Path)
    parser.add_argument("--report", type=pathlib.Path)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)
    try:
        result = analyse(args.tree)
        changed, reads = rewrite(args.tree, result["pruned"])
    except (OSError, ValueError, UnicodeDecodeError) as error:
        print(f"   gene prune skipped, the engine keeps every gene: {error}")
        if args.report:
            args.report.write_text(json.dumps({"pruned_genes": 0, "skipped": str(error)}) + "\n")
        return 0
    if not args.dry_run:
        for path, text in changed.items():
            path.write_text(text, encoding="utf-8")
    pruned = sorted(tag for tag, _ in result["pruned"].values())
    print(f"   left out {len(pruned)} of {result['rows']} genes no browser seat can turn on "
          f"({reads} reads in {len(changed)} files)")
    if args.report:
        args.report.write_text(json.dumps({
            "pruned_genes": len(pruned), "reads": reads, "files": len(changed),
            "pruned": pruned, "kept": result["kept"]}, indent=1, sort_keys=True) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
