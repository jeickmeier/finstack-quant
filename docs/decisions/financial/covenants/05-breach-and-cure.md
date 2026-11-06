# COVENANTS-05: Breach episodes, cure deadlines and historical state

**Decision status:** Draft — observed implementation; adoption pending for choices not already settled. Existing accepted decisions retain their scope.

**Implementation status:** Observed in the exact source paths identified below, including stated discrepancies and limitations.

**Validation status:** Source and identified test assertions inspected; tests, builds, numerical experiments and external benchmarks not executed. Test existence is not adoption or independent qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Ordered state transition

`evaluate_and_track(context,date,scope)` first validates the engine and computes every selected report. It then stages new breaches for failed tests having no uncured episode with the same label and `breach_date <= date`. An episode stores the first failing report's actual/threshold, static description, selected specification's consequence order, and the test date. Continuing failures do not refresh values, reset deadlines or append episodes. A future-dated existing breach is ignored for current selection.

For `Some(days)`, the cure deadline is test date plus that many calendar days using checked date arithmetic. Negative days are invalid; overflow errors before the staged history is committed. `Some(0)` still protects the breach date itself from consequence execution. `None` provides no cure deadline, makes consequences immediately eligible on the breach date, and does not allow automatic numerical recovery to mark the episode cured.

After staging, each passing report with a finite actual value can mark the newest eligible uncured episode cured only if a deadline exists and `date <= deadline`. Inactive, fully waived, springing-inactive and descriptive passes have no actual value and cannot cure history. Recovery under an amended effective threshold can cure within that deadline. Recovery after the deadline does not clear the episode. No sponsor-equity contribution, cure-use limit, consent event, automatic reversal of executed consequences or cure timestamp is modeled. The only stored recovery flag is `is_cured`.

Validation requires nonempty historical identity, unique `(covenant_id,breach_date)`, deadline not before breach, valid captured consequences and `applied_consequences` being a prefix. It does not require a history episode to refer to a current spec, which permits post-window execution, or require only one uncured episode per label. Historical actual/threshold finiteness and a chronological evaluation cursor are not checked.

## Rationale, alternatives and limitations

Source comments and regressions support one episode per continuous breach, captured terms surviving amendments, and numerical cures only while allowed. Event-sourced history with an action/recovery timestamp would support as-of replay; immutable date-ordered processing or a last-evaluated date would constrain mutable-state use. These alternatives are not adopted here.

Static control flow permits backdated calls after later history updates; a boolean cure state has no as-of meaning. A same-day fail, cure, then fail can stage a new episode with a key already held by the cured one: validation runs before append, so that transition can leave duplicate keys for the next validation. This sequence is a static concern; no runtime outcome is asserted from a test run.

## Evidence

Owning [tracking, active-breach selection and validation](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs) and [historical contract](../../../../finstack-quant/covenants/src/engine/types.rs). Inspected [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) cover retained window terms, no cure from inactive/waived tests, checked deadline overflow and stale snapshots. [Engine conventions](../../../../finstack-quant/covenants/tests/engine_conventions.rs) assert persistent-episode deduplication and recovery before deadline.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
