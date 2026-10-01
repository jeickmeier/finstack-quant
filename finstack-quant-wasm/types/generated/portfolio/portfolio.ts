// Generated from the finstack-quant-portfolio JSON schemas by scripts/generate-contract-types.mjs. Do not edit.
import type * as valuations from '../valuations/index.js';

/**
 * Strategy allocation scheme.
 */
export type AllocationScheme = "equal" | "fixed" | "inverse_volatility" | "risk_budget";
/**
 * A concrete market-data target changed while applying a scenario.
 *
 * Targets are recorded from hierarchy-expanded operations together with the
 * effects that were actually accepted by the engine. Consequently, every
 * identifier is a resolved market identifier rather than an unresolved
 * hierarchy path or a best-effort reconstruction of the original spec.
 */
export type ScenarioMarketTarget =
  | {
      /**
       * Concrete identifier changed in that collection.
       */
      curve_id: Id;
      /**
       * Curve family used to select the market-data collection.
       */
      curve_kind: CurveKind;
      kind: "curve";
    }
  | {
      /**
       * Concrete volatility-index curve identifier.
       */
      curve_id: Id;
      kind: "volatility_index";
    }
  | {
      kind: "base_correlation";
      /**
       * Concrete base-correlation surface identifier.
       */
      surface_id: Id;
    }
  | {
      kind: "vol_surface";
      /**
       * Concrete volatility-surface identifier.
       */
      vol_surface_id: Id;
    }
  | {
      kind: "equity_price";
      /**
       * Concrete `MarketContext::get_price` scalar identifier.
       */
      spot_id: Id;
    }
  | {
      /**
       * Base currency strengthened or weakened by the operation.
       */
      base: Currency;
      kind: "fx";
      /**
       * Quote currency used for the shocked rate.
       */
      quote: Currency;
    };
/**
 * A phantom-typed identifier that prevents mixing different kinds of IDs.
 *
 * This type wraps a string identifier with a phantom type parameter to ensure
 * type safety at compile time. Different `Id<T>` types with different `T`
 * cannot be compared or mixed accidentally, preventing entire classes of bugs.
 *
 * # Type Parameters
 *
 * * `T` - Phantom type tag that distinguishes this ID from IDs with different tags
 *
 * # Invariants
 *
 * - Storage uses `Arc<str>` for efficient cloning
 * - The phantom marker has zero size and runtime cost
 * - Two `Id<T>` values are equal if their string values are equal
 * - IDs with different type tags (`Id<A>` vs `Id<B>`) cannot be compared
 *
 * # Thread Safety
 *
 * `Id<T>` is `Send + Sync` as it wraps an `Arc<str>`. Multiple threads can
 * safely share and clone IDs with minimal synchronization overhead.
 */
export type Id = string;
/**
 * Identifies which family of curve an operation targets.
 *
 * These variants map to the market data collections exposed by
 * `finstack_quant_core::market_data::context::MarketContext`.
 * They also determine which quoting and interpolation conventions apply when
 * downstream helpers extract rates or apply node shocks.
 */
export type CurveKind = "discount" | "forward" | "par_cds" | "inflation" | "commodity";
/**
 * ISO 4217 currency enumeration
 */
export type Currency =
  | "AED"
  | "AFN"
  | "ALL"
  | "AMD"
  | "ANG"
  | "AOA"
  | "ARS"
  | "AUD"
  | "AWG"
  | "AZN"
  | "BAM"
  | "BBD"
  | "BDT"
  | "BGN"
  | "BHD"
  | "BIF"
  | "BMD"
  | "BND"
  | "BOB"
  | "BRL"
  | "BSD"
  | "BTN"
  | "BWP"
  | "BYN"
  | "BZD"
  | "CAD"
  | "CDF"
  | "CHF"
  | "CLF"
  | "CLP"
  | "CNY"
  | "COP"
  | "CRC"
  | "CUC"
  | "CUP"
  | "CVE"
  | "CZK"
  | "DJF"
  | "DKK"
  | "DOP"
  | "DZD"
  | "EGP"
  | "ERN"
  | "ETB"
  | "EUR"
  | "FJD"
  | "FKP"
  | "GBP"
  | "GEL"
  | "GHS"
  | "GIP"
  | "GMD"
  | "GNF"
  | "GTQ"
  | "GYD"
  | "HKD"
  | "HNL"
  | "HRK"
  | "HTG"
  | "HUF"
  | "IDR"
  | "ILS"
  | "INR"
  | "IQD"
  | "IRR"
  | "ISK"
  | "JMD"
  | "JOD"
  | "JPY"
  | "KES"
  | "KGS"
  | "KHR"
  | "KMF"
  | "KPW"
  | "KRW"
  | "KWD"
  | "KYD"
  | "KZT"
  | "LAK"
  | "LBP"
  | "LKR"
  | "LRD"
  | "LSL"
  | "LYD"
  | "MAD"
  | "MDL"
  | "MGA"
  | "MKD"
  | "MMK"
  | "MNT"
  | "MOP"
  | "MRU"
  | "MUR"
  | "MVR"
  | "MWK"
  | "MXN"
  | "MYR"
  | "MZN"
  | "NAD"
  | "NGN"
  | "NIO"
  | "NOK"
  | "NPR"
  | "NZD"
  | "OMR"
  | "PAB"
  | "PEN"
  | "PGK"
  | "PHP"
  | "PKR"
  | "PLN"
  | "PYG"
  | "QAR"
  | "RON"
  | "RSD"
  | "RUB"
  | "RWF"
  | "SAR"
  | "SBD"
  | "SCR"
  | "SDG"
  | "SEK"
  | "SGD"
  | "SHP"
  | "SLE"
  | "SLL"
  | "SOS"
  | "SRD"
  | "SSP"
  | "STN"
  | "SYP"
  | "SZL"
  | "THB"
  | "TJS"
  | "TMT"
  | "TND"
  | "TOP"
  | "TRY"
  | "TTD"
  | "TWD"
  | "TZS"
  | "UAH"
  | "UGX"
  | "USD"
  | "UYU"
  | "UZS"
  | "VED"
  | "VES"
  | "VND"
  | "VUV"
  | "WST"
  | "XAF"
  | "XCD"
  | "XOF"
  | "XPF"
  | "YER"
  | "ZAR"
  | "ZMW"
  | "ZWL";
/**
 * Rounding modes supported by the library.
 *
 * The variants mirror the most common conventions found in pricing engines.
 */
export type RoundingMode = "bankers" | "away_from_zero" | "toward_zero" | "floor" | "ceil";
/**
 * Finite JSON number that is strictly greater than zero.
 *
 * This type is used by serde field adapters so runtime deserialization and
 * generated schemas enforce the same positive-number contract.
 */
export type PositiveF64Wire = number;
/**
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
/**
 * Exact decimal encoded only as a JSON string.
 */
export type DecimalWire = string;
/**
 * A single warning emitted by an adapter, the engine, or a downstream helper.
 *
 * New variants will be added over time; pattern matches on `Warning` should
 * always include a wildcard arm.
 */
export type Warning =
  | {
      /**
       * Hazard curve affected by the approximate spread shock.
       */
      curve_id: string;
      kind: "hazard_spread_first_order";
      /**
       * Decimal recovery fraction used in the loss-given-default conversion.
       */
      recovery_rate: number;
      [k: string]: unknown;
    }
  | {
      /**
       * The discount curve selected by the heuristic.
       */
      chosen_discount: string;
      /**
       * Curve identifier the heuristic was applied to.
       */
      for_curve: string;
      kind: "discount_curve_heuristic";
      /**
       * Reason text describing the heuristic path.
       */
      reason: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Curve identifier.
       */
      curve_id: string;
      /**
       * Detail describing whether the trigger was a parallel or node shock
       * and which knot(s) were extreme.
       */
      detail: string;
      kind: "commodity_shock_outside_range";
      [k: string]: unknown;
    }
  | {
      /**
       * Equity price identifier.
       */
      id: string;
      kind: "equity_not_found";
      [k: string]: unknown;
    }
  | {
      /**
       * Curve identifier from which the rate would have been extracted.
       */
      curve_id: string;
      kind: "rate_binding_no_forecast_values";
      /**
       * Statement node identifier.
       */
      node_id: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Curve identifier.
       */
      curve_id: string;
      kind: "rate_binding_failed";
      /**
       * Statement node identifier.
       */
      node_id: string;
      /**
       * Underlying error message.
       */
      reason: string;
      [k: string]: unknown;
    }
  | {
      kind: "statement_node_no_values";
      /**
       * Statement node identifier.
       */
      node_id: string;
      /**
       * Operation that was attempted (e.g. `"forecast_percent"`).
       */
      op: string;
      [k: string]: unknown;
    }
  | {
      kind: "statement_op_failed";
      /**
       * Statement node identifier.
       */
      node_id: string;
      /**
       * Operation name.
       */
      op: string;
      /**
       * Underlying error message.
       */
      reason: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Free-text detail from the evaluator (already includes context).
       */
      detail: string;
      kind: "model_evaluation";
      [k: string]: unknown;
    }
  | {
      kind: "model_reevaluation_failed";
      /**
       * Underlying error message.
       */
      reason: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Free-text detail naming the inconsistent pair and computed values.
       */
      detail: string;
      kind: "fx_triangulation_inconsistent";
      [k: string]: unknown;
    }
  | {
      /**
       * Canonical instrument type that could not apply the shock directly.
       */
      inst_type: InstrumentType;
      kind: "instrument_shock_fallback";
      /**
       * Instrument label (id / name / unidentified).
       */
      label: string;
      /**
       * Whether the shock is a price (`"price"`) or spread (`"spread"`).
       */
      shock_kind: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Free-text description of the filter (e.g. `IndexMap` debug form).
       */
      filter_desc: string;
      kind: "instrument_shock_no_match";
      [k: string]: unknown;
    }
  | {
      kind: "correlation_shock_no_match";
      [k: string]: unknown;
    }
  | {
      /**
       * Detail string describing what was clamped.
       */
      detail: string;
      /**
       * Instrument identifier.
       */
      instrument_id: string;
      kind: "correlation_clamped";
      [k: string]: unknown;
    }
  | {
      /**
       * Curve identifier.
       */
      curve_id: string;
      /**
       * Free-text describing the tenor and the gap.
       */
      detail: string;
      kind: "tenor_extrapolated";
      [k: string]: unknown;
    }
  | {
      /**
       * Curve identifier.
       */
      curve_id: string;
      /**
       * Free-text describing the tenor and adjacent pillars.
       */
      detail: string;
      kind: "interpolated_node_bump_first_order";
      [k: string]: unknown;
    }
  | {
      kind: "base_corr_bucket_no_match";
      /**
       * Surface identifier.
       */
      surface_id: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Free-text from `ArbitrageViolation` Display.
       */
      detail: string;
      kind: "vol_surface_arbitrage";
      /**
       * Volatility-surface identifier.
       */
      vol_surface_id: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Whether this was a parallel (`false`) or bucket (`true`) shock.
       */
      bucket: boolean;
      kind: "vol_surface_large_negative_shock";
      /**
       * Percent shock value.
       */
      pct: number;
      /**
       * Volatility-surface identifier.
       */
      vol_surface_id: string;
      [k: string]: unknown;
    }
  | {
      kind: "hierarchy_no_match";
      /**
       * Operation kind name (e.g., `"HierarchyCurveParallelBp"`).
       */
      op_kind: string;
      /**
       * Hierarchy path the operation targeted, joined with `/`.
       */
      target_path: string;
      [k: string]: unknown;
    }
  | {
      /**
       * The resolved identifier that was skipped.
       */
      curve_id: string;
      kind: "hierarchy_resolved_id_skipped";
      /**
       * Operation kind name (e.g., `"HierarchyCurveParallelBp"`).
       */
      op_kind: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Instrument identifier.
       */
      instrument_id: string;
      kind: "time_roll_instrument_failed";
      /**
       * Underlying valuation error message.
       */
      reason: string;
      [k: string]: unknown;
    };
/**
 * Strongly-typed instrument classification for pricer dispatch.
 *
 * Each variant represents a distinct instrument type with its own pricing
 * logic and risk characteristics. Used by the pricing registry to route
 * instruments to appropriate pricer implementations.
 */
export type InstrumentType =
  | "bond"
  | "credit_default_swap"
  | "cds_index"
  | "cds_tranche"
  | "cds_option"
  | "interest_rate_swap"
  | "cap_floor"
  | "swaption"
  | "bermudan_swaption"
  | "basis_swap"
  | "basket"
  | "convertible_bond"
  | "deposit"
  | "equity_option"
  | "fx_option"
  | "fx_spot"
  | "fx_swap"
  | "xccy_swap"
  | "inflation_linked_bond"
  | "inflation_swap"
  | "yoy_inflation_swap"
  | "inflation_cap_floor"
  | "interest_rate_future"
  | "variance_swap"
  | "fx_variance_swap"
  | "equity"
  | "repo"
  | "forward_rate_agreement"
  | "structured_credit"
  | "private_markets_fund"
  | "revolving_credit"
  | "asian_option"
  | "barrier_option"
  | "lookback_option"
  | "quanto_option"
  | "autocallable"
  | "cms_option"
  | "cms_swap"
  | "cliquet_option"
  | "range_accrual"
  | "fx_barrier_option"
  | "term_loan"
  | "discounted_cash_flow"
  | "real_estate_asset"
  | "levered_real_estate_equity"
  | "trs_equity"
  | "trs_fixed_income_index"
  | "bond_future"
  | "commodity_forward"
  | "commodity_swap"
  | "commodity_option"
  | "commodity_asian_option"
  | "commodity_swaption"
  | "commodity_spread_option"
  | "volatility_index_future"
  | "fx_forward"
  | "ndf"
  | "agency_mbs_passthrough"
  | "agency_tba"
  | "dollar_roll"
  | "agency_cmo"
  | "fx_digital_option"
  | "fx_touch_option"
  | "tarn"
  | "cms_spread_option"
  | "callable_range_accrual"
  | "snowball"
  | "composite"
  | "commodity_future"
  | "fx_future"
  | "equity_future"
  | "equity_total_return_future"
  | "interest_rate_future_option"
  | "equity_future_option"
  | "fx_future_option"
  | "commodity_future_option"
  | "volatility_index_future_option"
  | "asset_backed_facility";
/**
 * Comparison operator for attribute-based filtering.
 *
 * For [`AttributeValue::Text`] attributes, only [`ComparisonOp::Eq`] and
 * [`ComparisonOp::Ne`] are meaningful; ordering comparisons on text return
 * `false`.  For [`AttributeValue::Number`] attributes, all six operators
 * apply.
 */
export type ComparisonOp = "eq" | "ne" | "lt" | "le" | "gt" | "ge";
/**
 * Value stored in a position or candidate attribute.
 *
 * Positions carry key/value attributes for grouping, filtering, and
 * optimization constraints.  Text values represent categorical data
 * (rating, sector), while numeric values represent continuous data
 * (credit score, ESG score) usable in metric expressions.
 */
export type AttributeValue = string | number;
/**
 * Factor types for P&L attribution.
 *
 * Groups `MarketContext` inputs by their economic role:
 * - **RatesCurves**: discount_curves + forward_curves
 * - **CreditCurves**: hazard_curves
 * - **InflationCurves**: inflation_curves + published inflation_indices
 * - **Correlations**: base_correlation_curves
 * - **Fx**: FxMatrix
 * - **Volatility**: surfaces and declared scalar volatility quotes
 * - **MarketScalars**: other prices, non-fixing series and dividends
 */
export type AttributionFactor =
  | "carry"
  | "rates_curves"
  | "credit_curves"
  | "inflation_curves"
  | "correlations"
  | "fx"
  | "volatility"
  | "market_scalars"
  | "model_parameters";
/**
 * Controls where attribution repricing work spends parallelism.
 *
 * `Serial` is the default: typical standalone factor sets are small enough
 * that inner Rayon costs more than it saves. `Parallel` opts into Rayon for
 * independent factor repricings when the caller is not already parallelizing
 * an outer portfolio or batch loop.
 */
export type ExecutionPolicy = "parallel" | "serial";
/**
 * Standard FX conversion strategies used to hint FX providers.
 *
 * The policy tells a provider *how* the rate will be applied so it can decide
 * between spot, forward, or averaged sources.
 */
export type FxConversionPolicy = "cashflow_date" | "period_end" | "period_average";
/**
 * Attribution methodology for decomposing P&L.
 *
 * Four methodologies are supported:
 * - **Parallel**: Independent factor isolation (may not sum due to cross-effects)
 * - **Waterfall**: Sequential application (guarantees sum = total, order matters)
 * - **MetricsBased**: Linear approximation using existing metrics (fast but approximate)
 * - **Taylor**: Sensitivity-based Taylor expansion (first/second order via bump-and-reprice)
 */
export type AttributionMethod =
  | "parallel"
  | {
      waterfall: AttributionFactor[];
    }
  | "metrics_based"
  | {
      taylor: TaylorAttributionConfig;
    };
/**
 * Book identifier.
 */
export type BookId = string;
/**
 * Position identifier.
 */
export type PositionId = string;
/**
 * Enumeration of cash-flow kinds for classification and ordering.
 *
 * Used to distinguish between different types of cashflows for
 * proper sequencing, risk calculation, and accounting treatment.
 *
 * # Sign Convention
 *
 * The enum itself is **view agnostic**: individual instruments are
 * responsible for mapping these kinds into a holder or issuer view.
 * By convention in this crate:
 *
 * | Kind | Holder View (Long) | Issuer View (Short) |
 * |------|-------------------|---------------------|
 * | Interest (Fixed/Float) | Positive (receive) | Negative (pay) |
 * | Notional (initial) | Negative (pay) | Positive (receive) |
 * | Notional (final) | Positive (receive) | Negative (pay) |
 * | Amortization | Positive (receive) | Negative (pay) |
 * | PIK | Increases notional | Increases liability |
 * | Fee | Negative (pay) | Positive (receive) |
 *
 * When constructing cashflow schedules, instruments should apply the appropriate
 * sign based on the economic perspective being represented.
 *
 * # Cashflow Categories
 *
 * Variants are grouped by category:
 * - **Interest**: `Fixed`, `FloatReset`, `Stub`
 * - **Inflation**: `InflationCoupon`
 * - **Fees**: `Fee`, `CommitmentFee`, `UsageFee`, `FacilityFee`
 * - **Principal**: `Notional`, `PIK`, `Amortization`, `PrePayment`
 * - **Revolving**: `RevolvingDraw`, `RevolvingRepayment`
 * - **Credit Events**: `DefaultedNotional`, `Recovery`
 * - **Margin/Collateral**: `InitialMarginPost`, `VariationMarginPay`, etc.
 */
export type CFKind =
  | "fixed"
  | "float_reset"
  | "inflation_coupon"
  | "fee"
  | "commitment_fee"
  | "usage_fee"
  | "facility_fee"
  | "lc_fee"
  | "fronting_fee"
  | "notional"
  | "pik"
  | "amortization"
  | "pre_payment"
  | "revolving_draw"
  | "revolving_repayment"
  | "defaulted_notional"
  | "recovery"
  | "accrued_on_default"
  | "stub"
  | "initial_margin_post"
  | "initial_margin_return"
  | "variation_margin_receive"
  | "variation_margin_pay"
  | "margin_interest"
  | "collateral_substitution_in"
  | "collateral_substitution_out";
/**
 * Entity identifier (company, fund, etc.)
 */
export type EntityId = string;
/**
 * Unit of position measurement.
 *
 * The unit describes how the `quantity` on a [`Position`] should be interpreted.
 * Callers should treat it as part of the valuation contract, not display-only
 * metadata.
 *
 * # Scaling contract
 *
 * Position value is ``scale_factor(unit) * instrument.value()``. The instrument
 * is already built with its deal notional (or face, or one share). Each variant
 * defines the scale factor explicitly:
 *
 * | Variant       | Scale factor              | Quantity interpretation              |
 * |---------------|---------------------------|--------------------------------------|
 * | `Units`       | `quantity`                | Number of instrument units/shares    |
 * | `Notional(_)` | `quantity`                | Lot multiplier (`1` = one deal)      |
 * | `FaceValue`   | `quantity`                | Held face-value multiplier           |
 * | `Percentage`  | `quantity / 100`          | Percentage points of the instrument  |
 *
 * ## `Notional` semantics
 *
 * `Notional(ccy)` means the instrument stores the deal notional and
 * `quantity` is a lot multiplier: `1` is one deal, `2` is two deals.
 * Position PV is ``quantity × instrument.value()``. Do not build the
 * instrument with unit notional of 1 and put the dollar notional in
 * `quantity`.
 *
 * Optional `Notional(Some(ccy))` only validates currency against the
 * instrument's native PV currency. It does not change the scale factor.
 * A warning is emitted when it disagrees with the instrument's valuation
 * currency.
 */
export type PositionUnit =
  | "units"
  | {
      notional: Currency | null;
    }
  | "face_value"
  | "percentage";
/**
 * Why a position did not contribute classified cashflows to a portfolio ladder.
 */
export type CashflowExtractionIssueKind = "build_failed";
/**
 * How [`PortfolioCashflows::collapse_to_base_by_date_kind`] converts
 * foreign-currency flows into the reporting currency.
 *
 * This is not [`finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate`]:
 * that policy names a spot-equivalent provider lookup on the payment date.
 * Collapse uses spot at `as_of` for due-or-past flows and CIP forwards for
 * later dates.
 */
export type CashflowFxPolicy = "cip_forward";
/**
 * Meaning of the emitted schedule relative to pricing and waterfall policy.
 */
export type CashflowRepresentation = "contractual" | "projected" | "placeholder" | "no_residual";
/**
 * Declarative constraint specification.
 */
export type Constraint =
  | {
      metric_bound: {
        /**
         * Human‑readable label for debugging/diagnostics.
         */
        label?: string | null;
        /**
         * Metric expression on the left‑hand side.
         */
        metric: MetricExpr;
        /**
         * Operator (<=, >=, ==).
         */
        op: Inequality;
        /**
         * Right‑hand side constant.
         */
        rhs: number;
      };
    }
  | {
      weight_bounds: {
        /**
         * Filter to select positions for this constraint.
         */
        filter: PositionFilter;
        /**
         * Human‑readable label for debugging/diagnostics.
         */
        label?: string | null;
        /**
         * Inclusive maximum weight.
         */
        max: number;
        /**
         * Inclusive minimum weight.
         */
        min: number;
      };
    }
  | {
      max_turnover: {
        /**
         * Human‑readable label for debugging/diagnostics.
         */
        label?: string | null;
        /**
         * Maximum allowed turnover (sum of absolute weight changes).
         */
        max_turnover: number;
      };
    }
  | {
      budget: {
        /**
         * Right‑hand side constant (typically 1.0 for normalization).
         */
        rhs: number;
      };
    };
/**
 * Portfolio‑level scalar metric expressed in terms of position metrics + weights.
 *
 * These expressions are intentionally restricted to linear or linearized forms
 * so they can be represented by the LP-based optimizer.
 */
export type MetricExpr =
  | {
      weighted_sum: {
        /**
         * Optional filter restricting which positions contribute.
         */
        filter?: PositionFilter | null;
        /**
         * Per‑position metric to aggregate.
         */
        metric: PerPositionMetric;
      };
    }
  | {
      value_weighted_average: {
        /**
         * Optional filter restricting which positions contribute.
         */
        filter?: PositionFilter | null;
        /**
         * Per‑position metric to average.
         */
        metric: PerPositionMetric;
      };
    };
/**
 * Filters for selecting which positions are included in a rule.
 */
export type PositionFilter =
  | "all"
  | {
      by_entity_id: EntityId;
    }
  | {
      by_attribute: AttributeTest;
    }
  | {
      by_position_ids: PositionId[];
    }
  | {
      not: PositionFilter;
    }
  | {
      and: PositionFilter[];
    }
  | {
      or: PositionFilter[];
    };
/**
 * Where a per‑position scalar metric comes from.
 */
export type PerPositionMetric =
  | {
      metric: MetricId;
    }
  | {
      custom_key: string;
    }
  | "pv_base"
  | "pv_native"
  | {
      attribute: string;
    }
  | {
      attribute_indicator: AttributeTest;
    }
  | {
      constant: number;
    };
/**
 * Strongly-typed metric identifier.
 *
 * Provides compile-time validation, autocomplete support, and safe refactoring
 * when metric names change. Covers bond, IRS, deposit, and risk metrics.
 *
 * See unit tests and `examples/` for usage.
 */
export type MetricId = string;
/**
 * Inequality/equality operator.
 */
export type Inequality = "le" | "ge" | "eq";
/**
 * Risk measure used when aggregating factor exposures.
 */
export type RiskMeasure =
  | "variance"
  | "volatility"
  | {
      var: {
        /**
         * Confidence level in the open interval `(0.5, 1)`.
         */
        confidence: number;
        [k: string]: unknown;
      };
    }
  | {
      expected_shortfall: {
        /**
         * Confidence level in the open interval `(0.5, 1)`.
         */
        confidence: number;
        [k: string]: unknown;
      };
    };
/**
 * Classification of a curve dependency's role.
 */
export type CurveType = "discount" | "forward" | "hazard" | "inflation" | "base_correlation";
/**
 * Stage of persisted artifact loading that produced a diagnostic.
 */
export type LoadPhase = "parse" | "version" | "structure" | "semantic" | "canonicalize" | "hash" | "build";
/**
 * Severity assigned to a persisted artifact diagnostic.
 */
export type Severity = "error" | "warning";
/**
 * A single market dependency extracted from an instrument.
 */
export type MarketDependency =
  | {
      curve: {
        /**
         * Role played by the curve.
         */
        curve_type: CurveType;
        /**
         * Curve identifier.
         */
        id: Id;
      };
    }
  | {
      credit_curve: {
        /**
         * Curve identifier.
         */
        id: Id;
      };
    }
  | {
      credit_index: {
        /**
         * Credit-index aggregate identifier.
         */
        id: Id;
      };
    }
  | {
      spot: {
        /**
         * Spot identifier or ticker.
         */
        id: string;
      };
    }
  | {
      vol_surface: {
        /**
         * Surface identifier.
         */
        id: string;
      };
    }
  | {
      fx_pair: {
        /**
         * Base currency.
         */
        base: Currency;
        /**
         * Quote currency.
         */
        quote: Currency;
      };
    }
  | {
      series: {
        /**
         * Series identifier.
         */
        id: string;
      };
    };
/**
 * Unique identifier for a risk factor.
 */
export type FactorId = string;
/**
 * Initial margin calculation methodology.
 *
 * Different methodologies are used depending on regulatory requirements,
 * product type, and whether trades are cleared or bilateral.
 *
 * # BCBS-IOSCO Standards
 *
 * For bilateral (uncleared) OTC derivatives, either SIMM or the regulatory
 * schedule approach may be used. SIMM is the industry standard for large
 * dealers due to its risk-sensitivity.
 */
export type ImMethodology = "haircut" | "simm" | "schedule" | "internal_model" | "clearing_house";
/**
 * Normalized market factor key for portfolio-level dependency tracking.
 */
export type MarketFactorKey =
  | {
      curve: {
        /**
         * Curve identifier matching [`CurveId`] in market data.
         */
        id: Id;
        /**
         * Curve kind (discount, forward, credit, or inflation).
         */
        kind: RatesCurveKind;
      };
    }
  | {
      spot: string;
    }
  | {
      vol_surface: string;
    }
  | {
      fx: {
        /**
         * Base currency.
         */
        base: Currency;
        /**
         * Quote currency.
         */
        quote: Currency;
      };
    }
  | {
      series: string;
    };
/**
 * Identifies a rate curve's market role for risk calculations.
 */
export type RatesCurveKind = "discount" | "forward" | "credit" | "inflation";
/**
 * Lightweight position referencing a unique instrument artifact.
 */
export type MaterializedPosition = PercentageMaterializedPositionWire | NonPercentageMaterializedPositionWire;
/**
 * Finite percentage-position quantity in the closed interval `[-100, 100]`.
 */
export type PercentageQuantityWire = number;
export type PercentagePositionUnitWire = "percentage";
export type NonPercentagePositionUnitWire =
  | NonPercentagePositionUnitName
  | {
      notional: Currency | null;
    };
export type NonPercentagePositionUnitName = "units" | "face_value";
/**
 * How to handle missing metrics for a position.
 */
export type MissingMetricPolicy = "zero" | "exclude" | "strict";
/**
 * Identifies a margin netting set.
 *
 * Instruments in the same netting set can offset each other for margin
 * calculation purposes. The netting set is typically defined by the
 * CSA agreement (bilateral) or by CCP membership (cleared) — these two
 * shapes are mutually exclusive, so the type encodes them as enum
 * variants rather than as a struct with two `Option<String>` fields
 * that could in principle both be set or both be unset.
 */
export type NettingSetId =
  | {
      /**
       * Counterparty identifier
       */
      counterparty_id: string;
      /**
       * CSA identifier
       */
      csa_id: string;
      kind: "bilateral";
    }
  | {
      /**
       * CCP identifier (also used as the counterparty id)
       */
      ccp_id: string;
      kind: "cleared";
    };
/**
 * Risk classes for SIMM categorization.
 */
export type SimmRiskClass =
  "interest_rate" | "credit_qualifying" | "credit_non_qualifying" | "equity" | "commodity" | "fx";
/**
 * Optimization direction and target.
 */
export type Objective =
  | {
      maximize: MetricExpr;
    }
  | {
      minimize: MetricExpr;
    };
/**
 * Status of an optimization run.
 */
export type OptimizationStatus =
  | "optimal"
  | "feasible_but_suboptimal"
  | {
      infeasible: {
        /**
         * Constraints that appear to conflict (if determinable).
         */
        conflicting_constraints: string[];
        [k: string]: unknown;
      };
    }
  | "unbounded"
  | {
      error: {
        /**
         * Error message describing what went wrong.
         */
        message: string;
        [k: string]: unknown;
      };
    };
/**
 * Sole supported portfolio-materialization contract marker.
 */
export type PortfolioMaterializationSchema = "finstack_quant.portfolio_materialization/1";
/**
 * Numeric schema revision for contracts whose sole supported revision is v1.
 */
export type SchemaVersion = number;
/**
 * Direction of a trade.
 */
export type TradeDirection = "buy" | "sell" | "hold";
/**
 * Whether a trade is for an existing position or a new candidate.
 */
export type TradeType = "existing" | "new_position" | "close_out";
/**
 * How optimization weights are defined.
 *
 * # Conventions
 *
 * `ValueWeight` and `NotionalWeight` are normalized portfolio shares, whereas
 * `UnitScaling` is a direct multiplier on current quantity for existing
 * positions.
 */
export type WeightingScheme = "value_weight" | "notional_weight" | "unit_scaling";
/**
 * Which metric set to offer every position in the portfolio.
 *
 * # A portfolio metric list is a menu
 *
 * Whichever variant is used, the resulting list is a **menu**, not a request
 * made of each position individually. It is chosen once for a book whose
 * positions have different instrument types, and there is no way to say
 * "`delta` for the options, `cs01` for the credit" — so a mixed book is only
 * valuable at all if each position takes the part of the list that applies to
 * it. Each position is therefore asked for exactly the entries its own
 * instrument type has a calculator for.
 *
 * Narrowing is confined to structural inapplicability, and it is reported,
 * not silent:
 *
 * - The entries skipped for a position are listed on
 *   [`PositionValue::inapplicable_metrics`].
 * - An identifier the standard metric registry does not know is still
 *   rejected when the request is parsed, by [`Self::try_from_metric_names`],
 *   so a typo fails loudly instead of narrowing away everywhere.
 * - A composite position is asked for the additive entries at least one of
 *   its legs supports (see `Instrument::applicable_metrics`).
 * - A metric an instrument type *does* support but fails to compute is
 *   governed by
 *   [`PortfolioValuationOptions::strict_risk`](PortfolioValuationOptions::strict_risk),
 *   which is unchanged: the valuation aborts, or the position degrades to
 *   PV-only and is listed on [`PortfolioValuation::degraded_positions`].
 *
 * Single-instrument pricing keeps the opposite contract: it rejects any
 * requested metric its instrument type has no calculator for, because there
 * the caller chose the list against that one instrument.
 */
export type RequestedMetrics =
  | {
      mode: "standard";
      [k: string]: unknown;
    }
  | {
      metrics: MetricId[];
      mode: "standard_plus";
      [k: string]: unknown;
    }
  | {
      metrics: MetricId[];
      mode: "only";
      [k: string]: unknown;
    };
/**
 * Position edits supported by `WhatIfEngine::position_what_if`.
 */
export type PositionChange =
  | {
      kind: "remove";
      /**
       * Position identifier to remove from the scenario.
       */
      position_id: PositionId;
    }
  | {
      kind: "resize";
      /**
       * Replacement quantity for the position.
       */
      new_quantity: number;
      /**
       * Position identifier to resize.
       */
      position_id: PositionId;
    };
/**
 * Source of a per-position residual variance contribution.
 *
 * Distinguishes residuals derived from a credit factor model (where the
 * idiosyncratic adder is calibrated per issuer) from generic / unattributed
 * residual sources used by other decomposers.
 */
export type ResidualContributionSource =
  | {
      /**
       * Issuer whose idiosyncratic vol drives this residual contribution.
       */
      issuer_id: string;
      kind: "from_credit_model";
      [k: string]: unknown;
    }
  | {
      kind: "other";
      [k: string]: unknown;
    };
/**
 * What to compute at each replay step.
 */
export type ReplayMode = "pv_only" | "pv_and_pnl" | "full_attribution";
/**
 * What to do when a single snapshot fails to revalue.
 */
export type ReplayErrorPolicy = "strict" | "best_effort";
/**
 * SIMM credit sector for bucket assignment.
 *
 * Maps reference entities to ISDA SIMM credit qualifying buckets.
 * See ISDA SIMM v2.6 Table 2.
 */
export type SimmCreditSector =
  | "sovereign"
  | "financial"
  | "basic_materials"
  | "consumer_goods"
  | "technology_media"
  | "health_care"
  | "high_yield_sovereign"
  | "high_yield_financial"
  | "high_yield_basic_materials"
  | "high_yield_consumer_goods"
  | "high_yield_technology_media"
  | "high_yield_health_care"
  | "residual";

/**
 * Aggregated metric across the portfolio.
 *
 * Contains portfolio-wide totals as well as breakdowns by entity.
 *
 * # Additive versus non-additive metrics
 *
 * Summing currency-denominated rate and credit sensitivities across
 * positions is standard desk practice: `dv01`, `cs01`, `pv01` and their
 * bucketed series all measure a change per basis point of the *same* kind of
 * risk factor, and their sum is a portfolio-level number a risk manager can
 * hedge with (Tuckman & Serrat, *Fixed Income Securities*). `fx_delta` and
 * `index_delta` are likewise currency-denominated and linear in size.
 *
 * Unqualified scalar Greeks (`delta`, `gamma`, `vega`, `vanna`, `volga`)
 * are not additive because their risk factors may differ across positions.
 * Portfolio totals require qualified keys such as `delta::<underlying>` or
 * `bucketed_vega::<surface>::<expiry>::<strike>`.
 * Instrument-specific measures such as yield and duration remain in
 * [`PortfolioMetrics::by_position`] and are listed on
 * [`PortfolioMetrics::unaggregated_metrics`].
 */
export interface AggregatedMetric {
  /**
   * Aggregated values by entity
   */
  by_entity: {
    [k: string]: number;
  };
  /**
   * Metric identifier
   */
  metric_id: string;
  /**
   * Total value across all positions (for summable metrics)
   */
  total: number;
  [k: string]: unknown;
}
/**
 * Portfolio-level allocation diagnostics.
 */
export interface AllocationDiagnostics {
  /**
   * Gross leverage, equal to sum of absolute weights.
   */
  leverage: number;
  /**
   * Sum of allocation weights.
   */
  weights_sum: number;
}
/**
 * Report describing what happened during [`super::ScenarioEngine::apply`].
 */
export interface ApplicationReport {
  /**
   * Authoritative metadata describing the state changed by applied effects.
   */
  changes: ScenarioChangeManifest;
  /**
   * Number of direct (non-hierarchy) operations produced after hierarchy
   * expansion and resolution-mode deduplication. No-match expansion and
   * deduplication can make this smaller than `user_operations`. Because
   * `operations_applied` counts effects rather than operations, the two
   * counters are not directly comparable.
   */
  expanded_operations: number;
  /**
   * Audit stamp describing the numeric mode, rounding context, and FX
   * policy under which the scenario was applied.
   */
  meta?: ResultsMeta | null;
  /**
   * Number of effects successfully applied to the execution context.
   *
   * One user-level operation can produce zero, one, or many effects after
   * hierarchy expansion and target resolution. This low-level effect count
   * is therefore not an operation-coverage ratio; inspect `changes` and
   * `warnings` to determine which targets changed or were skipped.
   */
  operations_applied: number;
  /**
   * Roll-forward report from the Phase 0 `TimeRollForward` operation,
   * when the scenario contained one. Carries the per-instrument carry
   * decomposition and the new valuation date; instruments whose valuation
   * failed during the roll are also surfaced as
   * [`Warning::TimeRollInstrumentFailed`] entries in `warnings`.
   */
  time_roll?: RollForwardReport | null;
  /**
   * Number of user-provided `OperationSpec` entries in the scenario
   * (before hierarchy expansion and deduplication).
   */
  user_operations: number;
  /**
   * Structured warnings generated during application (non-fatal).
   */
  warnings: Warning[];
}
/**
 * Authoritative change manifest produced while applying a scenario.
 *
 * Readers use the manifest to invalidate only the market factors and
 * instruments that actually changed, while `all_dirty` provides a
 * conservative escape hatch for changes that cannot be represented precisely.
 */
export interface ScenarioChangeManifest {
  /**
   * Whether callers must conservatively treat every dependency as dirty.
   *
   * This is set for effective time rolls because date-sensitive values can
   * change even when no explicit market or instrument target was mutated.
   */
  all_dirty: boolean;
  /**
   * Whether the execution context's effective valuation date changed.
   */
  as_of_changed: boolean;
  /**
   * Zero-based indices of portfolio instruments mutated in place.
   */
  changed_instrument_indices: number[];
  /**
   * Concrete market-data targets changed by applied effects.
   */
  market_targets: ScenarioMarketTarget[];
  /**
   * Whether instruments were inserted, removed, or reordered.
   *
   * Scenario operations do not change portfolio shape, so this stays `false`.
   */
  portfolio_shape_changed: boolean;
}
/**
 * Metadata bundle that accompanies valuation outputs.
 *
 * The metadata is intentionally small so it can be attached to reports and
 * downstream data stores for reproducibility and audit trails.
 */
export interface ResultsMeta {
  /**
   * Optional FX policy applied by the computing layer (human-readable key).
   */
  fx_policy_applied?: string | null;
  /**
   * Numeric engine mode used to produce the results.
   *
   * Always [`NUMERIC_MODE_F64`] today; a plain string so result
   * envelopes stay wire-compatible across host languages.
   */
  numeric_mode: string;
  /**
   * Whether the producing computation ran in parallel.
   *
   * Serial results omit the field entirely so existing payloads and golden
   * files stay byte-identical.
   */
  parallel?: boolean;
  /**
   * Rounding context snapshot applied to IO boundaries.
   */
  rounding: RoundingContext;
  /**
   * Timestamp when result was computed (ISO 8601 format).
   * Useful for audit trails and reproducibility.
   */
  timestamp?: string | null;
  /**
   * Finstack Quant library version used to produce the result.
   */
  version?: string | null;
  [k: string]: unknown;
}
/**
 * Snapshot of active rounding settings used for result stamping.
 *
 * Instances are typically produced via [`rounding_context_from`] and persisted
 * alongside valuation results.
 */
export interface RoundingContext {
  /**
   * Ingest scale map snapshot by currency code.
   */
  ingest_scale_by_currency: {
    [k: string]: number;
  };
  /**
   * Active rounding mode.
   */
  mode: RoundingMode;
  /**
   * Output scale map snapshot by currency code.
   */
  output_scale_by_currency: {
    [k: string]: number;
  };
  /**
   * Tolerance settings snapshot for floating-point comparisons.
   */
  tolerances?: ToleranceConfig;
  /**
   * Schema version for forward compatibility.
   */
  version: number;
  [k: string]: unknown;
}
/**
 * Numerical tolerance configuration for floating-point comparisons.
 *
 * Provides configurable epsilon values for zero-checks in rate calculations
 * and generic floating-point comparisons. These defaults are chosen to balance
 * numerical stability with practical precision requirements.
 */
export interface ToleranceConfig {
  /**
   * Epsilon for generic floating-point comparisons (default: 1e-10).
   *
   * Used for general numerical comparisons where higher tolerance is acceptable.
   */
  generic_epsilon?: PositiveF64Wire;
  /**
   * Epsilon for rate comparisons (default: 1e-12).
   *
   * Used when comparing interest rates, yields, and other small ratios.
   */
  rate_epsilon?: PositiveF64Wire;
}
/**
 * Report from time roll-forward operation.
 */
export interface RollForwardReport {
  /**
   * Calendar days between `old_date` and `new_date`.
   *
   * Always a calendar-day span, including under
   * [`TimeRollMode::BusinessDays`](crate::spec::TimeRollMode::BusinessDays):
   * the target date is business-day adjusted, but the span back to
   * `old_date` is still calendar days. Downstream ACT/365F annualization
   * depends on this.
   */
  days: number;
  /**
   * Instruments whose carry calculation failed but did not abort the roll.
   */
  failed_instruments: [unknown, unknown][];
  /**
   * Per-instrument carry accrual (if instruments provided), grouped by currency.
   */
  instrument_carry: [unknown, unknown][];
  /**
   * New as-of date after roll.
   */
  new_date: DateWire;
  /**
   * Original as-of date.
   */
  old_date: DateWire;
  /**
   * Total P&L from carry, grouped by currency.
   */
  total_carry: {
    [k: string]: Money;
  };
  [k: string]: unknown;
}
/**
 * Currency-tagged monetary amount with safe arithmetic.
 *
 * Values retain decimal precision independently of ISO 4217 display precision.
 *
 * When you need configurable rounding during ingestion, use
 * [`Money::new_with_config`].
 */
export interface Money {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: DecimalWire;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency: Currency;
}
/**
 * One asset's contribution to the specific (selection) effect.
 */
export interface AssetSpecificContribution {
  /**
   * Active weight `h_p,i − h_b,i`.
   */
  active_weight: number;
  /**
   * Asset identifier (mirrors [`FactorBrinsonInput::asset_ids`]).
   */
  asset: string;
  /**
   * Contribution `(h_p,i − h_b,i) * ε_b,i`.
   */
  contribution: number;
  /**
   * Specific return implied by the supplied factor returns,
   * `ε_b,i = r_i − (X f_b)_i`.
   */
  specific_return: number;
}
/**
 * Predicate that tests a single position attribute against a value.
 *
 * Reusable building block for [`crate::optimization::PositionFilter::ByAttribute`]
 * and [`crate::optimization::PerPositionMetric::AttributeIndicator`].
 */
export interface AttributeTest {
  /**
   * Attribute key to test.
   */
  key: string;
  /**
   * Comparison operator.
   */
  op: ComparisonOp;
  /**
   * Value to compare against.
   */
  value: AttributeValue;
}
/**
 * Attribution metadata.
 *
 * Records methodology, dates, repricing count, and residual statistics.
 */
export interface AttributionMeta {
  /**
   * Execution policy the attribution ran under (workspace
   * policy-visibility invariant: results stamp the parallel flag).
   * `None` for methods without a policy knob (metrics-based).
   */
  execution_policy?: ExecutionPolicy | null;
  /**
   * FX policy metadata (if FX conversions were applied).
   */
  fx_policy?: FxPolicyMeta | null;
  /**
   * Instrument identifier.
   */
  instrument_id: string;
  /**
   * Attribution method used.
   */
  method: AttributionMethod;
  /**
   * Diagnostic notes and warnings.
   */
  notes: string[];
  /**
   * Number of repricings performed.
   */
  num_repricings: number;
  /**
   * Residual as percentage of total P&L.
   */
  residual_pct: number;
  /**
   * Rounding context used for calculations.
   */
  rounding: RoundingContext;
  /**
   * Start date (T₀).
   */
  t0: DateWire;
  /**
   * End date (T₁).
   */
  t1: DateWire;
  /**
   * Absolute tolerance for residual validation.
   */
  tolerance_abs: number;
  /**
   * Percentage tolerance for residual validation.
   */
  tolerance_pct: number;
  [k: string]: unknown;
}
/**
 * Metadata describing the policy applied by the provider.
 *
 * Attach [`FxPolicyMeta`] to valuation results so auditors can understand how
 * FX conversions were sourced.
 */
export interface FxPolicyMeta {
  /**
   * Free-form provenance or audit notes supplied by the provider.
   */
  notes: string;
  /**
   * Strategy the provider actually applied or was instructed to apply.
   */
  strategy: FxConversionPolicy;
  /**
   * Optional target currency stamped onto the resulting valuation.
   */
  target_currency?: Currency | null;
}
/**
 * Configuration for Taylor-based P&L attribution.
 */
export interface TaylorAttributionConfig {
  /**
   * Credit spread bump size for CS01 computation (basis points).
   */
  credit_spread_bump_bp?: number;
  /**
   * Include second-order (gamma/convexity) terms.
   */
  include_gamma?: boolean;
  /**
   * Rate bump size for DV01 computation (basis points).
   */
  rate_bump_bp?: number;
  /**
   * Vol bump size for vega computation (absolute vol points, e.g. 0.01 = 1%).
   */
  vol_bump?: number;
}
/**
 * A book represents a folder-like organizational unit within a portfolio.
 *
 * Books can contain positions and/or child books, forming a hierarchical tree.
 * This allows multi-level aggregation (e.g., Americas > Credit > Investment Grade).
 *
 * # Design
 *
 * - Flat position list is default; books are optional
 * - Parent-child relationships tracked via `parent_id` field
 * - Positions reference books via optional `book_id` field
 * - Positions without `book_id` are not in any book
 * - Book hierarchies are expected to be acyclic trees or forests; aggregation
 *   helpers reject cycles and excessively deep nesting instead of recursing
 *   indefinitely
 * - [`Book::child_book_ids`] drives rollup in [`crate::grouping::aggregate_by_book`];
 *   [`crate::portfolio::Portfolio::validate`] checks parent/child consistency
 *   between child lists and [`Book::parent_id`]
 */
export interface Book {
  /**
   * Child book IDs (for hierarchical organization)
   */
  child_book_ids: BookId[];
  /**
   * Unique identifier for this book
   */
  id: BookId;
  /**
   * Additional metadata
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Human-readable name
   */
  name?: string | null;
  /**
   * Parent book identifier (None for root books)
   */
  parent_id?: BookId | null;
  /**
   * Position IDs directly assigned to this book (non-recursive)
   */
  position_ids: PositionId[];
  /**
   * Book-level tags for grouping and filtering
   */
  tags?: {
    [k: string]: string;
  };
}
/**
 * Single-period Brinson-Fachler result.
 */
export interface BrinsonPeriodResult {
  /**
   * Benchmark total return for the period, `Σ_i w_b,i · r_b,i`.
   */
  benchmark_return: number;
  /**
   * Portfolio total return for the period, `Σ_i w_p,i · r_p,i`.
   */
  portfolio_return: number;
  /**
   * Per-sector effects, in the order supplied.
   */
  sectors: SectorEffect[];
  /**
   * Sum of allocation effects across sectors.
   */
  total_allocation: number;
  /**
   * Active return, `portfolio_return − benchmark_return`.
   * Equals `total_allocation + total_selection + total_interaction`.
   */
  total_excess_return: number;
  /**
   * Sum of interaction effects across sectors.
   */
  total_interaction: number;
  /**
   * Sum of selection effects across sectors.
   */
  total_selection: number;
  [k: string]: unknown;
}
/**
 * Per-sector attribution effects (Brinson-Fachler three-way split).
 */
export interface SectorEffect {
  /**
   * Allocation effect `(w_p − w_b) · (r_b,i − r_b)`.
   */
  allocation: number;
  /**
   * Interaction effect `(w_p − w_b) · (r_p,i − r_b,i)`.
   */
  interaction: number;
  /**
   * Sector identifier (mirrors [`SectorPeriod::sector`]).
   */
  sector: string;
  /**
   * Selection effect `w_b · (r_p,i − r_b,i)`.
   */
  selection: number;
  /**
   * Sum of the three effects — equal to the sector's contribution to
   * active return.
   */
  total: number;
  [k: string]: unknown;
}
/**
 * A candidate instrument that could be added to the portfolio.
 *
 * This represents an instrument not currently held but available for trading.
 * The optimizer can allocate weight to candidates (up to `max_weight`).
 */
export interface CandidatePosition {
  attributes?: {
    [k: string]: AttributeValue;
  };
  entity_id: EntityId;
  id: PositionId;
  instrument_spec: valuations.InstrumentJson;
  max_weight: number;
  min_weight: number;
  unit: PositionUnit;
}
/**
 * Compounded portfolio vs.
 */
export interface CarinoLinkedAttribution {
  /**
   * Geometrically compounded benchmark return.
   */
  benchmark_return_compounded: number;
  /**
   * Sum of per-sector linked allocation effects.
   */
  linked_allocation: number;
  /**
   * Sum of per-sector linked interaction effects.
   */
  linked_interaction: number;
  /**
   * Per-sector Carino-smoothed effects summed across periods.
   * `sum(linked_allocation + linked_selection + linked_interaction)`
   * reconstructs the active compounded return exactly.
   */
  linked_sectors: SectorEffect[];
  /**
   * Sum of per-sector linked selection effects.
   */
  linked_selection: number;
  /**
   * Per-period decompositions in chronological order.
   */
  periods: BrinsonPeriodResult[];
  /**
   * Geometrically compounded portfolio return,
   * `∏_t (1 + r_p,t) − 1`.
   */
  portfolio_return_compounded: number;
  [k: string]: unknown;
}
/**
 * Detailed carry decomposition.
 *
 * When available, breaks carry into sub-components:
 * - **coupon_income**: Net cashflows (coupons, interest) received during the period
 * - **pull_to_par**: PV convergence toward par (time effect at flat yield)
 * - **roll_down**: Curve shape benefit from aging along a sloped curve
 * - **funding_cost**: Cost of financing the position
 *
 * Carry and endpoint P&L are both gross of financing:
 * `coupon_income + pull_to_par + roll_down = total`. Funding remains a
 * separately identified overlay; net financed carry is `total - funding_cost`.
 * Metrics-based attribution adds FundingCost back to the net CarryTotal metric.
 *
 * In metrics-based attribution, these fields are populated from pre-computed
 * carry decomposition metrics when available. In repricing-based attribution
 * methods, pull-to-par uses a flat curve that inverts the instrument's YTM
 * compounding convention, and `funding_cost` is isolated when a funding/repo
 * curve is configured and present on the carry market.
 *
 * `coupon_income` and `roll_down` are typed as [`SourceLine`] so that
 * callers with a `CreditFactorModel` may further split them into rates and
 * credit components.
 *
 * # Reference
 *
 * Bloomberg PORT decomposes carry into Carry (coupon/funding), Curve Roll-Down,
 * and Shift as distinct P&L components. `docs/REFERENCES.md#campisi-2000`
 */
export interface CarryDetail {
  /**
   * Coupon/interest income received during the period (with optional
   * rates / credit split).
   */
  coupon_income?: SourceLine | null;
  /**
   * Cost of financing the position. Pure rates, never split.
   */
  funding_cost?: Money | null;
  /**
   * PV convergence toward par (time effect at flat yield).
   *
   * The wire field is a single unsplit `Money` (v1 schema). When a
   * `CreditFactorModel` drives the carry split, its rates / credit shares
   * (same `s / (r + s)` ratio as `coupon_income`) enter
   * [`CreditCarryDecomposition::rates_carry_total`] and
   * [`CreditCarryDecomposition::credit_carry_total`], so those two totals
   * partition [`CarryDetail::total`] exactly.
   */
  pull_to_par?: Money | null;
  /**
   * Curve shape benefit from aging along a sloped curve, with optional
   * rates / credit split.
   *
   * This field includes slide/rolldown effects separate from pure pull-to-par.
   * On repricing paths where the full split is unavailable, the price-carry
   * residual is folded here so the partition invariant always holds.
   */
  roll_down?: SourceLine | null;
  /**
   * Gross carry P&L. Equals `CarryTotal + FundingCost` on the metrics-based
   * path, or the repricing-based time drift plus coupons paid on the
   * reprice paths. The populated sub-lines always partition this total.
   */
  total: Money;
  [k: string]: unknown;
}
/**
 * One source-line of carry, optionally split into rates / credit components.
 *
 * Used for `CarryDetail.coupon_income` and `CarryDetail.roll_down`. When no
 * `CreditFactorModel` is supplied to attribution, `rates_part` and
 * `credit_part` are both `None` and `total` carries the canonical scalar
 * value. When a model is supplied, the two parts sum to `total` at
 * 1e-8 absolute tolerance.
 */
export interface SourceLine {
  /**
   * Credit-only contribution (populated only when a credit factor model
   * drove the split).
   */
  credit_part?: Money | null;
  /**
   * Rates-only contribution (populated only when a credit factor model
   * drove the split).
   */
  rates_part?: Money | null;
  /**
   * Total signed amount for this line (always populated).
   */
  total: Money;
}
/**
 * Options for `aggregate_full_cashflows`.
 */
export interface CashflowAggregationOptions {
  /**
   * When `false` (default), a non-empty [`PortfolioCashflows::issues`]
   * list fails the call. When `true`, remaining positions still
   * contribute to the ladder and issues are returned on the result.
   */
  allow_partial: boolean;
  [k: string]: unknown;
}
/**
 * Structured issue captured while extracting full cashflow schedules.
 */
export interface CashflowExtractionIssue {
  /**
   * Underlying instrument identifier.
   */
  instrument_id: string;
  /**
   * Underlying instrument type key.
   */
  instrument_type: InstrumentType;
  /**
   * Failure category.
   */
  kind: CashflowExtractionIssueKind;
  /**
   * Human-readable failure detail.
   */
  message: string;
  /**
   * Position whose cashflow extraction was attempted.
   */
  position_id: PositionId;
  [k: string]: unknown;
}
/**
 * Configuration for `cell_returns_from_reference`.
 */
export interface CellConfig {
  /**
   * Width of each duration cell, in years. Must be finite and positive.
   */
  width: number;
}
/**
 * One duration cell's base return, observed or filled.
 *
 * This type is *input-reachable*: later stages of duration-matched excess
 * return attribution consume [`DurationCellTable`] values, and the
 * Python/WASM bindings deserialize them from JSON. It therefore denies
 * unknown fields like the other inbound types in this crate, so a
 * misspelled or stale key fails closed instead of being silently dropped.
 */
export interface CellReturn {
  /**
   * Cell base return: simple average of member reference returns, or
   * interpolated/extrapolated when the cell has no members.
   */
  base_return: number;
  /**
   * Human-readable cell label, e.g. `"5.0-5.5"` (see
   * [`duration_cell_label`]). In an inbound [`DurationCellTable`] consumed
   * by [`excess_returns`], labels must be non-empty and unique.
   */
  label: string;
  /**
   * Cell lower bound (inclusive), in years.
   */
  lower: number;
  /**
   * `true` if at least one reference instrument fell in this cell,
   * `false` if the value was filled by interpolation or flat
   * extrapolation.
   */
  observed: boolean;
  /**
   * Cell upper bound (exclusive), in years.
   *
   * One exception: the *top* cell (the cell containing the largest
   * reference duration) folds a reference instrument whose duration is
   * numerically equal to `upper` into this cell instead of raising `upper`
   * to start a new, empty cell. This only happens for the single top cell
   * of the table, and only when the largest reference duration is an
   * exact multiple of the configured cell width (e.g. a duration of
   * `2.0` with `width = 0.5` is folded into the `1.5-2.0` cell rather
   * than opening an empty `2.0-2.5` cell). Every other cell boundary is
   * strictly half-open.
   */
  upper: number;
}
/**
 * Detailed attribution for base correlation curves.
 *
 * Used for structured credit products (CDO tranches, synthetic credit).
 */
export interface CorrelationsAttribution {
  /**
   * P&L by correlation curve ID.
   */
  by_curve: {
    [k: string]: Money;
  };
  [k: string]: unknown;
}
/**
 * Per-factor breakdown of credit carry.
 */
export interface CreditCarryByLevel {
  /**
   * Optional per-issuer adder breakdown (gated by
   * `CreditFactorDetailOptions.include_per_issuer_adder`).
   */
  adder_by_issuer?: {
    [k: string]: Money;
  } | null;
  /**
   * Sum of issuer-specific adder carry contributions.
   */
  adder_total: Money;
  /**
   * Generic (PC) factor contribution to credit carry.
   */
  generic: Money;
  /**
   * One entry per hierarchy level, in spec order.
   */
  levels: LevelCarry[];
  [k: string]: unknown;
}
/**
 * Carry contribution from a single hierarchy level.
 */
export interface LevelCarry {
  /**
   * Optional per-bucket breakdown keyed by dotted bucket path.
   */
  by_bucket?: {
    [k: string]: Money;
  };
  /**
   * Human-readable level name (e.g. `"rating"`, `"region"`).
   */
  level_name: string;
  /**
   * Aggregate carry for this level across all buckets.
   */
  total: Money;
  [k: string]: unknown;
}
/**
 * Factor-cut decomposition of carry under a calibrated `CreditFactorModel`.
 * Populated only when an `AttributionSpec.credit_factor_model` was supplied.
 * Purely additive — does not modify any existing field.
 *
 * # Reconciliation invariants (§7.4, all at 1e-8 absolute tolerance)
 *
 * With `w = s / (r + s)` the credit share used for the coupon split
 * (see `compute_carry_credit_split_and_decomposition`):
 *
 * - `rates_carry_total + credit_carry_total ≡ carry_detail.total` — the two
 *   legs partition the full carry, **including `pull_to_par`**, which is
 *   split on the same `w` (its wire field stays a single unsplit `Money`)
 * - `credit_carry_total ≡ Σ_lines SourceLine.credit_part + w × pull_to_par`
 *   (lines = coupon + roll)
 * - `credit_carry_total ≡ generic + Σ_levels(level.total) + adder_total`
 * - `rates_carry_total ≡ Σ_lines SourceLine.rates_part
 *   + (1 − w) × pull_to_par`
 * - `funding_cost` is a separate financing overlay outside both gross totals.
 *
 * # Attribution method coverage
 *
 * All four methods populate `carry_detail`: Parallel, Waterfall and Taylor
 * via `apply_total_return_carry` (theta + coupon_income, with financing
 * identified separately when configured), MetricsBased from the carry
 * decomposition metrics. `credit_carry_decomposition` is therefore emitted
 * on any path whose `carry_detail.coupon_income` is populated when a
 * `CreditFactorModel` is supplied — the decomposition logic is
 * method-agnostic.
 */
export interface CreditCarryDecomposition {
  /**
   * Per-factor breakdown of `credit_carry_total`.
   */
  credit_by_level: CreditCarryByLevel;
  /**
   * Sum of credit components across split source lines.
   */
  credit_carry_total: Money;
  /**
   * Deterministic traceability id of the model used (matches
   * `CreditFactorAttribution.model_id`).
   */
  model_id: string;
  /**
   * Sum of rates components across split source lines, minus funding cost.
   */
  rates_carry_total: Money;
  [k: string]: unknown;
}
/**
 * Detailed attribution for credit hazard curves.
 *
 * Provides per-curve and per-tenor breakdown for credit spread risk.
 */
export interface CreditCurvesAttribution {
  /**
   * P&L by curve ID.
   */
  by_curve: {
    [k: string]: Money;
  };
  /**
   * P&L by (curve_id, tenor), serialized with `"{curve_id}|{tenor}"` keys.
   */
  by_tenor?: {
    [k: string]: Money;
  };
  [k: string]: unknown;
}
/**
 * Hierarchy-level decomposition of credit P&L, opt-in via
 * `AttributionSpec.credit_factor_model`.
 *
 * The reconciliation invariant
 *
 * ```text
 * generic_pnl + Σ_levels(level.total) + adder_pnl_total + curve_shape_pnl ≡ credit_curves_pnl
 * ```
 *
 * holds at absolute tolerance `1e-8` for both metrics-based and Taylor methods.
 * `curve_shape_pnl` is the non-parallel hazard-curve residual;
 * for a purely parallel credit move it is zero and the invariant reduces to
 * the historical `generic + Σ levels + adder` form.
 *
 * **Single-instrument scope**: when produced via the valuations-layer
 * per-instrument attribution wire (`metrics_based`, `taylor`), each call
 * processes a single instrument. Therefore `LevelPnl.by_bucket` will contain
 * at most one entry per call (the issuer's bucket at that level).
 * Portfolio-level multi-bucket aggregation is provided at the portfolio layer.
 */
export interface CreditFactorAttribution {
  /**
   * Diagnostic: absolute magnitude of the per-issuer adder step P&L
   * (`|adder_pnl_total|`). Surfaced so downstream consumers can detect
   * when the adder is absorbing significant non-parallel curve moves.
   * A `tracing::warn!` is also emitted by the cascade builder when the
   * adder magnitude exceeds `credit_cascade::ADDER_MAGNITUDE_WARN_RATIO`
   * of total credit P&L.
   */
  adder_magnitude?: Money | null;
  /**
   * Optional per-issuer adder breakdown (gated by
   * `CreditFactorDetailOptions.include_per_issuer_adder`, default off).
   */
  adder_pnl_by_issuer?: {
    [k: string]: Money;
  } | null;
  /**
   * Total adder P&L: `Σ_i CS01_i × Δadder_i` (canonical signed CS01).
   * This is the **parallel** issuer-idiosyncratic move only; non-parallel
   * curve-shape risk is reported separately in [`Self::curve_shape_pnl`].
   *
   * **Degenerate (no factor observations) semantics**: when the market
   * carries no scalar series for any credit factor, nothing about the
   * issuer's spread move is identifiably systematic, so the entire
   * parallel ΔS is routed here (generic and level components are zero).
   */
  adder_pnl_total: Money;
  /**
   * P&L attributed to the **non-parallel** part of the hazard-curve move
   * (steepening / twist / term-structure roll) — the curve-shape residual.
   *
   * On the linear (MetricsBased / Taylor) wire the parallel steps are
   * single-CS01 × Δbp products, so this closing residual also absorbs
   * **spread convexity** of large parallel moves — read it as
   * "non-parallel + higher-order", not pure curve shape, on that path.
   * The reprice-based parallel/waterfall cascades distribute convexity
   * into the steps via cumulative bumps.
   *
   * The reconciliation invariant is:
   * `generic + Σ levels + adder + curve_shape ≡ credit_curves_pnl`.
   */
  curve_shape_pnl: Money;
  /**
   * P&L attributed to the generic (PC) credit factor:
   * `Σ_i CS01_i × β_i^PC × ΔF_PC` (canonical signed CS01 = ∂PV/∂s,
   * negative for long credit — no extra negation).
   */
  generic_pnl: Money;
  /**
   * One entry per [`finstack_quant_models::factor::credit::hierarchy::HierarchyDimension`]
   * in the spec order recorded by the model's hierarchy.
   */
  levels: LevelPnl[];
  /**
   * Deterministic traceability ID for the calibrated model. Format:
   * `format!("{}/{:016x}", model.as_of, fnv1a64(serde_json::to_string(model)))`.
   */
  model_id: string;
  [k: string]: unknown;
}
/**
 * P&L contribution from a single hierarchy level.
 */
export interface LevelPnl {
  /**
   * Optional per-bucket breakdown keyed by dotted bucket path
   * (e.g. `"IG.EU.FIN"`). Empty when
   * `CreditFactorDetailOptions.include_per_bucket_breakdown == false`.
   */
  by_bucket?: {
    [k: string]: Money;
  };
  /**
   * Human-readable level name (e.g. `"rating"`, `"region"`, `"sector"`,
   * or a custom dimension key).
   */
  level_name: string;
  /**
   * Aggregate P&L for this level across all buckets.
   */
  total: Money;
  [k: string]: unknown;
}
/**
 * Aggregated credit risk grouped by hierarchy level.
 */
export interface CreditVolReport {
  /**
   * Per-hierarchy-level rollups.
   */
  by_level: LevelVolContribution[];
  /**
   * Optional position-level breakdown.
   */
  by_position_optional?: PositionVolContribution[] | null;
  /**
   * Contribution from the generic credit factor.
   */
  generic: number;
  /**
   * Portfolio idiosyncratic contribution.
   */
  idiosyncratic_total: number;
  /**
   * Risk measure used by the underlying decomposition.
   */
  measure: RiskMeasure;
  /**
   * Total risk under the selected measure.
   */
  total: number;
  [k: string]: unknown;
}
/**
 * Aggregated risk contribution for one hierarchy level.
 */
export interface LevelVolContribution {
  /**
   * Contributions keyed by canonical bucket path.
   */
  by_bucket: {
    [k: string]: number;
  };
  /**
   * Human-readable hierarchy level name.
   */
  level_name: string;
  /**
   * Total contribution across the level's buckets.
   */
  total: number;
  [k: string]: unknown;
}
/**
 * Position-level credit risk breakdown.
 */
export interface PositionVolContribution {
  /**
   * Systematic factor contribution.
   */
  factor_total: number;
  /**
   * Idiosyncratic contribution.
   */
  idiosyncratic: number;
  /**
   * Portfolio position identifier.
   */
  position_id: PositionId;
  /**
   * Sum of systematic and idiosyncratic contributions.
   */
  total: number;
  [k: string]: unknown;
}
/**
 * Detailed attribution for cross-factor interaction terms.
 */
export interface CrossFactorDetail {
  /**
   * P&L by human-readable factor-pair label.
   */
  by_pair?: {
    [k: string]: Money;
  };
  /**
   * Total cross-factor P&L across all populated pairs.
   */
  total: Money;
  [k: string]: unknown;
}
/**
 * A dated cashflow amount for money-weighted return calculations.
 */
export interface DatedCashflow {
  /**
   * Signed amount from the investor's cash account: contributions are
   * negative; terminal value or distributions back to the investor are
   * positive. Opposite of [`DietzFlow::amount`].
   */
  amount: number;
  /**
   * Cashflow date.
   */
  date: DateWire;
  [k: string]: unknown;
}
export interface DegradedPosition {
  message: string;
  position_id: string;
  [k: string]: unknown;
}
/**
 * One structured finding produced while loading a persisted artifact.
 *
 * Field names and enum representations are a persisted JSON contract. Unknown
 * fields are rejected during deserialization.
 */
export interface Diagnostic {
  /**
   * Version found in the payload, when one was present.
   */
  actual_version?: number | null;
  /**
   * Canonical artifact digest, including its algorithm prefix, when available.
   */
  artifact_hash?: string | null;
  /**
   * Stable machine-readable code, such as `"contract/version-unsupported"`.
   */
  code: string;
  /**
   * Stable persisted contract identifier, when the finding is contract-specific.
   */
  contract?: string | null;
  /**
   * Version expected by the loader, when version context is available.
   */
  expected_version?: number | null;
  /**
   * Instrument identifier associated with the finding, when available.
   */
  instrument_id?: string | null;
  /**
   * Human-readable explanation with source details such as parse line and column.
   */
  message: string;
  /**
   * Loading stage that produced the finding.
   */
  phase: LoadPhase;
  /**
   * JSON Pointer locating the finding in the source document, when known.
   */
  pointer?: string | null;
  /**
   * Portfolio-position identifier associated with the finding, when available.
   */
  position_id?: string | null;
  /**
   * Storage revision identifier associated with the artifact, when available.
   */
  revision_id?: string | null;
  /**
   * Whether the finding is fatal or recoverable.
   */
  severity: Severity;
}
/**
 * A single external cashflow within a TWRR sub-period.
 */
export interface DietzFlow {
  /**
   * Signed flow amount from the portfolio's books: positive = contribution
   * into the portfolio; negative = withdrawal. Opposite of
   * [`DatedCashflow::amount`].
   */
  amount: number;
  /**
   * Day-weighted fraction of the period from the flow to period end,
   * `w = (T − t_flow) / T ∈ [0, 1]`. Flow at period start has `w = 1`;
   * flow at period end has `w = 0`.
   */
  fraction_of_period_remaining: number;
}
/**
 * A full duration-cell base-return curve built from a reference universe.
 */
export interface DurationCellTable {
  /**
   * Label identifying the reference universe this curve was built from
   * (e.g. `"UST"`, `"SWAP"`).
   */
  base_label: string;
  /**
   * Cells in ascending duration order, spanning
   * `[0, ceil(max_duration / width) * width)` for tables generated by this
   * module. Inbound tables consumed by [`excess_returns`] may contain gaps,
   * but must remain sorted and non-overlapping.
   */
  cells: CellReturn[];
}
/**
 * An entity that can hold positions.
 *
 * Entities represent companies, funds, or other legal entities that
 * own instruments. For standalone instruments (derivatives, FX), use
 * the dummy entity.
 */
export interface Entity {
  /**
   * Unique identifier for the entity
   */
  id: EntityId;
  /**
   * Additional metadata
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Human-readable name
   */
  name?: string | null;
  /**
   * Entity-level tags for grouping and filtering
   */
  tags?: {
    [k: string]: string;
  };
}
/**
 * One position's beginning-of-period duration, weight and realized total return, the input to `excess_returns`.
 */
export interface ExcessReturnPosition {
  /**
   * Beginning-of-period duration in years, used to look up the
   * duration-matched cell in the [`DurationCellTable`]. Must be finite,
   * non-negative, and fall inside one of the table's cells; durations in a
   * valid gap are rejected like any other unmatched duration.
   */
  duration: number;
  /**
   * Position identifier, used to name the position in any validation
   * error and echoed onto the matching [`PositionExcess::id`].
   */
  id: string;
  /**
   * Realized period total return (decimal, e.g. `0.0225` = 2.25%). Must
   * be finite.
   */
  total_return: number;
  /**
   * Portfolio weight (decimal, e.g. `0.2` = 20%). Across all positions
   * passed to [`excess_returns`] these must sum to `1.0` within
   * `WEIGHT_TOLERANCE`. Must be finite.
   */
  weight: number;
}
/**
 * Per-position and portfolio-level duration-matched credit excess return result, produced by `excess_returns`.
 */
export interface ExcessReturnResult {
  /**
   * Label of the base curve the excess returns were measured against
   * (mirrors [`DurationCellTable::base_label`]).
   */
  base_label: string;
  /**
   * Weight-weighted portfolio base return `Σ w_i · base_return_i`.
   */
  portfolio_base_return: number;
  /**
   * Weight-weighted portfolio excess return `Σ w_i · excess_return_i`,
   * accumulated independently from `portfolio_total_return` and
   * `portfolio_base_return` — not computed as their difference. The
   * identity `portfolio_excess_return == portfolio_total_return −
   * portfolio_base_return` holds algebraically for any correct
   * implementation of [`excess_returns`], but computing
   * `portfolio_excess_return` *as* that difference would subtract two
   * large, nearly-equal weighted sums whenever positions' total and base
   * returns share a large common component — exactly the shape of
   * catastrophic cancellation. Accumulating the already-small
   * per-position excesses directly avoids that precision loss; see
   * [`excess_returns`]'s
   * `portfolio_excess_return_avoids_catastrophic_cancellation` test,
   * which constructs such a case and shows the two computations
   * measurably diverge (~3e-9 absolute, against sums of order 1e8).
   */
  portfolio_excess_return: number;
  /**
   * Weight-weighted portfolio total return `Σ w_i · total_return_i`.
   */
  portfolio_total_return: number;
  /**
   * Per-position results, in input order.
   */
  positions: PositionExcess[];
}
/**
 * One position's duration-matched credit excess return.
 *
 * This type is *input-reachable*: the Python/WASM bindings deserialize
 * [`ExcessReturnResult`] (which embeds a `Vec` of these) from JSON, so it
 * denies unknown fields like the other inbound types in this module.
 */
export interface PositionExcess {
  /**
   * The matched cell's base return.
   */
  base_return: number;
  /**
   * Label of the duration cell the position was matched to (mirrors
   * [`CellReturn::label`]).
   */
  cell: string;
  /**
   * `total_return − base_return` for this position.
   */
  excess_return: number;
  /**
   * Position identifier (mirrors [`ExcessReturnPosition::id`]).
   */
  id: string;
}
/**
 * Assignment results for a portfolio-level factor mapping pass.
 */
export interface FactorAssignmentReport {
  /**
   * Per-position matched dependencies and factor identifiers.
   */
  assignments: PositionAssignment[];
  /**
   * Dependencies that did not match any configured factor.
   */
  unmatched: UnmatchedEntry[];
  [k: string]: unknown;
}
/**
 * Matched factor assignments for a single portfolio position.
 *
 * Every factor matched for a dependency is recorded — hierarchical matchers
 * (e.g. the credit matcher) emit one `(dependency, factor_id, beta)` triple
 * per level, so a single dependency may appear in several mappings.
 */
export interface PositionAssignment {
  /**
   * Matched `(dependency, factor_id, beta)` triples for this position.
   */
  mappings: [unknown, unknown, unknown][];
  /**
   * Portfolio position identifier.
   */
  position_id: PositionId;
  [k: string]: unknown;
}
/**
 * Single unmatched dependency surfaced during assignment.
 */
export interface UnmatchedEntry {
  /**
   * Dependency that could not be matched.
   */
  dependency: MarketDependency;
  /**
   * Portfolio position identifier.
   */
  position_id: PositionId;
  [k: string]: unknown;
}
/**
 * One factor's contribution to the factor (allocation) effect.
 */
export interface FactorBrinsonContribution {
  /**
   * Active factor loading `w_k = Σ_i X_ik (h_p,i − h_b,i)`.
   */
  active_loading: number;
  /**
   * Contribution `w_k * f_k` (undemeaned; see module docs).
   */
  contribution: number;
  /**
   * Factor name (mirrors [`FactorBrinsonInput::factor_names`]).
   */
  factor: string;
  /**
   * Benchmark factor return `f_k` supplied by the caller.
   */
  factor_return: number;
}
/**
 * Inputs to `factor_brinson_attribution`: per-asset returns, a factor exposure matrix, and portfolio/benchmark weights.
 */
export interface FactorBrinsonInput {
  /**
   * Asset identifiers, length `n_assets`.
   */
  asset_ids: string[];
  /**
   * Realized asset returns (decimal, e.g. `0.02` = 2%), length `n_assets`.
   */
  asset_returns: number[];
  /**
   * Benchmark weight per asset, length `n_assets`. Must sum to `1.0`.
   */
  benchmark_weights: number[];
  /**
   * Row-major factor exposure matrix, `n_assets x n_factors`: asset `i`'s
   * exposure to factor `j` is `exposures[i * n_factors + j]`.
   */
  exposures: number[];
  /**
   * Factor names, length `n_factors` (defines `n_factors`).
   */
  factor_names: string[];
  /**
   * Portfolio weight per asset, length `n_assets`. Must sum to `1.0`.
   */
  portfolio_weights: number[];
}
/**
 * Factor-Brinson unified attribution result.
 */
export interface FactorBrinsonResult {
  /**
   * Active return, `portfolio_return − benchmark_return`.
   */
  active_return: number;
  /**
   * Factor (allocation) contribution, `FC = w'f_b` where
   * `w = X'(h_p − h_b)`.
   */
  allocation: number;
  /**
   * Per-asset breakdown of `selection`, in `asset_ids` order.
   */
  asset_contributions: AssetSpecificContribution[];
  /**
   * Benchmark total return, `h_b'r`.
   */
  benchmark_return: number;
  /**
   * Per-factor breakdown of `allocation`, in `factor_names` order.
   */
  factor_contributions: FactorBrinsonContribution[];
  /**
   * Portfolio total return, `h_p'r`.
   */
  portfolio_return: number;
  /**
   * Specific (selection) contribution, `SC = (h_p − h_b)'ε_b`.
   */
  selection: number;
}
/**
 * Contribution of a single factor to portfolio risk.
 *
 * See [`RiskDecomposition`] for the sign convention applied to each field.
 */
export interface FactorContribution {
  /**
   * Absolute contribution of the factor to the chosen risk measure.
   * Sign follows the measure's convention.
   */
  absolute_risk: number;
  /**
   * Identifier of the factor being reported.
   */
  factor_id: FactorId;
  /**
   * Marginal sensitivity of portfolio risk to the factor. Same sign
   * convention as `absolute_risk`.
   */
  marginal_risk: number;
  /**
   * Contribution expressed as a share of total portfolio risk. Dimensionless,
   * non-negative for standard long-risk portfolios (signs of numerator and
   * denominator cancel).
   */
  relative_risk: number;
  [k: string]: unknown;
}
/**
 * Base/after delta for a single factor contribution.
 */
export interface FactorContributionDelta {
  /**
   * Absolute change in the reported risk contribution.
   */
  absolute_change: number;
  /**
   * Factor identifier whose contribution changed.
   */
  factor_id: FactorId;
  /**
   * Relative change in the reported risk contribution.
   */
  relative_change: number;
  [k: string]: unknown;
}
/**
 * P&L profile for one factor across a scenario grid.
 */
export interface FactorPnlProfile {
  /**
   * Reporting currency of all per-position P&L amounts.
   */
  base_currency: Currency;
  /**
   * Identifier of the shocked factor.
   */
  factor_id: FactorId;
  /**
   * Ordered position identifiers indexing the inner `position_pnls` axis.
   */
  position_ids: string[];
  /**
   * Per-shift P&L vectors indexed as `[shift_idx][position_idx]`.
   */
  position_pnls: number[][];
  /**
   * Scenario shift coordinates in bump-size units.
   */
  shifts: number[];
  [k: string]: unknown;
}
/**
 * Configuration for `campisi_attribution`.
 */
export interface FiAttributionConfig {
  /**
   * Length of the attribution period in years (e.g. `0.25` for a
   * quarter). Scales `yield_annual` into the period carry.
   */
  period_years: number;
}
/**
 * Single-period Campisi benchmark-relative attribution result.
 *
 * This type is *input-reachable*: [`campisi_carino_link`] consumes a slice of
 * these, and the Python/WASM bindings deserialize them from JSON. It therefore
 * denies unknown fields like the other inbound types in this module, so a
 * misspelled or stale key fails closed instead of being silently dropped.
 */
export interface FiAttributionResult {
  /**
   * Active return `portfolio_return − benchmark_return`.
   */
  active_return: number;
  /**
   * Benchmark-side absolute Campisi split.
   */
  benchmark_components: FiComponents;
  /**
   * Benchmark total return `Σ w_b,j r_b,j`.
   */
  benchmark_return: number;
  /**
   * Portfolio-side absolute Campisi split.
   */
  portfolio_components: FiComponents;
  /**
   * Portfolio total return `Σ w_p,j r_p,j`.
   */
  portfolio_return: number;
  /**
   * Per-sector effects, in first-seen order (portfolio first, then
   * benchmark-only sectors).
   */
  sectors: FiSectorEffect[];
  /**
   * Sum of sector active carry effects.
   */
  total_active_carry: number;
  /**
   * Sum of sector active spread effects.
   */
  total_active_spread: number;
  /**
   * Sum of sector active treasury effects.
   */
  total_active_treasury: number;
  /**
   * Sum of sector allocation effects.
   */
  total_allocation: number;
  /**
   * Sum of sector selection effects.
   */
  total_selection: number;
}
/**
 * Absolute Campisi component contributions for one side (portfolio or
 * benchmark), each `Σ_j w_j × component_j`.
 */
export interface FiComponents {
  /**
   * Income effect `Σ w · y · Δt`.
   */
  carry: number;
  /**
   * Residual selection `Σ w · (r − explained)`.
   */
  selection: number;
  /**
   * Spread effect `Σ w · (−SD · Δs)`.
   */
  spread: number;
  /**
   * Sum of the four components — equals the side's total return.
   */
  total: number;
  /**
   * Treasury/duration effect `Σ w · (−MD · Δy)`.
   */
  treasury: number;
}
/**
 * Per-sector benchmark-relative effects.
 */
export interface FiSectorEffect {
  /**
   * Active income effect `w_p (carry_p,i − carry_b,i)`.
   */
  active_carry: number;
  /**
   * Active spread positioning `w_p (spread_p,i − spread_b,i)`.
   */
  active_spread: number;
  /**
   * Active duration positioning `w_p (treasury_p,i − treasury_b,i)`.
   */
  active_treasury: number;
  /**
   * Brinson-Fachler allocation `(w_p − w_b)(r_b,i − r_b)`.
   */
  allocation: number;
  /**
   * Benchmark sector return (weighted, per unit of sector weight).
   *
   * For a sector absent from the benchmark this is the benchmark *total*
   * return `R_b` (the off-benchmark convention; see "Off-benchmark
   * sectors" in the module docs), so [`Self::allocation`] is identically
   * zero for such sectors and their active contribution flows through the
   * component effects instead.
   */
  benchmark_return: number;
  /**
   * Benchmark weight in the sector.
   */
  benchmark_weight: number;
  /**
   * Portfolio sector return (weighted, per unit of sector weight).
   *
   * For a sector absent from the portfolio this mirrors
   * [`Self::benchmark_return`] (`r_p,i := r_b,i`) — reporting-only, since
   * every effect multiplies it by the zero portfolio weight.
   */
  portfolio_return: number;
  /**
   * Portfolio weight in the sector.
   */
  portfolio_weight: number;
  /**
   * Sector label (mirrors [`FiPositionSnapshot::sector`]).
   */
  sector: string;
  /**
   * Security selection residual `w_p (selection_p,i − selection_b,i)`.
   */
  selection: number;
  /**
   * Sum of the five effects — the sector's contribution to active return.
   */
  total_active: number;
}
/**
 * Multi-period Carino-linked Campisi attribution.
 */
export interface FiCarinoLinkedResult {
  /**
   * Geometrically compounded benchmark return.
   */
  benchmark_return_compounded: number;
  /**
   * Sum of linked active carry effects.
   */
  linked_active_carry: number;
  /**
   * Sum of linked active spread effects.
   */
  linked_active_spread: number;
  /**
   * Sum of linked active treasury effects.
   */
  linked_active_treasury: number;
  /**
   * Sum of linked allocation effects.
   */
  linked_allocation: number;
  /**
   * Per-sector linked effects; their grand total reconstructs the
   * compounded active return exactly.
   */
  linked_sectors: FiLinkedSectorEffect[];
  /**
   * Sum of linked selection effects.
   */
  linked_selection: number;
  /**
   * Per-period single-period results, in chronological order.
   */
  periods: FiAttributionResult[];
  /**
   * Geometrically compounded portfolio return `∏(1 + r_p,t) − 1`.
   */
  portfolio_return_compounded: number;
  [k: string]: unknown;
}
/**
 * Carino-linked per-sector FI effects summed across periods.
 */
export interface FiLinkedSectorEffect {
  /**
   * Linked active carry effect.
   */
  active_carry: number;
  /**
   * Linked active spread effect.
   */
  active_spread: number;
  /**
   * Linked active treasury effect.
   */
  active_treasury: number;
  /**
   * Linked allocation effect.
   */
  allocation: number;
  /**
   * Sector label.
   */
  sector: string;
  /**
   * Linked selection effect.
   */
  selection: number;
  /**
   * Sum of the five linked effects.
   */
  total_active: number;
  [k: string]: unknown;
}
/**
 * One attribution period's raw inputs for multi-period linking.
 */
export interface FiPeriodInput {
  /**
   * Benchmark snapshots for the period.
   */
  benchmark: FiPositionSnapshot[];
  /**
   * Portfolio snapshots for the period.
   */
  portfolio: FiPositionSnapshot[];
}
/**
 * Plain-data snapshot of one position (or pre-aggregated bucket) for one
 * attribution period.
 *
 * Weights are fractions of the whole portfolio (or benchmark) and must sum
 * to 1.0 within each side. Returns, yields and spreads are decimals
 * (`0.02` = 2 %); durations are in years.
 */
export interface FiPositionSnapshot {
  /**
   * Absolute change in the quote-reproducing Z-spread over the period
   * (decimal).
   *
   * Must use the same Z-spread basis as [`Self::spread_duration`] and
   * [`Self::spread`].
   */
  delta_spread: number;
  /**
   * Change in the treasury/benchmark yield relevant to this position's
   * duration bucket over the period (decimal).
   */
  delta_treasury_yield: number;
  /**
   * Modified duration in years at period start.
   */
  modified_duration: number;
  /**
   * Sector bucket label (industry / quality / any grouping).
   */
  sector: string;
  /**
   * Quote-reproducing Z-spread at period start (decimal).
   *
   * Carried for provenance and downstream reporting only — the
   * decomposition never divides by it, so any finite value (including zero
   * and negative levels) is accepted. See "Why there is no DTS spread mode"
   * in the module docs.
   */
  spread: number;
  /**
   * Quote-reproducing Z-spread duration in years at period start.
   *
   * Must use the same Z-spread basis as [`Self::spread`] and
   * [`Self::delta_spread`]; OAS, G-spread, and discount-margin durations
   * are not compatible inputs.
   */
  spread_duration: number;
  /**
   * Realized total return for the period (decimal).
   */
  total_return: number;
  /**
   * Weight as a fraction of the whole side at period start.
   */
  weight: number;
  /**
   * Annualized yield at period start (decimal; e.g. YTM).
   */
  yield_annual: number;
}
/**
 * Report from reconciling the five effect totals against the active return, mirroring `crate::attribution` reconciliation conventions.
 */
export interface FiReconciliationReport {
  /**
   * Whether the residual is within tolerance.
   */
  is_reconciled: boolean;
  /**
   * Tolerance used for the check.
   */
  tolerance: number;
  /**
   * `active_return − (allocation + carry + treasury + spread + selection)`.
   */
  total_residual: number;
}
/**
 * Detailed attribution for FX rate changes.
 *
 * Provides per-currency-pair breakdown.
 */
export interface FxAttribution {
  /**
   * P&L by (from_currency, to_currency) pair, serialized with
   * `"{FROM}/{TO}"` keys (e.g. `"EUR/USD"`).
   */
  by_pair?: {
    [k: string]: Money;
  };
  [k: string]: unknown;
}
/**
 * FX pair identifier using base/quote currency ordering.
 */
export interface FxPair {
  /**
   * Base currency (numerator).
   */
  base: Currency;
  /**
   * Quote currency (denominator).
   */
  quote: Currency;
}
/**
 * Single-period hierarchical grid attribution result.
 *
 * This type is *input-reachable*: multi-period linking (Task 5) consumes a
 * slice of these, and the Python/WASM bindings deserialize them from JSON.
 * It therefore denies unknown fields, like the other inbound types in this
 * module, so a misspelled or stale key fails closed instead of being
 * silently dropped.
 */
export interface GridAttributionResult {
  /**
   * Active return `r^P − r^B`.
   */
  active_return: number;
  /**
   * Benchmark total return `r^B = Σ_t x_t^B r_t^B`.
   */
  benchmark_return: number;
  /**
   * Per-cell curve effects, in first-appearance order.
   */
  curve_effects: GridCellEffect[];
  /**
   * Portfolio total return `r^P = Σ_t x_t^P r_t^P`.
   */
  portfolio_return: number;
  /**
   * Per-(cell, sector) allocation effects, in first-appearance order
   * (cells, then sectors within each cell).
   */
  sector_effects: GridSectorEffect[];
  /**
   * Per-(cell, sector) selection effects, in the same order as
   * `sector_effects`.
   */
  selection_effects: GridSelectionEffect[];
  /**
   * Sum of the curve effects.
   */
  total_curve: number;
  /**
   * Sum of the sector allocation effects.
   */
  total_sector: number;
  /**
   * Sum of the selection effects.
   */
  total_selection: number;
}
/**
 * Per-cell curve (duration-cell positioning) effect.
 */
export interface GridCellEffect {
  /**
   * Benchmark cell return, `r_t^B`. For a cell absent from the benchmark
   * this is the out-of-benchmark fallback (`r_t^P`); see the module docs.
   */
  benchmark_cell_return: number;
  /**
   * Benchmark net weight in the cell, `x_t^B`.
   */
  benchmark_weight: number;
  /**
   * Duration-cell label.
   */
  cell: string;
  /**
   * Curve effect `(x_t^P − x_t^B)(r_t^B − r^B)`.
   */
  curve_effect: number;
  /**
   * Portfolio net weight in the cell, `x_t^P`.
   */
  portfolio_weight: number;
}
/**
 * Per-(cell, sector) within-cell sector-allocation effect.
 */
export interface GridSectorEffect {
  /**
   * Allocation effect `x_t^P (z_st^P − z_st^B)(r_st^B − r_t^B)`.
   */
  allocation_effect: number;
  /**
   * Duration-cell label.
   */
  cell: string;
  /**
   * Sector label within the cell.
   */
  sector: string;
}
/**
 * Per-(cell, sector) security-selection effect.
 */
export interface GridSelectionEffect {
  /**
   * Duration-cell label.
   */
  cell: string;
  /**
   * Sector label within the cell.
   */
  sector: string;
  /**
   * Selection effect `y_st^P (r_st^P − r_st^B)`.
   */
  selection_effect: number;
}
/**
 * Multi-period Carino-linked hierarchical grid attribution.
 */
export interface GridCarinoLinkedResult {
  /**
   * Geometrically compounded benchmark return, `∏_t (1 + r_b,t) − 1`.
   */
  benchmark_return_compounded: number;
  /**
   * Sum of per-period Carino-scaled curve effects.
   *
   * `linked_curve + linked_sector + linked_selection` reconstructs
   * `portfolio_return_compounded − benchmark_return_compounded` exactly.
   */
  linked_curve: number;
  /**
   * Sum of per-period Carino-scaled sector allocation effects.
   */
  linked_sector: number;
  /**
   * Sum of per-period Carino-scaled selection effects.
   */
  linked_selection: number;
  /**
   * Per-period single-period grid attribution results, in chronological
   * order.
   */
  periods: GridAttributionResult[];
  /**
   * Geometrically compounded portfolio return, `∏_t (1 + r_p,t) − 1`.
   */
  portfolio_return_compounded: number;
  [k: string]: unknown;
}
/**
 * One position (or pre-aggregated bucket) in a duration-cell x sector grid, for one period and one side (portfolio or benchmark).
 */
export interface GridPosition {
  /**
   * Duration-cell label. Cell labels may come from
   * [`crate::excess_return::duration_cell_label`], but any string key is
   * accepted — this module has no dependency on how cells were built.
   */
  cell: string;
  /**
   * Sector bucket label within the cell.
   */
  sector: string;
  /**
   * Realized total return for the period (decimal).
   */
  total_return: number;
  /**
   * Weight as a fraction of the whole side at period start (decimal).
   */
  weight: number;
}
/**
 * One-way IM collateral account after applying the CSA's allocated threshold.
 *
 * IM remains separate from VM. Segregation determines the custody requirement;
 * it does not discount gross model risk or allow the account to offset VM.
 */
export interface ImCollateralResult {
  /**
   * Existing nonnegative collateral balance in this one-way IM account.
   */
  current_collateral: Money;
  /**
   * Gross model IM after MPOR adjustment, before contractual thresholds.
   */
  gross_initial_margin: Money;
  /**
   * Target collateral balance, max(gross IM minus allocated CSA threshold, 0).
   */
  required_collateral: Money;
  /**
   * True requires separate custody with no reuse to satisfy VM obligations.
   */
  segregated: boolean;
  /**
   * Signed transfer: positive posts additional IM, negative returns excess.
   * Absolute transfers strictly below MTA are zero; equality triggers transfer.
   */
  transfer: Money;
}
/**
 * Detailed attribution for inflation curves.
 *
 * Provides per-curve breakdown with optional tenor detail for
 * term-structured inflation curves.
 */
export interface InflationCurvesAttribution {
  /**
   * P&L by curve ID.
   */
  by_curve: {
    [k: string]: Money;
  };
  /**
   * P&L by (curve_id, tenor) for term-structured inflation curves,
   * serialized with `"{curve_id}|{tenor}"` keys.
   */
  by_tenor?: {
    [k: string]: Money;
  } | null;
  [k: string]: unknown;
}
/**
 * One unique, content-addressed instrument artifact.
 */
export interface InstrumentArtifact {
  /**
   * Immutable producer revision ID or content-addressed artifact ID.
   */
  artifact_id: string;
  /**
   * Optional claimed canonical digest, verified before decoding.
   */
  content_hash?: string | null;
  /**
   * Optional producer dependency claim, checked against runtime extraction.
   */
  dependencies?: MarketDependencies | null;
  /**
   * Full typed, strict instrument envelope.
   */
  envelope: valuations.InstrumentEnvelope;
}
/**
 * Unified dependency container for instrument market data requirements.
 */
export interface MarketDependencies {
  /**
   * Credit-index aggregates resolved through `MarketContext::get_credit_index`.
   *
   * These identifiers are distinct from direct hazard-curve IDs because a
   * credit index also carries base correlation and optional issuer curves.
   */
  credit_index_ids?: Id[];
  /**
   * Curve dependencies grouped by type.
   */
  curves: InstrumentCurves;
  /**
   * FX pairs required for pricing (spot matrices).
   */
  fx_pairs: FxPair[];
  /**
   * Scalar market-value identifiers resolved through `MarketContext::get_price`.
   *
   * This includes tradable spots and non-price unitless scalars such as
   * continuous dividend yields. [`Self::series_ids`] is reserved for
   * `MarketContext::get_series` dependencies.
   */
  market_scalar_ids: string[];
  /**
   * Scalar time series identifiers (e.g., OHLC price series for realized variance).
   */
  series_ids: string[];
  /**
   * Typed volatility dependencies in deterministic insertion order.
   */
  volatility_dependencies: VolatilityDependency[];
}
/**
 * Collection of curves used by an instrument, categorized by market role.
 */
export interface InstrumentCurves {
  /**
   * Credit/hazard curves used by the instrument.
   */
  credit_curves: Id[];
  /**
   * Discount curves used by the instrument (including primary and foreign).
   */
  discount_curves: Id[];
  /**
   * Forward/projection curves used by the instrument.
   */
  forward_curves: Id[];
  /**
   * Inflation curves or published inflation indices used by the instrument.
   */
  inflation_curves: Id[];
}
/**
 * A volatility-surface dependency with the context needed for diagnostics.
 */
export interface VolatilityDependency {
  /**
   * Optional contractual strike used by local volatility diagnostics.
   */
  reference_strike?: number | null;
  /**
   * Optional market-scalar id of the underlying spot paired with the surface.
   */
  spot_id?: Id | null;
  /**
   * Volatility surface identifier.
   */
  vol_surface_id: Id;
}
/**
 * Result of geometrically linking sub-period returns.
 */
export interface LinkedReturn {
  /**
   * Annualised return assuming the supplied `horizon_years` covers
   * the full sequence of periods.
   */
  annualised: number;
  /**
   * Cumulative return over the full horizon: `Π(1 + r_i) − 1`.
   */
  cumulative: number;
  /**
   * Number of sub-periods linked.
   */
  num_periods: number;
  [k: string]: unknown;
}
/**
 * Nanoseconds spent in each sequential materialization phase.
 *
 * Native values retain `std::time::Instant` precision. On WASM, values are
 * derived from the host's monotonic `performance.now()` clock and rounded to
 * nanoseconds, so their effective precision remains host-defined and a phase
 * shorter than the host clock resolution may legitimately report zero.
 */
export interface MaterializationPhases {
  /**
   * Ordered runtime position construction time.
   */
  build_positions: number;
  /**
   * Unique instrument decoding and dependency extraction time.
   */
  decode_instruments: number;
  /**
   * Portfolio index rebuilding and final invariant validation time.
   */
  index_build: number;
  /**
   * Outer bundle parsing time.
   */
  parse: number;
  /**
   * Post-parse count-limit, contract, reference, dependency-shape, and
   * content-hash validation time.
   */
  validate_versions: number;
}
/**
 * Outcome metadata for one successful portfolio materialization.
 */
export interface MaterializationReport {
  /**
   * Number of unique artifacts served from the shared decode cache.
   */
  cache_hits: number;
  /**
   * Sum of normalized market-dependency keys over unique artifacts.
   */
  dependencies: number;
  /**
   * Number of encoded bytes in the input bundle.
   */
  input_bytes: number;
  /**
   * Sequential phase timings in nanoseconds.
   */
  phase_nanos: MaterializationPhases;
  /**
   * Number of ordered positions materialized.
   */
  positions: number;
  /**
   * Bounded non-fatal diagnostics retained during loading.
   */
  report: ValidationReport;
  /**
   * Whether the host supplied a monotonic clock for every phase.
   *
   * When false, unavailable phase values are represented as zero in
   * `phase_nanos` and must not be interpreted as measured durations.
   */
  timing_available: boolean;
  /**
   * Number of unique instrument artifacts in the input bundle.
   */
  unique_instruments: number;
}
/**
 * Bounded collection of diagnostics produced by a validation operation.
 */
export interface ValidationReport {
  /**
   * Individual findings retained up to the configured diagnostic limit.
   */
  diagnostics: Diagnostic[];
  /**
   * Whether one or more additional findings were omitted due to the limit.
   *
   * A truncated report with no retained diagnostics is treated as fatal
   * because the wire format cannot distinguish omitted warnings from errors.
   */
  truncated: boolean;
}
export interface PercentageMaterializedPositionWire {
  /**
   * Identifier of the compiled artifact the instrument was resolved from.
   */
  artifact_id: string;
  /**
   * Typed classification tags used for grouping and scenario selection.
   */
  attributes?: {
    [k: string]: AttributeValue;
  };
  /**
   * Legal entity or book the position belongs to, used for rollups.
   */
  entity_id: string;
  /**
   * Position identifier, unique within the materialized portfolio.
   */
  id: string;
  /**
   * Identifier of the instrument this position holds.
   */
  instrument_id: string;
  /**
   * Free-form metadata carried through unchanged; not interpreted here.
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Holding size expressed as a percentage of the referenced notional.
   */
  quantity: PercentageQuantityWire;
  /**
   * Unit discriminator; always `percentage` for this variant.
   */
  unit: PercentagePositionUnitWire;
}
export interface NonPercentageMaterializedPositionWire {
  /**
   * Identifier of the compiled artifact the instrument was resolved from.
   */
  artifact_id: string;
  /**
   * Typed classification tags used for grouping and scenario selection.
   */
  attributes?: {
    [k: string]: AttributeValue;
  };
  /**
   * Legal entity or book the position belongs to, used for rollups.
   */
  entity_id: string;
  /**
   * Position identifier, unique within the materialized portfolio.
   */
  id: string;
  /**
   * Identifier of the instrument this position holds.
   */
  instrument_id: string;
  /**
   * Free-form metadata carried through unchanged; not interpreted here.
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Holding size in the unit named by `unit`. Negative values are short
   * positions.
   */
  quantity: number;
  /**
   * Unit `quantity` is expressed in, such as notional, shares, or contracts.
   */
  unit: NonPercentagePositionUnitWire;
}
/**
 * Producer and compiler version stamps for reproducibility.
 */
export interface MaterializerInfo {
  /**
   * Optional version of the artifact compiler used by the producer.
   */
  compiler_version?: string | null;
  /**
   * Finstack version targeted by the compiled artifacts.
   */
  finstack_version: string;
  /**
   * Stable producer implementation name.
   */
  producer: string;
  /**
   * Version of the producer that assembled the bundle.
   */
  producer_version: string;
}
/**
 * Detailed attribution for model-specific parameters.
 *
 * Extensible structure for instrument-specific model parameters
 * (prepayment speeds, default rates, recovery rates, etc.).
 */
export interface ModelParamsAttribution {
  /**
   * Conversion ratio changes (for convertible bonds).
   */
  conversion_ratio?: Money | null;
  /**
   * Default rate changes (for structured credit).
   */
  default_rate?: Money | null;
  /**
   * Other model-specific parameters.
   */
  other?: {
    [k: string]: Money;
  };
  /**
   * Prepayment speed changes (for MBS/ABS).
   */
  prepayment?: Money | null;
  /**
   * Recovery rate changes (for credit instruments).
   */
  recovery_rate?: Money | null;
  [k: string]: unknown;
}
export interface NettingSetMargin {
  as_of: DateWire;
  csa_id?: string | null;
  im_breakdown: {
    [k: string]: Money;
  };
  im_methodology: ImMethodology;
  initial_margin: Money;
  is_approximate: boolean;
  netting_set_id: NettingSetId;
  position_count: number;
  sensitivities?: SimmSensitivitiesJson | null;
  total_margin: Money;
  variation_margin: Money;
  [k: string]: unknown;
}
/**
 * JSON-friendly representation of [`SimmSensitivities`].
 *
 * Tuple-keyed maps cannot be represented directly as JSON object keys, so this
 * DTO stores each bucket as an array of tuples. It is the canonical JSON shape
 * used by language bindings and examples.
 */
export interface SimmSensitivitiesJson {
  /**
   * Base currency for the sensitivities.
   */
  base_currency: Currency;
  /**
   * Commodity delta buckets as `(bucket, amount)`.
   */
  commodity_delta?: [unknown, unknown][];
  /**
   * Commodity vega buckets as `(bucket, amount)`.
   */
  commodity_vega?: [unknown, unknown][];
  /**
   * Credit non-qualifying delta buckets as `(name, tenor, amount)`.
   */
  credit_non_qualifying_delta?: [unknown, unknown, unknown][];
  /**
   * Credit non-qualifying vega buckets as `(name, tenor, amount)`.
   */
  credit_non_qualifying_vega?: [unknown, unknown, unknown][];
  /**
   * Credit qualifying deltas as `(sector, name, tenor, amount)`.
   */
  credit_qualifying_delta?: [unknown, unknown, unknown, unknown][];
  /**
   * Credit qualifying vegas as `(sector, name, tenor, amount)`.
   */
  credit_qualifying_vega?: [unknown, unknown, unknown, unknown][];
  /**
   * Expiry-resolved volatility-weighted vega inputs before curvature scaling.
   */
  curvature?: SimmCurvatureSensitivity[];
  /**
   * Equity delta buckets as `(underlier, amount)`.
   */
  equity_delta?: [unknown, unknown][];
  /**
   * Equity vega buckets as `(underlier, amount)`.
   */
  equity_vega?: [unknown, unknown][];
  /**
   * FX delta buckets as `(currency, amount)`.
   */
  fx_delta?: [unknown, unknown][];
  /**
   * FX vega buckets as `(ccy1, ccy2, amount)`.
   */
  fx_vega?: [unknown, unknown, unknown][];
  /**
   * Interest-rate delta buckets as `(currency, tenor, amount)`.
   */
  ir_delta?: [unknown, unknown, unknown][];
  /**
   * Interest-rate vega buckets as `(currency, tenor, amount)`.
   */
  ir_vega?: [unknown, unknown, unknown][];
}
/**
 * One volatility-weighted vega input before SIMM curvature scaling.
 *
 * The amount is `sigma * dPV/dsigma` in the sensitivity container's base
 * currency, before HVR, vega risk weights or concentration. For equity, FX
 * and commodity, `sigma` is the paragraph 10(b) prescribed volatility proxy;
 * for rates and credit it is the matching quoted ATM volatility. Preserve
 * separate expiries until `SF(t) = 0.5 * min(1, 14/t_days)` has been applied.
 */
export interface SimmCurvatureSensitivity {
  /**
   * Currency code for IR, credit sector for qualifying credit, numeric
   * commodity bucket, `fx` for FX, or `residual` for equity/non-qualifying credit.
   */
  bucket: string;
  /**
   * Option-expiry tenor from [`SIMM_TENORS`]. Two weeks means 14 calendar
   * days; months use 365/12 days and years use 365 days.
   */
  expiry_tenor: string;
  /**
   * IR subcurve name, credit issuer, equity underlier, commodity name,
   * or an FX currency pair such as `EUR/USD`; must be nonempty.
   */
  factor: string;
  /**
   * SIMM risk class selecting the prescribed bucket and correlation rules.
   */
  risk_class: SimmRiskClass;
  /**
   * IR or credit risk-factor tenor. Required for IR and credit; absent for
   * equity, commodity and FX. This is distinct from the option expiry.
   */
  risk_tenor?: string | null;
  /**
   * Signed base-currency `sigma * dPV/dsigma` before SF, HVR, VRW or concentration.
   */
  volatility_weighted_vega: number;
}
/**
 * Complete P&L attribution result for a single instrument.
 *
 * Decomposes total P&L into constituent factors with optional detailed
 * breakdowns by curve, tenor, FX pair, etc.
 */
export interface PnlAttribution {
  /**
   * Carry P&L (theta + accruals).
   */
  carry: Money;
  /**
   * Detailed carry decomposition (theta + roll-down).
   */
  carry_detail?: CarryDetail | null;
  /**
   * Detailed correlations attribution (by curve).
   */
  correlations_detail?: CorrelationsAttribution | null;
  /**
   * Base correlation curves P&L.
   */
  correlations_pnl: Money;
  /**
   * Optional credit-factor-hierarchy decomposition of carry.
   *
   * Populated only when an `AttributionSpec.credit_factor_model` was
   * supplied.
   */
  credit_carry_decomposition?: CreditCarryDecomposition | null;
  /**
   * Credit hazard curves P&L.
   */
  credit_curves_pnl: Money;
  /**
   * Detailed credit curves attribution (by curve and tenor).
   */
  credit_detail?: CreditCurvesAttribution | null;
  /**
   * Optional credit-factor-hierarchy decomposition of `credit_curves_pnl`.
   *
   * Populated only when an `AttributionSpec.credit_factor_model` was supplied
   * to the attribution call. When present, this is purely additive detail —
   * `credit_curves_pnl` itself is unchanged.
   */
  credit_factor_detail?: CreditFactorAttribution | null;
  /**
   * Detailed cross-factor attribution (by factor-pair label).
   */
  cross_factor_detail?: CrossFactorDetail | null;
  /**
   * Cross-factor interaction P&L (rates×credit, spot×vol, FX×rates, etc.).
   *
   * Stored as the **additive contribution to the attributed sum**: for the
   * parallel method this is the negated mixed second difference
   * `V(a@T₀) + V(b@T₀) − V(all-T₁) − V(ab@T₀)` summed over factor pairs,
   * so that extracting cross terms drives the residual toward zero; for
   * the metrics-based method it is the Taylor cross-gamma term, which is
   * additive by construction. A positive value means factor co-movement
   * added P&L beyond the sum of the isolated factor effects.
   */
  cross_factor_pnl: Money;
  /**
   * Detailed FX attribution (by currency pair).
   */
  fx_detail?: FxAttribution | null;
  /**
   * FX rate changes P&L — the **pricing-impact** component of FX moves on
   * cross-currency instruments (the FX matrix feeding into the instrument's
   * own pricer). For pure single-currency instruments this is zero.
   */
  fx_pnl: Money;
  /**
   * FX translation P&L — the **reporting-currency** component of FX moves.
   *
   * Populated only when the attribution call passed an explicit
   * `target_currency` that differs from the instrument's native pricing currency
   * (`val_t1.currency()`). In that case the native-currency P&L is
   * translated into `target_currency` using `market_t0`'s FX at T₀ and
   * `market_t1`'s FX at T₁; the difference between those two translated
   * totals lands here.
   *
   * For attribution calls that report in native currency (the default —
   * `target_currency = None`), this is always zero in the `total_pnl` currency.
   *
   * The field is always constructed in the same currency as `total_pnl`.
   */
  fx_translation_pnl: Money;
  /**
   * Inflation curves P&L.
   */
  inflation_curves_pnl: Money;
  /**
   * Detailed inflation curves attribution (by curve, optional tenor).
   */
  inflation_detail?: InflationCurvesAttribution | null;
  /**
   * Pure mark-to-market change: `val_t1 − val_t0` with **no** intra-period
   * cashflow adjustment.
   *
   * Separates the raw user-input price change from
   * the total-return view stamped on `total_pnl`. When the attribution path
   * added coupon_income to `total_pnl` (the standard total-return convention
   * in parallel / waterfall / taylor attribution), this field still reports
   * the raw `val_t1 − val_t0` so a downstream consumer that computed their
   * own total from the underlying valuations can reconcile cleanly.
   *
   * Attribution paths that cannot provide the raw mark-to-market change use
   * `None`.
   */
  mark_to_market_pnl?: Money | null;
  /**
   * Market scalars P&L.
   */
  market_scalars_pnl: Money;
  /**
   * Attribution metadata.
   */
  meta: AttributionMeta;
  /**
   * Detailed model parameters attribution.
   */
  model_params_detail?: ModelParamsAttribution | null;
  /**
   * Model parameters P&L.
   */
  model_params_pnl: Money;
  /**
   * Interest rate curves P&L.
   */
  rates_curves_pnl: Money;
  /**
   * Detailed rates curves attribution (by curve and tenor).
   */
  rates_detail?: RatesCurvesAttribution | null;
  /**
   * Residual P&L (total - sum of attributed factors).
   */
  residual: Money;
  /**
   * True if residual computation encountered non-finite (NaN/Inf) inputs,
   * or if any factor P&L value is non-finite. When `true`, `residual` and
   * `meta.residual_pct` are not meaningful and downstream callers should
   * treat the attribution as invalid.
   */
  result_invalid: boolean;
  /**
   * Detailed market scalars attribution.
   */
  scalars_detail?: ScalarsAttribution | null;
  /**
   * Total P&L *as reported by this attribution*.
   *
   * In the standard total-return convention (the default carry path uses
   * the internal total-return carry helper), this is
   * `(val_t1 − val_t0) + coupon_income_between(T0, T1)`. Cashflows received
   * during the period are added back so that
   * `total_pnl == carry + factor_sum + residual` holds: the `carry` field
   * includes the coupon income, and reconciliation against the user's
   * observed val_t1 − val_t0 must subtract the coupon_income out of
   * `total_pnl` to recover the pure mark-to-market move.
   *
   * **For a raw mark-to-market view that excludes intra-period cashflows,
   * read [`Self::mark_to_market_pnl`].** That field, when present, is the
   * untouched `val_t1 − val_t0` and never absorbs coupon income.
   */
  total_pnl: Money;
  /**
   * Detailed volatility attribution (by surface).
   */
  vol_detail?: VolAttribution | null;
  /**
   * Implied volatility changes P&L.
   */
  vol_pnl: Money;
  [k: string]: unknown;
}
/**
 * Detailed attribution for interest rate curves.
 *
 * Provides aggregate and per-curve/per-tenor breakdown for discount
 * and forward curves.
 */
export interface RatesCurvesAttribution {
  /**
   * P&L by curve ID.
   */
  by_curve: {
    [k: string]: Money;
  };
  /**
   * P&L by (curve_id, tenor), serialized with `"{curve_id}|{tenor}"` keys.
   */
  by_tenor?: {
    [k: string]: Money;
  };
  /**
   * Total discount curves P&L.
   */
  discount_total: Money;
  /**
   * Total forward curves P&L.
   */
  forward_total: Money;
  [k: string]: unknown;
}
/**
 * Detailed attribution for market scalars.
 *
 * Includes dividends, equity/commodity prices, inflation indices, etc.
 */
export interface ScalarsAttribution {
  /**
   * Commodity price changes.
   */
  commodity_prices?: {
    [k: string]: Money;
  };
  /**
   * Dividend changes by equity ID.
   */
  dividends?: {
    [k: string]: Money;
  };
  /**
   * Equity price changes.
   */
  equity_prices?: {
    [k: string]: Money;
  };
  /**
   * Inflation index changes.
   */
  inflation?: {
    [k: string]: Money;
  };
  [k: string]: unknown;
}
/**
 * Detailed attribution for implied volatility changes.
 *
 * Provides per-surface breakdown.
 */
export interface VolAttribution {
  /**
   * P&L by volatility surface ID.
   */
  by_surface: {
    [k: string]: Money;
  };
  [k: string]: unknown;
}
/**
 * Portfolio-level P&L attribution result.
 *
 * Aggregates P&L attribution across all positions with currency conversion
 * to portfolio base currency.
 *
 * # FX Translation Effects
 *
 * For positions denominated in currencies other than the portfolio's base currency,
 * the attribution includes FX translation effects. The `total_pnl` field represents
 * the **all-in P&L** including both:
 *
 * 1. **Instrument-level P&L** converted to base currency at T₁ FX rates
 * 2. **FX translation P&L** from the revaluation of opening principal
 *
 * The decomposition is:
 *
 * ```text
 * total_pnl = sum(factor_pnl_at_T1_FX) + fx_translation_pnl + residual
 * ```
 *
 * Where each factor bucket (carry, rates, credit, vol, etc.) is converted to
 * base currency using the T₁ FX rate. This means the implicit FX translation
 * of the P&L *flow* (i.e., `PnL_native × (FX_T1 - FX_T0)`) is absorbed into
 * each factor bucket rather than isolated in `fx_translation_pnl`.
 *
 * `fx_translation_pnl` captures **only** the revaluation of the opening
 * principal:
 *
 * ```text
 * fx_translation_pnl = Val_T0_native × (FX_T1 - FX_T0)
 * ```
 *
 * This convention is consistent with systems that convert factor P&L at
 * closing rates and report principal revaluation separately.
 *
 * # Note on by_position Attribution
 *
 * The `by_position` map contains instrument-currency attribution before FX
 * translation effects are applied. To reconcile with `total_pnl`, apply the
 * FX rates and add the principal revaluation effect.
 *
 * # Conventions
 *
 * The portfolio-level aggregates are reported in portfolio base currency,
 * while `by_position` remains in each instrument's native currency so callers
 * can inspect raw instrument attribution before FX translation.
 */
export interface PortfolioAttribution {
  /**
   * Attribution by position in instrument-native currency.
   *
   * Note: These values are in each instrument's native currency and do not
   * include FX translation effects. Use the portfolio-level aggregates for
   * base-currency totals.
   */
  by_position: {
    [k: string]: PnlAttribution;
  };
  /**
   * Carry P&L (theta + accruals) in base currency.
   */
  carry: Money;
  /**
   * Aggregate correlations detail (optional).
   */
  correlations_detail?: CorrelationsAttribution | null;
  /**
   * Base correlation curves P&L in base currency.
   */
  correlations_pnl: Money;
  /**
   * Credit hazard curves P&L in base currency.
   */
  credit_curves_pnl: Money;
  /**
   * Aggregate credit curves detail (optional).
   */
  credit_detail?: CreditCurvesAttribution | null;
  /**
   * Cross-factor interaction P&L in base currency.
   *
   * Aggregated from each position's native-currency `cross_factor_pnl`
   * after conversion to portfolio base currency.
   */
  cross_factor_pnl: Money;
  /**
   * Aggregate FX detail (optional).
   */
  fx_detail?: FxAttribution | null;
  /**
   * FX rate changes P&L in base currency.
   *
   * This captures FX exposure within instruments (e.g., cross-currency swaps),
   * not the translation effect from converting instrument P&L to base currency.
   */
  fx_pnl: Money;
  /**
   * FX translation P&L from revaluing opening principal to base currency.
   *
   * For cross-currency positions, this captures the effect of FX rate changes
   * on the T₀ position value:
   *
   * ```text
   * fx_translation_pnl = Val_T0_native × (FX_T1 - FX_T0)
   * ```
   *
   * Note: The implicit FX translation of each factor's P&L flow
   * (converting native-currency factor P&L at T₁ FX rather than T₀ FX) is
   * absorbed into the respective factor buckets (carry, rates, etc.) and is
   * **not** included here.
   *
   * This is separate from `fx_pnl` which captures FX exposure within instruments.
   */
  fx_translation_pnl: Money;
  /**
   * Inflation curves P&L in base currency.
   */
  inflation_curves_pnl: Money;
  /**
   * Aggregate inflation curves detail (optional).
   */
  inflation_detail?: InflationCurvesAttribution | null;
  /**
   * Market scalars P&L in base currency.
   */
  market_scalars_pnl: Money;
  /**
   * Model parameters P&L in base currency.
   */
  model_params_pnl: Money;
  /**
   * Interest rate curves P&L in base currency.
   */
  rates_curves_pnl: Money;
  /**
   * Aggregate rates curves detail (optional).
   */
  rates_detail?: RatesCurvesAttribution | null;
  /**
   * Residual P&L (unexplained) in base currency.
   */
  residual: Money;
  /**
   * True if any constituent position's attribution was flagged invalid
   * (for example, a non-finite factor sensitivity — see
   * [`PnlAttribution::result_invalid`]). When `true`, the portfolio
   * aggregates and [`PortfolioAttribution::reconciliation_check`] are not
   * trustworthy and must not be relied on for reporting.
   */
  result_invalid: boolean;
  /**
   * Aggregate scalars detail (optional).
   */
  scalars_detail?: ScalarsAttribution | null;
  /**
   * Total portfolio P&L in base currency.
   *
   * This is the **all-in P&L** that includes:
   * - All factor attributions converted to base currency at T₁ rates
   * - FX translation effects from opening principal revaluation
   *
   * Note: This differs from a simple sum of factor attributions because
   * cross-currency positions include FX translation P&L on the principal.
   */
  total_pnl: Money;
  /**
   * Aggregate volatility detail (optional).
   */
  vol_detail?: VolAttribution | null;
  /**
   * Implied volatility changes P&L in base currency.
   */
  vol_pnl: Money;
}
/**
 * One scaled portfolio cashflow event derived from an instrument schedule.
 */
export interface PortfolioCashflowEvent {
  /**
   * Accrual factor used to compute the event when available.
   */
  accrual_factor: number;
  /**
   * Position-scaled amount.
   */
  amount: Money;
  /**
   * Payment date.
   */
  date: DateWire;
  /**
   * Underlying instrument identifier.
   */
  instrument_id: string;
  /**
   * Underlying instrument type key.
   */
  instrument_type: InstrumentType;
  /**
   * Cashflow classification preserved from the instrument schedule.
   */
  kind: CFKind;
  /**
   * Position contributing the event.
   */
  position_id: PositionId;
  /**
   * Effective rate used to compute the event when available.
   */
  rate?: number | null;
  /**
   * Optional reset date for floating coupons.
   */
  reset_date?: DateWire | null;
  [k: string]: unknown;
}
/**
 * Per-position cashflow summary, including empty-schedule intent metadata.
 */
export interface PortfolioCashflowPositionSummary {
  /**
   * Number of emitted dated events after schedule construction.
   */
  event_count: number;
  /**
   * Underlying instrument identifier.
   */
  instrument_id: string;
  /**
   * Underlying instrument type key.
   */
  instrument_type: InstrumentType;
  /**
   * Position identifier.
   */
  position_id: PositionId;
  /**
   * Schedule representation carried by the instrument.
   */
  representation: CashflowRepresentation;
  [k: string]: unknown;
}
/**
 * Rich portfolio cashflow ladder preserving event classifications.
 */
export interface PortfolioCashflows {
  /**
   * Aggregated totals by date, currency, and `CFKind`.
   */
  by_date: {
    [k: string]: {
      [k: string]: {
        [k: string]: Money;
      };
    };
  };
  /**
   * Per-position event drill-down keyed by position ID.
   */
  by_position: {
    [k: string]: PortfolioCashflowEvent[];
  };
  /**
   * Scaled cashflow events for all supported positions, sorted by payment date.
   */
  events: PortfolioCashflowEvent[];
  /**
   * FX policy applied by [`Self::collapse_to_base_by_date_kind`].
   *
   * Stamped so callers do not infer a spot-on-payment-date conversion from
   * [`finstack_quant_core::money::fx::FxConversionPolicy::CashflowDate`].
   */
  fx_collapse_policy?: CashflowFxPolicy;
  /**
   * Extraction issues for unsupported instruments and provider failures.
   */
  issues: CashflowExtractionIssue[];
  /**
   * Per-position schedule metadata, including placeholder/no-residual intent.
   */
  position_summaries: {
    [k: string]: PortfolioCashflowPositionSummary;
  };
  [k: string]: unknown;
}
/**
 * Portfolio fields shared by every materialized position.
 */
export interface PortfolioHeader {
  /**
   * Valuation date for the materialized portfolio.
   */
  as_of: DateWire;
  /**
   * Reporting currency used for portfolio aggregation.
   */
  base_currency: Currency;
  /**
   * Optional book hierarchy keyed by stable book IDs.
   */
  books?: {
    [k: string]: Book;
  };
  /**
   * Entities keyed by their stable IDs in deterministic order.
   */
  entities: {
    [k: string]: Entity;
  };
  /**
   * Stable portfolio identifier.
   */
  id: string;
  /**
   * Extension metadata retained as part of the persisted bundle.
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Optional human-readable portfolio name.
   */
  name?: string | null;
  /**
   * Portfolio-level grouping and classification tags.
   */
  tags?: {
    [k: string]: string;
  };
}
/**
 * Portfolio-wide margin calculation results.
 */
export interface PortfolioMarginResult {
  as_of: DateWire;
  base_currency: Currency;
  by_csa: {
    [k: string]: ImCollateralResult;
  };
  degraded_positions: DegradedPosition[];
  netting_sets: NettingSetMargin[];
  positions_without_margin: number;
  total_im_transfer: Money;
  total_initial_margin: Money;
  total_margin: Money;
  total_positions: number;
  total_required_im_collateral: Money;
  total_segregated_im: Money;
  total_variation_margin: Money;
  [k: string]: unknown;
}
/**
 * Strict, versioned, content-addressed portfolio materialization bundle.
 */
export interface PortfolioMaterializationEnvelope {
  /**
   * Unique strict instrument envelopes referenced by positions.
   */
  instruments: InstrumentArtifact[];
  /**
   * Optional producer and compiler version provenance.
   */
  materializer?: MaterializerInfo | null;
  /**
   * Portfolio fields that do not contain runtime instrument trait objects.
   */
  portfolio: PortfolioHeader;
  /**
   * Positions in the order required by the reconstructed portfolio.
   */
  positions: MaterializedPosition[];
  /**
   * Exact materialization contract marker.
   */
  schema: PortfolioMaterializationSchema;
}
/**
 * Complete portfolio metrics results.
 *
 * Holds both aggregated metrics and per-position values returned
 * by `aggregate_metrics`.
 *
 * # Completeness of the aggregated totals
 *
 * A total in [`aggregated`](Self::aggregated) is not necessarily a sum over
 * every position in the portfolio. Three fields make each omission visible,
 * and consumers that report totals should surface all three:
 *
 * - [`degraded_positions`](Self::degraded_positions) — positions that fell
 *   back to a PV-only valuation, so they carry **no** risk measures and
 *   contribute zero to every total.
 * - [`skipped_metrics`](Self::skipped_metrics) — individual non-finite
 *   values excluded from a total.
 * - [`unaggregated_metrics`](Self::unaggregated_metrics) — metrics that
 *   exist per position but are not portfolio-summable, so they have no
 *   total at all.
 *
 * A metric narrowed away because a position's instrument type has no
 * calculator for it is *not* one of these omissions: the model gives that
 * instrument type no such exposure, so the total is complete without it. Such
 * narrowing is reported per position on
 * [`PositionValue::inapplicable_metrics`](crate::valuation::PositionValue::inapplicable_metrics).
 */
export interface PortfolioMetrics {
  /**
   * Aggregated metrics (summable only)
   */
  aggregated: {
    [k: string]: AggregatedMetric;
  };
  /**
   * Raw metrics by position (all metrics), with explicit native currency context.
   */
  by_position: {
    [k: string]: PositionMetrics;
  };
  /**
   * Positions that carried no risk measures at all because their valuation
   * fell back to PV-only.
   *
   * Mirrored from
   * [`PortfolioValuation::degraded_positions`](crate::valuation::PortfolioValuation::degraded_positions)
   * (positions with `risk_metrics_complete == false`). Such positions
   * contribute zero to every total without producing a
   * [`SkippedMetric`] entry — that field only records non-finite values —
   * so this list is the only signal that the aggregate is partial.
   */
  degraded_positions?: PositionId[];
  /**
   * Positions whose non-finite metric values were excluded from aggregation.
   *
   * Each entry records the position and metric that was skipped, allowing
   * callers to detect incomplete aggregation (analogous to
   * [`crate::valuation::PortfolioValuation::degraded_positions`]).
   */
  skipped_metrics?: SkippedMetric[];
  /**
   * Metric identifiers present in [`by_position`](Self::by_position) that
   * were not aggregated into a portfolio total because they are not
   * summable across positions (for example `ytm`, `duration`, or any
   * metric outside the additive allowlist).
   *
   * Sorted and de-duplicated. The per-position values remain available in
   * `by_position` in their native currency.
   */
  unaggregated_metrics?: string[];
  [k: string]: unknown;
}
/**
 * Position-level metrics with explicit native currency context.
 */
export interface PositionMetrics {
  /**
   * Native currency for this position's valuation and non-summable metrics.
   */
  currency: Currency;
  /**
   * Raw metric values for the position.
   */
  metrics: {
    [k: string]: number;
  };
  [k: string]: unknown;
}
/**
 * A metric value that was excluded from portfolio aggregation because it was
 * non-finite (NaN or ±Inf).
 */
export interface SkippedMetric {
  /**
   * Metric identifier that was skipped.
   */
  metric_id: string;
  /**
   * Position that produced the non-finite value.
   */
  position_id: PositionId;
  /**
   * The non-finite value that was encountered.
   */
  value: number;
  [k: string]: unknown;
}
/**
 * Canonical typed v1 output from portfolio optimization.
 */
export interface PortfolioOptimizationResultWire {
  /**
   * Approximately binding constraint names.
   */
  binding_constraints: string[];
  /**
   * Constraint slack values.
   */
  constraint_slacks: {
    [k: string]: number;
  };
  /**
   * Current weights keyed by position.
   */
  current_weights: {
    [k: string]: number;
  };
  /**
   * Implied target quantities keyed by position.
   */
  implied_quantities: {
    [k: string]: number;
  };
  /**
   * Whether the solution can be consumed.
   */
  is_feasible: boolean;
  /**
   * Optional user-supplied problem label.
   */
  label?: string | null;
  /**
   * Evaluated metrics.
   */
  metric_values: {
    [k: string]: number;
  };
  /**
   * Objective value at the solution.
   */
  objective_value: number;
  /**
   * Optimal weights keyed by position.
   */
  optimal_weights: {
    [k: string]: number;
  };
  /**
   * Required numeric v1 marker.
   */
  schema_version: SchemaVersion;
  /**
   * Solver outcome.
   */
  status: OptimizationStatus;
  /**
   * Stable snake-case status label.
   */
  status_label: string;
  /**
   * Executable trade list.
   */
  trades: TradeSpec[];
  /**
   * Gross turnover.
   */
  turnover: number;
  /**
   * Weight changes keyed by position.
   */
  weight_deltas: {
    [k: string]: number;
  };
}
/**
 * Trade specification for a single position.
 */
export interface TradeSpec {
  /**
   * Pre‑trade quantity.
   */
  current_quantity: number;
  /**
   * Pre‑trade weight.
   */
  current_weight: number;
  /**
   * Quantity change (`target - current`).
   */
  delta_quantity: number;
  /**
   * Buy / Sell / Hold classification.
   */
  direction: TradeDirection;
  /**
   * Underlying instrument identifier (or candidate id).
   */
  instrument_id: string;
  /**
   * Position identifier in the optimized portfolio.
   */
  position_id: PositionId;
  /**
   * Post‑trade quantity.
   */
  target_quantity: number;
  /**
   * Post‑trade weight.
   */
  target_weight: number;
  /**
   * Trade type (existing, new position, close‑out).
   */
  trade_type: TradeType;
}
/**
 * JSON-serializable specification for a portfolio optimization problem.
 */
export interface PortfolioOptimizationSpec {
  /**
   * Constraints on the optimized portfolio. An explicit budget replaces the
   * default sum-of-weights budget of one; duplicate explicit budgets are rejected.
   */
  constraints?: Constraint[];
  /**
   * Optional label for auditability.
   */
  label?: string | null;
  /**
   * Policy for handling positions missing required metrics.
   */
  missing_metric_policy?: MissingMetricPolicy;
  /**
   * Optimization objective.
   */
  objective: Objective;
  /**
   * Portfolio specification (same format as `value_portfolio`).
   */
  portfolio: PortfolioSpec;
  /**
   * Optional trade universe (tradeable/held filters and candidate
   * additions). `None` means every existing position is tradeable and no
   * candidates are considered.
   */
  trade_universe?: TradeUniverse | null;
  /**
   * How weights are defined.
   */
  weighting?: WeightingScheme;
  [k: string]: unknown;
}
/**
 * Serializable portfolio specification.
 *
 * This struct allows portfolios to be serialized and deserialized by storing
 * positions as `PositionSpec` rather than `Position` (which contains non-serializable
 * `Arc<dyn Instrument>`).
 */
export interface PortfolioSpec {
  /**
   * Valuation date
   */
  as_of: DateWire;
  /**
   * Base currency for aggregation
   */
  base_currency: Currency;
  /**
   * Optional hierarchical book organization
   */
  books?: {
    [k: string]: Book;
  };
  /**
   * Entities that own positions
   */
  entities: {
    [k: string]: Entity;
  };
  /**
   * Portfolio identifier
   */
  id: string;
  /**
   * Additional metadata
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Human-readable name
   */
  name?: string | null;
  /**
   * Positions as serializable specs
   */
  positions: PositionSpec[];
  /**
   * Portfolio-level tags
   */
  tags?: {
    [k: string]: string;
  };
  [k: string]: unknown;
}
/**
 * Serializable position specification (without `Arc<dyn Instrument>`).
 *
 * This struct allows positions to be serialized and deserialized by storing
 * the instrument definition as JSON rather than a trait object.
 */
export interface PositionSpec {
  /**
   * Position-level attributes for grouping, filtering, and constraints
   */
  attributes?: {
    [k: string]: AttributeValue;
  };
  /**
   * Optional book identifier
   */
  book_id?: BookId | null;
  /**
   * Entity identifier
   */
  entity_id: EntityId;
  /**
   * Instrument identifier (for reference/lookup)
   */
  instrument_id: string;
  /**
   * Instrument definition for full serialization (optional)
   *
   * If `None`, the position can still be serialized but cannot be
   * reconstructed without an external instrument registry.
   */
  instrument_spec?: valuations.InstrumentJson | null;
  /**
   * Additional metadata
   */
  meta?: {
    [k: string]: unknown;
  };
  /**
   * Position identifier
   */
  position_id: PositionId;
  /**
   * Signed quantity. For [`PositionUnit::Notional`], a lot multiplier.
   */
  quantity: number;
  /**
   * Unit of measurement
   */
  unit: PositionUnit;
  [k: string]: unknown;
}
/**
 * Defines which instruments the optimizer can trade.
 *
 * The trade universe consists of:
 *
 * 1. **Tradeable positions**: existing portfolio positions that can be adjusted
 * 2. **Held positions**: existing positions locked at current weight
 * 3. **Candidate positions**: new instruments that could be added
 *
 * Serializes with serde; candidate instruments travel as their canonical
 * tagged JSON payload (see [`CandidatePosition`]).
 */
export interface TradeUniverse {
  /**
   * Whether candidates can receive negative weights (short selling).
   * Default: false (long‑only for new positions).
   */
  allow_short_candidates?: boolean;
  /**
   * Candidate instruments not currently in the portfolio.
   * These start with weight 0 and can be added by the optimizer.
   */
  candidates?: CandidatePosition[];
  /**
   * Filter for existing positions that are held constant.
   * Positions matching this filter keep their current weight.
   * Takes precedence over `tradeable_filter` if both match.
   */
  held_filter?: PositionFilter | null;
  /**
   * Filter for existing positions that can be traded.
   * Positions matching this filter have their weights optimized.
   * Default: all positions are tradeable.
   */
  tradeable_filter?: PositionFilter;
}
/**
 * Net and gross portfolio exposure for one primitive instrument identifier.
 */
export interface PortfolioPrimitiveAggregate {
  /**
   * Sum of absolute additive risk by metric.
   */
  gross_measures: {
    [k: string]: number;
  };
  /**
   * Sum of absolute primitive path quantities.
   */
  gross_quantity: number;
  /**
   * Sum of absolute primitive path values in portfolio base currency.
   */
  gross_value: Money;
  /**
   * Primitive instrument identifier.
   */
  instrument_id: Id;
  /**
   * Canonical primitive instrument type discriminator.
   */
  instrument_type: string;
  /**
   * Algebraic additive risk by metric.
   */
  net_measures: {
    [k: string]: number;
  };
  /**
   * Algebraic primitive quantity across all positions and paths.
   */
  net_quantity: number;
  /**
   * Algebraic primitive value in portfolio base currency.
   */
  net_value: Money;
}
/**
 * Portfolio primitive decomposition retaining both path and concentration views.
 */
export interface PortfolioPrimitiveExposureReport {
  /**
   * Net and gross aggregates ordered by primitive identifier.
   */
  aggregates: PortfolioPrimitiveAggregate[];
  /**
   * Portfolio reporting currency used for every value and risk amount.
   */
  base_currency: Currency;
  /**
   * Position-aware primitive paths before overlap netting.
   */
  paths: PortfolioPrimitivePath[];
}
/**
 * One primitive exposure path traced back to its owning portfolio position.
 */
export interface PortfolioPrimitivePath {
  /**
   * Primitive instrument identifier.
   */
  instrument_id: Id;
  /**
   * Canonical primitive instrument type discriminator.
   */
  instrument_type: string;
  /**
   * Additive primitive risk measures in portfolio base currency.
   */
  measures: {
    [k: string]: number;
  };
  /**
   * Composite and leg identifiers ending at the primitive instrument.
   */
  path: string[];
  /**
   * Portfolio position containing the direct instrument or root composite.
   */
  position_id: PositionId;
  /**
   * Signed primitive quantity after position and nested-leg scaling.
   */
  quantity: number;
  /**
   * Signed primitive value in portfolio base currency.
   */
  value: Money;
}
/**
 * Complete results from portfolio evaluation.
 */
export interface PortfolioResult {
  /**
   * Metadata about the calculation
   */
  meta: ResultsMeta;
  /**
   * Aggregated metrics
   */
  metrics: PortfolioMetrics;
  /**
   * Required wire-format schema version. Only numeric `1` is accepted.
   */
  schema_version: SchemaVersion;
  /**
   * Portfolio valuation results
   */
  valuation: PortfolioValuation;
  [k: string]: unknown;
}
/**
 * Complete portfolio valuation results.
 *
 * Provides per-position valuations, totals by entity, and the grand total.
 */
export interface PortfolioValuation {
  /**
   * Valuation date carried through from the portfolio.
   */
  as_of: DateWire;
  /**
   * Aggregated values by entity
   */
  by_entity: {
    [k: string]: Money;
  };
  /**
   * Positions whose valuation fell back to PV-only because requested risk
   * metrics could not be computed.
   */
  degraded_positions?: PositionId[];
  /**
   * FX policy applied when collapsing position values to the base currency.
   *
   * Base-currency rollups use an explicit spot-equivalent conversion at
   * [`as_of`](Self::as_of) through the market FX matrix; this records the
   * applied [`FxConversionPolicy`] so the result envelope satisfies the
   * policy-visibility invariant (the FX strategy is stamped, not implied).
   */
  fx_collapse_policy?: FxConversionPolicy;
  /**
   * Values for each position
   */
  position_values: {
    [k: string]: PositionValue;
  };
  /**
   * Total portfolio value in base currency
   */
  total_base_currency: Money;
  [k: string]: unknown;
}
/**
 * Result of valuing a single position.
 *
 * Holds both native-currency and base-currency valuations along with
 * the underlying [`ValuationResult`].
 */
export interface PositionValue {
  /**
   * Entity that owns this position
   */
  entity_id: EntityId;
  /**
   * Metrics on the portfolio menu that this position's instrument type has
   * no calculator for, and which were therefore never requested of it.
   *
   * A portfolio metric list is a menu offered to a heterogeneous book (see
   * [`RequestedMetrics`]), so this records the per-position narrowing that
   * makes such a list usable. It is not a failure report: unlike
   * [`PortfolioValuation::degraded_positions`] and
   * [`PortfolioMetrics::skipped_metrics`](crate::metrics::PortfolioMetrics::skipped_metrics),
   * nothing here could have been computed and was not. An entry means the
   * model gives this instrument type no such exposure, so the position
   * contributes nothing to that metric's portfolio total by construction.
   *
   * Listed in menu order. Empty for a position that supports the whole menu.
   */
  inapplicable_metrics?: MetricId[];
  /**
   * Linear scaling factor to apply to summable risk measures.
   *
   * This mirrors the economic position size and sign used for PV scaling,
   * but is kept separate so non-summable metrics such as YTM remain
   * unscaled at the position drill-down level.
   */
  metric_scale: number;
  /**
   * Position identifier
   */
  position_id: PositionId;
  /**
   * Original metrics failure message when the valuation fell back to PV-only.
   */
  risk_error?: string | null;
  /**
   * Whether every risk metric requested *of this position* was computed
   * successfully.
   *
   * The requested set is the portfolio metric menu narrowed to what this
   * position's instrument type supports, so structural narrowing does not
   * clear this flag; the narrowed-away entries are listed on
   * [`inapplicable_metrics`](Self::inapplicable_metrics). `false` means a
   * supported metric failed to compute and the position fell back to
   * PV-only.
   */
  risk_metrics_complete: boolean;
  /**
   * Full valuation result with metrics (including computed risk measures).
   */
  valuation_result?: valuations.ValuationResult | null;
  /**
   * Value converted to portfolio base currency
   */
  value_base: Money;
  /**
   * Value in the instrument's native currency
   */
  value_native: Money;
  [k: string]: unknown;
}
/**
 * Options controlling portfolio valuation behaviour.
 *
 * The default is a risk run: [`strict_risk`](Self::strict_risk) is `true`
 * and [`metrics`](Self::metrics) is [`RequestedMetrics::Standard`]. A
 * failed requested risk metric aborts the valuation. Set `strict_risk` to
 * `false` only when a PV-preserving best-effort fallback is intentional.
 */
export interface PortfolioValuationOptions {
  /**
   * Which metric set to request. See [`RequestedMetrics`].
   */
  metrics?: RequestedMetrics;
  /**
   * When `true` (default), any failure to compute the risk metrics
   * requested of a position causes the entire portfolio valuation to fail.
   *
   * When `false`, the engine falls back to PV-only valuation for that
   * position if metrics fail, preserving aggregate PV but leaving those
   * risk metrics missing (see [`PortfolioValuation::degraded_positions`]).
   *
   * This governs computation failures only. A metric the position's
   * instrument type has no calculator for is never requested of it in the
   * first place (see [`RequestedMetrics`]) and so fails neither mode; it is
   * reported on [`PositionValue::inapplicable_metrics`].
   */
  strict_risk?: boolean;
  [k: string]: unknown;
}
/**
 * Contribution of a single position to a specific factor bucket.
 */
export interface PositionFactorContribution {
  /**
   * Identifier of the contributing factor.
   */
  factor_id: FactorId;
  /**
   * Portfolio position identifier.
   */
  position_id: string;
  /**
   * Risk attributed to this position-factor pair.
   */
  risk_contribution: number;
  [k: string]: unknown;
}
/**
 * Annualized residual variance contributed by a single position.
 *
 * `residual_variance` is reported in variance units (not vol) so that
 * per-position residuals add linearly into a portfolio-level total. Callers
 * that want per-position residual *vol* should take the square root of this
 * field after summing the relevant subset.
 */
export interface PositionResidualContribution {
  /**
   * Portfolio position identifier.
   */
  position_id: string;
  /**
   * Annualized variance contributed by this position's idiosyncratic risk.
   * Always non-negative.
   */
  residual_variance: number;
  /**
   * Where the residual variance came from.
   */
  source: ResidualContributionSource;
  [k: string]: unknown;
}
/**
 * Report from reconciling position-level P&L attribution against portfolio totals.
 */
export interface ReconciliationReport {
  /**
   * Whether the reconciliation passes within tolerance.
   */
  is_reconciled: boolean;
  /**
   * Tolerance used for the check.
   */
  tolerance: number;
  /**
   * Total residual: `total_pnl - (sum of factor buckets + fx_translation_pnl)`.
   */
  total_residual: number;
  [k: string]: unknown;
}
/**
 * One reference (e.g.
 */
export interface ReferenceReturn {
  /**
   * Duration in years at period start. Must be finite and non-negative.
   */
  duration: number;
  /**
   * Realized total return for the period (decimal, e.g. `0.01` = 1%).
   * Must be finite.
   */
  total_return: number;
}
/**
 * Configuration for a replay run.
 */
export interface ReplayConfig {
  /**
   * Attribution method (only used in `FullAttribution` mode).
   */
  attribution_method?: AttributionMethod;
  /**
   * What to compute at each step.
   */
  mode: ReplayMode;
  /**
   * Strict-vs-best-effort handling of per-snapshot failures.
   */
  on_error?: ReplayErrorPolicy;
  /**
   * Valuation options compiled into each replay evaluation profile.
   */
  valuation_options?: PortfolioValuationOptions;
}
/**
 * Full output of a replay run.
 */
export interface ReplayResult {
  /**
   * Snapshots that were skipped because their valuation failed and the
   * run was configured for [`ReplayErrorPolicy::BestEffort`]. Empty in
   * strict mode (the run would have aborted instead).
   */
  skipped_dates?: [unknown, unknown][];
  /**
   * Per-step output.
   */
  steps: ReplayStep[];
  /**
   * Aggregate statistics.
   */
  summary: ReplaySummary;
  [k: string]: unknown;
}
/**
 * Output for a single replay step.
 */
export interface ReplayStep {
  /**
   * Factor attribution between prior step and this step. `None` at step 0
   * and in non-attribution modes.
   */
  attribution?: PortfolioAttribution | null;
  /**
   * Cumulative mark-to-market P&L, excluding paid cashflows (this PV minus initial PV). `None` at step 0.
   */
  cumulative_mtm_pnl?: Money | null;
  /**
   * Daily mark-to-market P&L, excluding paid cashflows (this PV minus prior PV). `None` at step 0.
   */
  daily_mtm_pnl?: Money | null;
  /**
   * Valuation date.
   */
  date: DateWire;
  /**
   * Full portfolio valuation at this date.
   */
  valuation: PortfolioValuation;
  [k: string]: unknown;
}
/**
 * Aggregate statistics across the full replay.
 */
export interface ReplaySummary {
  /**
   * Last date in the timeline.
   */
  end_date: DateWire;
  /**
   * Portfolio value at the last step.
   */
  end_value: Money;
  /**
   * Maximum mark-to-market drawdown from peak to trough, selected on the largest
   * base-currency (dollar) decline from a running high-water mark.
   */
  max_mtm_drawdown: Money;
  /**
   * Maximum percentage mark-to-market drawdown, selected independently of
   * [`max_mtm_drawdown`](Self::max_mtm_drawdown) as the largest `decline / peak`
   * ratio over positive peaks. The dollar-largest and percentage-largest
   * drawdowns can come from different peak/trough pairs (a small early
   * peak can host the deepest relative loss). `0.0` when no positive peak
   * ever existed: a percentage decline from a non-positive portfolio value
   * is not meaningful.
   */
  max_mtm_drawdown_pct: number;
  /**
   * Date of the peak before the maximum percentage-selected drawdown.
   * `None` when no positive peak ever produced a decline.
   */
  max_mtm_drawdown_pct_peak_date?: DateWire | null;
  /**
   * Date of the trough of the maximum percentage-selected drawdown.
   * `None` when no positive peak ever produced a decline.
   */
  max_mtm_drawdown_pct_trough_date?: DateWire | null;
  /**
   * Date of the peak before the maximum (dollar-selected) drawdown.
   */
  max_mtm_drawdown_peak_date: DateWire;
  /**
   * Date of the trough of the maximum (dollar-selected) drawdown.
   */
  max_mtm_drawdown_trough_date: DateWire;
  /**
   * Number of steps (including step 0).
   */
  num_steps: number;
  /**
   * First date in the timeline.
   */
  start_date: DateWire;
  /**
   * Portfolio value at step 0.
   */
  start_value: Money;
  /**
   * Total mark-to-market P&L (end PV minus start PV), excluding paid cashflows.
   */
  total_mtm_pnl: Money;
  [k: string]: unknown;
}
/**
 * Portfolio-level decomposition of total risk across common factors and residuals.
 *
 * # Sign convention
 *
 * The sign of [`RiskDecomposition::total_risk`], [`FactorContribution::absolute_risk`],
 * [`FactorContribution::marginal_risk`], and [`RiskDecomposition::residual_risk`]
 * depends on the selected [`RiskMeasure`]:
 *
 * * [`RiskMeasure::Variance`] and [`RiskMeasure::Volatility`] — non-negative.
 * * [`RiskMeasure::VaR`] and [`RiskMeasure::ExpectedShortfall`] — **non-positive**
 *   (losses reported as negative numbers; see [`RiskMeasure`] for details).
 *
 * [`FactorContribution::relative_risk`] is always a dimensionless share and
 * stays non-negative for a long-risk portfolio because numerator and denominator
 * carry the same sign.
 */
export interface RiskDecomposition {
  /**
   * Aggregate factor-level contributions to portfolio risk.
   */
  factor_contributions: FactorContribution[];
  /**
   * Risk measure used to aggregate and report the decomposition.
   */
  measure: RiskMeasure;
  /**
   * Per-position, per-factor contributions that roll up into the portfolio view.
   */
  position_factor_contributions: PositionFactorContribution[];
  /**
   * Per-position residual (idiosyncratic) variance contributions.
   *
   * Populated only by credit-aware position decomposers that have access to
   * per-issuer idiosyncratic vol estimates. Empty when the decomposer has no
   * position-level residual allocation.
   */
  position_residual_contributions: PositionResidualContribution[];
  /**
   * Unattributed or idiosyncratic risk left after factor aggregation.
   * Same sign convention as `total_risk`.
   */
  residual_risk: number;
  /**
   * Total portfolio risk under the selected `measure`. Sign follows the
   * measure's convention (see struct-level docs).
   */
  total_risk: number;
}
/**
 * Scenario-attributable profit and loss, in the portfolio base currency.
 *
 * Produced by [`scenario_pnl`] as the difference between the stressed and the
 * unstressed [`PortfolioValuation`](crate::valuation::PortfolioValuation).
 * Every amount is a [`Money`] in the portfolio's base currency — the
 * computation never leaves `Money`, so the Decimal rounding contract of
 * [`crate::valuation::value_portfolio`] carries through to the P&L unchanged.
 *
 * # Reconciliation
 *
 * `by_position` sums to `total` in the base currency: both sides are derived
 * from the same per-position `value_base` amounts, so a desk-style
 * "does the drill-down foot to the headline" check passes.
 *
 * # Ordering
 *
 * `by_position` is deterministically ordered: positions present in the
 * stressed valuation come first, in stressed-valuation order, followed by any
 * position that only exists in the base valuation, in base-valuation order.
 */
export interface ScenarioPnl {
  /**
   * Per-position scenario P&L in the portfolio base currency.
   */
  by_position: {
    [k: string]: Money;
  };
  /**
   * Total scenario P&L in the portfolio base currency
   * (`stressed.total_base_currency - base.total_base_currency`).
   */
  total: Money;
  [k: string]: unknown;
}
/**
 * One ordered result from `scenario_pnl_batch`.
 */
export interface ScenarioPnlBatchItem {
  /**
   * Scenario-attributable portfolio P&L.
   */
  pnl: ScenarioPnl;
  /**
   * Application provenance and warnings for this scenario.
   */
  report: ApplicationReport;
  /**
   * Identifier copied from the input scenario.
   */
  scenario_id: string;
  [k: string]: unknown;
}
/**
 * Scenario-P&L view returned by binding surfaces.
 */
export interface ScenarioPnlView {
  /**
   * Scenario-attributable profit and loss.
   */
  pnl: ScenarioPnl;
  /**
   * Scenario application report.
   */
  report: ApplicationReport;
  [k: string]: unknown;
}
/**
 * Scenario-and-revalue view returned by binding surfaces.
 */
export interface ScenarioRevalueView {
  /**
   * Scenario application report.
   */
  report: ApplicationReport;
  /**
   * Stressed portfolio valuation.
   */
  valuation: PortfolioValuation;
  [k: string]: unknown;
}
/**
 * Per-sector portfolio and benchmark weights and returns for a single attribution period.
 */
export interface SectorPeriod {
  /**
   * Benchmark return for the sector over the period.
   *
   * Ignored when [`benchmark_weight`](Self::benchmark_weight) is zero:
   * [`brinson_fachler`] substitutes the benchmark total `r_b` (Campisi
   * off-benchmark convention).
   */
  benchmark_return: number;
  /**
   * Benchmark weight in the sector at period start.
   */
  benchmark_weight: number;
  /**
   * Portfolio return for the sector over the period.
   */
  portfolio_return: number;
  /**
   * Portfolio weight in the sector at period start.
   */
  portfolio_weight: number;
  /**
   * Sector identifier (industry / country / any grouping).
   */
  sector: string;
  [k: string]: unknown;
}
/**
 * Canonical wire form of a factor-sensitivity matrix, shared by both hosts.
 */
export interface SensitivityMatrixJson {
  /**
   * Reporting currency of every monetary sensitivity.
   */
  base_currency: Currency;
  /**
   * Sensitivities as nested rows, `data[position][factor]`.
   */
  data: number[][];
  /**
   * Ordered factor identifiers, one per column of `data`.
   */
  factor_ids: FactorId[];
  /**
   * Ordered position identifiers, one per row of `data`.
   */
  position_ids: string[];
}
/**
 * Per-strategy allocation output row.
 */
export interface StrategyAllocation {
  /**
   * Rounded capital allocation.
   */
  capital: number;
  /**
   * Strategy identifier.
   */
  id: string;
  /**
   * Risk contribution fraction, when covariance diagnostics are available.
   */
  risk_contribution?: number | null;
  /**
   * Sample volatility used by inverse-volatility allocation, when applicable.
   */
  volatility?: number | null;
  /**
   * Fully invested allocation weight.
   */
  weight: number;
}
/**
 * Per-strategy allocation input row.
 */
export interface StrategyAllocationInput {
  /**
   * Fixed weight for `fixed` scheme.
   */
  fixed_weight?: number | null;
  /**
   * Stable strategy identifier.
   */
  id: string;
  /**
   * Historical returns for `inverse_volatility`.
   */
  returns?: number[];
  /**
   * Target risk budget fraction for `risk_budget`.
   */
  risk_budget?: number | null;
}
/**
 * P&L-only result of a factor-stress scenario.
 */
export interface StressPnl {
  /**
   * Per-position P&L contributions.
   */
  position_pnl: [unknown, unknown][];
  /**
   * Total portfolio P&L under the stressed market.
   */
  total_pnl: number;
  [k: string]: unknown;
}
/**
 * Result of a factor-stress scenario.
 */
export interface StressResult {
  /**
   * Per-position P&L contributions.
   */
  position_pnl: [unknown, unknown][];
  /**
   * Risk decomposition recomputed under the stressed market.
   */
  stressed_decomposition: RiskDecomposition;
  /**
   * Total portfolio P&L under the stressed market.
   */
  total_pnl: number;
  [k: string]: unknown;
}
/**
 * A single sub-period of a portfolio, with the information needed to compute a Modified-Dietz return.
 */
export interface TwrrPeriod {
  /**
   * PV at period start.
   */
  beginning_market_value: number;
  /**
   * External cashflows during the period.
   *
   * Sign convention: **positive** = contribution into the portfolio
   * (capital added by the client); **negative** = withdrawal. This
   * matches `ReplaySummary.total_pnl` conventions already in use in
   * `portfolio::replay` and is the **opposite** of [`DatedCashflow`] /
   * [`mwr_xirr`]. See the module-level Dietz vs XIRR table. Empty (the
   * wire default when omitted) for a period with no external flows.
   */
  cashflows?: DietzFlow[];
  /**
   * PV at period end.
   */
  ending_market_value: number;
}
/**
 * Strategy allocation result.
 */
export interface WeightAllocationResult {
  /**
   * Per-strategy allocation rows.
   */
  allocations: StrategyAllocation[];
  /**
   * Portfolio-level diagnostics.
   */
  diagnostics: AllocationDiagnostics;
  /**
   * Allocation scheme applied.
   */
  scheme: AllocationScheme;
}
/**
 * JSON specification for strategy-level allocation.
 */
export interface WeightAllocationSpec {
  /**
   * Optional covariance matrix for `risk_budget`, row-major as nested lists.
   */
  covariance?: number[][] | null;
  /**
   * Number of decimal places for capital rounding.
   */
  money_decimal_places?: number;
  /**
   * Allocation scheme to apply.
   */
  scheme: AllocationScheme;
  /**
   * Strategy input rows.
   */
  strategies: StrategyAllocationInput[];
  /**
   * Total capital to allocate across strategies.
   */
  total_capital: number;
}
/**
 * Result of a position what-if scenario.
 */
export interface WhatIfResult {
  /**
   * Decomposition after applying the requested position changes.
   */
  after: RiskDecomposition;
  /**
   * Baseline decomposition used as the comparison point.
   */
  before: RiskDecomposition;
  /**
   * Per-factor changes between `before` and `after`.
   */
  delta: FactorContributionDelta[];
  [k: string]: unknown;
}
