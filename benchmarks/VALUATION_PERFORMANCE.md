# Valuation benchmark decisions and results

This is the summary of the **November 7, 2026** valuation performance campaign.
The historical record accepted six scoped changes and the final B0 → F1
comparison across 111 frozen workloads. This document preserves those reported
results; shortening the record does not constitute a new benchmark run or
validation of the current checkout.

The [machine-readable results](valuation-performance-results.json) retain all
111 final cases, absolute process medians, within-process confidence intervals,
comparison summaries and historical source identities. The
[provenance manifest](valuation-evidence.json) identifies the retained Git
history and current numerical regression fixture.

## Evidence available to a reviewer

The [original complete report](https://github.com/jeickmeier/finstack/blob/827ebb75b9ec42a43906cf3a98c4b67834396f6f/benchmarks/VALUATION_PERFORMANCE.md)
is preserved at its immutable Git revision. It contains the detailed chronology,
rejected variants, failed attempts and original local artifact paths.
The checked-in summaries and numerical fixtures remain available in this
repository. Historical raw timing ledgers, archived B0 binaries/source snapshots,
profiles and validation logs were **not retained and are lost**. References
to those artifacts have been removed from the current summary. Historical
pass records are unverified.

Consequently, the figures below can be checked against the committed summary,
but the historical timing and acceptance decisions cannot be independently
replayed from this checkout alone. A build of current source is not the archived
B0 implementation. Keep that distinction when citing the results or planning a
new comparison. These records follow [ADR 0040](../docs/decisions/standards/0040-measured-performance-decisions.md)
and do not alter the separate materialization baselines or create CI budgets.

## Retained implementation decisions

| Slice | Retained change | Numerical boundary |
| --- | --- | --- |
| P1 arithmetic Asian MC | One paired traversal replaces two N-path captures; the arithmetic route retains neither trajectories nor payoff vectors. | Preserve grid, seeds, paired moments and independent-estimator counts. |
| P2 MBS OAS | Project monthly cashflow coefficients once per path; evaluate one spread factor per nonzero month per Brent trial. | Preserve production objective, root and repricing bounds. Scratch is at most 32 monthly blocks, with no extra paths-by-months matrix. |
| P3 OIS | Internal PV omits projected-fixing metadata and series-ID strings. | Preserve canonical daily projection and public cashflow/export metadata. Discount-identity routing was rejected. |
| P4 Hull–White MC | Prepare exact interval coefficients once per grid and reuse them across paths. | Preserve coefficient arithmetic, random draws, antithetic order and event order. |
| P5 CDS tranches | Retain the combined exact-count recurrence for eligible equal-positive-weight/recovery pools and Gaussian homogeneous-index paired integration. | Require frozen independent-reference gates; some former approximate-grid prices intentionally change. Standalone BC6 was not selected. |
| P6 commodity Asian | Replace quadratic moment reductions with linear work and compensated scalar accumulators. | Preserve the existing Turnbull–Wakeman approximation and independent moment/price bounds. Existing projection vectors remain. |

Benchmark validity repairs are part of the retained work: selected operations
must succeed with finite, structurally complete outputs before timing; measured
errors are fatal. Fixture preparation follows case selection. Python controls
use nonempty deposit portfolios with strict requested risk; WASM controls also
require nonempty outputs. Host timings include computation and conversion;
WASM measurements exercise raw wasm-bindgen exports rather than the JS facade.
The repairs also corrected Asian expiry reporting, bond-future held-market theta
and physical-delivery horizon checks, and listed-future mark extraction. Old
error-path timings cannot be used as before baselines for those repaired cases.

## Reported final results

Of 81 predeclared opportunities, 75 met the repeatable 10% median-latency
criterion; six were completed below-material outcomes. All 30 controls passed
the recorded no-over-10% regression assessment after two selected whole-case
confirmations using original timing settings. The final comparison used 666
primary processes. Slice percentages are not additive.

These representative B0 → F1 values are copied from the committed final
summary. Latencies are median process medians in microseconds; reductions are
rounded for display. The complete 111-case results and uncertainty remain in
the machine-readable companion.

| Case | B0 median (µs) | F1 median (µs) | Reduction |
| --- | ---: | ---: | ---: |
| `asian_option_mc/paths_50000_fixings_12` | 315571 | 117042 | 62.911% |
| `agency_mbs_oas_scaling/paths_8192_months_360` | 69304.9 | 42183.2 | 39.134% |
| `ois_pv/10Y` | 274.491 | 188.386 | 31.369% |
| `cds_tranche_heterogeneous/npv_hetero/17` | 1.60064e+07 | 13395.8 | 99.916% |
| `commodity_asian_option/fixing_count/arithmetic/1260` | 2516.18 | 18.0799 | 99.281% |
| `calibration_base_correlation_full_plan_5_knots` | 8.3275e+06 | 4.85312e+06 | 41.722% |
| `commodity_forward_pv/6M` (control) | 0.406404 | 0.419446 | -3.209% |
| `ois_cashflow_schedule/5Y` (control) | 144.778 | 146.511 | -1.197% |

Peak RSS was measured with macOS `/usr/bin/time -l`, in bytes, using isolated
selected-case processes with capture disabled and checked preflight plus one
operation. These are process peaks, not direct allocation counts.

| Heavy case | B0 median bytes | F1 median bytes | Reduction |
| --- | ---: | ---: | ---: |
| `asian_option_mc/paths_50000_fixings_12` | 5033099264 | 6373376 | 99.873% |
| `agency_mbs_oas_scaling/paths_8192_months_360` | 32817152 | 32751616 | 0.200% |
| `mc_bermudan_lsmc_axes/paths_50000/base_128` | 296091648 | 296124416 | -0.011% |

The historical final validation reported 456 native smoke rows across 37
targets, 20 release Python rows, nine release Node/WASM rows, all numerical
contracts, generation, formatting and the full gate passing. Smoke durations
are validity evidence, not comparative latency results. These counts describe
the historical campaign and have not been rerun by this document cleanup.

## Measurement contract and limits

The suite used synthetic fixed workloads on one machine/profile with
`RAYON_NUM_THREADS=8`. No workload frequency, weighted portfolio benefit or
universal speedup is implied. Each revision had three independent process
repetitions in alternating order, without a competing build, profiler or
benchmark. Repeating a seed does not add independent estimators.

B0–P4 used 50 samples, three seconds warmup, five seconds measurement and
10,000 resamples for ordinary cases. Nine slow cases used ten samples with
60-second Bermudan or 120-second tranche/calibration windows; warmup and
resamples were unchanged. P5, P6 and F1 used the approved matched protocol of
ten samples, one second warmup, two seconds requested measurement and 10,000
resamples, preserving workloads and numerical gates. Criterion may extend a
requested window to obtain enough samples.

The final assessment uses 109 complete short-protocol cases and two complete
original-setting confirmations. A selected confirmation reran every revision
and all three repetitions for the whole case. Short and long campaigns were
not pooled or selected repetition by repetition. Their historical raw files
are lost; the summary retains the reported statistics and methodology.

Within-process 95% Criterion intervals are separate from between-process
variation. Ratios of marginal interval endpoints are descriptive ranges, not
calibrated confidence intervals for ratios. The campaign's materiality criteria
were at least 10% latency reduction or 20% peak-memory reduction, with control
regressions over 10% requiring investigation. They are scoped acceptance
criteria rather than library-wide guarantees.

Six opportunities remained below 10%: geometric commodity fixing-count 12 and
five LSMC cases (original 50000; base-128 paths 5000/20000/50000; base-64 paths
20000). MBS and Bermudan RSS were effectively unchanged. No P5 or P6 RSS
benefit is claimed. The deposit-only host controls do not establish mixed-asset
performance or cross-language workload parity.

## Numerical evidence and disclosed tradeoffs

- P1 retained 23 numerical records. Maximum absolute price difference was
  `7.993605777301127e-14`; physical paths and independent-estimator counts
  remain distinct.
- P2 retained 90 production-helper records. Maximum baseline root difference
  was `9.575673587391975e-16` in decimal spread; maximum fixed-spread baseline
  PV difference was `1.4435499906539917e-8`.
- P3 retained 28 public numerical records matching baseline float bits.
  Discount-identity routing failed the unchanged `1e-8` PV/export gate at
  `1.095759216696024e-8` and was rejected.
- P4 retained ten Hull–White numerical records matching baseline exactly.
  Historically its analytic control slowed 10.17% versus P3 and 20.10% versus
  B0. F1 measured a 3.687% median slowdown with a worst descriptive lower
  endpoint of -8.915%. That reassessment does not explain the historical cost.
- P5 checked 18 fixtures, 24 scenarios, 504 periods and 25 aliases, plus seven
  calibration controls with three frozen PV-budget gates each. Exact-model
  baseline gates covered 273 periods across 11 fixtures; 231 periods across
  seven former-grid fixtures were report-only. Exact references exist for 17
  of 18 fixtures. Genuine recovery dispersion retains unchanged-output policy
  because an exact whole-pool reference is unavailable.
- P5 deliberately changes some former approximate-grid N>17 prices. The
  largest disclosed PV movement is USD `450.62676834710874`; acceptance rests
  on independent exact-reference gates, not B0 bit equality. Standalone BC6
  remained unselected after confirmed N18 controls slowed 19.48%/20.14%.
- P5 Gaussian integration uncertainty is estimated, not a rigorous quadrature
  certificate. Student-t checks validate conditional arithmetic under frozen
  nodes, probabilities and weights, not continuous-factor integration bias.
  Calibration uses `b_i = PV error budget / tranche notional`, with each gate
  at most `2*b_i`; this does not certify Simpson true-error or knot-error bounds.
- P6 retained 17 frozen numerical records and 13 additional edge functions.
  Arithmetic pricing still uses the existing Turnbull–Wakeman approximation.
  Derived dimensions and diagnostic counters are not measured total work.

## Running current checks

The [valuation benchmark guide](../finstack-quant/valuations/benches/README.md)
lists targets. The scoped task supports filtering, smoke execution and named
baselines; its [argument tests](../scripts/tests/test_rust_bench_crate.py)
exercise forwarding and failure behavior without a Rust benchmark build.
For example, this runs one current workload in Criterion smoke mode:

```bash
mise run rust-bench-crate -- finstack-quant-valuations mc_exotics_pricing --filter '^asian_option_mc/paths_50000_fixings_12$' --test
```

The sealed [tranche regression](../finstack-quant/valuations/credit/tests/performance/tranches.rs)
is deliberately slow and ignored by ordinary tests. Run it explicitly against
the retained independent reference fixture:

```bash
mise run rust-test-filter -- finstack-quant-valuations-credit tranche_check_exact_reference --lib --slow
```

The [independent oracle generator](../scripts/golden/valuation_tranche_reference.py)
documents its pinned NumPy/SciPy invocation. It consumes a reviewed Rust
numerical diagnostic export, not the instrument-input fixture directly.
With that reviewed export saved as `reviewed-export.json`, generate separate
candidate outputs without overwriting the sealed fixture:

```bash
uv run --no-project --with scipy==1.18.1 --with numpy==2.5.1 python scripts/golden/valuation_tranche_reference.py --input reviewed-export.json --output target/valuation-oracle/pins.json
```

Full regeneration is expensive and distinct from the slow regression. The
[provenance manifest](valuation-evidence.json) identifies the stored fixtures
and their history. New generator runs write regression pins to the requested
output and the full report to `<output-stem>.report.json` beside it. Pins omit
only the three report fields identified in the manifest; compact formatting
preserves retained numerical values. The original full fixture remains in Git
history. Regenerating current references does not recover missing historical
B0 timings, binaries or logs. Never replace a sealed reference merely to make
a changed pricing implementation pass.
