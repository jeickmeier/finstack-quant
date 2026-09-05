//! Position types for holding instruments in a portfolio.

use crate::book::BookId;
use crate::error::{Error, Result};
use crate::types::{AttributeValue, EntityId, PositionId};
use finstack_quant_core::currency::Currency;
use finstack_quant_core::money::Money;
use finstack_quant_valuations::instruments::{Instrument, InstrumentJson};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Unit of position measurement.
///
/// The unit describes how the `quantity` on a [`Position`] should be interpreted.
/// Callers should treat it as part of the valuation contract, not display-only
/// metadata.
///
/// # Scaling contract
///
/// Position value is ``scale_factor(unit) * instrument.value()``. The instrument
/// is already built with its deal notional (or face, or one share). Each variant
/// defines the scale factor explicitly:
///
/// | Variant       | Scale factor              | Quantity interpretation              |
/// |---------------|---------------------------|--------------------------------------|
/// | `Units`       | `quantity`                | Number of instrument units/shares    |
/// | `Notional(_)` | `quantity`                | Lot multiplier (`1` = one deal)      |
/// | `FaceValue`   | `quantity`                | Held face-value multiplier           |
/// | `Percentage`  | `quantity / 100`          | Percentage points of the instrument  |
///
/// ## `Notional` semantics
///
/// `Notional(ccy)` means the instrument stores the deal notional and
/// `quantity` is a lot multiplier: `1` is one deal, `2` is two deals.
/// Position PV is ``quantity × instrument.value()``. Do not build the
/// instrument with unit notional of 1 and put the dollar notional in
/// `quantity`.
///
/// Optional `Notional(Some(ccy))` only validates currency against the
/// instrument's native PV currency. It does not change the scale factor.
/// A warning is emitted when it disagrees with the instrument's valuation
/// currency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum PositionUnit {
    /// Number of units/shares (for equities, baskets). Scale factor = `quantity`.
    Units,

    /// Lot multiplier for a notional-quoted instrument, optionally tagged
    /// with a currency (for derivatives, FX).
    ///
    /// Scale factor = `quantity`. The instrument already carries deal
    /// notional; `1` is one deal and `2` is two deals. The optional
    /// [`Currency`] is a validation tag and does not alter scaling.
    Notional(Option<Currency>),

    /// Face value of debt instruments (for bonds, loans). Scale factor = `quantity`.
    FaceValue,

    /// Percentage of ownership where the value represents percentage points.
    ///
    /// For example, 50.0 means 50%, not 0.50. The scaling logic always divides
    /// by 100 to convert to a decimal multiplier.
    Percentage,
}

/// A position in an instrument.
///
/// Represents a holding of a specific quantity of an instrument,
/// belonging to an entity. Positions track the instrument reference,
/// quantity, and metadata for aggregation and analysis.
#[derive(Clone)]
pub struct Position {
    /// Unique identifier for this position
    pub position_id: PositionId,

    /// Entity that owns this position
    pub entity_id: EntityId,

    /// Instrument identifier (for reference/lookup)
    pub instrument_id: String,

    /// The actual instrument being held
    pub instrument: Arc<dyn Instrument>,

    /// Signed quantity (positive=long, negative=short).
    ///
    /// For [`PositionUnit::Notional`], this is a lot multiplier (`1` = one
    /// deal), not a dollar notional.
    pub quantity: f64,

    /// Unit of measurement for the quantity
    pub unit: PositionUnit,

    /// Optional book identifier for hierarchical organization
    pub book_id: Option<BookId>,

    /// Position-level attributes for grouping, filtering, and constraints
    pub attributes: IndexMap<String, AttributeValue>,

    /// Additional metadata
    pub meta: IndexMap<String, serde_json::Value>,
}

/// Serializable position specification (without `Arc<dyn Instrument>`).
///
/// This struct allows positions to be serialized and deserialized by storing
/// the instrument definition as JSON rather than a trait object.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PositionSpec {
    /// Position identifier
    pub position_id: PositionId,
    /// Entity identifier
    pub entity_id: EntityId,
    /// Instrument identifier (for reference/lookup)
    pub instrument_id: String,
    /// Instrument definition for full serialization (optional)
    ///
    /// If `None`, the position can still be serialized but cannot be
    /// reconstructed without an external instrument registry.
    pub instrument_spec: Option<InstrumentJson>,
    /// Signed quantity. For [`PositionUnit::Notional`], a lot multiplier.
    pub quantity: f64,
    /// Unit of measurement
    pub unit: PositionUnit,
    /// Optional book identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_id: Option<BookId>,
    /// Position-level attributes for grouping, filtering, and constraints
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub attributes: IndexMap<String, AttributeValue>,
    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,
}

impl Position {
    /// Create a new position.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Unique identifier for the position.
    /// * `entity_id` - Owning entity identifier.
    /// * `instrument_id` - Identifier of the underlying instrument.
    /// * `instrument` - Shared pointer to the instrument implementation.
    /// * `quantity` - Signed quantity of the instrument (must be finite).
    /// * `unit` - Interpretation of the quantity.
    ///
    /// # Returns
    ///
    /// A fully constructed position with empty tags and metadata.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidInput`] if `quantity` is NaN or infinite, or if
    /// `unit` is [`PositionUnit::Percentage`] and `quantity` is outside
    /// `[-100.0, 100.0]`.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_portfolio::position::{Position, PositionUnit};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    /// use std::sync::Arc;
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_quant_portfolio::Result<()> {
    /// let instrument = Deposit::builder()
    ///     .id("DEP_1M".into())
    ///     .notional(Money::from((1_000_000_i64, Currency::USD)))
    ///     .start_date(date!(2024-01-01))
    ///     .maturity(date!(2024-02-01))
    ///     .day_count(finstack_quant_core::dates::DayCount::Act360)
    ///     .discount_curve_id("USD".into())
    ///     .build()
    ///     .expect("example deposit should build");
    ///
    /// let position = Position::new(
    ///     "POS_001",
    ///     "ENTITY_A",
    ///     "DEP_1M",
    ///     Arc::new(instrument),
    ///     1.0,
    ///     PositionUnit::Units,
    /// )?;
    ///
    /// assert_eq!(position.instrument_id, "DEP_1M");
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(
        position_id: impl Into<PositionId>,
        entity_id: impl Into<EntityId>,
        instrument_id: impl Into<String>,
        instrument: Arc<dyn Instrument>,
        quantity: f64,
        unit: PositionUnit,
    ) -> Result<Self> {
        let pos_id: PositionId = position_id.into();

        if !quantity.is_finite() {
            return Err(Error::invalid_input(format!(
                "Position quantity must be finite, got: {} (position_id: {})",
                quantity, pos_id
            )));
        }

        if quantity.abs() > 1e15 {
            tracing::warn!(
                position_id = %pos_id,
                quantity,
                "Unusually large position quantity"
            );
        }

        if matches!(unit, PositionUnit::Percentage) && quantity.abs() > 100.0 {
            return Err(Error::invalid_input(format!(
                "Percentage quantity must be between -100.0 and 100.0, got: {} (position_id: {})",
                quantity, pos_id
            )));
        }

        Ok(Self {
            position_id: pos_id,
            entity_id: entity_id.into(),
            instrument_id: instrument_id.into(),
            instrument,
            quantity,
            unit,
            book_id: None,
            attributes: IndexMap::new(),
            meta: IndexMap::new(),
        })
    }

    /// Assign this position to a book.
    ///
    /// # Arguments
    ///
    /// * `book_id` - Book identifier for hierarchical organization.
    ///
    /// # Returns
    ///
    /// The updated position for fluent chaining.
    #[must_use]
    pub fn with_book(mut self, book_id: impl Into<BookId>) -> Self {
        self.book_id = Some(book_id.into());
        self
    }

    /// Add a text attribute to the position.
    ///
    /// # Arguments
    ///
    /// * `key` - Attribute key.
    /// * `value` - Text attribute value.
    ///
    /// # Returns
    ///
    /// The updated position for fluent chaining.
    #[must_use]
    pub fn with_text_attribute(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attributes
            .insert(key.into(), AttributeValue::Text(value.into()));
        self
    }

    /// Add a numeric attribute to the position.
    ///
    /// # Arguments
    ///
    /// * `key` - Attribute key.
    /// * `value` - Numeric attribute value.
    ///
    /// # Returns
    ///
    /// The updated position for fluent chaining.
    #[must_use]
    pub fn with_numeric_attribute(mut self, key: impl Into<String>, value: f64) -> Self {
        self.attributes
            .insert(key.into(), AttributeValue::Number(value));
        self
    }

    /// Add an attribute to the position.
    ///
    /// # Arguments
    ///
    /// * `key` - Attribute key.
    /// * `value` - Attribute value (text or numeric).
    ///
    /// # Returns
    ///
    /// The updated position for fluent chaining.
    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: impl Into<AttributeValue>,
    ) -> Self {
        self.attributes.insert(key.into(), value.into());
        self
    }

    /// Add multiple text attributes at once.
    ///
    /// # Arguments
    ///
    /// * `attrs` - Iterator of (key, value) string pairs.
    ///
    /// # Returns
    ///
    /// The updated position for fluent chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_portfolio::position::{Position, PositionUnit};
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    /// use std::sync::Arc;
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_quant_portfolio::Result<()> {
    /// let as_of = date!(2024-01-01);
    ///
    /// // Create an instrument to attach to the position (example: a simple deposit)
    /// let deposit = Deposit::builder()
    ///     .id("DEP_1M".into())
    ///     .notional(Money::from((1_000_000_i64, Currency::USD)))
    ///     .start_date(as_of)
    ///     .maturity(date!(2024-02-01))
    ///     .day_count(finstack_quant_core::dates::DayCount::Act360)
    ///     .discount_curve_id("USD".into())
    ///     .build()
    ///     .expect("deposit builder should succeed");
    ///
    /// let position = Position::new(
    ///     "POS_001",
    ///     "ACME_CORP",
    ///     "DEP_1M",
    ///     Arc::new(deposit),
    ///     1.0,
    ///     PositionUnit::Units,
    /// )?
    /// .with_text_attributes([("sector", "Technology"), ("region", "US")]);
    ///
    /// assert_eq!(position.attributes.get("sector"), Some(&finstack_quant_portfolio::AttributeValue::Text("Technology".to_string())));
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_text_attributes<K, V, I>(mut self, attrs: I) -> Self
    where
        K: Into<String>,
        V: Into<String>,
        I: IntoIterator<Item = (K, V)>,
    {
        for (k, v) in attrs {
            self.attributes
                .insert(k.into(), AttributeValue::Text(v.into()));
        }
        self
    }

    /// Add metadata.
    ///
    /// # Arguments
    ///
    /// * `key` - Metadata key.
    /// * `value` - Arbitrary JSON value.
    ///
    /// # Returns
    ///
    /// The updated position for fluent chaining.
    #[must_use]
    pub fn with_meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.meta.insert(key.into(), value);
        self
    }

    /// `true` when quantity is strictly positive.
    pub fn is_long(&self) -> bool {
        self.quantity > 0.0
    }

    /// `true` when quantity is strictly negative.
    pub fn is_short(&self) -> bool {
        self.quantity < 0.0
    }

    /// Scale a monetary value by [`Self::scale_factor`].
    ///
    /// # Arguments
    ///
    /// * `value` - Monetary value to scale, typically from `instrument.value()`.
    ///
    /// # Examples
    ///
    /// ```
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_portfolio::position::{Position, PositionUnit};
    /// use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    /// use std::sync::Arc;
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_quant_portfolio::Result<()> {
    /// let instrument = Deposit::builder()
    ///     .id("DEP_1M".into())
    ///     .notional(Money::from((1_000_000_i64, Currency::USD)))
    ///     .start_date(date!(2024-01-01))
    ///     .maturity(date!(2024-02-01))
    ///     .day_count(finstack_quant_core::dates::DayCount::Act360)
    ///     .discount_curve_id("USD".into())
    ///     .build()
    ///     .expect("example deposit should build");
    ///
    /// let position = Position::new(
    ///     "POS_001",
    ///     "ENTITY_A",
    ///     "DEP_1M",
    ///     Arc::new(instrument),
    ///     50.0,
    ///     PositionUnit::Percentage,
    /// )?;
    ///
    /// let scaled = position.scale_value(Money::from((200_i64, Currency::USD))).expect("valid scale_value fixture");
    /// assert_eq!(scaled.amount(), 100.0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn scale_value(&self, value: Money) -> finstack_quant_core::Result<Money> {
        if let PositionUnit::Notional(Some(notional_currency)) = self.unit {
            if notional_currency != value.currency() {
                tracing::warn!(
                    position_id = %self.position_id,
                    "Notional currency {} differs from instrument currency {}",
                    notional_currency, value.currency()
                );
            }
        }
        Money::new(value.amount() * self.scale_factor(), value.currency())
    }

    /// Unit-aware scale factor applied to instrument P&L or PV.
    ///
    /// Returns the multiplier defined by [`PositionUnit`]: `quantity` for
    /// `Units`, `Notional` (lot count), and `FaceValue`; `quantity / 100`
    /// for `Percentage`. Callers that compute raw P&L (not `Money`) should
    /// multiply by this factor to honor the scaling contract.
    ///
    /// This complements [`Self::scale_value`] for callers that work in
    /// `f64` rather than [`Money`] (e.g. factor-model stress engines that
    /// return currency-less P&L deltas).
    #[inline]
    pub fn scale_factor(&self) -> f64 {
        match self.unit {
            PositionUnit::Units | PositionUnit::Notional(_) | PositionUnit::FaceValue => {
                self.quantity
            }
            PositionUnit::Percentage => self.quantity / 100.0,
        }
    }

    /// Convert this position to a serializable specification.
    ///
    /// Attempts to extract the instrument JSON representation if the instrument
    /// implements the conversion. Returns `None` for `instrument_spec` if conversion
    /// is not supported.
    ///
    /// # Returns
    ///
    /// A serializable `PositionSpec` carrying tags, metadata, and an optional
    /// instrument payload.
    pub fn to_spec(&self) -> PositionSpec {
        let instrument_spec = self.instrument.to_instrument_json();

        PositionSpec {
            position_id: self.position_id.clone(),
            entity_id: self.entity_id.clone(),
            instrument_id: self.instrument_id.clone(),
            instrument_spec,
            quantity: self.quantity,
            unit: self.unit,
            book_id: self.book_id.clone(),
            attributes: self.attributes.clone(),
            meta: self.meta.clone(),
        }
    }

    /// Reconstruct a Position from a specification.
    ///
    /// # Arguments
    ///
    /// * `spec` - The position specification to reconstruct
    ///
    /// # Returns
    ///
    /// Reconstructed runtime position with a live instrument trait object.
    ///
    /// # Errors
    ///
    /// Returns [`Error`] if:
    /// - The quantity is invalid (NaN/Inf)
    /// - The instrument specification cannot be converted to an instrument
    pub fn from_spec(spec: PositionSpec) -> Result<Self> {
        let PositionSpec {
            position_id,
            entity_id,
            instrument_id,
            instrument_spec,
            quantity,
            unit,
            book_id,
            attributes,
            meta,
        } = spec;

        let instrument = if let Some(instr_json) = instrument_spec {
            Arc::from(instr_json.into_boxed().map_err(|e| {
                Error::invalid_input(format!("Failed to convert instrument JSON: {}", e))
            })?)
        } else {
            return Err(Error::invalid_input(
                "Cannot reconstruct position without instrument_spec".to_string(),
            ));
        };

        let mut position = Self::new(
            position_id,
            entity_id,
            instrument_id,
            instrument,
            quantity,
            unit,
        )?;
        position.book_id = book_id;
        position.attributes = attributes;
        position.meta = meta;
        Ok(position)
    }
}

impl std::fmt::Debug for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Position")
            .field("position_id", &self.position_id)
            .field("entity_id", &self.entity_id)
            .field("instrument_id", &self.instrument_id)
            .field("quantity", &self.quantity)
            .field("unit", &self.unit)
            .field("attributes", &self.attributes)
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        self.position_id == other.position_id
    }
}

impl Eq for Position {}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_valuations::instruments::rates::deposit::Deposit;
    use time::macros::date;

    #[test]
    fn test_position_creation() {
        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .start_date(date!(2024 - 01 - 01))
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .build()
            .expect("test should succeed");

        let position = Position::new(
            "POS_001",
            "FUND_A",
            "DEP_1M",
            Arc::new(deposit),
            1.0,
            PositionUnit::Units,
        )
        .expect("test should succeed")
        .with_text_attribute("type", "cash")
        .with_text_attribute("rating", "AAA");

        assert_eq!(position.position_id, "POS_001");
        assert_eq!(position.entity_id, "FUND_A");
        assert_eq!(position.instrument_id, "DEP_1M");
        assert!(position.is_long());
        assert!(!position.is_short());
        assert_eq!(
            position.attributes.get("type"),
            Some(&AttributeValue::Text("cash".to_string()))
        );
    }

    #[test]
    fn test_position_unit_serialization() {
        let unit = PositionUnit::Notional(Some(Currency::USD));
        let json = serde_json::to_string(&unit).expect("test should succeed");
        assert!(json.contains("notional"));
    }

    #[test]
    fn percentage_quantity_above_one_hundred_is_rejected() {
        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .start_date(date!(2024 - 01 - 01))
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .build()
            .expect("test should succeed");

        let err = Position::new(
            "POS_001",
            "FUND_A",
            "DEP_1M",
            Arc::new(deposit),
            100.0001,
            PositionUnit::Percentage,
        )
        .expect_err("percentage quantities above 100 should be invalid");

        assert!(err.to_string().contains("Percentage quantity"));
    }

    #[test]
    fn minor1_percentage_quantity_below_negative_one_hundred_is_rejected() {
        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .start_date(date!(2024 - 01 - 01))
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .build()
            .expect("test should succeed");

        let err = Position::new(
            "POS_001",
            "FUND_A",
            "DEP_1M",
            Arc::new(deposit),
            -100.0001,
            PositionUnit::Percentage,
        )
        .expect_err("minor 1: percentage quantities below -100 should be invalid");

        assert!(err.to_string().contains("Percentage quantity"));
    }

    #[test]
    fn notional_two_lots_scales_deal_pv_not_unit_notional() {
        let deposit = Deposit::builder()
            .id("DEP_1M".into())
            .notional(Money::from((1_000_000_i64, Currency::USD)))
            .start_date(date!(2024 - 01 - 01))
            .maturity(date!(2024 - 02 - 01))
            .day_count(finstack_quant_core::dates::DayCount::Act360)
            .discount_curve_id("USD".into())
            .build()
            .expect("test should succeed");

        let position = Position::new(
            "POS_2LOT",
            "FUND_A",
            "DEP_1M",
            Arc::new(deposit),
            2.0,
            PositionUnit::Notional(Some(Currency::USD)),
        )
        .expect("two-lot notional position should build");

        assert_eq!(position.scale_factor(), 2.0);
        let deal_pv = Money::from((1_000_000_i64, Currency::USD));
        let scaled = position
            .scale_value(deal_pv)
            .expect("valid scale_value fixture");
        assert_eq!(scaled.amount(), 2_000_000.0);
        assert!((scaled.amount() - 2e6 * deal_pv.amount()).abs() > 1.0);
    }
}
