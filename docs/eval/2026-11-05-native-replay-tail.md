# A stable final snapshot for ordered native-input replay

`civ6_captured_read_replay.py --delivery hybrid-tail` delivers the changing
prefix of each request through separate FIFO inodes, then publishes the final
snapshot as a regular file. The final contiguous run of identical recorded
reads uses that stable file. Entirely identical requests use the existing
regular-file delivery. The default `fifo` mode and existing `hybrid` mode keep
their original behavior.

This addresses a captured original-binary replay of the ordinary Indonesia
Science game `civvis-20261105T211703Z`. At request `19/0`, its first read contained
954,948 bytes and its following 17 reads contained 955,479 bytes. The original
hybrid delivery served all 18 FIFO snapshots but the decoder never returned a
reply before the 45-second deadline. It left an unwritten next FIFO at the
source path. The failure was retained; no policy-enabled board was queried.

A prototype publishing the stable final snapshot reproduced two independent
original-binary controls through turn 20: 35 requests per control. Every reply
matched the complete native payload, request identity, and actual ordered
captured bytes and counts. The source was
`352c0f0d26618a2c3b9da47b7a7a314c53f7ae73`, executable SHA256
`66c54b81327e5b309e2b64484b24b7061a3c78f2fde2aaf38a9af2d9438fbc79`.
This is evidence about input delivery, not a policy or native strength result.

The committed helper subsequently reproduced two original-binary controls
through turn 180: 463 requests and 8,050 actual captured reads per control,
with clean EOF. Independent reconstruction checked every original payload,
ordered captured byte sequence and count, executable and archive hashes,
arguments, working directory, and genome. Each control used regular files for
446 requests and a FIFO prefix with a regular tail for 17 requests.

That qualification used a prospectively specified 180-second request timeout.
An earlier 45-second regression remains a failure: after 392 matching requests
and 6,816 matching reads through `153/0`, request `153/1` timed out with only
7 of its 16 FIFO prefix reads delivered, each about 45 MB. The stable tail
does not remove prefix throughput costs, and these results do not establish a
full-history pass or reliable completion at the shorter timeout.

A regular file cannot acknowledge consumption. `FifoReads.served` therefore
returns `None` when a regular final snapshot is present; `prefix_served` counts
only the FIFO prefix. Delivery completion proves the prefix was drained and
the regular tail was published, not that the consumer read the tail. The full
recaptured byte sequence and count still have to match, together with frame
identity, the complete decision payload, and successful process exit.

Independent child-process tests read the actual path, construct their own
captures, and verify rejection of missing or extra tail reads, changed decision
payloads, and incorrect frames. A separate delivery test permits an extra
regular-tail open and confirms that the feeder does not claim a consumption
count. No timing or returned-payload equality substitutes for the captured-read
gate. All replay output stays in the supplied private output directory.
