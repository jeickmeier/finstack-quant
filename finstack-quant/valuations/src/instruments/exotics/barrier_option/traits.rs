//! Trait implementations for BarrierOption

use crate::instruments::exotics::barrier_option::BarrierOption;

crate::impl_equity_exotic_traits!(BarrierOption, curve_deps: true);
