# MODELS-AND-VALUATIONS-CREDIT-15: CDS option forward calibration and spread quadrature

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

BloombergCdso dispatch uses a lognormal spread distribution with an endogenous flat-hazard annuity and mean calibrated to the curve-implied forward CDS value. The Bloomberg label identifies the intended convention; independent vendor parity is not established by the label.

## Scope and input domain

European payer/receiver options on single-name or index CDS. Spread strikes must be positive decimal spreads; clean-price percent strikes require a non-knockout index, explicit coupon and original strike factor. Current index factor is positive and at most one; price-strike realized losses cannot exceed removed original notional. Physical delivery is valued only before expiry.

## Algorithm

1. Build a synthetic protection-buyer CDS using BloombergCdswClean, explicit underlying start or Spot/Forward convention, and current index-factor notional. Obtain scheduled premium accruals after expiry, relative DFs, conditional survival and the previous-coupon-to-expiry accrual stub. The forward clean annuity is the scheduled-coupon PV less the expiry stub, divided by discounted surviving notional (or no survival divisor for non-knockout index options). Forward protection extends the synthetic maturity by one day.
2. At spread state s use credit-triangle hazard s/LGD and `L(s)=Σ accrual_i exp(−s t_i/LGD) forwardDF_i − accruedStub`. Forward swap value is `V(s)=(s−coupon)L(s)`.
3. Front-end protection is LGD times conditional default probability from as-of to expiry. Index forward spread includes this once through FEP/bootstrapped-annuity. Solve `E[V(S)]=(forwardSpread−coupon)×bootstrappedAnnuity`, with `S=m exp(−σ²T/2+σ√T Z)`. Normal trapezoid quadrature calibrates m; log-m bracketing doubles around the forward spread, at most 30 expansions within [1e−8,100], and Brent solves to 1e−12. Forward spread above .10 is rejected.
4. Integrate `max(sign×V(S)+strikeAdjustment+realizedLossAdjustment,0)`. Spread adjustment is sign×(coupon−strike)×L(strike); price adjustment is sign×(price/100−1)×originalFactor/currentFactor. Realized index loss contributes sign×loss/currentFactor separately.
5. Payoff expectation uses adaptive Simpson with approximately half-unit normal panels, total tolerance 1e−12 and depth 32. Multiply by current factor and survival-to-expiry for knockout or single-name options. Non-knockout single-name payer FEP is added outside the exercise payoff. Discount all option settlement amounts to exercise settlement date (expiry if absent).

## Selection and controls

Only BloombergCdso is family-registered. Volatility is explicit implied-vol override or BlackLognormal Strike-axis surface at native strike; expiry uses ACT/365F. Volatility ceiling is 5. Normal integration tails are max(6,4σ√T); mean calibration uses fixed step .05 from [Bloomberg CDSO constants](../../../../finstack-quant/valuations/core/src/constants.rs). Default protection convention is Spot, settlement Cash, knockout false, index factor one; single-name coupon defaults to spread strike. Explicit exercise-settlement/underlying-start dates override defaults.

## Conventions

Spread strike is decimal, coupon bp, clean price percent of par, realized loss fraction of original notional, and volatility annualized decimal. Call means payer and Put receiver. Forward annuity time uses days/365; cash settlement delay changes discounting without changing legal expiry or volatility time.

## Limitations, failure and fallback

Nonpositive forward annuity, tiny LGD, failed calibration bracket and distressed forward fail. Survival ratios are clamped, warning if materially nonmonotone. Flat-annuity states can become negative when a positive stub is subtracted at high spreads; the numerical and financial accuracy of these states is not established by the calibration domain limits. Post-expiry pricing errors; physical settlement at expiry errors because delivery state is absent. No automatic standard-CDS Black-76 fallback exists.

## Alternatives and tradeoffs

Constant-annuity Black-76, normal spread or default-state models are possible alternatives, not selected pricers. The implemented annuity-aware payoff accounts for spread-annuity dependence at the cost of nested calibration and a restricted distressed domain.

## Evidence

Owners: [bloomberg_quadrature.rs](../../../../finstack-quant/valuations/credit/src/cds_option/bloomberg_quadrature.rs), [pricer.rs](../../../../finstack-quant/valuations/credit/src/cds_option/pricer.rs), [types.rs](../../../../finstack-quant/valuations/credit/src/cds_option/types.rs), [parameters.rs](../../../../finstack-quant/valuations/credit/src/cds_option/parameters.rs), [strike.rs](../../../../finstack-quant/valuations/credit/src/cds_option/strike.rs). Inline lifecycle/settlement/sigma tests in pricer.rs and [parameter tests](../../../../finstack-quant/valuations/credit/tests/cds_option/test_parameters.rs) cover contracts; source equations and [Bloomberg CDSO reference](../../../REFERENCES.md#bloomberg-cdso) identify intended provenance. No independent full vendor comparison was executed.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
