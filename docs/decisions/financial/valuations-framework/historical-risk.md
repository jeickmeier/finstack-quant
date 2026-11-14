# VALUATIONS-FRAMEWORK-14 — Historical scenario repricing, directional Taylor risk, VaR and expected shortfall

- Decision status: **Accepted — complete checked historical scenario risk package, 2026-11-14**.
- Implementation status: **Observed, with reconciliation pending** across history, scenario application, pricing, replay, tail estimators and hosts.
- Validation status: Source and primary references inspected. Independent bounded tail, scenario FX, stencil and aggregation examples checked. Runtime, replay, financial accuracy and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked historical scenario risk

1. **Require an explicit current valuation, history basis and method.**
   Select full revaluation or directional second-order Taylor and
   supply finite confidence `0.5<c<1`. Remove the silent full-revaluation
   and 95% defaults. Use one equally weighted scenario per distinct
   observed interval, with a unique ID and preserved chronological input
   order. Require observation endpoints, valuation/publication phase
   and movement provenance on a common declared native interval
   basis, which can be in sessions under an explicit calendar.
   Multiple adjacent intervals may share an endpoint, and overlapping
   intervals do not imply independent observations. Reject records
   that use information after the allowed cutoff or disagree with the
   declared interval basis. Validate supplied provenance; this calculator
   does not construct historical
   movements or retrieve market history. Record the actual admitted
   sample bounds. A lookback label does not filter, annualize, scale,
   change weights or determine a valuation horizon. Apply movements at
   the unchanged prepared effective date and phase without overwriting
   the observation dates or projecting future cash and fixing history.
   [Pritsker on historical simulation and its limits](https://www.federalreserve.gov/econres/feds/the-hidden-dangers-of-historical-simulation.htm),
   [record 13's separate horizon experiment](carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).

2. **Admit each complete joint movement before applying it.** Require
   typed target identities, units, exact coordinates and a declared
   movement scope. Distinguish observed zero, explicitly held fixed,
   and missing required coverage. A validated identity row counts as
   one observation. Reject repeated exact coordinates, conflicting
   writes and economic reciprocal duplicates instead of last-wins or
   silent netting. Distinct rate shapes on one curve can overlap
   under its accepted composition; such shapes remain distinct
   coordinates. Keep explicitly authored sequential action batches
   with their own owner. Resolve actual active pricing, calibration
   and reporting FX dependencies, including aliases, composites and
   derived FX routes. Prune operations only after full row admission
   and proof they cannot change the experiment; declarations alone
   do not establish this. Every scenario starts from the same
   coherent frozen base. Stage and validate privately and publish
   no partial result. [Comparable movements](../core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows),
   [typed context operations](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).

3. **Use exact native factor operations and coordinates.**
   The shared profile uses these movements on selected active bindings:

   | Factor | Admitted movement |
   | --- | --- |
   | Discount or projection | Additive continuous-zero move in annual decimal rate with exact typed curve, source-year triangle center and explicit grid or neighbors. Preserve the full dated evaluator. A measured simple forward-rate change needs a qualified mapping to this coordinate. |
   | Credit | Additive annual decimal par-spread changes at exact quote IDs or verified indices, with explicit checked conversion for a bp replay interface. |
   | Spot | Relative decimal move of the exact active positive underlying quote in its native representation, preserving contractual reference levels. |
   | FX | Relative decimal move of an exact ordered active quote with its authorized override or frozen query path and explicit current reference date and policy. A reciprocal view uses division by the move factor. |
   | Volatility | Additive annual sigma movement in the actual native units on exact qualified grid expiry and strike or tenor coordinates. Preserve the quote type, clock and qualified source-to-model bridge. |

   Check finite quantities, required represented changes and the
   complete transformed domains at the actual coordinates. Remove
   implicit standard rate neighbors and tenor matching tolerances.
   No floor, nearest node, quote convention conversion or model
   substitution is inferred. Explicit alternative volatility parameter
   and raw market-quote directions need their own admitted owner route.
   [Direct rate shocks](../core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning),
   [directional FX](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks),
   [native sigma](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).

4. **Stage noncredit inputs, then replay exact credit quotes.** Qualify
   original-source reproduction and a consistent par-risk center against
   the original market before replay. For each selected hazard retain
   the original observations, recovery, conventions and actual
   calibration dependencies. In each scenario or stencil sample, stage
   the full noncredit input move first. Apply the signed quote vector
   to the fixed center and refit against the exact staged calibration
   dependencies. Distinguish deal and calibration discount roles.
   For an unshifted credit source whose dependencies move, require a
   declared frozen-intensity or same-par-center refit policy. Neither
   recipe presence nor omitted coverage selects it. Require accepted
   target fits and update shared and composite references atomically.
   Deduplicate only identical complete replay bindings, and reject
   conflicting or unsupported dependencies. No direct-hazard or
   z-spread fallback for a requested credit quote shock.
   [Credit risk](credit-sensitivities.md#accepted-decision-for-checked-credit-sensitivities),
   [joint replay](cross-factor-risk.md#accepted-decision-for-checked-cross-factor-risk),
   [calibration provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

5. **Reprice and aggregate on the same position and currency basis.**
   Prepare each signed and sized holder position once. Retain the
   admitted dispatch, lifecycle, quantities, amount basis, and all
   required controls. The shared instrument list takes already sized
   economics; repeated supplied entries count as separate positions.
   An existing position adapter applies its explicit quantity exactly
   once. Require explicit reporting currency for mixed
   currencies or an empty book, with same-currency inference only for a
   qualified nonempty book. The shared total reporting P&L translates
   each native PV once using its own base or shocked state's frozen,
   explicitly qualified valuation-date FX query. This includes
   reporting-only FX risk and its interaction with native valuation.
   A separate frozen-translation basis requires an explicit owner.
   Use comparable raw reprices or an admitted checked Money basis
   for both methods. Preserve meaningful differences before rounding;
   enforce quantity, currency and finite output. Full revaluation
   sums scenario-minus-base P&Ls in stable position order with checked
   arithmetic. Take tails on the resulting aligned book distribution.
   Never sum position VaR/ES or translate a tail after estimation.
   [Prepared dispatch](pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle),
   [money precision](../core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity).

6. **Differentiate the complete scenario direction in Taylor mode.**
   Define `f(x)` as that same reporting-currency book PV with the
   admitted joint input moves scaled by common amplitude `x`,
   including every declared responsive fit and translation. Zero is
   the qualified base and one is the literal full move. Admit and
   stage the full move, including required transforms and fits,
   even when skipping its final price. Require explicit finite
   `0<h≤1` in dimensionless scenario units. Check the same actual
   common amplitudes and represented transformations at every
   required position. Remove automatic normalizing scales, boundary
   shrinking and per-position steps. At actual amplitudes `-c,0,+a`
   with `a,c>0`, define `d+=(f(a)−f(0))/a` and
   `d−=(f(0)−f(-c))/c`. Report
   `(c d+ + a d−)/(a+c) + (d+−d−)/(a+c)`, the opening first
   derivative plus half the curvature of the quadratic interpolant,
   evaluated as P&L for unit scenario amplitude. Use checked
   scale-aware arithmetic. This includes directional interactions
   without separately summing Greeks. [Represented stencils](finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk),
   [Fornberg on actual node weights](https://doi.org/10.1090/S0025-5718-1988-0935077-0).

7. **Qualify a separate forward volatility-edge stencil and stochastic
   execution.** Permit the edge stencil only as an explicit opt-in
   on an owner-admitted volatility boundary. Before pricing, confirm
   the full negative stencil step lies outside that boundary and
   that the requested positive `h/3, 2h/3, h` all qualify for the
   whole scenario. Exactly zero sigma is admissible where the pricer
   permits it. With actual `0<e1<e2<e3`, use the cubic interpolant
   through `0,e1,e2,e3` and return its first derivative plus half
   its second derivative at zero. Equivalently, for successive
   Newton divided differences `D01,D012,D0123`, return
   `D01+(1−e1)D012+(e1 e2−e1−e2)D0123`.
   Lost movements, failed prices or replay, other boundaries and
   unsupported lifecycle routes cannot switch the method or stencil.
   A stochastic route needs an owner-qualified price comparison
   or estimator, aligned logical draws, groups and slots as appropriate
   to its method, and paired uncertainty and limits. A matching seed
   alone is insufficient. Separate price-sampling uncertainty from
   historical sample adequacy. Taylor remains a local approximation;
   option kinks, large moves and higher terms may invalidate it even
   when a step comparison looks stable. It need not cost less than
   full revaluation. [Paired option risk](option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).

8. **Use signed R-7 VaR and fractional empirical expected shortfall.**
   For sorted finite book P&Ls `x_1≤...≤x_N`, let `p=1−c`.
   VaR is the checked R-7 quantile at zero-based rank `(N−1)p`.
   ES takes exactly the worst `p` probability mass of the equal
   scenario distribution: for `m=Np`, `k=floor(m)`, use
   `[Σ_{i=1..k} x_i + (m−k)x_(k+1)]/m`. Reuse the checked core
   kernels and preserve the represented confidence and fractional
   weights, including at ties. Do not integrate the R-7 interpolant
   for ES or average a count of observations below VaR. Return signed
   reporting-currency P&L, including positive gain tails, without a
   zero clip. Check sample support and `ES≤VaR` where both are
   requested. Report numerical failure without estimator substitution
   or repair. This explicitly selects the same two estimators for
   monetary scenarios as ADR 0033 chose for returns; it does not
   extend the return API or adopt a regulatory or factor model.
   [ADR 0033](../analytics/0033-value-at-risk-method-selection.md#accepted-decision-fixed-historical-tail-selection),
   [checked core statistics](../core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty),
   [R-7 definition](https://search.r-project.org/R/refmans/stats/help/quantile.html),
   [Rockafellar–Uryasev, discrete probability mass](https://sites.math.washington.edu/~rtr/papers/rtr187-CVaR2.pdf).

9. **Disclose sample resolution and distinguish zero from unavailable.**
   Validate request and history before zero or availability shortcuts.
   Nonempty holdings require at least one admitted scenario. An
   empty distribution has unavailable tails. An admitted empty book
   with nonempty validated history has one literal zero per scenario.
   A finite singleton returns its one P&L for both measures; when
   `m<1`, ES equals the worst observation and VaR still follows
   R-7. Report exact scenario count, probability and tail mass in
   observation units. Remove the universal 20-scenario reliability
   heuristic. A workflow can impose its own coverage or adequacy
   criterion. Equal weights, complete inputs and a computable tail
   do not establish forecast accuracy or independent observations.

10. **Publish one qualified distribution and reconcile all consumers.**
    Preserve ordered scenario IDs, observation cuts and P&Ls, and
    expose ranked P&Ls as a separate view. Record scope, currency,
    as-of, sample basis, method, confidence, selected controls,
    actual stencil or replay and any applicable uncertainty. HVaR
    and ES share that distribution only within the same qualified
    valuation request and publish only requested measures. Bound
    input and actual pricing, replay, storage and reduction work;
    propagate the first failing scenario, position or stencil in
    stable input order without omitted scenarios or partial results.
    Preserve deterministic results where the selected profile
    supports them without assuming identical host call counts.
    Restore only validated result state with internally consistent
    metadata, scenario rows, ranks, counts and requested estimates.
    Preserve identity scenarios in history projections and validate
    strict raw wire input before map materialization can erase duplicate
    fields. Reconcile standalone and registry Rust, portfolio callers,
    Python/WASM results and projections, schemas, declarations and
    parity. Factor-risk allocation, historical template fitting and
    regulatory metrics keep their own bases.
    [Metric request and output basis](metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).

## Observed behavior

### Choice and input domain

Historical risk uses equally weighted supplied scenarios at the current valuation date, either full model repricing or a second-order directional Taylor approximation. Default confidence is 95% and default method is full revaluation. The input is a list of instrument references, not an independently weighted portfolio; instrument economics must already represent the requested position. No horizon scaling, decay weighting, volatility normalization or history construction is performed. `base_date` and `window_days` are descriptive history metadata, not filters.

### Scenario and valuation algorithm

1. Rate/credit shifts are absolute decimal changes, converted to bp. Discount and forward shifts use triangular key-rate bumps whose neighbors come from the standard bucket grid. Equity and FX shifts are relative returns; volatility shifts are additive absolute volatility at exact expiry/strike grid nodes.
2. Repeated curve shifts are applied in successive rounds so the context cannot discard duplicates. Duplicate volatility-node shifts are added before applying one node update. A volatility result below zero or a non-grid point fails rather than clamps. Credit shifts are grouped by hazard curve and passed as tenor quote bumps to a required recalibration provider, using the original market as source and already-shocked market as target; this preserves scenario rate/credit interaction.
3. Replace the historical scenario's date with `as_of`; all prices are valued on that unchanged date. Full revaluation computes each scenario price minus its base price, using compensated summation across instruments. The selected pricing dispatch is preserved. Mixed-currency inputs require an explicit reporting currency; base and shocked PVs use their respective FX markets under cashflow-date conversion, so FX translation risk contributes to P&L.
4. Taylor mode first filters shifts by declared dependencies (and keeps FX shifts when currency conversion is needed). Let `f(a)` be the reporting-currency price with all retained scenario shifts multiplied by `a`. Set `h=1/max(1,max|shift|/unit)`, using unit 1bp for rates/credit and 0.01 for spot/vol. Shrink `h` to half the nearest symmetric positivity boundary for spot/FX/vol. Estimate P&L at full scenario scale as `(f(h)−f(−h))/(2h) + [f(h)−2f(0)+f(−h)]/(2h²)`. This captures directional cross terms through actual joint repricing; it is not a sum of separately stored Greeks.
5. If the symmetric volatility boundary is zero, use forward points at `h/3`, `2h/3`, `h`: with the reduced `h`, first derivative is `(-11f0+18f1−9f2+2f3)/(6h)` and second is `(2f0−5f1+4f2−f3)/h²`; combine first plus half second. Invalid full negative-vol/spot scenarios fail even when a local stencil could be evaluated.

### Tail estimator and conventions

Sort finite scenario P&Ls ascending. With confidence `c` strictly in `(0,1)`, use `p=1−c` and the core linearly interpolated quantile on index `(n−1)p`. VaR is `min(Q(p),0)`: **negative loss P&L**, not a positive loss amount. ES is `min((1/p)∫₀ᵖQ(u)du,0)`, integrating the same piecewise-linear quantile with trapezoids and compensated summation. It is not the arithmetic average of observations below VaR. One observation gives its loss-clipped value; an empty result distribution gives zero. An empty instrument list returns empty/zero risk; nonempty instruments with empty history error. Fewer than 20 scenarios warn but are accepted. HVar and ES metric calculators cache one another and record scenario count.

## Limitations, alternatives, and evidence

Taylor accuracy depends on smoothness, scenario size, dependency completeness, model noise and exact source quote semantics. It still performs scenario-direction repricing per instrument and can require full credit recalibration at stencil points. Full repricing is the implemented alternative. Weighted historical estimators, discrete empirical-tail ES and filtered historical simulation are unimplemented alternatives needing an explicit method decision.

- [var_calculator.rs](../../../../finstack-quant/valuations/core/src/metrics/risk/var_calculator.rs), [market_history.rs](../../../../finstack-quant/valuations/core/src/metrics/risk/market_history.rs), [hvar.rs](../../../../finstack-quant/valuations/core/src/metrics/risk/hvar.rs), and [risk_factors.rs](../../../../finstack-quant/valuations/core/src/metrics/risk/risk_factors.rs) own these algorithms and units. Inline tests cover distribution/formula and market-shift behavior; they were inspected/located, not executed.
- [McNeil–Frey–Embrechts](../../../REFERENCES.md#mcneil-frey-embrechts-qrm) supplies risk-model context; the particular interpolated-tail estimator and directional stencil remain repository choices awaiting approval.

The inspected full method prices Money with default pricing controls,
while Taylor filters the declared dependencies and uses raw reprices.
The current history table loses rows with no shifts and the ranked P&L
array loses scenario ordering. The
[Python entry and restore boundary](../../../../finstack-quant-py/src/bindings/valuations/var.rs)
and [WASM entry](../../../../finstack-quant-wasm/src/api/valuations/risk.rs)
consume the shared Rust results and need the same qualified basis.

### Independent review evidence

- At 95% confidence, 30 P&Ls of `−20,−10` and 28 zeros give
  R-7 VaR `−5.5` and fractional ES `−50/3`. Integrating the R-7
  interpolant instead gives `−12.75`. At 75% confidence and P&Ls
  `[1,2,3]`, the proposed signed VaR and ES are `1.5` and `1`.
- A EUR claim rises from `100` to `105` while its USD/EUR rate
  rises from `1.10` to `1.21`. Its USD P&L is `17.05`,
  including `5.50` from the local change, `11` from translation and
  `.55` from interaction. Translating only the local P&L gives
  `6.05`. Two positions with aligned P&Ls `[-10,+10]` and
  `[+10,-10]` yield joint distribution `[0,0]`.
- For `f(x)=100(1+.1x)(1+.2x)`, full, unequal-width central Taylor
  and forward Taylor all give `32` including `2` interaction.
  For `f(x)=100+30x+2x²+7x³`, the irregular forward stencil through
  `0,1/9,2/7,5/12` gives `32`; full repricing gives `39`.
  For `f(x)=100+max(0,10x−4)`, centered steps `.1` and `.05`
  both give zero even though the full move gives `6`.
- Starting at sigma `.20`, duplicate movements `−.30,+.20`
  would net to an admitted-looking `.10`. The shared historical
  vector rejects the duplicate coordinate, while deliberate sequential
  application rejects the first intermediate sigma `−.10`.

These are standalone exact arithmetic checks. They do not run any
production pricer, source replay, simulation, schema consumer or
accuracy benchmark.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
