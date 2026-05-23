//! Fixed and floating-rate bond instruments with embedded options.
//!
//! Provides comprehensive bond modeling including:
//! - Fixed-rate coupon bonds (bullet and amortizing)
//! - Floating-rate notes (FRNs) with caps/floors
//! - Callable and putable bonds (American/Bermudan exercise)
//! - Zero-coupon bonds
//! - Custom cashflow schedules (including PIK and bespoke amortization)
//!
//! # Signed Canonical Schedule
//!
//! All bond cashflows in this module follow a **signed canonical schedule** convention:
//! - **Positive amounts** represent contractual inflows to a long holder
//!   (coupons, amortization, redemption).
//! - **Initial draw / funding legs** are handled outside the schedule
//!   (e.g., via trade price) and are **not** included in the projected
//!   cashflow schedule.
//!
//! This convention is enforced by the bond's `CashflowProvider::dated_cashflows` implementation, which turns the
//! internal cashflow schedule into a simplified `(Date, Money)` stream used
//! by pricing and risk engines.
//!
//! # Bond Pricing
//!
//! Bonds are priced by discounting all future cashflows from the signed canonical schedule:
//!
//! ```text
//! PV = Σ CF_i · DF(as_of → t_i)
//! ```
//!
//! For bonds with embedded options (calls/puts), tree-based pricing is used
//! to value the optionality. The short-rate / rates+credit trees operate on
//! a time axis measured from `as_of` using the discount curve’s own
//! day-count, so that:
//! - `t = 0` corresponds to the valuation date `as_of`
//! - `t > 0` are year-fractions to future cashflow and exercise dates
//!
//! **Important**: PV is always anchored at `as_of` (the valuation date), not the
//! settlement date. This is the instrument's theoretical value on the valuation
//! date. Settlement-date pricing (for trade execution) is a separate concern.
//!
//! # Quote-Date Convention for Yield Metrics
//!
//! While PV is anchored at `as_of`, market-derived metrics (YTM, Z-spread, DM,
//! OAS, duration, convexity) are computed from the **quote date** (settlement
//! date) because market quotes reflect settlement-date pricing:
//!
//! - **quote_date** = `as_of + settlement_days` (or `as_of` if no settlement convention)
//! - **accrued_at_quote_date** = accrued interest computed at quote_date
//! - **dirty_price** = clean_price * notional / 100 + accrued_at_quote_date
//!
//! This separation ensures that:
//! 1. Curve discounting always uses `as_of` as the anchor
//! 2. Quote-derived metrics properly interpret market prices as settlement quotes
//! 3. The YTM/duration/convexity numbers match market standard conventions
//!
//! # Call/Put Exercise Convention
//!
//! For callable/putable bonds:
//!
//! - **`CallPut.price_pct_of_par`** is applied to the **outstanding principal**
//!   at the exercise date, not the original notional. This correctly handles
//!   amortizing callable bonds.
//! - **Exercise payoff**: Coupon is always paid regardless of exercise decision.
//!   The exercise decision applies only to the principal redemption vs. continuation.
//! - **Formula**: `node_value = coupon + min(max(continuation, put_price), call_price)`
//!
//! # Accrual and Ex-Coupon Conventions
//!
//! Accrued interest is driven directly off the true coupon schedule and
//! outstanding notional (for amortizing structures), with explicit support
//! for:
//! - Linear vs. compounded accrual (`AccrualMethod`)
//! - Ex-coupon windows where accrual drops to zero
//! - Custom-cashflow bonds that provide their own schedule and day-count
//!
//! # Regional Market Conventions
//!
//! Different bond markets follow distinct conventions:
//!
//! - **US Treasuries**: ACT/ACT ICMA, Semi-annual, T+1 settlement
//! - **UK Gilts**: ACT/ACT, Semi-annual, T+1 settlement
//! - **Eurozone**: 30E/360 or ACT/ACT, Annual, T+2 settlement
//! - **Japan**: ACT/365F, Semi-annual, T+2 settlement (cross-border)
//!
//! Use `Bond::with_convention()` for standard regional conventions.
//!
//! # Key Metrics
//!
//! - **Yield to Maturity (YTM)**: Internal rate of return
//! - **Modified Duration**: Interest rate sensitivity
//! - **Convexity**: Curvature of price/yield relationship
//! - **DV01**: Dollar value of 1bp rate change
//! - **Z-spread**: Spread over benchmark curve
//! - **Accrued Interest**: Interest accrued since last coupon
//!
//! # Examples
//!
//! See [`Bond`] for construction examples.
//!
//! # See Also
//!
//! - [`Bond`] for the main bond struct and factory methods
//! - `CallPutSchedule` for embedded option schedules
//! - `CashflowSpec` for fixed/floating/amortizing specifications
//! - `AmortizationSpec` for amortizing bonds
//! - bond metrics module for bond-specific risk metrics

pub mod cashflow_spec;
pub mod cashflows;
pub(crate) mod metrics;
/// Bond pricing engines including tree-based and analytical methods.
pub mod pricing;
mod types;

// Re-export cashflow accrual types for convenience
pub use crate::cashflow::accrual::AccrualMethod;
pub use cashflow_spec::{BondBuilderParams, CashflowSpec, FloatingConventionParams};
#[doc(hidden)]
pub use metrics::price_yield_spread::asw::{
    asw_market_with_forward, asw_market_with_forward_config, asw_par_with_forward,
    asw_par_with_forward_config, AssetSwapConfig,
};
#[doc(hidden)]
pub use metrics::{
    AssetSwapMarketCalculator, AssetSwapParCalculator, DiscountMarginCalculator, ZSpreadCalculator,
};
pub use types::AmortizationSpec;
pub use types::Bond;
pub use types::BondSettlementConvention;
pub use types::CallPut;
pub use types::CallPutSchedule;
pub use types::MakeWholeSpec;

#[cfg(test)]
mod tests {
    use crate::instruments::common_impl::parameters::BondConvention;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::instruments::fixed_income::bond::{Bond, CashflowSpec};
    use crate::instruments::PricingOverrides;
    use crate::pricer::InstrumentType;
    use finstack_core::currency::Currency;
    use finstack_core::dates::{BusinessDayConvention, DayCount, StubKind, Tenor};
    use finstack_core::money::Money;
    use finstack_core::types::CurveId;
    use rust_decimal::Decimal;
    use time::macros::date;

    #[test]
    fn test_bond_builder_minimal() {
        let bond = Bond::builder()
            .id("BOND_MIN".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build();

        assert!(bond.is_ok());
        let bond = bond.expect("should succeed");
        assert_eq!(bond.id.as_str(), "BOND_MIN");
        assert_eq!(bond.notional.amount(), 1000.0);
        assert_eq!(bond.discount_curve_id.as_str(), "USD-OIS");
    }

    #[test]
    fn test_bond_fixed_factory() {
        let bond = Bond::fixed(
            "BOND_FIXED",
            Money::new(100.0, Currency::USD),
            0.04,
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            "USD-TREASURY",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        assert_eq!(bond.id.as_str(), "BOND_FIXED");
        assert_eq!(bond.cashflow_spec.frequency(), Tenor::semi_annual());
        assert_eq!(bond.cashflow_spec.day_count(), DayCount::Thirty360);
        assert_eq!(bond.discount_curve_id.as_str(), "USD-TREASURY");
    }

    #[test]
    fn test_bond_with_convention_us_treasury() {
        let bond = Bond::with_convention(
            "UST-10Y",
            Money::new(1000.0, Currency::USD),
            0.03,
            date!(2025 - 01 - 01),
            date!(2035 - 01 - 01),
            BondConvention::USTreasury,
            "USD-TREASURY",
        )
        .expect("Bond::with_convention should succeed for US Treasury");

        assert_eq!(bond.id.as_str(), "UST-10Y");
        assert_eq!(
            bond.cashflow_spec.frequency(),
            BondConvention::USTreasury.frequency()
        );
        assert_eq!(
            bond.cashflow_spec.day_count(),
            BondConvention::USTreasury.day_count()
        );
        assert_eq!(
            bond.settlement_days(),
            Some(BondConvention::USTreasury.settlement_days())
        );
        assert_eq!(
            bond.ex_coupon_days(),
            BondConvention::USTreasury.ex_coupon_days()
        );
    }

    #[test]
    fn test_bond_with_convention_uk_gilt() {
        let bond = Bond::with_convention(
            "GILT-10Y",
            Money::new(1000.0, Currency::GBP),
            0.025,
            date!(2025 - 01 - 01),
            date!(2035 - 01 - 01),
            BondConvention::UKGilt,
            "GBP-GILTS",
        )
        .expect("Bond::with_convention should succeed for UK Gilt");

        assert_eq!(
            bond.cashflow_spec.frequency(),
            BondConvention::UKGilt.frequency()
        );
        assert_eq!(
            bond.cashflow_spec.day_count(),
            BondConvention::UKGilt.day_count()
        );
        assert_eq!(
            bond.settlement_days(),
            Some(BondConvention::UKGilt.settlement_days())
        );
        assert_eq!(
            bond.ex_coupon_days(),
            BondConvention::UKGilt.ex_coupon_days()
        );
    }

    #[test]
    fn test_bond_with_convention_sets_end_of_month() {
        let bond = Bond::with_convention(
            "EOM-UST",
            Money::new(1000.0, Currency::USD),
            0.03,
            date!(2025 - 01 - 31),
            date!(2030 - 01 - 31),
            BondConvention::USTreasury,
            "USD-TREASURY",
        )
        .expect("Bond::with_convention should succeed for EOM bond");

        if let CashflowSpec::Fixed(spec) = &bond.cashflow_spec {
            assert!(spec.end_of_month, "EOM bonds should enable EOM roll");
        } else {
            panic!("Expected fixed cashflow spec");
        }
    }

    #[test]
    fn test_bond_with_pricing_overrides() {
        let overrides = PricingOverrides::default()
            .with_quoted_clean_price(98.5)
            .with_ytm_bump_decimal(1e-4);

        let bond = Bond::builder()
            .id("BOND_OVERRIDE".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.06, Tenor::annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(overrides)
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        assert_eq!(
            bond.pricing_overrides.market_quotes.quoted_clean_price,
            Some(98.5)
        );
        assert_eq!(
            bond.pricing_overrides.metrics.bump_config.ytm_bump_decimal,
            Some(1e-4)
        );
    }

    #[test]
    fn test_bond_with_settlement_convention() {
        use crate::instruments::fixed_income::bond::BondSettlementConvention;

        let bond = Bond::builder()
            .id("BOND_SETTLE".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .settlement_convention_opt(Some(BondSettlementConvention {
                settlement_days: 2,
                ex_coupon_days: 7,
                ..Default::default()
            }))
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        assert_eq!(bond.settlement_days(), Some(2));
        assert_eq!(bond.ex_coupon_days(), Some(7));
    }

    #[test]
    fn test_bond_with_attributes() {
        let mut attrs = Attributes::new();
        attrs
            .meta
            .insert("sector".to_string(), "corporate".to_string());
        attrs.meta.insert("rating".to_string(), "AA".to_string());

        let bond = Bond::builder()
            .id("BOND_ATTRS".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(attrs)
            .build()
            .expect("should succeed");

        assert_eq!(
            bond.attributes.meta.get("sector"),
            Some(&"corporate".to_string())
        );
        assert_eq!(bond.attributes.meta.get("rating"), Some(&"AA".to_string()));
    }

    #[test]
    fn test_bond_zero_coupon() {
        let bond = Bond::builder()
            .id("ZERO_COUPON".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.0, Tenor::annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        // Zero coupon bond
        if let CashflowSpec::Fixed(spec) = &bond.cashflow_spec {
            assert_eq!(spec.rate, Decimal::ZERO);
        } else {
            panic!("Expected Fixed cashflow spec");
        }
    }

    #[test]
    fn test_bond_high_frequency() {
        let bond = Bond::builder()
            .id("MONTHLY".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2027 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.06, Tenor::monthly(), DayCount::Act360)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        assert_eq!(bond.cashflow_spec.frequency(), Tenor::monthly());
    }

    #[test]
    fn test_bond_with_calendar() {
        use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};

        let spec = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.05).expect("valid"),
            freq: Tenor::semi_annual(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::ModifiedFollowing,
            calendar_id: "USGS".to_string(),
            stub: StubKind::None,
            end_of_month: false,
            payment_lag_days: 0,
        };

        let bond = Bond::builder()
            .id("BOND_CAL".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(CashflowSpec::Fixed(spec.clone()))
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        if let CashflowSpec::Fixed(s) = &bond.cashflow_spec {
            assert_eq!(s.calendar_id, "USGS".to_string());
            assert_eq!(s.bdc, BusinessDayConvention::ModifiedFollowing);
        } else {
            panic!("Expected Fixed cashflow spec");
        }
    }

    #[test]
    fn test_bond_stub_conventions() {
        use crate::cashflow::builder::specs::{CouponType, FixedCouponSpec};

        // Short front stub
        let spec_short = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.05).expect("valid"),
            freq: Tenor::semi_annual(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::ShortFront,
            end_of_month: false,
            payment_lag_days: 0,
        };

        let bond_short_front = Bond::builder()
            .id("STUB_SHORT_FRONT".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 15))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(CashflowSpec::Fixed(spec_short.clone()))
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        if let CashflowSpec::Fixed(s) = &bond_short_front.cashflow_spec {
            assert_eq!(s.stub, StubKind::ShortFront);
        }

        // Long back stub
        let spec_long = FixedCouponSpec {
            coupon_type: CouponType::Cash,
            rate: Decimal::try_from(0.05).expect("valid"),
            freq: Tenor::semi_annual(),
            dc: DayCount::Act365F,
            bdc: BusinessDayConvention::Following,
            calendar_id: "weekends_only".to_string(),
            stub: StubKind::LongBack,
            end_of_month: false,
            payment_lag_days: 0,
        };

        let bond_long_back = Bond::builder()
            .id("STUB_LONG_BACK".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 02 - 15))
            .cashflow_spec(CashflowSpec::Fixed(spec_long.clone()))
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        if let CashflowSpec::Fixed(s) = &bond_long_back.cashflow_spec {
            assert_eq!(s.stub, StubKind::LongBack);
        }
    }

    #[test]
    fn test_bond_different_currencies() {
        let currencies = vec![
            (Currency::USD, "USD"),
            (Currency::EUR, "EUR"),
            (Currency::GBP, "GBP"),
            (Currency::JPY, "JPY"),
            (Currency::CHF, "CHF"),
        ];

        for (ccy, code) in currencies {
            let bond = Bond::builder()
                .id(format!("BOND_{}", code).into())
                .notional(Money::new(1000.0, ccy))
                .issue_date(date!(2025 - 01 - 01))
                .maturity(date!(2030 - 01 - 01))
                .cashflow_spec(
                    CashflowSpec::fixed(0.04, Tenor::annual(), DayCount::Act365F)
                        .expect("finite test coupon"),
                )
                .discount_curve_id(CurveId::new(format!("{}-OIS", code)))
                .pricing_overrides(PricingOverrides::default())
                .attributes(Attributes::new())
                .build()
                .expect("should succeed");

            assert_eq!(bond.notional.currency(), ccy);
        }
    }

    #[test]
    fn test_bond_instrument_trait() {
        let bond = Bond::fixed(
            "TRAIT_TEST",
            Money::new(1000.0, Currency::USD),
            0.05,
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let inst: &dyn Instrument = &bond;
        assert_eq!(inst.id(), "TRAIT_TEST");
        assert_eq!(inst.key(), InstrumentType::Bond);
        assert!(inst.as_any().is::<Bond>());
    }

    #[test]
    fn test_bond_clone_and_equality() {
        let bond1 = Bond::fixed(
            "CLONE_TEST",
            Money::new(1000.0, Currency::USD),
            0.05,
            date!(2025 - 01 - 01),
            date!(2030 - 01 - 01),
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let bond2 = bond1.clone();

        assert_eq!(bond1.id.as_str(), bond2.id.as_str());
        assert_eq!(bond1.notional.amount(), bond2.notional.amount());
        assert_eq!(bond1.maturity, bond2.maturity);
    }

    #[test]
    fn test_bond_near_maturity() {
        let issue = date!(2025 - 01 - 01);
        let maturity = date!(2025 - 02 - 01); // 1 month

        let bond = Bond::fixed(
            "SHORT_TERM",
            Money::new(1000.0, Currency::USD),
            0.03,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        assert!(bond.maturity > bond.issue_date);
        let days_to_maturity = (bond.maturity - bond.issue_date).whole_days();
        assert!(days_to_maturity < 365);
    }

    #[test]
    fn test_bond_long_maturity() {
        let issue = date!(2025 - 01 - 01);
        let maturity = date!(2055 - 01 - 01); // 30 years

        let bond = Bond::fixed(
            "LONG_TERM",
            Money::new(1000.0, Currency::USD),
            0.045,
            issue,
            maturity,
            "USD-OIS",
        )
        .expect("Bond::fixed should succeed with valid parameters");

        let years_to_maturity = bond
            .cashflow_spec
            .day_count()
            .year_fraction(
                bond.issue_date,
                bond.maturity,
                finstack_core::dates::DayCountContext::default(),
            )
            .expect("year fraction");
        assert!(years_to_maturity >= 30.0);
    }

    #[test]
    fn test_bond_premium_discount_par() {
        // Premium bond (price > 100)
        let premium = Bond::builder()
            .id("PREMIUM".into())
            .notional(Money::new(100.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.08, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default().with_quoted_clean_price(105.0))
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        assert_eq!(
            premium.pricing_overrides.market_quotes.quoted_clean_price,
            Some(105.0)
        );

        // Discount bond (price < 100)
        let discount = Bond::builder()
            .id("DISCOUNT".into())
            .notional(Money::new(100.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.03, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default().with_quoted_clean_price(95.0))
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        assert_eq!(
            discount.pricing_overrides.market_quotes.quoted_clean_price,
            Some(95.0)
        );

        // Par bond (price = 100)
        let par = Bond::builder()
            .id("PAR".into())
            .notional(Money::new(100.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default().with_quoted_clean_price(100.0))
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        assert_eq!(
            par.pricing_overrides.market_quotes.quoted_clean_price,
            Some(100.0)
        );
    }

    #[test]
    fn test_bond_builder_defaults_issue_date_from_maturity() {
        let maturity = date!(2030 - 06 - 15);
        let bond = Bond::builder()
            .id("NO_ISSUE".into())
            .notional(Money::new(1000.0, Currency::USD))
            .maturity(maturity)
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should succeed with defaulted issue_date");

        let expected_issue = maturity
            .checked_sub(time::Duration::days(365))
            .expect("subtraction should succeed");
        assert_eq!(bond.issue_date, expected_issue);
        assert!(bond.issue_date < bond.maturity);
    }

    #[test]
    fn test_bond_builder_explicit_issue_date_takes_precedence() {
        let bond = Bond::builder()
            .id("EXPLICIT_ISSUE".into())
            .notional(Money::new(1000.0, Currency::USD))
            .issue_date(date!(2024 - 03 - 01))
            .maturity(date!(2030 - 06 - 15))
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(PricingOverrides::default())
            .attributes(Attributes::new())
            .build()
            .expect("should succeed");

        assert_eq!(bond.issue_date, date!(2024 - 03 - 01));
    }

    // -- Price-from-quote override regression tests ------------------------------
    //
    // Verify that every price-driving field on `MarketQuoteOverrides` drives
    // `Bond::base_value` through the precedence chain defined on the struct.
    fn build_test_bond(overrides: PricingOverrides) -> Bond {
        Bond::builder()
            .id("BOND_QUOTE".into())
            .notional(Money::new(1_000_000.0, Currency::USD))
            .issue_date(date!(2025 - 01 - 01))
            .maturity(date!(2030 - 01 - 01))
            .cashflow_spec(
                CashflowSpec::fixed(0.05, Tenor::semi_annual(), DayCount::Act365F)
                    .expect("finite test coupon"),
            )
            .discount_curve_id("USD-OIS".into())
            .pricing_overrides(overrides)
            .attributes(Attributes::new())
            .build()
            .expect("bond should build")
    }

    fn flat_discount_market(rate: f64) -> finstack_core::market_data::context::MarketContext {
        use finstack_core::market_data::DiscountCurve;
        let tenors = [0.0_f64, 0.5, 1.0, 2.0, 3.0, 5.0, 7.0, 10.0];
        let knots: Vec<(f64, f64)> = tenors.iter().map(|&t| (t, (-rate * t).exp())).collect();
        let curve = DiscountCurve::builder("USD-OIS")
            .base_date(date!(2025 - 01 - 01))
            .knots(knots)
            .build()
            .expect("flat curve");
        finstack_core::market_data::context::MarketContext::new().insert(curve)
    }

    #[test]
    fn bond_value_honors_quoted_clean_price_override() {
        let overrides = PricingOverrides::default().with_quoted_clean_price(98.5);
        let bond = build_test_bond(overrides);
        let market = flat_discount_market(0.03);
        let pv = bond.value(&market, date!(2025 - 01 - 01)).expect("value");
        // Clean price 98.5% of 1M notional, zero accrued at issue date.
        assert!((pv.amount() - 985_000.0).abs() < 1e-4);
    }

    #[test]
    fn bond_value_honors_quoted_dirty_price_override() {
        let overrides = PricingOverrides::default().with_quoted_dirty_price(987_654.32);
        let bond = build_test_bond(overrides);
        let market = flat_discount_market(0.03);
        let pv = bond.value(&market, date!(2025 - 01 - 01)).expect("value");
        assert!((pv.amount() - 987_654.32).abs() < 1e-4);
    }

    #[test]
    fn bond_value_honors_quoted_ytm_override() {
        // A bond quoted at YTM = its coupon rate on a coupon date trades at par
        // (clean == 100, dirty == notional).
        let overrides = PricingOverrides::default().with_quoted_ytm(0.05);
        let bond = build_test_bond(overrides);
        let market = flat_discount_market(0.03);
        let pv = bond.value(&market, date!(2025 - 01 - 01)).expect("value");
        // `price_from_ytm` uses Street convention with the bond's day-count,
        // which differs from exact discrete compounding by a few basis points
        // for semi-annual Act/365F bonds; allow up to 5 bp of notional.
        assert!(
            (pv.amount() - 1_000_000.0).abs() < 500.0,
            "expected ~par, got {}",
            pv.amount()
        );
    }

    #[test]
    fn bond_value_honors_quoted_z_spread_override() {
        // A zero Z-spread over the same flat discount curve reproduces the
        // discount-engine PV (within f64 noise).
        let market = flat_discount_market(0.04);
        let base_pv = build_test_bond(PricingOverrides::default())
            .value(&market, date!(2025 - 01 - 01))
            .expect("base");
        let zspread_pv = build_test_bond(PricingOverrides::default().with_quoted_z_spread(0.0))
            .value(&market, date!(2025 - 01 - 01))
            .expect("zspread");
        assert!((base_pv.amount() - zspread_pv.amount()).abs() < 1.0);
    }

    #[test]
    fn market_quote_overrides_reject_mutually_exclusive_price_drivers() {
        let overrides = PricingOverrides::default()
            .with_quoted_clean_price(98.5)
            .with_quoted_ytm(0.05);
        assert!(
            overrides.validate().is_err(),
            "validate must reject more than one price driver"
        );
    }

    #[test]
    fn market_quote_overrides_accept_single_price_driver() {
        for overrides in [
            PricingOverrides::default().with_quoted_clean_price(98.5),
            PricingOverrides::default().with_quoted_dirty_price(987_654.32),
            PricingOverrides::default().with_quoted_ytm(0.05),
            PricingOverrides::default().with_quoted_ytw(0.05),
            PricingOverrides::default().with_quoted_z_spread(0.0125),
            PricingOverrides::default().with_quoted_oas(0.0100),
            PricingOverrides::default().with_quoted_discount_margin(0.008),
            PricingOverrides::default().with_quoted_i_spread(0.004),
            PricingOverrides::default().with_quoted_asw_market(0.005),
        ] {
            assert!(overrides.validate().is_ok());
        }
    }

    #[test]
    fn bond_value_applies_scenario_price_shock_exactly_once() {
        let market = flat_discount_market(0.04);
        let as_of = date!(2025 - 01 - 01);

        let baseline = build_test_bond(PricingOverrides::default())
            .value(&market, as_of)
            .expect("baseline");
        let shocked = build_test_bond(PricingOverrides::default().with_price_shock_pct(-0.10))
            .value(&market, as_of)
            .expect("shocked");

        let expected = baseline.amount() * 0.9;
        assert!(
            (shocked.amount() - expected).abs() < 1e-6,
            "shocked ({}) should equal 0.9 * baseline ({})",
            shocked.amount(),
            expected,
        );
    }

    #[test]
    fn bond_value_matches_price_with_metrics_value() {
        // The contract between `Instrument::value` and `price_with_metrics` is that
        // the returned `value` field must match `value()` for identical overrides.
        let as_of = date!(2025 - 01 - 01);
        let market = flat_discount_market(0.04);

        for overrides in [
            PricingOverrides::default(),
            PricingOverrides::default().with_quoted_clean_price(98.5),
            PricingOverrides::default().with_price_shock_pct(-0.10),
        ] {
            let bond = build_test_bond(overrides.clone());
            let direct = bond.value(&market, as_of).expect("value").amount();
            let via_metrics = bond
                .price_with_metrics(
                    &market,
                    as_of,
                    &[],
                    crate::instruments::PricingOptions::default(),
                )
                .expect("price_with_metrics")
                .value
                .amount();
            assert!(
                (direct - via_metrics).abs() < 1e-6,
                "value() {} should equal price_with_metrics().value {} for overrides {:?}",
                direct,
                via_metrics,
                overrides
            );
        }
    }

    #[test]
    fn cds_quote_bp_accepts_legacy_quoted_spread_bp_json() {
        use crate::instruments::pricing_overrides::MarketQuoteOverrides;
        let json = r#"{"quoted_spread_bp": 150.0}"#;
        let parsed: MarketQuoteOverrides =
            serde_json::from_str(json).expect("legacy alias should deserialize");
        assert_eq!(parsed.cds_quote_bp, Some(150.0));
    }
}
