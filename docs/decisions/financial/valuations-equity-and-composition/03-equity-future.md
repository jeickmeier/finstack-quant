# VALUATIONS-EQUITY-AND-COMPOSITION-03: Equity futures carry, quanto adjustment and risk

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-03`.
- **Decision status:** Accepted 2026-11-14 — complete equity futures carry, quanto, clearing and risk package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Source and primary references inspected. Independent fixing-horizon, quanto, clearing and smile-sensitive risk arithmetic checked. Market, venue, runtime and host qualification pending.

## Accepted decision for checked equity futures carry, quanto and clearing

1. **Bind the exact cash-settled contract.** Require the underlying and
   series identity, quote-point denomination, settlement currency, side,
   contract quantity and cash per point. Bind explicitly authored terms or
   an effective-dated venue specification. Resolve last trading cutoff,
   terminal underlying fixing, official publication and cash payment as
   distinct events. Preserve admitted rounding rules and apply tick and
   cash rounding at the governing stage. Physical delivery requires its
   own qualified economics and reporting path.
   [Accepted listed-product contract](../valuations-framework/listed-products.md#accepted-decision-for-checked-listed-futures-and-options),
   [CME final settlement procedures](https://www.cmegroup.com/trading/equity-index/settlement.html).

2. **Project the fair quote to the underlying fixing.** In the ordinary
   same-currency model, use the admitted cash-equivalent underlying and
   record 02's explicit none, continuous yield or qualified discrete cash
   election. Use dated relative asset funding, with the carry interval
   ending at the official underlying fixing cut. For no dividends use
   `S/D_A(V,T)`. With continuous carry use `S exp(−q τ)/D_A(V,T)` on the
   record 02 yield clock. For discrete carry, divide prepaid underlying
   by `D_A(V,T)`. Select retained rights by ex-date and phase and discount
   at actual payment dates, including those after `T`. Require qualified
   share or index point units. A price index needs an identified authorized
   index-point projection or an owner-qualified conversion. Total-return
   futures retain their own rules in record 04. Require positive finite
   fair model inputs and result. Keep the same-cut identity where admitted
   and reject invalid dates, coverage or prepaid arithmetic. The model
   assumes deterministic carry and omits stochastic-rate and variation-
   margin convexity. [Accepted cash equity and carry](02-spot-and-carry.md),
   [total-return futures](04-total-return-future.md),
   [CME equity index fair value](https://www.cmegroup.com/trading/equity-index/fairvalue.html).

3. **Use explicit fixed-conversion quanto with the matched FX orientation.**
   For distinct asset currency `A` and settlement currency `C`, use `X`
   quoted as `C` per unit `A`. On the compatible spot-value, observation and
   funding basis, form the same-fixing FX forward `X × D_A / D_C`. Bind the
   equity and oriented FX Black lognormal absolute-strike surfaces and
   sample the unadjusted equity forward and oriented FX forward respectively.
   Qualify each native surface clock on the same remaining fixing interval
   to obtain total standard deviations `w_equity` and `w_FX`. Strictly
   require covered inputs and coordinates. Remove implicit sampling clamps.
   Use `F_asset exp(−ρ w_equity w_FX)` in underlying points with `ρ` the
   signed correlation of the equity and oriented FX log-return shocks.
   A qualified reciprocal input reverses correlation once and needs its own
   exact smile-coordinate qualification. Preserve the zero covariance limit
   and checked stable arithmetic. Apply the fixed settlement-currency
   cash-per-point term once. FX spot affects the qualified model input and
   does not convert the contractual payment. This model admits explicit no
   distributions or continuous yield. Discrete dividends require separate
   qualification of the cash and surviving equity covariance horizons.
   [Accepted quanto law](../models-analytic/quanto-drift.md),
   [Eurex fixed-conversion specification](https://www.eurex.com/ex-en/markets/idx/stx/euro-stoxx-50-derivatives/products/EURO-STOXX-50-Index-Quanto-Futures-41960).

4. **Select the live mark and reference on their clearing basis.** Elect an
   observed live trading mark or the explicit live model mark. Require the
   applicable official final observation after the cutoff when needed and
   available to the valuation cut. Live mark P&L is
   `side × contracts × cash_per_point × (mark−reference)`, undiscounted.
   Use the trade fill on a newly traded interval or the last applied
   official clearing mark for a carried position. Keep cumulative trade
   P&L separate. Reconcile applied and pending variation without overlap.
   Use authoritative cash or the admitted venue rounding rule for actual
   variation. Value qualified unpaid final cash on its actual date and
   phase, with factor one for included same-day cash. Require proof of
   settlement for closure and explicit overdue-claim treatment.
   [CME clearing calculations](https://www.cmegroup.com/clearing/files/CME-Money-Calculations-Futures-and-Options.pdf).

5. **Report quotes and risk on the stated valuation basis.** `futures_price`
   uses the selected mark. The explicit theoretical quote reports its own
   model basis. Basis is selected live mark less current spot only on a
   matching underlying point and observation basis. Live point delta is
   signed contracts times cash per point. Spot delta and curve DV01 follow
   the selected valuation. A held observed mark has structural zero model
   spot and rate dependence. A domestic model spot delta differentiates
   the admitted carry on its frozen distribution and funding inputs. A
   quanto spot reprice holds the surfaces fixed and resamples their ATM
   coordinates with the moved underlying. Use checked comparable reprices
   and remove the frozen-sample shortcut from model delta. Shock only
   active typed curve roles, without fabricating buckets. Fixed final cash
   has no live quote risk and can retain discount risk while unpaid.
   [Accepted finite differences](../valuations-framework/finite-difference-rates.md).

6. **Share resolved state across reporting and hosts.** Export qualified
   dated unpaid clearing cash and count applied variation once. Reserve
   `NoResidual` for a proved empty cash scope. A live model quote supplies
   no clearing cash history. Crossing a cutoff, fixing or payment on a
   horizon roll requires admitted actual or scenario state. Bind only
   the sources active for each operation. Reconcile native and existing
   JSON-first Python and WASM routes, schemas, portfolio risk, attribution
   and futures-option consumers through the same Rust contract.
   [Cashflow export](../valuations-framework/cashflow-export.md),
   [horizon valuation and cash](../valuations-framework/carry-theta-breakeven.md).

## Observed behavior

### Choice and rationale

The fair futures quote is a carry projection to settlement, optionally multiplied by a fixed-currency quanto drift adjustment. Position PV uses shared listed-future mark-to-market semantics rather than discounted terminal cashflow valuation.

### Scope and input domain

Positive finite spot/forward, positive finite cash dividends no later than settlement, same-currency futures or a fully specified cross-currency quanto. The equity and FX surfaces used for quanto must be Black-lognormal strike surfaces.

### Algorithm

1. Validate listed terms and currency relationships. Select the asset discount curve for quanto, otherwise the configured discount curve; use ACT/365F to settlement.
2. Without a cash-dividend vector, `Fdom = S exp(-qT)/Dasset`; otherwise `Fdom=(S−Σ Di Dasset(ti))/Dasset(T)` using future dividends only and ignoring continuous yield.
3. For quanto, sample equity volatility at `(T,Fdom)`. Resolve unitless FX spot as settlement currency per underlying currency, derive `FXforward=FXspot Dasset/Dsettlement`, and sample its Black surface there. Compute `Fquanto=Fdom exp(−ρ σequity σFX T)`.
4. Mark selection and `contracts × multiplier × position_sign × (mark−entry)` delegate to `ListedFutureTerms`, including quote/settlement states. No residual cashflows are emitted.
5. Spot delta is point delta times domestic carry derivative times `Fquanto/Fdom`; it holds sampled quanto volatilities fixed. Basis uses embedded quoted price when present, otherwise the model fair quote, minus current spot; the FuturesPrice metric uses resolved mark.

### Selection, controls and conventions

Quanto must be absent when underlying and settlement currencies match and present otherwise, with matching asset currency and equity surface ID. Surface lookups clamp via the shared surface helper. Rate risk is shared combined/key-rate repricing. The direct fair-price/delta helpers continue to calculate a model projection even if PV uses a fixed embedded quote.

### Limitations, failures and alternatives

There is no stochastic-rate futures/forward convexity adjustment. Analytic delta omits changes in smile samples as spot shifts the ATM/FX-forward sampling point. A quote-marked or already-settled PV can therefore have different bumped risk from the reported model delta/basis. Model-consistent finite differences or explicit quote-risk reporting are meaningful alternatives, not established approvals.

### Evidence

[Hull futures carry](../../../REFERENCES.md#hull-options-futures) and [quanto reference](../../../REFERENCES.md#wystup-fx-options); inline tests in [types.rs](../../../../finstack-quant/valuations/equity/src/equity_future/types.rs) and the [equity-future regression fixture](../../../../finstack-quant/valuations/tests/golden/data/pricing/regression_goldens/equity_future/spx_es_3m.json) locate carry/quote/quanto evidence. The fixture is a regression reference; independent exchange reconciliation and convergence evidence are not established.

### Relationship to the accepted cash-dividend source boundary

[CORE-MARKET-13's complete accepted package](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates) preserves identified
cash-per-share events with separate ex/pay dates and exact underlying/share/
currency basis. Reconcile the product's ex-only cash inputs and same-date rules
through a qualified adapter; payment timing and entitlement cannot be recovered
by silently assigning pay=ex. Keep continuous/residual carry selection explicit.
CORE-MARKET-13 was accepted on **2026-11-08**. The accepted futures model
above qualifies the consumer's cash-flow and quote basis separately.

## Independent arithmetic and qualification

- At spot 5,200, annual asset funding 3%, settlement funding 5%, continuous
  yield 1%, volatilities 20% and 10% and oriented correlation `.5`, a two-day
  ACT/365F fixing gives quanto `5200.2849393133`. A five-day cash horizon
  would give `5200.7123775590`. The model ends at the fixing date.
- Three short contracts at 50 currency units per point, with fill 4,000,
  last applied official mark 4,020 and final mark 4,005, have `−3000` applied
  variation, `+2250` pending final cash and `−750` cumulative trade P&L.
  With a qualified `.999` payment factor, the pending cash is worth
  `2247.75` at valuation.
- A separate sloped-surface scenario gives spot delta `101.2677088679`
  per underlying spot point after resampling, versus `101.5267055658`
  holding only the original sampled volatilities fixed.

Qualify actual venue specifications, clock and FX-basis bindings, market
dividend and smile data, clearing cash, risk and existing host consumers.
Repository runtime suites and independent venue-price benchmarks remain open.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_future/types.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/equity_future/metrics.rs)
