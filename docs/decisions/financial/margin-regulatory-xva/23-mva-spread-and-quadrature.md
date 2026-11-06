# MARGIN-REGULATORY-XVA-023: MVA spread interpolation, survival weighting and average IM

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

MVA integrates posted initial margin funding over its own time grid. Standalone use optionally stops funding at own default; bilateral orchestration additionally stops at counterparty default. No FX conversion, wrong-way funding dependence or future margin simulation occurs in the integrator.

## Algorithm and conventions

1. Validate nonempty ImProfile with positive increasing finite times and nonnegative finite values. Validate a nonempty funding-spread curve of `(time,bp)` with finite nonnegative, strictly increasing times and finite nonnegative spreads; a single pair is flat.
2. On each IM bucket `(previous_t,t]`, evaluate the spread at the midpoint using linear interpolation and flat extrapolation through the shared core knot interpolator. Divide by 10,000 to convert bp to decimal; spread knots are not merged into the IM integration grid.
3. Initialize previous IM to the first sampled value, previous time zero, previous DF/own survival/counterparty survival to one. Average each factor's endpoint values separately. Increment `MVA=avg(IM)×spread(mid)×avg(DF)×avg(S_own)×avg(S_cp)×dt`. Absent survival curves contribute one.
4. Sum through the last IM time; there is no runoff beyond it. Report MVA, echoed IM profile, and `average_im=Σ avg(IM)×dt / last_time`. Average IM excludes discounting, spread and survival weights.

## Selection and controls

Public standalone MVA accepts optional own survival; the internal bilateral helper also accepts counterparty survival, selected by record 21's orchestrator. Curves are evaluated at IM endpoints; nonfinite evaluations error. The same positive-time grid rule and first-value back-extrapolation as record 22 apply to user-supplied IM profiles.

## Limitations and failure behavior

Midpoint spread multiplied by separate endpoint means is not exact integration of the full product. A spread slope change inside an IM interval is not a subdivision point, and no adaptive refinement/error estimate is offered. Curve time origin, currency and day-count alignment are not verified. The flat/simple linear tests cover only special cases. A zero final IM value does not itself imply a valid opening IM or lifetime runoff profile.

## Alternatives and tradeoffs

Merging spread and IM knots plus product-aware quadrature can reduce deterministic integration bias; full stochastic IM/funding/default integration changes the economic model. Requiring a valuation-time IM point makes initial cost explicit but must be coordinated with the profile type and DVA interpolation.

## Evidence and role

Owning runtime: [src/xva/mva.rs](../../../../finstack-quant/margin/src/xva/mva.rs); bilateral selection: [src/xva/cva.rs](../../../../finstack-quant/margin/src/xva/cva.rs). Inspected tests use constant no-discount IM/spread for exact 10,000 cost, own-survival trapezoid arithmetic, a linear `[1,2]` IM grid giving 7,500 cost and average IM 375,000, and midpoint spreads 75/125 bp giving 20,000. These are transparent hand examples of the chosen rule, not convergence or pathwise expectations. [Green XVA](../../../REFERENCES.md#green-xva), [Gregory XVA](../../../REFERENCES.md#gregory-xva-challenge).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
