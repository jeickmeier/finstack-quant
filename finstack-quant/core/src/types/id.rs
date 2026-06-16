//! Phantom-typed identifiers to prevent accidental mixing of different ID types.
//!
//! This module provides compile-time type safety for string identifiers using
//! phantom types. The [`Id<T>`] type wraps a string with a type tag, preventing
//! accidental misuse of identifiers across different domains (e.g., passing a
//! curve ID where an instrument ID is expected).
//!
//! # Design Philosophy
//!
//! - **Zero-cost abstraction**: The phantom type parameter has no runtime overhead
//! - **Compile-time safety**: Different `Id<T>` types cannot be mixed accidentally
//! - **Efficient storage**: Uses `Arc<str>` for cheap cloning and minimal memory
//! - **Ergonomic conversions**: Implements `From<&str>`, `From<String>`, `Display`
//!
//! # Common ID Types
//!
//! Finstack Quant defines standard ID marker types for financial domains:
//! - [`CurveId`]: Market data curve identifiers (yield curves, credit curves)
//! - [`InstrumentId`]: Financial instrument identifiers (bonds, swaps, options)
//! - [`IndexId`]: Market index identifiers (equity or fixed income indices)
//! - [`PriceId`]: Price/scalar identifiers for market observables
//! - [`CalendarId`]: Holiday calendar identifiers for schedule generation
//! - [`PoolId`]: Pool identifiers for securitized instruments
//! - [`DealId`]: Deal identifiers for structured products
//!
//! # Examples
//!
//! ## Basic usage with compile-time safety
//!
//! ```rust
//! use finstack_quant_core::types::{CurveId, InstrumentId};
//!
//! let curve = CurveId::from("USD-OIS");
//! let bond = InstrumentId::from("US912828XG60");
//!
//! // These are different types at compile time
//! assert_eq!(curve.as_str(), "USD-OIS");
//! assert_eq!(bond.as_str(), "US912828XG60");
//!
//! // This would fail to compile (different types):
//! // let same = curve == bond;  // Compile error!
//! ```
//!
//! ## Using IDs in collections
//!
//! ```rust
//! use finstack_quant_core::types::CurveId;
//! use finstack_quant_core::HashMap;
//!
//! let mut curves = HashMap::default();
//! curves.insert(CurveId::from("USD-OIS"), 0.045);
//! curves.insert(CurveId::from("EUR-OIS"), 0.035);
//!
//! let usd_rate = curves.get(&CurveId::from("USD-OIS"));
//! assert_eq!(usd_rate, Some(&0.045));
//! ```
//!
//! ## Creating custom ID types
//!
//! ```rust
//! use finstack_quant_core::types::Id;
//!
//! // Define custom marker types for your domain
//! struct Portfolio;
//! struct Counterparty;
//!
//! type PortfolioId = Id<Portfolio>;
//! type CounterpartyId = Id<Counterparty>;
//!
//! let portfolio = PortfolioId::from("PORT-001");
//! let counterparty = CounterpartyId::from("CPTY-JP-MORGAN");
//!
//! // Type system prevents mixing these
//! assert_eq!(portfolio.as_str(), "PORT-001");
//! ```
//!
//! # Performance
//!
//! The `Id<T>` type uses `Arc<str>` internally, making clones O(1) with atomic
//! reference counting. String comparison is delegated to the underlying `str`,
//! and the phantom type marker `PhantomData<T>` has zero size and runtime cost.
//!
//! # See Also
//!
//! - [`CurveId`], [`InstrumentId`], [`IndexId`], [`PriceId`] - Common ID type aliases

use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A phantom-typed identifier that prevents mixing different kinds of IDs.
///
/// This type wraps a string identifier with a phantom type parameter to ensure
/// type safety at compile time. Different `Id<T>` types with different `T`
/// cannot be compared or mixed accidentally, preventing entire classes of bugs.
///
/// # Type Parameters
///
/// * `T` - Phantom type tag that distinguishes this ID from IDs with different tags
///
/// # Invariants
///
/// - Storage uses `Arc<str>` for efficient cloning
/// - The phantom marker has zero size and runtime cost
/// - Two `Id<T>` values are equal if their string values are equal
/// - IDs with different type tags (`Id<A>` vs `Id<B>`) cannot be compared
///
/// # Examples
///
/// ```rust
/// use finstack_quant_core::types::{CurveId, InstrumentId};
///
/// // Create IDs with different type tags
/// let curve = CurveId::from("USD-SOFR");
/// let bond = InstrumentId::from("ISIN:US912828XG60");
///
/// // Can compare IDs of the same type
/// assert_eq!(curve, CurveId::from("USD-SOFR"));
/// assert_ne!(curve, CurveId::from("EUR-ESTR"));
///
/// // Cannot compare IDs of different types (compile error):
/// // let _ = curve == bond;  // Error: mismatched types
/// ```
///
/// # Thread Safety
///
/// `Id<T>` is `Send + Sync` as it wraps an `Arc<str>`. Multiple threads can
/// safely share and clone IDs with minimal synchronization overhead.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
#[serde(deny_unknown_fields)]
#[schemars(transparent)]
pub struct Id<T> {
    value: Arc<str>,
    #[serde(skip)]
    _marker: PhantomData<T>,
}

impl<T> Id<T> {
    /// Create a new ID with the given string value.
    ///
    /// # Arguments
    ///
    /// * `value` - Any type convertible to `String` (e.g., `&str`, `String`)
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::CurveId;
    ///
    /// let id1 = CurveId::new("USD-OIS");
    /// let id2 = CurveId::new(String::from("EUR-OIS"));
    /// assert_eq!(id1.as_str(), "USD-OIS");
    /// assert_eq!(id2.as_str(), "EUR-OIS");
    /// ```
    pub fn new(value: impl Into<String>) -> Self {
        let s: String = value.into();
        Self {
            value: Arc::<str>::from(s),
            _marker: PhantomData,
        }
    }

    /// Get the string representation of this ID.
    ///
    /// Returns a reference to the underlying string without cloning.
    ///
    /// # Returns
    ///
    /// A string slice containing the ID value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::CurveId;
    ///
    /// let id = CurveId::new("USD-OIS");
    /// assert_eq!(id.as_str(), "USD-OIS");
    /// ```
    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Convert this ID into an owned `String`.
    ///
    /// Consumes the ID and returns a new `String` containing the ID value.
    /// This involves cloning the underlying string data.
    ///
    /// # Returns
    ///
    /// An owned `String` containing the ID value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::CurveId;
    ///
    /// let id = CurveId::new("USD-OIS");
    /// let s: String = id.into_string();
    /// assert_eq!(s, "USD-OIS");
    /// ```
    pub fn into_string(self) -> String {
        self.value.as_ref().to_owned()
    }

    /// Create an ID from a string slice.
    ///
    /// This is equivalent to using `From<&str>` but provides a named constructor
    /// for clarity when the conversion intent is explicit.
    ///
    /// # Arguments
    ///
    /// * `value` - String slice to create the ID from
    ///
    /// # Returns
    ///
    /// A new `Id<T>` containing the string value.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::CurveId;
    ///
    /// let id = CurveId::from_string_slice("EUR-OIS");
    /// assert_eq!(id.as_str(), "EUR-OIS");
    /// ```
    pub fn from_string_slice(value: &str) -> Self {
        Self {
            value: Arc::<str>::from(value),
            _marker: PhantomData,
        }
    }

    /// Check if this ID is empty.
    ///
    /// # Returns
    ///
    /// `true` if the ID contains an empty string, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::CurveId;
    ///
    /// let empty = CurveId::new("");
    /// let non_empty = CurveId::new("USD-OIS");
    ///
    /// assert!(empty.is_empty());
    /// assert!(!non_empty.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    /// Get the length of the ID string in bytes.
    ///
    /// # Returns
    ///
    /// The number of bytes in the ID string (not the number of characters
    /// for non-ASCII strings).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_quant_core::types::CurveId;
    ///
    /// let id = CurveId::new("USD-OIS");
    /// assert_eq!(id.len(), 7);
    /// ```
    pub fn len(&self) -> usize {
        self.value.len()
    }
}

// Implement common traits

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialEq<str> for Id<T> {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl<T> PartialEq<&str> for Id<T> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<T> PartialEq<String> for Id<T> {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<T> PartialEq<Id<T>> for str {
    fn eq(&self, other: &Id<T>) -> bool {
        other.as_str() == self
    }
}

impl<T> PartialEq<Id<T>> for &str {
    fn eq(&self, other: &Id<T>) -> bool {
        other.as_str() == *self
    }
}

impl<T> PartialEq<Id<T>> for String {
    fn eq(&self, other: &Id<T>) -> bool {
        other.as_str() == self.as_str()
    }
}

impl<T> Hash for Id<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

impl<T> From<String> for Id<T> {
    fn from(value: String) -> Self {
        Self {
            value: Arc::<str>::from(value),
            _marker: PhantomData,
        }
    }
}

impl<T> From<&str> for Id<T> {
    fn from(value: &str) -> Self {
        Self {
            value: Arc::<str>::from(value),
            _marker: PhantomData,
        }
    }
}

impl<T> AsRef<str> for Id<T> {
    fn as_ref(&self) -> &str {
        &self.value
    }
}

impl<T> Deref for Id<T> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> std::borrow::Borrow<str> for Id<T> {
    fn borrow(&self) -> &str {
        &self.value
    }
}

impl<T> std::str::FromStr for Id<T> {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self {
            value: Arc::<str>::from(s),
            _marker: PhantomData,
        })
    }
}

// Common ID marker types for finstack domains

/// Marker type for curve identifiers
#[derive(Debug, Clone, Copy, Default, JsonSchema)]
pub struct CurveTag;

/// Marker type for instrument identifiers
#[derive(Debug, Clone, Copy, Default, JsonSchema)]
pub struct InstrumentTag;

/// Marker type for index identifiers (equity or fixed income)
#[derive(Debug, Clone, Copy, Default, JsonSchema)]
pub struct IndexTag;

/// Marker type for price/market-scalar identifiers
#[derive(Debug, Clone, Copy, Default, JsonSchema)]
pub struct PriceTag;

/// Marker type for underlying asset identifiers (equity, fx, commodity)
#[derive(Debug, Clone, Copy, Default, JsonSchema)]
pub struct UnderlyingTag;

/// Marker type for holiday calendar identifiers
#[derive(Debug, Clone, Copy, Default, JsonSchema)]
pub struct CalendarTag;

/// Marker type for securitized pool identifiers
#[derive(Debug, Clone, Copy, Default, JsonSchema)]
pub struct PoolTag;

/// Marker type for structured deal identifiers
#[derive(Debug, Clone, Copy, Default, JsonSchema)]
pub struct DealTag;

/// Marker type for credit issuer identifiers.
///
/// Used by the credit factor model hierarchy to key per-issuer beta rows,
/// adder-vol state, and idiosyncratic volatility models.
#[derive(Debug, Clone, Copy, Default, JsonSchema)]
pub struct IssuerTag;

/// Type aliases for common ID types
/// Type-safe identifier for market data curves
pub type CurveId = Id<CurveTag>;
/// Type-safe identifier for financial instruments
pub type InstrumentId = Id<InstrumentTag>;
/// Type-safe identifier for market indices
pub type IndexId = Id<IndexTag>;
/// Type-safe identifier for market prices/scalars
pub type PriceId = Id<PriceTag>;
/// Type-safe identifier for underlying assets
pub type UnderlyingId = Id<UnderlyingTag>;
/// Type-safe identifier for holiday calendars and registry lookups.
///
/// This is the canonical calendar identity type used by instrument fields and
/// by [`crate::dates::CalendarRegistry`] resolution helpers.
pub type CalendarId = Id<CalendarTag>;
/// Type-safe identifier for securitized pools
pub type PoolId = Id<PoolTag>;
/// Type-safe identifier for structured deals
pub type DealId = Id<DealTag>;
/// Type-safe identifier for a credit issuer.
///
/// This is the canonical key used throughout the credit factor model hierarchy
/// in `finstack-quant-factor-model`, in `VolState::idiosyncratic`, and anywhere
/// a per-issuer lookup is required.
pub type IssuerId = Id<IssuerTag>;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct User;
    #[derive(Debug)]
    struct Account;

    #[test]
    fn id_creation_and_access() {
        let id = Id::<User>::new("user123");
        assert_eq!(id.as_str(), "user123");
        assert_eq!(id.to_string(), "user123");
        assert_eq!(id.len(), 7);
        assert!(!id.is_empty());
    }

    #[test]
    fn id_equality() {
        let id1 = Id::<User>::new("user123");
        let id2 = Id::<User>::new("user123");
        let id3 = Id::<User>::new("user456");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn id_ordering() {
        let id1 = Id::<User>::new("aaa");
        let id2 = Id::<User>::new("bbb");
        let id3 = Id::<User>::new("ccc");

        assert!(id1 < id2);
        assert!(id2 < id3);
        assert!(id1 < id3);
    }

    #[test]
    fn id_hashing() {
        use crate::collections::HashMap;

        let mut map = HashMap::default();
        let id1 = Id::<User>::new("user123");
        let id2 = Id::<User>::new("user123");

        map.insert(id1, "value1");
        assert_eq!(map.get(&id2), Some(&"value1"));
    }

    #[test]
    fn type_safety() {
        let user_id = Id::<User>::new("123");
        let account_id = Id::<Account>::new("123");

        // These should be different types even with same string value
        // Uncommenting this would cause a compile error:
        // assert_eq!(user_id, account_id);

        // But we can convert to string for comparison if needed
        assert_eq!(user_id.as_str(), account_id.as_str());
    }

    #[test]
    fn conversions() {
        let id1 = Id::<User>::new("test");
        let id2 = Id::<User>::from("test");
        let id3 = Id::<User>::from(String::from("test"));

        assert_eq!(id1, id2);
        assert_eq!(id2, id3);

        let string_val: &str = id1.as_ref();
        assert_eq!(string_val, "test");
    }

    #[test]
    fn common_id_types() {
        let curve_id = CurveId::new("USD-OIS");
        let instrument_id = InstrumentId::new("BOND123");
        let price_id = PriceId::new("AAPL");

        assert_eq!(curve_id.as_str(), "USD-OIS");
        assert_eq!(instrument_id.as_str(), "BOND123");
        assert_eq!(price_id.as_str(), "AAPL");
    }

    #[test]
    fn serde_round_trip() {
        let id = Id::<User>::new("user123");
        let json = serde_json::to_string(&id).expect("JSON serialization should succeed in test");
        let deserialized: Id<User> =
            serde_json::from_str(&json).expect("JSON deserialization should succeed in test");

        assert_eq!(id, deserialized);
    }
}
