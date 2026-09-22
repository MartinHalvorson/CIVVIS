# Religious interception must precede conversion of the defending empire

## Native evidence

`civvis-20260922T051229Z-cont2` lost to Arabia's religious victory on turn 131, using source `9d8c98737c230390a97ad7c650c55b1506d296b6`. The war with Arabia remained open through the loss. Earlier, all seven of our cities followed Islam by turn 79. No alternative faith or religious unit survived to restore a recruiting source.

Arabia's cities were undiscovered at turns 79 and 84, although its Missionaries were visible. The military denial selector requires a known rival city. This initially suggested opening a religious interception without a city objective: a turn-83 Missionary was beside a Spearman, and a simulator fixture demonstrated that the existing diplomatic planner would not act.

## Rejected late interception

The proposed turn-83 counter is not supported by the native rules. The shipped `Base/Assets/Text/en_US/InGameText.xml:3141–3142` defines `LOC_UNITCOMMAND_CONDEMN_HERETIC_FOLLOWING_THIS_RELIGION` as: "A Religious unit in this tile adheres to the dominant Religion in a majority of your cities." Lines 3138–3139 separately describe a unit following the religion the defender founded. `Base/Assets/Gameplay/Data/UnitCommands.xml:47` registers Condemn Heretic as a command.

Our simulator's `do_condemn_heretic` checks military/religious classes, war, shared position and remaining movement, but lacks these faith restrictions. Its successful speculative condemnation would therefore not establish that a native interception is executable. The initial test fixture explicitly gave our empire an Islamic majority: that invalidates its claimed native counterexample. The red regression (one failed, one control passed) and uninstalled candidate are investigative artifacts, not validation of a shippable policy. No production change is retained from that proposal.

## Earlier window to investigate

The recorded game exposes possible pre-majority contacts: a Spearman was adjacent to an Arabian Missionary on turn 56, and a Heavy Chariot was adjacent on turn 61 while only one of four cities followed Islam. Those are geometric observations, not verified legal interceptions. The Cree war was already active; opening a second front has a strategic cost. Any revised policy must establish native faith eligibility, actual movement and command legality, and a credible benefit from diverting forces or changing fronts before promoting it.

The generic match-point response is too late to supply a non-founder's own religious counter once every recruiting city has converted. Founding or preserving another religion and pre-majority military interception remain alternatives to evaluate. Neither this investigation nor a simulated condemnation proves an avoided native loss or a Domination victory.
