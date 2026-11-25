# VALUATIONS-OTHER-FIXED-INCOME-08: Term-loan settlement value and callable rates-credit lattices

- **Decision status:** Accepted 2026-11-14 — full checked term-loan valuation and exercise package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Reconciliation pending. The observed entry points and kernels are described below.
- **Validation status:** Live source and primary references inspected. Independent fixing, settlement, recovery, exercise and OAS arithmetic checked. Contract, model, runtime and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked term-loan valuation and exercise

1. **Elect one valuation route and one quote authority.** Use
   the original valuation and knowledge cuts with the lender's
   admitted claims. The terms-based default is `Discounting`
   only for deterministic entitled cash with no live exercise
   or pricing-credit requirement. Otherwise require an
   explicit compatible model. The tree election identifies the
   rates-only Ho–Lee or joint RatesCredit profile and its
   active market and control bindings. An elected zero-factor
   regime is explicit. Resolve known termination/default and
   surviving claims before live model work. A supplied clean
   quote, an available hazard curve, or failed calibration
   cannot choose dispatch or silently suppress a live right.
   Missing active terms or incompatible overrides error.
   Permit a model-only request, one separately elected quote
   mark, or one qualified spread repricing basis. A quote
   supplied solely as a calibration target is an observation.
   The admitted raw model remains separately available.
   [Valuation dispatch and lifecycle](../valuations-framework/pricing-lifecycle.md),
   [accepted loan quote authority](06-loan-quotes-eir.md#accepted-decision-for-checked-loan-quotes-cash-cost-and-effective-interest).

2. **Resolve fixings before dependent cash and par.** Compile once
   on the accepted OFI-07 contractual parents, dates and economic
   principal state. Resolve each required term reset or daily
   observation from its exact index, observation date, admitted
   fixing version and availability at the stated cut. Distinguish
   the fixing series from the projection role. Eligible known
   observations take precedence. A required missing historical
   observation errors. Use a future projection or not-yet-published
   observation policy only where expressly authorized, with its
   actual data and publication provenance. Preserve the original
   accrual geometry, margin and gearing, tenor, contractual rate
   bounds, overnight lookback/cutoff, compounding and fallback
   rules. Admit no unused forward lookup for a fully known
   amount.

   Run the accepted whole-parent settlement and exact PIK
   capitalization before dependent balances and fees. A seasoned
   deterministic state either replays complete qualified history
   or starts from a validated authoritative opening with the
   remaining unpaid and contingent claims explicitly identified.
   Avoid replaying PIK already included in that opening. Remove
   the post-generation rate-ratio patch, near-zero fallback to
   payment-date balance and the blanket seasoned floating PIK
   rejection on this deterministic route. Preserve genuine
   unknown or unsupported historical state as such.
   [Accepted term-loan cashflows](07-term-loan-cashflows.md#accepted-decision-for-checked-term-loan-cashflow-compilation),
   [term observation and replay](../cashflows/06-floating-rate-projection.md),
   [overnight observation law](../cashflows/07-overnight-compounding.md),
   [parent-bound settlement](../cashflows/08-floating-replay.md),
   [ARRC syndicated SOFR conventions](https://www.newyorkfed.org/medialibrary/Microsites/arrc/files/2020/ARRC-Syndicated-Loan-Conventions-Technical-Appendices.pdf).

3. **Price at the original holder cut and finance buyer cash
   separately.** Deterministic unspread value uses actual
   entitled signed cash with native relative discount factors
   on its actual payment dates. A qualified fixed floating strip
   can consume the accepted curve-DM or z-spread settlement
   pricing law only as an explicit quote experiment. Use the
   relevant fixed cash, factor clock and compounding of that
   law. Keep the settlement curve spread separate from the
   live model OAS.

   At valuation `V`, let `H` be holder PV, `J` the PV of the
   buyer's actual settlement claim at `V`, and `F(V,S)` its
   elected positive settlement financing factor. Then buyer
   dirty `Q=J/F`. Apply `H=FQ+(H−J)` only when the retained
   claim or entitlement difference is independently qualified.
   The additive bridge admits exact matched rights and native
   cash. Intervening borrower termination, credit and changing
   obligations need a buyer-claim model that accounts for those
   effects on the original valuation basis. Do not reopen the
   tree at settlement or approximate a difference of risky claims
   with a risk-free payment-date sum. Absent that bridge, preserve
   raw holder PV and any independently observed buy-side quote.
   An observed quote mark uses explicit authority, the matching
   purchased funded denomination and signed accrued for a
   clean input. Direct dirty skips unused accrual. A quote-only
   mark needs model inputs only where its elected conversion
   actually uses them. Trade purchase cash remains separate.

   Keep a settled, defaulted or expired state distinct from a
   pending valid cash claim. An entitled known same-day amount
   uses factor one at its authorized phase. Paid cash is
   excluded. Legal maturity alone cannot erase an adjusted
   payment, fixed recovery receipt or remaining obligation.
   Require complete applicable settlement or claim state
   before a structural zero. Overdue unresolved cash needs
   its own admitted valuation.
   [Settlement buyer bridge](06-loan-quotes-eir.md),
   [related cash and financing roles](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf).

4. **Fit the selected lattice on exact valuation-relative
   geometry.** For a live tree, use ACT/365F from the actual
   valuation origin and the required horizon through the last
   applicable cash, decision or index-dependency node. Require
   explicit positive step count, fit tolerances, work budgets
   and active parameters. On a uniform grid require exact
   integer date placement `N × elapsed_days = j × horizon_days`
   for every mandatory date. Preserve actual observations,
   economic balance changes, exercise and payment phases.
   Preserve distinct fractional-day numerical coordinates.
   Use the exact source-specific mapping for relative
   `D(V,T)` and local integrated hazard. Admit fractional
   numerical nodes only with a qualified native clock bridge.
   Validate source coverage and the fit on the actual
   calibrated state. Remove nearest civil-date mapping,
   splitting or snapping cash, timing corrections, hidden
   refinement and the default 100 steps.

   The rates-only profile selects continuous additive Ho–Lee,
   zero mean reversion and explicit absolute rate volatility.
   One state at exact zero volatility preserves the admitted
   relative factors. Positive-volatility rates-only valuation
   admits fixed or already determined cash with fully known
   amount dependencies. A future floating reset requires the
   joint replay profile or another qualified model.
   The joint route supplies explicit rate/intensity axes,
   recovery, marginal controls and eligible correlation. Use
   the checked two-factor marginals, feasible coupling,
   variance retention and replay gates. An exact zero axis
   uses one state. Reject inert covariance parameters and
   absent or unsupported active states. Prepare new typed
   marginal targets when a selected source changes. No
   calibrated model acceptance rests on base-time `D(t)`
   evaluations alone.
   [Ho–Lee calibration](../models-numerical/ho-lee-curve-fit.md#accepted-decision-for-checked-holee-curve-fitting),
   [Ho and Lee](https://onlinelibrary.wiley.com/doi/10.1111/j.1540-6261.1986.tb02528.x),
   [rates-credit marginals](../models-numerical/rates-credit-marginal-calibration.md),
   [joint coupling](../models-numerical/rates-credit-coupling-and-sampling.md).

5. **Apply conditional survival and recovery once on the
   entitled hold path.** For each live credit interval use
   the admitted outgoing applied discount `D_a`, local
   integrated hazard `H_i` and conditional survival
   `exp(−H_i)`. Discount and survival weight the conditional
   expected next value once. Use prompt fractional recovery
   `R × recoverable_par`, with independently qualified
   `R∈[0,1]`, authority and full exposure path. A hazard
   source's stored recovery supplies it only when explicitly
   qualified. Use eligible repayable par through its actual
   admitted cash-payment boundary, and capitalization and
   write-down changes on their economic dates. Keep this
   recovery view distinct from the economic principal book.
   Unfunded commitment, premiums, coupon/accrued cash and
   extinguished par have no recovery under this profile.
   Stop at the authorized end of exposure. Observed default
   requires its own recognized claim and valuation. Count a
   separately payable recovery once.

   For a qualified constant equivalent local window with
   `A=−log D_a`, add
   `R × recoverable_par × H_i × [−expm1(−(A+H_i))/(A+H_i)]`.
   The bracket has limit one at zero. Use local hazard
   increments even when survival rounds to one. For the
   deterministic credit route, integrate across exact civil,
   curve and exposure splits under the accepted local bridge,
   independently of the exercise grid. Use the declared
   constant-local model on stochastic intervals and identify
   that approximation. No recovery floor, clamp or inferred
   exposure from a cash amount.
   [Checked bond FRP windows and exposure](../valuations-bonds/03-pv-models.md#accepted-decision-for-checked-bond-pv-selection-and-fractional-recovery-of-par),
   [joint rollback and FRP](../models-numerical/rates-credit-coupon-folding.md),
   [Duffie on prompt par recovery](https://www.darrellduffie.com/uploads/1/4/8/0/148007615/duffiedefaultabletermmodels1998.pdf).

6. **Replay only supported node-dependent coupons.** Zero
   rate volatility uses the qualified deterministic forward
   cash and known fixings. With stochastic rates, admit the
   joint model's one simple in-advance term observation for each
   recomputed parent. Retain its exact fixing, actual index
   effective and maturity nodes, coupon accrual and payment.
   Normalize the conditional index gross to the explicitly
   bound market projection using the base rate marginal.
   Apply contractual bounds, spread and gearing once and
   compute the whole coupon with eligible principal. Fold
   that full coupon to its fixing as an alive joint unit claim
   and replace exactly its corresponding cash contribution,
   with survival and applied discount counted once. Retain
   fixed known coupons unchanged. Projection uses the base
   fitted rate factors and holds OAS and credit out.

   Require the parent amount and all payoff dependencies
   admissible on the represented state. Decline unresolved
   stochastic PIK, overnight or multiple-observation paths,
   and path-dependent principal, fee or payoff state outside
   this model. Reject a live right, or an intervening event
   changing the coupon's amount or ownership, that the fold
   cannot represent. At fixing, only an authorized forfeiture
   independent of the folded amount can extinguish it. At
   payment, its payable cash must be common to the admitted
   alternatives. Never delete a legal right to make folding
   succeed.
   [Accepted full joint coupon folding](../models-numerical/rates-credit-coupon-folding.md#accepted-decision-for-checked-joint-credit-rollback-and-coupon-folding).

7. **Use exact authorized borrower rights and payable call cash.**
   Bind whole-balance callable dates or standing windows,
   eligibility/notice and phase, replacement and termination
   terms, and actual payable redemptions. Use the admitted
   pre-terminal legal call window. An adjusted final cash date
   creates no additional exercise rights. A standing-window
   lattice prices exercise at the expressly admitted eligible
   grid nodes and is a finite-grid approximation. Retain all
   specifically required dates exactly. A hard protection
   window, an unconditional cash premium and a repricing-
   contingent soft premium have different eligibility and
   price terms. Require the soft-call condition actually
   resolved for the scenario. Unresolved conditions, ranking
   or notice/deferred exercise economics outside the supported
   immediate profile are unavailable. Authoritative already-
   exercised fixed claims keep their own dated basis.

   Compute redeemed par at the authorized phase after due
   common principal and applicable capitalization and before
   replacement redemption. Pay shared coupons, par cash
   and other vested claims once. Resolve the treatment of
   undrawn commitments, later draws and continuing fee rights
   on both alternatives. A full-discharge exercise requires
   explicit cancellation or a qualified surviving claim for
   each affected obligation. Use `p × N_ex/100 + A` for
   a qualified cash call, with `A` the actual additionally
   payable earned cash. Apply the contract's treatment of
   earned uncapitalized PIK to premium-bearing par or an
   independently payable claim as authorized. Count it once.
   Buyer quote accrued follows its separate settlement law.
   An immediate exercise amount carries no outgoing step
   discount or survival. Share the legal right and payoff
   with workout preparation. Record 09 owns workout selection.

   For an expressly qualified deterministic reference-strip
   make-whole use a named reference evaluator, actual
   benchmark dates and horizon, and the contractual reference
   spread on its qualified native clock. Under the admitted
   continuous law compute `P_ref=Σ C_i D_ref(E,T_i) exp(−s_ref Δc_i)`
   on the authorized strictly future benchmark cash. The clean
   floor/accrued profile pays `max(F_clean,P_ref−B)+A` where
   `B` is the contract's accrued deduction. Fees, future loan
   draws and unrelated cash enter only if explicitly in the
   benchmark. An unresolved future stochastic make-whole
   benchmark requires a qualified model.

   Use explicit nonnegative friction `f` in cents per 100 par,
   including zero. At the admitted simultaneous phase select
   the cheapest legally available complete borrower redemption
   on the common entitlement basis. Call only when hold value
   strictly exceeds `C + N_ex f/10000`. Hold on equality.
   Pay `C` on exercise. Friction is a decision threshold in
   the selected risky lender-value comparison. This exercise
   policy applies only after contractual eligibility is resolved.
   [Related payoff and make-whole law](../valuations-bonds/04-rates-tree-exercise.md),
   [filed repricing premium terms](https://www.sec.gov/Archives/edgar/data/1401680/000119312520048774/d842548dex991.htm),
   [filed make-whole agreement](https://www.sec.gov/Archives/edgar/data/1704235/000119312517252868/d375817dex105.htm).

8. **Define OAS and verify the returned dirty-price fit.**
   Use one signed annual decimal continuous overlay on ACT/365F
   for the admitted loan tree. Apply it to the authorized
   continuation discount, folded contingent coupon and prompt
   recovery once. Hold the calibrated base transitions and
   hazard, contractual projection, benchmark and settlement
   financing fixed. Recompute coupon-fold pricing and exercise
   under each trial. Fit at the original valuation origin to
   `J(s)−FQ_target=0`, rebuilding any trial-dependent buyer
   entitlement relative to the held claim terms. Keep observed
   marks, curve DM/Z and credit spread changes separate.

   Require explicit finite ordered bounds, positive work controls
   and positive absolute native-currency dirty residual tolerance.
   Use checked fallible Brent with its positive coordinate tolerance
   on a qualified continuous and strictly monotone bracket.
   Otherwise allow only an expressly selected bounded local
   fit by sign-preserving midpoint trials. Friction can create
   a jump in price. A bracket that merely straddles a jump
   does not pass. Admit an exact endpoint only on a qualified
   isolated branch. Reject an insensitive or unidentifiable
   objective and an unsupported closed-claim fit. Reprice the
   actual represented result and qualify `|J(s)/F−Q_target|`
   before display rounding. Preserve the actual spread,
   bracket, residual and local-fit status. Remove historic
   hardcoded controls, numeric sentinels and convenience zeros.
   [Accepted model OAS and fit gates](../valuations-bonds/12-spread-quotes.md#accepted-decision-for-checked-bond-spreads-and-price-inversion),
   [checked Brent](../core-numerics/brent-root.md).

9. **Share the checked preparation and result basis.** Use
   the same Rust admission for native, registry, quote and export
   calls. Resolve only dependencies active on the chosen basis.
   Preserve recognized fixing and settlement state, actual
   flows and balances, held source identity, model/clock,
   calibrated fit controls, currency, spread experiment and
   separate mark and raw basis. Share raw arithmetic where
   checked repricing requires it and finalize Money at the
   qualified output boundary. Propagate pricing and source
   failures without losing independently available results.
   Reconcile term-loan inputs, Python/WASM typed and generic
   calls, generated schemas, UI and downstream statements
   consumers. Route risk and yield selection under record 09.
   [Term-loan risk and yields](09-term-loan-risk-yields.md),
   [metric reporting](../valuations-framework/metric-orchestration.md).

## Observed behavior

### Choice and rationale

[Default dispatch](../../../../finstack-quant/valuations/fixed-income/src/term_loan/types.rs) selects Tree whenever a credit curve is supplied or a nonempty call schedule exists; otherwise Discounting. Explicit registry alternatives remain available. A supplied clean-price quote bypasses model valuation and gives a marked settlement value. The inferred rationale is that credit and borrower optionality must influence model value, while a quote is a distinct price driver; approval of these choices remains open.

### Scope and algorithm

The [discounting pricer](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/discounting.rs) first compiles the schedule. For seasoned simple floating coupons, historical resets require exact fixings. It applies covenant spread deltas effective at the accrual start and rescales the coupon by new/old rate; a near-zero old rate uses outstanding-before-payment times rate times accrual. Seasoned floating PIK is rejected because historic capitalization cannot be restated. Non-simple compounding follows the builder path instead.

At settlement, discard cashflows paid on/before settlement and PIK entries. A clean quote becomes dirty value on outstanding plus pending principal, with cash accrued added. Otherwise use quoted periodic z-spread/DM discounting or the unshifted curve. As-of value is `settlement_value × DF(as_of,settlement) + PV(cashflows in (as_of,settlement])`.

The [tree engine](../../../../finstack-quant/valuations/fixed-income/src/term_loan/pricing/tree_engine.rs) performs these stages:

1. Start the uniform grid at settlement and end at legal maturity; default 100 steps. Map tree times to nearest calendar dates on the curve day count. Ordinary cashflows split across floor/ceil nodes; exercise-date cashflows snap to the ceil node. Each piece receives a DF timing correction and an OAS offset so it retains its dated discounting.
2. A supplied hazard curve selects the two-factor rates-credit lattice, calibrated to discount and conditional-survival targets. Its volatilities and correlations come from `resolve_rates_credit_config`; unset factor volatilities are deterministic. No credit curve selects `ShortRateTree` with the Ho–Lee default configuration. Fixed-rate default sigma is `.01`, while floating-rate default sigma is zero.
3. Book coupon/fee and signed principal cashflows separately. Track outstanding before and after each event. Survival discounts continuation; add fractional recovery of post-payment outstanding using `continuous_frp_weight` over the interval.
4. Calls are standing provisions from each start date until replaced. Redemption is clean call price plus accrued cash; exercise claims account for accrued but not yet capitalized PIK. Make-whole prices use the named reference curve and contractual spread, floored by the call percentage. `Soft` and `Hard` have the same price/exercise rule.
5. Issuer exercise replaces hold value only when `hold > dirty_call + outstanding × call_friction_cents/10000`. On the short-rate lattice the decision is resolved across each level with the exercise-kink correction and, with friction, the cell-fraction drop of `models/src/trees/kink_smoothing.rs`; the rates-credit lattice decides node by node. A standing call with friction is exercised on every step, so its value depends on the step count with or without the correction. Coupon and principal booked at the node are added outside this continuation decision. Future stochastic floating resets are rebuilt from rate nodes; calls inside a reset-to-payment period are disabled.
6. OAS freezes the prepared tree and solves price minus dirty quote in basis points with Brent: tolerance `1e-6 bp`, 50 iterations, initial bracket size 1000 bp. Failed trials return a large numeric sentinel. The first underlying error is surfaced only if the solver fails. The returned spread has no independent dirty-price residual gate.

### Controls and unsupported combinations

Rates-only trees reject floating coupons with positive sigma and reject unused hazard parameters. Rates-credit stochastic floating PIK is rejected because its outstanding balance is path dependent. Live make-whole calls reject positive rate volatility because exercise-date reference curves are not modeled. OAS is decimal in quote overrides and converted to basis points internally. Tree returns zero if settlement is at/after maturity; as-of return is zero at/after legal maturity in the discounting wrapper.

### Limitations and alternatives

Call-grid exercise and the reset-boundary restriction approximate continuous borrower behavior. Friction is a threshold, not a cash fee retained by the lender. A zero-volatility lattice is available; there is no automatic accuracy-based grid refinement. The rates-only calibration and cashflow timing adjustment call `disc.df(t)` with settlement-relative times. Correct re-anchoring for a nonflat curve whose base differs from settlement is not established by the inspected evidence. Models that explicitly simulate borrower refinancing or stochastic make-whole curves are unimplemented alternatives.

### Evidence

[Tree-pricing tests](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/tree_pricing.rs), [pricing tests](../../../../finstack-quant/valuations/fixed-income/tests/term_loan/pricing.rs), and colocated tree tests exercise dispatch, timing and call behavior; they are not a universal convergence certificate. [Hull–White numerical procedures](../../../REFERENCES.md#hull-white-1994-numerical-procedures) and [Duffie–Singleton](../../../REFERENCES.md#duffie-singleton-1999) are reference context.

## Independent arithmetic and qualification

- A 90-day ACT/360 parent with balance `100` for 30 days,
  then `50` for 60 days, and actual fixed index-plus-margin
  `.05` pays `5/6`. Multiplying the full-period accrual
  by end balance gives `5/8`. A known `2.5%` PIK year
  on `1000` creates `1025` par and a later `.04` cash
  coupon of `41`.
- Holder PV `107`, seller-retained claim PV `5` and financing
  factor `.98` imply dirty buyer value `5100/49`. Eligible
  cash of `100` after legal maturity with factor `.98` is
  worth `98`, and at the pre-payment cut on its due date it
  is worth `100`. With a source's absolute factors `.98` at
  valuation and `.92` at a node, the relative target is
  `.92/.98`. A day-31 event on a 92-day uniform horizon fails
  alignment at 46 steps and aligns at 92 or 184.
- On a one-year credit example at flat `r=.03`, hazard
  `.04`, OAS `.01`, and recovery `.40`, receive par `50`
  at half-year and `54` at maturity. Recoverable par `100`
  then `50` gives promised PV `97.88775466249449`,
  recovery `1.16094214461041` and total
  `99.04869680710490`. Constant par `100` overstates
  recovery by about `.3767309277`.
- With par `100`, PIK `5` and common amortization `20`
  before exercise, the remaining par is `85`. A call at
  `101` plus earned cash `2` costs `87.85`. With common
  coupon `3` and amortization `20`, total cash is
  `110.85`. Friction of 50 cents per 100 makes the decision
  threshold `88.275`. At equality the borrower holds.
- For a root decision with hold `101 exp(−s)`, call `100`
  and friction threshold increment `.50`, the switch
  occurs near `s=.00496278934213`. A narrow bracket around
  that point straddles price `100.25` but has no dirty-price
  match. A separate bracket can match near `.00745345065458`.

Standalone exact and high-precision checks cover these cases.
Qualification still requires fixing and holder-state replay, actual
buyer rights and financing, exercise and legal benchmark fixtures,
fit and grid-convergence evidence, stochastic coupon and recovery
verification, and native/registry/host agreement. Repository pricing
and host suites were not run.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
