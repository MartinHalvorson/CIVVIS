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


BASE = "45eb06d2bea6bde2462e092aac6d0ed778a2c367"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--repository", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--trace-through-turn", type=int, default=70)
    args = parser.parse_args()
    assert args.trace_through_turn >= 60
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
    text = replace(text, "(60..=70).contains(&self.turn)",
                   f"(1..={args.trace_through_turn}).contains(&self.turn)")
    write(name, text)

    name = "src/bin/civvis_orders.rs"
    text = read(name)
    text = replace(text, "    let mut planned_game = mirror_state.game.clone();", "    let mut planned_game = mirror_state.game.clone();\n    planned_game.set_income_probe_root(state.frame);")
    write(name, text)

    patch = ""
    manifest = {"base": BASE, "scope": "private diagnostic assembly only",
                "trace_turns": [1, args.trace_through_turn], "files": {}}
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
