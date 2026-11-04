"""Add a qualified, read-only sanctuary admission query to the root probe."""
from pathlib import Path
import argparse, difflib, hashlib, json, subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--source', type=Path, required=True)
parser.add_argument('--repository', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
name = 'src/ai/advanced/adopted_faith_sanctuary.rs'
original = subprocess.check_output(['git', 'show', '45eb06d2bea6bde2462e092aac6d0ed778a2c367:' + name], cwd=args.repository)
assert (args.source / name).read_bytes() == original
code = original.decode()
anchor = '''        let second_source = founded.is_some() && self.founder_wants_a_second_source(g, pid);
'''
assert code.count(anchor) == 1
probe = '''        if g.income_probe_active() {
            let owned: Vec<_> = g.player_city_ids(pid).into_iter().map(|city_id| {
                let city = &g.cities[&city_id];
                let faith = g.city_religion(city);
                let safe = faith.is_some_and(|faith| {
                    if let Some(own) = founded { faith == own }
                    else { faith != threat && self.counterfaith_is_safe(g, pid, faith) }
                });
                let shrine = city.buildings.iter()
                    .any(|b| g.building_is_family(b, crate::name!("shrine")));
                serde_json::json!({
                    "city": city_id, "name": city.name, "pos": city.pos,
                    "faith": faith, "safe_for_this_threat": safe,
                    "queue": city.queue, "free_for_production": free(&city_id),
                    "queue_held": self.sanctuary_queue_held(g, pid, city_id),
                    "has_shrine": shrine,
                    "has_holy_site": g.city_has_district_family(city, crate::name!("holy_site")),
                    "missionary_faith_quote": g.unit_purchase_cost(pid, city_id, "missionary", "faith")
                })
            }).collect();
            g.income_probe_event("sanctuary_admission", serde_json::json!({
                "threat": threat, "founded": founded, "threatened_city": threatened,
                "eligible": eligible, "second_source": second_source,
                "one_sanctuary": self.one_sanctuary,
                "sanctuary_yields_a_held_queue": self.sanctuary_yields_a_held_queue,
                "faith_bank": g.players[pid].faith, "owned": owned
            }));
        }
'''
code = code.replace(anchor, anchor + probe, 1)
args.out.mkdir(exist_ok=False)
(args.source / name).write_text(code)
(args.out / 'sanctuary-query.patch').write_text(''.join(difflib.unified_diff(
    original.decode().splitlines(True), code.splitlines(True), fromfile='a/' + name, tofile='b/' + name)))
manifest = {'scope': 'Observational query only on actual root Game. Adds immutable eligibility/source/price queries; no action or selection override. Require complete payload/read/trace controls.',
            'file': name, 'before_sha256': hashlib.sha256(original).hexdigest(),
            'after_sha256': hashlib.sha256(code.encode()).hexdigest()}
(args.out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
print(json.dumps(manifest))
