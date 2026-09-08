# Mirror browser control

The live mirror follower (`tools/follow.py`) serves its board without managing
Chrome by default. Starting or recovering a follower does not authorize it to
open windows, navigate tabs, or reopen a browser the operator closed.

Open the mirror manually at `http://127.0.0.1:8610/` while its server is running.
For an unattended display that should restore its window, explicitly opt in:

```sh
printf 'managed\n' > ~/.civvis-mirror-browser-intent
```

Return to manual browser control without stopping the game or HTTP mirror:

```sh
printf 'manual\n' > ~/.civvis-mirror-browser-intent
```

The follower reads this setting on each browser action, including the final
AppleScript dispatch. No restart is needed to change intent. Only `managed`
enables automation; a missing, invalid or unreadable file means manual control.
`CIVVIS_MIRROR_BROWSER_INTENT_FILE` can select a different intent file for an
isolated session. Managed mode intentionally restores closed mirror windows;
switch back to manual before closing Chrome if it should remain closed.

## Existing installations

An older follower already in memory cannot read the new setting. Stop its
`civvis-mirror-keeper.sh` process first, then its `tools/follow.py` processes,
and launch the updated follower only when needed. Inspect process command
lines before stopping them; multiple followers can survive their original
Terminal and even the deletion of their checkout.

On 2026-09-08 the Mac host had two such orphaned followers and a mirror keeper.
Both followers logged “mirror window is not on screen; restoring it” after
Chrome was force quit. They were stopped, and the local launchd jobs
`com.civvis.keepplaying`, `com.civvis.ladder-watchdog`, and the retired
`com.civvis.recover-cont2-20260903` were disabled to prevent desktop-session
recovery. The headless spectator was left running. Those launchd settings are
host state, not files to copy between machines; this versioned opt-in prevents
updated followers from reopening Chrome even if a recovery job starts them.

Keep Civvis fixes and optimizations in GitHub through the repository's task
branch and validated PR workflow. Record operational fixes in versioned tools
or documentation as well, so behavior does not depend on an unrecorded local
repair. Never sweep another task's uncommitted files into an unrelated sync.
