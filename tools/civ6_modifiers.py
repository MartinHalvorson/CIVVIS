#!/usr/bin/env python3
"""Census the shipped Civilization VI ``Modifiers`` tables.

Nearly all Civ VI *content* is data, not code. A leader ability, a belief, a
policy card and a governor promotion are all the same thing: rows in
``Modifiers`` naming a ``ModifierType``, which ``DynamicModifiers`` resolves to
an ``EffectType`` (what happens) and a ``CollectionType`` (who it happens to),
plus ``ModifierArguments`` and an optional ``RequirementSet``.

CIVVIS hardcodes those effects one at a time in Rust. That is a defensible
choice, but it leaves one question unanswered: *how much is left?* This tool
answers it by frequency. It ranks every *active* ``EffectType`` by how many
modifier rows reference it, cross-references ``tools/modifier_coverage.json``
for what CIVVIS does with it, and reports the unmodelled rows as a single
number that should only ever go down.

Usage::

    python tools/civ6_modifiers.py                    # markdown report
    python tools/civ6_modifiers.py --json out.json    # machine-readable
    python tools/civ6_modifiers.py --max-unmodelled N # CI ratchet
    python tools/civ6_modifiers.py --effect ADJUST_PLOT_YIELD   # drill in

It reads the game files and never writes them. Only the report is an artifact,
so the audit is reproducible without redistributing Firaxis data.
"""

from __future__ import annotations

import argparse
import collections
import json
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_fidelity import (  # noqa: E402
    CROSS_EXPANSION,
    LOAD_ORDER,
    PACK_EXCLUDE,
    REMOVE_DATA,
    REPO,
    find_install,
    slug,
    truthy,
)

# Every gameplay file can carry modifiers, so unlike the rules-data audit there
# is no useful filename filter; a full parse of the three load-order
# directories is a couple of seconds.
MODIFIER_TABLES = {"Modifiers", "DynamicModifiers", "ModifierArguments"}

COVERAGE = Path(__file__).resolve().parent / "modifier_coverage.json"

# What a coverage entry can claim. Anything absent from the coverage file
# counts as unmodelled, so new game content shows up rather than hiding.
STATUSES = ("implemented", "partial", "unmodelled", "out-of-scope")


def fields(node) -> dict:
    """Columns of a row-ish node, in both spellings the XML uses."""
    out = dict(node.attrib)
    for child in node:
        out[child.tag] = (child.text or "").strip()
    return out


class Modifiers:
    def __init__(self) -> None:
        self.dynamic: dict[str, dict] = {}
        self.rows: dict[str, dict] = {}
        self.arguments: dict[str, dict[str, str]] = collections.defaultdict(dict)
        self.attachments: dict[str, list[str]] = collections.defaultdict(list)
        self.owners: dict[str, list[str]] = collections.defaultdict(list)
        self.requirements: dict[str, dict] = {}
        self.requirement_arguments: dict[str, dict[str, str]] = collections.defaultdict(dict)
        self.requirement_sets: dict[str, list[str]] = collections.defaultdict(list)
        self.set_kinds: dict[str, str] = {}

    def detach(self, modifier_id: str, table: str, owner: str = "") -> None:
        """Remove matching direct owner bindings from an overlay ``Delete``.

        The same modifier can be granted from more than one owner table.  A
        ``PolicyModifiers`` delete must not silently detach a separate trait
        binding, and a delete containing an owner names only that one binding.
        """
        pairs = zip(
            self.attachments.get(modifier_id, []),
            self.owners.get(modifier_id, []),
        )
        retained = [
            (attached_table, attached_owner)
            for attached_table, attached_owner in pairs
            if attached_table != table or (owner and attached_owner != owner)
        ]
        if retained:
            self.attachments[modifier_id] = [attached_table for attached_table, _ in retained]
            self.owners[modifier_id] = [attached_owner for _, attached_owner in retained]
        else:
            self.attachments.pop(modifier_id, None)
            self.owners.pop(modifier_id, None)

    def active_modifier_ids(self) -> set[str]:
        """Return modifiers reachable from a live owner attachment.

        Expansion overlays often leave a retired row in ``Modifiers`` after
        deleting its owner binding.  It cannot execute.  Conversely,
        ``ATTACH_MODIFIER`` rows point at a child through their ``ModifierId``
        argument, so a direct-attachment-only filter would lose live nested
        effects.  Start with real owner bindings and take that graph closure.
        """
        # A separate table can retain an inert reference after the expansion
        # deletes the modifier row itself.  It is not a runnable root.
        active = {modifier_id for modifier_id in self.attachments if modifier_id in self.rows}
        pending = list(active)
        while pending:
            modifier_id = pending.pop()
            child = self.arguments.get(modifier_id, {}).get("ModifierId")
            if child in self.rows and child not in active:
                active.add(child)
                pending.append(child)
        return active

    @staticmethod
    def _where_set(node) -> tuple[dict, dict]:
        """The match and assignment halves of an `<Update>` element."""
        where, updates = node.find("Where"), node.find("Set")
        return (
            fields(where) if where is not None else {},
            fields(updates) if updates is not None else {},
        )

    @staticmethod
    def _matches(row: dict, where: dict) -> bool:
        return all(row.get(column) == value for column, value in where.items())

    def _update_rows(self, store: dict, key_column: str, where: dict, sets: dict) -> None:
        """Apply one `<Update>` to a store keyed by one column."""
        for key in list(store):
            row = store[key]
            if not self._matches(row, where):
                continue
            row.update(sets)
            renamed = row.get(key_column, key)
            if renamed != key:
                store[renamed] = store.pop(key)

    def _update_pairs(
        self, store: dict, owner_column: str, where: dict, sets: dict, table: str
    ) -> None:
        """Apply one `<Update>` to an (owner -> {Name: Value}) store.

        The store keeps only what the census reads, so an update is matched
        against the row it reconstructs. Anything the shape cannot express is
        reported rather than silently ignored -- an unapplied rebalance is
        exactly the failure this method exists to fix.
        """
        unhandled = set(sets) - {"Name", "Value"}
        if unhandled:
            print(
                f"warning: {table} update sets {sorted(unhandled)}, which this "
                "census does not model",
                file=sys.stderr,
            )
        for owner, entries in store.items():
            for name in list(entries):
                row = {owner_column: owner, "Name": name, "Value": entries[name]}
                if not self._matches(row, where):
                    continue
                renamed = sets.get("Name", name)
                value = sets.get("Value", entries[name])
                if renamed != name:
                    del entries[name]
                entries[renamed] = value

    def apply_file(self, path: Path) -> None:
        try:
            root = ET.parse(path).getroot()
        except ET.ParseError:
            return
        if root.tag != "GameInfo":
            return
        for table in root:
            if table.tag == "DynamicModifiers":
                for node in table:
                    if node.tag == "Update":
                        self._update_rows(self.dynamic, "ModifierType", *self._where_set(node))
                        continue
                    row = fields(node)
                    if node.tag == "Delete":
                        self.dynamic.pop(row.get("ModifierType", ""), None)
                    elif "ModifierType" in row:
                        self.dynamic[row["ModifierType"]] = row
            elif table.tag == "Modifiers":
                for node in table:
                    if node.tag == "Update":
                        self._update_rows(self.rows, "ModifierId", *self._where_set(node))
                        continue
                    row = fields(node)
                    modifier_id = row.get("ModifierId")
                    if node.tag == "Delete" and modifier_id:
                        self.rows.pop(modifier_id, None)
                        self.arguments.pop(modifier_id, None)
                        self.attachments.pop(modifier_id, None)
                        self.owners.pop(modifier_id, None)
                    elif modifier_id:
                        self.rows.setdefault(row["ModifierId"], {}).update(row)
            elif table.tag == "ModifierArguments":
                for node in table:
                    # ⚠ THE EXPANSIONS REBALANCE BY `<Update>`, NOT BY A NEW ROW.
                    # Gathering Storm leaves the base `COMPUTERS_BOOST_ALL_TOURISM`
                    # row in place and updates its Amount from 100 to 25, and does
                    # the same to the Airport and Hangar air slots (2 to 1).
                    # Ignoring `<Update>` here made the install walk report the
                    # base-game numbers while the compiled cache -- which the game
                    # built for itself -- reported the shipped ones.
                    if node.tag == "Update":
                        self._update_pairs(
                            self.arguments,
                            "ModifierId",
                            *self._where_set(node),
                            table="ModifierArguments",
                        )
                        continue
                    row = fields(node)
                    modifier_id = row.get("ModifierId")
                    name = row.get("Name")
                    if node.tag == "Delete" and modifier_id and name:
                        arguments = self.arguments.get(modifier_id)
                        if arguments is not None:
                            arguments.pop(name, None)
                            if not arguments:
                                self.arguments.pop(modifier_id, None)
                    elif modifier_id and name:
                        self.arguments[row["ModifierId"]][row["Name"]] = row.get("Value", "")
            elif table.tag == "Requirements":
                for node in table:
                    if node.tag == "Update":
                        self._update_rows(
                            self.requirements, "RequirementId", *self._where_set(node)
                        )
                        continue
                    row = fields(node)
                    if "RequirementId" in row:
                        self.requirements.setdefault(row["RequirementId"], {}).update(row)
            elif table.tag == "RequirementArguments":
                for node in table:
                    if node.tag == "Update":
                        self._update_pairs(
                            self.requirement_arguments,
                            "RequirementId",
                            *self._where_set(node),
                            table="RequirementArguments",
                        )
                        continue
                    row = fields(node)
                    if "RequirementId" in row and "Name" in row:
                        self.requirement_arguments[row["RequirementId"]][row["Name"]] = row.get(
                            "Value", ""
                        )
            elif table.tag == "RequirementSets":
                for node in table:
                    if node.tag == "Update":
                        where, sets = self._where_set(node)
                        for set_id in list(self.set_kinds):
                            row = {
                                "RequirementSetId": set_id,
                                "RequirementSetType": self.set_kinds[set_id],
                            }
                            if self._matches(row, where):
                                row.update(sets)
                                self.set_kinds.pop(set_id)
                                self.set_kinds[row["RequirementSetId"]] = row[
                                    "RequirementSetType"
                                ]
                        continue
                    row = fields(node)
                    if "RequirementSetId" in row:
                        self.set_kinds[row["RequirementSetId"]] = row.get("RequirementSetType", "")
            elif table.tag == "RequirementSetRequirements":
                for node in table:
                    row = fields(node)
                    if "RequirementSetId" in row and "RequirementId" in row:
                        # Overlays restate the same membership row, so keep
                        # the set a set rather than reporting each condition
                        # twice.
                        members = self.requirement_sets[row["RequirementSetId"]]
                        if row["RequirementId"] not in members:
                            members.append(row["RequirementId"])
            elif table.tag.endswith("Modifiers"):
                # An expansion can detach a modifier it no longer wants. Not
                # honouring that reports rules the shipped ruleset removed.
                for node in table:
                    if node.tag != "Delete":
                        continue
                    where = fields(node)
                    doomed = where.get("ModifierId")
                    if doomed:
                        owner = next(
                            (
                                value
                                for key, value in where.items()
                                if key not in ("ModifierId", "Name", "Id")
                            ),
                            "",
                        )
                        self.detach(doomed, table.tag, owner)
                # BuildingModifiers, TraitModifiers, BeliefModifiers, ... —
                # the tables that bind a modifier to the object that owns it.
                for node in table:
                    # ``Delete ModifierId=...`` has the same identifier shape
                    # as a binding row.  It was previously removed above and
                    # then accidentally appended again as an ownerless binding.
                    if node.tag == "Delete":
                        continue
                    row = fields(node)
                    if "ModifierId" not in row:
                        continue
                    # The other column names the object that owns the modifier
                    # -- PolicyType, BuildingType, BeliefType and so on.
                    # Without it a drill can say "some policy does this" but
                    # cannot say which, which is the whole job.
                    owner = next(
                        (
                            value
                            for key, value in row.items()
                            if key not in ("ModifierId", "Name", "Id")
                        ),
                        "",
                    )
                    bindings = zip(
                        self.attachments[row["ModifierId"]],
                        self.owners[row["ModifierId"]],
                    )
                    # Compatibility overlays sometimes restate an identical
                    # binding.  It remains one executable modifier, not two
                    # owners in the audit report.
                    if (table.tag, owner) not in bindings:
                        self.attachments[row["ModifierId"]].append(table.tag)
                        self.owners[row["ModifierId"]].append(owner)

    def condition(self, modifier_id: str) -> str:
        """The requirement set on a modifier, spelled out.

        A modifier row is only half a rule; the other half is the condition
        under which it fires. Reading the amount without the condition is how
        a base-game row gets mistaken for the shipped one.
        """
        row = self.rows.get(modifier_id, {})
        set_id = row.get("SubjectRequirementSetId") or row.get("OwnerRequirementSetId")
        if not set_id:
            return ""
        parts = []
        for requirement_id in self.requirement_sets.get(set_id, []):
            requirement = self.requirements.get(requirement_id, {})
            kind = requirement.get("RequirementType", requirement_id)
            arguments = self.requirement_arguments.get(requirement_id, {})
            negated = "NOT " if truthy(requirement.get("Inverse")) else ""
            rendered = kind.replace("REQUIREMENT_", "")
            if arguments:
                rendered += "(" + ", ".join(f"{k}={v}" for k, v in arguments.items()) + ")"
            parts.append(negated + rendered)
        joiner = " OR " if self.set_kinds.get(set_id) == "REQUIREMENTSET_TEST_ANY" else " AND "
        return f"{set_id}: " + (joiner.join(parts) if parts else "(no requirements)")

    def resolve(self, modifier_id: str) -> tuple[str, str]:
        """The (EffectType, CollectionType) a modifier row resolves to.

        A row names a ``ModifierType``; ``DynamicModifiers`` maps that to the
        pair. Rows whose type is not declared there use an effect the engine
        defines natively, which the census reports as ``UNDECLARED`` rather
        than dropping.
        """
        row = self.rows[modifier_id]
        dynamic = self.dynamic.get(row.get("ModifierType", ""))
        if dynamic is None:
            return ("UNDECLARED", "UNDECLARED")
        return (
            dynamic.get("EffectType", "UNDECLARED"),
            dynamic.get("CollectionType", "UNDECLARED"),
        )


def load(install: Path) -> Modifiers:
    modifiers = Modifiers()
    deferred: list[Path] = []
    for relative in LOAD_ORDER:
        directory = install / relative
        if not directory.is_dir():
            print(f"warning: missing load-order directory {relative}", file=sys.stderr)
            continue
        paths = sorted(directory.rglob("*.xml"))
        core = relative in LOAD_ORDER[:3]
        # The expansion manifests assign RemoveData priority 1: those deletes
        # happen before the expansion re-declares its replacement rows.  A
        # lexical walk instead applies them late and makes an old policy
        # modifier look live just because a compatibility XML was read after
        # it.  Keep this in lockstep with civ6_fidelity.load_database().
        if core:
            for path in paths:
                if REMOVE_DATA.match(path.name):
                    modifiers.apply_file(path)
        for path in paths:
            # Match the rules audit's baseline: optional game modes and
            # non-rules pack files are out of scope, so their modifiers are
            # not backlog.
            if core and REMOVE_DATA.match(path.name):
                continue
            if core and CROSS_EXPANSION.search(path.name):
                deferred.append(path)
            elif not core and PACK_EXCLUDE.search(path.name):
                continue
            else:
                modifiers.apply_file(path)
    # Rise and Fall's Gathering Storm compatibility overlay must run after the
    # two ordinary expansion passes, just as the game does.
    for path in deferred:
        modifiers.apply_file(path)
    return modifiers


# The compiled gameplay database the game leaves behind carries the same
# modifier tables the XML load order builds, so the census can run on a machine
# where Civilization VI is no longer installed — the same route
# ``civ6_fidelity.py --cache`` takes. The two are not guaranteed identical: the
# XML route reconstructs a chosen content set in a chosen order, the cache is
# whatever the game last compiled for itself. Where they disagree, that
# disagreement is itself a finding.
CACHE_PATHS = (
    "~/Library/Application Support/Sid Meier's Civilization VI/Cache/DebugGameplay.sqlite",
    "~/AppData/Local/Firaxis Games/Sid Meier's Civilization VI/Cache/DebugGameplay.sqlite",
)

# The owner tables bind a modifier to the object that grants it. Reading them is
# what lets a drill say *which* policy or belief does something rather than
# "some policy does".
OWNER_TABLES = (
    "BuildingModifiers", "TraitModifiers", "BeliefModifiers", "PolicyModifiers",
    "UnitPromotionModifiers", "GovernmentModifiers", "DistrictModifiers",
    "ImprovementModifiers", "FeatureModifiers", "GreatPersonIndividualActionModifiers",
    "GreatPersonIndividualBirthModifiers", "UnitAbilityModifiers", "CivicModifiers",
    "TechnologyModifiers", "ProjectModifiers", "ResourceModifiers", "WonderModifiers",
    "GovernorPromotionModifiers", "LeaderTraitModifiers", "CityStateModifiers",
)


def find_cache(explicit: str | None) -> Path:
    if explicit:
        path = Path(explicit).expanduser()
        if not path.is_file():
            raise SystemExit(f"no compiled gameplay database at {path}")
        return path
    for candidate in CACHE_PATHS:
        path = Path(candidate).expanduser()
        if path.is_file():
            return path
    raise SystemExit("no compiled gameplay database found; pass --cache <path>")


def load_cache(path: Path) -> Modifiers:
    import sqlite3
    import urllib.parse

    uri = "file:" + urllib.parse.quote(str(path)) + "?mode=ro&immutable=1"
    connection = sqlite3.connect(uri, uri=True)
    connection.row_factory = sqlite3.Row
    present = {
        row[0]
        for row in connection.execute(
            "select name from sqlite_master where type = 'table'"
        )
    }

    def rows(table: str) -> list[dict]:
        if table not in present:
            return []
        return [
            {k: ("" if v is None else str(v)) for k, v in dict(row).items()}
            for row in connection.execute(f"select * from {table}")
        ]

    modifiers = Modifiers()
    for row in rows("DynamicModifiers"):
        if row.get("ModifierType"):
            modifiers.dynamic[row["ModifierType"]] = row
    for row in rows("Modifiers"):
        if row.get("ModifierId"):
            modifiers.rows.setdefault(row["ModifierId"], {}).update(row)
    for row in rows("ModifierArguments"):
        if row.get("ModifierId") and row.get("Name"):
            modifiers.arguments[row["ModifierId"]][row["Name"]] = row.get("Value", "")
    for row in rows("Requirements"):
        if row.get("RequirementId"):
            modifiers.requirements.setdefault(row["RequirementId"], {}).update(row)
    for row in rows("RequirementArguments"):
        if row.get("RequirementId") and row.get("Name"):
            modifiers.requirement_arguments[row["RequirementId"]][row["Name"]] = row.get(
                "Value", ""
            )
    for row in rows("RequirementSets"):
        if row.get("RequirementSetId"):
            modifiers.set_kinds[row["RequirementSetId"]] = row.get("RequirementSetType", "")
    for row in rows("RequirementSetRequirements"):
        set_id, requirement = row.get("RequirementSetId"), row.get("RequirementId")
        if set_id and requirement:
            members = modifiers.requirement_sets[set_id]
            if requirement not in members:
                members.append(requirement)
    for table in OWNER_TABLES:
        for row in rows(table):
            if not row.get("ModifierId"):
                continue
            owner = next(
                (
                    value
                    for key, value in row.items()
                    if key not in ("ModifierId", "Name", "Id")
                ),
                "",
            )
            modifiers.attachments[row["ModifierId"]].append(table)
            modifiers.owners[row["ModifierId"]].append(owner)
    return modifiers


def shipped_text(install: Path, tag_fragment: str) -> list[tuple[str, str]]:
    """Localised descriptions matching a tag fragment, Gathering Storm first.

    The rules tables cannot always say which of two rows a ruleset actually
    uses: Gathering Storm restates a belief or a promotion without deleting the
    base row. The text the game shows the player can. Gathering Storm ships its
    replacements as ``_EXPANSION2_DESCRIPTION`` tags, so a description carrying
    that suffix is the live wording and the plain tag is superseded.
    """
    import xml.etree.ElementTree as Tree

    out: list[tuple[str, str]] = []
    for path in sorted(install.rglob("en_US/*.xml")):
        try:
            root = Tree.parse(path).getroot()
        except Tree.ParseError:
            continue
        for node in root.iter():
            tag = node.attrib.get("Tag", "")
            if tag_fragment.upper() not in tag.upper():
                continue
            text = "".join(node.itertext()).strip()
            if text:
                out.append((tag, " ".join(text.split())))
    # Gathering Storm wording first, then the base game's.
    out.sort(key=lambda row: ("EXPANSION2" not in row[0], row[0]))
    return out


def load_coverage() -> dict[str, dict]:
    if not COVERAGE.exists():
        return {}
    entries = json.loads(COVERAGE.read_text(encoding="utf-8"))["effects"]
    for name, entry in entries.items():
        if entry.get("status") not in STATUSES:
            raise SystemExit(f"{name}: status must be one of {STATUSES}")
    return entries


def short(effect: str) -> str:
    return effect[len("EFFECT_"):] if effect.startswith("EFFECT_") else effect


def census(modifiers: Modifiers) -> list[dict]:
    counts: collections.Counter = collections.Counter()
    owners: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    collections_by_effect: dict[str, collections.Counter] = collections.defaultdict(
        collections.Counter
    )
    for modifier_id in sorted(modifiers.active_modifier_ids()):
        effect, collection = modifiers.resolve(modifier_id)
        counts[effect] += 1
        collections_by_effect[effect][collection] += 1
        for table in modifiers.attachments.get(modifier_id) or ["(nested modifier)"]:
            owners[effect][table] += 1
    coverage = load_coverage()
    out = []
    for effect, rows in counts.most_common():
        entry = coverage.get(short(effect), {})
        out.append(
            {
                "effect": short(effect),
                "rows": rows,
                "status": entry.get("status", "unmodelled"),
                "note": entry.get("note", ""),
                "verified": bool(entry.get("verified")),
                "collections": dict(collections_by_effect[effect].most_common()),
                "owners": dict(owners[effect].most_common(4)),
            }
        )
    return out


def report(entries: list[dict], modifiers: Modifiers, install: Path, limit: int) -> str:
    total = sum(entry["rows"] for entry in entries)
    by_status: collections.Counter = collections.Counter()
    for entry in entries:
        by_status[entry["status"]] += entry["rows"]
    lines = [
        "# Modifier census",
        "",
        f"Reference: `{install}` (Gathering Storm load order).",
        "",
        f"{total} active modifier rows across {len(entries)} distinct effects, "
        f"rooted at {sum(modifier_id in modifiers.rows for modifier_id in modifiers.attachments)} "
        "direct attachments.",
        "",
        "| Status | Effects | Rows | Share |",
        "|---|---:|---:|---:|",
    ]
    for status in STATUSES:
        effects = sum(1 for entry in entries if entry["status"] == status)
        rows = by_status[status]
        lines.append(f"| {status} | {effects} | {rows} | {rows * 100 // max(total, 1)}% |")
    # How concentrated the work is decides the strategy. If a handful of
    # effects covered most rows, hardcoding them would finish the job; if the
    # tail is long, only an interpreter reaches the end of it.
    ranked = sorted((entry["rows"] for entry in entries), reverse=True)
    verified = sum(entry["rows"] for entry in entries if entry["verified"])
    claimed = by_status["implemented"] + by_status["partial"]
    lines += [
        "",
        f"Of the {claimed} covered rows, {verified} are verified row by row "
        "against the shipped modifiers; the rest are inspection judgements.",
        "",
        "| Share of rows | Effects needed |",
        "|---|---:|",
    ]
    for share in (50, 80, 95, 100):
        running = 0
        needed = 0
        for rows in ranked:
            if running * 100 >= share * total:
                break
            running += rows
            needed += 1
        lines.append(f"| {share}% | {needed} |")
    lines += [
        "",
        "## Largest unmodelled effects",
        "",
        "| Rows | Effect | Mostly attached to |",
        "|---:|---|---|",
    ]
    shown = 0
    for entry in entries:
        if entry["status"] not in ("unmodelled", "partial"):
            continue
        owners = ", ".join(f"{table} x{count}" for table, count in entry["owners"].items())
        lines.append(f"| {entry['rows']} | {entry['effect']} | {owners} |")
        shown += 1
        if shown >= limit:
            break
    return "\n".join(lines)


# --------------------------------------------------------------- catalog import
#
# The census counts what CIVVIS cannot express. This half emits what it can:
# every shipped modifier row of a declared effect becomes a named
# ``ModifierSpec`` bundle in ``data/modifiers.json``, and the CIVVIS rules
# object that the game says owns the row carries a ``modifiers: ["<bundle>"]``
# reference to it. The engine's loader flattens that reference into the object's
# ordinary effect map, so an imported row executes through the same consumer a
# hand-written number used to, with one difference that is the whole point: the
# number is the game's own.
#
# Three rules keep the import from inventing rules the game does not have.
#
# 1. AN EFFECT IS IMPORTED ONLY WHEN THIS FILE DECLARES A TRANSLATION. Anything
#    else is left out and stays in the census as unmodelled. A row emitted with
#    a key no consumer reads would be inert data counted as fidelity.
# 2. A ROW CARRYING A REQUIREMENT SET IS REFUSED. `ModifierRequirement` covers
#    player facts only, and the Diplomatic Quarter's Envoy row is conditional on
#    plot adjacency. Emitting it unconditionally would hand every Diplomatic
#    Quarter an Envoy it has not earned -- exactly the silent, everywhere wrong
#    answer that is worse than no answer.
# 3. ONLY `COLLECTION_OWNER` AND `COLLECTION_PLAYER` ARE FLATTENED. Those are
#    the rows whose scope is the owning object's own; a `PLAYER_CITIES` or
#    `PLAYER_UNITS` row means something the static fold cannot say, and the
#    engine's `expand_modifier_attachments` rejects it rather than guessing.
#
# `--emit-catalog` writes the file and prints the wiring. `--check-catalog`
# re-derives both from the database and fails on any drift, so the committed
# catalog cannot quietly stop matching the shipped tables.

# Which CIVVIS ruleset file each owner table names, and the prefix its rows
# carry. `slug()` (shared with the rules audit) resolves the game's identifier
# to CIVVIS' own spelling, aliases included.
OWNER_FILES: dict[str, tuple[tuple[str, str], ...]] = {
    "BuildingModifiers": (("buildings", "BUILDING_"), ("wonders", "BUILDING_")),
    "WonderModifiers": (("wonders", "BUILDING_"),),
    "DistrictModifiers": (("districts", "DISTRICT_"),),
    "PolicyModifiers": (("policies", "POLICY_"),),
    "CivicModifiers": (("civics", "CIVIC_"),),
    "TechnologyModifiers": (("techs", "TECH_"),),
    "GovernmentModifiers": (("governments", "GOVERNMENT_"),),
    "UnitPromotionModifiers": (("promotions", "PROMOTION_"),),
    "GreatPersonIndividualActionModifiers": (
        ("great_people", "GREAT_PERSON_INDIVIDUAL_"),
    ),
}

FLATTENABLE_COLLECTIONS = ("COLLECTION_OWNER", "COLLECTION_PLAYER")


def _amount(arguments: dict[str, str], name: str = "Amount") -> int:
    """A modifier argument as an integer.

    Civ VI's own values are integers, and `docs/FIDELITY.md` requires rules
    arithmetic to stay integral, so a non-integral argument is a translation
    this table does not understand rather than something to round.
    """
    raw = arguments[name]
    value = float(raw)
    if value != int(value):
        raise ValueError(f"{name}={raw} is not an integer")
    return int(value)


def _influence_token(arguments: dict[str, str], family: str) -> dict[str, float] | None:
    # One shipped effect, two CIVVIS consumers: a tree node grants its Envoys
    # from `free_envoys` when the node first completes, while a wonder,
    # district or Great Person grants them from `envoys` at its own completion.
    # The amount is the row's either way.
    if family in ("techs", "civics"):
        return {"free_envoys": _amount(arguments)}
    if family in ("wonders", "districts", "buildings", "great_people"):
        return {"envoys": _amount(arguments)}
    return None


# effect name -> (arguments, CIVVIS owner file) -> CIVVIS effect keys, or None
# when this effect does not translate for that owner family.
TRANSLATIONS: dict = {
    # Diplomatic Victory Points, awarded once by a wonder or a tree node.
    "ADJUST_PLAYER_DIPLOMATIC_VICTORY_POINTS": lambda arguments, family: (
        {"diplomatic_victory_points": _amount(arguments)}
        if family in ("wonders", "buildings", "techs", "civics")
        else None
    ),
    "GRANT_INFLUENCE_TOKEN": _influence_token,
    # Movement added to an embarked unit. CIVVIS reads it off the trees; the
    # Great Lighthouse row is carried by that wonder's naval movement, which
    # its own consumer already applies to embarked units.
    "ADJUST_PLAYER_EMBARKED_UNIT_MOVEMENT": lambda arguments, family: (
        {"embarked_movement": _amount(arguments)}
        if family in ("techs", "civics")
        else None
    ),
    # Percentage added to every Tourism source the empire produces.
    "ADJUST_PLAYER_TOURISM": lambda arguments, family: (
        {"tourism_pct": _amount(arguments)}
        if family in ("techs", "civics", "governments")
        else None
    ),
    # Air unit capacity: a promotion adds slots to its own unit, a building
    # adds them to the district it sits in.
    "GRANT_AIR_SLOTS": lambda arguments, family: (
        {"aircraft_slots": _amount(arguments)} if family == "promotions" else None
    ),
    "ADJUST_PLAYER_DISTRICT_AIR_SLOTS": lambda arguments, family: (
        {"air_slots": _amount(arguments)}
        if family in ("buildings", "wonders")
        else None
    ),
    # Per-unit combat and vision promotions.
    "ADJUST_UNIT_SIGHT": lambda arguments, family: (
        {"sight": _amount(arguments)} if family == "promotions" else None
    ),
    "ADJUST_UNIT_ATTACK_RANGE": lambda arguments, family: (
        {"range": _amount(arguments)} if family == "promotions" else None
    ),
    "ADJUST_UNIT_NUM_ATTACKS": lambda arguments, family: (
        {"extra_attacks": _amount(arguments)} if family == "promotions" else None
    ),
    "ADJUST_UNIT_ATTACK_AND_MOVE": lambda arguments, family: (
        {"move_after_attack": 1}
        if family == "promotions" and truthy(arguments.get("CanMove"))
        else None
    ),
}


def catalog_name(modifier_id: str) -> str:
    return modifier_id.lower()


def build_catalog(modifiers: Modifiers, data: Path) -> tuple[dict, dict, list[str]]:
    """Translate the shipped rows into bundles plus the wiring they imply.

    Returns ``(catalog, wiring, skipped)``. ``wiring`` maps a CIVVIS ruleset
    file to the bundle names each of its objects must reference; ``skipped``
    records every row of a declared effect that was deliberately not emitted,
    with the reason, so the refusals are visible rather than silent.
    """
    owned: dict[str, dict] = {}
    for name in sorted({file for tables in OWNER_FILES.values() for file, _ in tables}):
        owned[name] = json.loads((data / f"{name}.json").read_text(encoding="utf-8"))

    catalog: dict[str, dict] = {}
    wiring: dict[str, dict[str, set[str]]] = collections.defaultdict(
        lambda: collections.defaultdict(set)
    )
    skipped: list[str] = []

    for modifier_id in sorted(modifiers.active_modifier_ids()):
        effect, collection = modifiers.resolve(modifier_id)
        translate = TRANSLATIONS.get(short(effect))
        if translate is None:
            continue
        if collection not in FLATTENABLE_COLLECTIONS:
            skipped.append(f"{modifier_id}: {collection} is not a flattenable collection")
            continue
        if modifiers.condition(modifier_id):
            skipped.append(
                f"{modifier_id}: carries {modifiers.condition(modifier_id).split(':')[0]}, "
                "which the runtime requirement set cannot express"
            )
            continue
        arguments = modifiers.arguments.get(modifier_id, {})
        # Group this row's modelled owners by the effect keys they need. One
        # row usually needs one key; the Envoy award is attached to both civics
        # and a district, whose consumers read different keys, so it becomes two
        # bundles rather than one bundle carrying a key its owner never reads.
        by_keys: dict[str, list[tuple[str, str]]] = collections.defaultdict(list)
        unmodelled = 0
        for table, owner in zip(
            modifiers.attachments.get(modifier_id, []),
            modifiers.owners.get(modifier_id, []),
        ):
            for file, prefix in OWNER_FILES.get(table, ()):
                name = slug(owner, prefix)
                if name not in owned[file]:
                    continue
                try:
                    keys = translate(arguments, file)
                except (KeyError, ValueError) as error:
                    skipped.append(f"{modifier_id}: {error}")
                    keys = None
                if keys is None:
                    unmodelled += 1
                    continue
                by_keys[json.dumps(keys, sort_keys=True)].append((file, name))
        if not by_keys:
            if unmodelled:
                skipped.append(
                    f"{modifier_id}: no modelled owner whose family this effect translates for"
                )
            continue
        suffixed = len(by_keys) > 1
        for encoded, owners in sorted(by_keys.items()):
            keys = json.loads(encoded)
            name = catalog_name(modifier_id)
            if suffixed:
                name = f"{name}__{'_'.join(sorted(keys))}"
            catalog[name] = {"effects": keys}
            for file, obj in owners:
                wiring[file][obj].add(name)

    return (
        catalog,
        {file: {obj: sorted(names) for obj, names in sorted(objs.items())}
         for file, objs in sorted(wiring.items())},
        skipped,
    )


def catalog_json(catalog: dict) -> str:
    """The catalog as the repository stores it: sorted, integral, newline-ended.

    Byte stability matters twice over. `Rules::from_values` fingerprints the
    ruleset source, and `--check-catalog` compares this text with the committed
    file, so a re-import that reorders keys would read as drift.
    """
    def plain(value):
        return int(value) if float(value) == int(value) else value

    ordered = {
        name: {"effects": {key: plain(value) for key, value in sorted(spec["effects"].items())}}
        for name, spec in sorted(catalog.items())
    }
    return json.dumps(ordered, indent=2, sort_keys=True) + "\n"


def check_wiring(wiring: dict, data: Path) -> list[str]:
    """Every imported bundle is referenced by exactly the objects the game says own it."""
    problems: list[str] = []
    declared: dict[str, set[str]] = collections.defaultdict(set)
    for file in sorted({file for tables in OWNER_FILES.values() for file, _ in tables}):
        entries = json.loads((data / f"{file}.json").read_text(encoding="utf-8"))
        for name, spec in entries.items():
            if isinstance(spec, dict) and spec.get("modifiers"):
                declared[file] |= {f"{name}\t{bundle}" for bundle in spec["modifiers"]}
    for file, objects in wiring.items():
        expected = {f"{obj}\t{bundle}" for obj, names in objects.items() for bundle in names}
        for missing in sorted(expected - declared[file]):
            obj, bundle = missing.split("\t")
            problems.append(f"{file}.json: {obj} does not attach {bundle}")
        for extra in sorted(declared[file] - expected):
            obj, bundle = extra.split("\t")
            problems.append(f"{file}.json: {obj} attaches {bundle}, which no shipped row grants it")
    for file, entries in declared.items():
        if file in wiring:
            continue
        for extra in sorted(entries):
            obj, bundle = extra.split("\t")
            problems.append(f"{file}.json: {obj} attaches {bundle}, which no shipped row grants it")
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--civ6", help="path to the Civilization VI install")
    parser.add_argument(
        "--cache",
        nargs="?",
        const=True,
        help="read the compiled gameplay database the game leaves in its Cache "
        "directory instead of an install; give a path or let it be found",
    )
    parser.add_argument("--json", help="write the full census here")
    parser.add_argument("--out", help="write the markdown report here instead of stdout")
    parser.add_argument("--limit", type=int, default=40, help="rows in the backlog table")
    parser.add_argument("--effect", help="print every modifier using this effect and stop")
    parser.add_argument(
        "--sweep",
        help="print every entry of a data file beside the game's own wording",
    )
    parser.add_argument(
        "--describe",
        help="print the shipped descriptions matching this tag fragment, "
        "Gathering Storm wording first, and stop",
    )
    parser.add_argument(
        "--emit-catalog",
        nargs="?",
        const=str(REPO / "data" / "modifiers.json"),
        help="translate every shipped row of a declared effect into "
        "data/modifiers.json and print the wiring it implies",
    )
    parser.add_argument(
        "--check-catalog",
        action="store_true",
        help="re-derive the catalog and its wiring from the database and fail "
        "on any drift from the committed ruleset",
    )
    parser.add_argument(
        "--max-unmodelled",
        type=int,
        default=None,
        help="exit 1 when unmodelled+partial rows exceed this ratchet",
    )
    args = parser.parse_args()

    if args.cache:
        install = find_cache(None if args.cache is True else args.cache)
        modifiers = load_cache(install)
    else:
        install = find_install(args.civ6)
        modifiers = load(install)

    if args.sweep:
        if args.cache:
            raise SystemExit(
                "--sweep needs an install: it prints the game's own wording "
                "beside each entry, and the cache has no localised text"
            )
        # Every entry of one CIVVIS data file beside the wording the game
        # shows for it. Descriptions state clauses the effect rows only imply,
        # which is how the Lumber Mill's Mercantilism gate and four wrong
        # policy cards turned up; the rows remain the authority on magnitude.
        prefixes = {
            "improvements": "LOC_IMPROVEMENT_",
            "buildings": "LOC_BUILDING_",
            "policies": "LOC_POLICY_",
            "districts": "LOC_DISTRICT_",
            "units": "LOC_UNIT_",
            "wonders": "LOC_BUILDING_",
        }
        prefix = prefixes.get(args.sweep)
        if prefix is None:
            print(f"unknown sweep {args.sweep}; try {sorted(prefixes)}", file=sys.stderr)
            return 1
        ours = json.loads(
            (REPO / "data" / f"{args.sweep}.json").read_text(encoding="utf-8")
        )
        described = {tag: text for tag, text in shipped_text(install, prefix)}
        for name in sorted(ours):
            tag = f"{prefix}{name.upper()}"
            text = described.get(f"{tag}_EXPANSION2_DESCRIPTION") or described.get(
                f"{tag}_DESCRIPTION"
            )
            if not text:
                continue
            text = re.sub(r"\[ICON_\w+\]", "", text).replace("[NEWLINE]", " ")
            print(f"### {name}")
            print(f"    {' '.join(text.split())}")
            print(f"    civvis: {json.dumps(ours[name])[:300]}")
        return 0

    if args.describe:
        if args.cache:
            # The compiled gameplay database carries the rules tables and not
            # the localised text, which lives in a separate localization
            # database. Say so rather than printing nothing and reading as
            # "no descriptions match".
            raise SystemExit(
                "--describe needs an install: the compiled gameplay database "
                "has no localised text"
            )
        for tag, text in shipped_text(install, args.describe):
            marker = "GS  " if "EXPANSION2" in tag else "base"
            print(f"{marker} {tag}")
            print(f"     {text}")
        return 0

    if args.emit_catalog or args.check_catalog:
        data = REPO / "data"
        catalog, wiring, skipped = build_catalog(modifiers, data)
        text = catalog_json(catalog)
        problems = check_wiring(wiring, data)
        if args.check_catalog:
            committed = (data / "modifiers.json").read_text(encoding="utf-8")
            if committed != text:
                problems.insert(
                    0,
                    "data/modifiers.json differs from the database; "
                    "rerun --emit-catalog",
                )
            for problem in problems:
                print(f"FAIL: {problem}", file=sys.stderr)
            print(
                f"{len(catalog)} bundles over {len(TRANSLATIONS)} effects, "
                f"{sum(len(objects) for objects in wiring.values())} wired objects",
                file=sys.stderr,
            )
            return 1 if problems else 0
        Path(args.emit_catalog).write_text(text, encoding="utf-8")
        print(f"# {len(catalog)} bundles written to {args.emit_catalog}")
        print("#\n# wiring: each object below needs this `modifiers` list")
        for file, objects in wiring.items():
            for obj, names in objects.items():
                print(f"{file}.json  {obj}  {json.dumps(names)}")
        if skipped:
            print("#\n# refused rows of declared effects:")
            for reason in skipped:
                print(f"#   {reason}")
        for problem in problems:
            print(f"# WIRING TODO: {problem}")
        return 0

    if args.effect:
        wanted = args.effect if args.effect.startswith("EFFECT_") else f"EFFECT_{args.effect}"
        for modifier_id in sorted(modifiers.active_modifier_ids()):
            effect, collection = modifiers.resolve(modifier_id)
            if effect != wanted:
                continue
            arguments = modifiers.arguments.get(modifier_id, {})
            attached = modifiers.attachments.get(modifier_id) or ["(nested modifier)"]
            objects = list(modifiers.owners.get(modifier_id) or [])
            objects += [""] * (len(attached) - len(objects))
            owners = ", ".join(
                f"{table}:{obj}" if obj else table
                for table, obj in dict.fromkeys(zip(attached, objects))
            )
            print(f"{modifier_id}\n    {collection}  {owners}\n    {arguments}")
            if condition := modifiers.condition(modifier_id):
                print(f"    when {condition}")
        return 0

    entries = census(modifiers)
    text = report(entries, modifiers, install, args.limit)
    if args.out:
        Path(args.out).write_text(text + "\n", encoding="utf-8")
    else:
        print(text)
    if args.json:
        Path(args.json).write_text(json.dumps(entries, indent=2), encoding="utf-8")

    open_rows = sum(
        entry["rows"] for entry in entries if entry["status"] in ("unmodelled", "partial")
    )
    print(f"\n{open_rows} modifier rows unmodelled or partial", file=sys.stderr)
    if args.max_unmodelled is not None and open_rows > args.max_unmodelled:
        print(
            f"FAIL: {open_rows} exceeds the ratchet of {args.max_unmodelled}",
            file=sys.stderr,
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
