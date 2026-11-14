# VALUATIONS-BONDS-17: Recovery01 and immediate jump-to-default on hazard-priced bonds

**Decision status:** Accepted — complete checked bond recovery and jump-to-default package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, legal-claim, replay, model and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond recovery and jump-to-default

1. **Bind the actual holder, credit exposure and valuation phase.**
   Require the recognized alive bond claim at the elected valuation
   date `V` and information and event cuts. Bind the precise issuer
   or covered credit subject, holder entitlement, actual selected
   prompt fractional-recovery-of-par (FRP) model, typed active
   intensity and discount sources, and the exact authorized recovery
   fraction `R∈[0,1]`. A calibration source ID alone supplies neither
   issuer nor contractual recovery authority. Use the admitted
   HazardRate or RatesCredit profile, or a registered equivalent
   with a qualified claim and risk-reprice contract. A configured
   unused credit source cannot select these metrics. Unsupported
   rates-only, structural Merton, actual known-default receipts and
   delayed or otherwise different default payoffs need their own
   contracts. Missing, conflicting, wrong-kind or invalid required
   data fails. Remove recovery clamping. Keep native holder currency
   and the calibrated market separated from position weighting.
   [Selected bond model and recovery](03-pv-models.md),
   [joint bond recoverable par](06-joint-coupon-replay.md) and
   [Duffie and Singleton on recovery specifications](https://web.stanford.edu/~duffie/jstorlinks/ds2.pdf).

2. **Use one qualified raw holder center and preserve its authority.**
   Measure the selected model at original `V`, on the same recognized
   claim, amount, history and dispatch used in the experiment. A
   model-centered request uses its admitted inputs. For a price-anchored
   request resolve the original quote to settlement dirty `Q` and
   qualify the selected-model anchor through BONDS-15 on the base
   snapshot. Check the fitted spread on `J(s)−F Q`, with buyer claim PV
   `J` on the original valuation origin and the authorized settlement
   financing factor `F`. Use raw holder PV `H(s)` for the holder
   center, including the properly valued holder–buyer difference.
   A valid numerical mark alone supplies no model or cash decomposition.
   Hold that model spread and its law fixed across Recovery01 reprices.
   Prepare the same raw no-move center for a one-sided calculation.
   Do not subtract a naked clean or dirty settlement quote, use the
   rounded context mark, reset `V` to settlement, or refit the bond's
   spread after a recovery move. Retain the quote, fit residual and
   holder model authority separately.
   [Settlement and retained claims](02-settlement-and-clocks.md),
   [selected-model OAS](12-spread-quotes.md) and
   [quote-reproducing holder center](15-curve-risk.md).

3. **Elect one explicit Recovery01 experiment.**
   Report signed native holder currency per `+.01` absolute recovery
   (one percentage point) and identify the actual mode. A lossless
   recipe enables replay and does not choose the mode.

   - **Frozen intensity.** Preserve the active intensity, source
     clock and conditional survival law. Change the chosen future
     bond recovery cash binding. For an independent contractual
     recovery, the calibration source and fit remain valid. If the
     authored bond binding reads source recovery metadata, change
     it in an isolated admitted view, preserving intensity and
     applying the change to every authorized linked reference. Its changed metadata
     carries no current-fit authority. For an admitted deterministic
     affine FRP claim use the exact coefficient `.01 Σ_i N_i K_i`,
     with `N_i` the covered principal and `K_i` the selected model's
     discounted covered default mass at unit recovery. It retains
     the elected spread where pricing uses it. Compute from the
     covered local windows, including at `R=0` and `R=1`. No full
     +.01 scenario or division by current recovery is required. If
     recovery changes exercise or later exposure, use checked full
     reprices under point 4 at the frozen intensity.
   - **Quote-preserving.** Reproduce the original hazard and qualify
     its unchanged observations using the exact construction
     record, calibration discount, recovery bindings, conventions,
     provider and source/fit checks. Hold the original observations
     in their authored forms, including upfront with fixed coupon
     where specified. Keep the discount market fixed. Shift the
     elected bond cash and calibration recoveries coherently and
     rebuild each affected hazard. Explicitly resolve independently
     authored bond and calibration recoveries. An admitted common
     bump shifts the bound values from their respective centers and
     preserves any admitted offset. A shared source is replayed once
     when the complete bindings agree. Reject conflicts or unapproved
     scope changes. A missing recipe or failed fit fails this mode.
     Use the replay owner's actual stricter domain and require a
     qualified center. In particular, a zero-LGD `R=1` calibration
     cannot be rescued by an otherwise admissible down move.

   Keep realized recovery and settled cash fixed in both modes.
   Zero covered recovery exposure can give zero frozen recovery
   sensitivity. In quote-preserving mode changed survival can still
   move live contingent coupon PV after recoverable par is zero.
   [Shared recovery sensitivities](../valuations-framework/credit-sensitivities.md),
   [hazard recovery replacement and recalibration](../core-market/hazard-quotes-and-risk.md),
   [lossless replay](../core-market/calibration-provenance.md) and
   [White on recovery and value on default](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf#page=37).

4. **Check full recovery moves and rebuild only affected live state.**
   Where the mode needs finite differences, require an explicit
   finite positive step `h` in absolute recovery fraction. Validate
   the base and the two complete trial vectors before any scenario
   fit. If both full directions are in domain, report
   `.01(H+−H−)/(r+−r−)` using the checked represented scalar width
   or corresponding declared common vector amplitude span. If exactly
   one full direction crosses a recovery bound, use the entire other
   move and the same qualified raw center. For the up direction use
   `.01(H+−H0)/(r+−r0)` and for the down direction use
   `.01(H0−H−)/(r0−r−)`, with the matching width convention.
   If neither full direction qualifies, or a required represented
   move collapses or reverses, fail. Select the one-sided stencil
   only from the validated domain. Missing data, fit failures and
   arithmetic errors do not select a stencil. Remove implicit step,
   memberwise caps and floors and fabricated zero responses.
   Rebuild affected credit fit, contingent exposure and future
   stopping policy under the same selected model, preserving
   historical events and unselected inputs. Match logical random
   addresses across applicable samples, refit affected scenario
   policies on matched training groups and evaluate on disjoint
   matched pricing groups. Report any sampled uncertainty on the
   paired raw differences under the admitted fixed budget, with
   actual steps, sign, units and model/fit assumptions.
   [Checked represented stencils](../valuations-framework/finite-difference-rates.md),
   [joint policy and sampling](07-lsmc-policy.md) and
   [paired bond risk](15-curve-risk.md).

5. **Make the hypothetical default phase and covered par explicit.**
   `jump_to_default` inserts an immediate hypothetical issuer default
   at the elected valuation phase on `V`. Resolve same-day priority
   against payment, principal events and binding exercise or notice
   before valuing the stress. Freeze completed events and recognized
   rights. Build nonnegative recoverable par `N_rec` from the
   authoritative covered claim and paired events under BONDS-03/06.
   Apply authorized capitalization and write-down on their economic
   dates and retire covered repayable par at the authorized actual cash
   phase. A lagged covered repayment remains exposed up to that phase.
   Resolve past-due or failed cash under its own authorized treatment.
   Do not infer exposure from nominal face, a cash row alone, the
   economic balance or end-of-day event order. Preserve the actual
   entitlement and payment phases for independently qualified
   supplied schedules. The prompt modeled recovery cash is
   `R N_rec`, paid at the event with factor one. It includes no
   accrued interest, redemption premium, spread discount or modeled
   probability weight. The event terminates the affected contingent
   rights. Preserve unchanged legally default-independent rights.
   Entitlement or coupon detachment alone does not establish default
   independence. Apply the authorized contractual rule if an already
   committed right survives. An unresolved survivor, phase or
   stressed cash law makes the requested result unavailable or fails
   the explicit request under its contract.
   [Bond cash and event semantics](01-schedule-and-holder-cashflows.md),
   [recovery versus economic principal](02-settlement-and-clocks.md),
   [exercise phases](04-rates-tree-exercise.md) and
   [joint covered recovery](06-joint-coupon-replay.md).

6. **Report the signed economic jump on matching holder claims.**
   Qualify the raw center decomposition `H0=H_terminated+H_surviving`
   on the actual selected model, before rounding. With prompt FRP
   and unchanged surviving values, the stressed holder value is
   `H_default=R N_rec+H_surviving`. Report native currency
   `jump_to_default=H_default−H0=R N_rec−H_terminated`. Use checked
   decomposition and arithmetic. Treat gross recovery cash and
   economic jump as distinct quantities. Coupon, principal and
   exercise values that the event terminates belong in
   `H_terminated`. Do not count modeled recovery on that same
   terminated claim again in the stressed value. Permit any signed
   finite jump. Zero recoverable par can still leave a nonzero jump
   on default-contingent coupons. Proved extinguished exposure can
   give a structural zero after validation, even with unrelated
   default-independent cash still due. Do not fabricate a second
   jump for a known default or require CDS replay provenance solely
   to evaluate an admitted event. The measure is the specified
   economic event P&L. Regulatory DRC and expected loss use their
   own definitions.
   [Credit event P&L and surviving cash](../models-and-valuations-credit/13-cds-and-index-risk.md),
   [White on value on default](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf#page=37)
   and [Basel gross and net default risk definitions](https://www.bis.org/basel_framework/chapter/MAR/22.htm).

7. **Keep metric identity, mode and quantity across consumers.**
   Retain `recovery01` and `jump_to_default` as bond metric keys.
   Bind one Recovery01 mode per use of that key. Distinct modes
   require distinct authorized result identities or separate
   requests. Carry mode, source and claim basis, date/phase, actual
   stencil or exact coefficient, currency, and quote/model provenance
   through the metric registry, portfolio, attribution and reporting.
   Apply external signed quantity once. Aggregate only compatible
   risk factors, recovery modes and event definitions. Recognize
   genuine zero, inapplicable optional fields and failed explicit
   requests separately. Reuse checked Rust preparation and reconcile
   Python, WASM, schemas, UI and shared metadata. Preserve other
   instruments' recovery and default contracts.
   [Metric authority and result basis](../valuations-framework/metric-orchestration.md)
   and [signed amount conventions](../conventions/0007-units-and-sign-conventions.md).

## Observed behavior

### Choice and scope

These metrics require an explicit hazard curve and active HazardRate/RatesCredit model; other models error, including structural Merton. They use the hazard model's fractional recovery of outstanding par without accrued interest. Bond notional is positive; short exposure comes from negative portfolio quantity. Recovery and hazard-rate recalibration policy is delegated to the common Recovery01 implementation.

### Algorithm and controls

Recovery01 first builds the quote-reproducing OAS view, then reprices with a requested absolute step of `0.01`. The shared helper clamps moves to `[0,.999]`, chooses central or one-sided from the resulting widths, and normalizes to a one-percentage-point recovery change. Widths at or below its threshold can return zero. The one-sided route uses `context.base_value` at the center while its side uses the raw repricing callback. A hazard curve with calibration provenance is recalibrated to unchanged par quotes at the changed recovery. A hand-built curve keeps hazard rates fixed. Repricing retains selected model and registry. The bond discount ID supplies the helper's discount dependency.

Jump-to-default is `R·N(as_of)−base_value`, in currency. Clamp hazard recovery to [0,1], build the full schedule and its cash-date recovery-principal replay, then take outstanding **after** events on `as_of`; no future discount or accrued claim is included. The subtraction uses the actual current context base value, including a price quote. Negative values indicate holder loss on immediate default. Principal during repayment lags follows the special recovery path of record 02.

### Alternatives, limits and evidence

Frozen-hazard and recalibrated-hazard Recovery01 are both implemented but selected from curve provenance, not an independent metric switch. Market-value recovery, accrued recovery and Merton structural-default sensitivities would be other methods. JTD measures a discrete jump, whereas Recovery01 is a conditional model sensitivity; neither is expected loss or CS01.

Owning code: [bond metrics](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/default_risk.rs), [shared recovery helper](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/recovery01.rs), [cash-date principal replay](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/settlement.rs). [Default-risk tests](../../../../finstack-quant/valuations/fixed-income/tests/default_risk.rs) locate quote/recovery identity and nonhazard rejection; [recalibrated fixtures](../../../../finstack-quant/valuations/tests/instruments/bond/metrics/default_risk_recalibrated.rs) distinguish recalibrated versus frozen hazard. Evidence located, not executed. Reference: [Hull–White credit](../../../REFERENCES.md#hull-white-2000-cds).

Consumer reconciliation reaches the [credit metric definitions](../../../../finstack-quant/valuations/core/src/metrics/ids/credit.rs)
and the [Python](../../../../finstack-quant-py/tests/test_valuations_wasm_parity.py)
and [WASM](../../../../finstack-quant-wasm/tests/facade/valuations_parity.test.mjs)
host parity fixtures.

## Independent arithmetic and qualification

- A one-year toy FRP bond has recoverable par `100`, promised cash
  `108`, flat discount `.035`, recovery `.40` and flat intensity
  `.05`. A matched continuous-premium CDS toy par observation of
  `.03` implies `λ=.03/(1−R)`. This identity belongs to the toy
  premium/protection laws. The raw bond center is `101.116684729441`.
  Frozen-intensity Recovery01 is `+.047933950352`. Holding the par
  observation fixed and refitting at `R±.01` gives `−.003564700364`.
  At a toy center `R=.995`, step `.01` crosses the upper bound.
  Using the full lower direction at `.985` and the qualified raw
  center gives one-sided `+.675927737915`.
  Replaying actual dated CDS observations needs full premium,
  protection, accrued and fit checks.
- An economic amortization of `20` has already reduced economic
  principal to `80`, but the covered repayment is still unpaid
  before its authorized cash phase. In a synthetic pre-payment
  stress, recoverable par is `100`, terminated PV `91` includes
  the still-contingent `20`, and default-independent PV is `6`.
  At `R=.40`, `H0=97`, stressed holder value is `46`, and JTD
  is `−51`. After payment, par `80`, terminated PV `71` and the
  same independent PV give `H0=77`, stressed `38`, and JTD `−39`.
- A qualified base bridge with observed dirty `90`, funding factor
  `.98` and holder–buyer value difference `6` yields raw holder
  value `94.2`. If the `6` is proved default-independent and the
  rest terminates, immediate recovery `40` gives stressed holder
  value `46` and JTD `−48.2`. At zero recoverable par, a still-live
  default-contingent coupon PV of `3` gives `−3`.

Independent 70-digit FRP calculations reproduce the recovery mode
and boundary examples. Exact rational checks reproduce the event
phase, quote bridge and zero-par jump arithmetic.

Qualification must cover original upfront and par quote replay,
distinct recovery and calibration bindings and discount roles,
zero-LGD and both-boundary controls, affine recovery and option
response, paired sampling, matched holder/dirty mark bridges,
authored event phases including lagged repayments, recognized PIK
and write-down, retained versus terminated coupons and commitments,
actual-default exclusion, true zero vs live coupon-only exposure,
signed positions and host parity. Arithmetic fixtures do not
certify live model or legal claim behavior.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
