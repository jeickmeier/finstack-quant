//! Settlement and target amounts shared by structured-credit price and spread metrics.

use super::pricing::{accrued::accrued_at, prices::get_original_notional};
use crate::cashflow::traits::DatedFlows;
use crate::instruments::fixed_income::structured_credit::{StructuredCredit, TrancheCashflows};
use crate::metrics::MetricContext;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::term_structures::DiscountCurve;
use finstack_quant_core::{Error, Result};

pub(crate) fn settlement_date(deal: &StructuredCredit, as_of: Date) -> Result<Date> {
    let settlement = deal.quote_settlement_date.unwrap_or(as_of);
    if settlement < as_of || settlement < deal.closing_date {
        return Err(Error::Validation(
            "structured-credit quote settlement cannot precede valuation or closing".into(),
        ));
    }
    Ok(settlement)
}

/// One buyer entitlement date and original-face clean/dirty conversion.
pub(crate) struct SettlementQuote {
    pub(crate) settlement: Date,
    pub(crate) notional: f64,
    pub(crate) accrued: f64,
}

impl SettlementQuote {
    pub(crate) fn for_tranche(
        deal: &StructuredCredit,
        as_of: Date,
        notional: f64,
        cashflows: &TrancheCashflows,
    ) -> Result<Self> {
        let settlement = settlement_date(deal, as_of)?;
        let accrued = cashflows
            .accrual_periods
            .iter()
            .try_fold(0.0, |sum, period| {
                Ok::<_, Error>(sum + period.accrued(settlement)?)
            })?;
        Self::new(settlement, notional, accrued)
    }

    pub(crate) fn from_context(context: &mut MetricContext) -> Result<Self> {
        let settlement =
            settlement_date(context.instrument_as::<StructuredCredit>()?, context.as_of)?;
        let notional = get_original_notional(context)?;
        let accrued = accrued_at(context, settlement)?;
        Self::new(settlement, notional, accrued)
    }

    fn new(settlement: Date, notional: f64, accrued: f64) -> Result<Self> {
        if !notional.is_finite() || notional <= 0.0 || !accrued.is_finite() {
            return Err(Error::Validation("structured-credit quote requires positive original face and finite accrued interest".into()));
        }
        Ok(Self {
            settlement,
            notional,
            accrued,
        })
    }

    pub(crate) fn clean_target(&self, clean_price_pct: f64) -> Result<f64> {
        if !clean_price_pct.is_finite() || clean_price_pct <= 0.0 {
            return Err(Error::Validation(
                "structured-credit clean quote must be finite and positive".into(),
            ));
        }
        self.dirty_target(clean_price_pct / 100.0 * self.notional + self.accrued)
    }

    pub(crate) fn dirty_target(&self, dirty_currency: f64) -> Result<f64> {
        if !dirty_currency.is_finite() || dirty_currency <= 0.0 {
            return Err(Error::Validation(
                "structured-credit dirty quote target must be finite and positive".into(),
            ));
        }
        Ok(dirty_currency)
    }

    pub(crate) fn external_target(&self, deal: &StructuredCredit) -> Result<Option<f64>> {
        let quotes = &deal.instrument_pricing_overrides.market_quotes;
        quotes.validate().map_err(|error| {
            Error::Validation(format!("structured-credit price quote: {error}"))
        })?;
        if let Some(dirty) = quotes.quoted_dirty_price_currency {
            self.dirty_target(dirty).map(Some)
        } else {
            quotes
                .quoted_clean_price
                .map(|clean| self.clean_target(clean))
                .transpose()
        }
    }

    pub(crate) fn model_dirty(&self, flows: &DatedFlows, curve: &DiscountCurve) -> Result<f64> {
        flows
            .iter()
            .filter(|(date, _)| *date > self.settlement)
            .try_fold(0.0, |pv, (date, amount)| {
                Ok(pv + amount.amount() * curve.df_between_dates(self.settlement, *date)?)
            })
    }

    pub(crate) fn clean_price(&self, dirty_currency: f64) -> f64 {
        (dirty_currency - self.accrued) / self.notional * 100.0
    }
}
