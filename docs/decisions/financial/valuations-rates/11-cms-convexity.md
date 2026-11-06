# CMS reference swaps, convexity approximation and local risk

- ID: VALUATIONS-RATES-11-CMS-CONVEXITY
- Decision status: Draft (observed implementation; adoption pending).
- Implementation status: Observed in the owning paths below.
- Validation status: Evidence located/inspected, not executed; no universal accuracy qualification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, scope and rationale

CMS options and swaps default to Black76 with an annuity-weight convexity correction. Reference forwards are actual dual-curve swap rates using resolved reference-swap conventions; the payment-measure correction uses a flat-yield annuity approximation. Historical CMS coupons require observed swap-rate fixings, not a live forward. The rationale inferred from the code is a cheap correction with explicit payment delay; adoption and accuracy domain remain open.

## Algorithm and controls

1. CMS tenors must be positive month/year tenors. Explicit fixed/float frequency and day counts override registry fields. The index also supplies settlement days, calendar and business-day convention. USD requires an explicit index; implicit EUR/GBP/JPY mappings use ESTR/SONIA/TONAR OIS. Reference start advances fixing by market settlement business days.
2. Calculate reference swap floating-leg PV divided by fixed-leg annuity, with separate forward and discount curves, short-front stub and registry payment lag. The shared forward engine enforces term tenor where the convention is not OIS.
3. Define flat-yield annuity `A(r)=(1-(1+r/m)^(-m*tenor))/r`, and payment weight `w(r)=(1+r/m)^(-m*delay)/A(r)`. Near zero (`abs(m*tenor*r/m)<1e-3`) use a fourth-order annuity series and its derivatives; otherwise use `ln_1p`/`expm1` forms. The standalone par-annuity helper returns tenor for `abs(r)<1e-9`.
4. The convexity correction is `(w'(F)/w(F))*F^2*sigma_ATM^2*T`. Nonpositive F, T, tenor or invalid/nonpositive frequency returns zero in this helper; future stochastic Black pricing separately rejects nonpositive forward/strike and requires Black-lognormal quotes. ATM vol drives convexity; strike vol prices the option. ACT/365F measures expiry and signed payment delay.
5. CMS options sum `N*alpha*D*Black(F+correction,K,sigma_K,T)`. Fixed-but-unpaid coupons use historical intrinsic payoff; fixing equal to valuation uses live forward intrinsic. CMS swaps add spread to adjusted CMS; caps subtract a call at `cap-spread`, floors add a put at `floor-spread`. Pay means pay CMS/receive funding. Fixed funding uses contractual rate; simple floating funding uses exact past reset or period forward plus bp spread.
6. CMS swap schedule construction fixes short-front/ModifiedFollowing, zero payment lag and shared CMS frequency for generated funding periods; funding reset defaults to accrual start while CMS reset uses index settlement lag. Explicit funding periods preserve separate accrual/reset/payment dates and must be contiguous with positive accrual.

## Risk conventions and limitations

Option Delta and Rho are one-sided forward/discount curve differences per bp; Vega is one-sided surface bump per vol point; Volga is central second difference per squared vol point. StaticReplication Vanna uses four mixed forward-curve/vol reprices, normalized per decimal rate and vol point. Default Vanna instead sums a local Black vanna plus Black gamma times `2*convexity/ATM_vol`, at adjusted forward. It does not differentiate every forward dependence of the convexity correction or the smile; it is an approximation to the complete derivative.

ConvexityAdjustmentRisk subtracts a zero-correction **Black approximation** PV from the context base PV even when the base model is StaticReplication. It therefore need not isolate only the convexity correction within one method. CMS swap cashflow export likewise uses the default approximate CMS coupon path; it is not a replication-model-specific expected schedule.

## Alternatives and tradeoffs

[Normalized smile replication](12-cms-replication.md) is explicitly available and more expensive. A full term-structure payment-measure model would replace the flat-yield weighting assumption. Negative-rate normal CMS pricing is not implemented by this Black branch, despite a replication error message suggesting Black76/Hagan/Bachelier for negative regimes.

## Evidence

- [CMS conventions/weight derivatives](../../../../finstack-quant/valuations/rates/src/cms_common.rs), [forward swap engine](../../../../finstack-quant/valuations/rates/src/forward_swap_rate.rs), [option approximation](../../../../finstack-quant/valuations/rates/src/cms_option/pricer.rs), [option validation](../../../../finstack-quant/valuations/rates/src/cms_option/types.rs), [risk](../../../../finstack-quant/valuations/rates/src/cms_option/metrics.rs).
- [CMS swap pricing](../../../../finstack-quant/valuations/rates/src/cms_swap/pricer.rs), [funding and cashflow construction](../../../../finstack-quant/valuations/rates/src/cms_swap/types.rs), [swap metric selection](../../../../finstack-quant/valuations/rates/src/cms_swap/metrics.rs).
- [CMS option tests](../../../../finstack-quant/valuations/rates/tests/cms_option/test_pricing.rs) and inline CMS swap tests provide sign, fixed-coupon, cap/floor and schedule evidence. These do not establish full local-vanna accuracy over a smile.
- [Hagan CMS convexity](../../../REFERENCES.md#hagan-2003-cms-convexity) is the canonical reference entry; the precise chosen approximation and admissible regimes still require agreement.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
