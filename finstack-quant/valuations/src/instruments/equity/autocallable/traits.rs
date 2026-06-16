//! Trait implementations for Autocallable

use crate::instruments::equity::autocallable::Autocallable;

crate::impl_equity_exotic_traits!(Autocallable, curve_deps: true);
