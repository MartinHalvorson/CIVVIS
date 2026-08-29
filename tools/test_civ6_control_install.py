#!/usr/bin/env python3
"""The protected-DLC installer path must stay usable without shell access."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import Mock, call, patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

from civ6_control import install  # noqa: E402


class ProtectedInstallTest(unittest.TestCase):
    def test_city_export_preserves_great_works_citizens_yields_and_progress(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("local function exportState", 1)[1].split(
            "local units = {};", 1
        )[0]

        self.assertIn("citizens:IsPlotWorked(px, py)", exporter)
        self.assertIn("plot:GetWorkerCount()", exporter)
        self.assertIn("blds:GetGreatWorkInSlot", exporter)
        self.assertIn("Game.GetGreatWorkDataFromIndex", exporter)
        self.assertIn("city:GetYield(YieldTypes.SCIENCE)", exporter)
        self.assertIn(
            "local prodProgress, prodCost = productionProgress(city, queue)", exporter
        )
        self.assertIn("production_progress = prodProgress", exporter)
        self.assertIn("production_cost = prodCost", exporter)

    def test_state_export_has_fog_safe_public_empire_totals_for_every_major(self) -> None:
        """A standings row needs empire totals even when its cities are unseen.

        The detail loop remains visibility-gated, while this aggregate carries
        no city or unit identity/location. Keep the distinction in the export:
        losing the aggregate makes every fogged rival look like a zero-city
        empire in the player HUD.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("local function exportState", 1)[1]
        rivals = exporter.split("-- Rivals:", 1)[1].split(
            "-- Met city-states", 1
        )[0]
        rival_record = rivals.split("rivals[#rivals + 1] = {", 1)[1]

        self.assertIn("local function publicEmpireStats(subject, suzerainCounts)", exporter)
        self.assertIn("city:GetYield(YieldTypes.FOOD)", exporter)
        self.assertIn("city:GetYield(YieldTypes.PRODUCTION)", exporter)
        self.assertIn("weapons:GetWeaponCount(definition.Index)", exporter)
        self.assertIn("local function publicSuzerainCounts()", exporter)
        self.assertIn("public_stats = publicStats,", exporter)
        self.assertIn("public_stats = otherPublicStats,", rivals)
        self.assertIn("government = try(function()", rival_record)
        self.assertIn("culture:GetCurrentGovernment()", rival_record)
        self.assertIn("Game.GetEras():HasDarkAge(otherId)", rival_record)
        self.assertIn("Game.GetEras():HasGoldenAge(otherId)", rival_record)
        self.assertIn("Game.GetEras():HasHeroicGoldenAge(otherId)", rival_record)
        self.assertLess(
            rivals.index("local otherPublicStats = publicEmpireStats(other, suzerainCounts);"),
            rivals.index("for _, city in other:GetCities():Members() do"),
        )
        self.assertIn("local seen = plotRevealed(pid, cx, cy);", rivals)

    def test_state_export_keeps_completed_strategic_projects_out_of_city_queues(self) -> None:
        """A fresh mirror needs player history, not just the current queue.

        Firaxis removes a completed Manhattan Project from every city's queue,
        but keeps it in PlayerStats. The exact counter is also what the shipped
        World Rankings screen uses for completed science milestones. Pin the
        narrow strategic whitelist so repeatable district projects can never be
        mistaken for a completed science-victory stage.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("local function exportState", 1)[1]

        self.assertIn("local scienceProjects = {};", exporter)
        self.assertIn("playerStats:GetNumProjectsAdvanced(project.Index)", exporter)
        self.assertIn("science_projects = scienceProjects,", exporter)
        for project in (
            "PROJECT_MANHATTAN_PROJECT",
            "PROJECT_OPERATION_IVY",
            "PROJECT_LAUNCH_EARTH_SATELLITE",
            "PROJECT_LAUNCH_MOON_LANDING",
            "PROJECT_LAUNCH_MARS_REACTOR",
            "PROJECT_LAUNCH_MARS_HABITATION",
            "PROJECT_LAUNCH_MARS_HYDROPONICS",
            "PROJECT_LAUNCH_MARS_BASE",
            "PROJECT_LAUNCH_EXOPLANET_EXPEDITION",
        ):
            self.assertIn(project, exporter)

        self.assertNotIn(
            "for row in GameInfo.Projects()",
            exporter,
            "walking every project would count repeatable district conversions as milestones",
        )

    def test_purchase_actuator_preserves_faith_formation_and_district_placement(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        self.assertIn('kind == "purchase_faith"', source)
        self.assertIn('kind == "purchase_faith" and "YIELD_FAITH" or "YIELD_GOLD"', source)
        self.assertIn("MilitaryFormationTypes.CORPS_MILITARY_FORMATION", source)
        self.assertIn("MilitaryFormationTypes.ARMY_MILITARY_FORMATION", source)
        self.assertIn(
            "formationForCost = MilitaryFormationTypes.STANDARD_MILITARY_FORMATION",
            source,
        )
        self.assertNotIn(
            "params[CityCommandTypes.PARAM_MILITARY_FORMATION_TYPE] =\n"
            "\t\t\t\t\tMilitaryFormationTypes.STANDARD_MILITARY_FORMATION",
            source,
        )
        self.assertIn("params[CityOperationTypes.PARAM_X] = x", source)
        self.assertIn("params[CityOperationTypes.PARAM_Y] = y", source)
        self.assertIn("results[CityCommandResults.FAILURE_REASONS]", source)
        self.assertIn("reasons = reasons", source)

    def test_upgrade_refusal_uses_the_unit_panels_current_turn_probe(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        diagnostic = source.split("local function upgradeUnit(unit)", 1)[1].split(
            "return nil;", 1
        )[0]
        self.assertIn(
            'unit, CMD["UNITCOMMAND_UPGRADE"], false, true', diagnostic
        )
        self.assertNotIn(
            'unit, CMD["UNITCOMMAND_UPGRADE"], true, true', diagnostic
        )

    def test_every_unit_refusal_asks_the_engine_through_one_helper(self) -> None:
        """The `CanStartOperation` signature has been guessed wrong twice.

        Each time, the results table came back empty and every refusal the
        project recorded said `can_start=false` with no reasons — the one thing
        the event exists to report. The details that matter are the BOOLEAN
        fourth argument, `OperationResultsTypes.ALL` as the fifth (not `true`),
        and reasons under `UnitOperationResults.FAILURE_REASONS` rather than at
        the top level. Pin all three, in one helper, so a third guess cannot
        quietly reintroduce a silent refusal ledger.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        self.assertIn("local function refusalReason(unit, operation, params)", source)
        self.assertIn("results[UnitOperationResults.FAILURE_REASONS]", source)

        # ⚠ THE SIGNATURE HAS NOW BEEN GUESSED WRONG THREE TIMES. The third
        # guess put `params` in the PLOTS slot and `false` in the PARAMS slot,
        # which throws — and the first live run on that build read
        # `why: "unknown"` six times out of six. `canOperate` proves the hash
        # form takes params fourth; the shipped FOUND_CITY form takes a boolean
        # there. So both are tried and each names itself, and the broken shape
        # must never come back.
        self.assertIn("unit, operation, nil, params or {},", source)
        self.assertIn("unit, operation, nil, false, OperationResultsTypes.ALL);", source)
        # Matched against the CALL, not the comment above it, which quotes the
        # broken shape on purpose so the next reader knows what not to write.
        self.assertNotIn(
            "UnitManager.CanStartOperation(\n\t\t\t\t\tunit, operation, params, false,",
            source,
        )
        self.assertIn('{ form = "p4r", call = function()', source)
        self.assertIn('{ form = "t5r", call = function()', source)

        # A probe that cannot say which call answered is how this went wrong
        # three times, so every outcome carries its provenance and a throw is
        # reported rather than swallowed into a bare "unknown".
        self.assertIn('.. " [" .. attempt.form .. "]"', source)
        self.assertIn('"probe_threw[" .. attempt.form .. "]:"', source)
        self.assertNotIn('local why = "unknown";', source)

        # Both unit refusals go through it, and neither keeps a private copy of
        # the call — a second copy is how the signature drifts.
        self.assertIn(
            "refusalReason(unit, UnitOperationTypes.FOUND_CITY, nil)", source
        )
        self.assertIn(
            'refusalReason(unit, OP["UNITOPERATION_BUILD_IMPROVEMENT"],', source
        )
        self.assertEqual(
            source.count("UnitManager.CanStartOperation(\n\t\t\t\t\tunit, operation"),
            2,
            "exactly the two shipped forms, both inside the one helper",
        )

        # `improve_refused` is the most numerous refusal in the ledger and named
        # only the tile; it has to carry the cause or the three failures it
        # conflates (unowned ground, stale mirror, a builder that cannot act)
        # stay indistinguishable.
        self.assertIn('want = wanted or "IMPROVE", why = why,', source)

    def test_the_emergency_wall_override_needs_an_enemy_that_is_actually_near(self) -> None:
        """The gate claimed a neighbourhood it never had.

        `cityWarThreat` walks every visible unit of every player we are at war
        with and bounds the distance by nothing, so `nearestEnemy ~= nil` was
        true whenever any enemy was visible anywhere. Measured over eight live
        runs on 2026-08-11: 160 overrides, 94% at zero damage, 70% with the
        nearest enemy five or more tiles away, taking 46 Campus and 13 Library
        builds away from CIVVIS to buy a wall.

        Damage must still override at any distance — that is real evidence — so
        pin both halves, and pin that the threshold reaches the event, because a
        gate whose threshold is not recorded cannot be audited from the ledger.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        self.assertIn("local wallRadius = cfg.EmergencyWallRadius or 3;", source)
        self.assertIn(
            "local immediateThreat = maxWallDamage ~= nil and maxWallDamage <= 0\n"
            "\t\t\tand ((damage ~= nil and damage > 0)\n"
            "\t\t\t\tor (nearestEnemy ~= nil and nearestEnemy <= wallRadius));",
            source,
        )
        self.assertNotIn(
            "or nearestEnemy ~= nil) then",
            source,
            "the unbounded enemy test must not come back",
        )
        self.assertIn("radius = wallRadius,", source)

        # A wall that cannot finish before an already-queued defender is not
        # an emergency defense.  The live Ostia loss at t61 came from replacing
        # a one-turn Archer with Walls requiring four turns; the preserve path
        # must run before the remembered build can replace that Archer.
        self.assertIn(
            'return city:GetBuildQueue():GetTurnsLeft();', source,
        )
        self.assertIn('return GameInfo.Units[current];', source)
        self.assertIn(
            'local finishingDefender = currentTurns >= 0 and currentTurns <= 1',
            source,
        )
        self.assertIn('emit("emergency_defender_preserved", {', source)
        self.assertIn('return true, "finishing_defender_preserved";', source)
        self.assertLess(
            source.index('if immediateThreat and finishingDefender then'),
            source.index('civvisBuild[cityId] = resolved;'),
            "the finishing defender must be preserved before the next-build memo can replace it",
        )

        play = (Path(__file__).resolve().parent / "civ6_play.py").read_text()
        self.assertIn('"EmergencyWallRadius": args.emergency_wall_radius,', play)
        self.assertIn('"--emergency-wall-radius", type=int, default=3', play)

    def test_a_refused_promotion_names_what_the_engine_offered(self) -> None:
        """The engine's answer was computed and then discarded.

        `CanStartCommand` already returns `can` and the `PROMOTIONS` list three
        lines above the refusal, and the event recorded neither — 56 refusals
        across the eight live runs of 2026-08-11, each carrying only the name
        that failed. Nothing offered means the unit cannot promote at all and
        CIVVIS should not have asked; others offered means it can, just not into
        the tree named, which is a targeting bug. Opposite fixes, one blank line.

        Names rather than indices, because a bare index in the ledger is the
        exact defect the district refusal had to be repaired for.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        self.assertIn("can_promote = okCan and can or false,", source)
        self.assertIn("offered_promotions = offeredNames,", source)
        self.assertIn("return GameInfo.UnitPromotions[index];", source)
        self.assertIn("(row ~= nil and row.UnitPromotionType) or tostring(index)", source)
        self.assertNotIn(
            'emit("promotion_refused", {\n'
            "\t\t\t\t\tturn = turn, unit = subject, promotion = promotionName,\n"
            "\t\t\t\t});",
            source,
            "the blank refusal must not come back",
        )

    def test_a_refused_placement_names_the_plots_the_engine_offered(self) -> None:
        """`offered` proves a placement is possible here and never says WHERE.

        `productionPlot` asks the engine for every plot it would accept, reads x
        and y off each, and kept only the count. So CIVVIS goes on naming the
        plot the engine refuses: run `civvis-20260811T212652Z` recorded 56
        `build_no_plot` events in 232 turns and **55 were one pair** — a single
        Commercial Hub in one city, with plots offered every time.

        #1571 bounds how often that repeats. Only the coordinates can end it.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        self.assertIn("offeredPlots[#offeredPlots + 1] = { x = px, y = py };", source)
        self.assertIn("offered_plots = offeredPlots,", source)

        # ⚠ Capped: this rides in an event on every refusal and a large city can
        # offer many plots.
        self.assertIn("if #offeredPlots < (cfg.OfferedPlotsReported or 8) then", source)

        # Both placement callers must carry the engine's authoritative candidates.
        # A wonder used to discard the third return, leaving a valid host site
        # visible only as a count and suppressing its Great Engineer path.
        self.assertIn("local where, offered, offeredPlots = productionPlot(city,", source)
        self.assertIn("where, offered, offeredPlots = productionPlot(city,", source)
        wonder = source[source.index('building = row.Type or tostring(row.Hash),') :][:1200]
        self.assertIn("offered_plots = offeredPlots,", wonder)

    def test_a_refused_build_records_the_turn_it_happened(self) -> None:
        """⚠⚠⚠ Without a turn, every filter on this event is a no-op.

        `buildParams` is a top-level function taking no turn, so `build_no_plot`
        never carried one — and two readers silently depended on it.
        `refused_no_plot_through`'s replay bound (`event.turn > limit`) read the
        missing field as 0 and excluded nothing, and #1571's staleness window
        read it as 0 too, making every refusal look ancient and blocking
        NOTHING.

        Measured on `civvis-20260811T230324Z`, the first run carrying #1571: 40
        `build_no_plot` events in 131 turns, **zero** with a turn, and one Campus
        asked for forty times — the exact loop the TTL was meant to bound.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        # Both emits — a wonder and a district come through the same event under
        # different keys, and only one of them having a turn is the same defect.
        district = source[source.index('district = row.Type or tostring(row.Hash),'):][:1400]
        self.assertIn("Game.GetCurrentGameTurn(); end, -1),", district)
        wonder = source[source.index('building = row.Type or tostring(row.Hash),'):][:600]
        self.assertIn("Game.GetCurrentGameTurn(); end, -1),", wonder)

    def test_military_emergency_popup_uses_firaxis_pass_path(self) -> None:
        modinfo = (install.MOD_SOURCE / "CivvisControl.modinfo").read_text()
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()

        self.assertIn(
            '<LuaContext>WorldCongressPopup</LuaContext>',
            modinfo,
        )
        self.assertIn('NAME == "WorldCongressPopup"', closer)
        self.assertIn('type(OnPass) == "function"', closer)
        self.assertIn("OnPass();", closer)

    def test_wonder_completion_popup_is_registered_and_uses_firaxis_close(self) -> None:
        """A completed wonder must release its exclusive popup lock unattended."""
        modinfo = (install.MOD_SOURCE / "CivvisControl.modinfo").read_text()
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()

        self.assertIn(
            '<LuaContext>WonderBuiltPopup</LuaContext>',
            modinfo,
        )
        wonder = closer.split('if NAME == "WonderBuiltPopup"', 1)[1].split(
            "return false;", 1
        )[0]
        # WonderBuiltPopup.lua's own OnClose drains queued wonders and unlocks
        # the exclusive popup manager; Close is the defensive fallback if a
        # future game build omits that wrapper.
        self.assertIn('type(OnClose) == "function"', wonder)
        self.assertIn("OnClose();", wonder)
        self.assertIn('type(Close) == "function"', wonder)
        self.assertIn("Close();", wonder)

    def test_wonder_completion_wins_and_chains_known_ui_replacement(self) -> None:
        """A later UI mod must not replace the wonder closer or its audio."""
        modinfo = (install.MOD_SOURCE / "CivvisControl.modinfo").read_text()
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()

        self.assertIn(
            '805cc499-c534-4e0a-bdce-32fb3c53ba38',
            modinfo,
        )
        action = modinfo.split(
            '<ReplaceUIScript id="CivvisControlAutoCloseWonderBuilt">', 1
        )[1].split("</ReplaceUIScript>", 1)[0]
        self.assertIn("<LoadOrder>100000</LoadOrder>", action)
        self.assertIn(
            'WonderBuiltPopup = "Suk_WonderBuiltPopup"',
            closer,
        )

        # The known replacement includes the Firaxis script and must be loaded
        # before haveScreen() checks for OnClose/Close. Keep this assertion
        # textual because Lua's UI globals do not exist in the Python suite.
        chain_at = closer.index('WonderBuiltPopup = "Suk_WonderBuiltPopup"')
        include_at = closer.index("if CHAINED[NAME] then")
        self.assertLess(chain_at, include_at)

    def test_wonder_completion_waits_for_animation_before_minimizing(self) -> None:
        """A short generic clock must not cut off the stock wonder reveal."""
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()

        self.assertIn("local WONDER_MIN_SECONDS = 1.0;", closer)
        self.assertIn(
            "local WONDER_ANIMATION_TIMEOUT_SECONDS = 8.0;",
            closer,
        )
        self.assertIn(
            'if NAME == "WonderBuiltPopup" then\n\tSECONDS = math.max(SECONDS, WONDER_MIN_SECONDS);\nend',
            closer,
        )
        for control in ("HeaderAlpha", "HeaderSlide", "QuoteAlpha", "QuoteSlide"):
            self.assertIn(f"Controls.{control}", closer)
        self.assertIn("animation:IsStopped()", closer)
        self.assertIn('report("autoclose_wait_animation"', closer)
        self.assertIn('"animation_ready"', closer)
        self.assertIn('"animation_timeout"', closer)
        self.assertIn("if shown < WONDER_ANIMATION_TIMEOUT_SECONDS then", closer)
        self.assertIn("remaining = DIALOGUE_READY_RETRY_SECONDS;", closer)

        wait_at = closer.index("local wonderAnimationReadyAtClose = true;")
        close_at = closer.index("local upFor = shown;", wait_at)
        self.assertLess(wait_at, close_at)

    def test_between_turns_congress_uses_the_complete_firaxis_hide_path(self) -> None:
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()
        congress = closer.split('NAME == "WorldCongressBetweenTurns"', 1)[1].split(
            "return true;", 1
        )[0]

        self.assertIn('type(OnHide) == "function"', congress)
        self.assertIn("OnHide();", congress)
        self.assertNotIn("ReleaseEventLock();", congress)

    def test_diplomacy_popups_escalate_on_the_quick_clock(self) -> None:
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()

        self.assertIn('NAME == "DiplomacyActionView"', closer)
        self.assertIn('NAME == "DiplomacyDealView"', closer)
        self.assertIn("tonumber(cfg.DialogueSeconds) or 0.25", closer)
        self.assertIn("DESKTOP_AFTER = 4", closer)
        self.assertIn('report("autoclose_desktop"', closer)

    def test_spy_popups_clear_through_their_shipped_paths(self) -> None:
        """Spy overlays must disappear without leaving an end-turn decision behind."""
        modinfo = (install.MOD_SOURCE / "CivvisControl.modinfo").read_text()
        closer = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()

        for context in ("EspionagePopup", "EspionageEscape"):
            self.assertIn(f"<LuaContext>{context}</LuaContext>", modinfo)

        briefing = closer.split('if NAME == "EspionagePopup"', 1)[1].split(
            "return true;", 1
        )[0]
        self.assertIn('type(OnCancel) == "function"', briefing)
        self.assertIn("OnCancel();", briefing)

        escape = closer.split('if NAME == "EspionageEscape"', 1)[1].split(
            "return true;", 1
        )[0]
        self.assertIn('type(OnButton4) == "function"', escape)
        self.assertIn("OnButton4();", escape)
        self.assertIn('or type(OnButton4) == "function"', closer)

        # Treat these full-screen choices like dialogue: on a normal run they
        # close within 0.25s, while the ladder's 0.05s announcement setting
        # remains even faster.
        quick_clock = closer.split('if NAME == "DiplomacyActionView"', 1)[1].split(
            "end\nif SECONDS < 0", 1
        )[0]
        self.assertIn('NAME == "EspionagePopup"', quick_clock)
        self.assertIn('NAME == "EspionageEscape"', quick_clock)

    def test_governors_export_exact_state_and_use_stock_operation_indices(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("-- Governor Titles", 1)[1].split('emit("state"', 1)[0]
        actuator = source.split('if kind == "governor_appoint"', 1)[1].split(
            'if kind == "war"', 1
        )[0]

        self.assertIn("governors:GetGovernorList()", exporter)
        self.assertIn("governors:GetGovernorPoints()", exporter)
        self.assertIn("governors:GetGovernorPointsSpent()", exporter)
        self.assertIn("governor:GetAssignedCity()", exporter)
        self.assertIn("governor:HasPromotion(promotion.Hash)", exporter)
        self.assertIn("governor:IsEstablished()", exporter)
        self.assertIn("governor:GetNeutralizedTurns()", exporter)
        self.assertIn(
            "params[PlayerOperations.PARAM_GOVERNOR_TYPE] = governor.Index", actuator
        )
        self.assertIn(
            "params[PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE] = promotion.Index",
            actuator,
        )
        self.assertIn("params[PlayerOperations.PARAM_PLAYER_ONE] = cityOwner", source)
        self.assertIn("GovernorAppointed = onGovernorAppointed", source)
        self.assertNotIn("params[govParam] = row.Hash", source)
        self.assertIn("params[govParam] = row.Index", source)
        self.assertIn("params[playerParam] = pid", source)

    def test_district_export_distinguishes_foundations_from_completed_districts(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("-- ★★★★★ AND WHAT IT HAS DISTRICTED", 1)[1].split(
            "-- ★★★★★ WHOSE RELIGION", 1
        )[0]

        self.assertIn("district:GetX()", exporter)
        self.assertIn("district:GetY()", exporter)
        self.assertIn("district:IsComplete()", exporter)
        self.assertIn("complete = districtComplete[", exporter)

    def test_district_pillage_uses_firaxis_city_district_api(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn("cityDistricts:IsPillaged(dtype, plotIndex)", source)
        self.assertNotIn("plot:IsDistrictPillaged()", source)

    def test_production_resumes_placed_district_without_replacing_its_plot(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        builder = source.split("local function buildParams", 1)[1].split(
            "-- What each city was last told to build", 1
        )[0]

        self.assertIn("city:GetBuildQueue():HasBeenPlaced(row.Hash)", builder)
        self.assertIn("if not alreadyPlaced then", builder)
        self.assertIn("if not alreadyPlaced and where == nil then", builder)

    def test_controller_uses_real_project_and_great_person_command_ids(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertNotIn('"PROJECT_CAMPUS_RESEARCH_GRANT"', source)
        self.assertIn('"PROJECT_ENHANCE_DISTRICT_CAMPUS"', source)
        self.assertIn('"UNITCOMMAND_ACTIVATE_GREAT_PERSON"', source)
        self.assertIn("GetActivationHighlightPlots()", source)

    def test_controller_retires_a_unit_civvis_asks_it_to_delete(self) -> None:
        # The bridge retires the founded zero-charge Prophet (a ghost that
        # otherwise blocks its hex for the rest of the game) with a `DELETE`
        # verb; the mod must gate it through CanStartCommand and name a refusal.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn('if verb == "DELETE" then', source)
        delete_block = source.split('if verb == "DELETE" then', 1)[1].split('if verb == "ENTER_FORMATION" then', 1)[0]
        # Through the shipped UnitPanel's own gate (`loose`), not the strict form:
        # that form refused every DELETE ever asked (495 across three runs, zero
        # retirements) and the founded Prophet stood on its hex all game.
        self.assertIn(
            'local deleted, why = commandUnit(unit, CMD["UNITCOMMAND_DELETE"], true)',
            delete_block,
        )
        self.assertNotIn('commandUnit(unit, CMD["UNITCOMMAND_DELETE"])\n', delete_block)
        self.assertIn('emit("delete_refused"', delete_block)
        self.assertIn("why = why", delete_block)
        self.assertIn('action = "retired_by_civvis"', delete_block)
        helper = source.split("local function commandUnit(unit, hash, loose)", 1)[1].split("\nend\n", 1)[0]
        # The loose form is the one UnitPanel.lua gates its Delete button on;
        # then RequestCommand outright, exactly as OnDeleteUnit does. The strict
        # form stays the default for every other command.
        self.assertIn("UnitManager.CanStartCommand(unit, hash, true)", helper)
        self.assertIn("UnitManager.CanStartCommand(unit, hash, false, true)", helper)
        self.assertIn("UnitManager.RequestCommand(unit, hash)", helper)
        # And a refusal names its reason through the results table.
        self.assertIn("UnitCommandResults.FAILURE_REASONS", helper)
        # The Prophet branch of the Great Person routine retires through the same gate.
        self.assertIn('and commandUnit(unit, CMD["UNITCOMMAND_DELETE"], true) then', source)

    def test_civvis_envoy_orders_place_one_token_through_a_fresh_handle(self) -> None:
        # The bridge translates CIVVIS's SendEnvoy into an `envoy` order once
        # `envoys_free` is mirrored; the mod places exactly one token per order
        # through the shipped CityStates.lua accessors, reads every handle fresh
        # inside the order, and never writes the prompt-clearing flag — the
        # stale-handle write is the one defect the old lane's crash was pinned on.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn('if kind == "envoy" then', source)
        handler = source.split('if kind == "envoy" then', 1)[1].split("\n\t-- ★★★★★ CIVVIS'S OWN POLICY", 1)[0]
        self.assertIn("PlayerOperations.GIVE_INFLUENCE_TOKEN", handler)
        self.assertIn("PlayerOperations.PARAM_PLAYER_ONE", handler)
        self.assertIn("player:GetInfluence()", handler)
        self.assertIn("influence:GetTokensToGive()", handler)
        self.assertIn("influence:CanGiveInfluence()", handler)
        self.assertIn("influence:CanGiveTokensToPlayer(subject)", handler)
        self.assertIn("UI.RequestPlayerOperation(pid, giveOp, params)", handler)
        # One token per order: no loop over the held count in this arm.
        self.assertNotIn("for _ = 1,", handler)
        # The decision is CIVVIS's — this arm neither chooses a target nor clears the prompt.
        self.assertNotIn("SetGivingTokensConsidered", handler)
        self.assertNotIn("envoySpendOrder", handler)
        self.assertNotIn("cfg.EnvoyEnabled", handler)
        # Issuing-side telemetry only: no same-frame "after" count, which read equal
        # to `held` on every live placement while the tokens were landing; the
        # receiving side is the next export's envoys_free / minors[].envoys.
        self.assertIn('emit("envoy"', handler)
        self.assertIn('source = "civvis"', handler)
        self.assertNotIn("after =", handler)
        self.assertNotIn("player:GetInfluence():GetTokensToGive();", handler)
        # And a refusal names its reason.
        for reason in ("envoy_target_unmapped", "envoy_no_operation", "envoy_none_held", "envoy_cannot_give", "envoy_refused_"):
            self.assertIn(reason, handler)

    def test_civvis_levy_orders_pay_the_host_quote_through_a_fresh_handle(self) -> None:
        # LevyMilitary crosses as a `levy` order once the seat holds a
        # suzerainty; the mod gates on CanLevyMilitary and Firaxis's own cost
        # against the treasury, one LEVY_MILITARY request per order.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn('if kind == "levy" then', source)
        handler = source.split('if kind == "levy" then', 1)[1].split("\n\t-- ★★★★★ CIVVIS'S OWN POLICY", 1)[0]
        self.assertIn("PlayerOperations.LEVY_MILITARY", handler)
        self.assertIn("PlayerOperations.PARAM_PLAYER_ONE", handler)
        self.assertIn("influence:CanLevyMilitary(subject)", handler)
        self.assertIn("influence:GetLevyMilitaryCost(subject)", handler)
        self.assertIn("player:GetTreasury():GetGoldBalance()", handler)
        self.assertIn("UI.RequestPlayerOperation(pid, levyOp, params)", handler)
        self.assertNotIn("SetGivingTokensConsidered", handler)
        self.assertNotIn("cfg.EnvoyLevy", handler)
        self.assertIn('emit("levy"', handler)
        for reason in ("levy_target_unmapped", "levy_no_operation", "levy_refused_", "levy_unaffordable"):
            self.assertIn(reason, handler)

    def test_envoy_spend_records_a_next_frame_host_reconciliation(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        begin = source.split("local function beginTurn", 1)[1].split(
            "if cfg.EnvoyEnabled then", 1
        )[0]
        self.assertIn("local pending = envoyTally.pending", begin)
        self.assertIn("fresh:GetTokensToGive()", begin)
        self.assertIn('emit("envoy_reconcile"', begin)
        self.assertIn("minimum_after", begin)
        self.assertIn("envoyTally.pending = nil", begin)
        spend = source.split('emit("envoy", {', 1)[1].split("if levied ~= nil", 1)[0]
        self.assertIn("envoyTally.pending = { turn = turn", spend)

    def test_trade_route_exports_use_firaxis_own_origin_yield_sum(self) -> None:
        """Fogged destination districts must not make the mirror guess a route."""
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("local tradeRoutes = {};", 1)[1].split("local queue = try(function()", 1)[0]
        for api in (
            "CalculateOriginYieldsFromPotentialRoute",
            "CalculateOriginYieldsFromPath",
            "CalculateOriginYieldsFromModifiers",
        ):
            self.assertIn(api, exporter)
        # TradeSupport.lua indexes the returned one-based arrays by
        # `yieldIndex`, but `GetInternationalYieldModifier` takes its zero-based
        # YieldTypes index. Keep both sides of that contract explicit.
        self.assertIn("fromRoute[index + 1]", exporter)
        self.assertIn("GetInternationalYieldModifier(index)", exporter)
        self.assertIn("yields = routeYields", exporter)

    def test_a_stuck_screens_retries_walk_the_whole_exit_ladder_again(self) -> None:
        # Two leading games died on the 900 s watchdog under a late first-contact
        # leader scene: after twenty tries every 30 s retry called only the tail
        # rungs, because the failure count was passed as the rung number. The
        # rung cycles; the count still drives the desktop/stuck thresholds; and a
        # diplomacy view reports the mode/session/fade/popup it is stuck in.
        shim = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()
        self.assertIn("local rung = ((closes - 1) % GIVE_UP_AFTER) + 1;", shim)
        self.assertIn("pcall(function() ended = endScreen(rung); end);", shim)
        self.assertNotIn("endScreen(closes)", shim)
        # ⚠⚠⚠ THE DESKTOP ASK MUST REPEAT WHILE THE SCREEN IS STILL UP.
        # It used to latch on a boolean cleared only when the screen went away
        # — which is exactly what a screen needing help does not do. A leader
        # conversation cannot be dismissed blind (Escape does nothing on it, and
        # Escape with nothing to close opens the pause menu), so the desktop side
        # must see the screen; that capture fails transiently, and one such
        # failure ended the run because the ask never came again. Measured
        # 2026-08-29, run civvis-20260829T093602Z: one `autoclose_desktop` for
        # DiplomacyDealView at 4 attempts, "popup capture unavailable" back, and
        # the game sat on a leader screen until the watchdog killed it at t40.
        self.assertIn(
            "if closes >= DESKTOP_AFTER and closes - desktopReportedAt >= DESKTOP_AFTER then",
            shim,
        )
        self.assertIn("desktopReportedAt = closes;", shim)
        # The reset arms it again for the next appearance, and nothing else may
        # make it permanent: the declaration says "Giving up is a BACK-OFF,
        # never a stop".
        self.assertEqual(shim.count("desktopReportedAt = -1;"), 3)
        self.assertNotIn("desktopReported =", shim.replace("desktopReportedAt =", ""))
        self.assertIn("if closes >= GIVE_UP_AFTER then", shim)
        for field in ('"rung":%d', '"mode":%d', '"session":%d', '"fading":%s', '"popup":%s'):
            self.assertIn(field, shim)
        self.assertIn("ms_currentViewMode", shim)
        self.assertIn("ms_ActiveSessionID", shim)
        self.assertIn("Controls.BlackFadeAnim:IsStopped()", shim)

    def test_the_mod_answers_a_retire_row_with_the_shipped_action(self) -> None:
        # ⚠⚠ A killed game is UNFINISHED, not lost: Civilization VI files no
        # defeat, so `tools/civ6_ladder.py` records nothing and an attempt the
        # operator called on the score rule reads exactly like a crash.
        #
        # The retire itself is one call. The stock
        # `Base/Assets/UI/Menus/InGameTopOptionsMenu.lua` `OnReallyRetire` does
        # `UI.RequestAction(ActionTypes.ACTION_RETIRE)` and nothing else that
        # matters, so there is no pause menu to open and no confirm dialog to
        # find and click blind.
        shim = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn("ActionTypes.ACTION_RETIRE", shim)
        self.assertIn("kind = 'retire'", shim)
        # Matched on the RUN alone. The ordinary fetch filters on the exact turn
        # and frame it is reading, so a request made at an unscheduled moment
        # would sit unread until a frame happened to match.
        self.assertNotIn("AND turn = %d\" ..\n\t\t\t\"AND kind = 'retire'", shim)
        # Asked once: `RequestAction` is asynchronous and the tick keeps running.
        self.assertIn("CivvisBoard.retireAsked = true;", shim)
        self.assertIn("if not CivvisBoard.retireAsked", shim)
        # Latched on an existing table rather than a new file-scope local: this
        # main chunk sits at Civ 6's 200-register ceiling.
        self.assertNotIn("local retireAsked", shim)

    def test_the_congress_outcome_is_reported_once_per_session(self) -> None:
        # Seven diplomatic losses in a day and no record of what each session
        # resolved: the mod now emits `wc_outcome` from the shipped review data
        # (GetReview) once per change of content, with every voter, the
        # emergency results and every civ's DVP; read-only, nothing here votes.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        block = source.split("-- ★★★★ WHAT THE LAST WORLD CONGRESS SESSION DECIDED", 1)[1].split("-- ★★★★ AN EMPIRE WITH NO CITIES IS DEFEATED", 1)[0]
        self.assertIn("wc:GetReview(pid)", block)
        self.assertIn("review.Resolutions", block)
        self.assertIn("review.Discussions", block)
        self.assertIn("r.PlayerSelections", block)
        self.assertIn("prop.PlayerVotes", block)
        self.assertIn("v.PlayerType", block)
        self.assertIn("envoyTally.wc_review_signature", block)
        self.assertIn('emit("wc_outcome"', block)
        self.assertIn("GetDiplomaticVictoryPoints()", block)
        self.assertNotIn("RequestPlayerOperation", block)

    def test_the_congress_ballot_is_cast_from_the_popup_moment(self) -> None:
        # The blocker-time votes never registered (favor never fell, wc_outcome
        # read the core's default `option 1, votes 1` on every resolution); the
        # shim asks the agent to vote from inside the popup right before its
        # OnAccept, through a LuaEvent, and the agent answers with the same voter.
        shim = (install.MOD_SOURCE / "CivvisControlAutoClose.lua").read_text()
        # Both WorldCongressPopup rungs raise it, and the OnPass rung — the one
        # that actually runs for the session popup, since the shipped script
        # defines OnPass as well as OnAccept — raises it before it closes.
        for closer in ("OnPass", "OnAccept"):
            rung = shim.split(f'if NAME == "WorldCongressPopup" and type({closer}) == "function" then', 1)[1].split("return true;", 1)[0]
            self.assertIn("LuaEvents.CivvisCongressBallot()", rung, closer)
            self.assertLess(rung.index("LuaEvents.CivvisCongressBallot()"), rung.index(f"{closer}();"),
                            f"the ballot goes in before {closer}")
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        # One shared ballot, two triggers: the core's own stage-1 event and the
        # shim's popup event; once per turn, only latched when something was cast.
        self.assertIn("local function castBallot(trigger)", source)
        ballot = source.split("local function castBallot(trigger)", 1)[1].split("if not envoyTally.ballot_hooked then", 1)[0]
        self.assertIn("voteWorldCongress(ballotPid)", ballot)
        self.assertIn("envoyTally.ballot_turn", ballot)
        self.assertIn("favor_before = before", ballot)
        self.assertIn('LuaEvents.CivvisCongressBallot.Add(function() castBallot("popup"); end);', source)
        self.assertIn('Events.WorldCongressStage1.Add(function(playerID)', source)
        self.assertIn('castBallot("stage1")', source)
        self.assertIn('emit("wc_ballot_hooked"', source)
        # The blocker path defers to the triggers and falls back a cycle later.
        blocker = source.split('if name == "ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION"', 1)[1].split("local parked = UNIT_BLOCKERS[name]", 1)[0]
        self.assertIn("if envoyTally.ballot_turn == turn then", blocker)
        self.assertIn("elseif seen.forfeits >= 2 then", blocker)
        self.assertIn('source = "blocker"', blocker)

    def test_the_state_export_carries_the_emergencies(self) -> None:
        # The leader's +8 in one session came from World Fair / World Games
        # resolving — competitions the seat never entered because nothing said
        # they were running. The state now carries Firaxis's own crisis table.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("local function exportState", 1)[1]
        block = exporter.split("emergencies = try(function()", 1)[1].split("end, nil),", 1)[0]
        self.assertIn("Game.GetEmergencyManager():GetEmergencyInfoTable(pid)", block)
        for field in ("crisis.EmergencyType", "crisis.TargetID", "crisis.TurnsLeft",
                      "crisis.HasBegun", "crisis.bSuccess", "crisis.MemberIDs",
                      "crisis.ScoresTables", "crisis.MemberTiers", "crisis.GoalsTable",
                      "crisis.TargetGoalsTable", "crisis.ScoreSourcesTable"):
            self.assertIn(field, block)
        for key in ("turns_left =", "members = members", "scores = scores", "ours = {",
                    "goals = goals", "score_sources = sources"):
            self.assertIn(key, block)
        self.assertNotIn("RequestPlayerOperation", block)

    def test_the_rival_export_carries_victory_progress(self) -> None:
        # Five of the twelve runs the seat was leading on 2026-08-16/17 ended
        # at t229-245 on a rival's culture, technology or diplomatic victory
        # the mirror could not see coming. Every accessor is one the shipped
        # World Rankings screen calls on OTHER players.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        block = source.split("science = try(function() return other:GetTechs()", 1)[1]
        block = block.split("public_stats = otherPublicStats", 1)[0]
        self.assertIn("science_projects = try(function()", block)
        self.assertIn("stats:GetNumProjectsAdvanced(project.Index)", block)
        for project in ("PROJECT_LAUNCH_EARTH_SATELLITE",
                        "PROJECT_LAUNCH_MOON_LANDING",
                        "PROJECT_LAUNCH_MARS_BASE",
                        "PROJECT_LAUNCH_EXOPLANET_EXPEDITION"):
            self.assertIn(project, block)
        # Manhattan and Ivy are strategic programs, not public victory rows.
        self.assertNotIn("PROJECT_MANHATTAN_PROJECT", block)
        self.assertNotIn("PROJECT_OPERATION_IVY", block)
        self.assertIn("foreign_tourists = try(function()", block)
        self.assertIn("other:GetCulture():GetTouristsTo();", block)
        self.assertIn("domestic_tourists = try(function()", block)
        self.assertIn("other:GetCulture():GetStaycationers();", block)

    def test_unit_captured_is_registered_and_names_the_captor_for_our_units_only(self) -> None:
        """A settler taken by the barbarians must be told apart from one founding a city.

        `UnitRemovedFromMap` fires for both, and `unit_lost` reads the same for
        both — 24 settlers went to the barbarians in ten runs on 2026-08-28 and
        no ledger column could tell. The game's own word is
        `Events.UnitCaptured(currentUnitOwner, unit, owningPlayer,
        capturingPlayer)` (`Base/Assets/UI/Popups/UnitCaptured.lua:8`,
        registered at `:49`, filtered on the local owner at `:11`). Assert the
        registration and the handler's shape, not a sentence claiming them.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        registrations = source.split("function Initialize()", 1)[1].split(
            "pcall(function() Events[name].Add(handler); end);", 1
        )[0]
        self.assertIn("UnitRemovedFromMap = CivvisLedger.onUnitRemoved,", registrations)
        self.assertIn("UnitCaptured = CivvisLedger.onUnitCaptured,", registrations)

        handler = source.split("CivvisLedger.onUnitCaptured = function(", 1)[1].split(
            "\nend;", 1
        )[0]
        self.assertTrue(handler.startswith(
            "currentUnitOwner, unitId, owningPlayer, capturingPlayer)"))
        # Ours only, the way the shipped popup filters (`UnitCaptured.lua:11`).
        self.assertIn("if tonumber(currentUnitOwner) ~= pid then return; end", handler)
        self.assertIn('emit("unit_captured", {', handler)
        for key in ("turn =", "unit = tonumber(unitId)",
                    "unit_kind = CivvisLedger.kinds[tostring(unitId)]",
                    "owner = tonumber(currentUnitOwner)", "captor = captor",
                    "captor_is_barbarian = barbarian"):
            self.assertIn(key, handler)
        self.assertIn("Players[captor]:IsBarbarian()", handler)
        # `unit_lost` stays: the capture event is a second witness, not a replacement.
        self.assertIn('emit("unit_lost", {', source)

    def test_settler_combat_hold_checks_the_hostile_units_full_capture_reach(self) -> None:
        """A combat unit's capture reach is wider than destination adjacency."""
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        hold = source.split(
            "CivvisBoard.holdVisibleBarbarianCombatCaptureLegs = function", 1
        )[1].split("CivvisBoard.holdVisibleScoutCaptureLegs", 1)[0]

        self.assertIn("unit = unit,", hold)
        self.assertIn("local function threatReaches(threat, x, y)", hold)
        self.assertIn(
            "UnitManager.GetMoveToPathEx(threat.unit, destination)", hold
        )
        self.assertIn("local definition = GameInfo.Units[threat.unit:GetUnitType()]", hold)
        self.assertIn("definition.BaseMoves", hold)
        self.assertIn("if distance >= 0 and distance <= baseMoves then", hold)
        self.assertIn("hostile_reach = reachKind", hold)
        self.assertNotIn("distance == 1", hold)

    def test_the_seat_event_names_the_hosts_victory_table(self) -> None:
        # The TeamVictory event reports a raw integer and docs/CIV6_LADDER.md
        # rightly refuses guessed names for it. The seat event now carries the
        # host's own Victories table so every run's record is self-describing.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        block = source.split("victory_types = try(function()", 1)[1]
        block = block.split("end, nil),", 1)[0]
        self.assertIn("for row in GameInfo.Victories() do", block)
        self.assertIn("index = row.Index", block)
        self.assertIn("type = row.VictoryType", block)

    def test_the_penalty_resolutions_name_the_victory_leader(self) -> None:
        # Every player-targeted resolution except Diplomatic Victory used to
        # select THIS SEAT with option 1 — the option that BUFFS its target —
        # so the three ballots carrying a real penalty were spent on a small
        # bonus for us. 232 such ballots across 39 live games.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        block = source.split('elseif r.TargetType == "PlayerType" then', 1)[1]
        block = block.split("local params = {}", 1)[0]
        # The three the host's own Expansion2_Congress.xml gives a penalty on
        # effect 2. Deliberately an allowlist: option 2 is not the penalty on
        # every player-targeted resolution, and a blanket rule would guess.
        for res in ("WC_RES_TRADE_TREATY", "WC_RES_BORDER_CONTROL",
                    "WC_RES_MIGRATION_TREATY"):
            self.assertIn(res, block)
        self.assertIn("tonumber(t) == threat", block)
        self.assertIn("option = 2", block)
        # The old self-buff must remain the behaviour below the bar.
        self.assertIn("tonumber(t) == pid", block)
        self.assertIn("cfg.CounterResolutionBar", block)
        self.assertIn("cfg.CounterResolutions ~= false", block)

    def test_the_victory_threat_selector_is_exported_for_its_regression(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        # A bare global, like its sibling: the UI sandbox has no `_G` and the
        # chunk is near Civ 6's 200-register file-scope local ceiling.
        self.assertIn("CivvisSelectVictoryThreat = function(candidates)", source)
        self.assertNotIn("local CivvisSelectVictoryThreat", source)
        # Both lanes that actually end our games reach the selector.
        self.assertIn("GetTouristsTo()", source)
        self.assertIn("GetStaycationers()", source)

    def test_the_spy_missions_reach_the_host(self) -> None:
        # The engine models twelve missions and the AI aims them at the denial
        # target; none could be sent, and `Game::spies` was empty besides.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        for op in ("UNITOPERATION_SPY_TRAVEL_NEW_CITY",
                   "UNITOPERATION_SPY_GREAT_WORK_HEIST",
                   "UNITOPERATION_SPY_DISRUPT_ROCKETRY",
                   "UNITOPERATION_SPY_STEAL_TECH_BOOST"):
            self.assertIn(op, source)
        block = source.split('if verb:sub(1, 4) == "SPY_" then', 1)[1]
        block = block.split("Anything else is a named operation", 1)[0]
        # Aimed at a plot, the way Firaxis' own EspionagePopup aims it.
        self.assertIn("UnitOperationTypes.PARAM_X", block)
        self.assertIn("UnitOperationTypes.PARAM_Y", block)
        self.assertIn("operate(unit, hash, params)", block)
        # A mission with no destination is named, never sent empty-handed.
        self.assertIn('"no_spy_target:"', block)

    def test_the_export_carries_spy_capacity(self) -> None:
        # Without it the mirror must block Spy production unconditionally,
        # which is why the seat has never held one.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn("spy_capacity = try(function()", source)
        self.assertIn("player:GetDiplomacy():GetSpyCapacity()", source)
    def test_a_soldier_can_condemn_an_enemy_missionary(self) -> None:
        # Religious units are excluded from ordinary combat by design, so
        # Condemn Heretic is the only order that touches one. CIVVIS decided it
        # all along and the bridge dropped it.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn('"UNITCOMMAND_CONDEMN_HERETIC"', source)
        block = source.split('if verb == "CONDEMN_HERETIC" then', 1)[1]
        block = block.split("Anything else is a named operation", 1)[0]
        # A COMMAND, not an operation — the two use different request APIs.
        self.assertIn('CMD["UNITCOMMAND_CONDEMN_HERETIC"]', block)
        self.assertIn("commandUnit(unit, hash, true)", block)
        self.assertNotIn("operate(", block)
        # An unresolved hash is named, never silently skipped.
        self.assertIn('"unknown_cmd_"', block)

    def test_controller_votes_in_the_world_congress_before_forfeiting_the_session(self) -> None:
        # The session was a soft blocker the ladder dismissed: nineteen forfeits
        # in one game, no vote in 242 turns, and a rival's diplomatic victory
        # ended it. The votes go through the shipped popup's own operations.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn("local function voteWorldCongress(pid)", source)
        voter = source.split("local function voteWorldCongress(pid)", 1)[1].split("local player, pid = localPlayer();", 1)[0]
        self.assertIn("wc:GetResolutions(pid)", voter)
        self.assertIn("wc:GetVotesandFavorCost(pid)", voter)
        self.assertIn("GetDiplomaticVictoryPoints()", voter)
        # Against the leader on the diplomatic-victory resolution, with what
        # the favor affords on BOTH cost tables the host might charge: every
        # ask that saturated the reported Online table was refused whole
        # (17/17 across four runs), so the walks live in
        # `CivvisCongressVoteBudget` and the ask takes the smaller.
        self.assertIn('if rtype == "WC_RES_DIPLOVICTORY" then', voter)
        self.assertIn("option = 2;", voter)
        self.assertIn("CivvisCongressVoteBudget(favor, costs, maxVotes)", voter)
        self.assertIn("CivvisCongressVoteBudget = function(favor, costs, maxVotes)", source)
        budget = source.split("CivvisCongressVoteBudget = function(favor, costs, maxVotes)", 1)[1].split("\nend", 1)[0]
        self.assertIn("tonumber(costs[host]) <= bank", budget)
        self.assertIn("5 * (standard + 1) * standard <= bank", budget)
        # One operation carries the whole count: #2045's repeated single-vote
        # experiment came back `votes_sent 20, recorded 1` on run
        # civvis-20260819T004405Z -- the operation sets the ballot, it does
        # not accumulate -- so the repeat loop must stay gone.
        self.assertIn("PlayerOperations.WORLD_CONGRESS_RESOLUTION_VOTE", voter)
        self.assertNotIn("PlayerOperations.PARAM_WORLD_CONGRESS_VOTES] = 1", voter)
        self.assertIn("PlayerOperations.WORLD_CONGRESS_SUBMIT_TURN", voter)
        # Wired into the soft-blocker forfeit path, once per turn, before the dismissal.
        forfeit = source.split('if name == "ENDTURN_BLOCKING_WORLD_CONGRESS_SESSION"', 1)[1].split("local dropped = dismissBlocker(pid, blocker);", 1)[0]
        self.assertIn("seen.voted_turn ~= turn", forfeit)
        self.assertIn("voteWorldCongress(pid)", forfeit)
        self.assertIn('emit("wc_vote"', forfeit)
        # And the state now carries the points and the favor.
        self.assertIn("dvp = try(function() return player:GetStats():GetDiplomaticVictoryPoints(); end, nil)", source)

    def test_the_ballot_claims_the_diplomatic_points_when_the_bank_outvotes_every_block(self) -> None:
        """Decoded `wc_outcome` rows (T205104Z, T223457Z): option A of the
        Diplomatic Victory resolution wins every session because each rival
        votes A for itself with 6-11 votes and the target is the biggest block,
        so a B ballot against the leader changes nothing while 640-1441 Favor
        sat unspent. When the bank affords `DiploVictoryClaimVotes` (12) the
        ballot is A with every vote targeting us; below that the floor rule
        stands; the mode reaches `wc_vote`.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        voter = source.split("local function voteWorldCongress(pid)", 1)[1].split("local player, pid = localPlayer();", 1)[0]
        arm = voter.split('if rtype == "WC_RES_DIPLOVICTORY" then', 1)[1].split("elseif r.TargetType", 1)[0]
        self.assertIn("local claim = tonumber(cfg.DiploVictoryClaimVotes) or 12;", arm)
        self.assertIn("if tonumber(t) == pid then ourIdx = idx; end", arm)
        self.assertIn("if ourIdx ~= nil and budget >= claim then", arm)
        claim = arm.index("if ourIdx ~= nil and budget >= claim then")
        tail = arm[claim:]
        self.assertIn("option = 1;", tail)
        self.assertIn("selection = ourIdx;", tail)
        self.assertIn("n = budget;", tail)
        self.assertIn('mode = "claim";', tail)
        # The claim overrides the floor rule, so it comes after it and before
        # the votes are committed.
        gate = arm.index("if (tonumber(leaderPoints) or 0) >= floor then")
        commit = arm.index("votes = n;")
        self.assertLess(gate, claim)
        self.assertLess(claim, commit)
        # The bank is measured once, against both cost tables, before any gate.
        self.assertIn("CivvisCongressVoteBudget(favor, costs, maxVotes)", arm)
        budgetCall = arm.index("CivvisCongressVoteBudget(favor, costs, maxVotes)")
        self.assertLess(budgetCall, gate)
        # And the mode is reported on every ballot row.
        self.assertIn("return cast, spent, nil, leader, leaderPoints, leaderScore, mode;", voter)
        self.assertGreaterEqual(source.count("mode = mode"), 2)

    def test_favor_is_banked_until_a_congress_leader_is_within_reach(self) -> None:
        """Run civvis-20260816T123936Z spent 180/220/220/264 Favor against
        leaders on 8/11/14/15 points and still lost to a diplomatic victory at
        t239: the extra votes are on a rising cost ladder, so the bank buys the
        most at the sessions a leader can win from. Below the floor only the
        free vote is cast against the leader; from the floor the bank is spent.
        """
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        voter = source.split("local function voteWorldCongress(pid)", 1)[1].split("local player, pid = localPlayer();", 1)[0]
        arm = voter.split('if rtype == "WC_RES_DIPLOVICTORY" then', 1)[1].split("elseif r.TargetType", 1)[0]
        self.assertIn("local floor = cfg.DiploVictoryVoteFloor or 12;", arm)
        self.assertIn("if (tonumber(leaderPoints) or 0) >= floor then", arm)
        # The full bank spends inside the floor gate; the free vote (n = 1)
        # and the leader selection sit before it. Below the floor a
        # three-vote probe (`CongressVoteProbe`) keeps the purchase path
        # measured at every session -- three votes fit both cost tables from
        # the first congress bank on -- without draining what the floor
        # banks.
        gate = arm.index("if (tonumber(leaderPoints) or 0) >= floor then")
        spend = arm.index("n = budget;")
        free = arm.index("local n = 1;")
        selection = arm.index("if tonumber(t) == leader then selection = idx; end")
        probe = arm.index('elseif cfg.CongressVoteProbe ~= false and budget > 1 then')
        self.assertLess(selection, gate)
        self.assertLess(free, gate)
        self.assertLess(gate, spend)
        self.assertLess(spend, probe)
        self.assertIn("n = (budget < 3) and budget or 3;", arm)

        self.assertIn("favor = try(function() return player:GetFavor(); end, nil)", source)
        self.assertIn("dvp = try(function() return other:GetStats():GetDiplomaticVictoryPoints(); end, nil)", source)

    def test_controller_exports_the_strategic_stockpiles(self) -> None:
        # The board carried no strategic resources at all, so no resource unit
        # was ever producible on the live seat and nothing was ever obsolete.
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        self.assertIn("strategic_resources = try(function()", source)
        block = source.split("strategic_resources = try(function()", 1)[1].split("great_person_points = try(function()", 1)[0]
        self.assertIn("player:GetResources()", block)
        self.assertIn('row.ResourceClassType == "RESOURCECLASS_STRATEGIC"', block)
        self.assertIn("resources:GetResourceAmount(row.ResourceType)", block)
        # nil, not {}, when nothing is stocked (an empty table encodes as `[]`).
        self.assertIn("if not any then return nil; end", block)

    def test_controller_protects_damaged_unwalled_cities_and_retires_spent_prophets(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        self.assertIn("local function cityWarThreat", source)
        self.assertIn('GameInfo.Types["BUILDING_WALLS"]', source)
        self.assertIn('maxWallDamage <= 0', source)
        self.assertIn('emit("emergency_wall_override"', source)
        self.assertIn("GetReligionTypeCreated()", source)
        self.assertIn('CMD["UNITCOMMAND_DELETE"]', source)
        self.assertIn('action = "retired_founded_prophet"', source)
        self.assertIn("gp_retired = gpRetired", source)

    def test_controller_maps_every_civvis_project_to_its_firaxis_type(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        aliases = {
            "PROJECT_CAMPUS_RESEARCH_GRANTS": "PROJECT_ENHANCE_DISTRICT_CAMPUS",
            "PROJECT_HOLY_SITE_PRAYERS": "PROJECT_ENHANCE_DISTRICT_HOLY_SITE",
            "PROJECT_COMMERCIAL_HUB_INVESTMENT": (
                "PROJECT_ENHANCE_DISTRICT_COMMERCIAL_HUB"
            ),
            "PROJECT_HARBOR_SHIPPING": "PROJECT_ENHANCE_DISTRICT_HARBOR",
            "PROJECT_ENCAMPMENT_TRAINING": "PROJECT_ENHANCE_DISTRICT_ENCAMPMENT",
            "PROJECT_INDUSTRIAL_ZONE_LOGISTICS": (
                "PROJECT_ENHANCE_DISTRICT_INDUSTRIAL_ZONE"
            ),
            "PROJECT_THEATER_SQUARE_FESTIVAL": "PROJECT_ENHANCE_DISTRICT_THEATER",
        }
        for civvis_type, firaxis_type in aliases.items():
            self.assertIn(f'{civvis_type} = "{firaxis_type}"', source)

        produce = source.split('if kind == "produce" then', 1)[1].split(
            'if kind == "purchase" or kind == "purchase_faith" then', 1
        )[0]
        alias_at = produce.index("CIVVIS_PROJECT_TYPES[verb]")
        resolve_at = produce.index("resolveType(GameInfo.Types, verb)")
        self.assertLess(alias_at, resolve_at)

    def test_controller_maps_government_building_names_to_firaxis_types(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        aliases = {
            "BUILDING_AUDIENCE_CHAMBER": "BUILDING_GOV_TALL",
            "BUILDING_ANCESTRAL_HALL": "BUILDING_GOV_WIDE",
            "BUILDING_WARLORDS_THRONE": "BUILDING_GOV_CONQUEST",
            "BUILDING_FOREIGN_MINISTRY": "BUILDING_GOV_CITYSTATES",
            "BUILDING_INTELLIGENCE_AGENCY": "BUILDING_GOV_SPIES",
            "BUILDING_GRAND_MASTERS_CHAPEL": "BUILDING_GOV_FAITH",
            "BUILDING_WAR_DEPARTMENT": "BUILDING_GOV_MILITARY",
            "BUILDING_NATIONAL_HISTORY_MUSEUM": "BUILDING_GOV_CULTURE",
            "BUILDING_ROYAL_SOCIETY": "BUILDING_GOV_SCIENCE",
        }
        for civvis_type, firaxis_type in aliases.items():
            self.assertIn(f'{civvis_type} = "{firaxis_type}"', source)

        produce = source.split('if kind == "produce" then', 1)[1].split(
            'if kind == "purchase" or kind == "purchase_faith" then', 1
        )[0]
        alias_at = produce.index("CIVVIS_GOVERNMENT_BUILDING_TYPES[verb]")
        resolve_at = produce.index("resolveType(GameInfo.Types, verb)")
        self.assertLess(alias_at, resolve_at)

    def test_parameterless_unit_commands_match_firaxis_unit_panel_signature(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        helper = source.split("local function commandUnit", 1)[1].split(
            "-- Spend gold", 1
        )[0]

        self.assertIn("CanStartCommand(unit, hash, false, true)", helper)
        self.assertIn("RequestCommand(unit, hash);", helper)
        self.assertNotIn("params", helper)

    def test_parameterless_unit_operations_match_firaxis_unit_panel_signature(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        helper = source.split("local function canOperate", 1)[1].split(
            "-- Same discipline as `operate`", 1
        )[0]
        operate = source.split("local function operate", 1)[1].split(
            "-- Same discipline as `operate`", 1
        )[0]

        self.assertIn(
            "CanStartOperation(unit, hash, nil, false, false)", helper
        )
        self.assertIn("next(params) == nil", helper)
        self.assertIn("RequestOperation(unit, hash);", operate)
        self.assertIn("next(params) == nil", operate)
        self.assertIn(
            "UnitManager.RequestOperation(unit, hash, params);", operate
        )

    def test_builder_repair_uses_firaxis_repair_operation(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

        self.assertIn('"UNITOPERATION_REPAIR",', source)
        self.assertIn('local hash = OP["UNITOPERATION_" .. verb];', source)

    def test_unit_formation_bridge_uses_firaxis_unit_panel_signature(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        exporter = source.split("local units = {};", 1)[1].split("-- Rivals:", 1)[0]
        handler = source.split('if verb == "ENTER_FORMATION" then', 1)[1].split(
            'if verb == "EXIT_FORMATION" then', 1
        )[0]

        self.assertIn("unit:GetFormationUnitCount()", exporter)
        self.assertIn('CMD["UNITCOMMAND_ENTER_FORMATION"]', source)
        self.assertIn('CMD["UNITCOMMAND_EXIT_FORMATION"]', source)
        self.assertIn("params[UnitCommandTypes.PARAM_UNIT_PLAYER] = x", handler)
        self.assertIn("params[UnitCommandTypes.PARAM_UNIT_ID] = y", handler)
        self.assertIn("UnitManager.CanStartCommand(unit, hash, params)", handler)
        self.assertIn("UnitManager.RequestCommand(unit, hash, params)", handler)

    def test_resource_export_gates_on_the_database_reveal_rules(self) -> None:
        """IsResourceVisible alone passed pre-Refining oil (7 plots, run
        civvis-20260807T162004Z); the database PrereqTech/PrereqCivic columns
        are the reveal truth and must be enforced in the same gate."""
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        gate = source.split("local function visibleResourceName", 1)[1].split(
            "local function exportTiles", 1
        )[0]
        self.assertIn("IsResourceVisible(row.Hash)", gate,
                      "the engine gate stays; it hides game-mode-disabled rows")
        self.assertIn("row.PrereqTech", gate)
        self.assertIn("techs:HasTech(tech.Index)", gate)
        self.assertIn("row.PrereqCivic", gate)
        self.assertIn("culture:HasCivic(civic.Index)", gate)

    def test_religion_bridge_uses_firaxis_player_operations_and_exports_its_gate(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        handler = source.split('if kind == "religion" then', 1)[1].split(
            'if kind == "research" or kind == "civic" then', 1
        )[0]

        self.assertIn("HasReligiousFoundingUnit()", source)
        self.assertIn("founded_religion = founded_religion", source)
        self.assertIn("founded_religions = founded_religions", source)
        self.assertIn("religion_beliefs = religion_beliefs", source)
        self.assertIn("taken_religion_beliefs = taken_religion_beliefs", source)
        self.assertIn("prophet_pending = prophet_pending", source)
        self.assertIn("PlayerOperations.FOUND_RELIGION", handler)
        self.assertIn("PlayerOperations.ADD_BELIEF", handler)
        self.assertIn("PlayerOperations.PARAM_RELIGION_TYPE", handler)
        self.assertIn("PlayerOperations.PARAM_BELIEF_TYPE", handler)
        self.assertIn("gameReligion:IsInSomeReligion(follower.Index)", handler)
        self.assertNotIn("IsBeliefInSomeReligion", handler)
        self.assertIn('GameInfo.UnitOperations["UNITOPERATION_FOUND_RELIGION"]', handler)
        self.assertIn('"UNITOPERATION_FOUND_RELIGION",', source)
        self.assertIn(
            "CanStartOperation(prophet, foundOperation.Hash, nil, false,", handler
        )
        self.assertIn("OperationResultsTypes.NO_TARGETS", handler)
        self.assertIn("RequestOperation(prophet, foundOperation.Hash);", handler)

        # ⚠ THE PLAYER OPERATION MUST PRECEDE THE UNIT OPERATION. Requesting the
        # Prophet's spend first retires the founding unit before the founding it
        # was needed for: across the 24 completed live runs of 2026-08-07/08 the
        # Prophet was consumed on the order turn and a religion was founded in
        # 0 of 24, every order reporting `applied` with no refusal.
        self.assertLess(
            handler.index("PlayerOperations.FOUND_RELIGION, found"),
            handler.index("RequestOperation(prophet, foundOperation.Hash);"),
            "found the religion before spending the Prophet on it",
        )
        # And the request cannot report success on its own say-so: a pcall
        # verdict is "did not throw", so the next turn has to check.
        self.assertIn("pendingReligionChoice", handler)
        self.assertIn("religion_founding_failed", source)
        self.assertIn("religion_founded", source)

    def test_religious_units_export_progress_and_actuate_direct_operations(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        progress = source.split("local function unitProgress", 1)[1].split(
            "-- ⚠⚠ THE pcall GOES INSIDE THE LOOP", 1
        )[0]
        handler = source.split('if kind == "unit" then', 1)[1].split(
            'return false, "unknown_kind_"', 1
        )[0]

        self.assertIn("experience:GetExperiencePoints()", progress)
        self.assertIn("experience:GetLevel()", progress)
        self.assertIn("experience:GetPromotions()", progress)
        self.assertIn("unit:GetBuildCharges()", progress)
        self.assertIn("unit:GetSpreadCharges()", progress)
        self.assertIn("unit:GetReligionType()", progress)
        for operation in (
            "UNITOPERATION_SPREAD_RELIGION",
            "UNITOPERATION_LAUNCH_INQUISITION",
            "UNITOPERATION_REMOVE_HERESY",
            "UNITOPERATION_RELIGIOUS_HEAL",
            "UNITOPERATION_CONVERT_BARBARIANS",
        ):
            self.assertIn(f'"{operation}"', source)
        self.assertIn('"^PROMOTE:(.+)$"', handler)
        self.assertIn("results[UnitCommandResults.PROMOTIONS]", handler)
        self.assertIn(
            "params[UnitCommandTypes.PARAM_PROMOTION_TYPE] = promotion.Index",
            handler,
        )
        self.assertIn("UnitManager.RequestCommand(unit, hash, params)", handler)
        self.assertIn('local hash = OP["UNITOPERATION_" .. verb];', handler)
        self.assertIn("return operate(unit, hash, {}), verb;", handler)

    def test_apostle_belief_choice_uses_native_prompt_and_verifies_the_result(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        handler = source.split('if kind == "unit" then', 1)[1].split(
            'return false, "unknown_kind_"', 1
        )[0]
        blocker = source.split("local function answerBlocker", 1)[1].split(
            "-- The hand-written answer", 1
        )[0]
        exporter = source.split("local function exportState", 1)[1].split(
            "local founded_religion = nil;", 1
        )[0]

        self.assertIn('"UNITOPERATION_EVANGELIZE_BELIEF"', source)
        self.assertIn('"^EVANGELIZE_BELIEF:(.+)$"', handler)
        self.assertIn(
            'operate(unit, OP["UNITOPERATION_EVANGELIZE_BELIEF"], {})', handler
        )
        self.assertLess(
            handler.index('operate(unit, OP["UNITOPERATION_EVANGELIZE_BELIEF"], {})'),
            handler.index("pendingReligionChoice = {"),
            "the Apostle operation must create the native prompt before its belief is sent",
        )
        self.assertIn("ENDTURN_BLOCKING_BELIEF = true", source)
        self.assertIn('ENDTURN_BLOCKING_BELIEF = "unit"', source)
        self.assertIn('name == "ENDTURN_BLOCKING_BELIEF"', blocker)
        self.assertIn("PlayerOperations.ADD_BELIEF", blocker)
        self.assertIn("pendingReligionChoice.belief_hash", blocker)
        self.assertIn("religion_enhanced", exporter)
        self.assertIn("religion_enhancement_failed", exporter)

    def test_civvis_soft_blockers_do_not_invoke_legacy_unit_ai(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        handler = source.split("local blocker = currentBlocker(pid);", 1)[1].split(
            "-- Only if the same blocker", 1
        )[0]
        civvis_branch = handler.split("if cfg.CivvisDecides then", 1)[1].split(
            "else", 1
        )[0]
        legacy_branch = handler.split("if cfg.CivvisDecides then", 1)[1].split(
            "else", 1
        )[1]

        self.assertIn('answered = "civvis_complete"', civvis_branch)
        self.assertNotIn("orderUnits(", civvis_branch)
        self.assertIn("orderUnits(player, pid, turn);", legacy_branch)

    def test_completed_civvis_pass_does_not_invoke_owned_blocker_fallbacks(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        handler = source.split("local function answerBlocker", 1)[1].split(
            "local function dismissBlocker", 1
        )[0]
        generic = handler.split(
            "-- A CIVVIS pass is a complete decision for the mirrored state it received.", 1
        )[1]
        completed = generic.index('return "civvis_complete";')
        residual = handler.index("residualAnswers[name]")

        self.assertLess(handler.index(generic) + completed, residual)
        self.assertIn("CIVVIS_OWNED_BLOCKERS[name]", generic[:completed])
        self.assertIn('awaiting.source == "civvis"', generic[:completed])
        self.assertIn("driveProduction(player, turn, true)", handler[residual:])

    def test_empty_civvis_order_batch_completes_without_legacy_fallback(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        settle = source.split("local function settleTurn", 1)[1].split(
            "-- Past the wait", 1
        )[0]

        self.assertIn("ready ~= nil and ready >= 0", settle)
        self.assertIn("if #rows == ready then", settle)
        self.assertNotIn("ready > 0", settle)
        self.assertNotIn("#rows > 0", settle)

    def test_live_rehost_assigns_and_reads_back_the_requested_leader(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        rehost = source.split("local function applyConfiguration()", 1)[1].split(
            "local function rehost()", 1
        )[0]
        self.assertIn("PlayerConfigurations[id]:SetLeaderTypeName(cfg.Leader)", rehost)
        self.assertIn("GetLeaderTypeName()", source.split("local function rehost()", 1)[1])

    def test_permission_denied_deploys_a_staged_copy_through_finder(self) -> None:
        target = Path("/protected/DLC/CivvisControl")
        staging = Path(tempfile.mkdtemp(prefix="civvis-install-test-"))
        config = {"RunTag": "live-test"}
        try:
            with patch.object(install, "install_dir", return_value=target), \
                 patch.object(
                     install, "_write_mod", side_effect=[PermissionError("TCC"), None],
                 ) as write_mod, \
                 patch.object(install, "_finder_replace") as replace, \
                 patch.object(install, "_drop_mod_index") as drop, \
                 patch.object(install.tempfile, "mkdtemp", return_value=str(staging)), \
                 patch.object(install.shutil, "rmtree") as cleanup:
                self.assertEqual(install.install(config), target)

            self.assertEqual(write_mod.call_args_list, [call(target, config), call(staging, config)])
            replace.assert_called_once_with(staging, target)
            drop.assert_called_once_with()
            cleanup.assert_called_once_with(staging, ignore_errors=True)
        finally:
            staging.rmdir()

    def test_clear_run_tag_rewrites_directly_where_it_can(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            mod_dir = Path(temporary)
            (mod_dir / "config.json").write_text(json.dumps(
                {"RunTag": "civvis-20260807T152240Z", "Difficulty": "DIFFICULTY_SETTLER"}))
            with patch.object(install, "install_dir", return_value=mod_dir), \
                 patch.object(install, "_finder_put_file") as finder:
                self.assertTrue(install.clear_run_tag())
            finder.assert_not_called()
            data = json.loads((mod_dir / "config.json").read_text())
            self.assertIsNone(data["RunTag"])
            self.assertEqual(data["Difficulty"], "DIFFICULTY_SETTLER")

    def test_clear_run_tag_lands_through_finder_when_the_bundle_is_protected(self) -> None:
        """The end of both 2026-08-07 games: read allowed, write refused."""
        with tempfile.TemporaryDirectory() as temporary:
            mod_dir = Path(temporary)
            config = mod_dir / "config.json"
            config.write_text(json.dumps({"RunTag": "civvis-live"}))
            real_write = Path.write_text

            def refuse_bundle_writes(path, text, *args, **kwargs):
                if path == config:
                    raise PermissionError("Operation not permitted")
                return real_write(path, text, *args, **kwargs)

            with patch.object(install, "install_dir", return_value=mod_dir), \
                 patch.object(Path, "write_text", refuse_bundle_writes), \
                 patch.object(install, "_finder_put_file") as finder:
                self.assertTrue(install.clear_run_tag())
            finder.assert_called_once()
            staged, destination = finder.call_args[0]
            self.assertEqual(destination, config)
            self.assertEqual(staged.name, "config.json",
                             "the duplicate keeps the source's name")

    def test_clear_run_tag_reports_nothing_to_do(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            mod_dir = Path(temporary)
            with patch.object(install, "install_dir", return_value=mod_dir):
                self.assertFalse(install.clear_run_tag(), "no config at all")
            (mod_dir / "config.json").write_text(json.dumps({"RunTag": None}))
            with patch.object(install, "install_dir", return_value=mod_dir):
                self.assertFalse(install.clear_run_tag(), "tag already clear")


class SealAttributionTest(unittest.TestCase):
    """Issue #1342: installing the mod unsigns `Civ6.app`, and nothing said so.

    The literal `codesign -v --verbose=2` output recorded in the issue, and
    reproduced on this host on 2026-08-08. The point of parsing it is the
    attribution: our own files are expected and `--uninstall` restores the
    seal, while anyone else's are a problem teardown cannot fix.
    """

    MOD = Path(
        "/Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl"
    )
    BROKEN = (
        "/Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app: a sealed resource is missing or invalid\n"
        "file added: /Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl/config.json\n"
        "file added: /Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl/"
        "CivvisControlSetup.lua\n"
        "file added: /Users/x/Library/Application Support/Steam/steamapps/common/"
        "Sid Meier's Civilization VI/Civ6.app/Contents/Assets/DLC/CivvisControl/"
        "CivvisControlAgent.lua\n"
    )

    def test_the_issues_own_codesign_output_is_entirely_our_doing(self) -> None:
        ours, foreign = install.seal_breakers(self.BROKEN, self.MOD)

        self.assertEqual(len(ours), 3)
        self.assertEqual(foreign, [])
        self.assertTrue(all(p.endswith((".json", ".lua")) for p in ours))

    def test_the_summary_line_is_not_read_as_an_offending_file(self) -> None:
        """`<bundle>: <sentence>` and `file added: <path>` are the same shape
        reversed, so the parser keys on which half is the path."""
        ours, foreign = install.seal_breakers(self.BROKEN, self.MOD)

        self.assertNotIn(
            "a sealed resource is missing or invalid", "".join(ours + foreign)
        )

    def test_somebody_elses_modification_is_reported_separately(self) -> None:
        """The half that `--uninstall` will NOT fix."""
        text = self.BROKEN + (
            "file modified: /Users/x/Library/Application Support/Steam/steamapps/"
            "common/Sid Meier's Civilization VI/Civ6.app/Contents/Resources/other\n"
        )

        ours, foreign = install.seal_breakers(text, self.MOD)

        self.assertEqual(len(ours), 3)
        self.assertEqual([p.rsplit("/", 1)[-1] for p in foreign], ["other"])

    def test_a_sibling_directory_sharing_our_prefix_is_not_ours(self) -> None:
        """`CivvisControlOld/` starts with the mod path as a string and is a
        different directory; claiming it would promise an --uninstall that
        never removes it."""
        text = (
            "file added: " + str(self.MOD) + "Old/leftover.lua\n"
            "file added: " + str(self.MOD) + "/config.json\n"
        )

        ours, foreign = install.seal_breakers(text, self.MOD)

        self.assertEqual([p.rsplit("/", 1)[-1] for p in ours], ["config.json"])
        self.assertEqual([p.rsplit("/", 1)[-1] for p in foreign], ["leftover.lua"])

    def test_bundle_dir_is_the_enclosing_app_not_a_second_hardcoded_path(self) -> None:
        with patch.object(install, "install_dir", return_value=self.MOD):
            self.assertEqual(install.bundle_dir(), Path(str(self.MOD).split("/Contents/")[0]))

    def test_an_install_outside_a_bundle_breaks_no_signature(self) -> None:
        outside = Path("/Users/x/Library/Application Support/Sid Meier's Civ VI/Mods/CivvisControl")
        with patch.object(install, "install_dir", return_value=outside):
            self.assertIsNone(install.bundle_dir())
            self.assertEqual(install.signature_report()["state"], "no-bundle")

    def test_a_valid_bundle_reports_valid_and_names_nobody(self) -> None:
        with patch.object(install, "install_dir", return_value=self.MOD), \
             patch.object(install.subprocess, "run",
                          return_value=Mock(returncode=0, stdout="", stderr="")):
            seal = install.signature_report()

        self.assertEqual(seal["state"], "valid")
        self.assertEqual((seal["ours"], seal["foreign"]), ([], []))

    def test_a_broken_bundle_reports_the_verdict_and_attributes_the_files(self) -> None:
        with patch.object(install, "install_dir", return_value=self.MOD), \
             patch.object(install.subprocess, "run",
                          return_value=Mock(returncode=1, stdout="", stderr=self.BROKEN)):
            seal = install.signature_report()

        self.assertEqual(seal["state"], "broken")
        self.assertEqual(seal["detail"], "a sealed resource is missing or invalid")
        self.assertEqual(len(seal["ours"]), 3)

    def test_an_unrunnable_codesign_is_unknown_rather_than_valid(self) -> None:
        """Absence of a verdict must never be reported as a good one."""
        with patch.object(install, "install_dir", return_value=self.MOD), \
             patch.object(install.subprocess, "run", side_effect=OSError("no codesign")):
            self.assertEqual(install.signature_report()["state"], "unknown")


class UnitsBlockerForfeitTest(unittest.TestCase):
    """Issue #1374: a turn that wedged 900 s on `ENDTURN_BLOCKING_UNITS`.

    Run civvis-20260807T190903Z turn 39 answered the blocker `civvis_complete`
    once, recorded `attempts:1`, and then sat until an outside watchdog killed
    the attempt. Two facts make that unrecoverable without an explicit
    escalation, and both are pinned below:

    * `civvis_complete` changes the board by construction not at all, so the
      blocker cannot clear itself, and
    * the shipped `ActionPanel.lua` never requests `ACTION_ENDTURN` while one
      of three unit blockers is up -- it calls `UI.SelectNextReadyUnit()` --
      so the plain request at the bottom of `tick` is refused. Only the
      `{ REASON = "UserForced" }` form (the shipped SHIFT+ENTER path) ends it.
    """

    def setUp(self) -> None:
        self.source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

    @property
    def escalation(self) -> str:
        return self.source.split(
            "-- ★★★ A SOFT BLOCKER THAT SURVIVES ITS ANSWER", 1
        )[1].split("-- Only if the same blocker", 1)[0]

    @property
    def parking(self) -> str:
        return self.source.split("local function parkReadyUnits", 1)[1].split(
            "\nlocal ", 1
        )[0]

    def test_a_units_blocker_surviving_its_answer_is_parked_dismissed_and_forced(self) -> None:
        self.assertIn("parkReadyUnits(player)", self.escalation)
        self.assertIn("dismissBlocker(pid, blocker)", self.escalation)
        self.assertIn('REASON = "UserForced"', self.escalation)

    def test_forcing_is_reserved_for_the_three_blockers_the_engine_refuses(self) -> None:
        """The trio `ActionPanel.DoEndTurn` special-cases, and only those.

        Widening this table would force the turn past prompts the engine would
        have ended normally; narrowing it puts the wedge back.
        """
        table = self.source.split("local UNIT_BLOCKERS = {", 1)[1].split("};", 1)[0]
        self.assertEqual(
            sorted(
                line.split("=")[0].strip()
                for line in table.splitlines()
                if "=" in line
            ),
            [
                "ENDTURN_BLOCKING_STACKED_UNITS",
                "ENDTURN_BLOCKING_UNITS",
                "ENDTURN_BLOCKING_UNIT_NEEDS_ORDERS",
            ],
        )
        self.assertIn("UNIT_BLOCKERS[name] and parkReadyUnits(player)", self.escalation)

    def test_a_civvis_complete_answer_escalates_on_the_very_next_sighting(self) -> None:
        """Waiting longer buys no information: that answer touched nothing.

        The legacy answer does run `orderUnits`, so it keeps its `MaxSoftPasses`
        budget before being called stuck.
        """
        self.assertIn(
            'answered == "civvis_complete" and 2 or (cfg.MaxSoftPasses or 3) + 1',
            self.escalation,
        )

    def test_parking_forfeits_movement_but_never_calls_the_legacy_movement_ai(self) -> None:
        """`orderIdle`, not `orderFor`.

        The legacy pass once walked a Settler out of a safe capital into a
        barbarian capture zone after CIVVIS had deliberately left it in place;
        skip/fortify/alert/sleep forfeits only the movement that ending the
        turn forfeits anyway.
        """
        self.assertIn("orderIdle(unit)", self.parking)
        self.assertNotIn("orderFor(", self.parking)

    def test_live_settler_residual_pass_keeps_civvis_settlement_authority(self) -> None:
        """A post-answer unblock may clear readiness, never invent a city.

        The live planner can deliberately leave a settler unmentioned after
        rejecting every nearby tile on its loyalty forecast. A lingering unit
        blocker gets one residual `orderUnits` pass; that pass must idle the
        settler after either a current or stale CIVVIS answer, while a genuine
        timeout fallback retains the legacy founder. Explicit approved
        FOUND_CITY rows still go through `applyOrders`.
        """
        order_for = self.source.split("local function orderFor(player, pid, unit, turn)", 1)[
            1
        ]
        settler = order_for.split('if name == "UNIT_SETTLER" then', 1)[1].split(
            'elseif name == "UNIT_BUILDER"', 1
        )[0]
        apply_orders = self.source.split("local function applyOrders", 1)[1].split(
            "local function reportLostCities", 1
        )[0]

        self.assertIn("if cfg.CivvisDecides", settler)
        self.assertIn('awaiting.source == "civvis"', settler)
        self.assertIn('awaiting.source == "civvis_stale"', settler)
        self.assertIn("return orderIdle(unit)", settler)
        self.assertIn("return orderSettler(player, pid, unit, turn)", settler)
        self.assertLess(
            settler.index("return orderIdle(unit)"),
            settler.index("return orderSettler(player, pid, unit, turn)"),
        )
        self.assertIn('tostring(row.verb or "") == "FOUND_CITY"', apply_orders)
        self.assertIn(
            'local placed = operate(unit, OP["UNITOPERATION_FOUND_CITY"], {});',
            self.source,
        )

    def test_parking_sweeps_the_roster_after_the_ready_query_jams(self) -> None:
        """`GetFirstReadyUnit` offers an uncooperative unit forever.

        A forfeit built on that query alone parks one unit, leaves the rest
        ready, and the blocker comes straight back.
        """
        self.assertIn("GetFirstReadyUnit", self.parking)
        self.assertIn("eachUnit(player", self.parking)

    def test_the_forfeit_retry_is_bounded_and_then_names_the_wedge(self) -> None:
        self.assertIn("cfg.MaxSoftBlockerForfeits or 3", self.escalation)
        self.assertIn("seen.forfeits < cap", self.escalation)
        self.assertIn('emit("wedged"', self.escalation)

    def test_the_residual_answer_is_bounded_before_the_forfeit(self) -> None:
        """A residual ladder answer that leaves the blocker standing is tried
        twice, then the forfeit runs.

        Run `civvis-20260816T115139Z` -- the seat's best game, 804 against 715 --
        wedged at turn 178: `civvis_complete`, the ladder's `units` answer,
        `residual_unblock ... forfeits 0`, and the same blocker back, seven
        times, because the residual arm reset `attempts` and never fell through
        to the forfeit. The outside watchdog killed it after 900 s.
        """
        self.assertIn(
            '(seen.residuals or 0) < (cfg.MaxResidualAnswers or 2)',
            self.escalation,
        )
        self.assertIn("seen.residuals = (seen.residuals or 0) + 1", self.escalation)
        # The bound is checked BEFORE the ladder is asked, so an exhausted
        # residual budget yields a nil pick and the forfeit branch below runs.
        bound = self.escalation.index("(seen.residuals or 0) < (cfg.MaxResidualAnswers or 2)")
        ask = self.escalation.index("residual_pick = answerBlocker(player, pid, blocker, turn, true)")
        forfeit = self.escalation.index("(not residual_taken or UNIT_BLOCKERS[name]) and seen.forfeits < cap then")
        self.assertLess(bound, ask)
        self.assertLess(ask, forfeit)

    def test_a_units_blocker_forfeits_in_the_same_pass_as_its_residual_answer(self) -> None:
        """A quiet board never ticks again.

        Run civvis-20260816T151716Z wedged at turn 111 WITH the residual bound
        in place: two residual `units` answers, then no further game-core
        event, so the forfeit the next sighting was to bring never ran. For
        the units family the forfeit runs in the same pass; research and
        production keep the two-step.
        """
        self.assertIn("local residual_taken = false;", self.escalation)
        self.assertIn("residual_taken = true;", self.escalation)
        self.assertIn(
            "if (not residual_taken or UNIT_BLOCKERS[name]) and seen.forfeits < cap then",
            self.escalation,
        )
        self.assertIn("elseif not residual_taken and seen.forfeits == cap then", self.escalation)
        # The residual answer is issued BEFORE the forfeit in the same pass, so
        # its own requests are queued ahead of the forced end of turn.
        taken = self.escalation.index("residual_taken = true;")
        forfeit = self.escalation.index("(not residual_taken or UNIT_BLOCKERS[name]) and seen.forfeits < cap then")
        forced = self.escalation.index('REASON = "UserForced"', forfeit)
        self.assertLess(taken, forfeit)
        self.assertLess(forfeit, forced)

    def test_a_blocker_change_ticks_without_the_publish_divider(self) -> None:
        """A board sitting on a blocker publishes almost nothing.

        Routing `EndTurnBlockingChanged` through the 1-in-16 `onGameCoreTick`
        divider made every blocker transition wait for fifteen more publish
        batches that a wedged turn never produces.
        """
        self.assertIn("EndTurnBlockingChanged = onEndTurnBlockingChanged", self.source)
        self.assertNotIn("EndTurnBlockingChanged = onGameCoreTick", self.source)


class PeacetimeWarFloorsTest(unittest.TestCase):
    """On a CIVVIS seat, the ladder's war floors require an actual war.

    `warTarget` is "who we would fight" and exists from the first met major, so
    gating the battering-ram entry and the ranged floor on it alone kept a
    permanent peacetime war footing on CIVVIS runs (41 ranged orders at peace,
    zero ever alive, run civvis-20260818T212725Z). The `warFooting` gate keys
    them on `warPressure`'s at-war read instead; `cfg.PeacetimeWarFloors` is
    the recorded control arm and legacy no-decider runs keep the old build-up.
    """

    @classmethod
    def setUpClass(cls) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        cls.ladder = source.split("local function chooseProduction", 1)[1].split(
            "THE ECONOMY GOES ABOVE THE OPEN-ENDED ARMY", 1
        )[0]

    def test_the_war_footing_gate_reads_a_real_war(self) -> None:
        self.assertIn(
            "local warFooting = atWar or not cfg.CivvisDecides"
            " or cfg.PeacetimeWarFloors;",
            self.ladder,
        )

    def test_the_ram_entry_and_ranged_floor_sit_behind_the_gate(self) -> None:
        ram = self.ladder.split('{ "UNIT_BATTERING_RAM", "siege" }', 1)[0]
        self.assertIn("warTarget ~= nil and warFooting and not losingWar", ram)
        ranged = self.ladder.split('pushRangedLandUnits("ranged")', 1)[0]
        self.assertIn(
            "if warTarget ~= nil and warFooting\n"
            "\t\t\tand (counts.ranged or 0) < (cfg.RangedFloor or 3) then",
            ranged,
        )


class MeleeStrikeCarriesTheAttackModifierTest(unittest.TestCase):
    """A melee ATTACK must be a MOVE_TO *with* the ATTACK modifier.

    ⚠ WITHOUT THE MODIFIER THE ARMY SWINGS AT AIR AND NOTHING SAYS SO.
    Measured over every control run this repository's seat has recorded:
    8,828 melee ATTACK orders were issued and 89 combats came back — a 1.0%
    landing rate — while RANGE_ATTACK, which needs no modifier, landed 520 of
    841 (61.8%). On run civvis-20260821T130446Z the seat ordered 208 melee
    attacks across 104 turns and fought ZERO of them; a barbarian Slinger held
    (65,25) from t36 to t40 under an "attack" order every turn, and the empire
    lost eight Settlers to raiders it never once hit.

    Firaxis's shipped `Civ6Common.lua:RequestMoveOperation` sets
    `PARAM_MODIFIERS = ATTACK + MOVE_IGNORE_UNEXPLORED_DESTINATION` before
    requesting MOVE_TO. Without `ATTACK` the engine reads a plain move, the
    pathfinder refuses to enter an occupied plot, and the unit walks next to
    the target and stops -- while `CanStartOperation` answers TRUE, so
    `operate` reports the order as given and no refusal is ever logged.
    """

    def _agent_source(self) -> str:
        return (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()

    def test_the_attack_verb_sets_param_modifiers_before_requesting_move_to(self) -> None:
        source = self._agent_source()
        block = source.split(
            'if verb == "MOVE_TO" or verb == "ATTACK" or verb == "CAPTURE" then', 1
        )[1]
        block = block.split('local moved = operate(', 1)[0]

        self.assertIn("CivvisLedger.attackModifiers()", block)
        self.assertIn("params[UnitOperationTypes.PARAM_MODIFIERS] = modifiers", block)
        # The modifier belongs to ATTACK and CAPTURE alone: a plain MOVE_TO
        # that carried it would attack whatever happened to be standing on
        # the destination. CAPTURE is the same modifier for a move onto an
        # enemy civilian (a bare MOVE_TO walks next to it and stops — 65
        # sent, 0 captures across the 273 live runs that carried #2075).
        before_guard, guard = block.split('if verb == "ATTACK" or verb == "CAPTURE" then', 1)
        self.assertNotIn("PARAM_MODIFIERS", before_guard)
        self.assertIn("PARAM_MODIFIERS", guard)
        # Only a strike is entered in the ledger: a capture produces no combat
        # to match it against.
        outside_strike, strike = guard.split('if verb == "ATTACK" then', 1)
        self.assertNotIn("CivvisLedger.strike(", outside_strike)
        self.assertIn("CivvisLedger.strike(", strike)

    def test_the_modifier_resolves_the_shipped_pair_and_survives_their_absence(self) -> None:
        source = self._agent_source()
        helper = source.split("CivvisLedger.attackModifiers = function()", 1)[1]
        helper = helper.split("CivvisLedger.strike = function", 1)[0]

        self.assertIn("UnitOperationMoveModifiers.ATTACK", helper)
        self.assertIn(
            "UnitOperationMoveModifiers.MOVE_IGNORE_UNEXPLORED_DESTINATION", helper
        )
        # An absent enum must send the historical parameter table, not throw on
        # every attack for the rest of the game.
        self.assertIn("if attack == nil then return nil; end", helper)
        self.assertIn("if ignore == nil then return attack; end", helper)

    def test_the_helper_costs_no_main_chunk_local(self) -> None:
        """See `AgentChunkLocalLimitTest`: the file has no slots to spend."""
        source = self._agent_source()
        self.assertIn("CivvisLedger.attackModifiers = function()", source)
        self.assertNotIn("\nlocal attackModifiers", source)
        self.assertNotIn("\nlocal function attackModifiers", source)


class AgentChunkLocalLimitTest(unittest.TestCase):
    """The agent's main chunk must stay inside Lua's 200-local ceiling.

    ⚠ THIS IS A SILENT, TOTAL FAILURE, which is why it is worth a test. Lua
    allows 200 local variables per function and the mod's main chunk is one
    function, currently within single digits of the limit. Crossing it is a
    compile error, and a mod script that fails to compile writes NOTHING to any
    log -- the context loads, the script dies at parse time, and the run is
    indistinguishable from a game where CIVVIS simply never decided anything.
    `civ6_preflight.py` catches this with `luac -p`, but only on a host that
    has `luac` installed; this catches it everywhere, including CI.

    A file-scope `local` is almost always avoidable: nest the helper, hang the
    value off an existing table, or reuse a neighbouring one.
    """

    # `luac -l` reports one more register than this source proxy counts (the
    # current file is 198 locals / 199 slots). Keep the proxy below Lua's
    # 200-slot ceiling so the next file-scope local fails in CI as well.
    LIMIT = 199

    def test_main_chunk_locals_stay_under_the_limit(self) -> None:
        source = (install.MOD_SOURCE / "CivvisControlAgent.lua").read_text()
        count = 0
        for line in source.splitlines():
            if not line.startswith("local "):
                continue  # indented locals belong to a nested scope
            rest = line[len("local "):]
            if rest.startswith("function "):
                count += 1
                continue
            names = rest.split("=", 1)[0]
            count += len([n for n in names.split(",") if n.strip()])
        self.assertLess(
            count,
            self.LIMIT,
            f"CivvisControlAgent.lua declares {count} main-chunk locals; Lua "
            f"allows {self.LIMIT} and the mod would fail to compile in-game "
            f"with no log line anywhere. Nest a helper instead of adding one.",
        )
