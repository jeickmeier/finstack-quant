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
| [Core: synthetic day counts](core-conventions/003-thirty360.md#accepted-decision-synthetic-day-count-conventions) | Complete five-variant 30/360 selection/formulas, explicit termination status, canonical interval API, signed/zero and numerical/error package; implementation and independent qualification remain open. |
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
| [Core: Rate and Bps units/admission](core-conventions/018-rate-units.md#accepted-decision-rate-and-bps-units-admission-and-precision) | Complete finite model Rate/exact whole-i32 Bps, checked fractional unit conversion, single-final text admission, quantizer removal, exact integer division, truthful display and host/wire/product-boundary package. Runtime/product/host reconciliation and independent qualification remain open. |
| [Core: generic ratings and explicit scorecard scales](core-conventions/019-credit-ratings.md#accepted-decision-generic-ratings-and-explicit-scorecard-scales) | Complete NR-outside-quality/strict-symbol/default-marker package, fallible Moody rendering, removal of numeric defaults/fallbacks and explicit immutable custom scale snapshots with complete bands, self-membership, strict references and validated state. Runtime/consumer/host reconciliation and independent qualification remain open. |
| [Core: barrier classification and conditional payment timing](core-conventions/020-barrier-classification.md#accepted-decision-barrier-classification-touch-and-conditional-payment-timing) | Complete explicit relevant choices/default removal, exact inclusive eligible-observation touch, direction/history/conditional entitlement/settlement separation and strict wire/host/consumer-boundary package. Runtime/product/model/host reconciliation and independent qualification remain open. |
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
economic phase order. The other 12 cashflow records remain Draft. Runtime/caller/
host reconciliation and independent qualification remain pending.
Runtime/host/caller reconciliation and independent qualification remain open.

The [ADR 0044 collateral-input representation package](conventions/0044-financial-perspectives-and-boundary-conversions.md#accepted-amendment-explicit-collateral-components)
was accepted on 2026-11-07. Its explicit VM/NICA components supplement the
accepted perspective/sign contracts; broader margin methodology retains its
own status.

The complete [ADR 0045 volatility-mapping and information-cutoff package](conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility)
was accepted on 2026-11-07. This completes adoption of all six shared financial
convention ADRs within their stated scopes. Instrument defaults, aging models
and product-specific cutoffs retain their own owners; implementation and
independent qualification remain open.

The governance policy and ADR 0033's scoped historical tail selection are
accepted. The six other model-selection overviews remain Draft. Detailed
algorithms and broader policies retain their status in the owning catalogues.

| Record | Decision status | Implementation status | Validation status |
| --- | --- | --- | --- |
| [0026: Methodology inventory and decisions](governance/0026-methodology-decisions-and-inventory.md) | Accepted policy | Source catalogues established for the declared crates; exclusions explicit | Evidence mapped; no model certification |
| [0027: Vanilla equity and FX options](valuations-equity-and-composition/0027-vanilla-option-model-selection.md) | Draft | Entry points and registered alternatives observed | Source/test evidence inspected; numerical checks not rerun |
| [0028: Callable bond model selection](valuations-bonds/0028-callable-bond-model-selection.md) | Draft | Model resolution and option-bearing paths observed | Source/test evidence inspected; numerical checks not rerun |
| [0029: Rate-option exercise and volatility](valuations-rates/0029-rate-option-exercise-and-volatility.md) | Draft | Vanilla rate-option and Bermudan selection observed | Source/test evidence inspected; numerical checks not rerun |
| [0030: CDS survival and integration](models-and-valuations-credit/0030-cds-survival-and-integration.md) | Draft | CDS convention and integration paths observed | Source/test evidence inspected; numerical checks not rerun |
| [0031: Tranche loss model selection](models-and-valuations-credit/0031-tranche-loss-model-selection.md) | Draft | Gaussian default and explicit copula paths observed | Source/test evidence inspected; numerical checks not rerun |
| [0032: Curve interpolation and calibration](calibration-targets/0032-curve-interpolation-and-calibration.md) | Draft | Raw curve and calibration defaults observed | Source/test evidence inspected; numerical checks not rerun |
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
