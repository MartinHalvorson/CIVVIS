# Preserve known zero strategic stock readings

The native exporter now retains successful finite nonnegative
`GetResourceAmount` readings, including zero. Previously its `amount > 0` filter
discarded a valid zero. If every stock was zero, the whole `strategic_resources`
field disappeared; a reader could not distinguish that from unavailable APIs.

Each failed or invalid query remains omitted. If every query is unavailable,
the field remains nil. Successful zeros make a nonempty keyed object, avoiding
the encoder's empty-table array shape. Getter calls, arguments and resource-class
filter are unchanged. Installed
`DLC/Expansion2/UI/Replacements/ReportScreen_Expansion2.lua:115` also calls
`GetResourceAmount(row.ResourceType)`. No new native API argument was probed.
Historical missing fields keep their unknown interpretation.

## Validation

The regression executes the actual production export expression with mocked
host calls. The original passes 12 checks and fails four; the candidate passes
all 16. Cases cover mixed positive/zero stocks, all-zero banks, partial failures,
all-unavailable reads, invalid values and unchanged call arguments. The complete
production Agent compiles under Lua 5.1; all 115 discovered Lua test files and
139 installer tests pass.

A separate production-encoder check emits
`{"RESOURCE_COAL":0,"RESOURCE_IRON":0}` and parses it back as two numeric zeros.
That is an offline mocked readback. Native stock-map readback after deployment
is still pending. No deal, fuel delivery, policy/default promotion or strength
effect is claimed.

## Evidence

The [adjudication](2026-11-03-strategic-stockpile-zero.json) records source hashes
and encoder validation. The [archive](2026-11-03-strategic-stockpile-zero.raw.tar.xz)
contains before/candidate source and identical regressions, original failing and
candidate passing logs, full Lua/installer logs, SDK reference rows, patch and
`reproduce.py`. It has 14 members, 346,088 bytes, SHA-256
`eff2da5df7ff61ca1ad474f2c608de7a75f7828e397c6e376a186c5180e65ffb`.
The per-member manifest binds every other member; `python3 reproduce.py` with
`lupa.lua51` reproduces the 12/4 versus 16/0 comparison.
