# Cap/floor inverse volatility and risk differentiation

- ID: VALUATIONS-RATES-06-CAP-FLOOR-RISK
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

Analytic quote-model Greeks use the same resolved optionlet inputs as PV; overnight delta/gamma include the compound projection's chain rule. Curve risk, quote volatility, model sigma and date roll are separate perturbations. Their differing units and data-selection paths are material methodology choices.

## Algorithm, selection and conventions

- Delta sums kernel delta times coupon first parallel-forward sensitivity, weighted by N, accrual and payment DF. Gamma uses `kernel_gamma*sensitivity^2 + kernel_delta*second_sensitivity`. Both skip fixed/valuation-date optionlets; HullWhite1F dispatches to the shared full-repricing rate-Greek helper instead.
- Quote-model vega sums Black/Bachelier vega times N, accrual, DF and divides by 100, reporting currency per absolute 0.01 volatility increment. HW surface vega centrally bumps the **Normal surface** by `1e-4` and reprices, scaled to 0.01; it does not itself recalibrate explicit HW parameters. HwSigmaVega separately bumps resolved scalar sigma or every sigma-schedule ordinate with the shared HW bump. It is central if every ordinate exceeds the bump; otherwise forward-sided, scaled to 0.01.
- DV01 uses central quote-replay when required calibration recipes exist; same-ID discount-only OIS has its own eligibility path. Without recipes it uses direct combined rate curves (discount-only for single-curve OIS). ForwardPv01 explicitly requires and bumps a forward object; Rho is generic discount-only DV01; bucketed DV01 uses triangular bumps.
- Theta rolls the date with the shared theta-period rule, capped at the next unfixed optionlet fixing, otherwise final payment, and reprices on the unchanged market. It returns rolled PV minus base PV; it does not invent newly realized fixing data.
- ImpliedVol is supported only for Caplet/Floorlet. Read required quoted premium, obtain the convention using a zero-sigma quote override, and solve `(gross_option_PV(sigma)-target)/N=0` with Brent tolerance `1e-12`, 100 iterations and bracket `[0,1]` Normal or `[0,5]` lognormal. Settled/fixed optionlets return zero; negative/nonfinite target errors. Errors inside the objective become NaN and are handled by the solver. Multi-period caps must use stripping elsewhere.

## Limitations and alternatives

Local analytic Greeks hold quote inputs/discount weights fixed except the specified coupon chain rule. They are not smile-recalibrated total derivatives. Surface vega can be zero with fixed HW sigma and requires a Normal surface even though explicit-parameter pricing need not consume it. Implied vol solves gross option value, while instrument PV may be premium-net; target premium semantics must distinguish those. A flat cap-vol inverse solve is deliberately unsupported here.

## Evidence

[Metric selection and chain rule](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/mod.rs), [shared aggregation](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/common.rs), [vegas](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/vega.rs), [DV01](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/dv01.rs), [theta](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/theta.rs), [inverse solve](../../../../finstack-quant/valuations/rates/src/cap_floor/metrics/implied_vol.rs). [Implied-vol tests](../../../../finstack-quant/valuations/rates/tests/cap_floor/metrics/implied_vol.rs), [normal Greeks](../../../../finstack-quant/valuations/rates/tests/cap_floor/validation/normal_greeks.rs), [forward PV01](../../../../finstack-quant/valuations/rates/tests/cap_floor/metrics/forward_pv01.rs) provide roundtrip/finite-difference evidence. [Brent](../../../REFERENCES.md#brent-1973) supplies solver provenance.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
