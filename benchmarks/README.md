# benchmarks

Checked-in performance *records* — not benchmark code. The measurement
harnesses live with the code they measure (Criterion benches under
`finstack-quant/*/benches/`, pytest-benchmark under
`finstack-quant-py/benchmarks/`, and `finstack-quant-wasm/benchmarks/bench.mjs`).
This directory contains the portfolio-materialization acceptance record, the
completed valuation validity/performance record, and the core/cashflows/models
review and completed performance campaign. Materialization has immutable baselines and gated comparison results;
the other records identify their workload, validation and measurement scope.

Other benchmarks remain discovery-oriented unless a specific record states
otherwise. Benchmark availability does not create a library-wide performance
gate.

## Contents

| Path | What it is |
| --- | --- |
| [`VALUATION_PERFORMANCE.md`](VALUATION_PERFORMANCE.md) | Summary of the historical six valuation investigations and final 111-case comparison, retained decisions, numerical bounds and tradeoffs. |
| [`valuation-performance-results.json`](valuation-performance-results.json) | Historical valuation measurements, process uncertainty and memory; raw artifacts were not retained and historical pass records are unverified. |
| [`valuation-evidence.json`](valuation-evidence.json) | Provenance manifest for retained valuation summaries, numerical fixtures and immutable original Git revisions. |
| [`CORE_CASHFLOW_MODEL_PERFORMANCE.md`](CORE_CASHFLOW_MODEL_PERFORMANCE.md) | Completed benchmark repairs and six investigations: three retained changes, 23 qualifying latency cases, three RSS reductions and the explicit typed-volatility control cost. |
| [`core-cashflow-model-performance-results.json`](core-cashflow-model-performance-results.json) | Compact incremental and cumulative measurements, uncertainty, rejected LMM history, numerical validation and separate measured/delivered source identities. |
| [`CORE_CASHFLOW_MODEL_PERFORMANCE_REVIEW.md`](CORE_CASHFLOW_MODEL_PERFORMANCE_REVIEW.md) | Initial 21-target core/cashflows/models audit, validity findings and six planned investigations. Its initial smoke evidence is separate from the subsequent implementation and baseline. |
| [`MATERIALIZATION_BENCHMARKS.md`](MATERIALIZATION_BENCHMARKS.md) | The procedure and the human-readable reference result: fixture definitions, timing boundaries, gates, profiling recipe, and hardware/toolchain provenance. |
| `materialization/materialization-rust-baseline.json` | Immutable Rust baseline `materialization-v1`. Median-per-case for `cold_a_5000_unique`, `cold_b_5000_50`, `warm_b_5000_50`, `validation_b_5000_50`, plus tree revision and fixture digest. |
| `materialization/materialization-python-baseline.json` | Immutable Python baseline `python-materialization-v1`. Same three timed cases minus validation-only. |
| `materialization/materialization-benchmark-baseline.json` | Manifest sealing both baselines: their paths, names, tree revisions, fixture digests, SHA-256s, case lists, and the 10% median-regression threshold. |
| `materialization/materialization-benchmark-results.json` | The full record produced by one acceptance run: raw samples, percentiles, bootstrap intervals, phase counters, environment, exact commands, gate evaluations, and Criterion baseline/current/change estimates for Rust, Python, and WASM. |

The materialization JSON files carry a `schema` field
(`finstack_quant.materialization_benchmark_results/1` and siblings). These are
data artifacts written by tooling — edit them only through the steps below.

The valuation record describes the November 7, 2026 campaign. Its detailed
original report is preserved at an immutable Git revision linked from the
summary. Historical raw timing ledgers, archived binaries and validation logs
were not retained and are lost. References to them have been removed from
the current summary. Historical pass records are unverified; current test
runs do not recreate the historical campaign.

## Materialization fixtures are not checked in

Both fixtures are regenerated deterministically before every run:

```bash
scripts/materialization_bench.sh fixtures
```

That runs `cargo run --release -p finstack-quant-portfolio --example
materialization_fixtures`, which writes
`target/materialization-benchmarks/materialization-a-5000-unique.json`
(5,000 positions over 5,000 unique artifacts) and
`materialization-b-5000-50.json` (5,000 positions over 50 artifacts). The
generator body is shared with the Criterion bench via
`finstack-quant/portfolio/benches/materialization_fixtures.rs`, so Rust, Python
and Node/WASM measure identical bytes. A run whose combined fixture digest does
not match the sealed baseline is rejected before any comparison.

## Regenerating the materialization record

One command produces the result set:

```bash
scripts/materialization_bench.sh record
```

It regenerates fixtures, verifies baseline provenance, runs release Rust,
Python, and Node/WASM measurements, applies both strict regression gates,
writes `materialization-benchmark-results.json`, syncs the document digest, and
re-verifies. It never replaces a baseline, and it never rewrites the sealing
manifest — `materialization-benchmark-baseline.json` is written by the `seal`
step of the two baseline steps below, so the record run only reads it.

Individual stages:

| Step | Does |
| --- | --- |
| `scripts/materialization_bench.sh rust-baseline` | Establishes and seals the Rust baseline. Fails before measuring if one already exists. |
| `scripts/materialization_bench.sh python-baseline` | Same for the Python baseline. |
| `scripts/materialization_bench.sh rust-compare` | Fresh Criterion run compared against the checked-in Rust baseline at a 10% median gate. |
| `scripts/materialization_bench.sh python-compare` | Fresh release-Python run compared against the Python baseline at the same gate. |
| `scripts/materialization_bench.sh wasm` | Regenerates fixtures, builds the Node WASM target, and benchmarks materialization only. |
| `scripts/materialization_bench.sh doc-check` | Verifies the record digest, toolchain lines, and every baseline path/identity/hash — runs no measurement. Part of `all-doc` and the CI Documentation workflow. |

Baselines are immutable by design. Replacing one requires the explicit opt-in:

```bash
FQ_REPLACE_MATERIALIZATION_BASELINE=1 scripts/materialization_bench.sh rust-baseline
```

`FQ_MATERIALIZATION_P95_SAMPLES` defaults to 100 in every step above and must be
a finite integer of at least 100 for an acceptance record.
`FQ_MATERIALIZATION_SMOKE=1` lowers that floor to 1; it is a short-run override
for tests only and must not be used to produce a record.

The scripts behind these steps — baseline management, provenance capture,
regression gating, and result collection — are documented in
[`../scripts/README.md`](../scripts/README.md).

## Other benchmarks in the workspace

Nothing below writes to this directory.

| Where | Harness | Run with |
| --- | --- | --- |
| Manifest-declared targets under `finstack-quant/*/benches/` | Criterion | `mise run rust-bench`, or `mise run rust-bench-crate -- <package> <bench>` for one target |
| `finstack-quant-py/benchmarks/bench_bindings.py` | pytest-benchmark | `mise run python-bench` (all), `scripts/materialization_bench.sh python` (portfolio only) |
| `finstack-quant-wasm/benchmarks/bench.mjs` | Node | `npm --prefix finstack-quant-wasm run bench` |

Criterion timing is deliberately short by default; override with
`FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`, `FQ_BENCH_MEASUREMENT_TIME`,
and `FQ_BENCH_NRESAMPLES`. CI does not run benchmarks.

The scoped task supports `--filter <regex>`, `--test`,
`--save-baseline <name>` and `--baseline <name>`. Smoke mode checks execution;
it records no timing baseline. Named Criterion output is mutable, so recorded
campaigns retain separate raw process archives. There are no separate
`rust-bench-baseline` or `rust-bench-compare` tasks.
