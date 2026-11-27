#!/usr/bin/env python3
"""The repository's licence and its list of other people's work stay true.

CIVVIS's own code is MIT (`LICENSE`, `Cargo.toml`); what others wrote keeps
its own licence and is listed in `THIRD_PARTY_NOTICES.md` (operator,
2026-11-27: choose a licence and reduce takedown risk). A list like that goes
stale the day a crate is added or a vendored file is swapped, and nothing
else would notice, so this holds it to what the repository actually carries:

- every crate in `Cargo.lock` is named in the notices at its locked version,
  so a new dependency or a bump is a line in the notices too;
- every file in `beta/chess/vendor/` is named there;
- Stockfish's GPL text stays beside it, and the directions to the source of
  that exact build (the Stockfish.js commit and the network) are both beside
  it and in the notices, which is what passing it on under GPL-3.0 needs;
- the README ends with the licence and trademark notice, and the home and
  download pages carry the non-affiliation line.
"""

from __future__ import annotations

import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parent.parent
NOTICES = (ROOT / "THIRD_PARTY_NOTICES.md").read_text(encoding="utf-8")
VENDOR = ROOT / "beta" / "chess" / "vendor"

STOCKFISH_COMMIT = "93c994592dcf3b4b21052ab925e9b534df9c0918"
STOCKFISH_NET = "nn-9067e33176e8.nnue"
NOT_AFFILIATED = (
    "CIVVIS is a fan project, not affiliated with or endorsed by Firaxis Games, "
    "2K or Take-Two Interactive."
)


def locked_crates() -> list[tuple[str, str]]:
    lock = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
    found = re.findall(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"', lock)
    return [(name, version) for name, version in found if name != "civvis"]


class Licence(unittest.TestCase):
    def test_our_own_code_is_mit(self):
        licence = (ROOT / "LICENSE").read_text(encoding="utf-8")
        self.assertTrue(licence.startswith("MIT License\n\nCopyright (c) 2026 The CIVVIS authors\n"))
        self.assertIn("Permission is hereby granted, free of charge, to any person obtaining a copy", licence)
        self.assertIn('THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND', licence)
        package = (ROOT / "Cargo.toml").read_text(encoding="utf-8").split("\n[", 1)[0]
        self.assertIn('\nlicense = "MIT"\n', package)


class Notices(unittest.TestCase):
    def test_every_locked_crate_is_listed_at_its_version(self):
        crates = locked_crates()
        self.assertGreaterEqual(len(crates), 10, "Cargo.lock's packages were not found")
        for name, version in crates:
            with self.subTest(crate=name):
                self.assertIn(f"`{name}`", NOTICES)
                line = [text for text in NOTICES.splitlines() if f"`{name}`" in text]
                self.assertTrue(any(version in text for text in line), f"{name} {version} is not the version listed")

    def test_every_vendored_file_is_listed(self):
        for path in sorted(VENDOR.iterdir()):
            if path.name == "README.md":
                continue
            with self.subTest(file=path.name):
                self.assertIn(f"beta/chess/vendor/{path.name}", NOTICES)

    def test_stockfish_travels_with_its_licence_and_its_source(self):
        licence = (VENDOR / "stockfish.LICENSE").read_text(encoding="utf-8")
        self.assertIn("GNU GENERAL PUBLIC LICENSE", licence)
        self.assertIn("Version 3, 29 June 2007", licence)
        beside = (VENDOR / "README.md").read_text(encoding="utf-8")
        for said in (beside, NOTICES):
            self.assertIn(f"https://github.com/nmrugg/stockfish.js/tree/{STOCKFISH_COMMIT}", said)
            self.assertIn(f"https://tests.stockfishchess.org/api/nn/{STOCKFISH_NET}", said)
            self.assertIn("stockfish@18.0.8", said)
        # The build these directions name is the one the page loads.
        engine = (ROOT / "beta" / "chess" / "game.js").read_text(encoding="utf-8")
        self.assertIn("./vendor/stockfish-18-lite-single.js", engine)

    def test_the_readme_ends_with_the_licence_and_trademarks(self):
        readme = (ROOT / "README.md").read_text(encoding="utf-8")
        tail = readme[readme.rindex("\n## ") + 1:]
        self.assertTrue(tail.startswith("## Licence and trademarks\n"), "the notice is not the README's last section")
        for piece in ("[MIT License](LICENSE)", "[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)", NOT_AFFILIATED,
                      "Sid Meier's Civilization and Civilization VI are their trademarks.",
                      "You need your own copy of the game to play against it."):
            self.assertIn(piece, tail)

    def test_the_site_says_it_is_not_affiliated(self):
        for page in ("landing.html", "download.html"):
            with self.subTest(page=page):
                text = (ROOT / "beta" / page).read_text(encoding="utf-8")
                self.assertIn(NOT_AFFILIATED, text)
                self.assertIn("Sid Meier&#8217;s Civilization and Civilization VI are their trademarks.", text)
        download = (ROOT / "beta" / "download.html").read_text(encoding="utf-8")
        self.assertIn("MIT licence", download)
        self.assertIn("THIRD_PARTY_NOTICES.md", download)


if __name__ == "__main__":
    unittest.main()
