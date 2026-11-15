# VALUATIONS-STRUCTURED-CREDIT-17: OAS scenario discounting and inverse spread

- **Decision status:** Accepted 2026-11-15 — complete OAS scenario, settlement claim and continuous spread fitting package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent intensity, future-settlement, same-day cash and inverse-root calculations checked. Production calibration, scenario pricing, convergence and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for structured-credit OAS and settlement spread fitting

1. **Elect the OAS scenario composition explicitly.** Select deterministic
   or fitted Hull–White discounting and declare all active collateral
   responses. Run the complete contractual waterfall per scenario,
   including funding, calls and delayed receipts. Typed collateral and
   stochastic revolvers require their qualified dated inputs and innovation
   laws on the selected route. Reject unsupported active copulas, unresolved
   workout or card states, and spread-dependent exercises incompatible with
   a frozen-cashflow fit. Preserve model and calibration provenance.
   The main stochastic waterfall and frozen-strip Z-spread have separate
   entry points.
   [Accepted MBS OAS precedent](../valuations-other-fixed-income/14-mbs-risk-oas.md#accepted-decision-for-checked-agency-mbs-rate-risk-and-oas) ·
   [Accepted main Monte Carlo](15-stochastic-waterfall.md#accepted-decision-for-stochastic-waterfall-pricing-and-estimation).

2. **Use exact dated rate states and bank discounting.** Anchor the selected
   fitted Hull–White parameters and discount curve at valuation `V`.
   Simulate the joint state and integrated bank account on the actual
   ACT/365F model grid, including settlement, resets, collateral decisions
   and entitled cash through the last payable date. Keep source curve clocks
   and contractual accrual separate. Use conditional bond ratios and the
   qualified index basis for pending fixings and preserve known fixings.
   Remove approximate monthly integration, its separate variance correction,
   and zero-rate and horizon fallbacks. The zero-volatility limit recovers
   dated deterministic discounting and projection.
   [Accepted exact Hull–White paths](../models-analytic/hw1f-monte-carlo-discounting.md) ·
   [Accepted term projection](../models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection).

3. **Elect the prepayment and credit responses explicitly.** Compare the
   scenario refinance rate with its matching dated and tenored deterministic
   baseline. With explicit `β >= 0`, apply
   `exp(−β × rate difference)` to the eligible prepayment intensity.
   The elected credit overlay uses one horizon-persistent standard-normal
   driver independent of rate innovations, with explicit loadings.
   Low values raise default intensity and lower voluntary prepayment
   intensity. Apply mean-corrected lognormal multipliers to the eligible
   base integrated intensity `−log1p(−p)` and reconstruct probability
   with `−expm1` over actual exposure. Preserve literal zero and one.
   Remove the generic probability cap. The mean correction centers the
   multiplier under the modeled normal law. Mean PD, cash and PV may
   change. Keep named hazards and recovery under their approved authority.
   Reject an enabled overlay with no eligible affected exposure.
   [Accepted collateral credit authority](08-instrument-collateral.md#accepted-decision-for-instrument-collateral-and-liquidity).

4. **Fit the entitled buyer claim on its settlement basis.** Require
   explicit valuation, settlement `S`, entitlement phase, known positive
   delivered face and matched signed accrued. Convert clean per 100 to
   dirty cash as `face × clean / 100 + accrued` once. Value the signed
   buyer ledger, including eligible same-day cash and funding, and exclude
   noncash principal effects. For unconditional future settlement divide
   mean valuation-origin buyer PV by the qualified positive unspread
   curve factor `D(V,S)`. Retain original pre-settlement path history.
   The equivalent settlement-forward calculation weights each path's
   settlement-relative factor by `D_path(V,S)/D(V,S)`. Unresolved contingent
   delivery, face or accrued requires a separately qualified buyer bridge.
   [Accepted original-origin buyer bridge](../valuations-bonds/12-spread-quotes.md) ·
   [Change of numeraire](https://www.cambridge.org/core/journals/journal-of-applied-probability/article/abs/changes-of-numeraire-changes-of-probability-measure-and-option-pricing/EA730D6C18D56426D491B6A25563C0B3).

5. **Fit a continuous spread on frozen scenarios.** Apply
   `exp(−s × ACT/365F(S,T))` once to each already path-discounted flow.
   Same-day settlement cash has unit spread multiplier. Cache the
   cash-discount product by exact date, preserving their covariance.
   The supported inverse requires nonnegative mean discounted cash
   coefficients at positive spread times and at least one strictly
   positive coefficient. Require dirty target above the constant
   settlement term. Keep signed funding in the ledger and reject
   objectives outside the supported inverse domain. Use checked Brent
   with an explicit finite bracket, positive spread tolerance and work
   budget. Reprice the actual returned spread against the dirty target
   under a separate positive absolute settlement-currency residual
   tolerance. Remove hidden widening and last-iterate acceptance.
   [Accepted checked Brent](../core-numerics/brent-root.md).

6. **Use fixed checked sampling and publish the actual price basis.**
   Require at least two independent groups, explicit pairing and exact
   seed or approved derivation for a stochastic run. Retain the inclusive
   100,000 cap on physical waterfall trajectories, including reflected
   legs, and preflight total work. Use stable Philox addresses and the
   same complete sample at every trial. The exact deterministic route
   runs one ledger without sampling. Report annual decimal spread,
   matched clean model and target prices per 100, native-currency signed
   residual, actual solver and model provenance, independent group
   count and physical trajectory count. Report fitting-sample price SE
   from independent group prices, averaging paired legs first. Leave
   deterministic price SE unavailable. Reconcile required configuration,
   units, availability and checked restoration across Rust, Python and
   WASM.
   [Accepted random streams](../models-monte-carlo/02-random-streams.md) ·
   [Accepted independent estimators](../models-monte-carlo/01-engine-and-estimation.md).

## Independent calculation checks

- Two equal valuation-origin paths with settlement factors 0.9 and 1,
  relative payment factors 0.8 and 0.95, and cash 100 and 60 give
  settlement value `(0.9×0.8×100 + 1×0.95×60)/(2×0.95) = 67.8947`.
  Equally averaging the settlement-relative values gives 68.50.
- With face 80, clean price 97.5 and accrued 2, dirty target is 80.
  Entitled cash 2 at settlement and 82.32 of discounted cash one year
  later give `s=ln(82.32/78)=5.3905%`. Dropping the settlement
  cash gives 2.8587% and leaves a 2-unit residual on the entitled ledger.
- The signed price law `200 exp(−s)−100 exp(−2s)` equals 96 at
  both −18.2322% and +22.3144%. This stream has a negative exposed
  coefficient and is outside the selected inverse domain.
- Two equally weighted intensity multipliers 0.5 and 1.5 applied to
  baseline probability 0.25 give mean conditional probability
  0.242228. Unit mean multiplier alone does not preserve the mean PD.

## Observed implementation and scope

OAS has its own simulation model, separate from the main stochastic waterfall engine. It reruns the same deal cashflow engine under rate/credit modulation, caches path cashflows, then solves one spread. Calling the result OAS does not imply it uses the deal's configured copula or all main-MC stochastic specifications.

## Algorithm and numerical settings

Default configuration is 256 scenarios, stochastic credit true, stochastic rates false, kappa 0.05/year, absolute short-rate sigma 0.01, prepayment beta 7, credit loading 0.3, seed 42 and Brent tolerance 1e-7. Path count is capped at 100,000; zero is promoted to one. If both stochastic switches are off, one scenario is used.

Monthly forward rates anchor the short-rate path. A mean-zero OU deviation starts at zero and advances by its exact Gaussian step. Absolute rate is forward plus Hull-White deterministic alpha plus deviation. The path rate shifts floating coupons and prepayment; prepayment responds to `exp(-beta × (rate - initial monthly forward))` with clamps. A single Gaussian credit factor persists over each path: default receives `exp(-loading Z - loading²/2)`, prepayment the opposite sign with the same mean correction. Rate and credit streams are independent; instrument revolvers use another disjoint stream.

Each future cashflow uses the settlement-relative curve DF, multiplied by `exp(-monthly sum of OU deviations)` and the discrete-integral variance correction `exp(-Var/2)`. This correction targets the curve DF in expectation for the simulated monthly grid. Cached pairs `(Act365F settlement time, cash × path DF)` then receive `exp(-spread × time)`. Brent starts at zero with initial ±0.05 bracket size. The result is annual decimal OAS; model/market prices and standard error are percent of current face. Clean target adds accrued once.

## Limitations, failures and alternatives

Monthly forwards are evaluated from the discount curve's time zero,
whereas the path is described from `as_of`. For non-base-date valuations
this can anchor to the wrong forward segment. The future-settlement
formula uses the settlement-relative deterministic curve factor and the
valuation-origin approximate stochastic integral. The valuation-origin
integral is consistent with normalizing by a deterministic settlement
financing factor when the fitted bank path and buyer claim are qualified.
The observed monthly curve mapping, integral and claim state require
reconciliation.

Finite-result and solver failures error, but numeric OasConfig parameters do not receive a complete finite/nonnegative-domain validation here. Forward lookup has a zero-rate fallback. Monthly integration, fixed credit factors and mean correction before probability caps all introduce approximation. Deterministic z-spread is a supported limiting alternative; full calibrated joint rate/credit/copula dynamics are not supplied by these switches.

## Implementation and validation evidence

- [structured_credit/metrics/risk/oas.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/metrics/risk/oas.rs)
- [structured_credit/pricing/simulation_engine/pool_flow_source.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/pool_flow_source.rs)
- [structured_credit/pricing/simulation_engine/instrument_paths.rs](../../../../finstack-quant/valuations/structured-credit/src/structured_credit/pricing/simulation_engine/instrument_paths.rs)

Existing evidence: [structured_credit/unit/metrics/risk_tests.rs](../../../../finstack-quant/valuations/structured-credit/tests/structured_credit/unit/metrics/risk_tests.rs). These are behavior/formula/invariant tests; no test was rerun for this audit. Where only a test location was inspected, no independent reference validation is claimed.

Reference context: [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives), [interest-rate modeling](../../../REFERENCES.md#brigo-mercurio-2006-interest-rate-models), [Brent](../../../REFERENCES.md#brent-1973). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Located deterministic-limit and curve-shape tests do not settle those non-base-date/future-settlement cases.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
