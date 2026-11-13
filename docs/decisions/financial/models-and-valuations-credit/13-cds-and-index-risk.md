# MODELS-AND-VALUATIONS-CREDIT-13: CDS and index quote risk, default exposure and margin proxy

- Decision status: **Accepted — complete checked CDS and index risk package, 2026-11-13**.
- Implementation status: Current behavior observed below. The accepted contract requires Rust, calibration, margin, host and index reconciliation.
- Validation status: Source, selected tests and primary references inspected. Independent risk and lifecycle examples evaluated. Production and host qualification remain pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked CDS and index risk

1. **Freeze the pricing basis and effective exposures.** Reprice the
   holder's signed raw economic PV from [CREDIT-12](12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis),
   including eligible actual dated upfront and with no clean display
   adjustment. Hold valuation close, selected contractual terms, cash
   eligibility, position, composition and historical events fixed across
   bumps. Use the effective live notionals, recoveries and curve bindings
   returned by the index pricing owner. Single-curve pricing uses its
   effective index exposure; constituent pricing uses its individual live
   allocations. Do not derive another set of weights, reapply the index
   factor or assign exposure to a defaulted or zero-weight name. Distinguish
   an exhausted protection interval from an issuer whose curve has zero
   hazard. Credit risk can remain before a forward start. Earned coupons and
   eligible deterministic upfront can carry rate risk after protection ends.
   Use the actual model cash dependencies to establish a structural zero,
   after validating the request. Do not require quote replay for genuinely
   unaffected cash. Missing required inputs for live risk are errors.

2. **Use exact quote bindings and qualified replay.** Use the existing
   provider and the [CORE-MARKET-05](../core-market/hazard-quotes-and-risk.md)
   and [CORE-MARKET-06](../core-market/calibration-provenance.md) contracts
   for lossless recipes, typed original observations, the bound par-risk
   view, zero-shock source reproduction and target fit acceptance. Quote
   CS01 changes the declared par-risk inputs, including properly derived
   par values for original upfront quotes. Preserve original observations
   separately. Each quote is identified by its curve and exact stored ID
   or verified recipe index; dates are metadata unless they resolve
   uniquely. Do not choose a nearest tenor, synthetic zero pillar, or the
   first available curve. Freeze the recipe's actual calendar, conventions,
   documentation clause and discount dependency. Trade terms do not
   rewrite calibration terms. Remove the implicit deal-level quote override
   triggered by clean presentation. An authored change to the quote
   center belongs to an explicit market scenario with its own qualified
   center. No rate, intensity, z-spread, clip or fit fallback for missing
   or failed quote replay.

3. **Select the spread stencils, units and labels.** Require an explicit
   finite positive bump `h` in bp. For parallel CS01, move every selected
   par-risk observation by `±h`, holding discount, recovery and deal
   cash fixed. For bucketed CS01, move one exact bound quote at a time.
   Rebuild each affected distinct source once per scenario, then reprice
   the full trade or index. Shared source deduplication requires identical
   complete binding; it does not merge member exposures. Report
   `(V_+−V_-)/(2h)` in currency per bp in holder sign. A finite sum of
   separately repriced buckets is a separately labelled result, without
   guaranteed equality to parallel CS01. Keep fully qualified
   `cs01::<curve>` and `bucketed_cs01::<curve>::<quote>` identities
   for single-curve/bucket results; label the full multi-curve parallel
   result with its declared curve set. Direct intensity risk stays
   `hazard01` under its distinct contract. Coupon risky PV01 is the
   selected premium coefficient `N A_basis/10000` from CREDIT-12, with
   an explicitly selected clean/dirty basis and no quote repricing.
   For single-name parallel spread gamma use the same up/down views and
   the accepted zero-shock **par-risk** replay center, not the historical
   upfront-form or unrelated base value. Report
   `[(V_+−V_0)+(V_-−V_0)]/(h/10000)^2` in currency per decimal spread
   squared. A represented quote must move in the intended direction
   and remain admissible. Use the selected nominal common scalar step
   for simultaneous directions and retain represented input moves.
   Reject subresolution moves, invalid endpoints and failed replays.
   Use checked differences, scaling and compensated accumulation.
   No tiny-bump zero, catch-all one-sided difference or assumption
   about sensitivity sign. [White, section 8](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf).

4. **Make the rate basis and credit response independent explicit choices.**
   Select either typed rate-market quote rebuild or direct additive
   continuous-zero shock under [CORE-MARKET-03](../core-market/rate-shocks.md).
   The first uses exact quote targets in the rate recipe; the second
   uses the accepted parallel or exact-width triangular geometry on the
   retained discount clock. Separately select frozen active intensity
   sources or replay of affected hazard dependencies. In replay mode,
   rebuild each affected active hazard from unchanged **original**
   observations, not derived par-risk quotes, after constructing the
   bumped rate market. Require the replay for every affected active
   source in that direction; no mixed automatic freeze for missing
   records. Unaffected sources stay fixed. Preserve the dependency's
   declared discount role, even if it differs from the deal's discount
   role. Select target source IDs explicitly. Use the same two choices
   for both parallel and bucketed DV01. Report checked central
   `(V_+−V_-)/(2h_bp)` with positive-up rate sign and separate
   basis-qualified keys/provenance. Use a common nominal step and record
   represented moves on quote inputs. Require both selected sides to
   produce admissible shocks and move all selected quotes or direct
   target coordinates as defined by the operation. Express rate
   shifts in the native quote representation: for a future quoted
   as `100−100r`, a positive `h_bp` rate shift subtracts
   `h_bp/100` price points. Preserve the frozen convexity rule.
   No metadata-selected basis,
   reinterpretation of a quote as a zero rate, expired shortcut that
   drops rate-sensitive cash, or exact finite bucket-sum claim.
   [White, section 9](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf).

5. **Separate fixed-intensity recovery risk from holding credit quotes.**
   Require a named selection and report holder currency per +.01
   absolute recovery (one percentage point). Frozen-intensity recovery
   sensitivity is the exact affine coefficient
   `−side × .01 × Σ_i N_i K_i`, where `side` is +1 for buyer and
   −1 for seller, and `K_i` is discounted covered default mass from
   the unit-LGD protection kernel. Hold intensity and the entire premium
   leg fixed. No bump or division by LGD is required. This coefficient
   does not claim that a full +.01 scenario is admissible at the upper
   boundary. In quote-preserving Recovery01, hold the original credit
   observations, discount market and deal cash fixed. Refit all affected
   hazards at the changed recovery, synchronizing contractual and source
   recovery for each priced slice. Use one explicit common positive
   decimal bump `h` across the active recoveries. Select central
   `.01(V_+−V_-)/(2h)` if both full vectors admit `R∈[0,1)`.
   If exactly one full direction is outside that domain, use the
   other full direction and the same qualified no-shock center in a
   one-sided difference scaled by `.01/h`. Report the actual stencil,
   nominal step and represented moves. If neither direction is
   admissible, or a selected move fails to change a represented
   input, fail. Determine domain before any solve; provider, fit,
   missing-market or arithmetic errors never choose the stencil.
   Do not clip individual names or divide by weighted clipped spans.
   Reuse only identical complete recovery/recipe bindings and reject
   conflicting requests on one hazard identity. Keep settled defaults
   and historical recoveries unchanged. No recipe-presence switch
   between these two recovery measures.

6. **Measure expected protection loss over the covered interval.**
   Report unsigned, undiscounted expected protection cash in
   instrument currency, under the supplied pricing hazard and
   conditional on survival at valuation close. With `L` the close
   before the first modeled covered date, `U` maturity close and
   `J` the local integrated hazard, each slice contributes
   `N(1−R) exp(−J(v,L)) [−expm1(−J(L,U))]` when `U>L`.
   An empty covered interval gives +0. Preserve survival to the
   forward start, neither covering pre-start default nor conditioning
   again at that start. Use stable scaled evaluation; do not
   difference rounded survival endpoints. Index EL sums exactly the
   effective live slices used by pricing, without a second
   normalization. Include no premium, discount, counterparty loss
   or regulatory credit-loss measure.

7. **Separate gross payout, net settlement cash and jump P&L.** Define
   one modeled immediate default at the opening of the next civil
   day after valuation close. It is covered only if that named day
   falls within contractual protection through maturity inclusive.
   On the buyer axis, gross protection cash is `N(1−R)` if
   covered and zero otherwise. Net default settlement cash deducts
   contractual coupon accrued through the opening boundary from the
   current accrual period, only if covered. Preserve the accrual
   origin and zero accrual at a new period's opening. Earned coupons
   and unchanged dated upfront survive the stress; no modeled
   pre-protection termination premium. Discount the net event cash
   under the CREDIT-12 default settlement rule: factor one at the
   opening for contemporaneous settlement, otherwise a checked
   factor to the lagged settlement date. Economic jump-to-default
   is this event PV minus the signed dirty PV of the **terminated
   contingent rights** (protection less AoD and future unearned
   scheduled premium on the affected slice). A pre-coverage default
   can therefore remove a nonzero forward mark despite zero event
   cash. At or after maturity close no rights remain exposed,
   even if cash is still due. Reverse the cash and PV signs once for
   the seller. Give gross and net cash explicit distinct labels.
   Retire the ambiguous CDS clean `default_exposure` output that
   subtracts an arbitrary base mark, and the gross-only JTD alias
   in favor of the named gross payout. Use dirty economic jump as
   `jump_to_default`; no claim of Basel DRC equivalence.
   [White, section 10.2](https://quant.opengamma.io/Pricing-and-Risk-Management-of-Credit-Default-Swaps-OpenGamma.pdf).

8. **Use named index default scenarios and keep the anonymous proxy distinct.**
   With constituent pricing, target an exact member ID or return a
   series keyed by those identities. Apply the same event and jump
   definitions to that member's priced slice only, preserving all
   other members and deterministic common cash. Unknown names error.
   Known defaulted or zero-weight names have zero new exposure.
   Never infer member identity from a hazard ID. Remove the generic
   average of heterogeneous per-name payouts. With single-curve
   pricing, a caller may explicitly request an equal-weight gross
   per-name cash proxy with an authoritative positive **original**
   series count and index recovery. If covered and surviving
   exposure is positive, use `side × original notional × (1−R)/count`
   without multiplying a second time by the index factor. A fully
   extinguished or uncovered index has no gross payout. No name
   guessing, anonymous economic JTD, or invented clean mark by
   constituent. Composition, factor definition and realized-credit
   settlement belong to [CREDIT-14](14-index-valuation.md) and the
   existing index input owner. [S&P DJI, index credit events](https://www.spglobal.com/spdji/en/documents/campaigns/cds-indices-primer.pdf).

9. **Supply actual margin-factor credit deltas or report a failure.**
   Remove the CDS/index duration fallback and the replacement standard
   maturity for an exhausted contract. Do not assign an entire
   parallel CS01 to one trade-tenor bucket. Use signed raw forward
   `V(factor+1bp)−V(factor)` at each explicitly bound margin
   factor, from a qualified replay center. This differs from the
   configurable central risk stencil. Require the selected margin
   classification, exact issuer/seniority and payment currency for
   qualifying credit, and the margin owner's approved mapping to
   1Y/2Y/3Y/5Y/10Y vertices. Do not round quote maturities to guess
   factors or use curve-name parsing. Qualifying index delta must
   identify its underlying issuer/seniority slices. A shared source
   may be rebuilt once, but use separately attributable constituent
   reprices and never copy the full shared sensitivity to each name.
   Aggregate only like, qualified factor identities. If the existing
   representation cannot carry required dimensions, or a live
   binding/reprice is unsupported, return an explicit error. Do not
   fabricate a single-curve index allocation. Exhausted protection
   or known zero credit exposure yields no credit rows, while
   variation margin still uses the economic mark including eligible
   deterministic cash. Broader risk coverage, aggregation, currency
   conversion, parameter versions and regulatory application belong
   to the [margin owner](../margin-collateral-im/03-sensitivity-inputs.md).
   [Published ISDA SIMM, risk factors and deltas](https://www.isda.org/a/ZYxiE/ISDA-SIMM_v2.92606_PUBLIC.pdf)
   informs this adapter's factor shape and stencil, without selecting
   that edition for margin calculation.

10. **Keep one Rust experiment per basis and qualify it independently.**
    Expose checked immutable inputs, exact target identity, scenario
    choices, units, signed results and replay provenance through
    metric registries, reports and existing Python/WASM routes.
    Publish only complete results; no warning-and-proxy or
    missing-as-zero result for live requested risk. Direct, registry,
    prepared and host routes use the same canonical valuation.
    Use existing composite metric-key encoding for exact curve and
   quote identities; attribution matches these bindings instead of
   display tenors or unescaped prefixes. Reconcile attribution's
   spread/gamma normalization, report
    labels, margin adapters and schema/parity in the owning slices.
    Keep calibration solver and tolerance selection, option/tranche
    sensitivities, index composition and regulatory aggregation with
    their owners. Algebra and scenario discriminators do not
    qualify production replay, market inputs or regulated margin.

## Observed behavior

### Choice and rationale

Quote-space CS01/rebootstrap is distinct from coupon risky PV01 and direct hazard risk. Rate/recovery calculations conditionally replay calibration metadata; margin can fall back to a duration proxy.

### Scope and input domain

Single-name CDS and CDS indices, with active constituent hazard curves when configured. CS01 requires quote replay and a provider; missing rate calibration permits direct discount-curve bumps.

### Algorithm

CS01 rebuilds spread quotes and centrally differences raw reprices per bp; bucketed CDS uses cached schedule/discount factors. CS gamma uses up/down/replayed center divided by decimal spread bump squared. DV01 bumps rate calibration quotes when metadata exists, otherwise direct curve rates, then replays available hazard recipes under changed discount market. Parallel rate risk deduplicates active hazards, while the bucketed rate metric is registered to a generic triangular key-rate route. Recovery01 changes contractual recovery and rebuilds matching hazard; index applies active constituent recoveries and normalizes by weighted actual boundary span.
Expected loss=N×conditional default probability to maturity×LGD, unsigned and undiscounted; index weights active names and surviving factor. CDS JTD=signed LGD payout minus/plus accrued coupon; LGD-only excludes accrued, default exposure subtracts current PV. CDS protection-start/maturity gates apply to payout. Index JTD averages single-name LGD, uses active weights/factor for constituents, or original notional/name count for single curve; named count guesses warn and unknown names fail.
SIMM tries repriced CS01, otherwise warns and returns signed |N|×years×LGD×duration factor×1bp. Nonpositive maturity time substitutes standard CDS maturity before tenor bucketing. Index proxy scales notional by index factor.

### Selection and controls

Resolved sensitivity config controls quote/rate bumps. Doc-clause family and clean quote override feed single-name replay. Index JTD count can be explicit or inferred from name (125 IG/iTraxx,100 HY,75 crossover,40 EM).

### Conventions

CS01/DV01 per bp; CS gamma per decimal-spread squared. JTD and default exposure are different payoff/MTM definitions. Expected loss ignores protection-effective start, so it can include pre-protection default probability for forward starts.

### Limitations, failure and fallback

Rate/recovery frozen-versus-replayed behavior depends on metadata. Index JTD has no as-of maturity check; margin fallback may assign positive five-year-like risk to expired instruments if repricing fails. Quote risk fallback is not equivalent to calibrated market risk.

### Alternatives and tradeoffs

Fail-closed missing risk and explicit frozen/calibrated risk names are alternatives; coupon risky annuity is already a separate supported metric.

## Evidence

Current routes include [CDS metrics](../../../../finstack-quant/valuations/credit/src/cds/metrics),
[index metrics](../../../../finstack-quant/valuations/credit/src/cds_index/metrics),
[shared CDS CS01](../../../../finstack-quant/valuations/credit/src/cs01.rs),
[quote rebootstrap](../../../../finstack-quant/calibration/src/recalibration/hazard.rs),
[shared recovery bump helper](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/recovery01.rs),
[CDS margin](../../../../finstack-quant/valuations/credit/src/cds/marginable.rs),
[index margin](../../../../finstack-quant/valuations/credit/src/cds_index/marginable.rs)
and [attribution shift lookup](../../../../finstack-quant/attribution/src/metrics_based/shifts.rs).
[Credit tests](../../../../finstack-quant/valuations/credit/tests)
locate reprice, convention and factor checks. In-tree tests and host
suites were not run.

### Independent review evidence

- A toy two-quote PV `5x+7y+3x²y`, in bp shock coordinates,
  gives parallel CS01 `24` at `h=2` and a separately repriced
  bucket sum of `12`. With `V_-=995,V_0=1000,V_+=1007` and
  `h=2` bp, CS01 is `3` and spread gamma is `50,000,000`
  per decimal spread squared. A stale center of `999` doubles
  the reported gamma. A nominal +1 bp move at a binary64 quote
  of `2^53` bp does not move that endpoint.
- In a stylized five-year example with 10 million notional,
  20 equal quarterly periods, flat rate .03, flat intensity .02,
  recovery .40, contemporaneous default settlement and coupon 30
  bp above model par, a central direct zero-rate shock of ±1 bp
  gives buyer DV01 about `+39.97` with frozen intensity and
  `+33.25` after refitting the original par quote. Exact frozen
  Recovery01 is about `−8847.97`, while the original quote held
  fixed and ±.01 recovery refits give `+105.62`. The sign need
  not agree across recovery bases. An index with active recoveries
  .005 and .995 cannot take either complete .01 direction.
- With notional 10 million, LGD .60, pre-start hazard integral
  .10 and covered hazard integral .20, covered expected loss is
  about `984,115.18`. Including pre-start defaults gives
  `1,555,090.68`; conditioning again at coverage start gives
  `1,087,615.48`. Both alternative amounts use the wrong event
  probability for this contract.
- At 10 million notional, .40 recovery, 100 bp coupon and 36/360
  accrued, a covered event pays gross protection `6,000,000`
  and net cash `5,990,000` on the buyer axis. With settlement
  factor .98 and terminated dirty PV `340,000`, economic JTD is
  `5,530,200`. Unaffected earned cash cancels in that difference.
- With effective surviving notionals 5 million and 3 million,
  recoveries .40 and .70, and covered default probabilities .05
  and .20, member gross protection amounts are `3,000,000`
  and `900,000`, and index expected loss is `330,000`.
  A generic average gives neither member's cash exposure.
- The existing expired-duration proxy can emit `+2700` for
  a 10 million buyer, 40% recovery and substituted five-year
  term, or `+2160` with index factor .8. At valuation close
  on maturity there are no future covered days and no associated
  margin credit rows. A rolled earned coupon can still have PV.
- These checks used exact arithmetic or independently evaluated
  flat-curve integrals and bisection. They do not establish a
  production tolerance, quote fit, host parity, or margin compliance.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
