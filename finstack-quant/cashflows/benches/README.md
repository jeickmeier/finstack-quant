# finstack-quant-cashflows benchmarks

Criterion benchmarks for schedule construction, accrual, PV aggregation, and DataFrame
export. The crate sets `autobenches = false` in [`../Cargo.toml`](../Cargo.toml), so a
new file here is inert until it is added as a `[[bench]]` target. Two targets are
registered, both `harness = false`:

| Target | Scope |
|--------|-------|
| `cashflow_hot_paths` | Absolute cost of each hot path at one representative size |
| `cashflow_scaling` | How cost grows with schedule length — the complexity guard |

The split matters. `cashflow_hot_paths` answers "how expensive is this call"; only
`cashflow_scaling` can catch a super-linear term coming back, and it does so by reporting
`Throughput::Elements` so ns-per-coupon is directly comparable across sizes. Flat
ns-per-coupon is linear; rising ns-per-coupon is the regression signal.

## `cashflow_hot_paths` groups

| Group | Cases |
|-------|-------|
| `cashflow_pv_by_period` | `5y_40cf` — `CashFlowSchedule::pv_by_period_with_discounting` over quarterly reporting periods, `PvDiscountSource::Discount` with no credit leg |
| `cashflow_pv_by_period_credit` | `no_recovery/5y_40cf`, `with_recovery/5y_40cf` — the same call with a `PvCreditAdjustment` carrying a flat `HazardCurve`, with and without a 40% recovery rate |
| `cashflow_build_fixed` | `5y_q` — full `CashFlowSchedule::builder()` build of a quarterly fixed-coupon bullet |
| `cashflow_aggregate_by_period` | `120f_20p` — `aggregate_by_period` over nominal `DatedFlows` |
| `cashflow_aggregate_precise` | `120` — `aggregate_cashflows_checked`, the compensated single-currency sum |
| `cashflow_npv` | `5y` — `Discountable::npv` on a semi-annual schedule (one allocation per call) |
| `cashflow_merge_schedules` | `20` — `merge_cashflow_schedules` k-way concat plus re-sort |
| `cashflow_outstanding_by_date` | `40` — balance-path tracking over an amortizing schedule |
| `cashflow_wal` | `40` — `wal` over the same amortizing schedule |

Market fixtures are a six-knot `USD-OIS` `DiscountCurve` (`InterpStyle::LogLinear`,
discount factors 1.0 at 0y down to 0.375 at 30y — a downward-sloping zero curve, not a
flat one) and a flat 1.5% `USD-CREDIT` `HazardCurve` at 40% recovery, both built once
outside `b.iter`.

## `cashflow_scaling` groups

| Group | Sizes | Measures |
|-------|-------|----------|
| `scaling_build_monthly` | 60 / 120 / 240 / 480 / 960 coupons (5–80y monthly) | Schedule build must stay linear in coupon count |
| `scaling_build_adjustment_axes_20y_q` | `payment_only`, `accrual_adjusted`, `accrual_adjusted_lag2` | Marginal cost of business-day adjustment axes on a `usny` calendar at a fixed 20y quarterly size |
| `scaling_accrued_single_query` | 60 / 120 / 240 / 480 coupons | Cost of one `accrued_interest_amount` query against schedule length |
| `scaling_accrued_per_exercise_date` | `per_call_rebuild/{60,120,240}` vs `prebuilt_index/{60,120,240}` | The repeated-query pattern: N calls to `accrued_interest_amount` versus one `AccrualIndex::build` plus N `accrued_at` lookups |

`scaling_accrued_per_exercise_date` is the one to watch when touching accrual: it is the
callsite shape a Bermudan exercise loop actually has, and the two ids exist to keep the
`AccrualIndex` payoff visible. Note that the `prebuilt_index` case builds the index
*inside* `b.iter`, so it measures build-plus-N-lookups, not lookups alone.

`scaling_build_monthly` uses `BusinessDayConvention::Unadjusted` on `weekends_only`
deliberately — calendar work is isolated into `scaling_build_adjustment_axes_20y_q`
rather than mixed into the size sweep.

## Run

```bash
cargo bench -p finstack-quant-cashflows --bench cashflow_hot_paths
cargo bench -p finstack-quant-cashflows --bench cashflow_scaling
cargo bench -p finstack-quant-cashflows -- --quick
cargo bench -p finstack-quant-cashflows -- scaling_accrued      # filter by group name
cargo bench -p finstack-quant-cashflows -- --save-baseline before
cargo bench -p finstack-quant-cashflows -- --baseline before
```

Benchmarks are measurement tasks, not gates: they are not run by `mise run rust-test`
(nextest), not by `mise run all-test`, and not by PR CI. `mise run rust-lint` and
`mise run rust-fix` do not compile Criterion targets. Workspace-wide measurement goes through `mise run rust-bench` (reduced
sampling, tunable via `FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`,
`FQ_BENCH_MEASUREMENT_TIME`, `FQ_BENCH_NRESAMPLES`), with
`mise run rust-bench-baseline` and `mise run rust-bench-compare` (fails above a 10%
median regression).

Criterion writes to `target/criterion/<group>/<id>/report/index.html`; the
`mise run rust-bench*` tasks pass `--noplot`.

## Conventions when adding a case

- Put a fixed-size case in `cashflow_hot_paths` and a size sweep in `cashflow_scaling`.
  Do not add a sweep to the hot-paths target — it is read as absolute cost.
- Any sweep must set `group.throughput(Throughput::Elements(n))`, otherwise the
  ns-per-coupon reading that makes the target useful is unavailable.
- Build schedules and `MarketContext` outside `b.iter`; `black_box` the schedule
  reference so the call is not hoisted.
- Both targets set `#![allow(clippy::unwrap_used)]` and `#![allow(clippy::expect_used)]`
  at file scope. These are defensive, not load-bearing: those lints are denied by an
  inner attribute in [`../src/lib.rs`](../src/lib.rs), which covers the library crate
  only, and they are absent from `[workspace.lints.clippy]`. A bench target inherits
  neither, so `mise run rust-lint` is green over `.unwrap()` here with or without the
  attributes.

## See also

- [`../README.md`](../README.md) — crate overview and the test tree under
  [`../tests/`](../tests)
- [`../../core/benches/README.md`](../../core/benches/README.md) — `schedule_generation`,
  `daycount_operations`, and `cashflow_operations`, the core-level primitives these
  benchmarks sit on top of
- [`../../valuations/benches/README.md`](../../valuations/benches/README.md) —
  `cashflow_generation`, the instrument-level view of the same build path
