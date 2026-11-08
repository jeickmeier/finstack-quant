# finstack-quant-cashflows benchmarks

Two Criterion targets are declared in [Cargo.toml](../Cargo.toml), with
`autobenches = false` and `harness = false`:

| Target | Cases | Scope |
| --- | ---: | --- |
| `cashflow_hot_paths` | 34 | Fixed-size complete calls, history/piece axes and named workflows |
| `cashflow_scaling` | 52 | Coupon/query/row counts and fixed-size adjustment axes |

Every selected case runs an untimed output preflight before measurement. Market,
schedule and output preparation lives inside Criterion's selected-case closure,
so filtering one case does not build unrelated fixtures. Temporary preflight
outputs are dropped before the timing loop where they are not measured inputs.
Timed valid-case failures are fatal; successful outputs are consumed in full.
These checks establish benchmark validity, not an independent certification of
every financial convention.

## Fixed-size cases

| Group | Case suffixes and measured operation |
| --- | --- |
| `cashflow_pv_by_period` | `5y_q_22rows`: public reporting-period PV, no credit curve |
| `cashflow_pv_by_period_credit` | `{no_recovery,with_recovery}/5y_q_22rows`: public reporting PV with flat 1.5% hazard and 0%/40% recovery |
| `cashflow_build_fixed` | `5y_q`: complete fluent fixed-bullet build, 20 coupons |
| `cashflow_build_floating` | `term_5y_q`, `overnight_arrears_5y_q`, `overnight_lookback5_5y_q`: complete projected floating builds |
| `cashflow_build_structured` | `amortizing_linear_5y_q`, `fixed_plus_periodic_fee_5y_q`: complete deterministic amortization/fee builds |
| `cashflow_json_bridge` | `build_5y_q`, `validate_5y_q`, `dated_flows_5y_q`: parse/build/serialize, canonical validation/serialize, and cash extraction/serialize |
| `cashflow_calendar_year_ladder` | `10y_q_42rows`: canonical CFKind label parsing and calendar-year aggregation over prepared arrays |
| `cashflow_accrued_variants` | `compounded_5y`, `ex_coupon_rule_outside_window_usny_5y`, `ex_coupon_active_negative_usny_5y`: index build plus one AI query |
| `cashflow_aggregate_by_period` | `120inputs_20periods_20included`: nominal aggregation; only 20 of 120 prepared rows fall into the reporting windows |
| `cashflow_aggregate_precise` | `120`: compensated, currency-checked sum of 120 rows |
| `cashflow_extract_and_npv` | `5y_semiannual_12rows`: validated dated-cash extraction, then core Money NPV |
| `cashflow_clone_and_merge_schedules` | `20schedules_240rows`: deep input clone, Notional construction, consuming merge and canonical sort |
| `cashflow_outstanding_by_date` | `40`: economic balance replay over 40 equal amortization rows |
| `cashflow_wal` | `40`: Act/365F WAL over the same principal rows |
| `ccm_build_term_history`, `ccm_build_overnight_history` | `mixed_p{P}_extra{H}`: complete mixed historical/projected builds, 64 coupons each; cells `(P,H)=(16,0),(16,250),(16,2500),(16,10000),(1,2500),(64,2500)` |

The five-year quarterly fixed bullet has 20 coupons and two principal rows; the
ten-year version has 42 rows. The ladder uses each row's canonical kind string,
including `fixed` and `notional`, and supplied synthetic PVs of amount × 0.85.
It measures aggregation of those supplied PVs, not pricing. The active ex-coupon
case settles one calendar day before the April 15, 2026 coupon and requires
negative AI; the outside-window control retains the previous query date.

NPV deliberately includes `dated_flows()` validation and allocation on every
iteration. Merge deliberately includes cloning all 20 input schedules on every
iteration. They are complete named workflows, not kernel-only measurements.
JSON validation is serde/economic validation and canonicalization; it does not
invoke the JSON Schema validator. No DataFrame export is measured.

## Scaling cases

| Group | Sizes / suffixes | Throughput unit and boundary |
| --- | --- | --- |
| `scaling_build_monthly` | 60/120/240/480/960 | Coupons; complete fluent fixed build on an unadjusted grid |
| `scaling_build_amortizing_monthly` | 60/120/240/480/960 | Coupons; preconfigured builder's compile/project/build, with `2N+1` output rows |
| `scaling_build_adjustment_axes_20y_q` | `payment_only`, `accrual_adjusted`, `accrual_adjusted_lag2` | Fixed 80-coupon USNY comparison, not a length sweep |
| `scaling_build_overnight_quarterly` | 20/40/80 | Coupons; complete projected in-arrears build, not observations per second |
| `scaling_build_floating_term` | 20/40/80/160 | Coupons; complete projected term build |
| `scaling_accrued_single_query` | 60/120/240/480 | Schedule coupons; one index build plus one AI query |
| `scaling_accrued_per_exercise_date` | `{per_call_rebuild,prebuilt_index}/{60,120,240,480}` | Queries; N index-build/query calls versus one index build plus N lookups |
| `ccm_accrued_contractual_band` | `{early,middle,late}/{60,120,240,480}` | Queries; one index build plus N queries in a fixed contractual band |
| `scaling_pv_by_period` | 60/120/240/480 | Coupons, plus two principal input rows; complete risk-free reporting PV |
| `scaling_outstanding_by_date` | 40/80/160/320 | Synthetic amortization rows; complete balance replay |

The amortizing builder configuration remains outside timing but inside the
selected-case closure. Its `build()` still recompiles and projects each time.
The repeated-query `prebuilt_index` creates one index **inside each timed
operation** and then performs all N queries. Both variants consume every query
result and preserve their original scalar summation order. Their synthetic
30-day query dates do not implement an exercise decision.

The contractual bands use actual monthly anchors: each has N queries spread
over N/4 distinct mid-coupon dates. Early, middle and late bands start at coupon
0, N/2 and 3N/4. Repeated dates remain repeated queries in their original order;
the bands are not option exercise decisions. Every timed operation builds one
index and consumes all N scalars plus their ordered sum.

Throughput units differ by group. Compare matched cases and interpret rising
cost per element against the operation's actual dimensions; an adjustment-axis
comparison at one size does not establish asymptotic scaling.

## Fixtures and preflight expectations

All schedules start January 15, 2025, in USD with opening notional 1,000,000.
Ordinary fixed coupons are 6% Act/365F. Ordinary quarterly fixtures use weekends-only
ModifiedFollowing payments and unadjusted accrual dates; monthly length sweeps
use Unadjusted. Adjustment-axis cases independently check USNY dates and lag.
The six-knot `USD-OIS` fixture has LogLinear discount factors from 1.0 at zero to
0.375 at 30 years. Floating fixtures use a flat 4.5% Act/360 forward plus 200bp,
with curve origin 45 days before issue and no historical fixing series.

The history/piece workloads instead hold 64 quarterly coupons through January
15, 2041 fixed while changing the number of programme windows and extra older
fixing rows. Their explicit as-of is July 16, 2025. H counts **extra** rows before
the earliest required observation; H=0 still includes required historical
fixings. The term leg has two-business-day resets; the overnight leg has weekday
carry, zero lookback and Act/360. Both use 200bp spread and a linear forward
curve from 3% at zero to 5% at 40 years, based December 1, 2024. Fixing-series
currency and interpolation are explicit. Timed builds include compile, historical
selection, projection, emission and metadata normalization. Separate independent
references check complete outputs before timing; those reference/history copies
are dropped before the loop.

[checks.rs](support/checks.rs) checks counts, USD amounts, contractual dates,
principal signs, fixed coupon formulas, term reset dates and overnight weighted
products. Reporting PV uses a separate direct row/period sum with the canonical
DF primitive and explicit flat-hazard survival/recovery formula. AI uses a
separate linear days-ratio oracle; compounded AI uses a direct power formula.
Synthetic balance paths and WAL use the closed-form equal-installment results.
JSON and merge preflights check complete row correspondence and metadata.
Output comparisons use explicit absolute tolerances in currency units or the
stated dimensionless units; none are performance acceptance thresholds.

The untimed panels in `tests/performance` capture all scalar bits, Decimal
representations, ordered metadata, input hashes and error signatures for C0
comparison. Independent history bounds are 1e-7 USD for term coupons, 1e-5 USD
for overnight coupons, 1e-11/1e-10 for their annualized rates, and 1e-11 for raw
fixings. The monthly AI matrix uses 1e-7 USD per scalar and 1e-6 USD per ordered
sum. These fixed bounds do not authorize candidate drift: ownership/query
changes must also match the separate C0 records exactly on the same build.
The panels additionally cover same-day selection, missing fixing errors,
lookback/shift/cutoff/average, holiday carry, materialize/roll/rebuild, overlapping
accrual windows, ex-coupon boundaries and window-before-formula error ordering.

Complex optionality is captured separately in the fixed-income test-only
consumer panels. Default-time recovery, FX conversion and waterfalls still
require distinct workloads and numerical contracts before optimization.

## Run

```sh
mise run rust-bench-crate -- finstack-quant-cashflows cashflow_hot_paths --test
mise run rust-bench-crate -- finstack-quant-cashflows cashflow_scaling --test
mise run rust-bench-crate -- finstack-quant-cashflows cashflow_scaling --filter '^scaling_accrued_per_exercise_date/'
mise run rust-bench-crate -- finstack-quant-cashflows cashflow_hot_paths --filter '^ccm_build_.*_history/' --test
mise run rust-bench-crate -- finstack-quant-cashflows cashflow_scaling --filter '^ccm_accrued_contractual_band/' --test
mise run rust-bench-crate -- finstack-quant-cashflows cashflow_hot_paths --save-baseline ccm-c0-r1
mise run rust-bench-crate -- finstack-quant-cashflows cashflow_hot_paths --baseline ccm-c0-r1
```

`--test` is Criterion smoke mode, not a timing baseline. The scoped task accepts
`--filter`, `--test`, `--save-baseline` and `--baseline`. Sampling is controlled by
`FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`, `FQ_BENCH_MEASUREMENT_TIME` and
`FQ_BENCH_NRESAMPLES`. Archive raw repetitions before Criterion overwrites its
working directories; a named baseline alone is not an immutable evidence record.

Ordinary `rust-test`, `rust-lint`, `all` and CI gates do not execute these
Criterion targets. Benchmark compilation and checked smoke must be requested
explicitly. Performance acceptance applies only to a declared matched workload
and protocol, not to every microbenchmark automatically.

## Related benchmarks

- [Core benchmarks](../../core/benches/README.md): date grids, day counts and
  core-only dated Money NPV.
- [Valuation benchmarks](../../valuations/benches/README.md): instrument-level
  cashflow generation and complete pricing consumers.
