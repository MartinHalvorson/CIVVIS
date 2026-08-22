# CIVVIS experiment preregistrations and loose research notes

Archived 2026-08-22 from `~` and the loose `~/civvis-*` result directories on
`mbp-m5-max-128`. Every file here failed `git cat-file -e $(git hash-object
FILE)` — this machine's disk was the only copy of all 46 of them.

They are kept on this ref rather than merged into `main` because they are a
historical record, not code: 29 preregistrations written *before* their
matrices were run (the anti-p-hacking record behind `docs/GENE_SCREEN.md` and
the closed lanes in `docs/closed/`), the evolver's own synthesis, and five
loose notes.

Nothing here is derived from anything tracked; deleting the local copies is
safe once this ref exists.

| directory | contents |
|---|---|
| `preregistrations/` | 29 pre-registered experiment designs, 2026-07-26 → 2026-08-18 |
| `evolver-results/` | the evolver's 10 preregistrations, `SYNTHESIS.md`, champion PR body |
| `notes/` | `NOTES-20260731.md` (435 lines), the stopped-batch note, the PR #504 war-conversion plan, two directory READMEs |

Retrieve with:

    git fetch origin refs/civvis/archive/preregistrations-20260822
    git checkout FETCH_HEAD
