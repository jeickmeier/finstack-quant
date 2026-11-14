# VALUATIONS-FRAMEWORK-13 — Horizon theta, market-held fixing carry decomposition, and breakeven solves

- Decision status: **Accepted — complete checked horizon theta, carry and breakeven package, 2026-11-14**.
- Implementation status: **Observed, with reconciliation pending** across the horizon, carry and breakeven producers, product routes, attribution and hosts.
- Validation status: Source and primary references inspected. Independent bounded cash boundary, date conditioning, carry, funding and quote-breakeven examples checked. Runtime, product and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked horizon theta, carry and breakeven

1. **Resolve the horizon and both cuts explicitly.** Use the prepared
   effective opening date, information and settlement phases. Require a closing civil
   date or an explicit positive calendar tenor and the closing information
   and settlement phases. Resolve D/W/M/Y directly under
   [CORE-CONVENTIONS-007](../core-conventions/007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions);
   this shared tenor profile is unadjusted. A product or workflow that
   needs a business date supplies its separately resolved endpoint.
   Require a positive civil span. Remove the implicit 1D default and
   last-payment/expiry clamp. Reprice the requested horizon through an
   admitted lifecycle; if it is after termination, admit a closed zero
   closing PV only when the product shows that no claim or delivery
   remains. An unpaid amount or delivered position can outlive expiry.
   An unsupported endpoint is unavailable; missing required data errors.

2. **Prepare one comparable holder experiment for each horizon metric.**
   Use the same signed quantity, currency, holder entitlement, selected
   model and raw or explicitly admitted Money amount basis at opening and
   closing. Freeze actual history and all inputs except those named by
   the horizon policy. A quoted, clean, buyer-settlement or trade mark
   needs an owner-qualified bridge to the dirty/economic holder basis.
   Retain detached unpaid receivables in closing PV or an explicit
   bridge. Apply a qualified position or valuation adjustment once on
   the same basis through PV, cash and funding. Do not mix adjusted
   opening PV with an unrelated raw or default-dispatch reprice.
   [Valuation dispatch](pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle),
   [bond entitlement and quote dates](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf).

3. **Separate frozen sources from projected observations.** Freeze the
   opening information set. Resolve the exact fixing and observation
   keys needed by the product and use actual observations eligible at
   that cut. Project only future or yet-unpublished values admitted by
   the product from the opening source. Materialize them in a private
   scenario with their origin and hypothetical status. A later phase can
   make a projected observation applicable; it does not make it actual.
   Preserve eligible history and fail missing required past observations
   according to the owner. Use held spot, exercise, barrier, delivery,
   or other path rules only where the product admits those assumptions.
   Obtain both period cash and the closing claim from the same scenario,
   including owner-qualified contingent payments. A placeholder schedule
   does not establish zero cash. [Publication eligibility](../conventions/0045-financial-clocks-and-date-boundaries.md#accepted-amendment-volatility-clocks-and-fixing-eligibility),
   [hypothetical fixing materialization](../cashflows/16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization),
   [New York Fed publication example](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates).

4. **Select dated market evolution by active family.** Horizon theta
   uses the admitted target-date conditional views under
   [CORE-MARKET-20](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle).
   Condition at the actual closing date, using each source's own
   reference and evaluator. Do not add elapsed days to each source's
   reference. Carry holds the opening dated market; evaluate the actual
   remaining horizon-relative factors, for example `D(T)/D(H)`
   for a retained discount evaluator. A conditioned view and a held
   dated evaluator can be exactly equivalent on those queries.
   A constant-maturity reset or any other evolution needs a separately
   defined experiment. Require qualified active FX, volatility, model,
   correlation and credit-source behavior at the horizon. A cloned
   provider does not establish frozen FX, and advancing the date does
   not re-age a smile, recalibrate credit, or publish observations.

5. **Count signed settlement cash once between the cuts.** Use the
   scenario's complete economic holder ledger. When both PVs include
   eligible same-day cash, use `[open,close)`; when both exclude it,
   use `(open,close]`. For different phases resolve each endpoint
   separately. Preserve payment date, the actual holder entitlement,
   signed quantities and pending settlement; detachment alone is
   not receipt. Income includes admitted coupon, interest, accrued-on-
   default and fee cash, including margin interest when it belongs to
   the position. Other economic cash includes principal and exchanges,
   draws and repayments, exercise settlement, recovery and protection
   where applicable. PIK and write-downs have no settlement cash.
   Keep collateral transfers, financing flows and unrelated acquisitions
   outside this holder return unless the owner defines their separate
   economic treatment. Exclude an opening acquisition event only by
   verified identity and basis, not by its sign or date alone.
   Convert each foreign cash row once using the explicit ordered
   payment-date FX query and scenario policy, with qualified source
   coverage. Use checked Money and explicit rounding; a same-currency
   row needs no FX quote. [Cashflow semantics](../core-conventions/015-cashflow-semantics.md#accepted-decision-cash-settlement-principal-effects-and-accrual-metadata),
   [FX queries](../core-conventions/013-fx-resolution-and-shocks.md#accepted-decision-fx-resolution-state-and-relative-shocks).

6. **Define horizon theta as a period total.** With opening PV `V0`,
   conditioned closing PV `Vθ` and nominal signed period cash `C`,
   return `theta=C+Vθ−V0`. Report `theta_carry=C`,
   `theta_roll_down=Vθ−V0` and exact elapsed civil days under
   `theta_period_days`, together with both resolved cuts and the
   experiment basis. The PV component is the closing-minus-opening
   change; it need not isolate a curve-shape effect. Do not discount,
   reinvest or charge funding on cash in this theta. Keep local
   diffusion/time Greeks and local fixed-market reprices separate.
   A one-day horizon does not make them interchangeable.
   [Record 11](option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).

7. **Separate gross carry, funding and the optional yield split.** Let
   `VH` be the qualified held-dated-market closing PV. Compute gross
   carry directly as `C+VH−V0` and net `CarryTotal` as gross
   less the selected funding charge. Preserve signed income `I`
   and other economic cash `C_other=C−I`; the latter can include
   nonprincipal receipts. Select unpartitioned carry explicitly
   when no split is requested. Report the total and its supported
   components, leaving pull-to-par and yield-benchmark roll-down
   unavailable. A requested split requires an admissible product-owned
   quote, yield convention, clock, scope, cash entitlement and
   settlement bridge that reproduces the opening dirty amount.
   Keep that yield fixed and reprice the same holder basis at the
   horizon to get `VY`. Define `pull=VY−V0+C_other` and
   `roll=VH−VY`. Then `I+pull+roll` reconciles to gross.
   Roll is the difference from the qualified yield benchmark;
   it isolates a curve-shape effect only on a controlled equivalent
   basis. Missing or failed required yield never becomes zero pull.
   Remove the generic annual/Act365F and invented ICMA fallback
   and universal negative-yield warning. The quote owner admits
   negative yields or rejects them in its own convention.
   [Bond yield and settlement pricing](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf),
   [fixed-yield versus curve carry](https://arxiv.org/pdf/2201.01330),
   [bond carry and quote ownership](../valuations-bonds/19-carry-and-life.md).

8. **Qualify accumulation and select funding explicitly.** Where the
   owner verifies the factor-ratio identity, let `A(t)` be opening
   yield accumulation to `t`. The constant-yield mark can use
   `VY=V0 A(H)−Σ C_j A(H)/A(t_j)` for the same eligible signed
   cash rows. The intermediate-cash adjustment belongs only to the
   split; gross carry counts cash at par. Where the exact convention,
   settlement bridge or remaining cash does not permit that identity,
   use the admitted constant-yield horizon pricer or reject the split.
   Select unfunded or an explicit signed opening funding amount
   `B` and positive finite same-currency dated funding factor
   `D_f(open,H)`. Deduct `F=B(1/D_f−1)`. Full opening-PV funding
   is one explicit assumption. Retain negative rates and signed
   funding credits where admitted. Do not infer a financed amount
   from a curve ID or assume haircuts, cash reinvestment, interim
   repayments or margin. Count contractual funding cash and the
   modeled overlay on their qualified separate bases without duplication.
   [ICMA repo economics](https://www.icmagroup.org/market-practice-and-regulatory-policy/repo-and-collateral-markets/icma-ercc-publications/frequently-asked-questions-on-repo/1-what-is-a-repo),
   [repo product ownership](../valuations-rates/15-repo.md).

9. **Give both breakeven modes one horizon and one target.** Require
   explicit target and mode. Reuse the prepared carry horizon,
   signed net carry `C_net`, exact entitled remaining claim, chosen
   model, scenario and holder amount basis. Fix earned cash, funding,
   and qualified observations across trials. Bind the center and
   quote-to-price operation; require zero shift to reproduce the
   qualified horizon base. A settlement quote needs the exact anchor,
   workout, entitlement and dirty-to-holder PV bridge. A pinned mark
   may be changed only by an explicit owner rule with center
   reproduction. Solve the objective
   `F(δ)=C_net+V_H(center+δ)−V_H(center)` on its declared units
   and domain. Retain these target identities only for admitted routes:

   | Target | Horizon price operation and result increment |
   | --- | --- |
   | Z-spread | Exact entitled cash and owner spread application with required compounding and anchor. Report a spread move in bp. No hazard CS01 or generic discount proxy. |
   | Quoted YTM | Exact owner quoted-yield price, held remaining cash/workout, quote clock and settlement bridge. Report a yield move in bp. |
   | OAS | Actual spread input in the selected option model, held calibration and admitted exercise response, with exact quote compounding. Report an OAS move in bp. No Z-spread or CS01 substitute. |
   | Parallel zero rate | Separately named direct continuous-zero move on the exact selected typed rate scope. Report a rate move in bp, including eligible swap routes previously identified as YTM. |
   | Implied volatility | Exact active authored scalar sigma or qualified compatible native sigma grid direction. Preserve quote type and clock bridge. Report Black and shifted Black increments of `.01`; Normal requires an explicit native increment. No auxiliary grid or parameter proxy. |
   | Base correlation | Linear only on the admitted direct knot direction with whole-vector domain checks. Report in `.01` absolute correlation increments. |

   Preserve the quote owners for [Z-spread and OAS](../valuations-bonds/12-spread-quotes.md#accepted-decision-required-quote-compounding),
   [quoted yield](../valuations-bonds/10-quoted-yield.md),
   [rate and native volatility shocks](finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk),
   [native volatility scope](option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities),
   and [base correlation](../models-and-valuations-credit/19-tranche-risk.md#accepted-decision-for-checked-tranche-risk-and-default-scenarios).
   An active hazard alone neither admits nor disqualifies an actual
   spread target. Support requires the exact product operator.

10. **Qualify the linear answer and the iterative root separately.**
    Linear mode returns `−C_net/m_H` with a same-target horizon
    slope, from a qualified analytic derivative or a checked explicit
    stencil. Preserve units and one-time normalization. Remove
   the universal sensitivity floor. Reject zero or unresolved slope
   using the applicable owner price allowance and stencil widths. Reject
    inadmissible output, or a required move lost to representation.
    A linear estimate is not a residual-certified full price solution.
    Iterative mode needs no opening or horizon slope or linear prepass.
    Require a finite admitted bracket, continuity and a root identity
    or owner root selection rule, and explicit coordinate, work and
    money residual controls. Use the
    [checked Brent kernel](../core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search).
    Propagate missing target, unsupported boundaries, failed reprices
    and ambiguous or unresolved objectives. Apply the separate gate
    `abs(F(returned represented δ))+priced-PV allowance ≤ cash tolerance`
    after repricing the actual returned shift. Do not manufacture a
    zero sign from a price uncertainty interval. Zero carry may return
    an explicitly admitted no-shift result after target and center
    validation, without claiming a unique root. A nonzero carry on an
    unresponsive route has no qualified breakeven. Report signed shift,
    target, units, mode, horizon, and applicable fit, stencil or solver
    evidence. [Brent's method](https://maths-people.anu.edu.au/brent/pub/pub005.html).

11. **Keep each producer's horizon and result through existing consumers.**
    Carry has its own exact horizon provenance. Reserve
    `theta_period_days` for dated theta. Take the date and phase
    from the actual prepared endpoints, not a date-count match or
    a missing-stamp assumption. Do not prorate totals or substitute
    a local Greek for a dated change. Attribution must match the
    actual endpoint basis and distinguish forecast carry from
    realized cash. Where its owner selects gross carry, add back
    only the same checked funding deduction once. Portfolio and
    composite aggregation require compatible measures, currencies,
    periods and scenario basis; do not sum breakeven roots or
    elapsed-day stamps. A portfolio breakeven needs its own admitted
    joint price objective. Reconcile Rust producers, metric dependencies,
    portfolio, attribution, scenario endpoints, Python/WASM, result
    and configuration schemas, and parity using canonical Rust logic.
    Preserve structural zero, unavailable components and failures
    without sentinel fallbacks. Qualify terminal cash and delivery,
    source conditioning, signed funding and exact target reprices,
    and host behavior before treating them as implemented.
    [Metric basis and aggregation](metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting),
    [attribution carry ownership](../attribution/03-total-return-carry.md).

## Observed behavior

### Choice and conventions

Horizon defaults to one calendar day, may be overridden by a tenor, is unadjusted for business days, and is capped at the instrument's last payment date. A date at/before as-of gives zero. Outputs are **period totals**, with actual elapsed calendar days recorded separately. These algorithms distinguish rolling curves for theta from holding dated curves for carry decomposition; they are not alternative names for the same result.

### Theta and period cash

Reprice the base using instrument-signed economics. Project fixings crossed by the horizon from the original schedule, insert them in market history, then roll curves by elapsed days. Obtain product observation state implied by holding current spot, reprice that observed instrument at the rolled date, and return `cash_received + rolled_PV − base_PV`. Store cash carry, PV roll-down and realized horizon.

Cash classification includes cash coupons/fees/default-accrual/margin interest as income; principal, amortization, prepayment, revolving draws/repayments and recovery additionally count as economic cash. PIK, write-downs and collateral/IM/VM movements are excluded. Exclude negative issue-date notional draws identified by the instrument. Instruments excluding as-of cash use `(start,end]`; instruments including as-of cash use `[start,end)`, avoiding duplicate boundary cash across rolls. Convert each cashflow at its payment date into the base currency; cash is not discounted or reinvested in theta.

### Carry decomposition

Use held projected fixings but **unrolled dated curves** to price at the horizon. Income is cash income above; principal is total economic cash minus income. Default yield clock uses `(1+YTM)^τ` with context day count or Act/365F and an annual-frequency ICMA reference period where needed; yields at/below −100% fail, below −50% warn. Products may register a custom yield clock matching their quote convention.

For horizon accretion `A` and payment-date accretion `A_j`, compute reinvestment adjustment `Σ cash_j(A/A_j−1)`. Pull-to-par is `base_PV(A−1)−income−adjustment`. Roll-down is `horizon_PV−base_PV+principal−pull_to_par`. Funding cost is `base_PV(1/DF_repo(as_of,horizon)−1)` when the instrument names a repo curve, otherwise zero. Total carry is `income+pull_to_par+roll_down−funding`. If YTM is unavailable, pull-to-par becomes zero and roll-down absorbs the price move, with `CarryDecompositionDegenerate=1`. This fallback is reachable through normal registry resolution because a missing/inapplicable YTM dependency is skipped. A registered YTM calculator that fails still propagates its calculation error.

### Breakeven selection and solver

An explicit target/configuration is required. Linear mode returns `−carry_total/sensitivity`, rejecting absolute sensitivity below `1e−12`. Targets map ZSpread/OAS→CS01, YTM→DV01, ImpliedVol→Vega, BaseCorrelation→Correlation01, so reported changes follow those sensitivities' bp/vol-point/correlation-point units.

Iterative mode holds fixings and curves at the horizon and solves `carry_total+PV_bumped(horizon)−PV_base(horizon)=0` using fallible Brent, initial guess from the linear result, tolerance `1e−8` and 50 iterations. ZSpread/OAS shock the first discount curve; YTM shocks every unique declared discount/forward curve; implied vol shocks all present declared surfaces by `delta×0.01`. Base-correlation iteration is unsupported. ZSpread/OAS iteration is rejected when credit dependencies exist because quote-space CS01 would not match a discount-shift solve. Implied-vol iteration does not bump a scalar instrument vol override, even though Vega can. The named YTM target is implemented as curve shifts rather than an instrument yield-quote rebuild.

## Evidence and alternatives

[theta.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/theta.rs), [carry_decomposition.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/carry_decomposition.rs), [period_cash.rs](../../../../finstack-quant/valuations/core/src/metrics/period_cash.rs), [breakeven.rs](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/breakeven.rs), and [breakeven inputs](../../../../finstack-quant/valuations/core/src/instruments/breakeven.rs) own these rules. Inline tests are behavioral/formula evidence, not executed. [Hull](../../../REFERENCES.md#hull-options-futures) provides carry context; the source also cites Tuckman–Serrat for constant-yield decomposition, but a dedicated canonical adoption reference for this precise split remains needed.

Alternatives such as financed cash reinvestment theta, business-day horizons and instrument-quote yield solves are not silently implemented here.

The current theta path uses observed-state cloning for closing valuation
and reads cash from the original schedule. Some barrier and exercise
products have placeholder cashflow providers. The current carry path also
writes `theta_period_days`; metrics-based attribution can read that same
field for either metric, or assume one day when it is absent.
The [attribution consumer](../../../../finstack-quant/attribution/src/metrics_based/carry.rs)
and [portfolio endpoint preparation](../../../../finstack-quant/portfolio/src/attribution.rs)
need reconciliation along with the metric producers.

### Independent review evidence

- At zero rates, same-currency payment rows of `3` at opening,
  `5` and `7` inside the horizon, and `100` at closing give
  period cash of `15,115,12,112` for same-day inclusion at
  (both cuts, opening only, closing only, neither), respectively.
  Closing PV is `100,0,100,0`; in each case
  `cash+closing PV−opening PV=0`. Separately, foreign cash of
  `+5` and `−5` EUR at payment-date quotes of `1.10` and `1.20`
  USD/EUR gives `−.50` USD after conversion despite net zero EUR.
- At source time `2`, `3` and `5`, take original factors
  `.97,.95,.90`. An explicit closing reference at time `3`
  gives relative factor `18/19` to time `5`. An incorrectly
  shifted pre-existing source origin giving reference time `2`
  would instead yield `90/97`.
- With compatible annual accumulation at 10%, opening at year zero,
  closing at year two, cash of `30` (income `8`, principal `22`)
  at year one and `90` at year three, opening PV is about
  `94.89105935` and both qualified horizon marks are `81.81818182`.
  Pull is `8.92712246`, roll is zero and gross carry is
  `16.92712246`. Funding of `80` with factor `25/26` costs
  `3.20`, leaving net carry `13.72712246`. With simple interest
  at 10%, original factor ratio `D(0,3)/D(0,2)=12/13` differs
  from the re-anchored one-year factor `10/11`.
- For a `100` claim at annual yield 4%, with three years left at
  opening, two at the horizon and net carry `+3`, the opening
  slope proxy gives `+116.985856` bp. The matching horizon slope
  gives `+168.7296` bp. The full horizon yield solution is
  `+172.950083` bp; the opening proxy leaves about `+.95458`
  currency units unoffset at the horizon. At the same two-year center,
  an annual-periodic additive spread of 1 bp on a flat 4% annual
  zero rate changes PV by about
  `−.01777736`. A continuous-zero shift changes it by
  `−.01848928`.

These are standalone exact or high-precision arithmetic checks. They do
not exercise production pricing, fixings, product lifecycle, replay,
solvers, calibration or hosts.

## Relationship to the accepted context lifecycle boundary

[CORE-MARKET-20's accepted package](../core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle)
requires explicit target-date context conditioning through family-owned source
semantics, atomic composites and active frozen-source/model qualification.
Horizon valuation and source knowledge are distinct; projected fixings or held-
spot product observations do not become actual publications. Cloning FX does
not imply held quotes. Reconcile actual theta producers and distinguish their
conditioning from unrolled-curve carry decomposition; context plumbing chooses
no universal carry/aging formula. Accepted record 20 does not adopt
this record's cash, observation, reinvestment, funding, breakeven or risk budgets.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
