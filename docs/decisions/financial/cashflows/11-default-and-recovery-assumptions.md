# CASHFLOWS-011 — Default curves, original-balance loss targets and recovery severity

Decision status: **Draft — observed implementation, adoption pending**. Implementation status: **Observed**, scoped to specification calculators and validators. Validation status: **Evidence inspected/located, not executed**. Audit date: 2026-11-05. Source revision: [slice audit](README.md).

## Choice and domain

Default assumptions return a monthly default rate using either annual conditional CDR or original-balance cumulative targets. Recovery assumptions return a severity-derived or constant recovery rate and carry a recovery lag for downstream engines. These are deterministic assumption calculators, without hazard calibration, stochastic default simulation or event emission in this slice.

## Default algorithms

1. **Constant/Vector:** read annual decimal CDR and convert using [CASHFLOWS-010](10-prepayment-and-mortality.md). Vector month zero/one selects its first entry and tail months hold the last value. Checked conversion requires finite CDR in `[0,1]`.
2. **SDA:** base annual CDR is `0.006×m/30` for `m≤30`; `0.006` through month 60; linear decline `0.006−(m−60)/60×(0.006−0.0003)` through month 120; `0.0003` thereafter. Multiply by nonnegative finite speed. Derived annual CDR over one errors. Month zero yields zero.
3. **CumulativeLoss:** supplied monthly cumulative **net loss percentages of original principal**, not decimals, imply cumulative defaults `F(m)=loss_pct(m)/100/severity`, with `F(0)=0`. Severity must be finite in `(0,1]`; the curve must be nondecreasing and nonnegative under whole-spec validation. `F` may exceed one, intentionally allowing par build above original balance. Tail cumulative targets hold constant.
4. **Timing:** total cumulative default rate is a decimal in `[0,1]`; annual timing entries are nonnegative percentages summing to 100 within `1e-6`. Each year's default share accrues linearly over its twelve months. `F(m)=clamp(total_default_rate×Σ annual_pct[y]/100×clamp(m−12y,0,12)/12,0,1)`.
5. For either original-balance model, use `m=max(seasoning_month,1)`, `ΔF=max(F(m)−F(m−1),0)`. Conditional MDR is `clamp(ΔF/S,0,1)`, where default `S=1−F(m−1)` or explicit `mdr_with_survival` receives actual surviving balance/original balance. If `S≤f64::EPSILON`, return zero. Explicit `S` must be finite nonnegative but may exceed one. Capping at one limits defaults to available collateral.

`cumulative_default_fraction` validates the complete model before returning `Some(F)` for original-balance variants and `None` otherwise. `mdr` directly evaluates the requested branch and does not call full `validate`: decreasing loss targets can be locally floored to zero or malformed future timing can escape a single local request. Callers needing a whole-curve guarantee must validate first.

## Recovery algorithm and controls

`RecoveryModelSpec {rate,recovery_lag,severity_vector}` has no automatic product default. Without a nonempty vector, `recovery_rate(month)=clamp(rate,0,1)`. With a vector it selects the same first-entry/tail-hold month convention and returns `clamp(1-severity,0,1)`. If selected vector lookup fails, the infallible method substitutes severity `1-rate`, silently reverting to base recovery. `validate` instead requires finite base rate in `[0,1]`, nonempty optional vector and all severities in `[0,1]`. Thus validation and direct evaluation have materially different invalid-input behavior; NaN base rate can remain NaN through the direct clamp path.

`recovery_lag` is an unsigned month count stored for consumers. This file does not calculate a recovery cash date, discounted recovery value or outstanding-default balance. Recovery estimation in reporting PV is separately specified in [CASHFLOWS-015](15-credit-adjusted-pv.md).

## Rationale, alternatives and limitations

Source comments identify SDA's ramp/plateau/decline and explain survival-aware original-balance default allocation. No approval evidence for these collateral assumptions was located. Conditional CDR and cumulative-loss/timing models are implemented alternatives, with different behavior after prepayment, amortization or par build. Hazard-curve calibration, empirical severity calibration and loss given default correlated with default timing are not supplied here.

## Evidence

- [default.rs](../../../../finstack-quant/cashflows/src/builder/specs/default.rs), [recovery.rs](../../../../finstack-quant/cashflows/src/builder/specs/recovery.rs), [shared vector validation](../../../../finstack-quant/cashflows/src/builder/specs/mod.rs).
- Inspected default unit test `cumulative_loss_preserves_defaults_after_par_build` explicitly expects cumulative defaults 1.4 for 70% net loss at 50% severity, with available-balance clipping. Inspected SDA landmark expectations in [credit_models.rs](../../../../finstack-quant/cashflows/tests/cashflows/builder/credit_models.rs); several derive the annual rate via the production inverse conversion. Located whole-curve validation and nonfinite-implied-default tests in default source.
- [SIFMA](../../../REFERENCES.md#sifma-mbs-standard-formulas) and [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook) are general repository pointers; exact SDA and loss-timing parameter provenance and independent accuracy fixtures remain unverified.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing validation are tracked in the
[cashflow follow-ups](../FOLLOW_UPS.md#cashflows).
