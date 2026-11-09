# VALUATIONS-RATES-07-EUROPEAN-SWAPTIONS: European swaption forwards, settlement annuities and quote Greeks

- ID: VALUATIONS-RATES-07-EUROPEAN-SWAPTIONS
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

European swaptions use a Black/shifted-Black or Bachelier option on a projected par swap rate, multiplied by notional and a settlement-specific annuity. `vol_model` sets the default model; explicit Black/Normal pricers reject mismatch. SABR supplies a quote when parameters exist and there is no implied-vol override. HW tree and LMM paths are separate records. [ADR 0029](0029-rate-option-exercise-and-volatility.md) is the existing selection draft.

## Algorithm and conventions

1. Require positive notional, expiry no later than start, ordered underlying dates, matched fixed/float spans, finite strike magnitude at most 2 and valid legs/SABR params. Resolve leg sentinels/EOM with IRS helpers. Standard convenience underlier uses semiannual 30/360 fixed, quarterly ACT/360 float, ModifiedFollowing/no stub, zero lags; registered overnight ID can select compounding.
2. Fixed annuity is unpaid `sum(alpha_i*D_i)`. In the tightly matched single-curve case use discount-ratio coupons on the fixed schedule. When same-ID simple projection has no forward object, calculate discount-ratio coupons on the float schedule including spread. Otherwise price the floating leg of a zero-fixed-rate IRS and divide by N and fixed annuity. Near-zero annuity returns forward zero.
3. Physical, CollateralizedCashPrice and IsdaParPar use curve annuity. CollateralizedCashPrice is the default cash-settlement method. ParYield uses a regular-frequency geometric annuity based on period count and forward rate, discounted to underlying start; actual stub accrual sizes are not individually used in that formula. ZeroCoupon uses fixed-day-count total tenor times terminal DF.
4. Resolve volatility at ACT/365F expiry, contractual underlying tenor and strike. The tenor helper uses the positive calendar-month difference divided by12, ignores day-of-month and rejects spans with no positive month difference. Implied-vol override wins; otherwise embedded SABR produces Normal or Black quote consistent with vol_model, including its shift. Otherwise resolve surface/provider quote; clamp unless model extrapolation is Error. Apply displacement to both F and K; no conversion between incompatible Normal and Black sources.
5. Return `N*A*Black_or_Normal(F,K,sigma,T)`. Exactly at expiry return intrinsic times settlement annuity; after expiry return zero even for physical settlement. The object does not carry a post-exercise delivered swap or settlement claim.

## Risk and inverse solve

Analytic Delta/Gamma/Vega multiply kernel Greeks by N and the **held-fixed** annuity; Vega divides by 100. They do not differentiate ParYield annuity dependence on F or SABR smile dependence on F. HW Delta/Gamma use shared repricing; HW quote Vega errors and directs the caller to HwSigmaVega, which centrally bumps explicit sigma by `min(shared_bump,sigma/2)` and scales to 0.01. Generic combined DV01, discount-only Rho and triangular bucketed DV01 remain full repricing channels.

ImpliedVol solves quoted nonnegative premium divided by notional with Brent tolerance `1e-12`, bracket `[0,1]` Normal or `[0,5]` Black; it validates price at sigma zero first and requires final PV residual within `1e-8*max(abs(target),1)`. At/after expiry it returns zero. This solves a constant quote vol, not SABR parameter calibration.

## Alternatives, limitations and evidence

Cash-settlement annuities are materially distinct supported branches; equality of IsdaParPar and curve-annuity behavior is observed and must not be inferred to cover all possible market settlement definitions. Full cash-annuity stochastic expectation is not implemented by the closed-form multiplier. [Implementation](../../../../finstack-quant/valuations/rates/src/swaption/types/swaption.rs), [settlement definitions](../../../../finstack-quant/valuations/rates/src/swaption/types/definitions.rs), [model dispatch](../../../../finstack-quant/valuations/rates/src/swaption/pricer.rs), [Delta](../../../../finstack-quant/valuations/rates/src/swaption/metrics/delta.rs), [Gamma](../../../../finstack-quant/valuations/rates/src/swaption/metrics/gamma.rs), [Vega](../../../../finstack-quant/valuations/rates/src/swaption/metrics/vega.rs), [implied vol](../../../../finstack-quant/valuations/rates/src/swaption/metrics/implied_vol.rs), [metric registration](../../../../finstack-quant/valuations/rates/src/swaption/metrics/mod.rs).

[Forward/annuity tests](../../../../finstack-quant/valuations/tests/instruments/swaption/pricing/forward_annuity.rs), [QuantLib parity](../../../../finstack-quant/valuations/tests/instruments/swaption/integration/quantlib_parity.rs), [implied-vol tests](../../../../finstack-quant/valuations/tests/instruments/swaption/metrics/implied_vol.rs) are located formula/fixture/roundtrip evidence, not executed. The test file labelled QuantLib parity constructs a local Black expectation with production annuity/CDF helpers; its name does not establish independent vendor agreement. Inline analytic registry tests in `pricer.rs` compare default/explicit PV and raw-value paths, including SABR. References: [Black](../../../REFERENCES.md#black-1976), [Bachelier](../../../REFERENCES.md#bachelier-1900), [SABR](../../../REFERENCES.md#hagan-2002-sabr), [ISDA 2021](../../../REFERENCES.md#isda-2021-interest-rate-derivatives-definitions).

## Evidence limits

A physical-settlement label does not implement exercise-state transfer.

## Relationship to the accepted SABR source boundary

[CORE-MARKET-16's complete accepted package](../core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning)
requires active cube requests to select Normal/Black/ShiftedBlack explicitly,
retain the actual evaluated displacement and agree with the source expiry clock
and underlying reference. Match the contractual tenor and distinguish stored
construction forwards from the live pricing forward; choose any frozen/rebased
smile model explicitly. Scalar sigma plus explicit model inputs must not inspect
unused same-ID cubes. Record 16 was accepted on **2026-11-08**, with reconciliation/qualification
pending. Its acceptance does not adopt this record's
settlement, pricing, degenerate Greeks or smile dynamics.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
