# Valuations tests

Integration and unit tests for `finstack-quant-valuations`: instruments, cashflows, calibration, metrics, and golden vectors.

## Directory Structure

```
tests/
├── README.md               # This file
│
├── cashflows.rs            # Entry point: cashflow generation tests
├── cashflows/
│   ├── helpers.rs          # Shared helpers: tolerances, dates, curves
│   ├── builder/            # Cashflow schedule, amortization, credit models
│   ├── covenants/          # Covenant violation detection tests
│   ├── day_count.rs        # ISDA 2006 day count golden values
│   ├── provider_contract.rs # CashflowProvider trait compliance
│   ├── schema_roundtrip.rs # JSON schema roundtrip tests
│   └── examples/           # JSON example fixtures (.example.json)
│
├── instruments.rs          # Entry point: instrument pricing tests
├── instruments/
│   ├── common/             # Shared test infrastructure
│   │   ├── helpers.rs      # Curve builders, date utilities
│   │   ├── test_helpers.rs # Tolerances, fixtures
│   │   ├── parity/         # Parity testing framework (QuantLib, Bloomberg)
│   │   └── README.md       # Instrument test conventions
│   ├── bond/               # Fixed income: bonds, deposits, linkers
│   ├── irs/                # Interest rate swaps
│   ├── cds/                # Credit default swaps
│   ├── equity_option/      # Equity derivatives
│   ├── fx_option/          # FX derivatives
│   ├── swaption/           # Swaption tests
│   ├── cap_floor/          # Caps and floors
│   ├── term_loan/          # Private credit: term loans
│   ├── revolving_credit/   # Revolving credit facilities
│   ├── structured_credit/  # CLOs, CDOs, tranches
│   ├── golden/             # Golden test vectors (QuantLib, ISDA)
│   ├── json_examples/      # JSON instrument examples
│   └── ...                 # Other instrument directories
│
├── calibration.rs          # Entry point: curve calibration tests
├── calibration/
│   ├── mod.rs              # Module organization
│   ├── bootstrap.rs        # Curve bootstrapping determinism
│   ├── repricing.rs        # Repricing accuracy for calibrated curves
│   ├── config.rs           # Configuration helpers and validation
│   ├── finstack_config.rs  # Finstack Quant-specific config integration
│   ├── serialization.rs    # Serde roundtrip tests
│   ├── builder.rs          # Calibration builder API tests
│   ├── hazard_curve.rs     # Hazard/credit curve calibration
│   ├── inflation.rs        # Inflation curve calibration
│   ├── swaption_vol.rs     # Swaption volatility surface calibration
│   ├── base_correlation.rs # Base correlation surface calibration
│   ├── failure_modes.rs    # Engine error handling
│   ├── explainability.rs   # Explanation trace generation
│   ├── validation.rs       # Curve and surface validation
│   ├── quote_construction.rs # All-types quote construction verification
│   ├── bloomberg_accuracy.rs # Bloomberg benchmark accuracy
│   ├── v2_engine_smoke.rs   # V2 API smoke tests
│   ├── tolerances.rs       # Calibration-specific tolerances
│   └── term_structures/    # Independent term structure property tests
│       ├── mod.rs
│       ├── curve_monotonicity.rs  # Discount factor monotonicity
│       └── forward_parity.rs      # Forward rate parity relationships
│
├── market.rs               # Entry point: market data model tests
├── market/
│   ├── mod.rs              # Module organization
│   ├── quote_bumps.rs      # Rate, spread, and vol bump operations
│   ├── quote_schemas.rs    # Schema validation for quote types
│   └── build/              # Instrument building from quotes
│       ├── mod.rs
│       ├── credit.rs       # CDS instrument building
│       └── rates.rs        # Deposit/swap/FRA building
│
├── metrics.rs              # Entry point: risk metrics tests
├── metrics/
│   ├── mod.rs              # Module organization
│   ├── convergence.rs      # Analytical vs FD Greek convergence
│   ├── determinism.rs      # Deterministic results for identical inputs
│   ├── edge_cases.rs       # Boundary conditions and degenerate cases
│   ├── graceful_metrics_test.rs # Graceful failure handling
│   ├── greek_relationships.rs   # Mathematical relationships between Greeks
│   ├── invariants.rs       # Property-based tests for metric invariants
│   └── sign_conventions.rs # Correct sign conventions for all Greeks
│
├── attribution.rs          # Entry point: P&L attribution tests
├── attribution/
│   ├── mod.rs              # Module organization
│   ├── bond_attribution.rs # Bond P&L attribution (carry, roll, spread)
│   ├── fx_attribution.rs   # FX P&L attribution (spot, forward, basis)
│   ├── metrics_based_convexity.rs # Convexity P&L attribution
│   ├── model_params_attribution.rs # Model parameter change attribution
│   ├── scalars_attribution.rs # Scalar market data attribution
│   ├── serialization_roundtrip.rs # JSON roundtrip tests
│   └── json_examples/      # Attribution JSON fixtures
│
├── common/                 # Shared utilities across test suites
│   ├── mod.rs              # Module exports
│   ├── assertions.rs       # Assertion helpers with better error messages
│   ├── builders.rs         # Market context and option builders
│   ├── fixtures.rs         # Standard dates, curves, tolerances
│   └── tolerances.rs       # Tolerance constants
│
├── integration.rs          # Entry point: end-to-end tests
└── integration/
    ├── mod.rs              # Module organization
    ├── e2e/                # Full workflow tests (portfolio pricing, FX)
    ├── golden/             # Golden test framework and data loaders
    │   ├── README.md       # Golden test documentation
    │   └── data/           # Reference values (QuantLib, Bloomberg, ISDA)
    ├── metrics/            # Metrics strict mode tests
    ├── schema/             # Schema parity, TypeScript export tests
    └── serialization/      # JSON roundtrip tests
```

## Test Entry Points

The valuations test suite is organized into seven main entry points, each with its own `cargo test --test` target:

| Entry Point | Description | Test Count | Run Command |
|-------------|-------------|------------|-------------|
| `instruments` | Instrument pricing by asset class | ~500+ | `cargo test --test instruments` |
| `calibration` | Curve calibration, term structures | 71 | `cargo test --test calibration` |
| `market` | Market quotes, bumping, building | 20 | `cargo test --test market` |
| `metrics` | Risk metrics, Greeks, convergence | 48 | `cargo test --test metrics` |
| `attribution` | P&L attribution (parallel, waterfall) | 32 | `cargo test --test attribution` |
| `cashflows` | Cashflow generation, schedules, covenants | ~50 | `cargo test --test cashflows` |
| `integration` | E2E workflows, golden tests, serialization | ~30 | `cargo test --test integration` |

### Running Specific Test Categories

```bash
# Run all valuations tests
cargo test -p finstack-quant-valuations

# Run a specific test entry point
cargo test --test instruments
cargo test --test calibration
cargo test --test market
cargo test --test metrics
cargo test --test attribution
cargo test --test cashflows
cargo test --test integration

# Run tests for a specific instrument
cargo test --test instruments bond::
cargo test --test instruments irs::
cargo test --test instruments cds::

# Run specific test categories within an entry point
cargo test --test calibration term_structures::
cargo test --test calibration validation::
cargo test --test metrics sign_conventions::
cargo test --test metrics invariants::
cargo test --test attribution bond_attribution::
```

## Test Organization Patterns

### Module Root Files

Each test entry point (e.g., `instruments.rs`) uses `#[path = "..."]` attributes to include submodules:

```rust
//! Instrument integration tests - comprehensive test runner for all instruments

#[macro_use]
#[path = "instruments/common/mod.rs"]
mod common;

#[path = "instruments/bond/mod.rs"]
mod bond;

#[path = "instruments/irs/mod.rs"]
mod irs;

// ... other instruments
```

For test entry points that need shared utilities:

```rust
//! Calibration test suite entry point

// Shared test utilities
#[path = "common/mod.rs"]
mod common;

// Calibration tests
#[path = "calibration/mod.rs"]
mod calibration;
```

### Instrument Test Structure

Each instrument follows a consistent directory structure (see `instruments/README.md` for details):

```
instrument/
├── mod.rs                    # Module organization and documentation
├── construction.rs           # Builder tests, validation
├── cashflows.rs             # Cashflow generation tests
├── pricing.rs               # Core pricing engine tests
├── metrics/                 # Individual metric tests
│   ├── mod.rs
│   ├── dv01.rs
│   ├── theta.rs
│   └── ...
├── validation/              # Edge cases and boundaries
│   ├── mod.rs
│   └── edge_cases.rs
└── integration/             # Multi-metric and scenario tests
```

## Shared Test Infrastructure

### Common Test Helpers

The `common/` directory provides shared utilities used by `calibration`, `market`, `metrics`, and other test suites:

```rust
use crate::common::{
    fixtures::{base_date, usd_discount_curve, F64_ABS_TOL_STRICT},
    assertions::{assert_approx_eq, assert_relative_eq},
    builders::{TestMarketBuilder, TestOptionBuilder},
    tolerances::{STRICT_TOL, LOOSE_TOL},
};
```

### Instrument Common Helpers

The `instruments/common/` directory provides instrument-specific utilities:

```rust
use crate::common::test_helpers::{
    flat_discount_curve,    // Create flat discount curve
    flat_hazard_curve,      // Create flat hazard curve
    date,                   // Create test dates (y, m, d)
    usd, eur, gbp,          // Create Money values
    TOLERANCE,              // Standard numerical tolerance
};
```

### Parity Testing Framework

For validating against external sources (QuantLib, Bloomberg, ISDA):

```rust
use crate::parity::*;

// Default tolerance (0.01% = 1 basis point)
assert_parity!(calculated, reference);

// Custom tolerance
assert_parity!(calculated, reference, ParityConfig::tight());

// With descriptive message
assert_parity!(calculated, reference, ParityConfig::default(), "Bond PV");
```

### Golden Test Framework

The `integration/golden/` directory provides:

- JSON loaders for option pricing vectors
- Market compliance fixtures
- Tolerance-based assertion helpers

See `integration/golden/README.md` for details.

## Tolerance Policy

Tests use standardized tolerances based on calculation type:

| Tier | Constant | Value | Use Case |
|------|----------|-------|----------|
| Analytical | `tolerances::ANALYTICAL` | 1e-6 (0.0001%) | Closed-form solutions |
| Numerical | `tolerances::NUMERICAL` | 1e-4 (0.01%) | Iterative methods (Newton, trees) |
| Curve Pricing | `tolerances::CURVE_PRICING` | 5e-3 (0.5%) | Curve-based valuations |
| Relative | `tolerances::RELATIVE` | 1e-2 (1%) | Proportional comparisons |
| Bump vs Analytical | `tolerances::BUMP_VS_ANALYTICAL` | 1.5e-2 (1.5%) | Bump-and-reprice vs analytical |
| Statistical | `tolerances::STATISTICAL` | 2e-2 (2%) | Monte Carlo tests |

### Parity Configurations

| Config | Relative Tolerance | Use Case |
|--------|-------------------|----------|
| `ParityConfig::default()` | 0.01% (1 bp) | Standard financial calculations |
| `ParityConfig::tight()` | 0.001% (0.1 bp) | High-precision validation |
| `ParityConfig::loose()` | 0.1% (10 bp) | Known numerical instabilities |
| `ParityConfig::very_loose()` | 1% (100 bp) | Monte Carlo results |

### Usage Example

```rust
use crate::common::test_helpers::{tolerances, scaled_tolerance};

// For analytical calculations
assert!((computed - expected).abs() < tolerances::ANALYTICAL);

// For curve-based pricing
assert!((pv - par).abs() < notional * tolerances::CURVE_PRICING);

// For scaled tolerance (property tests)
let tol = scaled_tolerance(1e-4, intrinsic, 0.10);
assert!(price >= intrinsic - tol);
```

## Adding New Tests

### Adding an Instrument Test

1. Create a new directory under `instruments/`:

   ```
   instruments/new_instrument/
   ├── mod.rs
   ├── construction.rs
   ├── cashflows.rs
   ├── pricing.rs
   └── metrics/
       └── mod.rs
   ```

2. Add the module to `instruments.rs`:

   ```rust
   #[path = "instruments/new_instrument/mod.rs"]
   mod new_instrument;
   ```

3. Follow the standard test organization (construction, cashflows, pricing, metrics, validation).

4. Use shared helpers from `common/test_helpers.rs`.

### Adding a Calibration Test

1. Create a test file in `calibration/`:

   ```rust
   // calibration/new_calibration_test.rs
   use crate::common::fixtures::*;

   #[test]
   fn test_new_calibration_scenario() {
       // Arrange
       let quotes = build_test_quotes();

       // Act
       let curve = calibrate(&quotes)?;

       // Assert
       assert_reprices_inputs(&curve, &quotes, tolerance);
   }
   ```

2. Add the module to `calibration/mod.rs`.

### Adding a Market Test

1. Create a test file in `market/`:

   ```rust
   // market/new_quote_test.rs
   use crate::common::fixtures::F64_ABS_TOL_STRICT;

   #[test]
   fn test_new_quote_schema() {
       // ...
   }
   ```

2. Add the module to `market/mod.rs`.

### Adding a Metrics Test

1. Create a test file in `metrics/`:

   ```rust
   // metrics/new_greek_test.rs
   use crate::common::{builders::*, tolerances::*};

   #[test]
   fn test_new_greek_calculation() {
       // ...
   }
   ```

2. Add the module to `metrics/mod.rs`.

### Adding an Attribution Test

1. Create a test file in `attribution/`:

   ```rust
   // attribution/new_attribution_test.rs

   #[test]
   fn test_new_attribution_factor() {
       // ...
   }
   ```

2. Add the module to `attribution/mod.rs`.

### Adding a Golden Test

1. Add test case to appropriate JSON file in `integration/golden/data/`:

   ```json
   {
     "name": "unique_test_id",
     "description": "Human-readable description",
     // ... test parameters ...
     "expected_price": 8.916,
     "abs_tolerance": 0.05,
     "rel_tolerance": 0.005
   }
   ```

2. Set `status: "provisional"` until validated against external source.

3. Cross-reference with QuantLib, Bloomberg, or ISDA Standard Model.

4. Update status to `"certified"` when validated.

## Test Writing Guidelines

### AAA Pattern

All tests follow Arrange-Act-Assert:

```rust
#[test]
fn test_bond_pricing_at_par() {
    // Arrange: Set up test data
    let bond = Bond::fixed_coupon()
        .maturity(date!(2027, 1, 15))
        .coupon_rate(0.05)
        .build();
    let market = flat_discount_curve(0.05, as_of, "USD-OIS");

    // Act: Execute the operation
    let pv = bond.present_value(&market, as_of)?;

    // Assert: Verify results
    assert!((pv.amount() - 100.0).abs() < tolerances::CURVE_PRICING);
}
```

### Naming Conventions

- Test functions: `test_<component>_<scenario>_<expected>`
- Examples:
  - `test_ytm_par_bond_matches_coupon`
  - `test_dv01_positive_for_long_position`
  - `test_calibration_reprices_inputs`

### Slow / ignored tests

Long-running or vendor-golden tests may use `#[ignore]`. Run them explicitly:

```bash
cargo test -p finstack-quant-valuations -- --ignored
```

## Test Categories

| Category | Description | Location |
|----------|-------------|----------|
| Unit tests | Internal implementation | Source files (`#[cfg(test)]`) |
| Instrument tests | Per-instrument pricing & metrics | `instruments/` |
| Calibration tests | Curve fitting, term structures | `calibration/` |
| Market tests | Quote schemas, bumping, building | `market/` |
| Metrics tests | Greeks, sensitivities, convergence | `metrics/` |
| Attribution tests | P&L decomposition | `attribution/` |
| Golden tests | External reference validation | `integration/golden/` |
| Roundtrip tests | Serialization stability | `integration/serialization/` |
| E2E tests | Full workflow validation | `integration/e2e/` |

## Best Practices

1. **Use shared helpers** – Avoid duplicating curve builders and fixtures
2. **Descriptive names** – Tests should clearly communicate intent
3. **Single focus** – One logical assertion per test
4. **Deterministic** – Fixed seeds, no randomness in non-MC tests
5. **Isolated** – Tests don't depend on each other
6. **Fast** – Keep tests quick; use appropriate tolerance for convergence
7. **Documented** – Complex tests should have explanatory comments

## Running the Full Suite

```bash
# Via Makefile (recommended)
mise run rust-test

# Via cargo
cargo test -p finstack-quant-valuations

# With output for debugging
cargo test -p finstack-quant-valuations -- --nocapture

# In release mode (for performance validation)
cargo test -p finstack-quant-valuations --release

# Run all test entry points explicitly
cargo test --test instruments --test calibration --test market \
           --test metrics --test attribution --test cashflows --test integration
```

## Reference Sources

- **QuantLib** – Option pricing, Greeks, calibration
- **Bloomberg** – FXFA, SWPM, bond pricing
- **ISDA Standard Model** – CDS pricing conventions
- **Analytical Formulas** – Black-Scholes, Bachelier, etc.

## Contributing

When adding new tests:

1. Follow the standard directory structure
2. Use shared helpers from appropriate `common/` module
3. Provide comprehensive coverage (construction, cashflows, pricing, metrics, validation)
4. Update instrument status in `instruments/README.md` if applicable
5. Run `mise run rust-lint` and `mise run rust-test` before committing
