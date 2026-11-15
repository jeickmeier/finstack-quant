# VALUATIONS-STRUCTURED-CREDIT-18: Agency CMO waterfall and dated PAC contracts

- **Decision status:** Accepted 2026-11-15 — complete agency CMO collateral, tranche waterfall, dated PAC, settlement and risk package.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent PAC catch-up, endpoint PSA, funded accretion and quote calculations checked. Production deal terms, path-speed protection, pricing and host qualification remain pending.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Accepted decision for agency CMO waterfalls and dated PAC contracts

1. **Require authoritative collateral and opening state.** Use one
   explicitly supplied agency pass-through projection and a common dated
   waterfall for all reference classes. Remove inferred collateral from
   tranche face and embedded WAC, WAM, fee, PSA and maturity assumptions.
   Validate currency, agency program, actual dates, collateral factor,
   class balances and pending remittances on matching phases. Require
   authority for guarantee proceeds, other funding and terminal
   disposition. Select any reference class from that common waterfall.
   [Accepted agency MBS projection and rights](../valuations-other-fixed-income/13-mbs-projection.md#accepted-decision-for-checked-agency-mbs-projection-and-cash-ownership).

2. **Allocate by authored cash priorities and class terms.** Compute
   current-pay interest on each class's authorized opening basis and
   contractual accrual clock. Pay from eligible dated cash. Replace the
   generic coupon-versus-pool-yield tests with actual claim, funding and
   shortfall rules. Carry unpaid amounts where authorized. Elect principal
   tiers and equal-rank sequential or pro-rata allocation. Cap payments
   by live face, route residual cash only to named recipients and use
   checked Money and executable rounding. Qualify terminal closure against
   all remaining balances and claims.
   [Accepted waterfall claims and rounding](11-waterfall-claims-and-rounding.md#accepted-decision-for-waterfall-claims-and-rounding).

3. **Apply IO, PO and Z terms on their own bases.** An IO earns interest
   on its qualified opening reference notional and receives zero principal.
   Update notional after accrual under the selected pool-factor rule
   or expressly supported class linkage. A PO earns zero coupon and
   receives principal at its authored priority. A Z class follows its
   own dated accrual and release trigger. Funded accretion increases
   Z par through a noncash event and redirects the same eligible cash
   once to an authorized principal recipient. Preserve earned claims and
   authorized unpaid amounts. Distinguish scheduled collateral principal,
   prepayments and redirected interest as cash sources.
   [Filed Z and IO terms](https://www.fanniemae.com/syndicated/documents/mbs/remicsupp/1999-009.pdf).

4. **Use immutable dated PAC ending balances.** On each protected
   distribution date the elected tier claims
   `max(0, live face − planned ending face)`, capped by eligible cash.
   This catches up missed principal and reduces the claim after early
   paydown. Planned ending face zero targets full retirement. An explicit
   no-target action is distinct and a missing required date errors.
   Validate nonincreasing targets bounded by original face against
   actual distribution dates. Pay to the planned tier, then named
   support to its capacity, then any authorized excess tier. Report
   target shortfall, catch-up and excess separately from principal source.
   [Filed planned-balance priorities](https://www.fanniemae.com/syndicated/documents/mbs/remicsupp/2000-033.pdf#page=15).

5. **Limit endpoint PSA generation to structuring.** Project both
   explicitly ordered constant PSA endpoints with authoritative
   collateral, actual age, actual distribution dates and the shared
   level-pay step. Take the periodwise minimum and cap cumulative plan
   paydown at PAC face. Require authorized terminal routing for any
   remaining face. An authorized generated plan is a fixed dated input
   across valuation, seasoning and shocks. Endpoint construction
   establishes a schedule under its assumptions. Path-varying speed
   protection requires separate evidence.
   [Standard mortgage formulas](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf) ·
   [Filed structuring ranges](https://www.fanniemae.com/syndicated/documents/mbs/remicsupp/2000-033.pdf#page=17).

6. **Use one ledger for PV, exports and buyer settlement.** Discount
   entitled holder cash with dated relative factors. Export paid cash,
   principal effects and dates, including noncash Z accretion. At buyer
   settlement select rights and delivered face from the prepared state at
   the actual phase. Keep rights to pending seller cash distinct from
   principal already booked in opening balances. Use the class's
   authorized accrued clock and basis, an explicit IO denomination and
   qualified Z accrual-state terms. Add matching accrued once to clean
   price consideration.
   [Mortgage settlement conventions](https://www.sifma.org/wp-content/uploads/2017/08/chsf.pdf).

7. **Fit static Z-spread on the fixed buyer strip.** Apply a continuous
   annual-decimal spread on the selected nominal curve's native
   settlement-relative time. Entitled same-day cash has a unit spread
   multiplier. Require positive spread exposure and a qualified monotone
   objective. Use an explicit finite bracket, coordinate tolerance and
   work budget and reprice the returned spread under a separate native
   settlement-currency residual gate. Remove generic 30/360 spread time
   and hard-coded controls. A modeled future-settlement target requires
   a qualified original-origin financing bridge. Stochastic agency CMO
   OAS has its own qualification boundary.
   [Accepted fixed-strip spread](../valuations-other-fixed-income/14-mbs-risk-oas.md#accepted-decision-for-checked-agency-mbs-rate-risk-and-oas) ·
   [Accepted Brent](../core-numerics/brent-root.md).

8. **Reprice rate risk on a matched holder basis.** Elect frozen
   prepayment or a qualified dated monthly refinance response. Shock
   actual active curve roles and rerun the common waterfall with the
   same PAC plan, determined cash and any elected held spread.
   Report signed native DV01 per positive bp. Reconcile admission,
   event exports, quote availability and units through the existing
   Rust registry, Python, WASM and schemas.

## Independent calculation checks

- PAC starts at 60 with planned ending faces 50 and 40. Principal 6
  leaves 54 at the first date. Principal 17 on the second date pays 14
  to PAC and 3 to support. A fixed 10 installment would leave PAC at 44.
- A mature, zero-WAC pool of 100 and three level-pay months gives
  second-period endpoint minimum 33.3244 at constant 0% and 300% PSA.
  A path switching from 300% to 0% supplies 32.7866, short by 0.5378.
- Interest 0.55 pays 0.30 current-pay and 0.10 IO. The remaining 0.15
  funds Z accretion and redirects 0.15 of principal cash. With 14 of
  collateral principal also paid, aggregate debt par falls from 120
  to 106. A pool-linked IO notional of 60 becomes 53 after accrual.
- Buyer dirty target 80 and one-year discounted cash 81.48 give
  static Z-spread 1.8331%. Including a seller-owned payment of 5 at
  15 days with factor 0.9975 on an ACT/365F curve gives 8.2478%.

## Observed implementation and scope

CMOs project a single agency-MBS collateral pool, distribute it through one common waterfall, and return the chosen reference tranche. PAC targets are original dated contract data and are not regenerated when valuation date or prepayment assumptions change. The shared collateral kernel prevents separate PO/IO projections from inventing independent pool cash.

## Algorithm and controls

Explicit collateral is preferred by dispatch. If absent, the engine constructs a generic pool from aggregate principal-bearing current/original face, optional WAC/WAM overrides and embedded defaults: WAC 4.5%, WAM 360 months, servicing/guarantee fees 25 bp each and PSA 100%. Its maturity is valuation date plus assumed WAM. This is an observed material-data fallback, not permission to value an unknown pool as if those inputs were measured.

For each collateral period, current-pay interest is allocated by priority on beginning note balance, including IOs. When any nonaccrual principal-bearing class remains, funded accrual-class interest capitalizes into its balance and is redirected to principal for earlier notes. Accrual-class principal is initially locked out. Principal groups follow priority; PAC dated targets are paid first, proportionally if cash is insufficient, then other classes receive sequential or current-balance pro-rata principal. Remaining principal sweeps available balances, with accrual classes last, so broken/exhausted PAC support can result in payments beyond target. IO notionals amortize only after current-period interest by collateral ending/beginning balance, clamped [0,1]. Interest shortfalls are recorded, not silently capitalized for ordinary current-pay notes.

PAC construction projects the collateral principal stream at lower and upper PSA speeds, using actual age and the shared level-pay pool step. It takes the periodwise minimum of the two streams and caps cumulative targets at PAC face. Targets retain actual delayed/adjusted payment dates; date misses return zero. The algorithm is an endpoint-collar construction, not a proof that all intermediate/path-varying speeds are protected.

Reference cashflow tags retain scheduled/prepayment breakdown; funded accretion becomes scheduled principal attribution. Ordinary PV uses relative discount factors. CMO z-spread uses settlement-month quote-basis collateral, accrued on current face from month start/issue date, continuous spread with 30/360 time, Brent tolerance 1e-8, 100 iterations and bounds [-10%,20%].

## Domain, failures and alternatives

PAC requires a validated dated schedule; input validation rejects incompatible tranche/face/collateral definitions. Supported sequential/PAC/support/IO/PO/accrual branches differ mechanically. General nonagency credit loss, floating/inverse-floating tranches, stochastic prepayment OAS and arbitrary mortgage legal waterfalls are not established by these types. Explicit collateral and explicit PAC schedules are supported alternatives to assumptions/generated targets.

## Implementation and validation evidence

- [cmo/types.rs](../../../../finstack-quant/valuations/structured-credit/src/cmo/types.rs)
- [cmo/pricer.rs](../../../../finstack-quant/valuations/structured-credit/src/cmo/pricer.rs)
- [cmo/waterfall.rs](../../../../finstack-quant/valuations/structured-credit/src/cmo/waterfall.rs)
- [cmo/pac_support.rs](../../../../finstack-quant/valuations/structured-credit/src/cmo/pac_support.rs)
- [cmo/metrics/zspread.rs](../../../../finstack-quant/valuations/structured-credit/src/cmo/metrics/zspread.rs)
- [cmo/metrics/mod.rs](../../../../finstack-quant/valuations/structured-credit/src/cmo/metrics/mod.rs)

Reference context: [MBS formulas](../../../REFERENCES.md#sifma-mbs-standard-formulas), [fixed-income context](../../../REFERENCES.md#fabozzi-fixed-income-handbook). These references identify the method family; they do not certify the preset parameters or every implementation branch.

Inline production_mortgage_audit, persistent_pac_regressions, dated_pac_regressions and rate_risk_rebuild_tests were located/inspected as invariant and shared-kernel evidence; external deal/PAC benchmark provenance and path-speed protection remain open.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
