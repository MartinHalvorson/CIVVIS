# Closed study writeups

Finished investigations whose questions are settled. They are preserved as
evidence — the same role `experiments/closed/` plays for executables. Nothing
here describes current behaviour; check the date and the ledger (`docs/EVAL.md`)
before treating a number in one of these as still true.

A writeup belongs here when the lane it studied is closed (shipped, rejected, or
superseded) and nothing live routes a reader here to learn what the code does
now. It moves back out if the lane reopens.

Being closed does not mean being unreferenced. A closed negative result is often
the standing reason a live decision holds, and a null nobody can find gets
re-run: `docs/AI_GAPS.md` cites `FAITH_CONVERSION.md` and
`TERMINAL_FAITH_OPPORTUNITIES.md` for exactly that reason, `docs/EVAL.md` cites
`LIVE_GENOME_TRANSFER.md`, and `src/sphere.rs` names `SPHERE_PERFORMANCE.md` as
the protocol its frozen microbenchmark implements. So when a writeup moves in
here, rewrite every citation to the `docs/closed/` path in the same change —
grep the whole tree, not just `docs/`. A citation that 404s is worse than a
writeup filed in the wrong folder.

The line that does matter: a live doc or tool may cite one of these as *what was
measured*, never as *what the code does*. A writeup that something still needs
in the second sense is not closed yet — leave it in `docs/`.
