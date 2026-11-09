# Core market-data methodology audit

Decision status: **CORE-MARKET-01 through CORE-MARKET-19 Accepted; the other 3 detailed records remain Draft.** Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

[CORE-MARKET-01's complete discount-evaluation package](discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction)
was accepted on **2026-11-08**, covering explicit source choices, signed factor
domains, strict state, negative-rate admission and checked zero/interval/local
rate extraction. Its independent expression examples were executed separately;
they do not qualify the production implementation. Runtime/caller/host
reconciliation and independent qualification remain pending; the other 3
records retain their existing observed or proposed choices.

[CORE-MARKET-02's revised factor package](forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates)
was accepted on **2026-11-08**: one positive projection-factor source using
accepted01's numerical foundation, authoritative index/source association and
resolved index-period growth divided by contractual index accrual. It removes
seed means, reset-grid chaining, convention inference and lossy discount
resampling. Calibration, term/overnight projection, model baselines and risk
adapters require reconciliation and independent qualification.

[CORE-MARKET-03's complete shock/conditioning package](rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning)
was accepted on **2026-11-08**: additive continuous-zero parameter shocks for
both factor roles, explicit units and exact-width bucket geometry, one retained
complete evaluator, and dated frozen-source conditioning on the original clock.
It distinguishes direct risk from quote recalibration, first-order sensitivity
additivity from finite bumped PV changes, and newly constructed dated views
from old-view relative-factor queries. Runtime/caller/host reconciliation and
independent qualification remain pending.

[CORE-MARKET-04's complete intensity/probability package](hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation)
was accepted on **2026-11-08**: strict positive ending knots and nonnegative
intensities, explicit source clock/context, fixed last-intensity tail, direct
local survival/PD integration, no generic cap/origin repair, and underflow-safe
original-clock dated conditioning. Probability endpoint rounding, finite-J
range, recovery metadata and observed credit events remain distinct. Runtime/
caller/host reconciliation and independent qualification remain pending.

[CORE-MARKET-05's complete quote/risk/recovery package](hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery)
was accepted on **2026-11-08**: remove ambiguous core quote/triangle views,
use existing observations and full CDS pricing, distinguish direct intensity
hazard01 from quote-recalibrated CS01, require exact targets with explicit
parameter-bucket/tail behavior, and separate frozen-intensity recovery changes
from quote-preserving recovery recalibration. Runtime/caller/host reconciliation
and independent qualification remain pending.

[CORE-MARKET-06's complete construction/replay package](calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification)
was accepted on **2026-11-08**: immutable lossless effective construction records,
calibration-owned payload semantics, atomic quote bindings, explicit dependency/
output state and request-time source/target replay qualification. Historical
presence is not current-fit authority; ordinary replay returns the accepted
recalibrated source without default reconstruction or discrepancy overlays.
Runtime/caller/host reconciliation and independent qualification remain pending.

[CORE-MARKET-07's complete NS/NSS package](parametric-discount.md#accepted-decision-validated-nsnss-sources-and-analytic-factor-queries)
was accepted on **2026-11-08**: immutable validated model parameters with equal
horizons admitted, one checked analytic factor evaluator, explicit source clock
and pre-origin Error domain, stable coefficient-aware kernels, analytic trait
dispatch and retained-model dated conditioning/overlays. Calibration assumptions
remain separately owned; runtime/caller/host reconciliation and independent
qualification remain pending.

[CORE-MARKET-08's complete quoted-level package](price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks)
was accepted on **2026-11-08**: explicit Price/VolIndex sources without automatic
spot anchoring, mandatory clock/style/tails, checked level domains, native/
relative source-parameter shocks and frozen dated reuse on the original clock.
Quotation identity, observed spot/fixings, contractual aggregation and carry
models retain their actual owners. Runtime/caller/host reconciliation and
independent qualification remain pending.

[CORE-MARKET-09's complete derived-diagnostic package](basis-spreads.md#accepted-decision-source-derived-dated-zero-rate-differences)
was accepted on **2026-11-08**: remove BasisSpreadCurve and its optional market
byproduct/transport paths; derive an explicitly oriented dated continuous-zero
diagnostic from the actual discount sources with one common reporting clock,
checked cancellation-aware log arithmetic and recomputation at a new anchor.
Typed basis observations, source shocks and full quote recalibration retain
their owners. Runtime/caller/host reconciliation and independent qualification
remain pending.

[CORE-MARKET-10's complete CPI package](inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks)
was accepted on **2026-11-08**: explicit positive anchored index-level sources,
checked source-clock CAGR and stable original-anchor annual-growth parameter
shocks; remove core lag/principal-ratio conveniences and scalar CPI rebasing.
Frozen dated reuse retains absolute levels on the original clock while products
resolve actual reference CPI. Runtime/caller/host reconciliation and independent
qualification remain pending.

[CORE-MARKET-11's complete monthly-fixing package](inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability)
was accepted on **2026-11-08**: exact positive monthly fixing snapshots with
explicit version/knowledge cutoff and actual-versus-expected release facts;
remove source lag/interpolation/seasonal multiplication. Contract reference
resolution uses explicit calendar anchors/weights and observed/projected
eligibility, with no reference-date publication or missing-fixing fallback.
Runtime/caller/host reconciliation and independent qualification remain pending.

[CORE-MARKET-12's complete scalar/series/fixing package](scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings)
was accepted on **2026-11-08**: validated opaque scalars, strict ordered finite
series with explicit sampling/tails, native/factor generic transformations and
exact fixing association by observed IndexId. CMS identity and publication
qualification remain explicit at financial owners; ordinary fixing-history
bumps are refused. Runtime/caller/host reconciliation and independent
qualification remain pending.

[CORE-MARKET-13's complete cash-event package](dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates)
was accepted on **2026-11-08**: identified gross cash-per-share events with
explicit ex/pay dates and underlying/share/currency association; remove dated
stock/yield variants and preserve distinct same-date events/original source.
Financial adapters qualify actual entitlement/payment timing without a universal
pay>=ex rule or implicit one-date conversion. Runtime/caller/host reconciliation
and independent qualification remain pending.

[CORE-MARKET-14's complete volatility-grid/shock package](volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks)
was accepted on **2026-11-08**: explicit native-sigma conventions/axes, positive
expiries and accepted source-clock admission; no shift retagging. Direct native
delta/nonnegative-factor shocks use exact targets without floors/snap/tolerance
matching and preserve metadata/scoped scratch state. Pricing limits, off-grid
models, diagnostics and aging retain their owners. Runtime/caller/host
reconciliation and independent qualification remain pending.

[CORE-MARKET-15's complete FX quote package](fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs)
was accepted on **2026-11-08**: explicit ordered pair and supported forward-delta/
DNS/arithmetic-smile-BF profile, canonical raw quotes with shared checked wing
arithmetic, source clocks and actual delivery-forward inputs. Checked transforms
and supported geometry retain exact pillars while quote-space evaluation,
materialized strike grids and reporting/risk conventions remain distinct.
Runtime/caller/host reconciliation and independent qualification remain pending.

[CORE-MARKET-16's complete SABR source/slice package](sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
was accepted on **2026-11-08**: exact canonical parameter/forward sources,
mandatory cube modes and explicit evaluated quote output, without beta-derived
units or source-node repair. Preserve clocks/underlying association and supported
active selection; truthful fixed-expiry sigma slices and sampled surfaces remain
distinct from the original SABR model. Runtime/caller/host reconciliation and
independent qualification remain pending.

[CORE-MARKET-17's complete forward-variance package](forward-variance.md#accepted-decision-nonnegative-variance-sources-and-explicit-model-time-semantics)
was accepted on **2026-11-08**: nonnegative ordered point/interval sources,
literal boundaries/tails, checked stable level queries and explicit variance
clock/unit/original conditioning qualification. Separate Cheyette volatility
amplitudes and actual ATM proxies; source areas do not certify simulation-grid
accuracy or rough-model aging. Runtime/caller/host reconciliation and independent
qualification remain pending.

[CORE-MARKET-18's complete base-correlation package](base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks)
was accepted on **2026-11-08**: strict detachment-percent/rho sources with
constant singletons, checked bounded Linear/Flat queries, literal nonblocking
shape reports and unclipped native exact-target shocks. Qualify actual units,
financial association, supported endpoint pricing and honest risk geometry;
source bounds and rho shape do not certify loss consistency. Runtime/caller/host
reconciliation and independent qualification remain pending.

[CORE-MARKET-19's complete credit-pool package](credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts)
was accepted on **2026-11-08**: explicit Homogeneous/Named composition, atomic
Named issuer inputs/derived count, strict lookup and an explicit compensated
unit-mass budget without normalization. Separate future cash, hazard metadata
and historical recoveries; retain actual index/member dependencies and canonical
state/host contracts. Runtime/caller/host reconciliation and independent
qualification remain pending.

The next unaccepted record is [CORE-MARKET-20](context-lifecycle.md), covering
market-context selection, ordered mutations, composite references, scratch state,
dated conditioning and snapshots. Its methodology remains Draft.

The exact assigned scope is **60 Rust source files** under `finstack-quant/core/src/market_data`: **52 algorithm-bearing files, 6 support/adapter files, and 2 dedicated test files**. Every assigned file was inspected and classified once in [coverage.json](coverage.json). The **22 records** below describe numerical kernels, input validation, data resolution, transformations, approximations and failures at algorithm level. Default model names and stored metadata are not treated as approval. No runtime code, tests or fixtures were changed.

Production validation is **source and existing-test inspection/location, not test execution**. Inspected hand-computed formulas are distinguished from comparisons to the production kernel and from serialization/invariant tests. No build, production test suite, vendor reconciliation or new empirical accuracy experiment was run. The separately executed 2026-11-08 expressions/counterexamples are recorded in CORE-MARKET-01 through CORE-MARKET-19. An implementation domain or validation gap remains explicit even when existing regression tests cover ordinary inputs.

The shared interpolation kernels are owned by the [core numerics value-interpolation](../core-numerics/value-interpolation.md) and [forward-interpolation](../core-numerics/forward-interpolation.md) records. This slice records each curve's actual selection, variable being interpolated, domain, extrapolation and caller behavior. Core surface/cube/FX quote files store and validate data; [model volatility interpolation](../models-analytic/volatility-source-interpolation.md), [FX delta smiles](../models-analytic/fx-delta-smiles.md), and [SABR expansions/calibration](../models-analytic/sabr-expansions.md) own their numerical evaluators. Money/FX matrix resolution is outside this manifest; context FX delegation and units are covered here. Calibration recipes are inspected as recipe-validation/dispatch contracts; no inverse calibration solver exists in those two core files.

## Governing and overview ADRs

These ADRs retain their stated scope and decision status. They are separate
from the detailed algorithm records and source counts below.

| ADR | Decision status | Scope |
| --- | --- | --- |
| [0032](../calibration-targets/0032-curve-interpolation-and-calibration.md) | Draft | Rate-curve interpolation during calibration |

## Inventory

All production evidence in the fifth column is **not executed**. Inventory rows describe observed runtime selection, including defaults targeted by accepted numerical decisions and CORE-MARKET-01/02/03/04/05/06/07/08/09/10/11/12/13/14/15/16/17/18/19. Each linked record gives the exact owning source/test paths, method references, domain, alternatives and limitations.

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
