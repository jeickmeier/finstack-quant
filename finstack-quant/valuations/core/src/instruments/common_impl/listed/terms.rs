//! Shared listed-future position and lifecycle terms.

use crate::instruments::Position;
use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;

/// Final settlement mode for a listed future.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ListedFutureSettlement {
    /// Cash settlement of final variation margin.
    #[default]
    Cash,
    /// Delivery of a standardized quantity of an identified asset.
    Physical {
        /// Deliverable asset, grade, location, or basket identifier.
        asset: String,
        /// Physical units delivered per exchange contract.
        quantity_per_contract: f64,
    },
}

/// Delivery instruction produced after a physically settled future stops trading.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListedDeliveryObligation {
    /// Deliverable asset, grade, location, or basket identifier.
    pub asset: String,
    /// Settlement date on which delivery and invoice exchange occur.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub delivery_date: Date,
    /// Signed physical quantity: positive is received, negative is delivered.
    pub asset_quantity: f64,
    /// Signed settlement-currency invoice: positive is received, negative is paid.
    pub invoice_amount: f64,
    /// Settlement currency of the invoice amount.
    pub currency: Currency,
}

/// Standardized position, multiplier, and lifecycle terms for a listed future.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ListedFutureTerms {
    /// Number of exchange contracts. Fractional values are permitted for
    /// portfolio aggregation but must be finite and strictly positive.
    pub contracts: f64,
    /// Settlement-currency value of one full price point per contract.
    pub multiplier: f64,
    /// Currency in which variation margin is paid.
    pub currency: Currency,
    /// Trade fill price in the same price-point units as the market mark.
    pub entry_price: f64,
    /// Optional live exchange mark, in price points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quoted_price: Option<f64>,
    /// Optional official final settlement price, in price points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement_price: Option<f64>,
    /// Final date on which the contract trades.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub last_trading_date: Date,
    /// Date on which final cash settlement is completed.
    #[serde(with = "finstack_quant_core::wire::date")]
    #[cfg_attr(
        feature = "json-schema",
        schemars(with = "finstack_quant_core::wire::DateWire")
    )]
    pub settlement_date: Date,
    /// Long or short position direction.
    pub position: Position,
    /// Cash or physical final settlement convention.
    #[serde(default)]
    pub settlement: ListedFutureSettlement,
}

impl ListedFutureTerms {
    /// Construct required listed-future terms.
    ///
    /// # Arguments
    ///
    /// * `contracts` - Positive finite number of exchange contracts.
    /// * `multiplier` - Positive finite settlement-currency value per price point.
    /// * `currency` - Variation-margin and P&L currency.
    /// * `entry_price` - Finite trade fill in contract price points.
    /// * `last_trading_date` - Final exchange trading date.
    /// * `settlement_date` - Final settlement date, on or after the last trading date.
    /// * `position` - Long or short direction.
    pub fn new(
        contracts: f64,
        multiplier: f64,
        currency: Currency,
        entry_price: f64,
        last_trading_date: Date,
        settlement_date: Date,
        position: Position,
    ) -> finstack_quant_core::Result<Self> {
        let terms = Self {
            contracts,
            multiplier,
            currency,
            entry_price,
            quoted_price: None,
            settlement_price: None,
            last_trading_date,
            settlement_date,
            position,
            settlement: ListedFutureSettlement::Cash,
        };
        terms.validate()?;
        Ok(terms)
    }

    /// Set the current exchange mark.
    ///
    /// # Arguments
    ///
    /// * `quoted_price` - Finite current futures price in contract price points.
    #[must_use]
    pub fn with_quoted_price(mut self, quoted_price: f64) -> Self {
        self.quoted_price = Some(quoted_price);
        self
    }

    /// Set the official final settlement price.
    ///
    /// # Arguments
    ///
    /// * `settlement_price` - Finite official exchange settlement in price points.
    #[must_use]
    pub fn with_settlement_price(mut self, settlement_price: f64) -> Self {
        self.settlement_price = Some(settlement_price);
        self
    }

    /// Configure physical delivery for the exchange contract.
    ///
    /// # Arguments
    ///
    /// * `asset` - Non-empty deliverable asset, grade, location, or basket identifier.
    /// * `quantity_per_contract` - Positive finite physical units per contract.
    pub fn with_physical_delivery(
        mut self,
        asset: impl Into<String>,
        quantity_per_contract: f64,
    ) -> finstack_quant_core::Result<Self> {
        self.settlement = ListedFutureSettlement::Physical {
            asset: asset.into(),
            quantity_per_contract,
        };
        self.validate()?;
        Ok(self)
    }

    /// Validate position, price, and lifecycle invariants.
    pub fn validate(&self) -> finstack_quant_core::Result<()> {
        if !self.contracts.is_finite() || self.contracts <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "listed future contracts must be finite and positive".to_string(),
            ));
        }
        if !self.multiplier.is_finite() || self.multiplier <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(
                "listed future multiplier must be finite and positive".to_string(),
            ));
        }
        for (name, value) in [
            ("entry_price", Some(self.entry_price)),
            ("quoted_price", self.quoted_price),
            ("settlement_price", self.settlement_price),
        ] {
            if value.is_some_and(|price| !price.is_finite()) {
                return Err(finstack_quant_core::Error::Validation(format!(
                    "listed future {name} must be finite"
                )));
            }
        }
        if self.last_trading_date > self.settlement_date {
            return Err(finstack_quant_core::Error::Validation(format!(
                "listed future last_trading_date {} must not be after settlement_date {}",
                self.last_trading_date, self.settlement_date
            )));
        }
        if let ListedFutureSettlement::Physical {
            asset,
            quantity_per_contract,
        } = &self.settlement
        {
            if asset.trim().is_empty()
                || !quantity_per_contract.is_finite()
                || *quantity_per_contract <= 0.0
            {
                return Err(finstack_quant_core::Error::Validation(
                    "physically settled future requires a non-empty asset and positive finite quantity_per_contract"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Return P&L for a supplied futures mark with no discounting.
    ///
    /// Futures are variation-margined, so the value is
    /// `sign × contracts × multiplier × (mark − entry_price)`.
    ///
    /// # Arguments
    ///
    /// * `mark` - Finite current or final exchange price in contract price points.
    pub fn mark_to_market(&self, mark: f64) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        if !mark.is_finite() {
            return Err(finstack_quant_core::Error::Validation(
                "listed future mark must be finite".to_string(),
            ));
        }
        Ok(self.position.sign() * self.contracts * self.multiplier * (mark - self.entry_price))
    }

    /// Resolve the lifecycle-appropriate futures mark.
    ///
    /// Live contracts use `quoted_price` when supplied and otherwise evaluate
    /// the model callback. Once trading has ended, only the official
    /// `settlement_price` is accepted.
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Identifier included in missing-settlement diagnostics.
    /// * `as_of` - Valuation date used to separate live and post-trading states.
    /// * `model_mark` - Lazy calculation of the live theoretical mark.
    pub fn resolve_mark<F>(
        &self,
        instrument_id: &str,
        as_of: Date,
        model_mark: F,
    ) -> finstack_quant_core::Result<f64>
    where
        F: FnOnce() -> finstack_quant_core::Result<f64>,
    {
        self.validate()?;
        if as_of > self.last_trading_date {
            self.settlement_price.ok_or_else(|| {
                finstack_quant_core::Error::Validation(format!(
                    "listed future '{instrument_id}' requires settlement_price after last_trading_date {}",
                    self.last_trading_date
                ))
            })
        } else {
            self.quoted_price.map_or_else(model_mark, Ok)
        }
    }

    /// Variation-margin P&L for the lifecycle-appropriate mark.
    ///
    /// Returns `0.0` once `as_of` is past `settlement_date`; otherwise
    /// resolves the mark via [`Self::resolve_mark`] and applies
    /// [`Self::mark_to_market`].
    ///
    /// # Arguments
    ///
    /// * `instrument_id` - Identifier included in missing-settlement diagnostics.
    /// * `as_of` - Valuation date controlling the contract lifecycle.
    /// * `model_mark` - Lazy calculation of the live theoretical mark.
    pub fn npv_from_model_price<F>(
        &self,
        instrument_id: &str,
        as_of: Date,
        model_mark: F,
    ) -> finstack_quant_core::Result<f64>
    where
        F: FnOnce() -> finstack_quant_core::Result<f64>,
    {
        if as_of > self.settlement_date {
            return Ok(0.0);
        }
        self.mark_to_market(self.resolve_mark(instrument_id, as_of, model_mark)?)
    }

    /// P&L sensitivity to a one-point increase in the futures price.
    pub fn point_delta(&self) -> finstack_quant_core::Result<f64> {
        self.validate()?;
        Ok(self.position.sign() * self.contracts * self.multiplier)
    }

    /// Produce the post-trading physical delivery obligation, if any.
    ///
    /// The generic invoice is `settlement_price × multiplier × contracts`.
    /// Bond-future conversion factors and accrued interest remain the domain of
    /// the dedicated bond-future instrument.
    ///
    /// # Arguments
    ///
    /// * `as_of` - Valuation date; obligations exist after last trading through settlement.
    pub fn delivery_obligation(
        &self,
        as_of: Date,
    ) -> finstack_quant_core::Result<Option<ListedDeliveryObligation>> {
        self.validate()?;
        let ListedFutureSettlement::Physical {
            asset,
            quantity_per_contract,
        } = &self.settlement
        else {
            return Ok(None);
        };
        if as_of <= self.last_trading_date || as_of > self.settlement_date {
            return Ok(None);
        }
        let settlement_price = self.settlement_price.ok_or_else(|| {
            finstack_quant_core::Error::Validation(
                "physical delivery requires settlement_price after last trading".to_string(),
            )
        })?;
        let delivery_sign = self.position.sign();
        Ok(Some(ListedDeliveryObligation {
            asset: asset.clone(),
            delivery_date: self.settlement_date,
            asset_quantity: delivery_sign * self.contracts * quantity_per_contract,
            invoice_amount: -delivery_sign * self.contracts * self.multiplier * settlement_price,
            currency: self.currency,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn physical_long_receives_asset_and_pays_invoice() {
        let terms = ListedFutureTerms::new(
            2.0,
            100.0,
            Currency::USD,
            90.0,
            date!(2026 - 06 - 28),
            date!(2026 - 06 - 30),
            Position::Long,
        )
        .expect("terms")
        .with_physical_delivery("WTI-CUSHING", 1_000.0)
        .expect("delivery")
        .with_settlement_price(92.0);

        let obligation = terms
            .delivery_obligation(date!(2026 - 06 - 29))
            .expect("obligation")
            .expect("physical");
        assert_eq!(obligation.asset_quantity, 2_000.0);
        assert_eq!(obligation.invoice_amount, -18_400.0);
    }

    #[test]
    fn mark_resolution_switches_from_live_quote_to_official_settlement() {
        let terms = ListedFutureTerms::new(
            1.0,
            10.0,
            Currency::USD,
            90.0,
            date!(2026 - 06 - 28),
            date!(2026 - 06 - 30),
            Position::Long,
        )
        .expect("terms")
        .with_quoted_price(91.0)
        .with_settlement_price(92.0);

        assert_eq!(
            terms
                .resolve_mark("TEST", date!(2026 - 06 - 28), || Ok(100.0))
                .expect("live quote"),
            91.0
        );
        assert_eq!(
            terms
                .resolve_mark("TEST", date!(2026 - 06 - 29), || Ok(100.0))
                .expect("official settlement"),
            92.0
        );
    }
}
