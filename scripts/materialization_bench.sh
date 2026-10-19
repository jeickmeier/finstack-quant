#!/usr/bin/env bash
# Portfolio materialization benchmarks: fixtures, sealed baselines, comparisons
# and the release record. See benchmarks/README.md.
#
# Usage: scripts/materialization_bench.sh <step>
#   fixtures         Generate the deterministic benchmark fixtures
#   doc-check        Verify tracked digests, baselines and toolchains (no measurement)
#   python           Release-build Python, then run the portfolio pytest-benchmark suite
#   wasm             Release-build the Node WASM target, then benchmark materialization
#   rust-baseline    Establish and seal the Rust baseline (FQ_REPLACE_MATERIALIZATION_BASELINE=1 replaces)
#   python-baseline  Establish and seal the Python baseline (same guard)
#   rust-compare     Compare a fresh Criterion run against the Rust baseline (10% gate)
#   python-compare   Compare a fresh release-Python run against the Python baseline (10% gate)
#   record           Regenerate and verify the complete release benchmark record
set -eu

FIXTURE_A=target/materialization-benchmarks/materialization-a-5000-unique.json
FIXTURE_B=target/materialization-benchmarks/materialization-b-5000-50.json
MANIFEST=benchmarks/materialization/materialization-benchmark-baseline.json
RUST_BASELINE=benchmarks/materialization/materialization-rust-baseline.json
PYTHON_BASELINE=benchmarks/materialization/materialization-python-baseline.json
RESULTS=benchmarks/materialization/materialization-benchmark-results.json
SAMPLES=${FQ_MATERIALIZATION_P95_SAMPLES:-100}

fixtures() {
  cargo run --release --quiet -p finstack-quant-portfolio \
    --example materialization_fixtures -- target/materialization-benchmarks
}

doc_check() {
  uv run --no-sync pytest scripts/tests/test_materialization_baseline_orchestration.py -q
  uv run --no-sync python scripts/sync_materialization_benchmark_digest.py \
    --artifact "$RESULTS" --document benchmarks/MATERIALIZATION_BENCHMARKS.md --check
  uv run --no-sync python scripts/check_materialization_benchmark_docs.py \
    --artifact "$RESULTS" \
    --document benchmarks/MATERIALIZATION_BENCHMARKS.md \
    --baseline-manifest "$MANIFEST" \
    --rust-baseline "$RUST_BASELINE" \
    --python-baseline "$PYTHON_BASELINE"
}

python_bench() {
  fixtures
  mise run python-build -- --release
  uv run --no-sync pytest finstack-quant-py/benchmarks/bench_bindings.py \
    -m perf --benchmark-only -k Portfolio
}

wasm_bench() {
  fixtures
  npm --prefix finstack-quant-wasm run build:node
  npm --prefix finstack-quant-wasm run bench -- --materialization-only
}

criterion_run() {
  CARGO_TARGET_DIR=target/materialization-criterion \
  FQ_MATERIALIZATION_CRITERION_DIR="$PWD/target/materialization-criterion/criterion" \
  FQ_MATERIALIZATION_P95_SAMPLES=$SAMPLES \
  cargo bench -p finstack-quant-portfolio --bench materialization -- \
    --sample-size "$SAMPLES" \
    --warm-up-time 1 --measurement-time 5 --nresamples 1000 --noplot
}

rust_baseline() {
  replace_flag=$(uv run python scripts/materialization_baseline_replace_flag.py)
  uv run python scripts/manage_materialization_rust_baseline.py guard \
    --baseline "$RUST_BASELINE" $replace_flag
  fixtures
  uv run python scripts/check_python_materialization_regressions.py prepare \
    --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --output target/materialization-criterion/baseline-run-metadata.json
  criterion_run
  uv run python scripts/manage_materialization_rust_baseline.py establish \
    --criterion-dir target/materialization-criterion/criterion \
    --run-metadata target/materialization-criterion/baseline-run-metadata.json \
    --baseline-name materialization-v1 \
    --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --output "$RUST_BASELINE" \
    $replace_flag
  uv run python scripts/manage_materialization_rust_baseline.py seal \
    --baseline "$RUST_BASELINE" --manifest "$MANIFEST"
}

python_baseline() {
  uv run --no-sync pytest scripts/tests/test_check_python_materialization_regressions.py -q
  replace_flag=$(uv run python scripts/materialization_baseline_replace_flag.py)
  uv run python scripts/check_python_materialization_regressions.py guard \
    --baseline "$PYTHON_BASELINE" $replace_flag
  fixtures
  mise run python-build -- --release
  uv run python scripts/check_python_materialization_regressions.py prepare \
    --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --output target/materialization-python/baseline-run-metadata.json
  FQ_MATERIALIZATION_P95_SAMPLES=$SAMPLES \
  uv run python finstack-quant-py/benchmarks/materialization_measure.py \
    target/materialization-benchmarks target/materialization-python/baseline-measurement.json
  uv run python scripts/check_python_materialization_regressions.py establish \
    --measurement target/materialization-python/baseline-measurement.json \
    --run-metadata target/materialization-python/baseline-run-metadata.json \
    --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --output "$PYTHON_BASELINE" \
    --artifact-root target/materialization-python \
    --baseline-name python-materialization-v1 \
    $replace_flag
  uv run python scripts/check_python_materialization_regressions.py seal \
    --baseline "$PYTHON_BASELINE" --manifest "$MANIFEST"
}

rust_compare() {
  uv run --no-sync pytest scripts/tests/test_check_criterion_regressions.py -q
  fixtures
  uv run python scripts/prepare_materialization_criterion_run.py \
    --baseline "$RUST_BASELINE" \
    --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --output target/materialization-criterion/run-metadata.json
  criterion_run
  uv run python scripts/check_criterion_regressions.py \
    --criterion-dir target/materialization-criterion/criterion \
    --baseline "$RUST_BASELINE" \
    --run-metadata target/materialization-criterion/run-metadata.json \
    --comparison-output target/materialization-criterion/comparison.json \
    --threshold 0.10
}

python_compare() {
  uv run --no-sync pytest scripts/tests/test_check_python_materialization_regressions.py -q
  current_output=${FQ_MATERIALIZATION_PYTHON_CURRENT_OUTPUT:-target/materialization-python/current.json}
  result_output=${FQ_MATERIALIZATION_PYTHON_COMPARISON_OUTPUT:-target/materialization-python/comparison.json}
  fixtures
  mise run python-build -- --release
  uv run python scripts/prepare_materialization_criterion_run.py \
    --baseline "$PYTHON_BASELINE" \
    --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --output target/materialization-python/run-metadata.json
  FQ_MATERIALIZATION_P95_SAMPLES=$SAMPLES \
  uv run python finstack-quant-py/benchmarks/materialization_measure.py \
    target/materialization-benchmarks "$current_output"
  uv run python scripts/check_python_materialization_regressions.py compare \
    --baseline "$PYTHON_BASELINE" \
    --current "$current_output" \
    --run-metadata target/materialization-python/run-metadata.json \
    --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --artifact-root target/materialization-python \
    --result-output "$result_output" \
    --baseline-manifest "$MANIFEST" \
    --threshold 0.10
}

verify_baselines() {
  uv run python scripts/manage_materialization_rust_baseline.py verify \
    --baseline "$RUST_BASELINE" --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --manifest "$MANIFEST"
}

record() {
  uv run --no-sync pytest scripts/tests/test_collect_materialization_results.py -q
  fixtures
  verify_baselines
  uv run python scripts/check_python_materialization_regressions.py verify \
    --baseline "$PYTHON_BASELINE" --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --manifest "$MANIFEST"
  FQ_MATERIALIZATION_RAW_OUTPUT="$PWD/target/materialization-results/rust.json" rust_compare
  FQ_MATERIALIZATION_PYTHON_CURRENT_OUTPUT=target/materialization-python/current-record.json \
  FQ_MATERIALIZATION_PYTHON_COMPARISON_OUTPUT=target/materialization-python/comparison-record.json \
    python_compare
  npm --prefix finstack-quant-wasm run build:node
  FQ_MATERIALIZATION_P95_SAMPLES=$SAMPLES \
  FQ_MATERIALIZATION_RAW_OUTPUT=target/materialization-results/wasm.json \
    npm --prefix finstack-quant-wasm run bench -- --materialization-only
  verify_baselines
  uv run python scripts/collect_materialization_results.py \
    --rust target/materialization-results/rust.json \
    --python target/materialization-python/current-record.json \
    --wasm target/materialization-results/wasm.json \
    --fixture-a "$FIXTURE_A" --fixture-b "$FIXTURE_B" \
    --baseline "$MANIFEST" \
    --rust-baseline "$RUST_BASELINE" \
    --rust-comparison target/materialization-criterion/comparison.json \
    --python-baseline "$PYTHON_BASELINE" \
    --python-comparison target/materialization-python/comparison-record.json \
    --output "$RESULTS" \
    --command 'scripts/materialization_bench.sh record' \
    --command 'FQ_MATERIALIZATION_P95_SAMPLES=100 FQ_MATERIALIZATION_RAW_OUTPUT=$PWD/target/materialization-results/rust.json scripts/materialization_bench.sh rust-compare' \
    --command 'FQ_MATERIALIZATION_P95_SAMPLES=100 FQ_MATERIALIZATION_PYTHON_CURRENT_OUTPUT=target/materialization-python/current-record.json FQ_MATERIALIZATION_PYTHON_COMPARISON_OUTPUT=target/materialization-python/comparison-record.json scripts/materialization_bench.sh python-compare' \
    --command 'FQ_MATERIALIZATION_P95_SAMPLES=100 FQ_MATERIALIZATION_RAW_OUTPUT=target/materialization-results/wasm.json npm --prefix finstack-quant-wasm run bench -- --materialization-only' \
    --command 'uv run python scripts/collect_materialization_results.py --rust target/materialization-results/rust.json --python target/materialization-python/current-record.json --wasm target/materialization-results/wasm.json --fixture-a target/materialization-benchmarks/materialization-a-5000-unique.json --fixture-b target/materialization-benchmarks/materialization-b-5000-50.json --baseline benchmarks/materialization/materialization-benchmark-baseline.json --rust-baseline benchmarks/materialization/materialization-rust-baseline.json --rust-comparison target/materialization-criterion/comparison.json --python-baseline benchmarks/materialization/materialization-python-baseline.json --python-comparison target/materialization-python/comparison-record.json --output benchmarks/materialization/materialization-benchmark-results.json'
  uv run python scripts/sync_materialization_benchmark_digest.py \
    --artifact "$RESULTS" --document benchmarks/MATERIALIZATION_BENCHMARKS.md --write
  doc_check
}

case "${1:-}" in
  fixtures) fixtures ;;
  doc-check) doc_check ;;
  python) python_bench ;;
  wasm) wasm_bench ;;
  rust-baseline) rust_baseline ;;
  python-baseline) python_baseline ;;
  rust-compare) rust_compare ;;
  python-compare) python_compare ;;
  record) record ;;
  *)
    sed -n '5,14p' "$0" >&2
    exit 2
    ;;
esac
