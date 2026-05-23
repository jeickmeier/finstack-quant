---
trigger: model_decision
description: This document defines documentation standards for all public APIs in the Finstack Rust crates. Following these standards ensures consistency, maintainability, and excellent developer experience for both internal development and external users.
globs:
---
# Rust Documentation Standards for Finstack

This document defines documentation standards for all public APIs in the Finstack Rust crates. Following these standards ensures consistency, maintainability, and excellent developer experience for both internal development and external users.

## Guiding Principles

1. **Documentation is code**: Treat docs with the same rigor as implementation
2. **Examples first**: Every public API should have working examples
3. **Clarity over brevity**: Be explicit about behavior, constraints, and edge cases
4. **Testable documentation**: Use doctests to ensure examples stay correct
5. **Progressive disclosure**: Start simple, layer in complexity as needed
6. **Math/Algorithm/Financial Model**: Include source references to the canical methodologies.

## Module-Level Documentation

Every module (`mod.rs` or standalone file) must have a module-level doc comment (`//!`) that includes:

### Required Elements

1. **One-line summary**: Brief description of the module's purpose
2. **Detailed description**: What the module provides and why it exists
3. **Feature overview**: Key capabilities and types
4. **Quick example**: A working code example showing typical usage
5. **Cross-references**: Links to related modules or key types

### Module Documentation Template

```rust
//! One-line summary of what this module does.
//!
//! More detailed explanation of the module's purpose, design philosophy,
//! and how it fits into the broader system. Explain key concepts and
//! terminology used throughout the module.
//!
//! # Features
//! - Feature 1: Brief description
//! - Feature 2: Brief description
//! - Feature 3: Brief description
//!
//! # Quick Example
//! ```rust
//! use finstack_crate::module::{TypeA, TypeB};
//!
//! // Demonstrate typical usage
//! let instance = TypeA::new(args);
//! let result = instance.method();
//! # assert!(result.is_ok());
//! ```
//!
//! # See Also
//! - [`RelatedModule`] for related functionality
//! - [`KeyType`] for the main type in this module
```

### Real-World Example

```rust
//! Numerical helpers: root finding, summation, statistics, and mathematical functions.
//!
//! The implementations avoid heap allocation where possible. When the
//! `deterministic` feature is enabled, functions prefer numerically stable,
//! order-preserving algorithms.
//!
//! # Examples
//! ```rust
//! use finstack_core::math::{Solver, mean, variance};
//! use finstack_core::math::solver::NewtonSolver;
//!
//! // Root finding with Newton's method
//! let solver = NewtonSolver::new();
//! let root = solver.solve(|x| x * x - 2.0, 1.0).unwrap();
//! assert!((root - 2f64.sqrt()).abs() < 1e-9);
//!
//! // Basic statistics helpers
//! let data = [1.0, 2.0, 3.0, 4.0];
//! assert_eq!(mean(&data), 2.5);
//! ```
```

## Type Documentation (Structs & Enums)

All public structs and enums must have documentation that includes:

### Required Elements

1. **One-line description**: What the type represents
2. **Detailed description**: Purpose, invariants, and key behaviors
3. **Field documentation**: For structs with public fields
4. **Variant documentation**: For enums with public variants
5. **Usage example**: Working code showing construction and basic usage
6. **Invariants**: Constraints that are always maintained
7. **Thread safety**: Concurrency behavior if relevant

### Struct Documentation Template

```rust
/// One-line description of what this struct represents.
///
/// Detailed explanation of the struct's purpose, when to use it,
/// and important behavioral characteristics. Mention any invariants
/// that are maintained and important implementation details.
///
/// # Type Parameters
/// * `T` - Description of generic parameter and its requirements
///
/// # Invariants
/// - Invariant 1: Always true condition
/// - Invariant 2: Guaranteed property
///
/// # Examples
/// ```rust
/// use finstack_crate::TypeName;
///
/// // Demonstrate construction
/// let instance = TypeName::new(args);
///
/// // Demonstrate key operations
/// let result = instance.method();
/// assert_eq!(result, expected);
/// ```
///
/// # Thread Safety
/// This type is [Send/Sync/neither] because...
#[derive(Debug, Clone)]
pub struct TypeName<T> {
    /// Description of this field and its purpose
    pub field: T,
}
```

### Enum Documentation Template

```rust
/// One-line description of what this enum represents.
///
/// Detailed explanation of when to use each variant and the
/// relationships between them.
///
/// # Variants
/// Each variant is documented individually below with its purpose
/// and usage patterns.
///
/// # Examples
/// ```rust
/// use finstack_crate::EnumName;
///
/// // Demonstrate variant usage
/// let variant = EnumName::VariantA(value);
/// match variant {
///     EnumName::VariantA(v) => println!("Got A: {}", v),
///     EnumName::VariantB(v) => println!("Got B: {}", v),
/// }
/// ```
#[derive(Debug, Clone)]
pub enum EnumName {
    /// Description of this variant and when to use it.
    ///
    /// More details about behavior and constraints.
    VariantA(TypeA),

    /// Description of this variant and when to use it.
    ///
    /// More details about behavior and constraints.
    VariantB(TypeB),
}
```

### Real-World Example

```rust
/// Currency-tagged monetary amount with safe arithmetic.
///
/// Values are stored using a fixed-point representation derived from ISO 4217
/// decimal places. Use [`Money::new_with_config`] when you need configurable
/// rounding during ingestion.
///
/// # Invariants
/// - Currency is always valid (from ISO 4217)
/// - Amount is rounded to currency-appropriate decimal places
/// - Arithmetic operations preserve currency safety
///
/// # Examples
/// ```rust
/// use finstack_core::money::Money;
/// use finstack_core::currency::Currency;
///
/// let notional = Money::new(1_000_000.0, Currency::EUR);
/// assert_eq!(notional.currency(), Currency::EUR);
/// assert_eq!(notional.amount(), 1_000_000.0);
///
/// // Arithmetic preserves currency
/// let tax = Money::new(50_000.0, Currency::EUR);
/// let total = (notional + tax).unwrap();
/// assert_eq!(total.amount(), 1_050_000.0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Money {
    amount: AmountRepr,
    currency: Currency,
}
```

## Function Documentation

All public functions must have documentation that includes:

### Required Elements

1. **One-line description**: What the function does
2. **Detailed description**: How it works and when to use it
3. **Arguments section**: Description of each parameter with types
4. **Returns section**: Description of return value and meaning
5. **Errors section**: Conditions that cause errors (for `Result` types)
6. **Panics section**: Conditions that cause panics (if any)
7. **Examples**: Working code demonstrating usage
8. **Complexity**: Time/space complexity if non-trivial
9. **See Also**: Related functions or types

### Function Documentation Template

```rust
/// One-line description of what this function does.
///
/// More detailed explanation of the function's behavior, algorithm used,
/// and important considerations. Explain the "why" not just the "what".
///
/// # Arguments
///
/// * `param1` - Description of first parameter, including valid ranges,
///   special values, or constraints
/// * `param2` - Description of second parameter with type information
///   and requirements
///
/// # Returns
///
/// Description of the return value, its meaning, and interpretation.
/// For `Result`, describe the success case. For `Option`, when `None` is returned.
///
/// # Errors
///
/// Returns `Err` when:
/// - Condition 1 triggers specific error type
/// - Condition 2 triggers specific error type
///
/// # Panics
///
/// Panics if:
/// - Precondition 1 is violated
/// - Precondition 2 is violated
///
/// # Examples
///
/// Basic usage:
/// ```rust
/// use finstack_crate::function_name;
///
/// let result = function_name(arg1, arg2)?;
/// assert_eq!(result, expected);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Advanced usage:
/// ```rust
/// use finstack_crate::function_name;
///
/// // Demonstrate edge case or advanced pattern
/// let result = function_name(special_arg1, special_arg2)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Complexity
///
/// Time: O(n log n), Space: O(n)
///
/// # See Also
///
/// - [`related_function`] for related functionality
/// - [`RelatedType`] for type used in implementation
pub fn function_name(param1: Type1, param2: Type2) -> Result<ReturnType, Error> {
    // implementation
}
```

### Method Documentation Template

Methods follow the same template but may omit `self` from the Arguments section if its role is obvious.

```rust
/// One-line description of what this method does.
///
/// Detailed explanation of behavior and when to use this method.
///
/// # Arguments
///
/// * `param` - Description of parameter (omit `self` if obvious)
///
/// # Returns
///
/// Description of return value
///
/// # Examples
///
/// ```rust
/// use finstack_crate::TypeName;
///
/// let instance = TypeName::new();
/// let result = instance.method(arg);
/// assert_eq!(result, expected);
/// ```
pub fn method(&self, param: Type) -> ReturnType {
    // implementation
}
```

### Real-World Example

```rust
/// Format the amount with custom decimals and optional currency symbol.
///
/// The amount is rounded to the specified number of decimal places using
/// banker's rounding (round half to even) before formatting.
///
/// # Arguments
///
/// * `decimals` - Number of decimal places to display (0 or greater)
/// * `show_currency` - Whether to include currency code in output
///
/// # Returns
///
/// A formatted string representation of the monetary amount. If `show_currency`
/// is true, returns format "value CURRENCY" (e.g., "1042315.67 USD").
/// Otherwise returns just the numeric value (e.g., "1042315.67").
///
/// # Examples
///
/// ```
/// use finstack_core::money::Money;
/// use finstack_core::currency::Currency;
///
/// let amount = Money::new(1_042_315.67, Currency::USD);
/// assert_eq!(amount.format(2, true), "1042315.67 USD");
/// assert_eq!(amount.format(2, false), "1042315.67");
/// assert_eq!(amount.format(0, true), "1042316 USD");
/// ```
pub fn format(&self, decimals: usize, show_currency: bool) -> String {
    // implementation
}
```

## Trait Documentation

Traits require documentation similar to structs, plus:

### Required Elements

1. **Purpose**: What the trait abstracts and why
2. **Required methods**: What implementors must provide
3. **Provided methods**: What comes for free
4. **Implementation guide**: How to implement correctly
5. **Usage example**: How to use trait objects or generics
6. **Implementation example**: How to implement for a type

### Trait Documentation Template

```rust
/// One-line description of what this trait represents.
///
/// Detailed explanation of the trait's purpose, the abstraction it provides,
/// and when to implement or use it.
///
/// # Required Methods
///
/// Implementors must provide:
/// - [`method1`](Self::method1): Description of what it must do
/// - [`method2`](Self::method2): Description of what it must do
///
/// # Provided Methods
///
/// Default implementations are provided for:
/// - [`method3`](Self::method3): Description of default behavior
///
/// # Implementation Guide
///
/// When implementing this trait:
/// 1. Ensure invariant X is maintained
/// 2. Consider performance implications of Y
/// 3. Handle edge case Z appropriately
///
/// # Examples
///
/// Using the trait:
/// ```rust
/// use finstack_crate::TraitName;
///
/// fn use_trait<T: TraitName>(instance: T) {
///     instance.method();
/// }
/// ```
///
/// Implementing the trait:
/// ```rust
/// use finstack_crate::TraitName;
///
/// struct MyType;
///
/// impl TraitName for MyType {
///     fn method(&self) -> ReturnType {
///         // implementation
///     }
/// }
/// ```
pub trait TraitName {
    /// Description of required method
    fn method(&self) -> ReturnType;

    /// Description of provided method with default implementation
    fn provided_method(&self) -> bool {
        true
    }
}
```

## Doctest Best Practices

### General Guidelines

1. **All examples must compile and run**: Use `cargo test --doc`
2. **Hide boilerplate**: Use `#` prefix for setup code users don't need to see
3. **Assert results**: Show expected behavior explicitly
4. **Handle errors**: Use `?` or `unwrap()` appropriately
5. **Use realistic data**: Examples should reflect actual usage

### Common Patterns

**Basic example with hidden setup:**

```rust
/// # Examples
/// ```rust
/// use finstack_core::money::Money;
/// # use finstack_core::currency::Currency;
///
/// let amount = Money::new(100.0, Currency::USD);
/// # assert_eq!(amount.amount(), 100.0);
/// ```
```

**Example with error handling:**

```rust
/// # Examples
/// ```rust
/// use finstack_core::function_name;
///
/// let result = function_name(args)?;
/// assert!(result > 0.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
```

**Example that should not run:**

```rust
/// # Examples
/// ```rust,no_run
/// use finstack_core::expensive_operation;
///
/// // This example demonstrates the API but won't run in tests
/// let result = expensive_operation();
/// ```
```

**Example that should fail to compile:**

```rust
/// # Examples
/// This will not compile:
/// ```rust,compile_fail
/// use finstack_core::TypeName;
///
/// let instance = TypeName::new();
/// instance.method_that_doesnt_exist(); // Compilation error
/// ```
```

**Example showing expected panic:**

```rust
/// # Examples
/// ```should_panic
/// use finstack_core::function_name;
///
/// function_name(invalid_arg); // Will panic
/// ```
```

## Documentation for Different Visibility Levels

### Public API (`pub`)

**Must have:** Full documentation as described above, including examples

```rust
/// Comprehensive documentation with all sections and examples.
///
/// # Arguments
/// ...
///
/// # Examples
/// ...
pub fn public_function() { }
```

### Public but Internal (`pub(crate)`)

**Must have:** Description and purpose; examples optional but encouraged

```rust
/// Internal helper for X operation used by Y and Z modules.
///
/// Called during initialization to set up required state.
pub(crate) fn internal_helper() { }
```

### Private

**Should have:** Brief comment explaining purpose; full docs if complex

```rust
/// Validates input meets invariant requirements.
fn validate_input(x: f64) -> bool { }
```

## Special Documentation Sections

### Constants

Document the meaning, units, and rationale:

```rust
/// Default forward rate used as fallback when converting discount curves.
///
/// This value (4.5%) represents a typical mid-cycle interest rate for major
/// currencies and is used only when insufficient data prevents computing
/// forward rates from discount factor differences.
const DEFAULT_FORWARD_RATE_FALLBACK: f64 = 0.045;
```

### Type Aliases

Explain why the alias exists and when to use it:

```rust
/// Type alias for discount curve identifiers.
///
/// Used to distinguish curve IDs from generic strings and prevent
/// accidental mismatches between different ID types.
pub type CurveId = String;
```

### Macros

Document parameters, expansion, and usage:

```rust
/// Generates boilerplate for instrument registration.
///
/// # Arguments
///
/// * `$name` - Name of the instrument type
/// * `$pricer` - Pricer implementation to register
///
/// # Examples
///
/// ```ignore
/// register_instrument!(Bond, BondPricer);
/// ```
macro_rules! register_instrument {
    // implementation
}
```

## Error Documentation

Document errors comprehensively for excellent user experience:

### Error Enum

```rust
/// Errors that can occur during market data operations.
///
/// Each variant represents a specific failure mode with context
/// to help diagnose and resolve issues.
#[derive(Debug, thiserror::Error)]
pub enum MarketDataError {
    /// Curve with the given ID was not found in the market context.
    ///
    /// This typically indicates a configuration issue where the
    /// pricing code expects a curve that wasn't loaded.
    #[error("Curve not found: {curve_id}")]
    CurveNotFound {
        /// The ID of the missing curve
        curve_id: String,
    },

    /// Interpolation failed due to invalid input.
    ///
    /// This can occur when querying outside the valid range with
    /// strict extrapolation policies.
    #[error("Interpolation failed: {reason}")]
    InterpolationError {
        /// Explanation of why interpolation failed
        reason: String,
    },
}
```

## Documentation for Feature-Gated Code

Use `cfg_attr` and document feature requirements:

```rust
/// Monte Carlo pricing engine for derivative valuation.
///
/// This module is only available when the `mc` feature is enabled.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "mc")]
/// use finstack_valuations::instruments::common::mc::prelude::*;
///
/// # #[cfg(feature = "mc")]
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = EuropeanPricerConfig::new(100_000);
/// let pricer = EuropeanPricer::new(config);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "mc")]
pub mod mc {
    // implementation
}
```

## Academic and Industry References

Financial computation requires rigorous verification against established research and standards. Always cite authoritative sources for mathematical models and industry conventions.

### Mathematical and Algorithmic Citations

For pricing models, numerical methods, and algorithms, include:

1. **Primary research papers** with authors and publication year
2. **Specific equation numbers or page references** when applicable
3. **Alternative formulations** if your implementation differs
4. **Known limitations or assumptions** from the literature

**Template for mathematical citations:**

```rust
/// Computes European option price using the Black-Scholes-Merton model.
///
/// Implements the analytical solution from Black & Scholes (1973) and
/// Merton (1973) for pricing European call and put options under
/// geometric Brownian motion with constant volatility.
///
/// # Mathematical Foundation
///
/// The closed-form solution is:
/// ```text
/// C = S₀ * N(d₁) - K * e^(-rT) * N(d₂)
/// P = K * e^(-rT) * N(-d₂) - S₀ * N(-d₁)
///
/// where:
///   d₁ = [ln(S₀/K) + (r - q + σ²/2)T] / (σ√T)
///   d₂ = d₁ - σ√T
/// ```
///
/// # References
///
/// - Black, F., & Scholes, M. (1973). "The Pricing of Options and Corporate
///   Liabilities." *Journal of Political Economy*, 81(3), 637-654.
/// - Merton, R. C. (1973). "Theory of Rational Option Pricing." *Bell Journal
///   of Economics and Management Science*, 4(1), 141-183.
///
/// # Implementation Notes
///
/// This implementation uses the standard formulation with:
/// - Continuous dividend yield `q` (can be set to 0 for non-dividend paying assets)
/// - Standard normal CDF via error function approximation (accurate to 1e-6)
///
/// # Arguments
/// ...
```

**Real-world example for Monte Carlo:**

```rust
/// Longstaff-Schwartz Least Squares Monte Carlo for American option pricing.
///
/// Implements the backward induction algorithm with least-squares regression
/// to estimate continuation values at exercise dates.
///
/// # Algorithm
///
/// 1. Simulate forward paths under risk-neutral measure
/// 2. Backward induction from maturity:
///    - Regress continuation value on basis functions of state variables
///    - Compare immediate exercise vs continuation value
///    - Update optimal exercise decision
/// 3. Forward pass with optimal exercise to compute price
///
/// # Mathematical Foundation
///
/// The continuation value at time t is estimated by:
/// ```text
/// C(S_t) = E[V_{t+1} | S_t] ≈ Σ_j β_j φ_j(S_t)
/// ```
/// where φ_j are basis functions (e.g., Laguerre polynomials) and β_j
/// are coefficients from least-squares regression on in-the-money paths.
///
/// # References
///
/// - Longstaff, F. A., & Schwartz, E. S. (2001). "Valuing American Options
///   by Simulation: A Simple Least-Squares Approach." *Review of Financial
///   Studies*, 14(1), 113-147.
/// - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*.
///   Springer. Chapter 8, pp. 449-458.
///
/// # Implementation Details
///
/// - Basis functions: Laguerre polynomials up to degree 3
/// - Regression: Only on in-the-money paths (as per Longstaff-Schwartz)
/// - Numerical stability: Uses QR decomposition for least squares
///
/// # Arguments
/// ...
```

**Example for variance reduction:**

```rust
/// Antithetic variates for variance reduction in Monte Carlo simulation.
///
/// For each random draw Z ~ N(0,1), generates paired path with -Z,
/// reducing variance of the estimator by exploiting negative correlation.
///
/// # Theoretical Foundation
///
/// If f is monotonic, antithetic pairs (Z, -Z) satisfy:
/// ```text
/// Var[(f(Z) + f(-Z))/2] ≤ Var[f(Z)]
/// ```
/// Variance reduction is strongest when f is monotonic and symmetric payoffs
/// may see little benefit.
///
/// # References
///
/// - Glasserman, P. (2003). *Monte Carlo Methods in Financial Engineering*.
///   Springer. Section 4.1, pp. 185-189.
/// - Hammersley, J. M., & Handscomb, D. C. (1964). *Monte Carlo Methods*.
///   Methuen, London.
///
/// # Effectiveness
///
/// Typical variance reduction: 20-40% for path-dependent options with
/// monotonic payoffs. May be ineffective for barrier options with
/// discontinuous payoffs.
///
/// # Arguments
/// ...
```

### Industry Standards and Conventions

For implementations of industry standards, reference authoritative sources:

1. **Primary specification documents** (ISDA, ISO, regulatory bodies)
2. **Version numbers and publication dates** to ensure traceability
3. **Specific section references** within standards documents
4. **Known ambiguities or edge cases** in the standard

**Template for day count conventions:**

```rust
/// Actual/360 day count convention.
///
/// Counts actual calendar days between dates and divides by 360-day year,
/// commonly used in money market instruments and short-term interest rate
/// calculations.
///
/// # Definition
///
/// Day count fraction = (Date₂ - Date₁) / 360
///
/// where (Date₂ - Date₁) is the actual number of calendar days.
///
/// # Standards Reference
///
/// - ISDA: 2006 ISDA Definitions, Section 4.16(d)
/// - ISO: ISO 20022 Day Count Fraction Code "Actual/360" (A004)
/// - Also known as: Actual/360, Act/360, French
///
/// # Usage
///
/// Commonly used for:
/// - USD money market deposits
/// - EUR money market instruments
/// - Short-term rate derivatives (e.g., SOFR, €STR)
///
/// # Examples
/// ...
```

**Example for business day conventions:**

```rust
/// Modified Following business day convention.
///
/// Adjusts dates to the next business day unless that would cross into the
/// next month, in which case adjusts to the previous business day.
///
/// # Definition
///
/// If date D falls on a non-business day:
/// 1. Adjust to the next business day D'
/// 2. If month(D') ≠ month(D), adjust to previous business day instead
///
/// # Standards Reference
///
/// - ISDA: 2006 ISDA Definitions, Section 4.12(b)
/// - FpML: BusinessDayConventionEnum value "MODFOLLOWING"
/// - ISO 20022: Code "MODF"
///
/// # Rationale
///
/// Prevents month-end dates from shifting into the following month,
/// which is important for consistent period calculations in swaps
/// and other derivatives.
///
/// # Examples
/// ...
```

**Example for holiday calendars:**

```rust
/// New York Stock Exchange (NYSE) holiday calendar.
///
/// Implements the official NYSE trading holiday schedule including:
/// - New Year's Day
/// - Martin Luther King Jr. Day
/// - Presidents' Day
/// - Good Friday
/// - Memorial Day
/// - Juneteenth National Independence Day (since 2021)
/// - Independence Day
/// - Labor Day
/// - Thanksgiving Day
/// - Christmas Day
///
/// # Primary Source
///
/// - NYSE Official Holiday Schedule:
///   https://www.nyse.com/markets/hours-calendars
/// - Rules: NYSE Rule 7.1 - Days and Hours of Business
///
/// # Special Rules
///
/// - When holiday falls on Saturday: observed previous Friday
/// - When holiday falls on Sunday: observed following Monday
/// - Good Friday: always observed (unique to financial markets)
/// - Early close days (e.g., day after Thanksgiving): not marked as holidays
///
/// # Historical Changes
///
/// - Juneteenth added as federal holiday starting June 2021
/// - Calendar may be updated by NYSE; check primary source for current year
///
/// # Examples
/// ...
```

**Example for credit models:**

```rust
/// CDS pricing using ISDA Standard Model (ISDA CDS Standard Model v1.8.2).
///
/// Implements the industry-standard CDS pricing methodology with conventions
/// matching major credit indices (CDX, iTraxx).
///
/// # Model Description
///
/// Present value of protection leg:
/// ```text
/// PV_prot = (1 - R) * Σ P(t_i) * (DF(t_i-1) - DF(t_i))
/// ```
///
/// Present value of premium leg with accrual-on-default:
/// ```text
/// PV_prem = S * Σ Δt_i * DF(t_i) * P(t_i) + accrual_on_default
/// ```
///
/// # Standards Reference
///
/// - ISDA: ISDA CDS Standard Model v1.8.2 (October 2009)
///   https://www.cdsmodel.com/
/// - Conventions: ISDA CDS Standard Model Specification
/// - Big Bang Protocol (April 2009) for standardized terms
///
/// # Implementation Details
///
/// - Step-in date: T+1 from valuation date
/// - Coupon dates: IMM dates (20th of Mar/Jun/Sep/Dec)
/// - Day count: ACT/360 for premium leg
/// - Integration: Piecewise constant hazard rate
///
/// # References
///
/// - O'Kane, D. (2008). *Modelling Single-name and Multi-name Credit
///   Derivatives*. Wiley Finance. Chapter 4.
/// - ISDA (2009). "ISDA CDS Standard Model Specification."
///
/// # Arguments
/// ...
```

### Documentation Style Guide

### Language and Tone

- **Use present tense**: "Returns a value" not "Will return a value"
- **Use active voice**: "Computes the discount factor" not "The discount factor is computed"
- **Be concise but complete**: Every word should add value
- **Use consistent terminology**: Maintain the project's vocabulary
- **Avoid jargon**: Explain domain terms when first used

### Formatting

- **One blank line** between doc comment sections
- **Capitalize** section headers (# Arguments, # Returns)
- **Use backticks** for code: types, functions, parameters
- **Use links** for cross-references: `[`Type`]` or `[`module::function`]`
- **Use lists** for multiple items or conditions
- **Code blocks** for examples must specify language: `rust`

### Mathematical Notation

When documenting financial or mathematical concepts:

```rust
/// Computes the Black-Scholes call option price.
///
/// Uses the closed-form solution:
/// ```text
/// C = S₀ * N(d₁) - K * e^(-rT) * N(d₂)
/// where:
///   d₁ = [ln(S₀/K) + (r + σ²/2)T] / (σ√T)
///   d₂ = d₁ - σ√T
/// ```
///
/// # Arguments
///
/// * `spot` - Current asset price (S₀)
/// * `strike` - Strike price (K)
/// * `rate` - Risk-free rate (r, annualized)
/// * `vol` - Volatility (σ, annualized)
/// * `time` - Time to maturity (T, in years)
///
/// # Returns
///
/// Call option value in the same currency as `spot`
pub fn black_scholes_call(
    spot: f64,
    strike: f64,
    rate: f64,
    vol: f64,
    time: f64,
) -> f64 {
    // implementation
}
```

## Documentation Testing

### Running Doctests

```bash
# Test all documentation examples
cargo test --doc

# Test specific crate
cargo test --doc -p finstack-core

# Test with features
cargo test --doc --all-features
```

### Ensuring Quality

1. **All public APIs must have doctests** that compile and pass
2. **Examples must be realistic**: Use actual types and realistic data
3. **Test edge cases in docs**: Document and test boundary conditions
4. **Keep examples focused**: One concept per example
5. **Update docs with code**: Treat documentation failures as code failures

## Common Patterns for Finstack

### Currency-Safe Operations

```rust
/// Adds two monetary amounts in the same currency.
///
/// # Arguments
///
/// * `other` - Amount to add, must be in the same currency
///
/// # Returns
///
/// `Ok(Money)` with the sum if currencies match, `Err` otherwise
///
/// # Errors
///
/// Returns `Error::CurrencyMismatch` if the currencies differ.
///
/// # Examples
///
/// ```rust
/// use finstack_core::money::Money;
/// use finstack_core::currency::Currency;
///
/// let a = Money::new(100.0, Currency::USD);
/// let b = Money::new(50.0, Currency::USD);
/// let sum = (a + b)?;
/// assert_eq!(sum.amount(), 150.0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn add(&self, other: Money) -> Result<Money, Error> {
    // implementation
}
```

### Builder Patterns

```rust
/// Builder for constructing discount curves with validation.
///
/// # Examples
///
/// ```rust
/// use finstack_core::market_data::term_structures::DiscountCurve;
/// use finstack_core::dates::Date;
/// # use time::Month;
///
/// let curve = DiscountCurve::builder("USD-OIS")
///     .base_date(Date::from_calendar_date(2025, Month::January, 1).unwrap())
///     .knots([(0.0, 1.0), (1.0, 0.95), (5.0, 0.8)])
///     .build()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn builder(id: impl Into<CurveId>) -> DiscountCurveBuilder {
    // implementation
}
```

### Deterministic Operations

```rust
/// Performs Monte Carlo simulation with deterministic reproducibility.
///
/// The same seed produces identical results across runs and thread counts,
/// ensuring reproducible results for testing and production pipelines.
///
/// # Arguments
///
/// * `seed` - RNG seed for deterministic path generation
/// * `num_paths` - Number of paths to simulate
///
/// # Returns
///
/// `PricingResult` with value, standard error, and metadata
///
/// # Examples
///
/// ```rust
/// # use finstack_valuations::instruments::common::mc::prelude::*;
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let config = EuropeanPricerConfig::new(10_000)
///     .with_seed(42); // Deterministic seed
///
/// let result1 = pricer.price(&process, spot, &payoff, config.clone())?;
/// let result2 = pricer.price(&process, spot, &payoff, config)?;
///
/// // Identical results with same seed
/// assert_eq!(result1.value, result2.value);
/// # Ok(())
/// # }
/// ```
pub fn price_with_seed(&self, seed: u64, num_paths: usize) -> PricingResult {
    // implementation
}
```

## Documentation Review Checklist

Before marking documentation as complete, verify:

- [ ] Module has doc comment with example
- [ ] All public types have descriptions and examples
- [ ] All public functions have:
  - [ ] One-line description
  - [ ] Arguments documented with types
  - [ ] Return value documented
  - [ ] Errors documented (for Result types)
  - [ ] At least one working example
- [ ] Examples compile with `cargo test --doc`
- [ ] Examples use realistic data and patterns
- [ ] Links to related types/functions work
- [ ] Feature gates are documented
- [ ] Mathematical formulas are clear (if applicable)
- [ ] Currency safety is explicit (if applicable)
- [ ] Determinism guarantees stated (if applicable)
- [ ] **Academic citations included** for:
  - [ ] Pricing models (with paper authors, year, journal)
  - [ ] Numerical algorithms (with specific references)
  - [ ] Variance reduction techniques
  - [ ] Stochastic processes and discretization schemes
- [ ] **Industry standards referenced** for:
  - [ ] Day count conventions (ISDA section numbers)
  - [ ] Business day conventions (ISDA/ISO codes)
  - [ ] Holiday calendars (primary source links)
  - [ ] Market conventions (with version/date)
- [ ] Citations include:
  - [ ] Full bibliographic information (author, year, title, journal/book)
  - [ ] Specific page numbers or equation references where applicable
  - [ ] Version numbers for industry standards
  - [ ] Links to primary sources when available

## References and Resources

- [Rust API Guidelines - Documentation](https://rust-lang.github.io/api-guidelines/documentation.html)
- [rustdoc Book](https://doc.rust-lang.org/rustdoc/)
- [RFC 1574 - More API Documentation Conventions](https://rust-lang.github.io/rfcs/1574-more-api-documentation-conventions.html)

---

**Remember**: Documentation is not just about explaining what the code does—it's about teaching users how to use the API effectively, helping them avoid pitfalls, and showcasing the power and safety of the Finstack library.
