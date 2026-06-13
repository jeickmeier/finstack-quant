//! JSON wire-format types for margin results.
//!
//! These `*Wire` structs provide a stable, deterministically-ordered
//! serialization representation for the core domain types in
//! [`super::results`]. They are an internal implementation detail: the public
//! `serde::Serialize`/`Deserialize` impls for [`NettingSetMargin`] and
//! [`PortfolioMarginResult`] delegate to the corresponding wire type so that
//! `HashMap`-backed fields serialize in a stable order.

use finstack_core::currency::Currency;
use finstack_core::dates::Date;
use finstack_core::money::Money;
use finstack_core::HashMap;
use finstack_margin::{ImMethodology, NettingSetId, SimmRiskClass, SimmSensitivities};

use crate::types::PositionId;

use super::results::{NettingSetMargin, PortfolioMarginResult};

const MARGIN_WIRE_AMOUNT_TOLERANCE: f64 = 1e-9;

fn amounts_close(lhs: f64, rhs: f64) -> bool {
    (lhs - rhs).abs() <= MARGIN_WIRE_AMOUNT_TOLERANCE
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CurrencyTenorEntry {
    currency: Currency,
    tenor_bucket: String,
    value: f64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LabelTenorEntry {
    label: String,
    tenor_bucket: String,
    value: f64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LabelEntry {
    name: String,
    value: f64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CurrencyEntry {
    currency: Currency,
    value: f64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CurrencyPairEntry {
    base: Currency,
    quote: Currency,
    value: f64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct CurvatureEntry {
    risk_class: SimmRiskClass,
    value: f64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SimmSensitivitiesWire {
    base_currency: Currency,
    ir_delta: Vec<CurrencyTenorEntry>,
    ir_vega: Vec<CurrencyTenorEntry>,
    credit_qualifying_delta: Vec<LabelTenorEntry>,
    credit_non_qualifying_delta: Vec<LabelTenorEntry>,
    equity_delta: Vec<LabelEntry>,
    equity_vega: Vec<LabelEntry>,
    fx_delta: Vec<CurrencyEntry>,
    fx_vega: Vec<CurrencyPairEntry>,
    commodity_delta: Vec<LabelEntry>,
    curvature: Vec<CurvatureEntry>,
}

impl From<&SimmSensitivities> for SimmSensitivitiesWire {
    fn from(s: &SimmSensitivities) -> Self {
        let mut ir_delta: Vec<CurrencyTenorEntry> = s
            .ir_delta
            .iter()
            .map(|((ccy, tenor), &v)| CurrencyTenorEntry {
                currency: *ccy,
                tenor_bucket: tenor.clone(),
                value: v,
            })
            .collect();
        ir_delta.sort_by(|a, b| {
            a.currency
                .cmp(&b.currency)
                .then_with(|| a.tenor_bucket.cmp(&b.tenor_bucket))
        });

        let mut ir_vega: Vec<CurrencyTenorEntry> = s
            .ir_vega
            .iter()
            .map(|((ccy, tenor), &v)| CurrencyTenorEntry {
                currency: *ccy,
                tenor_bucket: tenor.clone(),
                value: v,
            })
            .collect();
        ir_vega.sort_by(|a, b| {
            a.currency
                .cmp(&b.currency)
                .then_with(|| a.tenor_bucket.cmp(&b.tenor_bucket))
        });

        let mut credit_qualifying_delta: Vec<LabelTenorEntry> = s
            .credit_qualifying_delta
            .iter()
            .map(|((label, tenor), &v)| LabelTenorEntry {
                label: label.clone(),
                tenor_bucket: tenor.clone(),
                value: v,
            })
            .collect();
        credit_qualifying_delta.sort_by(|a, b| {
            a.label
                .cmp(&b.label)
                .then_with(|| a.tenor_bucket.cmp(&b.tenor_bucket))
        });

        let mut credit_non_qualifying_delta: Vec<LabelTenorEntry> = s
            .credit_non_qualifying_delta
            .iter()
            .map(|((label, tenor), &v)| LabelTenorEntry {
                label: label.clone(),
                tenor_bucket: tenor.clone(),
                value: v,
            })
            .collect();
        credit_non_qualifying_delta.sort_by(|a, b| {
            a.label
                .cmp(&b.label)
                .then_with(|| a.tenor_bucket.cmp(&b.tenor_bucket))
        });

        let mut equity_delta: Vec<LabelEntry> = s
            .equity_delta
            .iter()
            .map(|(name, &v)| LabelEntry {
                name: name.clone(),
                value: v,
            })
            .collect();
        equity_delta.sort_by(|a, b| a.name.cmp(&b.name));

        let mut equity_vega: Vec<LabelEntry> = s
            .equity_vega
            .iter()
            .map(|(name, &v)| LabelEntry {
                name: name.clone(),
                value: v,
            })
            .collect();
        equity_vega.sort_by(|a, b| a.name.cmp(&b.name));

        let mut fx_delta: Vec<CurrencyEntry> = s
            .fx_delta
            .iter()
            .map(|(ccy, &v)| CurrencyEntry {
                currency: *ccy,
                value: v,
            })
            .collect();
        fx_delta.sort_by(|a, b| a.currency.cmp(&b.currency));

        let mut fx_vega: Vec<CurrencyPairEntry> = s
            .fx_vega
            .iter()
            .map(|((base, quote), &v)| CurrencyPairEntry {
                base: *base,
                quote: *quote,
                value: v,
            })
            .collect();
        fx_vega.sort_by(|a, b| a.base.cmp(&b.base).then_with(|| a.quote.cmp(&b.quote)));

        let mut commodity_delta: Vec<LabelEntry> = s
            .commodity_delta
            .iter()
            .map(|(name, &v)| LabelEntry {
                name: name.clone(),
                value: v,
            })
            .collect();
        commodity_delta.sort_by(|a, b| a.name.cmp(&b.name));

        let mut curvature: Vec<CurvatureEntry> = s
            .curvature
            .iter()
            .map(|(rc, &v)| CurvatureEntry {
                risk_class: *rc,
                value: v,
            })
            .collect();
        curvature.sort_by_key(|e| format!("{:?}", e.risk_class));

        Self {
            base_currency: s.base_currency,
            ir_delta,
            ir_vega,
            credit_qualifying_delta,
            credit_non_qualifying_delta,
            equity_delta,
            equity_vega,
            fx_delta,
            fx_vega,
            commodity_delta,
            curvature,
        }
    }
}

impl From<SimmSensitivitiesWire> for SimmSensitivities {
    fn from(w: SimmSensitivitiesWire) -> Self {
        let mut s = SimmSensitivities::new(w.base_currency);
        for e in w.ir_delta {
            s.ir_delta.insert((e.currency, e.tenor_bucket), e.value);
        }
        for e in w.ir_vega {
            s.ir_vega.insert((e.currency, e.tenor_bucket), e.value);
        }
        for e in w.credit_qualifying_delta {
            s.credit_qualifying_delta
                .insert((e.label, e.tenor_bucket), e.value);
        }
        for e in w.credit_non_qualifying_delta {
            s.credit_non_qualifying_delta
                .insert((e.label, e.tenor_bucket), e.value);
        }
        for e in w.equity_delta {
            s.equity_delta.insert(e.name, e.value);
        }
        for e in w.equity_vega {
            s.equity_vega.insert(e.name, e.value);
        }
        for e in w.fx_delta {
            s.fx_delta.insert(e.currency, e.value);
        }
        for e in w.fx_vega {
            s.fx_vega.insert((e.base, e.quote), e.value);
        }
        for e in w.commodity_delta {
            s.commodity_delta.insert(e.name, e.value);
        }
        for e in w.curvature {
            s.curvature.insert(e.risk_class, e.value);
        }
        s
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct NettingSetMarginWire {
    netting_set_id: NettingSetId,
    as_of: Date,
    initial_margin: Money,
    variation_margin: Money,
    total_margin: Money,
    position_count: usize,
    im_methodology: ImMethodology,
    sensitivities: Option<SimmSensitivitiesWire>,
    im_breakdown: HashMap<String, Money>,
}

impl From<&NettingSetMargin> for NettingSetMarginWire {
    fn from(m: &NettingSetMargin) -> Self {
        Self {
            netting_set_id: m.netting_set_id.clone(),
            as_of: m.as_of,
            initial_margin: m.initial_margin,
            variation_margin: m.variation_margin,
            total_margin: m.total_margin,
            position_count: m.position_count,
            im_methodology: m.im_methodology,
            sensitivities: m.sensitivities.as_ref().map(SimmSensitivitiesWire::from),
            im_breakdown: m.im_breakdown.clone(),
        }
    }
}

impl From<NettingSetMarginWire> for NettingSetMargin {
    fn from(w: NettingSetMarginWire) -> Self {
        Self {
            netting_set_id: w.netting_set_id,
            as_of: w.as_of,
            initial_margin: w.initial_margin,
            variation_margin: w.variation_margin,
            total_margin: w.total_margin,
            position_count: w.position_count,
            im_methodology: w.im_methodology,
            sensitivities: w.sensitivities.map(SimmSensitivities::from),
            im_breakdown: w.im_breakdown,
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
