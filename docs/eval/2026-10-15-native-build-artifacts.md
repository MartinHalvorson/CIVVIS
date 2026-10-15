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

The metadata guards are tested locally. The first hosted execution is pending.
This workflow supplies build and test evidence; it establishes no native
purchase acceptance, Domination win-rate increase or highest-level victory.
