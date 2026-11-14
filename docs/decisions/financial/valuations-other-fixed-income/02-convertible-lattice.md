# VALUATIONS-OTHER-FIXED-INCOME-02: Convertible defaultable-equity lattice and exercise sequencing

- **Decision status:** Accepted 2026-11-14 — full checked convertible lattice, exercise and settlement package.
- **Audit date/source:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Reconciliation pending. The observed entry points and kernels are described below.
- **Validation status:** Live source and primary references inspected. Independent payoff, recovery and terminal lattice checks run. Runtime, contract, market and host qualification pending.
- **Scope inventory:** [README](README.md); [complete source mapping](coverage.json).

## Accepted decision for checked convertible lattice and exercise

1. **Select the model and market inputs explicitly.** Select the native
   one-factor issuer-equity CRR `Tree`, with deterministic same-currency
   pricing-measure discount and proportional dividend carry. Require
   a positive finite spot in native currency on the admitted issuer
   share basis, and a finite nonnegative annual continuous dividend
   yield `q`, with explicit zero allowed. Bind the curve to both the
   cash discount and equity financing roles under this model. Require
   the election of no default or one issuer risk-neutral hazard source
   with explicit finite fractional par recovery `R∈[0,1]`. Equity
   jumps to zero on default in the latter mode. Exchangeables, quanto,
   a second credit/volatility factor, a separate stochastic funding
   basis and a Tsiveriotis–Fernandes split-discount model require
   distinct methodologies.

   For the live diffusion elect a finite nonnegative annual lognormal
   volatility on ACT/365F. It may be an explicitly declared constant
   or one constant obtained from a designated Black strike-surface
   observation. For the surface bind its valuation origin, exact
   diffusion horizon date, qualified positive strike and equity
   basis. Query on the source clock and bridge terminal total
   variance by `σ_tree=σ_source√(T_source/T_tree)` using positive
   intervals. Carry forward the quote coordinates and source policy.
   Remove implicit strike selection, forced clamping and source/type
   fallback. One sample gives a constant-volatility proxy across
   nodes and dates. Already resolved claims and same-cut terminal
   payoffs require only their active economic inputs.
   [Ayache–Forsyth–Vetzal on default jumps and survivor drift](https://cs.uwaterloo.ca/~paforsyt/convert.pdf),
   [volatility source contracts](../core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks).

2. **Use a checked, exactly aligned lattice.** Take explicit positive
   steps and a positive node-evaluation budget on a live positive
   horizon. Check the complete triangular stochastic node count or
   the linear deterministic count and relevant recovery budgets
   before work. Use ACT/365F from the valuation date and phase to
   the last unresolved model-dependent decision. Retain all
   actual coupon, principal, exercise, eligibility and balance
   dates and their phase order. On a uniform grid with `N` steps
   and `D` civil days, require `N × elapsed_days = j × D` for each
   dated event or exercise opportunity included in the rollback.
   A contractual standing window permits exercise at eligible
   nodes in that window and is a finite-grid approximation.
   Exact scheduled dates and observation opportunities remain
   mandatory. Keep fractional-day numerical nodes distinct and
   interpolate their source clocks on the admitted civil-cell law.
   Value separately vested dated claims on their own clocks and
   payment dates, including dates beyond the last decision. A
   qualified fixed cash claim that vests on a terminal branch
   enters that branch at its dated claim value, with its own
   surviving recovery basis and actual cash date. Use
   the same rights resolver at a zero-time root or terminal phase.
   Reject off-grid events, unrepresentable or ambiguous phase
   order, exceeded limits and incomplete active source coverage.
   Remove nearest-step mapping, timing corrections, patched
   maturity tails, the 200-step default and hidden refinement.
   [Checked CRR and budgets](../models-numerical/equity-binomial-construction.md#accepted-decision-for-checked-crr-and-leisenreimer-lattices),
   [exact dated lattice events](../models-numerical/dividends-and-lattice-exercise.md#accepted-decision-for-checked-escrowed-dividends-and-lattice-exercise).

3. **Compensate surviving equity drift and recover eligible par once.**
   On a step of model length `h`, let `ℓ=log D_i` be the relative
   discount log and `H_i` the local integrated issuer hazard
   (zero in the no-default mode). Use `b=−ℓ+H_i−q h`,
   `a=σ√h`, `u=exp(a)`, `d=exp(−a)` and
   `p_up=(exp(b)−d)/(u−d)`. For stochastic steps require strict
   `−a<b<a` and use checked stable complementary branch weights
   from MODELS-NUMERICAL-001. With exact zero volatility, advance
   one surviving state by `S_next=S exp(b)`. In both cases
   `D_i exp(−H_i) E_alive[S_next]=S exp(−q h)`. Reject nonfinite,
   unrepresentable or inadmissible evolution without probability
   clipping, arbitrary volatility/spacing floors or model changes.

   Continue at `exp(ℓ−H_i) E_alive[V_next]` plus the present value
   at step start of prompt default recovery. On the hold branch
   integrate `R × recoverable par` against discounted conditional
   default mass, splitting at actual exposure and relevant curve
   boundaries on each source's native clock. Use the checked
   BONDS-03/CREDIT-12 local log-linear discount and constant
   hazard bridge for both step factors and recovery. Use explicit
   recoverable par before each intervening payment/exercise,
   with balance updates effective at their contractual phase.
   Equity, coupon, accrued, premiums and extinguished par have
   zero recovery basis in this profile. Use local hazard
   increments and checked scaled arithmetic. Remove the
   smoothed diagnostic cash/equity split. Any separate result
   for a resolved cash or share claim follows the actual
   entitlement and valuation law.
   [Bond fractional recovery of par](../valuations-bonds/03-pv-models.md#accepted-decision-for-checked-bond-pv-selection-and-fractional-recovery-of-par),
   [shared checked hazard windows](../models-and-valuations-credit/12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis).

4. **Resolve all-in exercise and co-dated cash on one contractual
   basis.** Prepare eligible, identified whole-balance rights and
   original cash entitlements once. Source fixed and supported
   floating coupons from the shared checked schedule, with the
   accepted fixing, projection and principal replay conventions.
   A projected floating coupon must be deterministic under the
   selected sources. Admit immediate share conversion,
   immediate cash put/call and specifically supported deterministic
   make-whole redemption. Keep unpaid vested cash and same-phase
   cash common to all eligible alternatives exactly once outside
   the choice. Branch-dependent redemption/accrued and retained
   entitlements belong only to their payable branch. Scheduled
   principal is paid once on the applicable hold/redemption branch
   and extinguished on conversion. Coupon entitlement comes from
   the original terms, including where record date, accrual stop,
   exercise and payment dates differ. Remove automatic terminal
   coupons and date-only payoff sequencers.

   For the supported simultaneous noncrossing profile, let
   `B=max(hold, eligible voluntary conversion, eligible put)`,
   with unavailable alternatives omitted. For each eligible
   call `i`, compute its all-in payable cash branch and, if
   separately granted, its holder conversion-on-call branch on
   the same entitlement basis. The holder takes the larger
   applicable branch. The issuer chooses the minimum of these
   complete call responses `C`, with call only for `B>C`.
   Reject incompatible crossings between unconditional holder
   rights and the issuer cap, as well as unsupported priority.
   Exercise requires strict improvement for the acting party,
   with continuation preferred on a voluntary tie. On a call
   response tie select payable cash when terms leave the holder
   free to choose. Use stable authored right order for otherwise
   tied equivalent same-side choices. A call response can open
   independently of ordinary conversion. For admitted make-whole
   cash use the qualified BONDS-04 deterministic reference strip,
   explicit spread and clean floor/accrued law. Competing
   priorities, live notice and future path-dependent or
   discretionary settlement methods need a separately qualified
   profile.
   [Canonical schedules and balance replay](../cashflows/12-schedule-lifecycle.md),
   [floating coupon replay](../cashflows/08-floating-replay.md),
   [Payable make-whole amounts and cash rights](../valuations-bonds/04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise),
   [filed conversion and redemption terms](https://www.sec.gov/Archives/edgar/data/1392972/000139297225000126/indenturenotesdue2030.htm).

5. **Admit only explicit triggers and mandatory delivery terms.**
   Evaluate a qualified single-observation spot condition at its
   authorized node or use an authoritative realized observation
   known at the cut. Require the actual observation basis,
   comparator and effective right window, including an expressly
   specified multiple of the effective conversion price where
   applicable. IPO/change-of-control and historical soft-call
   eligibility enter only as resolved facts that establish the
   relevant right for its actual interval. An unresolved live
   right that depends on future price history, rolling counts,
   VWAP, another event state or a notice/election fails model
   admission. Remove the BGK trigger uplift and presumed
   inactivity for unsupported contingent rights.

   Treat compulsory whole-balance delivery as the only exercise
   choice when the contract grants that priority. Resolve the
   due coupon and other surviving claims by actual entitlement.
   Fixed-ratio delivery scales the authorized shares per
   denomination by eligible outstanding par once. Admit an
   explicitly authored single-spot mandatory collar only where
   its delivered shares on eligible par `N_e` are `N_e/L` for
   spot below `L`, `N_e/S` within `[L,U]`, and `N_e/U` above
   `U`, for same-currency finite `0<L<U`. Its native payable
   value follows `N_e S/L`, `N_e`, and `N_e S/U`. The lower
   branch fixes the maximum shares, and the upper branch fixes
   the minimum shares while delivery value still rises with
   spot. Carry only an authorized rounding or
   fraction rule. Averaging, mixed delivery, partial exercise
   and different contractual collars require their own
   qualified payoff.
   [Filed observation-count redemption test](https://www.sec.gov/Archives/edgar/data/2070457/000121390026040725/ea0285344-424b3_twenty.htm),
   [filed mandatory delivery using averaged VWAP](https://www.sec.gov/Archives/edgar/data/1341439/000119312526039344/d63638dex31.htm).

6. **Apply conversion adjustments at the effective cut.** Bind a
   positive finite canonical conversion price per share or ratio
   per stated denomination with the reciprocal representation
   derived from that state. Require an unambiguous authorized
   starting state and apply each realized adjustment known at the
   cut once, at its legal effective date and phase, in contractual
   order. Require explicit order for interacting same-phase
   events. For admitted down-round issuance terms with old
   conversion price `K`, qualified issue price `P`, previous
   share basis `O>0` and issued shares `M>0`, use full ratchet
   `min(K,P)` or, only for `P<K`, weighted average
   `(O K+M P)/(O+M)`. The weighted-average profile leaves `K`
   unchanged for `P≥K`. Qualify consideration, counted shares,
   exceptions, price and rounding to the actual terms before
   calculation. Evaluate the two weighted contributions using
   checked, scaled arithmetic. Remove the arbitrary price floor.

   Proportional dividend yield is equity carry. Remove automatic
   yield-based ratio or price accretion. Use only already effective
   contractual dividend protection or other adjustments through
   a qualified resolved state with explicit event provenance.
   Require an explicit assumption excluding additional modeled
   resets for the frozen live-tree scenario. If a known pending
   action or contractually material reset requires evolution the
   tree does not cover, require another qualified model. Spot,
   dividend carry, current conversion terms and dated strike
   consumers must share reconciled share and adjustment bases.
   [Filed issuance adjustment terms](https://www.sec.gov/Archives/edgar/data/1407038/000119312515294698/d21473dex31.htm),
   [filed dividend protection and its reference-price terms](https://www.sec.gov/Archives/edgar/data/1335258/000133525823000055/ex101indenture3125converti.htm).

7. **Value only surviving claims after a realized event.** Require
   the valuation phase and explicit live, pending-delivery, closed
   or default state as applicable. Bind authoritative exercise,
   delivery, payment and entitlement state. A passed mandatory
   date, call, put or lifecycle end with insufficient state is
   unavailable. An accrual-end date alone confers no surviving
   coupon. For a qualified pending fixed delivery of `m` shares
   and `C` cash with `τ` ACT/365F to delivery, use
   `m S exp(−qτ) + C D exp(−H) + FRP`, with recovery on the
   pending cash claim's own explicitly stated recoverable par.
   The prepaid share value follows the stock martingale with
   zero equity recovery. The old bond, option and par recovery
   cease on the effective exercise; no later dilution adjusts
   the fixed share count. Use exact phase at delivery, factor
   one for payable same-cut claims and zero only for the verified
   extinguished position. Qualify separate vested claims on
   their original dates and rights. A known default requires an
   explicitly resolved claim; the pre-default tree supplies no
   legal state or after-default recovery estimate.
   [Observed pending-claim route](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/engine.rs#L336),
   [shared holder claim selection](../valuations-bonds/02-settlement-and-clocks.md).

8. **Use one prepared Rust valuation contract.** Share the validated
   model, conversion state, cash and exercise preparation across
   native and registered pricing and existing consumers. Preserve
   currency, stock and claim basis, actual dated entitlement,
   effective adjustment cut, model/data identities and selected
   work controls. Surface unavailable rights, missing active
   inputs, incompatible terms and failed numerical evaluation
   distinctly from genuine zero value. Reconcile cashflow views,
   parity/premium and spot/volatility strike consumers, wire
   types, schemas, Python, WASM and UI without duplicate financial
   policy. Quote carry, Greeks, conversion sensitivity, implied
   volatility, OAS and regime choice remain decisions in record 03.
   [Convertible quotes and risk](03-convertible-quotes-risk.md).

## Observed behavior

### Choice and rationale

The native/default `Tree` route calls a defaultable-equity CRR lattice in [credit_tree.rs](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/credit_tree.rs), with cashflow/exercise mapping in [valuator.rs](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/valuator.rs). Equity jumps to zero at default and surviving equity drift includes hazard. Both total value and the diagnostic cash component use the same survival-weighted discounting plus fractional principal recovery. This is not a Tsiveriotis–Fernandes split-discount implementation, despite cash/equity bookkeeping and older terminology in surrounding documentation.

### Scope and algorithm

1. Validate contract, ratio/price consistency, dated conversion events, positive tree steps, spot currency and recovery. Build the coupon/principal schedule. Process time is Actual/365 Fixed; default grid has 200 steps. Volatility comes first from explicit implied-vol override, otherwise a Black-lognormal strike surface at effective conversion strike/maturity or a unitless flat scalar. Dividend yield is zero only when no identifier is configured; configured missing/wrong-type data errors.
2. Use constant CRR up/down factors for recombination. At each step infer conditional growth from dated risk-free DF and conditional survival: `r_step+lambda_step=−ln(DF×S_step)/dt`; branch probability is `(exp((r_step+lambda_step−q)dt)−d)/(u−d)`. Invalid probabilities fail through the shared evolution helper.
3. Dated flows map to the nearest Act/365F step, with discount-and-survival timing correction; post-maturity principal gets a recovery tail adjustment. Recovery within a step is split at actual balance events and integrated through `continuous_frp_weight` on outstanding principal. Explicit recovery in `[0,1]` is required whenever credit curve is supplied.
4. At maturity choose optional conversion versus cash principal/put, or force mandatory conversion; always retain scheduled coupon. Only the diagnostic cash/equity split is smoothed across one terminal log-spot cell; total payoff remains the maximum.
5. Backward step: `continuation = DF×S_step×E[next_total]+recovery`. Apply allowed voluntary conversion, then issuer call with the holder's allowed conversion response, then holder put; finally add coupon/principal due at that node. Mandatory conversion takes precedence and extinguishes call/put alternatives.
6. Conversion windows map dates to grid steps. Price-trigger conversion uses current node spot; IPO/change-of-control conversion remains inactive without a realized settlement state. Mandatory-variable payoff is `face×S/lower` below lower strike, face between bounds, and `face×S/upper` above upper strike. Scale conversion by outstanding/face.
7. Dividend protection accretes conversion ratio as `exp(max(q,0)t)` while leaving stock drift at `r−q+lambda`. Soft-call days-above conditions are approximated by a trigger uplift `1+β_BGK × required_days/observation_days × sigma × sqrt(1/252)`; no observation history is counted.

### Contract adjustments and controls

[Types](../../../../finstack-quant/valuations/fixed-income/src/convertible/types.rs) apply all ordered dilution events before pricing: full ratchet uses the minimum issue price; weighted average updates `CP←CP×(old_shares+new_money/CP)/(old_shares+new_shares)`. The resulting ratio is face/CP. Events are not filtered by `as_of`; their dates are validated but the full recorded list affects today's conversion ratio. CP below `1e-10` errors. Make-whole calls use deterministic reference-curve PV floored by clean call percentage plus accrued. Call/put overlaps resolve to minimum call and maximum put strikes per grid node.

### Limitations and alternatives

A one-factor equity lattice has deterministic rate, hazard and volatility term inputs, no equity-credit correlation factor, no default recovery in stock, and no true soft-call path memory. Future dilution events can affect current value before their event dates; the input contract must specify whether records represent already-effective events or future schedules. Grid mapping, terminal split smoothing and dividend protection are distinct choices requiring validation. A TF split model, two-factor credit model, PDE or path-dependent simulation would be alternatives, not currently registered substitutes.

### Evidence

[Pricing tests](../../../../finstack-quant/valuations/fixed-income/src/convertible/pricing/tests.rs) inspect exercise ordering, martingale/default behavior and numerical regressions; [type tests](../../../../finstack-quant/valuations/fixed-income/src/convertible/types.rs) inspect dilution ordering and collapsed price rejection. [TF](../../../REFERENCES.md#tsiveriotis-fernandes-1998), [Ayache–Forsyth–Vetzal](../../../REFERENCES.md#ayache-forsyth-vetzal-2003) and [BGK](../../../REFERENCES.md#broadie-glasserman-kou-1997) are comparison references, not proof of equivalence.

## Independent arithmetic and qualification

- With one year, flat annual `r=.03`, `λ=.04`, `q=.02`, `σ=.20`,
  `R=.40` and spot/par both `100`, the one-step up weight is
  `.5774931963561241`. Prompt recovery is `1.545284116435469`,
  the debt-only PV is `94.78466610703029` and optional terminal
  conversion at one share gives `106.70612169373337`. The
  zero-volatility convertible gives `99.56515144711100`. One
  mandatory share with zero recovery gives `98.01986733067553`
  from either the rollback or prepaid share formula.
- In the same optional terminal-only example the 200-step and
  1,600-step sums give `105.01851980848289` and
  `105.02708927962462`, against `105.02831425976798` from
  deterministic survival applied to par plus the conditional
  Black call, with separate prompt recovery. For a 365-day
  horizon and day-30 event, 200 steps fail exact alignment;
  365 and 730 steps put that event at nodes 30 and 60.
- With `N_e=1000`, `L=100` and `U=120`, spots `80`, `110`
  and `150` produce mandatory values `800`, `1000` and
  `1250`. A holder floor of `115` and issuer cap of `110`
  cross and fail the admitted simultaneous profile. An issuer
  call with cash `100` and conversion response `130` loses
  the issuer's comparison with cash `105` and response `110`.
- With `K=50`, `O=100` and `M=20`, a weighted down round
  at `P=40` gives exactly `145/3`. At `P=80` the accepted
  profile leaves `K=50`. For a resolved claim
  of ten shares, spot `100`, one year, flat `r=.05`, `λ=.10`,
  `q=.03`, cash `100` and `40%` recovery on par `80`,
  the share leg is `970.445533548508` and the cash plus
  recovery leg is `89.042360812105`.

Standalone exact and high-precision calculations exercise these
examples. Qualification still requires legally supported right and
entitlement fixtures, dated curve/recovery-window checks, grid
convergence across live exercise schedules, independent market PV
comparisons, and native/registry and host reconciliation. Repository
pricing tests and market repricing were not run for this review.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#other-fixed-income).
