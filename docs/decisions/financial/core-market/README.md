# Core market-data methodology audit

Decision status: **Draft — implementation observed, adoption pending.** Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

The exact assigned scope is **60 Rust source files** under `finstack-quant/core/src/market_data`: **52 algorithm-bearing files, 6 support/adapter files, and 2 dedicated test files**. Every assigned file was inspected and classified once in [coverage.json](coverage.json). The **22 records** below describe numerical kernels, input validation, data resolution, transformations, approximations and failures at algorithm level. Default model names and stored metadata are not treated as approval. No runtime code, tests or fixtures were changed.

Validation is **source and existing-test inspection/location, not test execution**. Inspected hand-computed formulas are distinguished from comparisons to the production kernel and from serialization/invariant tests. No build, test suite, vendor reconciliation or new empirical accuracy experiment was run. An implementation domain or validation gap remains explicit even when existing regression tests cover ordinary inputs.

The shared interpolation kernels are owned by the [core numerics value-interpolation](../core-numerics/value-interpolation.md) and [forward-interpolation](../core-numerics/forward-interpolation.md) records. This slice records each curve's actual selection, variable being interpolated, domain, extrapolation and caller behavior. Core surface/cube/FX quote files store and validate data; [model volatility interpolation](../models-analytic/volatility-source-interpolation.md), [FX delta smiles](../models-analytic/fx-delta-smiles.md), and [SABR expansions/calibration](../models-analytic/sabr-expansions.md) own their numerical evaluators. Money/FX matrix resolution is outside this manifest; context FX delegation and units are covered here. Calibration recipes are inspected as recipe-validation/dispatch contracts; no inverse calibration solver exists in those two core files.

## Governing and overview ADRs

These ADRs retain their stated scope and decision status. They are separate
from the detailed algorithm records and source counts below.

| ADR | Decision status | Scope |
| --- | --- | --- |
| [0032](../calibration-targets/0032-curve-interpolation-and-calibration.md) | Draft | Rate-curve interpolation during calibration |

## Inventory

All evidence in the fifth column is **not executed**. Each linked record gives the exact owning source/test paths, method references, domain, alternatives and limitations.

| Calculation / supported instruments | Implemented method / material variants | Actual selection | Decision record | Implementation / validation |
|---|---|---|---|---|
| Discount-curve construction | Positive DF pillars; exact origin; sorting; pillar monotonicity/forward floor | MonotoneConvex + FlatForward, Act/365F; named validation modes | [CORE-MARKET-01](discount-evaluation.md) | Builder/validation; negative-rate and domain regressions located |
| DF/zero/interval-forward queries | Selected interpolation; inverse compounding; log DF ratio | Zero rate at origin is 0; minimum forward interval 1e-6 years | [CORE-MARKET-01](discount-evaluation.md) | Curve API; elementary log-ratio formula inspected |
| Relative/date discounting; short forward | DF ratio; finite difference of log DF with bounded epsilon | Trait default; date clock/calendar belongs to curve | [CORE-MARKET-01](discount-evaluation.md) | Traits; finite-difference and relative-DF tests located |
| Term rate projection | Left-endpoint simple-compounding log-DF chain; expm1 forward | Explicit contractual reset grid or numeric tenor | [CORE-MARKET-02](forward-projection.md) | Forward curve; manual accrual products inspected |
| Arithmetic average forward | Composite Simpson 8/16/32 panels; exact shock area | Panel count chosen by interval length; point rule <=1e-12 | [CORE-MARKET-02](forward-projection.md) | Forward tests distinguish point/average/DF-implied rates |
| Forward conventions/date queries | ID-substring day count/reset lag; contractual accrual conversion | Explicit fields override inference; historical term starts error | [CORE-MARKET-02](forward-projection.md) | Convention source; cross-clock formula tests inspected |
| Discount-to-forward conversion | Simple tenor rate sampled at discount knots | Linear new curve unless overridden | [CORE-MARKET-02](forward-projection.md) | Discount conversion tests located |
| Risk bump units and buckets | bp/percent/fraction/factor dispatch; half/full triangles | Parallel default; finite ordered bucket coordinates | [CORE-MARKET-03](rate-shocks.md) | Bump helpers; partition tests located |
| Discount zero-rate shocks | Original interpolator times exponential of integrated shock | Additive only; relax monotonicity/floor after stress | [CORE-MARKET-03](rate-shocks.md) | Off-pillar exponential and bucket formula tests inspected |
| Forward shocks | Continuous additive piecewise-linear shift; parallel scaling | Multiplicative buckets rejected; exact adjustment integration | [CORE-MARKET-03](rate-shocks.md) | Sparse-bucket exact area inspected |
| Rate-curve roll/composition | Source-clock offset; DF normalization; serialized transform | Original interpolator retained; replacement knots reset transform | [CORE-MARKET-03](rate-shocks.md) | Multi-style roll/serde/shock invariants located |
| Hazard/survival construction | Ending-knot constant hazards; log-linear survival; constant tail | Nonnegative hazards; explicit recovery; default cap 10/year | [CORE-MARKET-04](hazard-survival.md) | Survival derivative and boundary formulas inspected |
| Default probabilities | Unconditional survival subtraction; conditional overlap integral with expm1 | Date conditional path floors pre-base times at zero | [CORE-MARKET-04](hazard-survival.md) | Tiny conditional PD and underflow formulas inspected |
| Hazard rolling | Shift future ending knots; rebuild conditional survival | Expired knots/quotes dropped; recipe invalidated | [CORE-MARKET-04](hazard-survival.md) | Conditional-survival roll invariant inspected |
| Indicative CDS quote lookup | Linear/log-linear quoted spread or local lambda*(1-R) | Insufficient quotes use triangle; failed log interpolation uses linear | [CORE-MARKET-05](hazard-quotes-and-risk.md) | Quote and 60-to-70 bp fallback tests inspected |
| Spread versus direct-hazard risk | Spread/(1-R) hazard shift versus direct hazard bp | Generic context path selects spread approximation | [CORE-MARKET-05](hazard-quotes-and-risk.md) | Shift/additivity/atomic-failure tests inspected |
| Tenor hazard risk | Containing ending-segment selection with 1e-6 match tolerance | Beyond-tail ignored; one-node curve sums all targets | [CORE-MARKET-05](hazard-quotes-and-risk.md) | Segment-target and repeated-bump tests inspected |
| Rate calibration provenance | Typed quote/role/OIS/method recipe; no solver here | Bootstrap or global solve metadata; retained after transforms | [CORE-MARKET-06](calibration-provenance.md) | Recipe cutoff serde test inspected |
| Hazard replay provenance | Sort original/risk inputs; pair IDs; strict time/type checks | Risk inputs must be par spread; quote payload remains JSON | [CORE-MARKET-06](calibration-provenance.md) | Recipe validators; transform invalidation inspected |
| NS/NSS discount curves | Analytic continuous zero and forward basis functions | Explicit model; positive tau; distinct NSS taus | [CORE-MARKET-07](parametric-discount.md) | Model formula/roll tests located; canonical references linked |
| Parametric roll/short-time behavior | Source DF ratio with time offset; nonpositive-time branches | No generic context bump; analytic method separate from trait | [CORE-MARKET-07](parametric-discount.md) | Parametric serde/date/roll invariants located |
| Price and vol-index term levels | Interpolate native levels; explicit/derived spot anchor | Linear/FlatZero default; Price signed, VolIndex nonnegative knots | [CORE-MARKET-08](price-curves.md) | Price domain and scalar-curve invariants located |
| Price shocks | Absolute native shifts or relative percentage scaling | Key-rate leaves spot unchanged; RateBp rejected | [CORE-MARKET-08](price-curves.md) | Signed/spot/bucket regressions located |
| Price roll and shocked roll state | Shift source evaluation; new spot; origin reinsertion if needed | Negative cumulative offset rebuilds from available pillars | [CORE-MARKET-08](price-curves.md) | Retained-source and backward-roll regressions located |
| Basis spreads | Signed level interpolation; retained-source rolling | Linear/FlatZero; context bump unsupported | [CORE-MARKET-09](basis-spreads.md) | Basis unit tests and heterogeneous lifecycle tests located |
| Projected CPI/index ratio | Log-linear positive CPI; months/12 lag; lagged/base ratio | 100 base CPI; three-month lag; flat-level tail | [CORE-MARKET-10](inflation-projection.md) | CAGR and lag identities inspected |
| Inflation shocks/roll | CAGR shock; multiplicative gross annual factor; base CPI pin | Knot/source-knot shock; retained-source roll | [CORE-MARKET-10](inflation-projection.md) | Inflation roll/serde/shock invariants located |
| Observed monthly reference CPI | Unique reference-month prints; daily weight in contract month | Step collapses to day one; Linear interpolates reference anchors | [CORE-MARKET-11](inflation-observations.md) | Hand-computed month/day and missing-month tests inspected |
| Seasonality/publication eligibility | Monthly multiplication; stored release schedule | Positive unnormalized factors; lookup does not enforce publication | [CORE-MARKET-11](inflation-observations.md) | Seasonal formula inspected; publication validation located |
| Generic scalar series | Sorted dated observations; Step or day-linear lookup | Default LOCF; no left extrapolation; constant right tail | [CORE-MARKET-12](scalar-series-and-fixings.md) | Series interpolation/domain tests located |
| Historical/CMS fixings | Exact observed-date requirement and typed ID formatting | Missing series/date errors; as_of only diagnostic | [CORE-MARKET-12](scalar-series-and-fixings.md) | Fixing tests located; exact lookup inspected |
| Scalar/time-series shocks | Parallel numeric shifts/scales; checked monetary arithmetic | Money Percent scales; Unitless Percent adds points | [CORE-MARKET-12](scalar-series-and-fixings.md) | Overflow/units/parallel-only tests located |
| Dividend inputs | Chronological event sorting; nonnegative cash/yield/stock ratio | Cash builder; all variants in wire; same-date duplicates retained | [CORE-MARKET-13](dividend-events.md) | Inline construction/count test inspected |
| Volatility quote-grid domain | Rectangular axes; quote convention; row displacement metadata | Black/Strike/Vol defaults; shifted Black needs per-expiry shifts | [CORE-MARKET-14](volatility-grids-and-shocks.md) | Displacement/expiry domain tests located |
| Parallel/node/bucket vega shocks | Floored additive; nonnegative scaling; nearest-node point shock | Bucket filters tolerance 1e-9; percentage bucket floors negatives | [CORE-MARKET-14](volatility-grids-and-shocks.md) | Mutation/source-preservation tests located |
| FX delta quote admissibility | ATM+BF +/- RR/2 positive wings; optional paired 10d quotes | Smile-vol BF identity, no delta-convention selection in core | [CORE-MARKET-15](fx-delta-quotes.md) | Nonpositive-wing rejection inspected |
| SABR cube input domain | Validated alpha/beta/nu/rho/shift; expiry-major node grid | nu=0 allowed; shifts/forwards only finite here | [CORE-MARKET-16](sabr-cube-data.md) | Parameter/shift/zero-nu/serde tests inspected |
| Materialized expiry slices | Positive expiry/tenors; tenor-major vol grid | Black/shifted/normal; per-tenor displacement | [CORE-MARKET-16](sabr-cube-data.md) | Slice constructor/serde domain tests inspected |
| Forward-variance inputs | Linear point interpolation or right-continuous interval values | Strictly positive values; flat tails; nonfinite query NaN | [CORE-MARKET-17](forward-variance.md) | Boundary and midpoint formulas inspected |
| Base correlation and diagnostics | Linear detachment interpolation; optional monotonicity report | Fixed flat-level tails; [0,100] detachment and [0,1] rho | [CORE-MARKET-18](base-correlation.md) | Curve tests located; stressed rebuild inspected |
| Correlation risk | Additive/multiplicative or matched-bucket shock then [0,1] clamp | Bucket tolerance 1e-10; generic rate-bp also accepted | [CORE-MARKET-18](base-correlation.md) | Correlation shock tests located |
| Credit-index constituents | Complete issuer map or homogeneous index; optional overrides | Explicit recovery/count; weights sum to one within 1e-9 | [CORE-MARKET-19](credit-index-composition.md) | Incomplete-pool rejection inspected; weight tests located |
| Issuer fallback resolution | Unknown curve/recovery/weight lookup falls back to aggregate | Index hazard, index recovery, equal weight defaults | [CORE-MARKET-19](credit-index-composition.md) | Fallback tests located; getter implementation inspected |
| Context data/bump target selection | Typed getters; ordered generic namespace precedence | Curves > surfaces > scalars > series; ordered shock batches | [CORE-MARKET-20](context-lifecycle.md) | Ordered 100*1.1*1.2 formula inspected |
| FX conversion and percentage shocks | Default-policy matrix query; checked Money conversion | Same currency bypass; FX percent/100; scratch price decimal pct | [CORE-MARKET-20](context-lifecycle.md) | Context code inspected; FX kernel separately owned |
| Context theta roll | Roll curve namespace only and rebind composites | Surface expiries, FX, observations, dividends unchanged | [CORE-MARKET-20](context-lifecycle.md) | Lifecycle invariants located |
| Composite lifecycle and restore | Canonical ID rebinding; drop invalidated indices; strict restore references | Insert/roll discard invalidated list; retention exposes it | [CORE-MARKET-20](context-lifecycle.md) | Rebinding and serde tests located |
| Snapshot state | Sorted validated data; per-namespace duplicate checks; quote-only FX | Versioned bytes versus typed state restoration | [CORE-MARKET-20](context-lifecycle.md) | Serialization/contract invariants located |
| Scenario hierarchy selection | Subtree tags, AND predicates, repeated overlap matches | MostSpecificWins default; Cumulative emits duplicates | [CORE-MARKET-21](hierarchy-selection.md) | Two-occurrence versus one-occurrence test inspected |
| Curve market-change measurement | Unweighted same-tenor average/per-tenor differences | Standard, baseline Dynamic or Custom grid; type-based units | [CORE-MARKET-22](market-change-measurement.md) | Diff dispatch/grid tests located |
| Inflation market-change source | CAGR curve change or latest raw print relative change | Projected curve pair preferred over print pair | [CORE-MARKET-22](market-change-measurement.md) | Print/seasonality bypass tests located |
| FX/scalar market changes | Relative percent or absolute native-unit differences | Default FX query; scalar unit/currency checks | [CORE-MARKET-22](market-change-measurement.md) | Scalar mismatch and selected-error tests located |

## Evidence boundaries and follow-up tracking

The [core-market follow-ups](../FOLLOW_UPS.md#core-market-data) own pending decisions, implementation discrepancies and missing validation. The records retain observed behavior, evidence and scope limits; neither the inventory nor an open item adopts a method or authorizes a runtime change.

The [coverage ledger](coverage.json) is the exact file accounting. Support files are re-exports/declarations, not uninspected algorithms. Dedicated test files contain only test fixtures/assertions; inline tests do not change the owning production file's classification. Test-only imports and declarations that appear before later production code were checked explicitly.
