# VALUATIONS-BONDS-19: Bond carry clock and cash-payment weighted average life

**Decision status:** Accepted — complete checked carry and repayment-life package, including retired-par WAL weights, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Runtime, product, market and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond carry and repayment life

1. **Bind carry to two explicit holder cuts on one scenario.** Use
   original valuation `V` and an authorized closing date `H>V` or
   explicit positive tenor under the shared unadjusted tenor law.
   Supply both information and payment phases. A workflow needing
   a business-adjusted horizon supplies the resolved date. Admit
   the actual requested horizon without clipping to contractual
   maturity, exercise or last cash. Use the same signed position,
   native currency, selected model and comparable raw holder PV
   at both cuts. A closed position gets zero closing PV only when
   the chosen scenario establishes that no right, unpaid cash or
   delivery survives. In the initial deterministic carry profile
   require a qualified continuing claim and period cash. Use an
   explicitly admitted scheduled no-voluntary-exercise or fixed
   exit scenario where it qualifies, honor irrevocable prior
   commitments and retain independently payable claims. A horizon
   crossing a live contingent exercise, default, or delivery can
   be used only with an owner-qualified common scenario that
   resolves both cash and remaining claims. A model PV alone does
   not supply that scenario. No 1D default or horizon zero shortcut.
   [Accepted horizon and carry](../valuations-framework/carry-theta-breakeven.md)
   and [holder cash](01-schedule-and-holder-cashflows.md).

2. **Hold the opening dated market and the named valuation basis.**
   Resolve opening actual history from its information cut. Where
   authorized, materialize later applicable projected observations
   privately from the opening sources and label them hypothetical.
   Missing required eligible history fails. Evaluate remaining
   cash at `H` under the same prepared model, claim authority and
   held dated source laws, such as `D(T)/D(H)` using the retained
   evaluator. Preserve the actual source roles, dated geometry and
   applicable model controls. No constant-maturity reset, simulated
   observation treated as actual, or automatic market or parameter
   recalibration. When an observed price centers the model, qualify
   the selected-model spread at original `V` using the BONDS-02/12/15
   buyer-value residual and keep the same spread and law for the
   eligible closing reprice. Keep the observed quote as provenance
   and remove its fixed-price short-circuit from the scenario. A raw
   clean price or settlement dirty quote alone cannot stand in for
   the opening holder PV. Preserve retained and detached claims
   through the admitted settlement and holder mapping.
   [Settled buyer and holder bridge](02-settlement-and-clocks.md),
   [matched model quote center](15-curve-risk.md),
   [qualified hypothetical fixing materialization](../cashflows/16-fixing-materialization.md)
   and [OpenGamma on bond cash and settlement dates](https://quant.opengamma.io/Bond-Pricing-OpenGamma.pdf).

3. **Count actual scenario cash once and define gross and net carry.**
   From that scenario use signed economic holder settlement cash
   `C` between the resolved cuts, and identify the admitted coupon,
   interest and fee income `I` and other economic cash `O=C−I`.
   When both PVs include eligible same-day cash use `[V,H)`.
   When both exclude it use `(V,H]`. For mixed or more precise
   phases reconcile each endpoint against its own PV. Keep
   entitlement, actual payment dates and pending cash identities.
   Detachment alone does not count as payment. Include applicable
   principal, exercise or recovery cash only when the scenario
   qualifies it. Noncash capitalization and write-downs supply no
   receipt. Exclude unrelated acquisitions by claim and event identity.
   From opening `V0` and held-dated-market closing holder PV `VH`,
   compute gross `G=C+VH−V0`. Carry does not reinvest period cash.
   Elect unfunded carry or an explicit signed opening funding amount
   `B` and finite positive same-currency dated funding factor `Df(V,H)`.
   Report `F=B(1/Df−1)` and `carry_total=G−F`. Full opening-PV
   funding is available only when elected. Keep signed funding
   credits, negative rates and separately authorized financing cash.
   The funding overlay and the quote-settlement financing bridge
   serve distinct amounts and intervals. Count each charge once.
   [Shared carry and funding](../valuations-framework/carry-theta-breakeven.md),
   [bond repo](../valuations-rates/15-repo.md) and
   [ICMA repo economics](https://www.icmagroup.org/market-practice-and-regulatory-policy/repo-and-collateral-markets/icma-ercc-publications/frequently-asked-questions-on-repo/1-what-is-a-repo).

4. **Elect an unpartitioned result or qualify the fixed-yield split.**
   Unpartitioned carry needs no YTM lookup. It reports the valid
   gross, net, income and funding amounts, with pull-to-par and
   yield-benchmark roll-down unavailable. For a split require a
   complete admitted fixed settlement cash scenario and elected
   BONDS-10 dirty-price law, with a qualified common holder scenario
   on both sides. Preserve `V` and `H` as the reporting cuts.
   Resolve each cut's eligible buyer settlement, original coupon
   parents, nominal or quasi-coupon grid, day count, first broken
   period and per-claim quote coordinates. Freeze the represented
   opening yield `y`. Verify the opening settlement price on that
   law and reconcile the benchmark's opening holder value to `V0`
   within the applicable native-currency tolerances. At each cut
   compute the benchmark's buyer amount `Q_t(y)` and its qualified
   holder value `Y_t=F_t Q_t(y)+Δ_t` under the benchmark's matching
   settlement and claim roles. Preserve eligible detached and
   lagged unpaid holder cash in that calculation. The bridge is
   constrained by the actual claims and cannot be a plug. At a
   proved terminal cut with no remaining holder rights, use
   `Y_H=0` without requiring a quote on an extinguished claim.
   Use explicit grid and branch admission on live claims. Settlement
   yield cannot be started at the valuation date by substituting
   a scalar calendar clock. If required quote coordinates are
   unqualified, or the only available yield is the distinct Japanese
   simple clean-price conversion, the split is unavailable. An
   invalid or failed explicit split surfaces its cause. A distinct
   valid unpartitioned calculation can still be reported.
   [Bond quote dates and day count](02-settlement-and-clocks.md),
   [exact quoted-yield price laws](10-quoted-yield.md) and
   [fixed workout ownership](11-workout-yield.md).

5. **Use the horizon benchmark price to reconcile the split.**
   Set `VY=Y_H`, `pull_to_par=VY−V0+O`, and `roll_down=VH−VY`.
   Then `I+pull_to_par+roll_down=G` before display rounding.
   Reprice at the closing horizon using its actual quote branch,
   including Street's final-period simple rule or Moosmüller's
   contractual anchor where qualified. Use an accumulation shortcut
   only when the same cash, quotes, cuts and bridge prove that
   the required factor ratios are identical to the direct horizon
   benchmark. A Simple or branch-changing law cannot inherit a
   generic periodic shortcut. Roll-down is relative to the
   chosen benchmark. Interpret it as a curve-shape effect only
   under a controlled equivalent comparison. Remove the hardcoded
   Street adapter, inferred coupon frequency, zero pull with a
   degenerate flag and any silent funding assumption.
   [Held yield and curve roll-down](https://arxiv.org/pdf/2201.01330)
   and [shared carry and breakeven](../valuations-framework/carry-theta-breakeven.md).

6. **Define `wal` as time to receipt weighted by retired par.**
   Use actual authorized repayment cash dates measured from the
   holder valuation `V` and its payment phase. The date of a future
   scheduled receipt identifies the admitted path and does not assert
   receipt has already happened. On the authorized deterministic
   scheduled claim or independently qualified fixed exit, prepare
   exact par weights `P_i>0` from genuine paired repayment events.
   For an optional bond label the expressly selected scheduled
   no-voluntary-exercise or fixed-exit path. Respect any recognized
   prior commitment. Preserve contract, cash-date, claimant and
   principal-effect identities, including supported supplied schedules.
   Include qualified amortization, redemption, or an explicitly
   admitted early or other repayment by economic role. The weight
   is retired par from the paired principal effect, independent
   of redemption premium or discount. A lagged repayment takes
   its cash date even if the economic balance fell earlier.
   Exclude draws, fees, coupons, recovery and noncash write-down
   or PIK. Properly capitalized PIK affects the weight only when
   it becomes part of repaid par. Do not infer absent par from
   a positive cash row. The prepared path must resolve any event
   that can change repayment. A contingent expectation, default or
   prepayment model needs a separately defined metric basis.
   [Paired cash and principal effects](../core-conventions/015-cashflow-semantics.md),
   [repayment life authority](../cashflows/14-aggregation-and-life.md)
   and [OCC on weighted average life](https://occ.gov/publications-and-resources/publications/examiner-guide-investments-products/pub-examiner-guide-investments.pdf).

7. **Use the accepted exact life clock and preserve availability.**
   Validate the whole selected repayment stream, including currency
   and exact principal weights, before filtering. Resolve the same-day
   payment phase per claim. Known eligible unpaid same-day repayments
   included under that phase have day weight zero. Paid or ineligible
   same-day rows are excluded. Apply the Rust `repayment_life` kernel
   to the admitted dates and native par amounts with its explicit
   same-day control. With actual-day offsets `d_i`, report
   `Σ(P_i d_i)/(365 ΣP_i)` in ACT/365F years. Use exact numerator
   and denominator arithmetic and one checked conversion under the
   shared kernel. Denominator is selected repaid par. No discounting
   or unsigned reversal of malformed principal enters the statistic.
   No eligible positive repayment means unavailable. Eligible positive
   repayments only today give genuine zero years. Do not fetch
   unused prices, curves or coupon inputs where they cannot affect
   the path. Retain `wal` with its new bond weight basis in result
   metadata. Economic-date life is a separate owner-qualified
   application of the kernel, and CASHFLOWS-014 governs removal of
   the unqualified `CashFlowSchedule::wal` route.

8. **Keep horizon, risk and amount semantics through consumers.**
   Publish both dated cuts and phases, signed cash and funding
   basis, selected model and claim, quote and fixing origins, and
   actual WAL weight/date basis and availability. Keep `theta_period_days`
   for theta. Carry totals cannot be prorated by a day-count stamp
   or substituted for realized cash. A qualified bond breakeven
   reuses this exact prepared horizon, net carry and remaining
   claim with the named target's closing repricer under FRAMEWORK-13.
   Do not use a generic opening metric as a closing target slope.
   Attribution matches the actual endpoints and producer basis and
   adds back the same funding once where it reports gross carry.
   Funding and monetary carry scale signed quantity once. WAL is
   an intensive statistic. A portfolio repayment life needs a
   qualified pooling of compatible dated weights. Reconcile Rust,
   registry and product metadata, Python, WASM, schemas, attribution,
   UI and reports. Distinguish actual zeros, unavailable optional
   outputs and typed failures for explicit requests. Lifetime returns
   remain with BONDS-20.
   [Shared result and aggregation rules](../valuations-framework/metric-orchestration.md),
   [attribution carry](../attribution/03-total-return-carry.md) and
   [bond lifetime returns](20-lifetime-returns.md).

## Observed behavior

### Choice and scope

The bond adapter selects a Street-compounded yield clock for the shared carry decomposition, and a separate principal-cash weighted-average-life calculation. These are deterministic reporting calculations. They do not estimate physical default/prepayment probabilities or optimal-exercise life.

### Algorithms

Carry accretion is `A(t)=1/DF_Street(y,t)`, with coupon frequency/day count from the bond. ICMA resolves the quasi-coupon period from cached payment dates; ACT/365L splits at contractual coupon boundaries. The origin is valuation date, even though YTM is quoted at settlement. The shared carry calculator reprices on unchanged curves at the horizon, materializes crossed fixings at their valuation projections, separates income and principal cash, and computes `pull=PV0(A−1)−coupon_income−Σ payment·(A/A_payment−1)`. Roll-down is `PV_horizon−PV0+principal_cash−pull`; total carry adds coupon income and subtracts funding. Missing YTM sets pull to zero with a degenerate diagnostic, while no forward horizon gives zeros. Funding, where a repo curve exists, is `PV0(1/DF_repo−1)`. Horizon selection and period-cash classification remain shared-framework methods.

Bond WAL filters future schedule rows strictly after valuation to positive Amortization and positive Notional cashflows only. Use actual **cash payment date**, ACT/365F time and cash amount: `WAL=Σ principal_cash·t/Σ principal_cash`; no qualifying cash returns zero. PIK, coupons, prepayments and revolver repayments are excluded from this bond metric. A premium/OID redemption weights its actual cash rather than par. This differs intentionally in source from `CashFlowSchedule::wal`, which includes other principal repayments and uses balance-effective dates/par amounts.

### Limits, alternatives and evidence

Cash WAL and economic-balance WAL can materially differ under payment lags, premiums and custom principal kinds. Carry inherits quote/model repricing semantics and does not pin OAS here through the dedicated bond risk view. Settlement-origin carry would be another convention. Name alone does not identify the WAL/carry basis; the underlying schedule and clock must be disclosed.

Owning code: [bond yield clock](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/carry.rs), [metric registration](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/mod.rs), [shared decomposition](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/carry_decomposition.rs), [bond WAL](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/wal.rs). [WAL tests](../../../../finstack-quant/valuations/fixed-income/tests/bond/metrics/wal.rs) locate bullet-maturity and passage-of-time checks; inline tests in the bond carry module locate convention and payment-boundary checks. Reference: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income).

The live [carry metric IDs and horizon](../../../../finstack-quant/valuations/core/src/metrics/ids/risk.rs),
[breakeven route](../../../../finstack-quant/valuations/core/src/metrics/sensitivities/breakeven.rs),
[carry attribution](../../../../finstack-quant/attribution/src/metrics_based/carry.rs)
and [bond result fixture](../../../../finstack-quant-ui/src/fixtures/results/bond.json)
also show the obsolete shared theta stamp or fallback assumptions.

## Independent arithmetic and qualification

- Suppose the opening buyer dirty input is `100`, the settlement
  factor is `.98`, and the retained-claim value is `5`. The holder
  open is `103`. If the closing yield benchmark bridges to `100`,
  the held-market closing value is `99`, income is `6` and other
  cash `2`, the gross is `4`, pull is `−1` and roll is `−1`.
  Funding `80` on `.99` costs `80/99` and net is `316/99`.
- On an admitted simple quote at `y=.10`, with matching
  valuation and settlement at the opening and again at closing,
  coupon `10` at `.5` years and redemption `100` at `2` give
  `V0=650/7`. A one-year horizon with the coupon paid leaves
  direct `VY=1000/11`. The unqualified simple factor-ratio
  shortcut gives `275/3` and misses by `25/33`.
- A regular semiannual Street quote at `.08` values coupon `5`
  at half a year and terminal cash `105` at one year at about
  `101.886094674556`. With settlement and valuation matched,
  a closing cut at `.75` years follows the final-simple branch
  and gives `102.941176470588`. Applying the regular periodic
  factor instead yields `102.960970947547`.
- When par `20` amortizes economically on day `365` and pays on
  day `395`, and par `80` redeems on day `730` with cash `84`,
  the proposed `wal` is `663/365 ≈ 1.816438` years. Economic-date
  par life is `1.8`, and the observed cash-weighted life gives
  `3461/1898 ≈ 1.823498`. If the first `20` instead remains
  eligible on the valuation date, life is `1.6` years including
  it, or `2` years after its payment. A sole eligible positive
  same-day weight gives zero years. An empty eligible stream
  is unavailable.

Exact rational checks reproduce the holder bridge, funding,
simple-law repricing and WAL examples. A 65-digit calculation
checks the Street final-period transition.

Qualification covers mixed-date endpoint cash, ex-coupon and other
retained claims, recognized exercise and termination, fixings and
unresolved projected cash, quote and payment lag, broken-period
and named yield branches, funding roles and signs, paired principal
and payment dates, premiums, capitalized PIK and write-downs,
custom and fixed exit claims, same-day availability, signed position
scaling, matching horizon consumers and host results. The examples
check formulas and do not certify live market or runtime behavior.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
