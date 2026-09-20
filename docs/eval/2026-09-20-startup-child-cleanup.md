# Failed startup child cleanup

The native recovery `civvis-20260920T080404Z-cont1` exhausted its 420-second
main-menu allowance. Its game child, PID 79010 (controller PID 78997), survived
the normal SIGTERM shutdown. Because startup wrote no readable run tag, the
batch correctly refused its generic teardown and stopped further recovery:

> cleanup for civvis-20260920T080404Z-cont1 was not proven; not launching a continuation over a surviving game

The operator's recorded parent/child identity allowed that particular orphan
to be removed later. Requiring manual reconstruction is avoidable: `launch`
already returns the owned `Popen` object, but the caller discarded it.

The launcher now retains that object during the main-menu wait. A failed or
interrupted startup terminates only that child, waits five seconds, then kills
and reaps it if necessary. Successful menu readiness releases it to the normal
game controller. The verification controller, restart helper, and launcher CLI
use the same wrapper. The scope ends before setup can load a save or start a
game; normal game shutdown and the batch's foreign-run guard are unchanged.
There are no Firaxis gameplay API or control-mod changes.

Validation uses disposable real Python children, including one that ignores
SIGTERM, alongside an unrelated live child. The failed-startup and interruption
cases failed on the original restart path; all five tests pass after the fix.
They also cover successful readiness and a child that already exited. The
broader `test_civ6*py` suites pass: 1,432 tests, one skipped. The full
`cargo test --profile ci --locked` suite passes: 3,665 library tests and
204 binary tests (49 library and four documentation tests ignored). Repository test
discovery includes the new `test_civ6_startup_cleanup.py` suite automatically.

This does not repair why a native launch stalls. It prevents a failed startup
from leaving its owned process behind and blocking the next recovery attempt.
The currently healthy native game is left running; its process is not a test
fixture for forced termination.
