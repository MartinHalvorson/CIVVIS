# Exact-source Apple Silicon native build artifacts

Native candidate builds on the active game host took about an hour before
their focused test compilation began. During that interval the private
publisher advanced, so adopting the older build would discard newer code.
This creates a validation delay in the Domination improvement loop.

The existing release workflow now accepts an optional `native_source_sha`.
An empty value preserves its ordinary release build. A forty-character SHA
selects a separate `macos-15` job with read-only repository permissions and
no release publication. It checks out the workflow helper separately from
the requested candidate, builds both native executables with the host's fast
profile, runs explicitly selected Rust tests, and smoke-tests the packaged
simulator. Compiler version, every tracked source file, selected test counts,
architecture and executable checksums are recorded in the artifact receipt.

The standard Apple Silicon runner is available to public repositories without
runner charges: [GitHub runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
The explicit toolchain input uses the action's `master` interface as required
by its [toolchain-input documentation](https://github.com/dtolnay/rust-toolchain#inputs).
The first requested compiler is `1.97.1`, matching this verification host;
the receipt records and checks what actually ran. One Cargo job limits memory
pressure on the smaller hosted runner. Only dependency downloads are cached.

Example dispatch after the workflow is available on the selected branch:

```sh
gh workflow run release.yml --ref <workflow-branch> \
  -f native_source_sha=<exact-candidate-sha> -f native_toolchain=1.97.1 \
  -f 'native_tests={"lib":["siege_train::muster_tests","find_the_capital::tests"]}'
```

After downloading its `native-arm64-<sha>-<run-id>` artifact, verify it against
the unchanged local candidate using `tools/native_build_artifact.py verify`
with the same SHA, compiler and test selection. Downloaded executables need
their executable bit restored before local smoke tests. Live adoption remains
a separate step: recheck the latest private publisher, preserve forcing and
challenge settings, and verify the actual next attempt's source and binary.

The first hosted run, [38676990169](https://github.com/MartinHalvorson/CIVVIS/actions/runs/38676990169),
completed successfully in 33 minutes 34 seconds. It built exact source
`652c056a7f43590e7d4a9ec3b30849921954adc4` with Rust 1.97.1 and passed
16 muster, 17 capital-recon and 11 existing open-city controls. The ordinary
release matrix and publishing job were skipped. After download, all 2,513
tracked source hashes and both executable hashes matched the receipt; the
actual downloaded simulator completed a local twenty-turn smoke run, and
the downloaded decider answered a nonempty turn-one/frame-zero input.

The nine local metadata/architecture guards, seven CI-wiring controls and
twelve workflow-report controls passed. A separate native army-policy build
was dispatched on the same fixed workflow revision. Workflow runs keep their
dispatch commit; advancing the PR after its checks finish does not rewrite
that running job's automation or requested source.

This workflow supplies build and test evidence. A native verification game
still needs the verified executable, complete current publisher parent,
forcing and settings read back from the actual new attempt. No native
purchase acceptance, Domination win-rate increase or highest-level victory
is established by the artifact or its smoke tests.
