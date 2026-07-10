//! Bond future pricing logic.
//!
//! This module implements pricing and valuation for bond futures, including:
//! - Conversion factor calculation
//! - Model futures price calculation
//! - NPV calculation

use crate::pricer::PricingError;
use finstack_quant_core::dates::Date;
use finstack_quant_core::market_data::context::MarketContext;
use finstack_quant_core::money::Money;
use finstack_quant_core::Result;

use crate::instruments::common_impl::traits::Instrument;
use crate::instruments::fixed_income::bond::Bond;
use crate::instruments::rates::ir_future::Position;

/// Bond future pricer.
///
/// Implements pricing logic for bond futures, including conversion factor calculation,
/// model price calculation, and NPV calculation.
pub struct BondFuturePricer;

impl BondFuturePricer {
    /// Calculate the CME/CBOT standard conversion factor for a deliverable bond.
    ///
    /// The conversion factor normalizes bonds with different coupons and
    /// maturities so they can be compared for delivery against a futures
    /// contract. It is a **fixed contractual quantity** for a given
    /// (security, delivery month) pair — it is the decimal price at which
    /// $1 par of the security would trade if it yielded the contract's
    /// notional coupon rate. It does **not** depend on the caller's
    /// valuation date.
    ///
    /// # CME standard formula
    ///
    /// Source: CME Group, "Calculating U.S. Treasury Futures Conversion
    /// Factors" (Interest Rate Resource Center, IR232). For a security with
    /// annual coupon `coupon` (rounded to the nearest 1/8 of 1%) and a
    /// notional yield `r` (6% for all U.S. Treasury futures, so the
    /// per-period rate is `r/2`):
    ///
    /// ```text
    /// factor = a × [ (coupon/2) + c + d ] − b      (rounded to 4 dp)
    ///
    ///   n = whole years from the first day of the delivery month to the
    ///       maturity (or first call) date.
    ///   z = whole months between n and maturity, rounded DOWN to the
    ///       nearest quarter (10Y/30Y contracts) or the nearest month
    ///       (2Y/3Y/5Y contracts).
    ///   v = z                       if z < 7
    ///     = 3                       if z ≥ 7   (10Y / 30Y contracts)
    ///     = z − 6                   if z ≥ 7   (2Y / 3Y / 5Y contracts)
    ///   a = 1 / (1 + r/2)^(v/6)
    ///   b = (coupon/2) × (6 − v) / 6
    ///   c = 1 / (1 + r/2)^(2n)      if z < 7
    ///     = 1 / (1 + r/2)^(2n + 1)  if z ≥ 7
    ///   d = (coupon / r) × (1 − c)
    /// ```
    ///
    /// # Important
    ///
    /// When the exchange publishes a conversion factor for a deliverable, use
    /// that value: it is carried on [`super::DeliverableBond::conversion_factor`]
    /// and is what the pricing path (`base_value`, the futures-price and
    /// conversion-factor metrics) actually uses. This function reproduces the
    /// published CME methodology and is the fallback when no exchange value is
    /// supplied.
    ///
    /// # Parameters
    ///
    /// - `bond`: The deliverable bond. Must be fixed-rate (or step-up); the
    ///   stated annual coupon is read from the bond's cashflow spec.
    /// - `standard_coupon`: The contract's notional coupon / yield (e.g.
    ///   `0.06` for the 6% U.S. Treasury contracts).
    /// - `standard_maturity_years`: The contract's standard maturity in years.
    ///   Selects the rounding convention: `>= 6` years uses the 10Y/30Y rules
    ///   (quarterly `z`, `v = 3` when `z ≥ 7`); otherwise the 2Y/3Y/5Y rules
    ///   (monthly `z`, `v = z − 6` when `z ≥ 7`).
    /// - `delivery_month_first_day`: Any date within the delivery month; the
    ///   CME anchor (the first day of that month) is derived internally.
    ///
    /// # Returns
    ///
    /// Conversion factor rounded to 4 decimal places.
    ///
    /// # Errors
    ///
    /// Returns [`finstack_quant_core::Error::Validation`] if the bond has no fixed
    /// coupon (e.g. a floating-rate note), if the notional coupon is
    /// non-positive, or if the bond matures before the delivery month.
    pub fn calculate_conversion_factor(
        bond: &Bond,
        standard_coupon: f64,
        standard_maturity_years: f64,
        delivery_month_first_day: Date,
    ) -> Result<f64> {
        use finstack_quant_core::dates::DateExt;

        if !standard_coupon.is_finite() || standard_coupon <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "standard (notional) coupon must be a positive, finite rate to compute a conversion factor, got {standard_coupon}"
            )));
        }

        // Stated annual coupon of the deliverable, rounded to the nearest
        // 1/8 of 1% (0.00125) per the CME methodology footnote.
        let raw_coupon = bond.cashflow_spec.fixed_coupon_rate().ok_or_else(|| {
            finstack_quant_core::Error::Validation(format!(
                "bond '{}' has no fixed coupon; the CME conversion factor is only defined for fixed-rate deliverables",
                bond.id.as_str()
            ))
        })?;
        let coupon = (raw_coupon / 0.00125).round() * 0.00125;

        // CME anchor: the first calendar day of the delivery month.
        let month_start = Date::from_calendar_date(
            delivery_month_first_day.year(),
            delivery_month_first_day.month(),
            1,
        )
        .map_err(|e| {
            finstack_quant_core::Error::Validation(format!("invalid delivery month date: {e}"))
        })?;

        // For callable issues the CME measures time to the first call date;
        // straight notes/bonds use the maturity date.
        let redemption_date = bond
            .call_put
            .as_ref()
            .and_then(|cp| cp.calls.iter().map(|c| c.start_date).min())
            .unwrap_or(bond.maturity);
        if redemption_date <= month_start {
            return Err(finstack_quant_core::Error::Validation(format!(
                "bond '{}' redemption date {redemption_date} is not after the delivery month start {month_start}",
                bond.id.as_str()
            )));
        }

        // Whole months from the first day of the delivery month to redemption.
        let total_months = month_start.months_until(redemption_date);
        // Round the *months* component down to the contract's granularity:
        // 3 months for the 10Y/30Y contracts, 1 month for 2Y/3Y/5Y.
        let is_long_contract = standard_maturity_years >= 6.0;
        let n: i64 = (total_months / 12) as i64;
        let z_raw = total_months % 12;
        let z: i64 = if is_long_contract {
            (z_raw - z_raw % 3) as i64
        } else {
            z_raw as i64
        };

        // v: months from the delivery-month start to the first coupon.
        let v: f64 = if z < 7 {
            z as f64
        } else if is_long_contract {
            3.0
        } else {
            (z - 6) as f64
        };

        // Per-period discount rate at the notional yield (r/2 per half-year).
        let period_base = 1.0 + standard_coupon / 2.0;

        let a = 1.0 / period_base.powf(v / 6.0);
        let b = (coupon / 2.0) * (6.0 - v) / 6.0;
        let c = if z < 7 {
            1.0 / period_base.powi((2 * n) as i32)
        } else {
            1.0 / period_base.powi((2 * n + 1) as i32)
        };
        let d = (coupon / standard_coupon) * (1.0 - c);

        let factor = a * (coupon / 2.0 + c + d) - b;

        // CME publishes conversion factors to 4 decimal places.
        Ok((factor * 10_000.0).round() / 10_000.0)
    }

    /// Calculate model futures price from the CTD bond, **carry-adjusted to
    /// the delivery date**.
    ///
    /// The model futures price is the theoretical fair value of the futures
    /// contract: the *forward* clean price of the cheapest-to-deliver bond at
    /// the contract's delivery date, divided by the CTD's conversion factor.
    ///
    /// # Formula
    ///
    /// ```text
    /// Forward_Dirty_CTD = ( Spot_Dirty_CTD − PV_coupons(as_of → delivery] ) / DF(as_of → delivery)
    /// Forward_Clean_CTD = Forward_Dirty_CTD − Accrued_CTD(delivery)
    /// Model_Price       = Forward_Clean_CTD_percent / CF
    /// ```
    ///
    /// The spot dirty price is carried forward at the CTD bond's own discount
    /// curve (the cost of financing the position to delivery), and any coupons
    /// received before delivery are credited at their present value. This
    /// removes the bias of the previous `Clean_Price / CF` proxy, which priced
    /// the futures off the *spot* clean price and so omitted cost-of-carry.
    ///
    /// # Repo specials
    ///
    /// This convenience entry point uses the CTD bond's discount curve as the
    /// financing proxy. Position pricing calls the financing-aware helper and
    /// uses the future's dedicated repo curve when configured.
    ///
    /// # Parameters
    ///
    /// - `ctd_bond`: The cheapest-to-deliver bond
    /// - `conversion_factor`: Pre-calculated conversion factor for the CTD bond
    /// - `market`: Market context with discount curves for pricing
    /// - `as_of`: Valuation date
    /// - `delivery_date`: Contract delivery date the CTD is carried forward to
    ///
    /// # Returns
    ///
    /// Model futures price as a decimal (e.g., 125.50 for 125-16 in 32nds)
    ///
    /// # Errors
    ///
    /// Returns an error if `conversion_factor` is non-positive, the discount
    /// factor to delivery is non-positive, or the CTD schedule/curve lookups
    /// fail.
    ///
    /// # Example
    ///
    /// ```text
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::pricer::BondFuturePricer;
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let market = MarketContext::new();
    /// let ctd_bond = Bond::fixed(
    ///     "US-CTD",
    ///     Money::new(100_000.0, Currency::USD),
    ///     0.05,
    ///     date!(2020-01-15),
    ///     date!(2030-01-15),
    ///     "USD-OIS",
    /// )?;
    /// let cf = 0.8234;
    /// let model_price = BondFuturePricer::calculate_model_price(
    ///     &ctd_bond,
    ///     cf,
    ///     &market,
    ///     date!(2025-01-15),
    ///     date!(2025-03-31),
    /// )?;
    /// // model_price might be 125.50
    /// # let _ = model_price;
    /// # Ok(())
    /// # }
    /// ```
    pub fn calculate_model_price(
        ctd_bond: &Bond,
        conversion_factor: f64,
        market: &MarketContext,
        as_of: Date,
        delivery_date: Date,
    ) -> Result<f64> {
        Self::calculate_model_price_with_financing_curve(
            ctd_bond,
            conversion_factor,
            market,
            as_of,
            delivery_date,
            None,
        )
    }

    fn calculate_model_price_with_financing_curve(
        ctd_bond: &Bond,
        conversion_factor: f64,
        market: &MarketContext,
        as_of: Date,
        delivery_date: Date,
        repo_curve_id: Option<&finstack_quant_core::types::CurveId>,
    ) -> Result<f64> {
        use crate::cashflow::accrual::accrued_interest_amount;
        use finstack_quant_core::math::summation::NeumaierAccumulator;

        if !conversion_factor.is_finite() || conversion_factor <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "conversion factor must be a positive, finite number to compute a model \
                 futures price, got {conversion_factor}"
            )));
        }

        // Spot dirty price of the CTD (PV in currency at the valuation date).
        let spot_dirty = ctd_bond.value(market, as_of)?.amount();

        let financing_curve_id = repo_curve_id.unwrap_or(&ctd_bond.discount_curve_id);
        let disc = market.get_discount(financing_curve_id)?;

        // Present value of coupons/principal received strictly between today
        // and delivery — these are credited to the carry (the long forward
        // does not receive them).
        let flows = ctd_bond.pricing_dated_cashflows(market, as_of)?;
        let mut pv_interim = NeumaierAccumulator::new();
        for (date, amount) in &flows {
            if *date > as_of && *date <= delivery_date {
                let df = disc.df_between_dates(as_of, *date)?;
                pv_interim.add(amount.amount() * df);
            }
        }

        // Discount factor from today to delivery: the cost of carrying the
        // financed position to the delivery date.
        let df_to_delivery = disc.df_between_dates(as_of, delivery_date)?;
        if !df_to_delivery.is_finite() || df_to_delivery <= 0.0 {
            return Err(finstack_quant_core::Error::Validation(format!(
                "discount factor from as_of to delivery ({delivery_date}) must be positive \
                 and finite to compute carry-adjusted forward CTD price, got {df_to_delivery}"
            )));
        }

        // Carry-adjusted forward dirty price of the CTD at delivery.
        let forward_dirty = (spot_dirty - pv_interim.total()) / df_to_delivery;

        // Forward clean price = forward dirty minus accrued at the delivery date.
        let schedule = ctd_bond.full_cashflow_schedule(market)?;
        let accrued_at_delivery =
            accrued_interest_amount(&schedule, delivery_date, &ctd_bond.accrual_config())?;
        let forward_clean = forward_dirty - accrued_at_delivery;

        // Express as a percentage of par, then divide by the conversion factor.
        let notional = ctd_bond.notional.amount();
        let forward_clean_percent = (forward_clean / notional) * 100.0;

        Ok(forward_clean_percent / conversion_factor)
    }

    /// Calculate the model price using the financing convention declared by a future.
    pub(crate) fn calculate_model_price_for_future(
        future: &super::BondFuture,
        ctd_bond: &Bond,
        conversion_factor: f64,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<f64> {
        Self::calculate_model_price_with_financing_curve(
            ctd_bond,
            conversion_factor,
            market,
            as_of,
            future.delivery_start,
            future.repo_curve_id.as_ref(),
        )
    }

    /// Calculate the NPV (present value) of a bond future position.
    ///
    /// The NPV represents the mark-to-market value of the futures position,
    /// calculated as the undiscounted model-to-contract value.
    ///
    /// # Formula
    ///
    /// NPV = (Model_Price - Contract_Price) × (Notional / 100) × Sign
    ///
    /// Where:
    /// - Contract_Price: Entry/contract price stored in `quoted_price`
    /// - Model_Price: Theoretical fair value based on CTD bond
    /// - Notional: Total notional exposure (contract_size × num_contracts)
    /// - Sign: +1 for Long positions, -1 for Short positions
    /// - Division by 100: Prices are quoted per $100 face value
    ///
    /// Note: No discount factor is applied because exchange-traded futures
    /// settle daily via variation margin (mark-to-market).
    ///
    /// # Parameters
    ///
    /// - `future`: The bond future contract
    /// - `ctd_bond`: The cheapest-to-deliver bond
    /// - `conversion_factor`: Pre-calculated conversion factor for the CTD bond
    /// - `market`: Market context with discount curves
    /// - `as_of`: Valuation date
    ///
    /// # Returns
    ///
    /// Present value in the same currency as the future's notional
    ///
    /// # Example
    ///
    /// ```text
    /// use finstack_quant_core::currency::Currency;
    /// use finstack_quant_core::market_data::context::MarketContext;
    /// use finstack_quant_core::money::Money;
    /// use finstack_quant_core::types::{CurveId, InstrumentId};
    /// use finstack_quant_valuations::instruments::Bond;
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::{BondFuture, DeliverableBond, Position};
    /// use finstack_quant_valuations::instruments::fixed_income::bond_future::pricer::BondFuturePricer;
    /// use time::macros::date;
    ///
    /// # fn main() -> finstack_quant_core::Result<()> {
    /// let market = MarketContext::new();
    /// let ctd_bond_id = InstrumentId::new("US912828XG33");
    /// let future = BondFuture::ust_10y(
    ///     InstrumentId::new("TYH5"),
    ///     Money::new(1_000_000.0, Currency::USD),
    ///     date!(2025-03-20),
    ///     date!(2025-03-21),
    ///     date!(2025-03-31),
    ///     125.50,
    ///     Position::Long,
    ///     vec![DeliverableBond { bond_id: ctd_bond_id.clone(), conversion_factor: 0.8234 }],
    ///     ctd_bond_id.clone(),
    ///     CurveId::new("USD-TREASURY"),
    /// )?;
    /// let ctd_bond = Bond::fixed(
    ///     ctd_bond_id.as_str(),
    ///     Money::new(100_000.0, Currency::USD),
    ///     0.05,
    ///     date!(2020-01-15),
    ///     date!(2030-01-15),
    ///     "USD-OIS",
    /// )?;
    /// let cf = 0.8234;
    ///
    /// let npv = BondFuturePricer::calculate_npv(
    ///     &future,
    ///     &ctd_bond,
    ///     cf,
    ///     &market,
    ///     date!(2025-01-15),
    /// )?;
    /// // For a long position with model > contract price, NPV is positive
    /// # let _ = npv;
    /// # Ok(())
    /// # }
    /// ```
    pub fn calculate_npv(
        future: &super::BondFuture,
        ctd_bond: &Bond,
        conversion_factor: f64,
        market: &MarketContext,
        as_of: Date,
    ) -> Result<Money> {
        if as_of > future.delivery_end {
            return Ok(Money::new(0.0, future.notional.currency()));
        }
        // Calculate the theoretical model price, carrying the CTD forward to
        // the contract's delivery date.
        let model_price = Self::calculate_model_price_for_future(
            future,
            ctd_bond,
            conversion_factor,
            market,
            as_of,
        )?;

        // Calculate price differential
        let price_diff = model_price - future.quoted_price;

        // Position sign: +1 for Long, -1 for Short
        let position_sign = match future.position {
            Position::Long => 1.0,
            Position::Short => -1.0,
        };

        // Futures MTM: no discounting — exchange-traded futures settle daily
        // via variation margin, so the mark-to-market is undiscounted.
        let notional_value = future.notional.amount();
        let npv_amount = price_diff * (notional_value / 100.0) * position_sign;

        // Return as Money with same currency as notional
        Ok(Money::new(npv_amount, future.notional.currency()))
    }
}

// ========================= PRICER TRAIT IMPLEMENTATION =========================

impl crate::pricer::Pricer for BondFuturePricer {
    fn key(&self) -> crate::pricer::PricerKey {
        crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::BondFuture,
            crate::pricer::ModelKey::BondFutureCleanPriceProxy,
        )
    }

    fn price_dyn(
        &self,
        instrument: &dyn crate::instruments::common_impl::traits::Instrument,
        market: &MarketContext,
        as_of: finstack_quant_core::dates::Date,
    ) -> std::result::Result<crate::results::ValuationResult, PricingError> {
        // Type-safe downcast to BondFuture
        let future = instrument
            .as_any()
            .downcast_ref::<super::BondFuture>()
            .ok_or_else(|| {
                crate::pricer::PricingError::type_mismatch(
                    crate::pricer::InstrumentType::BondFuture,
                    instrument.key(),
                )
            })?;

        let ctx = crate::pricer::PricingErrorContext::new()
            .instrument_id(future.id.as_str())
            .instrument_type(crate::pricer::InstrumentType::BondFuture)
            .model(crate::pricer::ModelKey::BondFutureCleanPriceProxy);

        // Delegate to BondFuture::value(), which resolves the CTD bond and computes NPV.
        let npv = future
            .value(market, as_of)
            .map_err(|e| crate::pricer::PricingError::from_core(e, ctx))?;

        Ok(crate::results::ValuationResult::stamped(
            future.id.as_str(),
            as_of,
            npv,
        ))
    }
}

impl Default for BondFuturePricer {
    fn default() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::Date;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::market_data::term_structures::DiscountCurve;
    use finstack_quant_core::math::interp::InterpStyle;
    use finstack_quant_core::money::Money;
    use finstack_quant_core::types::{CurveId, InstrumentId};
    use time::macros::date;

    use crate::cashflow::traits::CashflowProvider;
    use crate::instruments::fixed_income::bond::Bond;

    /// Helper to create a simple market context with a flat discount curve
    fn create_test_market(rate: f64) -> MarketContext {
        // Create a flat discount curve at the given rate
        // Using a simple 2-knot curve to approximate flat discount rate
        let base_date = date!(2025 - 01 - 15);

        // Calculate discount factors for a flat rate
        // DF(t) = exp(-rate * t) for continuous compounding
        // For semi-annual compounding: DF(t) = 1 / (1 + rate/2)^(2*t)
        let df_1y = 1.0 / (1.0 + rate / 2.0).powi(2);
        let df_5y = 1.0 / (1.0 + rate / 2.0).powi(10);
        let df_10y = 1.0 / (1.0 + rate / 2.0).powi(20);

        let curve = DiscountCurve::builder(CurveId::new("USD-TREASURY"))
            .base_date(base_date)
            .knots(vec![
                (0.0, 1.0),     // Today
                (1.0, df_1y),   // 1 year
                (5.0, df_5y),   // 5 years
                (10.0, df_10y), // 10 years
            ])
            .interp(InterpStyle::Linear)
            .build()
            .expect("Failed to build discount curve");

        // insert_discount consumes self and returns Self (builder pattern)
        MarketContext::new().insert(curve)
    }

    /// Helper to create a test bond with fixed semi-annual coupons
    fn create_test_bond(notional: f64, coupon_rate: f64, issue: Date, maturity: Date) -> Bond {
        Bond::fixed(
            "TEST_BOND",
            Money::new(notional, Currency::USD),
            coupon_rate,
            issue,
            maturity,
            "USD-TREASURY",
        )
        .expect("Test bond creation should succeed")
    }

    #[test]
    fn test_cashflow_debug() {
        // Debug test to see what cashflows are generated
        let bond = create_test_bond(
            100_000.0,
            0.06,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.06);
        let as_of = date!(2025 - 01 - 15);

        let cashflows = bond
            .dated_cashflows(&market, as_of)
            .expect("Failed to build cashflow schedule");

        println!("\n=== Bond Cashflows ===");
        println!("As of: {:?}", as_of);
        println!("Total flows: {}", cashflows.len());
        let mut total = 0.0;
        for (date, amount) in &cashflows {
            println!("  {:?}: ${:.2}", date, amount.amount());
            total += amount.amount();
        }
        println!("Total cashflows: ${:.2}", total);
        println!("Expected for 100k notional with 6% coupon: ~$103,000 (coupons) + $100,000 (redemption)");
    }

    // ========== Conversion Factor Tests (CME standard formula) ==========
    //
    // The five reference values below are the published CME conversion
    // factors from CME Group, "Calculating U.S. Treasury Futures Conversion
    // Factors" (IR232) — the worked examples in that brochure. Each is a
    // real, identified U.S. Treasury issue with a specific delivery month.

    /// CME IR232 Example #1 — 2-Year U.S. Treasury Note futures.
    /// CUSIP 912828JP6, the 1-1/2s of October 31, 2010, December 2008 expiry.
    /// Published CME conversion factor: 0.9229.
    #[test]
    fn test_conversion_factor_cme_2y_example() {
        let bond = create_test_bond(
            100_000.0,
            0.015,
            date!(2008 - 10 - 31),
            date!(2010 - 10 - 31),
        );
        // standard_maturity_years = 2.0 -> monthly z rounding, v = z - 6.
        let cf =
            BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 2.0, date!(2008 - 12 - 01))
                .expect("CME 2Y conversion factor should compute");
        assert!(
            (cf - 0.9229).abs() <= 1e-4,
            "CME-published CF for 912828JP6 (Dec-2008) is 0.9229, got {cf}"
        );
    }

    /// CME IR232 Example #2 — 3-Year U.S. Treasury Note futures.
    /// CUSIP 912828KB5, the 1-1/8s of January 15, 2012, March 2009 expiry.
    /// Published CME conversion factor: 0.8747.
    #[test]
    fn test_conversion_factor_cme_3y_example() {
        let bond = create_test_bond(
            100_000.0,
            0.01125,
            date!(2009 - 01 - 15),
            date!(2012 - 01 - 15),
        );
        let cf =
            BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 3.0, date!(2009 - 03 - 01))
                .expect("CME 3Y conversion factor should compute");
        assert!(
            (cf - 0.8747).abs() <= 1e-4,
            "CME-published CF for 912828KB5 (Mar-2009) is 0.8747, got {cf}"
        );
    }

    /// CME IR232 Example #3 — 5-Year U.S. Treasury Note futures.
    /// CUSIP 912828JQ4, the 2-3/4s of October 31, 2013, December 2008 expiry.
    /// Published CME conversion factor: 0.8653.
    #[test]
    fn test_conversion_factor_cme_5y_example() {
        let bond = create_test_bond(
            100_000.0,
            0.0275,
            date!(2008 - 10 - 31),
            date!(2013 - 10 - 31),
        );
        let cf =
            BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 5.0, date!(2008 - 12 - 01))
                .expect("CME 5Y conversion factor should compute");
        assert!(
            (cf - 0.8653).abs() <= 1e-4,
            "CME-published CF for 912828JQ4 5Y (Dec-2008) is 0.8653, got {cf}"
        );
    }

    /// CME IR232 Example #4 — 10-Year U.S. Treasury Note futures.
    /// CUSIP 912828JR2, the 3-3/4s of November 15, 2018, December 2008 expiry.
    /// Published CME conversion factor: 0.8357.
    #[test]
    fn test_conversion_factor_cme_10y_example() {
        let bond = create_test_bond(
            100_000.0,
            0.0375,
            date!(2008 - 11 - 15),
            date!(2018 - 11 - 15),
        );
        // standard_maturity_years = 10.0 -> quarterly z rounding, v = 3 when z >= 7.
        let cf =
            BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 10.0, date!(2008 - 12 - 01))
                .expect("CME 10Y conversion factor should compute");
        assert!(
            (cf - 0.8357).abs() <= 1e-4,
            "CME-published CF for 912828JR2 (Dec-2008) is 0.8357, got {cf}"
        );
    }

    /// CME IR232 Example #5 — 30-Year U.S. Treasury Bond futures.
    /// CUSIP 912810PX0, the 4-1/2s of May 15, 2038, December 2008 expiry.
    /// Published CME conversion factor: 0.7943.
    #[test]
    fn test_conversion_factor_cme_30y_example() {
        let bond = create_test_bond(
            1_000_000.0,
            0.045,
            date!(2008 - 05 - 15),
            date!(2038 - 05 - 15),
        );
        let cf =
            BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 30.0, date!(2008 - 12 - 01))
                .expect("CME 30Y conversion factor should compute");
        assert!(
            (cf - 0.7943).abs() <= 1e-4,
            "CME-published CF for 912810PX0 (Dec-2008) is 0.7943, got {cf}"
        );
    }

    /// A 6%-coupon note with a whole number of years to delivery prices at
    /// exactly par under the 6% notional yield, so its CF must be 1.0000.
    #[test]
    fn test_conversion_factor_par_bond_is_one() {
        let bond = create_test_bond(
            100_000.0,
            0.06,
            date!(2020 - 12 - 15),
            date!(2030 - 12 - 15),
        );
        let cf =
            BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 10.0, date!(2025 - 12 - 01))
                .expect("par-bond conversion factor should compute");
        assert!(
            (cf - 1.0).abs() <= 1e-4,
            "6% coupon bond with whole-year maturity should have CF 1.0000, got {cf}"
        );
    }

    /// The conversion factor must not depend on the caller's valuation
    /// date: only the delivery month enters the CME formula. Two dates in
    /// the same delivery month must produce the identical factor.
    #[test]
    fn test_conversion_factor_independent_of_valuation_date() {
        let bond = create_test_bond(
            100_000.0,
            0.0375,
            date!(2008 - 11 - 15),
            date!(2018 - 11 - 15),
        );
        let cf_first =
            BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 10.0, date!(2008 - 12 - 01))
                .expect("conversion factor should compute");
        let cf_mid =
            BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 10.0, date!(2008 - 12 - 19))
                .expect("conversion factor should compute");
        assert!(
            (cf_first - cf_mid).abs() <= 1e-12,
            "CF must be anchored to the delivery month, not the valuation date: {cf_first} vs {cf_mid}"
        );
    }

    /// Discount (low-coupon) bonds have CF < 1, premium bonds CF > 1.
    #[test]
    fn test_conversion_factor_discount_vs_premium() {
        let discount = create_test_bond(
            100_000.0,
            0.03,
            date!(2020 - 06 - 15),
            date!(2032 - 06 - 15),
        );
        let premium = create_test_bond(
            100_000.0,
            0.09,
            date!(2020 - 06 - 15),
            date!(2032 - 06 - 15),
        );
        let cf_discount = BondFuturePricer::calculate_conversion_factor(
            &discount,
            0.06,
            10.0,
            date!(2025 - 06 - 01),
        )
        .expect("conversion factor should compute");
        let cf_premium = BondFuturePricer::calculate_conversion_factor(
            &premium,
            0.06,
            10.0,
            date!(2025 - 06 - 01),
        )
        .expect("conversion factor should compute");
        assert!(
            cf_discount < 1.0,
            "discount-bond CF should be < 1, got {cf_discount}"
        );
        assert!(
            cf_premium > 1.0,
            "premium-bond CF should be > 1, got {cf_premium}"
        );
    }

    /// Floating-rate deliverables have no fixed coupon; the CME factor is
    /// undefined and must surface a validation error rather than a number.
    #[test]
    fn test_conversion_factor_rejects_floating_bond() {
        use crate::instruments::fixed_income::bond::CashflowSpec;
        use finstack_quant_core::dates::{DayCount, Tenor};

        let mut bond = create_test_bond(
            100_000.0,
            0.05,
            date!(2020 - 06 - 15),
            date!(2032 - 06 - 15),
        );
        bond.cashflow_spec = CashflowSpec::floating(
            CurveId::new("USD-SOFR-3M"),
            50.0,
            Tenor::quarterly(),
            DayCount::Act360,
        )
        .expect("finite test rate");

        let result =
            BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 10.0, date!(2025 - 06 - 01));
        assert!(
            result.is_err(),
            "conversion factor must reject a floating-rate deliverable"
        );
    }

    // ========== Model Futures Price Tests ==========

    #[test]
    fn test_model_futures_price_par_bond() {
        // For a par bond (coupon = market rate), clean price should be ~100
        // Model futures price should be close to 100 / CF
        let bond = create_test_bond(
            100_000.0,
            0.06,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.06);
        let as_of = date!(2025 - 01 - 15);

        // Calculate CF (should be ~1.0 for par bond)
        let cf = BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 10.0, as_of)
            .expect("Failed to calculate conversion factor for par bond");

        // Calculate model futures price
        let model_price = BondFuturePricer::calculate_model_price(
            &bond,
            cf,
            &market,
            as_of,
            as_of + time::Duration::days(90),
        )
        .expect("Failed to calculate model futures price for par bond");

        // For a par bond with CF ~1.0, model price should be close to 100
        println!("CF: {}, Model Price: {}", cf, model_price);
        assert!(
            (model_price - 100.0).abs() < 5.0,
            "Par bond model price should be near 100, got {}",
            model_price
        );
    }

    #[test]
    fn test_model_futures_price_discount_bond() {
        // Discount bond: coupon < market rate, clean price < 100
        let bond = create_test_bond(
            100_000.0,
            0.04,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.06); // Higher market rate than coupon
        let as_of = date!(2025 - 01 - 15);

        let cf = BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 10.0, as_of)
            .expect("Failed to calculate conversion factor for discount bond");

        let model_price = BondFuturePricer::calculate_model_price(
            &bond,
            cf,
            &market,
            as_of,
            as_of + time::Duration::days(90),
        )
        .expect("Failed to calculate model futures price for discount bond");

        // Model price should be positive and reasonable
        println!("Discount bond - CF: {}, Model Price: {}", cf, model_price);
        assert!(model_price > 0.0, "Model price should be positive");
        assert!(model_price < 150.0, "Model price should be reasonable");
    }

    #[test]
    fn test_model_futures_price_premium_bond() {
        // Premium bond: coupon > market rate, clean price > 100
        let bond = create_test_bond(
            100_000.0,
            0.08,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.06); // Lower market rate than coupon
        let as_of = date!(2025 - 01 - 15);

        let cf = BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 10.0, as_of)
            .expect("Failed to calculate conversion factor for premium bond");

        let model_price = BondFuturePricer::calculate_model_price(
            &bond,
            cf,
            &market,
            as_of,
            as_of + time::Duration::days(90),
        )
        .expect("Failed to calculate model futures price for premium bond");

        // Model price should be above 100 for premium bond
        println!("Premium bond - CF: {}, Model Price: {}", cf, model_price);
        assert!(
            model_price > 95.0,
            "Premium bond model price should be reasonably high"
        );
        assert!(model_price < 150.0, "Model price should be reasonable");
    }

    #[test]
    fn test_model_futures_price_manual_verification() {
        // Manual verification test with known values
        let bond = create_test_bond(
            100_000.0,
            0.05,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.05);
        let as_of = date!(2025 - 01 - 15);

        let cf = BondFuturePricer::calculate_conversion_factor(&bond, 0.06, 10.0, as_of)
            .expect("Failed to calculate conversion factor for manual verification");
        let model_price = BondFuturePricer::calculate_model_price(
            &bond,
            cf,
            &market,
            as_of,
            as_of + time::Duration::days(90),
        )
        .expect("Failed to calculate model futures price for manual verification");

        println!("\n=== Manual Verification ===");
        println!("Bond: 5% coupon, priced at 5% market rate");
        println!("As of: {:?}", as_of);
        println!("Standard coupon: 6%");
        println!("Conversion Factor: {:.4}", cf);
        println!("Model Futures Price: {:.4}", model_price);

        // Bond should price at par (clean price ~100) when coupon = market rate
        // With CF < 1.0 (since coupon < standard), futures price should be > 100
        // Model_Price = Clean_Price_Percent / CF = 100 / CF
        let expected_approx = 100.0 / cf;
        println!("Expected (100/CF): {:.4}", expected_approx);

        assert!(
            (model_price - expected_approx).abs() < 5.0,
            "Model price should be approximately 100/CF"
        );
    }

    /// Item 9 regression: the model futures price must be the **carry-adjusted
    /// forward** CTD clean price divided by CF — not the spot `clean / CF`
    /// proxy, which omits cost-of-carry to delivery and biases the futures NPV.
    ///
    /// The test reconstructs the carry-adjusted forward independently and
    /// asserts the pricer matches it, and that it differs from the old
    /// spot-clean proxy by the carry over the delivery horizon.
    #[test]
    fn model_price_is_carry_adjusted_forward() {
        use crate::cashflow::accrual::accrued_interest_amount;

        // Discount/premium CTD over a steep curve so carry is material.
        let bond = create_test_bond(
            100_000.0,
            0.04,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.06);
        let as_of = date!(2025 - 01 - 15);
        // Delivery roughly six months out — a sizeable carry horizon.
        let delivery_date = date!(2025 - 07 - 15);
        let cf = 0.8234_f64;

        let model_price =
            BondFuturePricer::calculate_model_price(&bond, cf, &market, as_of, delivery_date)
                .expect("carry-adjusted model price should compute");

        // --- Independent carry-adjusted forward reconstruction ---
        let disc = market
            .get_discount(&bond.discount_curve_id)
            .expect("discount curve");
        let spot_dirty = bond.value(&market, as_of).expect("ctd PV").amount();
        let flows = bond
            .pricing_dated_cashflows(&market, as_of)
            .expect("ctd flows");
        let mut pv_interim = 0.0;
        for (date, amount) in &flows {
            if *date > as_of && *date <= delivery_date {
                pv_interim +=
                    amount.amount() * disc.df_between_dates(as_of, *date).expect("interim df");
            }
        }
        let df_delivery = disc
            .df_between_dates(as_of, delivery_date)
            .expect("delivery df");
        let forward_dirty = (spot_dirty - pv_interim) / df_delivery;
        let schedule = bond.full_cashflow_schedule(&market).expect("schedule");
        let accrued_delivery =
            accrued_interest_amount(&schedule, delivery_date, &bond.accrual_config())
                .expect("accrued at delivery");
        let forward_clean_pct = (forward_dirty - accrued_delivery) / bond.notional.amount() * 100.0;
        let expected = forward_clean_pct / cf;

        assert!(
            (model_price - expected).abs() < 1e-9,
            "model price must equal the carry-adjusted forward CTD clean / CF: \
             model={model_price}, expected={expected}"
        );

        // The carry-adjusted forward must differ from the old spot-clean proxy:
        // discounting the dirty PV to a clean spot price and dividing by CF.
        let spot_accrued = accrued_interest_amount(&schedule, as_of, &bond.accrual_config())
            .expect("accrued today");
        let spot_clean_pct = (spot_dirty - spot_accrued) / bond.notional.amount() * 100.0;
        let old_proxy = spot_clean_pct / cf;
        assert!(
            (model_price - old_proxy).abs() > 1e-3,
            "carry adjustment must shift the model price away from the spot \
             clean/CF proxy (model={model_price}, old_proxy={old_proxy})"
        );
    }

    // ========== NPV Calculation Tests ==========

    /// Helper to create a test BondFuture
    fn create_test_bond_future(
        notional: f64,
        quoted_price: f64,
        position: Position,
        expiry: Date,
    ) -> crate::instruments::fixed_income::bond_future::BondFuture {
        use crate::instruments::fixed_income::bond_future::{
            BondFuture, BondFutureSpecs, DeliverableBond,
        };

        BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(notional, Currency::USD))
            .expiry(expiry)
            .delivery_start(expiry + time::Duration::days(1))
            .delivery_end(expiry + time::Duration::days(10))
            .quoted_price(quoted_price)
            .position(position)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![DeliverableBond {
                bond_id: InstrumentId::new("TEST_BOND"),
                conversion_factor: 0.8234,
            }])
            .ctd_bond_id(InstrumentId::new("TEST_BOND"))
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("Failed to build test bond future")
    }

    #[test]
    fn test_npv_long_position() {
        // Setup: Long position where quoted price > model price (profitable)
        let quoted_price = 125.50;
        let notional = 1_000_000.0; // 10 contracts × $100k
        let expiry = date!(2025 - 03 - 20);

        let future = create_test_bond_future(notional, quoted_price, Position::Long, expiry);

        // Create CTD bond that will result in model price < quoted price
        let ctd_bond = create_test_bond(
            100_000.0,
            0.05,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.06); // Higher market rate → lower bond price → lower model price
        let as_of = date!(2025 - 01 - 15);

        // Calculate conversion factor
        let cf = BondFuturePricer::calculate_conversion_factor(&ctd_bond, 0.06, 10.0, as_of)
            .expect("Failed to calculate conversion factor");

        // Calculate NPV
        let npv = BondFuturePricer::calculate_npv(&future, &ctd_bond, cf, &market, as_of)
            .expect("Failed to calculate NPV for long position");

        println!("\n=== NPV Long Position Test ===");
        println!("Quoted Price: {:.4}", quoted_price);
        println!("Conversion Factor: {:.4}", cf);
        println!("Notional: ${:.0}", notional);
        println!("NPV: ${:.2}", npv.amount());

        // For a long position with quoted > model, NPV should be positive
        // The exact value depends on the model price, but it should be positive
        // and scale with notional

        // Verify:
        // 1. NPV currency matches future currency
        assert_eq!(
            npv.currency(),
            future.notional.currency(),
            "NPV currency should match future currency"
        );

        // 2. NPV magnitude is reasonable (should be less than notional)
        assert!(
            npv.amount().abs() < notional,
            "NPV magnitude should be less than notional"
        );

        // 3. For most realistic scenarios with quoted around 125.50 and market rate 6%,
        //    model price will be in the range 90-110, giving a positive NPV for long
        // Note: We can't assert positive without knowing exact model price,
        //       but we can verify the calculation mechanics work
        println!("NPV calculation successful for long position");
    }

    #[test]
    fn future_model_price_and_npv_share_special_repo_curve() {
        use finstack_quant_core::market_data::term_structures::DiscountCurve;

        let as_of = date!(2025 - 01 - 15);
        let mut future =
            create_test_bond_future(1_000_000.0, 120.0, Position::Long, date!(2025 - 03 - 20));
        future.repo_curve_id = Some(CurveId::new("USD-SPECIAL-REPO"));
        let bond = create_test_bond(
            100_000.0,
            0.05,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.06).insert(
            DiscountCurve::builder("USD-SPECIAL-REPO")
                .base_date(as_of)
                .knots(vec![(0.0, 1.0), (1.0, (-0.02_f64).exp())])
                .build()
                .expect("repo curve"),
        );
        let cf = 0.8234;

        let model =
            BondFuturePricer::calculate_model_price_for_future(&future, &bond, cf, &market, as_of)
                .expect("future-aware model price");
        let fallback = BondFuturePricer::calculate_model_price(
            &bond,
            cf,
            &market,
            as_of,
            future.delivery_start,
        )
        .expect("discount-curve fallback");
        let npv = BondFuturePricer::calculate_npv(&future, &bond, cf, &market, as_of).expect("npv");
        let implied_model = future.quoted_price + npv.amount() * 100.0 / future.notional.amount();

        assert!((model - implied_model).abs() < 1e-10);
        assert!(
            (model - fallback).abs() > 1e-6,
            "special repo must affect carry"
        );
    }

    #[test]
    fn test_npv_short_position() {
        // Setup: Short position (opposite sign to long)
        let quoted_price = 125.50;
        let notional = 1_000_000.0; // 10 contracts × $100k
        let expiry = date!(2025 - 03 - 20);

        let future = create_test_bond_future(notional, quoted_price, Position::Short, expiry);

        // Use same CTD bond and market as long position test
        let ctd_bond = create_test_bond(
            100_000.0,
            0.05,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.06);
        let as_of = date!(2025 - 01 - 15);

        let cf = BondFuturePricer::calculate_conversion_factor(&ctd_bond, 0.06, 10.0, as_of)
            .expect("Failed to calculate conversion factor");

        let npv_short = BondFuturePricer::calculate_npv(&future, &ctd_bond, cf, &market, as_of)
            .expect("Failed to calculate NPV for short position");

        // For comparison, calculate NPV for equivalent long position
        let future_long = create_test_bond_future(notional, quoted_price, Position::Long, expiry);

        let npv_long = BondFuturePricer::calculate_npv(&future_long, &ctd_bond, cf, &market, as_of)
            .expect("Failed to calculate NPV for long position");

        println!("\n=== NPV Short Position Test ===");
        println!("Quoted Price: {:.4}", quoted_price);
        println!("Conversion Factor: {:.4}", cf);
        println!("NPV Short: ${:.2}", npv_short.amount());
        println!("NPV Long: ${:.2}", npv_long.amount());

        // Verify that short NPV = -1 × long NPV (within floating point precision)
        let expected_short = -npv_long.amount();
        assert!(
            (npv_short.amount() - expected_short).abs() < 1.0,
            "Short NPV should be negative of long NPV. Short: {:.2}, Expected: {:.2}",
            npv_short.amount(),
            expected_short
        );

        println!("NPV calculation successful: Short = -Long (within precision)");
    }

    #[test]
    fn test_npv_manual_calculation() {
        // Manual verification test with explicit values
        // This test verifies the NPV formula step-by-step

        let quoted_price = 125.00; // Round number for easier calculation
        let notional = 1_000_000.0; // 10 contracts
        let expiry = date!(2025 - 03 - 20);

        let future = create_test_bond_future(notional, quoted_price, Position::Long, expiry);

        // Create a par bond (coupon = market rate) for predictable model price
        let ctd_bond = create_test_bond(
            100_000.0,
            0.06,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
        );
        let market = create_test_market(0.06);
        let as_of = date!(2025 - 01 - 15);

        // Calculate components
        let cf = BondFuturePricer::calculate_conversion_factor(&ctd_bond, 0.06, 10.0, as_of)
            .expect("Failed to calculate conversion factor");

        // Use the contract's delivery date so the model price matches the one
        // `calculate_npv` derives internally (it carries the CTD to delivery).
        let model_price = BondFuturePricer::calculate_model_price(
            &ctd_bond,
            cf,
            &market,
            as_of,
            future.delivery_start,
        )
        .expect("Failed to calculate model price");

        let npv = BondFuturePricer::calculate_npv(&future, &ctd_bond, cf, &market, as_of)
            .expect("Failed to calculate NPV");

        println!("\n=== NPV Manual Verification ===");
        println!("Quoted Price: {:.4}", quoted_price);
        println!("Model Price: {:.4}", model_price);
        println!("Price Differential: {:.4}", model_price - quoted_price);
        println!("Conversion Factor: {:.4}", cf);
        println!("Notional: ${:.0}", notional);

        // Manual model-to-contract value (undiscounted futures convention).
        let price_diff = model_price - quoted_price;
        let manual_npv = price_diff * (notional / 100.0) * 1.0; // 1.0 for Long

        println!("Manual NPV: ${:.2}", manual_npv);
        println!("Calculated NPV: ${:.2}", npv.amount());

        // Verify match (within $100 tolerance for floating point)
        assert!(
            (npv.amount() - manual_npv).abs() < 100.0,
            "NPV should match manual calculation. Calculated: {:.2}, Manual: {:.2}",
            npv.amount(),
            manual_npv
        );

        println!("NPV formula verification successful!");
    }

    // ========== Pricer Registration Tests ==========

    #[test]
    fn test_pricer_registration() {
        // Test that BondFuturePricer is registered in the standard registry
        let registry = crate::pricer::standard_registry();
        let key = crate::pricer::PricerKey::new(
            crate::pricer::InstrumentType::BondFuture,
            crate::pricer::ModelKey::BondFutureCleanPriceProxy,
        );

        // Should be able to retrieve the pricer
        assert!(
            registry.get_pricer(key).is_some(),
            "BondFuturePricer should be registered in standard registry"
        );
    }

    #[test]
    fn test_pricer_key() {
        // Test that BondFuturePricer returns the correct key
        use crate::pricer::Pricer;

        let pricer = BondFuturePricer;
        let key = pricer.key();

        assert_eq!(
            key.instrument,
            crate::pricer::InstrumentType::BondFuture,
            "Pricer should have BondFuture instrument type"
        );
        assert_eq!(
            key.model,
            crate::pricer::ModelKey::BondFutureCleanPriceProxy,
            "Pricer should use explicit clean-price proxy model"
        );
    }

    #[test]
    fn test_pricer_price_dyn_uses_ctd_bond() {
        use crate::instruments::fixed_income::bond_future::{
            BondFuture, BondFutureSpecs, DeliverableBond,
        };
        use crate::pricer::Pricer;

        let as_of = date!(2025 - 01 - 15);
        let expiry = date!(2025 - 03 - 20);
        let delivery_start = date!(2025 - 03 - 21);
        let delivery_end = date!(2025 - 03 - 31);

        let bond_a = Bond::fixed(
            "BOND-A",
            Money::new(100_000.0, Currency::USD),
            0.04,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
            "USD-TREASURY",
        )
        .expect("Bond::fixed should succeed with valid parameters");
        let bond_b = Bond::fixed(
            "BOND-B",
            Money::new(100_000.0, Currency::USD),
            0.06,
            date!(2020 - 01 - 15),
            date!(2030 - 01 - 15),
            "USD-TREASURY",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let market = create_test_market(0.05);

        let cf_a =
            BondFuturePricer::calculate_conversion_factor(&bond_a, 0.06, 10.0, delivery_start)
                .expect("Failed to calculate conversion factor for bond A");
        let cf_b =
            BondFuturePricer::calculate_conversion_factor(&bond_b, 0.06, 10.0, delivery_start)
                .expect("Failed to calculate conversion factor for bond B");

        let future = BondFuture::builder()
            .id(InstrumentId::new("TYH5"))
            .notional(Money::new(1_000_000.0, Currency::USD))
            .expiry(expiry)
            .delivery_start(delivery_start)
            .delivery_end(delivery_end)
            .quoted_price(125.50)
            .position(Position::Long)
            .contract_specs(BondFutureSpecs::default())
            .deliverable_basket(vec![
                DeliverableBond {
                    bond_id: InstrumentId::new("BOND-A"),
                    conversion_factor: cf_a,
                },
                DeliverableBond {
                    bond_id: InstrumentId::new("BOND-B"),
                    conversion_factor: cf_b,
                },
            ])
            .ctd_bond_id(InstrumentId::new("BOND-B"))
            .ctd_bond(bond_b.clone())
            .discount_curve_id(CurveId::new("USD-TREASURY"))
            .attributes(crate::instruments::common_impl::traits::Attributes::new())
            .build()
            .expect("Valid bond future");

        let pricer = BondFuturePricer;
        let result = pricer
            .price_dyn(&future, &market, as_of)
            .expect("price_dyn should succeed for bond futures");

        let expected = BondFuturePricer::calculate_npv(&future, &bond_b, cf_b, &market, as_of)
            .expect("Failed to calculate expected NPV for CTD bond");
        let alt = BondFuturePricer::calculate_npv(&future, &bond_a, cf_a, &market, as_of)
            .expect("Failed to calculate alternate NPV for non-CTD bond");

        let diff = (result.value.amount() - expected.amount()).abs();
        assert!(
            diff < 1e-8,
            "Pricer NPV should match CTD bond NPV, diff={}",
            diff
        );
        assert!(
            (expected.amount() - alt.amount()).abs() > 1e-6,
            "CTD and non-CTD NPVs should differ"
        );
    }
}
