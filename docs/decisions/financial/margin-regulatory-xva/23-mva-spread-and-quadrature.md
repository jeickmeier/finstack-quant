# MARGIN-REGULATORY-XVA-023: MVA spread interpolation, survival weighting and average IM

- Decision status: **Accepted 2026-11-17 for time-zero-anchored MVA, full-product quadrature, and replayable reporting.**
- Implementation status: MVA and average IM are implemented. Opening-node and spread coverage, merged grids, full-product quadrature, and interval replay require reconciliation.
- Validation status: Exact arithmetic checked full-product integration against the separate-averages approximation. Rust fixtures were inspected. Curve alignment, convergence, and bilateral replay still require qualification.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision

1. Require the time-zero-anchored IM profile from [XVA 022](22-simm-deterministic-im-profile.md). Require an explicit flat funding spread or a piecewise-linear spread curve covering the declared IM horizon. Align currency, valuation cut, and curve clocks and integrate through that horizon. [Curve admission](19-xva-profile-and-epe.md).
2. Merge IM and funding-spread knots. Use checked quadrature on the full product of IM, decimal funding spread, discount factor, and elected survival. Qualify the method by grid refinement. Standalone MVA explicitly elects own survival or no survival weighting. Bilateral MVA uses the product of own and counterparty survival under the accepted independence assumption. [Funding composition](21-xva-funding-and-composition.md).
3. Retain replayable interval contributions and report unweighted average IM. Calculate average IM by time-weighting the IM profile alone.

For IM falling linearly from 100 to 0 and spread rising linearly from 0 to 100 bp over one year, unit discount and survival give MVA of 1/6. Multiplying separate averages gives 1/4.

## Observed choice and domain

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

Implementation reconciliation and independent qualification are tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
