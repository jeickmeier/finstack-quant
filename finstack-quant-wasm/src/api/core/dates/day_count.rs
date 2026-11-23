//! Day-count convention WASM handle.

use super::context::JsDayCountContext;
use crate::utils::input::{js_epoch_days, js_string};
use crate::utils::to_js_err;
use finstack_quant_core::dates::DayCount as RustDayCount;
use wasm_bindgen::prelude::*;

/// Day-count convention for computing year fractions and day counts.
///
/// Dates are represented as **epoch days** (`i32`, days since 1970-01-01).
/// Use `createDate` to convert from a `(year, month, day)` triple.
///
/// Available conventions (canonical names for `DayCount.fromName`) and their factories:
/// - `one_one` → `DayCount.oneOne`
/// - `act_360` → `DayCount.act360`
/// - `act_365f` → `DayCount.act365f`
/// - `act_365l` → `DayCount.act365l`
/// - `nl_365` (No-Leap/365) → `DayCount.nl365`
/// - `30_360` → `DayCount.thirty360`
/// - `30_360_isda` (bond basis) → `DayCount.thirty360Isda`
/// - `30e_360` → `DayCount.thirtyE360`
/// - `30e_360_isda` → `DayCount.thirtyE360Isda`
/// - `act_act` (ISDA) → `DayCount.actAct`
/// - `act_act_isma` (ICMA) → `DayCount.actActIsma`
/// - `act_act_afb` (AFB / Actual/Actual Euro) → `DayCount.actActAfb`
/// - `30_360_it` (Italian) → `DayCount.thirty360It`
/// - `bus_252` → `DayCount.bus252`
///
/// `fromName` accepts these canonical names after trimming surrounding
/// whitespace and normalizing ASCII case. Other spellings are rejected.
///
/// @example
/// ```javascript
/// import init, { core } from "finstack-quant-wasm";
/// await init();
/// const day_count = core.DayCount.act365f();
/// const start = core.createDate(2025, 1, 15);
/// const end   = core.createDate(2025, 7, 15);
/// const yf    = day_count.yearFraction(start, end);
/// // yf ≈ 0.4959 (181 / 365)
/// ```
#[wasm_bindgen(js_name = DayCount)]
pub struct JsDayCount {
    pub(crate) inner: RustDayCount,
}

#[wasm_bindgen(js_class = DayCount)]
impl JsDayCount {
    /// Look up a convention by its canonical name (Rust `DayCount::from_str`,
    /// the twin of Python `DayCount.from_name`).
    ///
    /// # Arguments
    ///
    /// * `name` - Canonical snake_case convention name, for example
    ///   `"act_360"`, `"30_360"` or `"act_act"` (see the class list).
    ///
    /// @returns The matching `DayCount`.
    /// @throws `TypeError` if `name` is not a string; `FinstackError` (kind
    /// `validation`) if no convention matches. Canonical names ignore surrounding
    /// whitespace and ASCII case; other spellings are rejected.
    /// @param name - Canonical snake_case convention name, for example `"act_360"`, `"30_360"` or `"act_act"` (see the class list).
    #[wasm_bindgen(js_name = fromName)]
    pub fn from_name(
        #[wasm_bindgen(unchecked_param_type = "string")] name: JsValue,
    ) -> Result<JsDayCount, JsValue> {
        let name: &str = &js_string(&name, "name")?;
        name.parse::<RustDayCount>()
            .map(|inner| JsDayCount { inner })
            .map_err(to_js_err)
    }

    /// No-Leap/365: actual days excluding February 29, over 365.
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = nl365)]
    pub fn nl365() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Nl365,
        }
    }

    /// One unit per nonempty contractual accrual period; empty periods return zero.
    /// @returns The 1/1 convention used for annual inflation accrual periods.
    /// @throws This constructor does not throw.
    #[wasm_bindgen(js_name = oneOne)]
    pub fn one_one() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::OneOne,
        }
    }

    /// Actual/360 day count: actual days divided by a 360-day year.
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = act360)]
    pub fn act360() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Act360,
        }
    }

    /// Actual/365 Fixed.
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = act365f)]
    pub fn act365f() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Act365F,
        }
    }

    /// Actual/365L (ISDA coupon-end-year rule). Requires a `DayCountContext`
    /// with the enclosing coupon period; no frequency is used. The
    /// denominator is 366 exactly when the coupon end date's year is a leap
    /// year, otherwise 365, for every coupon frequency. This is not ACT/ACT
    /// AFB.
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = act365l)]
    pub fn act365l() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Act365L,
        }
    }

    /// Fixed US 30U/360 EOM (QuantLib `Thirty360::USA`); not 30/360 PSA.
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = thirty360)]
    pub fn thirty360() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Thirty360,
        }
    }

    /// 30/360 ISDA bond basis (2006 ISDA Definitions, Section 4.16(f); QuantLib
    /// `Thirty360::BondBasis`), with no February rule.
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = thirty360Isda)]
    pub fn thirty360_isda() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Thirty360Isda,
        }
    }

    /// 30E/360 (Eurobond Basis).
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = thirtyE360)]
    pub fn thirty_e360() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ThirtyE360,
        }
    }

    /// 30E/360 ISDA day count (2006 ISDA Definitions, Section 4.16(h)). Requires
    /// a context with an explicit `endIsTerminationDate`.
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = thirtyE360Isda)]
    pub fn thirty_e360_isda() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ThirtyE360Isda,
        }
    }

    /// Actual/Actual (ISDA).
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = actAct)]
    pub fn act_act() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ActAct,
        }
    }

    /// Actual/Actual (ICMA/ISMA).
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = actActIsma)]
    pub fn act_act_isma() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ActActIsma,
        }
    }

    /// Actual/Actual AFB (Actual/Actual Euro).
    ///
    /// Walks whole years backwards from the end date (QuantLib
    /// `ActualActual::AFB`). A year-step landing on 28 February of a leap
    /// year is bumped to 29 February. The residual uses denominator 366 if
    /// 29 February lies in `[start, residual_end)`, else 365.
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = actActAfb)]
    pub fn act_act_afb() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::ActActAfb,
        }
    }

    /// 30/360 Italian day count, with the February end-of-month rule below.
    ///
    /// Day 31 becomes 30, and any February day after the 27th becomes 30
    /// (QuantLib `Thirty360::Italian`). Distinct from US SIA and 30E/360.
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = thirty360It)]
    pub fn thirty360_it() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Thirty360It,
        }
    }

    /// Business/252 day count: business days divided by 252 (Brazilian market convention).
    /// @returns A `DayCount` handle for this convention.
    #[wasm_bindgen(js_name = bus252)]
    pub fn bus252() -> JsDayCount {
        JsDayCount {
            inner: RustDayCount::Bus252,
        }
    }

    /// Compute the year fraction between two dates given as epoch days
    /// (Rust `DayCount::year_fraction(start, end, ctx)`).
    ///
    /// The published facade makes `ctx` optional: an omitted context is the
    /// empty Rust default (`new DayCountContext()`). Act/Act ISMA needs both a
    /// context frequency (`1M`, `2M`, `3M`, `4M`, `6M`, `12M` or `1Y`) and the
    /// enclosing unadjusted nominal `couponPeriod`, Act/365L needs an enclosing
    /// `couponPeriod` (accrual dates outside that coupon throw for both), and
    /// Bus/252 needs a context calendar; all throw without them. ICMA reference
    /// endpoints outside one unadjusted nominal month grid throw.
    ///
    /// @param startEpochDays - Start date as days since 1970-01-01.
    /// @param endEpochDays - End date as days since 1970-01-01; must not be
    /// before the start.
    /// @param ctx - DayCountContext supplying calendar, frequency, coupon-period
    /// and termination metadata; omitted means the empty default context.
    /// @returns Non-negative year fraction in years under the convention and context.
    /// @throws `TypeError` (kind `invalid_type`) if a date is not an integer
    /// epoch-day number; `FinstackError` (kind `validation`) if a date is out
    /// of range, the start is after the end, or the convention's required
    /// context is missing or invalid; kind `not_found` if the context names an
    /// unknown calendar.
    ///
    /// @example
    /// ```javascript
    /// const day_count = core.DayCount.act360();
    /// const start = core.createDate(2025, 1, 15);
    /// const end   = core.createDate(2025, 4, 15);
    /// day_count.yearFraction(start, end); // 90 / 360 = 0.25
    /// ```
    ///
    /// @example
    /// ```typescript
    /// const dayCount = core.DayCount.act360();
    /// const start = core.createDate(2025, 1, 15);
    /// const end   = core.createDate(2025, 4, 15);
    /// dayCount.yearFraction(start, end); // 90 / 360 = 0.25
    /// const bus = new core.DayCountContext("nyse");
    /// core.DayCount.bus252().yearFraction(start, end, bus);
    /// ```
    #[wasm_bindgen(js_name = yearFraction)]
    pub fn year_fraction(
        &self,
        #[wasm_bindgen(js_name = "startEpochDays", unchecked_param_type = "number")]
        start_epoch_days: JsValue,
        #[wasm_bindgen(js_name = "endEpochDays", unchecked_param_type = "number")]
        end_epoch_days: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "DayCountContext")] ctx: &JsDayCountContext,
    ) -> Result<f64, JsValue> {
        let start = js_epoch_days(&start_epoch_days, "startEpochDays")?;
        let end = js_epoch_days(&end_epoch_days, "endEpochDays")?;
        self.inner
            .year_fraction(start, end, ctx.inner.to_ctx().map_err(to_js_err)?)
            .map_err(to_js_err)
    }

    /// Compute a signed year fraction, preserving the start/end orientation
    /// (Rust `DayCount::signed_year_fraction(start, end, ctx)`).
    ///
    /// The published facade makes `ctx` optional, as for `yearFraction`.
    ///
    /// @param startEpochDays - Start date as days since 1970-01-01.
    /// @param endEpochDays - End date as days since 1970-01-01; may precede the start.
    /// @param ctx - DayCountContext supplying calendar, frequency, coupon-period
    /// and termination metadata; omitted means the empty default context.
    /// @returns Signed year fraction in years; negative when `end` is before `start`.
    ///
    /// # Errors
    ///
    /// Throws `TypeError` (kind `invalid_type`) if a date is not an integer
    /// epoch-day number; `FinstackError` (kind `validation`) if a date is out of
    /// range or the convention's required context is missing or invalid; kind
    /// `not_found` if the context names an unknown calendar.
    #[wasm_bindgen(js_name = signedYearFraction)]
    pub fn signed_year_fraction(
        &self,
        #[wasm_bindgen(js_name = "startEpochDays", unchecked_param_type = "number")]
        start_epoch_days: JsValue,
        #[wasm_bindgen(js_name = "endEpochDays", unchecked_param_type = "number")]
        end_epoch_days: JsValue,
        #[wasm_bindgen(unchecked_optional_param_type = "DayCountContext")] ctx: &JsDayCountContext,
    ) -> Result<f64, JsValue> {
        let start = js_epoch_days(&start_epoch_days, "startEpochDays")?;
        let end = js_epoch_days(&end_epoch_days, "endEpochDays")?;
        self.inner
            .signed_year_fraction(start, end, ctx.inner.to_ctx().map_err(to_js_err)?)
            .map_err(to_js_err)
    }

    /// Convention name.
    /// @returns Human-readable string form of this value.
    #[wasm_bindgen(js_name = toString)]
    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> String {
        self.inner.to_string()
    }
}
