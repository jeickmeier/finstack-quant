# COVENANTS-05: Breach episodes, cure deadlines and historical state

**Decision status:** Accepted 2026-11-17 for captured breach terms, chronological tracking and evidence-based cure.

**Implementation status:** Initial episode capture, checked deadline arithmetic and current numerical recovery are implemented. Chronology, replay identity, completed-action identity and dated cure evidence remain pending.

**Validation status:** The 115-test Covenants Rust baseline and nine focused cure tests passed on 2026-11-17 against current behavior. Replay conflicts, backdated mutations and dated cure evidence still need regression coverage.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Capture the first confirmed breach assessment, its effective terms, cure rule and ordered consequences. Keep one open episode per continuing maintenance obligation. Repeated failures leave captured terms intact. Key incurrence episodes by obligation and completed-action ID so distinct transactions on the same date can be tracked independently. Apply the [indeterminate-result election](03-comparison-and-headroom.md) and [completed-action requirement](04-activation-and-scope.md).
2. Process new history mutations in date order. An identical replay leaves history unchanged. Reject a conflicting repeat of the same maintenance test or incurrence action, and reject new backdated history mutations. Read-only historical assessment remains available. Validate the resulting history before committing it.
3. Derive any cure deadline using checked addition of the agreed calendar days to the breach date. The deadline is inclusive. Zero days protects the breach date itself. An explicit choice of no cure period makes consequences eligible on the breach date and disables automatic numerical cure.
4. Record the cure date, basis and evidence permitted by the captured rule. A passing reassessment cures only when the rule permits numerical recovery and its conditions are satisfied through the deadline. A later waiver or amendment alone leaves the episode open. Preserve completed consequences in history.

## Observed state transition

`evaluate_and_track(context,date,scope)` first validates the engine and computes every selected report. It then stages new breaches for failed tests having no uncured episode with the same label and `breach_date <= date`. An episode stores the first failing report's actual/threshold, static description, selected specification's consequence order, and the test date. Continuing failures do not refresh values, reset deadlines or append episodes. A future-dated existing breach is ignored for current selection.

For `Some(days)`, the cure deadline is test date plus that many calendar days using checked date arithmetic. Negative days are invalid; overflow errors before the staged history is committed. `Some(0)` still protects the breach date itself from consequence execution. `None` provides no cure deadline, makes consequences immediately eligible on the breach date, and does not allow automatic numerical recovery to mark the episode cured.

After staging, each passing report with a finite actual value can mark the newest eligible uncured episode cured only if a deadline exists and `date <= deadline`. Inactive, fully waived, springing-inactive and descriptive passes have no actual value and cannot cure history. Recovery under an amended effective threshold can cure within that deadline. Recovery after the deadline does not clear the episode. No sponsor-equity contribution, cure-use limit, consent event, automatic reversal of executed consequences or cure timestamp is modeled. The only stored recovery flag is `is_cured`.

Validation requires nonempty historical identity, unique `(covenant_id,breach_date)`, deadline not before breach, valid captured consequences and `applied_consequences` being a prefix. It does not require a history episode to refer to a current spec, which permits post-window execution, or require only one uncured episode per label. Historical actual/threshold finiteness and a chronological evaluation cursor are not checked.

## Rationale, alternatives and limitations

Capturing effective terms protects an episode from later window changes. Tracking identity and chronology allow retries while keeping distinct completed incurrence actions separate. Dated cure evidence identifies when and why the episode closed.

Static control flow permits backdated calls after later history updates; a boolean cure state has no as-of meaning. A same-day fail, cure, then fail can stage a new episode with a key already held by the cured one: validation runs before append, so that transition can leave duplicate keys for the next validation. This sequence is a static concern; no runtime outcome is asserted from a test run.

## Evidence

Owning [tracking, active-breach selection and validation](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs) and [historical contract](../../../../finstack-quant/covenants/src/engine/types.rs). The [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs) cover retained window terms, inactive and waived assessments, checked deadline overflow and stale snapshots. [Engine conventions](../../../../finstack-quant/covenants/tests/engine_conventions.rs) cover persistent episodes and recovery before deadline. These tests passed against current behavior.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
