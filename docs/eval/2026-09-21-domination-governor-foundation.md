# Economic governor foundation after religious defense

The completed native Domination runs `civvis-20260921T150141Z` and `civvis-20260921T152749Z` invested their eighth Governor title in Moksha's Patron Saint, at turns 102 and 130 respectively. Neither had appointed Pingala; neither had a Temple. The first run lost to German Culture on 154. The second lost to Byzantine Religion on 167. These are losses, not strategy successes.

The first run had 11 cities but only 48.58 culture per turn at 140; its domestic tourists stayed at 32 through the finish while Germany's foreign tourists rose from 33 to 85. The second had eight cities and 21.84 culture per turn at 140. Increasing culture late cannot be assumed to save either game, but spending scarce titles on an Apostle promotion without a source of Apostles leaves an immediate economic opportunity unused.

The change is limited to an explicit Domination target using the existing non-founder conversion-defense governor priority. Finish Moksha's Citadel of God first. While no unpillaged Temple exists, prioritize appointing Pingala and obtaining Researcher and Connoisseur before the remaining Moksha promotions. Existing incomplete governor foundations retain their priority. Reassess after each title so a batch of titles returns to the normal sequence once Pingala has both promotions. Founders, other victory lanes, disabled conversion defense, and empires with a Temple retain their previous priority.

Validation: 4,062 Rust tests passed, 53 ignored; all five focused governor regressions passed; 14 treatment append-point checks passed; eight four-player, 180-turn soak games completed; formatting and incremental Rust quality passed.

Paired full-history replays against source baseline `12696fa41` covered 437 decision frames through 154/2 in the first match and 469 through 167/2 in the second. The final candidate changes exactly one actionable decision in each: at 102/0 and 129/1 it appoints Pingala instead of promoting Moksha to Patron Saint. Each replay also changes the following verification receipt. All other orders and internal actions are unchanged. The second native appointment/promotion is first visible in the turn-130 snapshot.

The 19 forced treatments in the replay were checked against each native summary and match exactly. Local artifacts: `/tmp/civvis-governor-foundation-replay/`, `/tmp/civvis-governor-foundation-second-replay/`, `/tmp/civvis-3669-comparison-final.json`, and `/tmp/civvis-3669-comparison-second.json`.

 Recorded-state replay measures changed choices; it cannot simulate the resulting economy or establish a native victory. In particular, the original next-turn snapshot will report a counterfactual appointment as absent because the original game bought Patron Saint.
