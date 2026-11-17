# ATTRIBUTION-011: Issuer credit hierarchy planning and spread-replay allocation

- Decision status: Accepted 2026-11-16 for checked credit hierarchy and spread-replay allocation.
- Implementation status: Observed for the source scope listed below. Runtime, model, replay, cash and host reconciliation with the accepted contract are pending.
- Validation status: Independent exact examples checked signed beta moves and adder, order-dependent repricing, matched linear and replay shape, separate curvature and parent cash closure. Production attribution and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked credit hierarchy and spread-replay allocation

1. Bind the hierarchy to one frozen, semantically validated model eligible at the opening cut. Require the exact issuer row, signed betas, canonical levels and buckets, and validated fold records. Verify that the selected pricer's active typed hazards belong to the parent scope. Multiple curves of one issuer require the admitted shared parallel spread direction. Multi-issuer or index exposure needs qualified upstream issuer scopes that reconcile to the parent. Native intensity, credit-role zero rates and bond Z-spread retain their own bases. [Model validation contract](../../architecture/0047-credit-factor-wire-and-validation-contract.md) · [Accepted issuer exposure rules](../portfolio-attribution/12-factor-assignment-and-credit-exposures.md#accepted-decision-for-checked-factor-assignment-and-credit-exposures).
2. Measure the issuer move on one model-qualified par-spread reference with exact quote, pillar and contract identity. Pair the full opening and closing par-risk quote vectors on exact coordinates. Bind factor identity, eligible cuts, vintage and normalization. Difference comparable levels once or admit a qualified move for the exact interval. Retire standard-tenor and available-curve averaging, generic-ID fallback and the `99×` unit inference warning. Missing active observations fail required detail. A validated zero or folded loading can establish structural zero. [Accepted comparable measurements](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows).
3. Fix cascade order as generic credit, broadest to narrowest model hierarchy, then issuer adder. Each systematic move is `β_k × ΔF_k` in issuer spread bp. The adder is the verified issuer reference move less the sum of the systematic moves. It measures the unexplained reference spread move. Retire partial configuration-order matching and silent omission of levels. Publish the order because nonlinear repricing can change individual allocations when the order changes.
4. Use the authoritative parent states for Waterfall and Parallel. Waterfall starts from its actual pre-credit state. Parallel starts from its actual restored-credit state at the closing cut. Qualify original fits and the zero-shock par-risk center, and show the price bridge from the parent before-mark to that center. Replay each cumulative vector independently from the same immutable center on the actual typed calibration dependencies. Apply each shared source change once, rebind atomically and verify delivered moves. After the last parallel step, reprice the full matched quote vector. Its price difference from the parallel state is curve shape. Show the difference to the actual closing-credit mark as a separate bridge. Missing quote evidence or failed replay fails required detail. [Accepted Waterfall](04-ordered-waterfall.md#accepted-decision-for-checked-ordered-waterfall-revaluation) · [Accepted Parallel](05-parallel-and-cross-factor.md#accepted-decision-for-checked-parallel-repricing-and-cross-factor-effects) · [Accepted quote replay](../calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication).
5. Reuse the parent's opening par-risk sensitivities for MetricsBased and Taylor. With opening bucket sensitivities `s_i`, allocate each parallel step `a_k` as `Σ_i s_i a_k`. For observed quote moves `m_i` and reference move `g = Σ_k a_k`, measure linear curve shape as `Σ_i s_i(m_i − g)`. Scalar parent CS01 qualifies only when complete quote evidence establishes that every covered move equals `g`. Keep parent curvature and any qualified center bridge distinct. Retire the fresh closing-state `±1 bp` experiment and curve-shape balancing entry. [Accepted Taylor curve risk](06-taylor-curve-risk.md#accepted-decision-for-checked-taylor-curve-sensitivities-and-directional-gamma) · [Accepted metric credit P&L](09-metric-rates-and-credit.md#accepted-decision-for-checked-metric-rate-and-credit-pl).
6. Reconcile and publish nested detail. Elect no detail, required detail or advisory detail, with bucket and issuer breakdowns elected separately. Required failures propagate. Advisory failure returns typed unavailability. Reconcile ordered detail to the covered parent with checked Money. Show eligible parent cash separately and count the parent once in headline P&L. Publish the immutable model reference, source identities, moves, marks and actual pricing work or reuse through Rust for Python and WASM. Keep signed adder P&L and its absolute magnitude as factual diagnostics. Retire fixed `5%` warnings as qualification controls. [Accepted cash rules](03-total-return-carry.md#accepted-decision-for-checked-period-cash-gross-carry-and-funding) · [Credit carry record](12-credit-carry-partition.md).

Exact examples verified `−14` systematic P&L, `−7` adder and `+3` measured shape, yielding `−18` first order and `−14` price P&L after `+4` parent curvature. The current balancing method would label `+7` as shape. A separate nonlinear replay reconciled `+14` of parent price P&L and `+2` of separate cash. Changing replay order changed individual allocations while preserving the total.

## Observed choice and eligible scope

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

## Relationship to the accepted comparable-measurement package

[CORE-MARKET-22's accepted package](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows)
was accepted on **2026-11-08**. It removes inferred core par-spread/triangle measurement and
requires full matched quote/contract basis or the actual CDS pricer before a
spread move is claimed. Factor scalars remain opaque native values until their
bp meaning is explicitly established. Required source absence cannot silently
change an issuer mean's population. The separate issuer factor hierarchy and
its beta model/order, replay allocation, adder/shape and optional-detail policy
remain owned by this Draft record.

## Follow-up tracking

Runtime reconciliation and validation are tracked in the
[scope follow-ups](../FOLLOW_UPS.md#attribution). Production and host
qualification remain pending.
