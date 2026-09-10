# Test an earlier Rock Band window for Culture

`culture-cold-war-window` is an opt-in strategy experiment. It keeps Humanism
and Conservation first, then asks for Cold War before Professional Sports and
Cultural Heritage. The existing path reaches Cold War later as a prerequisite
of Space Race. Earlier government and Great Person goals keep their priority.
The band purchase pass, its Faith prices, and its active-band cap are unchanged.

The hypothesis is that opening a spendable Faith-to-tourism path earlier can
finish a Culture race sooner than the stadium and museum-tourism detours. It
can also lose: earlier Cold War consumes culture that could improve persistent
tourism, and a seat with little Faith or poor venues may get no useful concerts.
This is a choice to measure, not a correction that should silently become the
default.

The host unlock is grounded in
`DLC/Expansion2/Data/Expansion2_Units.xml:42`, whose Rock Band row names
`PurchaseYield="YIELD_FAITH"` and `PrereqCivic="CIVIC_COLD_WAR"`. The engine's
`data/civics.json` makes Cold War and Professional Sports siblings under
Ideology. No rules, purchase costs, or prerequisites are altered here.

Regression coverage checks the legal civic order through `advanced_research`,
including an explicit Culture target while the temporary plan is Expansion.
The control selects Professional Sports; the treatment selects Cold War.
The Science target is unchanged, the early Humanism/Conservation sequence
remains intact, and disabled Culture victory falls back to the old order.

## Evaluation protocol

Use one recorded binary and identical seeds for an all-Culture reachability
comparison with `victory_eval --target culture --deployment`, enabling only
this gene in the treatment. This evaluator targets every major at Culture,
uses no barbarians or city-states for that target, and does not accept a
difficulty flag. Its result is a simulator reachability comparison, not a
Pericles-versus-Emperor win-rate estimate.

Also run a standard-shape `gene_screen` batch at Emperor, varying this gene
among the deployed background's adaptive seats. Record the binary hash,
seeds, shape, exposure counts, complete outcomes and uncertainty. A small
screen is diagnostic and does not justify promotion. Do not pool it with
live Pericles outcomes or the all-Culture reachability comparison.

Measurements are pending. The gene remains off by default.
