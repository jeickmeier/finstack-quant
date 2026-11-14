# VALUATIONS-BONDS-06: Joint rates-credit pathwise coupon, principal and recovery replay

**Decision status:** Accepted — complete checked joint bond payoff and recovery replay package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, stochastic calibration, market, risk and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked joint bond payoff and recovery replay

1. **Use one dated prepared joint basis.** Consume the explicitly
   selected RatesCredit model from record 03 and the accepted shared
   controls, marginal fit and coupling. Require the two qualified
   rate and intensity axes, nullable correlation under the shared
   contract, matching market sources, valuation origin and information
   phase. Consume the fitted signed rates and effective nonnegative
   intensities directly under the accepted transitions.
   Use ACT/365F and a required positive integer number `m` of
   subdivisions per civil day for a positive span. A date `k` days
   from origin is node `k*m`, with checked integer geometry, work
   limits and strictly increasing represented times. Keep actual
   fixing, model availability, index interval, schedule, principal,
   exercise and payment dates. Keep each market source's native
   clock, using exact dated queries and the accepted adjacent-day
   native-coordinate mapping for numerical substeps. Extend model
   coverage through the last required contractual event and any
   later index maturity needed by an active quote. Factor-only
   coverage adds no cash, exercise or recovery exposure. Reject
   misaligned or unresolved state and remove tolerance snapping,
   time corrections and minimum-step defaults.
   [Bond clocks](02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks),
   [shared dated model](../valuations-framework/rates-credit-grid.md#accepted-decision-for-checked-dated-rates-credit-valuation),
   [marginal fit](../models-numerical/rates-credit-marginal-calibration.md)
   and [coupling and sampling](../models-numerical/rates-credit-coupling-and-sampling.md).

2. **Prepare authoritative parents, claims and opening state.**
   Bind each coupon calculation, principal event, settlement and
   retained claim to its original identity. Require a covered opening
   balance, pre-default claim status, fixing and event history,
   entitlement and due/paid status at the valuation cut. Apply the
   same exact parent and claim entitlement rule to ordinary and
   replayed floating cash. Remove date/amount matching and the
   dynamic ex-coupon bypass. A coupon detached from the current
   holder can still supply state for capitalization, recovery
   exposure, exercise or issue-basis return protection when required
   by that claim. Fixed and published-rate coupons become ordinary
   known cash only when all amount dependencies also resolve. A
   supplied schedule is admissible only with the original parents,
   sufficient payoff and balance provenance, and the same replay
   authority as generated terms. A bare cash strip or orphan
   segments supply no contingent coupon, recovery or exercise state.
   Distinct full parents can share a payment date.
   [Schedules and holder cash](01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash),
   [parent-bound replay](../cashflows/08-floating-replay.md) and
   [settlement and model claims](02-settlement-and-clocks.md).

3. **Respect information availability and project the actual index
   gross.** Keep each quote's index and denomination, exact fixing
   or value date `f`, effective date `s`, maturity `e`, positive
   index fraction `α` and authorized availability phase. Use the
   exact published fixing version when required. For a modeled
   observation on the admissible grid with `0≤f≤s<e`, set
   `G_f=G_0(s,e)*[B(f,s)/B(f,e)]/[B(0,s)/B(0,e)]`, where `B`
   is the fitted unspread rate marginal's achieved unit bond and
   `G_0` is the qualified market index gross. Compute the annual
   index rate as `expm1(log G_f)/α` with the checked log-ratio
   law. Preserve the market rate at origin and on a deterministic
   rate axis after validating the requested quote contract. Keep
   OAS, hazard, recovery and exercise out of projection. Remove
   the additive discount-forward proxy, inferred tenor ends,
   timing scales, implicit curve substitution and generic fixing
   fallbacks. Freeze a modeled quote from its declared rate-state
   node and expose it only at the admitted availability phase.
   Missing required history errors. A pre-origin unpublished quote
   needs an expressly qualified dated projection or scenario for
   the required purpose and phase. Retain that status and fail
   without the required authority. Normalization qualifies a
   raw gross first moment in its maturity-forward measure.
   Basis is deterministic and coupon valuation still depends on
   controls, cash dates, exercise and default.
   [Fixing publication authority](../conventions/0045-financial-clocks-and-date-boundaries.md#publication-aware-fixing-eligibility-without-a-universal-cutoff),
   [resolved term observations](../cashflows/06-floating-rate-projection.md),
   [joint coupon projection](../models-numerical/rates-credit-coupon-folding.md)
   and [Henrard multicurve model note](https://quant.opengamma.io/Hull-White-One-Factor-Model-OpenGamma.pdf).

4. **Compile original overnight blocks and settle one parent
   amount.** Use the authored observation calendar and admitted
   arithmetic or compounded bond terms, with lookback, observation
   shift or cutoff only in the combinations supported by the
   overnight owner. Compile and bound the full original parent's
   schedule. Keep the exact index quote interval, applied weight,
   coupon accrual fraction and payment date distinct. A partial
   block changes its weight while a Friday quote can still cover
   through Monday. Reuse the same frozen quote for repeats and
   cutoff. Preserve individual factor boundaries and one growth
   clock through balance processing and checkpoints. Use the
   accepted positive-gross rules for compounded growth, the selected
   native arithmetic or compounded formula, and the owned bounds,
   gearing, spread and final adjustment law. Capture exact Money
   principal at the parent's actual accrual start after the applicable economic
   phases, independently of locking a fixing. Apply the accepted
   whole-parent Money settlement and split to term and overnight
   alike. Admit a signed cash component and require the generated
   PIK component to obey the nonnegative rule. Support within-parent
   principal changes only through complete covered original-parent
   allocation. Reject generated stochastic PIK combined with
   balance-dependent amortization in this profile. A fixed authored
   principal delta can be admitted when its amount, phase and
   resulting parent dependencies are fully supported. Keep replay
   as this bond route's amount authority. The optional single-fixing
   coupon fold is a separate method with its own termination and
   pending-state eligibility.
   [Whole-parent cash and PIK](../cashflows/05-coupon-cash-and-pik.md),
   [overnight blocks and growth](../cashflows/07-overnight-compounding.md),
   [overnight projection](../valuations-framework/overnight-projection.md)
   and [New York Fed SOFR convention](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates).

5. **Replay the economic and cash phases separately.** For generated
   terms compute ending parents on their covered pre-end history.
   Capitalize admitted generated PIK once at its actual accrual end,
   then apply the explicit same-date economic principal batch and
   scheduled amortization and fee phases. Preserve the stated
   exercise and ordinary terminal redemption order and capture
   subsequent parent stock after the applicable start-date phases.
   Validate exact same-currency balances and atomic nonnegative
   stock, without epsilon repairs. Apply payable cash once at its
   actual admitted payment phase. A payment date alone changes no
   economic principal. Use an end or early payment only when all
   amount dependencies are available at that phase. Known future
   weights may be evaluated on a separate calculation state under
   the accepted cashflow law, without advancing the real economic
   state or fetching future observations. Support a payment on
   accrual start only when fixing, capture and cash phase are
   legally ordered and fully resolved. Identify each terminal
   economic/payment pair by contractual identity and keep a locked
   pending payment through settlement. Authored redemption cash
   controls nonpar terms. Generated par redemption uses the
   qualified live balance. Cash paid before a later economic event
   may extinguish the related future capitalization or repayment
   only where the terms and paired identities authorize it. No
   automatic terminal PIK add-back, duplicate cash, or par
   inference from positive settlement rows.
   [Dated principal and atomic phases](../cashflows/03-principal-and-emission-order.md)
   and [parent settlement and early timing](../cashflows/08-floating-replay.md).

6. **Admit complete exercise payoffs at the decision phase.**
   For live exercise select exact eligible dates, immediate full
   redemption, explicit holder entitlements and common versus
   branch-dependent cash. Preserve irrevocable prior exercises as
   qualified dated claims. Future deferred exercise and partial
   redemption require separate payoff contracts.
   Compare the payable exercise amounts with hold after survival
   and eligible recovery. Apply the accepted put/call ordering,
   shared cash and friction rules where qualified for this route.
   Require payable cash accrued and any cash-settled pending PIK
   to be determined by the actual termination provision using the
   original parent and available state. A fixed or locked term
   coupon may use the exact contractual original-parent prefix
   when authorized and resolved. A known overnight prefix alone
   cannot determine a payable amount with unfinished full-parent
   adjustments or future dependencies. Remove generic prorated
   ACT/365L, partial monetary getters, synthetic shorter coupons
   and unknown future accrual. Reject a live right whose required
   payoff or retained claim is unresolved. Capitalized PIK is
   already principal, while uncapitalized PIK enters proceeds only
   if due in cash. Apply record 05's separate issue-basis paid-through
   floor state at its authorized phase. Keep contingent surviving
   cash after redemption on its own qualified claim basis and
   keep contractual make-whole cash distinct from holder cash.
   Record 07 owns stochastic benchmark valuation and policy fitting.
   [Exercise and notice phases](04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise),
   [return floor state](05-return-floors.md#accepted-decision-for-checked-issue-basis-return-floors)
   and [joint exercise policy](07-lsmc-policy.md).

7. **Use a separate covered recoverable-par view and prompt FRP.**
   Validate the default-free discount and issuer intensity roles,
   eligible alive claim, authorized recovery fraction `R∈[0,1]`
   and exact recoverable par `N_rec`. Build that par from
   authoritative paired claim and principal events. Apply authorized
   capitalization and write-down on the economic dates and retire
   eligible repayable par at the actual repayment cash phase.
   Carry lagged repayment exposure until the authorized payment
   phase on its actual date.
   A qualified earlier repayment cash phase retires the covered
   claim there without retiring it again on its economic date.
   Leave economic principal on its own accrual/exercise dates.
   Reject unresolved exposure rather than infer it from cash or
   clip a balance. On an outgoing interval with applied discount
   exponent `A` and local integrated hazard `H≥0`, use risky
   continuation factor `q=exp(−(A+H))` and conditional discounted
   prompt recovery `R*N_rec*W`, where
   `W=H*[−expm1(−(A+H))]/(A+H)`. Use `W=0` at `H=0`
   and `W=H` at `A+H=0`. Compute from canonical exponents
   with checked scaling. Preserve tiny positive hazard even if
   rounded survival is one. Recovery excludes accrued, premium
   and market value. Add it only on a branch retaining the
   exposure. Separate known or legally default-independent cash
   from the risky stream under its authorized basis. Later
   default-contingent cash can still use survival after recoverable
   par reaches zero. This is a local equivalent-rate/intensity
   integral. Qualify subdivision for curved sources and the
   deterministic comparison on matching geometry.
   [Accepted recoverable par](03-pv-models.md#accepted-decision-for-checked-bond-pv-selection-and-fractional-recovery-of-par),
   [checked joint discount and recovery](../models-numerical/rates-credit-coupon-folding.md)
   and [Duffie on fractional recovery of par](https://web.stanford.edu/~duffie/par.pdf).

8. **Carry the same hold through deterministic and sampled
   rollback.** Exact zero volatility on both axes uses one-state
   rollback with the same contractual replay and recovery. With
   either axis stochastic, sample the accepted coupled factors
   and integrate default conditionally. A sampled bullet with no
   live exercise requires no fitted stopping policy. When applying
   OAS, require a signed annual decimal on the bond's explicitly
   selected quote basis. Sampled pricing admits Continuous only.
   The both-zero route also admits nominal Semiannual Periodic
   under the checked nodewise law. Use the resulting same `A` for
   hold and recovery. A shock that activates stochastic dynamics
   must have a qualified
   quote basis for that model. Fit and projection remain
   unspread. Pay common current cash once without outgoing
   survival. Between decision nodes `a<b`, let
   `Q(a,k)=exp(−Σ_[j=a,k) (A_j+H_j))`. Along a sampled path,
   hold excluding departure cash is `Q(a,b)*V_b` plus
   `Σ_[a,b) Q(a,k)*Recovery_k` and
   `Σ_(a,b) Q(a,k)*Cash_k`. Each recovery term is conditional
   local PV. `V_b` includes eligible endpoint cash. Compose
   the checked affine block or equivalent interval rollback
   without duplicate survival or endpoint cash. The final payoff
   has no outgoing rate, OAS or exposure. Prove extinguished
   claims before returning zero. Prove current entitlement for
   any same-day payoff with no future interval.
   [Checked tree OAS](../models-numerical/tree-discounting-and-induction.md),
   [shared quote controls](../valuations-framework/model-resolution.md)
   and [bond spread ownership](12-spread-quotes.md).

9. **Share checked replay, diagnostics and failures across callers.**
   Keep bounded immutable preparation and private Rust path and
   checkpoint state bound to the exact parents, qualified history,
   rate and principal phases, frozen observations, partial growth,
   paid and pending claims and distinct exposure and return-floor
   state. Checkpoint, precomputed and uncached replay must agree on
   events, payoffs and failures. Reuse terms only when their
   inputs match the current model, cash and discount experiment.
   Replace NaN cache sentinels, silent fallback and partial outputs
   with checked results. Share the admitted state across native
   and registered pricing and required OAS and risk calls,
   rebuilding or refitting affected terms under the selected
   shock. Report actual model/source roles, hold and recovery
   basis, dated event and actual work information, and the valuation
   information cut.
   Export contractual cash and any qualified model trace under
   their own provenance and reconciliation. Reconcile the
   existing Rust, Python, WASM and schema entry points, error
   mapping and input restoration. Keep private checkpoints and
   lattice internals in Rust. Record 07 owns policy training,
   regression, estimator counts and option-price uncertainty,
   record 12 owns the OAS inverse, and record 17 owns default
   risk outputs.
   [Cashflow exports](../valuations-framework/cashflow-export.md),
   [joint policy](07-lsmc-policy.md), [OAS](12-spread-quotes.md)
   and [default risk](17-default-risk.md).

## Observed behavior

### Choice and scope

Joint pricing calibrates daily ACT/365F discount/survival targets through the final adjusted payment. Zero rate and hazard volatility use deterministic FRP rollback; either positive volatility uses sampled calibrated rates-credit paths, including sampled bullets without regression when there are no rights. Factor calibration/dynamics are owned by the models/framework scopes; this record owns the bond payoff replay.

### Algorithm

Build schedule-owned cash, balance, accrual, reset, payment and exercise events. Exact calendar events must lie on daily grid nodes within `1e−10`. Historical balances seed the path. Floating rows with projected-index metadata are grouped by accrual start/end/payment and replaced by compiled coupon replay; known term fixings remain fixed. An unknown term index uses `F_index,base + ((1/P_node(reset,end)−1)/α − F_discount,base)`: deterministic index/discount basis plus the node's conditional discount-forward shock. End is curve tenor from reset, snapped to the grid; tenor past calibration horizon errors. There is no separately stochastic index basis.

Overnight replay compiles its actual observation/weight schedule, historical fixings and configured fallback, index caps/floors and overnight day basis. Each unknown daily observation similarly adds a conditional discount-forward shock to the projected index rate. Path checkpoints retain sufficient observation history for lookback/lockout. Started coupons capture scheduled accrual-start notional and known resets.

At a step: advance live overnight observations; collect static and floating cash distributions; capitalize floating PIK at accrual end; apply static economic balance events; validate/clamp tiny negative outstanding to zero; observe new term resets; capture new accrual-start notional after same-day balance changes. Separate payment and accrual-end events avoid premature PIK settlement. Accrued state retains unpaid cash and uncapitalized PIK; static ACT/365L uses actual-day elapsed/full-period ratio, other static claims use day-count ratios, and floating claims use compiled coupon accrual. Negative floating PIK is rejected.

For each interval set `q=D_path·exp(−OAS·Δt)·S_path` and recovery `R·N_after_events·W_FRP`. Between decision dates aggregate these into affine continuation `A·V_next+B`, with interim cash/recovery in `B`. Default is integrated conditionally via survival/FRP weights rather than sampled as a default stopping time. Terminal repayment uses replayed outstanding, including pending PIK owed at early payment. Custom schedules with exercise are rejected, as are stochastic floating PIK plus amortization, segmented overnight coupons lacking original full-period scope, and coupons needing future observations after payment.

Static products precompute replay states. When hazard volatility is zero and rate volatility positive, interval terms can be cached by rate node. A failed FRP calculation in that optional cache is marked NaN; lookup then recomputes through the fallible path so the original error is propagated. This is a cache bypass, not acceptance of a NaN financial result.

### Observed discrepancies and alternatives

Static cash entitlement uses the holder filter, but dynamic floating groups are built from schedule rows without the ex-coupon test, and every dynamic cash coupon is paid. This is a source-level discrepancy, not an executed regression. Recovery uses replay outstanding on economic balance dates, whereas the hazard/deterministic path deliberately retains principal exposure until cash repayment. Lagged amortization can therefore give different recovery exposure across routes; the routes do not represent the same recovery-balance timing.

The deterministic-basis conditional-forward model is cheaper than separate rate-index factors but cannot capture basis volatility. A full contractual state engine is the available route for supported floating coupons; rates-only preprojection is restricted to deterministic rates.

## Evidence and qualification

Owning code: [routing](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/tree_pricer.rs), [template/replay](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/replay.rs), [observation compilation](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/build.rs), [conditional rates/intervals](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/state.rs). [LSMC tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/tree/lsmc/tests.rs) explicitly inspect lagged/early PIK, economic-date amortization, conditional-forward clocks, segmented-overnight rejection and block replay. These establish code contracts, not independent stochastic pricing accuracy. General references: [interest-rate modeling](../../../REFERENCES.md#andersen-piterbarg-interest-rate-modeling), [credit intensity](../../../REFERENCES.md#jarrow-lando-turnbull-1997).

- With equally weighted time-zero state-price masses `.45,.45`
  at fixing/effective date and achieved end bonds `.80,1.00`,
  the two normalized index gross values for market gross 1.04
  are 1.17 and .936. The maturity-forward weighted mean is
  exactly 1.04. An annualized rate on index fraction .25 is
  .68 or −.256, respectively, both with strictly positive gross.
- A Friday-to-Monday index gross of 1.0003 is a 3.6% annual
  quote on ACT/360. A one-day coupon overlap uses the same
  quote and gives growth increment .0001. Replacing the
  three-day quote interval with the one-day overlap gives .0003.
- Opening principal 100 with 6 total coupon split into cash 3
  and PIK 3 capitalizes to 103 before economic amortization of
  20, leaving economic principal 83. With qualified delayed
  repayment, recoverable par is 103 before the cash is paid
  and 83 afterward. Cash detachment removes the coupon receipt
  of 3 for the current holder and leaves capitalization intact.
  When two otherwise identical co-dated cash coupons of 8 have
  different claim identities and only one is entitled, exactly
  8 is due regardless of insertion order.
- At `A=.03,H=.02,R=.4` with opening par 100 and a delayed
  repayment of 20, local recovery is about .78032921 on
  recoverable par of 100. The economic-principal proxy of 80
  would give .62426337. A positive `H=2^-60` still contributes
  recovery when its represented survival factor equals one.
- With risky interval factors `.9,.8`, local recoveries
  `2,3`, current cash 4, intermediate cash 5 and endpoint
  value 100, both interval rollback and affine composition
  give `4+.72*100+2+.9*(5+3)=85.2`.

Qualify with independent finite-state pricing, dynamic versus
ordinary ex-coupon entitlement, publication/cutoff and signed
coupon cases, same-day priority and actual paid dates, early and
lagged PIK, partial-state and unsupported payoff failures, curved
sources across admitted subdivisions, deterministic and sampled
replay, checkpoint and cache identity, and OAS, risk and host
reconciliation.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
