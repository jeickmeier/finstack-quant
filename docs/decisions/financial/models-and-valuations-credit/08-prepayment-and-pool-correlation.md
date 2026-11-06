# MODELS-AND-VALUATIONS-CREDIT-08: Stochastic prepayment, burnout and pool-correlation controls

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

CPR shocks are transformed to monthly SMM after clamping; regime switching carries explicit path state, while Richard–Roll uses a simplified arctangent refinancing multiplier and caller-updated burnout.

## Scope and input domain

Monthly seasoning, decimal CPR/coupon/market rate and burnout. Helpers clamp CPR to [0,1], loading to [−1,1], volatility to [0,1], refinancing sensitivity to [0,10] and burnout rate to [0,1]. Defaults are deterministic 100% PSA.

## Algorithm

1. Factor prepayment converts the base SMM to CPR (base errors become zero), multiplies by exp(loading×vol×Z)×burnout, clamps annual probability then converts back to SMM.
2. Expected SMM integrates the nonlinear capped annual-to-monthly transform under normal shocks. It integrates the survival function in log-SMM coordinates using composite Gauss–Legendre (16 nodes, four subintervals) over bounds implied by ±10σ; an integration failure becomes NaN.
3. Richard–Roll base CPR is multiplied by max(1+s×atan(20×(coupon−marketRate))/(π/2),0), a 30-month seasoning ramp, burnout, and the lognormal factor shock. expected_smm assumes market rate equals coupon and no burnout. Burnout update multiplies prior burnout by `1−burnoutRate×(realizedSMM/expectedSMM−1)` and clips [0,1]; negligible expected SMM substitutes ratio one.
4. Regime switching begins low at seasoning zero. High-state probability is up/(up+down)×[1−(1−up−down)^seasoning], with stable/alternating-power branches. sample_smm transitions the stored state before applying the selected low/high CPR shock; conditional/expected methods weight both regimes by the seasoning distribution.
5. Pool correlation defaults to .10 asset and −.20 prepay/default. Sectored scalar reduction is the equal average of intra/inter, matrix reduction is the off-diagonal arithmetic mean, and matrix prepay/default correlation returns −.20. Asset bumps shift sector intra by Δ and inter by Δ/2, or every matrix off-diagonal by Δ; clamps report diagnostics. Matrix prepay/default bump is a reported no-op.

## Selection and controls

Explicit spec chooses deterministic/factor/Richard–Roll/regime path. Default factor loading .4 and CPR volatility .20 apply to Richard–Roll and regime helper constructors. Matrix validation delegates correlation-matrix validation; post-bump matrices are not repaired to PSD here.

## Conventions

SMM and MDR are monthly competing-event inputs whose application order belongs to the consuming pool engine. Positive prepayment factor increases CPR; positive default factor usually decreases defaults. Matrix scalar reduction discards sector/weight composition.

## Limitations, failure and fallback

expected_smm is a baseline expectation, not a conditional current-burnout/current-rate forecast. Ratio burnout can increase toward one after below-baseline prepayment. Helpers clamp invalid finite parameters and many APIs return f64 instead of errors. A scalar asset correlation from a full matrix loses dependence structure.

## Alternatives and tradeoffs

Full Richard–Roll seasonality/loan-level refinance data, fitted transition models and dynamically updated borrower mix are unimplemented. Explicit regime states preserve persistence that a weighted-rate approximation loses.

## Evidence

Owners: [Prepayment implementations](../../../../finstack-quant/models/src/credit/pool/prepayment), [Expected SMM integration](../../../../finstack-quant/models/src/credit/pool/mod.rs), [correlation.rs](../../../../finstack-quant/models/src/credit/pool/correlation.rs). Inline tests locate expectation/transition/bump evidence. Reference: [Richard–Roll](../../../REFERENCES.md#richard-roll-1989) is background; exact simplified coefficients require separate calibration provenance.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
