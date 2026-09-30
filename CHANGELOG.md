# Changelog

## [Unreleased]

### WASM binding audit: model kernels (2026-09-30)

#### Changed (BREAKING)

- Forward-measure kernels are checked Rust functions in `models::closed_form`: `black76_price`, `black76_greeks`, `bachelier_price` (unit annuity), `bachelier_greeks`, `black_shifted_price`, `black_shifted_vega` and `heston_price`. Both bindings are one call; call/put dispatch and discounting are no longer done in the bindings.
  - `black76_price` rejects `df <= 0`; it used to return a negative premium.
  - Negative vol or expiry, a non-positive Black-76 forward or strike, and non-positive shifted coordinates are errors in both hosts; they used to return intrinsic value or 0.
  - The Greeks return `ForwardGreeks { delta, gamma, vega }`. In Python, `g["delta"]` becomes `g.delta`.
- Rust renames:
  - The dispatchers `barrier_call_str` / `barrier_put_str` / `asian_option_price_str` / `lookback_option_price_str` → `barrier_call` / `barrier_put` / `asian_option_price` / `lookback_option_price`, with arguments ordered `(spot, strike, [barrier,] rate, div_yield, vol, expiry, …)`.
  - `bs_implied_vol` / `black76_implied_vol` take `price` before `option_type`.
  - `volatility::normal::bachelier_price` → `bachelier_price_with_annuity`.
- SABR:
  - `SabrSmile::implied_vol(strike)` is new.
  - `validate_no_arbitrage` / `check_no_arbitrage` / `repair_arbitrage` drop the unused `q`.
  - `ArbitrageValidationResult` carries `arbitrage_free`.
  - The host method `arbitrage_diagnostics(strikes, r=0, q=0)` / `arbitrageDiagnostics` is `validate_no_arbitrage(strikes, r)` / `validateNoArbitrage(strikes, r)`.
  - `SabrShift` parses from `"auto"` via `FromStr`; `withShift(true)` throws `invalid_type` and Python `with_shift(True)` raises `ValueError`.
- WASM:
  - `priceHestonCall` / `priceHestonPut` return `MoneyEstimate` (`mean` and `ci_95` are money values), and `numPaths` and `seed` are optional with Rust defaults.
  - `nelsonSiegelYields(lambda, factors, tenors)` takes the three factors as an array.
  - `deltaToStrike` / `strikeToDelta` name their volatility parameter `vol`.
- `DEFAULT_THETA_DAYS_PER_YEAR` lives in `models::closed_form` and is re-exported by valuations. `DEFAULT_ASIAN_AVERAGING` and `DEFAULT_LOOKBACK_STRIKE_TYPE` are the Rust defaults both hosts use.

### WASM binding audit: calibration, attribution, cashflows, covenants, margin and features (2026-09-30)

#### Changed (BREAKING)

- Rust renames:
  - calibration `api::engine::execute` → `calibrate`, and `execute_json` → `calibrate_from_json`
  - covenants `evaluate_engine_map` → `evaluate_engine`
  - calibration `validate::dry_run` now returns the typed `CalibrationValidationReport`; the JSON form is `dry_run_json`
- WASM:
  - `dryRun` returns the report as an object; the JSON text is `dryRunJson`.
  - `AttributionParams` is renamed `AttributionJsonInputs`.
  - `calculateVm` returns the canonical `VmResult` (Money objects with decimal-string amounts); the hand-built `currency`, `net_margin` and `requires_call` fields are gone.
- Solver settings are validated when parsed: a zero, negative or NaN `tolerance`, or `max_iterations = 0`, is rejected by every calibration entry point in both hosts.
- `CsaSpec::regulatory_inner` validates the spec. `ExposureProfile` and `ExposureDiagnostics` reject unknown fields.
- Python:
  - Quote-set and payload conflicts in `CalibrationPlan(...)` raise `CalibrationEnvelopeError` (kinds `quote_set_conflict`, `conflicting_market_datum`) through the new Rust `CalibrationEnvelope::from_attached_steps`.
  - Feature key columns accept strings and dates only (int, float and bool keys raise `TypeError`), ordered by the new Rust `datetime_order_key`.
  - `transform_panel(dict)` goes through the Rust serde contract, so errors are serde text (`unknown field`, `missing field`).
  - `dated_flows` validates the schedule via the new Rust `cashflows::dated_flows`.
- `cdr_to_mdr` / `mdr_to_cdr` errors name the rate they reject.

#### Added

- `attributePnlEnvelope` (WASM) and `attribute_pnl_envelope` / `AttributionResultEnvelope` (Python) return the attribution envelope as a typed value.
- Rust: `validate::parse_envelope` (public), `validate::validate_fail_fast`, `CalibrationPlan::DEFAULT_ID`, `SolverConfig::validate`, `HashMapMetricSource::from_json`, `cashflows::dated_flows`, and `features::{datetime_order_key, naive_datetime_order_key}`.

### WASM binding audit: models credit, factor, correlation and liquidity (2026-09-29)

#### Changed (BREAKING)

- WASM factor-risk decompositions return the Rust types. `parametricVarDecomposition` and `historicalVarDecomposition` return `PositionRiskDecomposition`; historical results now carry `es_contributions`. `evaluateRiskBudget` returns `RiskBudgetResult`. The binding-only view types are removed.
- WASM factor-risk and liquidity inputs are arrays instead of JSON strings: `positionIds`, `weights`, `covariance`, `positionPnls`, `actualVar`, `targetVarPct`, `returns`, `volumes`. `confidence` is optional on the three decompositions (the Rust 95% presets), and `CreditCalibrator(configJson?)` and `factorModelAt(horizon, riskMeasureJson?)` take Rust defaults.
- WASM `models.correlation.trancheLossStatistics` is replaced by the `PortfolioLossResult` class (`fromLosses`, `fromJson`, `toJson`, `trancheLossStatistics`). Python gains `PortfolioLossResult.from_losses`. `PortfolioLossResult`, `DynamicRecoverySpec` and `EndogenousHazardSpec` validate on deserialize.
- Liquidity tiers use `liquidity_tier(days, thresholds?)` (was `classify_tier`). `LiquidityConfig::try_new` / `validate` require finite, positive, strictly ascending thresholds.
- Python binding-invented defaults are removed:
  - `analyze_exchange_offer` fees and `exchange_type`, `analyze_lme` `opt_acceptance_pct`, `to_hazard_curve` `day_count` and `simulate_paths` `antithetic` are required.
  - `config=` is removed from the VaR/ES decompositions, and `compute_incremental` is `bool = False`.
- Python P&L matrices are position-major only; the scenario-major guess is gone. `default_probabilities` follows the Rust horizon rule.
- `ToggleExerciseModel::threshold` / `stochastic` return `Result`, and `CreditState` and the credit spec constructors reject non-finite values. The WASM `*Json` helpers throw on NaN instead of writing `null`.
- Unknown-label errors for the model enums come from Rust and list the accepted labels.

#### Added

- `MertonModel::simulate_paths_seeded` (used by both hosts), `DecompositionConfig::historical_95` (Python `DecompositionConfig.historical_95`) and `ToggleExerciseModel::optimal`.
- `from_json` / `to_json` on `CreditFactorModel`, `LevelsAtDate` and `PeriodDecomposition`.

### WASM binding audit: portfolio (2026-09-29)

#### Changed (BREAKING)

- Sensitivity matrices have one wire shape in both hosts and both directions: nested `data` rows plus `base_currency`. The flat Python shape (with `n_factors`) is rejected.
  - Rust: `SensitivityMatrixJson` rejects unknown keys and converts through `TryFrom<SensitivityMatrixJson> for SensitivityMatrix`, which validates via the new `SensitivityMatrix::from_rows`. The models `SensitivityMatrix` no longer derives serde, and `FactorPnlProfileJson` is removed.
  - WASM `decomposeFactorRisk` requires `base_currency`, and malformed rows throw a Rust `validation` error.
- Python `FactorRiskDecomposition.measure` returns the serde form of `RiskMeasure`, so VaR and ES give a dict rather than a bare tag.
- `carinoLink` / `carino_link` / `carino_link_json` bind Rust `carino_link` over precomputed `BrinsonPeriodResult`s. The raw sector-period form moves to the new `carinoLinkFromSectorPeriods` / `carino_link_from_sector_periods(_json)`.
- WASM `portfolioResultTotalValue` and `portfolioResultGetMetric` are removed; use the typed result accessors.
- Python `replay_portfolio` / `replay_portfolio_json` take `(portfolio, snapshots, config)` with `config` required, and the `mode=` shorthand is removed.
- `twrr_modified_dietz(period)` is the only Python form. `TwrrPeriod` and `DietzFlow` reject unknown keys, and an omitted `cashflows` means no flows.
- Python `strict_risk` and `allow_partial` default to `None`, resolved against `PortfolioValuationOptions::default()`.
- Metric requests resolve through the new `MetricRegistry::resolve_metric_ids`, so `value_portfolio` / `valuePortfolio` accept every `list_standard_metrics()` id.
- Composite positions are narrowed leg by leg through the new `Instrument::applicable_metrics`. A composite with a deposit leg now reports `dv01`, a non-additive requested metric is listed in `inapplicable_metrics`, and nested composites receive their aggregated metrics.
- Portfolio optimization wire results compute `new_position_trades` and `binding_constraints` in Rust.

#### Added

- `Portfolio.fromMaterialization` / `validateMaterialization` accept a `null` cache.

### WASM binding audit: statements and statements analytics (2026-09-29)

#### Changed (BREAKING)

- `goal_seek` takes `&FinancialModelSpec` and returns `GoalSeekResult { solved_value, model }` without mutating its input. `update_model` is required in every host.
  - WASM `goalSeek(…, updateModel, bounds?)` takes `bounds` as `[lo, hi]` and returns `{ solved_value, model }`, with `model` an object or `null`. It was `updated_model_json`.
  - Python `GoalSeekResult` gains `to_json`, `from_json` and pickling.
- LBO and DCF entry points take the Rust config types:
  - WASM `evaluateLbo(modelJson, configJson)` and Python `evaluate_lbo(model, config)` take `LboConfig`. `sources` use the `[{ name, amount }]` form, `transaction_fees` is required, and `checks` is now populated.
  - WASM `dcfSensitivity(model, wacc, terminalValue, ufcfNode?, netDebtOverride?, optionsJson?, marketJson?)` and Python `dcf_sensitivity(…, options=None)` / `evaluate_dcf(…, options=None)` take `DcfOptions`. `DcfOptions` fills omitted keys from its defaults.
  - `ufcf_node` defaults to `DEFAULT_UFCF_NODE`.
- `traceDependencies` is replaced by `dependencyTree` (the Rust `DependencyTree` object) and `dependencyTreeText`. Python `DependencyTracer.dependency_tree` returns a typed `DependencyTree`, and `dependency_tree_text` is new. Both render with `├──` / `└──` connectors; the old output was flat.
- Comps statistics rename their input: WASM `percentileRank`/`zScore`/`peerStats` take `values` (was `data`), Python takes `values` (was `peer_values`), and Rust regression takes `x_values`/`y_values`. `CompanyMetrics::from_flat_metrics` takes `Option<f64>`, with `None` meaning missing.
- Every model entry point parses through the new `FinancialModelSpec::from_json` (serde plus semantic validation). Semantically invalid models are rejected by `modelNodeIds` and `applyScenario` too.
- Capital-structure specs validate in Rust: `CapitalStructureSpec::validate`, `EcfSweepSpec::validate` and `PikToggleSpec::validate`. The WASM `validate*SpecJson` functions call them, and Python gains `EcfSweepSpec.validate()` and `PikToggleSpec.validate()`.
- `ThreeStatementMapping`, `CreditMapping` and `VarianceConfig` reject unknown keys. Python requires `FormulaCheckSpec.category`/`severity`, dict-form `builtin_checks` and all three `ThreeStatementMapping` node groups.
- `ForecastMetrics` error measures and `Explanation` values serialize non-finite numbers as `"nan"`/`"inf"`/`"-inf"`.
- Python `StatementResult.warnings` and `MonteCarloResults.warnings` return dicts instead of Debug text.

#### Added

- `CheckReport::total_findings()` (Python delegates to it), `DEFAULT_UFCF_NODE` and `DependencyTracer::dependency_tree_text`.

### WASM binding audit: core primitives, dates and market data (2026-09-29)

#### Changed (BREAKING)

- Currency codes are parsed by one Rust parser that does not trim. `" USD "` is rejected in both hosts, including by WASM `new Currency`, `Money.fromDecimalStr` and Python `Money(1, " usd ")`. The error names the code: `Invalid currency code "<code>": not a supported ISO-4217 alphabetic code`.
  - Rust: `Currency: FromStr` now returns `core::Error` (was `strum::ParseError`), `TryFrom<&str>` is derived, and `InputError::UnknownCurrency` carries `code`.
- Python `Money(" 100.25 ", "USD")` is rejected (the amount parser no longer trims).
- `RoundingMode: FromStr` returns `core::Error` and accepts only the serde names. Both bindings parse rounding and scenario modes with it and take their defaults from Rust.
- A zero scalar divisor for `Money` is `Validation error: division by zero` in both hosts. The Python pre-check is gone.
- `realized_variance` and `realized_variance_ohlc` take `Option` for the method and annualization factor. The defaults are CloseToClose and `RealizedVarMethod::OHLC_DEFAULT` (YangZhang), and the factor defaults to 252 via `PeriodKind::Daily`. Python passes `None` through, and its default values are unchanged.
- `FxDeltaVolSurface::new` takes `rr_10d` and `bf_10d` as two `Option`s, and Rust `validate` owns the pairing rule.
- `ValidationMode::from_preset` takes `Option<&str>`.
- `FxForward::from_trade_date` takes `Option<BusinessDayConvention>`; `None` is the Rust default.
- Every Python valuations business-day-convention string goes through the Rust `FromStr`, so the short codes (`MF`, `F`, `P`, `MP`, `NONE`) are accepted everywhere.
- WASM API:
  - `new DiscountCurve({ id, baseDate, knots, interp?, extrapolation?, dayCount?, validationMode?, forwardFloor? })` takes a strict options object or JSON string instead of eight positional arguments.
  - `DayCount.yearFraction(start, end, ctx?)` and `signedYearFraction(start, end, ctx?)` replace `yearFractionWithContext`.
  - `DayCount.calendarDays(start, end)` is static.
  - `FxMatrix.rate(base, quote, date, policy?)` replaces `rateDefault`.
  - `Money.negate()` no longer throws (it uses `Money::checked_neg`).
- The WASM `HazardCurve` constructor no longer pre-checks the recovery rate. Validation order and messages come from the Rust builder (knots first, then recovery).
- Python `VolCube` SABR node dicts are decoded by the Rust `SabrParameterData` serde contract, so unknown or missing keys are rejected.

#### Added

- `core::money::fx::CurrencyPair` with a character-safe `FromStr` (`"EUR/USD"` or `"EURUSD"`). Python `FxMatrix.from_dict` uses it, so a malformed key raises `ValueError` instead of panicking.
- `DayCountContextState: Default`.

### WASM binding audit: wasm32 size safety (2026-09-29)

#### Changed (BREAKING)

- WASM `core.choleskySolve(chol, b)` drops its `n` argument; the system size is `b.length`, as in Rust `cholesky_solve` and Python `cholesky_solve(chol, b)`.
- `ArbitrageReport.elapsed_us` is removed, along with the Python getter and `skip_elapsed`. The arbitrage checker no longer reads the wall clock, so its report is deterministic and safe on wasm32.
- `cholesky_solve` and `apply_lower_triangular` return a `Validation` error that names both lengths, replacing the bare `DimensionMismatch`. `CorrelationError::InvalidSize` prints `expected n×n entries, got m`.

#### Fixed

- Flat-matrix size checks in `core::math::linalg` and `nearest_correlation_matrix` use checked `n * n`. A dimension whose square overflows `usize` (65536 on wasm32) used to wrap past the check and trap the WASM instance or panic in Python. It is now a validation error. `ledoit_wolf_shrinkage` checks `t * n` and `n * n` the same way.
- `MertonModel::simulate_paths` sizes its buffers with checked arithmetic and fallible allocation. An oversized request is a validation error, not a trap or abort.
- `student_t_cdf(NaN, df)` returns NaN instead of panicking inside statrs. `StudentTCopula::tail_dependence(NaN)` is therefore NaN in both hosts.
- The WASM and Python bindings no longer repeat these size checks.

### WASM binding audit: Rust-owned error kinds (2026-09-29)

#### Changed (BREAKING)

- Every Rust error now reports its own `kind()` (`NotFound`, `Validation`, `Computation`): statements, portfolio, scenarios, valuations `PricingError`, `DecompositionError`, `MigrationError`, `FourierError` and `CorrelationError`. Folding into `finstack_quant_core::Error` keeps that kind. Python raises `KeyError`, `ValueError` or `RuntimeError` from it, and WASM sets `error.kind` from it, so both hosts classify a failure the same way. Failures that change class:
  - A statements dependency cycle raises `RuntimeError` (was `ValueError`).
  - A missing statements node or period raises `KeyError`.
  - A COS pricing failure other than bad input raises `RuntimeError`.
  - Correlation repair that does not converge raises `RuntimeError`.
  - An unknown portfolio entity or missing FX rate raises `KeyError`.
  - A failed position valuation carries the kind of its cause.
- Python `finstack_quant.portfolio.ValuationError` and `FxError` are removed. Catch `KeyError`, `ValueError` or `RuntimeError` according to the failure, or `FinstackError`/`PortfolioError` for validation failures.
- Python `CalibrationEnvelopeError.solver_diagnostics` is a dict (was a JSON string), matching the WASM object.
- WASM `ContractValidationError.kind` is always `validation`. The contract-specific label moves to `error.code` (`report` or `limit_exceeded`). Portfolio and attribution errors no longer use domain kinds such as `unknown_entity` or PascalCase variant names.
- `invert_fx_rate(0)` is an `InvalidFxRate` validation error.
- An unknown calendar in `Performance.cagr` is `NotFound` in both hosts. Calendar unions such as `nyse+gblo` resolve.

#### Added

- WASM `CalibrationEnvelopeError` carries the strict-load `diagnostics` array.
- `ErrorKind::as_str`, `core::error::format_chain` and `AttributionEnvelope::from_json` in Rust.

### Master cleanup (2026-09-28)

#### Changed (BREAKING)

- `CashFlowSchedule::weighted_average_life` (now `wal`, matching `AssetPool::wal_from_cashflows` and `MetricId::WAL`). Rust and Python.

#### Fixed

- Variance swap `vega` and `variance_vega` now weight the forward leg by the unobserved share of contractual samples, the same weight the seasoned PV uses. They used the day-count elapsed fraction, which disagreed with dPV/dσ² on weekend-skipping schedules (about 3% on a daily schedule).
- The UI token checker no longer reports words spelled only with hex letters (such as "Fade") as colour literals; hex colours still need a `#`.

### Message and doc pass (2026-09-24)

#### Fixed

- Error messages and docs that cited the removed top-level `pricing_overrides` key now quote the full wire path: the MC path-cap error (`instrument_pricing_overrides.model_config.mc_paths`), the Bermudan swaption LMM error (`instrument_pricing_overrides.model_config.lmm_base_vol`), the CDS option vol doc (`instrument_pricing_overrides.market_quotes.implied_volatility`), the bond quote engine doc (`instrument_pricing_overrides.market_quotes.quoted_clean_price_pct`) and the HW1F cap/floor and swaption test docs. Rust message text only; no wire, type or number change.

### Remaining binding fixes (2026-09-24)

#### Changed (BREAKING)

- `EquityOption.day_count`, `FxOption.day_count`, `CdsTranche.day_count`, `RepLine.day_count` and `Tranche.day_count` (now `DayCount`, was a serde-name `str`), Python only.
- `Tranche.frequency` (now `Tenor`, was a tenor `str`), Python only. Every typed instrument now returns `DayCount`/`Tenor`, so a getter value can be passed back to its own builder.
- `TermLoanBuilder.frequency`/`day_count`, `CapFloorBuilder.frequency`/`day_count`, `TrancheBuilder.frequency`/`day_count` and `StructuredCreditBuilder.frequency` (now accept `Tenor | str` / `DayCount | str`, was wrapper only), Python only.

#### Removed

- `SwaptionBuilder.sabr_params_json` (use `sabr_params`, which accepts a dict or JSON `str`), Python only.
- `RevolvingCreditBuilder.fees_flat` (use `fees` with the `RevolvingCreditFees` dict/JSON shape), Python only.

### Rust constructors, setters and parameters (2026-09-24)

#### Changed (BREAKING)

- Pricing-override setters use the exact field name (Rust only):
  `InstrumentPricingOverrides::with_implied_vol` (now `with_implied_volatility`),
  `with_quoted_dirty_price` (now `with_quoted_dirty_price_currency`),
  `with_merton_mc` (now `with_merton_mc_config`);
  `MetricPricingOverrides::with_spot_bump` (now `with_spot_bump_decimal`),
  `with_vol_bump` (now `with_vol_bump_decimal`), `with_rate_bump` (now
  `with_rate_bump_bp`), `with_credit_spread_bump` (now
  `with_credit_spread_bump_bp`), `with_ytm_bump` (now `with_ytm_bump_bp`);
  `ScenarioPricingOverrides::with_spread_shock_bp` (now
  `with_scenario_spread_shock_bp`).
- Instrument setters use the exact field name (Rust only):
  `Equity::with_dividend_yield_id`, `EquityUnderlyingParams::with_dividend_yield`
  and `EquityOptionMarketData::with_dividend_yield` (now `with_div_yield_id`);
  `Bond::with_cashflows` (now `with_custom_cashflows`); `Swaption::with_sabr`
  (now `with_sabr_params`); `StructuredCredit::with_cleanup_call` (now
  `with_cleanup_call_decimal`); `StructuredCredit::with_calendar` (now
  `with_calendar_id`); `PrivateMarketsFund::with_discount_curve` (now
  `with_discount_curve_id`); `IndexUnderlyingParams::with_yield` /
  `with_duration` (now `with_yield_id` / `with_duration_id`);
  `PoolAsset::with_obligor` (now `with_obligor_id`).
- Every instrument `example()` returns `finstack_quant_core::Result<Self>`
  (30 instruments previously returned `Self` and panicked internally).
  `InterestRateSwap::example_standard` (now `example`) and
  `Snowball::example_snowball` (now `example`). Python:
  `InterestRateSwap.example_standard()` (now `example()`); the Python
  `example()` factories of `Swaption`, `CreditDefaultSwap`, `CdsIndex` and
  `CdsTranche` now raise `ValueError` on a validation failure.
- `EquityOption::european_call` (now `european`, with a trailing
  `option_type` argument) and `european_call_with_market_data` (now
  `european_with_market_data(id, ticker, option_params, market_data)`);
  `EquityOptionParams::european_call` / `european_put` (now `call` / `put`).
  Python: `EquityOption.european_call(...)` (now `EquityOption.european(...,
  option_type, ...)`).
- `CdsTranchePricer::with_params` (now `with_config`); `BasketCalculator::new(config)`
  (now `with_config(config)`) and `BasketCalculator::with_defaults` (now `new`).
- Constructor id parameters take `impl Into<InstrumentId>` (FxSpot, the
  listed future options, EquityOption, Equity, BasisSwap, XccySwap, Repo,
  StructuredCredit) and curve ids take `impl Into<CurveId>`
  (`FinancingLegSpec::new`, `EquityOption::new`, the StructuredCredit
  constructors, `CashflowSpec::floating*`, `PoolAsset::floating_rate_loan`);
  `Repo::overnight` takes `calendar_id: impl Into<CalendarId>`.
- Public instrument helpers name the market argument `market` and the
  valuation date `as_of` (previously `curves`, `context`, `market_ctx`,
  `market_context`, `valuation_date`); `asw_*_with_forward` take
  `forward_curve_id` (was `fwd_curve_id`).
- `CommoditySwap::payment_schedule` drops its unused `as_of` argument.

#### Removed

- `BasisSwap::new_allowing_same_curve` and `BasisSwap::with_allow_same_curve`
  (use `BasisSwap::builder().allow_same_curve(true)` or the JSON field
  `allow_same_curve`).
- `FxSpot::get_effective_notional` (read `notional`).
- Typed-input twins `CreditParams::new_pct`, `ProtectionLegSpec::new_pct`,
  `CdsTranchePricerConfig::with_rfl_copula_pct` /
  `with_custom_stochastic_recovery_pct` / `with_constant_recovery_pct`, and
  `RangeAccrualTermsBuilder::coupon_rate_rate`.

### Type-name hygiene (2026-09-24)

#### Changed (BREAKING)

- Acronyms in type names are CamelCase: `CDSIndex` (now `CdsIndex`),
  `CDSTranche` (now `CdsTranche`), `CDSOption` (now `CdsOption`) and every
  `CDSIndex*`/`CDSTranche*`/`CDSOption*`/`CDSPricer*` companion
  (`CdsIndexParams`, `CdsIndexConstituent`, `CdsTrancheParams`,
  `CdsTranchePricer`, `CdsTranchePricerConfig`, `CdsOptionStrike`, ...), and
  `FIIndexTotalReturnSwap` (now `FiIndexTotalReturnSwap`). Rust, Python
  (`valuations.instruments.CdsIndex`, `CdsTranche`, their builders and params)
  and schema `$defs` names; instrument JSON `type` tags are unchanged.
- Types that shared a name with a different concept take their schema names:
  basket `AssetType` (now `BasketAssetType`), private-markets `Tranche` (now
  `PeFundWaterfallTranche`) and `WaterfallSpec` (now `PeFundWaterfallSpec`),
  structured-credit stochastic `PricingMode` (now
  `StructuredCreditPricingMode`) and Merton `BarrierType` (now
  `MertonBarrierType`, also Python `models.credit.MertonBarrierType`). Rust
  only for the first four; schemas are unchanged.
- `AgencyProgram` wire values are snake_case: `"FNMA"` (now `"fnma"`),
  `"FHLMC"` (now `"fhlmc"`), `"GNMA_I"` (now `"gnma_i"`), `"GNMA_II"` (now
  `"gnma_ii"`); `as_str` and `Display` match. JSON for AgencyMbsPassthrough,
  AgencyTba, DollarRoll and AgencyCmo.
- `NdfFixingSource` wire values are snake_case: `"PBOC"` (now `"pboc"`),
  `"CNHFIX"` (now `"cnhfix"`), `"PHP_BVAL"` (now `"php_bval"`), `"OTHER"` (now
  `"other"`) and likewise for `rbi`, `kftc`, `ptax`, `taifx`, `jisdor`, `bnm`;
  `Display`, `FromStr` and `Ndf::effective_fixing_source` match. Ndf JSON.
- `DealType`, `CmoTrancheType` and `TbaTerm` `Display` prints the serde wire
  value (`clo`, `pac`, `thirty_year`) instead of a label (`CLO`, `PAC`, `30Y`).
  Rust only.
- `SchwartzSmithParams.kappa_x` (now `kappa`) and `SchwartzSmithParams.rho`
  (now `rho_xy`), Schwartz & Smith (2000) notation; `CommodityPricingModel::SchwartzSmith`
  now carries a `SchwartzSmithParams` instead of re-declaring its fields.
  Rust only.
- `swaption::GreekInputs.volatility_convention` (now `convention`). Rust only.

#### Removed

- The unused valuations `instruments::ScheduleSpec`; use core
  `dates::ScheduleSpec`. Rust only.

### Compounding and leg specs (2026-09-24)

#### Changed (BREAKING)

- `FloatingLegCompounding` is now defined once in `finstack_quant_cashflows::builder`
  (re-exported at `finstack_quant_valuations::instruments::rates::irs`) with the
  variants `simple`, `simple_average`, `compounded_in_arrears{lookback_days}`,
  `compounded_with_observation_shift{shift_days}` and
  `compounded_with_rate_cutoff{cutoff_days}`; day counts are `u32` (Rust, Python,
  JSON). Swap-family float legs, TRS financing legs and cross-currency legs reject
  `simple_average`.
- `FloatingRateSpec.overnight_compounding` (now `compounding:
  Option<FloatingLegCompounding>`) on every bond / loan / structured-credit coupon
  (Rust, Python, JSON). `compounded_in_arrears` (now
  `{"compounded_in_arrears": {"lookback_days": 0}}`),
  `compounded_with_lookback{lookback_days}` (now `compounded_in_arrears{lookback_days}`),
  `compounded_with_lockout{lockout_days}` (now `compounded_with_rate_cutoff{cutoff_days}`);
  `simple` keeps term projection and `None` still resolves from the index registry.
  Python `OvernightCompoundingMethod` (now `finstack_quant.cashflows.builder.FloatingLegCompounding`
  with `SIMPLE`, `SIMPLE_AVERAGE`, `compounded_in_arrears(lookback_days=0)`,
  `compounded_with_observation_shift(...)`, `compounded_with_rate_cutoff(...)`).
- `FinancingLegSpec.compounding` is now `FloatingLegCompounding` (Rust, JSON):
  `term_rate` (now `simple`), `overnight_compounded` (now
  `{"compounded_in_arrears": {"lookback_days": 0}}`); lookback, observation-shift
  and rate-cut-off financing legs are now expressible.
- `InterestRateFuture.rate_averaging` (now `compounding: FloatingLegCompounding`),
  also on the IR-future market conventions (Rust, Python `IrFutureConventions.compounding`,
  JSON): `term` (now `simple`), `arithmetic_average` (now `simple_average`),
  `compounded_overnight` (now `{"compounded_in_arrears": {"lookback_days": 0}}`);
  other variants are rejected.
- `InterestRateSwap.fixed` / `.float` (now `fixed_leg` / `float_leg`),
  `CreditDefaultSwap.premium` / `.protection` and `CDSIndex.premium` / `.protection`
  (now `premium_leg` / `protection_leg`), `EquityTotalReturnSwap.financing` and
  `FIIndexTotalReturnSwap.financing` (now `financing_leg`) (Rust fields and builder
  setters, Python getters and builder methods, JSON). The Python IRS stub no longer
  needs `builtins.float`.
- `BasisSwapLeg` (now `FloatLegSpec`) for `BasisSwap.primary_leg` / `reference_leg`
  (Rust, JSON; keys unchanged, `fixing_calendar_id` and `end_of_month` are now honoured).
- `XccySwapLeg` is now `{notional, side, leg: FloatLegSpec}` and
  `XccySwapLeg.allow_calendar_fallback` (now `XccySwap.allow_calendar_fallback`,
  instrument level like `BasisSwap`) (Rust, JSON): the flat float-leg fields move under `leg`, the redundant `currency`
  is removed (it is `notional`'s currency) and `reset_lag_days` is an `i32`
  defaulting to `0`; `leg.fixing_calendar_id` is rejected.
- `YieldCompounding` is now `{Rate(Compounding), Street, TreasuryActual, Moosmuller}`
  over core `Compounding`, so a zero-frequency periodic basis is unrepresentable
  (Rust only).
- `ModelConfig.oas_quote_compounding` is core `Compounding` (Rust, JSON):
  `semi_annual` (now `{"periodic": 2}`); only `continuous` and `{"periodic": 2}`
  validate.
- `finstack_quant_scenarios::spec::Compounding` is core `Compounding` (Rust, JSON):
  `semi_annual` / `quarterly` / `monthly` (now `{"periodic": 2|4|12}`); the Python
  `finstack_quant.scenarios.Compounding` class keeps its constructors and labels.

#### Removed

- `FixedLegSpec.compounding_simple` (Rust, Python, JSON): no pricing path read it;
  fixed legs accrue simple interest.
- `OvernightCompoundingMethod`, `FinancingRateCompounding`, `RateAveragingMethod`,
  `swap_legs::CompoundingMethod` (with `FloatingLegParams.observation_shift_days`),
  `OasQuoteCompounding` and the models `TreeCompounding` enum (tree discounting is
  the `TreeDiscounting` extension trait over core `Compounding`).

### Option and direction enums (2026-09-24)

#### Changed (BREAKING)

- `CDSTranche.side` is now `PayReceive` (Rust, Python, WASM, JSON): `buy_protection`
  (now `pay`) and `sell_protection` (now `receive`), matching CDS and CDS index.
- `EquityTotalReturnSwap.side` and `FIIndexTotalReturnSwap.side` are now
  `PayReceive` (Rust, Python, JSON): `receive_total_return` (now `receive`) and
  `pay_total_return` (now `pay`).
- `XccySwapLeg.side` is now `PayReceive` (Rust only; wire unchanged).
- `ModelConfig.vol_model` (now `tree_model`) on the bond tree (Rust, Python,
  JSON `instrument_pricing_overrides.model_config`): the value is the new
  `ShortRateTreeModel` (`hull_white` / `black_derman_toy`); `black` becomes
  `black_derman_toy` and an absent value still selects Hull-White.
- `InflationCapFloor.option_type` (now `rate_option_type`, typed
  `RateOptionType`) (Rust, JSON).
- `Swaption.exercise_style` is now `ExerciseStyle`, and `Swaption.settlement` /
  `BermudanSwaption.settlement` are now `SettlementType` (Rust; wire unchanged,
  `settlement` stays required). `Swaption.vol_model` and `SwaptionParams.vol_model`
  use the shared `VolatilityModel`, now re-exported at
  `finstack_quant_valuations::instruments` (Rust; wire unchanged).
- `CmsSpreadOption.option_type` is now `OptionType` (Rust, WASM; wire unchanged).
- `FxOption::atm_dns_strike_for_convention` takes `FxDeltaConventionKind`
  (Rust only).

#### Removed

- `TrancheSide`, `TrsSide`, `LegSide` (use `PayReceive`).
- `swaption::VolatilityModel` (schema `SwaptionVolatilityModel`),
  `SwaptionSettlement`, `SwaptionExercise` (use `VolatilityModel`,
  `SettlementType`, `ExerciseStyle`).
- `InflationCapFloorType` (use `RateOptionType`), `CmsSpreadOptionType` (use
  `OptionType`), `FxAtmDeltaConvention` (use `FxDeltaConventionKind`).

### Private markets and DCF (2026-09-24)

`RealEstateAsset` prices against the caller's `as_of` and stores no valuation
date; the sale date, acquisition costs and the DCF EV-to-equity bridge each
have one channel; the PE-fund waterfall spells its hurdle `hurdle_irr`, its
day count `day_count` and its catch-up `catch_up`.

#### Changed (BREAKING)

- `TerminalValueSpec::GordonGrowth.growth_rate` (now `stable_growth_rate`, matching `HModel`): Rust, JSON `terminal_value.stable_growth_rate`, Python `TerminalValueSpec.gordon_growth(stable_growth_rate)`.
- `DiscountedCashFlow.equity_bridge` is now a required `EquityBridge` (Rust/JSON) and the only EV-to-equity channel; a flat net-debt deduction is written `{"total_debt": x, "cash": y}`. `validate()` now rejects non-finite bridge amounts (`equity_bridge.<field>`). `effective_net_debt()` is `equity_bridge.net_adjustment()`.
- statements-analytics `evaluate_dcf` / `dcf_sensitivity` / `CorporateAnalysisBuilder`: when `DcfOptions.equity_bridge` is set, the model debt/cash (and `net_debt_override`) are no longer read, so a model without balance-sheet nodes no longer errors; otherwise `net_debt_override` becomes the bridge `total_debt` and the model's debt and cash become `total_debt`/`cash`.
- `Tranche::PreferredIrr.irr` (now `hurdle_irr`) and `Tranche::PromoteTier.hurdle: Hurdle::Irr { rate }` (now a flat `hurdle_irr`): Rust, JSON `tranches[].preferred_irr.hurdle_irr` / `tranches[].promote_tier.hurdle_irr`.
- `WaterfallSpec.irr_basis` (now `day_count`) and builder `irr_basis()` (now `day_count()`): Rust, JSON.
- `WaterfallSpec.catchup_mode` (now `catch_up_mode`), builder `catchup_mode()` / `catchup()` (now `catch_up_mode()` / `catch_up()`): Rust, JSON.
- `RealEstateAsset.sale_date` may now equal `as_of` (the old check required it strictly after the stored `valuation_date`); pricing still rejects a horizon before `as_of`.

#### Removed

- `RealEstateAsset.valuation_date` (Rust/JSON; the pricer already discounted from `as_of`) and the builder `.valuation_date(...)`.
- `RealEstateAsset.acquisition_cost` (scalar); use the `acquisition_costs: Vec<Money>` line items.
- `LeveredRealEstateEquity.exit_date`; the levered exit, schedule and metrics follow the asset horizon (`asset.sale_date`, else the last NOI date), the same horizon as PV.
- `DiscountedCashFlow.net_debt` (Rust/JSON/builder).
- `pe_fund::Hurdle` enum and `ClawbackSpec.enable` (presence of `waterfall_spec.clawback` is the switch) plus `impl Default for ClawbackSpec`.

#### Numbers change

- None observed in fixtures or goldens: `RealEstateAsset.valuation_date` was never read by the pricer (it already discounted from `as_of`), and every levered fixture set `exit_date` equal to the asset horizon. A levered position whose `exit_date` differed from `asset.sale_date` now exits on `sale_date`, so its cashflow schedule and DSCR/IRR metrics match its PV horizon.

### Composite and basket (2026-09-24)

#### Changed (BREAKING)

- `CompositeLegSpec.weight` (now `score`): Rust field and `CompositeLegSpec::new` argument, JSON key `legs[i].score`, Python `CompositeLegSpec(instrument_id, instrument, score)` and `.score`, WASM JSON. The user-defined expression column `leg.{id}.weight` is now `leg.{id}.score`.
- `RebalanceRule::Calendar.frequency` is now a `Tenor` (`{count, unit}` on the wire) instead of the `daily`/`weekly`/`monthly`/`quarterly` string; any cadence such as `6M` or `2W` is accepted. Python `RebalanceRule.calendar(start, frequency: Tenor | str, ...)` takes a `Tenor` or tenor string (`"1M"`).
- `WeightingMethod::UserDefined` gains a required `annualization_factor` (Rust/JSON) replacing the hard-coded `sqrt(252)` behind the `leg.{id}.volatility` column; both volatility columns now share one definition (sample std × `sqrt(annualization_factor)`).
- `weighting_inputs["leg.{id}.notional"]` is the signed reporting-currency notional under `NotionalWeighted` too (previously `abs`); only the quantity formula takes the absolute value.
- `Basket.currency` (now `reporting_currency`): Rust field, builder `.reporting_currency(...)`, JSON key.
- `CompositeInstrument::primitive_exposure_report` (now `primitive_exposures`) in Rust, matching Python/WASM; portfolio `primitive_exposure_report` (now `primitive_exposures`).
- `CompositeValuationDetails.exposure_report` (now `exposures`): Rust field and valuation-result JSON key, matching `CompositeHistoryRow.exposures`.
- `CompositeHistoryEngine::run` / `run_from_spec` (now free functions `composite::history` / `history_from_spec`): Rust and Python (`finstack_quant.valuations.composite.history` / `history_from_spec`); WASM already used `history` / `historyFromSpec`.
- Python `CompositeSpec.reporting_currency` returns a `Currency` (was `str`), matching `CompositeExposureReport.reporting_currency`.
- Python `CompositeState.resolved_quantities` (now `resolved_legs`), matching the Rust field and wire key.

#### Removed

- `RebalanceFrequency` (Rust/JSON schema) and `CompositeHistoryEngine` (Rust/Python).
- WASM `index.d.ts` `CompositePrimitivePath` / `CompositePrimitiveAggregate`; the facade re-exports the generated `PrimitiveExposure` / `PrimitiveAggregate` / `CompositeExposureReport` types.

### Structured-credit deal terms (2026-09-24)

A pool asset's agency rating is `rating` (as on `Tranche`) and its default
flag is `defaulted` (optional on the wire, as on `CDSIndexConstituent`); every
stochastic default/prepay variant names its systematic-factor beta
`factor_loading`; the hotel collateral class is `hospitality_mortgage`,
matching `RealEstatePropertyType`; the deal/pool/index denomination currency
is `currency`; and the tranche spread solvers take the same clean price, in
the same position. Retired keys are rejected by `deny_unknown_fields`
(`tests/instruments/structured_credit/integration/serialization_tests.rs`).
Numbers do not change.

#### Changed (BREAKING)

- `PoolAsset.credit_quality` (now `rating`). Rust / Python kwarg and getter /
  JSON.
- `PoolAsset.is_defaulted` (now `defaulted`, `#[serde(default)] = false`).
  Rust / Python kwarg and getter / JSON.
- `StochasticDefaultSpec::IntensityProcess.factor_sensitivity` (now
  `factor_loading`; also on the in-memory `HazardCurveBased` variant and the
  `intensity_process` / `from_hazard_curve` parameters). Rust / JSON.
- Structured-credit assumptions registry `default_factor_sensitivity` (now
  `default_factor_loading`). JSON.
- Every systematic-factor loading is clamped to, and the registry validates,
  one range, [-1, 1]. The intensity-process and hazard-curve default models
  previously clamped to [-2, 2] and the registry accepted any non-negative
  default loading; the four other stochastic variants already used [-1, 1],
  and every shipped registry value (0.5, 0.8) lies inside it. Rust / JSON.
- `AssetType::HotelMortgage` (now `HospitalityMortgage`, wire
  `hospitality_mortgage`). Rust / JSON.
- `AssetType::OtherMortgage.property_type` (now `description`, as on
  `Generic` and `Hybrid`). Rust / JSON.
- `AssetPool.base_currency` (now `currency`; `AssetPool::new` parameter too)
  and `AssetPool::get_base_currency` (now `get_currency`). Rust / Python
  constructor argument and getter / JSON.
- `Waterfall.base_currency` (now `currency`; builder and template parameters
  too). Rust / Python getter / JSON.
- `IndexUnderlyingParams.base_currency` (now `currency`; `new` parameter too),
  used by `FIIndexTotalReturnSwap.underlying`. Rust / JSON.
- `TrancheMetrics.currency` and `EquityMetrics.currency` are now typed
  `Currency` (unknown ISO codes fail to load). Rust; JSON and Python unchanged.
- `calculate_tranche_discount_margin` takes `market_price_pct` (clean price, %
  of CURRENT balance, accrued added at settlement) in place of the dirty
  `target_pv: Money`, as the OAS and metrics solvers do. Python / WASM
  `structured_credit_tranche_discount_margin` / `structuredCreditTrancheDiscountMargin`
  take `market_price_pct` / `marketPricePct` in place of `target_pv` /
  `targetPv`. Rust / Python / WASM.
- `calculate_tranche_oas` (and Python `structured_credit_tranche_oas`, WASM
  `structuredCreditTrancheOas`) takes `market_price_pct` after `as_of`:
  `(deal, tranche_id, market, as_of, market_price_pct, config)`. Rust / Python
  / WASM.
- `calculate_tranche_breakeven_cdr` and `scenario_table` name their market
  parameter `market`. Rust.
- `StructuredCredit::enable_stochastic_defaults` (now `enable_stochastic`,
  pairing with `disable_stochastic` and `is_stochastic`). Rust / Python.
- `ReserveInterestDestination::Tranche.tranche_id` is now `String`, like every
  other tranche reference. Rust (wire shape unchanged).
- Coverage-test results name the computed ratio `ratio` and the flag
  `passing` (was `current_ratio` / `is_passing` on `TestResult`). Rust.

#### Fixed

- The incentive fee's `share` is validated as a decimal in [0, 1] on the
  user-supplied paths (`fees.incentive_fee.share` and a custom waterfall's
  `IncentiveFee.share`), not only in the assumptions registry; a percent-style
  `20` is now rejected instead of paying 20x the residual.
- WASM docs described the OAS and metrics price as a percent of ORIGINAL
  balance; it is a percent of CURRENT balance.

### FX and money market (2026-09-24)

The FX family's quote-currency discount curve is `domestic_discount_curve_id`
on every instrument, including FxSpot; the base-currency notional is
`notional` on every instrument, including FxSwap; Ndf names its
settlement-currency calendar `settlement_calendar_id` and its fixing benchmark
`fixing_source`; an agreed forward rate is optional on both FxForward and Ndf,
with `None` meaning at-market; every FX option-style instrument defaults
`day_count` to ACT/365F; and repo collateral is identified once, by
`CollateralSpec.instrument_id`. Retired keys are rejected by
`deny_unknown_fields` (`tests/instruments/fx_money_market_wire_keys.rs`).
Numbers do not change.

#### Changed (BREAKING)

- `FxSpot.discount_curve_id` (now `domestic_discount_curve_id`, still
  optional). Rust / JSON.
- `FxSwap.base_notional` (now `notional`), including the builder setter and the
  `from_trade_date` / `from_broken_dates` parameters. Rust / JSON.
- `Ndf.quote_calendar_id` (now `settlement_calendar_id`), including the builder
  setter and the `from_trade_date` parameter. Rust / JSON.
- `Ndf.fixing_source_enum` (now `fixing_source`; builder `fixing_source_opt`).
  Rust / JSON.
- `Ndf.contract_rate` is now `Option<f64>`: omit it to value the NDF at-market
  (zero settlement amount and PV), as FxForward already allows. Rust / JSON.
- `FxDigitalOption.day_count`, `FxTouchOption.day_count`,
  `QuantoOption.day_count` and `FxVarianceSwap.day_count` are now optional on
  the wire and in the builder, defaulting to ACT/365F like FxOption and
  FxBarrierOption. Rust / JSON.
- `CollateralSpec.instrument_id` is now the `InstrumentId` newtype. Rust / JSON
  (wire shape unchanged).

#### Removed

- `CollateralType::Special.security_id`: the special security is identified by
  `CollateralSpec.instrument_id`. `CollateralSpec::special` drops its
  `security_id` argument. Rust / JSON.

### Loan and facility terms (2026-09-24)

A facility's size is `commitment` and its drawn balance `drawn`; a loan,
revolver, asset-backed facility or structured-credit note carries one coupon
type, `loan_terms::RateSpec { fixed { rate }, floating }`, with a decimal fixed
rate; term-loan amortization is the shared cashflows `AmortizationSpec`; a
term-loan make-whole call carries the bond `MakeWholeSpec`; contractual margin
and fee basis points are `Decimal` (no `i32` bp remains); a commitment step's
one-off fee is `reduction_fee_bp`; the undrawn fee is `commitment_fee_bp`;
the upfront fee is `Option<UpfrontFee>`; scheduled draws are
`draws: Vec<DrawEvent>`; the revolver utilization OU uses `kappa` / `theta` /
`sigma`; and a convertible's coupon is `cashflow_spec` with one `PriceTrigger`
for both of its share-price triggers. Retired keys are rejected by
`deny_unknown_fields`. Numbers do not change: every migrated input reprices
bit-for-bit to the value the equivalent old input gave
(`tests/instruments/loan_facility_wire_keys.rs`).

#### Changed (BREAKING)

- `RevolvingCredit.commitment_amount` / `drawn_amount` (now `commitment` /
  `drawn`, fields, builder setters and Python getters/setters);
  `DdtlSpec.commitment_limit` (now `commitment`); cashflows
  `CashFlowMeta.facility_limit` and `FeeBase::Undrawn { facility_limit }` (now
  `commitment`; Python `FeeBase.undrawn(commitment)` and
  `CashFlowMeta.commitment`); Rust/Python/WASM/JSON.
- `RevolvingCredit.base_rate_spec: BaseRateSpec` (now `rate: RateSpec`; Python
  `RevolvingCredit.rate` / `RevolvingCreditBuilder.rate`); structured-credit
  `Tranche.coupon: TrancheCoupon` (now `RateSpec`, same JSON);
  `TermLoan.rate` `{"fixed": {"rate_bp": 600}}` (now `{"fixed": {"rate": 0.06}}`,
  a decimal); `AssetBackedFacility.forward_curve_id` + `spread_bp` (now
  `rate: RateSpec`; a floating facility gives a full `FloatingRateSpec`;
  Python `AssetBackedFacility.rate` / builder `rate`); Rust/Python/WASM/JSON.
- `TermLoan.amortization` is the cashflows `AmortizationSpec`:
  `percent_per_period { bp }` (now `percent_of_remaining_per_period { pct }`,
  a decimal), `percent_of_original_notional { bp }` (now
  `percent_of_original_per_period { pct }`), `linear { start, end }` (now
  `linear_between { start, end }`), `custom [..]` (now
  `custom_principal { items }`); term loans reject `linear_to` and
  `step_remaining`. The cashflows `AmortizationSpec` gains
  `PercentOfRemainingPerPeriod { pct }` and `LinearBetween { start, end }`
  (bond builder support; Python `AmortizationSpec.percent_of_remaining_per_period`
  / `linear_between` and the `window` getter); Rust/Python/JSON.
- `LoanCallType::MakeWhole { treasury_spread_bp: i32 }` (now
  `MakeWhole(MakeWholeSpec { reference_curve_id, spread_bp: f64 })`, the bond
  type); Rust/JSON.
- `MarginStep.delta_bp: i32`, `OidPolicy::WithheldBp` / `SeparateBp(i32)` and
  `DdtlSpec.usage_fee_bp` / `commitment_fee_bp: f64` (now `Decimal`, JSON
  strings such as `"125"`); Rust/Python/JSON.
- `CommitmentStep.fee_bp: f64` (now `reduction_fee_bp: Decimal`, a one-off fee
  in bp of the reduced amount); Rust/Python/JSON.
- `AssetBackedFacility.unused_fee_bp: f64` (now `commitment_fee_bp: Decimal`);
  `FacilityProjection.unused_fees` (now `commitment_fees`, booked as
  `CFKind::CommitmentFee` instead of `CFKind::Fee`); Python
  `FacilityProjection.to_dataframe` column `unused_fee` (now
  `commitment_fee`); Rust/Python/JSON.
- `TermLoan.upfront_fee: Option<Money>` (now `Option<UpfrontFee>`, e.g.
  `{"amount": {...}}` or `{"fraction_of_commitment": 0.02}` of the DDTL
  commitment, else `notional_limit`); Python `TermLoan.upfront_fee` returns the
  serde dict and the builder also accepts that dict; Rust/Python/JSON.
- `AssetBackedFacility.draw_schedule: Vec<FacilityDraw>` (now
  `draws: Vec<DrawEvent>`; `DrawEvent` moves to `loan_terms` and is shared
  with the DDTL); Rust/Python/JSON.
- `UtilizationProcess::MeanReverting { target_rate, speed, volatility }` (now
  `{ theta, kappa, sigma }`); the enum now denies unknown fields;
  Rust/Python/WASM/JSON.
- `ConvertibleBond.fixed_coupon` / `floating_coupon` (now
  `cashflow_spec: CashflowSpec`, the bond type; a zero-coupon convertible is a
  fixed spec with rate `"0"`; Python getter/builder `cashflow_spec`);
  `SoftCallTrigger` (now `PriceTrigger`, same fields);
  `ConversionEvent::PriceTrigger { threshold, lookback_days }` (now
  `PriceTrigger(PriceTrigger { threshold_pct, observation_days,
  required_days_above })`: the level is a percent of the conversion price,
  not an absolute share price; the observation window is not modeled for
  contingent conversion); Rust/Python/WASM/JSON.

#### Removed

- `BaseRateSpec`, `TrancheCoupon`, the term-loan `RateSpec` and the
  term-loan `AmortizationSpec` (schema name `TermLoanAmortizationSpec`),
  `FacilityDraw`, `SoftCallTrigger`.

### Dates and calendars (2026-09-24)

Flat contract dates are `start_date` / `maturity`; bonds, loans and revolvers
carry `issue_date`; an option's underlying contract uses
`underlying_start_date` / `underlying_maturity`; every option expiry is
`expiry`; Bermudan exercise dates are `exercise_dates` (inside
`exercise_schedule` when a lockout travels with them); a call lockout is the
date `lockout_end`; window structs use `start` / `end`; a single-schedule deal
names its calendar `calendar_id` and its convention `business_day_convention`;
and every `*_calendar_id` field is typed `CalendarId`. Retired keys are rejected
by `deny_unknown_fields`. Numbers do not change.

#### Changed (BREAKING)

- `RangeAccrualTerms.accrual_start_date` (now `start_date`; also inside
  `CallableRangeAccrual.range_accrual`); Rust/JSON.
- `CDSTranche.effective_date` (now `start_date`); Python `CDSTranche.effective_date`
  getter and `CDSTrancheBuilder.effective_date` (now `start_date`);
  Rust/Python/JSON.
- `CommoditySwaption.swap_start`/`swap_end` (now
  `underlying_start_date`/`underlying_maturity`, fields and builder setters);
  `SwaptionParams.swap_start`/`swap_end` (now
  `underlying_start_date`/`underlying_maturity`); `Swaption::get_swap_start`/
  `get_swap_end` (now `get_underlying_start_date`/`get_underlying_maturity`,
  also on Python `Swaption`); Rust/Python/JSON.
- `ConventionSwapParams.start`/`end` (now `start_date`/`maturity`, matching
  `ConventionFraParams`); Python `InterestRateSwap.from_conventions` keywords
  `start`/`end` (now `start_date`/`maturity`); Rust/Python.
- `CDSOption.underlying_effective_date`/`cds_maturity` and
  `CDSOptionParams.cds_maturity` (now `underlying_start_date`/
  `underlying_maturity`); Rust/JSON.
- `CmsSpreadOption.expiry_date` (now `expiry`); Rust/JSON.
- `BermudanSwaption.bermudan_schedule` (now `exercise_schedule`);
  `EquityOption.exercise_schedule` and `CommodityOption.exercise_schedule` (now
  `exercise_dates`; Python `EquityOption.exercise_schedule` getter and
  `EquityOptionBuilder.exercise_schedule` now `exercise_dates`);
  Rust/Python/JSON.
- `BermudanCallProvision.lockout_periods: usize` (a count of observation
  periods) (now `lockout_end: Option<Date>`, an exclusive date bound matching
  `BermudanSchedule.lockout_end`); `BermudanCallProvision::new` takes
  `lockout_end` and `eligible_call_dates()` takes no argument. Migrate
  `lockout_periods = k > 0` to `lockout_end = observation_dates[k - 1]` and
  `k = 0` to omitting the key; Rust/JSON.
- `CallPut.start_date`/`end_date` (now `start`/`end`, for bond and convertible
  `call_put.calls[]`/`puts[]`); `ReinvestmentPeriod.end_date` (now `end`);
  `ControlledAccumulationSpec.start_date` (now `start`); Rust/Python/JSON.
- `StructuredCredit.payment_calendar_id`/`payment_business_day_convention` (now
  `calendar_id`/`business_day_convention`), `StructuredCredit::with_payment_calendar`/
  `with_payment_business_day_convention` (now `with_calendar`/
  `with_business_day_convention`), `AssetBackedFacility.payment_calendar_id`
  (now `calendar_id`); Python `payment_calendar_id` keyword and getter (now
  `calendar_id`); Rust/Python/JSON.
- `RevolvingCredit.commitment_date` (now `issue_date`, field, builder setter and
  Python getter/builder); Rust/Python/JSON.
- `CashflowScheduleBuildSpec.issue` (now `issue_date`);
  `InflationLinkedBondParams.issue` and the `issue` parameter of `Bond::fixed`,
  `Bond::with_convention`, `Bond::floating`, `Bond::floating_with_convention`
  and `Bond::zero_coupon` (now `issue_date`; Python `Bond` factory keyword `issue` now
  `issue_date`; WASM `issue` parameter now `issueDate`); Rust/Python/WASM/JSON.
- `CashFlowMeta.maturity_date` (now `maturity`; Python `CashFlowMeta` keyword and
  getter follow); Rust/Python/JSON.
- Every instrument and cashflow-spec `*_calendar_id` field that was `String` or
  `Option<String>` (legs, TermLoan, RevolvingCredit, StructuredCredit,
  AssetBackedFacility, CDSTranche, CommodityForward, InflationCapFloor,
  IRFuture, XccySwap, FX spot/forward/swap/NDF/variance swap, VarianceSwap, Bond
  ex-coupon, cashflows `ScheduleParams`/`FloatingRateSpec`/`ExCouponRule`) is
  now `CalendarId` / `Option<CalendarId>`; field names and wire format are
  unchanged; Rust only.

#### Removed

- `Deposit.settlement_days` (formerly `spot_lag_days`). `start_date` is now
  always the accrual start (spot) date; callers holding a trade date compute
  the spot date before building. `Deposit::from_conventions` now sets
  `start_date` to `trade_date` plus the index's `market_settlement_days`
  business days on its market calendar, and `Deposit::example` starts on its
  spot date 2024-01-03. PVs are unchanged. Rust, JSON (the key is rejected).

### Coupon, strike and inflation terms (2026-09-24)

A margin over a floating or CMS index is `spread_bp` / `cms_spread_bp` (basis
points), the fixing lag is `reset_lag_days`, a flat contractual rate is
`fixed_rate: Decimal`, the index multiplier is `gearing`, structured-credit
floors are `index_floor_bp` / `all_in_floor_bp`, explicit schedules are
`start_date` plus `payment_dates`, a CMS tenor is a `Tenor`, an option strike is
`strike`, the base reference CPI is `base_cpi`, and the contractual inflation
lag and interpolation are `lag` / `interpolation`. Retired keys are rejected by
`deny_unknown_fields`; every migrated example reprices bit-for-bit.

#### Changed (BREAKING)

- `CapFloor.spread` (decimal rate) (now `spread_bp`, a `Decimal` in basis
  points: `0.001` becomes `10`); Python `CapFloor.spread` getter and
  `CapFloorBuilder.spread` (now `spread_bp`, taking `float | Bps`);
  Rust/Python/JSON.
- `CmsSwap.cms_spread` (decimal `f64`) (now `cms_spread_bp`, a `Decimal` in
  basis points); `FundingLeg::Floating.spread` (decimal `f64`) (now
  `spread_bp`, a `Decimal` in basis points); `FundingLeg::Fixed.rate` is now a
  `Decimal` (JSON string); `FundingLegSpec` follows; `FundingLeg` now denies
  unknown fields; Rust/JSON.
- `CmsSwap.cms_fixing_dates`/`cms_payment_dates`/`cms_accrual_fractions`/`cms_day_count`
  (now `fixing_dates`/`payment_dates`/`accrual_fractions`/`day_count`, matching
  `CmsOption`); `CmsSwap::from_schedule` takes `cms_tenor: Tenor`,
  `cms_spread_bp: Decimal` and `day_count`; Rust/JSON.
- `CmsSwap.cms_tenor` and `CmsOption.cms_tenor` (fractional years `f64`) (now
  `Tenor`, wire `{"count": 10, "unit": "years"}`); a day- or week-based tenor is
  rejected; Rust/JSON.
- `CmsSpreadOption.strike` is now a `Decimal` (JSON string, `"0.005"` = 50bp);
  Rust/JSON.
- `AssetBackedFacility.margin_bp` (now `spread_bp`; still the all-in fixed rate
  when `forward_curve_id` is `None`); Python getter and builder setter
  `margin_bp` (now `spread_bp`); Rust/Python/JSON.
- `Bond::floating`/`floating_with_convention`, `CashflowSpec::floating`/`floating_bp`/`floating_with_reset_lag`
  parameter `margin_bp` (now `spread_bp`); Python `Bond.floating*` keyword
  `margin_bp` (now `spread_bp`); WASM `marginBp` (now `spreadBp`);
  Rust/Python/WASM.
- `ForwardRateAgreement.reset_lag` (now `reset_lag_days`); Rust/JSON. The
  reset-lag defaults of the other carriers are unchanged.
- `Deposit.quote_rate` (now `fixed_rate`), `ConventionDepositParams.quote_rate`
  (now `fixed_rate`); Rust/JSON. The `quote_rate` metric id is unchanged.
- `Snowball.leverage` (now `gearing`); `Snowball.fixed_rate` and
  `Tarn.fixed_rate` are now `Decimal` (JSON string); `Snowball.coupon_dates` and
  `Tarn.coupon_dates` (N + 1 boundaries) (now `start_date` plus N
  `payment_dates`); Rust/JSON.
- `snowball_coupon_profile` / `inverse_floater_coupon_profile` parameters
  `floor`/`cap`/`leverage` (now `coupon_floor`/`coupon_cap`/`gearing`), with
  `coupon_cap: Option<f64>` (`None` = uncapped; infinity is no longer
  accepted); Rust/Python/WASM.
- `PoolAsset.index_floor` and `RepLine.index_floor` (annual decimal) (now
  `index_floor_bp`, basis points: `0.01` becomes `100`);
  `ReinvestmentAssumptions.coupon_floor` (annual decimal) (now
  `all_in_floor_bp`, basis points); Python `PoolAsset`/`RepLine` keyword and
  getter `index_floor` (now `index_floor_bp`); Rust/Python/JSON.
- `QuantoOption.equity_strike: Money` (now `strike: f64`, per-unit price in
  `base_currency`); Rust/JSON.
- `CommoditySwap.fixed_price` is now an `f64` (JSON number), matching
  `CommoditySwaption.fixed_price`; Rust/JSON.
- `InflationLinkedBond.base_index` and `InflationLinkedBondParams.base_index`
  (now `base_cpi`); Rust/JSON.
- `InflationSwap`/`YoYInflationSwap`/`InflationCapFloor.lag_override` and
  `interpolation_override` (now `lag` and `interpolation`); Rust/JSON.

### Underlying identity, spot ids and size (2026-09-24)

The underlying spot is a typed `spot_id: PriceId`; no market id is read from
`attributes.meta` or derived from a ticker or instrument id (`-SPOT`, `-VOL`,
`_VOL`, `-DIVYIELD`, global `EQUITY-SPOT`/`EQUITY-DIVYIELD`). `underlying_ticker`
is a label only. Instrument-local quotes are `quoted_spot`/`quoted_forward`,
quanto inputs use the shared `QuantoSpec`, and a count of underlying units is
`quantity`. Every `MarketContext::get_price` key is typed `PriceId` (the wire
stays a string).

#### Changed (BREAKING)

- `ConvertibleBond.underlying_equity_id` (now `spot_id: PriceId`, required),
  plus new required `vol_surface_id` (a surface, or a unitless flat-vol scalar)
  and optional `div_yield_id`; the `attributes.meta` `vol_surface_id`/`div_yield_id`
  keys and the `{id}-VOL`/`{id}-DIVYIELD` fallbacks are gone. A configured
  `div_yield_id` must resolve (no silent q = 0). Python getters/builder
  `underlying_equity_id` (now `spot_id`, `vol_surface_id`, `div_yield_id`);
  Rust/Python/JSON.
- `VarianceSwap` gains required `spot_id: PriceId` and `vol_surface_id: CurveId`
  and optional `div_yield_id` (unset means a zero yield); the close series
  defaults to `spot_id`. The `{ticker}`/`{ticker}_VOL`/`{ticker}-DIVYIELD`
  lookups are gone; Rust/JSON.
- `Equity.price_id` (now `spot_id`), `Equity.price_quote` (now `quoted_spot`),
  `Equity.shares` (now `quantity`); builders `with_price_id`/`with_price`/`with_shares`
  (now `with_spot_id`/`with_quoted_spot`/`with_quantity`), `effective_shares()`
  (now `effective_quantity()`). Without `quoted_spot` a `spot_id` is required: the
  meta keys and ticker/id/`-SPOT`/`EQUITY-SPOT` candidates, and the
  `-DIVYIELD`/`EQUITY-DIVYIELD` dividend candidates, are removed; Rust/JSON.
- `EquityOption.notional`, `AsianOption.notional`, `BarrierOption.notional` and
  `LookbackOption.notional` (a `Money` unit count; now `quantity: f64` plus a
  separate `currency: Currency`). `EquityOptionParams.notional` and the
  `EquityOptionParams::new`/`european_call`/`european_put` and
  `EquityOption::european_call`/`european_call_with_market_data` `notional: Money`
  argument (now `quantity: f64, currency: Currency`). Python
  `EquityOption.notional` getter, `EquityOptionBuilder.notional` and the
  `EquityOption.european_call(..., notional, ...)` argument (now `quantity` and
  `currency`). PV is unchanged; Rust/Python/JSON.
- `FxForward.spot_rate_override`, `FxFuture.spot_rate_override`, `Ndf.spot_rate_override`
  and `FxSpot.spot_rate` (now `quoted_spot`); `Ndf.forward_rate_override` (now
  `quoted_forward`); `FxSpot::with_rate` (now `with_quoted_spot`); Python
  `FxForward.spot_rate_override` getter/builder (now `quoted_spot`); Rust/Python/JSON.
- `QuantoOption.fx_rate_id` (now `fx_spot_id: Option<PriceId>`), `QuantoOption.fx_vol_id`
  (now `fx_vol_surface_id`), `QuantoOption.underlying_quantity` (now `quantity`);
  Rust/JSON.
- `EquityFutureQuantoSpec` deleted; `EquityFuture.quanto` is the shared
  `QuantoSpec` (`asset_currency`, `asset_discount_curve_id`, `correlation`,
  `fx_vol_surface_id`, `fx_spot_id`). `EquityFuture.discount_curve_id` is now the
  settlement (payoff) curve, the carry curve is `quanto.asset_discount_curve_id`
  (was `discount_curve_id`), `quanto.settlement_discount_curve_id` is gone, and
  `quanto.equity_vol_surface_id` moves to `EquityFuture.vol_surface_id`;
  `quanto.asset_currency` must equal `underlying_currency`; Rust/JSON.
- Commodity flattened `ticker` (now `underlying_ticker`) on CommodityForward,
  CommodityOption, CommodityAsianOption, CommoditySwap and CommoditySwaption
  (`CommodityUnderlyingParams.ticker`); `CommodityFuture.underlying` (now
  `underlying_ticker`); Rust/JSON.
- `CommoditySpreadOption.notional` and `CommoditySwaption.notional` (unit counts,
  now `quantity`); Rust/JSON.
- `CommodityForward.spot_id`, `CommodityOption.spot_id`, `FxVarianceSwap.spot_id`,
  `IndexUnderlyingParams.yield_id`/`duration_id` (FI TRS) and
  `CollateralSpec.market_value_id` are typed `PriceId` (wire unchanged); Rust.
- `VolatilityDependency.underlying_id` (now `spot_id`); Rust/JSON.
- Scenario market target `equity_price.price_id: CurveId` (now `spot_id: PriceId`);
  Rust/JSON.
- Rough-Bergomi market scalars `RBERGOMI_ETA`/`RBERGOMI_HURST`/`RBERGOMI_RHO`
  (now `ROUGH_BERGOMI_ETA`/`ROUGH_BERGOMI_HURST`/`ROUGH_BERGOMI_RHO`).
- `MarketDependencies::add_market_scalar_id` takes `impl AsRef<str>`; Rust.

### Rates projection and index ids (2026-09-24)

A rates projection curve and a commodity price curve are `forward_curve_id`,
the rate-index identity is `index_id: IndexId` (the convention-registry key),
the observed index tenor is `index_tenor`, the financing curve is
`repo_curve_id` (always a discount curve), and the CMS reference-swap fixed-leg
day count is `swap_fixed_day_count`.

#### Changed (BREAKING)

- `FloatingRateSpec.index_id` (now `forward_curve_id`) on every host (Bond FRN,
  ConvertibleBond, TermLoan, RevolvingCredit, StructuredCredit, AssetBackedFacility
  collateral), including the `FloatingRateSpec(...)` Python keyword/property;
  Rust/Python/WASM/JSON.
- `AssetBackedFacility.index_id`, `PoolAsset.index_id`, `RepLine.index_id` and
  `ReinvestmentAssumptions.index_id` (now `forward_curve_id`); Rust/Python/JSON.
- `Bond::floating`/`Bond.floating(...)` and `PoolAsset.floating_rate_loan(...)`
  parameter `index_id` (now `forward_curve_id`; WASM `indexId` now `forwardCurveId`).
- `Snowball.floating_index_id`, `Tarn.floating_index_id`, `CommoditySwap.floating_index_id`,
  `CommodityFuture.price_curve_id` and `RangeAccrual.projection_curve_id` (now
  `forward_curve_id`); Rust/JSON.
- `Snowball.floating_tenor`, `Tarn.floating_tenor` and `RangeAccrual.reference_tenor`
  (now `index_tenor`); Rust/JSON.
- `RangeAccrual.rate_index_id` and `InterestRateFuture.fixing_index_id` (now
  `index_id`; the IR future still falls back to `forward_curve_id` for its fixing
  series); Rust/JSON.
- `CmsSwap`/`CmsOption`/`CmsSpreadOption.swap_convention: Option<IRSConvention>`
  (now `index_id: Option<IndexId>`, any registered rate index such as
  `"USD-SOFR-OIS"`, `"CHF-SARON-OIS"`); `CmsSwap::from_schedule` takes an
  `IndexId`; Rust/JSON.
- `CmsSwap`/`CmsOption`/`CmsSpreadOption.swap_day_count` (now
  `swap_fixed_day_count`); Rust/JSON.
- `Bond.funding_curve_id` and `Instrument::funding_curve_id()` (now
  `repo_curve_id`); Rust/Python/JSON.
- `Swaption::get_day_count()` / `BermudanSwaption::get_day_count()` (now
  `get_fixed_day_count()`); Rust.
- `BondConvention::default_disc_curve()` (now `discount_curve_id()`); Rust.
- `DollarRoll.repo_curve_id` resolves a discount curve (was a forward curve).

#### Removed

- `IRSConvention` and the unused `resolve_reference_swap_convention`; select
  the reference-swap convention with the registry `index_id`.

#### Numbers change

- DollarRoll `roll_specialness`: the reference repo rate is now
  `(DF(front)/DF(back) − 1)/τ` from the `repo_curve_id` discount curve (τ on
  ACT/360), the same formula already used when no repo curve is set. A market
  that registered `repo_curve_id` as a forward curve must register it as a
  discount curve. Reference: closed form `(exp(r·τ) − 1)/τ` on a flat
  continuously compounded curve (`specialness_resolves_the_supplied_repo_curve`).

### Exotic payoff terms (2026-09-24)

Barrier and fixing levels are bare quotes in the underlying's price units; a
rebate is a total `Money` amount; historical fixings are `past_fixings`; a
single realized fixing is `observed_fixing`; a recorded barrier hit is
`observed_barrier_breached`; lookback monitoring is the contractual
`monitoring` convention; and barrier timing, direction and Asian averaging
each have one enum.

#### Changed (BREAKING)

- `FxBarrierOption.rebate` is now `Option<Money>`: the total trade rebate in
  the quote (settlement) currency, independent of notional. It was a per-unit
  `f64` (quote currency per base unit); migrate as
  `rebate = per_unit × notional.amount` in the quote currency. Validation now
  rejects a rebate in another currency or a negative rebate. Rust, JSON.
- `BarrierOption.barrier` and `.expiry_fixing` (now `f64` / `Option<f64>`
  quotes in `strike` units, were `Money`); `LookbackOption.expiry_fixing`,
  `.observed_min`, `.observed_max` (now `Option<f64>`, were `Option<Money>`).
  Rust, JSON.
- `FxTouchOption.barrier_level` (now `barrier`) and `.observed_touch` (now
  `observed_barrier_breached`). Rust, JSON.
- `Ndf.fixing_rate` (now `observed_fixing`) and `Ndf::with_fixing_rate` (now
  `with_observed_fixing`). Rust, JSON.
- `CommodityAsianOption.realized_fixings`, `CommoditySwap.realized_fixings` and
  `CommodityFutureFixing::ArithmeticAverage.realized_fixings` (now
  `past_fixings`); `AsianOption::validate_realized_fixings` and
  `CommodityAsianOption::validate_realized_fixings` (now
  `validate_past_fixings`). Rust, JSON.
- `RangeAccrual.past_fixings_in_range` (now `past_observations_in_range`,
  matching `total_past_observations`; also on `CallableRangeAccrual.range_accrual`).
  Rust, JSON.
- `LookbackOption.use_gobet_miri` (now `monitoring: Monitoring`, default
  `continuous`). `discrete` carries the contractual `observation_dates`, prices
  by Monte Carlo and observes the extremum only on those dates; the analytical
  engine rejects it. Rust, JSON.
- `Autocallable.participation_rate` removed; the rate lives once in the payoff
  variant: `FinalPayoffType::CapitalProtection { floor, participation_rate }`
  and `FinalPayoffType::Participation { participation_rate }` (was `rate`).
  Rust, JSON.
- `CmsSpreadOption.spread_correlation` (now `correlation`, a bounded
  `CorrelationWire` on the wire). Rust, JSON.
- `finstack_quant_models::closed_form::barrier::RebateTiming` (now
  `finstack_quant_core::types::PayoutTiming`, shared by `rebate_timing` and
  `FxTouchOption.payout_timing`); `BarrierDirection` now lives once in
  `finstack_quant_core::types` (the models bridge copy is removed) and
  `BarrierType::direction()` returns it; `AveragingMethod` now lives once in
  `finstack_quant_models::monte_carlo::payoff::asian` (serde + JsonSchema) and
  is re-exported by valuations. Wire strings are unchanged; the schema
  `$defs/RebateTiming` becomes `PayoutTiming`. Rust.

#### Added

- QuantLib goldens `eurusd_up_out_call_rebate_at_hit_3m_quantlib` and
  `eurusd_up_in_call_rebate_at_expiry_3m_quantlib` for the FX barrier rebate
  (Reiner-Rubinstein, abs 1e-7).

### Credit family (2026-09-24)

`credit_curve_id` always names a `HazardCurve`, including on
`ConvertibleBond`; the CDS-family running coupon is `coupon_bp`; the only
contractual upfront is the dated `upfront` field; settled index loss is
`realized_loss`; the CDS roll grid is `roll_rule` (`cds_imm` or `none`) with
the tranche `stub` on the instrument; and structured-credit tranche bounds are
`attach_pct`/`detach_pct`.

#### Changed (BREAKING)

- `PremiumLegSpec.spread_bp` (now `coupon_bp`) and
  `PremiumLegSpec.standard_imm_dates` (now `roll_rule: RollRule`, required:
  `cds_imm` for the standard 20th-of-quarter grid, `none` for a bespoke
  frequency/stub schedule; `imm` is rejected at pricing). Rust, Python
  (`PremiumLegSpec(..., coupon_bp, ..., roll_rule=RollRule.CDS_IMM)`, getters
  `coupon_bp`/`roll_rule`), JSON.
- `CDSTranche.running_coupon_bp` (now `coupon_bp`), `.accumulated_loss` (now
  `realized_loss`) and `.standard_imm_dates` (now `roll_rule`, default `none`;
  `imm` is rejected by `validate`); new `CDSTranche.stub` (default
  `short_front`). `CDSTranche::new` now honours `ScheduleParams.stub` and
  `.roll_rule` instead of dropping them. `CDSTrancheParams.running_coupon_bp`
  and `.accumulated_loss` (now `coupon_bp`/`realized_loss`),
  `CDSTrancheParams::with_accumulated_loss` (now `with_realized_loss`). Rust,
  Python (`CDSTrancheBuilder.roll_rule`/`.stub`), JSON.
- `CDSTranche::upfront(curves, as_of)` (now `model_upfront`),
  `CDSTranchePricer::calculate_upfront` (now `calculate_model_upfront`) and the
  tranche metric key `upfront` (now `model_upfront`). Rust, metric key.
- `CDSIndexParams.fixed_coupon_bp` (now `coupon_bp`). Rust, Python.
- `CDSOption.underlying_cds_coupon` (a decimal rate) is now `coupon_bp` in
  basis points (`"0.01"` becomes `"100"`); `CDSOptionParams::with_underlying_cds_coupon`
  (now `with_coupon_bp`); `CDSOption.realized_index_loss: Option<f64>` (now
  `realized_loss: f64`, default `0.0`); `CDSOption.index_factor` and
  `CDSOptionParams.index_factor` are `f64` with default `1.0` instead of
  `Option<f64>`. Rust, JSON.
- `CDSOption` recovery validation now uses the shared credit domain `[0, 1)`
  (zero recovery is accepted). Rust.
- `CreditDefaultSwap::get_par_spread` (now `par_spread`). Rust, Python.
- `CDSPricerConfig.include_accrual` and
  `CDSTranchePricerConfig.accrual_on_default_enabled` (now
  `include_accrual_on_default`). Rust.
- `CdsQuote::CdsUpfront.running_spread_bp` and `CdsTrancheQuote.running_spread_bp`
  (now `coupon_bp`); `CdsQuote::quoted_running_spread_bp` (now `coupon_bp`);
  the Python calibration quote getter `running_spread_bp` (now `coupon_bp`).
  Rust, Python, JSON.
- `BaseCorrelationParams.use_imm_dates` and `CDSTrancheBuildOverrides.use_imm_dates`
  (now `roll_rule`, default `none`). Rust, JSON.
- `InstrumentCashflowEnvelope.hazard_curve_id` (now `credit_curve_id`) and
  `CashFlowSchedule::pv_by_period(hazard_curve_id)` (now `credit_curve_id`).
  Rust, Python, JSON output. `docs/SERDE_STABILITY.md` and `docs/CONTRACTS.md`
  now reserve `credit_curve_id` for every hazard-curve reference.
- Structured-credit `Tranche.attachment_point`/`.detachment_point` (now
  `attach_pct`/`detach_pct`), `Tranche::attachment_pct()`/`detachment_pct()`
  (now `effective_attach_pct()`/`effective_detach_pct()`),
  `TrancheBuilder::attachment_detachment` (now `attach_detach`), and
  `TranchePricingResult.attachment`/`.detachment` (fractions documented as
  percent; now `attach_pct`/`detach_pct` in percent, as are the stochastic
  result `to_dataframe()` columns). Rust, Python, JSON.
- `CreditDefaultSwap::new_isda(credit_id)` (now `credit_curve_id`). Rust.
- `HazardCurve` market bumps now accept triangular key-rate specs: each
  knot's hazard shift is the spread shift `/ (1 − R)` weighted by the bucket's
  triangular weight, so bucket shifts sum to the parallel shift. Rust.

#### Added

- `CDSIndex.upfront: Option<(Date, Money)>`, the contractual dated upfront
  (discounted from its payment date on the premium discount curve, applied
  once at the index level). Rust, Python (`CDSIndex.upfront`,
  `CDSIndexBuilder.upfront`), JSON.

#### Removed

- `MarketQuoteOverrides.upfront_payment` and
  `InstrumentPricingOverrides::with_upfront`: the already-discounted PV
  adjustment that CDS, CDS index and CDS tranche pricers summed on top of the
  dated `upfront`. Use `upfront: Some((as_of, amount))` for an amount paid on
  the valuation date. Rust, JSON.
- `CDSTranchePricerConfig.schedule_stub` and `.use_isda_coupon_dates`
  (now `CDSTranche.stub` and `CDSTranche.roll_rule`). Rust.

#### Changed (BREAKING): ConvertibleBond hazard semantics

- `ConvertibleBond.credit_curve_id` now names the issuer `HazardCurve`
  instead of a zero-recovery risky `DiscountCurve`. The cash component's
  zero-recovery risky forward is `rf_fwd × S(t_{i+1}) / S(t_i)`, blended with
  the risk-free forward by `recovery_rate` as before. The credit curve is a
  credit dependency only (no longer also a discount dependency).
- Convertible CS01/bucketed CS01 shock the hazard curve by 1 bp of spread
  (hazard shift `1bp / (1 − R_curve)`); without a credit curve the z-spread
  fallback uses a zero-hazard, zero-recovery synthetic curve and a forward
  difference. Convertible OAS shifts every hazard rate by the spread.
- Attribution `MarketSnapshot.credit_discount_curves` and `.credit_curve_ids`
  are removed; convertible credit P&L is measured on the hazard curve.

#### Numbers change

- ConvertibleBond PV, DV01, CS01 and OAS with a credit curve: the regression
  goldens `conv_bond_atm_3y` and `conv_bond_distressed` now reference the
  issuer hazard curve `ACME-HZD` (recovery 0 on the instrument) instead of the
  `USD-CORP` risky discount curve, and DV01 now reaches the PV through the
  risk-free `USD-OIS` curve. A hazard curve equivalent to the former risky
  curve reproduces the former PV and CS01 to 1e-10 relative
  (`convertible_hazard_equals_equivalent_risky_curve`, pinned on
  ed2d9f2f3); full recovery equals risk-free pricing; a deep
  out-of-the-money zero-recovery convertible converges to QuantLib's
  `usd_fixed_5y_hazard` NPV 95.3527 (2.3e-12 at 2000 steps).
- Convertible CS01 without a credit curve is now a forward difference
  (`O(bump²)` from the former central value).

### Listed futures (2026-09-24)

Interest-rate, bond and volatility-index futures now carry the shared
`ListedFutureTerms` (`terms`) like every other listed future: position size is
`terms.contracts` × `terms.multiplier`, the trade price is `terms.entry_price`,
the optional live mark is `terms.quoted_price`, the lifecycle dates are
`terms.last_trading_date` / `terms.settlement_date` and the official final
settlement is `terms.settlement_price`. The model futures price is `fair_price`
on every listed future. Fixtures were migrated with `contracts = notional /
face` and `multiplier = tick_value / tick_size` (IR), `face / 100` (bond) or
`contracts = notional / (multiplier × entry_price)` (VIX), so no numbers change
beyond float re-association.

#### Changed (BREAKING)

- `InterestRateFuture.notional`, `.expiry`, `.quoted_price`,
  `.settlement_price` and `.position` (now `terms: ListedFutureTerms`:
  `contracts = notional / contract_specs.face_value`, `multiplier =
  tick_value / tick_size` — $2,500 for CME SR3 —, `entry_price`,
  `last_trading_date`, `settlement_date` — the last trading date for term
  contracts, the reference-period end for overnight contracts —,
  `settlement_price`, `position`). `fixing_date` defaults to
  `terms.last_trading_date`; `implied_rate()` reads `terms.entry_price`.
  Rust, JSON.
- `FutureContractSpecs.tick_value` (removed; the value of a price point is
  `terms.multiplier`). Rust, JSON.
- `InterestRateFuture::mark_price` now resolves through the listed-future
  lifecycle (live `terms.quoted_price`, else the new
  `InterestRateFuture::fair_price`, `terms.settlement_price` after the last
  trading date). Rust.
- `BondFuture.notional`, `.expiry`, `.delivery_end`, `.quoted_price` and
  `.position` (now `terms: ListedFutureTerms` with `multiplier = contract face
  / 100`, `last_trading_date`, `settlement_date` = last delivery date);
  `delivery_start` stays. The official `terms.settlement_price` is required
  after the last trading date. Rust, JSON.
- `BondFutureSpecs.contract_size` (removed; now `terms.multiplier × 100`).
  Rust, JSON.
- `BondFuturePricer::calculate_model_price` (now `fair_price`); new
  `BondFuture::fair_price` and `BondFuture::mark_price`; the `futures_price`
  metric is the lifecycle mark. Rust.
- `VolatilityIndexFuture.notional`, `.expiry`, `.settlement_date`,
  `.quoted_price`, `.position` and `.contract_specs` (now `terms:
  ListedFutureTerms`), `.settlement_fixing` (now `terms.settlement_price`,
  required on the settlement date), `::forward_vol(context)` (now
  `fair_price(context, as_of)`), `::delta_vol()` now returns `Result<f64>`.
  Rust, JSON.
- `CommodityFuture::model_settlement_price` (now `fair_price`). Rust.
- `CommodityForward.quoted_price` (now `quoted_forward`). Rust, JSON.
- `FxFuture.quote_currency` (removed from the wire; the quote and
  variation-margin currency is `terms.currency`, read through
  `FxFuture::quote_currency()`). Rust, JSON.

#### Removed

- `VolIndexContractSpecs` with `vix()`, `mini_vix()` and `vstoxx()`, and the
  volatility-index-future section of the embedded contract-spec registry
  (`terms.multiplier` carries the contract multiplier). Rust, JSON.
- Unused `instruments::ContractSpec`. Rust.
- `VolatilityIndexFuture::num_contracts` (now `terms.contracts`). Rust.

### Settlement and payment timing (2026-09-24)

`settlement_date` is a date, `settlement` is only the delivery method, the T+N
lag is `settlement_days: u32`, the business-day payment lag is
`payment_lag_days`, the agency MBS calendar-day delay is `stated_delay_days`,
and an accessor that resolves a default is named `effective_*`. No numbers
change.

#### Changed (BREAKING)

- `FxSpot.settlement` (now `settlement_date`), `FxSpot::with_settlement` (now
  `with_settlement_date`) and `FxSpot.settlement_lag_days: Option<i32>` (now
  `settlement_days: Option<u32>`, with `with_settlement_days(u32)`). Negative
  (T-N) lags are no longer accepted. Rust, JSON.
- `CommodityFuture.settlement` (now `fixing`) and `CommodityFutureSettlement`
  (now `CommodityFutureFixing`, variants unchanged); the cash/physical method
  stays at `terms.settlement`. Rust, JSON.
- `CommodityForward.settlement_lag_days` (now `settlement_days`) and
  `CommodityForward::effective_settlement_lag` (now
  `effective_settlement_days`). Rust, JSON.
- `Deposit.spot_lag_days: Option<i32>` (now `settlement_days: Option<u32>`).
  Rust, JSON.
- `CDSOption.cash_settlement_date` (now `premium_settlement_date`) and
  `CDSOption::effective_cash_settlement_date` (now
  `effective_premium_settlement_date`). Rust, JSON.
- CapFloor `OvernightCouponConvention.payment_delay_days` (now
  `payment_lag_days`). Rust, Python docs, JSON.
- core `ScheduleSpec.payment_lag_business_days` and
  `ScheduleBuilder::payment_lag_business_days` (now `payment_lag_days`). Rust,
  Python `ScheduleBuilder.payment_lag_days`, JSON.
- `AgencyMbsPassthrough.payment_lag_days` (now `stated_delay_days`, calendar
  days from accrual start), `AgencyProgram::payment_lag_days` (now
  `stated_delay_days`) and `AgencyMbsPassthrough::effective_payment_delay` (now
  `effective_stated_delay_days`). Rust, JSON (agency MBS, TBA and CMO
  payloads).
- core `FxPairConvention.spot_lag_days` (now `settlement_days`; Python
  `settlement_days`, WASM `settlementDays`) and
  `dates::fx::fx_standard_spot_lag_days` (now `fx_standard_settlement_days`).
  Rust, Python, WASM.
- `FxForward::standard_spot_days` (now `standard_settlement_days`), and the
  `FxForward`/`FxSwap`/`Ndf::from_trade_date` argument `spot_lag_days: i32`
  (now `settlement_days: u32`; Python keyword `settlement_days`). Rust, Python.
- `XccyConventions.spot_lag_days: i32` (now `settlement_days: u32`, also the
  `xccy_conventions.json` registry key and the Python getter). Rust, Python,
  JSON.
- `AgencyTba::get_settlement_date` (now `effective_settlement_date`) and
  `DollarRoll::front_settle_date` / `back_settle_date` (now
  `effective_front_settlement_date` / `effective_back_settlement_date`). Rust.
- `DollarRoll::settlement_days()` and `CarryResult.settlement_days` (now
  `roll_days`: calendar days between the front and back settlement dates).
  Rust.
- `CDSTranchePricerConfig.index_settlement_lag` / `bespoke_settlement_lag`
  (now `index_settlement_days` / `bespoke_settlement_days`). Rust.

#### Removed

- `tba::TbaSettlement` (unused) and the inherent
  `AgencyTba::settlement_month(year, month)` helper; the builder's
  `settlement_year` / `settlement_month` setters remain. Rust.

### Pool statistics (2026-09-24)

Collateral pool statistics use the market acronym with a unit suffix unless
the value is a decimal or in years (`wac`, `wal`, `weighted_avg_spread_bp`,
`wam_months`, `speed_multiplier`), and the agency pass-through coupon and
factor share their siblings' names.

#### Changed (BREAKING)

- `AssetPool::weighted_avg_coupon()` / `PoolStats.weighted_avg_coupon` (now
  `wac()` / `wac`) and `PeriodDiagnostics.weighted_avg_coupon` (now `wac`,
  also the Python `SimulationDiagnostics` period record key and DataFrame
  column). Rust, Python, JSON.
- `AssetPool::weighted_avg_spread()` / `PoolStats.weighted_avg_spread` (now
  `weighted_avg_spread_bp()` / `weighted_avg_spread_bp`). Rust.
- `PoolStats.cumulative_default_rate` (now `defaulted_balance_pct`, percent
  points of the current balance; `DefaultModelSpec::timing`'s decimal
  `cumulative_default_rate` is unchanged). Rust.
- `StructuredCredit::calculate_prepayment_rate` (now `calculate_smm`; it
  returns the monthly SMM). Rust.
- `TranchePricingResult.average_life` / `with_average_life` (now `wal` /
  `with_wal`), also the stochastic `tranche_results[].wal` result key and the
  Python tranche DataFrame column. Rust, Python, WASM result schema, JSON.
- `AssetPool::weighted_avg_life_from_cashflows` (now `wal_from_cashflows`).
  Rust.
- `AgencyMbsPassthrough.pass_through_rate` (now `coupon`) and `.wam` (now
  `wam_months`); `AgencyCmo.collateral_wam` (now `collateral_wam_months`).
  Rust, JSON (agency MBS, TBA, dollar-roll and CMO payloads).
- `PacCollar.lower_psa` / `upper_psa` (now `lower_speed_multiplier` /
  `upper_speed_multiplier`, `1.0` = 100% PSA). Rust, JSON.
- The embedded TBA and structured-credit CMO assumption registries rename
  `psa_multiplier` to `speed_multiplier`; `psa_to_cpr(psa_speed, ..)` takes
  `speed_multiplier`. Rust, embedded JSON.
- Internal seasoning names converge on `seasoning_months`
  (`PacSchedule::generate(collateral_age_months)`,
  `ScenarioTreeConfig.initial_seasoning`, `AssetPool` weighted seasoning).
  Rust.

#### Removed

- `AgencyMbsPassthrough.current_factor`: the pool factor is now derived by
  `AgencyMbsPassthrough::factor()` (`current_face / original_face`), so it
  cannot disagree with the faces. Rust, JSON.
- The always-zero `PoolStats.weighted_avg_rating_factor`, `recovery_rate` and
  `prepayment_rate` stubs (use the `clo_warf` / `cpr` metrics). Rust.
- `MetricId::CloWac` (`clo_wac`), which had no calculator. Rust, Python,
  WASM metric metadata.

#### Fixed

- `clo_was` is reported in the basis-points unit group (it was tagged
  Decimal while its value is in bp). Metric metadata only.

#### Numbers change

- `AssetPool::wac()` (and so `PoolStats.wac`) is now the indenture WAC: the
  balance-weighted coupon of the performing, positive-balance, fixed-rate
  collateral, the same population as the simulated `PeriodDiagnostics.wac`.
  It previously averaged every row, including defaulted assets and floating
  rows at their spread, over the full pool balance. Example: 10M at 6%, 20M
  at 9%, a 30M floater at 400 bp and a defaulted 5M at 12% gave ≈6.462%; it
  is now 8.000% (hand WAC of the two performing fixed rows). Reference: the
  hand calculation, and bit-equality with `PeriodDiagnostics.wac` on the
  closing state. The stochastic RMBS refinancing incentive reads this WAC, so
  stochastic RMBS prepayment moves only for pools with defaulted or floating
  collateral; no pinned value in the test suite or goldens moved.

### `_pct`/`_decimal`/`_bp` pass (2026-09-24)

`_pct` now means percent points only (`100.0` = 100%). Decimal-valued fields
carry `_decimal`, a ratio noun (`share`, `fraction`, `ratio`) or `_rate`, and
basis-point values carry `_bp`. Where a field changed scale its input is
converted once, so the documented equivalent input prices bit-for-bit as
before.

#### Changed (BREAKING)

- `BumpConfig.spot_bump_pct` / `vol_bump_pct` (now `spot_bump_decimal` /
  `vol_bump_decimal`), also the `valuations.sensitivities.v1` extension keys
  and `GreekBumps` fields. Rust, Python, WASM, JSON.
- `ScenarioPricingOverrides.scenario_price_shock_pct` (now
  `scenario_price_shock_decimal`) and `with_price_shock_pct` (now
  `with_scenario_price_shock_decimal`); the scenario adapter's
  `attributes.meta` key follows. Rust, Python, WASM, JSON.
- `BermudanCallProvision.call_price` (now `price_pct_of_par`, percent of par:
  `100.0` = par, was the fraction `1.0`). CallableRangeAccrual pays
  `notional * price_pct_of_par / 100`. Rust, JSON, WASM test fixture.
- `Snowball.callable` (now `call_provision`). Rust, JSON.
- `AgencyMbsPassthrough.servicing_fee_rate` / `guarantee_fee_rate` (now
  `servicing_fee_bp` / `guarantee_fee_bp`, annual bp: `25.0` was `0.0025`);
  the embedded TBA and structured-credit CMO assumption registries rename
  `servicing_fee_rate`, `guarantee_fee_rate`, `agency_guarantee_fee_rate` and
  `gnma_guarantee_fee_rate` to `*_bp` with bp values. Rust, JSON.
- `EquityTotalReturnFuture.spread_basis_points_id` (now `spread_bp_id`). Rust,
  JSON.
- `CouponType::Split { cash_pct, pik_pct }` (now
  `{ cash_fraction, pik_fraction }`) and Python `CouponType.split(cash_pct,
  pik_pct)` / `.cash_pct` / `.pik_pct` (now `cash_fraction` / `pik_fraction`).
  Rust, Python, JSON.
- `AmortizationEvent::CumulativeLoss { max_pct }` (now
  `{ max_cumulative_loss }`, a decimal fraction of the original collateral in
  `(0, 1]`: `0.04` was `4.0`) and `ExcessSpread { min_3m }` (now
  `{ min_excess_spread_3m }`) on AssetBackedFacility; Python
  `AmortizationEvent.cumulative_loss(max_cumulative_loss)` /
  `.max_cumulative_loss` / `.excess_spread(min_excess_spread_3m)` /
  `.min_excess_spread_3m`. Rust, Python, JSON.
- `FinalPayoffType::KnockInPut { strike }` (now `{ strike_ratio }`, a ratio of
  the initial level like `final_barrier`: `1.0` was `strike = initial_level`).
  `FinalPayoffType` now denies unknown fields. Rust, JSON.
- Structured credit decimal fields: `cleanup_call_pct` (now
  `cleanup_call_decimal`, Python builder/getter too), `IncentiveFeeSpec.share_pct`
  (now `share`), `CardPortfolioSpec.fixed_allocation_pct` (now
  `fixed_allocation_decimal`, `with_fixed_allocation_decimal`),
  `ShiftingInterestStep.senior_pct` (now `senior_decimal`),
  `TargetOcSpec.pct_of_current` / `floor_pct_of_original` (now
  `fraction_of_current` / `floor_fraction_of_original`),
  `ExcessSpreadSpec.trap_loss_pct` (now `trap_loss_decimal`) and
  `ReserveTarget::PctOfCurrent` / `PctOfOriginal` (now `FractionOfCurrent` /
  `FractionOfOriginal`, wire `fraction_of_current` / `fraction_of_original`).
  Rust, Python, JSON.
- `ReinvestmentCriteria.max_price` (now `max_price_pct`, percent of par; the
  value is unchanged). Rust, Python docs, JSON.
- `RealEstateAsset.disposition_cost_pct` (now `disposition_cost_decimal`) and
  `ClawbackSpec.holdback_pct` (now `holdback_decimal`). Rust, JSON.
- `UpfrontFee::PctOfCommitment` (now `FractionOfCommitment`, wire
  `fraction_of_commitment`). Rust, Python, JSON.
- `OidPolicy::WithheldPct` / `SeparatePct` (now `WithheldBp` / `SeparateBp`,
  wire `withheld_bp` / `separate_bp`; the value was already basis points).
  Rust, JSON.
- Merton MC `PathStatistics.avg_recovery_pct` (now `avg_recovery_rate`, a
  decimal fraction). Rust, Python.

#### Numbers change

- Only the input scale moves: `price_pct_of_par` is percent (was fraction),
  MBS/TBA/CMO servicing and guarantee fees are bp (were decimals),
  `max_cumulative_loss` is a fraction (was percent), and `strike_ratio` is a
  ratio of the initial level (was an absolute price, divided by the initial
  level in the payoff). Each is converted once at the point of use, so the
  converted input reproduces the old result bit-for-bit: the CallableRangeAccrual
  PV at `102.0` equals the pinned PV at the retired `1.02`, the MBS coupon
  identity at 25 bp equals the retired `0.0025`, and the ABF engine threshold
  at `0.005` equals the retired `0.5 / 100`. A knock-in put strike on an
  underlying whose initial level is not 100 now reads as a ratio, which is
  checked against the term-sheet payoff `1 - max(K/S0 - S_T/S0, 0)`.

### Sensitivity units (2026-09-24)

Every `<factor>01` sensitivity and EquityOption rho now report the currency
change per stated bump, with the same unit in the Rust accessor, the `MetricId`
and the bindings.

#### Changed (BREAKING)

- `EquityOption::rho()` and `EquityOptionGreeks.rho` (now currency per 1bp,
  was per 1%). Rust only; the `rho` metric and Python/WASM `EquityOption.rho()`
  were already per 1bp and do not move.
- `MetricId::Recovery01` key `recovery_01` (now `recovery01`), with unit
  `MetricUnit::Currency` (was `Decimal`) and the doc "PV change per +1% (0.01
  absolute) recovery-rate move". Rust, Python, WASM and JSON result keys
  (`measures`, golden `expected`); `recovery_01` is rejected by
  `MetricId::parse_strict`.
- `MetricGroup` ranges corrected: `theta_gamma` and `variance_vega` are Greeks,
  `default_probability` and `recovery01` are Credit, and the Rates, FX, Equity,
  Structured Credit and Alternatives groups now start at their first member
  (each boundary was two metrics early). Rust, Python and WASM group listings.
- `real_estate::cap_rate_sensitivity` (now `real_estate::cap_rate01`, currency
  per 1bp cap-rate move from the resolved `rate_bump_bp`) on `RealEstateAsset`
  and `LeveredRealEstateEquity`. Rust metric key.
- `real_estate::discount_rate01` is now also registered on
  `LeveredRealEstateEquity` (asset discount-rate sensitivity; financing PV does
  not depend on that rate). Rust metric key.
- `constituent_delta` (and each `constituent_delta::<label>` bucket) is now the
  basket PV change per +1% relative constituent price move (was the PV change
  per unit relative move, 100× larger), documented on
  `MetricId::ConstituentDelta`. Rust, Python, WASM.

#### Removed

- `real_estate::discount_rate_sensitivity`: it was `discount_rate01` × 10,000.
  Use `real_estate::discount_rate01`. Rust metric key.

#### Fixed

- Formula docs for Severity01, Default01, Prepayment01, Conversion01,
  CollateralHaircut01 and CollateralPrice01 now show the per-bump value the code
  returns, `(PV_up − PV_down) / width × bump`, not a per-unit derivative.
- Real-estate README: it said cap-rate sensitivity is zero when `sale_price` is
  set. The metric has always returned a validation error (not applicable) for a
  DCF asset with an explicit `sale_price` or no `terminal_cap_rate`, on both
  `RealEstateAsset` and `LeveredRealEstateEquity`; `cap_rate01` keeps that
  behaviour, the README now says so, and a test pins it. Docs only.

#### Numbers change

- `EquityOption::rho()` / `EquityOptionGreeks.rho`: ÷100 (per 1% → per 1bp).
  Reference: QuantLib `spx_atm_call_1y` rho 0.4803947 (golden tolerance 1e-7);
  the accessor now equals the `rho` metric bit for bit.
- `real_estate::cap_rate01` = old `cap_rate_sensitivity` × 1e-4 at the default
  1bp bump. Reference: DirectCap closed form `−NOI / cap² × 1bp` (1e-5 relative,
  the O(h²) central-difference residual).
- `constituent_delta`: ÷100. Reference: a units basket is linear in each price,
  so a 1% move changes PV by exactly `0.01 × units × price` (1e-9).
- The Bloomberg `cds_5y_par_spread` Rec Risk (1%) of 76.03 is unchanged; only
  its key is renamed.

### Python kwargs for metric overrides (2026-09-24)

Binding-layer only; no JSON wire field or number changes. `pricing_options`
now names only the Rust `PricingOptions` service bundle.

#### Changed (BREAKING)

- `price_instrument(pricing_options=...)`, every typed `<Instrument>.price(pricing_options=...)`
  and `validate_instrument_json(json, pricing_options=...)` (now
  `metric_pricing_overrides`). Python; passing `pricing_options` raises
  `TypeError`.
- WASM `priceInstrument`, `priceInstrumentWithMarket`, `validateInstrumentJson`
  and the typed FX `.price` parameter `pricingOptions` (now
  `metricPricingOverrides`). WASM `index.d.ts`.
- Rust `pricer::validate_instrument_json` / `parse_boxed_instrument_from_json`
  parameter `pricing_options` (now `metric_pricing_overrides`); the parse error
  reads `invalid metric_pricing_overrides JSON` (was `invalid pricing options JSON`).
  Rust, Python and WASM error text.
- Python typed `.price()` and `validate_instrument_json` now accept
  `MetricPricingOverrides | dict | str | None` for `metric_pricing_overrides`,
  and typed `.price()` accepts `MarketHistory | dict | str | None` for
  `market_history` (was a JSON `str` only), matching `price_instrument`.
- UI `PriceRequest.pricingOptions` and `PricingParams.pricingOptions` (now
  `metricPricingOverrides`), matching the facade parameter. finstack-quant-ui
  registry contract, workbench and pricing-params form, the
  `tests/valuations/instruments/pricing-cases.json` request fixtures and the
  gallery examples.

#### Removed

- The duplicate Python binding helpers `pricing_options_json` (two copies) and
  `price_envelope`; every Python pricing entry point goes through one
  `metric_pricing_overrides_json` coercion and one `price_typed_envelope` path.

### Instrument fields that duplicate overrides (2026-09-24)

Numbers do not change for inputs that were already canonical. They change
only where a previously ignored or hidden setting is now honoured, as noted
below.

#### Changed (BREAKING)

- `Bond.forward_curve_id` (now `instrument_pricing_overrides.model_config.asw_forward_curve_id`
  only): the asset-swap forward curve has one channel. Rust, Python
  (`Bond.forward_curve_id` getter and `BondBuilder.forward_curve_id` removed),
  JSON (the key is rejected).
- `ConvertibleTreeType::Binomial(steps)` / `Trinomial(steps)` (now payload-free
  `Binomial` / `Trinomial`; the step count is
  `instrument_pricing_overrides.model_config.tree_steps`, default
  `DEFAULT_CONVERTIBLE_TREE_STEPS` = 200). Rust. `model_config.tree_steps` on a
  ConvertibleBond now sets the lattice for pricing, the bond floor, greeks,
  implied vol and OAS; before, it was validated and ignored.
- `EquityOption.theta_day_basis` (now `metric_pricing_overrides.theta_day_basis:
  Option<ThetaDayBasis>`, default calendar-365). `ThetaDayBasis` moves to
  `instruments::ThetaDayBasis`. Rust, Python (`EquityOption.theta_day_basis`
  getter and builder setter removed; `MetricPricingOverrides(theta_day_basis=...)`
  and its getter added), JSON.
- `metric_pricing_overrides.theta_period` (now `Option<Tenor>`): the wire form
  is `{"count": 1, "unit": "weeks"}`, not `"1W"`; a zero-length horizon is
  rejected. Rust (`with_theta_period(Tenor)`), Python
  (`MetricPricingOverrides(theta_period=Tenor | str)`, getter returns `Tenor`),
  WASM/JSON `pricing_options`. Metrics-based attribution no longer sets a
  horizon for a zero-day window (carry is zero there).
- `model_config.cds_aod_half_day_bias` / `model_config.cds_act360_include_last_day`
  (now implied by `CreditDefaultSwap.valuation_convention = "quant_lib_isda_parity"`,
  via `CdsValuationConvention::aod_half_day_bias` / `act360_includes_last_day`).
  Rust, JSON (the keys are rejected).
- `bs_greeks(theta_days=...)` (now `theta_days_per_year`, matching Rust).
  Python kwarg, WASM `thetaDays` (now `thetaDaysPerYear`).

#### Removed

- `VarianceSwap::annualization_factor_with_policy` and the
  `"{underlying_ticker}_TRADING_DAYS_PER_YEAR"` / `"TRADING_DAYS_PER_YEAR"`
  market scalars. The annualisation factor is now the contract field
  `trading_days_per_year` (below). Rust.
- `ParSpreadMethod` and `IndexParSpreadResult.method`: the CDS index par spread
  always uses the risky-annuity denominator, reported through
  `IndexParSpreadResult.denominator`. Rust.

#### Fixed

- `VarianceSwap.trading_days_per_year` and `FxVarianceSwap.trading_days_per_year`
  (new f64 contract field, default 252, schema-visible): daily realized
  variance annualises by it. `annualization_factor()` and PV now agree; the FX
  swap was hard-coded to 252. Rust, JSON.
- FxOption and FxDigitalOption analytic theta honour
  `metric_pricing_overrides.theta_day_basis` instead of a hard-coded 365
  (default unchanged).

### Override containers (2026-09-24)

Numbers do not change by default. They change only for a composite whose
`metric_pricing_overrides` is set: those settings now reach its legs as
defaults (reference: each leg priced standalone with the merged overrides,
exact match in `composite_metric_overrides_are_leg_defaults_and_leg_settings_win`).
Before this change they were never read.

#### Changed (BREAKING)

- `CallableRangeAccrual.range_accrual` (now `RangeAccrualTerms`): the nested
  range accrual carries contract terms only. The callable note has one `id`,
  one `attributes` map and one set of pricing overrides, all at its top level.
  Rust, Python/WASM JSON.
- `RangeAccrual` (now `RangeAccrual.terms: RangeAccrualTerms`, serialized flat):
  the RangeAccrual wire shape is unchanged. In Rust, term fields move to
  `.terms`, `RangeAccrual::builder().terms(RangeAccrualTerms::builder()...build()?)`
  replaces the flat term setters, and `accrual_year_fraction`,
  `effective_lower_bound`, `effective_upper_bound` and `coupon_rate_rate`
  (on `RangeAccrualTermsBuilder`) move to the terms.
- `CompositeSpec.instrument_pricing_overrides` / `metric_pricing_overrides` /
  `scenario_pricing_overrides` (now on `CompositeInstrument`, wire path
  `instrument.spec.*_pricing_overrides` like every other instrument). Rust,
  JSON. The `pricing_options` merge, `validate_instrument_json` and the WASM
  `pricing_options` path now reach composites. Rebalancing keeps the prior
  instrument's overrides.
- A composite's `metric_pricing_overrides` apply to every leg (and nested
  composite) as defaults. A leg's own setting wins, and
  `bump_config.adaptive_bumps` is on for a leg when either side enables it.
  This also carries attribution's theta window to composite legs.
- A non-empty composite `instrument_pricing_overrides` is now a validation
  error. A composite has no pricing model of its own, so set quotes and model
  inputs on the legs.
- The override fields share one description on every instrument:
  "Instrument-owned pricing inputs.", "Metric-time pricing configuration." and
  "Scenario-only pricing adjustments." (JSON-schema descriptions). The future
  options keep their tree-step wording, and the composite adds its leg-default
  and must-be-empty notes.

#### Fixed

- `XccySwap` gains `instrument_pricing_overrides`, `metric_pricing_overrides`
  and `scenario_pricing_overrides` (Rust, JSON). Scenario price shocks,
  bump sizes, theta period and VaR settings now apply to cross-currency swaps.
  Empty containers are omitted, so existing XccySwap JSON is unchanged.
- `AssetBackedFacility` omits empty override containers when serialized, like
  every other instrument (JSON).
- The `RangeAccrual` docs no longer suggest an empty call schedule for a
  non-callable rate-linked note, which `BermudanCallProvision` rejects.

### Structured-credit input channels (2026-09-24)

Numbers change only where a deal relied on a retired channel:

- `StructuredCredit.credit_model` is now the only channel for prepayment,
  default and recovery assumptions. A deal that set the retired overrides
  must state the same values on `credit_model`; doing so reproduces the old
  value bit for bit (`production_structured_credit_model_reproduces_retired_override_value`,
  pinned at `f64::to_bits` from the pre-change override path). The two
  structured-credit regression goldens that set every override were rewritten
  onto `credit_model` with all expected values unchanged.
- The reinvestment purchase price is `pool.reinvestment_period.assumptions.price_pct`
  only. A deal that set the retired `reinvestment_price` without
  `assumptions` now replicates surviving collateral at par (100.0).
- CMBS DSCR (`MetricId::CmbsDscr`) reads loan-level `pool.assets[].noi` only.
  A pool with no NOI on a performing loan is now a validation error instead
  of falling back to the deal-level NOI and user-supplied debt service, which
  the metric silently replaced whenever any loan carried NOI.
- Obligor industry is `pool.assets[].industry` only. Industry written inside
  `asset_type` was never read by concentration limits or the diversity count;
  it is now rejected. The structured-credit notebook passes it on the
  `PoolAsset` `industry` keyword, so its industry concentration now sees it.

#### Changed (BREAKING)

- `AgencyMbsPassthrough.prepayment_model`, `AgencyTba.prepayment_model`,
  `DollarRoll.prepayment_model` and the CMO collateral pool (now
  `prepayment_spec`, matching `StructuredCredit`/`AssetBackedFacility`); Rust,
  JSON, builder setter `prepayment_spec`.
- `DealFees.trustee_fee_annual` (now `trustee_fee`, per annum); Rust, JSON,
  Python/WASM through JSON.
- `clo_trustee_fee_annual`, `abs_trustee_fee_annual`, `cmbs_trustee_fee_annual`,
  `rmbs_trustee_fee_annual` (now `clo_trustee_fee`, `abs_trustee_fee`,
  `cmbs_trustee_fee`, `rmbs_trustee_fee`); Rust.
- `AssetType::{FirstLienLoan, SecondLienLoan, RevolverLoan, BridgeLoan,
  MezzanineLoan, HighYieldBond, InvestmentGradeBond, DistressedBond,
  EmergingMarketsBond}` carry no `industry` payload (`{"type": "first_lien_loan"}`);
  an `industry` key inside `asset_type` is rejected. Rust, JSON, Python/WASM
  `asset_type` dicts.
- `CDSTranchePricerConfig.recovery_spec` (now `stochastic_recovery_spec`,
  matching `CreditModelConfig.stochastic_recovery_spec`); Rust.
- Embedded `structured_credit_assumptions.v1.json` keys
  `prepayment_cpr_annual`, `default_cdr_annual`, `trustee_fee_annual` (now
  `prepayment_cpr`, `default_cdr`, `trustee_fee`).

#### Removed

- `StructuredCredit.behavior_overrides` and the `Overrides` type
  (`cpr_annual`, `psa_speed_multiplier`, `cdr_annual`, `sda_speed_multiplier`,
  `recovery_rate`, `recovery_lag_months`, `reinvestment_price`); Rust, JSON,
  Python `StructuredCredit.behavior_overrides` getter and
  `StructuredCreditBuilder.behavior_overrides`. Use
  `credit_model.prepayment_spec` (`constant_cpr`/`psa`),
  `credit_model.default_spec` (`constant_cdr`/`sda`),
  `credit_model.recovery_spec.{rate, recovery_lag}` and
  `pool.reinvestment_period.assumptions.price_pct`.
- `StructuredCredit.credit_factors` and the `CreditFactors` type
  (`annual_noi`, `annual_debt_service`); Rust, JSON, Python
  `StructuredCredit.credit_factors` getter and
  `StructuredCreditBuilder.credit_factors`. Set `noi` (per annum) on each
  CMBS loan instead.

### TermLoan override channel (2026-09-24)

Numbers change only for direct callers of the floating margin-step program:

- `CashFlowBuilder.float_margin_steps` (Rust, Python) and the
  `floating_margin_program` coupon leg (cashflows JSON, WASM) now read each
  step date as effective-from, the market convention every loan step input
  already uses: `base.rate_spec.spread_bp` applies from issue until the first
  step and each step's `spread_bp` from its date until the next. The former
  `float_margin_stepup` treated each date as a window end and ignored the base
  spread, so `[(2025-07-15, 250)]` over a 200 bp base paid 250 bp from issue;
  it now pays 200 bp until 2025-07-15 and 250 bp after. Reference:
  hand-computed Act/360 spread-only coupons
  (`float_margin_steps_apply_base_spread_until_first_step`, rel 1e-12). Step
  dates must now be strictly increasing, and a step dated on the issue date
  replaces the base spread from issue.
- TermLoan floating and fixed coupons do not move: the loan already applied
  covenant margin steps from the next period start. Pinned bit-for-bit by
  `floating_coupons_pinned_across_margin_step_semantics`.

#### Changed (BREAKING)

- `MarginStepUp` (now `MarginStep`); Rust.
- `TermLoanCovenantEvents.margin_stepups` (now `margin_steps`); Rust, JSON.
- `DdtlSpec.commitment_step_downs` (now `commitment_steps`); Rust, JSON.
- `RevolvingCredit.commitment_schedule` (now `commitment_steps`); Rust, JSON,
  Python `RevolvingCredit.commitment_steps` getter and
  `RevolvingCreditBuilder.commitment_steps`.
- `RateStepSpec` (now `MarginStepSpec`), `RateStepSpec.rate` (now
  `MarginStepSpec.spread_bp`, bp over the index); Rust, cashflows JSON, WASM
  `CouponLegSpec`.
- `CashFlowBuilder.float_margin_stepup` (now `float_margin_steps`,
  effective-from dates); Rust, Python.

#### Removed

- `TermLoanOverrides` and `InstrumentPricingOverrides.term_loan`
  (`margin_add_bp_by_date`, `pik_toggle_by_date`, `extra_cash_sweeps`,
  `draw_stop_date`); Rust, JSON. Express these on `TermLoan.covenants`
  (`margin_steps`, `pik_toggles`, `cash_sweeps`, `draw_stop_dates`), which
  validates them.
- `finstack_quant_core::wire::dated_bool_values` and `dated_i32_values`, whose
  only user was `TermLoanOverrides`.

#### Fixed

- A dated coupon or payment program whose first date is the issue date no
  longer fails `build` with an empty `[issue, issue)` window: the empty
  leading piece is dropped, as an empty trailing `[maturity, maturity)` piece
  already was. This covers `float_margin_steps`, `payment_split_program` and
  `fixed_to_float` with the switch on issue, and a TermLoan covenant margin
  step dated on the issue date.

### Monte Carlo settings (2026-09-24)

Numbers change for Merton structural-credit bond pricing and for explicit
rate-exotic path counts:

- `num_paths` always counts independent estimators. With antithetic sampling
  each estimator simulates a mirrored pair, so the engine simulates
  `2 × num_paths` paths and `MoneyEstimate.num_paths` reports estimators
  (`num_simulated_paths` reports simulated paths). The HW1F rate exotics
  (Tarn, Snowball, CallableRangeAccrual), the Bermudan
  swaption LSMC and the LMM Bermudan engine used to count mirrors in
  `num_paths`; their defaults were halved (`rust.rate_exotics.num_paths`
  20000 → 10000, `rust.lmm_bermudan.num_paths` 50000 → 25000,
  `BermudanSwaptionPricerConfig::DEFAULT_MC.num_paths` 100000 → 50000), so
  default PVs replay the same streams and stay bit-identical (pinned by
  `rate_exotic_default_pv_unchanged`, `lmm_default_pv_unchanged` and
  `lsmc_default_mc_pv_unchanged`). An explicit `model_config.mc_paths = N` on
  these instruments now simulates `2N` paths instead of `N`; the LMM engine no
  longer requires an even count.
- Merton MC (`ModelKey::MertonMc`, `Bond::price_merton_mc`) seeds from
  `derive_seed(instrument_id, model_config.mc_seed_scenario or "base")`
  instead of the fixed registry seed 42, and takes its estimator count and
  antithetic flag from `model_config.mc_paths`/`mc_antithetic` (registry
  `rust.merton_pik_bond`: 5000 antithetic estimators, the same 10000
  simulated paths as before). Reference: with the former seed the same
  estimator agrees within four combined standard errors
  (`merton_seed_comes_from_model_config_scenario`).
  `MertonMcCalibrationSpec.low_paths` also counts estimators, so the default
  low-path calibration (2000) simulates 4000 antithetic paths instead of 2000.
- AsianOption and LookbackOption Monte Carlo now honour
  `model_config.mc_antithetic` and the workspace path cap through the shared
  `merged_path_config`; their private copies applied `mc_paths` only.
  Geometric Asian MC stays within four standard errors of the discrete-fixing
  Kemna-Vorst price with either setting (`asian_honours_mc_antithetic`).
- RevolvingCredit stochastic pricing (`price_with_paths`, the
  `monte_carlo_three_factor` model, `draw_option_cost`) seeds from
  `derive_seed(facility_id, model_config.mc_seed_scenario or "base")` instead
  of the fixed seed 42 (`seed: None`) or the wire `seed`, and counts
  `model_config.mc_paths` as independent estimators (antithetic estimators
  simulate two paths). Defaults come from the new registry block
  `rust.revolving_credit` (10000 estimators, antithetic off). Stochastic PVs
  move by Monte Carlo noise only. Reference: with the former fixed seed 42
  the same estimator agrees within four combined standard errors
  (`revolver_seed_comes_from_model_config_scenario`).
- StructuredCredit `PricingMode::MonteCarlo.num_paths` counts independent
  estimators (antithetic estimators simulate two scenario paths). The
  default halved (10000 → 5000, `PricingMode::monte_carlo` clamp 100 → 50,
  Python `StructuredCredit.price_stochastic(num_paths=None)` 10000 → 5000), so
  the default stochastic PV and standard error stay bit-identical (pinned by
  `default_monte_carlo_pv_is_bit_identical_under_estimator_semantics`). An
  explicit `num_paths = N` or `model_config.mc_paths = N` with antithetic
  sampling now simulates `2N` paths; the default mode also honours
  `model_config.mc_antithetic`. `StochasticPricingResult.num_paths` still
  reports simulated scenario paths.

#### Changed (BREAKING)

- `MetricPricingOverrides.mc_seed_scenario` (now
  `InstrumentPricingOverrides.model_config.mc_seed_scenario`, builder
  `InstrumentPricingOverrides::with_mc_seed_scenario`); Rust, JSON and the
  Python `MetricPricingOverrides` keyword/getter (removed). Finite-difference
  Greeks set the common-random-number label on the repriced clone at the new
  path.
- `CommodityMcParams.n_paths` / `n_steps` (now `num_paths` / `num_steps`);
  Rust and JSON.
- `StructuredCreditPricingMode::Hybrid.mc_paths` (now `num_paths`); Rust and
  JSON.
- `MertonMcConfig.time_steps_per_year` (now `steps_per_year`, setter
  `steps_per_year`), `MertonMcConfig.default_recovery_rate` (now
  `recovery_rate`, setter `recovery_rate`) and
  `MertonMcCalibrationSpec.max_iter` (now `max_iterations`); Rust, JSON and the
  Python `MertonMcConfig` fluent setters. Registry key
  `rust.merton_pik_bond.time_steps_per_year` (now `steps_per_year`).
- `MertonMcEngine::price` takes a `MertonMcRun { num_paths, seed, antithetic }`
  alongside the `MertonMcConfig`; `MertonMcResult.num_paths` reports
  estimators.
- `RateExoticMcConfig::effective_path_count` and `raw_stream_count` (now
  `simulated_path_count`; the stream count is `num_paths`).

#### Removed

- `MertonMcConfig.num_paths`, `seed` and `antithetic` (fields, Rust setters
  and Python setters) and registry key `rust.merton_pik_bond.seed`: the path
  count, antithetic flag and seed label come from
  `instrument_pricing_overrides.model_config` (`mc_paths`, `mc_antithetic`,
  `mc_seed_scenario`).
- The private `merged_path_config` copies in the Asian and Lookback option
  pricers.
- `StochasticUtilizationSpec.num_paths`, `seed` and `antithetic` (Rust, JSON
  `draw_repay_spec.stochastic.*`, Python/WASM payloads): the estimator count,
  antithetic flag and seed label come from
  `instrument_pricing_overrides.model_config` (`mc_paths`, `mc_antithetic`,
  `mc_seed_scenario`), resolved by the new `RevolvingCreditMcRun::resolve`.
  `use_sobol_qmc` stays on the spec and is rejected together with
  `mc_antithetic = true`.

### Market-quote overrides (2026-09-24)

Numbers change for `ImpliedVol` on CapFloor, Swaption, FxOption, CDSOption
and EquityOption, and for the five futures options under volatility bumps:

- Every `ImpliedVol` calculator (EquityOption, FxOption, CapFloor, Swaption,
  CDSOption) now inverts the observed premium in the new
  `instrument_pricing_overrides.market_quotes.quoted_premium` (total trade PV
  in the instrument currency; not a price driver). A missing premium is a
  validation error naming that path. CapFloor used to read
  `quoted_clean_price` (a percent-of-par bond quote) as a currency premium;
  EquityOption read `attributes.meta["market_price"]`/`["market_price_id"]`;
  Swaption, FxOption and CDSOption inverted their own model PV. Reference:
  QuantLib 1.43 NPVs for `spx_atm_call_1y`, `eurusd_atm_call_3m`,
  `usd_black_caplet`, `usd_bachelier_floorlet` and the Black and Bachelier
  `1y1y` payer swaptions: with `quoted_premium` set to the QuantLib NPV,
  `ImpliedVol` recovers the fixture's flat surface volatility within 1e-9.
  ConvertibleBond `ImpliedVol` still inverts its clean bond price
  (`quoted_clean_price_pct`).
- The five futures options (commodity, equity, FX, interest-rate and
  volatility-index) take their flat volatility from
  `market_quotes.implied_volatility`, so generic vega and cross-factor
  volatility bumps now move their PV; a live option without it is a
  validation error naming the path. PV at an unchanged volatility does not
  move (Black-76 and Bachelier closed forms, 1e-10 relative).

#### Changed (BREAKING)

- `MarketQuoteOverrides.quoted_clean_price` (now `quoted_clean_price_pct`)
  and `InstrumentPricingOverrides::with_quoted_clean_price` (now
  `with_quoted_clean_price_pct`); Rust and the
  `instrument_pricing_overrides.market_quotes` JSON wire (Python/WASM/UI
  payloads). The old key is rejected.
- `Bond::from_cashflows(.., quoted_clean)` and
  `bond_from_cashflows_json(.., quoted_clean)` (now `quoted_clean_price_pct`);
  Rust, Python keyword and WASM `bondFromCashflowsJson(.., quotedCleanPricePct)`.
- `EquityOption::implied_vol(.., market_price)` (now `target_price`); Rust and
  the Python keyword.
- `FutureOptionTerms::npv_raw`/`cash_delta`/`cash_gamma`/`cash_vega`/
  `cash_theta` take the flat volatility as an `implied_volatility: Option<f64>`
  argument (Rust).

#### Added

- `MarketQuoteOverrides.quoted_premium` and
  `InstrumentPricingOverrides::with_quoted_premium` (Rust and the
  `instrument_pricing_overrides.market_quotes` JSON wire).

#### Removed

- `FutureOptionTerms.volatility` (now
  `instrument_pricing_overrides.market_quotes.implied_volatility` on
  CommodityFutureOption, EquityFutureOption, FxFutureOption,
  InterestRateFutureOption and VolatilityIndexFutureOption); Rust and the
  `terms` JSON wire. The old key is rejected.
- `InflationLinkedBond.quoted_clean` (now
  `instrument_pricing_overrides.market_quotes.quoted_clean_price_pct`, read by
  RealYield, RealDuration and BreakevenInflation); Rust and the JSON wire.
  The old key is rejected.
- `CDSOption::with_implied_vol` (set
  `instrument_pricing_overrides.market_quotes.implied_volatility`; validation
  still enforces the 500% ceiling).
- EquityOption `attributes.meta["market_price"]`/`["market_price_id"]` as the
  `ImpliedVol` target.
- `MetricContext::set_instrument_overrides`, left without a reader once
  CapFloor `ImpliedVol` reads the instrument's own `market_quotes` (Rust).

#### Fixed

- The `market_quotes.cds_quote_bp` doc now names CreditDefaultSwap only; no
  other instrument (CDSIndex included) reads it.

### Short-rate and hazard model parameters (2026-09-24)

Numbers change. The rates-only bond tree now reads only explicit short-rate
model inputs, and the same lattice values both legs of an option
decomposition:

- A bond priced on the `tree` model needs a complete Hull-White pair
  (`model_config.hw1f_mean_reversion` and `hw1f_sigma`, or the pre-fitted
  `{curve}_HW1F_KAPPA`/`_SIGMA` market scalars on the tree discount curve),
  or `vol_model = black` with `model_config.bdt_sigma`. The hard-coded
  κ = 0.03 default, the `market_quotes.implied_volatility` fallback and the
  σ = 0 deterministic default are gone; a callable or return-floor bond
  without them is a validation error naming the missing wire path.
- Deterministic rates on the rates-only tree are now an explicit choice:
  `hw1f_sigma = 0` with a positive `hw1f_mean_reversion` (other Hull-White
  paths still require σ > 0). This is the only rates-only setting that prices
  a floating coupon, since that tree preprojects coupons; a risk-free callable
  or return-floor FRN that used to price on the implicit σ = 0 default must
  now set it, and gets the same price as before. The stochastic floating
  rejection now names `hw1f_sigma = 0`, or `credit_curve_id` with
  `rates_credit`, as the remedy.
- `embedded_option_value` and bond `vega` value the straight leg on the same
  BDT/Hull-White lattice as the optioned bond. The straight leg used to be a
  Ho-Lee tree driven by the lognormal `implied_volatility` quote read as a
  normal volatility. Reference: Bloomberg OAS screens. IBM EUR 2034 option
  value is now -685.60 against Bloomberg -700.00 (tolerance 50, unchanged);
  BHCCN 10 2032 is -18312.68 against -19800.00 (still listed as an expected
  proprietary-tree gap, evidence refreshed). Vega stays within its Bloomberg
  tolerance for both (-0.0053 vs -0.01, -0.0547 vs -0.06). The QuantLib
  `usd_fixed_callable_8y_oas` fixture only renames its keys; its expected
  OAS and DV01 did not move.
- Every other output is unchanged; the bond tests that relied on the old
  channels now set the equivalent `hw1f_*` or `bdt_sigma` values
  (σ = old `implied_volatility`, κ = 0.03).

#### Changed (BREAKING)

- `ModelConfig.hazard_volatility` (now `hazard_sigma`) and
  `InstrumentPricingOverrides::with_hazard_volatility` (now
  `with_hazard_sigma`); Rust and the `instrument_pricing_overrides.model_config`
  JSON wire (Python/WASM/UI payloads). The old key is rejected.
- Structured-credit `OasConfig.hw_kappa`/`hw_sigma` (now
  `hw1f_mean_reversion`/`hw1f_sigma`); Rust and the JSON `config` accepted by
  the Python and WASM tranche OAS entry points. The old keys are rejected.
- RevolvingCredit `InterestRateProcessSpec::HullWhite1F { kappa, sigma, .. }`
  (now `{ hw1f_mean_reversion, hw1f_sigma, .. }`); Rust and the
  `mc_config.interest_rate_process.hull_white_1f` JSON wire. The old keys are
  rejected.
- Agency MBS `McOasConfig.hw_kappa`/`hw_sigma` (now
  `hw1f_mean_reversion`/`hw1f_sigma`; crate-internal).
- `bond_tree_config(bond)` (now `bond_tree_config(bond, market)`, which
  resolves the Hull-White pair through `resolve_hw1f_params`) and
  `TreeModelChoice::BlackDermanToy { mean_reversion, sigma }` (now
  `{ sigma }`: the BDT lattice has no mean reversion); Rust.
- HW1F validation errors name the full wire paths
  (`instrument_pricing_overrides.model_config.hw1f_mean_reversion` /
  `hw1f_sigma`) instead of the non-existent `hw1f_kappa`.

#### Added

- `ModelConfig.bdt_sigma`: the Black-Derman-Toy lognormal short-rate
  volatility for bonds with `vol_model = black` (Rust, JSON wire), with
  `InstrumentPricingOverrides::with_bdt_sigma` and
  `with_hw1f_mean_reversion` setters (Rust). The rates-credit path rejects it.

#### Removed

- `ModelConfig.mean_reversion`; use `hw1f_mean_reversion` (Rust, JSON wire).
  The old key is rejected.
- The bond tree's reading of `market_quotes.implied_volatility` as a
  Hull-White, BDT or Ho-Lee short-rate volatility. It stays an option quote.

### Factor-model bumps (2026-09-24)

Numbers change: volatility factors now produce sensitivities. The portfolio
`DeltaBasedEngine`, `FullRepricingEngine` and factor-model stress/what-if paths
used to reject every `FactorType::Volatility` factor ("VolPoint incompatible
with MarketMapping units Percent"). A volatility factor now bumps its
`VolShift` surfaces by `vol_points` as an additive `BumpUnits::Percent` shift
(1.0 = one vol point = 0.01 absolute vol), so its delta is P&L per vol point.
Reference: an ATM 6-month EquityOption's vol-factor delta equals its `Vega`
metric and the Black-Scholes vega per vol point within 1e-3 relative
(`portfolio/tests/factor_model_engines.rs`). Rates, credit, equity and FX
factor outputs are unchanged.

#### Changed (BREAKING)

- `FactorModelConfig.bump_size` (now `bump_config`); Rust, the
  `finstack_quant.factor_model_config/1` and attribution JSON wire (the old key
  is rejected), Python `FactorModelConfig.bump_config` getter and WASM
  `FactorModelConfig.bump_config`.
- `BumpSizeConfig::bump_size_with_unit_for_factor` returns
  `(f64, BumpUnits)` (core `BumpUnits`), and the canonical unit comes from the
  new `FactorType::bump_units()`: `RateBp` for rates, credit, inflation and
  custom factors; `Percent` for equity, commodity, FX and volatility (Rust).
- A `MarketMapping::CurveParallel`/`VolShift` mapping must declare the factor
  type's canonical `BumpUnits`; `CurveBucketed` requires `RateBp` (Rust error
  text now reads "factor bump units ... incompatible with MarketMapping units").

#### Removed

- `FactorBumpUnit` (`Absolute`, `BasisPoint`, `Percent`, `VolPoint`,
  `Fraction`, `Multiplier`), `FactorBumpUnit::canonical_for` and
  `FactorBumpUnit::to_fraction`; use core `BumpUnits` and
  `FactorType::bump_units` (Rust).

### Bump sizes: one field per bump (2026-09-24)

Numbers change only when a bump override is set, plus CliquetOption rho and
Bermudan swaption `hw_sigma_vega`. Every other default output is bit-identical,
pinned by `tests/metrics/bump_config_routing.rs`.

#### Changed (BREAKING)

- `BumpConfig.ytm_bump_decimal` (now `ytm_bump_bp`, in bp: 1.0 = 1bp) and
  `MetricPricingOverrides::with_ytm_bump_decimal` (now `with_ytm_bump`); Rust,
  Python docs and the `metric_pricing_overrides` JSON wire. It now sizes the
  InflationLinkedBond `RealDuration` shock and the structured-credit
  `DurationMod` and `Convexity` shocks. Structured-credit convexity keeps its
  10bp default when the field is unset.
- `TaylorAttributionConfig.credit_bump_bp` (now `credit_spread_bump_bp`); Rust,
  attribution JSON wire, notebooks and docs-site.
- `OptionGreeksProvider` methods take a `bumps: GreekBumps` argument and
  `OptionGreeksRequest` carries a `bumps` field (Rust). The new public
  `GreekBumps { spot_bump_pct, vol_bump_pct, rate_bump_bp }` is resolved from
  `valuations.sensitivities.v1` layered with `metric_pricing_overrides.bump_config`.
- `ConvertibleBond::greeks(curves, tree_type, bump_size, as_of)` (now
  `greeks(curves, tree_type, as_of)`) and Python `ConvertibleBond.greeks(market,
  as_of, bump_size=None)` (now `greeks(market, as_of)`). Bumps come from the
  bond's `metric_pricing_overrides.bump_config`. `calculate_convertible_greeks`
  takes `bumps: GreekBumps` in place of `bump_size: Option<f64>`.
- `CDSTranchePricer::calculate_cs01` takes `credit_spread_bump_bp: f64`;
  `InflationLinkedBond::real_duration` takes `ytm_bump_bp: f64`;
  `calculate_tranche_duration` and `calculate_tranche_convexity` take
  `ytm_bump_bp: f64` (Rust).

#### Removed

- `BumpConfig.rho_bump_decimal` and `BumpConfig.vega_bump_decimal` (twins of
  `rate_bump_bp` and `vol_bump_pct`) and their resolver fallbacks; Rust and the
  `metric_pricing_overrides` JSON wire, which now rejects both keys.
- `MetricPricingOverrides::rho_bump_bp()` (Rust).
- `CDSTranchePricerConfig.cs01_bump_size` (use `bump_config.credit_spread_bump_bp`)
  and `CDSTranchePricerConfig.corr_bump_abs` (Correlation01 uses the shared
  0.01 correlation bump); Rust only.
- The crate-internal `metrics::bump_sizes` constants, the local
  `DIVIDEND_BUMP_BP`/`INFLATION_BUMP_BP` constants (now `ONE_BASIS_POINT`) and
  the Bermudan `DEFAULT_VOL_BUMP_PCT`/cap-floor `DEFAULT_HW_VEGA_BUMP` (now one
  `rates::hw1f::HW_SIGMA_BUMP = 1e-4`).

#### Fixed

- **Every finite-difference greek honours `bump_config`.** EquityOption,
  FxOption, FxBarrierOption, FxTouchOption, QuantoOption and commodity option
  vanna/volga/delta/gamma/vega, EquityTRS delta, quanto `fx_delta`/`fx_vega`,
  commodity Asian and spread option greeks, CmsOption delta/rho/vega/volga,
  InflationCapFloor vega, CapFloor forward PV01, VarianceSwap DV01 and the
  Bermudan swaption delta ignored `spot_bump_pct`, `vol_bump_pct` or
  `rate_bump_bp`. Results stay in their reporting units (per 1%, per vol point,
  per bp) whatever the bump size.
- **Every Rho reports per 1bp** as `(pv_bumped - pv) / rate_bump_bp`: Quanto,
  FxBarrier, FxTouch, RangeAccrual, Lookback, Cliquet and CmsOption.
- **Charm, speed and color** share one spot bump that starts from the resolved
  bump; `adaptive_bumps` widens it only when no explicit `spot_bump_pct` is set.

**Numbers change:**

- CliquetOption `rho`: it was 0 for a cliquet whose resets are all observed
  (the time guard stopped at the last reset, not `expiry`) and otherwise the PV
  change for a 0.0001bp bump. It is now the per-1bp discount sensitivity,
  checked against `PV·(exp(−δr·τ) − 1)/bp` at 1bp and 10bp
  (`fully_observed_cliquet_rho_is_discount_sensitivity`, 1e-9·|PV|).
- Bermudan swaption `hw_sigma_vega` shifts σ by the absolute 1e-4 used by
  cap/floor `hw_sigma_vega`, not by 1% of σ; checked against σ ± 1e-4 tree
  re-runs (`bermudan_hw_sigma_vega_uses_absolute_bump`). The Bloomberg
  `usd_cap_5y_atm_black` cap/floor golden is unchanged.
- Any greek above whose `bump_config` override was set and ignored now moves to
  the overridden stencil (test-side stencil re-runs at 1e-12; EquityOption
  vanna/volga against Black-Scholes and FxOption volga against Garman-Kohlhagen
  at 5e-3).

### Changed

- The next library release is 0.9.0 and requires Rust 1.97.1 across the Rust,
  Python, and WASM crates.

### Fixed

- **Convertible tree PV is continuous at the conversion price.** With an even
  step count (the default 200) and spot at the conversion price, the centre
  terminal node sat on the conversion boundary and its Tsiveriotis-Zhang
  cash/equity split flipped from all-cash (risky discounting) to all-equity
  (risk-free), so PV jumped as spot crossed it and the 1% central-difference
  delta exceeded the conversion ratio (10,139 shares on a 10,000-ratio bond).
  The terminal split is now smoothed over each node's log-spot cell; nodes away
  from the boundary are unchanged. Credit-risky convertible prices near the
  money move by up to half the former jump; no stored goldens changed.

### Fixed income: senior-review remediation (2026-09-23)

Numbers change for the instruments listed under **Fixed**; each change is
pinned by a regression test against an independent hand calculation.

#### Fixed

- **Asset-backed facility PV nets future draws.** Draws were missing from the
  priced schedule while the lender IRR included them; a 10M draw one year out
  overstated PV by 9.51M. Indexed interest is tagged `FloatReset`.
- **Merton MC prices ACT/ACT ICMA bonds and books dirty PV.** Gilts and other
  ICMA bonds errored before simulating; seasoned bonds dropped the accrued part
  of the next coupon (102.894 vs 104.131 hand dirty PV). Clean and dirty
  prices now differ by accrued.
- **Bond futures use the current futures price** for gross basis, implied
  repo and invoice price, and look up the conversion factor by the delivered
  bond. The entry price could reorder the cheapest-to-deliver.
- **Overnight floors on term loans and revolvers** honour
  `overnight_index_constraints` (daily vs period) and the term loan passes
  `reset_frequency`/`index_tenor` through; both instruments now pay identical
  coupons for the same spec.
- **Term loans:** commitment fees are paid once per period on the payment date;
  PV is anchored at `as_of` (discount margin, yields, OAS and quoted CS01 stay
  on settlement).
- **Revolver:** unknown `pricing_model`, NaN utilization and fee-tier errors now
  raise; the stochastic engine books sub-threshold draws, so principal
  conserves.
- **Structured credit:** differently capped coverage tests at one position keep
  their own `divert_pct`; OAS and stochastic sources share the deterministic
  base rates (zero-vol OAS equals Z-spread); antithetic prepayment draws pair;
  WAM propagates errors and is measured on Act/365F, the same clock as WAL.
- **Japanese simple yield** uses the JSDA clean-price form in the metric and
  its inverse, so mid-period quotes round-trip.
- **Callable I-spread** uses the swap par rate to the workout date in both
  directions, so the quote round-trips. BHCCN moves from within 2 bp of
  Bloomberg to 3.5 bp and is recorded in `known_non_executable.json`.
- **FI TRS** prices mid-period from `initial_level`; ILB principal pays on the
  same adjusted date in PV and real yield; UK step-lag CPI anchors on the first
  of the month; convertible theta reports a failed market roll.
- **Core:** a discount curve with one pillar after `t = 0` rolls forward.

#### Performance

- Callable exercise windows use candidate dates (window ends, schedule dates,
  month-ends) instead of every calendar day; the largest move on six gate
  fixtures is 0.005 per 100. A 5-year call window: tree PV 108 ms → 0.4 ms,
  OAS metric 951 ms → 4.5 ms. OAS reuses one prepared tree; the workout path
  is computed once per metric request; LSMC pricing runs in parallel.
- Structured credit Monte Carlo 3–5× faster; convertible Greeks share one
  lattice run; term-loan discount margin 293 → 78 µs; CMO waterfalls
  1.4–2.8× faster.

#### Changed (BREAKING)

- Removed: `TreePricer::calculate_oas`, `TreePricerConfig::{high_precision,
  mean_reversion}`, the LSMC exercise-provider hook,
  `price_from_ytm_compounded`, `par_rate_and_annuity_from_forward`,
  `CashflowSpec::{fixed_with_conventions, floating_with_conventions}`,
  `FloatingConventionParams`, `calculate_conversion_premium`,
  `IndexationMethod::{standard_lag_modern, uses_daily_interpolation,
  uses_daily_interpolation_modern}`.
- Bond futures: `determine_ctd`, `determine_ctd_by_implied_repo`,
  `implied_repo_rate` and `invoice_price` take the current futures price
  (`DeliverableQuote`); `calculate_conversion_factor(bond, &specs, month)`;
  `BondFutureSpecs` drops `tick_size`, `tick_value`, `settlement_days` and
  `calendar_id` on the wire.
- FI TRS drops `contract_size`; `initial_level` is the reset level of the
  period in progress. ILB `real_yield`, `real_duration` and
  `breakeven_inflation` drop the market argument.
- Asset-backed facility `lender_cashflows()` and the revolver fee-tier
  functions return `Result`; `TermLoanTreePricer` is a unit struct.
- Structured credit: `TrancheBehaviorType`/`Tranche.behavior_type` (also in
  Python), `ConcentrationCheckResult`, `get_tranche_cashflows`, the Rust
  `structured_credit_tranche_*` string wrappers, `WaterfallWorkspace`, the
  `CoverageTest` enum and `Waterfall::new` are removed; `TrancheStructure`
  drops `total_size` and `Tranche` drops `payment_priority` on the wire;
  `WaterfallBuilder::build()` validates; `calculate_pool_stats` and
  `weighted_avg_maturity` return `Result`.
- Dilution events on a convertible must be in date order.

### Agency mortgages: quote-basis spreads, IO notional, prepayment-aware DV01 (2026-09-23)

#### Fixed

- **MBS MC-OAS and CMO Z-spread price the pool the quote buys.** A clean quote
  plus settlement-month accrued buys the settlement-month accrual onward; the
  prior month's in-flight P&I belongs to the seller. Both spreads previously
  projected it, so the same pool at the same quote solved 6.80 bp on Feb 10 and
  2.41 bp on Feb 27 (MBS) and 4.60 bp versus −5.05 bp (CMO). Holder NPV is
  unchanged. The TBA delivered pool shares the same helper.
- **CMO IO strips accrue on their current notional** and amortize with the
  collateral balance; the old rule scaled the IO's original face by the pool's
  original factor (a 70mm IO on factor-0.7 collateral paid 142,916.67 instead of
  204,166.67 in month one). `AgencyCmo::validate` now requires the collateral
  current face to equal the principal tranches' current faces.
- **Dollar-roll carry** uses the generic pool's prepayment model instead of a
  hard-coded 0.5% SMM and divides by the dirty front price.
- **DV01 of TBA, dollar roll and CMO is prepayment-aware**, like the MBS
  pass-through, through the new `Instrument::rate_risk_rebuild`.
- **MBS MC-OAS** reads Hull-White κ/σ from
  `model_config.hw1f_mean_reversion`/`hw1f_sigma` (together; default 5%/1%) and
  measures payment offsets on the discount curve's day count.
- **Custom MBS payment delays** follow the agency day-of-month rule with a
  `usny` roll (75 days on a January accrual → 15 March, not the Saturday 16th).
- **CMO tranche interest** accrues on the collateral's day count, not a flat 1/12.

#### Breaking (Rust only)

- `dollar_roll::carry::{implied_financing_rate, roll_specialness}` drop the
  `prepay_rate` argument; `break_even_drop` takes the dirty front price.
- `AgencyTba` and `DollarRoll` gain an optional `prepayment_model` wire field.
- Removed `mbs_passthrough::delay`, `MbsCashflow::sifma_date`,
  `cmo::waterfall::{execute_waterfall, execute_waterfall_with_pac,
  allocate_io_cashflow}`; `execute_waterfall_with_principal_breakdown` takes a
  collateral survival ratio and an accrual fraction.
- PSA lives in `finstack_quant_cashflows::builder::psa_cpr`.

### Agency MBS/TBA/CMO: unused parallel models removed (2026-09-23)

#### Removed (BREAKING, Rust only; none of these were bound in Python or WASM)

- `mbs_passthrough::prepayment` (`AgencyPrepaymentModel`,
  `StochasticPrepaymentClone`). The pricer uses `PrepaymentModelSpec`
  directly; rate-dependent speeds live in MC-OAS and effective duration.
- `mbs_passthrough::servicing`. Fees are the pool's `servicing_fee_rate` and
  `guarantee_fee_rate` fields.
- `tba::settlement`. Use `AgencyTba::get_settlement_date`.
- `tba::allocation` (`allocate_generic_pool`, `AllocationResult`,
  `PoolCharacteristics`, `validate_sifma_variance`) and the
  `pool_characteristics` block of `data/assumptions/tba_assumptions.v1.json`.
  It was a generic-pool stub, not a cheapest-to-deliver model.
- `cmo::tranches::io_po` (`IoStripCharacteristics`,
  `PoStripCharacteristics`). The CMO waterfall pricer prices IO/PO strips.
- `cmo::tranches::sequential` (`SequentialOrder`, `average_life`,
  `estimate_payment_window`). Waterfall priorities and WAL metrics cover them.
- `PacSchedule::is_within_collar` and `PacContext::actual_psa` (written but
  never read).

#### Changed

- A TBA priced on an explicit `assumed_pool` now rejects a pool whose
  pass-through coupon differs from the TBA coupon, or whose agency is not good
  delivery (FNMA and FHLMC UMBS are interchangeable; GNMA I and GNMA II are
  separate programs).
- `tba_assumptions.v1.json` must carry `schema`
  `finstack_quant.tba_assumptions/1` and `version` 1; both are now checked.

### Loans cleanup: one term-sheet vocabulary, one balance replay (2026-09-23)

#### Changed (breaking)

- **`TermLoanSpec` and its `TryFrom` are removed.** `TermLoan` is itself the
  serde-stable shape; build it with `TermLoan::builder()` (which validates) or
  deserialize it. The spec's `notional_limit: None` fallback to the DDTL
  commitment is gone: `notional_limit` is always required.
- **Term-loan DDTL step-downs use the shared `loan_terms::CommitmentStep`.**
  `CommitmentStepDown` is deleted; step JSON `{date, new_limit}` becomes
  `{date, amount, fee_bp}`. Term loans carry no reduction fee, so a non-zero
  `fee_bp` is rejected. `DdtlSpec::usage_fee_bp` / `commitment_fee_bp` are now
  `f64` basis points (finite, non-negative), matching the revolver.
- **The revolver's Monte Carlo model key is `monte_carlo_three_factor`**
  (`ModelKey::MonteCarloThreeFactor`), naming its utilization / short-rate /
  credit-spread simulation. `monte_carlo_gbm` no longer prices a revolving
  credit facility; it remains the GBM key for exotics.
- Removed dead items: `RevolvingCreditFees::flat_bp`,
  `revolving_credit::ZERO_TOLERANCE`.

### Component source registry 0.2.0 (2026-09-20)

- Prepares 150 registry items for financial forms, views and a composed pricing
  workbench using existing WASM contracts and unmodified shadcn controls. See the
  [UI changelog](finstack-quant-ui/CHANGELOG.md) and
  [upgrade instructions](docs-site/content/docs/registry/upgrading.mdx).
- No Rust, Python or WASM calculation/API additions are claimed. Typed cashflow
  rows, inferred units, full-state curve evaluation, plain volatility-surface
  off-grid evaluation, absent financial aggregates, unresolved detail contracts
  and quote-space calibration fits remain excluded. Scenario-price documentation
  drift remains upstream. The complete installation matrix and public CLI/MCP
  checks remain open; publication remains restricted to `master`.

### Requested metrics never go missing; windowed formulas work on money (2026-09-19)

#### Fixed

- **Windowed formula functions are usable on monetary nodes.** `lag`, `shift`,
  `diff`, `rolling_mean/sum/min/max/median/std`, `ewm_mean`, `ewm_std`,
  `annualize` and `fiscal_ytd` folded their trailing count, window, offset,
  smoothing-factor or month argument into the dimension check, so every one of
  them was rejected on a USD-denominated node with `Dimensional mismatch in
  lag: cannot combine scalar and USD`. A debt corkscrew written the natural way
  (`lag(closing_debt, 1) + drawdowns - repayments`) could not be expressed and
  had to be rebuilt with `cumsum`. Those functions now take their dimension
  from the series argument alone; an argument that is a count, window, offset,
  smoothing factor, digit count or month number never participates in dimension
  unification. `mean`, `sum`, `min`, `max`, `clamp` and `coalesce` are genuinely
  variadic over values and still reject a currency mismatch between them.

#### Changed (breaking)

- **A metric requested from `price_instrument` / `priceInstrument` /
  `Instrument::price_with_metrics` is now either returned or raises.**
  Previously a requested metric with no calculator for that instrument type was
  silently omitted from `measures`, so a caller could not tell "not supported"
  from "computed but missing". It now raises `MetricNotApplicable` (Python
  `ValueError`) naming both the metric and the instrument type.
  **16,509 of the 17,940 (instrument type, metric) pairs change from silence to
  an error**; 1,431 remain applicable. Callers passing a broad list across mixed
  instruments must narrow it.
- `MetricRegistry::compute` rejects unregistered and non-applicable requests
  before running any calculator, so the reported error no longer depends on
  dependency evaluation order.
- `MetricId::ThetaPeriodDays` is a registered metric that resolves on its own;
  previously it appeared only when `theta` was requested in the same call.
- **A portfolio metric list is a documented menu.** `value_portfolio` /
  `valuePortfolio` / `RequestedMetrics` take one list for a book of mixed
  instrument types and offer no way to say "`delta` for the options, `cs01` for
  the credit", so each position is now asked for exactly the entries its own
  instrument type has a calculator for. This restores mixed-book risk runs,
  which the strict single-instrument contract above would otherwise have made
  impossible for any list broader than the standard set. Narrowing covers
  structural inapplicability only: `strict_risk` still governs a metric an
  instrument type supports but fails to compute, and an unknown metric name
  still raises when the request is parsed.

#### Added

- `PositionValue.inapplicable_metrics` (Rust `Vec<MetricId>`, Python
  `list[str]`, serialized when non-empty) lists the requested metrics a
  position's instrument type has no calculator for, so the portfolio-level
  narrowing above is reported rather than silent. It is not a failure list:
  unlike `degraded_positions` and `PortfolioMetrics.skipped_metrics`, nothing in
  it could have been computed and was not, so a portfolio total is not
  understated by it.
- `MetricRegistry::applicable_subset(&[MetricId], InstrumentType)` narrows a
  cross-instrument superset to what one instrument type supports, so dropping
  the rest is a visible decision at the call site rather than a silent filter
  inside pricing. Used by composite legs, the standard option-Greek set, the
  portfolio evaluation menu and the attribution engine menu, each of which
  evaluates a fixed superset across heterogeneous instruments.
- `expected_loss` is a registered metric for `structured_credit` and `bond`,
  and `expected_shortfall` for `structured_credit`. These are published by the
  model pricers themselves — structured credit's deal Monte Carlo pass and the
  Merton-MC bond engine — rather than derived by a calculator, so the registry
  previously reported them as inapplicable to those instrument types even
  though every priced result carried them. Requesting one now returns the
  pricer's own value, unchanged and never recomputed; on structured credit
  `expected_shortfall` is the deal Monte Carlo tail loss rather than a
  historical-simulation estimate, matching what the result envelope already
  reported. Requesting either on a valuation whose model does not publish it
  (a discounted bond, say) is an error, as is requesting them on an instrument
  type that neither publishes nor calculates them.


### Structured credit: analyst remediation (2026-09-17 plan, Phases A–E)

#### Fixed

- Deal and cleanup calls realize the stub period's collateral flows; every
  "original balance" quantity (cumulative-loss triggers, cleanup-call factor,
  charge-off curves) uses `AssetPool.original_balance`; OC cures are sized for
  an interest-funded diversion (`max(0, D − N/r)`); `CoverageRules::clo_standard`
  carries performing collateral at par; Default01 / Prepayment01 / Recovery01 /
  Severity01 and DV01 reprice per tranche through `StructuredCreditTranche`;
  stochastic prices are per current tranche face (`TranchePricingResult.price_pct`).
- Card master trusts allocate collections on a fixed investor share
  (`CardPortfolioSpec.{seller_interest, fixed_allocation_pct}`); the CLO registry
  profile is the single source of CLO defaults (15/35 bp fees, 20% CPR, 2% CDR,
  60% recovery); the incentive fee shares principal proceeds above the hurdle;
  shifting interest keeps the equity residual out of pro-rata principal.
- Servicer advances are per loan: cures repay arrears (reimbursing advances
  first) and amortize the repaid principal, charge-offs write off pro rata and
  reimburse from their own proceeds; the coverage-test evaluation that gates
  reinvestment runs on the executor's cash and hedge-adjusted waterfall.
- `Tranche.frequency` sets the yield's compounding and its day count the time
  basis (a par 5% quarterly 30/360 note yields 5.00%); impaired notes price at
  zero with `TrancheValuation.ytm: None`; scenario cells quote the clean
  settlement price from one projection; the payment business-day default is
  documented as ModifiedFollowing.

#### Added

- Collateral terms: `PoolAsset.{origination_date, amortization_term_months,
  io_months, index_floor}` (per-asset seasoning on PSA/ABS/vector/SDA curves,
  level payment over the amortization term, IO windows, index floors),
  `BalloonSpec.{extension_prob, loss_prob, severity_pct, workout_months}` with
  workout claims, NPL timelines anchored on `acquisition_date`, DSCR on the live
  floating coupon and schedule.
- Deal mechanics: `LossRecognition::{AtDefault, AtLiquidation}`,
  `ShiftingInterestSpec::{mode, triggers}`, `WaterfallRules::{reserve, target_oc}`
  (`ReserveAccountSpec`, `ReserveTarget`, `TargetOcSpec`), `AfcSpec.carryover`,
  `CoverageTestSpec::{placement, divert_pct}` (`CoveragePlacement`),
  `Tranche.non_deferrable`, `AdvancingPolicy::PrincipalAndInterest.reimburse_from_collections`,
  `DealFees::{workout_fee_pct, liquidation_fee_pct}` with dynamic special
  servicing, `PrepaymentPenalty::{Lockout, StepDown, YieldMaintenance {
  discount_curve_id, floor_pct }}`, `CreditModelConfig.stochastic_recovery_spec`.
- Facilities: `EarlyAmortizationSpec.max_cumulative_loss`,
  `CallAssumption.after_early_amortization_months`, `StructuredCredit::{tranche_draws,
  tranche_readvance}`, `EligibilityRule::{max_days_past_due, exclude_non_performing}`
  on live collateral, `AssetBackedFacility::{fees, draw_schedule,
  readvance_to_borrowing_base}`, `FacilityProjection.draws`,
  `SimulationDiagnostics::{early_amortization_date, tranche_draws}`,
  `PeriodDiagnostics.servicer_advances_outstanding`.
- Analytics: `TrancheMetrics::{spread_convexity, dm_bp}` and effective
  `modified_duration` / `convexity` from ±1 bp re-projection;
  `calculate_tranche_spread_convexity`; `TranchePricingResult.paths_with_principal`
  (average life over paths that return principal).
- Python: typed `BalloonSpec`, `PrepaymentPenalty`, `SpecialServicingSpec`,
  `LiquidationSpec`, `BorrowingBaseRules`, `AdvanceRate`, `EligibilityRule`,
  `ConcentrationLimit`, `TermOutSpec`, `AmortizationEvent` (accepted wherever
  the dict form was), `TrancheCashflows.{total_deferred, total_writedown}`,
  `TrancheBuilder.non_deferrable`, `StructuredCreditBuilder.stochastic_recovery_spec`.

#### Removed

- `ReinvestmentCriteria.{maintain_credit_quality, maintain_wal}` (inert);
  `TranchePricingResult.spread` (never computed); `CoverageTestSpec.after_tranche`
  wire field (now `placement`); `BalloonSpec.default_prob` (now
  `extension_prob`); `EarlyAmortizationSpec.max_cumulative_loss_pct` (now the
  decimal `max_cumulative_loss`).

### Structured credit: collateral-vertical coverage (2026-09-15 plan, Phases A–F)

#### Added

- Coverage tests are waterfall positions (`PaymentType::CoverageTest` tiers
  carrying `CoverageTestSpec`s, default position after the tested class's own
  coupon, `after_tranche` to move it); only the interest still undistributed
  below the test is diverted, up to the binding cure.
- `LossAllocationPolicy` (`WriteDown` / `ParPreserving`) on the deal, with
  deal-type defaults; par-preserving notes realize shortfalls at legal final.
- Deal-level reinvestment: `ReinvestmentPeriod { amortizing_tranches,
  assumptions: ReinvestmentAssumptions }` books synthetic `REINVEST-{n}` rows
  (spread, price, maturity, coupon floor); notes are held flat unless listed.
- Waterfall funding sources (`WaterfallTier.funding`,
  `StructuredCredit.principal_covers_senior_interest`), hedge swaps as
  senior/junior fee recipients (`hedge_swaps: Vec<HedgeSwap>`), template
  fees with a subordinated management fee and an equity-IRR incentive fee
  (`TemplateFees`, `IncentiveFeeSpec`).
- `CoverageRules { rating_haircuts, defaulted_valuation, ccc_bucket,
  discount_obligation, borrowing_base }` for the OC numerator;
  `CoverageRules::clo_standard()`; `PoolAsset.market_price_pct`.
- Behavioral model library: `PrepaymentCurve::{Abs, Vector}`,
  `DefaultCurve::{Vector, CumulativeLoss, Timing}`,
  `RecoveryModelSpec.severity_vector`; roll-rate delinquency with servicer
  advancing and modifications (`DelinquencyModel`,
  `PoolAsset.delinquency_buckets`, `StepDownTrigger::MaxDelinquency`); card
  master trusts (`CardPortfolioSpec`, `EarlyAmortizationSpec.min_excess_spread_3m`,
  `abs_excess_spread`, `abs_payment_rate`, `abs_delinquency` metrics).
- Deal terms and analytics: `CallAssumption` (deal or tranche scope) with
  `liquidation_price_pct`, `TrancheMetrics.{wal_to_call, z_spread_to_call_bp,
  dm_to_call_bp}`; `SimulationDiagnostics.periods` (per-period pool,
  collections, accounts and coverage-test record); `calculate_equity_metrics`
  (`EquityMetrics`); CMBS terms (`BalloonSpec`, `PrepaymentPenalty`,
  `SpecialServicingSpec`, loan-level `noi` DSCR,
  `PaymentCalculation::PercentageOfSpecialServiced`).
- Attachment/detachment points are optional and derived from balances by
  payment priority (`Tranche::from_balance`, `TrancheStructure::from_balances`).
- `AssetBackedFacility` instrument (warehouse line: `BorrowingBaseRules`
  advance rates, eligibility and concentration limits, borrowing-base
  coverage test, unused fee, revolving period, early-amortization events,
  term-out) with `abf_*` metrics, JSON tag `asset_backed_facility`, typed
  Python (`AssetBackedFacility`, `FacilityProjection`) and WASM classes;
  `CoverageTestType::BorrowingBase` for any deal.
- NPL/RPL resolution timelines (`PoolAsset.liquidation: LiquidationSpec`).
- Python typed structured-credit surface: `PoolAsset` keyword constructor,
  `CallAssumption`, `CoverageRules`, `HedgeSwap`, `Waterfall`,
  `EquityMetrics`, `TrancheCashflows`, one builder setter per deal field,
  getters for every field, `SimulationDiagnostics.periods` /
  `to_dataframe` / `coverage_tests_dataframe`; typed curve-shape
  constructors on the cashflow specs.

#### Changed (BREAKING)

- Tranche prices and quotes are per CURRENT face (`price_pct`,
  `market_price_pct`, `quoted_clean_price`, `OasResult`, `ScenarioCell`).
- The two structured-credit regression goldens were re-blessed
  (`expected_loss` under par-preserving notes; hedge swap and `abs_speed`
  inputs replaced by their supported equivalents).
- Python `AssetPool.assets(value)` renamed to `with_assets`; the `assets`
  property now returns typed `PoolAsset` rows (`asset_records` keeps dicts).
- `AssetBackedFacility::example` is fallible; `InstrumentJson::AssetBackedFacility`
  is boxed.

#### Removed

- `Tranche.is_revolving`, `Tranche.can_reinvest`, `Tranche.target_balance`
  (Rust, Python, `.pyi`, JSON fixtures, WASM facade).
- `WaterfallTier.divertible`, `Waterfall.coverage_triggers`.
- `CoverageTestRules` (replaced by `CoverageRules`).
- `DealConfig`, `DealDates`, `CoverageTestConfig`, `DefaultAssumptions`.
- `Overrides.abs_speed` (use `PrepaymentModelSpec::abs`); registry
  `auto_abs` record, `default_auto_abs_speed`, `default_auto_ramp_months`,
  `seasonality`, `asset_type_defaults`, per-profile `assumptions`,
  `rmbs_standard_cpr`, `rmbs_standard_sda`, the seasonality constants.
- `hedge_npv`, `price_with_hedges`, `price_with_metrics_standalone`.
- `cleanup_call_premium`.
- `CardPortfolioSpec.purchase_rate` / `seller_interest_pct` were never added
  (the pool is the investor interest).

## [0.8.0] - 2026-09-06

### Changed (BREAKING) — post-RC tightening

- `AsianCall::new`, `AsianPut::new`, and both `with_history` constructors now
  return `Result`. A payoff requires at least one future or historical fixing;
  fully observed contracts may still have no future fixing steps.
- Portfolio margin-result JSON uses the margin crate's canonical
  `SimmSensitivitiesJson` tuple arrays for nested sensitivities. The duplicate
  portfolio-specific sensitivity object format was removed.
- FX barrier, digital, and touch options reject a monitoring start date after
  the valuation date.
- Black-Scholes and related model entry points reject non-positive spot or
  strike, negative volatility or expiry, and non-finite numerical inputs.
- `Rate` conversion, percentage construction, and rate negation return `Result`
  instead of panicking or silently wrapping invalid values.
- Callable bonds reject the `discounting` model. Use `tree` for rates-only
  optional pricing or `rates_credit` for joint rates-credit optional pricing.
- Lookback, goal-seek, covenant, carry-metric, and rolling-risk entry points
  fail closed on non-finite, out-of-range, or missing required inputs.

### Removed (BREAKING) — post-RC surface cuts

- `PvDiscountSource::Market`; callers pass an explicit discount source.
- Unused public CS01/hazard helpers and the unused hazard CS01 metric path.
- Compatibility and orphan surfaces including the option-market forwarding
  module, FX barrier payoff alias, empty ILB pricer module, reinvestment
  manager, RMBS WAL calculator, ASW configuration, basket-exposure metric,
  linear credit facade, inert ECL write-offs, formula-check alias,
  `PeriodDataFrame` export, `Bond::pricing_cashflows`, unread XVA
  config/netting wrappers, and speculative custom covenant evaluator hooks.
- Python duplicate instrument-validation helpers and unread sensitivity
  facades. Bindings take serde-owned wire shapes and typed error kinds.

### Added

- `CashFlowAccrual` records an optional `coupon_period` and
  `end_is_termination_date` so accrual day counts can honor the contractual
  coupon window and termination stub.
- `EmbeddedJsonRegistry` is public, with a document/registry split and an
  optional extension key.
- Act/Act ISMA day-count and inflation-index handling coverage.
- Documentation site under `docs-site/` with a notebook-backed curriculum.
- `finstack-quant-calibration` owns quote ingestion, market construction,
  calibration, and cached recalibration. Valuations keep instruments, pricing,
  and the recalibration port.

### Fixed

- Taylor attribution records cashflow collection failures and marks the result
  invalid instead of reporting successful theta with zero coupon income.
- Computed waterfall allocations, balances, and residual interest return
  errors when monetary conversion overflows instead of panicking.
- Taylor VaR borrows the supplied instruments directly, avoiding redundant
  instrument cloning and temporary reference collections.
- CMS option rho uses a 1 bp bump. Inflation `roll_forward` and key-rate
  bumps preserve interpolation style and extrapolation.
- Python stubs declare `PortfolioBuilder`, `PositionValue`, and
  `ReconciliationReport`, and nested credit types no longer shadow `pd` /
  `lgd` / `float` in annotations.

### Changed — model-engine consolidation (BREAKING)

- Product-independent mathematical and stochastic engines now have one owner:
  `finstack-quant-models`. Characteristic functions, DTSM, volatility
  evaluation and fitting, credit analytics, factor models and pure factor-risk
  kernels, liquidity, Hull-White equations, and structured-credit pool
  stochastic models moved out of `core`, `valuations`, `portfolio`, and the
  former standalone factor-model package.
- Core volatility surfaces and cubes are data artifacts. Model evaluation goes
  through `models::volatility::VolSource`; valuation-owned market resolution
  retains lookup and override precedence.
- Hull-White calibration remains in valuations but returns
  `models::rates::hull_white::HullWhiteParams`. Structured-credit deals,
  waterfalls, presets, and pricing remain in valuations while pool default,
  prepayment, and correlation engines live in `models::credit::pool`.
- Python model APIs now live under `finstack_quant.models`; WASM model APIs
  live under `models`. Rust and Python use snake case, while WASM retains the
  corresponding camel-case names.

### Changed — explicit recovery inputs (BREAKING)

- Calculations whose results depend on recovery now require a finite decimal
  recovery in `[0.0, 1.0]`; explicit zero remains valid. The implicit 40%
  recovery fallback and the global recovery assumption were removed.
- Because the project is pre-release, affected persisted contracts were fixed
  in place and remain version 1. This includes
  `finstack_quant.calibration/1`, `finstack_quant.credit_assumptions/1`, and
  their existing schema directories and registry keys.

### Removed — obsolete model paths (BREAKING)

- Removed the standalone `finstack-quant-factor-model` package and the old
  `finstack_quant.factor_model` host namespace.
- Removed model-bearing `core::credit`, `core::market_data::dtsm`, and
  `core::math::characteristic_function` paths.
- Removed portfolio-owned liquidity implementations, valuation-owned
  Hull-White kernels, valuation-owned structured-credit stochastic engines,
  and all related compatibility exports.

### Changed — reusable model ownership (BREAKING)

- Credit migration, PD calibration, scoring, LGD/EAD, rating-factor, recovery
  waterfall, liability-management, and assumptions-registry engines now live
  under `finstack_quant_models::credit` and `finstack_quant.models.credit`.
  `CreditRating` remains a neutral core type, but rating-factor lookup is now a
  models-owned function.
- WASM liability-management analytics moved from `core` to `models.credit`.
  The former Rust, Python, and WASM credit-model paths were removed rather than
  retained as compatibility exports.
- The credit-assumptions contract remains
  `finstack_quant.credit_assumptions/1`; its registry key is
  `models.credit_assumptions.v1`.

### Changed — credit derivative architecture (BREAKING)

- `CDSTranchePricer::with_params` now returns `Result` and rejects invalid
  copula, recovery, quadrature, bump, correlation, settlement, and convolution
  settings before constructing numerical caches. Direct tranche pricer methods
  validate instrument invariants before valuation.
- Credit-index market dependencies are distinct from direct hazard-curve
  dependencies. Portfolio factor-model orchestration resolves the aggregate
  index to its bound hazard curve when applying credit shocks.
- CDS-index bucketed CS01 and tranche Recovery01 now propagate failed
  par-spread recalibration instead of silently switching to a different risk
  definition or frozen-curve risk.
- CDS-option quadrature and synthetic-underlying modules are crate-private;
  callers use the validated instrument and metric surfaces.

### Changed — rates lifecycle, conventions, and settlement (BREAKING)

- Cash-settled swaptions now default to
  `CashSettlementMethod::CollateralizedCashPrice`; fixed and floating
  underlier legs carry independent day-count conventions, and the
  single-curve forward shortcut is restricted to economically equivalent,
  unseasoned swaps.
- Cap/floor schedules resolve currency-standard market calendars, `expiry()`
  reports the final contractual fixing date, and an optional dated premium
  is discounted as a holder outflow until settlement.
- FRA construction rejects zero accrual periods and fixed FRAs no longer
  require a forward curve. Basis swaps reject non-positive notionals.
- Remaining rates cashflows use start-of-day valuation semantics: events on
  or before `as_of` are settled. Single-curve OIS instruments no longer
  advertise redundant forward-curve dependencies.
- Rates documentation treats the ISDA 2021 Definitions as the current
  framework and labels ISDA 2006 conventions as legacy transaction terms.

### Changed — fixed-rate bond pricing and quote conventions (BREAKING)

- `Bond::fixed` now requires an explicit `StubKind`; Python and WASM expose
  the same stub choice. Existing callers must choose `None`, short/long front,
  or short/long back instead of inheriting a silent short-front assumption.
- Bond `Instrument::value` now remains an `as_of` NPV when a clean/dirty,
  yield, Z-spread, or OAS quote drives valuation. Clean, dirty, and accrued
  metrics are consistently settlement-anchored.
- Settlement is clamped to issue date and configured calendars fail closed.
- TreasuryActual long-first-coupon pricing follows 31 CFR Part 356,
  Appendix B, and its one-cashflow yield conversion now round-trips.

### Changed — scenario P&L and stress validation

- Equity and instrument price shocks reject percentages below −100%; an exact
  −100% wipeout remains valid, but scenarios can no longer create negative
  post-shock prices.
- Scenario P&L requires canonical PV pricing on both base and stressed legs
  instead of independently falling back to `Instrument::value`.
- FX scenarios conservatively reprice the full portfolio so triangulated
  native-PV dependencies cannot reuse stale base values.
- Portfolio scenario reports carry the caller's active `FinstackConfig`.
  `operations_applied` is documented as a low-level effect count, not an
  operation-coverage ratio.

## [0.7.0] - 2026-08-17

### Removed — legacy pathways, waves 1-6 (BREAKING)

Behavioral legacy: opt-in flags that restored pre-audit behavior, `Option`
fields whose `None` arm selected older semantics, and `serde(default)`s kept
only so pre-field payloads still parsed. The workspace has no `#[deprecated]`
attributes and no `serde(alias)`, so none of this was annotation-visible.

**Breaking (Rust)**

- Removed `finstack_quant_core::cashflow::NpvOptions` and `npv_with_options`.
  `npv` / `npv_with_ctx` always exclude flows dated on or before the valuation
  date. For an investment/project NPV containing the time-0 outlay, use
  `npv_amounts`, or value one day before the earliest flow.
- Removed `CDSTranchePricerConfig::validate_arbitrage_free` and
  `with_arbitrage_validation`. Base-correlation arbitrage beyond tolerance is
  now always an error; the silent zero-protection clamp is gone.
- Removed `CDSTranchePricerConfig::enforce_el_monotonicity` (its `false` branch
  was unreachable — no setter existed). EL/WD monotonicity is always enforced.
- Removed the unreachable `CreditPdError::ZeroAnnualDefaultRate` variant.
- Removed `PortfolioEclResult::from_results`; use `from_results_with_exposures`.
- Removed the dead best-effort wrapper `TrancheCoupon::current_rate_with_index`;
  use `try_current_rate_with_index`.
- Dropped unread parameters from public functions: `split_io_po`,
  `estimate_payment_window`, `allocate_pac_support`, `calculate_pay_up`,
  `project_floating_rate`, `try_rate_for_period`,
  `InterestRateFuture::calculate_convexity_adjusted_rate`.
- Removed `CreditAttributionInput::delta_spread` (write-only; the period
  decomposition already encodes it).
- `monte_carlo::rng::fbm::HybridFbm` → `WindowedConditionalFbm` (the old name
  disclaimed itself: it is not the Bennedsen-Lunde-Pakkanen scheme).
- Removed the `BarrierType` and `Position` re-export chains. Import
  `finstack_quant_core::types::BarrierType` and
  `finstack_quant_valuations::instruments::Position` directly.
- `TestContext::interest_claim_caps` is no longer `Option`; callers must supply
  the spec-derived claim map.
- Removed the one-variant `AltmanPdCalibration` enum. `altman_*_with_pd` take
  only the input struct.
- `arrow` no longer enables the `ipc` feature (no IPC code remains).

**Breaking (Rust, JSON) — waves 13-14: margin**

- A netting set with no `margin_spec` now records an `MO-16` degradation
  instead of silently reporting gross MTM as variation margin. Repo netting
  sets reach this path: they carry a `RepoMarginSpec` the aggregator does not
  consume, so their haircut terms are absent from VM. The number is unchanged;
  what changed is that it is no longer presented as a CSA-netted call amount.
- ISDA credit-qualifying bucket tables are **required** in the SIMM registry.
  The removed fallbacks fabricated them: broad weights via `unwrap_or(85.0)`,
  a flat `0.27` inter-bucket correlation across every sector pair, per-bucket
  concentration thresholds collapsed to the aggregate, and a `0.46`/`0.42`
  intra-bucket default. None correspond to any ISDA calibration.
- Removed `SimmVersion::V2_5` and its registry entry. It shipped **no** CQ
  tables, so it ran entirely on those fabricated constants. Versions are
  selectable only when their ISDA-published tables are present. `"v2_5"` no
  longer parses.

**Breaking (Rust, JSON, Python) — waves 15-16: explicit SIMM credit classification**

- CDS and CDS-index products using SIMM require
  `OtcMarginSpec.simm_credit_classification`. Qualifying exposures carry an
  explicit ISDA sector; non-qualifying is reserved for securitizations and
  designated CNQ exposures.
- Removed the scalar credit-qualifying map and approximation. The canonical
  `credit_qualifying_delta` shape is now `(sector, name, tenor, amount)` and
  always follows ISDA §3.B bucket aggregation.
- Replaced boolean `add_credit_delta` with explicit
  `add_credit_qualifying_delta` and `add_credit_non_qualifying_delta` APIs.

**Breaking (Rust, JSON, Python) — wave 12: the waterfall can now report insolvency**

- `WaterfallSpec.available_cash_node` is **required** and `impl Default for
  WaterfallSpec` is removed. With `None` the engine skipped its entire Step-5
  block: every scheduled fee, coupon and amortization was reported paid in full
  regardless of whether the model generated the cash — uses exceeded sources,
  cash was created from nothing, and no shortfall could ever be raised. The
  cash cap now always applies.
- The "every cash-consuming category must appear in `priority_of_payments`"
  rule is unconditional (it was gated on `available_cash_node` being set).
  Specs must list `Fees`, `Interest` and `Amortization`.
- Removed `CapitalStructureWarning::SweepExcessUnallocated`. It described only
  the no-equity-bucket case, which cannot arise once the cash cap is always on;
  sweep excess beyond debt capacity now falls to the equity residual.
- `default_priority_of_payments()` is public so hosts cannot drift from the
  canonical stack (fees, interest, amortization, sweep, equity).
- Python `WaterfallSpec(...)` requires `available_cash_node`.

**Breaking (Rust behavior) — wave 11: numeric defaults**

- **Bug fix**: `LatentFactorSpec::TwoFactor { correlation: 0.0 }` collapsed into
  the single-factor arm, which shares one factor — implied correlation **+1**,
  the opposite of the independence being requested. `factor_correlation` now
  returns `Some(rho)` for any two-factor spec and `None` only for
  `SingleFactor`; `MultiFactor` errors instead of silently sharing a factor.
- The callable-bond Hull-White tree no longer supplies an invented 100 bp
  short-rate vol when none is configured. It reads `model_config.hw1f_sigma`,
  then `market_quotes.implied_volatility`, and otherwise treats rates as
  deterministic (sigma = 0) rather than manufacturing a confident option value.
- CMS instruments (`CmsSwap`, `CmsOption`, `CmsSpreadOption`) derive unset leg
  conventions from the instrument's **currency** (EUR/GBP/JPY) instead of
  hard-coded USD constants. A EUR CMS previously took its calendar from the EUR
  convention but its frequencies and day counts from USD. USD is unchanged: its
  terminal constants encode the conventional fixed-vs-3M CMS underlying, which
  is a different swap from `IRSConvention::UsdSofr` and is a separate decision.
- `MetricId::FxDelta` and `MetricId::Fx01` were numerically identical with two
  implementations; both ids now share `GenericFx01Calculator` and the bespoke
  `FxDeltaCalculator` modules in `fx_swap` and `fx_spot` are deleted.

**Breaking (Rust behavior) — wave 10: silent degradation becomes an error**

- `BermudanSwaptionPricerConfig`, `CheyetteRoughConfig` and `LmmBermudanConfig`
  default `enforce_calibration` to **true**. Pricing off the generic starting
  parameters now errors; opting out is explicit.
- Hull-White swaption calibration **rejects** a malformed per-quote accrual
  schedule instead of silently substituting the synthetic constant-dt recipe,
  which calibrated to a different instrument than the caller described. The
  `schedule_fallback_count` / `schedule_fallback_quotes` report metadata is
  gone; `schedule_source` is now exactly what the caller supplied.
- `Swaption::validate` no longer has the undocumented "compatibility date"
  escape that allowed an expiry up to 5 business days **after** `swap_start`.
  Expiry must be on or before the swap start. This corrected one golden
  fixture whose expiry (2027-05-08, a Saturday) postdated its swap start:
  expiry moved to the swap start 2027-05-05 and its NPV re-pinned
  2,278,477.91 -> 2,259,795.96 (-0.82%, three fewer days of optionality).
- `CashFlowSchedule::to_period_dataframe` requires `meta.issue_date`. Inferring
  the funding anchor from the earliest flow silently anchored accrual to a
  coupon date rather than to issuance.
- `Covenant.label` is required and `Covenant::new` takes it. `None` fell back
  to the discriminant-only `covenant_id`, so two covenants of the same type
  collided in compliance reports and breach tracking. `Covenant::with_label`
  is removed.

**Breaking (Rust) — wave 9: `Instrument` trait**

- `Instrument::market_dependencies` is now a **required** method. Its old
  default returned an empty set, so emptiness could not be distinguished from
  "never declared" — the portfolio therefore treated every empty set as
  *unresolved* and repriced those positions for every factor. All 79 real
  instruments already declared their dependencies; only test mocks relied on
  the default. An empty set now means the instrument genuinely reads no market
  data, and such positions are repriced for no factor.
- `Instrument::base_value_raw_with_currency`'s default called both
  `base_value_raw` and `base_value`, pricing the instrument twice. It now
  prices once. Identical results for the 66 instruments that use the default;
  the 13 with a distinct high-precision raw kernel already override it.

**Breaking (JSON / serde)**

- `FxMatrixState.pinned_quotes` is required. A snapshot omitting it previously
  restored a matrix with no pinned fixings, silently re-deriving those dates
  from the provider.
- `SimmSensitivitiesWire.credit_qualifying_delta` requires a sector on every
  entry; the former scalar and parallel `*_bucketed` fields are removed.
- `IssuerBetaRow.level_fit_quality` is required; `FactorVolModel` now denies
  unknown fields.
- `LoadPhase::Migrate` removed (never constructed; no migration code existed).
- `NumericMode::Decimal` removed — never emitted. The enum now has one variant.
- `XccyConventions.notional_exchange` is required in the conventions registry.
- `core::wire::non_finite_f64` rejects JSON `null` instead of decoding it as
  `NaN`. Note the round trip this closes: a JS `Infinity` written by
  `restore_non_finite_ratios` stringifies to `null`, which previously decoded
  back as `NaN` — silent corruption. It now fails loudly.

**Breaking (Rust, JSON) — waves 7-8**

- Removed `DiscountedCashFlow.discount_curve_id` and `RealEstateAsset` /
  `LeveredRealEstateEquity.discount_curve_id`. These instruments discount at
  their own WACC / cap rate; the field only forced callers to load a curve no
  computation read. They now declare no market dependency
  (`no_market_dependencies = true` in the coverage manifest). Because these
  types deny unknown fields, stored payloads carrying it must drop it.
- Removed `PoolStats.weighted_avg_life` — it was assigned
  `weighted_avg_maturity` verbatim. Use
  `AssetPool::weighted_avg_life_from_cashflows` for a real WAL.
- `AssetPool.cumulative_scheduled_amortization` is now a required `Money`.
  `None` was treated as zero, understating the original-balance denominator
  and overstating `current_loss_percentage`.
- `RangeAccrual.accrual_start_date` is required. The `None` arm inferred the
  start by extrapolating one observation interval backwards.
- Removed `FloatingLegCompounding::CompoundedInArrears.observation_shift`, a
  duplicate of the `CompoundedWithObservationShift` variant whose combination
  with `lookback_days` had to be rejected on every pricing path. Use
  `CompoundedWithObservationShift { shift_days }`. The mirrored
  `RateCalibrationOisCompounding` field goes with it.
- `monte_carlo::registry::PythonBindingDefaults` → `ConvenienceDefaults` (and
  `Python{Engine,Pricer,Lsmc,Greek}Defaults` → `Convenience*Defaults`). The
  `python_bindings` key in `data/defaults/pricer_defaults.v1.json` is now
  `convenience`; the struct feeds Rust convenience pricers too, not just
  Python. Unknown keys are denied, so existing override docs must be renamed.

**Breaking (Python)**

- Removed the pre-rename aliases `FinstackValuationError`, `FinstackFxError`,
  `FinstackOptimizationError`. Use `ValuationError`, `FxError`,
  `OptimizationError`. (`FinstackError`, the base class, is unchanged.)
- `scoring.AltmanPdCalibration` removed; the `pd_calibration` argument on
  `altman_z_score` / `altman_z_prime` / `altman_z_double_prime` is now
  `with_implied_pd: bool = False`.
- `NumericMode.decimal()` removed.

**Breaking (WASM)**

- `Portfolio.validateMaterializationJson` → `Portfolio.validateMaterialization`.
  It returns a typed report object, so the `Json` wire suffix was wrong; the
  Python and WASM names now match and the rename-map entry is gone.

### Changed — result-return standardization (BREAKING)

Public APIs now hand results back the same way in Rust, Python, and WASM. The
full contract is recorded in `.claude/skills/finstack-consistency-reviewer/conventions.md`.
`to_json` / `from_json` still work everywhere — every converted entry point's
previous string output is available by calling `.to_json()` on the result.

**Rust**

- `finstack_quant_core::config::ResultsMeta` gained `parallel: bool`. It is
  omitted from JSON when false, so existing payloads and golden files are
  byte-identical for serial runs.
- `finstack_quant_statements::evaluator::ResultsMeta` → `EvalStats` (execution
  statistics, distinct from the workspace audit stamp). Serde field names are
  unchanged; the JSON Schema definition is renamed `StatementResultsMeta` →
  `StatementEvalStats`.
- `scenarios`: `ApplicationReport.rounding_context: Option<String>` →
  `meta: Option<ResultsMeta>`. `ApplicationEnvelope.market_json: String` /
  `model_json: Option<String>` → `market` / `model` as nested JSON objects.
- Typed twins are now public where only a JSON string was reachable:
  `attribute_return_contribution(&spec)` and `allocate_weights(&spec)` return
  typed results; the string forms are `*_json`.
- `margin`: three scalar/`_result` API pairs collapsed — `calculate_for_notional`,
  `calculate_netting_set_with_ngr`, and `calculate_for_collateral` now return
  `ImResult` (the scalar is the `amount` field).
- `XvaResult`, `EadResult`, `FrtbSbaResult`, `CovenantReport`, and the ECL
  results now stamp `meta: ResultsMeta`.
- Duplicate type names resolved: `CalibrationResult` → `TreeCalibrationResult`
  (short-rate tree), `ValidationReport` → `CalibrationValidationReport`
  (calibration validator), `WaterfallPeriodResult` → `CmoWaterfallPeriodResult`.
- `portfolio::performance`: `twrr_modified_dietz` and `twrr_linked` return
  `Result<_>` instead of `Option<_>`, so invalid inputs report why.

**Python**

- `attribute_pnl` returns `PnlAttribution` (its docstring already claimed this).
- `attribute_return_contribution` returns a new typed `ReturnContributionResult`
  with `to_dataframe()` and `to_series()`.
- `scenarios.apply_scenario` / `apply_scenario_to_market` return a typed
  `ApplicationResult` (`.market`, `.model`, `.report`) instead of a dict of JSON
  strings. `ApplicationReport` is a new typed class.
- `StatementResult.to_pandas_long` / `to_pandas_wide` → `to_dataframe(orient=...)`.
- `from_json` is `@staticmethod` everywhere (49 `@classmethod` conversions).
- `to_json()` emits compact JSON everywhere. Schema-document emitters
  (`*_schema`, `schema.index`) stay pretty-printed by design.
- The analytics domain and the Monte Carlo estimates gained `to_json` /
  `from_json` / `__reduce__`; they previously had no serialization at all.

**WASM**

- All 19 raw `serde_wasm_bindgen::to_value` call sites now route through
  `crate::utils::to_js_value`. Rust maps previously arrived as ES `Map`s in
  those returns, which `JSON.stringify` silently drops; they are now plain
  objects, matching `index.d.ts` and the Python dict shapes. `mise run
  wasm-lint` now fails if the raw serializer reappears.
- 47 exports converted from JSON strings to structured objects: the four
  `priceInstrument*` entry points and `calibrate`; 31 portfolio exports
  (valuation, aggregation, attribution, optimization, risk decomposition,
  liquidity); `covenants.evaluateEngine`; `statements.evaluateModel`,
  `evaluateModelWithMarket`, `runMonteCarlo`; six statements-analytics
  analyses; and `scenarios.computeHorizonReturn`.
- Wire surfaces that keep returning strings gained honest names: the seven
  covenant functions, five statements validators, two margin CSA specs, and
  `parsePortfolioSpec`/`buildPortfolioFromSpec` are now `*Json`.
- Prose-returning exports are now `*Text`: `parseFormulaText`,
  `plSummaryReportText`, `creditAssessmentReportText`. They previously shared
  the `Result<String, JsValue>` signature with ~130 real JSON exports, so
  callers had no way to tell prose from a parseable document.
- Numeric vectors cross as `Float64Array` instead of boxed-`Number` arrays
  (`correlationBounds`, `jointProbabilities`, `nearestCorrelation`,
  `generateSmile`, the two coupon profiles, `expiries`, `pillarVols`). Three
  of these were already *declared* `Float64Array` in `index.d.ts` and had been
  lying about their runtime type.
- `Portfolio.toSpecJson` → `toJson`; `margin.calculateVm` takes an ISO-8601
  date string instead of three integers; the comps functions and
  `portfolioResultGetMetric` return `undefined` rather than `null` for absent
  values.
- The JS facade is a pure namespace re-export again — `exports/valuations.js`
  no longer `JSON.parse`s `calibrate` while leaving its sibling `dryRun` alone.

### Known gap

Some entry points still return bare JSON strings under unsuffixed names in
*both* Python and WASM (parts of statements-analytics, factor-model, and the
scenario spec builders). They are consistent with each other but not with the
contract; converting one side alone would create the cross-language divergence
this work removes, so they are listed as paired follow-ups in
`.claude/skills/finstack-consistency-reviewer/conventions.md`.

### Added

- **Python:** ~90 `to_dataframe` / `to_*_dataframe` exports across portfolio,
  statements, statements-analytics, margin, scenarios, analytics, core.credit,
  core.market_data, monte_carlo, factor-model and valuations.correlation. Every
  frame documents its columns; `Money` becomes a float column plus one
  `currency` column, never a nested cell. 52 result types also gained
  `_repr_html_`, so they render as tables in Jupyter.
- **Python:** pickle support on every wrapper that round-trips through JSON, so
  results survive `multiprocessing` / `joblib` / `dask`. `copy.deepcopy` works
  through the same path. `PortfolioOptimizationResult` is the one exception: its
  Rust type has no `Deserialize`.
- **Python:** `finstack_quant.<domain>.schema` for `valuations`, `statements`,
  `factor_model` and `cashflows`, exposing the JSON Schemas compiled into the
  extension, so they can never drift from the installed wheel.
- **Python:** `finstack_quant.__version__`, sourced from the crate version.
- **Python:** `FinstackError` (`finstack_quant.core`) as the common base for the
  library's named exceptions. It derives from `ValueError`, so every existing
  `except ValueError` is unaffected. `CalibrationEnvelopeError` stays outside the
  tree because it is a `RuntimeError` and PyO3 cannot express two bases.
- **Rust, JSON:** clean-price CDS-option strikes (`CDSOptionStrike::CleanPricePct`),
  the CDX HY market convention, alongside the existing forward-spread strikes.
  Price strikes carry `strike_index_factor` (the strike's original index factor
  `f0`) and are validated as index-only, no-knockout, with an explicit positive
  coupon, factors in `(0, 1]`, `f <= f0`, and realized loss bounded by the removed
  original notional. Delta and gamma branch by strike kind: a clean price is not a
  valid argument to the Black `d₁`, so price strikes use a curve-reprice hedge
  ratio (option CS01 over underlying spread DV01 under a symmetric ±1 bp par-quote
  bump with hazard rebootstrap, sticky native strike, sticky surface volatility),
  and gamma is the change in that delta across the ±5 bp screen bump.
- **Rust:** a two-factor rates-credit lattice (`models::trees::two_factor_rates_credit`)
  for callable credit-risky bonds and term loans, with public `ModelConfig` inputs
  `hazard_volatility`, `hazard_mean_reversion` and `rate_credit_correlation`. One
  `resolve_rates_credit_config()` owns the public-config to lattice-input mapping
  for both engines. Correlation feasibility is proved against the per-node Fréchet
  bounds at calibration time rather than clamped during pricing, and
  `HazardFloorSaturation` reports how much state-price mass sat on the zero-hazard
  floor. Mean-reversion speeds above `KAPPA_MAX` are rejected — use `HullWhiteTree`.
- **Rust:** future floating-coupon resets valued at lattice nodes (`NodeCoupon`).
  The deterministic projection stays booked as before; the lattice adds only the
  node-dependent increment, which vanishes identically as `rate_vol → 0`, so an
  option-free floater's PV is invariant to rate volatility. Caps and floors on a
  floating leg are therefore priced as the caplet/floorlet strip they are. Call
  dates strictly inside a future floating period, and future floating PIK, are
  rejected rather than mispriced.
- **Rust:** `models::credit::market_anchored`, the shared fractional-to-absolute
  credit-volatility mapping (credit triangle `s = (1 − R)·λ`), used by both the
  callable lattice and the revolving-credit CIR path so they cannot drift apart.
  `CreditVolatilityConversion` carries every input and output together, so a 35%
  CDS-option quote cannot reach an additive hazard lattice without the 1.05%
  absolute figure sitting beside it. This is an explicit first-order local
  mapping, not a calibration.
- **Rust:** `market::credit_option_vol`, which resolves a CDX/iTraxx option-vol
  surface point into that additive hazard volatility. The surface is queried
  strictly at the strike's **native displayed** coordinate — a decimal spread for
  CDX IG and iTraxx, a clean price in percentage points (`107.0`, never `1.07`
  and never a spread equivalent) for CDX HY — and the index-derived *fractional*
  vol is anchored on the *target* curve's own reference hazard, so a single-name
  bond is never anchored to the index's hazard level. Selectors are `Strike`,
  `Moneyness` (against the native ATM-forward coordinate) and `Delta`.

### Changed

- **Breaking (Rust, JSON, Python, WASM):** the CDS-option strike is a typed enum
  instead of a bare decimal. `CDSOption.strike` and `CDSOptionParams.strike` are
  now `CDSOptionStrike`, whose canonical JSON is externally tagged, and **the old
  scalar wire shape is rejected with no compatibility fallback**:

  ```json
  { "strike": "0.0325" }                      // before — now rejected
  { "strike": { "spread": "0.0325" } }        // after, forward-spread strike
  { "strike": { "clean_price_pct": "107.0" } } // after, CDX HY clean-price strike
  ```

  Persisted `CDSOption` payloads must be migrated; Python and WASM reach CDS
  options through this JSON, so they are affected identically. `Spread` stays a
  decimal annual rate (`0.0325` = 325 bp) and `CleanPricePct` is quoted in
  percentage-price points (`107.0` = fraction `1.07`) — use
  `clean_price_fraction()` rather than re-dividing by 100. Spread strikes reject
  `strike_index_factor` as inert. `effective_underlying_cds_coupon` is now
  fallible, since a clean-price strike can never serve as the running coupon, and
  `settlement` is explicit on `CDSOptionParams` (default `Cash`).
- **Breaking (Rust behavior):** the callable rates-credit path reads only
  `hw1f_sigma` / `hw1f_mean_reversion` and **rejects** the legacy
  `implied_volatility` / `mean_reversion` channel when its canonical counterpart
  is absent, so a configuration cannot silently flip pricing regime; `hw1f_*` wins
  when both are set. Hazard inputs supplied without a `credit_curve_id` are
  rejected rather than ignored.
- **Breaking (Rust behavior, PV-affecting):** `RatesCreditConfig::default()` is
  now deterministic in both factors. The callable-bond path had been inheriting an
  undeclared `hazard_vol = 0.20` — worth roughly 33% of PV on the reference
  fixture — from `..Default::default()` construction. Callers that want a
  stochastic factor must now declare it explicitly, so previously-priced callable
  credit-risky bonds will change value.
- **Breaking (Rust behavior):** CDS-option volatility resolution is strict. An
  instrument-level implied-vol override wins; otherwise the surface is looked up
  at the native strike coordinate with a required `VolSurfaceAxis::Strike` and
  `VolQuoteType::BlackLognormal`, and out-of-grid coordinates error instead of
  clamping to the nearest edge. Delta and gamma screen metrics share this
  resolver. Valuing a physically-settled option at or after expiry now fails
  explicitly; the exercise/delivery lifecycle is not modelled.
- **Fixed (Rust):** bond rate vega bumped `market_quotes.implied_volatility`,
  which the rates-credit path no longer reads, producing a silently **zero** vega
  on every credit-risky callable (or an error when `hw1f_sigma` was unset). Vega
  now bumps whichever channel the instrument's own routing consumes.
- **Breaking (Python):** 40 `Performance` metrics return a `pandas.Series`
  indexed by ticker (with `.name` set to the metric) instead of `list[float]`;
  `skew_kurt` and `value_at_risk_and_es` return a tuple of two Series. Positional
  access must become `.iloc[i]` — `perf.sharpe()[0]` warns on pandas 2 and raises
  on pandas 3. `perf.sharpe()["FUND"]` is the preferred form, and
  `pd.concat([...], axis=1)` now yields correctly named columns.
- **Breaking (Python):** the pricers return typed results instead of JSON
  strings — `price_instrument` / `price_instrument_with_metrics` →
  `ValuationResult`, and `structured_credit_tranche_oas` / `_metrics` /
  `_scenario_table` → `OasResult` / `TrancheMetrics` / `ScenarioTable`. Replace
  `ValuationResult.from_json(price_instrument(...))` with the call itself, and
  `json.loads(result)` with `result.to_json()` where the wire payload is still
  wanted. `instrument_cashflows_json` is unchanged and still returns `str`.
- **Breaking (Python):** `ModelBuilder` / `MixedNodeBuilder` configuration
  methods return the builder instead of `None`, so calls chain. Statement-per-line
  code is unaffected (the object is mutated in place and the returned value *is*
  the same builder); the only visible change is that a notebook cell ending on a
  setter now echoes a builder repr. `build()` and `mixed()` remain terminal.
- **Python:** every date-valued `as_of` parameter accepts a `datetime.date` /
  `pandas.Timestamp` as well as an ISO string — the pricers, the scenario entry
  points, the `structured_credit_tranche_*` family, the portfolio sensitivity,
  stress, what-if and aggregation functions, `accrued_interest_json`,
  `evaluate_engine`, `decompose_levels` and `run_corporate_analysis`. The two
  functions that take a fiscal *period* rather than a date
  (`credit_assessment`, `credit_assessment_report`, e.g. `"2025Q4"`) still take
  a string, since a calendar date has no meaning there. A malformed date string
  is now rejected by the shared extractor, so the `ValueError` names the
  offending value and the reason and reads the same at every entry point.
- **Python:** zero-row result frames now carry their real dtypes instead of
  `object`. Concatenating an empty frame with a populated one previously
  downgraded every column, so a numeric column silently stopped being numeric
  and `groupby().sum()`, arithmetic and `to_parquet` broke — on the common path
  of iterating a book where some entries produce no rows.
- **Python:** `PnlAttribution.to_dataframe()` now raises `ValueError` when a
  factor's currency differs from `total_pnl`'s. The single-row frame carries one
  `currency` label for every factor, so a mixed-currency attribution would have
  made `df[factors].sum(axis=1)` add unlike units. Use `to_long_dataframe()`,
  which carries currency per row, for genuinely mixed input.

- Canonicalized editor and agent rules under `.agents/rules`. Cursor and
  Claude now resolve the same reviewed rule tree through checkout-relative
  `.cursor/rules` and `.claude/rules` symlinks.
- **Breaking (Rust, Python, JSON):** Removed `VolSurfaceKind` and the redundant
  `surface_kind` field from direct and hierarchy volatility-surface shocks;
  `vol_surface_id` now fully identifies the target surface.
- **Breaking (Rust, Python, JSON):** Removed the unused
  `OperationSpec::BaseCorrBucketPts.maturities` field and Python argument;
  base-correlation shocks target the supported detachment dimension only.
- **Breaking (Rust, JSON):** Removed the unused
  `IndexUnderlyingParams.convexity_id` field and its `with_convexity` builder;
  fixed-income index TRS pricing never consumed the identifier.
- **Breaking (Rust, JSON, WASM):** Removed the unwired
  `RateQuote::Futures.vol_surface_id` field and its persisted
  `RateCalibrationQuote` copy. Futures calibration continues to use the
  explicit `convexity_adjustment` value.
- **Breaking (Rust, Python):** Removed statement formula alias and fuzzy-name
  rewriting, including `ModelBuilder::with_name_normalization`. Formulas and
  `where` clauses now require exact node IDs; unknown IDs retain the dependency
  graph's nearest-name diagnostics.
- **Breaking (Python):** Removed the 13-argument `SaCcrTrade` constructor,
  which inferred supervisory delta and option classification from direction.
  Construct trades with `SaCcrTrade.from_json` and the complete canonical
  schema; deserialization now immediately applies the Rust SA-CCR regulatory
  validator.
- **Breaking (Rust, ECL policy JSON):** Simplified ECL staging and
  schedule-based calculation now use canonical `EclStageRequest` and
  `EclRequest` surfaces. The duplicate binding-default policy block, its seven
  Rust getters, and `compute_ecl_weighted_from_schedules` were removed; Python
  retains its established signatures and error types while delegating all
  policy, exposure, scenario, and configuration construction to Rust.
- **Breaking (Rust, JSON, Python):** Removed the unused
  `ScenarioDefinition.model_id` field and Python `ScenarioSet.model_ids`
  constructor input. Scenario-set JSON containing `model_id` is now rejected
  by the existing unknown-field validation.
- **Breaking (Rust, Python):** Corporate analysis now stores direct
  `CreditContextMetrics` per instrument instead of the speculative
  `CreditInstrumentAnalysis` wrapper. Non-positive DCF enterprise-value
  suppression is reported once at the top level as
  `ev_suppressed_non_positive`.
- **Breaking (Rust):** Removed the unused
  `CreditScoringError::OutOfRange` variant. Credit scoring input failures
  continue to use `NonFiniteInput` and `InvalidBinaryIndicator`.
- **Breaking (Rust):** Removed the unused `MigrationError::NotSquare` variant.
  Credit migration matrix shape failures continue to use `DimensionMismatch`.
- **Breaking (Rust):** Removed the unused
  `InputError::JointCalendarNonConvergent` variant. Active joint-calendar safety
  failures continue to use `JointCalendarIterationLimitExceeded`.
- **Breaking (Rust):** `ThresholdSchedule::new` now returns `Result` and is the
  sole threshold-schedule constructor; `try_new` was removed. Deserialization
  routes through the same finite-value and unique-date validation.
- **Breaking (Python):** Removed the module-level `price_european_call` and
  `price_european_put` functions. Use `EuropeanPricer.price_call` /
  `price_put` or `McEngine.price_european_call` / `price_european_put`.
  WASM retains `priceEuropeanCall` and `priceEuropeanPut` as
  function-oriented browser entry points.
- **Breaking (Rust, Python, WASM):** Kyle calibration now requires an explicit
  `reference_price` and returns price-space lambda. The working `*_with_mid`
  implementations now own the canonical `KyleLambdaModel::lambda_from_series`
  and `from_amihud` names; the legacy fail-closed signatures were deleted.
- **Breaking (Rust, Python, WASM):** `almgren_chriss_uniform_impact` now returns the canonical
  `ImpactEstimate`; the duplicate `AlmgrenChrissImpactView` was removed.
  Python and WASM now return the same five canonical fields, including
  `total_cost`, `cost_bp`, and `execution_risk`.
- **Breaking (Rust):** Removed the no-op `JumpEuler::with_max_jumps`
  constructor. Use `JumpEuler::new`; the aggregate jump sampler remains
  uncapped.
- **Breaking (Rust behavior):** Statement formula checks now use the canonical
  statements DSL evaluator, including time-series functions and `cs.*`
  references. `CheckSuiteSpec::resolve()` materializes formula checks directly;
  the duplicate analytics resolver was removed, and missing references or
  evaluation failures are returned instead of being silently skipped.
- Comparable-company flat field construction, named-field access, and metric
  selector parsing now share one canonical Rust implementation across scoring,
  Python, and WASM; accepted fields and scoring behavior are unchanged.
- **Breaking (Rust, Python, WASM):** Statement-model Monte Carlo now belongs
  exclusively to `finstack-quant-statements`. Import `MonteCarloConfig`,
  `MonteCarloResults`, and `run_monte_carlo` from `finstack_quant.statements`,
  or call `statements.runMonteCarlo` in WASM; the duplicate
  `statements_analytics` facade was removed without changing JSON payloads.
- **Breaking (Python):** `reporting.attribution_tearsheet` is now
  presentation-only and accepts only a precomputed `PnlAttribution` or its
  canonical JSON/dict payload. Inline instrument/market attribution parameters
  were removed; compute attribution through `finstack_quant.attribution` before
  rendering.
- **Breaking (Python):** `reporting.instrument_tearsheet` is now presentation-only
  and requires a precomputed `ValuationResult`; its `market`, `as_of`, `model`,
  and `market_price` parameters were removed. The presentation-owned
  `recommended_metrics` helper was also removed, so callers select metrics in
  the valuations API before rendering.
- **Breaking (Rust/Python):** Removed the unused SA-CCR engine
  `reporting_currency` field, builder option, and Python constructor argument.
  SA-CCR monetary inputs must already use one consistent currency; the engine
  does not perform currency conversion.
- **Breaking (Rust):** Removed the unused FRTB SBA engine
  `reporting_currency` field and builder option. Currency remains explicit on
  `FrtbSensitivities`, where it participates in the regulatory input contract.
- **Breaking (Rust):** Removed the speculative FRTB parameter-bundle and
  revision APIs, including `FrtbParams`, `FrtbRevision`, the JSON-overlay
  registry, and the corresponding `FrtbSbaEngine` builder and accessors. FRTB
  SBA continues to use the fixed BCBS d457 constants under
  `regulatory::frtb::params`.
- **Breaking (Rust):** Removed the duplicate scenario tenor and period parsing
  helpers, including their context wrappers. Use
  `finstack_quant_core::dates::Tenor` directly for parsing, simple year/day
  approximations, and calendar-aware year fractions.
- **Breaking (Rust):** `InterpolationResult` and
  `calculate_interpolation_weights` are now crate-private scenario adapter
  details and no longer part of the public or serialized API.
- **Breaking (Rust):** `TemplateRegistry` now has one validated registration
  path. Use the fallible `TemplateRegistry::with_embedded_builtins`,
  `register_json_template_str`, or `load_json_dir`; the builder-factory
  `register` / `register_with_components` methods and panicking `Default`
  implementation were removed.
- **Breaking (Rust):** Simplified the valuations surface. Use
  `schema::instrument_schema("bond")`, `TreePricer::calculate_oas`, and the
  free `solve_ytm` function in place of the removed bond wrappers and YTM
  solver objects. Tranche loss methods now use their stored balance, the
  valuations prelude no longer re-exports the core prelude, unused constants
  were removed, and rate-exotic Monte Carlo settings must be constructed as
  typed `RateExoticMcConfig` values.
- **Breaking (Rust):** Removed low-value public aliases and helpers from the
  foundational crates: use `analytics::regression::constrained_least_squares`
  and the correlation crate-root exports; construct `DatedSeries` with its
  public fields or `Default`; evaluate covenant schedules through covenant
  APIs; and import core `Error` / `Result` directly. The unused credit-calibrator
  `config` and `diagnostics` accessors were also removed.
- **Breaking (Rust, JSON, Python, WASM):** Removed the duplicate feature-operation
  names `clip_by_quantile` / `ClipByQuantile` and
  `dollar_neutral_weights` / `DollarNeutralWeights`. Use `winsorize` /
  `CrossSectionalOp::Winsorize` and `long_short_weights` /
  `CrossSectionalOp::LongShortWeights` in typed Rust calls, JSON panel specs,
  and Python or WASM string-dispatched calls.
- **Breaking (Rust):** Removed the public `CagrBasis` and
  `AnnualizationConvention` configuration types. `Performance::cagr()` keeps
  its existing date-based Act/365.25 behavior.
- **Breaking (Rust):** Factor-model configuration, covariance, envelope,
  primitive, and sensitivity types are now exposed only through their existing
  crate-root paths. The `matching`, `credit`, and `schema` modules remain public.
- **Breaking (Rust):** Removed the unused `FactorModelError` hierarchy. Factor-model
  workflows continue to return their canonical core or portfolio errors, and
  `UnmatchedPolicy` remains available at the factor-model crate root with the
  same `snake_case` JSON representation.
- **Breaking (Rust):** Renamed the serialize-only scenario outputs
  `ScenarioRevalueEnvelope` and `ScenarioPnlEnvelope` to
  `ScenarioRevalueView` and `ScenarioPnlView`, and renamed their helpers to
  `apply_and_revalue_view` and `scenario_pnl_view`. No deprecated aliases are
  provided because these pre-1.0 outputs are not round-trip persistence
  envelopes.
- **Breaking (JSON):** Attribution request deserialization now rejects missing
  or mismatched `schema` markers, matching the published schema `const` and
  existing attribution-result behavior.
- **Breaking (JSON):** Margin serialization and deserialization now enforce the
  published bounds for MPOR, collateral maturities, haircuts, concentration
  limits, default haircuts, and notification hours. Invalid publicly
  constructible values fail serialization instead of producing JSON that the
  same type cannot deserialize.

### Fixed

- Period cash / total-return carry no longer treats a deposit's opening
  notional draw (effective start) as buy-and-hold income. Bonds already
  skipped the issue-date draw; deposits now use the same rule, so a
  position opened on `as_of_t0` does not book `−notional` (FX-converted)
  into `carry` and `total_pnl`.
