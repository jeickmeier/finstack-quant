# Financial methodologies

A reference explains a method. A decision records why we use it, where it
applies, and what behavior callers can rely on.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

The catalogue maps **1,719 Rust files** to **625 detailed Draft methodology
records across 34 source scopes**.

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
| [Valuations: shared framework and risk](valuations-framework/README.md) | Market/convention resolution, lifecycle, projection, listed products, model controls, risk bumps/recalibration, Greeks, historical risk and reporting | 128 | 20 | [Coverage](valuations-framework/coverage.json) |
| [Valuations: rates](valuations-rates/README.md) | Deposits/FRAs, swaps/basis/XCCY, futures, inflation, repo, CMS, vanilla/early-exercise rate options and their metrics | 101 | 17 | [Coverage](valuations-rates/coverage.json) |
| [Valuations: bonds](valuations-bonds/README.md) | Settlement/quotes, discount/hazard/tree/joint/Merton pricing, yield/spread/OAS inverses, exercise/return floors and bond risk/returns | 71 | 20 | [Coverage](valuations-bonds/coverage.json) |
| [Valuations: other fixed income](valuations-other-fixed-income/README.md) | Loans/revolvers, convertibles, inflation bonds, MBS/TBA/dollar rolls, futures and fixed-income TRS | 87 | 16 | [Coverage](valuations-other-fixed-income/coverage.json) |
| [Valuations: equity, property and composition](valuations-equity-and-composition/README.md) | Equity/options/path products, DCF/private equity, real estate, baskets, composite sizing/history and selected-model risk | 97 | 24 | [Coverage](valuations-equity-and-composition/coverage.json) |
| [Valuations: FX, commodity and exotic products](valuations-fx-and-exotics/README.md) | FX linear/option/barrier/touch/variance contracts; commodity curves/averages/options; Asian/barrier/lookback/range/snowball/TARN calculations | 113 | 22 | [Coverage](valuations-fx-and-exotics/coverage.json) |
| [Valuations: structured credit](valuations-structured-credit/README.md) | Statistical/instrument collateral, waterfall claims/triggers, servicing/recovery/reinvestment, stochastic pools/OAS, CMO and facilities | 100 | 22 | [Coverage](valuations-structured-credit/coverage.json) |
| [Calibration: orchestration, solvers and replay](calibration-framework/README.md) | Plan validation and dependencies, quote/build materialization, bootstrap/global solvers, acceptance/reporting, quote risk and recalibration | 50 | 23 | [Coverage](calibration-framework/coverage.json) |
| [Calibration: quotes and curve/smile targets](calibration-targets/README.md) | Discount/forward/hazard/CPI/XCCY curves, parametric term structures, SABR/SVI/swaption surfaces, base correlation and tail calibration | 25 | 17 | [Coverage](calibration-targets/coverage.json) |
| [Calibration: Hull–White and LMM](calibration-rate-models/README.md) | Contractual/synthetic option targets, scalar/piecewise Hull–White fits, objectives and schedules, LMM covariance and parameter fits | 12 | 9 | [Coverage](calibration-rate-models/coverage.json) |
| [Analytics](analytics/README.md) | Returns and windows, wealth/ratios, drawdowns and tail risk, rolling statistics, benchmarks/capture, regression and multifactor estimation | 21 | 15 | [Coverage](analytics/coverage.json) |
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

The classifications are **1377 algorithm-bearing**, **285 support**,
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
| [0045](conventions/0045-financial-clocks-and-date-boundaries.md) | Preserve separately owned clocks, full volatility date-to-expiry mapping, contractual forward annualization, bounded discount-factor bridges and explicit fixing cutoffs. Metadata, same-day fixing reconciliation and implementation verification remain open. |

The [invariant-to-decision map](../INVARIANT_MAP.md) records the detailed
cross-crate conventions covered by these decisions. Acceptance does not adopt
drafts 0027–0033 or every observed instrument default.

## Methodology records

The governance policy is accepted; the seven model-selection overviews
below remain Draft. Their detailed algorithms are in the owning catalogues.

| Record | Decision status | Implementation status | Validation status |
| --- | --- | --- | --- |
| [0026: Methodology inventory and decisions](governance/0026-methodology-decisions-and-inventory.md) | Accepted policy | Source catalogues established for the declared crates; exclusions explicit | Evidence mapped; no model certification |
| [0027: Vanilla equity and FX options](valuations-equity-and-composition/0027-vanilla-option-model-selection.md) | Draft | Entry points and registered alternatives observed | Source/test evidence inspected; numerical checks not rerun |
| [0028: Callable bond model selection](valuations-bonds/0028-callable-bond-model-selection.md) | Draft | Model resolution and option-bearing paths observed | Source/test evidence inspected; numerical checks not rerun |
| [0029: Rate-option exercise and volatility](valuations-rates/0029-rate-option-exercise-and-volatility.md) | Draft | Vanilla rate-option and Bermudan selection observed | Source/test evidence inspected; numerical checks not rerun |
| [0030: CDS survival and integration](models-and-valuations-credit/0030-cds-survival-and-integration.md) | Draft | CDS convention and integration paths observed | Source/test evidence inspected; numerical checks not rerun |
| [0031: Tranche loss model selection](models-and-valuations-credit/0031-tranche-loss-model-selection.md) | Draft | Gaussian default and explicit copula paths observed | Source/test evidence inspected; numerical checks not rerun |
| [0032: Curve interpolation and calibration](calibration-targets/0032-curve-interpolation-and-calibration.md) | Draft | Raw curve and calibration defaults observed | Source/test evidence inspected; numerical checks not rerun |
| [0033: Return-series VaR and ES](analytics/0033-value-at-risk-method-selection.md) | Draft; preserves prior explicit-error direction | Scalar/summary selection and host defaults observed; CF fallback conflicts with prior direction | Source/test evidence inspected; numerical checks not rerun |

Use [ADR 0026's independent statuses](governance/0026-methodology-decisions-and-inventory.md#independent-statuses)
to distinguish agreement, observed implementation and validation evidence.
Accepted contracts and prior agreed directions retain their scope; they do
not adopt the Draft algorithms or selection choices listed here.

## Boundary and evidence

The [audit boundary](AUDIT_BOUNDARY.md) defines source inclusion, excluded
surfaces and the limits of inspected evidence. The catalogue records observed
algorithms, controls, branch differences and discrepancies; implementation
reconciliation and independent numerical qualification remain open. Numerical
suites, simulations and vendor comparisons were not rerun.

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
