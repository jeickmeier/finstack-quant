# VALUATIONS-BONDS-17: Recovery01 and immediate jump-to-default on hazard-priced bonds

**Decision status:** Draft — observed implementation; adoption pending.
**Implementation status:** Observed in the linked bond production scope; discrepancies are identified explicitly.
**Validation status:** Evidence inspected or located, not executed; no accuracy certification.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and scope

These metrics require an explicit hazard curve and active HazardRate/RatesCredit model; other models error, including structural Merton. They use the hazard model's fractional recovery of outstanding par without accrued interest. Bond notional is positive; short exposure comes from negative portfolio quantity. Recovery and hazard-rate recalibration policy is delegated to the common Recovery01 implementation.

## Algorithm and controls

Recovery01 first builds the quote-reproducing OAS view, then reprices with recovery shifted by an absolute 0.01. The shared helper takes a central difference where both recovery bumps are inside [0,1], a one-sided difference at a boundary, and normalizes to a +1 percentage-point recovery change. A hazard curve with calibration provenance is recalibrated to unchanged par quotes at the changed recovery; a hand-built curve keeps hazard rates fixed. These two economically different regimes can produce different signs. Repricing retains selected model/registry; bond discount-curve ID supplies the recovery helper's discount dependency.

Jump-to-default is `R·N(as_of)−base_value`, in currency. Clamp hazard recovery to [0,1], build the full schedule and its cash-date recovery-principal replay, then take outstanding **after** events on `as_of`; no future discount or accrued claim is included. The subtraction uses the actual current context base value, including a price quote. Negative values indicate holder loss on immediate default. Principal during repayment lags follows the special recovery path of record 02.

## Alternatives, limits and evidence

Frozen-hazard and recalibrated-hazard Recovery01 are both implemented but selected from curve provenance, not an independent metric switch. Market-value recovery, accrued recovery and Merton structural-default sensitivities would be other methods. JTD measures a discrete jump, whereas Recovery01 is a conditional model sensitivity; neither is expected loss or CS01.

Owning code: [bond metrics](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/default_risk.rs), [shared recovery helper](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/recovery01.rs), [cash-date principal replay](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/settlement.rs). [Default-risk tests](../../../../finstack-quant/valuations/fixed-income/tests/default_risk.rs) locate quote/recovery identity and nonhazard rejection; [recalibrated fixtures](../../../../finstack-quant/valuations/tests/instruments/bond/metrics/default_risk_recalibrated.rs) distinguish recalibrated versus frozen hazard. Evidence located, not executed. Reference: [Hull–White credit](../../../REFERENCES.md#hull-white-2000-cds).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
