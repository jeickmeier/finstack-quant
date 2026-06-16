//! Z-spread CS01 inputs for term loans.
//!
//! Term loans are valued by [`TermLoanDiscountingPricer`], which discounts
//! contractual cashflows on a single discount curve and never consumes a
//! hazard curve. A hazard-curve bump therefore never moves the PV, so the
//! canonical par-spread CS01 is identically zero for loans. Credit-spread risk
//! is instead reported via the market-standard z-spread bump in
//! [`ZSpreadParallelCs01`] / [`ZSpreadBucketedCs01`]; this module supplies the
//! holder-view cashflows that bump reprices, and the quoted dirty price used to
//! anchor the spread solve.
//!
//! [`TermLoanDiscountingPricer`]: crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer
//! [`ZSpreadParallelCs01`]: crate::metrics::ZSpreadParallelCs01
//! [`ZSpreadBucketedCs01`]: crate::metrics::ZSpreadBucketedCs01

use crate::instruments::fixed_income::term_loan::pricing::TermLoanDiscountingPricer;
use crate::instruments::TermLoan;
use crate::metrics::{ZSpreadCs01, ZSpreadCs01Inputs};
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;

impl ZSpreadCs01 for TermLoan {
    fn z_spread_cs01_inputs(
        &self,
        curves: &MarketContext,
        as_of: Date,
    ) -> finstack_quant_core::Result<ZSpreadCs01Inputs> {
        // Reuse the pricer's holder-view flow builder so PV_z(0) reproduces the
        // base PV exactly (same settlement anchor, same PIK/past-flow filter,
        // same seasoned-fixing handling).
        let (settlement, flows) = TermLoanDiscountingPricer::pricing_flows(self, curves, as_of)?;

        // Compounding frequency for the z-spread shift = coupon payments/year,
        // mirroring the bond z-spread convention.
        let years = self.frequency.to_years_simple();
        let compounds_per_year = if years > 0.0 && years.is_finite() {
            (1.0 / years).round().max(1.0)
        } else {
            1.0
        };

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
        // Loan-market quote convention: `quoted_clean_price` is a percent of
        // the funded outstanding at settlement; the dirty anchor adds accrued.
        let Some(px) = self.pricing_overrides.market_quotes.quoted_clean_price else {
            return Ok(None);
        };
        let schedule = crate::instruments::fixed_income::term_loan::cashflows::generate_cashflows(
            self, curves, as_of,
        )?;
        super::irr_helpers::quoted_dirty_from_clean_px(self, &schedule, as_of, px)
            .map(|m| Some(m.amount()))
    }
}
