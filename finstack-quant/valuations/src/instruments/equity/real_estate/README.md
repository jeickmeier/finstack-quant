# Real Estate Asset (Equity) — `RealEstateAsset`

Single-asset real estate valuation instrument supporting the two most common appraisal-style methods:

- **DCF**: discount explicit NOI (and optional CapEx) plus an **exit-cap** terminal value.
- **Direct Cap**: value a stabilized NOI at a cap rate.

This instrument is intentionally **single-asset** and **unlevered** by default. You can represent leverage by valuing the debt (e.g. `TermLoan`) separately and netting at the portfolio layer, or by using the return metrics that reference `purchase_price`.

## Levered equity composition

If you want a first-class “deal wrapper” around the asset + financing, use:

- **`LeveredRealEstateEquity`**: composes `RealEstateAsset` + a financing stack (e.g. `TermLoan`, `Bond`)
  - **Value convention**: \(PV_{equity} = PV_{asset} - PV_{financing}\) (financing valued from lender perspective)
  - **Return metrics**: computed off a simplified equity cashflow schedule with explicit **sale proceeds** and **financing payoff at exit**
  - **Financing stack**: supports multiple financing instruments via `InstrumentJson` (so you can mix `TermLoan`, `Bond`, etc.)
  - **Note**: cashflow-based leverage metrics (DSCR / payoff / levered IRR) currently require financing instruments that can produce cashflow schedules (e.g. `TermLoan`, `Bond`, `RevolvingCredit`, `Repo`). PV netting works for any `InstrumentJson`.

## Key conventions

- **Discounting**:
  - DCF always discounts at the property `discount_rate`; `discount_curve_id` is for risk attribution only. DV01 bumps the risk-free component inside the rate.
  - Otherwise, DCF discounts using **annual discrete compounding**: \( PV = CF / (1 + r)^t \) using `day_count`.
- **Terminal value (DCF)**:
  - **Explicit sale price** (if `sale_price` is set): terminal proceeds use `sale_price` (gross), realized on `sale_date` if provided (otherwise last NOI date).
  - Otherwise, **exit-cap** convention: \( TV = NOI_{N+1} / cap\_rate\_exit \)
  - `terminal_growth_rate` (optional) is used to project \(NOI_{N+1} = NOI_N \times (1 + g)\).
  - Disposition costs:
    - `disposition_cost_pct` (optional) reduces gross proceeds: \(TV_{net} = TV \times (1 - c)\).
    - `disposition_costs` (optional) are **dollar** line items subtracted from proceeds.
- **CapEx**:
  - `capex_schedule` values are treated as **positive outflows** and valued as `NOI - CapEx`.

## Fields you’ll typically set

- **Core**: `noi_schedule`, `valuation_method`, `discount_curve_id`, `day_count`
- **DCF**: `discount_rate` (required), `terminal_cap_rate`, `terminal_growth_rate`
- **Sale modeling**: `sale_date`, `sale_price`
- **Transaction**: `purchase_price`, `acquisition_cost` (scalar) and/or `acquisition_costs` (line items), `disposition_cost_pct` and/or `disposition_costs`
- **Cashflow realism**: `capex_schedule`

## Custom deal metrics registered

The real estate module registers these custom metric IDs (via `MetricId::custom(...)`):

- `real_estate::going_in_cap_rate`
- `real_estate::exit_cap_rate`
- `real_estate::unlevered_irr` (requires `purchase_price` + `terminal_cap_rate`)
- `real_estate::unlevered_multiple` (requires `purchase_price` + `terminal_cap_rate`)
- `real_estate::unlevered_cash_on_cash_first` (requires `purchase_price`)
- `real_estate::cap_rate_sensitivity` (finite-difference; DirectCap uses `cap_rate`, DCF uses `terminal_cap_rate` when applicable)
- `real_estate::discount_rate_sensitivity` (finite-difference bump of `discount_rate`; DCF always discounts at the property rate)

For `LeveredRealEstateEquity`:

- `real_estate::levered_irr`
- `real_estate::equity_multiple`
- `real_estate::ltv`
- `real_estate::ltv_at_origination`
- `real_estate::dscr_min`
- `real_estate::dscr_min_interest_only`
- `real_estate::debt_payoff_at_exit`
- `real_estate::cap_rate_sensitivity`
- `real_estate::discount_rate_sensitivity`
