# VALUATIONS-BONDS-13: Asset-swap annuities, projection and market/par spread conventions

**Decision status:** Accepted — complete checked bond asset-swap package, 2026-11-14.
**Implementation status:** Runtime and caller reconciliation pending. Observed behavior is recorded below.
**Validation status:** Source, primary references and independent arithmetic reviewed. Market, runtime and host qualification pending.
**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked bond asset swaps and price inversion

1. **Price the selected asset-swap structure from the bond buyer's side.**
   The buyer pays the swap's fixed leg, which exactly mirrors the
   eligible signed bond coupon cash, and receives the index plus a
   signed annual spread `s`. Both structures require a positive
   whole-position dirty settlement amount `P` and positive native
   face `N` for spread reporting. In a par asset swap the buyer pays
   the signed upfront `N−P`, receives floating coupons on `N`, and
   keeps the bond's par principal at redemption. The swap has no net
   terminal principal exchange. In a market (proceeds) asset swap
   the buyer pays `P` for the bond with zero swap upfront, receives
   floating coupons on `P`, and pays the signed net `N−P` in the
   swap at the actual bond redemption payment date. The resulting
   package principal is `P`. Negative upfront or terminal cash in
   these payment expressions is a receipt for the buyer. Adopt these
   meanings for `asw_par` and `asw_market`. Remove the legacy
   price-free coupon differential and the legacy market label on the
   face-notional upfront formula.
   [Lehman asset-swap cashflows, figures 1 and 3 and appendix](https://www.sfu.ca/~rjones/econ410/readings/OKane_2000_IntroAssetSwaps.pdf)
   and [ASISA valuation guidelines, pp. 170–171](https://www.asisa.org.za/media/uhcjg2ze/20200715_valuation-guideline-for-investment-portfolios.pdf).

2. **Match the bond's actual eligible cash and par redemption.**
   Initially admit a live single-currency, non-amortizing bond claim
   with deterministic fixed cash and one remaining full-face
   redemption after settlement. Admit level, stepped, stub and
   legitimately zero fixed coupon cash on that basis. Preserve the
   signs of all authorized fixed coupon cash. A generated or supplied
   strip must qualify the same contractual parent, payment,
   entitlement, principal and quote maps. Use actual cash dates and
   original coupon amounts, including an entire entitled coupon on
   a parent that started before settlement. Remove detached and other
   nonbuyer coupons. Include same-day coupon cash at factor one only
   under the qualified known payable cut and where the elected swap
   mirrors it. Exclude historic issue funding. Keep the bullet
   principal outside coupon PV and value any market-swap principal
   adjustment on the actual redemption payment date, which can
   differ from the contractual end and floating coupon payment dates.
   Reject a mismatched principal date or amount and any unsupported
   extra cash. Generic custom-equivalent coupons, FRN bond legs,
   amortization, PIK, fees, recovery, cross-currency and contingent
   settlement or termination structures are outside this profile.
   [Accepted bond cash and principal](01-schedule-and-holder-cashflows.md)
   and [accepted cashflow semantics](../core-conventions/015-cashflow-semantics.md).

3. **Fix exit, settlement and quote authority before preparing the legs.**
   Use ordinary maturity for a live no-option bond, or an explicitly
   selected, independently qualified fixed workout that satisfies
   the same cash, redemption and hedge conditions. Freeze the full
   right, candidate, exercise cutoff, redemption and actual payment
   identity. A workout depending on record 11 is unavailable while
   its basis is unqualified. Remove automatic lowest-yield workouts,
   clean-price-triggered branch selection and implicit maturity for
   a bond with optional cash. Bind actual buyer settlement `S`,
   valuation and knowledge cuts, quote units and native currency.
   Use one authorized price or spread driver. Use dirty settlement
   Money directly, convert clean with matching signed original-parent
   accrued once, or use the expressly selected noncircular holder
   to buyer quote bridge with its own financing and claim basis.
   Fetch accrued only when computing a clean/dirty conversion.
   Retain source quote, derived price,
   selected holder PV and any separately priced trade as distinct
   values. The ASW calculation uses no clean-price substitute or
   synthetic restamping to select a path.
   [Settlement and quote carry](02-settlement-and-clocks.md),
   [model and quote authority](03-pv-models.md),
   [workout ownership](11-workout-yield.md) and
   [fixed-path spread rules](12-spread-quotes.md).

4. **Prepare the floating grid, observations and market roles explicitly.**
   Start a new floating leg at the actual settlement under explicit
   effective/end dates, index and observation method, schedule
   frequency, roll/EOM, stubs, accrual and reset rules, calendars,
   payment dates and lags. Match the elected contractual exposure
   end and terminal principal date under their respective rules.
   Do not infer float schedules from index text, curve tenor or the
   bond's prior coupon date. Bind native swap/collateral discounting
   for both legs and terminal cash, separate from security discount,
   bond credit and settlement financing. Bind exact typed projection
   for each unresolved observation. An explicitly qualified shared
   overnight evaluator can serve both roles. A fully known float
   strip needs no projection lookup. Select exact required fixings
   or qualified projection according to index publication at the
   frozen knowledge cut, including a lagged reset or overnight
   lookback before settlement. Future settlement supplies no new
   information. Missing required published data fails. An already
   running swap with a different effective date needs a separately
   qualified trade valuation.
   [IRS schedules and source roles](../valuations-rates/03-interest-rate-swaps.md),
   [term fixing and projection](../cashflows/06-floating-rate-projection.md)
   and [overnight projection](../valuations-framework/overnight-projection.md).

5. **Calculate float coupon growth and annuity on their own bases.**
   Use unit gearing and no cap or floor. Simple term base coupon
   per unit is `u_i=L_i α_i`, using the resolved rate `L_i` and
   contractual payment accrual `α_i`. The exact term index period
   and its own accrual determine the fixing or projected index rate.
   For qualified overnight use direct growth
   `u_i=Π_j(1+r_ij w_ij)−1` under the elected arrears, observation
   shift or cutoff rules. Preserve positive index factors and the
   distinct observation blocks, including any retained partial
   blocks. Put the annual decimal spread outside compounding with
   the contractual coupon fraction `a_i` (`a_i=α_i` for this
   simple term profile). Its unit-notional coupon
   contribution is `s a_i`. Preserve signed coupon payments. Use
   `f=Σ_i u_i D(S,p_i)` and `a=Σ_i a_i D(S,p_i)` at the actual
   eligible payment dates. Use the source's positive checked dated
   relative factors, a finite strictly positive `a`, and checked
   contributions. A qualified zero spread accrual row contributes
   zero weight. The absence of positive spread exposure is
   unavailable. Keep the prepared unit-notional cash and annuity
   independent of the price and trial spread on that snapshot.
   Calculate from the dated rows in all cases and remove
   the discount-factor-difference fallback and fixed-annuity
   substitution. A telescoping identity holds only when the actual
   float cash and factors replicate the full endpoint difference.
   [Overnight growth and allocation](../cashflows/07-overnight-compounding.md)
   and [checked dated discounting](../core-conventions/016-discounting.md).

6. **Use the same two swap residuals in either quote direction.**
   Let `C` be settlement PV of the buyer-entitled signed bond coupon
   cash and `d=D(S,T)` for the actual aligned principal payment.
   Use these buyer-side equations and their closed-form inverses.

   | Structure | Buyer swap PV at fair spread | Spread from dirty `P` | Dirty amount from spread `s` |
   | --- | --- | --- | --- |
   | Par | `N(f+s a)−C−(N−P)=0` | `[C+N−P−N f]/(N a)` | `C+N−N(f+s a)` |
   | Market / proceeds | `P(f+s a)−C−(N−P)d=0` | `[C+(N−P)d−P f]/(P a)` | `(C+N d)/(d+f+s a)` |

   The spreads coincide at `P=N`. The market spread equals the
   par spread times `N/P` when `d+f=1`. Keep both equations in a
   general dated case. The positive face, proceeds and spread
   annuity requirements give a unique spread for valid finite
   inputs. In the market inverse, an exact zero denominator gives
   no price if `C+N d≠0` and no unique price if `C+N d=0`. With a
   nonzero denominator require a finite strictly positive dirty
   result. Preserve legitimate signed numerator and denominator.
   Reject out-of-domain, nonregular or unrepresentable results.
   Require every represented price output to remain strictly
   positive at the prescribed Money boundary.

7. **Check represented quote results and preserve boundaries.**
   Use stable, checked arithmetic and accumulation. Use exact
   contractual fixed cash from the buyer strip and the affine
   floating quote law before per-coupon settlement rounding.
   Apply the prescribed Money conversion and requested quote-unit
   conversion at their owning boundaries. Require an explicit
   positive native monetary residual budget in either direction.
   Recompose the corresponding buyer swap PV at the actual returned
   spread or dirty Money on the same terms and date cuts. Include
   any requested clean output conversion in the roundtrip check.
   Propagate invalid inputs, missing required market data, overflow,
   nonunique inverse and residual failure. No arbitrary annuity or
   denominator floors, fallback values or numerical root solve.
   Keep the sign and convert between decimal rate and bp once per
   appropriate boundary. Bond spread quote compounding does not
   supply the ASW coupon fraction. Keep issuer/default, hedge unwind,
   execution funding, separate fees and payable cash rounding under
   their own contracts, with no claim that the quote prices those
   features.
   [Money and precision](../core-conventions/012-money-decimal-precision.md)
   and [rate and basis-point units](../core-conventions/018-rate-units.md).

8. **Reconcile consumers with one checked Rust preparation and calculation.**
   Share basis, applicability, role dependencies, pricing and
   provenance across direct conversion, quote sets, metric and
   override registry, schemas and existing Python/WASM surfaces.
   Select either inverse through the same explicit quote driver.
   Update the two published metric definitions and quote payloads
   together and remove the obsolete scalar pin behavior. Distinguish
   a literal observation from a calculated output. Report an
   inapplicable optional field as unavailable, and surface a typed
   failure for an explicitly requested unsupported experiment or a
   failed applicable calculation. Use the same gate for standalone
   and bulk quotes and preserve independently available results.
   Where authorized as a risk price anchor, resolve once to the
   native dirty quote on the base snapshot and preserve its family,
   claim identity and basis. Bump/re-anchor rules remain with the
   risk owner. ASW does not delegate calibration targets or stand in
   for Z/DM/OAS, hazard spread or their risk. Host code maps types
   and errors only.
   [Metric contract](../valuations-framework/metric-orchestration.md),
   [cash-equivalent calibration targets](09-merton-calibration.md)
   and [spread and credit risk](16-spread-credit-risk.md).

## Observed behavior

### Choice and scope

Par ASW measures coupon versus swap return without a price adjustment. Market ASW includes bond purchase price and normalizes by the floating spread annuity. An optional ASW forward curve selects dual-curve projection; otherwise floating PV is the discount-factor difference. Ordinary quote inversion accepts only plain fixed bonds without custom cashflows or exercise; optional forward market-ASW reporting can use a clean-quoted workout. Custom calculator support is a distinct floating-spec equivalent-coupon route.

### Algorithms and conventions

Fixed annuity is `A_fix=Σα_fix D(payment)` on supplied dates, with ACT/365L per-period context and ICMA frequency context. Single-curve par rate is `(D_start−D_end)/A_fix`; helper returns zero rate/annuity for fewer than two dates or tiny annuity. Calculator boundaries reject unusable schedules/annuities.

Forward components compute `PV_float=Σ(F_i+spread_bp/10000)α_float D_i` and `A_float=Σα_float D_i`. For curve tenor at most one week, projection uses the forward curve's period rate and rescales its accrual to the floating-leg basis; otherwise use term `rate_between_dates`. Seasoned workout floating periods require exact historical reset fixings when reset precedes valuation; missing fixing errors. Other routes construct a fresh settlement-start schedule and project without that seasoned branch.

Ordinary par ASW is `c−par_rate` in the single-curve route or `(c A_fix−PV_float)/A_float` with a forward curve. Market ASW computes settlement-normalized coupon PV, adds principal-PV minus bullet maturity DF where principal rows exist, then `(coupon_PV+1−dirty/face−PV_float)/A_float`. Inversion rearranges that equation to dirty currency. Ex-coupon coupons are excluded from ordinary settlement coupon PV.

The clean-quoted early-workout market route builds a short-back schedule from the prior contractual fixed date, business-day adjusts fixed dates and uses clean price in `(c A_fix+1−clean/100−PV_float)/A_float`. It infers special float conventions from ID text: SOFR→ACT/360 annual, EURIBOR-6M→ACT/360 semiannual; otherwise forward metadata. The ordinary settlement route instead derives frequency from forward tenor. This difference is material for overnight IDs.

Custom forward helpers use future holder dates starting at `as_of`; custom coupon PV includes only Fixed/Stub rows. Equivalent coupon is coupon PV/(face·fixed annuity). Market helper requires explicit dirty currency and adds `(1−dirty/face)/A_float` to par ASW. Calculator dispatch requires a custom bond with Floating spec, while `from_cashflows` creates a Fixed placeholder by default; generic custom-bond ASW is therefore not automatically supported.

### Limits, alternatives and evidence

Several schedule conventions differ between plain, workout and custom paths; custom helpers do not apply the canonical settlement carry/ex-coupon adjustment consistently with ordinary settlement calculations. These observed differences are not accepted market exceptions. Explicit index conventions would avoid name-derived rules but are not implemented in these helpers.

Owning code: [ASW calculators](../../../../finstack-quant/valuations/fixed-income/src/bond/metrics/price_yield_spread/asw.rs), [annuity/projection/settlement components](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/annuity.rs), [inverse domain](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/spread_price.rs). [Quote tests](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/tests.rs) inspect forward-DF implied rates; inline annuity tests locate actual formula checks. General references: [Tuckman–Serrat](../../../REFERENCES.md#tuckman-serrat-fixed-income), [Fabozzi](../../../REFERENCES.md#fabozzi-fixed-income-handbook). Independent market-convention benchmarks for these distinct paths were not established.

Current routing and public shape evidence includes the [quote engine](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/compute.rs), [quote result](../../../../finstack-quant/valuations/fixed-income/src/bond/pricing/quote_conversions/types.rs), [metric keys](../../../../finstack-quant/valuations/core/src/metrics/ids/instrument.rs), [Python quote wrapper](../../../../finstack-quant-py/src/bindings/valuations/bond_quotes.rs) and [WASM fixed-income wrapper](../../../../finstack-quant-wasm/src/api/valuations/typed/fixed_income.rs).

## Independent arithmetic and qualification

- On `N=100`, `P=95`, `C=6`, `d=.90`, `f=.08`, `a=.90`, the
  buyer pays `5` upfront for par or `5` at redemption for proceeds.
  The spreads are `1/30` and `29/855` annual decimals, or
  `333.333333` and `339.181287` bp. Both inverses recover `95`.
  Multiplying par by `100/95` gives `350.877193` bp because
  `d+f=.98`. At `P=100` both spreads are `−222.222222` bp.
- With `P=101.70`, `N=100`, eligible `C=5.70`, `d=.94`, `f=.05`
  and `a=.95`, the par and proceeds quotes are `−105.263158` and
  `−101.744036` bp. A detached original coupon of `6` accrued
  through `14/15` gives signed accrued `−.40`. The corresponding
  clean price of `102.10` must convert to `101.70`. An explicitly
  qualified bridge with `H=105.366`, `Δ=5.70` and financing factor
  `.98` gives the same dirty amount. Treating clean as dirty moves
  the par spread down by `42.105263` bp.
- With discount and projection factors `1`, `.98`, `.94` at the
  settlement and two accrual ends, actual endpoint payments give
  `f=.06`. Moving the final floating payment to a date with
  discount factor `.93` gives `f=.0595744680851`. The common
  terminal principal payment can still use `d=.94`.
- With overnight `4%` for `3/360` and `5%` for `1/360`, direct
  growth is `10201/21600000`. A `10 bp` outside spread on coupon
  fraction `4/365` produces unrounded cash of about `483.227423`
  on `1,000,000` notional before discounting. The two index
  weights determine base growth and `4/365` determines the
  margin cash. [IRS overnight arithmetic](../valuations-rates/03-interest-rate-swaps.md#independent-review-evidence).
- At `N=1`, `C=−.5`, `d=.5`, `f=0`, `a=.5`, `s=−1`, market
  swap PV is zero for every positive dirty amount. With `C=0`
  and otherwise the same inputs the market price equation has no
  solution. Both cases require an inverse error.

Qualification requires independently dated par and proceeds ledgers
above and below par, term stubs and unequal accruals, direct-growth
overnight with an applicable fixing or lookback, lagged coupon and
actual principal payments, negative and zero coupon, same-day and
ex-coupon cuts, an equivalent qualified supplied schedule, missing and
invalid sources, singular market inverse and Money residual cases.
Compare direct pricing, quote-set availability, registry and host/wire
results on matched inputs. Cover both inverse and forward directions
and workout eligibility when its owner has qualified the basis.
The arithmetic examples establish the accepted cash identities.
Implementation, market data, runtime and host qualification remain
pending.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#bond-valuations).
