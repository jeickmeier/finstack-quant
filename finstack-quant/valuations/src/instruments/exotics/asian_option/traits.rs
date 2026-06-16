//! Trait implementations for AsianOption

use crate::instruments::exotics::asian_option::AsianOption;

crate::impl_equity_exotic_traits!(AsianOption, curve_deps: true);
