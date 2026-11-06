These are the versioned sources for the macOS recording install at
`~/.local/share/civvis-clip-recorder`. The story and publisher use the existing
installed keeper, video quality gate, and upload helpers. `victory_state.py`
from the parent tools directory must be installed alongside them.

Build the macOS renderers:

```
swiftc -swift-version 5 -O youtube_master.swift -o civvis-youtube-master
swiftc -swift-version 5 -O join_victory_tail.swift -o join-victory-tail
```

`repair_victory_endings.py VIDEO... --renderer PATH --joiner PATH` preserves
original videos and sidecars under `~/civvis-civ6-runs/clips/repairs/`, remuxes
the existing gameplay prefix, and renders the last twelve seconds plus a
five-second final hold. The terminal card starts at the recorded victory time.
The receipt distinguishes a confirmed winning minimum from measured counters,
checks original-prefix pixels, and points to an ending preview. The old raw
recording and event journal are untouched. No upload is performed.
