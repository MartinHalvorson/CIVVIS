#!/usr/bin/env python3
"""The Project Chronology page's record: the project's milestones, in date order.

    chronology_page.py <source_tree>              print the record as JSON
    chronology_page.py <source_tree> <page.html>  write it into the page

Project Chronology (`beta/chronology.html`, served at
`<lane>/charts/project-chronology/`) is the last page under Charts/ (operator
2026-11-28: "add a project chronology to charts/ at the end for now. post the
important milestones along a timeline. show the progress we've made. ai
capability, simulator performance, new features, other").

The milestones are `data/chronology.json`, a list somebody keeps by hand: what
happened, the UTC day it happened, which of the four categories it stands in
and where a reader can check it. Nothing here is counted from games; each
milestone names its own evidence. This script reads that file from the lane's
own revision, holds every line to its shape (`problems`), and writes the
page's record:

- **from**, **to** and **days**: the first and last milestone's day and the
  days between them.
- **categories**, in the file's order, the order of the timeline's rows: each
  with its key, its name, the sentence that says what it holds and how many
  milestones stand in it.
- **milestones**, oldest first (the file's order within a day): the day, the
  category's key, the title, the detail, `major` and `label` where the file
  sets them, and `sources`, each a `label` to print and, where there is a
  page to open, an `href`.
- **progress**, the row of figures over the timeline, as the file gives
  them: what was measured, the reading before and the reading now, the days
  they were read on, and the category whose colour the figure wears.
- **checked**, the file's sentence on when the ledger's figures were read.

How a source is worded (`source`): a pull request is `#541` and opens on
GitHub; a commit is its short hash and opens on GitHub; a release opens its
release page; a file of this repository opens on main; a video opens on
YouTube and reads as its `says`. A game on the ledger is printed as its tag
and has no address of its own (an early run has no folder on the ledger
branch, and `results.jsonl` is too big for GitHub to draw), so the page says
once where the tags are kept.

Like the site's other pages this runs inside the script that publishes the
whole site: a file it cannot read, or a line out of shape, costs the page its
record, said on stderr, and never the publish. `beta/chronology_page_test.py`
holds the committed file to the same shape, so a bad line fails there first.
"""

from __future__ import annotations

import datetime as dt
import json
import pathlib
import re
import sys

SLOT = re.compile(r'(<script type="application/json" id="chronology-data">)(.*?)(</script>)', re.S)
DATA = pathlib.Path("data/chronology.json")
REPO = "https://github.com/MartinHalvorson/CIVVIS"
#: The kinds of source a milestone may name, and what each `ref` must be.
KINDS = ("pr", "commit", "ledger", "data", "video", "release")
COMMIT = re.compile(r"^[0-9a-f]{7,40}$")
VIDEO = re.compile(r"^https://www\.youtube\.com/watch\?v=[\w-]{11}$")
#: What a figure of the progress row carries, every one of them words.
PROGRESS = ("category", "label", "was", "now", "span")
#: A release tag, a ledger game's tag and a path in the tree are put into an
#: address or printed as they are, so each is held to plain characters.
PLAIN = re.compile(r"^[A-Za-z0-9._/-]+$")


def day(text) -> dt.date | None:
    """A `YYYY-MM-DD` day, or `None` for anything else."""
    try:
        found = dt.date.fromisoformat(str(text))
    except ValueError:
        return None
    return found if found.isoformat() == text else None


def source(entry: dict) -> dict:
    """One source as the page prints it: `label`, and `href` where it opens."""
    kind, ref, says = entry["kind"], entry["ref"], entry.get("says")
    if kind == "pr":
        return {"kind": kind, "label": says or f"#{ref}", "href": f"{REPO}/pull/{ref}"}
    if kind == "commit":
        return {"kind": kind, "label": says or ref[:7], "href": f"{REPO}/commit/{ref}"}
    if kind == "release":
        return {"kind": kind, "label": says or ref, "href": f"{REPO}/releases/tag/{ref}"}
    if kind == "data":
        return {"kind": kind, "label": says or ref.rsplit("/", 1)[-1], "href": f"{REPO}/blob/main/{ref}"}
    if kind == "video":
        return {"kind": kind, "label": says or "the video", "href": ref}
    return {"kind": kind, "label": says or f"game {ref}"}


def source_problems(entry, where: str) -> list[str]:
    if not isinstance(entry, dict) or entry.get("kind") not in KINDS:
        return [f"{where}: a source is one of {', '.join(KINDS)}"]
    kind, ref, says = entry["kind"], entry.get("ref"), entry.get("says")
    out = []
    if says is not None and not (isinstance(says, str) and says.strip()):
        out.append(f"{where}: `says` is words or is left out")
    if kind == "pr":
        if not (isinstance(ref, int) and not isinstance(ref, bool) and ref > 0):
            out.append(f"{where}: a pull request is its number")
    elif not isinstance(ref, str):
        out.append(f"{where}: a {kind} source's `ref` is text")
    elif kind == "commit" and not COMMIT.match(ref):
        out.append(f"{where}: a commit is seven or more hex digits")
    elif kind == "video" and not VIDEO.match(ref):
        out.append(f"{where}: a video is a YouTube watch address")
    elif kind == "video" and not says:
        out.append(f"{where}: a video says what it shows")
    elif kind in ("ledger", "data", "release") and not PLAIN.match(ref):
        out.append(f"{where}: {ref!r} is not a plain tag or path")
    return out


def problems(data) -> list[str]:
    """Everything out of shape in the file, a sentence each; empty when sound."""
    if not isinstance(data, dict):
        return ["the file is not an object"]
    out = []
    categories = data.get("categories")
    keys: list[str] = []
    if not isinstance(categories, list) or not categories:
        out.append("no `categories`")
    else:
        for at, category in enumerate(categories):
            if not isinstance(category, dict) or not all(
                    isinstance(category.get(field), str) and category[field].strip()
                    for field in ("key", "name", "says")):
                out.append(f"category {at + 1}: needs a `key`, a `name` and a `says`")
            else:
                keys.append(category["key"])
        if len(set(keys)) != len(keys):
            out.append("two categories share a key")
    progress = data.get("progress", [])
    if not isinstance(progress, list):
        out.append("`progress` is a list or is left out")
    else:
        for at, figure in enumerate(progress):
            if not isinstance(figure, dict) or not all(
                    isinstance(figure.get(field), str) and figure[field].strip()
                    for field in PROGRESS):
                out.append(f"progress {at + 1}: needs {', '.join(f'`{field}`' for field in PROGRESS)}")
            elif figure["category"] not in keys:
                out.append(f"progress {at + 1} ({figure['label']}): `category` is one of {', '.join(keys)}")
    milestones = data.get("milestones")
    if not isinstance(milestones, list) or not milestones:
        return out + ["no `milestones`"]
    last = ""
    for at, milestone in enumerate(milestones):
        where = f"milestone {at + 1}"
        if not isinstance(milestone, dict):
            out.append(f"{where}: not an object")
            continue
        title = milestone.get("title")
        if isinstance(title, str) and title.strip():
            where = f"{where} ({title})"
        else:
            out.append(f"{where}: no `title`")
        if day(milestone.get("date")) is None:
            out.append(f"{where}: `date` is a YYYY-MM-DD day")
        else:
            if milestone["date"] < last:
                out.append(f"{where}: dated {milestone['date']}, before the line over it ({last})")
            last = max(last, milestone["date"])
        if milestone.get("category") not in keys:
            out.append(f"{where}: `category` is one of {', '.join(keys) or 'the categories'}")
        detail = milestone.get("detail")
        if not (isinstance(detail, str) and detail.strip()):
            out.append(f"{where}: no `detail`")
        if "major" in milestone and milestone["major"] is not True:
            out.append(f"{where}: `major` is true or is left out")
        label = milestone.get("label")
        if label is not None and not (isinstance(label, str) and label.strip()):
            out.append(f"{where}: `label` is words or is left out")
        if milestone.get("major") is True and not label:
            out.append(f"{where}: a major milestone has the short `label` the chart prints")
        sources = milestone.get("sources")
        if not isinstance(sources, list) or not sources:
            out.append(f"{where}: no `sources`")
        else:
            for entry in sources:
                out.extend(source_problems(entry, where))
    return out


def record(source_tree: pathlib.Path) -> dict | None:
    """The page's record, or `None` when the file cannot be read or is out of shape."""
    path = source_tree / DATA
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        print(f"chronology_page: {path} cannot be read ({error})", file=sys.stderr)
        return None
    wrong = problems(data)
    if wrong:
        for sentence in wrong:
            print(f"chronology_page: {DATA}: {sentence}", file=sys.stderr)
        return None
    milestones = []
    for milestone in data["milestones"]:
        row = {
            "date": milestone["date"],
            "category": milestone["category"],
            "title": milestone["title"].strip(),
            "detail": milestone["detail"].strip(),
            "sources": [source(entry) for entry in milestone["sources"]],
        }
        if milestone.get("major"):
            row["major"] = True
            row["label"] = milestone["label"].strip()
        milestones.append(row)
    # The file is in date order already (`problems`), and a day's milestones
    # keep the order the file gives them.
    first, last = day(milestones[0]["date"]), day(milestones[-1]["date"])
    return {
        "from": first.isoformat(),
        "to": last.isoformat(),
        "days": (last - first).days,
        "checked": data.get("checked") if isinstance(data.get("checked"), str) else None,
        "progress": [{field: figure[field].strip() for field in PROGRESS} for figure in data.get("progress", [])],
        "categories": [
            {
                "key": category["key"],
                "name": category["name"].strip(),
                "says": category["says"].strip(),
                "count": sum(1 for milestone in milestones if milestone["category"] == category["key"]),
            }
            for category in data["categories"]
        ],
        "milestones": milestones,
    }


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    # `<` is escaped so no title or sentence can close the script element.
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, count = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if count else None


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = record(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        print(f"chronology_page: {page} has no chronology slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"chronology_page: no milestones to write into {page}", file=sys.stderr)
    else:
        said = ", ".join(f"{category['name']} {category['count']}" for category in data["categories"])
        print(f"chronology_page: wrote {len(data['milestones'])} milestones from {data['from']} "
              f"to {data['to']} ({said}) into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
