# VALUATIONS-STRUCTURED-CREDIT-18: Agency CMO waterfall and dated PAC contracts

- **Decision status:** Draft — observed implementation; adoption pending agreement.
- **Implementation status:** Observed in the owning paths below; scope: sequential, PAC/support, IO, PO and accrual CMO reference tranches.
- **Validation status:** Evidence inspected/located, not executed. Source and selected assertions establish implemented behavior, not general numerical accuracy or market approval.
- **Audit date:** 2026-11-05.
- **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; runtime source unchanged by this audit.

## Choice and rationale

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

Adoption questions, implementation reconciliation and validation work are tracked in
the [structured-credit follow-ups](../FOLLOW_UPS.md#structured-credit).
