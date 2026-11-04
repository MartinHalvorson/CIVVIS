# Keep quality-check ancestry without eagerly fetching historical blobs

PR #4424's initial quality job timed out after 30m8s in checkout, before code checks. The local repository has 14.59 GiB of packed history; the current tracked tree totals 802,972,860 blob bytes. These measurements motivate reducing historical-blob transfer; they do not establish a hosted timing result.

The quality workflow adds `filter: blob:none` to `actions/checkout@v4`, preserving `fetch-depth: 0`. The full commit graph remains available to the existing merge-parent discriminator. The checked-out tree and historical Rust blobs needed by the existing diff are fetched on demand. The quality command and its checks are unchanged. This uses the official [checkout v4 filter input](https://github.com/actions/checkout/blob/v4/action.yml) and [Git partial-clone behavior](https://git-scm.com/docs/partial-clone).

Thirteen focused tests pass, including a real full-versus-filtered Git fixture. It checks identical quality bases, changed Rust files and line ranges for a synthetic PR merge with a stale supplied base, an ordinary push, and a local merge with reversed parents. An obsolete binary stays absent; needed historical Rust content is fetched. An unavailable historical Rust fetch raises an error. The fixture also checks workflow wiring.

The JSON pins the source checkpoint and file/log hashes. No simulator or native strategy behavior changed; engine soak and local Rust compilation are not applicable. Hosted CI still has to validate the final head, and there is no controlled network or whole-job speedup claim.

The broader local tooling run executed 3,658 tests in 345.654s: three failed and one skipped. The roadmap hotspot check reproduces on unchanged main; the database-source and foreground timing cases pass in isolated rechecks on both trees. The relevant source/document files are byte-identical to main. Those failures remain explicitly reported; this is not a clean full-suite claim.
