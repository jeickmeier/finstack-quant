/// Implements the standard boilerplate methods for the [`Instrument`](super::Instrument) trait.
///
/// Most instruments store their ID as `self.id` (an `InstrumentId`), attributes as
/// `self.attributes`, and have a fixed [`InstrumentType`](crate::pricer::InstrumentType) key.
/// This macro provides default implementations for the mechanical methods, leaving only the
/// instrument-specific methods (`value`, `market_dependencies`, etc.) to be
/// implemented manually.
///
/// # Requirements
///
/// The implementing type must:
/// - Have a field `id: InstrumentId` (with `.as_str()` method)
/// - Have a field `attributes: Attributes`
/// - Implement `Clone`
///
/// # Example
///
/// ```ignore
/// impl Instrument for MyInstrument {
///     impl_instrument_base!(InstrumentType::MyInstrument);
///
///     fn base_value(&self, market: &MarketContext, as_of: Date) -> Result<Money> {
///         // instrument-specific pricing logic
///     }
/// }
/// ```
#[macro_export]
macro_rules! impl_instrument_base {
    ($key:expr) => {
        fn id(&self) -> &str {
            self.id.as_str()
        }

        fn key(&self) -> $crate::pricer::InstrumentType {
            $key
        }

        fn as_any(&self) -> &dyn ::std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
            self
        }

        fn attributes(&self) -> &$crate::instruments::common_impl::traits::Attributes {
            &self.attributes
        }

        fn attributes_mut(&mut self) -> &mut $crate::instruments::common_impl::traits::Attributes {
            &mut self.attributes
        }

        fn clone_box(&self) -> Box<dyn $crate::instruments::Instrument> {
            Box::new(self.clone())
        }
    };
}

/// Implement a standard empty-schedule `CashflowProvider` for products whose
/// waterfall policy is intentionally empty for now.
#[macro_export]
macro_rules! impl_empty_cashflow_provider {
    ($ty:ty, $representation:expr) => {
        $crate::impl_empty_cashflow_provider!(
            $ty,
            $representation,
            None,
            finstack_quant_core::dates::DayCount::Act365F
        );
    };
    ($ty:ty, $representation:expr, $notional:expr, $day_count:expr) => {
        impl $crate::__private::finstack_quant_cashflows::traits::CashflowScheduleSource for $ty {
            fn notional(&self) -> Option<finstack_quant_core::money::Money> {
                $notional
            }

            fn raw_cashflow_schedule(
                &self,
                _market: &finstack_quant_core::market_data::context::MarketContext,
                _as_of: finstack_quant_core::dates::Date,
            ) -> finstack_quant_core::Result<
                $crate::__private::finstack_quant_cashflows::builder::CashFlowSchedule,
            > {
                Ok(
                    $crate::__private::finstack_quant_cashflows::traits::schedule_from_classified_flows(
                        Vec::new(),
                        $day_count,
                        $crate::__private::finstack_quant_cashflows::traits::ScheduleBuildOpts {
                            notional_hint: self.notional(),
                            meta: $crate::__private::finstack_quant_cashflows::builder::CashFlowMeta {
                                representation: $representation,
                                ..Default::default()
                },
            },
                    ),
                )
            }
        }
    };
}
