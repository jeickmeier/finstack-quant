//! Trait implementations for CliquetOption

use crate::instruments::equity::cliquet_option::CliquetOption;
use crate::metrics::HasExpiry;

crate::instruments::common_impl::traits::impl_equity_exotic_traits!(@mc_daycount CliquetOption);

impl HasExpiry for CliquetOption {
    fn expiry(&self) -> finstack_quant_core::dates::Date {
        // Return the explicit expiry field. This stays consistent with
        // `Instrument::expiry` and never panics when `reset_dates` is empty
        // (e.g. corrupted state from a path that bypasses builder validation).
        self.expiry
    }
}
