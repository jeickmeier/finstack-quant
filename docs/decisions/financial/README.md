# Financial methodologies

A reference explains a method. A decision records why we use it, where it
applies, and what behavior callers can rely on.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

The current catalogue maps **1,724 Rust files** to **625 detailed methodology
records across 34 source scopes**. Scope ledgers distinguish original audit
fingerprints from later source-ownership additions. Accepted scopes are
identified below; remaining choices retain their stated Proposed or Draft status.

- [Shared financial conventions](conventions/README.md): units, defaults, missing data, monetary arithmetic, perspectives and clocks.
- [Methodology governance](governance/README.md): documentation policy and the [record template](governance/METHODOLOGY_TEMPLATE.md).
- [Source coverage](SOURCE_COVERAGE.md): file ownership, record mappings and inspected responsibilities.
- [Algorithm follow-ups](FOLLOW_UPS.md): observed discrepancies, investigations, pending decisions and missing evidence.
- [Central follow-ups](../FOLLOW_UPS.md#financial-methodology-audit): cross-domain coordination.
- [Audit boundaries](AUDIT_BOUNDARY.md) and [canonical references](../../REFERENCES.md).

## Coverage

Each scope contains its calculation inventory, detailed records and source
ledger. Relevant ADRs live beside their owning catalogue; cross-family
overviews are linked from the other affected scopes. Shared policies live in
`conventions/` and `governance/`, which are not additional source-audit scopes.

| Scope and inventory | Calculations | Rust files | Detailed records | Source ledger |
| --- | --- | ---: | ---: | --- |
| [Core: dates, cashflows, money and conventions](core-conventions/README.md) | Day counts, calendars, schedules, FX dates/quotation, monetary representation, NPV/IRR, ratings | 57 | 20 | [Coverage](core-conventions/coverage.json) |
| [Core: numerical and expression methods](core-numerics/README.md) | Interpolation, roots/least squares, quadrature, matrix/correlation methods, distributions, statistics, RNGs, grids and expression evaluation | 65 | 26 | [Coverage](core-numerics/coverage.json) |
| [Core: market representations and transformations](core-market/README.md) | Discount/forward/hazard/CPI/price curves, volatility representations, FX quotes, shocks, rolls and market-state semantics | 60 | 22 | [Coverage](core-market/coverage.json) |
| [Cashflow construction and analysis](cashflows/README.md) | Schedule compilation, event ordering, fixed/floating/overnight coupons, PIK, amortization, fees, default/recovery, accrual and aggregation | 35 | 16 | [Coverage](cashflows/coverage.json) |
| [Models: analytic pricing, volatility and rates](models-analytic/README.md) | Closed forms, inverse volatility, Heston/rough Fourier, COS/jumps, SABR/SVI/local volatility, rate models and empirical term structures | 58 | 28 | [Coverage](models-analytic/coverage.json) |
| [Models: trees and finite differences](models-numerical/README.md) | CRR/LR, Ho–Lee/BDT/BK/HW, joint rates-credit lattices, grids/stencils, theta/ADI stepping, obstacles and PDE bridges | 36 | 17 | [Coverage](models-numerical/coverage.json) |
| [Models: simulation, payoffs and estimation](models-monte-carlo/README.md) | RNG/QMC, process-specific stepping, rough drivers, monitoring, path-dependent payoffs, regression/exercise, Greeks and uncertainty | 78 | 28 | [Coverage](models-monte-carlo/coverage.json) |
| [Models and valuations: credit and liquidity](models-and-valuations-credit/README.md) | Structural/intensity/default/recovery/prepayment models, copulas/correlation, loss distributions, liquidity, CDS/index/options/tranches and risk | 131 | 20 | [Coverage](models-and-valuations-credit/coverage.json) |
| [Valuations: shared framework and risk](valuations-framework/README.md) | Market/convention resolution, lifecycle, projection, listed products, model controls, risk bumps/recalibration, Greeks, historical risk and reporting | 129 | 20 | [Coverage](valuations-framework/coverage.json) |
| [Valuations: rates](valuations-rates/README.md) | Deposits/FRAs, swaps/basis/XCCY, futures, inflation, repo, CMS, vanilla/early-exercise rate options and their metrics | 101 | 17 | [Coverage](valuations-rates/coverage.json) |
| [Valuations: bonds](valuations-bonds/README.md) | Settlement/quotes, discount/hazard/tree/joint/Merton pricing, yield/spread/OAS inverses, exercise/return floors and bond risk/returns | 71 | 20 | [Coverage](valuations-bonds/coverage.json) |
| [Valuations: other fixed income](valuations-other-fixed-income/README.md) | Loans/revolvers, convertibles, inflation bonds, MBS/TBA/dollar rolls, futures and fixed-income TRS | 87 | 16 | [Coverage](valuations-other-fixed-income/coverage.json) |
| [Valuations: equity, property and composition](valuations-equity-and-composition/README.md) | Equity/options/path products, DCF/private equity, real estate, baskets, composite sizing/history and selected-model risk | 99 | 24 | [Coverage](valuations-equity-and-composition/coverage.json) |
| [Valuations: FX, commodity and exotic products](valuations-fx-and-exotics/README.md) | FX linear/option/barrier/touch/variance contracts; commodity curves/averages/options; Asian/barrier/lookback/range/snowball/TARN calculations | 113 | 22 | [Coverage](valuations-fx-and-exotics/coverage.json) |
| [Valuations: structured credit](valuations-structured-credit/README.md) | Statistical/instrument collateral, waterfall claims/triggers, servicing/recovery/reinvestment, stochastic pools/OAS, CMO and facilities | 100 | 22 | [Coverage](valuations-structured-credit/coverage.json) |
| [Calibration: orchestration, solvers and replay](calibration-framework/README.md) | Plan validation and dependencies, quote/build materialization, bootstrap/global solvers, acceptance/reporting, quote risk and recalibration | 50 | 23 | [Coverage](calibration-framework/coverage.json) |
| [Calibration: quotes and curve/smile targets](calibration-targets/README.md) | Discount/forward/hazard/CPI/XCCY curves, parametric term structures, SABR/SVI/swaption surfaces, base correlation and tail calibration | 25 | 17 | [Coverage](calibration-targets/coverage.json) |
| [Calibration: Hull–White and LMM](calibration-rate-models/README.md) | Contractual/synthetic option targets, scalar/piecewise Hull–White fits, objectives and schedules, LMM covariance and parameter fits | 12 | 9 | [Coverage](calibration-rate-models/coverage.json) |
| [Analytics](analytics/README.md) | Returns and windows, wealth/ratios, drawdowns and tail risk, rolling statistics, benchmarks/capture, regression and multifactor estimation | 23 | 15 | [Coverage](analytics/coverage.json) |
| [Statements: contracts, formulas and checks](statements-contracts/README.md) | Model and builder validation, DSL compilation, built-in financial metrics, input/registry conventions and statement checks | 36 | 16 | [Coverage](statements-contracts/coverage.json) |
| [Statements: evaluation, forecasting and adjustments](statements-evaluation/README.md) | Dependency/evaluation ordering, formula/time-series semantics, forecast sampling, periods/rollups, FX/capital integration and normalization | 28 | 15 | [Coverage](statements-evaluation/coverage.json) |
| [Statements: capital structure](statements-capital/README.md) | Dated debt and hedge flows, principal/PIK balances, residual schedules, funding/prepayment waterfalls and capital results | 19 | 9 | [Coverage](statements-capital/coverage.json) |
| [Statements analytics: valuation and analysis](statements-analytics-valuation/README.md) | Corporate/LBO valuation, comparable companies, goal seek and scenarios, backtesting/reports, scorecards and operating templates | 32 | 16 | [Coverage](statements-analytics-valuation/coverage.json) |
| [Statements analytics: credit and financial checks](statements-analytics-credit/README.md) | ECL/CECL projection/staging/reversion, credit measures, covenant adapters and financial consistency/reconciliation checks | 30 | 18 | [Coverage](statements-analytics-credit/coverage.json) |
| [Scenarios](scenarios/README.md) | Shock composition and execution, market/statement/instrument adapters, curve repair/calibration, horizon roll and partial-failure semantics | 28 | 17 | [Coverage](scenarios/coverage.json) |
| [Portfolio: execution, risk and aggregation](portfolio-execution/README.md) | State/dependency reuse, materialization and valuation, cashflows/FX/metrics, sensitivity/scenarios, margin aggregation and replay | 41 | 17 | [Coverage](portfolio-execution/coverage.json) |
| [Portfolio: attribution, allocation and optimization](portfolio-attribution/README.md) | Attribution composition, Brinson/fixed-income/factor/excess-return analysis, return measures, allocation, optimization and factor aggregation | 23 | 17 | [Coverage](portfolio-attribution/coverage.json) |
| [Attribution: instrument P&L decomposition](attribution/README.md) | Waterfall/parallel/Taylor/metric attribution, factor shocks and interactions, carry/roll, cashflow/FX translation and reconciliation | 29 | 16 | [Coverage](attribution/coverage.json) |
| [Margin: collateral and initial margin](margin-collateral-im/README.md) | SIMM/schedule/CCP calculations, CSA/variation-margin and collateral rules, registry versions and tabular sensitivity contracts | 34 | 22 | [Coverage](margin-collateral-im/coverage.json) |
| [Margin: regulatory risk and XVA](margin-regulatory-xva/README.md) | SA-CCR stages, FRTB sensitivities/curvature/default/residual risk, CVA/DVA/FVA/MVA integration and exposure profiles | 29 | 23 | [Coverage](margin-regulatory-xva/coverage.json) |
| [Factor models](factor/README.md) | Matching and factor identity, covariance/PSD handling, risk/tail allocations, credit-factor calibration and forecasts, decomposition and validation | 39 | 19 | [Coverage](factor/coverage.json) |
| [Covenants](covenants/README.md) | Metric/window resolution, threshold amendments, waiver/cure and consequence state, deterministic/stochastic forecasts and reporting | 14 | 13 | [Coverage](covenants/coverage.json) |
| [Panel features](features/README.md) | Panel indexing and pipelines, cross-sectional transforms/weights, time-series windows and moments, regression and multivariate transforms | 11 | 22 | [Coverage](features/coverage.json) |
| [Arrow interchange](arrow-interchange/README.md) | Tabular type/schema mapping, typed-array conversion, null/nonfinite behavior, metadata and record-batch validation | 1 | 3 | [Coverage](arrow-interchange/coverage.json) |

The classifications are **1379 algorithm-bearing**, **288 support**,
**53 dedicated test**, and **4 generated** Rust files. The
[readable source map](SOURCE_COVERAGE.md) links every source to its records or
explains its non-algorithm responsibility. A source can implement several
choices; several sources can implement one coherent choice. File counts are
coverage evidence, not a measure of financial correctness.

## Existing financial conventions

These accepted records constrain methodology decisions within their stated
scope; they do not independently adopt every implementation that uses the
convention.

| Record | Accepted scope |
| --- | --- |
| [0007](conventions/0007-units-and-sign-conventions.md) | Financial units and signs are public contracts. |
| [0008](conventions/0008-financial-defaults-and-cagr.md) | Explicit financial defaults; dated-window CAGR uses Actual/365 Fixed. |
| [0009](conventions/0009-missing-and-invalid-data.md) | Distinguish invalid inputs, permitted gaps, and undefined results. |
| [0010](valuations-framework/0010-metric-identifiers.md) | Preserve metric identity and attribution. |
| [0011](valuations-bonds/0011-bond-cs01-without-hazard-curve.md) | Supported no-hazard bond CS01 uses z-spread bump-and-reprice; option-bearing scope needs reconciliation. |
| [0020](conventions/0020-numerical-representation-and-rounding.md) | Distinguish model numerics, monetary precision, and explicit rounding. |
| [0044](conventions/0044-financial-perspectives-and-boundary-conversions.md) | Name financial perspectives and convert explicitly; retain the dividend magnitude default with an explicit signed alternative, and distinguish total collateral from its components. Implementation and regulatory qualification remain open. |
| [0045](conventions/0045-financial-clocks-and-date-boundaries.md) | Preserve separate clocks, explicit volatility mapping, dated-use/variance and publication-eligibility requirements. Operation-specific defaults and implementation/verification remain separately owned. |
| [Core market: discount sources and rate extraction](core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction) | Complete explicit clock/style/tail, normalized positive-pillar/signed-query, negative-rate admission, fallible relative/zero/interval/analytic-local-rate and strict state/host package. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: index projection factors](core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates) | Complete positive-factor source, authoritative index association, resolved index-period growth/contractual accrual, removal of seed/grid/mean inference and strict caller/state/host package. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: rate shocks and conditioning](core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning) | Complete additive continuous-zero shock, explicit units/exact bucket geometry, retained complete evaluator and original-clock dated conditioning package. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: intensity and default probabilities](core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation) | Complete strict ending-intensity source, explicit clock, local survival/default integration, probability-versus-finite-J range, no cap/origin repair and dated conditioning package. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: CDS quotes, hazard risk and recovery](core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery) | Complete removal of ambiguous quote/triangle views, explicit observations/pricing, direct intensity versus quote-recalibrated risk, exact targets and separate recovery operations. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: construction records and replay](core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification) | Complete lossless effective history, calibration-owned semantics, atomic quote bindings, explicit source/target dependencies and request-time replay qualification. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: NS/NSS analytic factors](core-market/parametric-discount.md#accepted-decision-validated-nsnss-sources-and-analytic-factor-queries) | Complete validated NS/NSS parameter/source, explicit clock/domain, stable coefficient-aware evaluation, analytic trait queries and retained-model conditioning/overlay package. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: quoted levels and price parameters](core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks) | Complete explicit quoted-level source without spot anchoring, checked units/domains, native/relative source-parameter risk and original-clock frozen reuse. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: dated zero-rate differences](core-market/basis-spreads.md#accepted-decision-source-derived-dated-zero-rate-differences) | Complete standalone basis-curve/byproduct removal, explicit dated source-pair/common-clock diagnostic, checked log arithmetic and source-versus-quote-risk boundaries. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: CPI sources and annual parameter risk](core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks) | Complete explicit positive anchored CPI source, checked source-clock growth, stable original-anchor annual-parameter shocks and frozen absolute dated reuse; remove core lag/implicit indexation conveniences. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: monthly CPI fixings and knowledge](core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability) | Complete exact monthly fixing snapshots, explicit version/knowledge cutoff and actual-versus-expected release facts, raw observed-source semantics and shared contractual reference resolution. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: scalars, series and exact fixings](core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings) | Complete validated scalar owner, strict explicit sampled series, native/factor transforms and exact observed-index/CMS fixing identity with publication qualification and fixing-history bump refusal. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: identified cash dividends](core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates) | Complete cash-only per-share event source, explicit identity/ex/pay dates and underlying/share/currency basis, immutable reuse and qualified financial adapters; remove stock/yield event variants. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core market: volatility grids and sigma risk](core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks) | Complete explicit quote/axis/clock grid, positive expiry and convention domains, native sigma shocks with exact targets, zero admission and exact scoped scratch restoration. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core: actual-day conventions](core-conventions/001-actual-day-counts.md#accepted-decision-actual-day-conventions) | Complete explicit seven-variant formula, AFB interpretation, Bus252 count/context, validation, signed/zero and numerical/error package; implementation and independent qualification remain open. |
| [Core: coupon-dependent day counts](core-conventions/002-coupon-context-day-counts.md#accepted-decision-coupon-context-day-counts) | Complete explicit contained ICMA reference/frequency, schedule-owned stub composition, ISDA end-year ACT/365L and context/tenor/host boundary package; implementation and independent qualification remain open. |
| [Core: synthetic day counts](core-conventions/003-thirty360.md#accepted-decision-synthetic-day-count-conventions) | Complete five-variant 30/360 selection/formulas, explicit termination status, canonical interval API, signed/zero and numerical/error package. Reconciled 2026-11-09 with a QuantLib-regenerated fixture; independent qualification remains open. |
| [Core: holiday engine](core-conventions/004-holiday-engine.md#accepted-decision-holiday-engine-and-data-boundaries) | Complete rule/nominal-year/transform semantics, fallible coverage boundaries, exact cache equivalence, strict frozen inputs and provenance package; jurisdiction selection, implementation and independent qualification remain separately owned. |
| [Core: jurisdiction calendars](core-conventions/005-jurisdiction-calendars.md#accepted-decision-jurisdiction-calendar-purposes-and-coverage) | Complete purpose/coverage/correction package for all 28 audited IDs: 26 retained, generic CME/mainland-banking IDs retired, bounded annual-data/forecast permissions and legal epochs. Runtime data reconciliation and independent qualification remain open. |
| [Core: rolling and joint calendars](core-conventions/006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions) | Complete explicit selection, six roll definitions, checked unbounded-by-100 search, identity/offset/count and strict joint/error/persistence package. Operation-owned defaults, runtime reconciliation and qualification remain separate. |
| [Core: tenor and date arithmetic](core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions) | Complete unit-preserving tenor/technical limits, checked calendar/completed-month arithmetic, explicit rolling/context, named approximations, nominal payment/compounding boundaries, epoch and host-domain package. Runtime reconciliation and independent qualification remain open. |
| [Core: schedule grids and events](core-conventions/008-schedule-generation.md#accepted-decision-schedule-grids-events-and-failure-boundaries) | Complete explicit anchored grid/stub/EOM/IMM selection, CDS-mode removal, separate payment/fixing recipes, strict atomic failure, final-output limit, nominal-reference and validated state/host package. Product-owned actual accrual selection, runtime reconciliation and independent qualification remain separate. |
| [Core: unadjusted roll dates](core-conventions/009-contractual-roll-dates.md#accepted-decision-unadjusted-roll-dates-and-product-boundaries) | Complete all-month/quarterly/semiannual lattice, strict/inclusive search, option-expiry-helper removal and date/error/host package. Actual product expiry, CDS tenor/regime composition, runtime reconciliation and independent qualification remain separately owned. |
| [Core: published SIFMA settlement](core-conventions/010-sifma-settlement.md#accepted-decision-published-settlement-dates-classes-and-coverage) | Complete explicit-class exact/strict-next lookup, required-data errors, unmodified Settlement cells, estimator/class-inference removal and frozen coverage/provenance/refresh/host package. Product selection, data/runtime reconciliation and independent qualification remain separate. |
| [Core: FX quote and value-date conventions](core-conventions/011-fx-date-and-quote-conventions.md#accepted-decision-explicit-fx-quote-metadata-and-value-dates) | Complete ordered explicit quote/pip metadata, checked reciprocal, generic spot/default-lag removal and explicit-calendar Natural/conditional business-EOM forward-date package. Product policy/caller/host reconciliation and independent current-market qualification remain separate. |
| [Core: monetary precision, rounding and currency identity](core-conventions/012-money-decimal-precision.md#accepted-decision-monetary-precision-rounding-and-currency-identity) | Complete exact admission/wire, shortest-text float conversion, exact sums, bounded transform/model-view accuracy, explicit financial rounding, Decimal-rate FX and alphabetic currency/frozen metadata package. Runtime/data reconciliation and independent qualification remain open. |
| [Core: FX resolution, state and shocks](core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks) | Complete explicit point-policy/routing, average-policy removal, scoped authority, directed cache, missing-only fallback, coherent complete snapshots, atomic fresh restoration and frozen directional pair-shock package. Runtime/host/caller reconciliation and independent qualification remain open. |
| [Core: reporting periods, fiscal context and LTM](core-conventions/014-fiscal-periods-ltm.md#accepted-decision-reporting-periods-fiscal-context-and-ltm-windows) | Complete Gregorian/fiscal partitions, explicit context, strict grammar/stepping/total ordering, separate statistical factors, validated state and reporting-period/calendar-day LTM with exact completeness and weekly exclusion. Runtime/host/caller reconciliation and independent qualification remain open. |
| [Core: cash settlement, principal effects and accrual](core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata) | Complete cash-only amount/zero-cash noncash representation, explicit paired balance effects/dates, exhaustive kinds/predicates, complete single-slice context, immutable validated state and faithful event/cash export package. Runtime/product/host reconciliation and independent qualification remain open; 016 NPV choices remain separate. |
| [Core: dated NPV and continuous discounting](core-conventions/016-discounting.md#accepted-decision-dated-npv-selection-arithmetic-and-continuous-discounting) | Complete explicit same-day/input/empty/zero selection, source-owned dated factors, strict numeric failures, 012 Money arithmetic, ordered checked scalar compensation and positive continuous flat-factor package with stated accuracy/currency limits. Runtime/product/numeric reconciliation and independent qualification remain open. |
| [Core: IRR/XIRR timing, eligibility and acceptance](core-conventions/017-irr.md#accepted-decision-irr-and-xirr-timing-eligibility-and-numerical-acceptance) | Complete per-period/fixed-ACT365F return, canonical rounded date-netting, one-switch eligibility, guess removal, finite log-bracket/shared Brent and common finite rate/residual package with explicit numerical/product limits. Runtime/solver/caller reconciliation and independent qualification remain open. |
| [Core: Rate and Bps units/admission](core-conventions/018-rate-units.md#accepted-decision-rate-and-bps-units-admission-and-precision) | Complete finite model Rate/exact whole-i32 Bps, checked fractional unit conversion, single-final text admission, quantizer removal, exact integer division, truthful display and host/wire/product-boundary package. Reconciled 2026-11-08 in core, hosts and the term-loan step-up consumer. |
| [Core: generic ratings and explicit scorecard scales](core-conventions/019-credit-ratings.md#accepted-decision-generic-ratings-and-explicit-scorecard-scales) | Complete NR-outside-quality/strict-symbol/default-marker package, fallible Moody rendering, removal of numeric defaults/fallbacks and explicit immutable custom scale snapshots with complete bands, self-membership, strict references and validated state. Runtime/consumer/host reconciliation and independent qualification remain open. |
| [Core: barrier classification and conditional payment timing](core-conventions/020-barrier-classification.md#accepted-decision-barrier-classification-touch-and-conditional-payment-timing) | Complete explicit relevant choices/default removal, exact inclusive eligible-observation touch, direction/history/conditional entitlement/settlement separation and strict wire/host/consumer-boundary package. Runtime/product/host reconciliation landed 2026-11-08; independent consumer observation/settlement qualification remains open. |
| [Core numerics: explicit compounding and periodic spreads](core-numerics/compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads) | Complete explicit convention/integer-frequency, fallible finite-domain/time-zero/limiting conversion, stable direct log calculation, instantaneous versus finite spread semantics, periodic z-helper identities and strict precision/wire/host/caller-boundary package. Runtime reconciliation and independent qualification remain open. |
| [Core numerics: validated scalar interpolation](core-numerics/value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics) | Complete one-owned-API, explicit Linear/LogLinear/PCHIP and Flat/Extend/Error policy, no gap floor/ValidationPolicy, exact knots/derivative sides, stable fallible arithmetic, strict restored state and helper-removal/owner-constant package. Runtime/host/consumer reconciliation and independent qualification remain open. |
| [Core numerics: natural/Hagan-West interpolation](core-numerics/forward-interpolation.md#accepted-decision-natural-log-discount-and-hagan-west-interpolation) | Complete fixed natural log-discount spline/scaled solve and unsmoothed uncollared Hagan-West, exact region/axis semantics, actual one-sided derivatives, source-anchored tails and stable checked state/evaluation package with explicit continuity/positivity/precision/owner limits. Runtime reconciliation and qualification remain open. |
| [Core numerics: checked quadrature](core-numerics/quadrature.md#accepted-decision-checked-quadrature-budgets-and-measures) | Complete fixed/adaptive/scalar/pair/Gaussian/grid definitions, fallible callbacks, oriented finite integrals, independent absolute component budgets/global estimator gates, depth/evaluation limits, stable checked arithmetic, explicit measures/provenance and true-error/owner limits. Runtime/caller and independent qualification remain open. |
| [Core numerics: checked Brent solving and bounded search](core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search) | Complete canonical fallible callback, explicit validated controls/search, ordered finite sampling, deterministic expansion, stable coordinate termination/work diagnostics and separate financial acceptance package. Runtime/caller reconciliation and independent numerical/financial qualification remain open. |
| [Core numerics: checked damped least squares and outcomes](core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes) | Complete augmented pivoted-QR step, explicit residual/step/attempt controls, fallible residual/Jacobian and checked derivative/norm/scaling/resource contracts, honest outcome/work diagnostics and separate financial acceptance package. Runtime/caller/host reconciliation and independent qualification remain open. |
| [Core numerics: checked matrix factors, solves and correlation admission](core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission) | Complete exact-symmetry, positive-pivot/nonzero-diagonal solve, explicitly truncated generic PSD, original shock ordering, strict correlation structure, bounded sorted eigen and checked arithmetic/error/host package. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core numerics: explicit nearest-correlation projection and repair limits](core-numerics/nearest-correlation.md#accepted-decision-explicit-nearest-correlation-projection-and-repair-limits) | Complete unweighted Higham/Dykstra projection, finite-input preprocessing, six explicit controls, three stopping tests/strict numerical admission, original-input distance refusal and separate financial permission/provenance package. Accepted 2026-11-08; runtime/caller/host reconciliation and independent qualification remain open. |
| [Core numerics: checked sums, moments, quantiles and mean uncertainty](core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty) | Complete ordered checked aggregation, explicit Result/Option availability, shifted/scaled atomic centered state/exact counts, normalized paired ratios, checked R7/filters and explicit-alpha Student-t package. Accepted 2026-11-08; runtime/caller/host and independent numerical/statistical qualification remain open. |
| [Core numerics: explicit log returns and realized variance](core-numerics/realized-variance.md#accepted-decision-explicit-log-returns-and-realized-variance) | Complete checked positive-price/log transforms, explicit method/annualization, all-bar anchored OHLC coverage, precise range/YZ variants and availability, nonnegative stable arithmetic and separate product/host/schema boundaries. Accepted 2026-11-08; runtime/caller/host and numerical/financial qualification remain open. |
| [Core numerics: checked scaled-identity covariance shrinkage](core-numerics/covariance-shrinkage.md#accepted-decision-checked-scaled-identity-covariance-shrinkage) | Complete centered population/Frobenius estimator, genuine ties/cap, stable centering/mixed-scale moments/complement, checked result/errors and existing host/credit-owner reconciliation package. Accepted 2026-11-08; runtime and independent numerical/statistical/financial qualification remain open. |
| [Core numerics: checked special functions and distribution domains](core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains) | Complete checked scalar domains/extended limits, positive-real log-Gamma, stable normal/log-tail evaluation, bounded private Student-t beta machinery, truthful errors/precision and coordinated host/caller ownership package. Accepted 2026-11-08; runtime and independent numerical/statistical/financial qualification remain open. |
| [Core numerics: checked full binomial probability mass](core-numerics/binomial-mass.md#accepted-decision-checked-full-binomial-probability-mass) | Complete strict-domain/one-million-trial resource, exact-mode/FMA relative recurrence, compensated normalization/mass gate, scratch/error and Rust-only/conditional-credit ownership package. Accepted 2026-11-08; runtime/caller and independent numerical/statistical/financial qualification remain open. |
| [Core numerics: checked beta and Gamma sampling](core-numerics/beta-gamma-sampling.md#accepted-decision-checked-beta-and-gamma-sampling) | Complete shape/RNG/zero contract, stable bounded MT acceptance and deferred component ratio, smaller-tail restoration, truthful failures/state advancement and Rust-only/model-owner package. Accepted 2026-11-08; runtime and independent numerical/statistical/financial qualification remain open. |
| [Core numerics: checked chi-squared log-tail inversion](core-numerics/chi-squared-inversion.md#accepted-decision-checked-chi-squared-log-tail-inversion) | Complete closed-endpoint/interior numerical domain, bounded preserved-log Gamma kernels, stable normalizers/bracket/shared Brent, actual-returned-positive-quantile tail gate and Rust-only/copula-owner package. Accepted2026-11-08; runtime/caller and independent numerical/statistical/financial qualification remain open. |
| [Core numerics: strict correlated Bernoulli laws](core-numerics/correlated-bernoulli.md#accepted-decision-strict-correlated-bernoulli-laws) | Complete required-nullable correlation/strict feasibility, preserved cell/conditional/sampling geometry, qualified numerical views, canonical getters/hosts and separate financial/multivariate ownership package. Accepted2026-11-08; runtime/host and independent numerical/statistical/financial qualification remain open. |
| [Core numerics: checked random draws, bridge and Poisson counts](core-numerics/pseudorandom-and-bridge.md#accepted-decision-checked-random-draws-bridge-and-poisson-counts) | Complete explicit PCG/grid, checked reflected transform/fallible cached normals, capped stable bridge, bounded u64 Poisson inverse and complete caller/owner reconciliation package. Accepted2026-11-08; runtime and independent numerical/statistical/financial qualification remain open. |
| [Core numerics: finite-period Sobol points and prefix hash scrambling](core-numerics/sobol-sequences.md#accepted-decision-finite-period-sobol-points-and-prefix-hash-scrambling) | Complete fixed direction/order and explicit seed, honest deterministic net versus randomization, exact midpoint/normal limits, checked finite-period state/buffers, adapter removal and Rust-only/estimator-owner package. Accepted 2026-11-08; runtime and independent numerical/statistical/financial qualification remain open. |
| [Core numerics: checked fractional increment covariance](core-numerics/fractional-covariance.md#accepted-decision-checked-fractional-increment-covariance) | Complete unit-normalized typed-Hurst, validated interval/grid, stable direct/touching/scaled general arithmetic with explicit cancellation refusal, atomic mirrored covariance and caller/schema/financial-owner package. Accepted 2026-11-08; runtime and independent numerical/statistical/financial qualification remain open. |
| [Core numerics: checked simulation grids and exact event knots](core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots) | Complete canonical-knot/actual-dt, explicit-count/zero-grid, exact event/lookup, checked access/resources, honest uniform-layout and financial-snapping removal/caller/host ownership package. Accepted 2026-11-08; runtime and independent numerical/financial qualification remain open. |
| [Core numerics: checked piecewise coefficients and exponential integration](core-numerics/piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration) | Complete right-continuous validated coefficient/query state, exact zero identities without deadbands, sign-stable analytic kernel/deferred scales/positive-total accumulation and OU/Hull–White/tree/host error-chain/financial-owner package. Accepted 2026-11-08; runtime and independent numerical/financial qualification remain open. |
| [Core numerics: checked scalar expressions and decimal quantization](core-numerics/expression-evaluation.md#accepted-decision-checked-scalar-expressions-missing-values-and-decimal-quantization) | Complete bounded eager DAG/static syntax, explicit exact rows, NaN-only missing/three-valued logic/invalid-math errors, exact binary-input rational quantization/checked clamp and caller/statements/host/financial-owner package. Accepted 2026-11-08; runtime and independent language/numerical/financial qualification remain open. |
| [Core numerics: explicit config, local tolerances and producer metadata](core-numerics/rounding-and-config.md#accepted-decision-explicit-rounding-config-local-tolerances-and-producer-metadata) | Complete accepted-Money config/snapshot reconciliation, retired global-zero authority with owner-local comparisons, fixed embedded validation and optional actual-producer metadata without request overlays, plus host/replay qualification. Accepted 2026-11-08; runtime and independent numerical/financial/provenance qualification remain open. |
| [Core numerics: strict numeric wire and canonical content identity](core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity) | Complete symmetric count/exact Decimal/strict date/sentinel boundaries, checked finite variants, independent version axes, UTF-8/serialized c2 byte identity, duplicate/private-token/lossy-ingress rejection, bounded single-pass SHA-256 and actual host/cache reconciliation. Accepted 2026-11-08; runtime and independent writer/financial qualification remain open. |
| [Core numerics: bounded contracts, validation and diagnostics](core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics) | Complete bounded/lossless loading and owner validation, guarded fatal reports/pointers, exact Money/recovery/registry/table boundaries, nullable/exact host exports, bounded hints/causes/traces, ordered execution and fail-closed lock package. Accepted 2026-11-08; runtime/caller/host/state and independent financial qualification remain open. |
| [Core numerics: canonical schemas, compact views and publication](core-numerics/schema-publication.md#accepted-decision-canonical-schemas-compact-views-and-bounded-publication) | Complete explicit2020-12 original-graph/offline-corpus, exact-example/identity, deterministic owned staged publication, description-only compact replacement and bounded root-context validation/report package. Accepted 2026-11-08, completing all26 core-numerics records; runtime/host/schema/filesystem and independent financial qualification remain open. |
| [Cashflows: contractual periods, calendars and day-count context](cashflows/01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context) | Complete explicit convention/preset/CDS cleanup, original nominal grid/reference context, ICMA/365L/termination and calendar/reset/event rules, immutable period queries and accepted core015 endpoint clarification. Accepted2026-11-08; runtime/caller/host and independent financial qualification remain open. |
| [Cashflows: coupon and payment program compilation](cashflows/02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation) | Complete independent-window/strict nesting, atomic fallible configuration, explicit settlement/exchange/cutoff, distinct fixed/FROM/UNTIL rules and exact shared build/host contract. Accepted 2026-11-08; runtime and independent geometry/financial/host qualification remain open. |
| [Cashflows: dated opening principal, signed cash and atomic event phases](cashflows/03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases) | Complete dated opening/history authority, literal signed cash/deltas, exact same-date batches, paired terminal dates and explicit par exchanges with faithful replay/export/host boundaries. Accepted 2026-11-08; runtime and independent financial/host qualification remain open. |
| [Cashflows: explicit amortization laws and live-balance installments](cashflows/04-amortization.md#accepted-decision-explicit-amortization-laws-live-balance-installments-and-monetary-boundaries) | Complete explicit seven-rule/exact-fraction contract, protected hard final targets, stateless live B/k installments, actual date/payment mapping and exact par effects without implicit financial rounding. Accepted 2026-11-08; runtime/caller/host and independent qualification remain open. |
| [Cashflows: parent-coupon accrual, exact splits and capitalization](cashflows/05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization) | Complete original-parent prefix/exact whole-coupon amount and cash-share/residual-PIK allocation, nonnegative generated-parent profile, truthful zero/noncash/provenance and strict nested ingress/consumer boundaries. Accepted 2026-11-08; runtime/caller/host and independent qualification remain open. |
| [Cashflows: resolved term observations and exact rate controls](cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures) | Complete index-specific factor/qualified-fixing resolution, explicit ordinary-floater controls, one bounded Decimal-rate stage and shared get_rate/provenance without generic failure substitutes or date/tenor/preset inference. Accepted 2026-11-08; runtime/caller/host and independent qualification remain open. |
| [Cashflows: overnight blocks, cumulative growth and parent allocation](cashflows/07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation) | Complete actual quote intervals, explicit paired carry/shift/cutoff geometry, frozen-quote stable model replay, fixed parent bridge and compound-rate/reference-cap limits with truthful monetary/state domains. Accepted 2026-11-09; runtime/caller/host and independent qualification remain open. |
| [Cashflows: parent-bound floating replay and full settlement](cashflows/08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership) | Complete immutable parent/state authority, independent exact principal capture and qualified observation progress, shared full-parent amount, removal of partial/segment monetary paths and explicit forecast/information/event timing boundaries. Accepted2026-11-09; runtime/caller/host and independent complete-path qualification remain open. |
| [Cashflows: explicit fee parents, exact amounts and literal tier selection](cashflows/09-fees.md#accepted-decision-explicit-fee-parents-exact-amounts-and-literal-tier-selection) | Complete explicit limited fee grids, Opening/Integrated parent-prefix bases, one exact final Money amount, per-interval undrawn clipping, literal fixed cash and strict nullable tiers with coordinated caller/host boundaries. Accepted2026-11-09; runtime/caller/host and independent complete-path qualification remain open. |
| [Cashflows: checked mortality and prepayment assumptions](cashflows/10-prepayment-and-mortality.md#accepted-decision-checked-monthly-mortality-explicit-model-clocks-and-benchmark-boundaries) | Complete stable survival conversion, immutable whole-model admission, one-based monthly assumptions, primary PSA cap/preserved survival and exact ABS depletion with explicit lockout/vector/terminal boundaries. Accepted2026-11-09; runtime/caller/host and independent complete-path qualification remain open. |
| [Cashflows: explicit default targets, strict capacity and recovery authority](cashflows/11-default-and-recovery-assumptions.md#accepted-decision-explicit-default-targets-strict-capacity-and-one-recovery-authority) | Complete guarded conditional/target interfaces, direct original-reference increments, strict represented capacity, normalized timing, SdaAge limits and exclusive recovery primary/vintage authority with actual caller/host boundaries. Accepted2026-11-09; runtime/caller/host and independent complete-path qualification remain open. |
| [Cashflows: complete schedules, balances and explicit cash views](cashflows/12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views) | Complete immutable source schedules, optional dated principal coverage, exact closing replay and separate explicit settlement views, removing residual rewrites, ambiguous scaling, fictional per-row balances and lossy ingress. Accepted2026-11-09; runtime/caller/host and independent complete-path qualification remain open. |
| [Cashflows: cash-coupon quotation and inclusive first-ex boundaries](cashflows/13-accrued-interest.md#accepted-decision-original-parent-cash-quotation-and-inclusive-first-ex-dates) | Complete original-parent linear cash quotation, explicit currency/Money and first-ex boundary, one final restoration and qualified source/earning/PIK/product boundaries without flat-row inference or generic compounding. Accepted2026-11-09; runtime/caller/host and independent complete-path qualification remain open. |
| [Cashflows: exact nominal reporting and explicit repayment life](cashflows/14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life) | Complete exact Money/retained PeriodPlan reporting, typed native cash ladders with separately qualified owner PV, and explicit nullable ACT/365F repayment-stream weighting with product-specific clocks preserved. Accepted2026-11-09; runtime/caller/host and independent complete-path qualification remain open. |
| [Cashflows: product-owned credit valuation and qualified PV reporting](cashflows/15-credit-adjusted-pv.md#accepted-decision-product-owned-credit-valuation-and-qualified-pv-reporting) | Complete removal of generic cashflow-kind/FIFO credit valuation and schedule PV shortcuts, retaining core016 discounting/014 exact reporting, actual owner claim/source/attribution authority, unsupported decomposition and canonical-price reconciliation. Accepted2026-11-09; runtime/caller/host and independent complete-path qualification remain open. |
| [Cashflows: qualified hypothetical fixing materialization](cashflows/16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization) | Complete explicit required-key/frozen-source realization, checked raw observations and exact conflicts, persistent paired market/model-role authority, configured series and atomic merge with actual consumer/host/state reconciliation. Accepted2026-11-09; all16 cashflow records are accepted, with runtime and independent complete-path qualification open. |
| [Analytic models: checked spot BSM prices and native Greeks](models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results) | Complete checked constant-coefficient spot pricing, explicit boundary price/whole-Greek availability, native derivative units and stable error semantics, guarded state and explicit composite calculation with actual product/host/UI reconciliation. Accepted2026-11-09; runtime and independent complete-path qualification remain open. |
| [Analytic models: unit forward prices and native model Greeks](models-analytic/forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles) | Complete common unit-price/Greek scale for Black, normal and shifted models, preserved original displacement/payoff, checked singular availability and output-range limits, with product-owned multipliers and actual consumer/calibration/host/UI reconciliation. Accepted2026-11-09; runtime and independent complete-path qualification remain open. |
| [Analytic models: explicit bounded volatility inversion](models-analytic/implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance) | One checked model inverse with native premium units, strict bounds, explicit Brent controls, two final premium checks and coordinated consumer reconciliation. Accepted 2026-11-10. Runtime and independent qualification remain open. |
| [Analytic models: discrete Asian pricing](models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing) | Checked observed and projected fixing slots, prepared expectations and independent payment discount, exact geometric pricing and future-only arithmetic moment matching with dated consumer reconciliation. Accepted 2026-11-10. Runtime and independent qualification remain open. |
| [Analytic models: continuous barrier premiums and cash](models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash) | Checked direct hit/survival premiums, independent tails, explicit conditional cash timing and scaled bounded first-passage integration with dated history and outstanding cash ownership. Accepted 2026-11-10. Runtime and independent qualification remain open. |
| [Analytic models: continuous lookback premiums](models-analytic/lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums) | Checked fixed/floating payoffs with qualified extrema, explicit net carry and independent discount, positive reflection assembly and exact carry limits with dated consumer reconciliation. Accepted 2026-11-10. Runtime and independent qualification remain open. |
| [Analytic models: fixed-conversion quanto pricing](models-analytic/quanto-drift.md#accepted-decision-for-fixed-conversion-quanto-premiums) | Checked prepared foreign carry and payout discount, explicit ordered FX orientation and scaled covariance, pre-conversion premium and dated cash ownership. Accepted 2026-11-10. Runtime and independent qualification remain open. |
| [Analytic models: checked Heston transform](models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform) | Immutable checked Heston parameters, inclusive Feller diagnostic, two normalized real-frequency contours, decaying affine logarithm and exact limits. Accepted 2026-11-10. Runtime and independent qualification remain open. |
| [Analytic models: bounded Heston Fourier pricing](models-analytic/heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing) | Checked scalar expiry, deterministic and stochastic prices with normalized OTM inversion, analytic truncation enclosure, bounded mesh work and explicit time-value resolution. Accepted 2026-11-10. Runtime and independent qualification remain open. |
| [Analytics: explicit frequency](analytics/01-panels-windows-and-wire.md#accepted-decision-explicit-observation-frequency) | Caller-supplied observation frequency for annualized risk; no date-index inference. |
| [Analytics: explicit tail confidence](analytics/01-panels-windows-and-wire.md#accepted-decision-explicit-tail-confidence) | Caller-supplied finite `0.5 < c < 1` for tail metrics and affected summaries, without an implicit 95% default. |
| [Analytics: panels, windows, wire and summary](analytics/01-panels-windows-and-wire.md#accepted-decision-panels-windows-wire-and-summary) | Complete actual valuation-grid admission, explicit context/unit selection, atomic active windows, validated state/wire and fixed-summary inputs, provenance and result/failure contract. |
| [Analytics: terminal losses](analytics/02-wealth-and-annualization.md#accepted-decision-terminal-investment-losses) | Exact terminal wipeouts and the supported nonnegative-wealth return domain. |
| [Analytics: cash rates](analytics/02-wealth-and-annualization.md#accepted-decision-cash-rates) | Explicit annual effective decimal cash wherever used, geometric period conversion and explicit validation errors. |
| [Analytics: wealth, means and dated growth](analytics/02-wealth-and-annualization.md#accepted-decision-wealth-means-cagr-relative-wealth-and-cash-series) | Complete compounding/means, empty/terminal outcomes, actual dated CAGR/context, relative positive-benchmark domain, paired dates, aligned annual-effective cash and requested-result numerical contract. |
| [Analytics: calendar grouping, lookbacks and win-loss statistics](analytics/03-periods-lookbacks-and-win-loss.md#accepted-decision-calendar-grouping-lookbacks-and-win-loss-statistics) | Complete explicit bucket-frequency, partial/occupied calendar-fiscal grouping, observed streaks, dated lookback coverage, conditional means, profit-factor/CPC/loss-normalized Kelly and numerical/wire contract. |
| [Analytics: insufficient history and zero risk](analytics/04-return-risk-ratios.md#accepted-decision-insufficient-history-and-zero-risk) | Distinct unavailable sample risk, measured zero risk, undefined reward/risk `0/0` and valid signed infinities. |
| [Analytics: volatility and Sharpe](analytics/04-return-risk-ratios.md#accepted-decision-sample-volatility-and-sharpe) | Sample SD, arithmetic excess-return Sharpe and unadjusted square-root frequency scaling, with stated aggregation limits. |
| [Analytics: explicit MAR](analytics/04-return-risk-ratios.md#accepted-decision-explicit-per-observation-mar) | Caller-supplied per-observation MAR for Sortino, rolling Sortino, downside deviation and affected summaries. |
| [Analytics: downside deviation and Sortino](analytics/04-return-risk-ratios.md#accepted-decision-downside-deviation-and-sortino) | All-observation downside denominator, frequency scaling and estimator-specific sample/zero-downside outcomes. |
| [Analytics: Omega](analytics/04-return-risk-ratios.md#accepted-decision-empirical-omega-and-explicit-threshold) | Empirical unannualized gain/shortfall ratio with explicit finite per-observation threshold and defined boundary outcomes. |
| [Analytics: gain-to-pain](analytics/04-return-risk-ratios.md#accepted-decision-arithmetic-gain-to-pain) | Arithmetic net return sum divided by negative-return magnitudes, without annualization, and explicit zero-loss outcomes. |
| [Analytics: modified Sharpe](analytics/04-return-risk-ratios.md#accepted-decision-observation-period-modified-sharpe) | Observation-period excess numerator, total-return CF loss denominator, no annualization, and estimator-specific availability/error rules. |
| [Analytics: standalone scalar domains and numerical failures](analytics/04-return-risk-ratios.md#accepted-decision-standalone-scalar-domains-and-numerical-failures) | Complete standalone total-investment domain, any-finite MAR, validation order, stable requested scalar evaluation, genuine zero-risk and calculation-error contract. |
| [Analytics: drawdown paths and episodes](analytics/05-drawdown-paths-and-episodes.md#accepted-decision-drawdown-paths-and-episodes) | Complete rebased drawdown path, scalar measures, strict episode recovery, dates/durations, nullable availability, selection and metadata-removal package. |
| [Analytics: drawdown reward and CDaR](analytics/06-drawdown-reward-and-cdar.md#accepted-decision-drawdown-reward-and-cdar) | Complete seven-metric package, with explicit Sterling count, return-observation-normalized all-episode modified Burke, endpoint weighting, CDaR and availability/error rules. |
| [Analytics: signed tail outputs](analytics/07-historical-tail-risk.md#accepted-decision-signed-tail-outputs) | Signed decimal-return VaR/ES without absolute values or a zero floor; nonpositive CDaR and distinct modified-Sharpe availability. |
| [Analytics: historical VaR and ES](analytics/07-historical-tail-risk.md#accepted-decision-historical-var-and-fractional-empirical-es) | R-7 interpolated VaR paired with exact fractional empirical ES, explicitly using different finite-sample conventions. |
| [Analytics: historical availability](analytics/07-historical-tail-risk.md#accepted-decision-historical-availability-and-small-tails) | Any nonempty valid return sample, undefined for empty history, and no hard tail-count cutoff; computability does not qualify accuracy. |
| [Analytics: tail ratio](analytics/07-historical-tail-risk.md#accepted-decision-empirical-tail-ratio) | Unannualized R-7 quantile magnitude ratio for nonempty history, with exact-zero outcomes and explicit same-sign interpretation limits. |
| [Analytics: historical-tail numerical evaluation and failures](analytics/07-historical-tail-risk.md#accepted-decision-historical-tail-numerical-evaluation-and-failures) | Complete rank/mass residuals, scaled R-7/ES, empirical support, joint tail-ratio evaluation, requested-output representability and error propagation. |
| [0033: Analytics tail-method selection](analytics/0033-value-at-risk-method-selection.md#accepted-decision-fixed-historical-tail-selection) | Fixed historical scalar/summary VaR and ES, with Gaussian/CF as explicit alternatives and no automatic substitution. |
| [Analytics: reported moments](analytics/08-moments-gaussian-and-cornish-fisher.md#accepted-decision-reported-skewness-and-excess-kurtosis) | Fisher-adjusted Type 2 skewness/excess kurtosis; three/four observations and positive variance required, otherwise undefined. |
| [Analytics: CF plug-in](analytics/08-moments-gaussian-and-cornish-fisher.md#accepted-decision-classical-plug-in-cf) | Classical quantile approximation using sample mean/SD and Type 2 shape coefficients, with availability checks and no moment-matching calibration. |
| [Analytics: CF ordering](analytics/08-moments-gaussian-and-cornish-fisher.md#accepted-decision-global-cf-quantile-ordering) | Global analytic quantile-ordering check, finite requested estimate and explicit rejection without automatic repair or substitution. |
| [Analytics: tail horizons](analytics/08-moments-gaussian-and-cornish-fisher.md#accepted-decision-observation-period-horizons) | Whole observation periods, omitted horizon one, additive i.i.d. scaling and explicit invalid-horizon errors; historical estimates stay native-period. |
| [Analytics: Gaussian VaR availability](analytics/08-moments-gaussian-and-cornish-fisher.md#accepted-decision-gaussian-var-availability-and-failures) | Two-return minimum, sample variance, constant-history point-mass quantiles and explicit numerical-failure errors after request validation. |
| [Analytics: model support, numerical evaluation and exact horizons](analytics/08-moments-gaussian-and-cornish-fisher.md#accepted-decision-model-support-numerical-evaluation-and-exact-horizons) | Complete unbounded/additive model-output interpretation, scaled requested-output refinement, exact shared horizon counts, same-polynomial global gate and error propagation. |
| [Analytics: rolling return-risk calculations](analytics/09-rolling-return-risk.md#accepted-decision-rolling-return-risk-calculations) | Complete explicit count-window, output/date, estimator, terminal-loss, numerical-rebuild, availability and failure package for four rolling metrics. |
| [Analytics: single-factor benchmark regression](analytics/10-benchmark-regression.md#accepted-decision-single-factor-benchmark-regression) | Complete intercept OLS/alignment, field-wise point/uncertainty availability, Jensen alpha, fit, fixed 95% interval and numerical/error package. |
| [Analytics: benchmark-relative metrics](analytics/11-benchmark-relative-metrics.md#accepted-decision-benchmark-relative-metrics) | Complete paired-span TE/IR, strict batting/capture populations, signed geometric capture, Treynor, absolute M-squared, availability and numerical/error package. |
| [Analytics: rolling alpha and beta](analytics/12-rolling-regression.md#accepted-decision-rolling-alpha-and-beta) | Complete explicit-window paired OLS/Jensen point estimates, dated availability, terminal-history, numerical-rebuild/error and validated result-carrier contract. |
| [Analytics: multifactor regression](analytics/13-multifactor-regression.md#accepted-decision-multifactor-regression) | Complete canonical raw OLS/Total-only panel adapter, explicit return interpretation, factor alignment, field-wise availability, residual risk, invariance and numerical/result contract. |
| [Analytics: normalized SVD and constrained regression](analytics/14-svd-and-constrained-regression.md#accepted-decision-normalized-svd-and-constrained-regression) | Complete roundoff-based full-rank SVD, bounded/scaled computation, unweighted single holdings identity, weighted-gross closure, final optimality and numerical-error contract. |
| [Analytics: common-sample Pearson correlation](analytics/15-correlation-alignment-and-repair.md#accepted-decision-common-sample-pearson-correlation) | Complete common-sample Pearson, all-column availability, singular-PSD acceptance, bounded numerical normalization, repair/flag removal and compact validated provenance contract. |

The linked analytics scopes were accepted on 2026-11-06–2026-11-07, with exact
dates in their owning sections. They complete methodology adoption for all
15 analytics records. Their 2026-11-08 implementation and bounded
[validation evidence](analytics/README.md#implementation-evidence) are recorded
separately; other financial catalogues retain their separately stated statuses.

Statements waterfall prepayments were made opt-in on 2026-11-15. The default
stack covers fees, interest, scheduled principal and equity. Each prepayment
requires an explicit priority and sizing input in a capital structure composed
of term loans and revolvers. [STATEMENTS-CAPITAL-001](statements-capital/01-instrument-construction.md#accepted-decision-for-opt-in-waterfall-prepayments).

Explicit bond and swap construction was accepted on 2026-11-15. Callers
construct instruments through canonical Valuations with authored terms or a
deliberately selected qualified dated convention, then attach them through
`add_debt` or `addDebt`. Removal of the four Statements helpers remains
pending. [STATEMENTS-CAPITAL-001](statements-capital/01-instrument-construction.md#accepted-decision-for-explicit-bond-and-swap-construction).

Statements per-instrument coupon netting was accepted on 2026-11-15.
Signed coupon settlements net within each instrument and reporting period.
Expense and income are summed separately across instruments, and positive net
expense supplies the scheduled Interest claim. [STATEMENTS-CAPITAL-002](statements-capital/02-cashflow-classification.md#accepted-decision-for-per-instrument-coupon-netting).

Separate Statements distress reporting was accepted on 2026-11-15. It keeps
identified signed distress settlements visible outside scheduled debt service
and recognizes explicit company-liability changes on their economic dates.
Implementation remains pending. [STATEMENTS-CAPITAL-002](statements-capital/02-cashflow-classification.md#accepted-decision-for-separate-distress-reporting).

The full Statements cashflow classification contract was accepted on
2026-11-15. It covers period admission, native currency and source perspective,
dated cash and principal effects, and contract-based fee and hedge
classification. Implementation reconciliation and independent qualification
remain pending.
[STATEMENTS-CAPITAL-002](statements-capital/02-cashflow-classification.md#accepted-decision-for-period-admission-and-cashflow-classification).

Statements opening principal reconciliation was accepted on 2026-11-15. The
canonical debt schedule and stateful ledger must agree at the same economic
boundary and currency before calculating debt service for carried debt.
Implementation remains pending. [STATEMENTS-CAPITAL-003](statements-capital/03-funding-and-balances.md#accepted-decision-for-opening-principal-reconciliation).

Statements contractual accrued cash interest was accepted on 2026-11-15. It
covers interest earned through period close and due afterward. Overdue arrears
and issue-discount amortization are reported separately from this measure.
Implementation remains pending. [STATEMENTS-CAPITAL-003](statements-capital/03-funding-and-balances.md#accepted-decision-for-contractual-accrued-cash-interest).

The Statements dated principal contract was accepted on 2026-11-15. It covers
stable dated claims, separate reporting of economic principal and unsettled
payments, advance reserves, scheduled principal priority and exact atomic
reconciliation. Implementation and independent qualification remain pending.
[STATEMENTS-CAPITAL-004](statements-capital/04-dated-principal.md#accepted-decision-for-dated-principal-claims).

The Statements residual coupon rebuilding contract was accepted on 2026-11-15.
It uses canonical terms to rebuild affected coupon parents, principal and fees
after dated balance changes while preserving earned interest, settled cash and
dated claims. Implementation and independent qualification remain pending.
[STATEMENTS-CAPITAL-005](statements-capital/05-residual-coupons.md#accepted-decision-for-residual-coupon-rebuilding).

The Statements waterfall allocation and conservation contract was accepted on
2026-11-15. It covers pre-debt-service cash, priority and seniority, exact or
authorized-quantum payments, arrears, and terminal distributions or retained
cash. Implementation and independent qualification remain pending.
[STATEMENTS-CAPITAL-006](statements-capital/06-priority-allocation.md#accepted-decision-for-waterfall-cash-allocation-and-conservation).

The Statements prepayment and ECF sweep contract was accepted on 2026-11-15.
It covers explicit agreement terms and eligible facilities, dated sizing,
payment limits and requested, paid and unpaid reporting. Implementation and
independent qualification remain pending.
[STATEMENTS-CAPITAL-007](statements-capital/07-prepayments-and-ecf.md#accepted-decision-for-agreement-defined-prepayments-and-ecf-sweeps).

The Statements PIK election and carried-claims contract was accepted on
2026-11-15. It covers contractual coupon elections, threshold and duration
rules, dated capitalization, separate arrears and atomic replay.
Implementation and independent qualification remain pending.
[STATEMENTS-CAPITAL-008](statements-capital/08-pik-and-carry-state.md#accepted-decision-for-contractual-pik-elections-and-carried-claims).

The Statements capital reporting and FX contract was accepted on 2026-11-15.
It covers native and consolidated views, explicit period-close FX, component
totals, exact chronological exports and atomic period replacement.
Implementation and independent qualification remain pending.
[STATEMENTS-CAPITAL-009](statements-capital/09-reporting-and-fx.md#accepted-decision-for-capital-reporting-and-fx).

The Statements model timeline, explicit value and readiness contract was
accepted on 2026-11-15. It covers canonical reporting plans, typed observations,
missing cells and the common validation boundary for runnable models.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-001](statements-contracts/01-model-timeline-and-values.md#accepted-decision-for-model-timelines-values-and-readiness).

The Statements authoring, import and observation availability contract was
accepted on 2026-11-15. It covers named atomic edits, validated period-label
imports, dated publication snapshots and full-snapshot mode. Implementation
and independent qualification remain pending.
[STATEMENTS-CONTRACTS-002](statements-contracts/02-authoring-and-availability.md#accepted-decision-for-authoring-imports-and-observation-availability).

The Statements typed forecast and validation staging contract was accepted
on 2026-11-15. It covers explicit method parameters, shared static checks,
runtime history and horizon checks, and exact stochastic seeds.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-003](statements-contracts/03-forecast-parameter-contract.md#accepted-decision-for-typed-forecasts-and-validation-staging).

The Statements formula grammar and dispatch contract was accepted on
2026-11-15. It covers canonical syntax and rendering, shared identifiers,
checked signatures and bounded validation of text and caller-built trees.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-004](statements-contracts/04-dsl-syntax-and-lowering.md#accepted-decision-for-formula-grammar-function-dispatch-and-ast-validation).

The Statements formula units and currency contract was accepted on
2026-11-15. It covers resolved dimensions, monetary and scalar operations,
decimal scales, the forecast boundary and capital reporting currencies.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-005](statements-contracts/05-currency-dimensions.md#accepted-decision-for-formula-units-and-currency).

The Statements metric registry and model import contract was accepted on
2026-11-15. It covers atomic registration, pinned formula bindings,
historical dependencies, same-period cycle checks and atomic model import.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-006](statements-contracts/06-registry-and-dependencies.md#accepted-decision-for-metric-registration-and-dependencies).

The Statements bundled earnings and margin contract was accepted on
2026-11-15. It covers reported costs, the included D&A add-back, signed
expenses, revenue eligibility and recomputed ratios at coarser periods.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-007](statements-contracts/07-earnings-and-margins.md#accepted-decision-for-reported-cost-earnings-and-margins).

The Statements bundled leverage and coverage contract was accepted on
2026-11-15. It covers gross debt, reported-cost EBITDA, complete TTM windows,
positive denominators and scheduled cash service on contractual due dates.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-008](statements-contracts/08-leverage-and-coverage.md#accepted-decision-for-bundled-leverage-and-coverage).

The Statements bundled period returns contract was accepted on 2026-11-15.
It covers same-period earnings, average comparable capital, explicit
operating tax and invested capital, and coarser-period recomputation.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-009](statements-contracts/09-returns-on-capital.md#accepted-decision-for-bundled-period-returns-on-capital).

The Statements balance-sheet articulation contract was accepted on
2026-11-15. It covers disjoint account groups, checked arithmetic,
explicit tolerance and completeness at every model close.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-010](statements-contracts/10-balance-sheet-articulation.md#accepted-decision-for-balance-sheet-articulation).

The Statements cash roll-forward and component reconciliation contract was
accepted on 2026-11-15. It covers first-period opening cash, sourced adjustments,
independent component checks and complete comparisons across periods.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-011](statements-contracts/11-cash-reconciliation.md#accepted-decision-for-cash-roll-forward-and-component-reconciliation).

The Statements retained-earnings reconciliation contract was accepted on
2026-11-15. It covers dated opening balances, same-scope net income, dividend
signs, direct movements and completeness. Implementation and independent
qualification remain pending.
[STATEMENTS-CONTRACTS-012](statements-contracts/12-retained-earnings-reconciliation.md#accepted-decision-for-retained-earnings-reconciliation).

The Statements data-quality contract was accepted on 2026-11-15. It covers
required observation scope, missing and unavailable values, invalid inputs
and advisory sign diagnostics. Implementation and independent qualification
remain pending.
[STATEMENTS-CONTRACTS-013](statements-contracts/13-data-quality-checks.md#accepted-decision-for-data-quality-coverage-and-validity).

The Statements numerical comparison and check-suite contract was accepted on
2026-11-15. It covers typed tolerances, unfiltered coverage verdicts, advisory
filters, ordered suites and supplied-result provenance.
Implementation and independent qualification remain pending.
[STATEMENTS-CONTRACTS-014](statements-contracts/14-comparison-and-suite-policy.md#accepted-decision-for-numerical-comparisons-and-check-suites).

The Statements formula-check contract was accepted on 2026-11-15.
It covers explicit modes and scope, typed residual comparisons, eligibility,
lazy observation consumption and execution errors. Implementation and
independent qualification remain pending.
[STATEMENTS-CONTRACTS-015](statements-contracts/15-formula-checks.md#accepted-decision-for-formula-checks-and-coverage).

The Statements wire admission, schema and error contract was accepted on
2026-11-15. It covers bounded loading, exact wire values, validated model
identity, canonical schemas and structured errors. Implementation and
independent qualification remain pending.
[STATEMENTS-CONTRACTS-016](statements-contracts/16-wire-schema-and-errors.md#accepted-decision-for-statements-wire-admission-schemas-and-errors).

The Statements dependency graph, cell precedence and prepared evaluation
contract was accepted on 2026-11-15. It covers closed dependencies, distinct
mask states, explicit snapshots, forecast eligibility and checked plan reuse.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-001](statements-evaluation/01-graph-precedence-and-prepared-plans.md#accepted-decision-for-dependency-graphs-cell-precedence-and-prepared-evaluation).

The Statements scalar formulas and unavailable-value contract was accepted on
2026-11-15. It covers checked arithmetic, exact comparisons, lazy Boolean
controls, complete aggregates, rounding and explicit annualization.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-002](statements-evaluation/02-scalar-formulas-and-undefined-values.md#accepted-decision-for-scalar-formulas-and-unavailable-values).

The Statements historical offsets, growth and reporting-window contract was
accepted on 2026-11-15. It covers exact calendar offsets, signed changes,
Actual/365 Fixed dated growth and complete additive windows. Implementation
and independent qualification remain pending.
[STATEMENTS-EVALUATION-003](statements-evaluation/03-temporal-operators-and-period-windows.md#accepted-decision-for-historical-offsets-growth-and-reporting-windows).

The Statements historical statistics and EWM contract was accepted on
2026-11-15. It covers canonical slot windows, available samples, exact
rank ties, cumulative prefixes and slot-based EWM decay.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-004](statements-evaluation/04-historical-statistics-and-ewm.md#accepted-decision-for-historical-statistics-and-ewm).

The Statements forecast segment, anchoring and bounds contract was accepted
on 2026-11-15. It covers one forecast suffix per node and run or path,
method-specific anchors and history, and output-only bounds.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-005](statements-evaluation/05-forecast-segments-and-bounds.md#accepted-decision-for-forecast-segments-anchoring-and-output-bounds).

The Statements deterministic forecast and fade contract was accepted on
2026-11-15. It covers signed per-slot growth, keyed carry, stable fade
domains and validation on empty horizons. Implementation and independent
qualification remain pending.
[STATEMENTS-EVALUATION-006](statements-evaluation/06-deterministic-forecasts-and-fades.md#accepted-decision-for-deterministic-forecasts-and-fades).

The Statements stochastic forecast kernel contract was accepted on
2026-11-15. It covers checked per-step kernels, unclipped lognormal
restoration, explicit bootstrap and versioned PCG64 replay. Implementation
and independent qualification remain pending.
[STATEMENTS-EVALUATION-007](statements-evaluation/07-stochastic-forecast-kernels.md#accepted-decision-for-stochastic-forecast-kernels-and-replay).

The Statements latent one-peer correlation contract was accepted on
2026-11-15. It covers active Monte Carlo eligibility, exact zero and endpoint
behavior, deterministic peer ordering and latent-shock replay.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-008](statements-evaluation/08-correlated-shocks.md#accepted-decision-for-latent-one-peer-correlation).

The Statements trend extrapolation contract was accepted on 2026-11-15.
It covers aligned supplied history, centered linear trend, damped Holt
and centered moving-average forecasts. Implementation and independent
qualification remain pending.
[STATEMENTS-EVALUATION-009](statements-evaluation/09-trend-extrapolation.md#accepted-decision-for-trend-extrapolation).

The Statements seasonal decomposition and projection contract was accepted
on 2026-11-15. It covers aligned seasonal history, centered trend estimates,
additive and multiplicative phase effects and per-slot projection.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-010](statements-evaluation/10-seasonal-decomposition.md#accepted-decision-for-seasonal-decomposition-and-projection).

The Statements Monte Carlo path, quantile and breach contract was accepted on
2026-11-15. It covers bounded complete paths, typed metric samples, exact R-7
quantiles, path export and strict-upper breach probability.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-011](statements-evaluation/11-monte-carlo-evaluation.md#accepted-decision-for-monte-carlo-paths-quantiles-and-breach-probabilities).

The Statements capital-state orchestration contract was accepted on
2026-11-15. It covers dated obligations, dependency validation, one waterfall
allocation, native and consolidated reporting, and atomic period snapshots.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-012](statements-evaluation/12-capital-state-orchestration.md#accepted-decision-for-capital-state-orchestration-and-period-snapshots).

The Statements typed result and export contract was accepted on 2026-11-15.
It covers exact money, distinct cell states, long and wide tables,
dated anchors, pricing admission and replay provenance.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-013](statements-evaluation/13-result-units-and-exports.md#accepted-decision-for-typed-results-and-exports).

The Statements period rollup contract was accepted on 2026-11-15. It covers
explicit methods, complete coarser periods, typed flow sums, closing cells,
formula reevaluation and source provenance.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-014](statements-evaluation/14-period-rollup.md#accepted-decision-for-period-rollup).

The Statements normalization bridge contract was accepted on 2026-11-15.
It covers ordered signed adjustments, individual caps, checked bridge
arithmetic and separate publication of adjusted metrics.
Implementation and independent qualification remain pending.
[STATEMENTS-EVALUATION-015](statements-evaluation/15-normalization-adjustments.md#accepted-decision-for-normalization-bridges).

The Statements corporate DCF contract was accepted on 2026-11-15. It covers
valuation-date and statement-snapshot admission, eligible forecast cashflows,
an explicit terminal basis, WACC, the equity bridge and pricing provenance.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-01](statements-analytics-valuation/01-corporate-dcf.md#accepted-decision-for-statements-corporate-dcf).

The Statements DCF sensitivity contract was accepted on 2026-11-15.
It covers a verified baseline, explicit independent shocks, endpoint
availability and ranking of complete enterprise-value ranges.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-02](statements-analytics-valuation/02-dcf-sensitivity.md#accepted-decision-for-statements-dcf-sensitivity).

The Statements LBO contract was accepted on 2026-11-15. It covers dated
transaction and earnings inputs, checked sponsor funding, the exit equity
bridge, funded and implied MOIC, and required-check coverage.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-03](statements-analytics-valuation/03-lbo.md#accepted-decision-for-statements-lbo-funding-and-returns).

The Statements corporate-analysis and typed reporting contract was accepted
on 2026-11-15. It covers explicit analysis selection, check coverage, matched
LTV and debt-to-EV, complete TTM ratios and consistent typed report values.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-04](statements-analytics-valuation/04-analysis-reports-units.md#accepted-decision-for-corporate-analysis-and-typed-reporting).

The Statements comparable-company population, multiples and statistics
contract was accepted on 2026-11-15. It covers peer admission, matched dates
and bases, eleven canonical metrics, coverage and matched-peer regression.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-05](statements-analytics-valuation/05-comps-population-statistics.md#accepted-decision-for-comparable-company-populations-and-statistics).

The Statements comparable-company scoring contract was accepted on 2026-11-15.
It covers explicit dimensions and directions, sample residual scores,
composite weight coverage and fit diagnostics.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-06](statements-analytics-valuation/06-comps-scoring.md#accepted-decision-for-comparable-company-scoring).

The Statements goal-seek contract was accepted on 2026-11-15. It covers a
selected forecast assumption, explicit bounded solving, typed forward
verification and replay evidence.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-07](statements-analytics-valuation/07-goal-seek.md#accepted-decision-for-statements-goal-seek).

The Statements forecast-error contract was accepted on 2026-11-15. It
covers as-issued forecast pairing, checked equal-weight errors, exact-zero
denominator rules and per-metric coverage.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-08](statements-analytics-valuation/08-forecast-errors.md#accepted-decision-for-forecast-error-pairing-and-metrics).

The Statements scenario inheritance and comparison contract was accepted
on 2026-11-15. It covers validated overrides, root-to-child cell precedence,
fresh evaluation and explicit typed baselines.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-09](statements-analytics-valuation/09-scenario-inheritance.md#accepted-decision-for-scenario-inheritance-and-comparisons).

The Statements sensitivity and tornado contract was accepted on 2026-11-15.
It covers verified baselines, distinct typed levels, bounded evaluation
and tornado ranking by complete tested responses.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-10](statements-analytics-valuation/10-sensitivity-grids.md#accepted-decision-for-statement-sensitivity-grids-and-tornado-reports).

The Statements variance and additive bridge contract was accepted on
2026-11-15. It covers matched typed inputs, signed changes, ordered
nonoverlapping components and coverage-aware residuals.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-11](statements-analytics-valuation/11-variance-bridges.md#accepted-decision-for-statement-variances-and-additive-bridges).

The Statements dependency traversal and formula explanation contract was
accepted on 2026-11-15. It covers validated graph relationships, original run
provenance, time-aware operands and explicit trace completeness.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-12](statements-analytics-valuation/12-dependency-explanations.md#accepted-decision-for-dependency-traversal-and-formula-explanations).

The Statements account roll-forward and articulation contract was accepted on
2026-11-15. It covers typed movements, checked opening continuity and closing
balances, explicitly selected articulation, and coverage-based verdicts.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-13](statements-analytics-valuation/13-corkscrew.md#accepted-decision-for-account-roll-forwards-and-articulation).

The Statements credit-scorecard contract was accepted on 2026-11-15. It
covers verified inputs and explicit scales, typed scoreable bands, weight
coverage and complete minimum-rating gates.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-14](statements-analytics-valuation/14-credit-scorecards.md#accepted-decision-for-credit-scorecard-bands-and-coverage).

The Statements roll-forward and vintage template contract was accepted on
2026-11-15. It covers validation before node insertion, typed openings and
movements, checked convolution and explicit cohort coverage.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-15](statements-analytics-valuation/15-recurrence-templates.md#accepted-decision-for-roll-forward-and-vintage-templates).

The Statements lease projection and unlevered property statement contract was
accepted on 2026-11-15. It covers calendar-slot lease timing, renewal and
concession attribution, typed property cashflows and validated node insertion.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-VALUATION-16](statements-analytics-valuation/16-real-estate-templates.md#accepted-decision-for-lease-projections-and-property-statements).

The Statements mapped check-suite contract was accepted on 2026-11-15. It
covers full and scoped three-statement reviews, explicit credit checks and
screening thresholds, preserved LBO thresholds and coverage-based gates.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-01](statements-analytics-credit/01-suite-selection.md#accepted-decision-for-mapped-check-suites-and-coverage).

The Statements stock-flow reconciliation contract was accepted on 2026-11-15.
It covers cash CapEx, net PPE, paid dividends and operating working capital,
with explicit adjustments and typed comparisons.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-02](statements-analytics-credit/02-stock-flow-reconciliation.md#accepted-decision-for-stock-flow-reconciliations).

The Statements interest-reconciliation contract was accepted on 2026-11-15.
It covers explicit direct recognition or advisory balance-based estimates,
required coverage and typed comparisons.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-03](statements-analytics-credit/03-interest-reconciliation.md#accepted-decision-for-interest-reconciliation-and-estimates).

The Statements period-change screening contract was accepted on 2026-11-15.
It covers signed change relative to prior magnitude, verified adjacency,
explicit forecast transitions and inclusive period bounds.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-04](statements-analytics-credit/04-growth-plausibility.md#accepted-decision-for-signed-period-change-relative-to-prior-magnitude).

The Statements effective-tax-rate screening contract was accepted on
2026-11-15. It covers signed recognized tax expense, a positive pretax
denominator, sourced informational bounds and explicit inapplicability.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-05](statements-analytics-credit/05-tax-plausibility.md#accepted-decision-for-informational-effective-tax-rates).

The Statements leverage and coverage screening contract was accepted on
2026-11-15. It covers bundled TTM metrics, named custom checks, explicit
nested thresholds and per-period coverage.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-06](statements-analytics-credit/06-leverage-and-coverage.md#accepted-decision-for-bundled-ttm-leverage-and-coverage-checks).

The Statements liquidity runway contract was accepted on 2026-11-15.
It covers spendable cash, signed cash use, supported month conversions,
immediate shortfalls and explicit threshold outcomes.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-07](statements-analytics-credit/07-liquidity-runway.md#accepted-decision-for-liquidity-runway-in-reporting-months).

The Statements sequential credit signals contract was accepted on 2026-11-15.
It covers negative FCF streaks, strict adverse transitions, gap handling,
forecast boundary policy and explicit thresholds.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-08](statements-analytics-credit/08-sequential-credit-signals.md#accepted-decision-for-negative-fcf-streaks-and-adverse-trends).

The Statements capital structure summary contract was accepted on 2026-11-15.
It covers period-end economic principal, native and consolidated amounts,
through-rank ratios, final maturity grouping and exact currency-tagged reporting.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-09](statements-analytics-credit/09-capital-structure-summary.md#accepted-decision-for-capital-structure-summaries-and-maturity-walls).

The Statements instrument credit context contract was accepted on 2026-11-15.
It covers verified instrument ratios, selected debt-service bases, dated
collateral and valuation references, and observed and full-path minima.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-10](statements-analytics-credit/10-credit-context.md#accepted-decision-for-instrument-credit-ratios-and-path-coverage).

The Statements covenant adapter contract was accepted on 2026-11-15. It
covers verified observations, exact fiscal or calendar test dates, elected
forecast periods, distinct applicability results and read-only delegation.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-11](statements-analytics-credit/11-covenant-statement-adapter.md#accepted-decision-for-verified-statement-covenant-assessments).

The Statements exposure and cumulative default probability contract was
accepted on 2026-11-15. It covers an explicit EAD basis, sourced LGD and
discount assumptions, anchored schedules and cumulative PD, and interval
and conditional probabilities.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-12](statements-analytics-credit/12-ecl-exposure-and-pd.md#accepted-decision-for-exposure-and-cumulative-default-inputs).

The Statements impairment staging and cure contract was accepted on
2026-11-15. It covers matched conditional default comparisons, explicit
triggers, default priority and separate dated probation for stage transitions.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-13](statements-analytics-credit/13-ecl-staging.md#accepted-decision-for-impairment-staging-and-cure).

The Statements impairment policy selection contract was accepted on
2026-11-15. It covers exact immutable revisions, reporting-date and cutoff
eligibility, method-level validation, named overrides and replay provenance.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-14](statements-analytics-credit/14-ecl-policy-defaults.md#accepted-decision-for-resolved-impairment-policy-snapshots).

The Statements stage-specific impairment measurement contract was accepted
on 2026-11-15. It covers performing horizons and knot-aligned buckets,
explicit LGD, dated Stage 3 recovery and currency and policy provenance.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-15](statements-analytics-credit/15-ecl-bucket-measurement.md#accepted-decision-for-stage-specific-impairment-measurement).

The Statements impairment scenario weighting contract was accepted on
2026-11-15. It covers shared verified inputs, exact weights, scenario LGD
or recovery evidence, and complete aggregate coverage with per-scenario
provenance. Implementation reconciliation and independent qualification
remain pending.
[STATEMENTS-ANALYTICS-CREDIT-16](statements-analytics-credit/16-ecl-scenario-weighting.md#accepted-decision-for-weighted-impairment-scenarios).

The Statements lifetime CECL forecast and reversion contract was accepted
on 2026-11-15. It covers explicit historical hazard and forecast inputs,
fixed-lookback reversion, survival across boundaries and direct impaired
recovery evidence. Implementation reconciliation and independent
qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-17](statements-analytics-credit/17-cecl-forecast-reversion.md#accepted-decision-for-lifetime-cecl-forecast-and-reversion).

The Statements WARM exposure-years contract was accepted on 2026-11-15. It
covers verified performing-pool eligibility, a sourced annual net loss rate,
knot-aligned exposure-years and undiscounted measurement.
Implementation reconciliation and independent qualification remain pending.
[STATEMENTS-ANALYTICS-CREDIT-18](statements-analytics-credit/18-cecl-warm.md#accepted-decision-for-warm-exposure-years-and-annual-loss-rates).

The [invariant-to-decision map](../INVARIANT_MAP.md) records the detailed
cross-crate conventions covered by these decisions. Acceptance does not adopt
drafts 0027–0032 or every observed instrument default; accepted analytics
selection in 0033 retains its own scope.

## Methodology records

[Core-market record01](core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction),
[record02](core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates),
[record03](core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning),
[record04](core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation),
[record05](core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery),
[record06](core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification),
[record07](core-market/parametric-discount.md#accepted-decision-validated-nsnss-sources-and-analytic-factor-queries),
[record08](core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks),
[record09](core-market/basis-spreads.md#accepted-decision-source-derived-dated-zero-rate-differences),
[record10](core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks),
[record11](core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability),
[record12](core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings),
[record13](core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates),
[record14](core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks),
[record15](core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs),
[record16](core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning),
[record17](core-market/forward-variance.md#accepted-decision-nonnegative-variance-sources-and-explicit-model-time-semantics),
[record18](core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks),
[record19](core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts),
[record20](core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle),
[record21](core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap),
and [record22](core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows)
were accepted on 2026-11-08 within their complete stated packages. **All 22
detailed core-market records are accepted**, completing this area's methodology
review. Runtime/caller/host reconciliation and independent qualification remain open. This does not accept ADR0032's remaining
calibration selections or broader model/context aging methodologies.

All 20 complete packages in [core-conventions records 001–020](core-conventions/README.md)
were accepted on 2026-11-07 within their stated scopes. This completes methodology
adoption for core financial types and dates. Runtime/data
reconciliation and independent qualification remain open; the accepted-scope
table above and individual records provide details.

[Core-numerics record 01](core-numerics/compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads)
was accepted on 2026-11-07 as a complete compounding/conversion/periodic-spread
package. [Record 02](core-numerics/value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics)
was also accepted on 2026-11-07 as a complete value-interpolation package.
[Record 03](core-numerics/forward-interpolation.md#accepted-decision-natural-log-discount-and-hagan-west-interpolation)
was accepted on 2026-11-07 as a complete forward-interpolation package.
[Record 04](core-numerics/quadrature.md#accepted-decision-checked-quadrature-budgets-and-measures)
was accepted on 2026-11-07 as a complete quadrature package.
[Record 05](core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)
was accepted on 2026-11-07 as a complete Brent/root-search package.
[Record 06](core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes)
was accepted on 2026-11-07 as a complete damped least-squares/termination package.
[Record 07](core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission)
was accepted on 2026-11-08 as a complete matrix-factor/solve/eigen/correlation-admission
package. [Record 08](core-numerics/nearest-correlation.md#accepted-decision-explicit-nearest-correlation-projection-and-repair-limits)
was accepted on 2026-11-08 as a complete explicit nearest-correlation repair package.
[Record 09](core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty)
was accepted on 2026-11-08 as a complete aggregation/moments/R7/mean-uncertainty
package. [Record 10](core-numerics/realized-variance.md#accepted-decision-explicit-log-returns-and-realized-variance)
was accepted on 2026-11-08 as a complete positive-price/log-return/realized-variance
package. [Record 11](core-numerics/covariance-shrinkage.md#accepted-decision-checked-scaled-identity-covariance-shrinkage)
has complete scoped acceptance as of 2026-11-08 for centered population/scaled-identity
shrinkage, stable moment/complement, checked result/error and host/credit-owner
reconciliation, with independent numerical/statistical/financial qualification pending; [Record 12](core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains)
has complete scoped acceptance as of 2026-11-08 for checked scalar/distribution
domains, normal/log-tail, bounded Student-t beta and host/caller reconciliation,
with independent numerical/statistical/financial qualification pending;
[Record 13](core-numerics/binomial-mass.md#accepted-decision-checked-full-binomial-probability-mass)
has complete scoped acceptance as of 2026-11-08 for strict binomial domains/
resources, exact-mode/FMA recurrence, normalization, scratch/errors and Rust-only/
credit-owner boundaries, with runtime and independent qualification pending; [Record 14](core-numerics/beta-gamma-sampling.md#accepted-decision-checked-beta-and-gamma-sampling)
has complete scoped acceptance as of 2026-11-08 for checked RNG/shapes, stable
bounded MT/component beta ratios, endpoint/errors and Rust-only/model ownership,
with runtime and independent qualification pending;
[Record 15](core-numerics/chi-squared-inversion.md#accepted-decision-checked-chi-squared-log-tail-inversion)
has complete scoped acceptance as of2026-11-08 for closed endpoints/interior
domain, bounded log-Gamma/Brent, actual-returned-output tail gate and Rust-only/
copula ownership, with runtime and independent qualification pending; [Record 16](core-numerics/correlated-bernoulli.md#accepted-decision-strict-correlated-bernoulli-laws)
has complete scoped acceptance as of2026-11-08 for strict two-event feasibility/
nullable correlation, stable law/conditional/half-open sampling, canonical hosts
and financial ownership, with runtime and independent qualification pending; [Record 17](core-numerics/pseudorandom-and-bridge.md#accepted-decision-checked-random-draws-bridge-and-poisson-counts)
has complete scoped acceptance as of2026-11-08 for explicit PCG/checked normals,
capped stable bridge, bounded Poisson counts and caller/financial ownership,
with runtime and independent qualification pending;
[Record 18](core-numerics/sobol-sequences.md#accepted-decision-finite-period-sobol-points-and-prefix-hash-scrambling)
has complete scoped acceptance as of 2026-11-08 for fixed directions/order,
explicit seed mode, honest prefix hash guarantees, checked whole-point state/
period/buffers, caller reconciliation and estimator ownership, with runtime and
independent qualification pending.
[Record 19](core-numerics/fractional-covariance.md#accepted-decision-checked-fractional-increment-covariance)
has complete scoped acceptance as of 2026-11-08 for typed/unit-normalized fBM,
validated intervals/grids, stable bounded increment arithmetic/cancellation refusal,
checked mirrored matrices and caller/schema/financial ownership, with runtime and
independent qualification pending.
[Record 20](core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots)
has complete scoped acceptance as of 2026-11-08 for canonical knots/actual dts,
strict exact required events, checked lookup/access, bounded geometry/resources,
honest uniform recognition and removal of generic financial snapping with explicit
caller/model/host ownership, with runtime and independent qualification pending.
[Record 21](core-numerics/piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration)
has complete scoped acceptance as of 2026-11-08 for right-continuous checked
coefficients, stable signed-kappa analytic integration without deadbands, scaled
positive-total range/zero rules and complete model/tree/host error propagation
with explicit financial ownership, with runtime and independent qualification
pending.
[Record 22](core-numerics/expression-evaluation.md#accepted-decision-checked-scalar-expressions-missing-values-and-decimal-quantization)
has complete scoped acceptance as of 2026-11-08 for bounded eager scalar syntax,
explicit exact-shape rows, NaN-only missing/invalid-calculation errors, three-valued
logic, exact binary-input decimal quantization and checked shared helpers with
explicit caller/statements/host/financial boundaries, with runtime and independent
qualification pending.
[Record 23](core-numerics/rounding-and-config.md#accepted-decision-explicit-rounding-config-local-tolerances-and-producer-metadata)
has complete scoped acceptance as of 2026-11-08 for explicit monetary config/
snapshots, local tolerance ownership, fixed embedded validation, optional actual
producer metadata and removal of obsolete global-zero/override/overlay paths,
with coordinated caller/host/replay qualification pending. [Record 24](core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity)
has complete scoped acceptance as of 2026-11-08 for strict symmetric count,
exact monetary/date/sentinel wire boundaries and c2 serialized identity,
including duplicate/lossy-ingress rejection, explicit UTF-8 ordering/byte profile,
bounded single-pass hashing and existing host/cache reconciliation. Runtime and
independent qualification remain pending. [Record 25](core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics)
has complete scoped acceptance as of 2026-11-08 for bounded lossless loading,
exact monetary/owner validation, independent fatal-report status, nullable/exact
table values, bounded trace/error diagnostics, ordered execution and fail-closed
locks. Runtime/caller/host/schema reconciliation and independent qualification
remain pending. [Record 26](core-numerics/schema-publication.md#accepted-decision-canonical-schemas-compact-views-and-bounded-publication)
has complete scoped acceptance as of 2026-11-08 for original-graph/offline-corpus
schemas, exact examples, bounded canonical validation, compact authoring views
and staged publication. **All 26 core-numerics records are accepted**, completing
that area's methodology review; none remain Draft or Proposed. Runtime/caller/
host/schema reconciliation and independent qualification remain pending.
[Cashflows record001](cashflows/01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context)
has complete scoped acceptance as of2026-11-08 for explicit contractual schedule
choices, original nominal references, ICMA/365L/termination context, coincident
events and checked immutable period queries. Its narrow core015 clarification
was applied in place: metadata endpoints describe actual calculation dates,
including explicit adjustment, separately from nominal references.
[Cashflows record002](cashflows/02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation)
has complete scoped acceptance as of 2026-11-08 for independent atomic windows,
explicit settlement/exchange/cutoff, globally valid overrides, atomic fallible
configuration and exact shared build paths.
[Cashflows record003](cashflows/03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases)
has complete scoped acceptance as of 2026-11-08 for explicit dated opening
stock, signed settlement cash, atomic same-date principal batches, covered
history, paired terminal dates and exact explicit exchanges.
[Cashflows record004](cashflows/04-amortization.md#accepted-decision-explicit-amortization-laws-live-balance-installments-and-monetary-boundaries)
has complete scoped acceptance as of 2026-11-08 for explicit amortization laws,
exact fractions, protected final targets, stateless live-balance installments,
par effects and monetary/payment boundaries. Its removal of the obsolete owner
installment-update slot was clarified in accepted003 without changing the relative
economic phase order.
[Cashflows record005](cashflows/05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization)
has complete scoped acceptance as of 2026-11-08 for original-parent prefix
weights, once-rounded parent amounts, one exact split share and conserved
cash/PIK allocation, truthful component metadata and capitalization. Accepted003
now explicitly points to005's generated-parent PIK unit/profile, retaining signed
internal contributions and separate explicit events.
[Cashflows record006](cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures)
has complete scoped acceptance as of 2026-11-08 for resolved term observations,
publication-qualified exact fixing/projection, explicit exact rate controls and
one shared strict rate path without generic failure substitutes. It inherits
accepted market02/12 and cashflows005;005 now explicitly names006's admitted
Decimal-rate handoff before its separate parent-Money stage.
[Cashflows record007](cashflows/07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation)
has complete scoped acceptance as of 2026-11-09 for actual overnight quote
intervals, paired carry/shift blocks, stable frozen-quote growth replay and an
explicit fixed-bridge compound-rate amount allocation with reference-cap limits.
005 now names that overnight contribution/admission authority explicitly.
[Cashflows record008](cashflows/08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership)
has complete scoped acceptance as of2026-11-09 for immutable parent-bound state,
independent qualified observation/exact principal capture, shared full-parent
settlement, removal of generic partial/segment monetary paths and explicit
information/event timing ownership.
[Cashflows record009](cashflows/09-fees.md#accepted-decision-explicit-fee-parents-exact-amounts-and-literal-tier-selection)
has complete scoped acceptance as of2026-11-09 for explicit limited fee grids,
opening/integrated original-parent bases, exact whole-parent amounts, literal
fixed cash and strict nullable tier selection.
[Cashflows record010](cashflows/10-prepayment-and-mortality.md#accepted-decision-checked-monthly-mortality-explicit-model-clocks-and-benchmark-boundaries)
has complete scoped acceptance as of2026-11-09 for checked survival conversion,
whole-model admission, one-based monthly assumptions and explicit PSA/ABS/
lockout/vector boundaries, with actual consumer clocks and host reconciliation.
[Cashflows record011](cashflows/11-default-and-recovery-assumptions.md#accepted-decision-explicit-default-targets-strict-capacity-and-one-recovery-authority)
has complete scoped acceptance as of2026-11-09 for explicit conditional/target
interfaces, direct original-reference demand, strict represented capacity,
normalized timing and one recovery authority with truthful source/vintage limits.
[Cashflows record012](cashflows/12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views)
has complete scoped acceptance as of2026-11-09 for complete immutable source
schedules, explicit opening/through principal scope, exact closing replay and
separate native cash views without implicit residual normalization. Restricted
composition and actual product/export/host reconstruction retain their owners.
[Cashflows record013](cashflows/13-accrued-interest.md#accepted-decision-original-parent-cash-quotation-and-inclusive-first-ex-dates)
has complete scoped acceptance as of2026-11-09 for original-parent linear cash
quotation, explicit native Money/currency, inclusive first-ex boundaries and one
final restoration, removing generic compounding/PIK/configuration and inference.
[Cashflows record014](cashflows/14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life)
has complete scoped acceptance as of2026-11-09 for exact native Money/PeriodPlan
reporting, typed cash-year ladders with separately owned qualified PV, and an
explicit ACT/365F repayment-stream statistic with owner-selected weights/dates.
Its correction of universal SIFMA WAL attribution does not adopt product WAL.
[Cashflows record015](cashflows/15-credit-adjusted-pv.md#accepted-decision-product-owned-credit-valuation-and-qualified-pv-reporting)
has complete scoped acceptance as of2026-11-09 removing generic credit-kind/FIFO
recovery valuation and schedule PV shortcuts. Actual product owners supply
qualified value attribution; existing core016 discounting/014 aggregation,
explicit unsupported outcomes and canonical-price reconciliation remain.
[Cashflows record016](cashflows/16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization)
has complete scoped acceptance as of2026-11-09 for explicit required observations,
qualified frozen-forward candidates, exact conflicts and persistent hypothetical
source/role state, replacing date-window and unqualified history insertion.
All16 cashflow records are Accepted; the whole-file cashflow review is complete.
[Analytic-model record01](models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results)
has complete scoped acceptance as of2026-11-09 for checked spot BSM pricing,
raw derivative units, explicit singular Greek availability and composite
calculation behavior. [Analytic-model record02](models-analytic/forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles)
has complete scoped acceptance as of2026-11-09 for common unit-forward pricing and
native Greek bundles, checked exact boundaries and original displacement identity,
with external product multipliers and explicit unit-output range limits.
[Analytic-model record03](models-analytic/implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance)
was accepted on 2026-11-10 with one explicit model inverse, five required controls,
strict bounds/identifiable time, bounded core Brent with stable OTM objectives and
paired original-premium/time-value acceptance. Its complete24-point package
retains caller normalization, real product objectives and truthful failures.
[Analytic-model record 04](models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing)
was accepted on 2026-11-10 for one discrete Asian pricing API with prepared
fixing levels and independent payment discount. The decision covers the exact
geometric law, arithmetic moment matching on the future contribution, and
seasoned equity and commodity reconciliation.
[Analytic-model record 05](models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash)
was accepted on 2026-11-10 for checked continuous barrier premiums,
independent first-passage tails and explicit hit/no-hit cash timing with
bounded numerical work.
[Analytic-model record 06](models-analytic/lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums)
was accepted on 2026-11-10 for checked inclusive continuous lookbacks with
qualified extrema, independent carry and discount, positive payoff components
and exact carry limits.
[Analytic-model record 07](models-analytic/quanto-drift.md#accepted-decision-for-fixed-conversion-quanto-premiums)
was accepted on 2026-11-10 for one checked normalized quanto price,
explicit payout-per-asset FX orientation and signed covariance, with
contractual conversion and quantity at the dated owner.
[Analytic-model record 08](models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform)
was accepted on 2026-11-10 for checked parameters, normalized cash and
share contours, the decaying affine logarithm and exact boundary laws.
[Analytic-model record 09](models-analytic/heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing)
was accepted on 2026-11-10 for one checked Heston price, exact singular
payoffs, stable OTM inversion, conservative tail enclosure and bounded mesh work.
[Analytic-model record 10](models-analytic/cos-inversion.md#accepted-decision-for-checked-cos-pricing)
was accepted on 2026-11-10 for direct bounded COS payoffs with explicit premium tolerance,
separate window and series bounds, and capped work.
[Analytic-model record 11](models-analytic/jump-characteristic-laws.md#accepted-decision-for-checked-characteristic-laws)
was accepted on 2026-11-10 for checked forward-normalized Black, Merton and variance-gamma
laws with selected contours, exact moment domains and support.
[Analytic-model record 12](models-analytic/rough-heston-riccati.md#accepted-decision-for-checked-rough-heston-pricing)
was accepted on 2026-11-10 for checked rough Heston Lewis pricing with explicit tolerances,
an analytic tail enclosure, separate time and mesh checks, and bounded work.
[Analytic-model record 13](models-analytic/sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes)
was accepted on 2026-11-10 for explicit SABR quote conventions, canonical parameter admission,
exact CEV distance, stable skew and Hagan midpoint time correction.
[Analytic-model record 14](models-analytic/sabr-calibration.md#accepted-decision-for-explicit-checked-sabr-calibration)
was accepted on 2026-11-10 for explicit SABR fitting, native quote-error acceptance,
literal shifts and an observed-ATM pin with a checked alpha bracket.
[Analytic-model record 15](models-analytic/sabr-smile-repair.md#accepted-decision-for-checked-sabr-smile-and-finite-grid-repair)
was accepted on 2026-11-10 for the checked ATM delta proxy, unit call grid diagnostics
and exact down-only repair with move and repricing gates.
[Analytic-model record 16](models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization)
was accepted on 2026-11-10 for typed source evaluation, exact support and explicit boundaries,
native sigma and total-variance interpolation, qualified SABR cube modes,
strict materialization and native-unit reference moves.
[Analytic-model record 17](models-analytic/fx-delta-smiles.md#accepted-decision-for-checked-fx-delta-smiles-and-strike-materialization)
was accepted on 2026-11-10 for checked FX delta strikes and labelled pillars, strict-expiry quote
evaluation and explicit strike wings, plus exact-union materialization
with qualified row forwards.
[Analytic-model record 18](models-analytic/svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance)
was accepted on 2026-11-10 for checked raw SVI, exact shape and wing boundaries, a finite density
scan, one seeded checked fit and separate annual-volatility and sampled-density
acceptance.
[Analytic-model record 19](models-analytic/dupire-local-volatility.md#accepted-decision-for-checked-dupire-extraction-and-local-volatility-grids)
was accepted on 2026-11-10 for checked fixed-moneyness Dupire stencils, explicit extraction
support and smoothing, and checked local sigma grids and simulation
consumption.
[Analytic-model record 20](models-analytic/surface-arbitrage-diagnostics.md#accepted-decision-for-checked-surface-arbitrage-diagnostics-and-coverage)
was accepted on 2026-11-10 for checked sampled call-price, common-support calendar and Dupire
sign checks, with explicit coverage and unfiltered status.
[Analytic-model record 21](models-analytic/variance-replication.md#accepted-decision-for-checked-variance-replication-and-explicit-black-wings)
was accepted on 2026-11-10 for checked forward-split variance replication, an explicit mesh
gate and selected finite-strip or full flat-Black wings.
[Analytic-model record 22](models-analytic/atm-volatility-conversion.md#accepted-decision-for-checked-price-equivalent-atm-volatility-conversion)
was accepted on 2026-11-10 for literal at-the-forward conversion using checked analytic
pricing, direct inversion and premium replay in the smaller boundary
component.
[Analytic-model record 23](models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures)
was accepted on 2026-11-10 for checked scheduled Hull–White moments
and affine bonds, explicit payment-measure caplets and coupons,
exact simple futures bias and a price-equivalent term ATM Normal quote.
[Analytic-model record 24](models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection)
was accepted on 2026-11-10 for checked dated clock mapping, an exact
deterministic curve shift, contractual term-period reconstruction and
active fitted-state rules.
[Analytic-model record 25](models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling)
was accepted on 2026-11-10 for exact joint Hull–White state and log-bank
steps, explicit event and stream controls, and pair-based sampling
uncertainty.
[Analytic-model record 26](models-analytic/hw1f-issuer-lsmc.md#accepted-decision-for-checked-issuer-call-policy-training-and-pricing)
was accepted on 2026-11-10 for dated issuer-call cash, checked
continuation regression and independent pricing of a frozen policy.
[Analytic-model record 27](models-analytic/diebold-li-var.md#accepted-decision-for-checked-nelsonsiegel-factors-and-var-forecasts)
was accepted on 2026-11-10 for explicit observation cadence, checked
Nelson–Siegel factor extraction, stable VAR dynamics and marginal
fitted-yield forecasts.
[Analytic-model record 28](models-analytic/yield-pca.md#accepted-decision-for-checked-yield-change-pca-and-scenarios)
was accepted on 2026-11-11 for checked yield-change PCA, full-spectrum
variance accounting and fitted-basis reconstruction and scenarios.
All 28 analytic-model records are Accepted.
[Numerical-model record 001](models-numerical/equity-binomial-construction.md#accepted-decision-for-checked-crr-and-leisenreimer-lattices)
was accepted on 2026-11-11 for checked CRR and Leisen–Reimer
transitions, explicit lattice counts and exact vanilla payoff limits.
[Numerical-model record 002](models-numerical/dividends-and-lattice-exercise.md#accepted-decision-for-checked-escrowed-dividends-and-lattice-exercise)
was accepted on 2026-11-11 for dated ex-claim values, a common escrow
reserve horizon, exact alignment and product-qualified exercise phases.
[Numerical-model record 003](models-numerical/tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)
was accepted on 2026-11-11 for checked interval discounting, explicit
nodewise OAS, validated rollback and genuine terminal state.
[Numerical-model record 004](models-numerical/ho-lee-curve-fit.md#accepted-decision-for-checked-holee-curve-fitting)
was accepted on 2026-11-11 for origin-relative Ho–Lee targets,
checked additive row fits and one repricing gate.
[Numerical-model record 005](models-numerical/bdt-lognormal-curve-fit.md#accepted-decision-for-checked-bdt-curve-fitting)
was accepted on 2026-11-11 for strictly decreasing BDT targets,
unclamped log-rate fits and one inclusive replay gate.
[Numerical-model record 006](models-numerical/black-karasinski-trinomial.md#accepted-decision-for-checked-bk-trinomial-fitting)
was accepted on 2026-11-11 for exact conditional OU moments on a
capped trinomial, positive rate fitting and one inclusive replay gate.
[Numerical-model record 007](models-numerical/hull-white-event-grid-and-affine-bonds.md#accepted-decision-for-checked-hullwhite-event-lattices-and-node-bonds)
was accepted on 2026-11-11 for exact Hull–White event grids,
conditional OU moments, Continuous base fitting and explicit
affine exercise valuation.
[Numerical-model record 008](models-numerical/rates-credit-marginal-calibration.md#accepted-decision-for-checked-rates-credit-marginal-calibration)
was accepted on 2026-11-11 for centered binary marginals,
strict variance retention, local integrated-hazard fitting
and checked replay.
[Numerical-model record 009](models-numerical/rates-credit-coupling-and-sampling.md#accepted-decision-for-checked-rates-credit-coupling-and-path-sampling)
was accepted on 2026-11-11 for strict Bernoulli coupling, indexed
Philox midpoints and bound segment replay.
[Numerical-model record 010](models-numerical/rates-credit-coupon-folding.md#accepted-decision-for-checked-joint-credit-rollback-and-coupon-folding)
was accepted on 2026-11-11 for checked joint survival and recovery,
projection on exact index dates and a full coupon fold with explicit
exercise eligibility.
[Numerical-model record 011](models-numerical/pde-grids-interpolation-and-greeks.md#accepted-decision-for-checked-pde-grids-interpolation-and-coordinate-greeks)
was accepted on 2026-11-11 for validated spatial grids, closed-domain
value extraction and interpolated quadratic nodal derivative estimates.
[Numerical-model record 012](models-numerical/pde-spatial-stencils.md#accepted-decision-for-checked-directional-and-mixed-pde-stencils)
was accepted on 2026-11-11 for checked diffusion admission, an inclusive
central-to-one-sided drift switch and an explicit tensor mixed stencil.
[Numerical-model record 013](models-numerical/pde-boundary-elimination.md#accepted-decision-for-checked-pde-boundary-elimination-and-reconstruction)
was accepted on 2026-11-11 for shared checked edge relations, explicit
x-edge corner ownership and stage-consistent reconstruction and cache reuse.
[Numerical-model record 014](models-numerical/theta-time-march-and-thomas.md#accepted-decision-for-checked-theta-marching-and-tridiagonal-solves)
was accepted on 2026-11-11 for exact event-grid theta steps, full-interval
damping and one checked Thomas kernel with a componentwise residual gate.
[Numerical-model record 015](models-numerical/pde-exercise-projection.md#accepted-decision-for-checked-grid-based-pde-exercise)
was accepted on 2026-11-11 for exact post-step payoff comparison,
exact exercise knots and actual-spot immediate exercise at eligible
equity valuations.
[Numerical-model record 016](models-numerical/mcs-adi-time-march.md#accepted-decision-for-checked-modified-craigsneyd-adi)
was accepted on 2026-11-11 for fixed full MCS, checked factorized startup
and atomic stages with application-specific stability and price qualification.
[Numerical-model record 017](models-numerical/black-scholes-and-heston-pde-bridges.md#accepted-decision-for-checked-pde-coefficient-bridges)
was accepted on 2026-11-11 for checked bridge parameters and diffusion,
explicit edge laws, exact deterministic limits and qualified price and
risk boundaries. All 17 trees and finite differences records are accepted.
Runtime/caller/host/state reconciliation and independent qualification remain pending.

The first Monte Carlo package was accepted on 2026-11-11 for fixed-budget
pseudorandom execution, scalar discounting, independent estimator units and
fixed-sample uncertainty. [Monte Carlo record 01](models-monte-carlo/01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).
The second package was accepted on 2026-11-11 for Philox stream addresses,
fallible uniforms and normals, explicit FNV seed derivation and replay.
[Monte Carlo record 02](models-monte-carlo/02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).
The third package was accepted on 2026-11-11 for equal dyadic Sobol
blocks, checked whole-path construction and descriptive replicate spread.
[Monte Carlo record 03](models-monte-carlo/03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics).
The fourth package was accepted on 2026-11-11 for checked exact GBM,
bounded carry, dated ATM variance bootstrapping and log-Euler local
volatility. [Monte Carlo record 04](models-monte-carlo/04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths).
The fifth package was accepted on 2026-11-11 for checked coordinatewise
Euler, removal of obsolete generic steppers and exact Gaussian transitions.
[Monte Carlo record 05](models-monte-carlo/05-generic-steppers.md#accepted-decision-for-checked-coordinatewise-euler-and-exact-gaussian-steps).
The sixth package was accepted on 2026-11-11 for exact-knot discrete
dividend jumps, explicit carry and ex phases, and the capped cash model.
[Monte Carlo record 06](models-monte-carlo/06-cash-dividend-jumps.md#accepted-decision-for-checked-discrete-dividend-jumps-on-exact-grid-knots).
The seventh package was accepted on 2026-11-11 for checked Hull–White
endpoints and reconciliation of dated callers to the accepted curve fit.
[Monte Carlo record 07](models-monte-carlo/07-hull-white.md#accepted-decision-for-checked-hullwhite-endpoints-and-curve-fit-boundaries).
The eighth package was accepted on 2026-11-11 for checked CIR and CIR++
QE, the shared log moment and signed shift projection.
[Monte Carlo record 08](models-monte-carlo/08-cir-qe.md#accepted-decision-for-checked-cir-and-cir-quadratic-exponential-paths).
The ninth package was accepted on 2026-11-11 for checked Heston QE-M, an
exact deterministic variance limit and strict moment errors on the
declared grid.
[Monte Carlo record 09](models-monte-carlo/09-heston-qe.md#accepted-decision-for-checked-heston-qe-m-paths).
The tenth package was accepted on 2026-11-11 for checked displaced LMM
predictor-corrector, exact grid boundaries and explicit tenor-bond ratios.
[Monte Carlo record 10](models-monte-carlo/10-lmm.md#accepted-decision-for-checked-terminal-measure-lmm-paths).
The eleventh package was accepted on 2026-11-11 for checked revolving-credit
paths with clamped utilization, explicit spread and rate laws, and joint
Hull–White discounting for the dated fitted mode.
[Monte Carlo record 11](models-monte-carlo/11-credit-line.md#accepted-decision-for-checked-revolving-credit-paths).
The twelfth package was accepted on 2026-11-11 for checked exact
Schwartz–Smith paths with explicit factor initialization, scaled
covariance and spot expectation.
[Monte Carlo record 12](models-monte-carlo/12-schwartz-smith.md#accepted-decision-for-checked-schwartzsmith-paths-and-expectations).
The thirteenth package was accepted on 2026-11-11 for checked standalone
fBM and actual-grid RL with Brownian leverage and sampled-variance compensation.
[Monte Carlo record 13](models-monte-carlo/13-fractional-noise.md#accepted-decision-for-checked-fractional-noise-and-volterra-drivers).
The fourteenth package was accepted on 2026-11-11 for checked adapted
rough Bergomi paths, sampled-variance normalization and bounded
European call estimation.
[Monte Carlo record 14](models-monte-carlo/14-rough-bergomi.md#accepted-decision-for-checked-rough-bergomi-spot-and-pricing).
The fifteenth package was accepted on 2026-11-11 for checked rough Heston
hybrid paths with full Volterra history, explicit variance projection
and fixed-grid European estimation.
[Monte Carlo record 15](models-monte-carlo/15-rough-heston.md#accepted-decision-for-checked-rough-heston-paths-and-pricing).
The sixteenth package was accepted on 2026-11-11 for checked rough
Cheyette raw paths with model-owned amplitude, a signed shift, exact
frozen-amplitude state marginals and Brownian increment covariance.
[Monte Carlo record 16](models-monte-carlo/16-rough-cheyette.md#accepted-decision-for-checked-rough-cheyette-paths).
The seventeenth package was accepted on 2026-11-11 for checked terminal
and Asian payoffs with strict digital ties, exact observation coverage,
count-normalized fixings and separate settlement and diagnostic values.
[Monte Carlo record 17](models-monte-carlo/17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).
The eighteenth package was accepted on 2026-11-11 for checked barrier
monitoring and rebates with exact observation binding, sampled continuous
first passage, an explicit Heston approximation and same-source discounting.
[Monte Carlo record 18](models-monte-carlo/18-barrier-monitoring.md#accepted-decision-for-checked-monte-carlo-barrier-monitoring-and-rebates).
The nineteenth package was accepted on 2026-11-11 for checked lookback
extrema with explicit monitoring, qualified history and terminal fixings,
and range-safe conditional GBM extremum sampling.
[Monte Carlo record 19](models-monte-carlo/19-lookback-monitoring.md#accepted-decision-for-checked-monte-carlo-lookback-extrema).
The twentieth package was accepted on 2026-11-11 for checked autocallable
payoffs with exact observation rows, explicit call and coupon eligibility,
shared history replay and actual dated coupon and principal cash.
[Monte Carlo record 20](models-monte-carlo/20-autocallable.md#accepted-decision-for-checked-monte-carlo-autocallable-payoffs).
The twenty-first package was accepted on 2026-11-11 for checked cliquet
payoffs with exact period ends, shared historical and live accumulation,
and one payoff on the actual payment date.
[Monte Carlo record 21](models-monte-carlo/21-cliquet.md#accepted-decision-for-checked-monte-carlo-cliquet-payoffs).
The twenty-second package was accepted on 2026-11-11 for checked range
accrual with a fixed contractual denominator, exact tied observation
rows, qualified history and one cash amount on the actual payment date.
[Monte Carlo record 22](models-monte-carlo/22-range-accrual.md#accepted-decision-for-checked-monte-carlo-range-accrual).
The twenty-third package was accepted on 2026-11-11 for exact GBM
exercise nodes, explicit sparse-date and origin rules, immutable policy
replay and conditional uncertainty on held-out price paths.
[Monte Carlo record 23](models-monte-carlo/23-lsmc-exercise.md#accepted-decision-for-checked-holder-exercise-policy-training-and-pricing).
The twenty-fourth package was accepted on 2026-11-11 for strike-centered
holder powers, explicit standardized multivariate bases, bounded
full-rank SVD and immutable prediction.
[Monte Carlo record 24](models-monte-carlo/24-regression-policies.md#accepted-decision-for-checked-continuation-regression).
The twenty-fifth package was accepted on 2026-11-11 for actual spot nodes,
complete common-draw groups, signed finite sensitivities and fixed-sample
uncertainty.
[Monte Carlo record 25](models-monte-carlo/25-finite-difference-greeks.md#accepted-decision-for-checked-paired-monte-carlo-spot-greeks).
The twenty-sixth package was accepted on 2026-11-11 for density-only
payoff eligibility, consumed-shock scoring and joint execution
with marginal uncertainty in native Greek units.
[Monte Carlo record 26](models-monte-carlo/26-likelihood-ratio-greeks.md#accepted-decision-for-checked-gbm-likelihood-ratio-greeks).
The twenty-seventh package was accepted on 2026-11-11 for a qualified
expected control PV, a fixed or independently fitted coefficient,
complete paired groups and held-out fixed-sample uncertainty.
[Monte Carlo record 27](models-monte-carlo/27-control-variate.md#accepted-decision-for-checked-single-control-monte-carlo-pricing).
The twenty-eighth package was accepted on 2026-11-11 for exact retained
subsets, timed cash diagnostics, explicit model-owned schemes and
checked replay on stored knots. All 28 Monte Carlo packages are accepted.
[Monte Carlo record 28](models-monte-carlo/28-capture-and-simulation-contracts.md#accepted-decision-for-checked-path-capture-and-raw-simulation).

The first credit and liquidity package was accepted on 2026-11-11
for explicit distress score profiles, qualified ratio inputs,
checked affine evaluation and removal of automatic grading.
[Credit record 01](models-and-valuations-credit/01-financial-distress-scores.md#accepted-decision-for-checked-financial-distress-scores).
The second credit and liquidity package was accepted on 2026-11-11
for checked Gaussian conditioning, equal-year default rates,
complete PD scales and explicitly selected rating-factor tables.
[Credit record 02](models-and-valuations-credit/02-pd-scales-and-rating-data.md#accepted-decision-for-checked-pd-transforms-and-explicit-rating-data).
The third credit and liquidity package was accepted on 2026-11-11
for explicit migration state and default roles, checked generator fitting
and projection, and bounded jump-path execution and results.
[Credit record 03](models-and-valuations-credit/03-rating-migration.md#accepted-decision-for-checked-rating-migration-generator-fitting-and-paths).
The fourth credit and liquidity package was accepted on 2026-11-11
for explicit structural laws and measures, checked probability tails,
calibration repricing, survival and spreads, and bounded asset-only paths.
[Credit record 04](models-and-valuations-credit/04-structural-default.md#accepted-decision-for-checked-structural-default-models-and-calibrated-outputs).
The fifth credit and liquidity package was accepted on 2026-11-11 for
checked leverage hazard maps and local market anchoring, explicit PIK
action inputs and bounded paired equity comparison.
[Credit record 05](models-and-valuations-credit/05-hazard-and-toggle.md#accepted-decision-for-checked-leverage-hazard-and-pik-election-models).
The sixth credit and liquidity package was accepted on 2026-11-12
for explicit exposure and recovery assumptions, bounded workout and
downturn scenarios, checked beta recovery and priority allocation.
[Credit record 06](models-and-valuations-credit/06-recovery-and-lgd.md#accepted-decision-for-checked-exposure-loss-and-recovery-allocations),
[core beta-quantile amendment](core-numerics/distribution-functions.md#accepted-amendment-checked-rust-beta-quantile).
The seventh credit and liquidity package was accepted on 2026-11-12
for explicit monthly default rates, one-period copula conditioning and
name selection, aligned replay, and checked expected MDR.
[Credit record 07](models-and-valuations-credit/07-pool-defaults.md#accepted-decision-for-checked-pool-default-rates-and-period-copula-resolution).
The eighth credit and liquidity package was accepted on 2026-11-12
for explicit monthly prepayment models, checked expectations and state,
authored correlation structures, and validated shocks.
[Credit record 08](models-and-valuations-credit/08-prepayment-and-pool-correlation.md#accepted-decision-for-checked-prepayment-and-pool-correlation).
The ninth credit and liquidity package was accepted on 2026-11-12
for explicit checked copula laws and conditional factors, shared mixing,
calibrated random loading, bounded integration and whole-vector factor application.
[Credit record 09](models-and-valuations-credit/09-copula-kernels.md#accepted-decision-for-checked-copula-laws-and-latent-factors).
The tenth credit and liquidity package was accepted on 2026-11-12
for explicit signed factors and recovery, stable path replay, checked empirical
loss and tranche measures, and validated sample state.
[Credit record 10](models-and-valuations-credit/10-portfolio-loss-and-correlated-recovery.md#accepted-decision-for-checked-portfolio-loss-and-correlated-recovery).
The eleventh credit and liquidity package was accepted on 2026-11-12
for explicit liquidity data, distinct estimators and impact proxies, checked
execution schedules, supplied spread stress and capacity outcomes.
[Credit record 11](models-and-valuations-credit/11-liquidity.md#accepted-decision-for-checked-liquidity-and-execution-estimates).
The twelfth credit and liquidity package was accepted on 2026-11-13
for explicit single-name CDS terms, inclusive maturity, local hazard,
separate default settlement and matching dirty, clean and par valuations.
[Credit record 12](models-and-valuations-credit/12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis).
The thirteenth credit and liquidity package was accepted on 2026-11-13
for qualified quote replay, explicit rate and recovery risk, covered
expected loss, named default scenarios and margin factor deltas.
[Credit record 13](models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk).
The fourteenth credit and liquidity package was accepted on 2026-11-13
for explicit index allocation, confirmed event and earned coupon cash,
aggregate quotes and expected cash from the CDS pricing geometry.
[Credit record 14](models-and-valuations-credit/14-index-valuation.md#accepted-decision-for-checked-cds-index-valuation-and-cashflows).
The fifteenth credit and liquidity package was accepted on 2026-11-13
for the dated CDS forward, a bounded continuous-premium state, explicit
exercise cash and loss, checked fit and integration, and resolved settlement.
[Credit record 15](models-and-valuations-credit/15-cds-option-quadrature.md#accepted-decision-for-checked-cds-option-valuation).
The sixteenth credit and liquidity package was accepted on 2026-11-13
for matched quote risk and hedge basis, explicit model Vega and diffusion
Theta, rate and recovery bases, and checked standalone premium inversion.
[Credit record 16](models-and-valuations-credit/16-cds-option-risk-and-inversion.md#accepted-decision-for-checked-cds-option-risk-and-implied-volatility).
The seventeenth credit and liquidity package was accepted on 2026-11-13
for explicit pool and copula selection, original-pool seasoning and loss,
bounded grid and factor methods, and consistency checks across dates.
[Credit record 17](models-and-valuations-credit/17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses).
The eighteenth credit and liquidity package was accepted on 2026-11-13
for explicit tranche schedules and settlement, daily protection and premium,
confirmed cash through maturity, residual quotes and one valuation trace.
[Credit record 18](models-and-valuations-credit/18-tranche-projection.md#accepted-decision-for-checked-tranche-cashflows-and-settlement).
The nineteenth credit and liquidity package was accepted on 2026-11-13
for selected-model tranche risk, exact quote and rate replay, explicit
correlation and recovery stencils, and named economic default scenarios.
[Credit record 19](models-and-valuations-credit/19-tranche-risk.md#accepted-decision-for-checked-tranche-risk-and-default-scenarios).
The twentieth credit and liquidity package was accepted on 2026-11-13
for exact holder PV, explicit tender hurdle, typed issuer effects and
isolated gross leverage. All 20 detailed credit and liquidity records
are accepted.
[Credit record 20](models-and-valuations-credit/20-liability-management.md#accepted-decision-for-checked-exchange-and-liability-management-arithmetic).

The first shared valuation framework package was accepted on 2026-11-13
for exact dispatch, validated ingress, effective dates, raw and Money
pricing, active market bindings, metric ownership and composite valuation.
[Valuation framework record 01](valuations-framework/pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).
The second shared valuation framework package was accepted on 2026-11-13
for exact volatility and parameter sources, dated carry, and explicit
simulation, lattice and quote controls.
[Valuation framework record 02](valuations-framework/model-resolution.md#accepted-decision-for-checked-valuation-model-inputs-and-controls).
The third shared valuation framework package was accepted on 2026-11-13
for qualified listed contract terms, marks and settlement references, option
margining, dated American delivery, authoritative exercise and native risk.
[Valuation framework record 03](valuations-framework/listed-products.md#accepted-decision-for-checked-listed-futures-and-options).
The fourth shared valuation framework package was accepted on 2026-11-13
for overnight quote periods and block weights, publication-aware selection,
native growth, product cash, distinct quote derivatives and fixing exports.
[Valuation framework record 04](valuations-framework/overnight-projection.md#accepted-decision-for-checked-overnight-projection-and-quote-risk).
The fifth shared valuation framework package was accepted on 2026-11-13
for exact dates, native source clocks, local hazard, explicit joint model
controls, interval recovery and eligible floating parent folds.
[Valuation framework record 05](valuations-framework/rates-credit-grid.md#accepted-decision-for-checked-dated-rates-credit-valuation).
The sixth shared valuation framework package was accepted on 2026-11-13
for fixed-notional TRS terms, actual payment dates, publication-qualified
financing, spread accrual and par on one basis, and faithful cash exports.
[Valuation framework record 06](valuations-framework/trs-legs.md#accepted-decision-for-checked-trs-legs-and-par-spread).
The seventh shared valuation framework package was accepted on 2026-11-13
for contractual variance observations and explicit annualization, qualified
estimator samples, explicit barrier monitoring and exact event binding.
[Valuation framework record 07](valuations-framework/observation-grids.md#accepted-decision-for-checked-observation-schedules-and-monitoring-grids).
The eighth shared valuation framework package was accepted on 2026-11-13
for metric producers and dependencies, scoped caches, exact attribution,
lossless reporting and compatible aggregation.
[Valuation framework record 08](valuations-framework/metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).
The ninth shared valuation framework package was accepted on 2026-11-13
for explicit controls, represented stencils, typed sources, direct and
quote-replayed DV01, and exact key-rate buckets.
[Valuation framework record 09](valuations-framework/finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk).

The tenth shared valuation framework package was accepted on 2026-11-13 for
exact quote and intensity risk, settlement z-spread, disjoint cash bands,
explicit recovery experiments and owner rate components.
[Valuation framework record 10](valuations-framework/credit-sensitivities.md#accepted-decision-for-checked-credit-sensitivities).

The eleventh shared valuation framework package was accepted on 2026-11-13
for selected-model Greeks, actual shock widths, native volatility risk and
sparse vega buckets, fixed directional FX01 and distinct theta bases.
[Valuation framework record 11](valuations-framework/option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).

The twelfth shared valuation framework package was accepted on 2026-11-13
for exact factor directions, represented mixed stencils, fixed-center
corner credit replay and matched attribution.
[Valuation framework record 12](valuations-framework/cross-factor-risk.md#accepted-decision-for-checked-cross-factor-risk).

The thirteenth shared valuation framework package was accepted on 2026-11-14
for explicit holder cash and valuation cuts, separate theta and carry bases,
funding and optional yield partition, and exact horizon breakeven targets.
[Valuation framework record 13](valuations-framework/carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).

The fourteenth shared valuation framework package was accepted on 2026-11-14
for explicit history and typed scenarios, matched repricing and Taylor,
and signed R-7 VaR and fractional empirical expected shortfall.
[Valuation framework record 14](valuations-framework/historical-risk.md#accepted-decision-for-checked-historical-scenario-risk).

The fifteenth shared valuation framework package was accepted on 2026-11-14
for qualified schedule cash and term observations, product-owned PV
attribution, checked reconciliation and faithful ladders and traces.
[Valuation framework record 15](valuations-framework/cashflow-export.md#accepted-decision-for-checked-cashflow-valuation-and-export).

The sixteenth shared valuation framework package was accepted on 2026-11-14
for exact typed dated convention selection, scoped family terms, explicit
contract choices and qualified editions through construction and replay.
[Valuation framework record 16](valuations-framework/convention-selection.md#accepted-decision-for-checked-convention-selection-and-dated-sources).

The seventeenth shared valuation framework package was accepted on 2026-11-14
for explicit inflation references, separate knowledge dates and fixing vintages,
qualified projection, issuer rules, and a checked discount-factor inverse.
[Valuation framework record 17](valuations-framework/inflation-references.md#accepted-decision-for-checked-inflation-references-and-information-dates).

The eighteenth shared valuation framework package was accepted on 2026-11-14
for prepared repricing, complete signed risk on exact factors and vertices,
explicit credit attribution, and qualified netting scope.
[Valuation framework record 18](valuations-framework/margin-bridge.md#accepted-decision-for-checked-margin-repricing-and-identity).

The nineteenth shared valuation framework package was accepted on 2026-11-14
for checked contractual terms and defaults, dated exercise and PIK selection,
ordered quanto binding, explicit stochastic controls and complete results.
[Valuation framework record 19](valuations-framework/shared-contract-controls.md#accepted-decision-for-checked-shared-contract-and-stochastic-controls).

The twentieth shared valuation framework package was accepted on 2026-11-14
for strict builder declarations, lazy defaults, one structural validator and
aligned restore, completing adoption of the 20 detailed framework records.
[Valuation framework record 20](valuations-framework/builder-validation.md#accepted-decision-for-checked-generated-financial-builders).

The first rates valuation package was accepted on 2026-11-14 for
resolved deposit dates, checked simple interest, separate holder and
trade cuts, dated factors, inception quote inversion and qualified risk.
[Rates valuation record 01](valuations-rates/01-deposits.md#accepted-decision-for-checked-deposit-cashflows-and-quote-inversion).

The second rates valuation package was accepted on 2026-11-14 for
exact FRA benchmark dates, publication-qualified fixing, checked upfront
settlement, explicit payment cuts and distinct direct and replay risk.
[Rates valuation record 02](valuations-rates/02-fra.md#accepted-decision-for-checked-fra-fixing-settlement-and-risk).

The third rates valuation package was accepted on 2026-11-14 for
resolved interest-rate swap schedules, term and direct-growth overnight
cash, eligible payments and par, distinct risk and qualified margin repricing.
[Rates valuation record 03](valuations-rates/03-interest-rate-swaps.md#accepted-decision-for-checked-interest-rate-swap-cashflows-and-risk).

The fourth rates valuation package was accepted on 2026-11-14 for
resolved basis swap legs, qualified source roles, eligible term and
overnight cash, replacement and incremental spread, and distinct rate risk.
[Rates valuation record 04](valuations-rates/04-basis-swaps.md#accepted-decision-for-checked-basis-swap-cashflows-and-par-spreads).

The fifth rates valuation package was accepted on 2026-11-14 for
resolved cap and floor optionlets, exact quote and payment bases, the
Hull–White term law and the qualified overnight approximation.
[Rates valuation record 05](valuations-rates/05-cap-floor-pricing.md#accepted-decision-for-checked-cap-and-floor-optionlet-pricing).

The sixth rates valuation package was accepted on 2026-11-14 for local
quote Greeks, declared Hull–White risks, exact active curve roles, holder
horizon theta and standalone gross optionlet premium inversion.
[Rates valuation record 06](valuations-rates/06-cap-floor-risk.md#accepted-decision-for-checked-cap-and-floor-risk-and-implied-volatility).

The seventh rates valuation package was accepted on 2026-11-14 for the
checked European swaption forward, settlement elections, live quote law
and expiry handoff, and distinct risk and gross premium inversion.
[Rates valuation record 07](valuations-rates/07-european-swaptions.md#accepted-decision-for-checked-european-swaption-pricing-and-risk).

The eighth rates valuation package was accepted on 2026-11-14 for exact
Hull–White swaption exercise cash, checked tree and exercise statistics,
joint Gaussian discounting, held-out policy pricing and named risk.
[Rates valuation record 08](valuations-rates/08-hull-white-swaptions.md#accepted-decision-for-checked-hull-white-swaption-exercise-and-risk).

The ninth rates valuation package was accepted on 2026-11-14 for exact
physical co-terminal LMM terms, explicit model inputs, terminal-measure
pricing, held-out exercise policies and paired curve risk.
[Rates valuation record 09](valuations-rates/09-lmm-bermudans.md#accepted-decision-for-checked-lmm-bermudan-pricing-and-risk).

The tenth rates valuation package was accepted on 2026-11-14 for
resolved rate futures terms and benchmark projection, explicit convexity,
official clearing and cash treatment, and separate futures option risk bases.
[Rates valuation record 10](valuations-rates/10-rate-futures.md#accepted-decision-for-checked-rate-futures-and-futures-option-risk).

The eleventh rates valuation package was accepted on 2026-11-14 for the
CMS reference swap and fixing basis, dated first-order convexity, checked
coupon and collar PV, and selected-method risk and cashflow reporting.
[Rates valuation record 11](valuations-rates/11-cms-convexity.md#accepted-decision-for-checked-cms-convexity-pricing-and-risk).

The twelfth rates valuation package was accepted on 2026-11-14 for exact
CMS smile binding, explicit finite support and price chords, common payment
normalization, and selected-method risk and cashflow reporting.
[Rates valuation record 12](valuations-rates/12-cms-replication.md#accepted-decision-for-checked-cms-smile-replication-and-risk).

The thirteenth rates valuation package was accepted on 2026-11-14. It
specifies CPI references and first contractual CPI, actual payment cuts,
zero-coupon and periodic payoff laws, verified par quotes, typed CPI and
nominal risk, and consistent calibration and cashflow reporting.
[Rates valuation record 13](valuations-rates/13-inflation-swaps.md#accepted-decision-for-checked-inflation-swap-cashflows-and-risk).

The fourteenth rates valuation package was accepted on 2026-11-14. It
specifies actual period payoffs and CPI knowledge cuts, live forward
qualification, explicit quote expiry and model, and one optionlet ledger
for price, cash export and typed risk.
[Rates valuation record 14](valuations-rates/14-inflation-options.md#accepted-decision-for-checked-inflation-option-cashflows-expiry-and-risk).

The fifteenth rates valuation package was accepted on 2026-11-14. It
specifies a final contractual repo rate, fixed or overnight settlement,
cash entitlement and rounding, same-currency collateral diagnostics,
separate funding and discount risk, and agreement-qualified margin.
[Rates valuation record 15](valuations-rates/15-repo.md#accepted-decision-for-checked-repo-cashflows-collateral-and-risk).

The sixteenth rates valuation package was accepted on 2026-11-14. It
specifies cross-currency leg and principal terms, actual reset dates,
coherent collateral and cash FX, the spread quote basis and directed
risk with one prepared native cash ledger.
[Rates valuation record 16](valuations-rates/16-cross-currency-swaps.md#accepted-decision-for-checked-cross-currency-swap-cashflows-fx-resets-and-risk).

The seventeenth rates valuation package was accepted on 2026-11-14. It
specifies the standalone CMS spread payoff, separate benchmarks,
conditional lognormal model, exact limits, bounded integration, and
selected-model cash export and rate risk. All 17 detailed rates records
are accepted.
[Rates valuation record 17](valuations-rates/17-cms-spread.md#accepted-decision-for-checked-cms-spread-cashflows-model-and-risk).

The first bond valuation package was accepted on 2026-11-14. It
specifies one generated or supplied cashflow source, explicit terms,
separate cash and principal dates, claim-level entitlement and the
selected valuation model's cash and metric basis.
[Bond valuation record 01](valuations-bonds/01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash).

The second bond valuation package was accepted on 2026-11-14. It
specifies eligible settlement, coupon-parent accrued, the separate
financing and security roles in clean/dirty carry, model-owned
contingent valuation, and independent yield and dated model clocks.
[Bond valuation record 02](valuations-bonds/02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks).

The third bond valuation package was accepted on 2026-11-14. It
specifies model selection, settlement-quote marks, signed security
discounting, spread conventions and prompt fractional recovery of
covered par on local hazard windows.
[Bond valuation record 03](valuations-bonds/03-pv-models.md#accepted-decision-for-checked-bond-pv-selection-and-fractional-recovery-of-par).

The fourth bond valuation package was accepted on 2026-11-14. It
specifies explicit rates-only lattice and fit controls, exact dated
exercise and cash, one deterministic-reference make-whole call law,
and checked nodewise OAS and reporting.
[Bond valuation record 04](valuations-bonds/04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise).

The fifth bond valuation package was accepted on 2026-11-14. It
specifies explicit issue-basis MOIC or fixed ACT/365F XIRR protection
on authorized early calls, qualified paid-through history, and one
dirty-strike law for deterministic and pathwise exercise.
[Bond valuation record 05](valuations-bonds/05-return-floors.md#accepted-decision-for-checked-issue-basis-return-floors).

The sixth bond valuation package was accepted on 2026-11-14. It
specifies exact dated joint payoff replay, publication-aware index
gross projection, parent cash/PIK and exercise phases, separate
recoverable par and checked interval recovery and continuation.
[Bond valuation record 06](valuations-bonds/06-joint-coupon-replay.md#accepted-decision-for-checked-joint-bond-payoff-and-recovery-replay).

The seventh bond valuation package was accepted on 2026-11-14. It
specifies the conditional make-whole benchmark, fixed continuation
regression, non-anticipating stopping, separate simulation stages and
the uncertainty of pricing with frozen policies.
[Bond valuation record 07](valuations-bonds/07-lsmc-policy.md#accepted-decision-for-checked-joint-bond-exercise-policy-and-make-whole).

The eighth bond valuation package was accepted on 2026-11-14. It
specifies structural default and detecting-node recovery, exact dated
coupon and PIK replay, checked simulation, signed benchmark losses
and a verified effective-spread calculation.
[Bond valuation record 08](valuations-bonds/08-merton-simulation.md#accepted-decision-for-checked-structural-bond-simulation-and-outputs).

The ninth bond valuation package was accepted on 2026-11-14. It
specifies a qualified cash-equivalent settlement target, one prepared
dated basis, a bounded price-residual search and independent cash
validation before contractual repricing.
[Bond valuation record 09](valuations-bonds/09-merton-calibration.md#accepted-decision-for-checked-cash-equivalent-structural-calibration).

The tenth bond valuation package was accepted on 2026-11-14. It
specifies explicit settlement yield conventions, contractual quote
coordinates, qualified named market profiles and price-residual
checks for returned yields.
[Bond valuation record 10](valuations-bonds/10-quoted-yield.md#accepted-decision-for-checked-settlement-yield-and-price-inversion).

The bond workout package was accepted on 2026-11-14, completing all
twenty detailed bond decisions. It specifies an issuer-controlled YTW
minimum on a common settlement buyer basis, full branch verification
for inverse YTW and fixed-path-only I-spread inversion.
[Bond valuation record 11](valuations-bonds/11-workout-yield.md#accepted-decision-for-checked-bond-workouts-and-yield-to-worst).

The bond spread package was accepted on 2026-11-14. It specifies
settlement-native Z-spread and curve discount margin, separate swap
and government benchmarks, selected-model buyer-claim OAS and checked
price residuals. Workout-dependent calculations require a qualified
workout basis.
[Bond valuation record 12](valuations-bonds/12-spread-quotes.md#accepted-decision-for-checked-bond-spreads-and-price-inversion).

The bond asset-swap package was accepted on 2026-11-14. It specifies
face-notional par and dirty-notional proceeds structures, a matched
settlement buyer claim, explicit floating-leg and source roles, and checked
spread and price conversions. It adopts the corresponding published
metric meanings and quote-input reconciliation.
[Bond valuation record 13](valuations-bonds/13-asset-swaps.md#accepted-decision-for-checked-bond-asset-swaps-and-price-inversion).

The quoted-yield bond risk package was accepted on 2026-11-14. It uses
one fixed settlement path and exact yield-law derivatives, retains the
signed currency tangent for yield DV01 and adopts unscaled bond yield
convexity `P″/P` as the published value.
[Bond valuation record 14](valuations-bonds/14-yield-risk.md#accepted-decision-for-checked-quoted-yield-bond-risk).

The curve and effective bond risk package was accepted on 2026-11-14.
It fixes the holder PV origin, quote-center and direct rate-source rules,
the signed parallel and triangular risk definitions, and distinct
unscaled effective measures with paired model replay where applicable.
[Bond valuation record 15](valuations-bonds/15-curve-risk.md#accepted-decision-for-checked-bond-curve-and-effective-risk).

The bond CS01 and spread duration package was accepted on 2026-11-14.
It distinguishes exact par-CDS replay on holder PV from the instrument's
forward one-bp settlement Z price change and normalizes spread duration
on that same Z basis.
[Bond valuation record 16](valuations-bonds/16-spread-credit-risk.md#accepted-decision-for-checked-bond-spread-and-credit-risk).

The bond recovery and jump-to-default package was accepted on 2026-11-14.
It specifies explicit frozen-intensity and quote-preserving Recovery01 on
raw holder PV and prompt fractional recovery on covered par at the actual
event phase, net of the value of terminated holder rights.
[Bond valuation record 17](valuations-bonds/17-default-risk.md#accepted-decision-for-checked-bond-recovery-and-jump-to-default).

The bond embedded-option package was accepted on 2026-11-14. It
specifies matched live and disabled holder claims, qualified fixed OAS,
explicit native-sigma stencils and paired pricing. It adopts the
`embedded_option_vega` key in signed native currency per `+.01` model sigma.
[Bond valuation record 18](valuations-bonds/18-embedded-option-risk.md#accepted-decision-for-checked-bond-embedded-option-value-and-vega).

The bond carry and repayment-life package was accepted on 2026-11-14.
It specifies explicit phase-matched holder carry, elected funding and
optional exact fixed-yield repricing, and weights bond WAL by retired
par on actual repayment dates under ACT/365F.
[Bond valuation record 19](valuations-bonds/19-carry-and-life.md#accepted-decision-for-checked-bond-carry-and-repayment-life).

The issue-to-exit return package was accepted on 2026-11-14. It
specifies explicit original investment, covered signed cash, net MOIC,
canonical ACT/365F XIRR and complete issuer-call minima with exact cash
phases and no skipped failed candidates.
[Bond valuation record 20](valuations-bonds/20-lifetime-returns.md#accepted-decision-for-checked-bond-issue-to-exit-returns).

The checked bond-futures package was accepted on 2026-11-14. It
requires an explicit fixed deliverable, authoritative factor and
financing, with listed marking and a separate physical delivery
invoice on their own contractual bases.
[Other fixed-income record 01](valuations-other-fixed-income/01-bond-futures.md#accepted-decision-for-checked-bond-futures-carry-factors-and-listed-marks).

The convertible lattice package was accepted on 2026-11-14. It
requires explicit default and market roles, exact event placement,
contractual exercise and entitlement, and effective conversion
adjustments. Surviving fixed cash and share claims keep their dated basis.
[Other fixed-income record 02](valuations-other-fixed-income/02-convertible-lattice.md#accepted-decision-for-checked-convertible-lattice-and-exercise).

The checked convertible quotes and risk package was accepted on
2026-11-14. It uses matched buyer claims and signed cash accrued,
with separate option-adjusted discount and issuer intensity fits,
returned-price residuals and explicit dated risk experiments.
[Other fixed-income record 03](valuations-other-fixed-income/03-convertible-quotes-risk.md#accepted-decision-for-checked-convertible-quotes-and-risk).

The checked inflation-linked bond indexation and floors package was
accepted on 2026-11-14. It binds issue-specific references and
protection, point-in-time anchor resolution, real-par retirement
and dated nominal cash and floor valuation.
[Other fixed-income record 04](valuations-other-fixed-income/04-inflation-indexation.md#accepted-decision-for-checked-inflation-linked-bond-indexation-and-floors).

The checked inflation-bond yields, breakeven and risk package was
accepted on 2026-11-14. It uses explicit real settlement quote laws,
analytic real duration, an authorized comparable nominal coupon
benchmark and typed actual projection and nominal curve risk.
[Other fixed-income record 05](valuations-other-fixed-income/05-inflation-yields-risk.md#accepted-decision-for-checked-inflation-bond-yields-breakeven-and-risk).

The shared loan quotes, cash cost and effective-interest package was
accepted on 2026-11-14. It requires an entitled buyer cash strip,
explicit curve DM and annual-effective yield, covered funded balance-time,
and an authorized origination cash ledger for effective interest.
[Other fixed-income record 06](valuations-other-fixed-income/06-loan-quotes-eir.md#accepted-decision-for-checked-loan-quotes-cash-cost-and-effective-interest).

The checked term-loan cashflow package was accepted on 2026-11-14.
It binds covered funding and contractual dates, nonrevolving draw
capacity, settled OID allocations, live principal phases, coupon
and PIK elections and independently authored fee periods.
[Other fixed-income record 07](valuations-other-fixed-income/07-term-loan-cashflows.md#accepted-decision-for-checked-term-loan-cashflow-compilation).

The checked term-loan pricing and exercise package was accepted on
2026-11-14. It uses original-cut fixing-led valuation, a separate
settlement buyer bridge, exact rates or rates-credit lattice dates,
eligible-par recovery and contractual exercise and OAS controls.
[Other fixed-income record 08](valuations-other-fixed-income/08-term-loan-pricing.md#accepted-decision-for-checked-term-loan-valuation-and-exercise).

The checked term-loan workout yields and risk package was accepted on
2026-11-14. It uses actual buyer settlement, complete borrower workouts,
separate quote laws, held model risk and phased recovery and default.
[Other fixed-income record 09](valuations-other-fixed-income/09-term-loan-risk-yields.md#accepted-decision-for-checked-term-loan-workout-yields-and-risk).

The checked revolver cashflows package was accepted on 2026-11-14.
It uses covered elapsed state, exact phased revolving capacity, independently
dated claims and fee parents, and modeled funding at explicit dated nodes.
[Other fixed-income record 10](valuations-other-fixed-income/10-revolver-cashflows.md#accepted-decision-for-checked-revolver-contractual-cashflows).

The checked revolver simulation and default valuation package was accepted on
2026-11-14. It covers phased joint funding, fitted bank discounting, explicit
credit modes, claim-level survival and recovery, and checked Monte Carlo results.
[Other fixed-income record 11](valuations-other-fixed-income/11-revolver-monte-carlo-credit.md#accepted-decision-for-checked-revolver-simulation-and-default-valuation).

The checked revolver quotes, exposure and risk package was accepted on
2026-11-14. It covers actual buyer and accrued claims, cash-cost and EIR
bases, phased exposure, same-path expected loss and distinct risk experiments.
[Other fixed-income record 12](valuations-other-fixed-income/12-revolver-quotes-risk.md#accepted-decision-for-checked-revolver-quotes-exposure-and-risk).

The checked agency MBS projection and cash ownership package was accepted on
2026-11-14. It covers dated fixed-rate pool state, exact principal retirement,
security-specific remittance, factor vintages and holder and purchaser claims.
[Other fixed-income record 13](valuations-other-fixed-income/13-mbs-projection.md#accepted-decision-for-checked-agency-mbs-projection-and-cash-ownership).

The checked agency MBS rate risk and OAS package was accepted on
2026-11-14. It covers the shared monthly refinancing response, source-exact
held-spread risk, dated Hull–White bank discounting and the verified buyer spread fit.
[Other fixed-income record 14](valuations-other-fixed-income/14-mbs-risk-oas.md#accepted-decision-for-checked-agency-mbs-rate-risk-and-oas).

The checked TBA settlement and dollar-roll package was accepted on
2026-11-14. It covers dated delivery authority and actual face, transferred
claims and invoice valuation, physical reporting and surviving-face financing.
[Other fixed-income record 15](valuations-other-fixed-income/15-tba-and-dollar-roll.md#accepted-decision-for-checked-tba-settlement-and-dollar-rolls).

The checked fixed-income index TRS package was accepted on 2026-11-14. It
covers contractual return observations and settlement, explicit carry, shared
financing and verified par spread, curve and hedge risk, margin and reporting.
All sixteen other fixed-income records are accepted.
[Other fixed-income record 16](valuations-other-fixed-income/16-fi-index-trs.md#accepted-decision-for-checked-fixed-income-index-total-return-swaps).

The checked instrument loading, model dispatch and metrics package was accepted
on 2026-11-14. It covers strict admission, post-overlay validation, exact
registry selection, role-aware result metadata and schema qualification.
[Equity and composition record 01](valuations-equity-and-composition/01-contract-dispatch.md#accepted-decision-for-checked-instrument-loading-model-dispatch-and-metrics).

The checked cash-equity valuation and dividend carry package was accepted on
2026-11-14. It covers explicit shares and quote exposure, dated funding and
carry, payment-date discounting and separate actual holder claims.
[Equity and composition record 02](valuations-equity-and-composition/02-spot-and-carry.md#accepted-decision-for-checked-cash-equity-valuation-and-dividend-carry).

The checked equity futures package was accepted on 2026-11-14. It covers
fixing-date carry, fixed-conversion quanto, phased clearing, selected-mark
risk and qualified cash reporting.
[Equity and composition record 03](valuations-equity-and-composition/03-equity-future.md#accepted-decision-for-checked-equity-futures-carry-quanto-and-clearing).

The checked equity total-return futures package was accepted on 2026-11-14.
It covers qualified spread-to-points conversion, a contract-specific spread
clock, selected-mark sensitivities and phased clearing and cash reporting.
[Equity and composition record 04](valuations-equity-and-composition/04-total-return-future.md#accepted-decision-for-checked-equity-total-return-futures).

The checked volatility-index futures and listed futures-option package was
accepted on 2026-11-14. It covers exact quote and fixing authority,
futures-point DeltaVol, matched option models, and exercise and cash state.
[Equity and composition record 05](valuations-equity-and-composition/05-vol-index-and-listed-options.md#accepted-decision-for-checked-volatility-index-futures-and-listed-futures-options).

The checked equity option exercise and settlement package was accepted on
2026-11-14. It covers authoritative exercise state, signed cash and ordinary
physical claims, claim-specific risk and horizon and reporting rules.
[Equity and composition record 06](valuations-equity-and-composition/06-option-lifecycle.md#accepted-decision-for-checked-equity-option-exercise-and-settlement).

The checked live equity option pricing and Greeks package was accepted on
2026-11-14. It covers exact model selection, separate fixing and payment,
qualified dividend and volatility bases, exact tree placement and matched risk.
[Equity and composition record 07](valuations-equity-and-composition/07-black-tree-and-direct-greeks.md#accepted-decision-for-checked-live-equity-option-pricing-and-greeks).

The checked selected-model equity risk and implied-volatility package was
accepted on 2026-11-14. It covers matched raw reprices, typed sensitivity
experiments, Dividend01 on actual signed yield and bounded European
Black–Scholes inversion on the observed gross premium basis.
[Equity and composition record 08](valuations-equity-and-composition/08-selected-risk-and-implied-vol.md#accepted-decision-for-checked-selected-model-equity-risk-and-implied-volatility).

The equity Black–Scholes and Heston PDE package was accepted on 2026-11-14.
It covers dated fixing and payment, qualified American stopping,
widened log-spot domains, strict unit bounds and matched risk.
[Equity and composition record 09](valuations-equity-and-composition/09-equity-pde.md#accepted-decision-for-checked-equity-blackscholes-and-heston-pde-pricing).

The dated equity Heston Fourier and QE-M package was accepted on 2026-11-14.
It covers scoped model inputs, the actual payment discount, bounded Fourier,
explicit fixed-budget simulation and separate sampling, bias and risk evidence.
[Equity and composition record 10](valuations-equity-and-composition/10-heston-adapters.md#accepted-decision-for-checked-dated-heston-fourier-and-monte-carlo-pricing).

The equity rough Heston and rough Bergomi adapter package was accepted on
2026-11-14. It covers scoped model inputs and dated payment, qualified
variance sources and exact break alignment, bounded parity estimation,
explicit history work and matched risk.
[Equity and composition record 11](valuations-equity-and-composition/11-rough-volatility-adapters.md#accepted-decision-for-checked-equity-rough-volatility-adapters).

The ATM term-GBM and pinned equity path-risk package was accepted on
2026-11-14. It covers dated carry and native-clock ATM variance, exact
live observation grids, qualified held fixing state, explicit simulation
work and responsive selected-model risk.
[Equity and composition record 12](valuations-equity-and-composition/12-path-gbm.md#accepted-decision-for-checked-atm-term-gbm-and-pinned-equity-path-risk).

The dated autocallable package was accepted on 2026-11-14. It covers
observation rows and qualified fixings, coupon-first replay and redemption,
per-leg dated settlement and one held-spot holder horizon scenario.
[Equity and composition record 13](valuations-equity-and-composition/13-autocallable.md#accepted-decision-for-checked-dated-autocallable-pricing-and-risk).

The dated cliquet package was accepted on 2026-11-14. It covers distinct
strike-setting and reset roles, checked return accumulation and one dated
option payment, paired risk precision and phase-based holder theta.
[Equity and composition record 14](valuations-equity-and-composition/14-cliquet.md#accepted-decision-for-checked-dated-cliquet-pricing-and-risk).

The equity variance swap package was accepted on 2026-11-14. It covers
qualified scheduled history, close-to-close interval forecasts,
complete-history OHLC settlement, dated cash and active-source risk.
[Equity and composition record 15](valuations-equity-and-composition/15-variance-swap.md#accepted-decision-for-checked-equity-variance-swap-pricing-and-risk).

The dated equity total-return swap package was accepted on 2026-11-14.
It covers qualified reset and cash state, explicit carry and payment roles,
dividend treatment, shared financing and par, and qualified margin repricing.
[Equity and composition record 16](valuations-equity-and-composition/16-equity-trs.md#accepted-decision-for-checked-dated-equity-total-return-swaps).

The corporate DCF and dilution package was accepted on 2026-11-14.
It covers authored forecast and terminal timing, normalized annual growth
terminal cash flow, the checked common-equity bridge and treasury-stock
active set, and full WACC repricing.
[Equity and composition record 17](valuations-equity-and-composition/17-corporate-dcf.md#accepted-decision-for-checked-corporate-dcf-and-dilution).

The dated private-fund waterfall and final clawback package was accepted on
2026-11-14. It covers exact event order, dated hurdle and complete LP
allocation, reachable catch-up and promote, explicit escrow and final settlement.
[Equity and composition record 18](valuations-equity-and-composition/18-private-fund-waterfall.md#accepted-decision-for-checked-private-fund-waterfalls-and-final-clawback).

The private-fund residual value, performance and risk package was accepted on
2026-11-14. It covers the NAV or projected valuation basis, outstanding cash
and commitments, settled LP performance and aggregate carry, and prospective
parameter and current NAV risk.
[Equity and composition record 19](valuations-equity-and-composition/19-private-fund-valuation.md#accepted-decision-for-checked-private-fund-residual-value-performance-and-risk).

The property valuation and unlevered return package was accepted on
2026-11-14. It covers explicit appraisal or modeled property value,
qualified annual NOI, dated exit and operating cash, the separate
acquisition basis and active rate risk and export.
[Equity and composition record 20](valuations-equity-and-composition/20-property-valuation.md#accepted-decision-for-checked-property-valuation-and-unlevered-returns).

The levered property value and financing package was accepted on 2026-11-14.
It covers matched lender marks, borrower cash and obligation-level exit
payoff, market and origination LTV, aligned operating coverage and isolated
property and financing risk.
[Equity and composition record 21](valuations-equity-and-composition/21-levered-property.md#accepted-decision-for-checked-levered-property-value-and-financing).

The basket value, expense and risk package was accepted on 2026-11-14.
It covers explicit lot or coefficient sizing and basket units, qualified
marks and FX, known accrued expenses, exact source mark shocks and
per-target weight scenarios.
[Equity and composition record 22](valuations-equity-and-composition/22-basket-nav-and-risk.md#accepted-decision-for-checked-basket-value-expenses-and-risk).

The composite sizing and rebalance date package was accepted on 2026-11-14.
It covers fixed quantities, gross lot notional, matched metric contributions,
settled-cash volatility, scalar expressions, frozen state and exact eligible
dates.
[Equity and composition record 23](valuations-equity-and-composition/23-composite-sizing.md#accepted-decision-for-checked-composite-sizing-and-rebalance-dates).

The composite valuation and financed history package was accepted on 2026-11-14,
completing adoption for all 24 equity and composition records. It covers
matched nested FX, strict risk totals, exact proposed trade changes, settled
cash at exact rebalance closes and a fixed-capital index with a disclosed
modeled financing transfer.
[Equity and composition record 24](valuations-equity-and-composition/24-composite-valuation-history.md#accepted-decision-for-checked-composite-valuation-and-financed-history).

The deliverable FX package was accepted on 2026-11-14. It covers booked
two-currency spot, forward and swap cash, fixed value dates and the
settlement cut, cash FX with an actual spot-date bridge, quote
diagnostics and risk and export on fixed booked terms.
[FX-DELIVERY-CIP](valuations-fx-and-exotics/fx-delivery-cip.md#accepted-decision-for-checked-deliverable-fx-and-covered-interest-parity).

The NDF fixing and settlement package was accepted on 2026-11-14.
It covers fixed strikes, exact fixing authority, explicit pending
projection, payment-date discounting and state-scoped risk. The existing
core cashflow record now includes cash-only `DerivativeSettlement`.
[NDF-FIXING-SETTLEMENT](valuations-fx-and-exotics/ndf-fixing-settlement.md#accepted-decision-for-checked-ndf-fixing-and-settlement) ·
[CORE-CONVENTIONS-015](core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata).

The listed FX package was accepted on 2026-11-14. It covers exact series
and quote units, observed or elected model marks, separate variation and
delivery, shared option pricing with authoritative exercise and risk and
cash export on the resolved claim.
[FX-LISTED-CONTRACTS](valuations-fx-and-exotics/fx-listed-contracts.md#accepted-decision-for-checked-listed-fx-futures-and-futures-options).

The FX vanilla package was accepted on 2026-11-14. It covers European
delivery pricing on the expiry clock, authoritative exercise and premium
cash, PV and hedge deltas, exact model boundaries and bounded IV, dated
risk and explicit smile response.
[FX-VANILLA-DELTA](valuations-fx-and-exotics/fx-vanilla-delta.md#accepted-decision-for-checked-fx-vanilla-pricing-and-delta-conventions).

The FX digital package was accepted on 2026-11-14. It covers native
quote or base payout, the benchmark forward and payment date, strict
observed and deterministic triggers, fixed residual claims and dated
risk and cashflow export.
[FX-DIGITAL](valuations-fx-and-exotics/fx-digital.md#accepted-decision-for-checked-fx-digital-pricing-and-settlement).

The FX barrier package was accepted on 2026-11-14. It covers exact
monitoring and retained claims, total quote rebates, the qualified
continuous analytic law, dated GBM with exact observations and paired
market risk and cashflow export on the resolved state.
[FX-BARRIERS](valuations-fx-and-exotics/fx-barriers.md#accepted-decision-for-checked-fx-barrier-monitoring-and-settlement).

The FX touch package was accepted on 2026-11-14. It covers inclusive
authorized touch, immediate first-passage and separately dated deferred
payments, retained native quote claims and active-source risk.
[FX-TOUCH](valuations-fx-and-exotics/fx-touch.md#accepted-decision-for-checked-fx-touch-pricing-and-settlement).

The FX variance package was accepted on 2026-11-14. It covers qualified
observations, explicit annualization, checked smile and dated carry for
pending close returns, separately dated quote cash and active-source risk.
[FX-VARIANCE](valuations-fx-and-exotics/fx-variance.md#accepted-decision-for-checked-fx-variance-swap-pricing-and-risk).

The quanto package was accepted on 2026-11-14. It covers explicit fixed
conversion, directed FX and correlation, domestic-measure carry, qualified
dated claims and separate asset, FX, correlation and rate risk.
[QUANTO-DRIFT](valuations-fx-and-exotics/quanto-drift.md#accepted-decision-for-checked-dated-quanto-pricing-and-risk).

The commodity delivery and listed-contract package was accepted on
2026-11-14. It covers qualified forward and listed marks, actual
commodity and invoice obligations, clearing variation, authoritative
exercise and active-source risk.
[COMMODITY-DELIVERY-LISTED](valuations-fx-and-exotics/commodity-delivery-listed.md#accepted-decision-for-commodity-delivery-and-listed-contracts).

The commodity spot-option package was accepted on 2026-11-14. It covers
dated Black pricing, checked early exercise, explicit Schwartz–Smith
simulation, outstanding cash or physical obligations and model-specific risk.
[COMMODITY-SPOT-OPTION](valuations-fx-and-exotics/commodity-spot-option.md#accepted-decision-for-commodity-spot-option-pricing-and-exercise).

The commodity averaging swap package was accepted on 2026-11-14. It
covers booked period quantities, distinct observation and payment dates,
publication-aware averaging, net cash, price-curve and discount risk.
[COMMODITY-AVERAGE-SWAP](valuations-fx-and-exotics/commodity-average-swap.md#accepted-decision-for-commodity-averaging-swap-pricing-and-settlement).

The commodity swaption package was accepted on 2026-11-14. It covers
quantity-weighted annuity and forward, checked European Black pricing,
authorized swap handoff or dated cash and distinct local and curve risk.
[COMMODITY-SWAPTION](valuations-fx-and-exotics/commodity-swaption.md#accepted-decision-for-commodity-swaption-pricing-and-exercise).

The commodity Asian package was accepted on 2026-11-14. It covers exact
fixing slots, checked geometric and arithmetic model selection, dated
cash and active forward, volatility and discount risk.
[COMMODITY-ASIAN](valuations-fx-and-exotics/commodity-asian.md#accepted-decision-for-commodity-asian-pricing-and-settlement).

The commodity spread package was accepted on 2026-11-14. It covers
normalized ordered legs, qualified ATM volatility and correlation,
checked Kirk, authorized dated cash and explicit leg and source risk.
[COMMODITY-SPREAD](valuations-fx-and-exotics/commodity-spread.md#accepted-decision-for-commodity-spread-pricing-and-settlement).

The asset Asian package was accepted on 2026-11-14. It covers exact
qualified observations, shared dated carry, the two checked analytic
routes, explicit GBM and QE-M, retained cash and model-specific risk.
[ASSET-ASIAN](valuations-fx-and-exotics/asset-asian.md#accepted-decision-for-asset-asian-pricing-and-settlement).

The asset barrier package was accepted on 2026-11-14. It covers authorized
monitoring, dated option and rebate claims, selected analytic, GBM, QE-M and
PDE routes, and state-specific risk and export.
[ASSET-BARRIER](valuations-fx-and-exotics/asset-barrier.md#accepted-decision-for-asset-barrier-pricing-and-settlement).

The asset lookback package was accepted on 2026-11-15. It covers explicit
fixed and floating payoffs, qualified extrema, checked continuous pricing,
exact-event GBM simulation, retained dated cash and state-specific risk.
[ASSET-LOOKBACK](valuations-fx-and-exotics/asset-lookback.md#accepted-decision-for-asset-lookback-pricing-and-settlement).

The asset range-accrual package was accepted on 2026-11-15. It covers
the fixed contractual band, qualified exact observations, checked static
or dated GBM pricing and quanto carry, retained coupon and active risk.
[ASSET-RANGE-ACCRUAL](valuations-fx-and-exotics/asset-range-accrual.md#accepted-decision-for-asset-range-accrual-pricing-and-settlement).

The callable rate range-accrual package was accepted on 2026-11-15.
It covers dated single-curve Hull–White projection, actual redemption
cash, earned-count continuation, independent policy pricing and rate risk.
[CALLABLE-RANGE-HW](valuations-fx-and-exotics/callable-range-hw.md#accepted-decision-for-callable-rate-range-accrual).

The snowball and inverse-floater package was accepted on 2026-11-15.
It covers applied-coupon recursion, qualified seasoned state, dated
multiplicative projection, actual cash dates, the explicit inverse-floater
approximation and rate risk.
[SNOWBALL-HW](valuations-fx-and-exotics/snowball-hw.md#accepted-decision-for-snowball-and-inverse-floater-notes).

The TARN package was accepted on 2026-11-15. It covers the Decimal
target ledger, scheduled redemption, fixed unpaid claims, dated
Hull–White pricing and rate risk. All 22 FX, commodity and exotic
valuation records are accepted.
[TARN-TARGET](valuations-fx-and-exotics/tarn-target.md#accepted-decision-for-tarn-target-and-redemption).

Structured-credit record 01 was accepted on 2026-11-15. It covers
deterministic and stochastic dispatch, authoritative collateral,
explicit assumptions and versioned profiles, contractual scheduling
and the common resolved request.
[VALUATIONS-STRUCTURED-CREDIT-01](valuations-structured-credit/01-dispatch-and-assumptions.md#accepted-decision-for-structured-credit-dispatch-and-assumptions).

Structured-credit record 02 was accepted on 2026-11-15. It covers dated
collateral normalization, the cut-off par bridge, weighted statistics,
effective obligor count and structural bands from original tranche face.
[VALUATIONS-STRUCTURED-CREDIT-02](valuations-structured-credit/02-pool-normalization-and-statistics.md#accepted-decision-for-collateral-normalization-and-pool-statistics).

Structured-credit record 03 was accepted on 2026-11-15. It covers actual
monthly exposure, dated default targets, ordered cash, cohort installments,
qualified floating coupons and recoveries dated from default.
[VALUATIONS-STRUCTURED-CREDIT-03](valuations-structured-credit/03-statistical-collateral-projection.md#accepted-decision-for-statistical-collateral-projection).

Structured-credit record 04 was accepted on 2026-11-15. It covers
dated monthly migration, distinct modification cohorts, borrower
arrears, servicer claims and authorized reimbursement.
[VALUATIONS-STRUCTURED-CREDIT-04](valuations-structured-credit/04-delinquency-and-advancing.md#accepted-decision-for-delinquency-migration-and-advancing).

Structured-credit record 05 was accepted on 2026-11-15. It covers
explicit workout dates, pending and resolved state, checked par splits,
qualified reperformance terms, carrying costs and deterministic pricing.
[VALUATIONS-STRUCTURED-CREDIT-05](valuations-structured-credit/05-npl-resolution.md#accepted-decision-for-npl-resolution-and-reperformance).

Structured-credit record 06 was accepted on 2026-11-15. It covers
contractual balloon resolution, dated workouts, appraisal and fee
bases, voluntary prepayment protection and full-pool operating DSCR.
[VALUATIONS-STRUCTURED-CREDIT-06](valuations-structured-credit/06-cmbs-servicing.md#accepted-decision-for-cmbs-balloon-and-servicing).

Structured-credit record 07 was accepted on 2026-11-15. It covers
monthly card cash and charge-offs, component-specific investor and
seller allocation, funding and accumulation, dated triggers and metrics.
[VALUATIONS-STRUCTURED-CREDIT-07](valuations-structured-credit/07-card-master-trust.md#accepted-decision-for-card-master-trust-allocation).

Structured-credit record 08 was accepted on 2026-11-15. It covers
typed collateral ledgers, dated eligibility, credit and recovery
authority, authorized draw funding and active dependencies.
[VALUATIONS-STRUCTURED-CREDIT-08](valuations-structured-credit/08-instrument-collateral.md#accepted-decision-for-instrument-collateral-and-liquidity).

Structured-credit record 09 was accepted on 2026-11-15. It covers
contractual exercise chronology and priority, actual payoff, complete
workout yield and qualified incentive scenarios.
[VALUATIONS-STRUCTURED-CREDIT-09](valuations-structured-credit/09-collateral-exercise.md#accepted-decision-for-collateral-call-and-put-exercise).

Structured-credit record 10 was accepted on 2026-11-15. It covers
purchase cash eligibility, explicit replacement cohorts, reserve uses
and earnings, accumulation release, dated hedge claims and conservation.
[VALUATIONS-STRUCTURED-CREDIT-10](valuations-structured-credit/10-reinvestment-reserves-and-hedges.md#accepted-decision-for-reinvestment-reserves-and-hedge-cash).

Structured-credit record 11 was accepted on 2026-11-15. It covers
elected waterfall priority and funding, the dated claim ledger,
cash shortfalls and PIK, capped quotas and the dated equity hurdle.
[VALUATIONS-STRUCTURED-CREDIT-11](valuations-structured-credit/11-waterfall-claims-and-rounding.md#accepted-decision-for-waterfall-claims-and-rounding).

Structured-credit record 12 was accepted on 2026-11-15. It covers
dated collateral and claim numerators and denominators, overlapping
haircuts, breach state and authorized cure diversion.
[VALUATIONS-STRUCTURED-CREDIT-12](valuations-structured-credit/12-coverage-and-cures.md#accepted-decision-for-coverage-tests-and-cure-diversion).

Structured-credit record 13 was accepted on 2026-11-15. It covers
reversible step-down, separate principal allocations, target OC,
controlled accumulation and dated, latched early amortization.
[VALUATIONS-STRUCTURED-CREDIT-13](valuations-structured-credit/13-principal-regimes-and-amortization.md#accepted-decision-for-principal-regimes-and-early-amortization).

Structured-credit record 14 was accepted on 2026-11-15. It covers
independent loss and note-balance policies, actual call funding, and
authorized terminal proceeds, releases and discharge.
[VALUATIONS-STRUCTURED-CREDIT-14](valuations-structured-credit/14-loss-recognition-and-terminal-cash.md#accepted-decision-for-loss-recognition-and-terminal-cash).

Structured-credit record 15 was accepted on 2026-11-15. It covers checked
pathwise waterfalls, explicit factors and credit granularity, stable seeded
sampling, independent-estimator uncertainty and empirical trajectory losses.
[VALUATIONS-STRUCTURED-CREDIT-15](valuations-structured-credit/15-stochastic-waterfall.md#accepted-decision-for-stochastic-waterfall-pricing-and-estimation).

Structured-credit record 16 was accepted on 2026-11-15. It covers explicit
spread and utilization laws, due-phase funding and outstanding draw-spread
attribution through matched waterfall replay.
[VALUATIONS-STRUCTURED-CREDIT-16](valuations-structured-credit/16-revolver-utilization-and-option-cost.md#accepted-decision-for-revolver-utilization-and-draw-spread-attribution).

Structured-credit record 17 was accepted on 2026-11-15. It covers
explicit OAS composition, dated rates and collateral responses,
valuation-origin buyer settlement and a checked continuous spread fit.
[VALUATIONS-STRUCTURED-CREDIT-17](valuations-structured-credit/17-oas-scenarios.md#accepted-decision-for-structured-credit-oas-and-settlement-spread-fitting).

Structured-credit record 18 was accepted on 2026-11-15. It covers
authoritative agency collateral, dated PAC ending balances and support
tiers, class cashflows, buyer settlement and fixed-strip Z-spread.
[VALUATIONS-STRUCTURED-CREDIT-18](valuations-structured-credit/18-agency-cmo-and-pac.md#accepted-decision-for-agency-cmo-waterfalls-and-dated-pac-contracts).

Structured-credit record 19 was accepted on 2026-11-15. It covers dated
eligibility and concentration, sequenced advances and cures, actual
unused-commitment fees, lender cash and independent quote compounding.
[VALUATIONS-STRUCTURED-CREDIT-19](valuations-structured-credit/19-borrowing-base-facility.md#accepted-decision-for-borrowing-base-facilities-and-lender-cashflows).

Structured-credit record 20 was accepted on 2026-11-15. It covers
buyer settlement and original-parent accrued, price and projection
authority, fixed-strip yield, continuous spread and checked inversion.
[VALUATIONS-STRUCTURED-CREDIT-20](valuations-structured-credit/20-quotes-and-inverse-pricing.md#accepted-decision-for-structured-credit-settlement-quotes-and-inversion).

Structured-credit record 21 was accepted on 2026-11-15. It covers
quote and live risk, qualified collateral perturbations, typed
breakeven CDR, deterministic scenario grids and matched stochastic reprices.
[VALUATIONS-STRUCTURED-CREDIT-21](valuations-structured-credit/21-risk-bumps-and-scenarios.md#accepted-decision-for-structured-credit-risk-and-scenarios).

Structured-credit record 22 was accepted on 2026-11-15. It covers
signed ledger and diagnostics, repayment life, equity returns and
qualified collateral ratios. All 22 detailed structured-credit
records are accepted. Runtime and host qualification remain pending.
[VALUATIONS-STRUCTURED-CREDIT-22](valuations-structured-credit/22-diagnostics-and-reporting.md#accepted-decision-for-structured-credit-diagnostics-and-reporting).

Calibration-framework record 001 was accepted on 2026-11-15. It covers
bounded typed envelopes, explicit identities and financial choices,
solver-free drafts, checked restoration and Rust-owned publication.
[CALIBRATION-FRAMEWORK-001](calibration-framework/01-wire-and-authoring.md#accepted-decision-for-strict-calibration-envelopes-and-explicit-authoring).

Calibration-framework record 002 was accepted on 2026-11-15. It covers
explicit target controls, distinct acceptance gates, exact freshness,
optional quote-side metadata and atomic initial market validation.
[CALIBRATION-FRAMEWORK-002](calibration-framework/02-controls-and-market-context.md#accepted-decision-for-calibration-controls-freshness-and-initial-market).

Calibration-framework record 003 was accepted on 2026-11-15. It covers
typed dependencies, target preflight, authored-order execution,
failure reporting and atomic installation.
[CALIBRATION-FRAMEWORK-003](calibration-framework/03-dependencies-and-execution.md#accepted-decision-for-calibration-dependencies-preflight-and-ordered-execution).

Calibration-framework record 004 was accepted on 2026-11-15. It covers
dated deposit and FRA construction, exact fixing and curve roles,
trade and projection residuals, and prepared quote replay.
[CALIBRATION-FRAMEWORK-004](calibration-framework/04-deposit-fra-materialization.md#accepted-decision-for-deposit-and-fra-materialization-and-prepared-clocks).

Calibration-framework record 005 was accepted on 2026-11-15. It covers
listed contract identity and reference windows, benchmark observations,
explicit convexity, quote-point and cash residuals, and prepared replay.
[CALIBRATION-FRAMEWORK-005](calibration-framework/05-futures-materialization.md#accepted-decision-for-listed-rate-futures-calibration-and-reference-periods).

Calibration-framework record 006 was accepted on 2026-11-15. It covers
resolved IRS leg schedules, term and overnight observations, the actual
final-payment pillar, raw residuals and prepared quote replay.
[CALIBRATION-FRAMEWORK-006](calibration-framework/06-swap-materialization.md#accepted-decision-for-term-and-overnight-swap-calibration).

Calibration-framework record 007 was fully accepted on 2026-11-15.
It covers single-name CDS dates, clean and dirty quote forms,
the hazard pillar and exact observation replay.
[CALIBRATION-FRAMEWORK-007](calibration-framework/07-cds-materialization.md#accepted-decision-for-single-name-cds-calibration-and-quote-settlement).

Calibration-framework record 008 was accepted on 2026-11-15.
It covers index tranche dates, original and outstanding notionals,
explicit upfront basis, shared raw residuals and exact provenance.
[CALIBRATION-FRAMEWORK-008](calibration-framework/08-tranche-materialization.md#accepted-decision-for-index-tranche-calibration-and-quote-settlement).

Calibration-framework record 009 was accepted on 2026-11-15.
It covers cross-currency quote orientation, sizing and valuation FX,
dated cash, foreign-curve support and exact replay.
[CALIBRATION-FRAMEWORK-009](calibration-framework/09-xccy-materialization.md#accepted-decision-for-cross-currency-swap-calibration-and-fx-authority).

Calibration-framework record 010 was accepted on 2026-11-15.
It covers exact quote and knot admission, private local checks,
complete final repricing and truthful work reporting.
[CALIBRATION-FRAMEWORK-010](calibration-framework/10-sequential-bootstrap.md#accepted-decision-for-sequential-bootstrap-and-final-repricing).

Calibration-framework record 011 was accepted on 2026-11-15.
It covers deterministic bounded scans, typed feasibility gaps,
Brent candidate evidence and independent financial acceptance.
[CALIBRATION-FRAMEWORK-011](calibration-framework/11-scalar-root-search.md#accepted-decision-for-bounded-scalar-search-and-candidate-evidence).

Calibration-framework record 012 was accepted on 2026-11-15.
It covers free solver coordinates, weighted residuals and
derivatives, and final gating before candidate ranking.
[CALIBRATION-FRAMEWORK-012](calibration-framework/12-weighted-global-solve.md#accepted-decision-for-weighted-global-solve-and-final-candidate-gates).

Calibration-framework record 013 was accepted on 2026-11-15.
It covers exact Halton proposals, ordered execution, full fit
gates, deterministic selection and all-start work.
[CALIBRATION-FRAMEWORK-013](calibration-framework/13-deterministic-multi-start.md#accepted-decision-for-deterministic-multi-start-and-final-selection).

Calibration-framework record 014 was accepted on 2026-11-15.
It covers exact fit rows, native tolerances, ratio-only plan
summaries, staged outcomes, typed work and report restoration.
[CALIBRATION-FRAMEWORK-014](calibration-framework/14-fit-reports.md#accepted-decision-for-fit-acceptance-and-report-semantics).

The complete calibration sensitivity and conditioning package was accepted
on 2026-11-15. It covers final-artifact probes, signed derivatives, typed
partial results, direct weighted-Jacobian conditioning and bounded work.
[CALIBRATION-FRAMEWORK-015](calibration-framework/15-sensitivity-diagnostics.md#accepted-decision-for-calibration-sensitivities-and-conditioning).

The complete curve validation and publication package was accepted
on 2026-11-15. It covers source and pricing support, elected economic
checks in native units, bounded coverage claims and final publication.
[CALIBRATION-FRAMEWORK-016](calibration-framework/16-curve-validation.md#accepted-decision-for-curve-validation-and-publication).

The complete surface validation and publication package was accepted
on 2026-11-15. It covers qualified calendar overlap, anchored call
constraints, native tolerances, required coverage and delivery policy.
[CALIBRATION-FRAMEWORK-017](calibration-framework/17-surface-validation.md#accepted-decision-for-surface-validation-and-publication).

The complete dated Hull–White calibration adapter was accepted
on 2026-11-15. It covers contractual swaptions and term cap strips,
native fit gates and atomic scalar or scheduled fitted parameters.
[CALIBRATION-FRAMEWORK-018](calibration-framework/18-hull-white-market-adapter.md#accepted-decision-for-dated-hull-white-quote-preparation-and-publication).

The complete rate quote replay package was accepted on 2026-11-15.
It covers source reproduction, exact quote actions, target refit and
atomic publication of the accepted evaluator and provenance.
[CALIBRATION-FRAMEWORK-019](calibration-framework/19-rate-quote-replay.md#accepted-decision-for-exact-rate-quote-replay-and-publication).

The complete linked single-curve OIS replay package was accepted
on 2026-11-15. It covers explicit shared binding, one shared target
fit, exact dated overnight projection and atomic publication.
[CALIBRATION-FRAMEWORK-020](calibration-framework/20-linked-ois-replay.md#accepted-decision-for-linked-single-curve-ois-replay-and-publication).

The complete hazard quote replay package was accepted on 2026-11-15.
It covers original-source reproduction, exact par-risk actions, original
observations for dependency and recovery replay, and the final fit gate.
[CALIBRATION-FRAMEWORK-021](calibration-framework/21-hazard-quote-replay.md#accepted-decision-for-qualified-hazard-quote-replay-and-publication).

The complete deal override and horizon replay package was accepted
on 2026-11-15. It covers exact deal bindings, model-derived horizon
quotes, a bounded dated survival gate and atomic delivery.
[CALIBRATION-FRAMEWORK-022](calibration-framework/22-hazard-deal-and-horizon.md#accepted-decision-for-deal-overrides-and-horizon-hazard-replay).

All 23 calibration framework records are accepted. The final cache
package was accepted on 2026-11-15 and covers request/state lifetime,
exact source and target identity, bounded retention and typed retry.
[CALIBRATION-FRAMEWORK-023](calibration-framework/23-recalibration-cache.md#accepted-decision-for-exact-request-local-recalibration-caching).

The calibration quote package was accepted on 2026-11-15. It covers
checked admission, exact typed identities and date roles, native units,
product domains and validated quote shifts.
[CALIBRATION-TARGETS-001](calibration-targets/01-quote-contracts.md#accepted-decision-for-exact-typed-calibration-quotes-and-native-units).

The rate preparation package was accepted on 2026-11-15. It covers
explicit clocks, prepared quote and knot identity, isolated candidate
bindings, native residuals and weights, and final all-quote repricing.
[CALIBRATION-TARGETS-002](calibration-targets/02-rate-preparation-and-residuals.md#accepted-decision-for-explicit-rate-preparation-and-native-fit-residuals).

The discount calibration package was accepted on 2026-11-15. It covers
prefix-qualified bootstrap, explicit bounded simultaneous fitting,
exact discount output binding and delivered-curve fit and validation.
[CALIBRATION-TARGETS-003](calibration-targets/03-discount-calibration.md#accepted-decision-for-explicit-discount-calibration-and-delivered-curve-fit).

The projection calibration package was accepted on 2026-11-15. It covers
exact index binding, live native residuals, independently declared knots,
bounded simultaneous fitting and delivered-source repricing and replay.
[CALIBRATION-TARGETS-004](calibration-targets/04-forward-calibration.md#accepted-decision-for-simultaneous-calibration-of-index-projection-factors).

The single-name hazard calibration package was accepted on 2026-11-15.
It covers exact live CDS terms and recovery, explicit intensity fitting,
delivered-market repricing and separate par-spread qualification.
[CALIBRATION-TARGETS-005](calibration-targets/05-hazard-calibration.md#accepted-decision-for-explicit-single-name-hazard-calibration-and-par-risk-qualification).

The CPI calibration package was accepted on 2026-11-15. It covers
independent source and contract anchors, frozen fixing availability,
bounded positive level fitting and delivered-market swap repricing.
[CALIBRATION-TARGETS-006](calibration-targets/06-inflation-calibration.md#accepted-decision-for-explicit-cpi-anchors-and-delivered-inflation-calibration).

The cross-currency calibration package was accepted on 2026-11-15. It
covers collateral-qualified quote selection, dated FX authority, bounded
foreign discount fitting and delivered-market repricing. The dated zero-rate
difference is an explicitly requested diagnostic.
[CALIBRATION-TARGETS-007](calibration-targets/07-xccy-calibration.md#accepted-decision-for-collateral-qualified-foreign-discount-calibration).

The NS/NSS calibration package was accepted on 2026-11-15. It covers
analytic pricing throughout the fit, named parameter bounds and
identification, native quote gates and analytic model delivery.
[CALIBRATION-TARGETS-008](calibration-targets/08-parametric-calibration.md#accepted-decision-for-calibration-on-the-analytic-nsnss-discount-source).

The equity smile forward package was accepted on 2026-11-15. It covers
exact European quotes, dated spot, explicit carry and funding, and dividend
entitlement at fixing with actual payment-date discounting.
[CALIBRATION-TARGETS-009](calibration-targets/09-equity-forward-inputs.md#accepted-decision-for-explicit-equity-spot-carry-and-dated-smile-forwards).

The equity surface materialization package was accepted on 2026-11-15.
It covers fixed-strike total variance, explicit fitted tails, exact
delivered-grid quote fit and scoped surface diagnostics.
[CALIBRATION-TARGETS-010](calibration-targets/10-total-variance-materialization.md#accepted-decision-for-explicit-total-variance-materialization-and-delivered-surface-fit).

The equity SABR calibration package was accepted on 2026-11-15.
It covers fixed beta, explicit Black quotes, distinct optimizer strikes,
native slice gates and complete delivered-grid acceptance.
[CALIBRATION-TARGETS-011](calibration-targets/11-equity-sabr.md#accepted-decision-for-fixed-beta-equity-sabr-and-native-slice-acceptance).

The equity SVI calibration package was accepted on 2026-11-15.
It covers the seeded raw fit, native slice gates, mandatory fitted-slice
density checks and strict acceptance on the delivered grid.
[CALIBRATION-TARGETS-012](calibration-targets/12-equity-svi.md#accepted-decision-for-raw-equity-svi-and-mandatory-fitted-slice-density).

The swaption calibration forward package was accepted on 2026-11-15.
It covers dated reference contracts, explicit expiry and tenor
coordinates, contractual observations, cash-based par rates and
construction-forward provenance.
[CALIBRATION-TARGETS-013](calibration-targets/13-swaption-forwards.md#accepted-decision-for-dated-swaption-coordinates-and-calibration-forwards).

The SABR swaption cube package was accepted on 2026-11-15.
It covers exact dated smiles, explicit fit and quote controls,
checked node construction and strict native delivered-cube replay.
[CALIBRATION-TARGETS-014](calibration-targets/14-swaption-cube.md#accepted-decision-for-exact-sabr-swaption-smiles-and-strict-cube-publication).

The base-correlation calibration package was accepted on 2026-11-15.
It covers exact dated tranche inputs, fixed attachment, identified
bounded roots, elected ordering, native quote gates and atomic
publication of the curve and rebound index.
[CALIBRATION-TARGETS-015](calibration-targets/15-base-correlation.md#accepted-decision-for-identified-base-correlation-bootstrap-and-atomic-publication).

The single-tranche Student-t calibration package was accepted on 2026-11-15.
It covers exact dated preparation, a fixed flat correlation, bounded root
and sensitivity checks, native repricing and typed scalar publication.
[CALIBRATION-TARGETS-016](calibration-targets/16-student-t.md#accepted-decision-for-identified-single-tranche-student-t-calibration).

The rate and hazard construction and replay package was accepted on
2026-11-15. It covers exact canonical construction records, native and
par-risk quote bases, explicit source qualification and atomic target
refit publication. This completes the calibration targets review.
[CALIBRATION-TARGETS-017](calibration-targets/17-calibration-replay.md#accepted-decision-for-exact-rate-and-hazard-construction-and-qualified-replay).

The Hull–White calibration entry and quote package was accepted on
2026-11-15. It covers explicit dated and numeric profiles, fit origins
and curve roles, native quote clocks, fit controls and complete fitted state.
[CALIBRATION-RATE-MODELS-001](calibration-rate-models/01-curve-entry-and-quote-domains.md#accepted-decision-for-explicit-hull-white-calibration-entry-and-quote-domains).

The Hull–White cap and floor strip package was accepted on 2026-11-15.
It covers exact live strips, benchmark and payment roles, bounded
flat Normal inversion and separate gross, time-value and native fit gates.
[CALIBRATION-RATE-MODELS-002](calibration-rate-models/02-cap-schedules-and-quote-inversion.md#accepted-decision-for-checked-cap-and-floor-strips-and-implied-normal-inversion).

The Hull–White ATM swaption target package was accepted on 2026-11-15.
It covers exact dated and synthetic preparation, cash-based par,
native quote clocks and checked analytic price-to-volatility inversion.
[CALIBRATION-RATE-MODELS-003](calibration-rate-models/03-atm-swaption-quotes.md#accepted-decision-for-exact-atm-swaption-targets-and-native-quote-reconstruction).

The synthetic Jamshidian pricing package was accepted on 2026-11-15.
It covers the exact regular payoff including its negative-rate branch,
bounded exercise solving, a root-price allowance and checked bond put
aggregation.
[CALIBRATION-RATE-MODELS-004](calibration-rate-models/04-jamshidian-decomposition.md#accepted-decision-for-checked-synthetic-jamshidian-decomposition).

The contractual Gaussian swaption package was accepted on 2026-11-15.
It covers explicit coupon laws, signed exercise-date cash terms,
weighted tails, bounded integration and missed-exercise checks.
[CALIBRATION-RATE-MODELS-005](calibration-rate-models/05-contractual-gaussian-integration.md#accepted-decision-for-checked-contractual-gaussian-swaption-integration).

The free two-parameter Hull–White fit was accepted on 2026-11-15.
It covers tolerance-scaled premiums, bounded checked least squares,
explicit deterministic starts, local identification and native
fit acceptance before candidate ranking.
[CALIBRATION-RATE-MODELS-006](calibration-rate-models/06-two-parameter-fit.md#accepted-decision-for-bounded-identified-two-parameter-hull-white-fitting).

The fixed-κ Hull–White cap and floor fit was accepted on 2026-11-15.
It covers raw unit-notional premiums, explicit bounded golden-section
minimization on a qualified branch, resolved local price response,
native Normal quote acceptance and actual fit reporting.
[CALIBRATION-RATE-MODELS-007](calibration-rate-models/07-fixed-kappa-cap-fit.md#accepted-decision-for-bounded-fixed-kappa-cap-and-floor-premium-fitting).

The piecewise Hull–White sigma bootstrap was accepted on 2026-11-15.
It covers live-fixing support, bounded per-segment branches, local
identification, final quote checks and atomic schedule publication.
[CALIBRATION-RATE-MODELS-008](calibration-rate-models/08-piecewise-sigma-bootstrap.md#accepted-decision-for-live-fixing-hullwhite-sigma-bootstrapping).

The single-slice LMM loading-scale fit was accepted on 2026-11-15.
It covers exact co-terminal target selection, explicit displacement
and shape, strict native Black lookup, checked ATM premium conversion,
returned-scale replay and explicit fit consumption.
[CALIBRATION-RATE-MODELS-009](calibration-rate-models/09-lmm-rebonato-calibration.md#accepted-decision-for-single-slice-lmm-loading-scale-calibration).

Scenario specification and composition were accepted on 2026-11-15.
The package covers checked ingress, stable immediate-block order,
explicit IDs and units, exact active targets and roll admission.
[SCENARIOS-001](scenarios/01-specification-and-composition.md#accepted-decision-for-checked-scenario-specifications-and-ordered-composition).

Ordered and atomic scenario execution was accepted on 2026-11-15.
It covers preflight, serial phases and failure order, checked statement
evaluation, atomic publication and request-local replay state.
[SCENARIOS-002](scenarios/02-execution-and-partial-success.md#accepted-decision-for-ordered-and-atomic-scenario-execution).

Scenario hierarchy expansion and overlap were accepted on 2026-11-15.
The package covers opening selection, typed precedence, execution-stage
binding and ordered retained or suppressed selection evidence.
[SCENARIOS-003](scenarios/03-hierarchy-resolution.md#accepted-decision-for-typed-scenario-hierarchy-expansion-and-overlap).

FX and equity scenario shocks were accepted on 2026-11-15. The
package covers frozen directional FX pairs, scoped cross-rate
advisories and checked scaling of exact price-qualified equity targets.
[SCENARIOS-004](scenarios/04-fx-and-equity-shocks.md#accepted-decision-for-frozen-fx-and-checked-equity-scenario-shocks).

Direct rate and CPI scenario shocks were accepted on 2026-11-15.
The package covers exact typed sources, continuous-zero factor
changes, original clocks and anchors, and CPI pillar growth changes.
[SCENARIOS-005](scenarios/05-direct-rate-shocks.md#accepted-decision-for-typed-direct-rate-and-cpi-parameter-shocks).

Scenario tenor matching and node delivery were accepted on 2026-11-15.
The package covers exact source coordinates, direct evaluated factor
shifts, explicitly identified native-pillar inverses and complete
post-action residual checks.
[SCENARIOS-006](scenarios/06-node-delivery-solve.md#accepted-decision-for-exact-scenario-coordinates-and-checked-node-delivery).

Native hazard shocks and exact CDS scenario replay were accepted on
2026-11-15. The package covers exact native or par-risk selection,
bound original and staged markets, explicit replay elections and
atomic delivery with fit evidence.
[SCENARIOS-007](scenarios/07-hazard-quote-shocks.md#accepted-decision-for-native-hazard-shocks-and-exact-cds-scenario-replay).

Commodity and volatility-index scenario shocks were accepted on
2026-11-15. The package covers explicit percent and point units,
source-owned level domains, and checked exact or evaluated delivery.
[SCENARIOS-008](scenarios/08-commodity-and-vol-index.md#accepted-decision-for-typed-commodity-and-volatility-index-scenario-shocks).

Scenario volatility grid shocks were accepted on 2026-11-15.
The package covers exact typed buckets, one checked sigma change,
separately elected diagnostics and actual effect evidence.
[SCENARIOS-009](scenarios/09-volatility-surface-shocks.md#accepted-decision-for-exact-scenario-volatility-grid-shocks).

Scenario correlation shocks were accepted on 2026-11-15.
The package covers exact detachment and model-parameter selection,
whole-candidate validation and actual effect evidence.
[SCENARIOS-010](scenarios/10-correlation-shocks.md#accepted-decision-for-checked-scenario-correlation-shocks).

Scenario instrument overrides were accepted on 2026-11-15.
The package covers exact selectors, checked mark multipliers and
settlement Z-spread shocks, frozen composite routing and atomic effects.
[SCENARIOS-011](scenarios/11-instrument-overrides.md#accepted-decision-for-checked-scenario-instrument-overrides).

Scenario statement shocks and rate bindings were accepted on
2026-11-15. The package covers existing typed inputs, dated
factor conversion and atomic publication with the evaluated result.
[SCENARIOS-012](scenarios/12-statement-bindings.md#accepted-decision-for-checked-scenario-forecast-shocks-and-rate-bindings).

Scenario time rolls were accepted on 2026-11-15. The package covers
explicit date resolution, exact fixing dependencies, retained source
conditioning and expressly elected horizon replay.
[SCENARIOS-013](scenarios/13-market-time-roll.md#accepted-decision-for-checked-scenario-time-rolls-and-horizon-replay).

Scenario roll carry was accepted on 2026-11-15. The package covers
shared endpoint pricing, phase-consistent settlement cash, native
aggregation and optional measurement coverage.
[SCENARIOS-014](scenarios/14-carry-aggregation.md#accepted-decision-for-checked-scenario-roll-carry-and-native-aggregation).

Scenario horizon attribution and returns were accepted on 2026-11-15.
The package covers one-instrument market horizons, checked attribution
and residuals, complete contributions and ACT/365F return normalization.
[SCENARIOS-015](scenarios/15-horizon-attribution-and-returns.md#accepted-decision-for-checked-scenario-horizon-attribution-and-returns).

Event-inspired stress templates were accepted on 2026-11-15.
The package covers fixed shock parameters, strict ordered
composition, exact typed bindings and run lineage.
[SCENARIOS-016](scenarios/16-historical-template-parameters.md#accepted-decision-for-checked-event-inspired-stress-templates).

Scenario application reports and replay were accepted on 2026-11-15,
completing all 17 scenario decisions. The package covers distinct
execution counts, ordered effects, state-based invalidation, scoped
identities and bounded restoration.
[SCENARIOS-017](scenarios/17-reports-and-replay.md#accepted-decision-for-checked-scenario-reports-and-replay).

Portfolio holding identity and signed sizing were accepted on
2026-11-15. The package covers explicit quantities, checked scaling,
reference validation and atomic construction and edits.
[PORTFOLIO-EXECUTION-001](portfolio-execution/01-position-sizing-and-validation.md#accepted-decision-for-checked-portfolio-holdings-and-signed-sizing).

Portfolio book hierarchy and rollups were accepted on 2026-11-15.
The package covers bounded graph admission, matched valuations,
deterministic Money reduction and unassigned holding coverage.
[PORTFOLIO-EXECUTION-002](portfolio-execution/02-book-hierarchy-and-rollups.md#accepted-decision-for-checked-book-hierarchy-and-nav-rollups).

Portfolio valuation profiles and failures were accepted on 2026-11-15.
The package covers explicit metric requests, qualified canonical PV,
atomic enrichment and typed coverage in strict and best-effort risk modes.
[PORTFOLIO-EXECUTION-003](portfolio-execution/03-valuation-profiles-and-failures.md#accepted-decision-for-checked-portfolio-valuation-profiles-and-failures).

Portfolio execution and NAV reduction were accepted on 2026-11-15.
The package covers checked request identities, logical work and error
order, checked Money NAV and qualified raw factor endpoints.
[PORTFOLIO-EXECUTION-004](portfolio-execution/04-execution-order-and-reduction.md#accepted-decision-for-checked-portfolio-execution-and-nav-reduction).

Portfolio dependencies and selective repricing were accepted on 2026-11-15.
The package covers typed active coverage, authoritative change evidence,
fresh valuation when needed and independent reporting FX refresh.
[PORTFOLIO-EXECUTION-005](portfolio-execution/05-dependencies-and-selective-reuse.md#accepted-decision-for-checked-portfolio-dependencies-and-selective-repricing).

The [ADR 0044 collateral-input representation package](conventions/0044-financial-perspectives-and-boundary-conversions.md#accepted-amendment-explicit-collateral-components)
was accepted on 2026-11-07. Its explicit VM/NICA components supplement the
accepted perspective/sign contracts; broader margin methodology retains its
own status.

The complete [ADR 0045 volatility-mapping and information-cutoff package](conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility)
was accepted on 2026-11-07. This completes adoption of all six shared financial
convention ADRs within their stated scopes. Instrument defaults, aging models
and product-specific cutoffs retain their own owners; implementation and
independent qualification remain open.

The governance policy and the scoped selections in ADRs 0032 and 0033
are accepted. Five other model-selection overviews remain Draft. Detailed
algorithms and broader policies retain their status in the owning catalogues.

| Record | Decision status | Implementation status | Validation status |
| --- | --- | --- | --- |
| [0026: Methodology inventory and decisions](governance/0026-methodology-decisions-and-inventory.md) | Accepted policy | Source catalogues established for the declared crates; exclusions explicit | Evidence mapped; no model certification |
| [0027: Vanilla equity and FX options](valuations-equity-and-composition/0027-vanilla-option-model-selection.md) | Equity and FX vanilla baselines and lifecycle accepted in the owners | Reconciliation pending | Detailed records retain independent examples and qualification status |
| [0028: Callable bond model selection](valuations-bonds/0028-callable-bond-model-selection.md) | Draft | Model resolution and option-bearing paths observed | Source/test evidence inspected; numerical checks not rerun |
| [0029: Rate-option exercise and volatility](valuations-rates/0029-rate-option-exercise-and-volatility.md) | Draft | Vanilla rate-option and Bermudan selection observed | Source/test evidence inspected; numerical checks not rerun |
| [0030: CDS survival and integration](models-and-valuations-credit/0030-cds-survival-and-integration.md) | Accepted | Single-name CDS engine selection; detailed contract in CREDIT-12 | Independent date, integral and quote checks; runtime reconciliation pending |
| [0031: Tranche loss model selection](models-and-valuations-credit/0031-tranche-loss-model-selection.md) | Draft | Gaussian default and explicit copula paths observed | Source/test evidence inspected; numerical checks not rerun |
| [0032: Curve interpolation and calibration](calibration-targets/0032-curve-interpolation-and-calibration.md#accepted-decision-for-explicit-rate-curve-interpolation-and-calibration) | Accepted | Explicit factor interpolation, tails, fit selection and final artifact admission | Independent factor, tail and PCHIP checks; production qualification pending |
| [0033: Return-series VaR and ES](analytics/0033-value-at-risk-method-selection.md) | Accepted fixed historical selection | Historical scalar/summary dispatch; explicit inputs and invalid-CF errors reconciled | [Scoped implementation evidence](analytics/README.md#implementation-evidence) |

Use [ADR 0026's independent statuses](governance/0026-methodology-decisions-and-inventory.md#independent-statuses)
to distinguish agreement, observed implementation and validation evidence.
Accepted contracts and prior agreed directions retain their stated scope.
The [15 accepted analytics records](analytics/README.md#accepted-scopes-in-detailed-records)
define the implemented analytics methods; acceptance does not adopt unrelated
Draft algorithms or selection choices.

## Boundary and evidence

The [audit boundary](AUDIT_BOUNDARY.md) defines source inclusion, excluded
surfaces and the limits of inspected evidence. The catalogue records observed
algorithms, controls, branch differences and discrepancies. Remaining
implementation and qualification work is scoped in its owning records.
Numerical suites, simulations and vendor comparisons were not rerun in the
original documentation audit; subsequent analytics implementation checks are
recorded in its [inventory evidence](analytics/README.md#implementation-evidence).

The [algorithm backlog](FOLLOW_UPS.md) owns detailed reconciliation, investigation,
decision and evidence items. The [central follow-ups](../FOLLOW_UPS.md#financial-methodology-audit)
own broader work. Each action has one authoritative register entry linked back
to its record; records and inventories link to those entries or their scoped
sections instead of repeating task lists. Records retain formulas, observed
behavior, material limitations, evidence qualifications, and concise adoption
uncertainties. Neither documentation coverage nor an open item authorizes a
runtime change.

## Checking and maintaining coverage

The per-scope `coverage.json` files own classification and record mapping.
For a material methodology change, update the calculation row, owning records
and ledger alongside code, tests and references. State the intended choice,
effective caller behavior and independent implementation/validation statuses;
link unresolved work to the registers above. Source hashes describe the
audited snapshot; updating a hash alone is not an audit.

```bash
uv run --no-sync python scripts/check_methodology_coverage.py --verify-source
```

This checks exact current source enumeration, unique ownership, record
references and unchanged inspected Rust content. To regenerate the readable
map after reviewing ledger changes:

```bash
uv run --no-sync python scripts/check_methodology_coverage.py --write
```

The checker verifies documentation coverage. It does not approve methods or
certify numerical correctness. [ADR 0026](governance/0026-methodology-decisions-and-inventory.md)
defines adoption and evidence requirements.
