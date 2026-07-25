from pathlib import Path
import json
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civvis_fleet as fleet


def completed(returncode=0, stdout="", stderr=""):
    return subprocess.CompletedProcess(args=[], returncode=returncode, stdout=stdout, stderr=stderr)


PROBE_OUTPUT = "cores=8\nload=1.5\nrevision=abc1234\nbuilt=yes\nworker=no\n"


class ConfigTests(unittest.TestCase):
    def test_a_missing_config_is_a_fleet_of_one(self):
        with tempfile.TemporaryDirectory() as tmp:
            cfg = fleet.load_config(Path(tmp) / "nope.json")
        self.assertEqual([h.name for h in cfg.hosts], ["local"])
        self.assertTrue(cfg.home_host.is_local)

    def test_hosts_and_defaults_are_read(self):
        cfg = fleet.parse_config(
            {
                "home": "boxa",
                "league_dir": "/srv/league",
                "games": 32,
                "hosts": [
                    {"name": "boxa", "root": "/srv/civvis"},
                    {"name": "boxb", "transport": "ssh", "ssh": "b.local", "root": "/r", "jobs": 4},
                ],
            }
        )
        self.assertEqual(cfg.home, "boxa")
        self.assertEqual(cfg.league_dir, "/srv/league")
        self.assertEqual(cfg.games, 32)
        self.assertEqual(cfg.host("boxb").ssh, "b.local")
        self.assertEqual(cfg.host("boxb").jobs, 4)
        self.assertFalse(cfg.host("boxb").is_local)

    def test_an_unknown_transport_is_rejected_rather_than_guessed(self):
        with self.assertRaises(fleet.FleetError):
            fleet.parse_config({"hosts": [{"name": "x", "transport": "carrier-pigeon"}]})

    def test_a_home_that_is_not_in_the_fleet_is_an_error(self):
        cfg = fleet.parse_config({"home": "ghost", "hosts": [{"name": "real"}]})
        with self.assertRaises(fleet.FleetError):
            cfg.home_host


class HostCommandTests(unittest.TestCase):
    def test_a_remote_command_goes_through_ssh_without_a_password_prompt(self):
        host = fleet.Host(name="spark", transport="ssh", ssh="spark.local")
        cmd = host.command("echo hi")
        self.assertEqual(cmd[0], "ssh")
        self.assertIn("BatchMode=yes", cmd)
        self.assertIn("spark.local", cmd)
        # cargo lives outside a non-interactive PATH on both macOS and Linux.
        self.assertIn(".cargo/bin", cmd[-1])

    def test_a_local_command_does_not_go_through_ssh(self):
        cmd = fleet.Host(name="local").command("echo hi")
        self.assertEqual(cmd[0], "/bin/sh")


class ProbeTests(unittest.TestCase):
    def test_a_reachable_host_reports_its_capacity(self):
        with patch("subprocess.run", return_value=completed(stdout=PROBE_OUTPUT)):
            status = fleet.probe(fleet.Host(name="boxa", root="/srv"))
        self.assertTrue(status.reachable)
        self.assertEqual(status.cores, 8)
        self.assertEqual(status.revision, "abc1234")
        self.assertTrue(status.built)
        self.assertFalse(status.worker_running)
        # Some cores are left for whoever owns the machine.
        self.assertEqual(status.jobs, 8 - fleet.RESERVED_CORES)

    def test_an_explicit_job_count_overrides_the_core_count(self):
        with patch("subprocess.run", return_value=completed(stdout=PROBE_OUTPUT)):
            status = fleet.probe(fleet.Host(name="boxa", jobs=3))
        self.assertEqual(status.jobs, 3)

    def test_a_host_that_is_down_costs_one_timeout_and_nothing_else(self):
        with patch(
            "subprocess.run",
            side_effect=subprocess.TimeoutExpired(cmd="ssh", timeout=fleet.PROBE_TIMEOUT),
        ):
            status = fleet.probe(fleet.Host(name="spark", transport="ssh"))
        self.assertFalse(status.reachable)
        self.assertIn("timed out", status.detail)
        self.assertEqual(status.cores, 0)

    def test_a_disabled_host_is_never_contacted(self):
        with patch("subprocess.run", side_effect=AssertionError("must not run")):
            status = fleet.probe(fleet.Host(name="retired", enabled=False))
        self.assertFalse(status.reachable)
        self.assertIn("disabled", status.detail)

    def test_the_fleet_survives_every_remote_host_being_down(self):
        cfg = fleet.parse_config(
            {
                "hosts": [
                    {"name": "local"},
                    {"name": "a", "transport": "ssh"},
                    {"name": "b", "transport": "ssh"},
                ]
            }
        )

        def fake_run(cmd, **kwargs):
            if cmd[0] == "ssh":
                raise subprocess.TimeoutExpired(cmd="ssh", timeout=1)
            return completed(stdout=PROBE_OUTPUT)

        with patch("subprocess.run", side_effect=fake_run):
            statuses = fleet.probe_fleet(cfg)
        self.assertEqual([s.reachable for s in statuses], [True, False, False])


class DeployTests(unittest.TestCase):
    def test_the_fleet_builds_a_private_detached_worktree_not_a_checkout(self):
        script = fleet.deploy_script("/srv/civvis", fleet.REPOSITORY)
        self.assertIn("--detach origin/main", script)
        self.assertIn("/srv/civvis/src", script)
        self.assertIn("cargo build --release --locked", script)
        # A fleet build must never touch a development checkout, so the only
        # path it resets is the one it created under its own root.
        self.assertNotIn("git -C /Users", script)

    def test_a_failed_build_is_reported_not_swallowed(self):
        with patch("subprocess.run", return_value=completed(1, stderr="linker exploded")):
            ok, detail = fleet.deploy(fleet.Host(name="boxa", root="/srv"))
        self.assertFalse(ok)
        self.assertIn("linker exploded", detail)


class WorkerTests(unittest.TestCase):
    def test_the_worker_command_carries_the_hosts_identity_and_capacity(self):
        cfg = fleet.parse_config(
            {"home": "boxa", "league_dir": "/srv/league", "hosts": [{"name": "boxa", "root": "/srv"}]}
        )
        status = fleet.HostStatus(host=cfg.hosts[0], reachable=True, cores=10)
        cmd = fleet.league_command(cfg, status)
        self.assertIn("--worker boxa", cmd)
        self.assertIn("--jobs 8", cmd)
        self.assertIn("--dir /srv/league", cmd)

    def test_a_remote_host_keeps_its_own_mirror_of_the_league(self):
        cfg = fleet.parse_config(
            {
                "home": "boxa",
                "league_dir": "/srv/league",
                "hosts": [{"name": "boxa", "root": "/srv"}, {"name": "boxb", "transport": "ssh", "root": "/r"}],
            }
        )
        self.assertEqual(fleet.league_dir_for(cfg, cfg.host("boxa")), "/srv/league")
        self.assertEqual(fleet.league_dir_for(cfg, cfg.host("boxb")), "/r/league")

    def test_a_started_worker_lets_go_of_the_pipe_it_was_launched_through(self):
        # The worker outlives the command that starts it, so every one of its
        # descriptors has to leave the caller's pipe. Leaving any attached
        # blocks the fleet cycle until the worker exits, which is hours.
        cfg = fleet.parse_config({"hosts": [{"name": "local", "root": "/srv"}]})
        status = fleet.HostStatus(host=cfg.hosts[0], reachable=True, cores=4)
        seen = {}

        def capture(cmd, **kwargs):
            seen["script"] = cmd[-1]
            return completed(stdout="started 4242")

        with patch("subprocess.run", side_effect=capture):
            ok, detail = fleet.start_worker(cfg, status)
        self.assertTrue(ok)
        script = seen["script"]
        self.assertIn("</dev/null", script)
        self.assertIn(">> /srv/league-worker.log 2>&1", script)
        # `mkdir && nohup ... &` would background the whole compound and hold
        # stdout open; the directories must be made before the fork.
        self.assertNotIn("&& nohup", script)

    def test_starting_a_worker_twice_does_not_start_two(self):
        cfg = fleet.parse_config({"hosts": [{"name": "local", "root": "/srv"}]})
        status = fleet.HostStatus(host=cfg.hosts[0], reachable=True, cores=4, worker_running=True)
        with patch("subprocess.run", side_effect=AssertionError("must not spawn")):
            ok, detail = fleet.start_worker(cfg, status)
        self.assertTrue(ok)
        self.assertIn("already running", detail)


class ReplicationTests(unittest.TestCase):
    def test_replicating_a_directory_onto_itself_is_a_no_op(self):
        host = fleet.Host(name="local")
        with patch("subprocess.run", side_effect=AssertionError("must not rsync")):
            ok, detail = fleet.replicate(host, host, "/srv/league", "/srv/league")
        self.assertTrue(ok)

    def test_a_remote_league_is_addressed_as_an_rsync_target(self):
        host = fleet.Host(name="boxb", transport="ssh", ssh="b.local")
        self.assertEqual(fleet.rsync_spec(host, "/r/league"), "b.local:/r/league")
        self.assertEqual(fleet.rsync_spec(fleet.Host(name="local"), "/l"), "/l")

    def test_replication_between_two_remote_hosts_is_refused_rather_than_wrong(self):
        a = fleet.Host(name="a", transport="ssh", ssh="a")
        b = fleet.Host(name="b", transport="ssh", ssh="b")
        ok, detail = fleet.replicate(a, b, "/x", "/y")
        self.assertFalse(ok)
        self.assertIn("local hop", detail)


class HealthTests(unittest.TestCase):
    LEARNING = """\
forecast quality, scored before each result was revealed (168 games, 6.0 seats on average)

rating system                    winner LL  accuracy   info/game     pair LL pair Brier
uniform (no information)            1.7918     27.4%      0.0000      0.6931     0.2500
glicko-2 (league today)             1.3216     43.5%      0.4701      0.5146     0.1704
staged + civ context                1.2702     41.7%      0.5215      0.5188     0.1701
(random guess)                      1.7918     16.7%      0.0000      0.6931     0.2500   <- guessing
"""

    STALLED = """\
forecast quality, scored before each result was revealed (302 games, 6.0 seats on average)

rating system                    winner LL  accuracy   info/game     pair LL pair Brier
uniform (no information)            1.7918     20.5%      0.0000      0.6931     0.2500
glicko-2 (league today)             1.8168     16.4%     -0.0251      0.6913     0.2490
staged + civ context                1.8054     18.8%     -0.0137      0.6907     0.2487
(random guess)                      1.7918     16.7%      0.0000      0.6931     0.2500   <- guessing
"""

    def test_a_league_whose_games_separate_its_roster_reads_as_learning(self):
        health = fleet.parse_health(self.LEARNING)
        self.assertEqual(health.verdict, "learning")
        self.assertTrue(health.learning)
        self.assertAlmostEqual(health.information, 0.5215)
        self.assertEqual(health.games, 168)
        self.assertAlmostEqual(health.seats, 6.0)

    def test_a_converged_league_is_reported_as_stalled_not_as_healthy(self):
        health = fleet.parse_health(self.STALLED)
        self.assertEqual(health.verdict, "stalled")
        self.assertFalse(health.learning)
        self.assertLess(health.information, 0.0)
        # The point of the check is that it says what to do about it.
        self.assertIn("Fix the experiment", health.detail)

    def test_the_baseline_rows_never_count_as_a_system_that_learned(self):
        # Both baselines score exactly zero; a fleet that treated "uniform"
        # as a candidate would call every stalled league healthy.
        health = fleet.parse_health(self.STALLED)
        self.assertNotAlmostEqual(health.information, 0.0)

    def test_an_unreadable_report_is_unknown_rather_than_a_false_pass(self):
        health = fleet.parse_health("no table here")
        self.assertEqual(health.verdict, "unknown")
        self.assertFalse(health.learning)

    def test_a_rating_binary_that_fails_does_not_look_like_a_healthy_league(self):
        with patch("subprocess.run", return_value=completed(1, stderr="no such league")):
            health = fleet.audit_league("civvis", "/nowhere")
        self.assertEqual(health.verdict, "unknown")
        self.assertFalse(health.learning)


SRC_AI_RS = """
impl Default for Weights {
    fn default() -> Weights {
        Weights {
            city_target: 4.0,
            war_ratio: 1.8,
            cohesion: 3.0,
        }
    }
}

impl Weights {
    pub fn bounds() -> [(f64, f64); 3] {
        [
            (2.0, 12.0),
            (0.8, 5.0),
            (0.0, 10.0),
        ]
    }
}
"""


class ExperimentTests(unittest.TestCase):
    def genome(self, text=SRC_AI_RS):
        with tempfile.TemporaryDirectory() as tmp:
            Path(tmp, "src").mkdir()
            Path(tmp, "src", "ai.rs").write_text(text)
            return fleet.read_genome_defaults(tmp)

    def test_the_genome_is_read_from_source_not_duplicated(self):
        defaults, bounds = self.genome()
        self.assertEqual(defaults["city_target"], 4.0)
        self.assertEqual(bounds["war_ratio"], (0.8, 5.0))
        self.assertEqual(list(defaults), list(bounds))

    def test_a_genome_whose_bounds_do_not_line_up_is_refused_not_guessed(self):
        broken = SRC_AI_RS.replace("            (0.0, 10.0),\n", "")
        with self.assertRaises(fleet.FleetError):
            self.genome(broken)

    def test_a_sweep_varies_one_gene_at_a_time_around_the_shipped_default(self):
        defaults, bounds = self.genome()
        roster = fleet.gene_sweep_roster(defaults, bounds, ["city_target", "war_ratio"])
        names = [r["name"] for r in roster]
        self.assertEqual(names[:3], ["advanced", "basic", "control"])
        self.assertIn("city_target_lo", names)
        self.assertIn("war_ratio_hi", names)
        control = next(r for r in roster if r["name"] == "control")
        self.assertEqual(control["kind"]["Advanced"]["weights"], defaults)
        low = next(r for r in roster if r["name"] == "city_target_lo")
        self.assertEqual(low["kind"]["Advanced"]["weights"]["city_target"], 2.0)
        # every other gene must be untouched, or the sweep measures a mixture
        self.assertEqual(low["kind"]["Advanced"]["weights"]["war_ratio"], 1.8)

    def test_a_default_already_on_a_bound_does_not_produce_a_duplicate_control(self):
        defaults, bounds = self.genome()
        defaults["cohesion"] = 0.0  # sits exactly on its lower bound
        roster = fleet.gene_sweep_roster(defaults, bounds, ["cohesion"])
        self.assertNotIn("cohesion_lo", [r["name"] for r in roster])
        self.assertIn("cohesion_hi", [r["name"] for r in roster])

    def test_an_unknown_gene_is_an_error_rather_than_a_silent_no_op(self):
        defaults, bounds = self.genome()
        with self.assertRaises(fleet.FleetError):
            fleet.gene_sweep_roster(defaults, bounds, ["telepathy"])

    def test_seeding_refuses_to_overwrite_a_running_experiment(self):
        defaults, bounds = self.genome()
        roster = fleet.gene_sweep_roster(defaults, bounds, ["city_target"])
        with tempfile.TemporaryDirectory() as tmp:
            fleet.seed_experiment(tmp, roster)
            self.assertTrue(Path(tmp, "league.json").exists())
            with self.assertRaises(fleet.FleetError):
                fleet.seed_experiment(tmp, roster)

    def test_a_gap_inside_the_confidence_interval_is_reported_as_noise(self):
        variants = [
            fleet.Variant("control", 1500.0, 40.0, 200, 40),
            fleet.Variant("close", 1530.0, 40.0, 200, 42),
            fleet.Variant("clear", 1800.0, 40.0, 200, 90),
            fleet.Variant("weak", 1200.0, 40.0, 200, 5),
        ]
        rows = {v.name: verdict for v, _, verdict in fleet.separation(variants)}
        self.assertTrue(rows["close"].startswith("noise"))
        self.assertEqual(rows["clear"], "stronger")
        self.assertEqual(rows["weak"], "weaker")

    def test_a_variant_with_too_few_games_is_never_called_a_result(self):
        variants = [
            fleet.Variant("control", 1500.0, 40.0, 200, 40),
            fleet.Variant("fresh", 1900.0, 300.0, 4, 3),
        ]
        rows = {v.name: verdict for v, _, verdict in fleet.separation(variants)}
        self.assertEqual(rows["fresh"], "too few games")

    def test_a_missing_control_yields_no_claims_at_all(self):
        variants = [fleet.Variant("a", 1700.0, 30.0, 100, 30)]
        self.assertEqual(fleet.separation(variants), [])


class RosterTests(unittest.TestCase):
    def test_the_active_roster_excludes_retired_strategies(self):
        with tempfile.TemporaryDirectory() as tmp:
            Path(tmp, "league.json").write_text(
                json.dumps(
                    {
                        "round": 42,
                        "strategies": [
                            {"name": "a"},
                            {"name": "b", "retired": True},
                            {"name": "c", "retired": False},
                        ],
                    }
                )
            )
            active, rnd = fleet.league_roster(tmp)
        self.assertEqual((active, rnd), (2, 42))

    def test_a_missing_league_reads_as_empty_rather_than_crashing(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(fleet.league_roster(tmp), (0, 0))


if __name__ == "__main__":
    unittest.main()
