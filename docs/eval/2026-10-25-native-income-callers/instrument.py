"""Apply diagnostic-only logging to an exact historical source assembly.

This is never a deployment patch. The instrumented executable must reproduce
every full native planning payload and ordered read sequence before its logs
can support an inference about the original controller.
"""
import argparse
import difflib
import hashlib
import json
from pathlib import Path
import subprocess


BASE = "b4dfb57ea13b5d1bc08918d71769a94c88b690f7"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--repository", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    args.out.mkdir(exist_ok=False)
    changes = {}

    def read(name):
        original = subprocess.check_output(["git", "show", BASE + ":" + name], cwd=args.repository)
        assert (args.source / name).read_bytes() == original, name + " is not the exact base"
        changes[name] = [original.decode(), None]
        return original.decode()

    def write(name, text):
        changes[name][1] = text
        (args.source / name).write_text(text)

    def replace(text, old, new):
        assert text.count(old) == 1, old[:100]
        return text.replace(old, new, 1)

    name = "src/game/actions.rs"
    text = read(name)
    text = replace(text, "use super::*;\n", """use super::*;

std::thread_local! {
    static INCOME_PROBE_ROOT: std::cell::Cell<(usize, u32)> = std::cell::Cell::new((0, 0));
}
""")
    old = "    pub fn apply(&mut self, pid: usize, action: &Action) -> Result<(), String> {"
    text = replace(text, old, """    pub fn set_income_probe_root(&self, frame: u32) {
        INCOME_PROBE_ROOT.with(|root| root.set((self as *const Self as usize, frame)));
        self.income_probe_event("root_begin", serde_json::json!({}));
    }

    pub fn income_probe_active(&self) -> bool {
        (60..=70).contains(&self.turn)
            && INCOME_PROBE_ROOT.with(|root| root.get().0 == self as *const Self as usize)
    }

    pub fn income_probe_event(&self, event: &str, mut row: serde_json::Value) {
        if !self.income_probe_active() {
            return;
        }
        row["event"] = event.into();
        row["turn"] = self.turn.into();
        row["frame"] = INCOME_PROBE_ROOT.with(|root| root.get().1).into();
        row["model_gold"] = self.players[0].gold.into();
        row["model_gold_per_turn"] = self.players[0].gold_per_turn.into();
        eprintln!("[income-probe] {row}");
    }

    #[track_caller]
    pub fn apply(&mut self, pid: usize, action: &Action) -> Result<(), String> {
        let before = if pid == 0 && self.income_probe_active() {
            if let Action::Produce { city, item } = action {
                let caller = std::panic::Location::caller();
                Some(serde_json::json!({
                    "city": city, "city_name": self.cities.get(city).map(|c| &c.name),
                    "item": item, "queue_before": self.cities.get(city).map(|c| &c.queue),
                    "model_gold_before": self.players[pid].gold,
                    "model_gold_per_turn_before": self.players[pid].gold_per_turn,
                    "log_before": self.log.len(),
                    "caller_file": caller.file(), "caller_line": caller.line(),
                    "caller_column": caller.column()
                }))
            } else { None }
        } else { None };
        let result = self.apply_without_income_probe(pid, action);
        if let Some(mut row) = before {
            row["ok"] = result.is_ok().into();
            row["error"] = serde_json::to_value(result.as_ref().err()).unwrap();
            row["log_after"] = self.log.len().into();
            self.income_probe_event("produce", row);
        }
        result
    }

    fn apply_without_income_probe(&mut self, pid: usize, action: &Action) -> Result<(), String> {""")
    write(name, text)

    name = "src/bin/civvis_orders.rs"
    text = read(name)
    text = replace(text, "    let mut planned_game = mirror_state.game.clone();", "    let mut planned_game = mirror_state.game.clone();\n    planned_game.set_income_probe_root(state.frame);")
    write(name, text)

    name = "src/ai/commercial_hub.rs"
    text = read(name)
    old = "    pub(super) fn commercial_hub_step("
    text = replace(text, old, """    #[track_caller]
    pub(super) fn commercial_hub_step(
        &self, g: &Game, pid: usize, cid: u32, n_cities: usize, traders: usize,
    ) -> Option<Item> {
        let result = self.commercial_hub_step_without_probe(g, pid, cid, n_cities, traders);
        if g.income_probe_active() {
            let caller = std::panic::Location::caller();
            g.income_probe_event("commercial_step", serde_json::json!({
                "city": cid, "cities": n_cities, "traders": traders,
                "threatened": self.commercial_city_threatened(g, cid),
                "window_open": self.commercial_hub_window_open(g),
                "result": result, "caller_file": caller.file(), "caller_line": caller.line()
            }));
        }
        result
    }

    fn commercial_hub_step_without_probe(""")
    old = "        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));"
    text = replace(text, old, old + """
        if g.income_probe_active() {
            let candidates: Vec<_> = ranked.iter().map(|(production, city)| {
                let item = Item::District { district: hub, pos: best_site(*city).unwrap() };
                serde_json::json!({"city": city, "production": production,
                    "item": item, "queue": g.cities[city].queue,
                    "can_produce": g.can_produce(pid, *city, &item),
                    "turns": Self::commercial_build_turns(g, pid, *city, &item)})
            }).collect();
            g.income_probe_event("commercial_ranking", serde_json::json!({
                "city": cid, "held": held, "open": open, "ranked": candidates
            }));
        }
""")
    write(name, text)

    name = "src/ai/advanced.rs"
    text = read(name)
    old = """            if committed.is_none()
                && self.commercial_hub_in_the_strategic_queue"""
    text = replace(text, old, """            g.income_probe_event("strategic_hub_reached", serde_json::json!({
                "city": cid, "committed": committed,
                "enabled": self.commercial_hub_in_the_strategic_queue,
                "window_open": self.base.commercial_hub_window_open(g),
                "threatened_city": plan.threatened_city,
                "military": counts.military, "cities": n_cities,
                "settlers": counts.settlers, "traders": counts.traders
            }));
""" + old)
    write(name, text)

    patch = ""
    manifest = {"base": BASE, "scope": "private diagnostic assembly only", "files": {}}
    for name, (before, after) in changes.items():
        patch += "".join(difflib.unified_diff(before.splitlines(True), after.splitlines(True),
                                             fromfile="a/" + name, tofile="b/" + name))
        manifest["files"][name] = {"before_sha256": hashlib.sha256(before.encode()).hexdigest(),
                                   "after_sha256": hashlib.sha256(after.encode()).hexdigest()}
    (args.out / "diagnostic.patch").write_text(patch)
    (args.out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(json.dumps(manifest))


if __name__ == "__main__":
    main()
