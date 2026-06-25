# Foundations API Coverage Audit (01_foundations notebooks)

Date: 2026-06-23 (automated review during implementation)

Scope: Public symbols under `finstack_quant.core.*` that belong to the foundations vertical per parity_contract.toml and .pyi (currency, money, types, config, dates, market_data, math, rating_scales). Higher-tier usage (instruments, analytics, full credit models) is out of scope.

Legend:
- ✅ Demonstrated with runnable example in 01_foundations tree
- ⚠️ Partially / indirectly (e.g. via registry key string only, or higher module)
- ❌ Not demonstrated in foundations notebooks

## Summary Table

### core.currency
| Symbol | Status | Notes / Notebook |
|--------|--------|------------------|
| Currency (ctor, from_numeric, code, numeric, decimals, to/from_json, ==/!=/</>, compare str) | ✅ | core_types_and_money.ipynb |
| Module constants (USD, EUR, ...) | ✅ | core_types_and_money.ipynb |

### core.money
| Symbol | Status | Notes / Notebook |
|--------|--------|------------------|
| Money (ctor with str/Currency, zero, amount, currency, format, to/from_json, to/from_tuple, + - * / unary-, cross-currency ValueError) | ✅ | core_types_and_money.ipynb + mini example |

### core.types
| Symbol | Status | Notes / Notebook |
|--------|--------|------------------|
| Rate (decimal, from_percent, from_bps, as_*, ZERO, arith) | ✅ | core_types_and_money.ipynb |
| Bps (as_*, ZERO, arith) | ✅ | core_types_and_money.ipynb |
| Percentage (as_*, ZERO) | ✅ | core_types_and_money.ipynb |
| CreditRating (AAA etc, from_name with notch normalization) | ✅ | core_types_and_money.ipynb |
| CurveId, InstrumentId (as_str) | ✅ | core_types_and_money.ipynb |
| Attributes (set/get/contains/keys/len) | ✅ | core_types_and_money.ipynb |

### core.config
| Symbol | Status | Notes / Notebook |
|--------|--------|------------------|
| RoundingMode (BANKERS etc, from_name) | ✅ | core_types_and_money.ipynb |
| ToleranceConfig (rate_epsilon etc) | ✅ | core_types_and_money.ipynb |
| FinstackConfig (ctor default, output_scale/ingest_scale, extensions, to/from_json, remove) | ✅ | core_types_and_money.ipynb + registry_defaults_and_overrides.ipynb (FinstackConfig() shown as canonical default) |

### core.dates
| Symbol | Status | Notes / Notebook |
|--------|--------|------------------|
| DayCount (all 8: ACT_360..BUS_252, from_name, year_fraction, calendar_days; DayCountContext for ISMA/BUS) | ✅ | dates_calendars_schedules.ipynb + dates/day_count_conventions.ipynb |
| signed_year_fraction | ❌ | Not shown |
| Tenor (parse, daily/weekly/.../annual classmethods, count/unit/months) | ✅ | dates_calendars_schedules.ipynb + schedule notebooks |
| PeriodKind, PeriodId (month/quarter/annual/half/week/day, parse, next/prev, code), build_periods (with/without actuals_cutoff) | ✅ | dates_calendars_schedules.ipynb |
| build_fiscal_periods, FiscalConfig (calendar_year, us_federal, uk, ...) | ❌ | Only string mentions of "calendar_year_*"; no FiscalConfig or build_fiscal_periods |
| BusinessDayConvention (all 5 + from_name) | ✅ | Multiple dates/* notebooks |
| HolidayCalendar (code, is_holiday/is_business_day, metadata), available_calendars, adjust(date, conv, cal) | ✅ | dates/holiday_calendars.ipynb + dates_calendars_schedules.ipynb |
| StubKind (NONE/SHORT_*/LONG_* + from_name) | ✅ | dates/schedule_building.ipynb |
| ScheduleErrorPolicy (STRICT/MISSING_*/GRACEFUL_EMPTY) | ✅ | dates/schedule_building.ipynb |
| Schedule (dates, has_warnings, used_graceful_fallback, warnings, len) | ✅ | dates/schedule_building.ipynb |
| ScheduleBuilder (frequency, stub_rule, adjust_with, end_of_month, cds_imm, imm, error_policy, build) | ✅ | dates/schedule_building.ipynb + dates_calendars_schedules.ipynb |
| create_date, days_since_epoch, date_from_epoch_days | ⚠️ | Low-level; not explicitly demoed but available |

### core.market_data
| Symbol / Area | Status | Notes / Notebook |
|---------------|--------|------------------|
| DiscountCurve (id, base_date, df, zero, forward, knots, day_count) | ✅ | market_data_and_curves.ipynb + market_data/discount_curves.ipynb |
| ForwardCurve (id, tenor, rate, ...) | ✅ | market_data_and_curves.ipynb + market_data/forward_curves.ipynb |
| HazardCurve (id, sp, hazard_rate, recovery) | ✅ | market_data_and_curves.ipynb + market_data/hazard_curves.ipynb |
| PriceCurve (id, price) | ✅ | market_data/price_curves.ipynb |
| InflationCurve (cpi, cpi_with_lag, inflation_rate, inflation_rate_simple, base_cpi, lag) | ⚠️ | inflation_rate shown; inflation_rate_simple not shown |
| VolSurface (ctor with quote_type/secondary_axis/interp, value_checked/clamped, grid_shape, ...) | ✅ (partial) | market_data/vol_surfaces.ipynb; quote_type not explicitly exercised in cells |
| VolatilityIndexCurve (forward_level) | ✅ | market_data/volatility_index_curves.ipynb |
| FxMatrix (set_quote, rate + policy + triangulated, inverse) | ✅ | market_data/fx_matrix.ipynb (direct, synthetic via USD, error on missing cross) |
| FxConversionPolicy, FxRateResult | ✅ | fx_matrix.ipynb |
| MarketContext (insert chain for curves/surfaces, insert_fx, get_* for discount/forward/hazard/surface/vol_index/price_curve/fx_delta/vol_cube, fx prop, to/from_json) | ✅ (partial) | market_data_and_curves.ipynb + vol_surfaces; additional insert_price/insert_credit_index + some getters not shown |
| dtsm (diebold_li_fit_factors, diebold_li_forecast, yield_pca_fit, yield_pca_scenario) | ✅ | market_data/dynamic_term_structure.ipynb |
| arbitrage (check_butterfly_grid, check_calendar_spread_grid, check_local_vol_density_grid, check_surface_grid) | ❌ | Not present (SABR notebook uses its own smile.arbitrage_diagnostics) |
| BaseCorrelationCurve, CreditIndexData, FxDeltaVolSurface, VolCube (types) | ⚠️ | Listed in parity; accessible via MarketContext getters or calibration snapshot; no direct foundation demo of construction/usage |

### core.math
| Symbol / Area | Status | Notes / Notebook |
|---------------|--------|------------------|
| count_consecutive | ❌ | Exposed in __all__ and .pyi; no usage |
| linalg (cholesky_decomposition, cholesky_solve, validate_correlation_matrix, constants SINGULAR_*/DIAGONAL_*) + flat variant via valuations | ✅ | math_toolkit.ipynb (nested); flat noted |
| stats (mean, variance, population_variance, correlation, covariance, quantile) | ✅ | math_toolkit.ipynb |
| special_functions (norm_cdf/pdf, standard_normal_inv_cdf, erf, ln_gamma, student_t_*) | ✅ | math_toolkit.ipynb |
| summation (kahan_sum, neumaier_sum) | ✅ | math_toolkit.ipynb + stability example |

### core.rating_scales (entire module)
| Symbol | Status | Notes / Notebook |
|--------|--------|------------------|
| UnknownScalePolicy (ERROR/FALLBACK_TO_DEFAULT/WARN_AND_FALLBACK + from_name) | ❌ | Only extension key "core.rating_scales.v1" string appears in registry notebook |
| RatingLevel (ctor, name/score/min_score, json) | ❌ | |
| ScorecardScale (ctor, scale_name, ratings, description, len) | ❌ | |
| RatingScaleRegistry (default_scorecard_score, default_scale_id, unknown_scale_policy, is_known_rating_scale, rating_scale, json) | ❌ | |
| embedded_registry() | ❌ | |
| registry_from_config(config) | ❌ | |
| RATING_SCALES_EXTENSION_KEY | ⚠️ | Mentioned as key only |

## Cross-cutting / Calibration
- market_bootstrap_tour.ipynb: ✅ canonical calibrate -> result.market, report_to_dataframe, market_json snapshot, dry_run/dependency_graph in error paths, TypedDict envelope construction.

## Notes on "FinstackConfig default"
- Notebooks consistently use `FinstackConfig()` for default.
- A stale doctest in rating_scales.pyi references `FinstackConfig.default()` (does not exist on the binding; defaulting happens in the Rust `new` + Python `FinstackConfig()`).
- Recommendation: keep `FinstackConfig()` as the documented default in foundations; the doctest can be cleaned separately.

## Recommendations (aligns with implementation plan)
1. Add rating_scales demo (embedded + registry_from_config) — tie to registry notebook.
2. Add `count_consecutive` cell.
3. Add vol grid arbitrage checks in vol_surfaces.ipynb.
4. Extend MarketContext examples for insert_price/insert_credit_index + typed getters.
5. One-liners / small cells: inflation_rate_simple, signed_year_fraction, FiscalConfig + build_fiscal_periods, VolSurface quote_type.
6. Verify FxMatrix inverse/error paths remain clear (they are).
7. Run `run_all_notebooks.py --directory 01_foundations` + type/lint after changes.

This audit was performed by:
- Extracting public surfaces from parity_contract.toml + all relevant *.pyi under finstack_quant/core*
- Grepping notebook sources (ipynb JSON) and reading representative notebooks for usage
- Cross-referencing plan gap list (confirmed + minor additional surface notes)

No changes to notebooks were made during this read-only audit pass.
