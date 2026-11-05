# A Scout queue cannot satisfy a confirmed siege defense

The completed, qualified Deity Korea control
`civvis-20261105T155735Z-deterrence-matched-01-on` lost Yangsan at turn43.
Its source was `a4884eab8765382ef1c928c6d9e4f41e9bd599b1`, with native
orders SHA256 `0e401e4a02fa681040e2825fc021905e955c87b154375faceafe5fbd422d613a`.
This world is selected retrospectively for investigation, not a strength sample.

At turn30 the journal launched a Scout from Yangsan to reopen exploration.
At31/0 the native city had33 damage, was still producing that Scout, and
offered legal Ancient Walls with a12-turn native quote. The journal named
Yangsan threatened and placed its defense first. The actual top-level orders
at31/0 and31/1 contained no production order for Yangsan. It still held the
Scout at32. At40 the journal finally selected Walls after a host queue release;
native states observed their paid production at41 and42 before the loss.
None of this establishes that earlier Walls would have completed or saved it.

The original31/0 canonical record checksum is
`72a09ed0e61e4b8ad473a5ab385a32016ea3363db87fb16309579165f476f460`.
All18 actual captured reads for that request are identical. The independent
control audit already verifies449 canonical records and7808 captured reads.

The source-level exemption explains a specific handoff failure:
`active_queue_answers_siege` delegated military queues to
`active_queue_is_defensive`, which accepts land non-siege military units.
Scouts meet that broad classification. Recon production therefore reserved
the queue instead of allowing the confirmed defense selector's legal item.
The same classification could retain a later Scout over an already-confirmed
defense during the final production pass.

The fix excludes recon promotion classes only from the confirmed-siege
classification, including formations. The broader pre-war exemption remains
conservative. Existing fortifications, repair rules, Warriors and Archers keep
their protection. The existing threat/damage evidence, defensive item selector,
one-queue arbitration and opt-in activation paths remain authoritative. No new
heuristic, flag, default, registry entry or native game rule is introduced.

Regression coverage exercises the complete handoff from a damaged Scout queue
with no major war; the inactive treatment and healthy-city cases; restoring a
confirmed wall after a later Scout writer; and recon formations versus actual
local defenders. Three existing defense-repair tests remain in the module.

Exact-source CI, Mac native tests/build/smokes, and captured native proposal
qualification are pending. Native construction timing, survival and Science
strength remain unmeasured for this fix. Fresh captured-input queries cannot
reconstruct the original private planner or prove a counterfactual saved city.
