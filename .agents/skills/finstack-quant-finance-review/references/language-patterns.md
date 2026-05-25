# Language-Specific Patterns Reference

Quant finance idioms, pitfalls, and best practices for Rust, Python, WASM/JS, and SQL.

## Rust

### Numerical Computing

- **Float ordering**: `f64` does not implement `Ord` (because of NaN). Use `total_cmp()` for sorting, or wrap in `OrderedFloat`. Flag any code using `partial_cmp().unwrap()` in hot paths without NaN guards.
- **SIMD**: Use `std::simd` (nightly) or `packed_simd2`/`wide` crate for vectorized operations. Check that data alignment requirements are met. Verify fallback for non-SIMD targets (WASM may not support all SIMD instructions).
- **Memory layout**: For Monte Carlo paths, prefer `Vec<Vec<f64>>` laid out as paths×timesteps (row-major) if iterating over timesteps, or transpose if iterating over paths. Check cache access patterns.
- **Unsafe blocks**: Audit every `unsafe` block. In numerical code, common unsafe uses are SIMD intrinsics and FFI to C/Fortran LAPACK. Verify invariants documented in comments.

### Error Handling for Quant

- Use `Result<T, E>` for operations that can fail (calibration, root-finding). Never `unwrap()` in library code.
- Define domain-specific error types: `CalibrationError`, `NumericalError`, `DataError`. Include context (what parameter, what value, what constraint was violated).
- For pricing functions, distinguish between "no valid price" (model failure) and "price is zero" (legitimate result). Use `Option<f64>` or a custom type.

### Performance Patterns

- **Allocation-free hot paths**: Preallocate buffers and pass `&mut [f64]` slices to inner functions. Avoid `Vec::push` in tight loops.
- **Rayon parallelism**: Use `par_iter()` for embarrassingly parallel Monte Carlo. Check that the per-thread RNG is properly seeded (don't share a single RNG across threads).
- **Const generics**: Use for fixed-dimension linear algebra (e.g., `Matrix<N, M>`) to enable stack allocation and compiler optimization.
- **Iterators over indexing**: Prefer `iter().zip()` over index-based loops. The compiler can better optimize bounds-check-free iteration.

### Trait Design for Numerics

```
// Good: trait-based model interface
trait PricingModel {
    fn price(&self, market: &MarketData, trade: &Trade) -> Result<f64, PricingError>;
    fn greeks(&self, market: &MarketData, trade: &Trade) -> Result<Greeks, PricingError>;
}

// Good: generic numerical type
trait Scalar: Copy + Add<Output=Self> + Mul<Output=Self> + ... {
    fn exp(self) -> Self;
    fn ln(self) -> Self;
    fn sqrt(self) -> Self;
}
```

Flag implementations that hardcode `f64` where a generic `Scalar` trait would enable automatic differentiation.

## Python

### NumPy/SciPy Patterns

- **Vectorization**: Flag any `for` loop over array elements that could be vectorized. Common offenders: payoff calculation, path simulation, Greek computation.
- **Broadcasting**: Verify shapes are intentional, not accidental broadcasting. A (N,) array broadcast with a (M,1) array creates an (M,N) matrix — check that this is desired.
- **Copy vs. view**: `array[mask]` creates a copy; `array[slice]` creates a view. Writing to a copy doesn't modify the original. Flag any code that modifies a fancy-indexed array expecting in-place mutation.
- **dtype consistency**: Ensure all arrays are `float64` for pricing. Flag `float32` unless explicitly justified for performance. Check for silent upcasting in mixed operations.

### Pandas for Market Data

- **Index alignment**: Pandas aligns on index by default. Two DataFrames with different date indices will silently introduce NaN. Always verify alignment or use explicit join.
- **GroupBy pitfalls**: `groupby().apply()` can be slow and memory-intensive. Prefer vectorized groupby operations (`transform`, `agg`).
- **Time zones**: Use `tz_localize` and `tz_convert` explicitly. Flag any `pd.Timestamp` without timezone info in production code.
- **Chained indexing**: `df[condition][column] = value` may not work (SettingWithCopyWarning). Use `df.loc[condition, column] = value`.

### Performance

- **Numba JIT**: For custom numerical kernels that can't be vectorized. Check that `@njit` (no-Python mode) is used, not `@jit` which falls back silently.
- **Memory profiling**: For large simulations, check peak memory. A 10,000-path × 252-step × 5-asset simulation = 12.6M doubles = ~100MB. Multiply by number of intermediate arrays.
- **Multiprocessing**: Use `multiprocessing.Pool` (not `threading`) for CPU-bound work due to GIL. Check that large arrays are passed via shared memory, not pickle serialization.

### Common Anti-Patterns

```python
# BAD: growing a list in a loop
results = []
for i in range(n_simulations):
    results.append(simulate_path(...))
np.array(results)  # slow conversion

# GOOD: preallocate
results = np.empty((n_simulations, n_steps))
for i in range(n_simulations):
    results[i] = simulate_path(...)

# BEST: vectorize the entire simulation
Z = np.random.standard_normal((n_simulations, n_steps))
paths = S0 * np.exp(np.cumsum((r - 0.5*sigma**2)*dt + sigma*np.sqrt(dt)*Z, axis=1))
```

## WASM/JS

### Numerical Precision

- **JS number type**: All numbers are IEEE 754 double-precision (f64). No integer overflow for values up to 2⁵³. But `0.1 + 0.2 !== 0.3` — never use `===` for float comparison in financial calculations.
- **BigInt for exact arithmetic**: Use for integer currency amounts (cents, basis points). Never mix BigInt and Number in arithmetic.
- **WASM↔JS boundary**: Passing `f64` between Rust-WASM and JS is efficient (no conversion). But check that arrays use `Float64Array` (not `Float32Array`) for financial data.
- **TypedArray alignment**: `Float64Array` requires 8-byte alignment. If slicing from a shared `ArrayBuffer`, verify offset alignment.

### Memory Management

- **WASM linear memory**: Finite and must be grown explicitly. For large simulations, pre-calculate required memory and grow upfront. Check for `memory.grow` failures.
- **Passing arrays**: Use `wasm-bindgen` with `&[f64]` / `Vec<f64>`. Avoid repeated small allocations across the WASM boundary — batch data transfer.
- **Garbage collection interaction**: JS objects referencing WASM memory can prevent GC. Use explicit `free()` calls for WASM-allocated objects when done.

### Concurrency

- **Web Workers**: For parallel Monte Carlo in the browser. Check that `SharedArrayBuffer` is available (requires specific HTTP headers: `Cross-Origin-Opener-Policy`, `Cross-Origin-Embedder-Policy`).
- **Atomics**: If using shared memory between workers, verify that `Atomics.wait`/`Atomics.notify` are used correctly. Race conditions in financial simulations can produce subtly wrong results.
- **WASM threads**: Rust WASM with threads requires `wasm32-unknown-unknown` + atomics target feature. Verify the build configuration.

### Performance Patterns

- **Hot path in WASM, orchestration in JS**: Keep pricing kernels in Rust-WASM. Use JS for UI, data fetching, and result presentation.
- **Batch pricing**: Send all trades to WASM in one call, price them all, return results. Avoid per-trade WASM calls (each has overhead).
- **Streaming results**: For large simulations, stream partial results back to JS for progress updates rather than blocking until completion.

## SQL

### Time-Series Patterns

```sql
-- Point-in-time data retrieval (avoid look-ahead bias)
WITH ranked AS (
    SELECT *,
        ROW_NUMBER() OVER (
            PARTITION BY security_id
            ORDER BY publication_date DESC
        ) AS rn
    FROM fundamental_data
    WHERE publication_date <= @as_of_date
)
SELECT * FROM ranked WHERE rn = 1;
```

- **ASOF joins**: If the database supports them (DuckDB, kdb+, TimescaleDB), prefer native ASOF join over the ROW_NUMBER pattern.
- **Calendar alignment**: Use a trading calendar table for date arithmetic. Don't use `DATEADD(day, -1, ...)` for "previous business day."

### Aggregation Pitfalls

- **NULL in aggregates**: `AVG()` ignores NULLs, which can bias results. Use `COUNT(*)` vs `COUNT(column)` to detect NULL prevalence.
- **Division by zero**: Always guard with `NULLIF(denominator, 0)`. Common in ratio calculations (P/E, Sharpe, etc.).
- **Distinct counting**: `COUNT(DISTINCT ...)` in window functions is not supported in all dialects. Check compatibility.
- **Overflow**: `SUM()` of large integers can overflow in some databases. Cast to DECIMAL or FLOAT first.

### Window Functions for Finance

```sql
-- Rolling volatility (20-day)
SELECT date, security_id,
    STDDEV(ln_return) OVER (
        PARTITION BY security_id
        ORDER BY date
        ROWS BETWEEN 19 PRECEDING AND CURRENT ROW
    ) * SQRT(252) AS annualized_vol
FROM daily_returns;

-- Drawdown from peak
SELECT date,
    price / MAX(price) OVER (
        PARTITION BY security_id
        ORDER BY date
        ROWS BETWEEN UNBOUNDED PRECEDING AND CURRENT ROW
    ) - 1 AS drawdown
FROM prices;
```

- Verify `ROWS` vs `RANGE` semantics. `RANGE` with dates can produce unexpected results when there are gaps.
- Check frame specification: `ROWS BETWEEN 19 PRECEDING AND CURRENT ROW` gives exactly 20 rows. A common mistake is `ROWS BETWEEN 20 PRECEDING` which gives 21 rows.

### Performance

- **Partitioning**: Time-series tables should be partitioned by date. Check that queries include partition pruning predicates.
- **Materialized views**: For expensive aggregations (daily risk metrics), use materialized views with incremental refresh. Verify refresh schedule.
- **Index design**: For market data, composite index on `(security_id, date)` is essential. Check that the most selective column comes first.
- **Query plan review**: For any query running over millions of rows, review the execution plan. Flag full table scans on large tables.
