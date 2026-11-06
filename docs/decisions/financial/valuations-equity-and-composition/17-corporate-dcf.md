# Corporate DCF terminal value, equity bridge and dilution

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-17`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

The enterprise DCF discounts explicit cashflows and a caller-selected terminal formula at a single WACC. Equity value then applies a claims bridge and multiplicative valuation discounts. These are observed choices, not approval of a WACC, growth rate, multiple or liquidity discount for a company.

## Scope and input domain

Finite increasing flows strictly after the configured valuation date, WACC in (-50%,100%) as decimal annual rate, consistent instrument currency and optional positive basic shares/dilutive security counts. The market context does not supply WACC or growth. Growth terminal formulas require an annual terminal-flow override when average forecast spacing is below 0.75 years.

## Algorithm

1. Use actual days/365.25 for times. With mid-year convention, explicit cashflow times shift earlier by half average flow spacing (half a year for a single flow), floored at zero; otherwise use full dates.
2. Gordon terminal value is `CFterminal×(1+g)/(WACC−g)`, requiring WACC>g and warning when the spread is below 0.001. Exit-multiple terminal value is metric×multiple. H-model value is `CFterminal×[(1+gstable)+half_life×(ghigh−gstable)]/(WACC−gstable)` with positive half-life, high growth at least stable growth and WACC above stable growth.
3. Discount exit-multiple value at the full terminal horizon. Growth terminal formulas use the ordinary mid-year shift unless an annual terminal-flow override is supplied, in which case the terminal shift is half a year. Enterprise value is explicit-flow PV plus terminal PV.
4. Subtract the explicit equity-bridge net claims, then multiply by each `(1−discount)` factor. Bridge cash/nonoperating assets offset debt/preferred/minority/other liabilities.
5. For dilution, sort security exercise prices and solve the active set `price=(equity_value+exercise_proceeds)/(basic_shares+in_the_money_securities)`. Convert this to treasury-stock diluted shares and require the reconstructed equity residual to be within `1e-10×max(abs(equity_value),1)`; return no diluted result if the solvable domain is absent. Per-share metrics require shares.
6. Wacc01 uses the shared rate-component bump: change both discount factors and the growth terminal denominator at the terminal bump, preserving bridge and discounts.

## Selection, controls and conventions

`DiscountedCashFlow`/`DcfPricer` use the instrument's configured valuation date even if the caller requests another date, and stamp that date on the result. Market discount curves are not dependencies. Cashflow export includes explicit projected flows only; terminal value and the equity bridge are valuation constructs, not exported contractual cashflows.

## Limitations, failures and alternatives

Mid-year spacing is an average-grid approximation; it does not model actual intra-period receipt patterns. Growth terminal values become highly sensitive near WACC=g despite the warning. Exit multiples and annual terminal overrides are supported alternatives; varying discount rates, explicit fade paths and alternative dilution conventions would require another methodology. A returned price does not substantiate economic assumptions.

## Evidence

[corporate valuation reference](../../../REFERENCES.md#koller-valuation) and [DCF reference](../../../REFERENCES.md#rosenbaum-pearl-2020); inline [types.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/types.rs) includes manual Gordon/H-model, timing, bridge and large-warrant active-set identities; [pricer.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/pricer.rs) checks configured-date behavior and instrument/pricer agreement. These are formula and implementation-consistency evidence, not company forecast validation.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [types.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/types.rs)
- [pricer.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/pricer.rs)
- [metrics.rs](../../../../finstack-quant/valuations/equity/src/dcf_equity/metrics.rs)
