# Basis swap floating legs and absolute par spread

- ID: VALUATIONS-RATES-04-BASIS-SWAPS
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

Basis swaps value receive-primary/pay-reference floating coupons, each with its own projection and discount curve. The par-spread metric solves the primary leg's total quoted spread; incremental spread subtracts its existing quote. Simple and compounded overnight legs share the IRS/overnight kernels but retain basis-specific calendar and validation rules.

## Algorithm and controls

1. Require positive notional and positive leg tenors. Identical forward IDs error unless `allow_same_curve=true`; missing/invalid calendars error unless `allow_calendar_fallback=true`, which selects weekends-only. Negative reset/payment lags and SimpleAverage are rejected; simple overnight-ID misuse is rejected by the shared convention guard.
2. For term coupons, build unadjusted accrual boundaries and adjusted payments. Derive resets using the separate fixing calendar when provided. Select exact observed reset or period forward through shared fixing/projection helpers. Coupon is `N*(L+spread_bp/10000)*alpha`; discount with that leg's curve.
3. Compounded coupons use adjusted observation boundaries, the shared overnight engine, simple spread outside compounding and projected-fixing metadata. Degenerate adjusted intervals are skipped. PV excludes payments on or before valuation and uses Neumaier summation.
4. Annuity sums discounted accrual factors on unpaid periods. For overnight legs it reuses the projector's effective accrual rather than assuming the nominal schedule alpha. Reject annuity below `1e-12`.
5. `BasisParSpread = 10000*(PV_reference-PV_primary_with_zero_spread)/(N*A_primary)`. Keep the reference spread unchanged. Reject near-zero annuity and notional below 0.01 for this inverse metric. `IncrementalParSpread=BasisParSpread-current_primary_spread_bp`.
6. DV01 selects generic **per-curve** parallel sensitivities; BucketedDv01 selects triangular key-rate bumps. Annuity/PV metrics expose both legs separately. Model choice is deterministic discounted cashflow; there is no local quote-calibration replay branch.

## Conventions, failures and alternatives

The leg spread is bp, not decimal. Absolute spread above 5000 bp errors and above 500 bp warns. Separate leg discount curves are accepted here, unlike IRS validation. Zero-spread linearity makes algebraic par solving exact for the implemented simple additive spread; compounding spread inside daily factors would require a different solve and is not implemented. Explicit same-index spread trades are allowed only through the opt-in flag.

## Evidence

- [Basis implementation](../../../../finstack-quant/valuations/rates/src/basis_swap/types.rs) and [metrics](../../../../finstack-quant/valuations/rates/src/basis_swap/metrics.rs).
- [Par-spread tests](../../../../finstack-quant/valuations/rates/tests/basis_swap/test_basis_swap_par_spread.rs), [overnight tests](../../../../finstack-quant/valuations/rates/tests/basis_swap/test_basis_swap_overnight.rs), [sensitivities](../../../../finstack-quant/valuations/rates/tests/basis_swap/test_basis_swap_sensitivities.rs): algebraic solve, projection and bump evidence, not executed.
- [Ametrano/Bianchetti](../../../REFERENCES.md#ametrano-bianchetti-2013), [Sadr](../../../REFERENCES.md#sadr-2009-interest-rate-swaps).

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
