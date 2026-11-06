# Calibration target methodology audit

- **Decision status:** Draft — observed implementation; adoption pending.
- **Implementation status:** Observed for all assigned source files; scope and evidence limits are described in the records; open work is tracked in the register.
- **Validation status:** Source and selected evidence inspected/located; no tests or builds executed.
- **Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

This scope covers **25 assigned Rust source files**: **21 algorithm-bearing files, 2 support/module files and 2 dedicated test files**, accounted exactly once with source hashes in [coverage.json](coverage.json). The **17 records** document quote contracts, target parameterization, residuals/weights, numerical search controls, carry/clock resolution, volatility materialization, inverse solves and replay. The **45 inventory rows** distinguish meaningful algorithms and dispatch choices rather than only product families. Runtime after the early test-only import in swaption.rs was inspected; no production code was excluded at that marker.

[Calibration framework](../calibration-framework/README.md) owns envelope/preflight, quote instrument builders, shared bootstrap/global/scalar solvers, reports, validators and recalibration. [Core market](../core-market/README.md) owns curve representation; [model analytic methods](../models-analytic/README.md) own SABR/SVI evaluation/fitting kernels. This scope records the actual target selections and transformations at those boundaries. Existing tests are evidence descriptions only: same-kernel roundtrips, algebraic checks, invariants and externally attributed fixtures are identified separately. No runtime/source/test/fixture edits, model adoption, numerical experiments or external standard certification occurred.

## Governing and overview ADRs

These ADRs retain their stated scope and decision status. They are separate
from the detailed algorithm records and source counts below.

| ADR | Decision status | Scope |
| --- | --- | --- |
| [0032](0032-curve-interpolation-and-calibration.md) | Draft | Rate-curve interpolation during calibration |

The [core-market catalogue](../core-market/README.md) owns curve representation
and evaluation; ADR 0032 links those dependencies to calibration selection.

## Algorithm inventory

Every validation entry below is **not executed**. Each linked record specifies exact sources, controls, conventions, alternatives and evidence limits. The [follow-up register](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets) owns open work.

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation |
|---|---|---|---|---|
| Quote identity and selection | Wire ID check; tenor-first pillar parse; class filtering | MarketQuote validation; constructors can bypass ID check | [CALIBRATION-TARGETS-001](01-quote-contracts.md) | Quote schemas and rejection tests located |
| Rate/futures risk quotes | Decimal rates; 100-price conversion minus convexity; bp shifts | Negative rates allowed; bump helpers do not revalidate | [CALIBRATION-TARGETS-001](01-quote-contracts.md) | RateQuote source inspected |
| CDS and tranche quote domains | Par bp; standard running coupons; upfront fractions | CDS upfront finite; tranche upfront bounded to ±1 | [CALIBRATION-TARGETS-001](01-quote-contracts.md) | Hazard rejection tests located |
| Inflation/vol/XCCY quotes | Finite inflation/basis; typed vol; positive optional FX | Target-specific stricter domains follow quote validation | [CALIBRATION-TARGETS-001](01-quote-contracts.md) | Quote validators inspected |
| Prepared rate instruments and clocks | Convention-built instruments; payment-aware pillars | Explicit clock overrides; otherwise common index day count | [CALIBRATION-TARGETS-002](02-rate-preparation-and-residuals.md) | Prepared builder and utility inspected |
| Global knot grids | Stable aligned sort; duplicate time tolerance 1e-10 | Forward target has its own origin-capable grid | [CALIBRATION-TARGETS-002](02-rate-preparation-and-residuals.md) | Source inspected |
| Objective normalization and weights | PV/N; annuity-proxy inverse square × scheme factor | 1e-3 weight floor; separate hazard/inflation schemes | [CALIBRATION-TARGETS-002](02-rate-preparation-and-residuals.md) | Scaling/day-count tests inspected/located |
| Discount bootstrap | Maturity-dependent DF bounds; mixed log/linear scans; PV/N root | Bootstrap default; nearest-first; 48/20 scan controls | [CALIBRATION-TARGETS-003](03-discount-calibration.md) | Inline and integration repricing evidence located |
| Global discount fit | Clamped zeros → exp(-zt) → hard DF bounds | Optional bootstrap seed; better passing seed may win | [CALIBRATION-TARGETS-003](03-discount-calibration.md) | Global sanity tests located |
| Discount Jacobian and nonlocal polish | Central FD; local-causality zero shortcut | Analytical flag selects custom numerical Jacobian; nonlocal Bootstrap polishes globally | [CALIBRATION-TARGETS-003](03-discount-calibration.md) | Source and interpolation tests located |
| Discount curve ID binding | Trial pricing alias versus final output ID | Same ID by default; alternate pricing ID is allowed | [CALIBRATION-TARGETS-003](03-discount-calibration.md) | Static source/caller tracing |
| Forward projection grid | All contractual reset boundaries; sparse quote control rates | GlobalSolve only; curve-clock intervals | [CALIBRATION-TARGETS-004](04-forward-calibration.md) | Deposit 365/360 formula inspected; contract tests located |
| Forward parameter collision resolution | Deposit sequencing; swap last-reset/maturity fallback | Duplicate unresolved control times error | [CALIBRATION-TARGETS-004](04-forward-calibration.md) | Grid source and multiple-deposit tests located |
| Forward residuals/bounds | Deposit accrual-growth error; other PV/N; annuity weights | Effective rate bounds; constant last-forward tail | [CALIBRATION-TARGETS-004](04-forward-calibration.md) | FRA/future/swap repricing tests located |
| Hazard instrument fit | Fixed-recovery piecewise constant hazards; full CDS PV/N | Bootstrap or GlobalSolve; convention compatibility | [CALIBRATION-TARGETS-005](05-hazard-calibration.md) | CDS repricing and replay tests inspected/located |
| Hazard seed and search | Spread/LGD seed; 48-point log window + 16 upper-tail points | Configured [0,10] default; exhaustive bracketing | [CALIBRATION-TARGETS-005](05-hazard-calibration.md) | Distressed scan tests located |
| Hazard global weights | Direct hazards; previous-curve seed; time scheme only | Positive finite active maturities; 1e-3 weight floor | [CALIBRATION-TARGETS-005](05-hazard-calibration.md) | Global smoothness test located |
| Inflation quote exposure and clock | Lagged reference origin; final required monthly anchor | Month/no lag only; observed index consistency preflight | [CALIBRATION-TARGETS-006](06-inflation-calibration.md) | Reference-origin and leap/month tests located |
| CPI bootstrap/global fit | Direct CPI levels; growth seeds; full swap PV/N | Hard levels [1,10000]; seed-only -10%/+50% growth | [CALIBRATION-TARGETS-006](06-inflation-calibration.md) | ZC repricing flow inspected; YoY tests located |
| Foreign curve from XCCY/rates | Merged full-contract PV/N sequential bootstrap | Rates or XCCY or both; Bootstrap only; per-quote spot precedence | [CALIBRATION-TARGETS-007](07-xccy-calibration.md) | XCCY acceptance/clock tests located |
| Derived XCCY zero spread | Relative date DFs → zforeign−zdomestic; Linear curve | Optional output; origin zero; common foreign clock | [CALIBRATION-TARGETS-007](07-xccy-calibration.md) | Signed and base/clock invariance tests located |
| NS/NSS parameter fit | Four/six analytic parameters; PV/N global objective | Default seed; beta ±2; tau [.01,30]; equal weights | [CALIBRATION-TARGETS-008](08-parametric-calibration.md) | Analytic delivered-curve integration tests located |
| Parametric date adapter | Daily exact-date DF samples; LogLinear temporary curve | All dates to last nondeposit pillar; deposit endpoints | [CALIBRATION-TARGETS-008](08-parametric-calibration.md) | Delayed-OIS/short-deposit tests located |
| NSS decay repair | Clamp taus; move near-coincident pair by .5 years | Minimum separation .01; shared numerical Jacobian | [CALIBRATION-TARGETS-008](08-parametric-calibration.md) | Source inspected |
| Equity carry selection | Yield override → yield scalar → unique cash schedule | Missing carry errors; option underlying exact filter | [CALIBRATION-TARGETS-009](09-equity-forward-inputs.md) | Hand-computed carry/clock tests inspected |
| Equity forward calculation | (Spot−PV cash)/relative DF or Sexp(−qT)/DF | Ex-date inclusion; positive forward; SVI optional zero discount rate | [CALIBRATION-TARGETS-009](09-equity-forward-inputs.md) | ACT360/365 and shifted-base formula tests inspected |
| Equity expiry/strike grid fill | Linear total variance at fixed absolute strike; each slice own forward | SABR Error/Clamp; SVI hardcoded Clamp | [CALIBRATION-TARGETS-010](10-total-variance-materialization.md) | Flat-variance formula inspected; interpolation tests located |
| Published equity surface acceptance | Re-evaluate original quotes through materialized grid | Residual in decimal vol; total-variance surface mode | [CALIBRATION-TARGETS-010](10-total-variance-materialization.md) | Published-grid tests located |
| Equity SABR slices | Fixed beta; >=3 quotes per live expiry; shared model fit | Auto shift; beta>=1e-4; absolute/min-quote relative tolerance | [CALIBRATION-TARGETS-011](11-equity-sabr.md) | Slice-failure and model-generated tests located |
| SABR outcome/provenance | Retain parameters, winning starts, evaluations and bounds | Any live slice failure aborts; final artifact quote budget | [CALIBRATION-TARGETS-011](11-equity-sabr.md) | Report construction inspected |
| SVI slice fitting | Raw five-parameter SVI via direct LM | >=5 quote entries; positive vols; numerical config tol/iterations | [CALIBRATION-TARGETS-012](12-equity-svi.md) | Model-fit/density and error tests located |
| SVI density diagnostic | 201-point Durrleman scan across target wings ±1 log unit | Only if check_arbitrage; threshold -1e-10; worst per slice | [CALIBRATION-TARGETS-012](12-equity-svi.md) | Arbitrage-report tests located |
| Swaption quote coordinates | ACT365F option time; settlement-adjusted contractual tenor | Quote conventions; fixed DC/calendar overrides | [CALIBRATION-TARGETS-013](13-swaption-forwards.md) | Settlement/clock tests located |
| Calibration swap forward/annuity | PV projected growth / fixed coupon annuity; payment-date discount | Projection curve optional; period DF ratios | [CALIBRATION-TARGETS-013](13-swaption-forwards.md) | Telescoping and multicurve algebra tests inspected |
| Interpolated node forward | Index-default schedules; rounded-month expiry/tenor | swap_index required when not direct calibrated key | [CALIBRATION-TARGETS-013](13-swaption-forwards.md) | Date-range tests located |
| Swaption SABR bucket fit | Normal beta0+ATM pin; Black no shift; shifted fixed shift | Rounded 1e-4-year bucket keys; >=3 strikes; .0015 default budget | [CALIBRATION-TARGETS-014](14-swaption-cube.md) | Quote-unit/shift/ATM tests located |
| Swaption parameter interpolation | Log alpha/nu; linear clipped rho; inherit beta/shift | Error default; optional clamp and deterministic missing corners | [CALIBRATION-TARGETS-014](14-swaption-cube.md) | Bilinear bounds test inspected |
| Published cube acceptance | Re-evaluate all quote vols via delivered clamped cube | Any failed bucket forces failed report | [CALIBRATION-TARGETS-014](14-swaption-cube.md) | Discarded-smile and failed-bucket tests located |
| Base-correlation quote binding | Exact index/series/maturity; decimal detachments → percent axis | One maturity; optional complete expected detachment set | [CALIBRATION-TARGETS-015](15-base-correlation.md) | Boundary/IMM/index-refresh tests located |
| Monotone correlation bootstrap | Previous-correlation lower bound; bounded scan; full tranche PV/Ntranche | Nearest-first; .001 discounted-upfront tolerance; approximate knots allowed | [CALIBRATION-TARGETS-015](15-base-correlation.md) | Thin-tranche settlement repricing tests located |
| Correlation final construction | Synthetic flat second knot for one quote; final nondecreasing shape | Return curve plus rebound index; <index>_CORR ID | [CALIBRATION-TARGETS-015](15-base-correlation.md) | Synthetic/final-shape tests located |
| Student-t calibration contract | One selected tranche; fixed flat correlation; unit tranche notional | Discount-curve base is as-of; unique/explicit discount ID | [CALIBRATION-TARGETS-016](16-student-t.md) | Settlement upfront roundtrip inspected |
| Student-t inverse solve | Bounded scalar df root; initial acceptance; nearest-first scan | df>2; default [2.1,50]; hardcoded .001 upfront budget | [CALIBRATION-TARGETS-016](16-student-t.md) | Narrow-bounds and returned-fit tests inspected/located |
| Rate calibration sidecar | Typed quote/role/clock/compounding conversion | No original QuoteIds or numeric config; generated replay IDs | [CALIBRATION-TARGETS-017](17-calibration-replay.md) | Mixed-shape and absolute-pillar tests located |
| Hazard calibration/risk replay | Original JSON and model-implied par-risk quotes; exact rebuilt pillars | Params/config retained; 1e-12 relative time binding tolerance | [CALIBRATION-TARGETS-017](17-calibration-replay.md) | ACT365F/tampering and upfront/par tests located |

## Evidence and follow-up tracking

Source inspection does not establish market suitability or independent numerical
qualification. Model-family references identify theory; they do not approve the
repository-specific controls. No numerical effects were reproduced during this
documentation audit.

Adoption questions, observed discrepancies and missing qualification evidence are
owned by the [calibration-target follow-ups](../FOLLOW_UPS.md#calibration-quotes-and-curvesmile-targets).
