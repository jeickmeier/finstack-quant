# MODELS-AND-VALUATIONS-CREDIT-16: CDS option screen risk, quote risk and volatility inversion

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Public option risk mixes screen conventions and repriced sensitivities. Spread-strike Delta/Gamma are Black screen quantities, while clean-price strikes use quote-rebootstrap hedge ratios; these are not uniformly derivatives of the quadrature PV.

## Scope and input domain

Live CDS options under record 15. Price-strike Delta/Gamma and CS01 require replayable hazard quotes and a recalibration provider. Volatility inversion accepts finite nonnegative currency premium and positive model volatility within [1e−6,5].

## Algorithm

1. Spread Delta is Black N(d1) (payer) or N(d1)−1 (receiver), using the loss-adjusted displayed forward, strike, sigma and expiry time. Nonpositive inputs or near-zero σ√T return zero. Spread Gamma is Delta(forward+5bp)−Delta(max(forward−5bp,1e−12)), without dividing by the bump.
2. Clean-price Delta divides the option PV difference by the synthetic underlying CDS PV difference under symmetric ±1bp par-quote hazard rebootstrap. Both legs hold native strike and resolved sigma fixed. Gamma is the difference of these hedge ratios under outer ±5bp quote bumps. Missing provider or near-zero underlying difference fails.
3. Underlying SpreadDv01 is one-sided +1bp quote-rebootstrap CDS PV change; it is not the symmetric denominator used by Delta. Option CS01 uses shared quote-rebootstrap logic. DV01 shifts only discount rates and freezes the hazard curve, while Recovery01 changes contract recovery and replays available hazard calibration.
4. Vega is PV(σ+.01)−PV(σ), recalibrating the spread mean at each sigma; it errors if the bump exceeds 5. Theta reduces diffusion time by 1/365.25 year with calibrated m, schedules, curves and settlement DF held fixed; it is not a full date roll.
5. Implied volatility Brent-solves premium residual in log sigma, tolerance 1e−10, bounds [1e−6,5]; endpoints must bracket the target. Expiry at/before as-of returns zero before pricing, even though direct PV rejects post-expiry valuation.

## Selection and controls

Strike kind dispatches screen risk. Direct CdsOption.delta/gamma passes no provider, so clean-price strikes require the metric engine for these quantities. Standard sensitivity controls set rate/CS01 bumps; Delta/Gamma/Vega screen bumps are fixed constants. Bucketed DV01 delegates triangular key-rate framework logic.

## Conventions

Delta is a dimensionless hedge ratio; Gamma is the change across the full ±5bp screen interval, not currency per decimal-spread squared. Vega is currency change per +1 volatility point; Theta is currency per one fixed-model day. CS01/DV01 are currency per bp.

## Limitations, failure and fallback

Screen Delta returns zero in the zero-vol limit rather than deterministic exercise Delta. CS01/Delta/Gamma return zero at expiry whereas cash-settled PV can still carry payoff; IV also returns zero after expiry. Quote replay for price-strike Delta/Gamma and underlying SpreadDv01 leaves doc-clause/valuation-convention overrides absent, unlike generic option CS01. Fixed mean Theta and frozen hazard DV01 need explicit consumer interpretation.

## Alternatives and tradeoffs

Full PV bump Greeks, analytic differentiation through the mean calibration, and full market-date-roll Theta are alternatives requiring a separately named convention. Current supported screen and quote-risk measures should remain distinguished.

## Evidence

Owners: [option metric implementations](../../../../finstack-quant/valuations/credit/src/cds_option/metrics), [pricer.rs](../../../../finstack-quant/valuations/credit/src/cds_option/pricer.rs), [bloomberg_quadrature.rs](../../../../finstack-quant/valuations/credit/src/cds_option/bloomberg_quadrature.rs). pricer.rs lifecycle tests and the metric implementation formulas provide behavioral evidence; [Bloomberg CDSO](../../../REFERENCES.md#bloomberg-cdso) and [Black 1976](../../../REFERENCES.md#black-1976) provide convention/background pointers, not proof of this implementation’s parity.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
