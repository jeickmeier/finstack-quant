# VALUATIONS-BONDS-09: Structural-parameter calibration to a cash-equivalent quote

**Decision status:** Accepted — complete checked cash-equivalent structural calibration package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, stochastic fit, market, risk and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked cash-equivalent structural calibration

1. **Elect a single-scalar cash-equivalent pricing fit.** Require
   the explicit structural pricing law of record 08 and select
   either positive debt barrier in the qualified asset monetary
   basis or positive annualized asset volatility in decimal units
   per square root of a model year. Fit one observed or expressly
   qualified proxy price for the identified cash-equivalent claim.
   Preserve the structural law, monitoring, asset level, model
   rate, payout, applicable barrier growth, recovery rule and
   other admitted model inputs. Update only the selected base
   parameter and quantities explicitly derived from it. An already
   fixed payoff or one structurally insensitive to that parameter
   cannot supply an active fit. A tolerated price match identifies a
   bracket-selected parameter on the declared simulation basis.
   It supplies no simultaneous fit, unique market identification
   or physical probability calibration. A quote on the original
   toggle or PIK bond can be used only with explicit provenance
   qualifying it as a proxy for the cash-equivalent claim.
   [Accepted structural simulation](08-merton-simulation.md#accepted-decision-for-checked-structural-bond-simulation-and-outputs),
   [structural measures and fits](../models-and-valuations-credit/04-structural-default.md)
   and [Merton on structural asset and debt pricing](https://onlinelibrary.wiley.com/doi/full/10.1111/j.1540-6261.1974.tb03058.x).

2. **Prepare the cash equivalent without rewriting the past.**
   Freeze the original valuation date and information cut, issue
   identity, current accreted par, authoritative PIK and payment
   history, captured parent balances, and paid, pending, vested or
   previously elected claims. Replace only still-unresolved future
   coupon elections with the identified proxy's authorized cash
   terms. Use original parent IDs, actual dates, accrual factors,
   stub and step rates, and the cash rate and rounding applicable
   to the proxy. Preserve actual prior capitalization and binding
   election commitments. Admit a partly accrued or previously
   elected parent only if its full resulting amount and state can
   be prepared without overwriting an acquired or paid claim.
   Use a separately qualified source if a cash-equivalent coupon
   schedule cannot be inferred from the contract itself. Reject
   an unresolved proxy, unsupported claim or conflicting definition.
   Remove the clone-wide cash substitution and synthetic stepped
   override. This fit's cash proxy and record 08's no-default loss
   benchmark each need their own stated terms and provenance.
   [Schedules and paid claims](01-schedule-and-holder-cashflows.md),
   [whole-parent cash and PIK](../cashflows/05-coupon-cash-and-pik.md)
   and [structural cash and toggle terms](08-merton-simulation.md).

3. **Admit one qualified settlement price and compare the same claim.**
   Select a clean per-100 settlement price on its explicit
   positive quote denomination or a dirty settlement Money
   amount in the bond currency. Require a positive finite
   settlement dirty target `Q` on the identified cash proxy.
   Compute it from clean using the original parent's qualified
   signed buyer accrued. Validate currency, settlement date and
   phase, paid and detached claims, quote identity and authority.
   This initial fit accepts those two price types. Yield and
   spread inputs require their own qualified proxy conversions.
   Resolve record 02's financing factor `F=D_fin(V,S)>0` and
   compare `J(θ)−F*Q` in native valuation currency. `J(θ)`
   is the same structurally modeled value at `V` of the
   admitted settlement buyer claims for candidate `θ`, on the
   original model origin and snapshot. The holder basis is
   equivalent when `H(θ)` is compared with
   `F*Q+Δ(θ)`, where `Δ(θ)=H(θ)−J(θ)`.
   Preserve actual delivery, survival and retained-cash rights.
   On identical dated cuts with identical rights, `F=1` and
   `Δ=0`. A differing cut requires a qualified selected-model
   bridge. Resolve any model-dependent claim effect for every
   trial and validation run, using common addressed paths for
   shared effects. Freeze a claim difference only when it is
   proved candidate-independent. An unavailable bridge fails
   active calibration. Remove security-factor-only carry,
   arbitrary valuation-entitled yield inputs, and repricing
   from a fresh settlement origin.
   [Accepted bond settlement and financing](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks),
   [market marks and model value](03-pv-models.md),
   [quoted yield ownership](10-quoted-yield.md),
   [spread quote ownership](12-spread-quotes.md)
   and [OpenGamma on issuer discount and settlement financing](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf).

4. **Freeze one dated pricing basis before the first trial.**
   Prepare the cash-equivalent claim, target, exact holder and
   buyer cuts, monitoring horizon, actual ACT/365F grid,
   dated cash discount and separate financing source before
   evaluating the bracket. Prepare the original contractual
   program at the same time. Use one grid containing the exact
   required model events of both programs. Qualify its actual
   monitoring geometry under the selected model before fitting.
   Use source-native checked discount
   factors at actual cash dates and the accepted fractional
   coordinate map at numerical detection knots. An explicitly
   selected flat discount basis stays flat across phases.
   Fit and validation share the same dated basis, structural
   parameters other than the fitted scalar, event preparation
   and applicable comparisons. Contractual pricing uses the
   same dated market snapshot, source laws, structural law,
   information cut and monitored grid, with the actual
   contract's own payment and claim identities. Decline the
   comparison if the two programs require incompatible
   monitoring. Changing discount basis or monitoring only
   after fitting fails the calibration contract.
   Use raw native-currency PV with checked arithmetic for every
   residual. Do not reconstruct PV from a displayed price
   percentage or run nonessential trial diagnostics.
   [Accepted structural pricing and time](08-merton-simulation.md#accepted-decision-for-checked-structural-bond-simulation-and-outputs)
   and [exact bond clocks](02-settlement-and-clocks.md).

5. **Preassign fixed budgets and disjoint random groups.** Require
   an explicit positive work budget, exact root u64 seed or the
   selected qualified seed derivation, explicit pairing and
   execution mode, and declared independent group counts in
   `2..=10,000,000` per active stage, subject to tighter checked
   resource bounds. Keep the accepted BONDS-08 PCG sequence,
   normal and uniform assignment, and ordinary contractual
   pricing stream IDs starting at zero. Reserve stage starts
   `10,000,000` for trial-fit groups and `20,000,000` for
   cash validation groups under the same selected root.
   The trial objective reuses identical fit addresses and
   pairing across candidate parameters. Confirm bounded work
   for endpoint evaluations, solver trials, actual candidate
   recheck, validation and contractual pricing, including all
   legs, grids, applicable nested policy work and peak storage.
   Report actual group and physical leg counts and simulation
   work by stage. Disjoint addressed streams prevent sample
   reuse across stages under this profile. They carry the
   shared PCG statistical and replay limits. Remove automatic
   low-path and pricing counts, silent minimum repairs, seed
   inheritance and automatic stage resizing.
   [Accepted PCG profile](../core-numerics/pseudorandom-and-bridge.md),
   [fixed-budget Monte Carlo](../models-monte-carlo/01-engine-and-estimation.md)
   and [Glasserman and Yao on common random numbers](https://business.columbia.edu/sites/default/files-efs/pubfiles/4261/glasserman_yao_guidelines.pdf).

6. **Use a bounded residual-gated bisection candidate search.**
   Require an explicit finite ordered `0<lo<hi` bracket,
   positive midpoint-evaluation budget and finite positive
   absolute trial PV tolerance in native currency. Check the
   two endpoints in order, propagating the first failed
   preparation or evaluation. An isolated exact-zero endpoint
   proposes that endpoint. Reject two exact zeros as an
   ambiguous bracket. Otherwise require strict opposite signs.
   Use a checked representable interior midpoint and retain
   the observed sign bracket until a midpoint meets the
   inclusive trial PV tolerance. Re-evaluate the actual
   represented candidate on the same trial groups and apply
   the same residual gate. Count the recheck separately.
   Return a contextual error on nonadvancing midpoint,
   exhausted budget without a qualifying candidate, or a
   failed residual recheck.
   Preserve endpoint and actual work diagnostics. Remove
   automatic bracketing, replacement trial prices and
   last-iterate acceptance. Finite-sample barrier objectives
   can jump, and first-passage PV or terminal volatility
   relationships need not be monotone. A sign change and
   vanishing parameter width alone cannot establish a fit,
   exact root or unique economic parameter.
   [Shared scalar numerical and financial boundaries](../core-numerics/brent-root.md)
   and [terminal and first-passage laws](../models-and-valuations-credit/04-structural-default.md).

7. **Require independent cash-equivalent validation before use.**
   Freeze the candidate after its trial gate and run the full
   declared cash-validation budget once on its disjoint groups.
   Reprice the same qualified cash-equivalent buyer target and
   apply a separately supplied finite positive absolute native
   PV tolerance to the actual validation residual. If elected,
   apply a positive native-currency sample half-width gate
   after the fixed validation run, separately from the price
   residual. Report validation PV, signed residual and
   pricing uncertainty computed on independent estimator
   groups, with contributing counts. Reject failed validation
   with the candidate and stage diagnostics. Do not retry,
   reseed, refit or grow budgets in response to a failed check.
   Only the candidate meeting both PV gates is accepted for
   use. Validation qualifies this frozen candidate on the
   declared sample experiment. It supplies neither parameter
   uncertainty nor a second market observation.
   [Accepted fixed-budget inference](../models-monte-carlo/01-engine-and-estimation.md)
   and [NIST on sample mean confidence limits](https://www.itl.nist.gov/div898/handbook/eda/section3/eda352.htm).

8. **Price the contractual bond and qualify its own result.**
   Apply the accepted parameter to the original bond and its
   actual authorized cash, PIK and toggle terms, on reserved
   pricing groups and the same qualified structural, market
   and valuation basis. Use the contractual pricing budget
   and any separately selected price half-width gate.
   Preserve independent election policy terms and apply the
   fitted parameter only where the policy explicitly uses
   that model input. Report the original contractual PV,
   uncertainty and count independently of the two cash
   fit diagnostics. The contractual PV can differ from the
   cash target. An exact all-cash match still uses the assigned
   pricing stage when contractual pricing is requested.
   A standalone reprice of the original bond with the fitted
   parameter and the same qualified grid and run reuses its
   ordinary pricing addresses. Keep a fitted parameter fixed in
   ordinary same-model risk or replay and preserve the
   accepted paired addresses when qualified. An expressly
   selected recalibration scenario prepares its own target,
   sources and fit gates. Do not silently recalibrate to
   the old price inside curve risk or recast the fit as OAS.
   [Accepted bond simulation and outputs](08-merton-simulation.md),
   [paired Monte Carlo risk](../models-monte-carlo/25-finite-difference-greeks.md)
   and [bond curve risk](15-curve-risk.md).

9. **Reconcile entry points, errors and fitted-state reporting.**
   Give native, registered and supported typed callers one
   prepared calibration and pricing contract. A path lacking
   the inputs for an elected calibration must reject it
   explicitly. Remove silent direct-host bypass and the
   registry's ambiguous low-budget residual label. Report
   selected parameter and units, target provenance and
   currency, valuation/settlement phase and sources, bracket
   and controls, trial PV/residual and work, independent
   validation PV/residual and uncertainty, and contractual
   price diagnostics as applicable. Errors identify preparation,
   bracket, fit, validation or pricing failure with the
   affected input or stage. Publish no calibrated success
   or partial requested price after a failed gate. Keep
   quote marks and other accepted measures on their own
   valuation basis. Restore validated terms and admitted
   fitted provenance atomically, carrying exact seed identity
   and supported diagnostics through existing Rust, Python,
   WASM and schema routes. Keep numerical and path state
   Rust-owned. Runtime calibration, source consistency,
   market applicability, risk and host qualification remain
   separate from accepting the method.
   [Shared structural calibration controls](../valuations-framework/shared-contract-controls.md)
   and [pricing lifecycle](../valuations-framework/pricing-lifecycle.md).

## Observed behavior

### Choice and domain

Optional Merton calibration solves one structural parameter against a cash-pay version of the bond, then uses that parameter for full-budget PIK/toggle pricing. It is not a calibration directly to the PIK bond's price. Fixed and step-up coupon specs are accepted by the cash-equivalent helper; amortizers subsequently fail in the MC adapter. Supported targets are clean/dirty price, Japanese simple yield, YTM/YTW and Z-spread; DM/OAS/ASW/I-spread targets error.

### Algorithm and controls

Clone the bond and make its coupon type cash. Convert the target quote at settlement to dirty currency; multiply by settlement DF to obtain target valuation PV. YTM target conversion uses valuation-entitled flows, whereas other quote paths can use settlement entitlement. Build a trial config with a nondefault stepped all-cash PIK schedule so bond PIK defaults cannot override it. Preserve base asset value, payout, risk-free rate, barrier type and dynamics while replacing either debt barrier or asset volatility.

Default barrier bracket is `[0.001V0,0.999V0]`; volatility bracket `[0.01,2]`. Explicit brackets require finite `0<lo<hi`. Evaluate endpoints and require opposite residual signs (exact endpoint match returns immediately). Bisection uses common random numbers, default 2,000 low-path estimators (clamped to at least two), full-run antithetic setting and optional calibration seed. Default maximum is 40 iterations and absolute PV tolerance `1e−4` currency; implementation clamps iterations to at least one and tolerance to nonnegative. Stop only on PV residual, not parameter width. Failure to bracket or converge errors with residual/last parameter.

Return solved model, low-path calibrated PV, target, residual, iterations and parameter. The registry clears calibration and reprices at the ordinary full path budget. Its diagnostic residual is the low-path residual; no full-price revalidation is performed.

### Observed discrepancies and located evidence

The registry calibrates with `base_config` **before** populating term-structure cashflow DFs. With absent DF knots, calibration discounts at the supplied flat horizon-implied rate, while the final price uses curve DFs. Curved-term-structure quotes therefore need not be reproduced by final full-budget valuation even apart from sample noise. Target carry also omits detached-cash terms present in the canonical settlement bridge. These are source-supported mismatches; their numerical impact was not reproduced.

Alternative supported calibration parameter is volatility versus barrier; simultaneous fit, deterministic analytic calibration, CI-aware loss, automatic bracket expansion or full-budget acceptance are unimplemented alternatives. Tight currency tolerances against a discontinuous finite-sample barrier price can be unattainable; error is preferable to treating the last iterate as calibrated, but tolerance/budget suitability is not established.

Owning code: [calibration](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/calibration.rs), [registry ordering](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/pricer.rs), [default calibration spec](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/merton_mc.rs). Inline pricer tests locate calibration failure/term-structure checks; [Merton tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/merton_mc/tests.rs) cover config decoding and shared simulation. These do not independently establish an identifiable structural parameter. [Merton](../../../REFERENCES.md#merton-1974) provides model background, not approval of this low-sample fitting policy.

The current [typed Rust pricing adapter](../../../../finstack-quant/valuations/fixed-income/src/bond/types/traits.rs)
reaches the simulator directly. Its
[Python caller](../../../../finstack-quant-py/src/bindings/valuations/instruments.rs)
and [WASM caller](../../../../finstack-quant-wasm/src/api/valuations/typed/fixed_income.rs)
can supply a configuration with calibration while this price
route leaves it unused. Reconcile all active configurations
through the shared checked preparation boundary.

## Independent arithmetic and qualification

- Initial face 100 with already capitalized PIK of 5 gives
  principal 105. The next qualified cash coupon of 8% over
  half a year is 4.20. Rewriting historical PIK would produce
  4.00. Separately, a clean quote 98 per 100 on denomination
  105 and signed accrued −1.40 give `Q=101.50`. Finance at
  .99 for a buyer-value target of 100.485. A separate
  vested holder receipt of 6 discounted at .98 gives
  `Δ=5.88` and equivalent holder target 106.365.
- A year-one coupon of 20 and year-two principal 100 or
  recovery 40 with default probability .50 give PV 75.70
  at dated factors .95 and .81. A flat proxy with the same
  year-two factor gives year-one factor .90 and PV 74.70.
  Matching 75.70 on that flat basis implies default probability
  `233/486`, and repricing that probability on the dated
  basis gives 76.70.
- With two sample terminal assets 90 and 110, survivor cash
  100 and recovery 40, the inclusive barrier test gives PV
  100 just below barrier 90 and PV 70 at barrier 90. Target
  85 has opposite endpoint residual signs on bracket [80,100]
  and no observed fit within a tolerance below 15. Separately,
  with `log(V0/B)=1/8`, `r_m−q=−1/4`, and a one-year
  terminal horizon, volatilities .25 and 1 both give
  standardized distance `d2=−.625` and the same ideal PD.
- Trial antithetic group means of 80 and 90 match target 85.
  Disjoint validation group means of 80 and 70 give PV 75,
  residual −10 and group standard error 5. A validation
  tolerance of 2 fails despite exact trial fit.

Qualify source reuse on curved terms, seasoning and settlement
carry, jump and plateau rejection, endpoint and budget checks,
independent validation, contractual cash versus PIK replay,
frozen-parameter risk and parity at the actual Rust and host entry
points. Compare structural probabilities and finite-sample
calibration error independently.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
