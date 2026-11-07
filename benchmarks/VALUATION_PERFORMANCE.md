# Valuation benchmark validity and performance record

**Status: B1–B4 and P1–P6 complete; F1 accepted, November 7, 2026.** All 111 frozen cases have fresh cumulative B0/F1 evidence. Of 81 predeclared opportunities, 75 meet the repeatable 10% latency criterion; six are completed below-material outcomes. All 30 controls are reviewed, including two complete original-setting confirmations. The heavy Asian case reduces median process peak RSS by 99.873%; MBS and Bermudan memory are effectively unchanged. Generation, formatting, the full gate, 456 native smoke rows and all 20 release Python/nine release WASM rows passed. Historical slice tradeoffs and rejected variants remain disclosed below.

The compact machine-readable companion is
[`valuation-performance-results.json`](valuation-performance-results.json).
This record follows [ADR 0040](../docs/decisions/standards/0040-measured-performance-decisions.md).
It does not change the separate materialization baselines or their gates.

The JSON is a compact summary: P5/P6 keep absolute process medians, 95%
confidence-interval arrays and per-edge aggregates. Its format notes explain
how to reconstruct paired changes and between-process variability; hash-pinned
assessments and raw summaries retain the complete unmodified evidence.

## Verified validity checkpoint

The work began from clean revision
`5140b499ccec38035a86cf78b21692c449e9d472`. The evidence below describes the
subsequent working tree, not that commit alone. The
[B0 manifest](../target/valuation-benchmarks/B0/manifest.json) identifies 77
archived changed/new source files, eight release benchmark binaries and a
separate credit diagnostic test binary. The source archive includes the record
draft at freeze time; this document continues to track later work. Its current
contents and current source links do not redefine the archived B0 implementation.

| Scope | Verified result | Retained evidence |
| --- | --- | --- |
| Original Rust valuation suite | 359 cases across all 34 declared targets passed checked smoke execution | [Initial inventory](../target/valuation-benchmarks/validity/case-inventory.json), [smoke ledger](../target/valuation-benchmarks/validity/smoke-ledger.json) |
| Adjacent Rust portfolio controls | 25 cases across two targets passed | Same smoke ledger |
| Expanded B4 workloads | 63 new cases; 144 cases across seven affected targets passed; 110 fixture/output captures | [B4 smoke ledger](../target/valuation-benchmarks/B4/smoke-ledger.json), [captures](../target/valuation-benchmarks/B4/captures.json) |
| Combined Rust inventory at B4 freeze | 422 valuation plus 25 portfolio cases; no source-hash mismatches or unexpected skips at reconciliation | [Expanded inventory](../target/valuation-benchmarks/B4/case-inventory.json) |
| Additional calibration target | Nine smoke rows passed; the five-knot full-plan control passed structural checks and has one complete fixture/output capture | [Calibration smoke ledger](../target/valuation-benchmarks/B4/calibration-smoke-ledger.json), [capture](../target/valuation-benchmarks/B4/calibration-capture.json) |
| Release Python / Node-WASM rows | All 20 required Python and nine required WASM rows passed | [Host success ledger](../target/valuation-benchmarks/validity/host-success-ledger.json) |

The combined B4 inventory reconciles earlier successful runs against the benchmark hashes at freeze. F1 subsequently completed one fresh checked pass of all 456 native rows across 37 targets, plus 20 release Python and nine release WASM rows. Smoke and host validity-run durations remain validity evidence, not comparative latency measurements.

Selected Rust operations now require successful finite outputs and the expected
currency, metrics, buckets or cashflows before timing. Timed errors are fatal.
Selection precedes expensive fixture preparation. The
[verification ledger](../target/valuation-benchmarks/validity/verification-ledger.json)
retains invalid-output/error probes, scoped regressions and host verification,
including failed attempts and successful retries. The scoped task's filter,
smoke and named-baseline controls were also exercised.

Baseline correctness repairs are separate from performance work: Asian expiry
reporting and the GBM post-expiry day-count guard; bond-future held-market theta
and physical-delivery horizon rejection; and listed-future observed-mark
extraction without unnecessary CTD market dependencies. Fixture and host
harness repairs supply required fixings, current envelopes/API calls and
complete nonempty outputs. Their old failure-path timings cannot serve as a
before baseline. See the [validity summary](../target/valuation-benchmarks/validity/validity-summary.json)
and [source snapshot](../target/valuation-benchmarks/validity/source-hashes.json).

B4 additionally exposes an optional Rust-only Bermudan MC base-step count so
the requested grid sweep retains contractual exercise events. An omitted value
preserves the prior grid. This is workload capability added before B0, not a
measured speed improvement.

## Workloads and numerical evidence

Exact IDs, models, outputs, dimensions, timing boundaries and available fixture
hashes are in the expanded inventory and captures. The
[retained design notes](../target/valuation-benchmarks/workload-designs/)
explain the pre-candidate numerical bounds; implemented checks and diagnostic
exports identify which parts have actually run. Proposed controls in a design
note do not establish a passing check.

| Slice | Frozen workload scope | Current numerical evidence |
| --- | --- | --- |
| P1 arithmetic Asian MC | `mc_exotics_pricing`: original 2,500-path/two-fixing case, 2,500/10,000/50,000-path and 2/12/52-fixing sweeps, seasoned control | 23 records: seven base fixtures and eight additional seeds for each of fresh and seasoned controls; grid/path identity, paired moments, adjusted standard error and estimator counts |
| P2 MBS OAS | `fi_misc_pricing`: original 512-path/348-month case; 512/2,048/8,192 paths at 360 months, 60/180/360 months at 2,048 paths, payment/seasoning controls | 90 records: ten fixtures × nine seeds; direct objective/root checks, paired path moments, slope-derived root and repricing bounds |
| P3 OIS | `swap_pricing`: 1/2/5/10Y PV/risk plus seasoned, convention, explicit-forward and projection controls | Five records: four tenor PV/par/DV01 references and compact export, missing-fixing, roll and payment-boundary checks |
| P4 Hull–White | `mc_pricing`: retained original case, 5,000/20,000/50,000 paths and 64/128/256 base steps; `rate_exotics_pricing` controls | Ten records: tree refinement reference plus nine MC observations covering six configurations and four seeds; actual grid sizes and uncertainty retained |
| P5 CDS tranches / existing BC6 | `cds_tranche_pricing`: 10/16/17/18/50/125 names; heterogeneous, concentrated, recovery-sensitive and Student-t controls; full-plan five-knot calibration | All three candidate policy tests passed the unchanged independent gates across 18 fixtures/24 scenarios/504 periods; seven contracts passed three frozen calibration gates each. Old approximate-grid price drift is disclosed below |
| P6 commodity Asian | `commodity_pricing`: 12/60/252/1,260 fixings for each method, seasoned and zero/small-volatility controls | 17 records with production call/put values, compensated quadratic references, scale-aware moment/price bounds and parity |

The diagnostic sources and raw exports are:

At the user's request, seven newly added performance diagnostic modules now
live under `tests/performance/`, as recorded in the [relocation manifest](../target/valuation-benchmarks/test-relocation/manifest.json).
At relocation, six parent modules changed only their `cfg(test)` paths and six
diagnostic files were byte-identical; the calibration exporter changed only its relative
`include_str!` path to the same fixture. Test names and numerical semantics are
preserved. Links below name the current owners; historical B0/P1/P2/P3/P4 archive
filenames, source hashes and recorded results identify the original snapshots
and have not been rewritten. All five relocated owning-crate lints passed in
the [relocation check ledger](../target/valuation-benchmarks/test-relocation/check-ledger.json),
and P4 was rebuilt and archived afterward. The current tranche diagnostic also
includes later P5 additions and canonical test-only cleanup. All three P5 variants
passed their numerical checks before cleanup, and the canonical post-cleanup
scoped regression passed. Those additions do not redefine the relocation or B4 evidence.

- P1: [Asian panel](../finstack-quant/valuations/exotics/tests/performance/asian_option.rs), [design](../target/valuation-benchmarks/workload-designs/valuation-b4-mc.md), [23-record export](../target/valuation-benchmarks/B4/asian-accuracy-baseline.json).
- P2: [MBS panel](../finstack-quant/valuations/fixed-income/tests/performance/mc_oas.rs), [design](../target/valuation-benchmarks/workload-designs/valuation-b4-mbs.md), [90-record export](../target/valuation-benchmarks/B4/mbs-accuracy-baseline.json).
- P3: [OIS panel](../finstack-quant/valuations/rates/tests/irs/validation/ois_performance_baseline.rs), [design](../target/valuation-benchmarks/workload-designs/valuation-b4-ois.md), [five-record export](../target/valuation-benchmarks/B4/ois-accuracy-baseline.json).
- P4: [Hull–White panel](../finstack-quant/valuations/rates/tests/performance/bermudan.rs), [design](../target/valuation-benchmarks/workload-designs/valuation-b4-hw.md), [ten-record export](../target/valuation-benchmarks/B4/hw-accuracy-baseline.json).
- P5: [tranche workload](../finstack-quant/valuations/benches/cds_tranche_pricing.rs), [design](../target/valuation-benchmarks/workload-designs/valuation-b4-tranches.md), [tranche diagnostic source](../finstack-quant/valuations/credit/tests/performance/tranches.rs), [actual paired-cap diagnostics](../target/valuation-benchmarks/tranche-b4-diagnostics.json), [independent period ledger](../target/valuation-benchmarks/B4/oracle-periods/ledger.json), [sealed reference fixture](../finstack-quant/valuations/credit/tests/fixtures/valuation_b4_tranche_reference.json), [independent generator](../scripts/golden/valuation_tranche_reference.py).
- P6: [commodity panel](../finstack-quant/valuations/exotics/tests/performance/commodity_asian_option.rs), [design](../target/valuation-benchmarks/workload-designs/valuation-b4-commodity.md), [17-record export](../target/valuation-benchmarks/B4/commodity-accuracy-baseline.json).

The B4 MBS factorization was test-only evidence; P2's accepted validation below
exercises the actual production aggregation helpers. Commodity legacy-moment
replay remains test-only evidence. Numerical exports are untimed; repeated processes
with the same seed do not add independent estimators. Asian physical paths,
duplicate baseline traversals and estimator counts remain distinct. Arithmetic
commodity references preserve the existing Turnbull–Wakeman approximation.

The existing full-plan, five-knot base-correlation calibration benchmark passed
checked smoke execution: all three steps succeeded and validated, all 16 quote
residuals were finite, and the five knots satisfied the checked shape, range
and index-consistency requirements. Its capture is separate from the 110
valuation captures and 447 valuation/portfolio inventory. It is the 111th case
in the [measurement contract](../target/valuation-benchmarks/B4/measurement-contract.json)
and the 26th case in P5's comparison set.

The additional calibration exports also passed. The [calibration-side exporter](../finstack-quant/calibration/tests/performance/base_correlation.rs)
records the five actual prepared quotes and two off-grid controls, their fixed
market and two local sensitivity stencils. Its [baseline export](../target/valuation-benchmarks/B4/calibration-accuracy-baseline.json)
has the same serialized calibration result as the release benchmark capture.
The [credit-side exporter](../finstack-quant/valuations/credit/tests/performance/calibration.rs)
uses the [seven-contract fixture](../finstack-quant/valuations/credit/tests/fixtures/valuation_b4_calibration.json)
to record actual period coefficients and [frozen budgets](../target/valuation-benchmarks/B4/calibration-budget-baseline.json);
all seven normalized PV bit patterns match the calibration export. For each
contract, `b = PV error budget / tranche notional`; both same-curve kernel
differences and total calibrated-output drift must be at most `2b`. The two
stencils use absolute-correlation steps `1e-4` and `5e-5` as local diagnostics,
not certified knot-error bounds. These are test-only baseline additions, not
performance algorithm changes.

Independent tranche oracle processing completed all 504 periods without a
worker failure. The first
[acceptance attempt](../target/valuation-benchmarks/B4/tranche-reference-freeze.log)
reported a conditional expectation error of `5.563e-13` against `5e-13` for
the homogeneous Gaussian 125-name, 40–80% control at June 20, 2028. The checker
had applied the frozen Student-t node bound to Gaussian diagnostic nodes.
The [predeclared design](../target/valuation-benchmarks/workload-designs/valuation-b4-tranches.md)
assigns Gaussian raw expectations a `2e-10` bound and Student-t conditional
arithmetic the separate `5e-13` bound. Correcting that applicability preserved
both contracts. The [successful retry](../target/valuation-benchmarks/B4/tranche-reference-freeze-retry.log)
sealed the references; the [cache verification ledger](../target/valuation-benchmarks/B4/tranche-reference-applicability/rekey-ledger.json)
records unchanged numerical calculations and values for all 504 reused
records. A [final provenance reseal](../target/valuation-benchmarks/B4/tranche-reference-final.log)
followed a generator docstring correction; its [verification ledger](../target/valuation-benchmarks/B4/tranche-reference-applicability/doc-cleanup-rekey-ledger.json)
confirms unchanged code apart from that module docstring and unchanged reference
data. All 273 exact-model periods across 11 fixtures passed their applicable
baseline gates. The 231 periods across seven previous-grid fixtures remain
report-only. Exact references are available for 17 of 18 fixtures; the genuine
50-name recovery-dispersion control retains its unchanged-output policy because
an exact whole-pool reference is unavailable.

Exact completed commands and pass/fail results are retained in the
[B4 check ledger](../target/valuation-benchmarks/B4/check-ledger.json) and
[Asian/MBS command provenance](../target/valuation-benchmarks/B4/asian-mbs-command-provenance.json).
All referenced `target/` files are local retained artifacts, not checked-in
acceptance baselines. Their paths and snapshot hashes are recorded in the JSON.

## B0 archive and initial observations

B0 was frozen before applying P1. The [manifest](../target/valuation-benchmarks/B0/manifest.json)
links the source archive, fixture/evidence hashes, binary hashes and build logs;
the [environment record](../target/valuation-benchmarks/B4/environment.json)
records rustc 1.97.1, macOS 27.0, Mac17,6, 18 physical/logical CPUs and 128 GiB
RAM. Clean timings use the regular release/bench profile: optimization level 3,
fat LTO, one codegen unit and eight Rayon threads. The archived dev-profile
credit test binary is reserved for numerical cross-pricing, never latency.

The [initial summary](../target/valuation-benchmarks/B0/initial-summary.json), [latency request](../target/valuation-benchmarks/B0/before-request.json) and [24-process ledger](../target/valuation-benchmarks/B0/before-request.ledger.json) retain before-only observations for eight workloads, with three independent processes each. Completed matched slice campaigns and the full 111-case F1 matrix are retained separately; the initial observations have not been substituted for fresh matched B0 measurements.

| B0 workload | Median of process medians (ms) | Range of process medians (ms) |
| --- | ---: | ---: |
| Asian MC: 50,000 paths, 12 fixings | 302.421 | 294.356–320.722 |
| MBS OAS: 8,192 paths, 360 months | 72.132 | 71.476–72.248 |
| OIS PV: 10Y | 0.278 | 0.274–0.281 |
| Bermudan LSMC: 20,000 paths, 128 base steps | 2,460.453 | 2,458.989–2,487.685 |
| Issuer-curve tranche: 18 names | 234.453 | 232.366–237.571 |
| Homogeneous Gaussian tranche: 125 names, 40–80% | 71.612 | 71.419–71.747 |
| Commodity arithmetic Asian: 1,260 fixings | 2.523 | 2.521–2.523 |
| Commodity geometric Asian: 1,260 fixings | 0.826 | 0.821–0.832 |

The displayed ranges describe variation between the three processes; they are
not confidence intervals. Each process's separate 95% Criterion confidence
interval remains in the summary and raw ledger. The 18-name B0 tranche uses
the previous approximate grid, whose numerical discrepancy is recorded
separately from its latency.

The [RSS request](../target/valuation-benchmarks/B0/memory-before-retry-request.json)
and [nine-process ledger](../target/valuation-benchmarks/B0/memory-before-retry-request.memory-ledger.json)
use macOS `/usr/bin/time -l` maximum resident set size in bytes, in isolated
selected-case processes with capture disabled. Each process includes the same
case's checked preflight and one operation; unrelated fixture setup is excluded.
Across three processes, peak RSS was 4.681–5.116 GiB for the 50,000-path Asian,
31.297–31.359 MiB for the 8,192-path MBS and 282.359–282.422 MiB for the
50,000-path/128-base-step Bermudan. The [first RSS attempt](../target/valuation-benchmarks/B0/memory-before-request.memory-ledger.json)
failed because the sandbox denied `kern.clockrate` statistics access and
produced no RSS value; the successful approved retry is retained separately.

## P1 accepted: one-pass arithmetic Asian MC

The [accepted decision](../target/valuation-benchmarks/P1/disposition.json)
retains the [Asian pricing change](../finstack-quant/valuations/exotics/src/exotics/asian_option/pricer.rs)
and [paired MC collection](../finstack-quant/models/src/monte_carlo/engine/pricing.rs):
two captured `N`-path simulations become one paired `N`-path traversal, without
retaining trajectories or payoff vectors in this route. The existing grids,
random streams, fixing inputs and supported modes are preserved.

The [P1 manifest](../target/valuation-benchmarks/P1/manifest.json) archives 81
changed/new files and eight release binaries. The [measurement request](../target/valuation-benchmarks/P1/measurement-request.json),
[48-process ledger](../target/valuation-benchmarks/P1/measurement-request.ledger.json)
and [comparison summary](../target/valuation-benchmarks/P1/measurement-request.summary.json)
cover eight cases, three independent repetitions and alternating B0/P1 order.
B0 is also the preceding accepted implementation for this first slice, so its
incremental and cumulative comparisons coincide. These are fresh matched B0
runs; the earlier before-only observations are not used to calculate these gains.

| Matched workload | B0 median (ms) | P1 median (ms) | Median latency reduction |
| --- | ---: | ---: | ---: |
| Original Asian: 2,500 paths, two fixings | 24.134 | 10.994 | 54.45% |
| Asian: 2,500 paths, 12 fixings | 29.293 | 13.129 | 55.18% |
| Asian: 10,000 paths, 12 fixings | 71.374 | 28.962 | 59.42% |
| Asian: 50,000 paths, 12 fixings | 319.635 | 119.903 | 62.49% |
| Asian: 10,000 paths, two fixings | 60.631 | 24.231 | 60.03% |
| Asian: 10,000 paths, 52 fixings | 93.753 | 40.478 | 56.82% |
| Seasoned Asian: 10,000 paths, 12 fixings, six past | 36.431 | 13.433 | 63.13% |
| Lookback control: 2,500 paths | 12.977 | 13.118 | −1.09% |

Medians are medians of three process medians. Every Asian process pair exceeds
the 10% materiality target; each pair's descriptive marginal-CI endpoint range
also stays above 10%. Those endpoint ratios are not a calibrated confidence
interval for the speedup. The summary separately retains each process's 95%
Criterion interval, the between-process range and standard deviation, and each
observed paired reduction. The lookback control is 1.09% slower by the median
ratio, without a material regression in any pair.

The [matched RSS request](../target/valuation-benchmarks/P1/memory-request.json)
and [six-process ledger](../target/valuation-benchmarks/P1/memory-request.memory-ledger.json)
measure the 50,000-path/12-fixing Asian with the same isolated-process method
described above. B0 peak RSS was 4,786.391–4,878.891 MiB (median 4,857.953 MiB);
P1 was 5.953–6.047 MiB (median 6.016 MiB). The median process-level peak-RSS
reduction is 99.876%; this is distinct from an allocation counter or a profiler
memory estimate.

The [23-record numerical comparison](../target/valuation-benchmarks/P1/baseline-numerical-comparison.json)
confirms unchanged fixtures, configuration, grid and frozen bounds; maximum
absolute price difference is `7.99e-14`. The actual paired moments and estimator
checks pass in the [candidate accuracy export](../target/valuation-benchmarks/P1/asian-accuracy.json).
The [scoped check ledger](../target/valuation-benchmarks/P1/check-ledger.json)
records passing owning suites (models: 2,065; exotics: 300), 12 umbrella
consumer tests, focused checks and the ignored accuracy panel. These overlapping
test runs are not summed as unique coverage. The [release smoke ledger](../target/valuation-benchmarks/P1/smoke-ledger.json)
records all 16 target cases passing with unchanged fixtures; the nine non-Asian
outputs are bitwise unchanged. A test-only clone spelling correction resolved
the initial lint failure without changing the numerical algorithm.

## P2 accepted: aggregate MBS cashflows before solving OAS

The [accepted decision](../target/valuation-benchmarks/P2/disposition.json)
retains the [MBS OAS change](../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/mc_oas.rs):
project cashflows once per path into monthly coefficients, then evaluate each
Brent trial with one spread factor per nonzero month. Deterministic accumulation
uses at most 32 monthly scratch blocks, without another paths-by-months matrix.
The financial conventions, simulation and solver budgets remain unchanged.

The [P2 manifest](../target/valuation-benchmarks/P2/manifest.json) archives 81
changed/new files and eight release binaries. The initial release build reused
stale artifacts because of source timestamps. This was detected before any P2
timing; the [check ledger](../target/valuation-benchmarks/P2/check-ledger.json)
retains those build logs as invalid for candidate timing, followed by corrected
builds. All measured binary hashes match the corrected archive.

The [request](../target/valuation-benchmarks/P2/measurement-request.json),
[108-process ledger](../target/valuation-benchmarks/P2/measurement-request.ledger.json)
and [summary](../target/valuation-benchmarks/P2/measurement-request.summary.json)
cover twelve cases with three independent processes for each of B0, P1 and P2,
reversing process order between repetitions. Both incremental P1 comparisons
and cumulative B0 comparisons use these matched runs.

| Matched MBS workload | B0 (ms) | P1 (ms) | P2 (ms) | Reduction vs P1 | Reduction vs B0 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Original OAS: 512 paths, 348 months | 4.536 | 4.543 | 2.749 | 39.50% | 39.40% |
| OAS: 512 paths, 360 months | 4.669 | 4.703 | 2.860 | 39.19% | 38.74% |
| OAS: 2,048 paths, 360 months | 17.986 | 17.900 | 10.731 | 40.05% | 40.34% |
| OAS: 8,192 paths, 360 months | 71.830 | 71.430 | 42.234 | 40.87% | 41.20% |
| OAS: 2,048 paths, 60 months | 2.990 | 2.986 | 1.839 | 38.40% | 38.49% |
| OAS: 2,048 paths, 180 months | 8.748 | 8.595 | 5.421 | 36.94% | 38.03% |
| GNMA I: 2,048 paths, 360 months | 18.040 | 18.033 | 10.716 | 40.57% | 40.60% |
| 15-day delay: 2,048 paths, 360 months | 18.024 | 18.002 | 10.729 | 40.40% | 40.47% |
| Fresh ACT/360: 2,048 paths, 300 months | 15.131 | 15.142 | 8.946 | 40.92% | 40.87% |
| Seasoned ACT/360: 2,048 paths, 300 months | 15.111 | 15.174 | 8.958 | 40.97% | 40.72% |
| Passthrough PV control | 0.153 | 0.152 | 0.153 | −1.20% | −0.03% |
| Duration/convexity control | 0.625 | 0.626 | 0.634 | −1.20% | −1.41% |

Values are medians of three process medians. For every OAS workload, every
paired reduction and every descriptive marginal-CI endpoint range exceeds 10%
against both comparators. The summary keeps the separate within-process
Criterion intervals and between-process variability; endpoint ratios are not
calibrated confidence intervals for reductions. Neither control shows a
material regression, and the incremental and cumulative gains are not added.

The [RSS request](../target/valuation-benchmarks/P2/memory-request.json) and
[nine-process ledger](../target/valuation-benchmarks/P2/memory-request.memory-ledger.json)
use the same isolated selected-case method for 8,192 paths and 360 months.
Median peak RSS was 31.313 MiB for B0, 31.266 MiB for P1 and 31.250 MiB for
P2. This is no material memory change; latency is the accepted benefit.

The [90-record numerical comparison](../target/valuation-benchmarks/P2/baseline-numerical-comparison.json)
and [candidate accuracy export](../target/valuation-benchmarks/P2/mbs-accuracy.json)
exercise the production helpers against the direct reference across ten
fixtures and nine seeds. Fixtures, grids, model/path hashes, seeds, estimator
counts and all frozen bounds match B0. Maximum baseline solved-root difference
is `9.58e-16` in decimal spread; maximum fixed-spread PV difference is
`1.44e-8`. Fixed-spread, paired-moment, root, repricing and public/helper
agreement gates passed.

Scoped validation records 1,245 owning fixed-income tests, 60 focused tests,
90 numerical cases and consumer groups of 61, one and one passing, without
summing overlapping runs. The owning run included a nonfatal nextest
leaked-handle diagnostic on
`effective_duration_uses_single_valuation_date_oas_quote`; its test passed and
the raw log is retained in the check ledger. The [release smoke ledger](../target/valuation-benchmarks/P2/smoke-ledger.json)
records 31 passing cases and twelve captures with unchanged fixtures.

## P3 accepted: omit internal projected-fixing metadata

The [accepted P3 decision](../target/valuation-benchmarks/P3/disposition.json)
retains only the metadata change in [IRS cashflows](../finstack-quant/valuations/rates/src/irs/cashflow.rs)
and [pricing](../finstack-quant/valuations/rates/src/irs/pricer.rs).
Internal PV schedules skip constructing `ProjectedFixing` records and their
series-ID strings. The canonical projector still requests observation exposures
and performs the same complete daily calculation; public exports retain all
fixings and metadata. The [static review](../target/valuation-benchmarks/P3/static-review.md)
confirms the shared overnight projector is byte-identical to its pre-P3 source.

The initial discount-identity variant was [rejected before timing](../target/valuation-benchmarks/P3/rejected-identity/disposition.json)
after the existing PV/export check failed at `1.095759216696024e-8` against its
unchanged `1e-8` bound. A direct-versus-reciprocal ratio probe produced the same
rounded factor. All shared-projector changes were restored; no gain is attributed
to that rejected variant.

The [P3 manifest](../target/valuation-benchmarks/P3/manifest.json) archives 84
changed/new files and eight freshly built release binaries. Its [request](../target/valuation-benchmarks/P3/measurement-request.json),
[180-process ledger](../target/valuation-benchmarks/P3/measurement-request.ledger.json)
and [summary](../target/valuation-benchmarks/P3/measurement-request.summary.json)
cover twenty cases with three independent processes per B0, P2 and P3, reversing
order between repetitions. Table values are medians of those process medians;
the time unit here is microseconds.

| Matched OIS workload | B0 (µs) | P2 (µs) | P3 (µs) | Reduction vs P2 | Reduction vs B0 |
| --- | ---: | ---: | ---: | ---: | ---: |
| PV: 1Y | 27.716 | 27.849 | 18.960 | 31.92% | 31.59% |
| PV: 2Y | 54.901 | 54.709 | 37.245 | 31.92% | 32.16% |
| PV: 5Y | 136.706 | 136.564 | 95.626 | 29.98% | 30.05% |
| PV: 10Y | 273.013 | 270.320 | 186.339 | 31.07% | 31.75% |
| Standard risk: 1Y | 143.966 | 143.500 | 101.018 | 29.60% | 29.83% |
| Standard risk: 2Y | 282.520 | 282.423 | 197.529 | 30.06% | 30.08% |
| Standard risk: 5Y | 708.577 | 705.487 | 495.498 | 29.77% | 30.07% |
| Standard risk: 10Y | 1,382.316 | 1,399.964 | 978.478 | 30.11% | 29.21% |
| Seasoned PV: 5Y | 133.859 | 135.730 | 93.476 | 31.13% | 30.17% |
| Two-day lookback PV: 5Y | 291.768 | 292.741 | 249.724 | 14.69% | 14.41% |
| Two-day observation shift PV: 5Y | 293.444 | 294.611 | 248.460 | 15.66% | 15.33% |
| Two-day cutoff PV: 5Y | 138.841 | 137.486 | 93.419 | 32.05% | 32.71% |
| Arithmetic 10bp spread PV: 5Y | 137.118 | 137.343 | 94.339 | 31.31% | 31.20% |
| Explicit-forward PV: 5Y | 172.050 | 169.922 | 111.876 | 34.16% | 34.97% |
| Discount-derived-forward PV: 5Y | 171.700 | 171.861 | 115.744 | 32.65% | 32.59% |
| Seasoned risk: 5Y | 691.115 | 695.232 | 476.197 | 31.51% | 31.10% |
| Explicit-forward risk: 5Y | 862.403 | 863.409 | 580.388 | 32.78% | 32.70% |
| Term-swap PV control: 5Y | 14.224 | 14.422 | 14.705 | −1.96% | −3.38% |
| Full cashflow export control: 5Y | 144.695 | 144.360 | 144.484 | −0.09% | 0.15% |
| Compounded 10bp spread projector control: 1Y | 17.815 | 17.997 | 17.554 | 2.46% | 1.46% |

Every affected workload exceeds the 10% latency target against both B0 and P2
in all three process pairs and descriptive marginal-CI endpoint ranges. The
three unaffected controls have no material regression. The summary retains
within-process confidence intervals and between-process variability separately;
the endpoint ranges are not calibrated confidence intervals for ratios. P3 is
not a heavy MC slice and makes no RSS improvement claim.

The [numerical comparison](../target/valuation-benchmarks/P3/baseline-numerical-comparison.json)
confirms all 28 public records are bitwise equal: 23 from the preproduction
reference checks and five from B4. Both private coupon-amount errors are zero.
The [successful validation snapshot](../target/valuation-benchmarks/P3/latest-check-summary.json)
and [check ledger](../target/valuation-benchmarks/P3/check-ledger.json) identify
the source snapshots, 973 passing rates tests and all scoped consumer groups.
These counts are not added to the overlapping focused checks. The ledger
preserves both the rejected variant's Rust failure and a later checker
`KeyError` caused by serialization omitting an empty `projected_fixings` field.
The checker was corrected to require that omission; no Rust source or numerical
bound changed in that correction, and its retry passed.

The [release smoke ledger](../target/valuation-benchmarks/P3/smoke-ledger.json)
records 35 passing cases and twenty captures with unchanged fixture and output
payloads. The public export control therefore remains intact alongside the
internal metadata omission.

## P4 accepted with tradeoff: prepare Hull–White interval coefficients

The [P4 decision](../target/valuation-benchmarks/P4/disposition.json) retains
three production-file changes: the [exact stepper](../finstack-quant/models/src/monte_carlo/discretization/exact_hw1f.rs),
[shared HW path driver](../finstack-quant/models/src/rates/hw1f/mc.rs) and
[swaption LSMC driver](../finstack-quant/valuations/rates/src/swaption/pricing/monte_carlo_lsmc.rs).
Exact interval coefficients are prepared once per grid and reused across paths.
Coefficient arithmetic, random draws, antithetic ordering and event ordering
remain unchanged. Scalar-cache reuse also checks the process parameters and
clears on empty-grid preparation. The [P4 manifest](../target/valuation-benchmarks/P4/manifest.json)
archives 87 changed/new source files and eight rebuilt release binaries.

The [original request](../target/valuation-benchmarks/P4/measurement-request.json)
and [135-process raw ledger](../target/valuation-benchmarks/P4/measurement-request.ledger.json)
cover 15 cases with three B0/P3/P4 repetitions, reversing order between
repetitions. Completion-wall anomalies affected the 50,000-path/base-128
case's second repetition. Its entire three-revision triplet was replaced using
[three fresh processes](../target/valuation-benchmarks/P4/interruption-replacement-request.ledger.json),
selected from timing metadata independently of performance outcomes. The
[qualification provenance](../target/valuation-benchmarks/P4/qualification-provenance.json)
retains all original rows and maps 132 retained originals plus three replacements
to the [135-row qualified ledger](../target/valuation-benchmarks/P4/qualified-measurement-request.ledger.json).
Completion-wall proxies do not establish a cause or prove latency distortion.

The [qualified summary](../target/valuation-benchmarks/P4/qualified-measurement-request.summary.json)
supplies these fourteen rows. The remaining analytic control received a complete
separate confirmation campaign, reported below. Values are medians of three
process medians; reductions against P3 and B0 are separate comparisons.

| Matched workload | B0 (ms) | P3 (ms) | P4 (ms) | Reduction vs P3 | Reduction vs B0 |
| --- | ---: | ---: | ---: | ---: | ---: |
| MBS OAS control | 4.539 | 2.761 | 2.751 | 0.39% | 39.40% |
| HW tree: calibrate each price | 1.207 | 1.211 | 1.206 | 0.36% | 0.04% |
| HW tree: precalibrated, 100 steps | 1.079 | 1.081 | 1.076 | 0.39% | 0.23% |
| Original LSMC: 50,000 paths | 5,559.293 | 5,577.416 | 5,467.408 | 1.97% | 1.65% |
| LSMC: 5,000 paths, base 128 | 608.761 | 610.668 | 564.021 | 7.64% | 7.35% |
| LSMC: 20,000 paths, base 128 | 2,479.349 | 2,472.616 | 2,281.099 | 7.75% | 8.00% |
| LSMC: 50,000 paths, base 128 | 6,256.868 | 6,235.768 | 5,702.082 | 8.56% | 8.87% |
| LSMC: 20,000 paths, base 64 | 2,271.985 | 2,277.540 | 2,214.461 | 2.77% | 2.53% |
| LSMC: 20,000 paths, base 256 | 2,966.650 | 2,986.549 | 2,331.961 | 21.92% | 21.39% |
| Quanto range-accrual analytic control | 0.007758 | 0.007595 | 0.008121 | -6.92% | -4.67% |
| Quanto range-accrual GBM control | 9.014 | 8.963 | 9.067 | -1.16% | -0.58% |
| Callable range accrual: 2,500 paths | 12.338 | 12.311 | 8.675 | 29.54% | 29.69% |
| Snowball: 2,500 paths | 8.297 | 8.266 | 3.967 | 52.01% | 52.19% |
| TARN: 2,500 paths | 15.143 | 14.962 | 6.021 | 59.76% | 60.24% |

The dense base-256 case and three HW rate-note cases exceed the 10% target
against both comparators in every process pair and descriptive marginal-CI
endpoint range. Other Bermudan MC rows improve approximately 2–9% against P3,
below materiality. The MBS control's cumulative gain comes from the earlier P2
change; its 0.39% incremental movement is not a P4 material gain. Within-process
Criterion intervals and between-process variability remain separate in the
summaries. Marginal-CI endpoint ratios are not calibrated intervals for speedup.

The [analytic-control confirmation request](../target/valuation-benchmarks/P4/control-confirmation-request.json),
[nine-process ledger](../target/valuation-benchmarks/P4/control-confirmation-request.ledger.json)
and [summary](../target/valuation-benchmarks/P4/control-confirmation-request.summary.json)
repeat all three revisions under the original settings. This complete campaign
supplies the final control assessment; its samples are not pooled with the
primary campaign.

| Confirmed analytic control | B0 (µs) | P3 (µs) | P4 (µs) | Increase vs P3 | Increase vs B0 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Range accrual: 12 monthly observations | 6.239 | 6.802 | 7.493 | 10.17% / 0.692 µs | 20.10% / 1.254 µs |

The control is slower versus B0 in all three pairs by 15.06–20.10%; its
incremental pair increases are 8.50–11.25%. Static tracing found no changed HW
kernel reachable from this analytic pricing route, and captured price bits are
unchanged. **The timing cause remains unexplained.** Acceptance is an explicit
MC/rate-note workload tradeoff: the dense case saves about 0.65 seconds against
P3, and the three rate notes save several milliseconds per call. No workload
frequency or weighted portfolio benefit is assumed. F1 freshly rechecked this control: its median is 3.687% slower than B0, with all three point reductions and descriptive lower endpoints above −10%. The historical regression remains disclosed; no cause is inferred from the different F1 result.

The [RSS request](../target/valuation-benchmarks/P4/memory-request.json) and
[nine-process ledger](../target/valuation-benchmarks/P4/memory-request.memory-ledger.json)
use the unchanged isolated selected-case method for 50,000 paths/base 128,
with capture disabled. Median peak RSS is 282.406 MiB for B0 and P3, and
282.391 MiB for P4. Ranges are 282.391–282.438 MiB, 282.391–287.594 MiB and
282.359–282.406 MiB respectively. There is no material memory improvement.

The [ten-record HW export](../target/valuation-benchmarks/P4/hw-accuracy.json)
is byte-identical to B4, including inputs, tree references, MC prices,
uncertainty and diagnostic bits. The [check ledger](../target/valuation-benchmarks/P4/check-ledger.json)
records 2,069 models, 974 rates and 300 exotics tests passing, plus focused,
numerical and consumer checks; overlapping counts are not added. An initial
numerical command exited successfully, but tracked source changed during its
run, so the provenance checker rejected that evidence. Repeated checks passed
with a stable snapshot and unchanged bounds. The
[release smoke ledger](../target/valuation-benchmarks/P4/smoke-ledger.json)
records fifteen passing cases and fifteen captures with identical fixture and
output values.

## P5 accepted: exact count distribution plus Gaussian paired integration

The [P5 decision](../target/valuation-benchmarks/P5/disposition.json) selects
**P5-combined** after count, BC6 and combined were validated separately against
the same P4 predecessor. All 21 predeclared combined opportunities exceed the
10% latency target versus both B0 and P4 in all three paired point estimates
and descriptive marginal-CI endpoint lower bounds. The five common controls
have no repeatable regression over 10%. Standalone BC6 is not selected because
two issuer-curve N18 controls retain confirmed regressions, disclosed below.

The exact count recurrence applies only to eligible pools with exactly equal
positive contractual weights and recoveries. It retains each issuer's distinct
default probability and the actual common contractual weight, without replacing
that weight with `1/N`. Gaussian homogeneous-index pair integration retains
current precursor behavior; the BC6 label does not establish BC1–BC5 completion
or a benefit after a future prerequisite change.

The [case roles](../target/valuation-benchmarks/P5/pre-timing-case-roles.json)
were fixed before any P5 timing: ten count opportunities, eleven BC6
opportunities, their 21-case combined union and five common controls. Issuer
pools retain scalar integration and are BC6 controls. Standalone jump-to-default
performs no factor integration. Combined versus count isolates the BC6 role
set; combined versus BC6 isolates the count role set.

### Matched latency and confirmation

The [short request](../target/valuation-benchmarks/P5/expedited-request.json),
[390-process ledger](../target/valuation-benchmarks/P5/expedited-request.ledger.json)
and [summary](../target/valuation-benchmarks/P5/expedited-request.summary.json)
completed all 26 cases across B0, P4, count, BC6 and combined, with three
independent processes per revision. Five whole cases were
[selected for original-setting confirmation](../target/valuation-benchmarks/P5/confirmation-selection.json):
junior-mezz, senior-mezz and senior NPV, plus issuer N18 NPV and correlation
delta. The complete [75-process confirmation ledger](../target/valuation-benchmarks/P5/original-confirmation-01.ledger.json)
and [summary](../target/valuation-benchmarks/P5/original-confirmation-01.summary.json)
supply the final assessment for those five cases; the other 21 use the complete
short campaign. Both campaigns passed. No within-case pooling, individual
repetition substitution or favorable-protocol selection is used.

Both [short](../target/valuation-benchmarks/P5/clock-audit.json) and
[confirmation](../target/valuation-benchmarks/P5/clock-audit-original-confirmation-01.json)
clock metadata audits are clear at the retained five-second threshold. These
checks do not establish a cause for any timing movement. The
[case-level assessment](../target/valuation-benchmarks/P5/assessment.json)
retains all process medians, within-process 95% Criterion intervals,
between-process ranges and standard deviations, paired reductions and all
eight comparison edges. Those edges share the same three independent processes
per revision; they are not eight independent experiments. Marginal-CI endpoint
ratios are descriptive ranges, not calibrated confidence intervals for ratios.

All table latencies are medians of three process medians in milliseconds.
`†` identifies a complete original-setting confirmation; all other rows use
short timing. Negative reductions mean slower execution. Names distinguish
issuer equal-exposure count opportunities from homogeneous-index BC6 cases.

| Matched workload | B0 (ms) | P4 (ms) | Count (ms) | BC6 (ms) | Combined (ms) | Combined reduction vs P4 | vs B0 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Index NPV: equity 0–3% | 35.825 | 36.324 | 35.420 | 23.991 | 24.021 | 33.87% | 32.95% |
| Index NPV: junior mezz 3–7% † | 46.743 | 46.853 | 46.441 | 26.044 | 26.004 | 44.50% | 44.37% |
| Index NPV: senior mezz 7–10% † | 48.223 | 48.802 | 47.517 | 26.375 | 26.392 | 45.92% | 45.27% |
| Index NPV: senior 10–15% † | 49.136 | 48.860 | 47.639 | 26.031 | 26.002 | 46.78% | 47.08% |
| Index CS01 | 337.940 | 335.925 | 333.105 | 273.333 | 272.793 | 18.79% | 19.28% |
| Index correlation delta | 95.508 | 95.642 | 93.326 | 52.250 | 52.224 | 45.40% | 45.32% |
| Standalone JTD control | 0.000122 | 0.000121 | 0.000121 | 0.000122 | 0.000121 | -0.13% | 0.96% |
| Index par spread | 47.825 | 47.264 | 46.572 | 26.096 | 26.102 | 44.77% | 45.42% |
| Index all metrics | 481.622 | 482.207 | 474.809 | 350.303 | 351.272 | 27.15% | 27.06% |
| Homogeneous Student-t control | 12.208 | 11.992 | 11.841 | 11.840 | 11.841 | 1.26% | 3.01% |
| Issuer NPV: N10 | 82.113 | 80.356 | 7.079 | 80.537 | 7.113 | 91.15% | 91.34% |
| Issuer NPV: N16 | 7,782.285 | 7,670.122 | 12.341 | 7,674.545 | 12.395 | 99.84% | 99.84% |
| Issuer NPV: N17 | 16,009.425 | 15,831.156 | 13.429 | 15,809.439 | 13.389 | 99.92% | 99.92% |
| Issuer NPV: N18 † | 229.988 | 226.666 | 14.527 | 270.820 | 14.492 | 93.61% | 93.70% |
| Issuer NPV: N50 | 420.098 | 431.526 | 66.671 | 419.734 | 68.309 | 84.17% | 83.74% |
| Issuer NPV: N125 | 2,878.925 | 2,880.386 | 330.283 | 2,957.914 | 336.136 | 88.33% | 88.32% |
| Issuer correlation delta: N18 † | 460.868 | 445.159 | 29.131 | 534.821 | 28.968 | 93.49% | 93.71% |
| Issuer correlation delta: N125 | 5,758.242 | 5,753.926 | 672.068 | 5,963.897 | 675.551 | 88.26% | 88.27% |
| Unequal-exposure four-name control | 6.097 | 6.058 | 6.097 | 6.087 | 6.085 | -0.44% | 0.19% |
| Concentrated N50 control | 1,155.346 | 1,157.087 | 1,164.551 | 1,228.037 | 1,157.210 | -0.01% | -0.16% |
| Recovery-dispersion N50 control | 485.482 | 536.949 | 459.331 | 503.445 | 492.399 | 8.30% | -1.42% |
| Issuer equal-exposure N125: 40–80% | 4,239.001 | 4,270.723 | 490.410 | 4,403.446 | 488.811 | 88.55% | 88.47% |
| Homogeneous index N125: 40–80% | 68.929 | 70.572 | 68.835 | 37.032 | 36.879 | 47.74% | 46.50% |
| Homogeneous index N125: 60–100% | 55.186 | 54.935 | 53.980 | 36.279 | 36.271 | 33.97% | 34.28% |
| Student-t equal-exposure N50 | 221.587 | 227.950 | 39.293 | 218.748 | 39.526 | 82.66% | 82.16% |
| Full-plan five-knot calibration | 8,304.421 | 8,271.100 | 8,276.882 | 4,826.195 | 4,809.499 | 41.85% | 42.09% |

Count's ten opportunities improve 82.76–99.92% versus P4 (82.27–99.92%
versus B0). BC6's eleven targeted opportunities improve 18.63–47.53% versus
P4; standalone full calibration improves 41.65%. The selected combined build
improves full calibration **41.85% versus P4** and 42.09% versus B0. These
are measured end-to-end results; component percentages are not added.

**Standalone BC6 is not selected.** Under complete original-setting
confirmation, issuer N18 NPV is 19.48% slower than P4 and issuer N18 correlation
delta is 20.14% slower. Each control regresses over 10% in all three paired
point estimates versus both P4 and B0. Independent static comparison found
unchanged production logic on this BC6-only issuer scalar route, including
preparation and `adaptive_simpson`; **the build-level timing cause is unknown**.
No LTO, code-layout or clock cause is asserted.

The Gaussian paired kernel is retained only in the separately validated
combined configuration. Actual combined N18 NPV and correlation-delta latency
reductions are 93.61% and 93.49% versus P4, but only 0.25% and 0.56% versus
count. Those large N18 gains belong to the exact-count route and must not be
attributed to BC6. Combined versus either component has no control point
regression over 10% after the complete confirmations. The retained decision
uses actual combined measurements, without assuming workload frequencies.

The common recovery-dispersion control is 1.42% slower than B0 and 7.20%
slower than count, while 8.30% faster than P4. All are below the 10% materiality
threshold. No mechanism-specific gain is claimed for that ineligible route.
No P5 peak-RSS benefit is claimed.

### Numerical results preserve the independent gates, not every old price

Each variant's retained diagnostics cover 18 fixtures, 24 scenarios and 504 periods, with 25 benchmark aliases. The corresponding successful Rust policy test supplies the numerical acceptance; the small retention report verifies diagnostic identity, immutable fields and provenance. That retention report is not another independent repricing run, and a policy label alone is not a compiled dispatch trace. The frozen independent references and numerical gates remain unchanged.

Count and combined deliberately differ from the old approximate loss grid on some equal-exposure N=125 cases. The largest absolute PV movement in their retained scenario comparisons is **USD 450.62676834710874**, for the 125-name correlation-delta workload's correlation-down scenario: PV moves from −1,478,654.089492688 to −1,479,104.716261035. The equal-exposure Gaussian 125-name 40–80% case in its base scenario moves from 2,148,025.07133545 to 2,148,205.818113623, an increase of **USD 180.74677817337215**. Despite its benchmark group's name, this case is a predeclared count opportunity. These are intentional departures from the previous approximation, accepted against the unchanged independent exact-reference gates. They are not evidence of bitwise equality to B0, nor a reason to enlarge a frozen tolerance.

N≤17 exact-model comparisons retain their distinct before/after and independent-reference gates; a different summation order need not be bit-identical. Unchanged genuine-heterogeneity routes remain subject to their separate unchanged-output controls. The count change's Student-t result is accuracy on the unchanged factor rule; it does not certify continuous-factor integration accuracy. The previously unavailable exact whole-pool reference for the genuine recovery-dispersion control remains unavailable.

For BC6 alone, the largest absolute PV movement in the same small scenario-comparison report is USD 0.0005908804014325142 on the equity 0–3% base case. This observed drift is distinct from the accepted error bounds. The diagnostic work counters refer to separate untimed diagnostic invocations; paired-cap and separate-scalar counters must not be added into a supposed whole-price work count. Dimensions derived from other counters are not directly measured work.

Evidence: [count retention report](../target/valuation-benchmarks/P5-count/checks/tranche-evidence-report.json), [BC6 retention report](../target/valuation-benchmarks/P5-bc6/checks/tranche-evidence-report.json), [combined retention report](../target/valuation-benchmarks/P5-combined/checks/tranche-evidence-report.json). These reports point to the large preserved diagnostics; those large exports were not reprocessed for this publication.

### Seven-control calibration comparison

All seven frozen controls—five actual quoted contracts and two off-grid controls—passed all three four-way comparison gates for each variant. The four values are the baseline and candidate pricing kernels evaluated at both the baseline and candidate calibrated curves. The checks separately bound the kernel difference at each curve and the total calibrated-output drift.

For contract i, the frozen normalized allowance is `b_i = PV error budget / tranche notional`, with each same-curve difference and the total calibrated-output drift required to be at most `2*b_i`. This is normalized by **tranche notional**, not pool notional. These output-drift allowances are conditional on the sealed cap-error contract, with the fixed upfront scale, absolute tiny-row suppression and roundoff treatment preserved. They are **not a certified true-error bound for Simpson integration**. Separately, they are not a certified error bound for the fitted correlation knots or a substitute for the independent pricing gates. The calibration solver's existing fit-acceptance tolerance is not the numerical-accuracy gate for this work. The retained local sensitivity stencils remain diagnostics, not certificates or acceptance gates.

Count has exactly zero recorded differences across the three comparison gates for all seven contracts and zero knot displacement. BC6 and combined have identical summarized maxima: `6.780362382663441e-11` at the baseline curve, `6.78037903600881e-11` at the candidate curve and `8.899891934532889e-11` for total calibrated-output drift, all in normalized PV. Their largest absolute observed knot displacement is `1.9031712317207905e-10`; that observation is not a knot-error guarantee. The fixtures were restored after cross-pricing, prerequisite discount/hazard curves stayed unchanged and the validation contract was preserved.

Evidence: [count comparison](../target/valuation-benchmarks/P5-count/calibration-cross-pricing/comparison.json), [BC6 comparison](../target/valuation-benchmarks/P5-bc6/calibration-cross-pricing/comparison.json), [combined comparison](../target/valuation-benchmarks/P5-combined/calibration-cross-pricing/comparison.json).

### Scoped tests, release smoke and preserved failures

| Configuration | Owning suites passed | Focused groups passed | Separate exact-reference panel |
| --- | --- | --- | --- |
| Count | Credit: 228 | Credit count route: 11 | One selected policy test |
| BC6 | Core: 2,210; credit: 226 | Core: 14; credit: 10 and 1 | One selected policy test |
| Combined | Core: 2,210; credit: 235 | Count: 11; core: 14; credit: 10 and 1 | One selected policy test |

Each configuration also passed consumer groups of 91 ordinary umbrella tranche tests, six slow umbrella tranche tests, 15 calibration/base-correlation tests and one full-credit-desk calibration integration test. The calibration-control exporter and PV-budget exporter each selected one passing test. These runs overlap owning/focused coverage and must not be summed as unique tests.

All three configurations built and archived the same eight release benchmark targets. Each passed 25 tranche smoke cases with 25 captures, plus nine calibration smoke rows with one full-plan calibration capture. The separate 25-capture report checks fixture comparability only: it normalizes the declared `fixture.dimensions.branch_basis` metadata in memory. It leaves captured outputs and the frozen 111-case contract unchanged, and does not independently accept numerical values. The full-plan calibration control is separate from those 25 tranche captures. Smoke durations are not comparative latency measurements.

The archive source inventories contain 772 files for count, 773 for BC6 and 775 for combined. These counts include complete observed financial-Markdown snapshots under the new archival policy; they are not counts of production files changed by P5. The eight binaries are recorded separately in each manifest. Binary hashes are retained from the manifests; no binary was rehashed for this publication.

Historical failed attempts remain visible. The initial count preflight stopped before tests when unrelated financial Markdown appeared outside its then-allowed source set. A subsequent credit lint failed on the exact floating-point comparisons used for contractual weight/recovery eligibility. The retained lint fix preserves exact equality, including normal IEEE signed-zero equality; it does not introduce an approximation or change runtime semantics. A later documentation-only source-drift guard stopped the count run; successful completed commands were retained, then the remaining `consumers-04` credit-desk example, two calibration exports and diagnostic metadata retention completed after executable inputs were verified unchanged. These failures were not numerical tolerance failures and were not erased.

Current checks fingerprint the unrelated financial Markdown before and after commands while keeping executable/configuration/fixture inputs strict. Build/archive source identity additionally fixes HEAD, bytes, modes and deletions for all guarded inputs. Only the reviewed `docs/decisions/financial/**/*.md` namespace and the pre-existing evolving performance-report build exclusions receive their declared treatment. Financial documents are archived from captured bytes with explicit before/captured/after observations, and the supplemental patch explicitly identifies its exclusions. Existing archives remain immutable. This guard change prevents unrelated prose edits from forcing a release rebuild; it does not relax executable-source or numerical validation.

Performance diagnostics retain the requested `tests/performance/` locations. The P5 count tests are under `credit/tests/performance/count.rs`, and common diagnostic retention extends `credit/tests/performance/tranches.rs`. The seven earlier diagnostic relocations and their historical B0/P1/P2/P3/P4 filenames/hashes remain preserved; current source links and later P5 additions must not be substituted into historical archives.

Primary check evidence: [count provenance](../target/valuation-benchmarks/P5-count/checks/provenance.json), [BC6 provenance](../target/valuation-benchmarks/P5-bc6/checks/provenance.json), [combined provenance](../target/valuation-benchmarks/P5-combined/checks/provenance.json). Each variant's retained archive and smoke records remain separate:

| Variant | Release archive | Smoke ledger | Capture comparability |
| --- | --- | --- | --- |
| Count | [Manifest](../target/valuation-benchmarks/P5-count/manifest.json) | [25 tranche + nine calibration](../target/valuation-benchmarks/P5-count/smoke-ledger.json) | [25 fixtures](../target/valuation-benchmarks/P5-count/tranche-capture-report.json) |
| BC6 | [Manifest](../target/valuation-benchmarks/P5-bc6/manifest.json) | [25 tranche + nine calibration](../target/valuation-benchmarks/P5-bc6/smoke-ledger.json) | [25 fixtures](../target/valuation-benchmarks/P5-bc6/tranche-capture-report.json) |
| Combined | [Manifest](../target/valuation-benchmarks/P5-combined/manifest.json) | [25 tranche + nine calibration](../target/valuation-benchmarks/P5-combined/smoke-ledger.json) | [25 fixtures](../target/valuation-benchmarks/P5-combined/tranche-capture-report.json) |

Preserved attempts: [count preflight failure](../target/valuation-benchmarks/P5-count/checks-preflight-01/failure.json), [lint log](../target/valuation-benchmarks/P5-count/checks-lint-01/lint-01.log), [lint fix manifest](../target/valuation-benchmarks/P5-count/lint-fix-manifest.json), [documentation drift](../target/valuation-benchmarks/P5-count/checks/source-drift.json). The successful provenance contains the explicit documentation-drift resume record.

### Canonical test cleanup and remaining work

After selecting combined, the [cleanup application](../target/valuation-benchmarks/F1/canonical-cleanup-01/applied.json)
applied the reviewed [test-only source cleanup](../target/valuation-benchmarks/F1/canonical-cleanup-01/source-stage-manifest.json)
and [two validation tools](../target/valuation-benchmarks/F1/canonical-cleanup-01/tools-stage-manifest.json).
The obsolete B4 frozen-artifact writer and three conflicting candidate wrappers
were removed. The canonical `tranche_check_exact_reference` test writes under
`target/valuation-benchmarks/current/` and retains the selected combined policy.
The [scoped regression](../target/valuation-benchmarks/F1/tranche-regression-01/provenance.json)
passed credit lint, exactly one canonical ignored policy test, current-output
metadata resolution and the final retention helper. The [retention report](../target/valuation-benchmarks/F1/tranche-regression-01/retention-report.json)
records the same 18 fixtures, 24 scenarios, 504 periods and 25 aliases. This
scoped result is not the complete F1 gate. The later test-only state does not
redefine the immutable accepted P5-combined build or any historical raw result.

The historical shared validator hash
`5f9a2fd79e8ccb5829065136a32abd0aab19a3a6e1298a0ac5efd8a60b9d85c5`
identifies the [durably preserved preimage](../target/valuation-benchmarks/F1/canonical-cleanup-01/preimages/target/valuation-benchmarks/tools/check-p5-evidence.py).
The current shared validator hash is
`989a4f98896cc75d2b7cc97cd8e95738b1bdd3fea4f1d018cd5d8a7966a006b3`.
Historical P5 records continue to identify their original validator and source;
no historical artifact is rewritten to claim the newer tool ran earlier.

P6 is accepted below. F1 is now complete: generation, formatting, the final full gate, complete native/release-host execution, all 111 cumulative B0 comparisons, the explicit P4 analytic-control reassessment and 18 successful isolated RSS processes passed their required checks. Historical P5 results and selected variants remain unchanged.

## P6 accepted: linear-time commodity Asian moments

The [P6 decision](../target/valuation-benchmarks/P6/disposition.json) accepts the
commodity Asian moment reductions against preceding accepted P5-combined and
B0. Fifteen of the 17 predeclared Asian opportunities clear the 10% latency
target against both comparators in every paired point estimate and descriptive
marginal-CI lower endpoint. Geometric fixing-count 12 additionally clears the
incremental target against P5-combined under complete original-setting
confirmation; its cumulative gain against B0 remains below materiality.
The six unrelated commodity controls have no individual point estimate or
descriptive endpoint beyond a 10% regression.

Both moment reductions now use linear work and a constant number of compensated
scalar accumulators. Existing per-valuation projection vectors remain. No extra
suffix vector, sort, global cache or runtime dependency was introduced.
**No P6 RSS reduction is claimed.** The [P6 manifest](../target/valuation-benchmarks/P6/manifest.json)
retains eight release binaries and an observed 775-file source inventory,
including financial-Markdown snapshots. That inventory is not a count of
production files changed. Its candidate hashes identify the archived P6 source,
not live files after subsequent F1 generation.

### Matched latency and confirmation

The [pre-timing roles](../target/valuation-benchmarks/P6/pre-timing-case-roles.json)
fix 17 Asian opportunities and six controls. The
[short request](../target/valuation-benchmarks/P6/expedited-request.json),
[207-process ledger](../target/valuation-benchmarks/P6/expedited-request.ledger.json)
and [summary](../target/valuation-benchmarks/P6/expedited-request.summary.json)
completed all 23 cases across B0, P5-combined and P6 with three independent
processes per revision. Geometric fixing-count 12 was
[selected before confirmation](../target/valuation-benchmarks/P6/confirmation-selection.json)
because short timing did not resolve its incremental materiality. Its complete
[nine-process original-setting campaign](../target/valuation-benchmarks/P6/original-confirmation-01.ledger.json)
and [summary](../target/valuation-benchmarks/P6/original-confirmation-01.summary.json)
supply that case's final assessment. The remaining 22 cases use the complete
short campaign. Both campaigns passed; protocols are not pooled and individual
repetitions are not substituted.

Both [short](../target/valuation-benchmarks/P6/clock-audit.json) and
[confirmation](../target/valuation-benchmarks/P6/clock-audit-original-confirmation-01.json)
clock metadata audits are clear. The [assessment](../target/valuation-benchmarks/P6/assessment.json)
retains all 23 cases and 46 comparison edges, process medians, within-process
95% Criterion intervals, between-process ranges/standard deviations and paired
reductions. The two edges for each case share the same processes. Marginal-CI
endpoint ratios remain descriptive ranges, not calibrated ratio confidence
intervals.

Table latencies are medians of three process medians in microseconds.
`†` marks the complete original-setting confirmation; other rows use short
timing. Negative reductions mean slower execution.

| Matched workload | B0 (µs) | P5-combined (µs) | P6 (µs) | Reduction vs P5-combined | vs B0 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Forward 6M control | 0.405 | 0.420 | 0.405 | 3.509% | -0.202% |
| Swap 2Y control | 6.482 | 6.500 | 6.442 | 0.898% | 0.619% |
| Spot option PV 6M control | 0.315 | 0.336 | 0.320 | 4.899% | -1.504% |
| Spot option Greeks 6M control | 7.275 | 7.216 | 7.144 | 0.998% | 1.800% |
| Original arithmetic: 12 observations | 0.734 | 0.735 | 0.546 | 25.800% | 25.649% |
| Original arithmetic method | 0.742 | 0.746 | 0.547 | 26.731% | 26.375% |
| Original geometric method | 0.607 | 0.610 | 0.557 | 8.702% | 8.255% |
| Spread option 6M control | 0.280 | 0.278 | 0.258 | 7.010% | 7.679% |
| Swaption 2Y control | 6.366 | 6.341 | 6.339 | 0.037% | 0.424% |
| Arithmetic: 12 fixings | 0.731 | 0.755 | 0.530 | 29.770% | 27.502% |
| Arithmetic: 60 fixings | 7.019 | 7.025 | 1.320 | 81.203% | 81.186% |
| Arithmetic: 252 fixings | 104.599 | 104.577 | 4.105 | 96.075% | 96.076% |
| Arithmetic: 1260 fixings | 2,516.470 | 2,516.382 | 18.034 | 99.283% | 99.283% |
| Geometric: 12 fixings † | 0.579 | 0.607 | 0.534 | 11.999% | 7.784% |
| Geometric: 60 fixings | 3.048 | 3.052 | 1.337 | 56.172% | 56.125% |
| Geometric: 252 fixings | 35.520 | 35.483 | 4.210 | 88.135% | 88.147% |
| Geometric: 1260 fixings | 808.361 | 807.980 | 18.412 | 97.721% | 97.722% |
| Seasoned arithmetic: 252 observations, 126 realized | 40.521 | 39.723 | 14.171 | 64.326% | 65.028% |
| Seasoned geometric: 252 observations, 126 realized | 22.890 | 22.005 | 14.271 | 35.146% | 37.652% |
| Arithmetic 252: zero volatility | 103.603 | 103.776 | 3.771 | 96.366% | 96.360% |
| Arithmetic 252: small volatility | 103.708 | 103.795 | 3.874 | 96.267% | 96.264% |
| Geometric 252: zero volatility | 35.299 | 35.369 | 4.121 | 88.349% | 88.325% |
| Geometric 252: small volatility | 35.434 | 35.478 | 4.158 | 88.281% | 88.266% |

At 1,260 fixings, arithmetic latency falls 99.28% versus both comparators,
and geometric latency falls 97.72%. Seasoned arithmetic and geometric improve
64.33% and 35.15% respectively versus P5-combined. These are measured
end-to-end reductions; percentages from separate slices are not added.

The confirmed geometric 12-fixing case improves **11.999% versus P5-combined**,
with all three incremental point estimates and descriptive lower endpoints
above 10%; its **7.784% versus B0** is below materiality. The original geometric
method improves 8.255% versus B0 and 8.702% versus P5-combined. No repeatable
material gain is claimed for that original case; one marginal interval crossing
the target does not establish one. Neither short geometric result is promoted
into a universal small-case claim.

### Numerical, scoped and release validation

The [scoped check ledger](../target/valuation-benchmarks/P6/check-ledger.json)
records passing format/lint checks, 32 focused tests including the frozen
17-record panel and thirteen new edge functions, 313 owning tests and two
umbrella consumer tests. Coverage overlaps and is not summed as unique tests.
The [numerical comparison](../target/valuation-benchmarks/P6/commodity-comparison.json)
preserves the exact fixtures, grids, compensated quadratic references, scales,
independent and baseline price gates, parity checks and predeclared bounds.
Arithmetic references retain the existing Turnbull–Wakeman approximation.

The [release smoke ledger](../target/valuation-benchmarks/P6/smoke-ledger.json)
records all 23 cases and captures passing. Its
[release comparison](../target/valuation-benchmarks/P6/commodity-release-comparison.json)
checks 17 Asian call prices against the independent references and matched B4
limits; all six unrelated commodity output payloads are exactly unchanged.
The largest absolute release-call discrepancy is USD `3.346940502524376e-10`
against an independent reference and USD `1.000444171950221e-9` versus B4;
these observations are separate from the unchanged per-case acceptance limits.
Smoke duration is not a comparative latency measurement. The
[archived captures](../target/valuation-benchmarks/P6/captures.json) and source
manifest preserve this release-build result independently of later F1 work.

P6 remains accepted for its frozen workloads. F1 subsequently completed generation, formatting, the final full gate, all required native/release-host checks, the final archive, the 111-case cumulative campaign and heavy-case RSS. Its fresh results are reported separately below; historical P6 percentages are not added to other slices.

## Remaining comparisons and final gate

Before-case profiling completed ten selected workloads plus three Hull–White
rate-note controls. The [main profile ledger](../target/valuation-benchmarks/B0/profiles/ledger.json)
and [rate-note ledger](../target/valuation-benchmarks/B0/profiles-hw-controls/ledger.json)
retain exact commands, binary hashes and successful sampler exits. These used
separate symbolized release builds and are attribution evidence only. Their
process durations and reported memory footprints are not clean latency or RSS
measurements, and profiling binaries will not supply those comparisons.

The original 111-case measurement contract fixes matched release builds and
`RAYON_NUM_THREADS=8`. Ordinary settings are 50 samples, three seconds warmup,
five seconds measurement and 10,000 resamples. Six slow Bermudan rows use ten
samples and 60-second windows; the 16/17-name tranche rows and full-plan
calibration control use ten samples and 120-second windows. These settings
were selected before clean timings or candidate execution and governed B0–P4; all retain the
three-second warmup and 10,000 resamples. Criterion may extend the window to
obtain the required sample count. Run at least three
independent process repetitions in alternating order, with no competing build,
profiler or benchmark. Each accepted slice is compared with both B0 and the
preceding accepted implementation.

The user approved shorter future matched timings. The separate
[expedited protocol](../target/valuation-benchmarks/expedited-timing-protocol.json)
applies to fresh P5, P6 and F1 campaigns: ten samples, one-second warmup,
two-second requested measurement window, 10,000 resamples and three independent
process repetitions, with eight Rayon threads. Every revision in a comparison
uses the same settings. All 111 case identities, stage case sets, workload
sizes, seeds, models, outputs, numerical references and accuracy bounds remain
fixed. Two seconds is a requested window, not a process time limit. The original
plan, B4 contract and existing results remain unchanged.

A contemplated materiality or restored-control conclusion unsupported by the
variation/intervals, a potentially repeatable control regression over 10%, or
a timing-artifact/clock anomaly triggers original-setting confirmation. The
selected whole case block is rerun for every revision with three fresh
repetitions; both complete campaigns remain available and are not pooled.
Confirmation supplies the final assessment for those cases; unresolved evidence
is inconclusive. A clear win or stable below-materiality/negative result is
complete evidence and does not automatically require longer timing.

A [retrospective metadata audit](../target/valuation-benchmarks/interruption-audits/prior-latency-20261107T014222Z.json)
found no completion-wall or adjacent-process flags across the 360 earlier B0,
P1, P2 and P3 latency rows at its five-second threshold. This is a timestamp
check, not additional performance acceptance. Earlier RSS ledgers lacked the
required clock metadata and were not audited by that check.

Exact commands and archived binary hashes are in the requests and run ledgers.
Each process uses an isolated Criterion output directory under
`target/valuation-benchmarks/<revision>/<case>/<run>/`; a benchmark rebuilt
from the current candidate is not the archived B0 binary. Preserve raw outputs
before they are overwritten. Report within-process confidence intervals
separately from between-process variability. Heavy-MC peak RSS uses isolated
selected-case processes with capture disabled; retain the measurement method
and units. Work counts and profiles are separate from clean latency/RSS runs.

Working materiality targets are a repeatable end-to-end median latency reduction
of at least 10% or peak-memory reduction of at least 20%, with unchanged
numerical contracts and supporting uncertainty. Investigate repeatable control
regressions over 10% and material memory increases. These are scoped acceptance
criteria for this investigation, not universal CI budgets. Negative findings
with retained evidence are valid outcomes.

P4 remains accepted with its disclosed historical analytic-control tradeoff. P5 retains the separately validated combined count/BC6 configuration; standalone BC6 remains unselected because of its confirmed N18 control regressions. Canonical test cleanup and the scoped regression passed. P6 remains accepted after its numerical, scoped, release-smoke and matched timing checks. F1 is complete and accepted: all 111 cumulative cases have absolute before/after medians and uncertainty, all 30 controls were reviewed, two selected controls use complete original-setting confirmations, and all 18 RSS processes passed. Generation, formatting, the final archive, complete native/release-host smoke and full gate also passed. The final tables, numerical/work evidence, limitations and exact evidence links follow.

## F1 completed: cumulative comparison and release gate

F1 is accepted for the frozen 111-workload suite: 75 of 81 predeclared opportunities meet the repeatable 10% latency criterion; six are completed below-material outcomes. All 30 controls pass the observed no-over-10% regression assessment after two preselected whole-case original-setting confirmations. The heavy Asian case has a 99.873% median peak-RSS reduction; MBS and Bermudan RSS are effectively unchanged. All numerical contracts, 456 native/20 release Python/nine release WASM validity rows, generation, formatting and final full gate passed. Historical P4 analytic-control and standalone-BC6 tradeoffs, rejected P3 identity routing and P5 approximation drift remain disclosed; no historical cause is inferred and slice percentages are not added.

All 111 frozen cases have complete B0/F1 timing evidence: 666 primary processes, with complete original-setting confirmations used for selected whole cases. 75 of 81 predeclared opportunities have repeatable material support. All 30 controls were reviewed. Historical slice percentages remain separate and are not added.

Within-process 95% Criterion intervals and between-process variation remain separate. Marginal interval endpoint ratios are descriptive, not ratio confidence intervals. All absolute process medians/CIs and reconstruction rules are retained in the machine companion; raw campaigns remain pinned.

| Case | B0 median (µs) | F1 median (µs) | Reduction |
| --- | ---: | ---: | ---: |
| `commodity_forward_pv/6M` | 0.406404 | 0.419446 | -3.209% |
| `commodity_swap_pv/2Y` | 6.51521 | 6.60482 | -1.375% |
| `commodity_spot_option_pv/6M` | 0.323704 | 0.323155 | 0.170% |
| `commodity_spot_option_greeks/6M` | 7.33772 | 7.55532 | -2.965% |
| `commodity_asian_option/obs_count/12obs` | 0.744608 | 0.558603 | 24.980% |
| `commodity_asian_option/method/arithmetic` | 0.76099 | 0.547364 | 28.072% |
| `commodity_asian_option/method/geometric` | 0.63604 | 0.556681 | 12.477% |
| `commodity_spread_option/tenor/6M` | 0.285293 | 0.280707 | 1.607% |
| `commodity_swaption/swap_tenor/2Y` | 6.33552 | 6.36836 | -0.518% |
| `commodity_asian_option/fixing_count/arithmetic/12` | 0.736748 | 0.553828 | 24.828% |
| `commodity_asian_option/fixing_count/arithmetic/60` | 7.01855 | 1.33176 | 81.025% |
| `commodity_asian_option/fixing_count/arithmetic/252` | 104.745 | 4.18282 | 96.007% |
| `commodity_asian_option/fixing_count/arithmetic/1260` | 2516.18 | 18.0799 | 99.281% |
| `commodity_asian_option/fixing_count/geometric/12` | 0.606481 | 0.579296 | 4.482% |
| `commodity_asian_option/fixing_count/geometric/60` | 3.07598 | 1.3575 | 55.868% |
| `commodity_asian_option/fixing_count/geometric/252` | 35.7122 | 4.27361 | 88.033% |
| `commodity_asian_option/fixing_count/geometric/1260` | 815.154 | 18.6121 | 97.717% |
| `commodity_asian_option/seasoned/arithmetic_252obs_126realized` | 40.8922 | 14.7152 | 64.015% |
| `commodity_asian_option/seasoned/geometric_252obs_126realized` | 22.9784 | 15.0785 | 34.380% |
| `commodity_asian_option/volatility/arithmetic_252obs_zero` | 103.668 | 3.8141 | 96.321% |
| `commodity_asian_option/volatility/arithmetic_252obs_small` | 103.818 | 3.93807 | 96.207% |
| `commodity_asian_option/volatility/geometric_252obs_zero` | 35.6328 | 4.18201 | 88.264% |
| `commodity_asian_option/volatility/geometric_252obs_small` | 35.5817 | 4.26847 | 88.004% |
| `cds_tranche_npv/equity_0_3` | 36132.9 | 23942.2 | 33.739% |
| `cds_tranche_npv/junior_mezz_3_7` | 47911.8 | 26082.4 | 45.562% |
| `cds_tranche_npv/senior_mezz_7_10` | 49094.5 | 26433 | 46.159% |
| `cds_tranche_npv/senior_10_15` | 48685.8 | 26113.4 | 46.363% |
| `cds_tranche_cs01/cs01` | 339283 | 272420 | 19.707% |
| `cds_tranche_correlation_delta/correlation_delta` | 95847.7 | 52262.4 | 45.473% |
| `cds_tranche_jump_to_default/jump_to_default` | 0.122607 | 0.121427 | 0.962% |
| `cds_tranche_par_spread/par_spread` | 47265.1 | 26116.4 | 44.745% |
| `cds_tranche_all_metrics/all_metrics` | 483836 | 350690 | 27.519% |
| `cds_tranche_student_t_npv/junior_mezz_3_7_df6` | 12267.3 | 11851.2 | 3.392% |
| `cds_tranche_heterogeneous/npv_hetero/10` | 82041.1 | 7043.16 | 91.415% |
| `cds_tranche_heterogeneous/npv_hetero/16` | 7.79546e+06 | 12255.8 | 99.843% |
| `cds_tranche_heterogeneous/npv_hetero/17` | 1.60064e+07 | 13395.8 | 99.916% |
| `cds_tranche_heterogeneous/npv_hetero/18` | 235208 | 14487.5 | 93.841% |
| `cds_tranche_heterogeneous/npv_hetero/50` | 433295 | 68346.2 | 84.226% |
| `cds_tranche_heterogeneous/npv_hetero/125` | 2.86373e+06 | 341857 | 88.063% |
| `cds_tranche_heterogeneous/correlation_delta/18` | 468498 | 28965.8 | 93.817% |
| `cds_tranche_heterogeneous/correlation_delta/125` | 5.75377e+06 | 682128 | 88.145% |
| `cds_tranche_p5_controls/gaussian_heterogeneous_four_3_7` | 6110.68 | 6137.95 | -0.446% |
| `cds_tranche_p5_controls/gaussian_concentrated_50_3_7` | 1.16407e+06 | 1.16364e+06 | 0.037% |
| `cds_tranche_p5_controls/gaussian_recovery_dispersion_50_3_7` | 491020 | 494331 | -0.674% |
| `cds_tranche_p5_controls/gaussian_equal_exposure_125_40_80` | 4.25949e+06 | 489603 | 88.506% |
| `cds_tranche_p5_controls/gaussian_homogeneous_125_40_80` | 71620.3 | 36966.9 | 48.385% |
| `cds_tranche_p5_controls/gaussian_homogeneous_125_60_100` | 55525.2 | 36279.1 | 34.662% |
| `cds_tranche_p5_controls/student_t_equal_exposure_50_3_7_df6` | 224123 | 39581.2 | 82.340% |
| `swap_pv/5Y` | 14.1405 | 14.5852 | -3.145% |
| `ois_pv/1Y` | 27.9077 | 19.3602 | 30.628% |
| `ois_pv/2Y` | 54.829 | 38.1327 | 30.452% |
| `ois_pv/5Y` | 137.854 | 94.4546 | 31.482% |
| `ois_pv/10Y` | 274.491 | 188.386 | 31.369% |
| `ois_standard_metrics/1Y` | 142.837 | 102.497 | 28.242% |
| `ois_standard_metrics/2Y` | 284.657 | 197.464 | 30.631% |
| `ois_standard_metrics/5Y` | 695.947 | 483.8 | 30.483% |
| `ois_standard_metrics/10Y` | 1393.93 | 973.991 | 30.126% |
| `ois_controls_pv/5Y_seasoned` | 134.965 | 93.4139 | 30.787% |
| `ois_controls_pv/5Y_lookback_2bd` | 291.176 | 246.616 | 15.304% |
| `ois_controls_pv/5Y_observation_shift_2bd` | 289.992 | 247.637 | 14.606% |
| `ois_controls_pv/5Y_cutoff_2bd` | 138.298 | 95.7392 | 30.773% |
| `ois_controls_pv/5Y_arithmetic_spread_10bp` | 137.707 | 92.574 | 32.775% |
| `ois_controls_pv/5Y_explicit_forward` | 170.045 | 113.008 | 33.542% |
| `ois_controls_pv/5Y_discount_derived_forward` | 171.753 | 114.414 | 33.385% |
| `ois_controls_metrics/5Y_seasoned` | 689.379 | 478.671 | 30.565% |
| `ois_controls_metrics/5Y_explicit_forward` | 861.602 | 576.785 | 33.057% |
| `ois_cashflow_schedule/5Y` | 144.778 | 146.511 | -1.197% |
| `overnight_projection_controls/1Y_compounded_spread_10bp` | 17.6288 | 17.9624 | -1.892% |
| `agency_mbs_pv/passthrough_example` | 147.25 | 147.697 | -0.303% |
| `agency_mbs_metrics/oas` | 4511.25 | 2725.9 | 39.576% |
| `agency_mbs_metrics/duration_convexity` | 611.522 | 626.311 | -2.418% |
| `agency_mbs_oas_scaling/paths_512_months_360` | 4659.73 | 2832.26 | 39.218% |
| `agency_mbs_oas_scaling/paths_2048_months_360` | 17852.6 | 10720.8 | 39.948% |
| `agency_mbs_oas_scaling/paths_8192_months_360` | 69304.9 | 42183.2 | 39.134% |
| `agency_mbs_oas_scaling/paths_2048_months_60` | 2971.05 | 1828.25 | 38.465% |
| `agency_mbs_oas_scaling/paths_2048_months_180` | 8549.82 | 5390.88 | 36.947% |
| `agency_mbs_oas_scaling/gnma_i_paths_2048_months_360` | 18147.6 | 10711.5 | 40.975% |
| `agency_mbs_oas_scaling/delay_15_paths_2048_months_360` | 18231 | 10748.6 | 41.042% |
| `agency_mbs_oas_scaling/fresh_act360_paths_2048_months_300` | 15380.6 | 8956.26 | 41.769% |
| `agency_mbs_oas_scaling/seasoned_act360_paths_2048_months_300` | 15372.2 | 8956.97 | 41.733% |
| `asian_option_mc/2500` | 24807.5 | 11532.2 | 53.513% |
| `asian_option_mc/paths_2500_fixings_12` | 29656.8 | 13560.6 | 54.275% |
| `asian_option_mc/paths_10000_fixings_12` | 73600.6 | 29246.6 | 60.263% |
| `asian_option_mc/paths_50000_fixings_12` | 315571 | 117042 | 62.911% |
| `asian_option_mc/paths_10000_fixings_2` | 63621.3 | 25463.1 | 59.977% |
| `asian_option_mc/paths_10000_fixings_52` | 96706.1 | 40443.9 | 58.179% |
| `asian_option_mc/seasoned_paths_10000_fixings_12_past_6` | 32472.6 | 13783.2 | 57.554% |
| `lookback_option_mc/2500` | 12975.5 | 13091.1 | -0.891% |
| `autocallable_mc/2500` | 10598.7 | 10388.4 | 1.984% |
| `cliquet_option_mc/12resets` | 226547 | 224750 | 0.793% |
| `barrier_monitoring/continuous/gbm_2500_paths` | 13899.8 | 13802.6 | 0.699% |
| `barrier_monitoring/continuous/heston_2500_paths` | 9766.35 | 9731.65 | 0.355% |
| `barrier_monitoring/continuous/pde` | 282.233 | 269.04 | 4.674% |
| `barrier_monitoring/quarterly/gbm_2500_paths` | 12220.7 | 12358.1 | -1.124% |
| `barrier_monitoring/quarterly/heston_2500_paths` | 8617.83 | 8761.72 | -1.670% |
| `barrier_monitoring/quarterly/pde` | 285.844 | 277.922 | 2.771% |
| `mc_bermudan_hw_tree/calibrate_each_price` | 1205.2 | 1195.2 | 0.830% |
| `mc_bermudan_hw_tree/pre_calibrated_100_steps` | 1073.78 | 1070.95 | 0.264% |
| `mc_bermudan_lsmc/50000` | 5.57644e+06 | 5.44418e+06 | 2.372% |
| `mc_bermudan_lsmc_axes/paths_5000/base_128` | 602211 | 551196 | 8.471% |
| `mc_bermudan_lsmc_axes/paths_20000/base_128` | 2.44934e+06 | 2.25316e+06 | 8.010% |
| `mc_bermudan_lsmc_axes/paths_50000/base_128` | 6.22685e+06 | 5.73339e+06 | 7.925% |
| `mc_bermudan_lsmc_axes/paths_20000/base_64` | 2.26606e+06 | 2.22528e+06 | 1.799% |
| `mc_bermudan_lsmc_axes/paths_20000/base_256` | 2.9548e+06 | 2.31557e+06 | 21.634% |
| `range_accrual_analytic/12_monthly_obs` | 6.54488 | 6.78616 | -3.687% |
| `quanto_range_accrual/analytic_12_observations` | 7.69905 | 7.8422 | -1.859% |
| `quanto_range_accrual/gbm_2500_paths_12_observations` | 8949.46 | 9066.07 | -1.303% |
| `callable_range_accrual_lsmc/2500` | 12179.5 | 8623.54 | 29.196% |
| `snowball_hw1f_mc/2500` | 8194.81 | 3987.16 | 51.345% |
| `tarn_hw1f_mc/2500` | 15401.7 | 6068.87 | 60.596% |
| `calibration_base_correlation_full_plan_5_knots` | 8.3275e+06 | 4.85312e+06 | 41.722% |

Complete short campaign: median reduction -3.686570%; paired reductions -3.686570%, -2.332777%, -7.787513%; worst descriptive lower endpoint -8.915032%. Every process and descriptive range supports no over-10% regression for this measured control. Retain its movement as control evidence; no optimization gain is claimed. This is the required fresh P4 analytic-control reassessment. Its F1 median is 3.687% slower than B0 and its worst lower endpoint is -8.915%; no current confirmation trigger. Historical P4 acceptance and unexplained 10.17% incremental/20.10% cumulative slowdown remain historical facts; this run establishes no explanation for them.

The complete validation passed 456 native rows across 37 targets, 20 release Python rows and nine release Node/WASM rows. Smoke durations are validity evidence. The retained gate history includes unsuccessful attempts and their qualified fixes.

Peak RSS uses macOS `/usr/bin/time -l`, bytes, isolated selected-case processes with capture disabled: checked same-case preflight plus one operation. These are process peaks, not direct allocation measurements.

| Heavy case | B0 median bytes | F1 median bytes | Reduction |
| --- | ---: | ---: | ---: |
| `asian_option_mc/paths_50000_fixings_12` | 5033099264 | 6373376 | 99.873% |
| `agency_mbs_oas_scaling/paths_8192_months_360` | 32817152 | 32751616 | 0.200% |
| `mc_bermudan_lsmc_axes/paths_50000/base_128` | 296091648 | 296124416 | -0.011% |

Numerical contracts and work evidence:

- P1: Two N-path captures become one paired N-path traversal without retaining trajectories/payoff vectors on the arithmetic-Asian route. All 23 numerical records passed unchanged fixture/grid/bound checks; maximum absolute price difference was 7.993605777301127e-14. Physical paths and independent estimator counts remain distinct.
- P2: Cashflows are projected once per path into monthly coefficients; each Brent trial evaluates one spread factor per nonzero month. Scratch uses at most 32 monthly blocks, without another paths-by-months matrix. All 90 production-helper numerical records passed unchanged path, objective, root and repricing gates. Maximum baseline root difference was 9.575673587391975e-16 in decimal spread; maximum fixed-spread baseline PV difference was 1.4435499906539917e-8.
- P3: Internal PV omits ProjectedFixing records and their series-ID strings. Canonical daily projection and observation exposures remain unchanged; public exports retain their metadata. All 28 public numerical records match baseline float bits. The rejected discount-identity variant is absent from the accepted implementation.
- P4: Exact Hull-White interval coefficients are prepared once per grid and reused across paths. Coefficient arithmetic, random draws, antithetic ordering and event ordering remain unchanged. All ten HW numerical records exactly match B4, including reference fields, prices, uncertainty and diagnostic bits; prepared/direct-path checks passed unchanged bounds.
- P5: The selected combined configuration retains exact-count recurrence for eligible equal-positive-weight/recovery pools and Gaussian homogeneous-index paired integration. Each of count, BC6 and combined passed its selected independent-reference policy test across 18 fixtures, 24 scenarios, 504 periods and 25 aliases, plus seven calibration controls with three frozen PV-budget gates each. Diagnostic counters belong to separate untimed invocations; paired-cap and scalar counters are not added into a whole-price count.
- P6: Both commodity-Asian moment reductions use linear work and a constant number of compensated scalar accumulators; existing projection vectors remain. No suffix vector, sort, global cache or runtime dependency was added. The 17 frozen numerical records and 13 additional edge functions passed unchanged independent-reference, baseline and bound checks; all 23 release smoke cases passed. This is not a P6 RSS-benefit claim.
- F1 identity qualification passed: the final archive/request remain bound to the frozen 111-case economic contract after declared descriptive-metadata projections. There are 41 affected emitted-fixture proofs across commodity, MBS and rate-exotic targets. The 60-case smoke inventory includes 19 native-only fi_misc cases without emitted fixture records. This offline qualification is not new repricing, timing acceptance or binary hashing.

Limitations:

- These are fixed synthetic workloads on one machine/profile with Rayon 8, not a measured user workload mix or universal portfolio speedup. No workload frequency or weighted benefit is assumed.
- Within-process Criterion 95% median intervals remain separate from between-process variation. Ratios of marginal interval endpoints are descriptive ranges, not calibrated ratio confidence intervals. Three process pairs remain three independent repetitions.
- The final assessment uses 109 complete short cases and two complete original-setting confirmation cases. Both raw campaigns are retained. No pooling, individual-repetition substitution or favorable-protocol selection occurred. The short autocallable endpoint and quanto-analytic adverse process remain visible; their causes are not established.
- Six opportunities are consistently below 10%: commodity geometric fixing_count/12 and five LSMC cases (original 50000 and base-128 paths 5000/20000/50000 plus base-64 paths 20000). They are valid completed outcomes; no material gain is claimed for them.
- P4 historically retained an analytic-control slowdown of 10.17% versus P3 and 20.10% versus B0 under an explicit MC/rate-note workload justification. Its timing cause remains unexplained. F1 freshly measures a 3.687% median slowdown, with all points and descriptive lower endpoints above -10%; this does not explain or erase the historical result.
- P3 discount-identity routing was rejected after failing the unchanged 1e-8 PV/export gate with 1.095759216696024e-8. Accepted production uses metadata-only omission and canonical daily calculations.
- P5 standalone BC6 was not selected: its confirmed N18 NPV/correlation controls were 19.48%/20.14% slower than P4 despite an unchanged scalar route. No compiler, LTO, layout or clock cause is established.
- Count/combined deliberately change some old approximate-grid N>17 prices. The largest disclosed PV movement is USD 450.62676834710874; acceptance rests on frozen independent exact-reference gates, not B0 bit equality. No exact whole-pool reference exists for genuine recovery dispersion.
- P5 calibration b_i uses tranche notional and each gate is at most 2*b_i. Conditional cap-error budgets are not certified Simpson true-error or correlation-knot bounds. Student-t checks do not certify continuous-factor integration accuracy.
- P6 arithmetic pricing retains the existing Turnbull-Wakeman approximation. Mechanism descriptions or derived dimensions are not measured operation totals. Numerical/scoped counts overlap, and repeated seeds/processes do not add independent estimators.
- RSS covers exactly three original heavy cases, measured as process peaks using macOS /usr/bin/time -l in bytes, with same-case checked preflight plus one operation and capture disabled. It is not a direct allocation count. MBS and Bermudan have no material memory benefit or increase. The initial sandbox-denied attempt produced no RSS and is retained separately from all 18 successful approved retry processes.
- The fixed plan remains unchanged. Generic FD scenario caching, Taylor VaR, PE waterfalls, convertible optimization and a comprehensive mixed-asset host benchmark programme remain separate follow-ups.

The complete F1 evidence graph, including exact raw request/ledger/summary locations, archive lineage, clock audits, metadata qualification and all gate attempts, is hash-pinned in `final_gate` of the machine companion.
