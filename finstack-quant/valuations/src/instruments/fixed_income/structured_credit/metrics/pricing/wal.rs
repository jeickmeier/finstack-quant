//! WAL (Weighted Average Life) calculator for structured credit.

use crate::cashflow::builder::schedule::weighted_average_life_from_principal;
use crate::instruments::fixed_income::structured_credit::types::TrancheCashflows;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_quant_core::cashflow::CFKind;
use finstack_quant_core::dates::Date;
use finstack_quant_core::Result;

/// Calculate tranche-specific WAL from a `TrancheCashflows`.
///
/// WAL measures the average time until principal is repaid, weighted by the
/// amount of principal. This is a critical metric for structured credit as it
/// captures the impact of prepayments, amortization, and defaults.
///
/// # Formula
///
/// WAL = Σ(Principal_i × Time_i) / Σ(Principal_i)
///
/// Where:
/// - Principal_i = principal payment at time i
/// - Time_i = years from valuation date to payment date i
///
/// # Arguments
///
/// * `cashflows` - Projected cashflows for one tranche; only principal flows
///   contribute to the balance-weighted repayment horizon.
/// * `as_of` - Valuation date from which each principal payment time is
///   measured in years.
pub fn calculate_tranche_wal(cashflows: &TrancheCashflows, as_of: Date) -> Result<f64> {
    weighted_average_life_from_principal(cashflows.principal_flows.iter().copied(), as_of)
}

/// Calculates WAL (Weighted Average Life) in years.
///
/// WAL measures the average time until principal is repaid, weighted by the
/// amount of principal. This is a critical metric for structured credit as it
/// captures the impact of prepayments, amortization, and defaults.
///
/// # Formula
///
/// WAL = Σ(Principal_i × Time_i) / Σ(Principal_i)
///
/// Where:
/// - Principal_i = principal payment at time i
/// - Time_i = years from valuation date to payment date i
///
/// # Market Conventions
///
/// - **CLO**: Typically 3-5 years
/// - **ABS**: Typically 2-4 years (varies with prepayment assumptions)
/// - **RMBS**: Typically 3-7 years (highly sensitive to PSA speed)
/// - **CMBS**: Typically 4-8 years
///
pub struct WalCalculator;

impl MetricCalculator for WalCalculator {
    fn calculate(&self, context: &mut MetricContext) -> Result<f64> {
        if let Some(details) = context.detailed_tranche_cashflows.as_ref() {
            return calculate_tranche_wal(details, context.as_of);
        }

        // Fallback: derive WAL from tagged cashflows when detailed tranche-level
        // cashflows are not cached into the metric context.
        if let Some(flows) = context.tagged_cashflows.as_ref() {
            return weighted_average_life_from_principal(
                flows
                    .iter()
                    .filter(|&flow| {
                        matches!(
                            flow.kind,
                            CFKind::Amortization | CFKind::Notional | CFKind::PrePayment
                        )
                    })
                    .map(|flow| {
                        (
                            flow.date,
                            finstack_quant_core::money::Money::new(
                                flow.amount.amount().abs(),
                                flow.amount.currency(),
                            ),
                        )
                    }),
                context.as_of,
            );
        }

        // Final fallback: use aggregate positive flows only.
        // This path is less accurate because interest and principal are not distinguished.
        if let Some(flows) = context.cashflows.as_ref() {
            return weighted_average_life_from_principal(flows.iter().copied(), context.as_of);
        }

        Err(finstack_quant_core::Error::from(
            finstack_quant_core::InputError::NotFound {
                id: "detailed_tranche_cashflows".to_string(),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cashflow::builder::{CashFlowMeta, CashFlowSchedule, Notional};
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::pricer::InstrumentType;
    use finstack_quant_core::cashflow::{CFKind, CashFlow};
    use finstack_quant_core::currency::Currency;
    use finstack_quant_core::dates::DayCount;
    use finstack_quant_core::market_data::context::MarketContext;
    use finstack_quant_core::money::Money;
    use std::sync::Arc;
    use time::Month;

    #[derive(Clone, Debug)]
    struct DummyInstrument {
        attrs: Attributes,
    }

    crate::impl_empty_cashflow_provider!(
        DummyInstrument,
        crate::cashflow::builder::CashflowRepresentation::NoResidual
    );

    impl Instrument for DummyInstrument {
        fn id(&self) -> &str {
            "dummy"
        }

        fn key(&self) -> InstrumentType {
            InstrumentType::StructuredCredit
        }

        fn base_value(
            &self,
            _ctx: &MarketContext,
            _as_of: Date,
        ) -> finstack_quant_core::Result<Money> {
            Ok(Money::new(0.0, Currency::USD))
        }

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn attributes(&self) -> &Attributes {
            &self.attrs
        }

        fn attributes_mut(&mut self) -> &mut Attributes {
            &mut self.attrs
        }

        fn clone_box(&self) -> Box<dyn Instrument> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn wal_uses_tagged_principal_flows_only() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let instrument = Arc::new(DummyInstrument {
            attrs: Attributes::new(),
        }) as Arc<dyn Instrument>;
        let curves = Arc::new(MarketContext::new());
        let base_value = Money::new(0.0, Currency::USD);

        let mut context = MetricContext::new(
            instrument,
            curves,
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        context.tagged_cashflows = Some(vec![
            CashFlow::new(
                Date::from_calendar_date(2026, Month::January, 1).expect("valid date"),
                None,
                Money::new(10.0, Currency::USD),
                CFKind::Fixed,
                0.0,
                None,
            ),
            CashFlow::new(
                Date::from_calendar_date(2026, Month::January, 1).expect("valid date"),
                None,
                Money::new(100.0, Currency::USD),
                CFKind::Amortization,
                0.0,
                None,
            ),
            CashFlow::new(
                Date::from_calendar_date(2027, Month::January, 1).expect("valid date"),
                None,
                Money::new(50.0, Currency::USD),
                CFKind::PrePayment,
                0.0,
                None,
            ),
        ]);

        let wal = WalCalculator.calculate(&mut context).expect("wal");
        let expected = (100.0 * 1.0 + 50.0 * 2.0) / 150.0;
        assert!(
            (wal - expected).abs() < 1e-9,
            "wal={wal}, expected={expected}"
        );
    }

    /// Write-downs are losses, not principal repayments: the tagged-cashflow
    /// fallback must ignore `DefaultedNotional` flows so it matches the
    /// primary `principal_flows` path (which never contains write-downs).
    #[test]
    fn wal_fallback_excludes_writedowns() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let instrument = Arc::new(DummyInstrument {
            attrs: Attributes::new(),
        }) as Arc<dyn Instrument>;
        let curves = Arc::new(MarketContext::new());
        let base_value = Money::new(0.0, Currency::USD);

        let mut context = MetricContext::new(
            instrument,
            curves,
            as_of,
            base_value,
            MetricContext::default_config(),
        );
        context.tagged_cashflows = Some(vec![
            CashFlow::new(
                Date::from_calendar_date(2026, Month::January, 1).expect("valid date"),
                None,
                Money::new(100.0, Currency::USD),
                CFKind::Amortization,
                0.0,
                None,
            ),
            // A large write-down much later: if counted as principal it would
            // drag WAL far above 1y.
            CashFlow::new(
                Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
                None,
                Money::new(-500.0, Currency::USD),
                CFKind::DefaultedNotional,
                0.0,
                None,
            ),
        ]);

        let wal = WalCalculator.calculate(&mut context).expect("wal");
        assert!(
            (wal - 1.0).abs() < 1e-2,
            "wal={wal}: write-down must not be weighted as principal"
        );
    }

    #[test]
    fn tranche_and_schedule_wal_share_the_canonical_kernel() {
        let as_of = Date::from_calendar_date(2025, Month::January, 1).expect("valid date");
        let first = Date::from_calendar_date(2026, Month::January, 1).expect("valid date");
        let second = Date::from_calendar_date(2027, Month::January, 1).expect("valid date");
        let principal_flows = vec![
            (first, Money::new(40.0, Currency::USD)),
            (second, Money::new(60.0, Currency::USD)),
        ];
        let zero = Money::new(0.0, Currency::USD);
        let tranche = TrancheCashflows {
            tranche_id: "A".to_string(),
            cashflows: principal_flows.clone(),
            detailed_flows: Vec::new(),
            interest_flows: Vec::new(),
            principal_flows: principal_flows.clone(),
            pik_flows: Vec::new(),
            deferred_flows: Vec::new(),
            writedown_flows: Vec::new(),
            final_balance: zero,
            total_interest: zero,
            total_principal: Money::new(100.0, Currency::USD),
            total_pik: zero,
            total_deferred: zero,
            total_writedown: zero,
        };
        let schedule = CashFlowSchedule::from_parts(
            principal_flows
                .iter()
                .map(|(date, amount)| {
                    CashFlow::new(*date, None, *amount, CFKind::Amortization, 0.0, None)
                })
                .collect(),
            Notional::par(100.0, Currency::USD),
            DayCount::Thirty360,
            CashFlowMeta::default(),
        );

        let tranche_wal = calculate_tranche_wal(&tranche, as_of).expect("tranche WAL");
        let schedule_wal = schedule.weighted_average_life(as_of).expect("schedule WAL");

        assert_eq!(tranche_wal, schedule_wal);
    }
}
