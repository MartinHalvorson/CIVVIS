# Domination aircraft preparation across peace

Native match `civvis-20260922T013415Z` and its continuations used source `590641be9305e94fd42020f26dc93e07e3053198`. Its air appointment ended at peace on turn 158. Flight arrived at 162, Radio at 164 and Advanced Flight at 184, but every city lacked a legal airfield slot at 172. The first two Aerodromes began around 203; no Bomber was observed through 208. The initial slot-reservation fix changes 25 exported and 26 internal-action frames in a 477-frame replay of this game, but its commitment still depends on a current war appointment.

This prototype keeps a small aircraft preparation program for an industrialized Domination seat without requiring a war target. It preserves the research goal and one productive airfield slot, claims an idle queue for the first base, then prepares two supplied Bombers. It counts queued aircraft and newer bomber generations. Immediate home threats suspend preparation, existing queues are not cleared, and aircraft require a treasury reserve for upkeep. It does not open a war or retain a peace-ended war appointment.

Validation and paired native replay are pending. This is not evidence of earlier completed aircraft, captures or a native victory. Local evidence is under `/tmp/civvis-013415-airfield-replay` and `/tmp/civvis-013415-continuations-audit.json`.
