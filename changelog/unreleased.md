# Unreleased

## Itemized changes

### Portfolio: dated Monte Carlo exposure

The portfolio and standalone instrument exposure APIs reprice admitted
positions in their contractual direction on each dated Philox path.
The result records the reporting currency and hashes the netting scope into its source
identity. An elected cash variation-margin agreement reduces exposure
as calls settle and collateral interest accrues.

### Margin: dated exposure grids for CVA, DVA and FVA

Exposure profiles carry an opening date, calendar observation dates, the
reporting currency, and completeness provenance. Credit and funding legs
sample each curve on its own date axis and normalize factors at the
valuation cut. Effective EPE uses the right-endpoint running maximum.

### Portfolio: ordinary FRTB delta grids

`portfolio_frtb_sensitivities` / `portfolioFrtbSensitivities` combine the
qualified GIRR and single-name CDS CSR quote grids. Each result preserves
its exact repricing evidence and identifies positions outside those grids.

### Portfolio: exact ordinary CDS spread delta

`csr_quote_delta` / `csrQuoteDelta` rebuild the original single-name CDS
calibration for each ordinary non-securitisation CSR vertex. The source
carries dated issuer, seniority, currency and bucket evidence with exact
executed quote bindings. The signed reprices report native P&L per basis
point and reporting-currency P&L per percentage point.

### Python: data-class constructors are keyword-only and read by Rust

A data-class constructor that builds one Rust serde struct now takes its
fields as keyword-only arguments, and the Rust binding reads them as one
mapping into the struct: the Rust struct supplies every default and its
`deny_unknown_fields` rejects an unknown keyword. A missing or unknown field
raises `ValueError` naming the missing or unknown field, a
positional argument `TypeError`. Each field takes its serde wire form or a
typed object the params hook writes as exactly that form; host conveniences
are gone: an exact-decimal field (`spread_bp`, `rate`, `coupon_bp`, `bp`,
`gearing`, the bp floors and caps) takes a `decimal.Decimal` or its string
(`"0.045"`), not a `float`, `Rate` or `Bps`; a tenor field takes a `Tenor`,
not `"3M"`; a date a `datetime.date` or an ISO string, not a
`datetime.datetime` or `pd.Timestamp`; a `PoolAsset` sub-spec its typed class
or dict, not JSON text. The params hook now also encodes `decimal.Decimal`
and the schedule enums (`BusinessDayConvention`, `StubKind`, `RollRule`,
`FloatingLegCompounding`, `OvernightIndexConstraintApplication`,
`FloatingRateFallback`, `FeeAccrualBasis`) as their wire forms. Every
`to_json()` built from the same inputs is unchanged. Rust defaults added:
`LiquidityProfile.spread_volatility_kind` (`relative`) and `observation_days`
(20), `CdsTrancheParams.realized_loss` (0.0); `RecoveryClaim` gains
`deny_unknown_fields`; new `CdsTrancheParams::validate`. The statements
classes follow the same rule: Rust `StagingConfig` and `QualitativeFlags`
default every omitted field (`StagingConfig::default()`, all flags off) and
`Exposure` defaults `segments` and `qualitative_flags`; `QualitativeFlags`
gains `deny_unknown_fields`. The params hook also encodes `NodeType`, `Stage`
and `LeaseGrowthConvention` as their wire labels, and a non-finite float in any
params mapping raises `ValueError` "non-finite float" (JSON carries only finite
floats) instead of a serde parse error.

### Cashflows: `FloatingRateSpec`, `ScheduleParams` and `FeeSpec.periodic_bp` are keyword-only

- Python `FloatingRateSpec("USD-SOFR-3M", Decimal("200"), "3M", ...)` → `FloatingRateSpec(forward_curve_id="USD-SOFR-3M", spread_bp=Decimal("200"), reset_frequency=Tenor.parse("3M"), ...)`
- Python `ScheduleParams("3M", DayCount.ACT_360, "weekends_only", ...)` → `ScheduleParams(frequency=Tenor.parse("3M"), day_count=DayCount.ACT_360, calendar_id="weekends_only", ...)`
- Python `FeeSpec.periodic_bp(FeeBase.DRAWN, 25, "3M", DayCount.ACT_360, "weekends_only", ...)` → `FeeSpec.periodic_bp(base=FeeBase.DRAWN, bp="25", frequency=Tenor.parse("3M"), day_count=DayCount.ACT_360, calendar_id="weekends_only", ...)` (the fields of the `periodic_bp` variant of the `FeeSpec` wire form)

### Margin: `SaCcrTrade` is keyword-only

- Python `SaCcrTrade("t1", "interest_rate", 1e6, "2025-01-01", "2030-01-01", "USD-SOFR", "rates", 1.0, 1.0, 0.0, False)` → `SaCcrTrade(trade_id="t1", asset_class="interest_rate", notional=1e6, start_date="2025-01-01", end_date="2030-01-01", underlier="USD-SOFR", hedging_set="rates", direction=1.0, supervisory_delta=1.0, mtm=0.0, is_option=False)`

### Models: `LiquidityProfile`, `RecoveryClaim` and the scoring inputs read their Rust struct

- Python `LiquidityProfile("ACME", 100.0, 99.95, 100.05, 1e6, 500.0, 0.0)` → `LiquidityProfile(instrument_id="ACME", mid=100.0, bid=99.95, ask=100.05, avg_daily_volume=1e6, avg_trade_size=500.0, spread_volatility=0.0)`
- Python `RecoveryClaim("SEN", "secured", 1, 100.0, 0.0, 0.0, 0.0)` → `RecoveryClaim(id="SEN", seniority="secured", priority=1, principal=100.0, accrued=0.0, penalties=0.0, collateral_haircut=0.0)`
- Python `AltmanZScoreInput`, `AltmanZPrimeInput`, `AltmanZDoublePrimeInput`, `OhlsonOScoreInput`, `ZmijewskiInput`: calls unchanged (already keyword-only); a missing ratio now raises `ValueError` instead of `TypeError`
- Python `MertonModel.new_with_dynamics`, `almgren_chriss_impact`, `hw1f_cap_floor_price` and WASM `almgrenChrissImpact`, `hw1fCapFloorPrice`: unchanged (seven inputs)

### Statements: `NodeSpec` is keyword-only

- Python `NodeSpec("revenue", NodeType.value(), values=[("2025Q1", 1.0)], rollup="sum")` → `NodeSpec(node_id="revenue", node_type=NodeType.value(), values={"2025Q1": 1.0}, rollup="sum")`: `values` is the wire mapping (period id → float or `Money`), not a list of pairs or a `pd.Series`; the Rust fields `availability_dates`, `value_type` and `meta` are keywords too

### Statements analytics: the mapping, filter, ECL and real-estate data classes are keyword-only

- Python `ThreeStatementMapping("cash", "re", "ni", ["assets"], ["liabilities"], ["equity"])` → `ThreeStatementMapping(cash_node="cash", retained_earnings_node="re", net_income_node="ni", assets_nodes=["assets"], liabilities_nodes=["liabilities"], equity_nodes=["equity"])`
- Python `CreditMapping("total_debt", "ebitda", "interest_expense")` → `CreditMapping(debt_node="total_debt", ebitda_node="ebitda", interest_expense_node="interest_expense")`
- Python `PeerFilter(...)`, `QualitativeFlags(...)`, `PropertyTemplateNodes(...)`: keyword calls unchanged; positional calls and unknown keywords are rejected; `PropertyTemplateNodes(rent_roll=None)` is rejected (omit it for the default)
- Python `StagingConfig(...)`: keyword calls unchanged, except `None` for a field other than `pd_delta_relative` (`StagingConfig(pd_delta_absolute=None)`) is rejected: omit the keyword for the Rust default
- `StagingConfig` JSON (Rust `from_json`, Python `from_json`, WASM `classifyStage`): an omitted field now takes `StagingConfig::default()`. For `pd_delta_relative` that changes the meaning of an omitted key from "relative SICR trigger disabled" to the documented default `2.0`; pass `"pd_delta_relative": null` to disable it
- Python `Exposure("loan", 1e6, 0.45, 0.06, 3.0, 0.02, 0.015, dpd=35)` → `Exposure(id="loan", ead=1e6, lgd=0.45, eir=0.06, remaining_maturity_years=3.0, current_pd=0.02, origination_pd=0.015, days_past_due=35)`: the maturity and days-past-due keywords are the Rust field names (`remaining_maturity` → `remaining_maturity_years`, `dpd` → `days_past_due`); the `remaining_maturity` / `dpd` properties and `to_dataframe` columns are renamed the same way, so every Python name is the Rust field name; `None` for `days_past_due`, `consecutive_performing_periods`, `qualitative_flags` or `segments` is rejected (omit it for the default)
- Python `LeaseSpec("lease_a", "2025Q1", 100.0, end="2025Q4")` → `LeaseSpec(node_id="lease_a", start="2025Q1", base_rent=100.0, end="2025Q4")`; `growth_convention` also takes its serde name (`"per_period"`)

### Valuations: leg specs, `CdsTrancheParams` and the structured-credit inputs are keyword-only

- Python `FixedLegSpec("USD-OIS", 0.04, Tenor.semi_annual(), DayCount.THIRTY_360, start, end)` → `FixedLegSpec(discount_curve_id="USD-OIS", rate="0.04", frequency=Tenor.semi_annual(), day_count=DayCount.THIRTY_360, start=start, end=end)`
- Python `FloatLegSpec("USD-OIS", "USD-SOFR-3M", 0.0, Tenor.quarterly(), DayCount.ACT_360, start, end)` → `FloatLegSpec(discount_curve_id="USD-OIS", forward_curve_id="USD-SOFR-3M", spread_bp="0", frequency=Tenor.quarterly(), day_count=DayCount.ACT_360, start=start, end=end)`
- Python `PremiumLegSpec(start, end, Tenor.quarterly(), DayCount.ACT_360, 100.0, "USD-OIS", roll_rule=RollRule.CDS_IMM)` → `PremiumLegSpec(start=start, end=end, frequency=Tenor.quarterly(), day_count=DayCount.ACT_360, coupon_bp="100", discount_curve_id="USD-OIS", roll_rule=RollRule.CDS_IMM)`
- Python `CdsTrancheParams("CDX.NA.IG", 42, 3.0, 7.0, notional, "2029-12-20", 100.0)` → `CdsTrancheParams(index_name="CDX.NA.IG", series=42, attach_pct=3.0, detach_pct=7.0, notional=notional, maturity="2029-12-20", coupon_bp=100.0)`
- Python `PoolAsset("L1", {"type": "first_lien_loan"}, balance, 0.08, maturity, day_count=...)` → `PoolAsset(id="L1", asset_type={"type": "first_lien_loan"}, balance=balance, rate=0.08, maturity=maturity, day_count=...)`
- Python `RepLine("LINE-1", balance, 0.07, maturity, 12, DayCount.ACT_360, asset_type=...)` → `RepLine(id="LINE-1", balance=balance, rate=0.07, maturity=maturity, seasoning_months=12, day_count=DayCount.ACT_360, asset_type=...)`
- Python `StandardDealParams("ABS-1", pool, tranches, closing, maturity, "USD-OIS")` → `StandardDealParams(id="ABS-1", pool=pool, tranches=tranches, closing_date=closing, maturity=maturity, discount_curve_id="USD-OIS")`
- Python `AssetPool.with_accounts(...)`: keyword calls unchanged; the keywords are now the new Rust `PoolAccounts` struct (`valuations` schema `pool_accounts`), applied by the new `AssetPool::with_accounts(PoolAccounts)`

### Valuations: every Money input takes a `Money`; `FxOptionBuilder.delta_convention` takes the convention

One route per value. In Python a money input is a `finstack_quant.core.money.Money`
and nothing else: the bare-float-plus-`currency=` route is removed, and a float
raises `TypeError` naming the expected type. `FxOptionBuilder.delta_convention`
takes the Rust `FxDeltaConvention` in its serde form (the shape the
`FxOption.delta_convention` getter returns and `FxOption.european` takes) in
both hosts, and runs the new Rust `FxDeltaConvention::validate` (non-blank
venue), which `FxDeltaConvention::new` and `FxOption::validate` now share.
WASM money setters already took `Money` handles. Old → new:

- Python `Bond.zero_coupon(id, 1e6, issue_date, maturity, discount_curve_id, currency="USD")` → `Bond.zero_coupon(id, Money(1e6, "USD"), issue_date, maturity, discount_curve_id)`
- Python `BondBuilder.notional(1e6, currency="USD")`, `TermLoanBuilder.notional_limit(..)`, `InterestRateSwapBuilder.notional(..)`, `SwaptionBuilder.notional(..)`, `CapFloorBuilder.notional(..)`, `RevolvingCreditBuilder.commitment(..)` / `.drawn(..)`, `AssetBackedFacilityBuilder.commitment(..)` / `.drawn(..)` → `.notional(Money(1e6, "USD"))` (and likewise for each)
- Python `CapFloorBuilder.premium(date, 10_000.0, currency="USD")` → `CapFloorBuilder.premium(date, Money(10_000.0, "USD"))`
- Python `AssetPool.with_reserve(5e6, reserve_target=6e6, currency="USD")` → `AssetPool.with_reserve(Money(5e6, "USD"), reserve_target=Money(6e6, "USD"))`
- Python `TermLoanBuilder.upfront_fee(Money(..))` / `.upfront_fee(20_000.0, currency="USD")` → `TermLoanBuilder.upfront_fee({"amount": Money(20_000.0, "USD")})` (the `UpfrontFee` serde form only, as in WASM `upfrontFee`)
- Python `FxOptionBuilder.delta_convention("forward", Currency("USD"), "generic_interbank")` → `FxOptionBuilder.delta_convention({"kind": "forward", "premium_currency": "USD", "venue": "generic_interbank"})`; WASM `FxOptionBuilder.deltaConvention(kind, premiumCurrency, venue)` → `FxOptionBuilder.deltaConvention({ kind, premium_currency, venue })`
- Rust: new `FxDeltaConvention::validate(&self)`

### Valuations: instrument convenience constructors take one params object

Every instrument convenience constructor with more than seven inputs takes one
serde params struct (`deny_unknown_fields`, each a new `valuations` input
schema), and the hosts take the same object: a dict or JSON text in Python
(typed `Money`, `Currency`, `Tenor`, `DayCount`, `Bps` and `datetime.date`
values may sit inside it), a plain object or JSON text in WASM, typed
`generated.valuations.<Type> | JsonInput`. Inside a params object `notional` is
the `Money` wire form `{"amount": "1000000", "currency": "USD"}`, a tenor
`{"count": 3, "unit": "months"}`, a day count its label (`"act_360"`) and a
`spread_bp` an integer. One route per input: `Bond.fixed` and
`Bond.with_convention` lose the `currency=` keyword (the notional is a `Money`);
`FxForward.from_trade_date` and `FxOption.european` no longer take a bare float
notional; `FxForward.from_trade_date` reads `business_day_convention` by its
serde name only (no case or whitespace folding);
`EquityOption.european` drops its market-data id keywords (it binds the Rust
`european` default ids; use `EquityOption.builder()` for others). Instrument
wire forms and results are unchanged. Old → new:

- Rust `CdsIndex::from_preset(&preset, id, notional, side, start, end, recovery_rate, discount_curve_id, credit_curve_id)` → `CdsIndex::from_preset(&preset, CdsTradeParams { .. })` (new `cds::CdsTradeParams`, shared with the crate-internal `CreditDefaultSwap::new_isda(convention, coupon_bp, params)`)
- Rust `Bond::floating(id, notional, forward_curve_id, spread_bp, issue_date, maturity, frequency, day_count, discount_curve_id)` → `Bond::floating(FloatingBondParams { .. })`; `Bond::floating_with_convention(.., convention, discount_curve_id)` → `Bond::floating_with_convention(FloatingBondParams { .. }, convention)`
- Rust `FxForward::from_trade_date(13 args)` → `FxForward::from_trade_date(FxForwardTradeDateParams { .. })`; `FxSwap::from_trade_date(13 args)` → `FxSwap::from_trade_date(FxSwapTradeDateParams { .. })`; `Ndf::from_trade_date(14 args)` → `Ndf::from_trade_date(NdfTradeDateParams { .. })`; `FxOption::european(9 args)` → `FxOption::european(FxOptionParams { .. })`. The three trade-date structs are separate because their spot conventions differ: `FxForward` defaults `settlement_days` and `business_day_convention` (`Option`), `FxSwap` and `Ndf` require them, and `Ndf` names a `settlement_calendar_id` and adds `fixing_offset_days`.
- Rust `CapFloor::new(11 args)` → `CapFloor::new(CapFloorParams { .. })` (`frequency` may be omitted on the wire); `CmsSwap::from_schedule(14 args)` → `CmsSwap::from_schedule(CmsSwapScheduleParams { .. })` (`FundingLegSpec` is now serde, tagged by `type`); `BermudanSwaption::new(10 args)` → `BermudanSwaption::new(BermudanSwaptionParams { .. })` (the swap dates are `underlying_start_date` / `underlying_maturity`, as on `BermudanSwaption`; `swap_start` / `swap_end` are retired wire keys)
- Rust `ConventionSwapParams<'a>` loses its lifetime: `index_id` is an `IndexId` and the curve ids are `CurveId`s; it is now serde
- Python `Bond.fixed(id, notional, coupon_rate, issue_date, maturity, stub, discount_curve_id, *, currency=None)` → `Bond.fixed(id, notional: Money, coupon_rate, issue_date, maturity, stub, discount_curve_id)`; the same for `Bond.with_convention(.., convention, discount_curve_id)`
- Python `Bond.floating(id, notional, forward_curve_id, spread_bp, issue_date, maturity, frequency, day_count, discount_curve_id, *, currency=None)` → `Bond.floating(params)`; `Bond.floating_with_convention(.., convention, discount_curve_id, *, currency=None)` → `Bond.floating_with_convention(params, convention)`; WASM `Bond.floating(id, notional, forwardCurveId, spreadBp, issueDate, maturity, frequency, dayCount, discountCurveId)` → `Bond.floating(params)`, `Bond.floatingWithConvention(.., convention, discountCurveId)` → `Bond.floatingWithConvention(params, convention)`
- Python `CdsIndex.from_preset(preset, id, notional, side, start, end, recovery_rate, discount_curve_id, credit_curve_id)` → `CdsIndex.from_preset(preset, params)`; WASM `CdsIndex.fromPreset(preset, id, notional, side, start, end, recoveryRate, discountCurveId, creditCurveId)` → `CdsIndex.fromPreset(preset, params)`
- Python `FxForward.from_trade_date(id, base_currency, quote_currency, trade_date, tenor, notional, domestic_discount_curve_id, foreign_discount_curve_id, *, base_calendar_id=None, quote_calendar_id=None, settlement_days=None, business_day_convention=None, end_of_month=False)` → `FxForward.from_trade_date(params)`; WASM `FxForward.fromTradeDate(id, baseCurrency, quoteCurrency, tradeDate, tenor, notional, domesticDiscountCurveId, foreignDiscountCurveId, options?)` → `FxForward.fromTradeDate(params)` (the private `FromTradeDateOptions` is removed)
- Python `FxOption.european(id, base_currency, quote_currency, strike, expiry, notional, vol_surface_id, option_type, delta_convention_kind, premium_currency, venue)` → `FxOption.european(params)` with `delta_convention` as the `FxDeltaConvention` serde object `{"kind", "premium_currency", "venue"}`; WASM `FxOption.european(id, …, deltaConventionKind, premiumCurrency, venue)` → `FxOption.european(params)`
- Python `InterestRateSwap.from_conventions(id, notional, side, fixed_rate, start_date, maturity, index_id, discount_curve_id, forward_curve_id, *, currency=None)` → `InterestRateSwap.from_conventions(params)`; WASM `InterestRateSwap.fromConventions(id, notional, side, fixedRate, startDate, maturity, indexId, discountCurveId, forwardCurveId)` → `InterestRateSwap.fromConventions(params)`
- Python `EquityOption.european(id, ticker, strike, expiry, quantity, currency, option_type, *, discount_curve_id="USD-OIS", spot_id="EQUITY-SPOT", vol_surface_id="EQUITY-VOL", div_yield_id="EQUITY-DIVYIELD")` → `EquityOption.european(id, ticker, strike, expiry, quantity, currency, option_type)` (the WASM twin already had this shape)

### Python: params dicts accept `Tenor` and `DayCount` objects

Every Python params object (dict passed to a binding that reads a Rust serde
struct) now encodes a `Tenor` as its wire form `{"count": 6, "unit": "months"}`
and a `DayCount` as its label (`"act_360"`), as it already encoded `Money`,
`Currency` and `datetime.date`. `Tenor.semi_annual()` and `DayCount.ACT_360`
can stand where the wire values go; the wire values still work.

### Valuations: pricing calls take `model`, `metrics` and one `extras` object

`price_instrument` and every typed instrument's `price` keep `model` and
`metrics` and take the three rarer inputs as one optional `extras` object, the
serde form of the new Rust `pricer::PriceExtras` (a new `valuations` schema):
`metric_pricing_overrides` (the same JSON patch as before: present keys
replace, `bump_config` keys merge, an explicit `null` clears), `market_history`
(a `MarketHistory`) and `explain` (default `false`). Python reads a dict or JSON
text (typed `MetricPricingOverrides` / `MarketHistory` objects may sit inside
the dict); WASM a plain object or JSON text. An unknown key is rejected, and a
non-boolean `explain` is now a `validation` error rather than a `TypeError`.
Inside `extras`, the overrides and history are objects, not JSON strings.
Rust `pricer::price_instrument` takes `market_history: Option<&MarketHistory>`
instead of `Option<&str>` JSON. Results are unchanged. Old → new:

- `price_instrument(instrument, market, as_of, model, metrics, metric_pricing_overrides=None, market_history=None, explain=False)` → `price_instrument(instrument, market, as_of, model="default", metrics=None, extras=None)`
- `Instrument.price(market, as_of, model, metrics, metric_pricing_overrides=None, market_history=None, explain=False)` → `Instrument.price(market, as_of, model="default", metrics=None, extras=None)`
- WASM `priceInstrument(instrumentJson, marketJson, asOf, model?, metrics?, metricPricingOverrides?, marketHistory?, explain?)` → `priceInstrument(instrumentJson, marketJson, asOf, model?, metrics?, extras?)`; the same for `priceInstrumentWithMarket(instrumentJson, market, asOf, …)` and the typed `price(marketJson, asOf, model?, metrics?, extras?)`

### Statements: capital-structure swap and convention-bond setters take the Rust params objects

`ModelBuilder.add_swap`, `add_bond_with_convention` and
`add_swap_with_conventions` take the serde forms of the Rust
`SwapParams`, `BondConventionParams` and `SwapConventions` the Rust
`ModelBuilder` already took (Python dict or JSON text, with `Money` and
`datetime.date` values allowed inside; WASM object or JSON text). The three
types are new `statements` schemas. Unknown keys are rejected; in
`conventions` the tenors use their wire form `{"count": 1, "unit": "years"}`
(not `"1Y"`), day counts their labels (`"act_360"`, a Python `DayCount` object
is no longer accepted), and an omitted `business_day_convention` is Modified
Following. The instruments added are unchanged. Old → new:

- `add_swap(id, notional, fixed_rate, start_date, maturity_date, discount_curve_id, forward_curve_id)` → `add_swap(params)`; WASM `addSwap(...7 args)` → `addSwap(params)`
- `add_bond_with_convention(id, notional, coupon_rate, issue_date, maturity_date, convention, discount_curve_id)` → `add_bond_with_convention(params)`; WASM `addBondWithConvention(...7 args)` → `addBondWithConvention(params)`
- `add_swap_with_conventions(id, ..., forward_curve_id, fixed_frequency, fixed_day_count, float_frequency, float_day_count, business_day_convention=None)` → `add_swap_with_conventions(params, conventions)`; WASM `addSwapWithConventions(...12 args)` → `addSwapWithConventions(params, conventions)`

### Statements analytics: DCF, goal seek and corporate analysis take one inputs object

Rust `evaluate_dcf` takes `(model, &DcfInputs, &DcfOptions, market, as_of)`,
where the new `DcfInputs { wacc, terminal_value, ufcf_node, net_debt_override }`
holds the headline assumptions (`ufcf_node` defaults to `"ufcf"`,
`net_debt_override` to `None`). Rust `goal_seek` takes `(model, &GoalSeekSpec)`
with the new `GoalSeekSpec { target_node, target_period, target_value,
driver_node, driver_period, update_model, bounds }` (`bounds` optional, every
other field required). Both are new `statements_analytics` schemas. Python
`run_corporate_analysis` takes the `CorporateAnalysisOptions` object WASM
`runCorporateAnalysis` already took. Inside these objects a terminal value or
check suite is a typed object or its dict, not JSON text. A NaN `wacc` or
`target_value` in WASM is now a `TypeError` (`kind: "invalid_type"`) at the
JSON boundary instead of a Rust `validation` error. Results are unchanged.
Old → new:

- Rust `evaluate_dcf(model, wacc, terminal_value, ufcf_node, net_debt_override, options, market, as_of)` → `evaluate_dcf(model, &DcfInputs { .. }, options, market, as_of)`; Python `evaluate_dcf(model, wacc, terminal_value, ufcf_node="ufcf", net_debt_override=None, options=None, market=None, as_of=None)` → `evaluate_dcf(model, inputs, options=None, market=None, as_of=None)`; WASM `evaluateDcf(model, wacc, terminalValue, ufcfNode?, netDebtOverride?, options?, market?, asOf?)` → `evaluateDcf(model, inputs, options?, market?, asOf?)`
- Rust `goal_seek(model, target_node, target_period, target_value, driver_node, driver_period, update_model, bounds)` → `goal_seek(model, &GoalSeekSpec { .. })`; Python `goal_seek(model, target_node, target_period, target_value, driver_node, driver_period, update_model, bounds=None)` → `goal_seek(model, spec)`; WASM `goalSeek(modelJson, targetNode, targetPeriod, targetValue, driverNode, driverPeriod, updateModel, bounds?)` → `goalSeek(modelJson, spec)`
- Python `run_corporate_analysis(model, wacc=None, terminal_value=None, net_debt_override=None, cfads_node=None, interest_coverage_node=None, check_suite=None, market=None, as_of=None, ltv_value_node=None)` → `run_corporate_analysis(model, options=None, market=None)`

### Attribution: Python P&L attribution takes the `AttributionInputs` object

Python `attribute_pnl` and `attribute_pnl_many` take the Rust
`AttributionInputs` serde form (dict or JSON text) that WASM `attributePnlMany`
takes, with the instrument (typed wrapper or envelope JSON) beside it. Inside
the dict the markets are `MarketContext` objects or their serde dicts (not JSON
text) and the dates `datetime.date` or ISO strings (a `datetime.datetime` or
`pandas.Timestamp` is rejected); the opening model parameters and credit factor
model are objects under `model_params_t0` and `credit_factor_model`, no longer
JSON strings under `*_json`. Results are unchanged. Old → new:

- `attribute_pnl(instrument, market_t0, market_t1, as_of_t0, as_of_t1, method, config=None, full_cross_attribution=False, model_params_t0_json=None, credit_factor_model_json=None, credit_factor_detail_options=None)` → `attribute_pnl(instrument, inputs)`
- `attribute_pnl_many(instruments, market_t0, ..., credit_factor_detail_options=None)` → `attribute_pnl_many(inputs, instruments)` (the Rust and WASM argument order)

### Calibration: step constructors and CDS upfront/tranche quotes take one params object

Every `CalibrationStep` constructor now takes the step id, one `params` object
holding all wire fields of its `StepParams` variant (required and optional,
keyed by their serde names), and an optional quote set. Python passes a dict or
JSON text (a `Currency` value serializes to its code and a `datetime.date` to
its ISO string; currency codes are upper-case, so `"usd"`, which the positional
arguments accepted, is now rejected); WASM passes an object or JSON text. Python `**params`
keyword overrides are gone. Omitted `curve_id` / `vol_surface_id` and
`quote_set` still default to the id, a hazard `seniority` to `"senior"` and a
`parametric` `model` to `"ns"`; missing or unknown fields raise `ValueError`
(WASM `validation`). Rust `CdsQuote::upfront` and `CdsTrancheQuote::new` are
removed: build the `CdsQuote::CdsUpfront { .. }` variant or the
`CdsTrancheQuote { .. }` literal and call `validate()`. The step and quote
objects produced are unchanged. Old → new:

- `CalibrationStep.discount(id, currency, base_date, quote_set=None, curve_id=None, **params)` → `CalibrationStep.discount(id, params, quote_set=None)`; WASM `calibrationStepDiscount(id, currency, baseDate, quoteSet?, curveId?, params?)` → `calibrationStepDiscount(id, params, quoteSet?)`
- likewise `forward`, `hazard`, `inflation`, `vol_surface`, `swaption_vol`, `base_correlation`, `student_t`, `hull_white`, `cap_floor_hull_white`, `svi_surface`, `xccy_basis` and `parametric` (WASM `calibrationStepForward` … `calibrationStepParametric`): every positional field moves into `params` under its snake_case name (`tenor_years`, `discount_curve_id`, `recovery_rate`, `fit_tolerance`, ...)
- `CdsQuote.upfront(id, entity, currency, doc_clause, pillar, coupon_bp, upfront_pct, recovery_rate)` → `CdsQuote.upfront(params)` with `{id, entity, convention: {currency, doc_clause}, pillar: {tenor: {count, unit}} | {date}, coupon_bp, upfront_pct, recovery_rate}`; WASM `cdsQuoteUpfront(...8 args)` → `cdsQuoteUpfront(params)`
- `CdsTrancheQuote(id, index, series, attachment, detachment, maturity, upfront_pct, coupon_bp, currency, doc_clause)` → `CdsTrancheQuote(params)` with the `CdsTrancheQuote` wire fields (`convention: {currency, doc_clause}`), the object WASM `cdsTrancheQuote` already takes

### Models: option pricers take one market object in Rust, Python and WASM

The option market inputs travel as one object. Rust: `closed_form::bs_greeks`
and `bs_greeks_unchecked` take `(&OptionMarketParams, theta_days_per_year)`;
`barrier_option_price`, `asian_option_price`, `lookback_option_price`,
`quanto_option_price`, `quanto_price`, `geometric_asian_price` and
`arithmetic_asian_tw_price` take `&OptionMarketParams` plus their product
inputs (for the quanto, `rate` is the domestic rate and `volatility` the asset
volatility); `geometric_asian_price_times` and
`arithmetic_asian_tw_price_times` take the new `AsianFixingSchedule`
(expiry, discount factor, fixing times). `OptionMarketParams`,
`OptionContractParams`, `GbmEuropeanFdSpec` and `HestonMcSettings` have
published JSON schemas.

Python passes them as a dict or JSON text, WASM as the schema-typed object or
JSON text. The `market` keys are `spot, strike, rate, dividend_yield,
volatility, time_to_expiry, option_type` (`"call"`/`"put"`, replacing `is_call`);
the finite-difference Greek `spec` takes the same keys plus optional
`num_paths, seed, num_steps, bump_size, currency`. Black–Scholes COS uses
`OptionMarketParams`. Variance Gamma and Merton COS share
`OptionContractParams` and take model dynamics separately. COS term counts
are optional arguments. Heston and rough Heston simulations take only opening
spot in `initial_state`; both derive opening variance from `v0`. Heston
pricing uses `OptionContractParams` and separate `HestonParams` with
`v0, kappa, theta, vol_of_vol, rho`. A non-finite number inside one of these objects is rejected
when the object is read (Python `ValueError`, WASM `TypeError`). Prices are
unchanged. Old → new (WASM: the camelCase twin with the same arguments):

- `bs_greeks(spot, strike, rate, div_yield, vol, expiry, is_call, theta_days_per_year)` → `bs_greeks(market, theta_days_per_year)`
- `barrier_option_price(spot, strike, barrier, rate, div_yield, vol, expiry, direction, knock, is_call)` → `barrier_option_price(market, barrier, direction, knock)`
- `asian_option_price(spot, strike, rate, div_yield, vol, expiry, num_fixings, averaging, is_call)` → `asian_option_price(market, num_fixings, averaging)`
- `lookback_option_price(spot, strike, rate, div_yield, vol, expiry, extremum, strike_type, is_call)` → `lookback_option_price(market, extremum, strike_type)`
- `quanto_option_price(spot, strike, expiry, rate_domestic, rate_foreign, div_yield, vol_asset, vol_fx, correlation, is_call)` → `quanto_option_price(market, rate_foreign, vol_fx, correlation)`
- `heston_price(spot, strike, expiry, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, is_call)` → `heston_price(contract, heston, settings)`
- `bs_cos_price(spot, strike, rate, div_yield, vol, expiry, is_call, n_terms)` → `bs_cos_price(market, n_terms)`
- `vg_cos_price(spot, strike, rate, div_yield, sigma, theta, nu, expiry, is_call, n_terms)` → `vg_cos_price(contract, sigma, theta, nu, n_terms)`
- `merton_jump_cos_price(spot, strike, rate, div_yield, sigma, mu_jump, sigma_jump, lambda_, expiry, is_call, n_terms)` → `merton_jump_cos_price(contract, sigma, mu_jump, sigma_jump, lambda_, n_terms)` (WASM `lambda`)
- `monte_carlo.price_heston(spot, strike, rate, div_yield, kappa, theta, vol_of_vol, rho, v0, expiry, is_call, num_paths, seed, num_steps, currency)` → `price_heston(contract, heston, settings)` (`settings`: optional `num_paths`, `seed`, `num_steps`, `currency`)
- `monte_carlo.finite_diff_delta` / `finite_diff_gamma(spot, strike, rate, div_yield, vol, expiry, is_call, num_paths, seed, num_steps, bump_size, currency)` → `finite_diff_delta(spec)` / `finite_diff_gamma(spec)`
- `EuropeanPricer.price_gbm`, `PathDependentPricer.price_asian` and `price_with_lrm_greeks(spot, strike, rate, div_yield, vol, expiry, is_call, num_steps, currency)` → `(market, num_steps, currency)`
- `LsmcPricer.price_american(spot, …, is_call, currency, num_steps, basis, basis_degree)` → `price_american(market, currency, num_steps, basis, basis_degree)`
- `LsmcPricer.price_american_unbiased(spot, …, is_call, pricing_seed, currency, num_steps, basis, basis_degree)` → `price_american_unbiased(market, pricing_seed, currency, num_steps, basis, basis_degree)`

### Models: Monte Carlo, payoff and Hull-White caplet entry points take one inputs struct

Rust functions with more than seven inputs now take a struct. `McEngine::price`
and `price_with_capture` take `engine::McPricingInputs`;
`engine::simulate_path_fractional` takes `engine::FractionalPath`;
`LsmcEngine::{price, fit_exercise_policy, price_with_policy, price_unbiased}`
take `pricer::lsmc::LsmcInputs`; `pricer::heston::price_heston` takes
`OptionContractParams`, `HestonParams` and optional `HestonMcSettings`; `EuropeanPricer::price_gbm` and
`PathDependentPricer::{price_asian, price_with_lrm_greeks}` take
`OptionMarketParams`; `EuropeanPricer::price`, `PathDependentPricer::price` and
`price_with_lrm_greeks_from_payoff` take a `TimeGrid` (the latter reads its
score parameters from the process). `PathDependentPricer::price_with_grid` is
gone (use `price`). `AutocallablePayoff::new`, `BarrierOptionPayoff::new` and
`CliquetCallPayoff::new` take `AutocallableParams`, `BarrierOptionParams` and
`CliquetCallParams`; `Asian::with_history` takes `AsianFixingHistory`;
`hw1f_term_caplet_price_from_dfs_with_model` takes `TermCapletPeriod`. Prices
are unchanged and Python and WASM signatures do not change.

### Valuations: field-unit tables move to `results`; the registry catalog to its own file

`valuations/src/pricer/json.rs` (2,800 lines) is split by concern. The result
field-unit tables (`MetricFieldUnit`, `tranche_metric_field_units`,
`tranche_solve_field_units`, `scenario_table_field_units`,
`pool_report_field_units`, `bond_quote_set_field_units`,
`simulation_field_units`, `equity_metrics_field_units` and their structs) move
to valuations-core `results` and are reached as
`finstack_quant_valuations::results::*` instead of `pricer::*`. The catalog
(`list_models*`, `list_standard_metrics*`, `metrics_for_instrument`,
`metric_metadata`, `MetricMetadata`, `MetricCoordinate`) moves to
`pricer/catalog.rs` with its `pricer::` paths unchanged. Python and WASM
names do not change.

### WASM: TypeScript declarations come from the Rust bindings

The hand-written `index.d.ts` is gone. Every `#[wasm_bindgen]` export now types
its `JsValue` parameters and returns (`unchecked_param_type`,
`unchecked_optional_param_type`, `unchecked_return_type`), so wasm-bindgen's
`pkg/finstack_quant_wasm.d.ts` publishes no `any`, and the Rustdoc above each
export is its JSDoc. `index.d.ts` is generated over it by
`scripts/gen-facade-dts.mjs`. TypeScript-visible changes:

- Namespace interfaces are named by their facade path
  (`ValuationsInstrumentsNamespace`, `ModelsVolatilityNamespace`, ...); the
  `*Constructor` interfaces are gone (use `typeof valuations.instruments.Bond`).
- Parameters are declared under the camelCase names the runtime errors use
  (`instrumentJson`, `marketJson`, `envelopeJson`, `varValue`, ...).
- Classes declare `[Symbol.dispose]()`; the package declares `Symbol.dispose`
  itself, so ES2020 consumers still compile without `esnext.disposable`.
- Members the facade adds (`MarketContext.insert`, `CashFlowBuilder.build(market?)`,
  `Performance.lookbackReturns`/`periodStats` with a `FiscalConfig`, the chained
  builder methods) are declared by augmenting the package's classes.

### Statements: forecasts run on typed parameters; covenant bridge reads the result calendar

`ForecastSpec::parameters` reads a forecast's parameter map once into `ForecastParameters`, and every method runs on that typed form; `ForecastSpec::validate` is that read. A missing required parameter (`rate`, `season_length`, `mode`, ...) now fails when the model is built rather than when it is evaluated, and errors name the offending key (`Invalid GrowthPct forecast parameter 'rate': ...`, or the unknown key with the accepted ones); `null` is rejected rather than read as absent. Values do not move.

Renamed or reshaped public APIs:

- `RollupSpec::default_for` is `RollupSpec::declared_methods` (Python `RollupSpec.declared_methods`, WASM `statements.rollupSpecDeclaredMethods`).
- `EvaluationContext::new` and `PeriodHistory::from_named_maps` take the model's periods explicitly; `ttm()` no longer derives Gregorian periods from history keys.
- `evaluate_covenants(covenants, results)` and `forecast_breaches(covenants, results, config)` drop the `model` argument and test every period of `StatementResult::periods` (Python and WASM `statements_analytics.evaluateCovenants` / `forecastBreaches` likewise; the UI covenant requests drop `modelJson`). `StatementsAdapter::new` takes only the results. `CreditAssessment::compute` also spans `StatementResult::periods`.
- `finstack_quant_covenants::evaluate_series_generic` is `evaluate_series_from_model`.
- `PeerSet::from_universe` returns `(PeerSet, excluded_count)`; the Python and WASM constructors still return the peer set.
- `PnlBucketShare::field` is the `PnlBucket` enum and `MertonReport::errors` is keyed by `MertonReportMeasure`; both serialize to the same snake-case strings as before.

### Python: relative value report keeps typed extractors and takes universe JSON

`RelativeValueDimension.y` and `.x` return the `MetricExtractor` document (`{"named": "oas_bp"}`, `{"custom": ...}`, `{"multiple": "ev_ebitda"}`) instead of a flattened `"name"` / `"multiple:<name>"` label that could not tell a custom key from a canonical metric of the same name; they now equal the WASM report's `y` and `x`. `relative_value_report` also takes the whole `universe` as one JSON array text, as WASM does. No value moves.

### Core and FX: explicit quote metadata and value dates

`FxPairConvention::new(base, quote, pip_size)` (Python `FxPairConvention(base, quote, pip_size)`, WASM `new core.FxPairConvention(...)`) keeps the stated pair order and an explicit pip size, with `rate_points(pips)`. `FxQuoteConvention`, `fx_pair_convention` (market ranking, USD direct/indirect, inferred pips and lag), `fx_spot_date`, `fx_standard_settlement_days` and `add_fx_standard_tenor` are removed in every host. `fx_forward_date(spot, tenor, business_day_convention, preserve_business_eom, calendar)` adds a tenor to an open spot on one explicit calendar, with business month-end preservation for month/year tenors and errors for closed or non-advancing results. FX spot, forward, swap, NDF and XCCY calibration count their stated spot lag in joint business days on their two stated calendars; the CLS-style USD-intermediate exception is gone, so a US holiday at T+1 now delays spot. Forward/swap/NDF trade-date params require both calendars, the lag, the business-day convention and `preserve_business_eom` (was `end_of_month`). `FxSpot` uses an explicit `settlement_date` as given and drops `business_day_convention`; without one it needs `settlement_days` and both calendars. `FxForward.with_forward_pips(spot, pips, convention)` takes the matching convention (record CORE-CONVENTIONS-011).

### Core: statistical annualization separate from calendar counts

`PeriodKind::periods_per_year` is renamed `annualization_factor` (`annualizationFactor` in WASM) and keeps 252/52/12/4/2/1 for return statistics only. The new `slots_per_year` (12/4/2/1, `None` for daily and weekly) is the calendar partition count used by period bounds, LTM, rollup, `growth_rate` year conversion, the rent-roll annual escalator and the liquidity runway check; the escalator and runway check now reject daily and weekly models instead of using 252 trading days. `PeriodKind::prior_observation_date` is removed in every host. Statements `annualize(value, annualization_factor)` and `growth_rate(series, periods)` require their second argument; the frequency-derived defaults are gone (record CORE-CONVENTIONS-014 #9).

### Statements: LTM windows step through the period plan

`ttm()`, `capital_structure_summary` and the credit assessment's trailing sums (leverage, interest coverage and the leverage check) share one window, `finstack_quant_core::dates::ltm_window(target, plan)`: the twelve calendar months ending at the reporting period's end, stepped through the model's `PeriodPlan` in its own calendar (record CORE-CONVENTIONS-014 #11, #12). The target must be a period of the plan, and slots the plan does not hold count as missing. Monthly, quarterly, semi-annual and annual plans read 12, 4, 2 and 1 periods, Gregorian or fiscal. A daily window needs every calendar day of the trailing year, 365 or 366: a business-day grid's absent weekends and holidays are not proven zero, so its LTM is incomplete (NaN in `ttm()`, `None` in the credit metrics). Daily models used to read a fixed 252 periods, about 8.3 months of a calendar-day grid, so LTM EBITDA read low and leverage about 1.45x high. A weekly plan has no twelve-month window (52 weeks is 364 days and a fiscal week 53 may be short), so weekly `ttm()` and weekly credit windows are errors. `StatementResult.periods` is now the evaluated `PeriodPlan` (`{calendar, periods}`, omitted for hand-built results), and `CreditAssessmentReport::new` is fallible so its text rendering cannot fail. `PeriodId::shift(periods, calendar)` takes the calendar context in Rust, Python and WASM and replaces `next_fiscal`/`prev_fiscal`.

### UI: facade-coverage ledger counts three more computations

`core.SifmaSettlementClass` (`fromAgencyTerm` classifies a term), `models.credit.lgd.WorkoutCosts` (`totalRate` computes and `standard` is a preset) and `margin.excessCollateral` (computes collateral less required) are no longer excluded as data constructors. The `data-constructor` definition now also covers a handle class whose other members only read back what it was built from, which is what `DayCountContext`, `VolCubeExpirySlice` and `LatentSingleFactor` are. Coverage reads 176 of 639 units.

### UI: attribution fixture with a book Rust does not reconcile

The portfolio attribution fixture adds a native `metrics_based` attribution at a host tolerance of zero, where `portfolioAttributionReconciliationCheck` reports the book not reconciled (the bucket sum leaves about 5e-13 USD). Tests now render the "Not reconciled" badge and a position whose residual is outside Rust's tolerance. At the host's one-cent tolerance the book always reconciles, because Rust's buckets include the residual.

### UI: portfolio returns keep NaN and Infinity apart

`@finstack/use-portfolio-returns` keys its three queries with `requestSnapshot`, as `use-correlation-matrix` does, instead of putting the cloned request in the TanStack key, whose JSON hashing turns NaN and Infinity both into `null`. A blank horizon followed by an overflowing one now makes two native `twrrLinked` calls and shows each native error.

### UI: correlation heatmap always draws Rust's matrix

`@finstack/correlation-heatmap` no longer checks coefficients against [-1, 1] and drops its chart for one outside; Rust now returns every coefficient inside the bound, so the heatmap and the table always show. The Repaired-stamp test says plainly that no native panel reaches Rust's repair today.

### Portfolio: attribution explain text and bucket shares agree on a zero total

`PortfolioAttribution::explain` treated `total_pnl` as zero only below `1e-10`, while `pct_of_total_by_bucket` uses the default `RoundingContext` (half a unit of the base currency's output scale). A total of 0.003 USD printed percentage shares in the text while the table had none. Both now use one rule, so the text prints `0.0%` exactly where the shares are `None`. Python and WASM explain text follow. No UI fixture moves.

### Analytics: correlation matrices stay inside [-1, 1]

`Performance::correlation_matrix` now returns a diagonal of exactly 1 and clamps every off-diagonal to `[-1, 1]`, on the clean path and after Higham repair. The validator it applies first tolerates `1e-10` on the diagonal and `1e-12` past each bound, so rounding in the Pearson estimate or the repair could return a coefficient such as `1.0000000000000002`, which a correlation cannot take. Python `Performance.correlation_matrix` and WASM `correlationMatrix` document the bound. No UI fixture moves.

### Portfolio: a linked return rejects a non-finite horizon

`twrr_linked` now returns a validation error when `horizon_years` is NaN or infinite. A blank horizon field (NaN) used to come back as an unannualised cumulative return and an overflowing one (`1e999`, infinity) as an annualised return of zero. Zero and negative horizons still skip annualisation. Python `twrr_linked` raises `ValueError`; WASM `portfolio.twrrLinked` throws a `validation` `FinstackError`. The returns UI fixture records both native errors. No existing value moves.

### UI: facade-coverage ledger lists what a screen never wires

`finstack-quant-ui/facade-coverage.json` now excludes 108 more facade units, each with a reason from a closed set in `scripts/coverage.mjs` that `coverage --check` enforces: constructors and label parsers (`data-constructor`), enum-variant constructors (`schema-form`), copy-with-field setters (`setter`), stored-field lookups (`getter`), shape validators (`validator`) and the four `analytics` series functions that repeat a `Performance` method (`performance-duplicate`). The six covenant `with_*` setters move from `schema-form` to `setter`. Coverage reads 176 of 636 units instead of 176 of 744; computations, numeric checks, presets and later-phase targets stay counted.

### UI: correlation heatmap on the workbench Risk tab

`@finstack/correlation-heatmap` draws the Pearson correlation matrix of a host return panel, one native `analytics.Performance` `correlationMatrix` chain through `@finstack/use-correlation-matrix`, as a heatmap and a table in Rust's ticker order, stamped Repaired only when Rust's `repaired` flag says it projected the estimate to the nearest valid correlation matrix. `portfolio-workbench` shows it on the Risk tab when the host supplies a `correlation` panel.

### UI: exposure report on the workbench Risk tab

`@finstack/exposure-report` presents a book's primitive exposures, one native `portfolio.primitiveExposures` call through `@finstack/use-exposure-report`: each primitive's net and gross quantity and value in the base currency with a chart of the two values, each requested additive metric's net and gross amount, and the position-aware paths, all as returned; and, when the host supplies factor definitions, the book's `portfolio.computeFactorSensitivities` matrix. Rust returns both net and gross; nothing is summed or netted in JavaScript. `portfolio-workbench` shows it on the Risk tab over its book and market, with the host's `exposures` metric ids and factors.

### UI: Brinson report and the workbench Brinson tab

`@finstack/brinson-report` presents multi-period Brinson-Fachler attribution of the host's sector weights and returns, one native `portfolio.brinsonLinked` call through `@finstack/use-brinson-linked`: the compounded portfolio and benchmark returns, each sector's Carino-linked allocation, selection, interaction and total with Rust's linked total of each effect, a chart of the sectors' linked totals, and every period's returns, sector effects and totals, all as returned. Nothing is summed, differenced or linked in JavaScript. `portfolio-workbench` adds a Brinson tab when the host supplies `brinson` periods.

### UI: portfolio returns and the workbench Returns tab

`@finstack/portfolio-returns` answers how the book has performed from the host's performance data in three forms, each one native call through `@finstack/use-portfolio-returns`: `twrrLinked` of sub-period decimal returns over a horizon in 365-day years (cumulative, annualised, `is_annualised` and the period count as returned, saying when Rust did not annualise), `twrrModifiedDietz` of one period's values and flows (a decimal for the period; contributions positive) and `mwrXirr` of dated investor flows (an annualised decimal on Act/365F; contributions negative). Nothing is defaulted, derived from dates or sign-flipped, and each form shows its native error. `portfolio-workbench` adds a Returns tab when the host supplies `returns`.

### Portfolio: linked returns say whether they were annualised

`LinkedReturn` gains `is_annualised`: true when `twrr_linked`'s `horizon_years` is at least one year, false for shorter or non-positive horizons, where `annualised` mirrors `cumulative` (GIPS 2020 reports them cumulatively). A report can now say which figure it shows without restating that rule. Python `LinkedReturn.is_annualised` (and a `to_dataframe` column); WASM `portfolio.twrrLinked` returns the field. The host parity fixture pins a two-year and a half-year link for both hosts. No existing value moves.

### UI: portfolio attribution report and the workbench Attribution tab

`@finstack/portfolio-attribution-report` shows why the book moved between two snapshots as Rust returns it: the book total, the reconciliation verdict, gap and tolerance from `portfolioAttributionReconciliationCheck`, the base-currency buckets with their shares from `portfolioAttributionPctOfTotalByBucket` as a table and a signed chart, a row per position with one column per bucket from that position's own Rust buckets, the selected position through `pnl-attribution-report` (which now exports its factor table and chart as `PnlFactorBuckets`), and `portfolioAttributionExplainText`. `@finstack/use-attribute-portfolio-pnl` makes the one `attributePortfolioPnl` call. `portfolio-workbench` adds an Attribution tab when the host supplies `attribution` (closing market, closing date and reconciliation tolerance, none defaulted); every Rust method is offered with none preselected, and a failing method shows the native error.

### Portfolio: attribution buckets and their shares from Rust

`PortfolioAttribution::pct_of_total_by_bucket()` lists the book's eleven base-currency factor buckets (`fx_translation_pnl` included) and `residual`, the amounts `reconciliation_check` adds up, as `PnlBucketShare` records with each share of `total_pnl` (`None` when the total is effectively zero), so a report reads the buckets from Rust instead of naming them. Bound as Python `PortfolioAttribution.pct_of_total_by_bucket` and WASM `portfolio.portfolioAttributionPctOfTotalByBucket`; the host parity fixture pins both. No existing value moves.

### UI: portfolio workbench

`@finstack/portfolio-workbench` puts the portfolio components under tabs over a host-supplied book, market and margin date: Positions (summary and positions), Risk (aggregated metrics and a bucketed series), Scenarios (scenario P&L, shown when the host supplies scenarios), Margin and Book (a `PortfolioSpec` validated natively before it is valued). It adds no native call; the Scenarios and Margin runs start when their tab opens. No value moves.

### Portfolio: multi-period Brinson in one call

`brinson_linked(periods)` attributes each period with Brinson-Fachler and Carino-links the results, returning the existing `CarinoLinkedAttribution`: every period's sector allocation, selection and interaction, the linked effects by sector and by effect, and the compounded portfolio and benchmark returns, whose difference the linked effects sum to. It is identical to `carino_link` over each period's `brinson_fachler` result, so a host no longer loops over periods; a period that fails Brinson-Fachler is named by its zero-based index. Bound in Python (`brinson_linked`) and WASM (`portfolio.brinsonLinked`). No value moves.

### UI: recovery-waterfall fields from the contracts; DCF WACC default

`recovery-waterfall` labels its claim, collateral and workout fields through `labelFor` and takes their help from the generated contracts (the new `recovery_claim` contract root, and `workout_lgd`), so "Direct costs" reads "Direct cost rate", "Time to resolution" reads "Workout years" and "Exposure at default" reads "EAD" (`labelFor` upper-cases `ead`). "Add claim" starts the accrued, penalties and haircut fields empty instead of at 0, so an untyped amount is rejected by Rust rather than sent as zero. The statements workbench's valuation panel leaves the DCF form's WACC unset until the calculator or an edit sets it, so a form given `defaultAssumptions` shows its WACC instead of an empty field.

### Comps: each scatter point says whether it is the subject

`RelativeValuePoint` carries `is_subject`, true on the subject's point (the first point of a dimension whose subject has both metrics) and false on every peer's, as `RelativeValueCompany` already does for the comps table. Python `RelativeValuePoint.is_subject`, the WASM `relativeValueReport` result and the generated schema and TypeScript types include it; `RelativeValuePoint.from_json` now requires it. `relative-value-table` marks the subject on its scatter from it instead of comparing `company_id` with `subject_id`. No value moves.

### Comps and capital structure: only the conditions they name

`capital_structure_summary` lists the hedges it leaves out (interest rate swaps, cap/floors and swaptions) instead of skipping every instrument that is not one of the four debt kinds; an instrument that is neither is an error naming its type, as the statements crate's capital structure already does. Every instrument a model can hold today is one or the other, so no value moves. `relative_value_report` reports `non_finite_score` only when the composite or confidence is non-finite; any other scoring error is returned instead of being relabelled. No value moves.

### UI: capital structure and recovery tables fit the gallery width

`capital-structure-report` and `recovery-waterfall` now show returned numbers as `relative-value-table` does: ratios and rates to four decimals (never "-0") and the share of debt as a percent to two decimals, each cell keeping every returned digit in its title. The debt stack names its currency once in the caption ("Debt stack at 2025Q4 (USD)") instead of in three column headers. Both tables now show every column at gallery width, the debt stack's maturity and the recovery table's deficiency included. `formatRatio`, `formatShare` and `formatValue` still override the display.

### UI: id-combobox keeps typed free text

The `id-combobox` registry primitive no longer empties its value when the suggestions close. Typing an id that is not among the suggestions and then pressing Tab, Escape or clicking elsewhere left the field (and every SchemaForm node, metric or curve id built on it) as `''`, so Rust reported for example "Target node '' not found". Typed text is now the value until the user types again or picks a suggestion.

### Comps: relative value table in the UI

The `relative-value-table` registry component, with the `use-relative-value-report` hook, takes a host-supplied subject and universe of `CompanyMetrics`, edits a `RelativeValueReportConfig` (peer filter, period basis, scoring dimensions, multiples) with a schema form over the generated contract, and draws `statements_analytics.relativeValueReport` as returned: the comps table with the subject marked, each multiple's subject value, percentile bar, z-score and peer statistics, each dimension's regression scatter through Rust's fitted values with its signal, z-score and exclusion reason, and the composite score or Rust's reason for none. `labelFor` upper-cases `ev`, `fcf`, `ntm`, `pb`, `pe` and `ptbv`, so a multiple reads "EV EBITDA" or "PE".

### Comps: partial and strict peer filters

Every `PeerFilter` field may now be omitted from its serde document, an omitted list or bound screening on nothing, so `{"gics_sectors": ["Energy"]}` is a complete filter and `{}` accepts every company; until now a filter had to list all nine fields. `PeerFilter` and `CompanyMetrics` reject unknown keys, so a misspelt criterion (`gics_sector`) or metric (`ebidta`) is a validation error instead of being dropped. This applies to every host that passes either as JSON (`relative_value_report`, `PeerSet`, `PeerFilter.from_json`, `CompanyMetrics.from_json`, WASM `relativeValueReport` and `peerSetFromUniverse`); the flat-dict constructors, which file an unknown name under `custom`, are unchanged. No value moves.

### Statements: capital structure report in the UI

The `capital-structure-report` registry component, with the `use-capital-structure-summary` hook, edits a `CapitalStructureSummaryConfig` (period, EBITDA node, optional cash node) with a schema form and draws `statements_analytics.capitalStructureSummary` as returned: headline debt, cash, net debt, LTM EBITDA with the LTM periods found, leverage and coverage; the debt stack in Rust's tranche order with the figures through each payment class; and the maturity wall by year. Ratios Rust leaves undefined show as Undefined. In the statements workbench's Metrics & ratios tab it sits in a capital structure disclosure. `labelFor` upper-cases `ebitda` and `ltm`, so labels such as "EBITDA bridge" and "LTM EBITDA" change case.

### Statements: normalization editor in the UI

The `normalization-editor` registry component, with the `use-normalize` and `use-normalization-config-validator` hooks, edits a `NormalizationConfig` (add-backs, deductions and caps) with a schema form validated by `statements.validateNormalizationConfigJson`, and draws `statements.normalize` as returned: a base-to-adjusted bridge for a chosen period, each step between Rust's `adjusted_value`s, and a table of the adjustments by period. In the statements workbench's Metrics & ratios tab it sits in an Adjusted EBITDA disclosure over the reported result.

### Statements: adjusted value through each normalization step

`AppliedAdjustment` gains `adjusted_value` (Python `AppliedAdjustment.adjusted_value`, WASM and JSON `adjusted_value`): the target's value through that adjustment, the period's `base_value` plus the capped amounts of this and every earlier adjustment in declaration order. The last step equals `final_value` exactly, so a base-to-adjusted bridge reads each step's position from the result instead of summing. The field is required in `NormalizationResult` JSON; no existing value moves.

### Models: recovery waterfall in the UI

The `recovery-waterfall` registry component and `use-recovery-waterfall` hooks draw `models.credit.recoveryWaterfall.allocateRecovery` (Rust's per-claim allocations with recovery rates and deficiencies, and the estate's distributed and undistributed totals), `models.credit.lgd.workoutLgd` and `seniorityRecoveryStats`. Collateral types and seniority classes come from the generated `workout_lgd` and `seniority_calibration` contracts, now UI schema roots, and the calibration choice from `seniorityCalibrationIds`.

### Models: seniority calibration ids

`seniority_calibration_ids()` (Python `models.credit.lgd.seniority_calibration_ids`, WASM `models.credit.lgd.seniorityCalibrationIds`) lists the seniority recovery calibrations of the embedded credit registry, the default first, so a host can offer the exact ids `seniority_recovery_stats` accepts without holding its own list.

### Statements: table and CSV export in the UI

The `statement-export` registry component and `useStatementResultTable` hook draw `statements.statementResultToTableWide` and `statementResultToTableLong`, with a CSV download of exactly those cells (RFC 4180; a cell with no value is an empty field; numbers are unrounded). The statements workbench offers it below the grids, over the reported or annual result they show. `TableEnvelope` gains an optional `missingCell`.

### Statements: WACC calculator in the UI

The `wacc-calculator` registry component and `useWacc` hook draw `statements_analytics.wacc`: five decimal inputs, the two capital weights separate, and the rate Rust returns or its rejection. In the statements workbench's Valuation tab, Use as WACC sets the DCF form's WACC field on the user's click; `DcfAssumptionsForm` gains optional `wacc` and `onWaccChange` props for that.

### Comps: relative value report

`relative_value_report(subject, universe, config)` filters the universe and returns what a comps screen draws, so a host computes nothing: `companies` (the subject first, then peers in universe order, with one value per configured multiple), `multiples` (each with the subject's value, peer statistics, percentile and z-score) and `dimensions` (one per configured dimension in config order, excluded ones included, with subject values, peer statistics, percentile, z-score, regression, the direction-adjusted `signal`, `excluded_reason`, and scatter `points` that carry their fitted Y). `composite` is the result of `score_relative_value` on the same peers and dimensions, which now shares one per-dimension evaluation with the report; when it cannot be produced it is null, `composite_excluded_reason` says why and the table still renders. Empty `dimensions` and `multiples` together, or a negative or non-finite weight, are errors. Metrics carry no currency, so normalizing them is the caller's job.

Bound in Python (`relative_value_report`, `RelativeValueReport`, `RelativeValueCompany`, `RelativeValueMultiple`, `RelativeValueDimension`, `RelativeValuePoint`) and WASM (`statements_analytics.relativeValueReport`), with schemas for the request and the result. No value moves.

### Statements: debt stack by seniority

`capital_structure_summary(results, model, config)` answers how much debt sits ahead of each tranche and what leverage and coverage look like through it. It gives one row per bond, convertible, term loan and revolver at its balance at `config.period`, with a revolver at its drawn balance and swaps, cap/floors and swaptions left out; rank and class come from the waterfall's payment classes, or one implicit class when there are none. Tranches in a class are pari passu, so cumulative debt, leverage and coverage are figures through the whole class. It also gives total debt, cash, net debt, LTM EBITDA, total and net leverage, total coverage, and a maturity wall by calendar year of final maturity, which uses the balance at the period and ignores later amortization.

`ebitda_node` is required and never guessed; `cash_node` is optional. LTM EBITDA and interest sum the trailing year of calendar periods, the slots `ttm()` reads. A window short of a year, or with an EBITDA value missing, gives null ratios and `ltm_periods_found` says how many periods held: a quarterly figure is never annualised. Zero or negative LTM EBITDA also gives null ratios. Mixed currencies are an error, since no FX is applied.

Bound in Python (`capital_structure_summary`, `CapitalStructureSummary`, `CapitalStructureTranche`, `MaturityWallBucket`) and WASM (`statements_analytics.capitalStructureSummary`), with schemas for the request and the result. No value moves.

### Convertibles: theta carry no longer counts the issue draw or a same-day coupon

`theta_carry`, and so `theta`, on a `ConvertibleBond` counted the -notional issue draw when valued on its issue date, and a coupon paid on the valuation date, which the tree's PV already leaves out. A bond issued on the valuation date showed theta near -1,000,000. Both now follow `Bond`: carry excludes the draw and receipts fall in `(as_of, horizon]`. Values change on those two dates only.

P&L attribution of a convertible from one of those dates reads the same carry, so it changes too: the UI's fixture, one day from a 1,000 issue on 2025-01-01, moves from total P&L -1,060.42 and carry -999.90 to -60.42 and 0.10.

### Convertibles: value across share prices

`convertible_spot_ladder(bond, market, as_of, spots)` answers how a convertible moves with its stock. Each row reprices on the market with only the share price moved and gives price, delta, hedge ratio, regime, conversion value, parity, conversion premium and the bond floor, in ascending share price. Omit `spots` for 0.25 to 3 times the market's share price. Spots must be finite and positive; they are sorted and duplicates dropped. Price, delta, parity, conversion value, premium and floor are the definitions of the metrics of those names, so the market row equals them.

Regime is new and defined once in Rust, from `hedge_ratio`, delta over the effective conversion ratio: `busted` below 0.40, `balanced` to 0.80, `equity_sensitive` above. The cutoffs are one market convention, the bands Putnam and Lord Abbett publish; MSCI uses thirds (`docs/REFERENCES.md#convertible-delta-bands`). A mandatory-variable bond delivers a moving share count, so its hedge ratio and regime are null.

Bound in Python (`convertible_spot_ladder`, `ConvertibleSpotLadder`, `ConvertibleSpotRow`) and WASM (`valuations.instruments.convertibleSpotLadder`). `metricMetadata` now declares the units of `parity` (dimensionless), `conversion_premium` (decimal), `conversion_value` and `bond_floor` (currency), which were `unknown`. No value moves.

`@finstack/convertible-card` draws it: price, conversion value, floor and regime by share price, parity and premium, one row per share price and metrics by id. A priced convertible in the pricing workbench adds it as an Analytics tab, priced on opening; the ladder and all 17 metric ids take milliseconds. Share prices come from the user or Rust's default ladder.

### UI: bond and loan analytics in the pricing workbench

A priced bond now adds an Analytics result tab with `bond-credit-card`, `yield-workout-ladder` and `quote-converter`, each opening on its own over the completed request; a priced term loan adds one with `loan-yield-card`. No value moved.

### UI: bond credit card, shared metric table, loan default risk

`bond-credit-card` shows a bond's spreads, option value, durations, default risk, carry and breakeven by metric id, each id with its own native error, and offers the schema's form for `theta_period` and `breakeven_config`. `metric-values-table` is the table it shares with `loan-yield-card`; `applicableLoanYieldMetrics` and `LoanYieldRow` give way to its `applicableMetrics` and `MetricValueRow`. `loan-yield-card` adds `recovery01` and `jump_to_default`. No value moved.

### Bonds and loans: recovery01 and jump to default on a hazard curve

`recovery01` and `jump_to_default` are now registered on `Bond`, `TermLoan` and `RevolvingCredit`, in currency, for the whole notional, from the holder's perspective.

`recovery01` is the PV change per +1% (0.01 absolute) recovery move, by central difference of the full PV. It bumps the recovery each pricer reads: the hazard curve's on a bond or term loan, the facility's `recovery_rate` on a revolver. As for a CDS, a curve carrying its calibration recipe is re-bootstrapped to the same par spreads at the bumped recovery, and a hand-built curve keeps its hazard rates. The two differ greatly: at 250bp flat spreads and 40% recovery, a 5% five-year bond on 1mm has 8.03 recalibrated against 1,767.89 frozen, and a premium bond's recalibrated figure is negative. A quoted bond or loan is held at its calibrated spread, as CS01 is.

`jump_to_default` is value on immediate default less current value, and it follows the fractional recovery of par each pricer uses, without accrued interest: `R` times outstanding principal on a bond, `R` times funded balance on a term loan, and `R × drawn + (R − 1) × (leq × undrawn + LC draw)` on a revolver. Notional must be positive on these instruments, so a short is a position with negative quantity.

A bond, term loan or revolver not priced on a hazard curve gets a `Validation` error for either id, never a zero: no `credit_curve_id`, a bond under a model other than `hazard_rate` or `rates_credit`, a term loan under a model other than `tree`, or a revolver with stochastic draws, which takes survival from its simulated spread path. `metricsForInstrument` reads the type, so it lists both for every such instrument; nothing requests them by default. No existing value moves.

### Bonds: G-spread over a named government curve

The new `g_spread` metric is the bond's yield minus the par yield of a named government discount curve, in decimals. It uses the same yield and horizon as `i_spread`, through one shared helper: the worst exit on a bond with exercise rights and a quoted clean price, maturity otherwise. Over the bond's own discount curve it equals `i_spread` bit for bit. This is a working convention, pending the desk's decision on the benchmark: it interpolates the named curve at the horizon and selects no on-the-run bond.

Name the curve in `instrument_pricing_overrides.model_config.government_curve_id`, where `asw_forward_curve_id` already lives. No curve named is a `Validation` error (Python `ValueError`, WASM `kind: "validation"`), never a fallback to the discount curve, and a named curve missing from the market is the usual not-found error. `metricsForInstrument` lists `g_spread` for every bond because it reads the type, so a bond with no curve gets that error only when it asks for the id. Nothing requests it by default: `compute_quotes` and every default metric set are unchanged.

A named curve joins the bond's `market_dependencies` as a discount curve. No price or parallel DV01 moves, with the curve in the market or not, and `bucketed_dv01` gains zero rows for that curve.

### Bonds: an asset-swap quote no longer prices a bond with exercise rights

**Behaviour change.** A bond with exercise rights and `quoted_asw_market`, or `BondQuoteInput::AswMarket` on one, is now an error at every spread, as a Z-spread quote already is. It used to return a price: the inverse runs the swap to maturity, while the `asw_market` metric runs it to the worst exit, which depends on the price, so the returned `asw_market` could differ from the quote (a callable at 108 came back at 119.1566). This reaches every route through the quote path: `value`, `price_instrument`, the price metrics, OAS from a quote and `compute_quotes`. The `asw_market` metric and output field on such a bond are unchanged, and so is the quote on a plain fixed-coupon bond. Quote a clean price, or use OAS.

### Bonds: the whole quote set from any one quote

`compute_quotes` is now bound in Python (`compute_quotes`, `BondQuoteSet`) and WASM (`valuations.instruments.computeQuotes`), with JSON Schemas for `BondQuoteSet` and `BondQuoteInput`. Each yield and spread equals the metric of the same name at that clean price. A field is null when it does not apply to the bond or its metric has no finite value, never infinity or NaN; no finite value moves. A quote kind the bond does not have is an error: `discount_margin` with no floating coupons, `asw_market` or `japanese_simple_yield` on a floating coupon, `z_spread` or `asw_market` with exercise rights. One failed metric fails the whole set. `BondQuoteInput` adds `deny_unknown_fields`; it already rejected an unknown or second key.

`bond_quote_set_field_units(currency)` (Python) and `valuations.instruments.bondQuoteSetFieldUnits` (WASM) state the unit of every field in struct order, including `clean_price_pct` as `% of par`.

The `quote-converter` registry component and `useComputeQuotes` and `useBondQuoteSetFieldUnits` hooks draw it: enter any one quote, read the whole set Rust returns.

### Bonds: yield and spread to every exit

`bond_workout_ladder(bond, market, as_of)` answers which call date is the worst workout and what the yield to each is. It returns one row per admissible call, put and maturity exit: date, kind, redemption price and amount, whether the redemption is make-whole, Street yield, I-spread and whether the row is the worst. The ladder and the `ytw` metric run one walk (`solve_workout_exits`), so the worst row holds that metric's yield and the `i_spread` metric's spread. As that metric does, the spread's par rate runs to the exit date when the bond has an exit other than maturity, and to contractual maturity when it has none. It needs a quoted clean price on a bond with exercise rights and lowers a return floor into calls, as `ytw` does. Yield and spread are null on a path with no finite yield: an exit on the quote date, or same-day cash at or above the price. A worst row can be null, where `ytw` is negative infinity. Z-spread is not reported per exit: no per-exit convention exists.

Bound in Python (`bond_workout_ladder`, `BondWorkoutLadder`, `BondWorkoutRow`) and WASM (`valuations.instruments.bondWorkoutLadder`). No existing value moves.

The `yield-workout-ladder` registry component and `useBondWorkoutLadder` hook draw it, with the row Rust marks as the worst workout.

### Structured credit: a custom waterfall pays a debt class only what a note is owed

Per tranche and date, `interest_flows` plus `principal_flows` now equals `cashflows` with no exception. A note is owed interest at its rate and the return of its balance. A fixed or percentage amount is a fee formula with no contractual basis as either: it cannot be an extra coupon, and treating it as principal would silently retire the notional of what is usually an interest-only strip or a misrouted fee. So these waterfalls are now refused rather than reclassified.

#### Changed

- Breaking: `validate_custom_waterfall` and `validate_tiers` reject a `RecipientType::Tranche` paid by `FixedAmount`, `PercentageOfCollateral`, `PercentageOfSpecialServiced`, `IncentiveFee` or `ReserveReplenishment` (`ValidationError::NonNoteCalculation`), or by an interest, principal or carryover calculation naming another tranche (`ValidationError::TrancheMismatch`). `run_simulation` raises the same error. A fixed periodic paydown has no representation for now: `TranchePrincipal.target_balance` is a single target, not a schedule. Model a class that takes excess cash with `TrancheSeniority::Equity`.
- Breaking: a custom `NetWacCarryover` recipient must be named `net_wac_carryover_<tranche_id>` and the tranche must accrue carryover under `waterfall_rules.afc`.
- A debt class's non-principal cash is booked in full as interest instead of being capped at the claim. Only a coupon rounded above its claim moves: with `RoundingConvention::Ceiling` on a 7,000,000 `CLASS_A`, 2024-04-01 interest gains the 0.0078 that used to reach `cashflows` alone.
- What each rejected case booked as neither interest nor principal over the life of a 10,000,000 CLO with a 2,000,000 `CLASS_B`, in a residual tier ahead of equity: `FixedAmount` 50,000 per period 1,000,000.00 (lifetime cash 3,000,000.00); `PercentageOfCollateral` 1% annualized 446,980.89, 1% per period 1,762,221.97; `PercentageOfSpecialServiced` 10% per period 1,803,433.16; `IncentiveFee` 200,809.17; `ReserveReplenishment` 1,948,016.20; a misnamed `NetWacCarryover` of 10,000 per period 200,000.00. No template waterfall or checked-in fixture used these, so no standard deal's values move.

### Structured credit: residual cash paid to a debt class is a turbo

Moves values for any custom waterfall that pays `PaymentCalculation::ResidualCash` to a debt class. A note is owed its coupon and the return of its balance, so remaining cash directed at one is the turbo feature of ABS and CLO interest waterfalls. An extra coupon has no contractual basis and would not retire the balance the coverage tests read. Model income or subordinated notes that truly take the residual with `TrancheSeniority::Equity`, whose treatment does not change.

#### Changed

- Breaking: residual cash paid to a debt class is now principal. It is booked to `principal_flows` and `total_principal`, retires the balance, is capped at the balance the class has left in the period, and passes the excess to the next recipient or tier. It does not repay deferred interest first. It used to reach `cashflows` only, uncapped, with the balance untouched.
- Values on a 10,000,000 CLO with a 2,000,000 `CLASS_B` that takes the residual: first-period (2024-04-01) `CLASS_B` cash stays 110,769.19, with principal moving from 0 to 110,769.19. Lifetime `CLASS_B` cash moves from 4,004,045.88 to 2,000,000.00 and `total_principal` stays 2,000,000.00. The 2,004,045.88 that was booked as neither interest nor principal now flows on past the class.

### Structured credit: the standard deal families come from Rust

#### Added

- `standard_deal_families()` (`StandardDealFamily`; Python `standard_deal_families`, WASM `valuations.instruments.standardDealFamilies`) returns each family `new_standard` accepts with its registry profile id and an example `StandardDealParams` whose `id`, tranche ids and `pool.deal_type` name the family.

#### Changed

- Breaking: `StructuredCredit::new_standard` returns a validation error when `pool.deal_type` contradicts the family. It must equal the family, and the `abs` family also takes `auto` and `card` pools, which have no constructor of their own.

### Structured credit: absent values are null, not zero or infinity

Wire changes. Each field below is now nullable; every value that exists is unchanged. An engine's pass, fail and pricing logic does not move.

#### Changed

- `CoverageTestDiagnostic.ratio` and `.cushion` are `null` (Python `None`) when the test has no claim outstanding: the notes it covers are retired (OC) or owe no interest (IC). They were `Infinity` in JavaScript and `None` in Python. `passing` is still `true` there. Python `coverage_tests_dataframe` gives `NaN`.
- `AssetPool::wac`, `weighted_avg_spread_bp`, `weighted_avg_maturity` and `diversity_score` (Python alike, WASM `assetPool*` `number | undefined`), `PoolStats` of the same names, `PoolReport.warf` and `PeriodDiagnostics.wac`, `weighted_avg_spread_bp` and `warf` are `null` when their population is empty, so a floating-only pool has no WAC rather than a WAC of zero. `SimulationDiagnostics::to_table` makes those three columns nullable (Python `to_dataframe`: pandas `Float64`, `<NA>`). The `clo_was`, `clo_warf` and `wam` metrics still return `0.0` there, and `clo_warf` is unchanged for any pool with performing balance; RMBS `enable_stochastic` still passes a WAC of `0.0` to the prepayment model.
- `EquityMetrics.moic` and each `cash_on_cash` share are `null` when nothing is invested. A class that receives nothing on a positive investment keeps a real MOIC of `0.0`.
- `pool_report_field_units`: `wac` is `annual decimal, fixed-rate only`, `num_obligors` and `num_industries` are `named obligors` and `named industries`, as an asset naming neither is not counted.
- `scenario_table_field_units`: `price` is `clean, % of current balance`.

#### Added

- `EquityMetricsFieldUnits.purchase_price` states the unit of the `purchase_price_pct` argument and that omitting it means par.

### Structured credit: units of an equity tranche's metrics

#### Added

- `equity_metrics_field_units(currency)` (`EquityMetricsFieldUnits`; WASM `equityMetricsFieldUnits`) states the unit of each `EquityMetrics` number.

#### Changed

- `EquityMetrics` now documents its decimal, percent and multiple conventions and what a zero investment returns.

#### Fixed

- WASM `StructuredCredit.equityMetrics` documented an omitted `purchasePricePct` as investing at model NAV. It invests at par, as Rust and Python state.

### Structured credit: units of a simulation's charted series

#### Added

- `simulation_field_units(currency)` (`SimulationFieldUnits`; WASM `simulationFieldUnits`) states the unit of each `CoverageTestDiagnostic` number, `diverted_cash`, `credit_enhancement` and `excess_spread`.

#### Changed

- `PeriodDiagnostics.excess_spread` now documents its definition; its value is unchanged.

### Structured credit: scenario table currency and units

#### Added

- `ScenarioTable.currency` names the currency of each cell's `writedown`, as `TrancheMetrics.currency` does for its result.
- `scenario_table_field_units(currency)` (`ScenarioTableFieldUnits`; WASM `scenarioTableFieldUnits`) states the unit of every `ScenarioCell` field, split into swept `axes` and reported `measures`.

#### Changed

- `ScenarioTable` JSON now requires `currency`.

### Structured credit: a tranche's cash flows by date in WASM

#### Added

- WASM `valuations.instruments.trancheCashflowsTable(cashflows)` returns Rust `TrancheCashflows::to_table` as a `TableEnvelope`: one row per payment date with cashflow, interest, principal, PIK, deferred and writedown columns. Python already reaches the same table through `TrancheCashflows.to_dataframe`.

### Structured credit: one report for a collateral pool

#### Added

- `pool_report(pool, as_of)` returns a `PoolReport`: `calculate_pool_stats`, Moody's WARF of the performing collateral (the number `clo_warf` and `PeriodDiagnostics.warf` report) and concentration by obligor, industry and rating. Each `PoolConcentrationEntry` holds the key (`null` for assets with no value on the dimension), balance, `balance_pct` in percent points and asset count; tables cover defaulted assets, sum to 100% and are ordered by balance descending, then key ascending. Python `pool_report`, WASM `poolReport`.
- `pool_report_field_units(currency)` (`PoolReportFieldUnits`) states the unit of every report field. Python `pool_report_field_units`, WASM `poolReportFieldUnits`.

#### Changed

- All structured-credit WARF numbers share one function, so `CloWarfCalculator` now skips non-positive-balance rows, as `PeriodDiagnostics.warf` already did. Pools with positive balances are unaffected.

### Structured credit: who got paid each period

#### Added

- `PeriodDiagnostics.waterfall` (`PeriodWaterfall`): each period's tier allocations, `PaymentRecord`s, diversions and carried cash, on the `SimulationDiagnostics.periods` that `StructuredCredit::run_simulation` returns (Python `run_simulation`, WASM `runSimulation`). `PaymentRecord` gains `is_principal`.
- `PeriodDiagnostics.non_waterfall_payments` (`NonWaterfallPayment`, `NonWaterfallSource`): cash paid to a tranche outside the waterfall (routed reserve interest, redemption, terminal releases). Per tranche and date, payment records plus these sum to the tranche's cash flows.
- `PeriodDiagnostics.credit_enhancement` (`TrancheCreditEnhancement`): each tranche's end-of-period share of collateral beneath it, generalising the function the `MinCreditEnhancement` step-down trigger reads.
- `SimulationDiagnostics.first_breaches` (`CoverageTestBreach`; Python `SimulationDiagnostics.first_breaches`): each coverage test's first failing payment date, including tranche triggers that never reach `coverage_tests`.

#### Changed

- A deal with no payment period left that still settles cash now reports one record in `SimulationDiagnostics.periods` holding that settlement.

### UI boundary: rules decided in Rust

`check-worker-compose` (`ui-check`) now flags a worker service that decides from a native result; pure composition takes `// finstack-compose: <reason>`.

#### Changed

- `Performance::lookback_returns` takes `Option<Date>` (Python `ref_date=None`, WASM optional `refDate`) and defaults to the last active date, erroring when none is active; it returns `Result`. `LookbackReturns` gains a required `ref_date` stating the date the windows ended on.
- `generate_tornado_entries` (Python `generate_tornado_entries`, WASM `statements_analytics.generateTornadoEntries`) returns `None` / `null` for a full-grid result instead of an error. A full grid has no tornado by construction, so a host no longer checks the mode first.

#### Removed

- The `rate-input` registry item and the UI's `rateColumn`, `rateToWire`, `convertRate`, `formatRate` and `RatePresentation`. Nothing in the registry used them, and they scaled rate units in JavaScript. Display percent is now `formatPercent`.

#### Added

- `models::credit::merton_report` (Python `models.credit.merton_report`, WASM `models.credit.mertonReport`) returns the full Merton report in one call. It owns the KMV default point of an equity source and the default master scale, so hosts cannot differ on them. A non-positive horizon now fails the whole report, and a measure rejected for the model is null with its message in `errors` rather than a structured error.
- `MarketQuoteOverrides::price_driver_fields()` (Python and WASM `valuations.instruments.price_driver_fields` / `priceDriverFields`) publishes the mutually exclusive price-driving quote fields. `price_driver_count` reads the same definition, and `InstrumentForm` takes the list through `priceDriverFields` (connect `usePriceDriverFields`) instead of holding a copy.
- `pricer::tranche_metric_field_units(currency)` (Python `tranche_metric_field_units`, WASM `valuations.instruments.trancheMetricFieldUnits`) publishes the unit and order of the scalar `TrancheMetrics` fields. `TrancheAnalytics` requires them as `metricUnits` (connect `useTrancheMetricFieldUnits`).
- `MetricMetadata.coordinates`: for each entry of `components`, the `group` and numeric `value` (tenor in years or decimal strike) that order it on its axis. The UI sorts by these instead of parsing labels.
- `PnlAttribution::pct_of_total_by_bucket()` (Python `PnlAttribution.pct_of_total_by_bucket`, WASM `attribution.pnlAttributionPctOfTotalByBucket`) lists every money bucket with its amount and share of total. `PnlAttributionReport` requires it as `buckets` and always shows the share column; `HorizonReturnReport` requires it as `attributionBuckets`. A bucket added to `PnlAttribution` now reaches the report, which previously hid it behind two JavaScript field lists.
- `risk::evaluate_risk_budget_from_decomposition(decomposition, targets, threshold)` (Python `evaluate_risk_budget_from_decomposition`, WASM `models.factor.risk.evaluateRiskBudgetFromDecomposition`) evaluates a budget against a decomposition in one call, joining component VaR and portfolio VaR in Rust. `usePositionRisk` calls it.
- `ApplicationReport::effective_as_of(as_of)` (Python `ApplicationReport.effective_as_of`, WASM `scenarios.applicationReportEffectiveAsOf`) returns the valuation date after a scenario: the time roll's new date, else `as_of`.
- `InstrumentCashflowEnvelope::calendar_year_ladder_by_currency()` (Python `InstrumentCashflowEnvelope.calendar_year_ladder_by_currency`, WASM `valuations.instruments.calendarYearLadderByCurrency`) returns one `CurrencyLadder` per row currency over `native_pv`, so a host no longer partitions the flows or picks the PV column. `useCashflowLadder` calls it.
- `pricer::tranche_solve_field_units()` (Python `tranche_solve_field_units`, WASM `valuations.instruments.trancheSolveFieldUnits`) publishes the units of the break-even CDR, discount margin and `OasResult` fields. `TrancheAnalytics` requires them as `solveUnits` (connect `useTrancheSolveFieldUnits`); `num_paths` now reads "paths".
- `statements::types::ForecastParameters`, published as the `forecast_parameters` schema, states each forecast method's parameters in the wire shape of `ForecastSpec`. The workbench forecast editor renders it with `SchemaForm` instead of a per-method field table, so its field labels are now the Rust parameter names and optional fields appear behind Add.

### UI: schema forms hold errors until an edit or a submit

#### Changed

- `SchemaForm` holds a field's validation issue until an edit touches its path or submit is attempted, and the first attempt shows them all. Choosing a union branch, adding an optional term or adding an array row no longer counts as an edit, so these actions no longer mark the field dirty. A form with `actions={false}` has no submit to attempt and shows its issues as before.

### Statements: rollup methods declared per line

#### Changed

- `StatementResult` has a required `periods` field (Python `StatementResult.periods`): the model's periods for an evaluated result and the target periods for a rollup, each actual only when every source period is. Empty only for hand-built results. JSON without it no longer deserializes.

#### Added

- `NodeSpec.rollup` (Rust `NodeSpec::with_rollup`, Python `NodeSpec(..., rollup=...)`, WASM through the JSON model): each statement line declares its rollup method on the node. `StatementResult::rollup` resolves a line as its entry in `RollupSpec::methods`, then its declaration, then `sum`, so an empty spec gives every host the same numbers. A line resolving to `formula` with no formula is an error naming the node. `RollupMethod` moved from `statements::evaluator` to `statements::types`.
- `ModelBuilder::rollup(node_id, method)` and `MixedNodeBuilder::rollup(method)` (Python `rollup`, WASM `rollup`) declare a line's method from the public builder, so no line has to drop to `NodeSpec` and `insert_node`.
- `RollupSpec::default_for(&model, frequency)` (Python `RollupSpec.default_for`, WASM `statements.rollupSpecDefaultFor`) lists the declared methods so a host can seed its choices. The tag rule it briefly carried is gone: `balance_sheet`, `ratio` and `metric` tags no longer choose a method anywhere, so a model that relied on them sums until its lines declare `rollup`. The statements workbench no longer decides in JavaScript, and it now sends an explicit `sum` so choosing Sum overrides a line's declaration.

### Analytics: one path per capability

#### Changed

- Python and WASM: an unknown `benchmark_ticker` in the `Performance` constructors (Python `Performance()`, `from_arrays`, `from_returns`, `from_returns_arrays`; WASM `new Performance`, `fromReturns`) and in `reset_bench_ticker` / `resetBenchTicker` raises `KeyError` (WASM kind `not_found`), not `AnalyticsError` (kind `validation`). A failed `reset_bench_ticker` leaves the benchmark unchanged.
- `gain_to_pain` and the `PeriodStats` payoff and profit ratios return `NaN` when a sum or a compounded bucket growth overflows `f64`; they previously returned `+inf` or `0.0`.
- The `multi_factor_greeks` tracing span and events carry the target `finstack_quant_analytics::multi_factor` instead of `finstack_quant_analytics::benchmark`.
- Python `Performance.lookback_returns` and `period_stats`, and WASM `lookbackReturns` and `periodStats`, take the core `FiscalConfig` handle (`fiscal_config` / `fiscalConfig`) instead of the `fiscal_year_start_month` / `fiscal_year_start_day` integers. An omitted config is Rust `None`: a calendar-year FYTD window, and Gregorian periods with ISO weeks for `period_stats`. An out-of-range fiscal start is now a `ValueError` from the `FiscalConfig` constructor, not an `AnalyticsError`.

#### Removed

- Core `FiscalConfig::from_parts(start_month, start_day)`, which filled an omitted half of the fiscal start. The bindings take the `FiscalConfig` handle now; build one with `FiscalConfig::new` or use `FiscalConfig::default()` (calendar year).

### Valuations: one path per capability

#### Changed

- Structured credit has one per-tranche API: `tranche_metrics(deal, tranche_id, market, as_of, market_price_pct, metrics)` (was `calculate_tranche_metrics`, and absorbs `value_tranche_with_metrics` / `TrancheValuation`). `TrancheMetrics` renames `price_pct` → `clean_price_pct`, `modified_duration` → `effective_duration`, `convexity` → `effective_convexity`, and gains `dirty_price_pct`, `accrued`, `ytm` and requested registry `metrics`. The other tranche analytics are `tranche_oas`, `tranche_discount_margin`, `tranche_breakeven_cdr` and `tranche_scenario_table` in Rust and Python (was `structured_credit_tranche_*`) and `trancheMetrics` etc. in WASM (was `structuredCreditTranche*`).
- WASM: the `valuations.fx` namespace is folded into `valuations.instruments`, and FX TypeScript types take their Rust names (`FxForward`, `FxOption`, ...).
- `X::builder()` is the one construction entry point: `#[derive(FinancialBuilder)]` no longer emits `XBuilder::new()`, and the `*FutureOption::new`, `EquityOption::new`, `FxSpot::new` and `TrancheBuilder::new` constructors are gone. `PrivateMarketsFund::new` returns `Result`; `CdsTranche::new` runs the full validation.
- `EquityOptionParams::new` defaults settlement to Cash (was Physical), matching the instrument's serde and builder default.
- `CashflowSpec::fixed_rate`, `floating_bp` and `floating_with_reset_lag` are gone; use `fixed` and `floating(.., reset_lag_days: Option<i32>)`.
- Commodity forwards pay on maturity plus the convention's settlement days on a registered exchange calendar (NYMEX, COMEX and CBOT on `cme`, ICE Brent and LME on `gblo`, power on the new `nerc` calendar), so PV and cashflows discount to that payment date.
- MBS Monte Carlo OAS takes its path count and seed from `model_config` (`mc_paths`, `mc_seed`) like the other MC pricers.
- Ten instruments (FX spot, forward, swap, NDF, quanto option; equity TRS, DCF, cliquet, autocallable; real-estate asset) deserialize with `#[serde(remote = "Self")]` and `validate_on_deserialize!` instead of hand-written `*Unchecked` mirrors; their published schema property docs now come from the real structs.
- `BrentSolver::solve_fallible` propagates objective errors; YTM, Z-spread and DM now report the pricer's error instead of the solver's.
- Missing-metric errors use the metric's wire id (`metric:z_spread`).
- Python docstrings come from the `.pyi` stubs for the converted modules (`valuations.market`, `valuations.composite`, the typed instruments and every domain's `schema`); `finstack_quant.valuations.{instruments, composite, market}` are the compiled modules (the `.py` re-export shims are gone).
- Stricter edge cases where duplicate code paths disagreed: an FX variance swap with no return interval is an error, commodity Black-76 rejects degenerate inputs, a configured but missing FI-TRS index yield is NotFound (Python `KeyError`), out-of-range SMM/MDR conversions error, `FxOption::validate` rejects a non-finite or negative strike, and a quanto whose `fx_spot_id` scalar is missing fails instead of using the FX matrix.

#### Removed

- Wire fields and variants that changed no number: `ClawbackSpec.settle_on` and `ClawbackSettle` (clawback settles at fund end), `CdsOption.exercise_style` and `premium_settlement_date`, `CommoditySpotOption.{convention, premium_settlement_days}`, `VolatilityIndexFuture.discount_curve_id`, `BermudanSchedule.notice_days`, and `YieldCompounding::TreasuryActual`. JSON carrying these fields is rejected.
- Always-zero metrics: repo `collateral_haircut01` / `collateral_price01` and CDS tranche `tail_dependence`.
- Setters that shadowed builders or public fields (swaption, cap/floor, IR future, Bermudan schedule, basis swap, structured credit, pool asset, tranche, FX spot, CDS index and option, `ValuationResult::with_covenant(s)`).
- Dead public API with no callers, including `extract_risk_factors`, `ListedDeliveryObligation`, `CdsPricerConfig`, the CDS index `*_detailed` results, `StructuredCredit::{generate_cashflows, apply_deal_defaults}`, `BasketCalculator`, `CompositeInstrument::new`, `instrument_cashflows_json` and `format_bucket_label_cow` (`format_bucket_label` returns `Cow`).
- Python `ValuationResult.{price_decimal, metric_keys, metric_count}`, the composite JSON twins `execution_trades_json` / `trades_json` / `instrument_json` (`CompositeHistoryResult.row` is the typed twin of `row_json`), and WASM `weightingMethodFixedQuantity`, `weightingMethodNotionalWeighted`, `rebalanceRuleManual` and the FX bare-spec constructor.

### Statements: one path per capability

#### Changed

- Gaussian Monte Carlo forecasts retain their sampled shocks directly, preserving correlation when small increments round away at large levels. Zero-volatility nodes remain ineligible as stochastic correlation peers.

- Capital-structure residual scaling warnings use `ResidualScaleMismatch { applied_ratio }` (`residual_scale_mismatch` in JSON), replacing the inaccurate `ScaleClamped` warning and its obsolete fields. Cashflow scaling is unchanged.

- Capital-structure runtime state, dated principal claims, and waterfall execution are crate-private. The evaluator owns their lifecycle; the unused cumulative-principal accumulator and redundant waterfall period argument are removed.

- `PikToggleSpec.target_instrument_ids` is a required list across Rust, Python, and JSON. Missing and null target lists are rejected; validation requires at least one borrowing-debt target.

- Waterfall failures leave capital-structure runtime state unchanged. Successful allocations commit balances, PIK state, and carried claims together.

- Accounting identity checks report period-level error findings when sums, differences, tolerances, or materiality calculations overflow. They publish no non-finite comparison payload for that period and continue checking later periods.

- `CapitalStructureCashflows::get_component(component, instrument_or_total, period_id)` replaces the 16 per-component and total getters in Rust and Python; WASM exposes `capitalStructureCashflowsGetComponent`. Instrument lookups retain native currency and `total` reads the reporting-currency aggregate.

- `Evaluator::evaluate_prepared` evaluates a model's capital structure exactly as `Evaluator::evaluate` does (an empty market as of the first period's start), so prepared evaluation now yields `cs_cashflows` and `cs.*` values. Before, it skipped the capital structure.
- `ModelBuilder::with_builtin_metrics` no longer overwrites a node the model already defines with the same id.
- `ModelBuilder` has one mutable configuration API: setters return `&mut Self` or `Result<&mut Self>` and rejected setters preserve the accumulated model. `build` and `mixed` consume the builder; `MixedNodeBuilder::build` returns its parent builder. The consuming setter and `try_*` twins are removed. `ModelBuilder::new` is the construction entry point, and `build` rejects a model without a timeline.
- The statements-analytics entry points take the names the bindings already used: `run_sensitivity`, `run_variance`, `variance_bridge`, `explain_formula`, `render_check_report_text`/`_html`, `evaluate_dcf` (was `evaluate_dcf_with_market`). The engine structs behind them are private.
- `dcf_sensitivity` takes no market argument (Rust, Python and WASM).
- Each statements and statements-analytics item has one public path: internal modules (`checks::{suite,traits,types,helpers}`, `dsl::{ast,compiler,parser}`, `capital_structure::waterfall`, `registry::dynamic`, `evaluator::formula`, `adjustments::{engine,types}`, `forecast`, and the analysis submodules) are private, with their public items re-exported at the parent. `NormalizationEngine` is replaced by `adjustments::normalize` and `adjustments::merge_into_results`.

#### Removed

- Wire fields, variants and types that had no effect: `DcfOptions.discount_curve_id`, `VarianceRow.driver_contribution`, `EvalWarning::NaNPropagated`, `EclRequest`, `EclStageRequest`, `ExposureEclResult` and the `ecl::portfolio` module, and the toggled-PIK interest path in `calculate_period_flows`.
- Forwarders and dead analysis API: `EclEngine`, the `config()` accessors, the comps `extract_metric`/`subject_metric` and `Multiple::is_*`/`label`, the analysis `forecast_covenant` forwarder, `VarianceReport::to_table`/`is_empty`, and `statements::formula`.
- Python `normalize_json` and WASM `normalizeJson`; Python `explain_formula_text`, `pl_summary_report_text` and `validate_scorecard_config`.
- The 20 WASM constructor functions that only built a plain object (`forecastSpec*`, `adjustment*`, `terminalValueSpec*`, `sensitivityConfigAddParameter`). Pass the object literal instead, e.g. `{ method: 'growth_pct', params: { rate: 0.05 } }`.

### Cashflows: one path per capability

#### Changed

- `CFKind`, `CashFlow` and `CashFlowAccrual` bind where Rust defines them: Python `finstack_quant.core.cashflow` (was `finstack_quant.cashflows.primitives`) and WASM `core.cfKindParse` / `core.cfKindIs*` / `core.cashFlowGetBalanceDate` / `core.cashFlowValidate` (were under `cashflows`).
- `CashFlowSchedule::builder()` is the only builder entry point. `schedule_from_dated_flows` / `schedule_from_classified_flows` validate their rows and return `Result`. Python and WASM schedules built from host parts are validated.
- `pv_by_period` drops its unused day-count argument (Rust, Python, WASM); the PV path rejects flows in a currency other than the requested base.
- `weighted_average_life_from_principal` moves from `builder` to `aggregation`. `CFKind::is_principal_repayment()` names the repayment kind set used by schedule WAL, principal-repaid aggregation and statements.
- Accrual items and `PeriodAggregation` are reached through `accrual::` and `aggregation::` only; `PrincipalEventSpec` is exported from `builder`, and `MarginStepSpec` / `PaymentStepSpec` from the crate root. `WEEKENDS_ONLY_ID` lives in `core::dates`.
- `FloatingRateSpec::DEFAULT_GEARING`, `DEFAULT_GEARING_INCLUDES_SPREAD` and `DEFAULT_RESET_LAG_DAYS` are public; the Python constructor uses them. Fee specs default their business-day convention through serde like `ScheduleParams`.
- Term-loan call exercise builds its outstanding path and accrual indexes once per schedule, not on every tree step.

#### Removed

- `finstack_quant_cashflows::primitives` (use `finstack_quant_core::cashflow`).
- `CashFlowSchedule::npv` (use `finstack_quant_core::cashflow::npv` over `dated_flows()`), the `IntoIterator` impls on `CashFlowSchedule` (use `into_flows()` / `get_flows()`), `PeriodAggregation::into_inner`, and the six `CashFlowBuilder::get_*` getters (the builder implements `Display`).
- `builder::emit_inflation_coupons` (inlined at the inflation-linked bond) and `AssetPool::wal_from_cashflows`.
- `clamped_smm_to_cpr`, `clamped_mdr_to_cdr`, `psa_to_cpr` and `cdr_2pct` (Rust, Python, WASM), and the inert SDA registry record (`SDA_PEAK_CDR` replaces it).
- Host `from_parts` constructors, `DateContext`, `valuations::__private`, the valuations re-exports of cashflows types, and the cashflows crate-root re-exports of accrual items.

### Statements: scenario forecast overrides, rollups, capital-structure scenarios

#### Added

- `ScenarioDefinition.forecast_overrides` replaces a node's whole `ForecastSpec` in a scenario: the method and every parameter (growth rates, curves, seeds, volatilities, external series, bounds). Child scenarios replace a parent's spec for the same node, and explicit values still win (`Value > Forecast > Formula`). Python `ScenarioSet` accepts a `ForecastSpec` as an override value.
- `StatementResult::rollup(&model, &RollupSpec)` rolls results up to a coarser frequency (monthly into quarterly, semi-annual or annual; quarterly into semi-annual or annual; semi-annual into annual; fiscal periods keep their `FY` labels). Nodes sum by default; `period_end` nodes take the last source value and `formula` nodes re-evaluate their formula on the rolled-up values. Every target period must be complete. Python `StatementResult.rollup` / `RollupSpec`; WASM `statements.statementResultRollup`.
- `ScenarioSet::evaluate_all_with_market(&model, &market, as_of)` evaluates every scenario like `Evaluator::evaluate_with_market`. Python `evaluate_scenario_set_with_market`; WASM `statements_analytics.evaluateScenarioSetWithMarket`.

#### Changed

- `Evaluator::evaluate` (and `ScenarioSet::evaluate_all`, Python `evaluate_scenario_set`, WASM `evaluateScenarioSet`) evaluates a model's capital structure from an empty market as of the first period's start, so fixed-rate debt yields `cs_cashflows` and `cs.*` values. Instruments that need market data (floating coupons, swaps, caps, swaptions) now fail with a missing-data error naming the instrument (Python `KeyError`) instead of being skipped silently; evaluate those with `evaluate_with_market`. `evaluate_prepared` still skips the capital structure.
- Formula checks report a period whose same-period input is missing or non-finite, or whose formula consumes a non-finite value (a `lag` before the first period), as a `warning` "insufficient data" finding that does not fail the check. Such periods previously failed with the configured severity, or aborted the suite when a value was missing. A division by zero over finite inputs still fails.

### Valuations: verified pricing and risk fixes

#### Fixed

- Rates: the default Black cap/floor pricer and the Hull-White cap/floor pricer now subtract an unsettled premium, as `CapFloor::base_value` does; registry prices were gross before. CMS option `Vanna` no longer errors on seasoned options and uses the ATM vol in the convexity adjustment, as the pricer does. `XccySwap::validate` rejects same-currency legs for every swap, not only MtM-resetting ones. Bermudan `ExpectedExerciseTime` is published by the pricer: under the LMM pricer it now errors instead of silently building a Hull-White tree.
- Fixed income: `Bond::metrics_equivalent` converts PIK coupons to cash for step-up, floating and amortizing step-up bonds (it handled only fixed), so their spread and yield metrics are on a cash basis. Convertible `parity`, `conversion_premium` and the conversion-value metric reject a spot price quoted in a currency other than the bond's.
- Credit: the `CdsIndex`, `CdsOption` and `CdsTranche` registry pricers report a missing curve as missing market data (Python `KeyError`) rather than a model failure.
- FX: `FxBarrierOption` and `QuantoOption` vega, vanna and volga bump the implied-volatility override when one is set (they returned 0 or failed). `FxBarrierOption` delta, gamma and vanna work when spot comes from the FX matrix (no `fx_spot_id`). `QuantoOption` vega is a central difference (was one-sided).
- Exotics: a missing curve on `AsianOption`, `BarrierOption`, `LookbackOption` and `QuantoOption` raises `KeyError` instead of `ValueError`. Lookback and expired-barrier spot reads reject a spot price in another currency. Lookback and range-accrual `Rho` reprice with the requested model (a non-default model previously mixed with the default) and are central discount-curve bumps.

### Structured credit: scenario-shock consistency and dead-code removal

#### Fixed

- Pricing a `StructuredCredit` through the registry with `ModelKey::StructuredCreditStochastic` and a `scenario_price_shock_decimal` now shocks the `ValuationDetails::StructuredCreditStochastic` payload (npv, clean/dirty price, standard error, confidence interval, tranche PVs) and the `pv_std_error` / `tranche_npv::<id>` measures once, as `price_stochastic_with_mode` does. Before, only the headline value was shocked.

#### Changed

- `calculate_tranche_metrics` takes a floating tranche's discount margin from the z-spread it already solved instead of re-running the deal waterfall; the ±5000 bp bound still applies. `Severity01` is computed as `-Recovery01` from the same two repricings (bit-identical to before), so requesting both reprices twice instead of four times.

#### Removed

- Unused public structured-credit items: `cmo::tranches::pac_support::allocate_pac_support`, `structured_credit::PaymentMode`, and the constants `MONTHS_PER_YEAR`, `QUARTERLY_PERIODS_PER_YEAR`, `PERCENTAGE_MULTIPLIER`, `MIN_PREPAYMENT_RATE` and the function `pool_balance_cleanup_threshold()` (the simulation reads the threshold from the assumptions registry). `BASIS_POINTS_DIVISOR` stays.

### Valuations metric registration cleanup

#### Changed

- `register_metrics!` takes an optional `custom: [("id", calculator), ...]` list registered under `MetricId::custom`, after the `metrics` list. Metric ids, applicability and values are unchanged.
- `finstack_quant_valuations_rates::{cms_common, forward_swap_rate, cms_swap::pricer}` are `pub(crate)`; nothing outside the crate used them. `cms_option::pricer` and `cms_option::replication_pricer` stay public.

#### Removed

- `FxSpot::new_t1` (use `FxSpot::new(..).with_settlement_days(1)`), `Ndf::effective_fixing_source` and `Ndf::with_quote_convention` (use the builder's `quote_convention`).
- `InstrumentPricingOverrides::with_bk_sigma` and `with_bk_mean_reversion`; set `model_config.bk_sigma` / `model_config.bk_mean_reversion` directly.

### Statements: formula grammar and runnable-model validation follow the accepted contracts

Formulas (STATEMENTS-CONTRACTS-004): `cumsum`, `cumprod`, `cummin`,
`cummax`, `std`, `var` and `median` take exactly one argument; a negative
literal offset to `lag`, `shift`, `diff`, `pct_change` or `growth_rate` is
rejected at compile time; an identifier needs nonempty dot-separated segments
(`a..b` and `a.` are rejected); canonical rendering writes logical negation as
`!`, so `parse_formula("not a > b")` returns `"!(a > b)"`; and `dsl::compile`
validates a caller-built `StmtExpr` (finite literals, names, arity, 256 terms,
320 levels) before lowering it.

Models (STATEMENTS-CONTRACTS-001): a period plan must be canonical and
contiguous, each period with the exact bounds of its identifier (core gains
`PeriodPlan::validate_canonical`), so gapped plans, business-day grids and
re-bounded periods are rejected. A node ID follows the formula identifier
rules (no hyphens or spaces). A `value` node takes observations only and a
`calculated` node needs a formula; attach a forecast to a `mixed` node.
Observations must be finite: leave a missing cell out instead of storing NaN.
Build and load now reject a formula that references an unknown node (it was a
warning until evaluation) and a model with no node and no capital-structure
instrument, so `with_builtin_metrics` needs every catalogue input defined.
An absent observation on a `value` or `mixed` node evaluates as a missing
cell (NaN) with the new cell source `missing`, where it was an evaluation
error.

### Statements: authoring, imports and point-in-time availability follow the accepted contract

STATEMENTS-CONTRACTS-002. Node ids are unique: `value`, `value_money`,
`value_scalar`, `compute`, `insert_node`, `mixed().build()` and imports reject
a repeated id, a repeated period within one call, and a repeated availability
date, where the last definition used to win. Change an existing node with the
new `replace_node` / `remove_node` builder edits or the atomic
`FinancialModelSpec::apply_edits` (Python `FinancialModelSpec.apply_edits`,
WASM `statements.applyModelEdits`), which takes a batch of named `ModelEdit`s
and commits all or none. `where_clause(node_id, clause)` names its target and
validates the predicate; it no longer applies to the last touched node.

Imports need a timeline: Rust `ModelBuilder::import_observations(columns,
rows)` replaces `resolve_period_columns`, and Python `from_dataframe` no longer
infers periods or takes `actuals_until`. Period labels are unique, may be in
any order, and must belong to the timeline; a missing cell stays missing and a
zero stays a known zero.

Point-in-time evaluation is explicit: `evaluate_with_market(model, market,
as_of, cutoff)` takes `ObservationCutoff::FullSnapshot` or `Through(date)`
(Python and WASM: `"full_snapshot"` or a date as the fourth argument). `as_of`
dates pricing only. A cutoff date needs the model's new `source_snapshot`
identity and an availability date on every observation in an actual period;
the default of "visible at the period's end" is removed. Statements analytics
entry points (`evaluate_dcf`, the orchestrator, scenario sets) read the full
snapshot.

### Statements: every unit resolves before a model runs

STATEMENTS-CONTRACTS-005. A model builds only when every node and formula
resolves to `Scalar` or `Monetary(currency)`. A node with no observations and
no declared `value_type` is rejected (declare it, for example with
`value_scalar(id, [])`); so is a formula whose units nothing anchors. A
declared unit must agree with inference. A corkscrew such as
`lag(balance, 1) + flow` resolves from its flow. The variance of a monetary
series (`var`, `rolling_var`, `ewm_var`) is rejected, where it used to leave
the node's units unknown; use the standard deviation or a same-currency ratio.
A formula that reads a `cs` reference with no resolved currency is the one
case that may still carry unresolved units.

### Statements: cells have distinct states; a masked cell is not zero

STATEMENTS-EVALUATION-001. The evaluator tracks each cell as a value, a
missing observation, a masked cell or an unavailable cell with a reason
(`UnavailableReason`). A false `where` mask now leaves the cell without a
value (status `masked`) where it was `0.0`,
so downstream formulas no longer read a masked cell as zero. A mask that has
no value leaves the cell unavailable. An observation hidden by the observation
cutoff with no forecast or formula is unavailable instead of an evaluation
error. A monetary formula or forecast result that cannot be represented as
`Money` is an evaluation error naming the node and period; it was a warning.

`lag(x, n)` / `shift(x, n)` drop the same-period dependency on `x` only for a
positive integer literal `n`; a zero, non-integer or dynamic offset keeps it,
which can surface a same-period cycle that was previously hidden.

`evaluate_prepared` reuses a plan only for payload changes in existing
observation slots. Adding or removing an observation slot, or changing an
availability date, mask, forecast, formula, timeline or the capital structure,
needs a new `prepare`. Sensitivity and goal seek re-prepare when a shock adds
a slot.

### Statements: results are typed cells; `nodes`, `monetary_nodes` and `node_sources` are gone

STATEMENTS-EVALUATION-013. `StatementResult.cells` is the one result: for each
node and each plan period a `ResultCell` with a `status` (`value`, `missing`,
`masked`, `unavailable`), an exact `Money` amount or finite scalar `value`, a
`reason` for an unavailable cell and the `source` that supplied it. The float
map `nodes`, the parallel `monetary_nodes` and `node_sources` are removed, and
the result JSON changes with them. A result can no longer hold `NaN` or an
infinity. `get` returns a value only for a value cell; `get_cell` returns the
typed cell (WASM `statementResultGetCell`). `get_or` substitutes its fallback
only for a missing observation at a known node and period: an unknown node or
period, a masked cell and an unavailable cell are errors.

The long table has one row per node and plan period with columns `node_id`,
`period_id`, `status`, `value`, `amount` (exact decimal text), `currency`,
`value_type`, `reason` and `source`; `value_money` is gone. The wide table is
a nullable float view over the full period index with each column's units in
its metadata.

`node_to_dated_schedule` (Python `to_dated_schedule`, WASM
`nodeToDatedSchedule`) no longer takes the model and has no default anchor:
pass `"start"` or `"end"`. It returns one dated typed cell per result period,
gaps included. New `node_to_pricing_flows` (`to_pricing_flows`,
`nodeToPricingFlows`) reads a monetary node for pricing from a
`PricingFlowSelection`: expected currency, sign convention
(`inflow_positive` or `outflow_magnitude`, negated once), required periods
and anchor; any required period without an amount is an error.

Each result carries `provenance`: model id and content hash, source snapshot,
observation cutoff, valuation date and run id.

### Statements: formulas are checked; division by zero and invalid math fail the run

STATEMENTS-EVALUATION-002. Formulas evaluate on typed values: a finite scalar,
exact money, a Boolean or an unavailable value.

- **Errors, not `NaN`.** Division or remainder by zero, `ln` or `log10` of a
  non-positive number, `sqrt` of a negative number, an overflow, mixed units
  and mixed currencies are evaluation errors naming the node and period. They
  used to give `NaN` and a warning. Guard a denominator that can be zero, for
  example `if(sign(ebitda) > 0, debt / ebitda, 0)`.
- **Unavailable propagates.** A missing, masked or unavailable operand makes
  the result unavailable. `sum`, `mean`, `min` and `max` no longer skip it,
  and the `non_finite_skipped` warning is removed. `coalesce` returns the
  first available value, including zero, and does not recover an error.
  `is_missing` is true only for an unavailable value.
- **Three-valued logic.** `false and x` is false and `true or x` is true
  even when `x` is unavailable, in either operand order; otherwise an
  unavailable operand makes the result unavailable.
- **Boolean controls.** `if`, `and`, `or`, `not` and `where` need a Boolean:
  a comparison, a logical operator or `is_missing`. A number is rejected when
  the model is built (`if(flag, a, b)` becomes `if(flag == 1, a, b)`; a
  `where` clause of `0` becomes `1 == 0`). A Boolean is not a number:
  `(a > b) * x` becomes `if(a > b, x, 0)`. A node whose formula is Boolean
  is published as `1` or `0`.
- **Exact comparisons.** `==` and `!=` compare the represented values; the
  `1e-10` tolerance is gone. Same-currency money compares, adds, subtracts,
  sums and averages as exact decimals.
- **Rounding and clamp.** `round` of money needs its decimal places stated;
  `floor`, `ceil` and one-argument `round` of money are rejected when the
  model is built. The digits of `round` and the offset of `shift` must be
  exact integers. `clamp` with a lower bound above its upper bound is an error.
- **Annualization.** `annualize_rate` takes method `0` (simple) or `1`
  (compound); any other value is an error, as is compounding a rate below
  -100%. An annualization factor must be positive and finite.

### Statements: offsets, changes, growth and reporting windows are exact and typed

STATEMENTS-EVALUATION-003. `lag`, `shift`, `diff`, `pct_change` and the
reporting windows read typed history, so money keeps its currency and exact
amount (`lag(cash, 1) + cash` is exact).

- **Offsets** must be exact non-negative integers. A fractional offset and a
  future read (`shift` by a negative amount) are errors; a negative `shift`
  used to give no value. A slot before the first period is unavailable.
- **`pct_change`** is unavailable for an exactly zero base (reason
  `undefined_base`); a base within `1e-10` of zero used to qualify.
- **`growth_rate`** needs an exact positive lookback, a strictly positive
  base and a non-negative ending value; outside that domain it is
  unavailable (`undefined_base`). A zero ending value is -100%. Daily and
  weekly plans measure years as Actual/365 Fixed, where they used
  Actual/Actual.
- **`ytd`, `qtd`, `fiscal_ytd`, `ttm`** sum only complete windows: a window
  with a missing slot is unavailable, where `ytd`, `qtd` and `fiscal_ytd`
  used to sum the values present. `qtd` follows the fiscal quarter on a
  fiscal plan. `fiscal_ytd` is rejected on a fiscal plan; use `ytd`.

### Statements: historical statistics run on calendar slots

STATEMENTS-EVALUATION-004. Rolling, cumulative, full-history and
exponentially weighted functions read the typed value of each calendar slot.

- **Rolling windows** cover the last `window` calendar slots, where they used
  the last `window` observations: a missing period now occupies a slot.
  `min_periods` must be from 1 through the window (`0` is rejected).
- **Cumulative functions** (`cumsum`, `cumprod`, `cummin`, `cummax`) need
  every slot from the model start; a gap makes the result unavailable, where
  it was skipped. Recover a gap explicitly, for example
  `cumsum(coalesce(x, 0))`. A `cumprod` overflow is an error.
- **Full-history statistics** (`std`, `var`, `median`) add a `gapped_sample`
  warning with the sample and slot counts when periods are missing.
- **`rank`** uses exact ties; the `1e-10` tie band is gone.
- **Money** sums and means are exact, and `std`, `quantile`, `median` and
  `ewm_mean` keep the currency.
- **EWM** takes its correction toggle as a Boolean, `0` or `1`; any other
  value is an error.

### Statements: metric registration pins bindings; imports are atomic and reject conflicts

STATEMENTS-CONTRACTS-006.

- **Conflicts are rejected.** `add_metric_from_registry` and
  `with_builtin_metrics` fail when a node with a metric's name (or a
  dependency's) is already defined differently, and import nothing. A
  pre-existing user node used to win silently. To override a metric, import
  it and then call `replace_node`. A node that holds the same unchanged
  template is shared.
- **Bindings are pinned at registration.** An unqualified reference binds in
  the metric's own namespace, an exact qualified reference (`other.metric`)
  in any namespace, and anything else is a model input. A metric registered
  later does not capture an earlier metric's reference.
  `Registry.get_dependencies` now returns the closure across namespaces and
  through historical references.
- **Registration is atomic** for a document and for the whole built-in
  catalogue: `load_builtins` adds nothing if any built-in identifier is
  already registered.
- **Names.** Namespaces follow the formula identifier grammar and `cs` is
  reserved. A metric may share its name with a function (`mean`): references
  are qualified in the parsed formula, not in its text.
- `StoredMetric` gains `qualified_formula`, `dependencies` and
  `requirements`.

### Statements: bundled earnings use reported costs; margins need positive revenue

STATEMENTS-CONTRACTS-007. **The bundled `fin.*` convention flips.** `cogs`
and `opex` are now the reported costs, with their depreciation and
amortization included; they used to exclude D&A. `depreciation` and
`amortization` are the D&A included in those lines.

| Metric | Was | Now |
| --- | --- | --- |
| `fin.operating_income` | `revenue - cogs - opex - depreciation - amortization` | `gross_profit - opex` |
| `fin.ebit` | `ebitda - depreciation - amortization` | `operating_income` |
| `fin.ebitda` | `revenue - cogs - opex` | `ebit + depreciation + amortization` |
| `fin.ebt` | inlined chain | `ebit - interest_expense` |
| `fin.net_income` | inlined chain | `ebt - tax_expense` |

A model that feeds ex-D&A operating expenses must restate `opex` to include
D&A, or define its own metrics in another namespace. `fin.roe`, `fin.roa`,
`fin.roic` and `fin.roce` drop their separate D&A deductions for the same
reason. `fin.ebitda` now needs `depreciation` and `amortization` inputs.

The four margins and two cost ratios are `ratio(numerator, revenue)`: they are
unavailable when revenue is zero or negative (`undefined_base`) or missing,
where a zero revenue used to give no value or fail the run.

New formula function `ratio(numerator, base)`: a decimal ratio over a strictly
positive base in the same units, unavailable for a non-positive base. The
reason `undefined_change` is renamed `undefined_base`.

### Statements: one typed forecast specification; seeds are exact strings on the wire

STATEMENTS-CONTRACTS-003. `ForecastSpec` is now a closed typed enum (a method
with its typed parameters) and replaces the free-form parameter map.
`ForecastParameters` is renamed `ForecastSpec`; `ForecastSpec::parameters()`
and the public `method` / `params` fields are gone (`method()` and `params()`
are accessors, `from_params` and `with_param` build checked specifications).
The schema `forecast_parameters` is renamed `forecast_spec`.

- **Rejected on read.** An unknown, duplicate, `null` or method-inapplicable
  field fails when the document is read or the parameter is set, where it
  used to fail when the forecast ran.
- **No default choices.** `time_series` needs its `method`, `bootstrap` its
  `mode`, and `fade_to_target` its `shape`. The constructors take them:
  `ForecastSpec.time_series(history, "linear")`,
  `ForecastSpec.bootstrap(history, "growth", seed)`,
  `ForecastSpec.fade_to_target(target, "linear")`.
- **Seeds are exact.** In JSON a seed is a canonical decimal string
  (`"seed": "42"`); a JSON number is rejected, including an integer-valued
  float. Rust and Python pass an integer. Only stochastic methods take one.
- **Static checks at build and load**, even when observations cover every
  forecast period: finite numbers, `min` not above `max`, non-negative
  standard deviations, `half_life` only with the exponential fade, smoothing
  factors only with the exponential trend, a moving-average `window`, a
  growth curve with one rate per forecast period, override keys that are
  periods of the model, and a correlation peer that is another stochastic
  forecast node.

### Statements: one forecast suffix per node, an explicit base and recorded clipping

STATEMENTS-EVALUATION-005. A forecast is generated once per node and run over
the model's forecast periods, one step per period.

- **Base.** A method that steps from a level (`growth_pct`, `curve_pct`,
  `fade_to_target`, `normal`, `log_normal`, `mean_reverting`, `bootstrap`)
  takes an optional `start` level. Without it the base is the visible value
  in the period just before the first forecast period. A forecast with
  neither is unavailable with reason `missing_forecast_base`, where it used to
  search further back or fail the run.
- **Explicit values do not restart the forecast.** A value published in a
  forecast period replaces that cell only; later cells follow the same
  recurrence, seasonal phase and draws. A model whose periods are all
  forecast periods needs `start`, since an explicit value in a forecast
  period is no longer a base.
- **Hidden actuals.** An actual hidden by the observation cutoff is
  unavailable (`hidden_by_cutoff`); it is not forecast, and the forecast that
  follows it has no base.
- **Bounds.** `min` / `max` clip the published value only, for every method;
  later steps build on the unclipped level. Each clipped cell adds a
  `forecast_clipped` warning with the raw and published values.

### Statements: signed growth, keyed override carry and exact fade targets

STATEMENTS-EVALUATION-006.

- **Override keys are canonical.** An `override` forecast key must be the
  canonical text of its period (`"2025M02"`, not `"2025m2"`); another
  spelling is rejected when the model is built.
- **Override without a base.** Slots before the first key carry the base
  when there is one and are otherwise unavailable (`missing_forecast_base`);
  the run no longer fails.
- **Fades.** Linear and geometric fades return the exact target in the final
  period. The geometric fade interpolates log magnitudes, so extreme
  endpoints no longer overflow. A geometric fade to zero, a `start` across
  zero from the target and an exponential fade without `half_life` are
  rejected when the model is built.
- **Growth.** Rates below -100% flip the sign of the level, as before; the
  log advisories about negative bases and large rates are removed.

### Statements: stochastic forecasts on versioned random streams

STATEMENTS-EVALUATION-007. **Every seeded forecast value changes**: normal,
log-normal, mean-reverting and bootstrap forecasts, single-run and Monte
Carlo, now draw from a stream derived from a versioned key (the seed, the
node, the evaluation mode, and the run seed and path on a Monte Carlo path).
Re-pin any stored stochastic numbers.

- **Replay.** A single run records `provenance.forecast_streams`: per
  stochastic node the `seed`, `stream_version` and the PCG64 `state` and
  `stream` (decimal strings), enough to replay its draws.
- **Log-normal.** The step is taken in log space with no exponent clipping.
  A level that is not finite and strictly positive is an error (it used to be
  clipped, or underflow to an absorbing zero); a positive subnormal is valid.
  A `start` level must be strictly positive.
- **Bootstrap.** Index draws are unbiased. Growth mode needs strictly positive
  history, `start` and base; fewer than two history levels fail at build.
- **Zero volatility.** A zero `std_dev` is the deterministic transition and
  consumes no draws.
- **Mean reversion.** `reversion_speed` outside `(0, 1]` fails at build.

### Statements: correlated forecasts need Monte Carlo and no longer depend on node order

STATEMENTS-EVALUATION-008.

- **Breaking: ordinary runs.** `Evaluator.evaluate` on a model with a non-zero
  forecast `correlation` now fails with an error naming Monte Carlo; it used
  to ignore the correlation. Use `evaluate_monte_carlo`, or set the
  coefficient to zero.
- **Order.** A correlated node no longer needs its peer declared or evaluated
  first; the helper formula that forced the order (`peer * 0 + ...`) is not
  needed.
- **Build checks.** A non-zero correlation needs a positive `std_dev` on both
  nodes, and peer cycles are rejected.
- **Exact cases.** A zero coefficient is an uncorrelated node (same draws);
  `1` copies and `-1` negates the peer shock exactly.
- New unavailable reason `missing_peer_shock`.

### Statements: trend forecasts check their parameters at build

STATEMENTS-EVALUATION-009.

- **Rejected at build.** An exponential trend without `alpha` and `beta`, a
  smoothing factor of exactly 1 (the range is the open interval), a history
  of fewer than two observations and a moving-average `window` longer than
  the history now fail when the model is built; they used to fail when the
  forecast ran, and a smoothing factor of 1 passed the build.
- **Numerics.** The linear trend is fitted on centered and scaled
  coordinates with compensated sums, so a small slope at a large level is
  kept; moving-average windows of extreme finite levels no longer overflow.
  Ordinary results change only in the last digits.

### Statements: seasonal forecasts state their multiplicative domain

STATEMENTS-EVALUATION-010.

- **Rejected at build.** A seasonal history shorter than two complete
  seasons, and multiplicative `growth` at or below -1, fail when the model is
  built.
- **Multiplicative mode.** The trend must be non-zero with one sign and every
  observation must share it or be exactly zero. A wholly negative series is
  now accepted; an observation against the trend's sign is rejected, where
  it used to produce a negative factor.
- **Edges.** Edge trend slopes are used as computed; slopes below 1e-10 were
  snapped to zero. The log advisory comparing the history with the node's
  last actual is removed.

### Statements: bond and swap construction helpers removed

STATEMENTS-CAPITAL-001. **Breaking.** `ModelBuilder.add_bond`,
`add_bond_with_convention`, `add_swap` and `add_swap_with_conventions`
(`addBond`, `addBondWithConvention`, `addSwap`, `addSwapWithConventions` in
WASM) are removed, with the `BondConventionParams`, `SwapParams` and
`SwapConventions` types and their schemas. Build the instrument through its
own Valuations API and attach it:

```python
bond = Bond.fixed("SENIOR", Money(1_000_000.0, usd), 0.05, issue, maturity, "short_front", "USD-OIS")
builder.add_debt("SENIOR", bond)
swap = InterestRateSwap.from_conventions({...})
builder.add_debt("IRS", swap)
```

`Bond.fixed` uses the same US corporate conventions `add_bond` did.
`InterestRateSwap.from_conventions` takes its conventions from the named
index, which may differ from the fixed semi-annual 30/360 against quarterly
ACT/360 legs `add_swap` used; author the legs explicitly where they matter.

### Statements: capital cash is classified by name and sign; distress cash is reported

STATEMENTS-CAPITAL-002.

- **Distress cash.** Recoveries, accrual paid on default and protection
  payments are reported in `cs_cashflows.distress_cash`: per period, each row
  has its instrument, payment date, kind and the company's signed amount.
  They are outside debt service, totals and the waterfall. They used to be
  dropped with a `cashflow_ignored` warning, which is removed.
- **Rejected instead of ignored or absolute-valued.** A cashflow kind with no
  capital-structure classification (margin, collateral, unknown) fails the
  run. A fee or repayment row that is a company receipt fails instead of
  being counted as a claim. An empty or reversed reporting period fails
  before a schedule is read. A zero opening balance in another currency than
  the schedule's fails.
- **Inflation coupons** are netted with the other coupons; they used to be
  ignored.
- **PIK** is its signed principal effect, so a correction reduces the
  capitalized total.
- **Rust.** `calculate_period_flows` takes a supplied schedule as
  `SuppliedSchedule { schedule, perspective }`
  (`SuppliedSchedule::holder(&schedule)` for the canonical form).

### Statements checks: exact by default, four outcomes, verified results

STATEMENTS-CONTRACTS-014. **Breaking.**

- **Tolerance.** `CheckConfig` defaults are `default_tolerance = 0` and
  `default_relative_tolerance = 0` (they were `0.01` and `1e-9`): an identity
  must hold exactly unless a tolerance is stated. State one where floating
  point noise matters, for example
  `CheckConfig(default_tolerance=0.01, default_relative_tolerance=1e-9)`.
- **Outcomes.** `CheckResult.passed` is replaced by `outcome` (`pass`,
  `fail`, `incomplete`, `unevaluated`) and `evaluated`; `CheckReport` gains
  `outcome`, and its summary `incomplete` and `unevaluated`. A check that
  lacked required data is incomplete, and an empty suite or a check with
  nothing to examine is unevaluated; neither is a pass. Python
  `CheckReport.passed` is true only for the outcome `pass`, and
  `to_dataframe()` has `outcome` and `evaluated` in place of `passed`.
- **Filters.** `min_severity` and `materiality_threshold` hide advisory
  findings only. Error-severity findings, coverage findings (new
  `CheckFinding.coverage`) and comparisons are always reported.
- **Identifiers.** A suite with two checks of one identifier is rejected.
  `CheckSuiteBuilder::build`, `CheckSuite::merge` and the analytics suite
  factories return `Result`; a merge needs equal configurations.
  `TrendCheck` has a required `id`, and the credit suite's non-finite check
  is `credit_inputs_non_finite`.
- **Supplied results.** Results passed to a check run must be an evaluated
  run of the same model; hand-built or stale results are rejected. Rust
  `CheckSuite::run_model` takes the requested observation cutoff.

### Statements checks: accounting identities reconcile every period from explicit openings

STATEMENTS-CONTRACTS-010, -011 and -012. **Breaking.**

- **Openings.** `cash_reconciliation` and `retained_earnings_reconciliation`
  need `opening_balance: {"date", "value"}` dated at the first period start,
  and reconcile every period including the first (they used to start in the
  second). `ThreeStatementMapping` takes `opening_cash` and
  `opening_retained_earnings`; without them the suite leaves that
  roll-forward out.
- **Cash adjustments.** `cash_reconciliation.adjustment_nodes` lists signed
  FX and cash-scope adjustments.
- **Rejected as configuration errors.** An unknown node, an account listed
  twice or in two groups, only one or two of CFO/CFI/CFF, and a negative
  dividend magnitude (select `inflow_positive` for signed dividends). These
  used to warn and skip.
- **Incomplete, not skipped.** A period with a missing input is a coverage
  finding naming the nodes, and the check is `incomplete`.
- **Arithmetic.** Money is summed exactly and scalars with compensated sums;
  references have no floor. A sum that cannot be represented is an error
  instead of a failed finding.

### Statements capital: fee rebates, repayment refunds and funding amounts

- A fee row or repayment row that is a company receipt is reported as a
  receipt: `fee_rebates` and `repayment_refunds` on the cashflow breakdown.
  Such a row used to fail the evaluation (STATEMENTS-CAPITAL-002).
- The breakdown of a borrowing reports `funding_proceeds`, the signed company
  cash of funding settled in the period, apart from `funding_principal`, the
  principal that funding established.
- All four are selectable as `cs.<component>.<scope>` and appear in the
  capital long tables.

### Statements capital: dated sweeps with arrears, PIK notice periods

- `EcfSweepSpec.payment_lag_days` (optional) makes the measured sweep a dated
  obligation due that many days after the close of the measured period. It
  is paid in the period that contains its due date, oldest first, and what
  cannot be paid is carried as arrears. `PrepaymentOutcome` gains `carried`:
  the part of `unpaid` carried forward (zero without a lag, as before)
  (STATEMENTS-CAPITAL-007).
- `PikElectionSpec.notice_days` (default 0): an election takes effect at the
  first coupon period starting at least that many days after the close of
  the period that observed the metric (STATEMENTS-CAPITAL-008).

### Statements capital: contractual PIK toggles count in the forecast exercise

- A PIK toggle recorded on a term loan (`covenants.pik_toggles`) that is in
  force when the forecast begins is PIK in force for the modeled exercise: a
  metric at or above the threshold ends it at the next coupon period, after
  the election's minimum number of coupon periods counted from the toggle's
  date. The exercise used to read only its own elections, so such a loan
  stayed PIK (STATEMENTS-CAPITAL-008).

### Loans: scheduled principal after a posting

- A revolving credit's contractual repayment dated after a principal posting
  repays what is left of the drawn balance; it used to fail the rebuild when
  a sweep had left less than the scheduled repayment.
- A term loan's amortization is capped at the principal outstanding
  including capitalized interest, in exact decimal arithmetic. On a loan
  that capitalizes interest a capped amortization used to stop at the funded
  balance and leave the capitalized interest to maturity.

### Statements capital: exact table amounts, typed host lookup

- `CapitalStructureCashflows::to_table` and `to_totals_table` (Python
  `to_dataframe` / `to_totals_dataframe`) report each amount twice: `value`,
  its float view, and `amount`, the exact amount as decimal text. `amount`
  used to be the float (STATEMENTS-CAPITAL-009).
- Python `CapitalStructureCashflows.get_money(component, scope, period_id)`
  and WASM `capitalStructureCashflowsGetMoney` return a component as exact
  money, the typed twin of `get_component`.

### Statements checks: typed comparisons, coverage in the analytics checks

Breaking, with no compatibility path (STATEMENTS-CONTRACTS-014).

- `CheckComparison` records `actual`, `expected`, `difference` and
  `tolerance` as typed values: a number for a scalar, `{amount, currency}`
  for money. `difference` is a recorded field (the exact residual), not a
  method, and the verdict is judged on the exact values.
  `CheckComparison::from_floats` builds one from float views in one unit.
- `Materiality.relative_pct` is optional: absent when the percentage is
  undefined (a zero reference) or does not apply. It used to be `0.0`.
- Python `CheckReport.to_dataframe()` gains `comparison_currency` and
  `to_comparisons_dataframe()` gains `currency`; their value columns stay
  float views.
- The statements-analytics checks (leverage, coverage, liquidity, FCF sign,
  trend, growth, tax rate, working capital, capex, depreciation, dividends,
  interest) report a configured input without a value as a coverage finding
  and are incomplete; they used to skip the period and could pass. An
  optional component node that is configured but has no value is a gap too
  (it was read as zero). `checks::coverage_finding` is public.

### Statements: checks on rolled-up results, `bigint` seeds

- A check suite runs against a rollup at its target periods:
  `CheckSuite::run_rolled(model, rolled)`, and `run_model` (the entry point
  behind Python and WASM `run_checks`) when the supplied results are a
  rollup. The rollup must come from a verified run of the model
  (STATEMENTS-EVALUATION-014). Checks iterate the period plan of the results
  they are given (`CheckContext::periods`).
- WASM Statements inputs accept a JavaScript `bigint` for a seed in a plain
  object; it is read as the seed's decimal wire text
  (STATEMENTS-CONTRACTS-003).

### Statements: located load diagnostics, typed waterfall inputs, `numeric_series`

Breaking, with no compatibility path.

- A model that decodes but is not runnable now fails on load with a located
  diagnostic: `ContractError::Report` whose code is `statements/<failure>`
  and whose pointer is the offending node, period plan or capital-structure
  field (STATEMENTS-CONTRACTS-016). Python raises `ContractValidationError`
  for it (a `ValueError`; a dependency cycle used to raise `RuntimeError`),
  and WASM throws `ContractValidationError` with kind `validation`.
- The message of every contract report error now ends with its first error
  diagnostic's text: `validation failed with 1 error(s): <message>`.
- Waterfall inputs (the cash pool, prepayment amounts, ECF terms, the PIK
  metric) are evaluated as typed values. Money keeps its exact amount and
  must be in the waterfall currency; an input in another currency, a
  condition, or an input with no value is an error naming the expression.
- Python `StatementResult.get_node` is renamed `numeric_series` and WASM
  `statementResultGetNode` is renamed `statementResultNumericSeries`,
  matching the Rust accessor they wrap.
- `EvaluationContext::get_cs_value`, a float reader of a capital reference,
  is removed; capital references are typed.
- Monetary rounding in formulas keeps its one mode, midpoint ties away from
  zero (owner resolution; STATEMENTS-EVALUATION-002).

### Statements: fixes from the branch review

- A run's provenance records the content identity of the validated model, so
  a hand-built model whose value types were not yet inferred keeps a run that
  verifies against it (rollups and supplied-results checks rejected it).
- `lag`, `diff` and the other history functions over a consolidated
  `cs.<component>.total` are unavailable for a period whose close FX quote is
  missing, as the same-period read is; the evaluation used to fail.
- A formula check rejects a reference to an unknown node inside `lag` or
  `shift` when it is built; it used to report every period incomplete.
- `MonteCarloResults.breach_probability` refuses a restored result whose path
  count is zero or above 4,294,967,295.
- The P&L summary, the credit assessment and the variance and scenario
  comparison tables treat a cell without a value as a gap or as missing data;
  they showed `NaN`.
- A principal posting may repay interest a term loan has capitalized; it was
  refused as exceeding the funded principal.
- An excess-cash-flow sweep request is rounded to the currency's minor unit
  (half away from zero) before it is paid; it used to be truncated.
- `CapitalStructureCashflows.periods()` lists every period an instrument has,
  including one that could not be consolidated.
- WASM `applyScenario` reports a model that fails to load as a
  `ContractValidationError` with its `report`, as every other entry point does.

### Statements models: one bounded loading path, identity after validation, decimal-string seeds

Breaking, with no compatibility path (STATEMENTS-CONTRACTS-016).

- `FinancialModelSpec::from_json` is now the bounded loader every host uses:
  at most 64 MiB and 96 levels of nesting, no repeated key, no unknown field,
  and `schema_version` as the integer `1`. It returns `ContractError`
  (it returned the statements `Error`), and `from_slice_strict` returns the
  model alone (it returned a pair with an empty report).
  `FinancialModelSpec::from_value` is removed.
- A decode failure is a contract diagnostic with a code and, where one exists,
  a JSON pointer: Python raises `ContractValidationError` (a `ValueError`)
  with `report`, and `ContractLimitExceededError` for an oversized or
  over-nested input; WASM throws `ContractValidationError` with `code` and
  `report`. A model that decodes but is not runnable still raises by kind
  (`ValueError`, `KeyError`, `RuntimeError`; `kind` in WASM), now through the
  core error, so its message gains the core prefix.
- A document that repeats a key inside a map (for example two entries for one
  node) is rejected; the last entry used to win silently.
- `FinancialModelSpec.content_hash` validates the model first and hashes the
  validated model. A model that is not runnable has no hash (it used to hash),
  and a hand-built model whose value types were not yet inferred now hashes
  the same as its validated form.
- `MonteCarloConfig.seed` is a canonical decimal string in JSON (`"42"`), as
  forecast seeds and the result seed already were; the number `42` is
  rejected. The Python constructor still takes an `int`.
- `GoalSeekResult::from_json` parses under the same limits, rejects repeated
  keys and validates its embedded model, which must carry `schema_version: 1`.
- The published `financial_model_spec` schema example is a runnable model with
  a value node and a formula node.

### Statements Monte Carlo: explicit metrics, one entry point with the evaluation context, typed samples

Breaking, with no compatibility path (STATEMENTS-EVALUATION-011).

- `MonteCarloConfig` requires `metrics`: the node identifiers to report, at
  least one, each a node of the model, none repeated. Only these are sampled
  and stored (every node was). `MonteCarloConfig::new(n_paths, seed, metrics)`
  in Rust; `MonteCarloConfig(n_paths, seed, metrics, percentiles=None,
  include_path_data=False)` in Python.
- `n_paths` must be 1 to 4,294,967,295, and paths x metrics x forecast periods
  may not exceed 100,000,000 samples. Both are checked before a path runs.
- Requested percentiles must be distinct; a repeated level is an error (it was
  dropped).
- `Evaluator::evaluate_monte_carlo(model, config, context)` takes an optional
  market, valuation date and observation cutoff, the context of
  `evaluate_with_market`. A model with floating-rate debt can now be
  simulated. Python: `evaluate_monte_carlo(model, config, market=None,
  as_of=None, observation_cutoff=None)`; WASM: `evaluateMonteCarlo(model,
  config, market?, asOf?, observationCutoff?)`.
- A requested sample without a value no longer fails the run. The run is
  incomplete: `percentile_results` is empty and `incomplete` names the first
  such sample, its reason and the count. A value that is not finite is still
  an error.
- `MonteCarloResults` gains `incomplete`, `seed` (a decimal string in JSON),
  `stream_version` and `metrics`; each `PercentileSeries` gains `value_type` and `sample_count`.
- `percentile_by_period` matches the probability level exactly (it matched
  within 1e-12).
- `breach_probability(metric, threshold)` takes a typed threshold: a number
  for a scalar metric, money in the metric's currency for a monetary one. A
  threshold in other units or not finite is an error, as is a restored path
  table without exactly one finite sample per path and forecast period. In
  Rust it returns `Result<Option<f64>>`.

### Statements capital: a period's debt state commits once, and the PIK metric is a traced input

Breaking, with no compatibility path (STATEMENTS-EVALUATION-012).

- `pik_toggle.liquidity_metric` may not depend on a same-period `cs` output,
  directly or through other nodes; the run is rejected before it starts, like
  the cash pool and the prepayment sizing inputs. Read a prior period with
  `lag(...)` and state the first period's fallback (for example
  `coalesce(lag(cs.debt_balance.total, 1), 0)`).
- A period now works on a staged copy of the debt state. The ledger, the
  coupon elections and the rebuilt schedules replace the committed state only
  after the formulas that read final capital amounts have evaluated, so a
  period that fails leaves the state of the last committed period.

### Statements metrics: bundled leverage and coverage use `ratio`, complete windows and due-date service

Breaking, with no compatibility path (STATEMENTS-CONTRACTS-008).

- The six `fin_leverage` metrics divide with `ratio`: a missing input and a
  zero or negative denominator are unavailable, where an exactly zero
  denominator failed the run and a negative one produced a negative multiple.
  Zero debt is a valid zero; a negative EBITDA still gives signed coverage over
  a positive denominator.
- `fin.debt_service_coverage` is
  `ratio(ttm(ebitda), ttm(scheduled_cash_interest_due + scheduled_principal_due))`.
  It no longer reads `interest_expense` or `principal_payment`: supply the two
  new inputs, the scheduled cash interest and scheduled principal recognized
  once on their contractual due dates.
- New capital components `cs.scheduled_cash_interest_due.<scope>` and
  `cs.scheduled_principal_due.<scope>` report those amounts from each
  borrowing's contractual schedule: a missed installment is recognized on its
  due date, and arrears settlements, prepayments, sweeps, PIK, fees and hedge
  flows are excluded. `CashflowBreakdown` gains the two fields.
- The bundled descriptions and the metrics README no longer state leverage or
  coverage thresholds.

### Statements capital: typed `cs` selectors, three reporting scopes and period-close FX with provenance

Breaking, with no compatibility path (STATEMENTS-CAPITAL-009,
STATEMENTS-CONTRACTS-004, STATEMENTS-CONTRACTS-005).

- A capital reference has three exact scopes: `cs.<component>.<instrument_id>`
  (the instrument's native currency), `cs.<component>.total.<CCY>` (the total
  of the instruments in one native currency, new) and `cs.<component>.total`
  (the consolidated total in the reporting currency). `total` is reserved and
  cannot be an instrument id.
- New components: `net_interest_expense_cash`, `scheduled_principal`,
  `mandatory_prepayment`, `voluntary_prepayment`, `sweep_prepayment` and
  `opening_balance`.
- An unknown component or a malformed scope fails when the formula is parsed.
  An unknown instrument, a native currency no instrument uses, a consolidated
  total with instruments in more than one currency and no reporting currency,
  and any `cs` reference in a model without a capital structure fail when the
  model is built (they failed at evaluation, or resolved to an unknown unit).
- A `cs` reference evaluates to exact money in the scope's currency.
- `capital_structure.fx_policy` accepts only `period_end`; `cashflow_date` is
  removed. A model whose instruments differ from its reporting currency must
  set it: there is no default.
- Each native total converts at one rate read on the inclusive period-close
  date. `CapitalStructureCashflows.fx_conversions` records the pair, rate,
  date and source per period.
- A missing quote or FX matrix no longer fails the run. The period's
  consolidated total is unavailable (`consolidation_unavailable` gives the
  reason), a formula reading it gets an unavailable cell with the new reason
  `missing_fx_quote`, and per-instrument and native-currency values stay.
- `CapitalStructureCashflows::set_period` replaces a period as a whole, clears
  rows the replacement omits, returns an error for a conflicting reporting
  currency and keeps periods in chronological order.
- Rust: `CsComponent`, `CsScope`, `FxConversionRecord`,
  `CapitalStructureCashflows::get_money` and `types::CapitalFxPolicy`;
  `StmtExpr::CsRef` holds `{component, scope}`; `ModelBuilder::fx_policy`
  takes `CapitalFxPolicy`.
- Python: `CapitalStructureCashflows.fx_conversions` and
  `consolidation_unavailable`; `get_component` accepts the new scopes and
  components.

### Statements capital: PIK is elected per loan and applied by the loan's coupon terms

Breaking, with no compatibility path (STATEMENTS-CAPITAL-008).

- `PikToggleSpec` is `{liquidity_metric, threshold, elections}`.
  `target_instrument_ids` and `min_periods_in_pik` are removed. Each election
  is `{instrument_id, pik_fraction, min_coupon_periods}`: `pik_fraction` is the
  share of each elected coupon that capitalizes, an exact decimal in `(0, 1]`,
  and `min_coupon_periods` (default 0) counts the loan's coupon periods, not
  reporting periods.
- An election may name only a term loan of the capital structure (it could
  name a revolver or a bond). The threshold must be finite.
- A request takes effect at the start of the loan's next coupon period after
  the period that observes the metric. The waterfall no longer moves the
  coupon due in the current period into PIK or capitalizes at the period
  close: the elected share is zero cash and capitalizes once on the coupon
  date, through the loan's own coupon terms.
- Overdue cash interest is never transferred into PIK. It stays a cash claim
  and is carried as arrears.
- The toggle is not exercised in actual periods; record a historical election
  on the loan (`covenants.pik_toggles`).
- A request made when no coupon period is left, including after a full payoff,
  elects nothing (in the maturity period it failed the run since the previous
  entry).
- Python: `PikToggleSpec` takes keyword-only `liquidity_metric`, `threshold`
  and `elections`; the `elections` property replaces `target_instrument_ids`
  and `min_periods_in_pik`.
- Rust: `loan_terms::CouponElection` and `TermLoan::coupon_period_starts` in
  `finstack-quant-valuations-fixed-income`;
  `TermLoan::cashflow_schedule_with_postings` takes the elections.

### Statements capital: coupons are rebuilt by the instrument after a principal change

Breaking, with no compatibility path (STATEMENTS-CAPITAL-005).

- After a waterfall prepayment or a toggled PIK coupon, the term loan or
  revolver recompiles its own schedule from its terms and the dated principal
  changes. Statements no longer infers a coupon rate from schedule rows.
  Coupons, the contractual cash/PIK split, balance-based fees, later
  amortization and the terminal repayment all follow the dated principal.
- A change posted at a period close earns from the following day. Interest
  earned before a change inside a coupon is kept whole in `cs.accrued_interest`.
- Revolver commitment and usage fees now respond to a sweep (they were left on
  the original balance), and a swept revolver keeps its commitment and terms
  for a contractual redraw.
- The debt ledger and the rebuilt schedules commit together. A period fails,
  changing nothing, when a schedule cannot be rebuilt or does not carry the
  ledger's closing balance.
- `pik_toggle.target_instrument_ids` may name only term loans and revolvers;
  a bond target is rejected when the model is validated. A revolver rejects a
  principal change on or before its balance anchor (the later of the valuation
  date and the commitment date). A coupon toggled to PIK in the period in which
  its loan matures fails that period (it was capitalized after maturity).
- The ledger and schedule agreement bounds are half of the currency's minor
  unit (they were 0.005 in every currency).
- Rust: `loan_terms::PrincipalPosting`, `TermLoan::cashflow_schedule_with_postings`
  and `RevolvingCredit::cashflow_schedule_with_postings` in
  `finstack-quant-valuations-fixed-income`. A revolver emits one interest row
  per stretch between postings inside a coupon period.

### Statements capital: prepayments elect their facilities; the sweep formula is authored term by term

Breaking, with no compatibility path (STATEMENTS-CAPITAL-007).

- `WaterfallSpec.mandatory_prepay_node` and `voluntary_prepay_node` are
  replaced by `mandatory_prepayment` and `voluntary_prepayment`, each
  `{"amount": <formula>, "facilities": ...}`. `facilities` is required:
  `"all_eligible"` or `{"named": ["TL-A", ...]}`.
- `EcfSweepSpec` is `{cash_measure, deductions, credits, sweep_percentage,
  facilities}`. `ebitda_node`, `taxes_node`, `capex_node`,
  `working_capital_node`, `cash_interest_node` and `target_instrument_id` are
  removed. A deduction or credit is `{"type": "input", "expression": ...}`,
  `{"type": "paid_cash_interest"}`, `{"type": "paid_fees"}` or
  `{"type": "paid_scheduled_principal"}`; the paid terms are the cash earlier
  priorities actually paid this period. Nothing is deducted unless listed, so
  a sweep that relied on the automatic interest, fee and scheduled-principal
  deductions must now list them.
- ECF is exact money in the waterfall currency:
  `cash_measure - deductions + credits`. The percentage applies to positive
  ECF; a non-positive ECF requests nothing.
- A negative mandatory or voluntary request is an error (it was floored to
  zero). A sizing expression that reads a same-period `cs` output is rejected
  when the run starts.
- A named facility must be a debt instrument of the capital structure. A term
  loan with a call schedule cannot be elected by any prepayment: the waterfall
  repays at par and does not price the call premium or make-whole.
- Each rung reports its requested, paid and unpaid amounts:
  `CapitalStructureCashflows.prepayments` maps a period to its
  `PrepaymentOutcome` list. Python gains `CapitalStructureCashflows.prepayments`
  and `retained_cash`; `EcfSweepSpec` takes its fields as
  keyword-only arguments and the `WaterfallSpec` prepayment arguments take the
  wire mappings.

### Statements capital: waterfall allocation is exact; residual cash is equity or retained

STATEMENTS-CAPITAL-006. **Breaking.**

- **Whole minor units.** When a class's cash does not cover its claims, each
  pro-rata quota is floored to the currency's minor unit and the units left
  over go to the largest fractional remainders, ties by instrument id. Five
  cents across three two-cent claims pay 2, 2 and 1. Allocation used
  floating-point shares with the rounding residue on the last row.
- **Every unpaid amount is carried.** The `1e-6` materiality threshold on
  interest, fee and principal shortfalls is removed.
- **Negative claims are rejected.** A negative fee or interest claim fails
  the period. The `negative_claim_neutralized` warning and
  `CapitalStructureClaimCategory` are removed.
- **Retained cash.** The residual is an equity distribution only when
  `Equity` is the terminal priority. Otherwise it is
  `CapitalStructureCashflows.retained_cash` (Python `retained_cash`), and
  `equity_distribution` is zero. It used to go to equity either way.
- **Cash pool independence is transitive.** `available_cash_node` may not
  reach any same-period `cs` component through the formulas and masks of the
  nodes it names; the error names the chain. Only four components were
  checked, and only in the pool's own text.
- **Debt-free periods run.** With no debt claims the whole pool is residual,
  in the model's capital currency.
- **Exact conservation.** A period whose uses do not equal its available
  cash exactly fails.

### Statements capital: principal follows effective dates; unpaid principal is arrears

STATEMENTS-CAPITAL-004. **Breaking.**

- **`debt_balance` is economic principal.** A scheduled reduction takes
  effect on its contractual date whether or not its cash was paid. Unpaid
  scheduled principal no longer stays in the balance and no longer rebuilds
  interest on it; it is `principal_arrears`.
- **Three new amounts** on `CashflowBreakdown`, as `cs.<component>` and as
  rows of `flows`: `principal_pending` (cash owed for a reduction already
  effective and not yet due), `principal_arrears` (cash due and unpaid) and
  `principal_advance_reserve` (face paid for whose reduction is still to
  come, excluded from prepayment capacity).
- **A sweep cannot prepay scheduled principal.** Principal a scheduled
  reduction removes in the period is not prepayable, so a sweep ordered ahead
  of amortization goes to other debt.
- **Dated claims are required.** A waterfall fails for an instrument without
  dated principal claims; the aggregate same-period fallback is removed.
  Claims must equal the contractual principal cash exactly, and scheduled
  principal beyond the balance is rejected instead of clamped.

### Statements capital: the ledger must agree with its schedule; arrears are separate from accrued interest

STATEMENTS-CAPITAL-003. **Breaking.**

- **No rescaling.** `calculate_period_flows` fails when the opening balance
  it is given disagrees with the principal the dated schedule carries into
  the period, naming the instrument, period, currency and both amounts. It
  used to scale the period's coupons, fees and capitalization by the ratio
  and warn. `scheduled_opening_principal(schedule, period_start)` returns the
  agreeing opening. The `residual_scale_mismatch` warning is removed from
  `CapitalStructureWarning`.
- **One balance clock.** Every debt balance closes on the dated schedule:
  newly issued and redrawn debt moves on balance-effective dates, as carried
  debt does. A schedule whose opening notional and funding row both hold the
  same principal now reports their sum.
- **Contractual accrual.** `accrued_interest` is calculated from the
  instrument's own earning terms
  (`Instrument::contractual_accrual_config`), so ACT/ACT ISMA debt accrues
  instead of failing. An instrument that states no terms cannot be accrued.
- **Arrears.** Cash interest due and unpaid is `interest_arrears`
  (`cs.interest_arrears.<instrument or total>`, and a row of
  `CashflowBreakdown::flows`). It is no longer added to `accrued_interest`.

### Statements metrics: bundled returns use average balances and explicit ROIC inputs

STATEMENTS-CONTRACTS-009. **Breaking.**

- **Average balances.** `fin.roe`, `fin.roa`, `fin.roce` and `fin.roic`
  divide same-period earnings by the average of the opening balance (the
  prior period's close) and the closing balance. They used the period-end
  balance. The first period of a model is unavailable.
- **Shared earnings.** ROE and ROA read `fin.net_income` (after
  `tax_expense`); ROCE and ROIC read `fin.ebit`. The `taxes` input is no
  longer read.
- **ROIC inputs.** `fin.roic` is `(ebit - operating_tax) / average
  operating_invested_capital`. A model that loads it supplies
  `operating_tax` (signed; a credit is negative) and
  `operating_invested_capital`; it no longer reads `total_debt +
  total_equity`.
- **Unavailable, not an error.** A zero or negative average balance or a
  missing input leaves the return unavailable; a loss stays negative.

### Statements normalization: an ordered bridge to a new adjusted node

STATEMENTS-EVALUATION-015. **Breaking.**

- **`output_node` is required** on `NormalizationConfig`
  (`NormalizationConfig::new(target, output)`, Python
  `NormalizationConfig(target_node, output_node)`). The adjusted metric is
  published under that new id; the reported node is never overwritten.
- **`merge_into_results(results, config)`** calculates the bridge and
  publishes it. It no longer takes result rows and a node id, rejects any id
  the result already holds, needs a source run, keeps the check report and
  records the configuration in `StatementResult.normalizations`.
- **Typed amounts.** A fixed amount is a scalar for a scalar target and money
  in its currency for a monetary one (`{"amount": "5", "currency": "USD"}`;
  Python accepts `Money`). Bridge values (`base_value`, `raw_amount`,
  `capped_amount`, `adjusted_value`, `final_value`) are typed the same way on
  the wire; the Python accessors return floats.
- **Caps are one of two forms**: `{"type": "absolute", "amount": ...}` or
  `{"type": "reference", "base_node": ..., "fraction": ..., "base_mode":
  ...}`. The `{base_node, value}` form is removed. Rust:
  `AdjustmentCap::Absolute` / `AdjustmentCap::Reference` and
  `Adjustment::with_cap(cap)`; Python `Adjustment.with_cap(cap)` takes the
  mapping and `Adjustment.cap` replaces `cap_base_node`, `cap_value` and
  `cap_base_mode`. Rows record `cap_base` and `cap_limit`.
- **Validated before calculation.** Unknown references, nodes without recorded
  units, unit or currency mismatches, fixed amounts keyed outside the result's
  periods, non-finite numbers and negative caps are rejected up front.
- **Unavailable, not an error.** A target, percentage reference or cap base
  without a value leaves the period `unavailable` with its reason
  (`base_value` and `final_value` may be absent). A row whose arithmetic
  cannot be represented is an error and nothing is published.
- **Rollup.** A result holding a published adjusted node cannot be rolled
  up; normalize the rolled result.

### Statements rollup: every line needs a method and results are typed

STATEMENTS-EVALUATION-014. **Breaking.**

- **No automatic sum.** A line rolls up by its `RollupSpec.methods` entry or
  else its declared `NodeSpec.rollup`; a line with neither fails the rollup,
  naming every such line. Declare `sum` on flows. The bundled flow metrics
  (`fin.gross_profit`, `fin.ebitda` and the rest of `fin_basic`) now declare
  it.
- **Verified source.** `StatementResult::rollup` accepts only a result that
  is a run of the given model (content, source snapshot, period plan and run
  identity); a hand-built result or another model's result is rejected.
- **`sum`.** Money is summed exactly and scalars with a Neumaier compensated
  sum, which the formula functions `sum()` and `mean()` now use too (they
  used Kahan summation). A
  source cell that is missing, masked or unavailable leaves the target
  `unavailable` (it used to be absent); a sum that cannot be represented is
  an error (it used to be dropped).
- **`period_end`.** The final source cell is copied with its status.
- **`formula`.** The mask and formula are re-evaluated on typed cells: a
  false mask is a `masked` cell (it used to be `0`), and an input without a
  value leaves the result `unavailable` instead of failing the rollup.
- **Provenance.** A rolled result has `rollup_provenance` (source run,
  frequency, the method used for each node, the source periods of each
  target) and no run `provenance`. Python: `StatementResult.rollup_provenance`.

### Statements checks: formula checks state their mode and scope

STATEMENTS-CONTRACTS-015. **Breaking.**

- **`mode` and `scope` are required** on every `FormulaCheckSpec`
  (`"mode": "predicate" | "residual"`, `"scope": "all_periods" |
  "actuals_only" | "forecast_only"`). The mode is no longer inferred from the
  tolerance.
- **A predicate is Boolean.** `revenue > 0`, not `revenue`; a numeric
  predicate, a Boolean residual (`a == b` with a tolerance: write `a - b`) and
  a tolerance on a predicate are rejected. A residual without a tolerance is
  an exact comparison with zero.
- **`eligibility`.** An optional Boolean formula; a period in which it is
  false is out of scope.
- **Lazy evaluation.** `coalesce(a, b)` uses `b` when `a` has no value and an
  untaken `if` branch does not read its inputs; the check no longer skips a
  period because some referenced cell is missing.
- **Incomplete, not a warning.** A period whose formula or eligibility
  condition needed an observation without a value is a coverage finding and
  the check is `incomplete`. A reference to a node the model does not have is
  an error.
- **Python.** `FormulaCheckSpec(spec)` takes the canonical fields as one
  mapping and has `mode`, `scope` and `eligibility` properties; the positional
  constructor is removed.

### Statements checks: data-quality coverage and validity are separate

STATEMENTS-CONTRACTS-013. **Breaking.**

- **`non_finite` needs a selection.** `nodes` is required: `"all"` or a list
  of node ids (`{"type": "non_finite", "nodes": "all"}`). The empty list no
  longer means all nodes. Rust: `NodeSelection::all()` /
  `NodeSelection::named(..)`.
- **Unavailable is not invalid.** `non_finite` fails only on a NaN or
  infinity in a value cell. A missing or unavailable cell is coverage:
  require it with `missing_value`.
- **`missing_value`.** A missing required observation is a coverage finding
  and the check is `incomplete`, in actual and forecast periods alike (it
  used to be an error in actuals and a warning in forecasts). An empty scope
  is `unevaluated`.
- **Configuration errors.** Unknown or duplicate node ids in any of the three
  checks, overlapping sign groups, and a result cell outside the model's
  periods.
