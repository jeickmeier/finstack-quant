# Documentation Standards Reference

## Rust documentation conventions

### Module documentation

Every module should have a `//!` doc comment at the top:

```rust
//! Brief module description.
//!
//! Extended description of what the module provides,
//! its main types, and how to use them.
//!
//! # Examples
//!
//! ```rust
//! use crate::module::MainType;
//!
//! let thing = MainType::new();
//! ```
```

### Struct documentation

```rust
/// Brief description of the struct.
///
/// Extended description explaining:
/// - What this type represents
/// - When to use it
/// - Any important invariants
///
/// # Examples
///
/// ```rust
/// let instance = MyStruct::new(value);
/// ```
pub struct MyStruct {
    /// Description of this field.
    pub field1: Type1,
    /// Description with units or constraints.
    /// Value must be non-negative.
    pub field2: f64,
}
```

### Enum documentation

```rust
/// Brief description of the enum.
///
/// Extended description of what choices this enum represents.
pub enum MyEnum {
    /// First variant - when to use it.
    Variant1,
    /// Second variant with associated data.
    ///
    /// The inner value represents...
    Variant2(InnerType),
}
```

### Trait documentation

```rust
/// Brief description of what implementors provide.
///
/// # Required methods
///
/// Implementors must define:
/// - `method1`: for doing X
/// - `method2`: for doing Y
///
/// # Examples
///
/// ```rust
/// struct MyImpl;
///
/// impl MyTrait for MyImpl {
///     fn method1(&self) -> Output {
///         // implementation
///     }
/// }
/// ```
pub trait MyTrait {
    /// Description of this required method.
    fn method1(&self) -> Output;
}
```

### Error handling documentation

```rust
/// Brief description.
///
/// # Errors
///
/// Returns `Err` if:
/// - Input is negative
/// - Curve lookup fails
///
/// # Panics
///
/// Panics if `debug_assertions` are enabled and invariant X is violated.
pub fn fallible_function() -> Result<T, Error> {
    // ...
}
```

## Python documentation conventions

### NumPy docstring style

This project uses NumPy-style docstrings for Python.

### Class documentation

```python
class MyClass:
    """Brief description of the class.

    Extended description explaining:
    - What this type represents
    - When to use it
    - Any important invariants

    Attributes
    ----------
    field1 : Type1
        Description of this attribute.
    field2 : float
        Description with units or constraints.

    Examples
    --------
    >>> obj = MyClass(value1, value2)
    >>> obj.field1
    expected_value
    """

    def __init__(self, field1: Type1, field2: float) -> None:
        """Initialize the instance.

        Parameters
        ----------
        field1 : Type1
            Description of first parameter.
        field2 : float
            Description with constraints (must be non-negative).
        """
```

### Method documentation

```python
def calculate_price(
    self,
    spot: float,
    strike: float,
    time_to_expiry: float,
    volatility: float,
) -> float:
    """Calculate the option price using Black-Scholes.

    Computes the price of a European call option under the
    Black-Scholes-Merton framework with continuous dividend yield.

    Parameters
    ----------
    spot : float
        Current spot price of the underlying.
    strike : float
        Option strike price.
    time_to_expiry : float
        Time to expiry in years (ACT/365 basis).
    volatility : float
        Annualized volatility (e.g., 0.20 for 20%).

    Returns
    -------
    float
        Option price in the same units as spot.

    Raises
    ------
    ValueError
        If time_to_expiry is negative.
    ValueError
        If volatility is non-positive.

    Examples
    --------
    >>> pricer = OptionPricer(rate=0.05, dividend_yield=0.02)
    >>> price = pricer.calculate_price(100.0, 100.0, 1.0, 0.20)
    >>> round(price, 4)
    10.4506

    Sources
    -------
    - Black-Scholes (1973): see docs/REFERENCES.md#blackScholes1973
    - Merton (1973): see docs/REFERENCES.md#merton1973
    """
```

## Academic reference format

### In-code references

Reference the canonical entry in `docs/REFERENCES.md`:

**Rust:**
```rust
/// # References
///
/// - Black (1976): see docs/REFERENCES.md#black1976
/// - Hull: Options, Futures, and Other Derivatives, Ch. 19
```

**Python:**
```python
"""
Sources
-------
- Black (1976): see docs/REFERENCES.md#black1976
- Hull: Options, Futures, and Other Derivatives, Ch. 19
"""
```

### When to add references

| Code type | Reference required |
|-----------|-------------------|
| Pricing model | Yes - cite original paper |
| Day count convention | Yes - cite ISDA or market standard |
| Greeks formula | Yes - cite derivation source |
| Monte Carlo technique | Yes - cite methodology paper |
| Curve interpolation | Yes if non-trivial (e.g., monotonic cubic) |
| Standard algorithm | Only if non-obvious implementation |

### Standard reference keys

Use these anchor keys from `docs/REFERENCES.md`:

| Model | Key |
|-------|-----|
| Black-Scholes | `#blackScholes1973` |
| Black-76 | `#black1976` |
| Merton | `#merton1973` |
| Bachelier | `#bachelier1900` |
| ISDA definitions | `#isda2006Definitions` |
| Garman-Kohlhagen | `#garmanKohlhagen1983` |
| Heston | `#heston1993` |
| SABR | `#haganSABR2002` |
| Hull textbook | `#hullOptionsFuturesDerivatives` |
| Brigo-Mercurio | `#brigoMercurio2006` |
| O'Kane credit | `#okane2008` |
| Variance swaps | `#demeterfiVarianceSwaps1999` |

## Quality standards

### Description quality

**Good:**
> Calculate the present value of a fixed-rate bond by discounting
> projected cashflows using the provided discount curve.

**Bad:**
> Calculate PV.

### Argument documentation quality

**Good:**
```
* `settlement` - Settlement date; cashflows before this date are excluded
```

**Bad:**
```
* `settlement` - The settlement date
```

### Example quality

**Good:**
```rust
/// ```rust
/// use finstack_valuations::pricer::PricerRegistry;
///
/// let registry = PricerRegistry::builder()
///     .with_rates()
///     .with_credit()
///     .build();
///
/// assert!(registry.get(InstrumentType::Bond, ModelKey::Discounting).is_some());
/// ```
```

**Bad:**
```rust
/// ```rust
/// let x = my_function();
/// ```
```

## Documenting conventions

When code relies on financial conventions, document them explicitly:

```rust
/// Calculate accrued interest.
///
/// # Conventions
///
/// - Day count: ACT/ACT (ISDA) per `isda2006Definitions`
/// - Settlement: T+2 business days
/// - Accrual direction: buyer pays seller
///
/// # Arguments
/// ...
```

## Documenting numerical precision

For numerical code, note precision characteristics:

```rust
/// Compute implied volatility via Newton-Raphson.
///
/// # Numerical notes
///
/// - Convergence tolerance: 1e-8 relative
/// - Maximum iterations: 100
/// - Initial guess: Brenner-Subrahmanyam approximation
/// - May not converge for deep OTM options
///
/// # Arguments
/// ...
```
