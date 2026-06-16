use crate::instruments::common_impl::GenericInstrumentPricer;

/// IR Future Option pricer using the generic discounting implementation.
pub(crate) type IrFutureOptionPricer =
    GenericInstrumentPricer<crate::instruments::rates::ir_future_option::IrFutureOption>;

impl Default for IrFutureOptionPricer {
    fn default() -> Self {
        Self::discounting(crate::pricer::InstrumentType::IrFutureOption)
    }
}
