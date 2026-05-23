---
trigger: model_decision
description: Information about the coding standards of the rust finstack library
globs:
---
# Rust Code Standards for rfin Project

## Core Principles

1. **Zero-cost abstractions** - Performance should not be sacrificed for ergonomics
2. **Type safety first** - Use the type system to prevent errors at compile time
3. **No-std compatibility** - Core functionality must work without the standard library
4. **Clear error handling** - All fallible operations return `Result` with meaningful errors

## Module Organization

### Structure

- Use facade patterns for complex modules (see `market_data` as example)
- Public re-exports at module root for ergonomic imports
- Separate files for submodules over 500 lines
- Group related functionality (e.g., all interpolators in `interp/`)

### Documentation

```rust
//! Module-level documentation explaining purpose and usage.
//!
//! Include examples for common use cases.

/// Item-level documentation with examples
///
/// # Examples
/// ```
/// use rfin_core::Currency;
/// let usd = Currency::USD;
/// ```
pub struct MyType;
```

- Do not add #[allow(missing_docs)], add missing docs instead

### Mathematical Utilities Organization

**Default Principle**: Mathematical functions belong in `finstack_core::math::` unless they are exclusively useful for a specific domain (e.g., Monte Carlo path simulation, not general numerical computation).

**Core Math Modules** (`finstack_core::math::`):

- `stats` - Statistical functions (mean, variance, correlation, streaming statistics via `OnlineStats`)
- `special_functions` - Normal distribution functions (norm_cdf, norm_pdf, erf, inverse CDF)
- `linalg` - Linear algebra (Cholesky decomposition, correlation matrices)
- `random` - RNG traits and implementations (SimpleRng, Philox, Sobol)
- `integration` - Numerical integration (Gauss-Hermite, adaptive Simpson, etc.)
- `solver` - Root finding and optimization
- `summation` - Numerically stable summation (Kahan, pairwise)
- `interp` - Interpolation methods
- `distributions` - Probability distributions and sampling

**Domain-Specific Modules** (e.g., `valuations::mc::`):
Keep in domain-specific modules ONLY if exclusively useful for that domain:

- **MC-specific**: Stochastic processes (GBM, Heston), discretization schemes (Euler, Milstein, QE), payoff definitions, pricing engines, variance reduction strategies specific to MC simulation
- **Statements-specific**: Precedence evaluation, corkscrew logic, articulation
- **Portfolio-specific**: Position aggregation, book hierarchies

**Examples**:

- ✅ Move `PhiloxRng` to `core::math::random` - useful for parallel simulations across modules
- ✅ Move `cholesky_decomposition` to `core::math::linalg` - useful for factor models, portfolio optimization
- ✅ Move `OnlineStats` to `core::math::stats` - useful for streaming metrics anywhere
- ❌ Keep `GbmProcess` in `mc::process` - specific to stochastic path simulation
- ❌ Keep `EuropeanCall` payoff in `mc::payoff` - specific to MC option pricing

**Before adding math utilities**:

1. Check if similar functionality exists in `core::math`
2. If adding to a domain module, justify why it can't be generalized to `core::math`
3. Prefer re-exporting from `core::math` over duplicating code

## Error Handling

### Unified Error Type

```rust
// Use the crate's unified error type
use crate::Result; // type alias for Result<T, crate::Error>

// Domain-specific error variants
#[non_exhaustive] // Always use for future compatibility
pub enum InputError {
    TooFewPoints,
    NonMonotonicKnots,
    // ...
}
```

### Result Usage

- Return `crate::Result<T>` for all fallible operations
- Use `?` for error propagation
- Provide context with error variants, not strings

## Type Design

### Builder Pattern

- **Entry point**: Use `Type::builder(...)` as the only public entry point; do not add `Builder::new` or `new()` aliases.

```rust
pub struct CurveBuilder {
    // Required fields without defaults
    id: &'static str,
    // Optional fields with sensible defaults
    base: Date,
    style: InterpStyle,
}

impl Curve {
    pub fn builder(id: &'static str) -> CurveBuilder { /* ... */ }
}

impl CurveBuilder {
    // Fluent setters returning Self
    pub fn base_date(mut self, date: Date) -> Self {
        self.base = date;
        self
    }

    // Terminal method consumes self
    pub fn build(self) -> Result<Curve> { /* ... */ }
}
```

### Newtype Pattern

```rust
// Use for type safety and zero-cost abstraction
#[repr(transparent)]
pub struct CurveId(&'static str);

impl CurveId {
    pub const fn new(id: &'static str) -> Self {
        assert!(!id.is_empty(), "CurveId cannot be empty");
        Self(id)
    }
}
```

## Performance Guidelines

### Inlining

```rust
// Always inline trivial accessors
#[inline]
pub const fn amount(&self) -> f64 { self.amount }

// Consider inlining hot-path functions
#[inline(always)]
pub fn locate_segment(xs: &[F], x: F) -> Result<usize> { /* ... */ }
```

### Memory Management

- Prefer `Box<[T]>` over `Vec<T>` for immutable data
- Use `SmallVec` for small, bounded collections
- Pre-allocate with `with_capacity` when size is known

## Feature Flags

### Organization

```toml
[features]
default = ["std"]
std = ["serde?/std"]  # Use ? for optional dependencies
parallel = ["dep:rayon"]
decimal128 = ["dep:rust_decimal"]
```

### Code Organization

```rust
#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[cfg_attr(docsrs, doc(cfg(feature = "parallel")))]
pub fn parallel_npv(&self) -> Money {
    #[cfg(feature = "parallel")]
    { /* parallel implementation */ }

    #[cfg(not(feature = "parallel"))]
    { /* sequential fallback */ }
}
```

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Helper functions for test data
    fn sample_curve() -> DiscountCurve {
        DiscountCurve::builder("TEST")
            .knots([(0.0, 1.0), (1.0, 0.98)])
            .build()
            .unwrap()
    }

    #[test]
    fn descriptive_test_name() {
        // Arrange
        let curve = sample_curve();

        // Act
        let result = curve.df(0.5);

        // Assert with tolerance for floats
        assert!((result - 0.99).abs() < 1e-12);
    }
}
```

### Integration Tests

- Place in `tests/` directory
- One test file per major feature
- Share fixtures via helper modules

## Trait Design

### Object Safety

```rust
// Ensure traits are object-safe when needed for polymorphism
pub trait InterpFn: Send + Sync + Debug {
    fn interp(&self, x: F) -> F;
}

// Use associated types for non-object-safe traits
pub trait Discount: TermStructure {
    fn base_date(&self) -> Date;
    fn df(&self, t: F) -> F;

    // Provide default implementations
    #[inline]
    fn zero(&self, t: F) -> F {
        -self.df(t).ln() / t
    }
}
```

## Naming Conventions

### General Rules

- Types: `CamelCase` (e.g., `DiscountCurve`)
- Functions/methods: `snake_case` (e.g., `year_fraction`)
- Constants: `SCREAMING_SNAKE_CASE` (e.g., `MAX_ENTRIES`)
- Modules: `snake_case` (e.g., `market_data`)

### Specific Patterns

- Builders: `Type::builder()` returning `TypeBuilder`
- Conversions: `into_*` (consuming), `as_*` (borrowing), `to_*` (expensive)
- Predicates: `is_*` or `has_*`

## Documentation Standards

### Module Documentation

```rust
//! Short description of module purpose.
//!
//! Longer explanation with context and use cases.
//!
//! # Examples
//! ```
//! use rfin_core::dates::{Date, DayCount};
//! let yf = DayCount::Act360.year_fraction(start, end)?;
//! ```
//!
//! # Sub-modules
//! * [`submod`] - Description
```

### Public API Documentation

- All public items must have doc comments
- Include at least one example for non-trivial APIs
- Use tables for enumerating options/variants
- Add `# Errors` section for fallible functions
- Add `# Panics` section if applicable

## Code Organization

### Import Style

```rust
// Standard library
use std::collections::HashMap;

// External crates
use time::{Date, Month};
use serde::{Serialize, Deserialize};

// Crate imports
use crate::{Currency, Error, Result};
use crate::dates::DayCount;

// Module imports
use super::traits::Discount;
```

### Visibility

- Start with minimum visibility, increase as needed
- Use `pub(crate)` for internal APIs
- Seal traits that shouldn't be implemented downstream

## Unsafe Code

### Guidelines

- Avoid `unsafe` unless absolutely necessary
- When used, must have:
  - `// SAFETY:` comment explaining invariants
  - Debug assertions validating preconditions
  - Comprehensive tests including edge cases

## Benchmarking

### Criterion Benchmarks

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_interpolation(c: &mut Criterion) {
    let interp = setup_interpolator();

    c.bench_function("monotone_convex_df", |b| {
        b.iter(|| {
            for &x in &test_points {
                black_box(interp.interp(black_box(x)));
            }
        })
    });
}
```

## Macro Usage

### Guidelines

- Prefer const generics and traits over macros
- Use declarative macros for reducing boilerplate
- Document macro inputs and outputs
- Provide examples in macro documentation

## Version Control

### Commit Messages

- Use conventional commits (feat:, fix:, docs:, etc.)
- Reference issue numbers where applicable
- Keep changes focused and atomic

### Backwards Compatibility

- Mark new APIs with version tags in docs
- Use `#[non_exhaustive]` for enums/structs that may grow
- Follow semver strictly

## Clippy and Formatting

### Required Lints

```rust
#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]  // Common in finance code
```

### Formatting

- Run `cargo fmt` before committing
- Use `rustfmt.toml` for project-wide consistency
- Maximum line width: 100 characters

## Financial Domain Specifics

### Numeric Types

- Use `F` type alias (`f64`) for consistency
- Document precision requirements
- Use tolerance checks for float comparisons

### Currency Safety

```rust
// Operations must check currency compatibility
impl Add for Money {
    type Output = Result<Self, Error>;

    fn add(self, rhs: Self) -> Self::Output {
        ensure_same_currency(&self, &rhs)?;
        Ok(Money::new(self.amount + rhs.amount, self.currency))
    }
}
```

### Date Handling

- Use `time` crate types consistently
- Document timezone assumptions
- Use `Date` for dates, `OffsetDateTime` for timestamps

## Configuration Pattern

Configuration parameters follow a three-tier approach to balance simplicity with flexibility:

### Tier 1: Module-Local Constants (Default)

For algorithm-specific parameters that are rarely changed, define constants inline in the module where they're used:

```rust
// In math/linalg.rs
/// Threshold for detecting singular matrices in Cholesky decomposition.
pub const SINGULAR_THRESHOLD: f64 = 1e-10;

/// Tolerance for diagonal elements in correlation matrices.
pub const DIAGONAL_TOLERANCE: f64 = 1e-6;

pub fn cholesky_decomposition(matrix: &[f64], n: usize) -> Result<Vec<f64>> {
    // Uses SINGULAR_THRESHOLD directly
}
```

```rust
// In cashflow/xirr.rs
pub const DEFAULT_TOLERANCE: f64 = 1e-6;
pub const DEFAULT_MAX_ITERATIONS: usize = 100;
pub const DEFAULT_GUESS: f64 = 0.1;
```

**Use for:**

- Numerical tolerances (epsilon values)
- Algorithm iteration limits
- Cache sizes and memory budgets
- Internal thresholds not exposed to users

### Tier 2: Builder Methods (Customization)

For parameters that users occasionally need to customize, provide fluent builder methods:

```rust
pub struct NewtonSolver {
    pub tolerance: f64,
    pub max_iterations: usize,
    pub fd_step: f64,
}

impl Default for NewtonSolver {
    fn default() -> Self {
        Self {
            tolerance: 1e-12,
            max_iterations: 50,
            fd_step: 1e-8,
        }
    }
}

impl NewtonSolver {
    pub fn new() -> Self { Self::default() }

    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        self.tolerance = tolerance;
        self
    }

    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }
}
```

**Use for:**

- Solver parameters (tolerance, iterations)
- Interpolator settings
- Any parameter where the default works 95% of the time

### Tier 3: FinstackConfig Extensions (User-Facing)

Reserve `FinstackConfig` extensions for truly user-facing configuration that:

- Needs to be serialized/persisted
- Applies across multiple computations
- Is part of a reproducible pipeline

```rust
// In config.rs - part of FinstackConfig
pub struct ToleranceConfig {
    pub rate_epsilon: f64,
    pub generic_epsilon: f64,
}

// Complex calibration config - appropriate for extensions
pub const CALIBRATION_CONFIG_KEY_V1: &str = "valuations.calibration.v1";
```

**Use for:**

- Rounding and precision settings (`ToleranceConfig`)
- Calibration configurations (many parameters, user-specified)
- Anything that needs audit trail / reproducibility

### Decision Criteria

| Question | If Yes → |
|----------|----------|
| Is this an internal algorithm parameter? | Tier 1: const |
| Do users rarely need to change it? | Tier 1: const |
| Is it occasionally customized per-call? | Tier 2: builder |
| Must it be serialized/audited? | Tier 3: FinstackConfig |
| Is it complex with many parameters? | Tier 3: FinstackConfig |

### Anti-Patterns to Avoid

❌ **Don't** create separate config files/modules for simple parameters
❌ **Don't** use FinstackConfig extensions for algorithm-specific tolerances
❌ **Don't** pass config objects through call stacks when a const suffices
❌ **Don't** duplicate defaults across config structs and implementations

✅ **Do** keep constants near the code that uses them
✅ **Do** provide builder methods for occasional customization
✅ **Do** reserve extensions for complex, user-facing configuration
