# benchmarks

Checked-in performance *records* — not benchmark code. The measurement
harnesses live with the code they measure (Criterion benches under
`finstack-quant/*/benches/`, pytest-benchmark under
`finstack-quant-py/benchmarks/`, and `finstack-quant-wasm/benchmarks/bench.mjs`).
What is tracked here is the portfolio-materialization acceptance record: the
immutable baselines a comparison run is gated against, and the machine-readable
result set the Markdown document quotes.

Only the materialization path is recorded this way. Every other benchmark in
the workspace is discovery-oriented: run it, read the numbers, nothing is
committed.

## Contents

| Path | What it is |
| --- | --- |
| [`MATERIALIZATION_BENCHMARKS.md`](MATERIALIZATION_BENCHMARKS.md) | The procedure and the human-readable reference result: fixture definitions, timing boundaries, gates, profiling recipe, and hardware/toolchain provenance. |
| `materialization/materialization-rust-baseline.json` | Immutable Rust baseline `materialization-v1`. Median-per-case for `cold_a_5000_unique`, `cold_b_5000_50`, `warm_b_5000_50`, `validation_b_5000_50`, plus tree revision and fixture digest. |
| `materialization/materialization-python-baseline.json` | Immutable Python baseline `python-materialization-v1`. Same three timed cases minus validation-only. |
| `materialization/materialization-benchmark-baseline.json` | Manifest sealing both baselines: their paths, names, tree revisions, fixture digests, SHA-256s, case lists, and the 10% median-regression threshold. |
| `materialization/materialization-benchmark-results.json` | The full record produced by one acceptance run: raw samples, percentiles, bootstrap intervals, phase counters, environment, exact commands, gate evaluations, and Criterion baseline/current/change estimates for Rust, Python, and WASM. |

Each JSON carries a `schema` field
(`finstack_quant.materialization_benchmark_results/1` and siblings). These are
data artifacts written by tooling — edit them only through the steps below.

## Fixtures are not checked in

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

## Regenerating the record

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
| `finstack-quant/{analytics,attribution,cashflows,core,factor-model,monte_carlo,portfolio,scenarios,statements,valuations}/benches/` | Criterion | `mise run rust-bench`, or `mise run rust-bench-crate -- <package> <bench>` for one target; the four-case PR gate is `python3 -m scripts.pr_benchmarks baseline` on the base commit, then `compare`, at a 10% gate |
| `finstack-quant-py/benchmarks/bench_bindings.py` | pytest-benchmark | `mise run python-bench` (all), `scripts/materialization_bench.sh python` (portfolio only) |
| `finstack-quant-wasm/benchmarks/bench.mjs` | Node | `npm --prefix finstack-quant-wasm run bench` |

Criterion timing is deliberately short by default; override with
`FQ_BENCH_SAMPLE_SIZE`, `FQ_BENCH_WARM_UP_TIME`, `FQ_BENCH_MEASUREMENT_TIME`,
and `FQ_BENCH_NRESAMPLES`. Benchmarks do not gate PR CI.
