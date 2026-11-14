# VALUATIONS-BONDS-18: Holder embedded-option value and short-rate model vega

**Decision status:** Accepted — complete checked bond embedded-option package, including `embedded_option_vega` in native currency, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, model, market and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond embedded-option value and vega

1. **Define the live-rights comparison on one holder basis.**
   Bind the selected qualified model and dispatch, original valuation
   date `V`, information and event phases, holder and contractual
   rights, currency and amount. At the same admitted pricing basis,
   report `embedded_option_value=H_live−H_disabled` in signed native
   holder currency. `H_live` keeps the admitted remaining discretionary
   issuer calls and holder puts, with their make-whole and return-floor
   exercise terms. `H_disabled` removes those choices and their
   dependent exercise protections. A return floor contributes through
   its authorized exercise clause. Preserve irrevocable notices and
   exercise obligations, actual payable and retained claims, historical
   state, contractual coupon and principal economics, and unchanged
   entitlement and recovery law. Co-dated cash and rights retain their
   phases and identities. Changes in future branch cash or exercise
   are consequences of disabling the live rights. Do not cancel a
   vested payment or create a new maturity path for a locked redemption.
   If no qualifying discretion remains after lifecycle validation,
   both measures have a structural zero and need no inactive option
   inputs. A live right can have zero value or zero vega on particular
   inputs without establishing structural absence. Missing history
   or an unsupported live right cannot produce a structural zero.
   [Bond holder cash](01-schedule-and-holder-cashflows.md),
   [exercise and straight comparator](04-rates-tree-exercise.md),
   [return-floor rights](05-return-floors.md) and
   [CFA Institute on the option-free bond comparison](https://www.cfainstitute.org/insights/professional-learning/refresher-readings/2026/valuation-analysis-bonds-embedded-options).

2. **Prepare comparable raw reprices under one selected model.**
   Reuse the actual selected Tree or RatesCredit profile, or an
   equivalent registry route explicitly qualified to deliver the
   same raw holder pair. Keep the matched event grid, dates, model
   clock, parameters, pricing geometry, discount, projection,
   hazard, recovery and make-whole bindings, where relevant to the
   surviving contract. Feed both claims through the same valuation
   law. The disabled leg may skip unnecessary exercise training
   or use a specialized evaluation of the same prepared law when
   its equivalence is qualified. Dropping rights cannot select a
   different model, time grid or claim treatment. A selected dispatch
   without a qualified matched reprice fails the explicit request.
   Under exact comparable exercise laws, an issuer-call-only right
   that can be left unexercised gives a nonpositive holder difference,
   and an analogous holder-only put gives a nonnegative difference.
   Report combined rights net after the actual exercise ordering
   and applicable friction. No addition of separate standalone
   option results or clipping of value or vega signs. Use these
   controlled signs for validation, with qualified sampling or
   policy approximations identified.
   [Model dispatch and quote authority](03-pv-models.md),
   [bond cash and joint recovery](06-joint-coupon-replay.md) and
   [exercise policy](07-lsmc-policy.md).

3. **Bind the model spread once on the original optional claim.**
   A model-centered experiment uses the authorized model state,
   authored spread on its admitted law, or the explicitly admitted
   unspread center. For an observed-price anchor, resolve the
   authorized quote once to settlement dirty `Q` and qualify the
   live-claim model spread at the original valuation origin under
   BONDS-12/15. Verify `J(s)−F Q` on the actual represented spread,
   with modeled buyer value `J` and the admitted financing factor
   `F`. Report raw valuation-date holder values on both legs.
   A settlement price is an anchor and a provenance item. Hold the
   spread value and law across the disabled leg and all sigma
   scenarios. Remove the active price short-circuit in isolated
   reprice views. No separate straight-leg spread fit, refitting
   OAS on volatility moves, blind reuse of a displayed scalar,
   restart at settlement, or zero fallback after a failed fit.
   A literal spread needs its own qualified model price basis.
   [Settlement bridge](02-settlement-and-clocks.md),
   [selected-model OAS](12-spread-quotes.md) and
   [quote-reproducing holder center](15-curve-risk.md).

4. **Shock one actual native scalar short-rate sigma.** Elect the
   complete compatible authored or fitted parameter block. Tree
   supports the accepted scalar Hull–White, BDT and BK profiles.
   Hull–White and the RatesCredit rate axis use absolute decimal
   short-rate sigma. BDT and BK use log-rate sigma of the elected
   Simple short-rate quote. Each is annualized per square root of
   an ACT/365F model year. Keep the exact model axis and units in
   the result. Scheduled Hull–White vol, Ho–Lee, implied option
   vol, credit vol and direct or recalibrated market-vol shock lie
   outside this scalar experiment. Require valid explicit sigma,
   including zero where admitted. Hold mean reversion, hazard vol,
   admitted dependence, target curves, historical state and other
   selected parameters fixed. Rebuild affected tree shifts, joint
   marginal fits and model projection or exercise policy on the
   same admitted targets. Keep the original fitted block as
   provenance and require each changed-state fit to qualify. Do
   not refit sigma or other model parameters to calibration quotes.
   [Bond short-rate models](04-rates-tree-exercise.md),
   [model control and fit identity](../valuations-framework/model-resolution.md)
   and [joint rates-credit coupling](../models-numerical/rates-credit-coupling-and-sampling.md).

5. **Use represented sigma moves and a dedicated currency measure.**
   Require an explicit finite positive calculation step `h` on
   the selected sigma axis. Precheck the base and complete requested
   endpoints against the owner's domain. For valid represented
   `σ−<σ0<σ+`, write `E(σ)=H_live(σ)−H_disabled(σ)` and report
   `embedded_option_vega=.01[E(σ+)−E(σ−)]/(σ+−σ−)`.
   When the full lower move crosses the admitted lower sigma
   boundary, use the complete admitted up move and same raw center,
   with `.01[E(σ+)−E(σ0)]/(σ+−σ0)`. An exact zero lower endpoint
   is included when compatible with the chosen pricing, spread
   and coupling laws. Require strict represented motion and a
   finite checked result. A rounded or collapsed move, missing
   binding, incompatible state transition or failed model fit
   fails the stencil. Remove the `1e−8` floor, automatic shrink,
   epsilon-based zero and unexpected one-sided fallback. For
   joint RatesCredit reject a move that changes a one-axis
   state into a two-axis state, or a two-axis state into a
   one-axis state. A deterministic-to-rate-only transition
   with zero hazard sigma needs a qualified common spread law and
   exact event/draw identity. Sampled Continuous-only OAS cannot
   be substituted for a selected Semiannual Periodic spread.

   Report signed native currency per `+.01` of the named model
   sigma on the whole bond amount. This equals 100 bp of absolute
   short-rate volatility on the Hull–White rate axis, or one
   percentage point of log-rate volatility on the BDT/BK axis.
   The axes remain economically distinct. The calculation width
   is separate from the reporting increment. Retire the bond's
   legacy `vega` registration and its price-points-per-100 scale.
   Total bond PV vega and market-volatility sensitivity are
   separate experiments. No separate per-100 result is required
   by this profile.
   [Represented rate and option stencils](../valuations-framework/option-sensitivities.md)
   and [joint bond rollback and spread law](06-joint-coupon-replay.md).

6. **Pair the full comparison for sampled models.** Use the model
   owner's fixed work and specified stage, group, antithetic leg,
   node and draw addresses. Use matched addresses for the live and
   disabled center and for all four values defining a volatility
   difference. Refit affected benchmark and exercise policies
   per scenario on matching logical training stages. Freeze each
   valid fitted policy and price on matched disjoint pricing
   addresses. Unneeded exercise stages keep their reserved slots
   vacant. A qualified exact deterministic value can be repeated
   across groups on the same basis. For each pricing leg form the
   optional-minus-disabled difference for value, or the difference
   of the two paired rights differences for vega. Convert the latter
   using the checked represented width and reporting increment.
   Reduce an antithetic pair into one independent observation. Use
   the admitted conditional sample error and Student-t interval on
   these matched group contributions. A separately elected positive
   half-width gate applies to the complete fixed run in the reported
   metric's units. It does not edit the budget or step. A common seed
   alone and subtraction of standalone leg intervals cannot establish
   this uncertainty. Keep policy, calibration, grid, finite-step and
   model error separate. Reusing an OAS fit sample supplies conditional
   run diagnostics without an independent coverage or fit-validation
   claim. Fail incomplete or unqualified pairing and propagate leg
   failures.
   [Bond sampling and fit stages](07-lsmc-policy.md),
   [paired risk estimator principles](../models-monte-carlo/25-finite-difference-greeks.md)
   and [Glasserman and Yao on common random numbers](https://doi.org/10.1287/mnsc.38.6.884).

7. **Publish the matched experiment and units through existing consumers.**
   Record retained and disabled rights, scenario and quote provenance,
   selected model/axis and clock, actual endpoints and stencil,
   currency, amount and uncertainty where applicable. Keep monetary
   value and vega signed, applying external position weight once.
   Aggregate only identical authorized economic factors, units and
   matched valuation bases. Do not feed embedded model-sigma vega into
   observed implied-volatility attribution. Reconcile the Rust registry,
   catalog, schemas, Python, WASM, reports and UI with the distinct
   key and currency units. Preserve independent metrics, distinguish
   genuine zero, inapplicable optional results and explicit failures,
   and share only identical qualified state. Other products retain
   their own option-value and vega contracts.
   [Public metric identity](../valuations-framework/0010-metric-identifiers.md)
   and [shared result basis](../valuations-framework/metric-orchestration.md).

## Observed behavior

### Choice, scope and controls

Embedded option value is the holder's net value of call/put/return-floor rights under the chosen model. Resolve fixed OAS from computed OAS, a quote-based solve, or zero; remove price drivers. Price the original optional bond and a clone with only call/put and return floor removed, keeping other model inputs and dispatch. Return `P_optional−P_straight` in currency; no rights returns zero. A call-only right normally gives negative holder value, a put-only right positive, while combined rights report net value.

### Vega algorithm

Vega is the sensitivity of that **option-value difference**, not total bond PV, to the selected short-rate volatility. Optional bonds require Tree or RatesCredit; other models error. RatesCredit reads `hw1f_sigma` (default zero). Rates-only trees resolve Hull–White scalar sigma/mean reversion, BDT sigma or BK sigma. Ho–Lee is rejected. Hull–White mean reversion is pinned on bumped clones so only sigma changes; the straight leg retains bumped rate volatility and all credit inputs.

With configured absolute decimal vol bump `b`, use `σ_up=σ+b`, `σ_down=max(σ−b,1e−8)` and the actual width. Return `(V_up−V_down)/notional·100/width·0.01`, price points per one absolute volatility percentage point. Effectively zero width returns zero. A small/zero sigma yields an asymmetric secant, not a symmetric derivative. Normal-rate sigma and lognormal-rate sigma are different model parameters despite this shared reporting unit.

### Limits, alternatives and evidence

Fixed OAS is calibrated only once; no market-volatility surface is shocked/recalibrated. Straight-leg subtraction removes model-rate volatility exposure unrelated to exercise. For joint simulation, differences can include sampling/regression error; no vega confidence interval or automatic bump-convergence check is produced. Total-PV vega and implied-market-quote vega would be distinct additional metrics.

Owning code: [embedded value](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/embedded_option_value.rs), [vega](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/vega.rs), [tree resolver](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/config.rs). Inline vega tests locate preservation of joint-model inputs on the straight leg, saturated marginal behavior and equivalent clean/dirty quote normalization. These are model consistency tests, not independent vega accuracy. [Hull–White 1990](../../../REFERENCES.md#hull-white-1990-pricing-ird) and [numerical procedures](../../../REFERENCES.md#hull-white-1994-numerical-procedures) supply model background.

The [shared vega definition](../../../../finstack-quant/valuations/core/src/metrics/ids/equity_volatility.rs)
uses a currency amount. The live bond path emits price points per 100
under that key, which is also read by
[implied-volatility attribution](../../../../finstack-quant/attribution/src/metrics_based/volatility.rs).

## Independent arithmetic and qualification

- In a toy two-state tree, the later continuation is `90` or `110`
  with equal weights and discount `.99`. An issuer call caps it
  at `102`, and an earlier holder put pays `97`. Add the same
  independent PV `6` to each root value. The disabled, call-only,
  put-only and both-rights values are `105`, `101.04`, `105` and
  `103`. The combined holder difference is `−2`. The sum of the
  individual differences would be `−3.96`.
- With `H_live(σ)=980+20σ−50σ²` and
  `H_disabled(σ)=1000+30σ`, a `.01` step and reporting increment
  at `σ=.02` give embedded vega `−.12` in currency. The live
  leg alone gives `+.18`. At `σ=.004` the full up move and base
  give `−.109`. At `σ=.01`, the exact zero lower endpoint and
  the full up endpoint give `−.11` on this smooth fixture.
- With actual width `.02`, three paired group tuples
  `(H_live+,H_disabled+,H_live−,H_disabled−)` of
  `(104,110,102,106)`, `(152,160,149,154)` and `(200,210,196,202)`
  give reported observations `−1`, `−1.5`, `−2`. The mean is
  `−1.5`, standard error is `.288675134595`, and the two-sided
  95% Student-t half-width with two degrees of freedom is about
  `1.2420688559` native currency per `.01` sigma. This synthetic
  example illustrates estimator arithmetic, not lattice or fit accuracy.

Exact rationals reproduce the rollback, represented sigma stencil
and paired group examples. A 70-digit Student-t calculation checks
the reported half-width.

Qualification covers recognized exercise and notice history, matched
straight pricing, entitled and retained cash, combined rights and
return-floor/payoff dependencies, original-origin spread anchoring,
all selected sigma units and endpoints, affected fits, correlation
and spread domain transitions, paired group and budget controls,
finite-step and grid refinement, and signed quantity and host parity.
Require independent model-price and risk references before claiming
production accuracy or market agreement.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
