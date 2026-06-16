# Finstack Quant Core Benchmarks

Criterion benchmark suites for `finstack-quant-core`.

The benchmark sources are the ground truth. This README explains what is
measured and how to run the suites. It intentionally avoids hard latency,
allocation, or "all targets met" claims unless you have current benchmark
results to back them up.

## Running Benchmarks

```bash
# Run all core benchmarks
cargo bench --package finstack-quant-core

# Run selected suites
cargo bench --package finstack-quant-core --bench money_operations
cargo bench --package finstack-quant-core --bench daycount_operations
cargo bench --package finstack-quant-core --bench calendar_operations
cargo bench --package finstack-quant-core --bench interpolation
cargo bench --package finstack-quant-core --bench curve_operations
cargo bench --package finstack-quant-core --bench expr_dag
cargo bench --package finstack-quant-core --bench rolling
cargo bench --package finstack-quant-core --bench solver_operations
cargo bench --package finstack-quant-core --bench market_context
cargo bench --package finstack-quant-core --bench vol_surface
cargo bench --package finstack-quant-core --bench integration
cargo bench --package finstack-quant-core --bench statistical_functions
cargo bench --package finstack-quant-core --bench rate_conversions
cargo bench --package finstack-quant-core --bench cashflow_operations
cargo bench --package finstack-quant-core --bench schedule_generation
cargo bench --package finstack-quant-core --bench factor_model

# Compile benchmark targets without running them
cargo bench --package finstack-quant-core --bench expr_dag --bench market_context --no-run

# Save and compare Criterion baselines
cargo bench --package finstack-quant-core -- --save-baseline baseline_name
cargo bench --package finstack-quant-core -- --baseline baseline_name
```

## Benchmark Coverage

### `money_operations.rs`

- Money construction and arithmetic
- FX-backed conversion paths
- Batch monetary operations
- Formatting overhead

### `daycount_operations.rs`

- Year-fraction calculations across supported day-count conventions
- Batch date-period calculations
- More complex conventions such as `ActActIsma` and `Bus252`

### `calendar_operations.rs`

- Holiday and business-day checks
- Business-day adjustments
- Composite calendar behavior
- Batch date checks and counting loops

### `interpolation.rs`

- Single-point and batch interpolation
- Interpolation style comparisons
- Extrapolation behavior

### `curve_operations.rs`

- Discount, forward, and hazard curve lookup costs
- Batch evaluation across multiple tenors
- Curve construction overhead

### `expr_dag.rs`

- Complex DAG evaluation without planning
- Planned DAG execution
- Cached DAG execution
- Row-scaling behavior for larger series

### `rolling.rs`

- Rolling mean, median, and standard deviation
- Different data sizes and window sizes
- Repeated expression-evaluation overhead for rolling operators

### `solver_operations.rs`

- Newton and Brent root finding
- IRR/XIRR solver paths
- Multi-dimensional solver scenarios where present

### `market_context.rs`

- Typed curve and surface lookups
- Batch lookups
- Context cloning
- Bump operations
- Credit-index rebinding-sensitive bump scenarios

### `vol_surface.rs`

- Vol-surface construction
- Interpolation and boundary handling
- Bump operations

### `integration.rs`

- Simpson, adaptive Simpson, trapezoidal, Gauss-Legendre, and Gauss-Hermite paths

### `statistical_functions.rs`

- Distribution helpers
- Random sampling utilities
- Basic statistics and probability primitives

### `rate_conversions.rs`

- Simple, periodic, and continuous rate compounding conversions
- Round-trip conversion accuracy paths
- Batch conversion scaling
- Market scenario conventions (treasury, LIBOR, corporate)
- Negative rate handling

### `cashflow_operations.rs`

- Curve-based NPV with Money-typed cashflows (flat and shaped curves)
- Scalar NPV with flat discount rates
- Batch cashflow count scaling (4 to 240 flows)
- Day count convention comparison overhead
- Discountable trait dispatch vs direct function
- Investment profile scenarios (bond coupons, swap netted flows)

### `schedule_generation.rs`

- Frequency variant comparison (monthly, quarterly, semi-annual, annual)
- Stub convention handling (short/long front/back)
- Tenor scaling from 1Y to 30Y
- End-of-month convention overhead
- IMM and CDS-IMM schedule generation
- Business day adjustment with calendar lookup
- Schedule iteration and collection

### `factor_model.rs`

- FactorCovarianceMatrix validated construction (5 to 100 factors)
- Variance, covariance, and correlation lookups by factor ID
- Batch variance and full correlation matrix extraction
- MappingTableMatcher with first-hit, last-hit, and miss scenarios
- HierarchicalMatcher tree traversal at varying depths
- CascadeMatcher multi-stage chain evaluation

## Reading Results

Criterion writes results under `target/criterion/`. Useful outputs include:

- Terminal summaries with confidence intervals
- HTML reports in `target/criterion/*/report/index.html`
- Raw measurement data under each benchmark directory

## Evidence Standard

Use current benchmark output, not this README, to make performance claims.

Recommended workflow:
1. Compile touched benchmark targets with `--no-run` during refactoring.
2. Run the relevant suites on the current branch.
3. Save a baseline before larger changes.
4. Compare against that baseline after the change.
5. Record any release-note or README performance claims only after those results exist.

## Notes

- Benchmarks run under Cargo's benchmark profile.
- Results vary by hardware, toolchain, thermal state, and background load.
- `black_box()` is used to reduce optimizer distortion.
- If you add a new benchmark suite, update this README with what it measures, not with guessed numbers or stale target values.
