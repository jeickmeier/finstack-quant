# Summary

- Reviewed `finstack/valuations/src/attribution/parallel.rs`.
- Performance risk: moderate, because the path runs across portfolio-scale attribution workloads.

## Performance concerns

- Per-factor allocation appears inside the portfolio loop.
- Metric key string construction is repeated for every instrument/factor pair.
- No benchmark baseline was cited for parallel vs serial attribution after the change.

## Findings

### Majors
- Move reusable allocation outside the inner loop or pre-size accumulators based on known factor count. Expected impact: lower allocation pressure in portfolio-scale runs.

### Minors / Nits
- Add a Criterion benchmark comparing serial and parallel attribution on a representative instrument set.

## Benchmarking recommendations

- Run `mise run rust-bench` for attribution benchmarks.
- If exposed through Python, run the same workload after `mise run python-build -- --release`.

## Action items

- [ ] Add or update a portfolio-scale attribution benchmark.
- [ ] Compare serial and parallel outputs for deterministic equality.
