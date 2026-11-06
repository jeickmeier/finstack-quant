# VALUATIONS-RATES-02-FRA: FRA historical fixings, early settlement and quote risk

- ID: VALUATIONS-RATES-02-FRA
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

FRAs settle the discounted interest difference at accrual start, with no residual value on or after that start. Pay denotes pay fixed/receive floating. The forward-rate and discount curve are independent dependencies, subject to the observed-fixing selection caveat below.

## Algorithm and controls

1. Require ordered start/maturity, positive notional, finite observed fixing and reset lag magnitude no greater than 31. Explicit fixing date must be on/before start. Otherwise resolve calendar from the explicit fixing calendar or forward-ID convention registry, subtract reset-lag business days and apply the selected fixing BDC (default ModifiedFollowing); no calendar means error.
2. Compute accrual alpha from raw start/maturity and instrument day count; clamp at zero and reject below the local minimum period threshold.
3. If fixing is strictly before valuation, use explicit observed fixing first, otherwise exact fixing-series observation. If fixing is today/future, prefer a forward-curve period rate; only if that curve is unavailable use the supplied observed fixing. Thus an observed fixing is not an unconditional override before its date.
4. Before start, compute settlement `N*(F-K)*alpha/(1+F*alpha)`, positive for Pay, negative for Receive, and discount at the start payment date. Reject a denominator close to zero by absolute-value threshold; a substantially negative denominator is not separately rejected. Extreme forwards warn. Start-or-later returns an empty NoResidual schedule.
5. ParRate returns the selected forward/fixing. PV01 and ForwardPv01 both use forward-only curve bumps; BucketedDv01 uses generic triangular rate risk.
6. DV01 centrally bumps/replays calibration quotes only when both discount and forward carry recipes and the forward recipe has no Basis quote. Otherwise it uses generic combined curve bumps. Replay errors after eligibility selection propagate; this is not a catch-all numerical fallback.

## Conventions, limitations and alternatives

Fixed rate and observed rates are decimal; reset lag is business days; quote-risk bumps are bp. Futures convexity is not applied to an FRA. End-settled FRAs or different fixing publication cutoffs would need distinct behavior and are not branches here. Quote-replay risk and direct curve-shift risk can differ materially despite sharing the DV01 name.

## Evidence

- [FRA implementation](../../../../finstack-quant/valuations/rates/src/fra/types.rs), [metrics and replay eligibility](../../../../finstack-quant/valuations/rates/src/fra/metrics.rs).
- [Pricing](../../../../finstack-quant/valuations/rates/tests/fra/pricing.rs), [cashflows](../../../../finstack-quant/valuations/rates/tests/fra/cashflows.rs), [QuantLib parity](../../../../finstack-quant/valuations/rates/tests/fra/quantlib_parity.rs), [DV01](../../../../finstack-quant/valuations/rates/tests/fra/metrics/dv01.rs): formula, lifecycle, external-reference and risk evidence with the scope encoded by each fixture; not executed in this audit.
- [Hull](../../../REFERENCES.md#hull-options-futures), [ISDA conventions](../../../REFERENCES.md#isda-2006-definitions).

## Evidence limits

`market_dependencies` omits the forward curve and fixing series whenever `observed_fixing` exists, although today's/future-fixing pricing still prefers an available forward curve. This can hide a real curve dependency from generic sensitivity/risk tooling.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
