//! Z-spread CS01 inputs for term loans.
//!
//! Explicit deterministic discounting values contractual cashflows on a single
//! discount curve, so this module supplies holder-view inputs for the
//! market-standard z-spread CS01 fallback. When the active model is the default
//! rates-credit tree, the metric registry delegates to canonical hazard-curve
//! CS01 instead.
//!
//! [`TermLoanDiscountingPricer`]: crate::term_loan::pricing::TermLoanDiscountingPricer
//! [`ZSpreadParallelCs01`]: finstack_quant_valuations_core::metrics::ZSpreadParallelCs01
//! [`ZSpreadBucketedCs01`]: finstack_quant_valuations_core::metrics::ZSpreadBucketedCs01

use crate::term_loan::pricing::TermLoanDiscountingPricer;
use crate::term_loan::TermLoan;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_valuations_core::metrics::{ZSpreadCs01, ZSpreadCs01Inputs};

impl ZSpreadCs01 for TermLoan {
    fn z_spread_cs01_quoted_spread(&self) -> Option<f64> {
        self.instrument_pricing_overrides
            .market_quotes
            .quoted_z_spread
    }

    fn z_spread_cs01_inputs(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<ZSpreadCs01Inputs> {
        // Quote-space flows: a settlement-date buyer's flows, matching the
        // settlement-date dirty quote the spread is solved against (same
        // PIK filter and seasoned-fixing handling as the pricer).
        let schedule = TermLoanDiscountingPricer::pricing_schedule(self, curves, as_of)?;
        let (settlement, flows) = TermLoanDiscountingPricer::pricing_flows(self, &schedule, as_of)?;

        // Compounding frequency for the z-spread shift = coupon payments/year,
        // mirroring the bond z-spread convention.
        let compounds_per_year = self.frequency.compounding_frequency();

        Ok(ZSpreadCs01Inputs {
            settlement,
            discount_curve_id: self.discount_curve_id.clone(),
            compounds_per_year,
            flows,
        })
    }

    fn z_spread_cs01_quoted_dirty(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<Option<f64>> {
        // Loan-market quote convention: `quoted_clean_price_pct` is a percent of
        // the funded outstanding at settlement; the dirty anchor adds accrued.
        // Use the fixings-applied pricing schedule so settlement accrued on a
        // seasoned floater reflects the actual fixing, matching the flows in
        // `z_spread_cs01_inputs`.
        let Some(px) = self
            .instrument_pricing_overrides
            .market_quotes
            .quoted_clean_price_pct
        else {
            return Ok(None);
        };
        let schedule = TermLoanDiscountingPricer::pricing_schedule(self, curves, as_of)?;
        super::irr_helpers::quoted_dirty_from_clean_px(self, &schedule, as_of, px)
            .map(|m| Some(m.amount()))
    }
}
