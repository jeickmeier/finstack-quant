# VALUATIONS-EQUITY-AND-COMPOSITION-16: Equity total-return swap projection, dividends and margin exposure

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-16`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

The return leg projects equity price return plus net dividends over each reset period; a shared financing engine supplies floating coupons, annuity and par spread. The equity-specific adapter preserves observed reset levels and fixed-but-unpaid amounts.

## Scope and input domain

Same-currency notional/underlying and financing inputs, explicit schedule, finite positive initial/fixing levels, dividend tax in [0,1], ordered nonnegative cash dividends and ordered fixings inside the schedule. Financing model/conventions are owned by the shared TRS engine.

## Algorithm

For each period, use exact observed start/end levels when already fixed; initial level can supply the first start. End fixing on as-of may use live spot; older missing end/start fixings error. Project future levels as `(S−PV future dividends)/D` when any discrete-dividend schedule exists, or `S exp(-qT)/D` otherwise.

Discrete total return is `(Fend−Fstart+net_dividends)/Fstart`. Dividends in `(period_start,period_end]` are after-tax; payment at dividend date uses `Ddividend/Dperiod_payment`, while period-end settlement uses ratio one. Already-paid dividend-date amounts are excluded. Continuous dividend return is the simple approximation `q×period_year_fraction×(1−tax)` added to price return.

PV is receive/pay sign times return-leg PV minus financing-leg PV. Exported net cashflows merge same-date signed legs and carry projected financing fixings. IndexDelta pins an omitted initial level, bumps current spot centrally and divides by actual spot width. Dividend01 uses the shared yield bump; annuity and par-spread bp solve delegate to the shared engine. SIMM exposure is a proxy: signed notional×0.01 under the equity ticker; VM uses full PV.

## Selection, controls and conventions

The financing discount curve also projects the equity carry. Nonempty cash-dividend schedules suppress continuous carry/yield income. Seasoned discrete-dividend trades require initial level explicitly. Default dividend settlement is set by the contract type; the supplied variant governs payment treatment.

## Limitations, failures and alternatives

Continuous dividend income is simple, not reinvested compounded total return. The SIMM proxy does not use the modeled delta, live spot, residual maturity or financing exposure. Alternate dividend-reinvestment and margin-risk calculations would be different methods. Missing reset history is not replaced with projected history.

## Evidence

[equity carry reference](../../../REFERENCES.md#hull-options-futures); [test_equity_trs.rs](../../../../finstack-quant/valuations/tests/instruments/trs/test_equity_trs.rs) and inline [pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/pricer.rs) locate seasoned/dividend/payment cases. Tests establish local leg identities; an independent financing/payment-lag and SIMM reconciliation is not established.

## Relationship to the accepted cash-dividend source boundary

[CORE-MARKET-13's complete accepted package](../core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates) uses gross source cash
per one explicitly associated share/unit with independent ex/pay dates. Resolve
actual position entitlement and contractual payment/net-tax treatment once; do
not read a source magnitude as position cash or apply investor withholding
twice. Reconcile this product's ex-only cash vectors, payment-date forwarding
and continuous/discrete priority through qualified adapters. Source aging does
not settle receivables. Record13 was accepted on **2026-11-08**, with
reconciliation/qualification pending; these remaining TRS choices stay Draft.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/metrics.rs)
- [marginable.rs](../../../../finstack-quant/valuations/equity/src/equity_trs/marginable.rs)
