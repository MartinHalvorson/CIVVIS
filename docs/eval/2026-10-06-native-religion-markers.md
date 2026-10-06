# Native religion markers and false conversion forecasts

In `civvis-20261006T061051Z`, Gran Colombia lost to Mongolia's Religious
victory at turn 108. At turn 107, Bogotá and Caracas remained Buddhist after
one Confucian Missionary charge each. Maracaibo and Valencia also remained
Buddhist after an enemy Buddhist Missionary was condemned. The reconstructed
game instead converted the first two cities and removed the latter two
majorities. These four observations come from one history, not four games.

The mirror imports majority and impending-conversion warnings as synthetic
pressure values (100, 60, 50 and 1). They are not measured pressure. The shipped
`DLC/Expansion2/UI/CityBanners/CityBannerManager.lua:1938–1950` reads
`cityReligion.Followers` independently and assigns
`LifetimePressure=cityReligion.Pressure`. Arithmetic on our warning values
cannot predict a native conversion or the number of charges it requires.

The correction records which cities have these imported markers. Modeled
spreads still consume charges and movement; condemnation still removes its
target. Spreads and religious combat retain those cities' observed religious
state until a subsequent native snapshot reports an update. The marker set
survives save/load and AI clones, and is cleared with the other city
observations. Ordinary simulated cities and older saves continue to simulate
accumulated pressure.

The conservative forecast may retain a majority that a real subsequent action
would change. Actual native pressure and follower exports would be needed to
predict such within-frame conversions. Growth, founding and inquisition are
outside this correction's scope.

Prior exact-source research used public revision
`0f4b4e1224807d9851a8babb465b4af3e11a1445` and native private revision
`ec2424ff7c567f578413bcb27879dcbb899b496b`. Six spread and four condemnation
regressions failed before the corresponding fixes; all 16 controls passed
afterward, including payment of charges/movement, legal refusals, later native
updates, save/load, AI clones and unchanged unmarked simulation.

The fixed-history native replay covered 288 frames. Five complete replies
changed, beginning at turn 106 frame 0; each of those original replies had
matched the recorded native decision. The first-change fidelity gate passed.
Four city-majority component comparisons matched the next native observations.
The condemnation guard added no further complete-reply changes over the spread
guard in this history. These are model and decision checks, not execution of
the candidate in Civilization VI, survival estimates or win-rate evidence.

Original native input SHA-256:

- `events.jsonl`: `4786d4dd6026638220bb3fea1d9447e811213460decee713b6cffae6f991d399`
- `decisions.jsonl`: `c0cc0bc561cee8163fc9309d7bac02b4afdf2a219e8fe634e33372d92b15c0c2`

The complete local receipts remain in
`civvis-tactics-results/2026-10-05/culture-building-reservation/religious-defeat-108-audit`
on the owning host. Earlier full-suite attempts there failed on 18 unchanged
socket permission tests; those failures were not waived. This PR reruns the
required full suite in its own isolated worktree with the current execution
environment, followed by independent GitHub checks.
