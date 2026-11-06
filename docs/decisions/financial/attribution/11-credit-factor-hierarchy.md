# ATTRIBUTION-011: Issuer credit hierarchy planning and spread-replay allocation

- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed for the source scope listed below.
- Validation status: Source evidence inspected; named test evidence located, not executed. No numerical accuracy certification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and eligible scope

Optional credit-factor detail allocates an existing instrument credit P&L to generic, hierarchy-level, idiosyncratic-adder and curve-shape components. It requires the `credit::issuer_id` attribute, an issuer-beta row in the supplied factor model, declared hazard curves and at least one curve measurable in both snapshots. Missing eligibility returns no detail with notes. Factor-model validation/calibration belongs to the factor crate; this path does not independently validate the full model before use.

## Planning algorithm and conventions

Measure each declared credit curve's standard-tenor average par-CDS spread move in bp, omit only missing/not-found measurements, and average available curves equally. Factor scalar moves are absolute differences interpreted as bp. Generic scalar uses the model series ID with canonical generic ID fallback; hierarchy IDs derive from issuer tags. Scalar measurement errors are suppressed to absence. Multiply observed generic/level moves by issuer beta. Follow matching entries of `model.config.factors` in order, ignoring duplicate IDs; if none match, use generic then hierarchy order. A partially matching configured list does not append unmatched levels. If no scalar move is available, systematic moves are zero. Adder move is observed issuer mean spread move minus included beta-weighted systematic moves; append a final curve-shape step.

A nonzero factor move at least 99 times smaller than the nonzero issuer spread move produces a unit-scale warning and note, not an error: real idiosyncratic moves can also trigger it. Missing scalar units or an absent series cannot be inferred from this warning. Output includes model date plus FNV-1a hash of serialized model as an identity/provenance label, not a cryptographic authenticity guarantee.

## Repricing versus linear detail

Waterfall/parallel reprice cumulative bp steps from one fixed credit base, then difference successive PVs. Each step first replays opening hazard calibration into the hybrid dependency market using the first declared discount curve, then applies a parallel quote-spread bump through the recalibration provider. Zero bump still performs dependency replay. Missing source hazards are skipped; missing discount ID or failed recipe/recalibration errors. Curve-shape replaces listed hazards with available closing hazards. Consecutive price changes telescope; this path does not back-solve convexity into curve shape.

Metrics/Taylor detail instead measures a fresh central ±1 bp CS01 at closing date, using closing noncredit state and opening hazard recipes. Each generic/level/adder contribution is that CS01 times planned bp movement. Curve shape is then the aggregate credit P&L minus those linear parts, so it also absorbs nonlinear/remainder effects. Errors from this postprocessing become notes and leave aggregate credit unchanged.

## Output and limits

Per-bucket detail defaults on; per-issuer adder detail defaults off. Missing level output is explicit zero. `adder_magnitude` is absolute adder P&L. Adder and curve-shape magnitudes exceeding 5% of total absolute component P&L emit tracing warnings, not result invalidation. Generic/level ordering can affect repricing marginal allocations. A single issuer tag governs all declared hazard curves; multi-issuer exposure requires a supported upstream decomposition.

## Rationale and alternatives

The implementation uses quote-space replay to match par-spread units, and separates shape from systematic/idiosyncratic parallel moves. Located tests cover beta scaling, configured factor order and unit mismatch; linear reconciliation is an accounting identity, not independent evidence of economic correctness.

## Source and evidence

- [credit_cascade.rs](../../../../finstack-quant/attribution/src/credit_cascade.rs) — owning implementation.
- [credit_decomposition.rs](../../../../finstack-quant/attribution/src/credit_decomposition.rs) — owning implementation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). This record does not
authorize a runtime change or certify the observed method.
