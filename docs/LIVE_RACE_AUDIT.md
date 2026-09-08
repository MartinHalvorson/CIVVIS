# Live race evidence

`python3 tools/civ6_race_audit.py RUN_DIR [RUN_DIR ...] --out report.json`
reads retained `summary.json` and `events.jsonl` (also `.gz`). It changes no
game, tournament setting, deployment default, or historical ledger row.
New ladder records automatically retain its race observations, boost summary,
full treatment identity, seat settings, turn cap, and seed provenance.

The report follows Writing through the launch technologies, completed and
unpillaged Campuses and Spaceports, Libraries, launch projects, and accelerated
flight. Exact-turn checkpoints record empire size, science, culture, production,
and the current research/project state. Per-city observations expose the delay
between acquiring a city and its economic development. Idle city-turns deduplicate
mid-turn frames. Boost observations survive the host removing a boost on completion.

Every date is a first observation. A milestone already present when a resumed
segment begins is explicitly censored; it is not claimed to have completed on
the resume turn. A missing turn-100 frame cannot become a turn-100 measurement
using turn 175. A boost never observed is not proof it never fired: the report
retains first-frame and missing-turn coverage for this reason.

Continuation suffixes group segments of one game; capture-free attempt numbers
remain separate games. The comparison gate requires a retained opening,
verified settings, an actual victory or own elimination, complete profile and
treatment provenance, and one unchanged revision and binary across segments.
It excludes interruptions and changes of controller instead of counting them
as losses or attributing their outcome to the last controller. Cohort hashes
separate settings and controllers; compare explicitly selected baseline and
candidate cohorts, with fresh maps and the same profile. These are descriptive
records, not randomized treatment estimates. Keep seeds for paired comparisons
where the host actually reports them; never infer paired maps from run names.

For a bounded live comparison, pin one baseline and one candidate revision and
their effective treatment sets, alternate complete games with matching settings,
and retain every outcome. Read wins first and milestone, city-retention, boost,
and execution metrics as explanations. Do not retire games based on these
diagnostics or change a healthy game's controller to obtain a comparison.
