//! WAL (Weighted Average Life) calculator for structured credit.

use crate::instruments::fixed_income::structured_credit::types::TrancheCashflows;
use crate::metrics::{MetricCalculator, MetricContext};
use finstack_core::cashflow::CFKind;
use finstack_core::dates::Date;
use finstack_core::Result;

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
pub fn calculate_tranche_wal(cashflows: &TrancheCashflows, as_of: Date) -> Result<f64> {
    let mut weighted_sum = 0.0;
    let mut total_principal = 0.0;

    for (date, amount) in &cashflows.principal_flows {
        if *date <= as_of {
            continue;
        }

        let years = finstack_core::dates::DayCount::Act365F
            .year_fraction(
                as_of,
                *date,
                finstack_core::dates::DayCountContext::default(),
            )
            .unwrap_or(0.0);
        weighted_sum += amount.amount() * years;
        total_principal += amount.amount();
    }

    if total_principal > 0.0 {
        Ok(weighted_sum / total_principal)
    } else {
        Ok(0.0)
    }
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
            let mut weighted_sum = 0.0;
            let mut total_principal = 0.0;

            for flow in flows {
                if flow.date <= context.as_of {
                    continue;
                }
                // Cash principal only — Amortization / Notional / PrePayment.
                // Write-downs (`DefaultedNotional`) are losses, not principal
                // repayments, and are excluded so this fallback matches the
                // primary `principal_flows` path (which never contains
                // write-downs).
                if !matches!(
                    flow.kind,
                    CFKind::Amortization | CFKind::Notional | CFKind::PrePayment
                ) {
                    continue;
                }

                let principal = flow.amount.amount().abs();
                if principal <= 0.0 {
                    continue;
                }

                let years = finstack_core::dates::DayCount::Act365F
                    .year_fraction(
                        context.as_of,
                        flow.date,
                        finstack_core::dates::DayCountContext::default(),
                    )
                    .unwrap_or(0.0);
                weighted_sum += principal * years;
                total_principal += principal;
            }

            return if total_principal > 0.0 {
                Ok(weighted_sum / total_principal)
            } else {
                Ok(0.0)
            };
        }

        // Final fallback: use aggregate positive flows only.
        // This path is less accurate because interest and principal are not distinguished.
        if let Some(flows) = context.cashflows.as_ref() {
            let mut weighted_sum = 0.0;
            let mut total_principal = 0.0;

            for (date, amount) in flows {
                if *date <= context.as_of {
                    continue;
                }

                // Only use positive flows as an approximation of principal.
                // Negative flows (if any) are ignored; interest cannot be
                // distinguished from principal in aggregated mode.
                let principal = amount.amount();
                if principal <= 0.0 {
                    continue;
                }

                let years = finstack_core::dates::DayCount::Act365F
                    .year_fraction(
                        context.as_of,
                        *date,
                        finstack_core::dates::DayCountContext::default(),
                    )
                    .unwrap_or(0.0);
                weighted_sum += principal * years;
                total_principal += principal;
            }

            return if total_principal > 0.0 {
                Ok(weighted_sum / total_principal)
            } else {
                Ok(0.0)
            };
        }

        Err(finstack_core::Error::from(
            finstack_core::InputError::NotFound {
                id: "detailed_tranche_cashflows".to_string(),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instruments::common_impl::traits::{Attributes, Instrument};
    use crate::pricer::InstrumentType;
    use finstack_core::cashflow::{CFKind, CashFlow};
    use finstack_core::currency::Currency;
    use finstack_core::market_data::context::MarketContext;
    use finstack_core::money::Money;
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

        fn base_value(&self, _ctx: &MarketContext, _as_of: Date) -> finstack_core::Result<Money> {
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
            CashFlow {
                date: Date::from_calendar_date(2026, Month::January, 1).expect("valid date"),
                reset_date: None,
                amount: Money::new(10.0, Currency::USD),
                kind: CFKind::Fixed,
                accrual_factor: 0.0,
                rate: None,
            },
            CashFlow {
                date: Date::from_calendar_date(2026, Month::January, 1).expect("valid date"),
                reset_date: None,
                amount: Money::new(100.0, Currency::USD),
                kind: CFKind::Amortization,
                accrual_factor: 0.0,
                rate: None,
            },
            CashFlow {
                date: Date::from_calendar_date(2027, Month::January, 1).expect("valid date"),
                reset_date: None,
                amount: Money::new(50.0, Currency::USD),
                kind: CFKind::PrePayment,
                accrual_factor: 0.0,
                rate: None,
            },
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
            CashFlow {
                date: Date::from_calendar_date(2026, Month::January, 1).expect("valid date"),
                reset_date: None,
                amount: Money::new(100.0, Currency::USD),
                kind: CFKind::Amortization,
                accrual_factor: 0.0,
                rate: None,
            },
            // A large write-down much later: if counted as principal it would
            // drag WAL far above 1y.
            CashFlow {
                date: Date::from_calendar_date(2030, Month::January, 1).expect("valid date"),
                reset_date: None,
                amount: Money::new(-500.0, Currency::USD),
                kind: CFKind::DefaultedNotional,
                accrual_factor: 0.0,
                rate: None,
            },
        ]);

        let wal = WalCalculator.calculate(&mut context).expect("wal");
        assert!(
            (wal - 1.0).abs() < 1e-2,
            "wal={wal}: write-down must not be weighted as principal"
        );
    }
}
