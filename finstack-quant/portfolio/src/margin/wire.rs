//! JSON wire-format types for margin results.
//!
//! These `*Wire` structs provide a stable, deterministically-ordered
//! serialization representation for the core domain types in
//! [`super::results`]. They are an internal implementation detail: the public
//! `serde::Serialize`/`Deserialize` impls for [`NettingSetMargin`] and
//! [`PortfolioMarginResult`] delegate to the corresponding wire type so that
//! `HashMap`-backed fields serialize in a stable order. Nested SIMM sensitivities
//! use the margin crate's canonical [`SimmSensitivitiesJson`] tuple-array shape.

use finstack_quant_core::currency::Currency;
use finstack_quant_core::dates::Date;
use finstack_quant_core::money::Money;
use finstack_quant_core::HashMap;
use finstack_quant_margin::types::SimmSensitivitiesJson;
use finstack_quant_margin::{ImCollateralResult, ImMethodology, NettingSetId, SimmSensitivities};
use std::collections::BTreeMap;

use crate::types::PositionId;

use super::results::{NettingSetMargin, PortfolioMarginResult};

const MARGIN_WIRE_AMOUNT_TOLERANCE: f64 = 1e-9;

fn amounts_close(lhs: f64, rhs: f64) -> bool {
    (lhs - rhs).abs() <= MARGIN_WIRE_AMOUNT_TOLERANCE
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NettingSetMarginWire {
    netting_set_id: NettingSetId,
    csa_id: Option<String>,
    as_of: Date,
    initial_margin: Money,
    variation_margin: Money,
    total_margin: Money,
    position_count: usize,
    im_methodology: ImMethodology,
    is_approximate: bool,
    sensitivities: Option<SimmSensitivitiesJson>,
    im_breakdown: BTreeMap<String, Money>,
}

impl From<&NettingSetMargin> for NettingSetMarginWire {
    fn from(m: &NettingSetMargin) -> Self {
        Self {
            netting_set_id: m.netting_set_id.clone(),
            csa_id: m.csa_id.clone(),
            as_of: m.as_of,
            initial_margin: m.initial_margin,
            variation_margin: m.variation_margin,
            total_margin: m.total_margin,
            position_count: m.position_count,
            im_methodology: m.im_methodology,
            is_approximate: m.is_approximate,
            sensitivities: m.sensitivities.as_ref().map(SimmSensitivitiesJson::from),
            im_breakdown: m
                .im_breakdown
                .iter()
                .map(|(name, amount)| (name.clone(), *amount))
                .collect(),
        }
    }
}

impl From<NettingSetMarginWire> for NettingSetMargin {
    fn from(w: NettingSetMarginWire) -> Self {
        Self {
            netting_set_id: w.netting_set_id,
            csa_id: w.csa_id,
            as_of: w.as_of,
            initial_margin: w.initial_margin,
            variation_margin: w.variation_margin,
            total_margin: w.total_margin,
            position_count: w.position_count,
            im_methodology: w.im_methodology,
            is_approximate: w.is_approximate,
            sensitivities: w.sensitivities.map(SimmSensitivities::from),
            im_breakdown: w.im_breakdown.into_iter().collect(),
        }
    }
}

impl serde::Serialize for NettingSetMargin {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        NettingSetMarginWire::from(self).serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for NettingSetMargin {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = NettingSetMarginWire::deserialize(deserializer)?;
        let currency = wire.initial_margin.currency();
        if wire.variation_margin.currency() != currency || wire.total_margin.currency() != currency
        {
            return Err(serde::de::Error::custom(
                "minor 17: netting-set margin currencies must match",
            ));
        }
        let expected_total = wire.initial_margin.amount() + wire.variation_margin.amount().max(0.0);
        if !amounts_close(wire.total_margin.amount(), expected_total) {
            return Err(serde::de::Error::custom(format!(
                "minor 17: netting-set total_margin {} does not equal initial_margin + max(variation_margin, 0) {}",
                wire.total_margin.amount(),
                expected_total
            )));
        }
        Ok(wire.into())
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct DegradedPositionWire {
    position_id: String,
    message: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct PortfolioMarginResultWire {
    as_of: Date,
    base_currency: Currency,
    total_initial_margin: Money,
    by_csa: BTreeMap<String, ImCollateralResult>,
    total_required_im_collateral: Money,
    total_im_transfer: Money,
    total_segregated_im: Money,
    total_variation_margin: Money,
    total_margin: Money,
    netting_sets: Vec<NettingSetMarginWire>,
    total_positions: usize,
    positions_without_margin: usize,
    degraded_positions: Vec<DegradedPositionWire>,
}

impl From<&PortfolioMarginResult> for PortfolioMarginResultWire {
    fn from(r: &PortfolioMarginResult) -> Self {
        let mut netting_sets: Vec<NettingSetMarginWire> = r
            .by_netting_set
            .values()
            .map(NettingSetMarginWire::from)
            .collect();
        netting_sets.sort_by(|a, b| {
            a.netting_set_id
                .to_string()
                .cmp(&b.netting_set_id.to_string())
        });
        let degraded_positions = r
            .degraded_positions
            .iter()
            .map(|(id, msg)| DegradedPositionWire {
                position_id: id.to_string(),
                message: msg.clone(),
            })
            .collect();
        Self {
            as_of: r.as_of,
            base_currency: r.base_currency,
            total_initial_margin: r.total_initial_margin,
            by_csa: r
                .by_csa
                .iter()
                .map(|(id, account)| (id.clone(), account.clone()))
                .collect(),
            total_required_im_collateral: r.total_required_im_collateral,
            total_im_transfer: r.total_im_transfer,
            total_segregated_im: r.total_segregated_im,
            total_variation_margin: r.total_variation_margin,
            total_margin: r.total_margin,
            netting_sets,
            total_positions: r.total_positions,
            positions_without_margin: r.positions_without_margin,
            degraded_positions,
        }
    }
}

impl From<PortfolioMarginResultWire> for PortfolioMarginResult {
    fn from(w: PortfolioMarginResultWire) -> Self {
        let degraded_positions = w
            .degraded_positions
            .into_iter()
            .map(|d| (PositionId::new(d.position_id), d.message))
            .collect();
        let by_netting_set: HashMap<NettingSetId, NettingSetMargin> = w
            .netting_sets
            .into_iter()
            .map(|wire| {
                let ns = NettingSetMargin::from(wire);
                (ns.netting_set_id.clone(), ns)
            })
            .collect();
        Self {
            as_of: w.as_of,
            base_currency: w.base_currency,
            total_initial_margin: w.total_initial_margin,
            by_csa: w.by_csa.into_iter().collect(),
            total_required_im_collateral: w.total_required_im_collateral,
            total_im_transfer: w.total_im_transfer,
            total_segregated_im: w.total_segregated_im,
            total_variation_margin: w.total_variation_margin,
            total_margin: w.total_margin,
            by_netting_set,
            total_positions: w.total_positions,
            positions_without_margin: w.positions_without_margin,
            degraded_positions,
        }
    }
}

impl serde::Serialize for PortfolioMarginResult {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        PortfolioMarginResultWire::from(self).serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for PortfolioMarginResult {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = PortfolioMarginResultWire::deserialize(deserializer)?;
        let base = wire.base_currency;
        for (label, money) in [
            ("total_initial_margin", wire.total_initial_margin),
            (
                "total_required_im_collateral",
                wire.total_required_im_collateral,
            ),
            ("total_im_transfer", wire.total_im_transfer),
            ("total_segregated_im", wire.total_segregated_im),
            ("total_variation_margin", wire.total_variation_margin),
            ("total_margin", wire.total_margin),
        ] {
            if money.currency() != base {
                return Err(serde::de::Error::custom(format!(
                    "minor 17: {label} currency {} does not match base currency {base}",
                    money.currency()
                )));
            }
        }

        for (id, account) in &wire.by_csa {
            if !wire
                .netting_sets
                .iter()
                .any(|ns| ns.csa_id.as_ref() == Some(id))
            {
                return Err(serde::de::Error::custom(
                    "IM account has no corresponding CSA netting set",
                ));
            }
            let currency = account.gross_initial_margin.currency();
            for amount in [
                account.gross_initial_margin,
                account.required_collateral,
                account.current_collateral,
            ] {
                if amount.currency() != currency
                    || !amount.amount().is_finite()
                    || amount.amount() < 0.0
                {
                    return Err(serde::de::Error::custom(
                        "IM account balances must be nonnegative in one currency",
                    ));
                }
            }
            let difference =
                account.required_collateral.amount() - account.current_collateral.amount();
            if account.transfer.currency() != currency
                || !account.transfer.amount().is_finite()
                || (account.transfer.amount() != 0.0
                    && !amounts_close(account.transfer.amount(), difference))
                || account.required_collateral.amount() > account.gross_initial_margin.amount()
            {
                return Err(serde::de::Error::custom(
                    "Invalid IM collateral transfer or target",
                ));
            }
        }
        // FX-dependent account totals cannot be rederived without the valuation
        // snapshot. Where every account uses the reporting currency, verify them.
        if wire
            .by_csa
            .values()
            .all(|a| a.gross_initial_margin.currency() == base)
        {
            let required: f64 = wire
                .by_csa
                .values()
                .map(|a| a.required_collateral.amount())
                .sum();
            let transfers: f64 = wire.by_csa.values().map(|a| a.transfer.amount()).sum();
            let segregated: f64 = wire
                .by_csa
                .values()
                .filter(|a| a.segregated)
                .map(|a| a.required_collateral.amount())
                .sum();
            if !amounts_close(required, wire.total_required_im_collateral.amount())
                || !amounts_close(transfers, wire.total_im_transfer.amount())
                || !amounts_close(segregated, wire.total_segregated_im.amount())
            {
                return Err(serde::de::Error::custom(
                    "Portfolio IM collateral totals do not equal CSA accounts",
                ));
            }
        }

        let mut sum_im = 0.0;
        let mut sum_vm = 0.0;
        let mut sum_total = 0.0;
        let mut sum_positions = 0usize;
        for netting_set in &wire.netting_sets {
            if netting_set.initial_margin.currency() != base
                || netting_set.variation_margin.currency() != base
                || netting_set.total_margin.currency() != base
            {
                return Err(serde::de::Error::custom(format!(
                    "minor 17: netting set {:?} is not stored in base currency {base}",
                    netting_set.netting_set_id
                )));
            }
            let expected_total = netting_set.initial_margin.amount()
                + netting_set.variation_margin.amount().max(0.0);
            if !amounts_close(netting_set.total_margin.amount(), expected_total) {
                return Err(serde::de::Error::custom(format!(
                    "minor 17: netting-set total_margin {} does not equal initial_margin + max(variation_margin, 0) {}",
                    netting_set.total_margin.amount(),
                    expected_total
                )));
            }
            sum_im += netting_set.initial_margin.amount();
            sum_vm += netting_set.variation_margin.amount();
            sum_total += netting_set.total_margin.amount();
            sum_positions += netting_set.position_count;
        }
        if !amounts_close(wire.total_initial_margin.amount(), sum_im)
            || !amounts_close(wire.total_variation_margin.amount(), sum_vm)
            || !amounts_close(wire.total_margin.amount(), sum_total)
        {
            return Err(serde::de::Error::custom(
                "minor 17: portfolio margin totals do not equal netting-set sums",
            ));
        }
        if wire.total_positions != sum_positions {
            return Err(serde::de::Error::custom(format!(
                "minor 17: total_positions {} does not equal netting-set position count {}",
                wire.total_positions, sum_positions
            )));
        }
        Ok(wire.into())
    }
}
