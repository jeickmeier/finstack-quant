# VALUATIONS-BONDS-03: PV selection, quote pinning, discounting and fractional recovery of par

**Decision status:** Accepted — complete checked bond PV selection and fractional recovery of par package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
**Validation status:** Source, published references and independent arithmetic reviewed. Runtime, market, model and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond PV selection and fractional recovery of par

1. **Select one exact valuation model.** Preserve an explicitly
   requested Bond model in the selected registry. For a native
   request with no explicit model, an authored joint-factor
   election or exercise rights with an authored hazard binding
   select RatesCredit. Hazard binding alone selects HazardRate,
   exercise rights alone select Tree, and a straight bond
   with neither selects Discounting. MertonMc requires explicit
   selection. Only typed economic inputs can elect a native
   branch. Generic implied volatility, unrelated tree controls
   and market inventory do not select a joint model. An incomplete
   or incompatible selection fails. Discounting and HazardRate
   reject embedded calls, puts and return-floor rights. Explicit
   rates-only Tree or Discounting can leave a declared hazard
   source inactive, with its roles and request provenance
   preserved. Incompatible active controls error. Keep the
   exact tree family, exercise mechanics and stochastic joint
   and Merton choices with their owners. The rates-only tree
   uses an explicitly selected subtype under the shared model
   contract.
   [Accepted pricing lifecycle](../valuations-framework/pricing-lifecycle.md),
   [Tree model controls](../valuations-framework/model-resolution.md),
   [rates-only exercise](04-rates-tree-exercise.md),
   [joint replay](06-joint-coupon-replay.md),
   [Merton](08-merton-simulation.md)
   and [callable bond overview](0028-callable-bond-model-selection.md).

2. **Elect a model price or a settlement-quote mark.** A selected
   model PV comes from its admitted raw pricing path on the
   prepared holder cut. An explicitly selected clean, dirty,
   yield or other supported settlement quote can instead produce
   a quote-derived mark through its exact law and the accepted
   financing and claim bridge. Validate the contract, route,
   price-driver exclusivity, units and authority before valuing.
   Resolve only inputs required by the selected purpose. A
   quote-derived mark can have a qualified bridge without running
   a numerical model or resolving its inactive inputs. Label
   the source quote and derived mark. A quote-only path supplies
   no unevaluated model diagnostics, recovery or sensitivity.
   An explicitly requested same-model risk or calibration
   experiment must obtain a qualified model center and actually
   evaluate it. Preserve original quote identity and any
   source-scope restrictions. Do not restamp the result as a
   synthetic clean-price input or restart the model on the
   settlement date with the original information snapshot.
   [Accepted settlement and quote carry](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks),
   [quoted yields](10-quoted-yield.md)
   and [spread definitions and inverses](12-spread-quotes.md).

3. **Price Discounting on the prepared signed security cash.**
   Use the elected source and its checked positive native
   dated relative factors `D(V,T)`. Include eligible known
   same-day cash at factor one on an explicitly entitled cut.
   Use the signed amount for each eligible holder claim, including
   genuine negative settlements. Extinguished and excluded claims
   need no inactive market inputs. Discount each amount once,
   accumulate checked raw PV in defined order and restore
   Money at the final price boundary. Share exactly that basis
   across native, registered, raw and finalized prices. A
   discount-only price applies no additional survival weighting.
   The settlement financing evaluator retains its separate role.
   [Accepted schedule and holder cash](01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash),
   [dated NPV](../core-conventions/016-discounting.md)
   and [security and financing discounting](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf).

4. **Apply one admitted spread in its own model or quote law.**
   Treat supplied OAS as a signed annual decimal model input.
   Under Discounting, require explicit Continuous or nominal
   semiannual periodic quotation for an active spread. Use the
   source's valuation-relative coordinate
   `t=c_source(T)−c_source(V)` with full clock context.
   Continuous uses `log D_s=log D−s t`.
   Periodic uses the exact additive shift of the implied
   semiannual zero rate under the checked core law. Accept
   signed rates, spreads and factors above one where admitted.
   Simple compounding needs a separate finite-horizon law.
   A genuine zero spread retains the admitted unspread
   factor and an entitled same-date cash factor stays one.
   No implied rate inversion at zero time. Refuse an unresolved
   active spread/time basis. HazardRate accepts continuous OAS
   on ACT/365F and applies it to both promised and recovery
   terms. The security spread holds projections, hazard and
   settlement financing fixed. Other model owners supply their
   own admitted spread law. Keep the current scenario spread control only as an
   expressly selected Discounting settlement Z-spread experiment.
   Add its signed bp move once to the admitted Z center on the
   same quote/carry basis. Zero shock preserves that center,
   and an unshocked model-price request keeps its model basis.
   Reject a conflicting pin, missing qualified center or wrong
   model. An OAS or credit shock is a separate experiment.
   [Accepted spread arithmetic](../core-numerics/compounding.md)
   and [bond spread ownership](12-spread-quotes.md).

5. **Define the pre-default deterministic hazard claim.** Require
   recognized alive-state authority for the elected valuation
   basis, an exact typed default-free discount role, a matching
   issuer pricing-intensity source and a qualified recovery
   fraction `R∈[0,1]`. A source recovery field can supply
   that value only by an authorized matching calibration or
   contract election. Reject missing or invalid active data
   without clipping recovery or survival. For eligible
   default-contingent scheduled cash `C_i` at `T_i`, use
   `C_i D(V,T_i) exp(−J(V,T_i)−s τ_i)`, where `J` is
   local integrated hazard on its source clock and `τ_i`
   is ACT/365F from valuation. Use the prepared entitlement
   cut. Entitled same-day cash uses the admitted pre-default
   phase with discount and conditional survival equal to one.
   A legally distinct unconditional or known-default claim requires its own
   admitted present-value treatment. A projected survival
   rounded to zero supplies no observed default event.
   [Local hazard survival](../core-market/hazard-survival.md)
   and [Duffie on fractional recovery of face](https://web.stanford.edu/~duffie/par.pdf).

6. **Select prompt recovery on recoverable par.** Under this
   straight-bond approximation pay `R N` at modeled default,
   with nonnegative `N` established by the explicit
   claim and its event coverage. No accrued interest,
   redemption premium or market-value component enters this
   payoff. Use a separate recovery exposure view. Qualify
   cash-date retirement for repayable par under this
   profile, carrying the eligible claim until repayment
   cash is due under the selected timing. Use economic dates
   for authorized capitalization and write-down events.
   Preserve after-event exposure for the next interval.
   Require complete dated stock, no inferred face from
   payments and no mutation of the economic principal ledger.
   Stop modeled recovery at the end of the authorized
   exposure, which can differ from the last coupon date.
   Later eligible contingent coupons can still need survival
   and discounting after recoverable par reaches zero. A
   proved extinction permits inactive-source shortcuts.
   Absence of future promised cash alone cannot establish it.
   Actual defaulted or failed transactions and delayed
   recovery need their qualified claims. A known receipt
   and integrated recovery for the same entitlement cannot
   both be counted.
   [Accepted settlement and recovery views](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks),
   [product recovery ownership](../cashflows/15-credit-adjusted-pv.md)
   and [Duffie and Singleton on recovery alternatives](https://web.stanford.edu/~duffie/jstorlinks/ds2.pdf).

7. **Integrate recovery on checked local curve windows.** Walk
   active civil-date cells on the retained source clocks.
   Within each cell map the native clocks affinely and split
   at relevant hazard and discount/transform breakpoints and
   exposure boundaries. Use the exact local integrated hazard,
   and log-linear relative discount across each split window,
   including the continuous OAS factor once. Integrated hazard
   accrues linearly in the split window's fractional coordinate.
   For window `[a,b]` let `H=J(a,b)`,
   `G_i=log D(V,i)−s τ_i−J(V,i)` and
   `z=Δlog D−s Δτ−H`. Value recovery as
   `R N H exp(max(G_a,G_b)) φ(|z|)`, with
   `φ(x)=−expm1(−x)/x` and `φ(0)=1`. Use checked
   scaled products and ordered sums. Derive the increment
   from local hazards and logs. An exact zero hazard window
   has no recovery mass. A positive window cannot vanish
   because two represented survival values coincide.
   Reuse the checked window machinery and limits of
   CREDIT-12 where applicable: at most 100,000 active civil
   cells and 1,000,000 split windows. Remove the tree-step
   control from this leaf. Reject unresolved work, coverage
   or numerical results. The integral is analytic under its
   local constant-intensity and log-linear discount law.
   Curved within-window sources require separate numerical
   qualification.
   [Checked credit windows and limits](../models-and-valuations-credit/12-cds-legs.md)
   and [Berd on survival and recovery pricing](https://arxiv.org/pdf/0912.4623).

8. **Publish only the results the selected basis supplies.**
   Price, diagnostics, export and authorized risks reuse the
   prepared model, cuts and active bindings. Show promised
   cash and any integrated recovery contribution separately.
   Provide an FRP fair-value decomposition only by the same
   model using its own interval contribution authority and
   a checked reconciliation to raw and finalized PV.
   An interval's recovery PV is not settled cash on an
   invented date. A quote mark can report its own quote and
   bridge but has no automatic modeled cash decomposition.
   Return unavailable only for the declared capability or
   inapplicability cases. Required data, evaluation failure
   and nonfinite result errors keep their context. For quote
   conversions compute each requested qualified output on
   its actual basis. Reject placeholder zero, synthetic clean
   normalization and nonfinite-to-missing conversion.
   [Cashflow export](../valuations-framework/cashflow-export.md)
   and [metric provenance](../valuations-framework/metric-orchestration.md).

9. **Reconcile callers and qualify the supported domains.** Apply
   the same dispatch, quote and cash semantics in Rust and
   existing Python and WASM model, metric, quote-table and
   export routes. Keep the exact units and source provenance
   through schemas, traces and attribution. Leave yield and
   spread inverses, credit calibration, workout, quote replay
   and model-specific sensitivity calculations with their
   owning records. Qualify quote-only and model-only cases,
   negative rates, tiny/local hazard, settlement timing, curved
   discount integration, signed cash and recovery boundaries
   before implementation or market accuracy claims.
   [Spread and credit risk](16-spread-credit-risk.md),
   [default risk](17-default-risk.md),
   [Python quote surface](../../../../finstack-quant-py/src/bindings/valuations/bond_quotes.rs)
   and [WASM quote surface](../../../../finstack-quant-wasm/src/api/valuations/typed/fixed_income.rs).

## Observed behavior

### Choice, scope and selection

Native model selection is joint `RatesCredit` when credit-factor inputs exist, or a credit curve accompanies exercise/rate-factor inputs; otherwise attached credit selects `HazardRate`, exercise selects `Tree`, and remaining bonds select `Discounting`. Explicit registry selection preserves its model family. Hazard-consuming models require an explicit hazard ID; Discounting/HazardRate reject exercise rights. `Tree` does not consume a hazard curve. Structural Merton is a distinct explicit registry model (record 08). [ADR 0028](0028-callable-bond-model-selection.md) documents selection rationale without adopting it.

Validation precedes quote handling. A finite quoted OAS prices inside the selected model. Other mutually exclusive price drivers convert to settlement dirty, then carry to `as_of`; a quoted PV therefore need not exercise the model's numerical kernel. Scenario spread shock is allowed only on Discounting without another non-Z price driver, and adds bp/10,000 to quoted Z-spread (default zero). Quote conversion computes all applicable quote metrics after converting the input to a common clean-price driver; applicable errors propagate, explicitly inapplicable fields and nonfinite returned metrics become `None`.

### Algorithms and conventions

**Discounting:** sum entitled signed holder flows `Σ CF_i D(as_of,T_i)`. For OAS `s`, at each cashflow derive continuous base zero rate `r=−ln D/t`, convert the configured quote-compounding spread to its continuous shift, and multiply by `exp(−shift·t)`. Time follows the discount curve. Neumaier compensated sums accumulate PV.

**Hazard FRP:** build dated discount/survival targets from `as_of` through the final adjusted cash-settlement date; include every calendar day, with `tree_steps` a minimum resolution (default one for this leaf). Add promised cash `CF_k D_k S_k exp(−s t_k)`. For each interval add `D_k S_k exp(−s t_k) R N_k W_k`, with `W_k=λ(1−exp(−(r+s+λ)Δt))/(r+s+λ)` through the shared stable FRP helper. `N_k` is after-event outstanding, replayed on actual repayment dates and including PIK/draws; recovery is clamped to [0,1]. No accrued-on-default recovery is added. OAS is continuous only; other quote compounding errors. No future cash gives zero. The same daily target helper is used by deterministic joint rollback.

### Limitations and alternatives

Risk-free cash discounting, survival-only discounting and FRP are different economic choices. Only the first and FRP paths are present here; market-value recovery would be a new model. The FRP exact interval weight is exact under its interval-constant inputs, not a guarantee for curved within-day rates or stochastic dependence. Quotes can hide missing calibration until a risk metric is requested. The quoted-price and model-value distinction must remain visible in adoption criteria.

## Evidence

Owning code: [native dispatch](../../../../finstack-quant/valuations/fixed-income/src/bond/types/pricing.rs), [native lifecycle](../../../../finstack-quant/valuations/fixed-income/src/bond/types/traits.rs), [family registration](../../../../finstack-quant/valuations/fixed-income/src/family.rs), [registry envelope/MC diagnostics](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/mod.rs), [discount kernel](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/discount.rs), [hazard kernel](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/hazard.rs), [quote orchestration](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/compute.rs), [price metrics](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/prices.rs). Hazard inline tests compare zero hazard with discounting, require explicit curves and reject optionality; these are limiting-case/internal consistency evidence. [Callable credit-risk tests](../../../../finstack-quant/valuations/tests/instruments/bond/metrics/callable_credit_risk.rs) locate model-sensitive risk comparisons, not independent callable-credit accuracy qualification. [Quote tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/tests.rs) inspect applicable-error propagation and nonfinite-field handling. Reference: [Hull–White credit valuation](../../../REFERENCES.md#hull-white-2000-cds).

### Independent review evidence

- An admitted whole-position settlement dirty amount of 98,
  financing factor .997 and holder-only claim PV 2 give a
  quote-derived holder mark of 99.706. The price input supplies
  no implied recovery or model sensitivity.
- Give an entitled holder 1 today and signed later cash of
  102 and −3 at source time one year. Start with a 5% nominal
  semiannual zero rate. A −100 bp nominal semiannual spread
  changes the dated factor from `1600/1681` to `2500/2601`
  and yields PV `27789/289≈96.15570934256`. Applying the
  same decimal spread under the continuous law gives
  approximately 96.17664870085.
- For recoverable par 100, promised 3 at .75 years and
  103 at two years, flat discount rate .02, continuous OAS .01,
  hazard .04 and recovery .40, promised PV is
  92.3904612092444 and prompt recovery PV is
  2.98609747659872. Total PV is 95.3765586858431. With
  a separate terminal cash of 102 at one year, rate .02,
  hazard .03 and OAS −.05 make the total log-weight increment
  zero. For par 100 and recovery .40 the recovery is exactly
  1.20, giving PV 103.20.
- With no discount or spread, hazard .1 and recovery .4,
  economic retirement of 20 at .20 years with actual
  payment at .25 years adds 0.0391100902273811 to prompt
  recovery PV when the legal modeled claim stays exposed
  during the lag. The original economic stock still changes
  at .20 years.
- At local hazard `H=2^−54`, conditional survival rounds
  to one in binary64. With par `2^90`, no discount and full
  recovery, modeled recovery is about `2^36`.
  The local hazard retains that contribution. A historical
  integrated hazard of 1,000 can round absolute survival to
  zero even though a new local hazard of .1 gives conditional
  survival about .904837418.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
