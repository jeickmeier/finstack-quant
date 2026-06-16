//! Convenient re-exports of commonly used types.
//!
//! This module provides a single import point for the most frequently used types
//! in finstack-quant-core, reducing boilerplate in user code.
//!
//! # Example
//!
//! ```rust
//! use finstack_quant_core::prelude::*;
//!
//! let usd = Currency::USD;
//! let amount = Money::new(100.0, usd);
//! let date = create_date(2025, time::Month::January, 15)?;
//! # Ok::<(), finstack_quant_core::Error>(())
//! ```

pub use crate::currency::Currency;
pub use crate::money::{
    fx::{FxConversionPolicy, FxMatrix, FxProvider},
    Money,
};

pub use crate::dates::{
    adjust, create_date, BusinessDayConvention, Calendar, Date, DayCount, ScheduleBuilder, Tenor,
};

pub use crate::market_data::{
    context::MarketContext,
    scalars::MarketScalar,
    surfaces::VolSurface,
    term_structures::{
        BaseCorrelationCurve, DiscountCurve, ForwardCurve, HazardCurve, InflationCurve, PriceCurve,
        VolatilityIndexCurve,
    },
};

pub use crate::config::{FinstackConfig, RoundingMode};

pub use crate::types::{Bps, CurveId, InstrumentId, Percentage, Rate};

pub use crate::{Error, Result};
