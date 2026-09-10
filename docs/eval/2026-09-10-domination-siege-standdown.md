# A failed capture must stay stood down during its cooldown

`capture-go-or-stand-down` can mark a stalled siege objective unavailable and
make its plan stale immediately. The ranking honors that cooldown. With
`siege-commitment` also enabled, however, the cached target was selected ahead
of the filtered ranking, putting the same stood-down city straight back into
the plan. Reassessment could therefore keep restoring the rejected objective.

The cached commitment now consults the same `capture_stood_down_holds`
predicate as the ordinary ranking. Both stand-down versions apply; an expired
cooldown stops excluding the objective. An active commitment with no stand-down
still holds. The existing home-emergency priority remains in place.

This is an interaction correction, not a new gene or a change to deployment
selection. A separate eight-seed exploration and forty-seed holdout found that
simply enabling siege commitment did not reliably improve Domination completion;
those observations motivated inspection but do not prove this interaction caused
the outcome difference. The regression directly exercises the conflicting plan
selection paths. Validation and a matched replay of this fix are pending.
