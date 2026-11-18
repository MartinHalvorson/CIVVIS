"""`civ6_leader_labels.py` reads the leader picker's rows the way the game assembles them.

The harvest is checked on a small install written here, so it runs anywhere:
a base leader, a persona pack that deletes and replaces that leader's row, a
file gated on another pack, a scenario roster, and our own control mod, which
must never be read back as something the game ships. On a host with the game,
the committed snapshot is also compared with the install itself.
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_env  # noqa: E402
import civ6_leader_labels as harvest  # noqa: E402

ROW = ('<Row {domain}CivilizationType="CIVILIZATION_{civ}" LeaderType="LEADER_{leader}" '
       'CivilizationName="LOC_CIVILIZATION_{civ}_NAME" LeaderName="LOC_LEADER_{key}_NAME"/>')


def row(civ: str, leader: str, key: str | None = None, domain: str | None = None) -> str:
    return ROW.format(civ=civ, leader=leader, key=key or leader,
                      domain=f'Domain="{domain}" ' if domain else "")


def players(*items: str) -> str:
    return "<GameInfo><Players>" + "".join(items) + "</Players></GameInfo>"


def text(table: str, **strings: str) -> str:
    rows = "".join(f'<Row Tag="{tag}"><Text>{value}</Text></Row>'
                   for tag, value in strings.items())
    return f"<GameInfo><{table}>{rows}</{table}></GameInfo>"


def modinfo(mod_id: str, actions: str, criteria: str = "") -> str:
    return (f'<Mod id="{mod_id}" version="1"><ActionCriteria>{criteria}</ActionCriteria>'
            f"<FrontEndActions>{actions}</FrontEndActions></Mod>")


EXPANSION = "Players:Expansion2_Players"
PERSONA_CONFIG = players(
    '<Delete Domain="Players:StandardPlayers" CivilizationType="CIVILIZATION_AMERICA" '
    'LeaderType="LEADER_T_ROOSEVELT"/>',
    f'<Delete Domain="{EXPANSION}" CivilizationType="CIVILIZATION_AMERICA" '
    'LeaderType="LEADER_T_ROOSEVELT"/>',
    row("AMERICA", "T_ROOSEVELT", "T_ROOSEVELT_ORIGINAL"),
    row("AMERICA", "T_ROOSEVELT_ROUGHRIDER"),
    row("AMERICA", "T_ROOSEVELT", "T_ROOSEVELT_ORIGINAL", EXPANSION),
    row("AMERICA", "T_ROOSEVELT_ROUGHRIDER", domain=EXPANSION),
)


def install(root: Path, *, persona: bool = True, extra: dict[str, str] | None = None) -> Path:
    """A tiny Assets tree: a base game, one expansion, and optional packs."""
    files = {
        "Base/Assets/Configuration/Data/Players.xml": players(
            row("AMERICA", "T_ROOSEVELT"), row("ROME", "TRAJAN")),
        "Base/Assets/Text/en_US/Leaders_Text.xml": text(
            "BaseGameText", LOC_LEADER_T_ROOSEVELT_NAME="Teddy Roosevelt",
            LOC_LEADER_TRAJAN_NAME="Trajan", LOC_CIVILIZATION_AMERICA_NAME="America",
            LOC_CIVILIZATION_ROME_NAME="Rome"),
        "DLC/Expansion2/Expansion2.modinfo": modinfo(
            "expansion-2",
            "<UpdateDatabase><File>Config/Expansion2_Players.xml</File></UpdateDatabase>"
            "<UpdateText><File>Text/en_US/Expansion2_ConfigText.xml</File>"
            "<File>Text/Expansion2_Translations_ConfigText.xml</File></UpdateText>"),
        "DLC/Expansion2/Config/Expansion2_Players.xml": players(
            row("AMERICA", "T_ROOSEVELT", domain=EXPANSION),
            row("ROME", "TRAJAN", domain=EXPANSION),
            row("FRANCE", "ELEANOR_FRANCE", domain=EXPANSION),
            row("FRANCE", "SCENARIO_KING", domain="SomeScenario_Players")),
        "DLC/Expansion2/Text/en_US/Expansion2_ConfigText.xml": text(
            "EnglishText", LOC_LEADER_ELEANOR_FRANCE_NAME="Eleanor of Aquitaine (France)",
            LOC_CIVILIZATION_FRANCE_NAME="France"),
        # A translation of the same key is not the English label.
        "DLC/Expansion2/Text/Expansion2_Translations_ConfigText.xml": (
            '<GameInfo><LocalizedText><Replace Tag="LOC_LEADER_TRAJAN_NAME" Language="fr_FR">'
            "<Text>Trajan le Grand</Text></Replace></LocalizedText></GameInfo>"),
        # Our own mod is installed into the game's tree and whitelists one leader.
        "DLC/CivvisControl/CivvisControl.modinfo": modinfo(
            "civvis", "<UpdateDatabase><File>Config.xml</File></UpdateDatabase>"),
        "DLC/CivvisControl/Config.xml": players(row("ROME", "CIVVIS_INVENTED")),
    }
    if persona:
        files.update({
            "DLC/TeddyRoosevelt/Teddy_Roosevelt.modinfo": modinfo(
                "teddy",
                "<UpdateDatabase><File>Data/TeddyRoosevelt_ConfigData.xml</File></UpdateDatabase>"
                "<UpdateText><File>Text/en_US/TeddyRoosevelt_ConfigText.xml</File></UpdateText>"),
            "DLC/TeddyRoosevelt/Data/TeddyRoosevelt_ConfigData.xml": PERSONA_CONFIG,
            "DLC/TeddyRoosevelt/Text/en_US/TeddyRoosevelt_ConfigText.xml": text(
                "EnglishText",
                LOC_LEADER_T_ROOSEVELT_ORIGINAL_NAME="Teddy Roosevelt (Bull Moose)",
                LOC_LEADER_T_ROOSEVELT_ROUGHRIDER_NAME="Teddy Roosevelt (Rough Rider)"),
        })
    files.update(extra or {})
    for relative, content in files.items():
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    return root


class HarvestTests(unittest.TestCase):
    def harvest(self, **kwargs) -> dict:
        with tempfile.TemporaryDirectory() as temporary:
            return harvest.harvest(install(Path(temporary), **kwargs))

    def test_a_persona_pack_replaces_the_base_leaders_label(self) -> None:
        document = self.harvest()
        self.assertEqual(harvest.labels(document), {
            "LEADER_ELEANOR_FRANCE": "Eleanor of Aquitaine (France)",
            "LEADER_T_ROOSEVELT": "Teddy Roosevelt (Bull Moose)",
            "LEADER_T_ROOSEVELT_ROUGHRIDER": "Teddy Roosevelt (Rough Rider)",
            "LEADER_TRAJAN": "Trajan",
        })
        teddy = document["leaders"]["LEADER_T_ROOSEVELT"]
        self.assertEqual(teddy["name_key"], "LOC_LEADER_T_ROOSEVELT_ORIGINAL_NAME")
        self.assertEqual(teddy["civilization_name"], "America")
        self.assertEqual(teddy["text"],
                         "DLC/TeddyRoosevelt/Text/en_US/TeddyRoosevelt_ConfigText.xml")
        # The live row in both rulesets is the pack's, not the one it deleted.
        self.assertEqual(teddy["rows"], {
            "DLC/TeddyRoosevelt/Data/TeddyRoosevelt_ConfigData.xml":
                ["RULESET_EXPANSION_2", "RULESET_STANDARD"]})

    def test_without_the_pack_the_base_label_stands(self) -> None:
        document = self.harvest(persona=False)
        self.assertEqual(harvest.labels(document)["LEADER_T_ROOSEVELT"], "Teddy Roosevelt")
        self.assertNotIn("LEADER_T_ROOSEVELT_ROUGHRIDER", document["leaders"])

    def test_a_ruleset_lists_only_its_own_domain(self) -> None:
        document = self.harvest()
        self.assertNotIn("LEADER_ELEANOR_FRANCE", harvest.labels(document, "RULESET_STANDARD"))
        self.assertIn("LEADER_ELEANOR_FRANCE", harvest.labels(document, "RULESET_EXPANSION_2"))
        self.assertEqual(harvest.labels(document, "RULESET_EXPANSION_1"), {})

    def test_a_scenario_roster_and_our_own_mod_are_not_harvested(self) -> None:
        leaders = self.harvest()["leaders"]
        self.assertNotIn("LEADER_SCENARIO_KING", leaders)
        self.assertNotIn("LEADER_CIVVIS_INVENTED", leaders)

    def test_a_file_gated_on_a_pack_loads_only_with_that_pack(self) -> None:
        def gated(on: str) -> dict[str, str]:
            return {
                "DLC/Builders/Builders.modinfo": modinfo(
                    "builders",
                    '<UpdateDatabase criteria="NeedsPack"><File>Korea.xml</File></UpdateDatabase>'
                    "<UpdateText><File>Text/en_US/Builders_ConfigText.xml</File></UpdateText>",
                    f'<Criteria id="NeedsPack"><ModInUse>{on}</ModInUse></Criteria>'),
                "DLC/Builders/Korea.xml": players(row("KOREA", "SEJONG", domain=EXPANSION)),
                "DLC/Builders/Text/en_US/Builders_ConfigText.xml": text(
                    "EnglishText", LOC_LEADER_SEJONG_NAME="Sejong",
                    LOC_CIVILIZATION_KOREA_NAME="Korea"),
            }

        # The game compares pack ids without regard to case.
        self.assertEqual(self.harvest(extra=gated("EXPANSION-2"))["leaders"]
                         ["LEADER_SEJONG"]["label"], "Sejong")
        self.assertNotIn("LEADER_SEJONG", self.harvest(extra=gated("not-installed"))["leaders"])

    def test_what_cannot_be_read_into_one_answer_is_refused(self) -> None:
        collision = {
            "DLC/Other/Other.modinfo": modinfo(
                "other", "<UpdateDatabase><File>Other.xml</File></UpdateDatabase>"),
            "DLC/Other/Other.xml": players(row("ROME", "TRAJAN", "T_ROOSEVELT")),
        }
        unknown_gate = {
            "DLC/Other/Other.modinfo": modinfo(
                "other",
                '<UpdateDatabase criteria="Odd"><File>Other.xml</File></UpdateDatabase>',
                '<Criteria id="Odd"><RuleSetInUse>RULESET_STANDARD</RuleSetInUse></Criteria>'),
            "DLC/Other/Other.xml": players(),
        }
        no_text = {
            "DLC/Other/Other.modinfo": modinfo(
                "other", "<UpdateDatabase><File>Other.xml</File></UpdateDatabase>"),
            "DLC/Other/Other.xml": players(row("ROME", "UNNAMED")),
        }
        two_strings = {
            "DLC/Other/Other.modinfo": modinfo(
                "other", "<UpdateText><File>Text/en_US/Other.xml</File></UpdateText>"),
            "DLC/Other/Text/en_US/Other.xml": text(
                "EnglishText", LOC_LEADER_TRAJAN_NAME="Marcus Ulpius Traianus"),
        }
        for why, extra in {"two live rows": collision, "which this tool cannot evaluate":
                           unknown_gate, "no en_US text": no_text,
                           "two en_US strings": two_strings}.items():
            with self.subTest(why=why):
                with self.assertRaisesRegex(harvest.HarvestError, why):
                    self.harvest(extra=extra)

    def test_the_snapshot_text_is_one_leader_a_line_and_round_trips(self) -> None:
        document = self.harvest()
        rendered = harvest.render(document)
        self.assertEqual(json.loads(rendered), document)
        lines = rendered.splitlines()
        for leader in document["leaders"]:
            self.assertEqual(sum(f'"{leader}": ' in line for line in lines), 1)

    def test_the_committed_snapshot_is_in_the_rendered_form(self) -> None:
        committed = harvest.SNAPSHOT.read_text(encoding="utf-8")
        self.assertEqual(harvest.render(json.loads(committed)), committed)


def installed_assets() -> Path | None:
    try:
        assets = civ6_env.assets_dir()
    except SystemExit:
        return None
    return assets if (assets / harvest.BASE_CONFIG).is_dir() else None


class InstalledGameTests(unittest.TestCase):
    """On a host that has the game: the snapshot is what that install ships."""

    @unittest.skipUnless(installed_assets(), "Civilization VI is not installed on this host")
    def test_the_snapshot_matches_this_install_when_it_has_every_pack(self) -> None:
        document = harvest.harvest(installed_assets())
        committed = json.loads(harvest.SNAPSHOT.read_text(encoding="utf-8"))
        if len(document["leaders"]) < len(committed["leaders"]):
            self.skipTest("this install lacks packs the snapshot was read from")
        self.assertEqual(document, committed)


if __name__ == "__main__":
    unittest.main()
