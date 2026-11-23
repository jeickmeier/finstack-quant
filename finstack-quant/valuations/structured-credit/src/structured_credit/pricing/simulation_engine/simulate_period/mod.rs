//! One payment period of the deal simulation, in phases: pool collections
//! and the recovery queue ([`cash`]), loss allocation and loss-driven tests
//! ([`losses`]), the side accounts that set the waterfall cash ([`cash`]),
//! the period's resolved waterfall ([`period_waterfall`]), execution, and
//! recording the payments per tranche ([`record`]).

use super::*;
use crate::structured_credit::types::{PaymentCalculation, PaymentType};

mod cash;
mod losses;
mod period_waterfall;
mod record;
mod step;
#[cfg(test)]
mod tests;

use step::PeriodInputs;
pub(super) use step::simulate_period;
#[cfg(test)]
use step::*;
