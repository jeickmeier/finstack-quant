# COVENANTS-06: Consequence conversion, ordering and retry semantics

**Decision status:** Accepted 2026-11-17 for consequence ordering, per-action commits and retry reporting.

**Implementation status:** Eligibility checks, captured action order and successful-prefix retry are implemented. Stable episode identity, cross-episode conflict handling, dated application records and structured partial results remain pending.

**Validation status:** The 115-test Covenants Rust baseline and all seven focused consequence tests passed on 2026-11-17 against current behavior. Dated records and cross-episode conflicts need qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope inventory and coverage ledger](README.md).

## Accepted decision

1. Resolve stable episode IDs against recorded history and preserve each episode's captured action order. Skip cured episodes, execution before breach and dates through the inclusive cure deadline. Process independent episodes oldest first with a stable episode ID tie-breaker. Require an agreement-specified resolution rule for competing terms before mutating the instrument.
2. Commit each instrument action together with its application record. Record the episode ID, action position, instrument ID, application date and outcome. On partial failure, return the actions already committed and identify the failed action. Resume at the first outstanding action on retry. Keep external side effects outside this instrument mutation path.
3. Preserve fractional basis points and pass sweep fractions in `[0,1]`. Acceleration must move maturity earlier and take effect on or after the execution date. A sweep, collateral requirement or default action that lacks required typed target inputs stays outstanding. Preserve contractual conversion precision under [ADR 0020](../conventions/0020-numerical-representation-and-rounding.md).

## Observed execution

`apply_consequences` validates the engine, resolves every supplied snapshot to a current history episode by `(covenant_id,breach_date)`, and errors before mutation if any key is unknown. Snapshot cure flags and consequence lists do not override history. It then visits episodes in caller-supplied order, skipping cured episodes, dates before breach, and dates on or before an inclusive cure deadline. Current windows, waivers and later term amendments do not cancel captured historical consequences.

For each eligible episode, the next action index is `applied_consequences.len()`. Clone the target, run that action on the clone, replace the original only on success, append the successful action to history, and emit a `ConsequenceApplication`. A later failure returns an error but retains earlier target/history commits. Retrying resumes at the first outstanding action; duplicate snapshots do not reapply a completed prefix. There is no cross-episode aggregation or priority sorting.

Conversions and target calls are exact dispatch choices: default sets status true with `as_of`; a nonnegative finite `bp_increase` becomes decimal rate `bp/10000`; cash sweep passes a fraction in `[0,1]`; distributions are blocked; a nonempty collateral description is passed with `as_of`; maturity is set to the specified date. The engine does not require an accelerated date to be earlier than current maturity or at/after execution. Cash-sweep amounts, default recoveries and actual collateral transfers are target responsibilities. Validation permits repeated actions in the ordered list.

## Dependency and failure boundary

`InstrumentMutator + Clone` supplies transactional isolation only if cloning isolates mutable state and methods have no external side effects, as the source contract requires. Rollback of network/accounting effects is not implemented. An error discards the local applications vector even if prior actions committed; current history retains the successful prefix but not its application dates. Consumers must not interpret an error as whole-call rollback.

The inspected [TermLoan adapter](../../../../finstack-quant/valuations/fixed-income/src/term_loan/types.rs) is an important boundary example: the current rate-change adapter preserves fractional basis points without recording a dated coupon event; realized default, nonzero percentage sweeps and collateral requirements error without richer typed inputs; distribution blocks set metadata; maturity changes revalidate the loan. Those choices are owned outside this 14-file ledger. An engine template is not evidence that every declared action is executable on a given instrument.

## Rationale, alternatives and evidence

The successful prefix supports action-level retries. Dated records make partial success observable after the call returns. Agreement rules govern conflicts where setter-style sweeps, rates or competing maturity dates can change the result.

Owning [execution implementation](../../../../finstack-quant/covenants/src/engine/covenant_engine.rs), [mutator interface](../../../../finstack-quant/covenants/src/engine/helpers.rs), and [consequence/history contracts](../../../../finstack-quant/covenants/src/engine/types.rs). Inspected `consequence_retry_commits_each_action_once_and_discards_failed_mutation`, `unsupported_collateral_remains_outstanding`, and `stale_breach_snapshot_cannot_override_cure_or_invent_history` in [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs).

The focused Rust tests exercised current eligibility, partial commit and retry behavior on test instruments. Agreement-level conflict and dated application evidence remain pending.

## Follow-up tracking

Implementation reconciliation and qualification are tracked
in the [covenant follow-ups](../FOLLOW_UPS.md#covenants).
