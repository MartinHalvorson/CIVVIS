# Russia: the next faith civic and native religious outcome measurement

The existing `religion-seat-reaches-reformed-church` opt-in changes the
post-Theology civic path on captured Russia inputs. This is a planning
diagnostic, not a measured native strength improvement. No deployment default
or host experiment setting changes here.

## Native control

`civvis-20261109T183445Z` played Russia/Peter, Gathering Storm, Deity,
six-player Small Continents, Online speed. Its frozen executable was built from
`c591f8ec9fdd9ac74711ac98c479f4948a536123`, SHA-256
`6ae3850fa4f7c9b045afc619f1f8f21c2c04ff8b8544f7f8e23d6ef298f5267e`.
The bytes match the recorded native executable identity. The run had the
Apostle and Holy Site treatments active; the Reformed Church treatment was
absent from the runtime treatment list.

The native seat founded at t22, completed Theology at t47, and had its first
Temple and Apostle at t51. The host verified the t51 Apostle Faith purchase
on t52. It lost to a rival Diplomatic victory at t166, with ten owned cities
following Buddhism, fifteen world cities following Buddhism, and no positively
reported rival majority following Buddhism. One rival's null majority is
ambiguous; it is not evidence that the rival was unconverted. The final
government was Communism, without Reformed Church or Religious Orders.

## Fixed captured-input comparison

Both arms use that same frozen native executable, the same forced treatment
list and the same captured input bytes. The on arm adds only
`--with religion-seat-reaches-reformed-church`; the off arm omits it. Runtime
`treatments` readback confirms the intended membership in both arms.

Each original decision recorded eighteen identical reads of one snapshot.
The snapshots below were reconstructed with `civ6_decision_trace.captured_input`
from the actual capture archive. Each arm is a fresh `--resumed --explain`
one-shot invocation, with `--players 6 --victory religious`. It does not
reproduce the persistent controller's earlier decisions, and its proposed
orders were never sent to the host.

| Turn, frame | Off civic | On civic | Interpretation |
| --- | --- | --- | --- |
| 47, 0 | Games and Recreation | Games and Recreation | Shared prerequisite; the goal journal differs. |
| 67, 0 | Divine Right | Divine Right | Shared prerequisite; no immediate order difference. |
| 75, 0 | Medieval Faires | Guilds | The Reformed Church goal changes the next prerequisite. |
| 94, 0 | Mercantilism | Reformed Church | The on arm directly requests the missing faith civic. |
| 164, 0 | Suffrage | Reformed Church | The goal still applies late; both arms propose Communism on this input. |

The t164 input has not unlocked Theocracy. These proposals do not establish
how government selection behaves after Reformed Church completes, or whether
the general policy-capacity preference would then displace Theocracy.

| Turn | Input bytes | SHA-256 |
| --- | ---: | --- |
| 47 | 6,279,766 | `fa613fea1696d810398cfeee5e60919ffa0d9887d885d6f94930b0dd827446f0` |
| 67 | 12,273,915 | `c24930130babf89bd6845b50faf65ea9e279aa0b85172f084a262edf9dfcaee3` |
| 75 | 15,048,247 | `729c6bbe7b412e452ce5f2476bdb777a255fd5d8be860affead6090620e7242f` |
| 94 | 22,713,396 | `ac550a6e15ad99db2b9d5c97060ff898a0ddd95d4473e2b21316fd73249cb002` |
| 164 | 64,031,478 | `19ef515cd1cdc4ee84b54fa1b53c503fefd997f14254504720292792eb0fda61` |

Local sealed inputs, arm outputs, logs and machine-readable comparison:
`~/civvis-tactics-results/2026-11-09/russia-reformed-church-diagnostic/captured-reads/`.
An earlier end-of-turn-prefix diagnostic selected states with civic research
already active; its empty civic proposals are not credited as goal decisions.

## Measure the requested victory

The new reader reports one game per continuation chain, explicitly distinguishes
our Religious victory from other wins, and keeps retirements and incomplete
endings censored. It preserves each segment's screen label, source revisions
and reported Reformed Church membership rather than inferring activation from
the requested command line. First-observed milestones are observations, not
guaranteed completion times when a segment is missing.

```sh
python3 tools/civ6_religion_report.py ~/civvis-civ6-runs/control \
  --since 2026-11-09 --civ CIVILIZATION_RUSSIA
```

The local snapshot at approximately 19:14 UTC contains 24 Russia religious-lane
games after continuation joining: nine observed losses and fifteen censored
endings, with no observed wins. Seven losses ended on Technology, one on
Diplomacy, and one on our elimination. The censored endings include thirteen
automatic abandonments and two operator retirements. These games span source
revisions and treatment configurations; this is not a comparison of the
Theology change or the Reformed Church option. Censored games are not extra
losses, and these counts are not an unbiased win-rate estimate.

The reader retains world-city following counts separately from positively
reported rival majorities. Missing rival/unit exports remain null rather than
becoming zeros. A rival null majority retains the native ambiguity. Reports
never infer a Religious victory from either progress count; only the native
terminal outcome establishes it. The final continuation supplies the final
state, including an explicit null when its state evidence is absent. Partial
JSON lines are counted so a readable prefix does not silently imply complete
evidence. It also reads gzipped pulled-ledger events.

## Native rule and remaining experiment

Installed Firaxis `Base/Assets/Gameplay/Data/Policies.xml:179` assigns
`POLICY_RELIGIOUS_ORDERS` the prerequisite `CIVIC_REFORMED_CHURCH` and economic
slot type. The next row assigns the same civic prerequisite to Simultaneum.
This report makes no flat Faith-yield claim for Simultaneum across rulesets.
The native exporter's rival-majority read cites shipped
`WorldRankings.lua:2049`, `GetReligionInMajorityOfCities`; the world-city count
uses `GetNumCitiesFollowingReligion` as in `WorldRankings.lua:44`.

The native screen file was already assigned to `culture-cold-war-window` when
inspected. It was preserved, along with the retirement and victory-lane
policies. No Reformed Church native screen was started by this task.

The remaining experiment is a native Reformed Church on/off comparison under
the same Russia religious-lane configuration, recording actual completion,
government/cards, rival majorities and explicit Religious outcomes. An
interleaved randomized screen is distinct from a matched-seed comparison.
Neither can be replaced by these one-shot proposals, and the treatment stays
opt-in pending evidence.
