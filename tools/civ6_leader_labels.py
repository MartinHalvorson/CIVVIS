#!/usr/bin/env python3
"""Harvest the label Civilization VI's leader picker shows for every playable leader.

`civ6_play.py` finds a requested leader on the Create Game screen by its
rendered label and reads the selection back by the same label, so a label that
is wrong by one word is a launch that never starts. The label is not the
leader's type and cannot be derived from it: `LEADER_BASIL` is "Basil II",
`LEADER_LADY_TRIEU` is "Bà Triệu", and once a persona pack is installed the
base leader's own row is replaced, so `LEADER_T_ROOSEVELT` stops being "Teddy
Roosevelt" and becomes "Teddy Roosevelt (Bull Moose)".

The picker's rows are the frontend `Players` table. The shipped query is in
`Base/Assets/Configuration/Data/SetupParameters.xml`:

    <Row QueryId="Players" SQL="SELECT Domain, LeaderName AS Name, ...,
         LeaderType AS Value, ... FROM Players"/>

so the label is `LeaderName`, a LOC key, in the row for the ruleset's domain.
This reads those rows the way the game assembles them (the base configuration,
then every pack's `FrontEndActions` in its `.modinfo`, with a pack's `Delete`
removing the row it replaces), resolves each key through the en_US text the
same packs ship, and writes `data/civ6_leader_picker_labels.json`.
`civ6_play.LEADER_PICKER_LABELS` is pinned to that snapshot by
`tools/test_civ6_play_leader_picker.py`.

    python3 tools/civ6_leader_labels.py            # compare the snapshot to this install
    python3 tools/civ6_leader_labels.py --write    # rewrite the snapshot

It reads only the small configuration and text XML, never the art or the
gameplay database, and it skips `DLC/CivvisControl`: that is our own mod,
installed into the game's tree, and its one-row leader whitelist is not a row
the game ships.
"""

from __future__ import annotations

import argparse
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import civ6_env  # noqa: E402

SNAPSHOT = HERE.parent / "data" / "civ6_leader_picker_labels.json"
SCHEMA = "civvis.civ6-leader-picker-labels.v1"
OUR_OWN_MOD = "CivvisControl"
DEFAULT_DOMAIN = "Players:StandardPlayers"
#: The three rulesets a Create Game screen offers, and the `Players` domain
#: each one lists (the same table as `civ6_control.install.LEADER_DOMAINS`).
RULESET_DOMAINS = {
    "RULESET_STANDARD": "Players:StandardPlayers",
    "RULESET_EXPANSION_1": "Players:Expansion1_Players",
    "RULESET_EXPANSION_2": "Players:Expansion2_Players",
}
BASE_CONFIG = "Base/Assets/Configuration/Data"
BASE_TEXT = "Base/Assets/Text/en_US"
PLAYERS_QUERY = (BASE_CONFIG + "/SetupParameters.xml: "
                 "SELECT Domain, LeaderName AS Name, ..., LeaderType AS Value FROM Players")
#: A criterion on a frontend action that holds when the pack it names is
#: installed. Any other kind is refused rather than guessed at.
INSTALLED_PACK_CRITERIA = {"ModInUse", "ModIsEnabled"}


class HarvestError(RuntimeError):
    """The install's configuration cannot be read into one answer."""


def _fields(element: ET.Element) -> dict[str, str]:
    """A row's columns, whether written as attributes or as child elements."""
    fields = dict(element.attrib)
    fields.update({child.tag: (child.text or "").strip() for child in element})
    return fields


def _parse(path: Path) -> ET.Element | None:
    try:
        return ET.fromstring(path.read_bytes())
    except (OSError, ET.ParseError):
        return None


def frontend_files(assets: Path) -> tuple[list[Path], list[Path]]:
    """The configuration and en_US text files the frontend loads on this install."""
    config = sorted((assets / BASE_CONFIG).glob("*.xml"))
    text = sorted((assets / BASE_TEXT).glob("*.xml"))
    packs = [path for path in sorted((assets / "DLC").glob("*/*.modinfo"))
             if path.parent.name != OUR_OWN_MOD]
    roots = {path: _parse(path) for path in packs}
    installed = {root.attrib.get("id", "").lower() for root in roots.values() if root is not None}
    for modinfo, root in roots.items():
        if root is None:
            raise HarvestError(f"{modinfo} is not readable XML")
        criteria = {item.attrib.get("id"): item for item in root.iter("Criteria")}
        for action in root.findall("./FrontEndActions/*"):
            if action.tag not in ("UpdateDatabase", "UpdateText"):
                continue
            named = action.attrib.get("criteria")
            if named is not None:
                conditions = list(criteria[named]) if named in criteria else None
                if not conditions or any(c.tag not in INSTALLED_PACK_CRITERIA for c in conditions):
                    raise HarvestError(
                        f"{modinfo.name}: frontend action {action.attrib.get('id')!r} is gated "
                        f"on {named!r}, which this tool cannot evaluate")
                if not all((c.text or "").strip().lower() in installed for c in conditions):
                    continue
            for item in action.iter("File"):
                path = modinfo.parent / (item.text or "").strip()
                if action.tag == "UpdateDatabase":
                    config.append(path)
                elif "en_US" in path.parts:
                    text.append(path)
    return config, text


def players(config: list[Path]) -> dict[tuple[str, str], dict]:
    """The live `Players` row for each (domain, leader) once every pack has loaded.

    A persona pack deletes the leader's shipped row and inserts its own, so a
    row is live unless another file deletes it. Two live rows for one key would
    be a primary-key collision in the game; here it is an error, never a pick.
    """
    rows: list[tuple[Path, dict[str, str]]] = []
    deletes: list[tuple[Path, dict[str, str]]] = []
    for path in config:
        root = _parse(path)
        if root is None:
            continue
        for table in root.iter("Players"):
            for item in table:
                fields = _fields(item)
                if item.tag == "Row":
                    fields.setdefault("Domain", DEFAULT_DOMAIN)
                    rows.append((path, fields))
                elif item.tag == "Delete":
                    deletes.append((path, fields))
                else:
                    raise HarvestError(f"{path}: unsupported <{item.tag}> in <Players>")
    live: dict[tuple[str, str], dict] = {}
    for path, fields in rows:
        if fields.get("HumanPlayable", "1") in ("0", "false"):
            continue
        if any(other != path and all(fields.get(key) == value for key, value in where.items())
               for other, where in deletes):
            continue
        key = (fields["Domain"], fields["LeaderType"])
        if key in live:
            raise HarvestError(
                f"{key} has two live rows: {live[key]['path']} and {path}")
        live[key] = {"path": path, **fields}
    return live


def english(text: list[Path], wanted: set[str]) -> dict[str, tuple[str, Path]]:
    """Each wanted LOC key's en_US string and the file that ships it."""
    found: dict[str, tuple[str, Path]] = {}
    for path in text:
        try:
            if not any(tag.encode() in path.read_bytes() for tag in wanted):
                continue
        except OSError:
            continue
        root = _parse(path)
        if root is None:
            continue
        for table in root:
            for item in table:
                tag = item.attrib.get("Tag")
                if tag not in wanted or item.attrib.get("Language", "en_US") != "en_US":
                    continue
                value = (item.findtext("Text") or "").strip()
                if tag in found and found[tag][0] != value:
                    raise HarvestError(
                        f"{tag} has two en_US strings: {found[tag][0]!r} in "
                        f"{found[tag][1]} and {value!r} in {path}")
                found.setdefault(tag, (value, path))
    return found


def harvest(assets: Path) -> dict:
    """The snapshot document for the install at ``assets``."""
    config, text = frontend_files(assets)
    live = players(config)
    domains = {domain: ruleset for ruleset, domain in RULESET_DOMAINS.items()}
    wanted = {row[column] for (domain, _), row in live.items() if domain in domains
              for column in ("LeaderName", "CivilizationName")}
    strings = english(text, wanted)
    leaders: dict[str, dict] = {}
    for (domain, leader), row in sorted(live.items()):
        if domain not in domains:
            continue  # a scenario's own roster
        for column in ("LeaderName", "CivilizationName"):
            if not strings.get(row[column], ("",))[0]:
                raise HarvestError(f"{leader}: no en_US text for {row[column]}")
        label, label_file = strings[row["LeaderName"]]
        entry = leaders.setdefault(leader, {
            "label": label,
            "civilization": row["CivilizationType"],
            "civilization_name": strings[row["CivilizationName"]][0],
            "name_key": row["LeaderName"],
            "text": label_file.relative_to(assets).as_posix(),
            "rows": {},
        })
        if (entry["label"], entry["civilization"]) != (label, row["CivilizationType"]):
            raise HarvestError(f"{leader} reads {entry['label']!r} under one ruleset and "
                               f"{label!r} under {domains[domain]}")
        entry["rows"].setdefault(row["path"].relative_to(assets).as_posix(),
                                 []).append(domains[domain])
    for entry in leaders.values():
        entry["rows"] = {path: sorted(rulesets) for path, rulesets in sorted(entry["rows"].items())}
    return {
        "schema": SCHEMA,
        "source": "tools/civ6_leader_labels.py --write, from an install with every pack",
        "query": PLAYERS_QUERY,
        "rulesets": RULESET_DOMAINS,
        "leaders": leaders,
    }


def render(document: dict) -> str:
    """The snapshot's text: one leader a line, so a changed label is a one-line diff."""
    head = {key: value for key, value in document.items() if key != "leaders"}
    lines = [json.dumps(head, indent=1, ensure_ascii=False)[:-2] + ",", ' "leaders": {']
    rows = [f"  {json.dumps(leader)}: {json.dumps(entry, ensure_ascii=False)}"
            for leader, entry in sorted(document["leaders"].items())]
    return "\n".join(lines + [",\n".join(rows), " }", "}"]) + "\n"


def labels(snapshot: dict | None = None, ruleset: str | None = None) -> dict[str, str]:
    """Leader type -> picker label, for one ruleset or for every ruleset."""
    if snapshot is None:
        snapshot = json.loads(SNAPSHOT.read_text(encoding="utf-8"))
    return {leader: entry["label"] for leader, entry in snapshot["leaders"].items()
            if ruleset is None or any(ruleset in rulesets for rulesets in entry["rows"].values())}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--civ6", default=None, help="Civilization VI install or Assets directory")
    parser.add_argument("--write", action="store_true", help="rewrite the committed snapshot")
    args = parser.parse_args(argv)
    try:
        document = harvest(civ6_env.assets_dir(args.civ6))
    except SystemExit as missing:
        print(missing, file=sys.stderr)
        return 2
    except HarvestError as error:
        print(f"refusing to guess: {error}", file=sys.stderr)
        return 3
    if len(document["leaders"]) < 19:
        print(f"only {len(document['leaders'])} leaders harvested; the base game alone "
              "ships 19, so this is not a readable install", file=sys.stderr)
        return 3
    rendered = render(document)
    if args.write:
        SNAPSHOT.write_text(rendered, encoding="utf-8")
        print(f"wrote {len(document['leaders'])} leaders to {SNAPSHOT}", file=sys.stderr)
        return 0
    current = SNAPSHOT.read_text(encoding="utf-8") if SNAPSHOT.is_file() else ""
    if current != rendered:
        print(f"{SNAPSHOT} does not match this install; rerun with --write and read "
              "the diff", file=sys.stderr)
        return 1
    print(f"{SNAPSHOT.name} matches this install ({len(document['leaders'])} leaders)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
