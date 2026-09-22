# Keep an urgent score counter in the Domination campaign

Native `civvis-20260922T041757Z`, pinned to `453093af81ea2986843474f2fd421e11be309dc4`, took Germany's original capital Munich on turn 225. It held Munich at the end but lost on score to China at turn 251. After peace with Germany, the explicit Domination seat assessed Expansion in response to a rival's score threat, despite holding an army much larger than any rival. The generic counter-in-lane policy was answering score with expansion.

An exploratory 700-frame replay through turn 237 disabled counter-in-lane. It changed 15 exported and internal-action frames, beginning at 232/2, and kept the late plan in Conquest. The army still failed the staging gate for China. Enabling the existing capital-focus option on top of that experiment changed no frames, so that option is not promoted here. These recorded-board experiments demonstrate policy routing, not new native captures or a prevented loss.

The proposed change is narrower than the exploratory switch: only an urgent score threat against an explicitly assigned Domination seat chooses Conquest instead of Expansion. Existing pressure eligibility, optional stand-down, target legality and declaration readiness still apply. Other target contracts, Science counters and eligible but nonurgent score alarms retain their policies.

## Validation in progress

The actual score-leader regression fails on the base because the actionable denial is Expansion rather than Conquest. Its control covering other target contracts and existing pressure gates passes. The candidate's focused checks, full suite and isolated current-source replay are pending. No native victory improvement is claimed.
