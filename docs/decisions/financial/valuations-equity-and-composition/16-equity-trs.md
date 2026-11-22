# VALUATIONS-EQUITY-AND-COMPOSITION-16: Equity total-return swap projection, dividends and margin exposure

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-16`.
- **Decision status:** Accepted 2026-11-14 — complete dated equity total-return swap projection, dividends, financing, risk and margin package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent equity carry, dividend-payment, financing, par-spread and spot-risk checks completed. Production pricing, risk, agreement, margin and host qualification remain pending.

## Accepted decision for checked dated equity total-return swaps

1. **Use one prepared dated ledger for both legs.** Require fixed
   nonnegative Money notional, explicit receive- or pay-return side,
   matching underlying, financing and settlement currencies and
   one qualified payment discount source. Compile contractual
   periods once. Preserve equity observations, financing fixings,
   accrual and actual payment dates and phases separately. Require
   unique positive exact historical reset levels on the qualified
   publication basis, including same-day observations once eligible.
   An expressly qualified authored initial level may establish
   the first observation. Resolve outstanding cash and known fixings
   before unused live inputs. Include eligible unpaid same-day cash
   at factor one. Require evidence of discharge and a qualified
   basis for overdue cash.
   [Accepted TRS dates and lifecycle](../valuations-framework/trs-legs.md),
   [accepted fixing authority](../core-market/scalar-series-and-fixings.md).

2. **Bind equity carry separately from projection and discount roles.**
   Resolve financing index projection, payment discounting and equity
   carry as explicit roles. The contract may bind the same curve to
   equity carry and payment discounting. Elect no distributions,
   signed continuous yield on ACT/365F, or an identified gross cash
   schedule with qualified coverage. With proportional carry, project
   a reset at `b` by `S exp(−q τ)/D_c(V,b)`, using `q=0` on
   the no-distribution basis. For discrete cash use
   `[S−Σ gross_i D_c(V,p_i)]/D_c(V,b)` for the qualified rights
   detached through the reset phase. Use the issuer's actual pay
   dates `p_i`, including payments after the reset. Check positive
   reset levels. For two future resets under proportional carry,
   form the direct dated carry ratio. A ratio of two projected
   cash-dividend levels is an explicit deterministic approximation
   to the future return.
   [Accepted dated equity carry](02-spot-and-carry.md),
   [accepted gross dividend events](../core-market/dividend-events.md).

3. **Preserve manufactured dividend cash as a separate component.**
   For opening and closing levels `L_a,L_b` use price-return
   cash `N(L_b/L_a−1)`. For each covered event allocate the
   gross cash by exact detachment phase and parent period, using
   `N/L_a` equivalent shares on the same price-per-share basis.
   Apply the contractual payout or withholding factor once.
   Elect issuer payment or parent return payment explicitly.
   Preserve the original event, ex-date, issuer pay date, actual
   swap due date and disposition. Require a determinable entitlement
   and amount on the admitted payment basis. Dividend cash may
   remain pending after the related price cash is settled. Use
   price levels that exclude the separately credited distributions.
   The continuous-income election is a simple nonreinvested yield
   accrual over the qualified contractual fraction. Keep earned
   income fixed on its admitted basis and forecast only the
   unresolved fraction.
   [Accepted dividend source](../core-market/dividend-events.md),
   [FpML dividend conditions](https://www.fpml.org/spec/fpml-5-9-6-rec-1/html/confirmation/schemaDocumentation/schemas/fpml-eq-shared-5-9_xsd/complexTypes/DividendConditions.html).

4. **Use the shared financing and replacement-spread basis.** Elect
   simple-term or compounded-overnight financing and use
   `N(H_i+sA_i)` with spread outside native floating growth
   and the contractual spread accrual fraction `A_i`. Receive-return
   PV is return PV minus financing PV, with the opposite sign for
   pay-return. Use annuity only over eligible replaceable spread cash.
   A known benchmark fixing can retain spread weight. Confirmed fixed
   financing amounts stay in the intercept. Quote the side-independent
   replacement spread `10000(R−F_0)/A` bp on the same eligible ledger
   and require a reprice within an explicit native-cash residual
   tolerance. Zero annuity leaves par unavailable. Export each dated
   signed component with its source and status and reconcile any same-date
   net cash view to the component ledger.
   [Accepted TRS financing and par](../valuations-framework/trs-legs.md),
   [accepted cashflow export](../valuations-framework/cashflow-export.md).

5. **Compute active risk on the same prepared ledger.** Use checked
   raw `index_delta` per actual represented spot unit and preserve
   contractual levels and eligible observations across reprices.
   Apply `dividend01` on the elected signed continuous yield; an
   identified cash-event shock is a separate direction. Apply each
   actual dated curve shock once across all active roles. Fixed unpaid
   amounts can retain discount sensitivity after equity exposure ends.
   For an elected horizon scenario, preserve actual fixings, label
   projected crossed equity and financing observations separately,
   and derive period cash and closing claims from one prepared ledger.
   Reconcile valuation, risk, diagnostics and cash through the
   existing Rust, Python, WASM and schema routes.
   [Accepted equity risk](08-selected-risk-and-implied-vol.md),
   [accepted dated DV01](../valuations-framework/finite-difference-rates.md),
   [accepted holder horizon](../valuations-framework/carry-theta-breakeven.md).

6. **Replace the notional SIMM proxy with qualified spot repricing.**
   For an explicitly elected qualified SIMM request use signed
   upward spot P&L `PV(1.01 S)−PV(S)` on the authorized prepared
   basis. Require the selected edition, authoritative equity
   classification and all applicable risk rows, including financing
   interest-rate factors. Preserve actual agreement and netting scope,
   amount currency and provenance. For VM use the agreement-authorized
   mark. The selected model PV is eligible when qualified on that
   cash basis.
   [Accepted margin bridge](../valuations-framework/margin-bridge.md),
   [ISDA SIMM equity sensitivity](https://www.isda.org/a/ZYxiE/ISDA-SIMM_v2.92606_PUBLIC.pdf).

## Observed behavior

### Choice and rationale

The return leg projects equity price return plus net dividends over each reset period; a shared financing engine supplies floating coupons, annuity and par spread. The equity-specific adapter preserves observed reset levels and fixed-but-unpaid amounts.

### Scope and input domain

Same-currency notional/underlying and financing inputs, explicit schedule, finite positive initial/fixing levels, dividend tax in [0,1], ordered nonnegative cash dividends and ordered fixings inside the schedule. Financing model/conventions are owned by the shared TRS engine.

### Algorithm

For each period, use exact observed start/end levels when already fixed; initial level can supply the first start. End fixing on as-of may use live spot; older missing end/start fixings error. Project future levels as `(S−PV future dividends)/D` when any discrete-dividend schedule exists, or `S exp(-qT)/D` otherwise.

Discrete total return is `(Fend−Fstart+net_dividends)/Fstart`. Dividends in `(period_start,period_end]` are after-tax; payment at dividend date uses `Ddividend/Dperiod_payment`, while period-end settlement uses ratio one. Already-paid dividend-date amounts are excluded. Continuous dividend return is the simple approximation `q×period_year_fraction×(1−tax)` added to price return.

PV is receive/pay sign times return-leg PV minus financing-leg PV. Exported net cashflows merge same-date signed legs and carry projected financing fixings. IndexDelta pins an omitted initial level, bumps current spot centrally and divides by actual spot width. Dividend01 uses the shared yield bump; annuity and par-spread bp solve delegate to the shared engine. SIMM exposure is a proxy: signed notional×0.01 under the equity ticker; VM uses full PV.

### Selection, controls and conventions

The financing discount curve also projects the equity carry. Nonempty cash-dividend schedules suppress continuous carry/yield income. Seasoned discrete-dividend trades require initial level explicitly. Default dividend settlement is set by the contract type; the supplied variant governs payment treatment.

### Limitations, failures and alternatives

Continuous dividend income is simple, not reinvested compounded total return. The SIMM proxy does not use the modeled delta, live spot, residual maturity or financing exposure. Alternate dividend-reinvestment and margin-risk calculations would be different methods. Missing reset history is not replaced with projected history.

### Evidence

[equity carry reference](../../../REFERENCES.md#hull-options-futures); [test_equity_trs.rs](../../../../finstack-quant/valuations/equity/tests/equity_trs/test_equity_trs.rs) and inline [pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/pricer.rs) locate seasoned/dividend/payment cases. Tests establish local leg identities; an independent financing/payment-lag and SIMM reconciliation is not established.

## Independent numerical checks

- Start fixing 100, spot 102, notional 1,000 and gross dividend 3
  give projected reset `(102−3×.96)/.98=101.1428571429`.
  Contractual withholding .20 gives dividend cash 24. With
  return payment factor .97 and issuer payment factor .95 on
  the payment discount basis, the return-leg PV is 33.8857142857
  when the dividend pays on the issuer date and 34.3657142857
  when it pays with the parent return.
- A separate receive-return case uses notional 1,000, fixed
  opening reset 100, live spot 115, no dividends, fixing factor
  .96 and return payment factor .94. The exact upward 1% spot
  reprice gives 11.2604166667. The observed notional proxy gives
  10.00. With the closing level already fixed at 115, the
  pending cash is 150 and its PV is 141, with zero spot-shock
  P&L on the defined axis.
- A fixed 100-to-102 return on notional 1,000,000 with a
  separate 22,500 net dividend gives return PV 41,875 at
  price and dividend payment factors .98 and .99. A term
  fixing of .04 and contractual fraction .25 give financing
  intercept 9,800 and annuity 245,000. The receive-return
  PV at 25 bp is 31,462.50 and the replacement par is
  1,309.18367347 bp on the eligible replaceable basis.

Production pricing, portfolio and margin reconciliation, market
risk and host qualification remain pending.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/metrics.rs)
- [marginable.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/marginable.rs)
