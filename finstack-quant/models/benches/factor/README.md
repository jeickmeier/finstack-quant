# Factor-model benchmarks

Criterion benchmarks for the factor-model primitives that sit in inner loops:
`FactorCovarianceMatrix` construction and lookups, and the three `FactorMatcher`
implementations. The crate sets `autobenches = false` in
[`../../Cargo.toml`](../../Cargo.toml), so a new file here is inert until it is added as a
`[[bench]]` target. One target is registered: `factor_model` (`harness = false`).

Scope note: this target does **not** cover credit calibration or risk decomposition.
Everything measured here is matrix lookup and matcher dispatch — the per-dependency work
a risk run repeats once per instrument leg.

## Groups

| Group | Ids | Measures |
|-------|-----|----------|
| `covariance_construction` | `validated/50` | `FactorCovarianceMatrix::new` on a 50×50 matrix, including the symmetry and PSD validation it performs on every construction |
| `covariance_lookups` | `variance/50`, `covariance/50`, `correlation/50` | Single-entry lookups by `FactorId` |
| `covariance_batch_lookups` | `all_variances/50`, `all_correlations/50` | The N and N(N−1)/2 loop shapes a risk run actually issues |
| `mapping_table_matcher` | `hit_first/50`, `hit_last/50`, `miss/50` | `MappingTableMatcher::match_factor_with_betas` over 50 rules; the three ids bracket best case, worst case, and full-scan miss |
| `hierarchical_matcher` | `{shallow_2x3,medium_3x3,deep_4x2}_{hit,fallback}` | `HierarchicalMatcher` tree traversal at three depth/branching shapes, matched on an `Attributes` tag and on a tag that hits no child |
| `cascade_matcher` | `hit_first_stage`, `hit_second_stage`, `miss_all_stages` | `CascadeMatcher` over two `MappingTableMatcher` stages (exact rule, then generic-credit fallback) |

`hit_first` vs `hit_last` vs `miss` in `mapping_table_matcher` is the linear-scan guard:
if rule matching ever gains an index, `hit_last` should collapse toward `hit_first`. If
`hit_last` drifts away from `hit_first` faster than rule count grows, the scan gained a
per-rule cost.

The covariance fixture is deterministic and PSD by construction — unit diagonal with
off-diagonal `0.3 / (|i−j| + 1)` — so `FactorCovarianceMatrix::new` always takes the
success path and the measurement is not an early-return.

## Layout

| File | Registered bench? | Contents |
|------|-------------------|----------|
| `factor_model.rs` | yes | All six groups above |
| `support/bench_utils.rs` | no | `bench_iter(group, id, f)`, pulled in via `#[path = "support/bench_utils.rs"] mod bench_utils;` — a helper module, not a target |

`bench_iter` only wraps `group.bench_function(id, |b| b.iter(f))`. The file duplicates
the same-named helper in [`../../core/benches/support/`](../../core/benches/support);
they are independent copies, and core's additionally carries `bench_with_criterion`.

## Run

```bash
mise run rust-bench-crate -- finstack-quant-models factor_model
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

- A new `FactorMatcher` implementation should get the same three-id treatment as the
  existing ones — first-rule hit, last-rule hit, and total miss. Matcher regressions
  show up in the miss path first.
- Build matchers and matrices outside `b.iter`; only the `match_factor_with_betas` or
  lookup call belongs inside.
- Register the function in the `criterion_group!` list at the bottom of
  `factor_model.rs`, or it never runs.

## See also

- [`../README.md`](../README.md) — crate overview, the credit-calibration surface not
  covered here, and the contract tests under [`../tests/`](../tests)
- [`../../portfolio/benches/README.md`](../../portfolio/benches/README.md) — where these
  primitives are exercised at book scale (`sensitivity_simulation`,
  `parallel_thresholds`)
- [`credit_factor_calibration.rs`](credit_factor_calibration.rs) — end-to-end
  `CreditCalibrator` panel this target omits
