# VALUATIONS-OTHER-FIXED-INCOME-14: MBS effective rate risk and Monte Carlo OAS

- **Decision status:** Accepted 2026-11-14 — full checked agency MBS rate risk and OAS package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the entry points and kernels linked below. Reconciliation to the accepted contract is pending.
- **Validation status:** Live source and primary references inspected. Independent refinancing response, rate risk, dated discount, spread-fit and estimator arithmetic checked. Market, model, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked agency MBS rate risk and OAS

1. **Bind the exact claim and elect the experiment.** Use record 13's
   immutable dated pool, actual factor vintage, opening stock and
   pending distribution ownership. Select valuation date `V`,
   information cut, native position amount, one active model and all
   curve roles. The default registered Discounting holder price is
   the record 13 dated price on its admitted base projection. Request
   effective rate risk, a fixed-strip spread, or a fitted Hull–White
   OAS explicitly. A request for the OAS metric alone leaves the
   selected holder price authority intact. Use checked raw native
   amounts before display rounding and before any external signed
   position multiplication.

   For a quote, bind actual buyer settlement `S`, acquired current
   face, paid or pending state and the exact transferred claims.
   Use one observed clean or dirty target with the accepted
   record 13 settlement rules. Convert clean with qualified accrued
   cash. Direct dirty needs no unused accrued inputs. Preserve
   eligible immediate buyer cash and exclude seller entitlements.
   An unqualified factor, cash claim or future settlement basis
   leaves the dependent quote unavailable.
   [Accepted MBS claim and quotation basis](13-mbs-projection.md#accepted-decision-for-checked-agency-mbs-projection-and-cash-ownership),
   [registry selection and lifecycle](../valuations-framework/pricing-lifecycle.md).

2. **Use one checked monthly refinancing response.** Elect frozen
   prepayment or an explicit responsive profile. Preserve the
   covered original accrual months and each baseline conditional
   monthly mortality from record 13. With interior baseline
   `m`, define `H=−log1p(−m)`,
   `H_shocked=H × exp(−β Δq)` and
   `m_shocked=−expm1(−H_shocked)`. Here `β≥0` is a
   finite supplied sensitivity per unit of annual decimal
   refinancing rate, and `Δq=q_live−q_ref` is in that rate
   unit. This scales the monthly survival exponent. It provides
   one law for constant CPR, PSA and approved annual CPR vectors.
   Keep literal baseline zero and one, and preserve the exact
   baseline for a zero response. Carry the checked baseline
   survival or exponent near endpoints so a rounded interior
   probability cannot masquerade as a literal benchmark cap.
   Evaluate interior moves in scale from `log H−β Δq`.
   Fail a required unresolved interior result, including
   numerical false depletion or false zero. Remove the implicit
   doubling coefficient, separate CPR/speed and SMM adjustments,
   the `.9999` cap and mutations of the underlying loan-age
   and prepayment terms.

   Apply the response only to remaining eligible projected
   prepayment, after scheduled principal in the original
   monthly transition. Use the admitted opening decision phase
   of that month. If a live valuation follows the decision,
   require the qualified already selected monthly assumption
   or determined cash. No modeled rate observes an earlier cut
   retrospectively. Keep recognized factors, pending P&I,
   entitled dated claims and literal terminal principal fixed.
   Independence of refinance data may be established by an
   admitted frozen mode or absence of live eligible prepayment.
   Such a branch needs no unused market lookup.
   [Accepted mortality and endpoints](../cashflows/10-prepayment-and-mortality.md),
   [mortgage rate and refinance evidence](https://www.newyorkfed.org/research/staff_reports/sr674.html).

3. **Use a dated refinance proxy with a frozen reference.** For the
   responsive profile select an explicit rate-factor source role,
   positive calendar tenor, exact observation date/phase `d` and
   qualified end-date rule giving `T>d`. A covered deterministic
   continuous-zero proxy is
   `q(d)=−log P_R(d,T)/ACT365F(d,T)`, with qualified relative
   source factors and native source clocks. This is a stipulated
   fixed-tenor forward rate proxy. It requires empirical or
   scenario qualification for the stated use. It neither estimates
   mortgage underwriting rates nor calibrates the borrower response.
   Elect a distinct refinance source or an explicit shared discount
   and refinance source. Freeze the dated `q_ref(d)` from the
   original unshocked source at those same endpoints. Recompute
   actual proxy moves at each eligible decision using the whole
   applicable shocked evaluator. Remove the single WAM-endpoint
   reduction and the short-rate minus initial short-rate comparison.

   On fitted Hull–White paths, use the conditional discount bond
   `P_D(d,T | x_d)`. For a distinct refinance source, derive
   the proxy discount factor as the conditional discount
   bond times the ratio of initial relative refinance and discount
   factors for `(d,T)`, with checked log arithmetic. For a
   shared source use the conditional discount bond directly.
   Rebuild the deterministic basis from each allowed scenario
   while keeping the original reference unchanged. A discount-only
   shock to a distinct discount source then leaves the proxy
   unchanged on matched centered state paths with fixed
   Gaussian parameters. Refinance-factor queries can extend
   beyond the last security cash date. They need covered factor
   data, without inventing future observed states. OAS does
   not enter the refinance proxy or its basis.
   [Accepted model/source clocks and conditional bonds](../models-analytic/rate-clock-and-projection.md),
   [primary mortgage prepayment research](https://www.anderson.ucla.edu/documents/areas/fac/finance/1989-1.pdf).

4. **Reprice the comparable raw holder basis for rate shocks.**
   Elect unspread raw held model or an authored or fitted model
   spread on its qualified forward law. Fit a quote-centered
   anchor once on the corresponding buyer claim and model, then
   hold the spread and law for all sides. Use comparable holder
   cash, including entitled pending distributions. Keep a
   fitted stochastic OAS on the same stochastic pricing
   composition. Apply its held spread from the same explicit
   spread reference date to eligible modeled holder cash, with
   any pre-reference holder cash unspread. Retain that law across
   shocks. An observed mark held fixed remains an observation.
   Do not substitute a stochastic OAS into the static pricing
   branch or refit the mark under each shock. If the selected
   state has no qualified comparable reprice, preserve the
   independently admitted price and mark dependent risk unavailable.

   Move only actual selected typed rate sources on the source's
   accepted continuous-zero coordinates. Combined parallel moves
   all elected active rates together. A triangular key-rate move
   shocks one source and exact coordinate at a time, with the
   shared checked weights. A genuinely shared evaluator moves
   coherently across active roles. Inactive declarations and a
   financing curve used only by the quote bridge are excluded
   from the holder direction. Preserve history, contract, factor,
   prepayment reference, Gaussian parameters and spread. Rebuild
   the affected forward proxy, model curve fit and live cash
   using the same ownership. A frozen prepayment comparison
   explicitly freezes the schedule. Its result basis identifies
   that choice. Retain actual step, coordinates, sources, roles
   and signed native amounts. Report bucket sums separately
   from the simultaneous parallel reprice. Finite sums can
   differ even for partition-of-unity weights.
   [Shared direct rates and finite differences](../valuations-framework/finite-difference-rates.md),
   [accepted held-spread risk precedent](../valuations-bonds/15-curve-risk.md).

5. **Publish signed DV01 and unscaled effective duration and convexity.**
   Require an explicit positive calculation step and exact key
   coordinates for buckets. Evaluate the full approved two-sided
   scenarios without an adaptive fallback. At actual signed
   bp offsets `b_-<0<b_+`, report
   `DV01=(H_+−H_-)/(b_+−b_-)` as holder currency per
   positive bp. Use exact source and key identities. Fail a
   collapsed represented move or incomplete requested side.

   Use `effective_duration` and `effective_convexity` on
   the combined parallel basis with strictly positive raw
   center `H_0`. Let `a>0` and `c>0` be actual up
   and down magnitudes in annual decimal rate units. With
   `u=(H_+−H_0)/a`, `l=(H_0−H_-)/c`, use
   `D_eff=−(c u+a l)/[(a+c)H_0]` and
   `C_eff=2(u−l)/[(a+c)H_0]`. Report years and unscaled
   years squared respectively. Remove effective results from
   the modified-duration and generic convexity keys. No
   near-zero price threshold or forced curvature sign.
   Keep normalized measures unavailable on zero center.
   A closed admitted claim can have zero monetary risk and
   a pending fixed payable can retain discount risk after
   collateral retirement. Separate proved zero from a small
   or noisy finite difference. For live Monte Carlo risk use
   common logical draws and report paired native contrast
   precision. Duration and convexity normalize the estimated
   combined prices. A confidence interval for the normalized
   ratio requires its own method.
   [Accepted bond stencil and normalization](../valuations-bonds/15-curve-risk.md),
   [FHFA on effective risk and held OAS](https://www.fhfa.gov/media/33831#page=62).

6. **Use fitted Hull–White and the same dated bank path for OAS.**
   Bind complete qualified centered Hull–White parameters
   to the selected discount curve at `V`. Admit the shared
   constant or qualified piecewise absolute volatility profile
   and supplied nonnegative mean reversion on ACT/365F model
   time. The accepted zero mean-reversion case remains valid.
   Record supplied parameters or admitted fit identity and
   applicable source, clock and provenance. Remove default
   mean reversion and volatility, fitted theta shortcuts and
   implicit swaption calibration. The pricing composition uses
   the money-market measure. The refinancing response is a
   separately supplied model assumption under that composition.

   Prepare the exact union of live monthly decision dates,
   actual payable dates and required numerical/event knots
   through the last entitled cash. Keep canonical ACT/365F
   model times, resolved source clocks and nominal borrower
   accrual separate. Use the approved joint Gaussian
   endpoint and integrated log bank account. Carry the same
   discount law through pending and delayed remittance,
   including beyond the last collateral amortization.
   A later analytic proxy maturity requires source coverage,
   without a new payoff step. Apply the actual sampled bank
   factor to each path payment once. Remove monthly `1/12`
   as model step, left-point bank integration and frozen-rate
   delay factors. At literal zero volatility use the checked
   dated curve and deterministic projection under the same
   response. The unshocked proxy matches its frozen dated
   reference and recovers base prepayment. A no-draw exact
   branch reports zero actual trajectories. Fixed prepayment
   alone does not remove stochastic discounting from an
   elected simulated run.
   [Accepted exact joint Hull–White discount path](../models-analytic/hw1f-monte-carlo-discounting.md),
   [Hull and White rate model](https://doi.org/10.1093/rfs/3.4.573).

7. **Fit the continuous OAS on the original-origin buyer claim.**
   Use annual decimal continuous spread `s` on the accepted
   ACT/365F payment time measured from `S). For a pure
   settlement quote spread exposure starts at settlement.
   Let`J(s)=E_V[Σ C_j × D_path(V,T_j) × exp(−s τ(S,T_j))]`
   over the selected buyer receipts `T_j≥S`.
   Accept same-date`V=S` with unit financing. Later
   settlement is admissible only with an expressly qualified
   original-origin buyer treatment and positive unspread
   financing factor `F(V,S)`. Future entitlement, delivery,
   factor and intervening cash or uncertainty must be covered
   by that treatment. The model remains at`V`. It never
   restarts at`S` using future state as known. Buyer cash at
   `S` has unit spread multiplier. At the earlier origin
   it still uses its actual model discount and claim state.
   Solve `J(s)/F=Q_dirty` on the authorized settlement
   amount basis.

   Freeze paths, cut, parameters, cash response, recognized
   entitlements and financing across all trials. Discount
   each cashflow by its own path bank factor first. With a
   common known payable grid aggregate complete group
   means `A_j=mean(C_j × D_path(V,T_j))` by exact date
   before multiplying by the common spread factor. Preserve
   cash/discount covariance and actual estimator weights.
   Restrict this inverse to the qualified nonnegative long
   buyer cash profile with at least one positive exposed
   coefficient at strictly positive spread time. Immediate
   cash is a constant component. Require the positive dirty
   target above the constant settlement floor and an
   explicitly ordered finite in-domain sign bracket.
   Use checked fallible Brent for this continuous decreasing
   objective with explicit coordinate tolerance and work
   limit. Verify the actual represented spread by repricing
   to a separately supplied positive absolute settlement
   currency tolerance. Preserve the bracket, steps,
   returned spread and residual. An insensitive price has
   no identified spread. Fail bad trials or an inadmissible
   bracket. Remove embedded ranges, hard-coded guesses,
   hidden widening and last-iterate acceptance.
   [Accepted original-origin spread and residual conventions](../valuations-bonds/12-spread-quotes.md),
   [checked Brent](../core-numerics/brent-root.md),
   [New York Fed OAS methodology](https://www.newyorkfed.org/medialibrary/media/research/staff_reports/sr674.pdf).

8. **Keep deterministic frozen-strip Z separate.** Use the
   exact authorized settlement cash strip and a positive
   dated relative factor `D(S,T)` from its native nominal
   source. Freeze the entire selected prepayment projection
   before spread pricing. For this product's fixed-strip
   comparison elect Continuous on the native source
   coordinate `t=c(T)−c(S)`, with
   `D_z=D(S,T)exp(−z t)`. Preserve unchanged base factors
   at zero spread. Entitled settlement cash and qualified
   unit-factor native plateaus are spread-insensitive.
   A requested inverse needs one matched dirty authority,
   a positive exposed cash bucket, and the same checked
   bracket and native price residual admission. For an
   observed target on this strip no holder financing lookup
   is needed. A derived target needs its explicitly
   qualified buyer valuation and bridge. Retain this
   companion only by explicit request, with its own
   identified result basis. A failed OAS never selects it.
   Zero-volatility OAS coincides with it only when the
   cash, entitlement, price origin, curve, Continuous
   exponents and all financing inputs agree.
   [Shared settlement spread arithmetic](../valuations-bonds/12-spread-quotes.md).

9. **Make sampling, error budgets and result ownership explicit.**
   Require authored fixed independent group count, literal
   u64 seed or selected named derivation, and pairing
   election. Use the accepted checked Philox and
   joint Hull–White layout and deterministic reduction.
   Admit 2 through 5,000,000 independent groups for a
   simulated MBS pricing run, subject to checked step,
   allocation and total work bounds. One trajectory or one
   complete reflected pair mean supplies one observation.
   Report the actual group and trajectory counts and
   parameter and stream provenance. Remove the default
   512 paths and implicit seed. No interval-driven stopping
   or regenerated trial samples.

   At the returned spread, compute price dispersion and
   standard error from complete group PVs with cross-date
   covariance, using checked replay or bounded aggregation.
   Call fitting-sample dispersion descriptive. If a fixed
   spread is independently specified, or a fitted spread is
   held in a separately elected independent holdout, report
   the accepted fixed-sample 95% Student-t price interval
   on those complete pricing groups. An optional holdout
   precision gate uses an explicit positive price half-width
   and fixed budget. Neither diagnostic supplies a spread
   confidence interval. Report numerical residual, model
   and integration or response qualification, and sampling
   separately. Qualify market use and parameter/borrower
   provenance, convergence and flat, nonflat and zero-vol
   limits under explicit amount, risk and work budgets.

   Use the same Rust claim prep, exact risk IDs and
   spread/model basis across native and registered metrics,
   generic Python/WASM, generated schemas and UI, portfolio
   and attribution. Match attribution to the exact rate
   direction and repricing law. Combined parallel curvature
   cannot be applied to an arbitrary twist by averaging
   key-rate moves. Keep projected cash exports distinct
   from pathwise OAS price. Reconcile TBA and dollar-roll
   call sites under record 15 and CMO under its waterfall
   owner. Those consumers must qualify their own price,
   settlement, allocation and shock bases.
   [Philox layout](../models-monte-carlo/02-random-streams.md),
   [sampling and true work](../models-monte-carlo/01-engine-and-estimation.md),
   [matched curve attribution](../valuations-framework/finite-difference-rates.md),
   [TBA and dollar-roll boundary](15-tba-and-dollar-roll.md).

## Observed behavior

### Choice and rationale

MBS duration/convexity reproject cashflows after rate shocks, while OAS fits a spread on stochastic Hull–White paths. The common coefficient `β=ln(2)/.01` encodes a doubling/halving rule for a 100 bp move; source comments give a rule-of-thumb rationale, not pool calibration. These are separate approximations and have not been adopted as one calibrated borrower model.

### Algorithm and selection

**Effective risk:** [duration.rs](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/duration.rs) reprices at zero and ±25 bp by default. A positive finite shock is required. For any rate-risk rebuild, infer one rate move from the DF change at remaining WAM: `Δr=(ln DF_base − ln DF_bumped)/T`. Multiply the PSA speed or CPR-like model parameter by `exp(−βΔr)`; bounded probability variants clamp to `[0,1]`. Rebuild and price the pool. Report `(P_down−P_up)/(2P0h)` and `(P_up+P_down−2P0)/(P0h²)`; near-zero base PV yields zeros. Bucketed DV01 uses the same WAM-endpoint response to a local curve bump.

**OAS:** [mc_oas.rs](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/mc_oas.rs) does the following:

1. Convert the clean quote per 100 current face to dirty money with settlement accrual and a quote-basis pool excluding seller-owned delayed P&I.
2. Fit the Hull–White drift to the discount curve using caller scalar mean reversion/sigma or defaults `.05` and `.01`; a partial pair or sigma schedule is rejected. No swaption-volatility calibration occurs. Default paths are 512; deterministic seed comes from instrument/model configuration.
3. Generate Philox substream paths on a monthly `dt=1/12` grid. Each transition is `r_next=r exp(−κdt)+theta(t)(1−exp(−κdt))+step_stddev z`.
4. At each month, multiply the **base SMM**, not CPR, by `exp(−β(r−r0))`, clamp to `[0,.9999]`, and use the shared level-pay pool step. Discount with accumulated left-endpoint `exp(−(r+oas)dt)` and an additional same-rate factor for the exact agency payment-date offset on the curve day count.
5. Reuse the same simulated paths for every candidate spread. Average path PVs and solve average minus dirty target with Brent, tolerance `1e-7`, 200 iterations, bounds `[-.10,.20]`, initial bracket size `.05`. Return decimal OAS; convergence failure is a validation error.

### Scope, conventions and limitations

Default pricing remains deterministic; requesting OAS selects this MC calculation. The duration path scales CPR/speed and the OAS path scales SMM, so their refinancing responses differ despite sharing β and source comments calling them consistent. Exponential response, hard SMM cap, monthly left-endpoint rate integration and frozen-rate payment-delay adjustment are material approximations. OAS reports no path standard error or confidence interval; its root tolerance is not a statistical price-accuracy guarantee. Native parallel and WASM sequential paths retain deterministic path aggregation order.

### Alternatives and tradeoffs

Deterministic z-spread is cheaper but holds projected prepayments fixed; the registered OAS deliberately uses stochastic rates. An empirical S-curve with burnout and calibrated mortgage-refinancing spread is an unimplemented alternative. Retaining monthly paths is efficient, while finer integration and joint delay-period simulation could reduce discretization bias.

### Evidence

Colocated tests include `payment_offset_uses_the_curve_day_count`, `test_mc_oas_deterministic_with_seed`, `mbs_clean_quote_reprices_at_zero_oas_in_zero_vol_limit`, and `mc_oas_is_stable_across_the_in_flight_payment_date`. The zero-vol comparison allows 0.5% relative PV error; it is a limiting-case comparison to the deterministic kernel, not a calibrated-market benchmark. [Metric tests](../../../../finstack-quant/valuations/fixed-income/src/mbs_passthrough/metrics/mod.rs) verify OAS dispatch and override plumbing. [Hull–White](../../../REFERENCES.md#hull-white-1990-pricing-interest-rate-derivatives), [Philox](../../../REFERENCES.md#salmon-2011-philox) and [SIFMA](../../../REFERENCES.md#sifma-mbs-standard-formulas) are reference context.

## Independent arithmetic and qualification

- On base SMM .20, the explicitly chosen test coefficient
  `β=ln(2)/.01` gives SMM .36 after a 100 bp decline and
  `.105572809000` after a 100 bp rise. In a 1/2/3-year
  triangular grid, a +25 bp one-year key shock changes the
  continuous one-year forwards on successive year windows
  by +25, −25 and 0 bp. The three-year endpoint shift is zero.
- A two-installment zero-WAC and zero-coupon pool of 100
  has eligible cash on 2025-02-18 and 2025-03-17 for the test
  15th-day Following calendar. Its opening 2025-01-01 flat
  continuous ACT/365F curve is 3%, and first-month SMM is
  .20. Apply the selected test coefficient and ±100 bp parallel
  shocks to that shared refinance and discount source. Raw
  center is `99.51793708797`, down `99.69014867842` and
  up `99.34391628541`. The resulting effective duration is
  `.17395476792`, effective convexity `−.18179758996`
  and DV01 `−.0017311619650`.
- With annual short rates of 2% for 12 days and 8% for the
  next 30 days, the dated zero-volatility discount for day
  42 is `.99279321759025`. Carrying the 2% rate through day
  42 gives `.99770127625832`. On a flat 4% model and
  financing curve and 45-day settlement, cash of 100 on
  day 90 at 1% settlement-window OAS gives settlement value
  `99.3854577270`. Applying spread from the valuation origin
  instead gives `99.2630032118`.
- For two equally weighted paths with cash times discounts
  of `(50×.96, 60×.80)` and `(0, 110×.82)` at years
  one and two, the mean coefficients are 24 and 69.1.
  With admitted immediate cash 2, the dirty target 79.571
  fits spread `−log(.9)`, with price slope `−133.542`
  per unit annual decimal spread. The two complete path
  values give descriptive price SE `4.509` at that fit.
  Multiplying the two-date mean cash and mean discount
  separately gives a different price. On a separate
  frozen strip with immediate 2, future 20 and 100 and
  native relative discounts .99 and .95 at years one and
  two, the same continuous spread gives dirty 96.77.

These checks exercise the selected arithmetic, shock shape and
discount and quote clocks. Independent dealer or vendor OAS
benchmarks, borrower-response and parameter evidence, live
dated-rate and settlement paths, paired scenario convergence,
pricing and host qualification remain pending. Repository
pricing and host suites were not run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
