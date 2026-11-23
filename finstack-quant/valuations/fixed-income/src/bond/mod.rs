//! Fixed and floating-rate bond instruments with embedded options.
//!
//! bond modeling including:
//! - Fixed-rate coupon bonds (bullet and amortizing)
//! - Floating-rate notes (FRNs) with caps/floors
//! - Callable and putable bonds (American/Bermudan exercise)
//! - Minimum-MOIC and minimum-XIRR return-floor rights
//! - Zero-coupon bonds
//! - Custom cashflow schedules (including PIK and bespoke amortization)
//!
//! # Signed Canonical Schedule
//!
//! All bond cashflows in this module follow a **signed canonical schedule** convention:
//! - **Positive amounts** represent contractual inflows to a long holder
//!   (coupons, amortization, redemption).
//! - **Initial draw / funding legs** are handled outside the schedule
//!   (e.g., via trade price) and are **not** included in the projected
//!   cashflow schedule.
//!
//! This convention is enforced by the bond's `CashflowProvider::dated_cashflows` implementation, which turns the
//! internal cashflow schedule into a simplified `(Date, Money)` stream used
//! by pricing and risk engines.
//!
//! # Bond Pricing
//!
//! Bonds are priced by discounting all future cashflows from the signed canonical schedule:
//!
//! ```text
//! PV = Σ CF_i · DF(as_of → t_i)
//! ```
//!
//! For bonds with embedded options (calls, puts, or return floors), model-based
//! rollback values the optionality. The short-rate tree operates on the
//! discount curve's day-count axis; the rates-credit lattice uses Act/365F.
//! Cashflows, balances, and exercise dates are mapped with the same day count
//! as their model grid, so that:
//! - `t = 0` corresponds to the valuation date `as_of`
//! - `t > 0` are year-fractions to future cashflow and exercise dates
//!
//! **Important**: PV is always anchored at `as_of` (the valuation date), not the
//! settlement date. This is the instrument's theoretical value on the valuation
//! date. Settlement-date pricing (for trade execution) is a separate concern.
//!
//! # Quote-Date Convention for Yield Metrics
//!
//! While PV is anchored at `as_of`, market-derived metrics (YTM, Z-spread, DM,
//! OAS, duration, convexity) are computed from the **quote date** (settlement
//! date) because market quotes reflect settlement-date pricing:
//!
//! - **quote_date** = `as_of + settlement_days` (or `as_of` if no settlement convention)
//! - **accrued_at_quote_date** = accrued interest computed at quote_date
//! - **dirty_price** = clean_price * notional / 100 + accrued_at_quote_date
//!
//! This separation ensures that:
//! 1. Curve discounting always uses `as_of` as the anchor
//! 2. Quote-derived metrics properly interpret market prices as settlement quotes
//! 3. The YTM/duration/convexity numbers match market standard conventions
//!
//! # Call/Put Exercise Convention
//!
//! For bonds with call, put, or return-floor rights:
//!
//! - **`CallPut.price_pct_of_par`** is applied to the **outstanding principal**
//!   at the exercise date, not the original notional. This correctly handles
//!   amortizing callable bonds.
//! - **Exercise payoff**: Current contractual cash is paid regardless of the
//!   exercise decision. The decision applies to risky continuation, including
//!   within-step fractional-recovery-of-par value.
//! - **Formula**: `node_value = current_cash + min(max(risky_continuation, put_price), call_price)`.
//! - At maturity, exercise compares against pre-redemption principal. Current
//!   coupon and non-redemption payments are added once; scheduled redemption is
//!   the hold alternative.
//!
//! # Accrual and Ex-Coupon Conventions
//!
//! Accrued interest is driven directly off the true coupon schedule and
//! outstanding notional (for amortizing structures), with explicit support
//! for:
//! - Linear vs. compounded accrual (`AccrualMethod`)
//! - Ex-coupon windows where accrual drops to zero
//! - Custom-cashflow bonds that provide their own schedule and day-count
//!
//! # Regional Market Conventions
//!
//! Different bond markets follow distinct conventions:
//!
//! - **US Treasuries**: ACT/ACT ICMA, Semi-annual, T+1 settlement
//! - **UK Gilts**: ACT/ACT, Semi-annual, T+1 settlement
//! - **Eurozone**: 30E/360 or ACT/ACT, Annual, T+2 settlement
//! - **Japan**: ACT/365F, Semi-annual, T+2 settlement (cross-border)
//!
//! Use `Bond::with_convention()` for standard regional conventions.
//!
//! # Key Metrics
//!
//! - **Yield to Maturity (YTM)**: Internal rate of return
//! - **Modified Duration**: Interest rate sensitivity
//! - **Convexity**: Curvature of price/yield relationship
//! - **DV01**: Dollar value of 1bp rate change
//! - **Z-spread**: Spread over benchmark curve
//! - **Accrued Interest**: Interest accrued since last coupon
//!
//! # Examples
//!
//! See [`Bond`](crate::bond::Bond) for construction examples.
//!
//! # See Also
//!
//! - [`Bond`](crate::bond::Bond) for the main bond struct and factory methods
//! - `CallPutSchedule` for embedded option schedules
//! - `CashflowSpec` for fixed/floating/amortizing specifications
//! - `AmortizationSpec` for amortizing bonds
//! - bond metrics module for bond-specific risk metrics

pub(crate) mod bond_quote;
mod cashflow_spec;
pub(crate) mod conventions;
pub(crate) mod merton_mc_config;
pub(crate) mod metrics;
/// Bond pricing engines including tree-based and analytical methods.
pub mod pricing;
mod types;

pub use cashflow_spec::CashflowSpec;
#[doc(hidden)]
pub use metrics::price_yield_spread::asw::{asw_market_with_forward, asw_par_with_forward};
#[doc(hidden)]
pub use metrics::price_yield_spread::dm::DiscountMarginCalculator;
#[doc(hidden)]
pub use metrics::price_yield_spread::z_spread::ZSpreadCalculator;
pub use types::construction::FloatingBondParams;
pub use types::definitions::Bond;
pub use types::definitions::BondBuilder;
pub use types::definitions::BondSettlementConvention;
pub use types::definitions::CallPut;
pub use types::definitions::CallPutSchedule;
pub use types::definitions::MakeWholeSpec;
pub use types::return_floor::IssuePrice;
pub use types::return_floor::ProtectionWindow;
pub use types::return_floor::ReturnFloorKind;
pub use types::return_floor::ReturnFloorSpec;

#[cfg(test)]
mod tests;
