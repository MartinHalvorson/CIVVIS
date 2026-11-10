//! PROBE ONLY, never committed: how a played-on stock world fights to its last civ.
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args[1].parse().unwrap();
    let extra_cap: u32 = args[2].parse().unwrap();
    let wall_s: u64 = args[3].parse().unwrap();
    let dumps: u32 = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(0);
    civvis::lcs_probe::run(seed, extra_cap, wall_s, dumps, |line| println!("{line}"));
}
