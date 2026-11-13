# Analytic model methodology inventory

**Decision status:** MODELS-ANALYTIC-01 through 28 Accepted.

**Implementation scope:** All **58 assigned Rust source files** in models `closed_form`, `fourier`, `rates` and `volatility`: **47 algorithm-bearing**, **9 support**, **2 test-only**, **0 generated**. Runtime implementations, material domain/default/dispatch decisions and interleaved test-helper boundaries were inspected. The exact one-file-one-entry ledger is [coverage.json](coverage.json).

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

**Validation:** Evidence inspected/located, **not executed**. No pricing code, tests or fixtures changed; explicit adoption status is identified above. Source inspection establishes behavior, not financial approval or a universal numerical certification. External provenance is described as recorded by existing fixtures; original vendor runs were not repeated. Numerical dependencies outside this manifest (core solvers, OU stepping, regression, RNG and curve storage) are referenced as dependency contracts and belong to their owning slices.

The **28 detailed records** below separate formulas, inverse solvers, calibration objectives, interpolation, integration, risk units, exercise and fallbacks. Multiple rows reference a record only where they share the same coherent model contract. Product-level selection in [ADR 0027](../valuations-equity-and-composition/0027-vanilla-option-model-selection.md), [ADR 0028](../valuations-bonds/0028-callable-bond-model-selection.md) and [ADR 0029](../valuations-rates/0029-rate-option-exercise-and-volatility.md) does not approve these kernel controls. Embedded HW defaults are in [pricer_defaults.v1.json](../../../../finstack-quant/models/data/defaults/pricer_defaults.v1.json).

[MODELS-ANALYTIC-01's complete spot-price/Greek/calculator package](spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results)
was accepted on **2026-11-09**: checked BSM prices with explicit zero boundaries,
native derivative units and nullable whole singular Greek bundles, stable checked
numerics and an explicitly composite price/IV-plus-Greeks calculator. It removes
public unchecked kernels, embedded reporting scales and theta defaults, with
actual product/host/UI/state reconciliation required. Bounded independent
Decimal/scalar/derivative references and read-only reviews support definitions;
production and independent complete-product qualification remain pending.
Record01 is Accepted; forward02 and inverse03 retain their separately stated review status.

[MODELS-ANALYTIC-02's complete forward-model package](forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles)
was accepted on **2026-11-09**: one unit-numeraire price/Greek scale for Black,
normal and shifted Black, with explicit volatility/shift units, preserved original
shifted payoff/parity, nullable regular bundles and stable checked calculations.
It removes Black-only df and annuity/raw/scalar siblings, retains actual product
multipliers/risk chains and states the unit-output range boundary. Shared
calculator/inverse normalization and actual product/calibration/host/UI/state
reconciliation remain required. Bounded independent Decimal/Fraction references
and read-only reviews support definitions, not production qualification.
Record02 is Accepted; inverse03 retains its separately stated review status.

[MODELS-ANALYTIC-03's complete inverse package](implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance)
was accepted on **2026-11-10** with one OptionModel inverse and immutable required
five-field controls, literal upper-first/zero-IV bounds and positive identifiable
time, caller-bounded core Brent on annual volatility with a checked log-OTM
objective, and final original-premium plus independent time-value gates.
It removes fixed floors/caps, duplicated inverses and universal accuracy claims,
preserves actual product objectives and requires shared-consumer/host/UI/state
reconciliation. Bounded independent references support mathematical distinctions;
production and independent complete-path qualification remain pending.
[MODELS-ANALYTIC-04](asian-moments.md#accepted-decision-for-discrete-asian-pricing) was accepted on
**2026-11-10**. It selects one checked discrete API with prepared fixing
levels and a separate payment discount, exact geometric pricing and a
two-moment arithmetic approximation on the future contribution. It includes
seasoning, strict fixing and numerical rules and consumer reconciliation.

[MODELS-ANALYTIC-05](barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash) was accepted on
**2026-11-10**. It selects checked continuous single-barrier premiums, independent
hit and survival tails, three explicit cash events and bounded first-passage
integration when the real-root formula is unavailable. Asset and FX owners
retain dated monitoring, history and payment status.

[MODELS-ANALYTIC-06](lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums) was accepted on
**2026-11-10**. It selects checked fixed and floating premiums with qualified
inclusive extrema, explicit net carry and independent payment discount.
The four prices share positive reflection and vanilla components. The
decision includes exact zero carry, stable original-carry evaluation and
dated history, expiry and volatility-source reconciliation.

[MODELS-ANALYTIC-07](quanto-drift.md#accepted-decision-for-fixed-conversion-quanto-premiums) was accepted on
**2026-11-10**. It selects checked foreign log carry and payout discount,
explicit payout-per-asset FX orientation, scaled covariance and one
normalized discounted price. Contractual conversion and quantity remain
with the dated payout owner. The decision covers actual market,
fixing and host reconciliation.

[MODELS-ANALYTIC-08](heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform) was accepted
on **2026-11-10**. It selects checked Heston parameters, real-frequency
cash and share contours, and a carry-normalized affine logarithm with
the decaying principal-log branch. Exact zero vol-of-vol uses the
deterministic variance law. Feller remains an explicit diagnostic.
Pricing integration remains in record 09.

[MODELS-ANALYTIC-09](heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing) was accepted
on **2026-11-10**. It selects direct expiry and deterministic prices,
paired OTM inversion for the stochastic law, a conservative analytic
tail enclosure, fixed-cutoff mesh refinement and cumulative work
and resolution gates. It uses one checked scalar price across hosts.

[MODELS-ANALYTIC-10](cos-inversion.md#accepted-decision-for-checked-cos-pricing) was accepted
on **2026-11-10** for checked Black, Merton and variance-gamma COS prices. It uses a bounded
payoff for each requested side, explicit premium tolerance and separate
window and series bounds with a fixed work cap.

[MODELS-ANALYTIC-11](jump-characteristic-laws.md#accepted-decision-for-checked-characteristic-laws)
was accepted on **2026-11-10** for private checked Black, Merton and variance-gamma laws
with forward normalization, separate cash and reflected share parameters,
exact support and moment domains, and stable inputs to COS.

[MODELS-ANALYTIC-12](rough-heston-riccati.md#accepted-decision-for-checked-rough-heston-pricing)
was accepted on **2026-11-10** for the checked fractional Riccati law and Lewis half contour,
with explicit premium controls, an analytic tail enclosure, separate time
and mesh checks, and capped cumulative work.

[MODELS-ANALYTIC-13](sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes)
was accepted on **2026-11-10** for explicit Normal, Black and shifted Black SABR quotes
with canonical parameter admission, exact CEV distance, stable skew
evaluation and Hagan midpoint time correction.

[MODELS-ANALYTIC-14](sabr-calibration.md#accepted-decision-for-explicit-checked-sabr-calibration)
was accepted on **2026-11-10** for explicit checked single-expiry fitting with native
quote-error acceptance, literal displacement, nine bounded starts and
an optional observed-ATM pin on a checked increasing alpha bracket.

[MODELS-ANALYTIC-15](sabr-smile-repair.md#accepted-decision-for-checked-sabr-smile-and-finite-grid-repair)
was accepted on **2026-11-10** for an explicit smile convention, a checked ATM delta
proxy, unit-price grid diagnostics and a greatest down-only repair
with a native move cap and post-repricing checks.

[MODELS-ANALYTIC-16](volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization)
was accepted on **2026-11-10** for checked source evaluation with exact support and explicit boundaries,
native sigma and total-variance interpolation, common-beta SABR parameter
interpolation and expiry-row quote blending, strict materialization and
native-unit reference moves. Scalar arithmetic checks support the formulas.

[MODELS-ANALYTIC-17](fx-delta-smiles.md#accepted-decision-for-checked-fx-delta-smiles-and-strike-materialization)
was accepted on **2026-11-10** for checked FX forward-delta transforms with returned-strike acceptance,
complete labelled pillars, strict-expiry quote-space lookup and explicit
strike wings. Materialization samples the exact union of original strikes
with qualified row forwards and explicit output interpolation.

[MODELS-ANALYTIC-18](svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance)
was accepted on **2026-11-10** for immutable raw SVI with exact shape and wing bounds, stable variance
and density, a caller-seeded checked LM fit and independent native quote
and sampled-density acceptance.

[MODELS-ANALYTIC-19](dupire-local-volatility.md#accepted-decision-for-checked-dupire-extraction-and-local-volatility-grids)
was accepted on **2026-11-10** for qualified fixed-moneyness Dupire extraction with explicit
support and smoothing, unequal-grid derivatives, strict slope and density
checks, and local sigma interpolation with checked simulation consumption.

[MODELS-ANALYTIC-20](surface-arbitrage-diagnostics.md#accepted-decision-for-checked-surface-arbitrage-diagnostics-and-coverage)
was accepted on **2026-11-10** for one checked report with sampled Black call constraints, strict
overlap calendar probes and independent Dupire signs. Verdicts include
coverage and use all findings regardless of display.

[MODELS-ANALYTIC-21](variance-replication.md#accepted-decision-for-checked-variance-replication-and-explicit-black-wings)
was accepted on **2026-11-10** for checked forward-split OTM pricing, one deterministic trapezoid
refinement and an explicit mesh gate. Wing policy is explicit, with
optional full flat-Black continuation using bounded quadrature.

[MODELS-ANALYTIC-22](atm-volatility-conversion.md#accepted-decision-for-checked-price-equivalent-atm-volatility-conversion)
was accepted on **2026-11-10** for literal at-the-forward conversion with positive expiry, exact
shifted levels, analytic inversion and replay of the actual target
volatility against absolute premium and relative boundary controls.

[MODELS-ANALYTIC-23](hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures)
was accepted on **2026-11-10** for one checked constant or scheduled Gaussian
law, stable affine and bond-option pricing, explicit term and continuous
payment measures, exact simple futures bias and a price-equivalent term ATM
Normal quote.

[MODELS-ANALYTIC-24](rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection)
was accepted on **2026-11-10** for a dated ACT/365F model clock mapped to
source-owned curve clocks, an exact shifted Gaussian curve fit, checked
projection on the actual index period and coherent fitted-state selection.

[MODELS-ANALYTIC-25](hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling)
was accepted on **2026-11-10** for exact joint centered-rate and log-bank
steps, explicit event and Philox controls, checked pathwise discounting
and pair-based sampling uncertainty.

[MODELS-ANALYTIC-26](hw1f-issuer-lsmc.md#accepted-decision-for-checked-issuer-call-policy-training-and-pricing)
was accepted on **2026-11-10** for actual dated issuer-call cash and
exact decision order, checked state-based continuation regression, and
independent pricing of a frozen admissible policy.

[MODELS-ANALYTIC-27](diebold-li-var.md#accepted-decision-for-checked-nelsonsiegel-factors-and-var-forecasts)
was accepted on **2026-11-10** for explicit observation cadence, checked
full-rank Nelson–Siegel and VAR fits, strict stationarity and marginal
fitted-yield prediction intervals.

[MODELS-ANALYTIC-28](yield-pca.md#accepted-decision-for-checked-yield-change-pca-and-scenarios)
was accepted on **2026-11-11** for checked centered-data SVD, full-spectrum
variance accounting, qualified cadence and maturity identity, and
fitted-basis reconstruction and scenarios.

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation | Scope and evidence limits |
|---|---|---|---|---|---|
| Spot European price | BSM direct call/put, deterministic/expiry branches | Explicit BlackScholes; checked or unchecked | [MODELS-ANALYTIC-01](spot-vanilla-and-greeks.md) | Observed; source and evidence links in record; not executed | Observed floor/domain; full checked/native-unit/availability package accepted2026-11-09 |
| Spot Greeks and request result | Analytic delta/gamma, percentage-point vega/rho, per-day theta | Calculation always computes Greeks | [MODELS-ANALYTIC-01](spot-vanilla-and-greeks.md) | Observed; source and evidence links in record; not executed | Observed mixed units/ties; raw partials, no fabricated singular bundles and explicit composite accepted2026-11-09 |
| Forward European price | Black76, Bachelier, shifted Black; df/annuity scaling | Explicit model/convention | [MODELS-ANALYTIC-02](forward-normal-shifted.md) | Observed; source and evidence links in record; not executed | Observed scale variants; complete unit-price/shift/numerical package accepted2026-11-09 |
| Forward Greeks | Analytic unit-vol vega; zero-vol delta tie | Forward API is undiscounted | [MODELS-ANALYTIC-02](forward-normal-shifted.md) | Observed; source and evidence links in record; not executed | Observed scale mismatch; matching unit Greeks/nullable boundaries/full shifted bundle accepted2026-11-09 |
| Price-to-vol inversion | OTM parity,20 bisections,4 Halley/Newton refinements | Fixed floors/caps, final repricing check | [MODELS-ANALYTIC-03](implied-volatility-solvers.md) | Observed; source and evidence links in record; not executed | Complete explicit-model/controls, bounded Brent and paired-gate package accepted 2026-11-10 |
| Geometric average | Lognormal average law, equal-time or explicit-time covariance | Explicit geometric; n=0 raw continuous limit | [MODELS-ANALYTIC-04](asian-moments.md) | Observed; source and evidence links in record; not executed | Observed timing gap. Explicit dated expectations and exact seasoned law accepted 2026-11-10 |
| Arithmetic average | Two-moment lognormal matching, O(n) covariance sum | Arithmetic dispatch default | [MODELS-ANALYTIC-04](asian-moments.md) | Observed; source and evidence links in record; not executed | Future-only moment match, exact support branches and accuracy limits accepted 2026-11-10 |
| Continuous barriers | Image terms by K/H regime, knockout parity | Explicit up/down and in/out | [MODELS-ANALYTIC-05](barriers-and-rebates.md) | Observed; source and evidence links in record; not executed | Direct requested reflection-law premium and inclusive live window accepted 2026-11-10 |
| Touch/rebate cashflows | First-passage probability, hit Laplace transform, negative-rate Simpson | Explicit timing; knock-in rebate at expiry | [MODELS-ANALYTIC-05](barriers-and-rebates.md) | Observed; source and evidence links in record; not executed | Independent cash tails, bounded work and dated settlement ownership accepted 2026-11-10 |
| Extremum options | Fixed/floating formulas, carry-zero limits, historical repair | Explicit fixed/floating; fixed default | [MODELS-ANALYTIC-06](lookback-extrema.md) | Observed; source and evidence links in record; not executed | Qualified extrema, common positive continuation and exact carry limits accepted 2026-11-10 |
| Quanto option | Foreign carry−rho asset-vol FX-vol; domestic discount | Explicit scalar quanto inputs | [MODELS-ANALYTIC-07](quanto-drift.md) | Observed; source and evidence links in record; not executed | Explicit orientation, scaled covariance and pre-conversion output accepted 2026-11-10 |
| Heston probability CF | Little-trap form, small-argument series and status reporting | Explicit parameters; Feller opt-in | [MODELS-ANALYTIC-08](heston-characteristic-function.md) | Observed; source and evidence links in record; not executed | Checked contours, g-free affine law and exact limits accepted 2026-11-10 |
| Heston Fourier price | Cached GL probabilities, tail gates and six-grid refinement | Adaptive omitted settings or explicit starting grid | [MODELS-ANALYTIC-09](heston-fourier-integration.md) | Observed; source and evidence links in record; not executed | Analytic truncation bound, fixed-cutoff mesh and time-value gates accepted 2026-11-10 |
| COS inversion | Cumulant window, analytic put coefficients, CF parity call | N=128,L=10 defaults; explicit controls | [MODELS-ANALYTIC-10](cos-inversion.md) | Observed; source and evidence links in record; not executed | Separate window and series bounds, direct bounded payoffs and explicit refusal accepted 2026-11-10 |
| Jump-return law | BS Gaussian, compensated Merton, variance-gamma | Explicit law/wrapper | [MODELS-ANALYTIC-11](jump-characteristic-laws.md) | Observed; source and evidence links in record; not executed | Checked normalized contours, stable cumulants, moment domains and exact support accepted 2026-11-10 |
| Rough Heston | Implicit fractional Riccati plus fixed Lewis contour | H∈(0,.5);200..1600/3200 time steps | [MODELS-ANALYTIC-12](rough-heston-riccati.md) | Observed; source and evidence links in record; not executed | Checked implicit roots, enclosed tail, separate time and frequency checks accepted 2026-11-10 |
| SABR quote formula | Normal beta0/general beta, lognormal/shifted Hagan, chi series | Automatic quote from beta or explicit expansion | [MODELS-ANALYTIC-13](sabr-expansions.md) | Observed; source and evidence links in record; not executed | Explicit quote type, exact CEV distance and checked asymptotic limits accepted 2026-11-10 |
| SABR parameter fit | Vega-normalized relative residual,9 transformed LM starts | Fixed beta; default tolerance1e−4 | [MODELS-ANALYTIC-14](sabr-calibration.md) | Observed; source and evidence links in record; not executed | Explicit fit controls, native quote cap and ordered bounded starts accepted 2026-11-10 |
| SABR shift/ATM pin | Standard shifts with10bp headroom, variance interpolation, inner Newton | Explicit None/Fixed/Auto; pin off by default | [MODELS-ANALYTIC-14](sabr-calibration.md) | Observed; source and evidence links in record; not executed | Literal shift and observed-ATM pin on increasing Brent bracket accepted 2026-11-10 |
| SABR delta strike | ATM-vol normal/Black inversion | Absolute premium-unadjusted delta | [MODELS-ANALYTIC-15](sabr-smile-repair.md) | Observed; source and evidence links in record; not executed | Explicit quote and returned-strike delta gate accepted 2026-11-10 |
| SABR grid repair | Monotonic/convex greedy price reduction, IV inversion, recheck | Explicit repair iteration budget | [MODELS-ANALYTIC-15](sabr-smile-repair.md) | Observed; source and evidence links in record; not executed | Exact down-only hull, finite-grid checks and native move limit accepted 2026-11-10 |
| Surface quote lookup | Vol or total-variance bilinear; strict/clamped axes | Stored mode, explicit query variant | [MODELS-ANALYTIC-16](volatility-source-interpolation.md) | Observed behavior and independent scalar checks | Exact support, common displacement and explicit boundary accepted on 2026-11-10 |
| SABR cube evaluation | Interpolated parameters/forward, nearest beta; optional expiry variance | Stored mode, explicit normal/lognormal output | [MODELS-ANALYTIC-16](volatility-source-interpolation.md) | Observed behavior and independent scalar checks | Common-beta parameter mode and checked row quote blend accepted on 2026-11-10 |
| Materialization/shift metric | Clamped slice sampling; point or middle-strike average vol-point shift | Explicit slice/optional reference point | [MODELS-ANALYTIC-16](volatility-source-interpolation.md) | Observed behavior; complete-path checks pending | Strict sampling with provenance and native comparable moves accepted on 2026-11-10 |
| FX smile conversion | RR/BF arithmetic wings, DNS ATM, forward-delta strikes | 25delta wings with optional10delta | [MODELS-ANALYTIC-17](fx-delta-smiles.md) | Observed behavior and independent scalar checks | Checked side transforms, returned-strike gate and complete labelled pillars accepted 2026-11-10 |
| FX smile lookup | Own-expiry linear strike grids; direct quote-expiry interpolation | Direct delta source or materialization | [MODELS-ANALYTIC-17](fx-delta-smiles.md) | Observed behavior and independent scalar checks | Strict expiry, explicit strike wings and exact-union materialization accepted 2026-11-10 |
| SVI formula/density | Raw total variance, Lee/minimum bounds, analytic Durrleman g | Explicit diagnostics | [MODELS-ANALYTIC-18](svi-fit-and-density.md) | Observed behavior and independent scalar checks | Exact shape bounds, strict right wing and checked finite scan accepted 2026-11-10 |
| SVI calibration | Single-start transformed LM on total variance | Default300 iterations,1e−12; solver override | [MODELS-ANALYTIC-18](svi-fit-and-density.md) | Observed behavior and independent scalar checks | Explicit seed and controls, native-vol replay and sampled-density gate accepted 2026-11-10 |
| Dupire extraction | Fixed-k time differences, nonuniform strike derivatives | Black strike surfaces and supplied forwards | [MODELS-ANALYTIC-19](dupire-local-volatility.md) | Observed behavior and independent scalar checks | Explicit matched support, nonuniform stencils and checked ratio accepted 2026-11-10 |
| Local-vol smoothing/query | Gaussian implied-vol smoothing, bilinear local-vol lookup | Explicit strike-unit bandwidth | [MODELS-ANALYTIC-19](dupire-local-volatility.md) | Observed behavior and independent scalar checks | Explicit smoothing and grid boundary with simulation reconciliation accepted 2026-11-10 |
| Surface arbitrage report | Butterfly/calendar/Dupire scans, severity and filtered pass flag | Three checks on by default; forwards required | [MODELS-ANALYTIC-20](surface-arbitrage-diagnostics.md) | Observed behavior and independent scalar checks | Sampled call constraints, strict calendar support, independent Dupire signs and complete verdict accepted 2026-11-10 |
| Variance replication | OTM trapezoids, log-anchor adjustment, fixed flat-vol tails | User strike grid/callbacks | [MODELS-ANALYTIC-21](variance-replication.md) | Observed behavior and independent scalar checks | Exact-forward split, checked Black prices, mesh gate and explicit wings accepted 2026-11-10 |
| ATM convention conversion | Source price then Brent target solve, scale seed | Explicit source/target convention | [MODELS-ANALYTIC-22](atm-volatility-conversion.md) | Observed behavior and independent scalar checks | Positive expiry, exact shifts, analytic inverse and actual sigma replay accepted 2026-11-10 |
| HW affine bonds/options | Stable B, integrated scheduled variance, affine A, Black ZCB option | Scalar/scheduled params; caller curves | [MODELS-ANALYTIC-23](hull-white-analytic-kernels.md) | Observed behavior and independent Gaussian checks | Common model state, exact limits and scaled original-leg pricing accepted on 2026-11-10 |
| HW caplets/payment measure | Bond-option transform or ratio loading with delayed-pay covariance | Explicit periods; deterministic projection basis | [MODELS-ANALYTIC-23](hull-white-analytic-kernels.md) | Observed behavior and independent scalar checks | Explicit term periods and separate continuous accumulation accepted on 2026-11-10 |
| HW convexity/normal vol | Leading futures adjustment, coupon covariance factors, normal-vol proxy | Explicit helpers | [MODELS-ANALYTIC-23](hull-white-analytic-kernels.md) | Observed behavior and independent scalar checks | Exact quote-dependent bias and price-equivalent term ATM Normal quote accepted on 2026-11-10 |
| Rate clock/curve fit | Actual-days/365 to source convention, rebasing, monthly theta setup | Explicit as_of/source curve/horizon | [MODELS-ANALYTIC-24](rate-clock-and-projection.md) | Observed behavior and independent scalar and Gaussian checks | Source-owned clock composition and exact deterministic shift accepted on 2026-11-10 |
| Conditional period projection | Affine bond ratio to simple forward plus additive basis | Explicit coeffs or flat-rate representation | [MODELS-ANALYTIC-24](rate-clock-and-projection.md) | Observed behavior and independent scalar and Gaussian checks | Actual fixing and index dates, checked gross basis and literal known fixings accepted on 2026-11-10 |
| HW MC paths/discount | Exact rate marginal, Philox antithetics, event grid, trapezoid bank | 10000 streams/seed42/4 minimum substeps defaults | [MODELS-ANALYTIC-25](hw1f-monte-carlo-discounting.md) | Observed behavior and independent Gaussian checks | Exact joint Gaussian transitions and checked log-bank discounting accepted on 2026-11-10 |
| HW estimate statistics | Average antithetic pairs then independent-estimator stats | Optional held-out odd streams | [MODELS-ANALYTIC-25](hw1f-monte-carlo-discounting.md) | Observed behavior and independent sampling checks | Explicit Philox controls and complete pair estimates with checked Student-t uncertainty accepted on 2026-11-10 |
| Issuer exercise regression | Backward continuation, coupon separation, polynomial bases | Optional OOS; issuer minimizes amount vs continuation | [MODELS-ANALYTIC-26](hw1f-issuer-lsmc.md) | Observed behavior and independent cash and policy checks | Dated branch cash, checked full-rank fit and independent frozen-policy pricing accepted on 2026-11-10 |
| Historical NS/VAR | Fixed-lambda OLS factors, VAR(1), Gaussian marginal bands | lambda=.7308 inverse years default | [MODELS-ANALYTIC-27](diebold-li-var.md) | Observed behavior and independent scalar and VAR checks | Explicit observation cadence, strict stationarity and conditional marginal intervals accepted on 2026-11-10 |
| Yield-change PCA | Centered sample covariance, clamped eigenvalues, sign normalization | All components retained; explicit truncation | [MODELS-ANALYTIC-28](yield-pca.md) | Observed behavior and independent arithmetic checks | Centered-data SVD, numerical retention and scoped fitted axes accepted on 2026-11-11 |
| PCA scenarios/reconstruction | SD-scaled loading shocks; score reconstruction+mean | Explicit component shocks/count | [MODELS-ANALYTIC-28](yield-pca.md) | Observed behavior and independent arithmetic checks | Actual tenors and cadence, full-spectrum thresholds and checked scenarios accepted on 2026-11-11 |

## Evidence boundaries

[QuantLib/mpmath fixture runner](../../../../finstack-quant/models/tests/vol_models_quantlib_tests.rs) and [generator](../../../../finstack-quant/models/tests/golden/data/gen_vol_golden.py) provide separate reference values for selected vanilla, SABR, Heston and SVI formula cases. IV and calibration round trips, scalar/strip parity and synthetic recovery are consistency evidence, not independent economic validation. Rough-Heston classical-limit comparison does not certify finite-H behavior; local-vol SSVI checks exclude edges. Rate LSMC coupon tests do not certify the exercise boundary or claimed bias bracket.

## Follow-up tracking

Adoption questions, implementation discrepancies and missing validation are tracked in the [analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models). The [central follow-ups](../../FOLLOW_UPS.md#financial-methodology-audit) coordinate cross-domain work.
