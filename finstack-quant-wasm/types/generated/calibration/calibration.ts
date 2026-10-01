// Generated from the finstack-quant-calibration JSON schemas by scripts/generate-contract-types.mjs. Do not edit.
import type * as valuations from '../valuations/index.js';

/**
 * Calibration method selection (bootstrap vs global solve).
 *
 * Defines the numerical approach used to solve for curve/surface parameters.
 * Bootstrap is the traditional sequential approach, while GlobalSolve
 * solves all parameters simultaneously.
 *
 * # Variants
 * - `Bootstrap`: Traditional sequential bootstrap where each knot is solved
 *   independently based on the previous knots.
 * - `GlobalSolve`: Simultaneous optimization of all knots using Levenberg-Marquardt
 *   or Newton-Raphson.
 */
export type CalibrationMethod =
  | "bootstrap"
  | {
      global_solve: {
        /**
         * Use analytical Jacobian if available (otherwise finite-difference).
         */
        use_analytical_jacobian: boolean;
        [k: string]: unknown;
      };
    };
/**
 * Policy for weighting residuals in global solve calibration.
 *
 * Determines how the objective function weights individual instrument fitting
 * errors (residuals) during optimization.
 *
 * # Variants
 * - `Equal`: Every instrument contributes equally to the objective.
 * - `LinearTime`: Weights increase linearly with time to maturity.
 * - `SqrtTime`: Weights increase with the square root of time (market-standard).
 * - `InverseDuration`: Weights based on inverse DV01 approximation.
 */
export type ResidualWeightingScheme = "equal" | "linear_time" | "sqrt_time" | "inverse_duration";
/**
 * Selected side of the market snapshot.
 */
export type MarketQuoteSide = "mid" | "bid" | "ask";
/**
 * How `CalibrationConfig` obtains rate bounds.
 *
 * Market-standard bounds depend on currency/market regime. `AutoCurrency` makes this choice
 * explicit and avoids relying on `RateBounds::default()` as an implicit assumption.
 */
export type RateBoundsPolicy = "auto_currency" | "explicit";
/**
 * Runtime validation behavior for arbitrage/consistency checks.
 */
export type ValidationMode = "warn" | "error";
/**
 * A single id-addressable input to the calibrator.
 *
 * Each variant is tagged via serde as `{"kind": "<snake_case_variant>", ...}`
 * so callers can author flat heterogeneous lists in JSON/YAML.
 */
export type MarketDatum =
  | (
      | {
          kind: "rate_quote";
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Rate index identifier (e.g. "USD-SOFR-3M").
           */
          index: valuations.Id;
          /**
           * Maturity pillar; on the wire `{"tenor": {"count": 3, "unit": "months"}}`
           * or `{"date": "2024-01-01"}`.
           */
          pillar: Pillar;
          /**
           * Rate value (decimal).
           */
          rate: number;
          type: "deposit";
          [k: string]: unknown;
        }
      | {
          kind: "rate_quote";
          /**
           * End date pillar.
           */
          end: Pillar;
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Rate index identifier.
           */
          index: valuations.Id;
          /**
           * Rate value (decimal).
           */
          rate: number;
          /**
           * Start date pillar.
           */
          start: Pillar;
          type: "fra";
          [k: string]: unknown;
        }
      | {
          kind: "rate_quote";
          /**
           * Future contract identifier (e.g. "CME:SR3").
           */
          contract: IrFutureContractId;
          /**
           * Convexity adjustment as a decimal rate (Hull convention).
           *
           * The implied forward is
           * `forward = (100 - price) / 100 − convexity_adjustment`.
           * A positive adjustment lowers the futures-implied rate toward the true
           * forward. Callers that want no adjustment must pass `0.0` explicitly.
           */
          convexity_adjustment: number;
          /**
           * Last trading date of the future.
           *
           * The convention registry derives the underlying reference period from
           * this date. For in-arrears IMM contracts such as `CME:SR3`, pass the
           * business day before the ending IMM Wednesday, not the named contract
           * month's starting IMM date.
           */
          expiry: valuations.DateWire;
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Price of the future (e.g. 98.50).
           */
          price: number;
          type: "futures";
          [k: string]: unknown;
        }
      | {
          kind: "rate_quote";
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Rate index identifier (floating leg).
           */
          index: valuations.Id;
          /**
           * Maturity pillar of the swap.
           */
          pillar: Pillar;
          /**
           * Fixed rate (decimal) making the swap PV=0.
           */
          rate: number;
          /**
           * Optional spread over the index in decimal format (e.g., 0.0010 for 10 basis points).
           *
           * This spread is added to the floating leg rate. The value is in decimal format
           * and will be converted to basis points internally (multiplied by 10,000).
           */
          spread_decimal?: number | null;
          type: "swap";
          [k: string]: unknown;
        }
    )
  | (
      | {
          kind: "cds_quote";
          /**
           * Convention key (currency + doc clause).
           */
          convention: CdsConventionKey;
          /**
           * Reference entity name.
           */
          entity: string;
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Maturity pillar.
           */
          pillar: Pillar;
          /**
           * Recovery rate assumption (e.g. 0.40).
           */
          recovery_rate: number;
          /**
           * Par spread in basis points (e.g. 100.0).
           */
          spread_bp: number;
          type: "cds_par_spread";
          [k: string]: unknown;
        }
      | {
          kind: "cds_quote";
          /**
           * Convention key.
           */
          convention: CdsConventionKey;
          /**
           * Contractual running coupon in basis points (25.0, 100.0, 500.0 or 1000.0).
           */
          coupon_bp: number;
          /**
           * Reference entity name.
           */
          entity: string;
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Maturity pillar.
           */
          pillar: Pillar;
          /**
           * Recovery rate assumption.
           */
          recovery_rate: number;
          type: "cds_upfront";
          /**
           * Upfront payment percentage of notional (e.g. 0.01 for 1%).
           */
          upfront_pct: number;
          [k: string]: unknown;
        }
    )
  | {
      /**
       * Attachment point (decimal, e.g. 0.03).
       */
      attachment: number;
      /**
       * Convention key (currency + doc clause).
       */
      convention: CdsConventionKey;
      /**
       * Contractual running coupon of the tranche, in basis points.
       */
      coupon_bp: number;
      /**
       * Detachment point (decimal, e.g. 0.07).
       */
      detachment: number;
      /**
       * Unique identifier.
       */
      id: QuoteId;
      /**
       * Index identifier (e.g. CDX.NA.HY).
       */
      index: string;
      kind: "cds_tranche_quote";
      /**
       * Maturity date.
       */
      maturity: valuations.DateWire;
      /**
       * CDS index series number.
       */
      series: number;
      /**
       * Upfront payment as a decimal fraction of tranche notional (e.g., -0.025 for -2.5%).
       */
      upfront_pct: number;
    }
  | (
      | {
          kind: "inflation_quote";
          inflation_swap: {
            /**
             * Per-instrument conventions
             */
            convention: InflationSwapConventionId;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * Inflation index identifier
             */
            index: string;
            /**
             * Swap maturity
             */
            maturity: valuations.DateWire;
            /**
             * Fixed rate (decimal)
             */
            rate: number;
          };
          [k: string]: unknown;
        }
      | {
          kind: "inflation_quote";
          yoy_inflation_swap: {
            /**
             * Instrument-wide conventions
             */
            convention: InflationSwapConventionId;
            /**
             * Payment frequency
             */
            frequency: valuations.Tenor;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * Inflation index identifier
             */
            index: string;
            /**
             * Swap maturity
             */
            maturity: valuations.DateWire;
            /**
             * Fixed rate (decimal)
             */
            rate: number;
          };
          [k: string]: unknown;
        }
    )
  | (
      | {
          kind: "vol_quote";
          option_vol: {
            /**
             * Option expiry
             */
            expiry: valuations.DateWire;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * Option type (Call or Put).
             */
            option_type: OptionType;
            /**
             * Strike
             */
            strike: number;
            /**
             * Underlying identifier
             */
            underlying: valuations.Id;
            /**
             * Implied volatility in decimal units (for example, `0.20` for 20%).
             */
            vol: number;
          };
          [k: string]: unknown;
        }
      | {
          kind: "vol_quote";
          swaption_vol: {
            /**
             * Option exercise conventions
             */
            convention: SwaptionConventionId;
            /**
             * Option expiry
             */
            expiry: valuations.DateWire;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * Underlying swap maturity date
             */
            maturity: valuations.DateWire;
            /**
             * Volatility quoting convention.
             */
            quote_type: VolQuoteType;
            /**
             * Strike rate
             */
            strike: number;
            /**
             * Implied volatility in canonical decimal units: absolute rate
             * volatility for normal quotes and Black volatility for lognormal quotes.
             */
            vol: number;
          };
          [k: string]: unknown;
        }
      | {
          kind: "vol_quote";
          cap_floor_vol: {
            /**
             * Cap/floor maturity or caplet expiry.
             */
            expiry: valuations.DateWire;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * `true` for cap, `false` for floor.
             */
            is_cap: boolean;
            /**
             * Volatility quoting convention.
             */
            quote_type: VolQuoteType;
            /**
             * Strike rate.
             */
            strike: number;
            /**
             * Implied volatility in canonical decimal units: absolute rate
             * volatility for normal quotes and Black volatility for lognormal quotes.
             */
            vol: number;
          };
          [k: string]: unknown;
        }
    )
  | {
      /**
       * Basis spread in basis points on the base-currency leg.
       */
      basis_spread_bp: number;
      /**
       * XCCY pair convention identifier (e.g., `EUR/USD-XCCY`).
       */
      convention: XccyConventionId;
      /**
       * Far-leg maturity pillar; near leg is the convention spot date.
       */
      far_pillar: Pillar;
      /**
       * Unique identifier for the quote.
       */
      id: QuoteId;
      kind: "xccy_quote";
      /**
       * Optional spot FX quote (quote currency per 1 unit of base currency).
       */
      spot_fx?: number | null;
    }
  | {
      /**
       * Base currency (e.g. `EUR` in `EUR/USD`).
       */
      from: valuations.Currency;
      /**
       * Stable identifier for this datum.
       */
      id: string;
      kind: "fx_spot";
      /**
       * Rate such that `1 from = rate to`.
       */
      rate: number;
      /**
       * Quote currency (e.g. `USD` in `EUR/USD`).
       */
      to: valuations.Currency;
    }
  | {
      /**
       * Stable identifier (e.g., asset ticker).
       */
      id: string;
      kind: "price";
      /**
       * Scalar value (unitless or monetary).
       */
      scalar: MarketScalar;
    }
  | {
      kind: "dividend_schedule";
      /**
       * The dividend schedule itself.
       */
      schedule: DividendSchedule;
    }
  | {
      /**
       * Optional currency
       */
      currency?: valuations.Currency | null;
      /**
       * Series identifier
       */
      id: string;
      /**
       * Interpolation method
       */
      interpolation: SeriesInterpolation;
      kind: "fixing_series";
      /**
       * Observations as (date, value) pairs
       */
      observations: [unknown, unknown][];
    }
  | {
      /**
       * Currency
       */
      currency: valuations.Currency;
      /**
       * Unique identifier
       */
      id: string;
      /**
       * Interpolation method
       */
      interpolation: InflationInterpolation;
      kind: "inflation_fixings";
      /**
       * Lag policy
       */
      lag: InflationLag;
      /**
       * Observations as (date, value) pairs
       */
      observations: [unknown, unknown][];
      /**
       * Optional seasonality factors
       *
       * @minItems 12
       * @maxItems 12
       */
      seasonality?:
        [number, number, number, number, number, number, number, number, number, number, number, number] | null;
    }
  | {
      /**
       * ID of the base correlation curve (must exist in context curves)
       */
      base_correlation_curve_id: string;
      /**
       * Unique identifier for this credit index
       */
      id: string;
      /**
       * ID of the index hazard curve (must exist in context curves)
       */
      index_credit_curve_id: string;
      /**
       * Optional map of issuer ID → hazard curve ID
       */
      issuer_credit_curve_ids?: {
        [k: string]: string;
      } | null;
      /**
       * Optional map of issuer ID → recovery rate
       */
      issuer_recovery_rates?: {
        [k: string]: number;
      } | null;
      /**
       * Optional map of issuer ID → weight
       */
      issuer_weights?: {
        [k: string]: number;
      } | null;
      kind: "credit_index";
      /**
       * Number of constituents
       */
      num_constituents: number;
      /**
       * Recovery rate
       */
      recovery_rate: number;
    }
  | {
      /**
       * ATM delta-neutral straddle vols per expiry.
       */
      atm_vols: number[];
      /**
       * Optional 10-delta butterfly per expiry.
       */
      bf_10d?: number[] | null;
      /**
       * 25-delta butterfly per expiry.
       */
      bf_25d: number[];
      /**
       * Expiry times in years.
       */
      expiries: number[];
      /**
       * Surface identifier.
       */
      id: valuations.Id;
      kind: "fx_vol_surface";
      /**
       * Optional 10-delta risk reversal per expiry.
       */
      rr_10d?: number[] | null;
      /**
       * 25-delta risk reversal per expiry.
       */
      rr_25d: number[];
    }
  | {
      /**
       * Option expiries in years, strictly increasing. Indexes the first axis.
       */
      expiries: number[];
      /**
       * Row-major: forwards[expiry_idx * n_tenors + tenor_idx]
       */
      forwards: number[];
      /**
       * Identifier the cube is registered and looked up under.
       */
      id: string;
      /**
       * How volatilities are interpolated between the cube's grid points.
       */
      interpolation_mode: VolInterpolationMode;
      kind: "vol_cube";
      /**
       * Row-major: params[expiry_idx * n_tenors + tenor_idx]
       */
      params: SabrParameterData[];
      /**
       * Underlying swap tenors in years, strictly increasing. Indexes the
       * second axis.
       */
      tenors: number[];
    }
  | {
      /**
       * Collateral / CSA currency.
       */
      csa_currency: valuations.Currency;
      /**
       * Trade-leg currency this CSA mapping applies to.
       */
      id: valuations.Currency;
      kind: "collateral";
    };
/**
 * A stable identifier for a market quote (e.g., "USD-OIS-SWAP-5Y").
 *
 * This ID is used for human readability, logging, and potentially matching against external
 * data sources. Quote IDs should be unique within a calibration set and follow a consistent
 * naming convention (e.g., "{currency}-{index}-{type}-{pillar}").
 *
 * [`QuoteId::new`] is infallible, including for empty strings. Empty or
 * whitespace-only IDs are rejected when this type is deserialized from the
 * wire, and again by [`MarketQuote::validate`](super::market_quote::MarketQuote::validate).
 */
export type QuoteId = string;
/**
 * The maturity pillar of a quote.
 *
 * The pillar represents the maturity of the instrument referenced by the quote. OTC instruments
 * (swaps, deposits) typically use `Tenor` (e.g., "5Y") to allow rolling headers that automatically
 * adjust as the valuation date changes. Futures or bespoke runs may use `Date` to pin a specific
 * maturity date.
 *
 * The JSON wire form is externally tagged: `{"tenor": {"count": 5, "unit": "years"}}`
 * or `{"date": "2029-06-20"}`. A bare string such as `"5Y"` is rejected.
 */
export type Pillar =
  | {
      tenor: valuations.Tenor;
    }
  | {
      date: valuations.DateWire;
    };
/**
 * Stable identifier for an Interest Rate Future contract (e.g., "CME:SR3").
 *
 * Used to look up [`IrFutureConventions`](crate::market::conventions::defs::IrFutureConventions)
 * from the convention registry.
 */
export type IrFutureContractId = string;
/**
 * CDS market standard documentation clauses.
 *
 * Represents the ISDA documentation clause used for CDS contracts. Different clauses
 * define different restructuring events and settlement procedures. Used as part of
 * [`CdsConventionKey`] to look up CDS conventions.
 */
export type CdsDocClause =
  "cr14" | "mr14" | "mm14" | "xr14" | "isda_na" | "isda_eu" | "isda_as" | "isda_au" | "isda_nz" | "custom";
/**
 * Identifier for Inflation Swap market conventions (e.g., "USD-CPI", "UK-RPI").
 *
 * Used to look up [`InflationSwapConventions`](crate::market::conventions::defs::InflationSwapConventions)
 * from the convention registry.
 */
export type InflationSwapConventionId = string;
/**
 * Option payoff direction used by analytical and numerical model engines.
 */
export type OptionType = "call" | "put";
/**
 * Identifier for Swaption market conventions (e.g., "USD", "EUR").
 *
 * Used to look up [`SwaptionConventions`](crate::market::conventions::defs::SwaptionConventions)
 * from the convention registry.
 */
export type SwaptionConventionId = string;
/**
 * Quoting convention of the volatilities stored on a [`VolSurface`].
 *
 * The same `vol_surface_id` channel is read by consumers with very different
 * expectations: rates calibrations typically read normal (Bachelier, absolute)
 * vols on an `expiry × tenor` ATM matrix, while equity/FX/swaption smile
 * consumers read Black (lognormal, relative) vols on `expiry × strike`. The
 * stored numbers are an order of magnitude apart (e.g. 0.008 normal vs 0.20
 * Black), so misreading one as the other silently mis-prices. Tagging the
 * quote type lets consumers enforce their convention via
 * [`VolSurface::require_quote_type`].
 */
export type VolQuoteType = "black_lognormal" | "normal";
/**
 * Identifier for cross-currency swap market conventions (e.g., "EUR/USD-XCCY").
 */
export type XccyConventionId = string;
/**
 * Single market observable that doesn't require a full curve.
 *
 * Represents point-in-time market data like spot prices, spreads, or unitless
 * parameters. Stored in [`MarketContext`](crate::market_data::context::MarketContext)
 * alongside curves for simple lookups.
 *
 * # Use Cases
 *
 * - **Spot prices**: Equity spots, commodity prices, FX spots
 * - **Recovery rates**: Credit recovery assumptions (unitless)
 * - **Correlation parameters**: Equity-FX correlation, basis correlations
 * - **Spreads**: Credit spreads, basis spreads (unitless or monetary)
 * - **Multipliers**: Beta, vega notionals, adjustment factors
 */
export type MarketScalar =
  | {
      unitless: number;
    }
  | {
      price: valuations.Money;
    };
/**
 * Type of dividend event.
 *
 * Distinguishes between cash payments, stock distributions, and continuous
 * yield approximations used in different pricing models.
 *
 * # Variants
 *
 * - **Cash**: Actual dividend payment (quarterly, semi-annual, etc.)
 * - **Stock**: Share distribution (less common, complicates option pricing)
 * - **Yield**: Continuous approximation for analytical models
 */
export type DividendKind =
  | {
      cash: valuations.Money;
    }
  | {
      yield: number;
    }
  | {
      stock: {
        /**
         * Stock distribution ratio; 0.05 corresponds to a 5% stock dividend.
         */
        ratio: number;
        [k: string]: unknown;
      };
    };
/**
 * Interpolation strategy for [`ScalarTimeSeries`].
 */
export type SeriesInterpolation = "step" | "linear";
/**
 * Interpolation method for CPI/RPI values between monthly observations.
 *
 * Determines how index values are computed for dates between published
 * monthly levels. Different markets use different conventions.
 *
 * # Market Conventions
 *
 * - **US TIPS**: Linear interpolation (daily pro-rata)
 * - **UK Index-Linked Gilts**: Linear interpolation with 3-month lag
 * - **Euro inflation bonds**: Varies by issuer (typically linear)
 */
export type InflationInterpolation = "step" | "linear";
/**
 * Publication lag for inflation index reference dates.
 *
 * Inflation indices are published with a delay (typically 2-4 weeks). Securities
 * using these indices incorporate a lag to ensure the reference index is published
 * by the settlement date.
 *
 * # Standard Lags by Market
 *
 * - **US TIPS**: 3-month lag (reference index from 3 months prior)
 * - **UK Index-Linked Gilts**: 3-month lag (8-month for older issues)
 * - **French OATi**: 3-month lag
 * - **German index-linked**: 3-month lag
 *
 * # Rationale
 *
 * The lag ensures:
 * 1. Reference index is published before settlement
 * 2. Index value is known at coupon payment date
 * 3. No estimation or forecasting required for payment calculation
 */
export type InflationLag =
  | {
      months: number;
    }
  | {
      days: number;
    }
  | "none";
/**
 * Interpolation contract for vol surfaces.
 */
export type VolInterpolationMode = "vol" | "total_variance";
/**
 * A single step in the calibration process.
 *
 * Each step targets the construction or update of a specific market object
 * (e.g., a yield curve) using a specified set of quotes.
 */
export type CalibrationStep =
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Step-level conventions for pricing and curve time axis.
       */
      conventions?: RatesStepConventions;
      /**
       * Currency of the curve.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the discount curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Extrapolation policy for the curve.
       */
      extrapolation?: ExtrapolationPolicy;
      /**
       * Interpolation style for the constructed discount curve.
       *
       * Defaults to log-linear discount factors, preserving positive discount
       * factors and piecewise-constant continuously compounded forwards.
       * `Linear` remains available only when explicitly requested.
       */
      interpolation?: InterpStyle;
      kind: "discount";
      /**
       * Calibration method to use.
       */
      method?: CalibrationMethod;
      /**
       * Optional separate ID for pricing logic (defaults to curve_id).
       */
      pricing_discount_id?: valuations.Id | null;
      /**
       * Optional forward curve ID for pricing (if needed).
       */
      pricing_forward_id?: valuations.Id | null;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Step-level conventions for pricing and curve time axis.
       */
      conventions?: RatesStepConventions;
      /**
       * Currency of the curve.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the forward curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Identifier for the discount curve to use.
       */
      discount_curve_id: valuations.Id;
      /**
       * Interpolation style for the constructed forward curve.
       *
       * Defaults to Hagan-West monotone-convex interpolation for a smooth,
       * shape-preserving forward term structure. `Linear` remains explicitly
       * opt-in.
       */
      interpolation?: InterpStyle;
      kind: "forward";
      /**
       * Calibration method to use.
       *
       * Forward curves require [`CalibrationMethod::GlobalSolve`]. Sequential
       * bootstrap is rejected because contractual off-grid reset intervals
       * couple adjacent rates through projection discount factors.
       */
      method?: CalibrationMethod;
      /**
       * Tenor in years for the forward curve.
       */
      tenor_years: number;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Optional CDS valuation convention used by synthetic CDS instruments
       * during hazard calibration and rebootstrap.
       */
      cds_valuation_convention?: CdsValuationConvention | null;
      /**
       * Currency of the curve.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the hazard curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Identifier for the discount curve to use.
       */
      discount_curve_id: valuations.Id;
      /**
       * Optional CDS documentation-clause assertion.
       *
       * Hazard schedule conventions come from the quote `CdsConventionKey`. When
       * this field is set, it must be consistent with those quote-derived
       * conventions (same clause or the matching regional family).
       */
      doc_clause?: string | null;
      /**
       * Entity name.
       */
      entity: string;
      /**
       * Interpolation style for survival probabilities between pillars.
       *
       * Only log-linear survival interpolation is supported, preserving the
       * piecewise-constant hazard representation. Other styles are rejected
       * before calibration starts.
       */
      interpolation?: InterpStyle;
      kind: "hazard";
      /**
       * Calibration method to use.
       */
      method?: CalibrationMethod;
      /**
       * Notional used to price synthetic CDS instruments during calibration.
       *
       * Calibration normalizes residuals by notional, so this is typically left as
       * the unit-notional default unless you have a specific reason to change it.
       */
      notional?: number;
      /**
       * Interpolation method for par spreads reported by the calibrated curve.
       *
       * Note: this is used for *quoting/interpolation of stored par spreads* and does not affect
       * survival no-arbitrage, which is enforced via non-negative hazards together with the
       * required log-linear survival interpolation.
       */
      par_interp?: ParInterp;
      /**
       * Required recovery-rate assumption as a decimal fraction in `[0, 1]`.
       */
      recovery_rate: number;
      /**
       * Seniority of the debt.
       */
      seniority: Seniority;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base CPI level used as the curve's reference CPI at t=0.
       *
       * This is the contractual reference CPI at the start date after applying
       * observation lag and monthly interpolation. Its curve date is
       * `base_date - observation_lag`. Supplied index observations must reproduce
       * this value, including seasonality; a mismatch is rejected.
       */
      base_cpi: number;
      /**
       * Valuation and calibration-instrument start date. The output curve's
       * zero-time reference CPI date is this date minus `observation_lag`.
       */
      base_date: valuations.DateWire;
      /**
       * Currency of the curve.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the inflation curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Identifier for the discount curve to use.
       */
      discount_curve_id: valuations.Id;
      /**
       * Reference index (e.g. "USA-CPI-U").
       */
      index: string;
      /**
       * Interpolation style for the curve.
       */
      interpolation?: InterpStyle;
      kind: "inflation";
      /**
       * Calibration method to use.
       */
      method?: CalibrationMethod;
      /**
       * Notional used to price synthetic inflation swaps during calibration.
       *
       * Calibration normalizes residuals by notional, so this is typically left as
       * the unit-notional default unless you have a specific reason to change it.
       */
      notional?: number;
      /**
       * Observation lag (e.g. "3M").
       *
       * Overrides the quote convention's lag and must match any supplied index
       * lag. The same lag determines the output curve's reference-date origin and the dates
       * of the CPI observations consumed by calibration instruments.
       */
      observation_lag: string;
      /**
       * Optional seasonal adjustment factors for deseasonalizing CPI observations.
       *
       * When provided, the calibrator will:
       * 1. Deseasonalize input CPI levels using the monthly factors
       * 2. Fit the smooth zero-coupon curve to deseasonalized levels
       * 3. Reseasonalize the output CPI path
       *
       * Monthly adjustments are additive to log CPI level. They should approximately
       * sum to zero over 12 months.
       */
      seasonal_factors?: SeasonalFactors | null;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the surface.
       */
      base_date: valuations.DateWire;
      /**
       * SABR Beta parameter.
       */
      beta?: number;
      /**
       * Discount curve ID.
       */
      discount_curve_id?: valuations.Id | null;
      /**
       * Optional dividend yield override.
       */
      dividend_yield_override?: number | null;
      /**
       * Extrapolation policy for total-variance fill across expiries.
       *
       * After each quoted expiry is calibrated, the published expiry×strike
       * grid is filled by interpolating total variance `w = σ²T` in expiry
       * (not by interpolating SABR α/ν/ρ and evaluating Hagan at the target
       * `T`). This policy controls targets that fall outside the calibrated
       * expiry range: `Error` rejects them; `Clamp` holds the nearest slice's
       * total variance flat.
       */
      expiry_extrapolation?: SurfaceExtrapolationPolicy;
      kind: "vol_surface";
      /**
       * Volatility model used for calibration.
       */
      model: VolSurfaceModel;
      /**
       * Optional spot price override.
       */
      spot_override?: number | null;
      /**
       * Target expiries for calibration.
       */
      target_expiries?: number[];
      /**
       * Target strikes for calibration.
       */
      target_strikes?: number[];
      /**
       * Identifier for the underlying instrument.
       */
      underlying_ticker: string;
      /**
       * Identifier for the volatility surface being built.
       */
      vol_surface_id: string;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Allow deterministic fallbacks when SABR grid corners are missing during interpolation.
       *
       * If `false` (default), missing corner buckets are treated as an error instead of
       * silently substituting a nearby bucket.
       */
      allow_sabr_missing_bucket_fallback?: boolean;
      /**
       * Base date for the calibration.
       */
      base_date: valuations.DateWire;
      /**
       * Optional calendar identifier for date adjustments.
       */
      calendar_id?: string | null;
      /**
       * Currency for the swaption surface.
       */
      currency: valuations.Currency;
      /**
       * Discount curve identifier for pricing.
       */
      discount_curve_id: valuations.Id;
      /**
       * Optional day count convention for fixed leg calculations.
       */
      fixed_day_count?: valuations.DayCount | null;
      /**
       * Optional forward curve identifier (if different from discount curve).
       */
      forward_id?: string | null;
      kind: "swaption_vol";
      /**
       * SABR beta parameter (typically 0.0 for normal, 1.0 for lognormal).
       */
      sabr_beta?: number;
      /**
       * Extrapolation policy used when interpolating SABR parameters across the
       * expiry–tenor grid for target points that do not have a directly calibrated bucket.
       */
      sabr_extrapolation?: SurfaceExtrapolationPolicy;
      /**
       * SABR parameter interpolation method between expiries/tenors.
       */
      sabr_interpolation?: SabrInterpolationMethod;
      /**
       * Optional floating index identifier used to resolve market swap conventions.
       *
       * Swaption forward/par rate calculations require swap schedule conventions (fixed frequency,
       * day count, calendar, BDC) that are now indexed off a rate index conventions registry.
       *
       * If omitted, individual swaption quotes must provide `float_leg_conventions.index`.
       */
      swap_index?: valuations.Id | null;
      /**
       * Target expiry times (in years) for the surface grid.
       */
      target_expiries?: number[];
      /**
       * Target tenor times (in years) for the surface grid.
       */
      target_tenors?: number[];
      /**
       * Volatility quoting convention (normal or lognormal).
       */
      vol_convention?: SwaptionVolConvention;
      /**
       * Identifier for the volatility surface.
       */
      vol_surface_id: string;
      /**
       * Maximum absolute error of any fitted volatility quote; defaults to 0.0015.
       *
       * This is distinct from `plan.settings.tolerance` (solver tolerance). For swaption-vol
       * calibration, success should reflect whether the fitted smile residuals are within a
       * market-appropriate tolerance (e.g., 10–20 normal vol bp), not machine epsilon.
       * Normal quotes use decimal rate per square-root year; Black quotes use
       * dimensionless annual volatility. Errors are never scaled by vega.
       */
      vol_tolerance?: number | null;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the calibration.
       */
      base_date: valuations.DateWire;
      /**
       * Business day convention for synthetic tranche schedule adjustments.
       */
      business_day_convention?: valuations.BusinessDayConvention | null;
      /**
       * Optional calendar identifier for schedule generation and date adjustments.
       */
      calendar_id?: string | null;
      /**
       * Currency used for synthetic tranche pricing.
       */
      currency: valuations.Currency;
      /**
       * Day count convention for synthetic tranche premium accrual.
       */
      day_count?: valuations.DayCount | null;
      /**
       * Detachment points (as percentages) for the tranches.
       */
      detachment_points?: number[];
      /**
       * Discount curve identifier for pricing.
       */
      discount_curve_id: valuations.Id;
      /**
       * Payment frequency for synthetic tranches (e.g., quarterly).
       */
      frequency?: valuations.Tenor | null;
      /**
       * Credit index identifier (e.g., CDX, iTraxx).
       */
      index_id: string;
      kind: "base_correlation";
      /**
       * Maturity of the tranches in years.
       */
      maturity_years: number;
      /**
       * Notional used to price synthetic tranches during calibration.
       *
       * Calibration can be expressed in upfront % terms, so this is typically left
       * as unit-notional unless you have a specific reason to change it.
       */
      notional?: number;
      /**
       * Coupon roll-date grid for the synthetic tranches: `cds_imm` for the
       * standard CDS roll dates (20th of Mar/Jun/Sep/Dec) or `none` (the
       * default) for a schedule generated from the convention frequency and
       * stub. `imm` is rejected.
       */
      roll_rule?: RollRule;
      /**
       * Series number of the credit index.
       */
      series: number;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Identifier for the pre-calibrated base correlation curve.
       */
      base_correlation_curve_id: string;
      /**
       * Market-implied flat correlation for the tranche.
       */
      correlation?: number;
      /**
       * Feasible domain for `df` as `(lower_bound, upper_bound)`.
       *
       * `df` must be > 2 for finite variance. Typical range: `(2.1, 50.0)`.
       *
       * @minItems 2
       * @maxItems 2
       */
      df_bounds?: [unknown, unknown];
      /**
       * Discount curve identifier used to price the tranche.
       *
       * When omitted, calibration falls back to the only discount curve present
       * in the market context as a convenience default.
       */
      discount_curve_id?: valuations.Id | null;
      /**
       * Starting guess for degrees of freedom (typically 4-10).
       */
      initial_df?: number;
      kind: "student_t";
      /**
       * Identifier for the reference tranche instrument.
       */
      tranche_instrument_id: string;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the calibration.
       */
      base_date: valuations.DateWire;
      /**
       * Currency for conventions.
       */
      currency: valuations.Currency;
      /**
       * Discount curve ID (must already exist in market context).
       */
      curve_id: valuations.Id;
      /**
       * Required positive maximum implied-quote error in quoted volatility units.
       * Normal quotes use decimal rate volatility; Black quotes use relative volatility.
       * This acceptance budget is independent of the numerical solver tolerance.
       */
      fit_tolerance: number;
      /**
       * Optional initial guess for mean reversion κ.
       */
      initial_kappa?: number | null;
      /**
       * Optional initial guess for short rate vol σ.
       */
      initial_sigma?: number | null;
      kind: "hull_white";
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the calibration.
       */
      base_date: valuations.DateWire;
      /**
       * Currency for conventions.
       */
      currency: valuations.Currency;
      /**
       * Discount curve ID (must already exist in market context).
       */
      discount_curve_id: valuations.Id;
      /**
       * Required positive maximum implied-quote error in quoted volatility units.
       * Normal quotes use decimal rate volatility; Black quotes use relative volatility.
       * This acceptance budget is independent of the numerical solver tolerance.
       */
      fit_tolerance: number;
      /**
       * Optional source mean reversion κ. Required for one-quote calibration.
       */
      fixed_kappa?: number | null;
      /**
       * Forward/projection curve ID. If equal to `discount_curve_id`, the
       * discount curve is used as the single-curve projection proxy.
       */
      forward_curve_id: valuations.Id;
      /**
       * Optional initial guess for mean reversion κ when solving both κ and σ.
       */
      initial_kappa?: number | null;
      /**
       * Optional initial guess for short-rate volatility σ when solving both κ and σ.
       */
      initial_sigma?: number | null;
      kind: "cap_floor_hull_white";
      /**
       * Payment frequency used to decompose quoted caps/floors into caplets.
       */
      payment_frequency?: SwapFrequency;
      /**
       * Scalar or expiry-bootstraped piecewise short-rate volatility calibration.
       */
      volatility_mode?: HullWhiteVolatilityMode;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the surface.
       */
      base_date: valuations.DateWire;
      /**
       * Discount curve ID (optional).
       */
      discount_curve_id?: valuations.Id | null;
      /**
       * Optional continuous dividend yield; defaults to the market scalar
       * `"<underlying_ticker>-DIVYIELD"` or zero.
       */
      dividend_yield_override?: number | null;
      kind: "svi_surface";
      /**
       * Optional spot price override.
       */
      spot_override?: number | null;
      /**
       * Target expiries for calibration.
       */
      target_expiries?: number[];
      /**
       * Target strikes for calibration.
       */
      target_strikes?: number[];
      /**
       * Underlying instrument ticker.
       */
      underlying_ticker: string;
      /**
       * Identifier for the volatility surface being built.
       */
      vol_surface_id: string;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Optional ID for the byproduct basis spread curve.
       */
      basis_spread_curve_id?: valuations.Id | null;
      /**
       * Step-level conventions for pricing and curve time axis.
       */
      conventions?: RatesStepConventions;
      /**
       * Foreign currency being calibrated.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the foreign discount curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Identifier for the pre-calibrated domestic discount curve.
       */
      domestic_discount_id: valuations.Id;
      /**
       * Extrapolation policy for the foreign curve.
       */
      extrapolation?: ExtrapolationPolicy;
      /**
       * T+0 cash FX rate (domestic per foreign), used when a quote omits `spot_fx`.
       *
       * Covered-interest parity from today needs the cash FX, not the screen
       * "spot". Market spot is T+2 for most G10 pairs and T+1 for USD/CAD;
       * convert to T+0 using ON/TN points before passing the rate here.
       * Mixing T+2 screen spot with T+0 discounting biases long-tenor basis
       * by roughly 1–2 bp.
       */
      fx_spot: number;
      /**
       * Interpolation style for the constructed foreign discount curve.
       *
       * Defaults to log-linear discount factors. `Linear` remains available
       * only when explicitly requested.
       */
      interpolation?: InterpStyle;
      kind: "xccy_basis";
      /**
       * Calibration method to use.
       */
      method?: CalibrationMethod;
    }
  | {
      /**
       * Unique identifier for the object being calibrated in this step.
       */
      id: string;
      /**
       * Reference to a named quote set in the parent plan.
       */
      quote_set: string;
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Identifier for the single discount curve being fitted. All calibration
       * instruments use this curve for discounting and implied projection.
       */
      curve_id: valuations.Id;
      /**
       * Optional initial parameter guesses.
       */
      initial_params?: NelsonSiegelModel | null;
      kind: "parametric";
      /**
       * Nelson-Siegel variant (NS or NSS).
       */
      model: NsVariant;
    };
/**
 * How the fixings of one accrual period combine into the period rate.
 *
 * One canonical enum for every floating leg and coupon: swaps, basis and
 * cross-currency legs, TRS financing, IR futures, FRN / loan / structured
 * credit coupons.
 *
 * | Variant | Period rate | Typical use |
 * |---------|-------------|-------------|
 * | `simple` | One term fixing / forward over the period | EURIBOR, Term SOFR, legacy IBOR |
 * | `simple_average` | `(Σ rᵢ·dᵢ) / D` of daily overnight fixings | Averaged overnight loans, Fed-Funds futures |
 * | `compounded_in_arrears` | `[∏(1 + rᵢ·dᵢ/B) − 1]·B/D` | SOFR / SONIA / €STR / TONA OIS (lookback 0) |
 * | `compounded_with_observation_shift` | as above, observations and weights shifted | ISDA 2021 observation shift |
 * | `compounded_with_rate_cutoff` | as above, last fixings frozen | ARRC lockout / SWPM "Rate Cut-Off Days" |
 *
 * Day counts are business days and must be non-negative.
 *
 * # References
 *
 * - ISDA 2021 Definitions, compounded RFR conventions `docs/REFERENCES.md#isda-2021-definitions`
 * - ARRC (2020). "SOFR: A User's Guide." `docs/REFERENCES.md#arrc-sofr-users-guide`
 * - BoE SONIA conventions `docs/REFERENCES.md#boe-sonia-key-features`
 */
export type FloatingLegCompounding =
  | "simple"
  | "simple_average"
  | {
      compounded_in_arrears: {
        /**
         * Business days by which observation dates move backward while the
         * day-count weights stay on the original accrual dates ("lookback
         * without observation shift", ISDA 2021 / ARRC). `0` is plain
         * in-arrears, the cleared-OIS convention.
         */
        lookback_days: number;
      };
    }
  | {
      compounded_with_observation_shift: {
        /**
         * Business days to shift both observation dates and weights.
         */
        shift_days: number;
      };
    }
  | {
      compounded_with_rate_cutoff: {
        /**
         * Business days before period end over which the rate is frozen.
         */
        cutoff_days: number;
      };
    };
/**
 * Extrapolation policy for evaluation outside the knot range.
 *
 * All [`InterpolationStrategy`](super::traits::InterpolationStrategy)
 * implementations honour this policy at both ends of the knot range.
 * Choose based on what behaviour the downstream pricer expects:
 *
 * - `FlatZero` (default): hold the boundary value constant. Safe for
 *   discount factors and survival probabilities.
 * - `FlatForward`: extend the boundary slope. Useful for forward-rate
 *   curves where extrapolated rates should track the local term-structure
 *   slope.
 * - `Nan`: refuse to extrapolate; return `NaN` and require the caller to
 *   detect and handle out-of-range queries explicitly. Recommended for
 *   production risk systems.
 *
 * `#[non_exhaustive]` so that future policies (e.g. linear-zero,
 * asymptotic) can be added without breaking downstream code.
 */
export type ExtrapolationPolicy = "flat_zero" | "flat_forward" | "none";
/**
 * Enum of supported interpolation styles. The default is `Linear`.
 */
export type InterpStyle = "linear" | "log_linear" | "monotone_convex" | "cubic_hermite" | "piecewise_quadratic_forward";
/**
 * Valuation presentation and pricing policy for CDS marks.
 *
 * Each variant bundles a coherent set of choices (premium-leg accrual schedule,
 * Act/360 accrual day counting, accrual-on-default bias, clean/dirty NPV,
 * par-spread denominator). Mixing those choices via separate boolean
 * overrides is intentionally not supported — the variants here are the only
 * conventions traded in practice.
 */
export type CdsValuationConvention =
  "isda_dirty" | "bloomberg_cdsw_clean" | "bloomberg_cdsw_clean_full_premium" | "quant_lib_isda_parity";
/**
 * Interpolation method for reporting par spreads stored on the curve.
 *
 * Applies only to *par-spread* readouts (the spreads quoted at calibration
 * pillars), not to the underlying hazard rates. Hazard interpolation always
 * follows piecewise-constant survival. Use `LogLinear` when spreads span
 * multiple decades (e.g. high-yield issuers) so interpolation stays in
 * log-space; otherwise the default `Linear` is fine.
 */
export type ParInterp = "linear" | "log_linear";
/**
 * Seniority level for credit exposures.
 *
 * Used to tag a hazard curve with the seniority of the issuer's debt
 * observed by the curve. Drives the recovery-rate prior in default
 * modelling and selects the right LGD prior in
 * `finstack_quant_models::credit::lgd::seniority`.
 *
 * Order is **not** total — `SeniorSecured` is strictly senior to `Senior`,
 * `Subordinated`, and `Junior`, but the relative ordering of `Subordinated`
 * vs. `Junior` is jurisdiction-dependent. Do not rely on `Ord` semantics.
 */
export type Seniority = "senior_secured" | "senior" | "subordinated" | "junior";
/**
 * Extrapolation policy for volatility surface construction.
 */
export type SurfaceExtrapolationPolicy = "error" | "clamp";
/**
 * Parameters for volatility surface calibration step.
 */
export type VolSurfaceModel = "sabr";
/**
 * Interpolation method for SABR parameters across the expiry–tenor grid.
 */
export type SabrInterpolationMethod = "bilinear";
/**
 * Volatility quoting convention for swaptions.
 */
export type SwaptionVolConvention =
  | "normal"
  | "lognormal"
  | {
      shifted_lognormal: {
        /**
         * Shift amount for negative rate handling
         */
        shift: number;
        [k: string]: unknown;
      };
    };
/**
 * Roll-date rule applied when generating schedule anchors.
 *
 * Selects the core `ScheduleBuilder` date-generation mode. The IMM modes
 * force a quarterly grid: the schedule frequency and stub rule configured on
 * [`ScheduleParams`] are overridden with quarterly / short-back, matching
 * `ScheduleBuilder::imm` and `ScheduleBuilder::cds_imm`.
 *
 * # Variants
 *
 * - **`None`**: plain tenor stepping from the schedule boundaries (default).
 * - **`Imm`**: quarterly third Wednesdays of Mar/Jun/Sep/Dec (CME IMM dates
 *   for rate, currency, and equity index futures).
 * - **`CdsImm`**: 20th of Mar/Jun/Sep/Dec (post-Big-Bang standard CDS roll
 *   dates). When the start date is not itself a roll date, the first period
 *   accrues from the roll date immediately **preceding** the start (standard
 *   front accrual per the ISDA Big Bang Protocol, April 2009).
 *
 * # References
 *
 * - `docs/REFERENCES.md#isda-cds-standard-model`
 * - CME IMM date rules (third Wednesday of the contract month)
 */
export type RollRule = "none" | "imm" | "cds_imm";
/**
 * Number of coupon payments per year for the underlying swap in HW1F calibration.
 *
 * USD swaps are semi-annual (2), EUR swaps are annual (1).
 */
export type SwapFrequency = "annual" | "semi_annual" | "quarterly";
/**
 * Parameters for Hull-White 1-factor calibration to cap/floor volatility quotes.
 */
export type HullWhiteVolatilityMode = "scalar" | "piecewise";
/**
 * Nelson-Siegel model parameters.
 *
 * Stores either the 4-parameter NS or 6-parameter NSS specification.
 */
export type NelsonSiegelModel =
  | {
      /**
       * Long-term rate level.
       */
      beta0: number;
      /**
       * Short-term component.
       */
      beta1: number;
      /**
       * Medium-term hump.
       */
      beta2: number;
      /**
       * Decay factor (must be > 0).
       */
      tau: number;
      variant: "ns";
      [k: string]: unknown;
    }
  | {
      /**
       * Long-term rate level.
       */
      beta0: number;
      /**
       * Short-term component.
       */
      beta1: number;
      /**
       * Medium-term hump.
       */
      beta2: number;
      /**
       * Second hump.
       */
      beta3: number;
      /**
       * First decay factor (must be > 0).
       */
      tau1: number;
      /**
       * Second decay factor (must be > 0, ≠ τ₁).
       */
      tau2: number;
      variant: "nss";
      [k: string]: unknown;
    };
/**
 * Nelson-Siegel model variant selector.
 */
export type NsVariant = "ns" | "nss";
/**
 * A pre-built calibrated market object.
 *
 * Variants are tagged via serde as `{"kind": "<snake_case_variant>", ...}` so
 * callers can author flat heterogeneous lists in JSON/YAML. Each wrapped
 * curve or surface contributes its own derived JSON Schema.
 */
export type PriorMarketObject =
  | {
      /**
       * Whether non-monotonic DFs are allowed (dangerous override)
       */
      allow_non_monotonic: boolean;
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * OIS cut-off (business days) the curve was calibrated under, if any.
       */
      calibration_ois_cutoff_days?: number | null;
      /**
       * Day count convention for discount time basis
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Opaque FX policy stamp; see [`DiscountCurve::fx_policy`].
       */
      fx_policy?: string | null;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      kind: "discount_curve";
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Minimum forward rate floor (if set)
       */
      min_forward_rate?: number | null;
      /**
       * Minimum tenor for forward rate calculations
       */
      min_forward_tenor: number;
      /**
       * Exact typed calibration replay recipe.
       */
      rate_calibration?: RateCalibrationRecipe | null;
    }
  | {
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * Day count convention
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
       */
      fx_policy?: string | null;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      kind: "forward_curve";
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Optional contractual reset/end-date boundaries.
       *
       * `None` selects fixed numeric-tenor discount-factor stepping.
       */
      projection_grid?: number[] | null;
      /**
       * Exact typed calibration replay recipe.
       */
      rate_calibration?: RateCalibrationRecipe | null;
      /**
       * Reset lag in business days
       */
      reset_lag: number;
      /**
       * Index tenor in years
       */
      tenor: number;
    }
  | {
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * Currency
       */
      currency?: valuations.Currency | null;
      /**
       * Day count convention
       */
      day_count: valuations.DayCount;
      /**
       * Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
       */
      fx_policy?: string | null;
      /**
       * Exact calibration replay inputs.
       */
      hazard_calibration?: HazardCalibrationRecipe | null;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Optional issuer
       */
      issuer?: string | null;
      kind: "hazard_curve";
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Par interpolation method
       */
      par_interp?: ParInterp;
      /**
       * Par spread points for reporting
       */
      par_points: [unknown, unknown][];
      /**
       * Recovery rate
       */
      recovery_rate: number;
      /**
       * Seniority
       */
      seniority?: Seniority | null;
      /**
       * Survival-probability interpolation style between pillars
       */
      survival_interp?: InterpStyle;
    }
  | {
      /**
       * Base CPI level at t=0
       */
      base_cpi: number;
      /**
       * Base date
       */
      base_date: valuations.DateWire;
      /**
       * Day count convention
       */
      day_count?: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Indexation lag in months
       */
      indexation_lag_months?: number;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      kind: "inflation_curve";
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
    }
  | {
      correlations: number[];
      detachment_points: number[];
      id: valuations.Id;
      kind: "base_correlation_curve";
    }
  | {
      /**
       * Base date.
       */
      base: valuations.DateWire;
      /**
       * Day count convention.
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy.
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier.
       */
      id: string;
      /**
       * Interpolation style.
       */
      interp_style: InterpStyle;
      kind: "basis_spread_curve";
      /**
       * Time/value pairs used to construct the curve.
       */
      knot_points: [unknown, unknown][];
    }
  | {
      /**
       * Base date.
       */
      base_date: valuations.DateWire;
      /**
       * Day count convention.
       */
      day_count: valuations.DayCount;
      /**
       * Curve identifier.
       */
      id: string;
      kind: "parametric_curve";
      /**
       * Nelson-Siegel model parameters.
       */
      model: NelsonSiegelModel;
    }
  | {
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * Day count convention
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      kind: "price_curve";
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Spot index level (volatility index curves)
       */
      spot_level?: number | null;
      /**
       * Spot price (signed price curves)
       */
      spot_price?: number | null;
    }
  | {
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * Day count convention
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      kind: "volatility_index_curve";
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Spot index level (volatility index curves)
       */
      spot_level?: number | null;
      /**
       * Spot price (signed price curves)
       */
      spot_price?: number | null;
    }
  | {
      /**
       * Expiry times in years
       */
      expiries: number[];
      /**
       * Surface identifier
       */
      id: string;
      /**
       * Interpolation contract.
       */
      interpolation_mode: VolInterpolationMode;
      kind: "vol_surface";
      /**
       * Quote convention.
       */
      quote_type: VolQuoteType;
      /**
       * Semantic meaning of the secondary axis.
       */
      secondary_axis: VolSurfaceAxis;
      /**
       * Strike prices
       */
      strikes: number[];
      /**
       * Volatility values in row-major order
       */
      vols_row_major: number[];
    };
/**
 * Numerical method used to calibrate a rate curve.
 */
export type RateCalibrationMethod =
  | "bootstrap"
  | {
      global_solve: {
        /**
         * Whether the original solve requested its specialized Jacobian.
         */
        use_analytical_jacobian: boolean;
        [k: string]: unknown;
      };
    };
/**
 * OIS floating-leg compounding convention used during calibration.
 */
export type RateCalibrationOisCompounding =
  | "simple"
  | {
      compounded_in_arrears: {
        /**
         * Business-day lookback applied to rate observations.
         */
        lookback_days: number;
        [k: string]: unknown;
      };
    }
  | {
      compounded_with_observation_shift: {
        /**
         * Business days by which observations and accrual weights are shifted.
         */
        shift_days: number;
        [k: string]: unknown;
      };
    }
  | {
      compounded_with_rate_cutoff: {
        /**
         * Number of business days in the rate-cutoff window.
         */
        cutoff_days: number;
        [k: string]: unknown;
      };
    };
/**
 * Lossless rate quote representation used by calibration replay.
 */
export type RateCalibrationQuote =
  | {
      deposit: {
        /**
         * Referenced rate index.
         */
        index_id: valuations.Id;
        /**
         * Relative-tenor or absolute-date pillar.
         */
        pillar: RateCalibrationPillar;
        /**
         * Quoted deposit rate.
         */
        rate: number;
      };
    }
  | {
      fra: {
        /**
         * FRA end pillar.
         */
        end: RateCalibrationPillar;
        /**
         * Referenced rate index.
         */
        index_id: valuations.Id;
        /**
         * Quoted FRA rate.
         */
        rate: number;
        /**
         * FRA start pillar.
         */
        start: RateCalibrationPillar;
      };
    }
  | {
      futures: {
        /**
         * Convention-registry identifier for the futures contract.
         */
        contract: RateCalibrationFutureContractId;
        /**
         * Optional pre-computed convexity adjustment.
         */
        convexity_adjustment?: number | null;
        /**
         * Futures expiry date.
         */
        expiry: valuations.DateWire;
        /**
         * Quoted futures price.
         */
        price: number;
      };
    }
  | {
      swap: {
        /**
         * Referenced floating-rate index.
         */
        index_id: valuations.Id;
        /**
         * Relative-tenor or absolute-date maturity pillar.
         */
        pillar: RateCalibrationPillar;
        /**
         * Quoted fixed rate.
         */
        rate: number;
        /**
         * Optional floating-leg spread.
         */
        spread_decimal?: number | null;
      };
    }
  | {
      basis: {
        /**
         * Referenced projection-rate index.
         */
        index_id: valuations.Id;
        /**
         * Relative-tenor or absolute-date maturity pillar.
         */
        pillar: RateCalibrationPillar;
        /**
         * Quoted basis spread in decimal form.
         */
        spread_decimal: number;
      };
    };
/**
 * Typed maturity specification retained from an original market quote.
 */
export type RateCalibrationPillar =
  | {
      tenor: valuations.Tenor;
    }
  | {
      date: valuations.DateWire;
    };
/**
 * Serialized identifier for an interest-rate futures convention.
 */
export type RateCalibrationFutureContractId = string;
/**
 * Role of a curve and its linked rate-curve identifier during calibration.
 */
export type RateCalibrationCurveRole =
  | {
      discount: {
        /**
         * Projection curve identifier used by the calibration instruments.
         */
        projection_curve_id: valuations.Id;
        [k: string]: unknown;
      };
    }
  | {
      projection: {
        /**
         * Discount curve identifier used by the calibration instruments.
         */
        discount_curve_id: valuations.Id;
        [k: string]: unknown;
      };
    };
/**
 * Semantic meaning of the secondary axis on a [`VolSurface`].
 *
 * Most option surfaces are defined on `expiry × strike`, but some calibration
 * workflows materialize ATM matrices on `expiry × tenor`. Keeping the axis type
 * explicit prevents consumers from accidentally interpreting tenor buckets as
 * strikes.
 */
export type VolSurfaceAxis = "strike" | "tenor";
/**
 * Exact schema marker accepted by calibration envelopes.
 */
export type CalibrationSchema = "finstack_quant.calibration/1";
/**
 * Domain-specific trace entry types.
 *
 * Each variant captures relevant details for different types of computations:
 * - Calibration: iteration details, convergence status
 * - Pricing: cashflow-level PV breakdowns
 * - Waterfall: step-by-step payment allocations
 */
export type TraceEntry =
  | {
      /**
       * Whether convergence was achieved
       */
      converged: boolean;
      /**
       * Iteration number (0-based)
       */
      iteration: number;
      kind: "calibration_iteration";
      /**
       * Knot points that were updated
       */
      knots_updated: string[];
      /**
       * Objective function residual
       */
      residual: number;
    }
  | {
      /**
       * Cashflow amount (stored as f64 for JSON simplicity)
       */
      cashflow_amount: number;
      /**
       * Cashflow currency
       */
      cashflow_currency: string;
      /**
       * Discount curve ID used
       */
      curve_id: string;
      /**
       * Cashflow payment date (ISO8601)
       */
      date: valuations.DateWire;
      /**
       * Discount factor applied
       */
      discount_factor: number;
      kind: "cashflow_pv";
      /**
       * Present value of this cashflow
       */
      pv_amount: number;
      /**
       * PV currency
       */
      pv_currency: string;
    }
  | {
      /**
       * Cash inflow amount
       */
      cash_in_amount: number;
      /**
       * Cash inflow currency
       */
      cash_in_currency: string;
      /**
       * Cash outflow amount
       */
      cash_out_amount: number;
      /**
       * Cash outflow currency
       */
      cash_out_currency: string;
      kind: "waterfall_step";
      /**
       * Period index
       */
      period: number;
      /**
       * Shortfall amount if any
       */
      shortfall_amount?: number | null;
      /**
       * Shortfall currency
       */
      shortfall_currency?: string | null;
      /**
       * Step name/description
       */
      step_name: string;
    }
  | {
      /**
       * Step description
       */
      description: string;
      kind: "computation_step";
      /**
       * Arbitrary metadata (JSON object)
       */
      metadata?: {
        [k: string]: unknown;
      };
      /**
       * Step name
       */
      name: string;
    };
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
 * Serializable state representation for any curve type.
 *
 * Produced when persisting market data snapshots through serde.
 */
export type CurveState =
  | {
      /**
       * Whether non-monotonic DFs are allowed (dangerous override)
       */
      allow_non_monotonic: boolean;
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * OIS cut-off (business days) the curve was calibrated under, if any.
       */
      calibration_ois_cutoff_days?: number | null;
      /**
       * Day count convention for discount time basis
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Opaque FX policy stamp; see [`DiscountCurve::fx_policy`].
       */
      fx_policy?: string | null;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Minimum forward rate floor (if set)
       */
      min_forward_rate?: number | null;
      /**
       * Minimum tenor for forward rate calculations
       */
      min_forward_tenor: number;
      /**
       * Exact typed calibration replay recipe.
       */
      rate_calibration?: RateCalibrationRecipe | null;
      type: "discount";
    }
  | {
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * Day count convention
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
       */
      fx_policy?: string | null;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Optional contractual reset/end-date boundaries.
       *
       * `None` selects fixed numeric-tenor discount-factor stepping.
       */
      projection_grid?: number[] | null;
      /**
       * Exact typed calibration replay recipe.
       */
      rate_calibration?: RateCalibrationRecipe | null;
      /**
       * Reset lag in business days
       */
      reset_lag: number;
      /**
       * Index tenor in years
       */
      tenor: number;
      type: "forward";
    }
  | {
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * Currency
       */
      currency?: valuations.Currency | null;
      /**
       * Day count convention
       */
      day_count: valuations.DayCount;
      /**
       * Opaque FX policy stamp; see [`crate::market_data::term_structures::DiscountCurve::fx_policy`].
       */
      fx_policy?: string | null;
      /**
       * Exact calibration replay inputs.
       */
      hazard_calibration?: HazardCalibrationRecipe | null;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Optional issuer
       */
      issuer?: string | null;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Par interpolation method
       */
      par_interp?: ParInterp;
      /**
       * Par spread points for reporting
       */
      par_points: [unknown, unknown][];
      /**
       * Recovery rate
       */
      recovery_rate: number;
      /**
       * Seniority
       */
      seniority?: Seniority | null;
      /**
       * Survival-probability interpolation style between pillars
       */
      survival_interp?: InterpStyle;
      type: "hazard";
    }
  | {
      /**
       * Base CPI level at t=0
       */
      base_cpi: number;
      /**
       * Base date
       */
      base_date: valuations.DateWire;
      /**
       * Day count convention
       */
      day_count?: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Indexation lag in months
       */
      indexation_lag_months?: number;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      type: "inflation";
    }
  | {
      correlations: number[];
      detachment_points: number[];
      id: valuations.Id;
      type: "base_correlation";
    }
  | {
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * Day count convention
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Spot index level (volatility index curves)
       */
      spot_level?: number | null;
      /**
       * Spot price (signed price curves)
       */
      spot_price?: number | null;
      type: "price";
    }
  | {
      /**
       * Base date
       */
      base: valuations.DateWire;
      /**
       * Day count convention
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier
       */
      id: string;
      /**
       * Interpolation style
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve
       */
      knot_points: [unknown, unknown][];
      /**
       * Spot index level (volatility index curves)
       */
      spot_level?: number | null;
      /**
       * Spot price (signed price curves)
       */
      spot_price?: number | null;
      type: "vol_index";
    }
  | {
      /**
       * Base date.
       */
      base: valuations.DateWire;
      /**
       * Day count convention.
       */
      day_count: valuations.DayCount;
      /**
       * Extrapolation policy.
       */
      extrapolation: ExtrapolationPolicy;
      /**
       * Curve identifier.
       */
      id: string;
      /**
       * Interpolation style.
       */
      interp_style: InterpStyle;
      /**
       * Time/value pairs used to construct the curve.
       */
      knot_points: [unknown, unknown][];
      type: "basis_spread";
    }
  | {
      /**
       * Base date.
       */
      base_date: valuations.DateWire;
      /**
       * Day count convention.
       */
      day_count: valuations.DayCount;
      /**
       * Curve identifier.
       */
      id: string;
      /**
       * Nelson-Siegel model parameters.
       */
      model: NelsonSiegelModel;
      type: "parametric";
    };
/**
 * Numeric schema revision for contracts whose sole supported revision is v1.
 */
export type SchemaVersion = number;
/**
 * Errors surfaced when an envelope is invalid or calibration fails.
 */
export type EnvelopeError =
  | {
      /**
       * Identifiers available at the time the step would run.
       */
      available: string[];
      kind: "missing_dependency";
      /**
       * The missing curve/surface identifier referenced by the step.
       */
      missing_id: string;
      /**
       * Kind of the missing dependency (e.g. `"discount"`, `"surface"`).
       */
      missing_kind: string;
      /**
       * Step identifier.
       */
      step_id: string;
      /**
       * Zero-based index of the offending step in `plan.steps`.
       */
      step_index: number;
      /**
       * Step kind (e.g. `"forward"`, `"hazard"`).
       */
      step_kind: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Defined `quote_set` names in the plan.
       */
      available: string[];
      kind: "undefined_quote_set";
      /**
       * The missing `quote_set` name as referenced by the step.
       */
      ref_name: string;
      /**
       * Step identifier.
       */
      step_id: string;
      /**
       * Zero-based index of the offending step.
       */
      step_index: number;
      /**
       * Closest-match suggestion (Levenshtein distance ≤ 3), if any.
       */
      suggestion?: string | null;
      [k: string]: unknown;
    }
  | {
      /**
       * Zero-based index of the conflicting declaration.
       */
      duplicate_index: number;
      /**
       * Zero-based index of the first declaration.
       */
      first_index: number;
      kind: "duplicate_step_id";
      /**
       * Duplicated step identifier.
       */
      step_id: string;
      [k: string]: unknown;
    }
  | {
      /**
       * Iterations performed before termination.
       */
      iterations: number;
      kind: "solver_not_converged";
      /**
       * Largest absolute residual at termination.
       */
      max_residual: number;
      /**
       * Step identifier.
       */
      step_id: string;
      /**
       * Configured solver tolerance.
       */
      tolerance: number;
      /**
       * Identifier of the worst-fitting quote, if known.
       */
      worst_quote_id?: string | null;
      /**
       * Residual of the worst-fitting quote, if known.
       */
      worst_quote_residual?: number | null;
      [k: string]: unknown;
    }
  | {
      kind: "quote_data_invalid";
      /**
       * Quote identifier that failed validation.
       */
      quote_id: string;
      /**
       * Human-readable reason describing the validation failure.
       */
      reason: string;
      /**
       * Step identifier consuming the quote.
       */
      step_id: string;
      [k: string]: unknown;
    }
  | {
      /**
       * `"quote"` (shared namespace for the eight `*_quote` variants) or
       * the specific datum kind name for non-quote variants.
       *
       * Renamed to `datum_kind` in the Rust struct because the enum's serde
       * tag is already named `kind`; the JSON payload uses `datum_kind`.
       */
      datum_kind: string;
      /**
       * The duplicated identifier.
       */
      id: string;
      kind: "duplicate_market_datum_id";
      [k: string]: unknown;
    }
  | {
      /**
       * The unresolved quote identifier.
       */
      id: string;
      kind: "quote_id_not_in_market_data";
      /**
       * The named quote set in `plan.quote_sets`.
       */
      quote_set: string;
      [k: string]: unknown;
    }
  | {
      kind: "quote_set_conflict";
      /**
       * The conflicting quote-set name.
       */
      quote_set: string;
      [k: string]: unknown;
    }
  | {
      /**
       * The quote identifier attached with conflicting payloads.
       */
      id: string;
      kind: "conflicting_market_datum";
      [k: string]: unknown;
    }
  | {
      /**
       * Structured contract diagnostics retained by the bounded loader;
       * empty when the failure carried no per-pointer findings.
       */
      diagnostics?: StrictLoadDiagnostic[];
      kind: "strict_load";
      /**
       * Bounded parser or semantic-validation summary, including each
       * retained diagnostic's pointer and message.
       */
      message: string;
      [k: string]: unknown;
    }
  | {
      kind: "json_serialize";
      /**
       * Serializer-provided error description.
       */
      message: string;
      /**
       * Payload being serialized, e.g. `"CalibrationValidationReport"`.
       */
      target: string;
      [k: string]: unknown;
    };
/**
 * Single-name CDS par-spread or upfront quote.
 */
export type CdsQuote =
  | {
      /**
       * Convention key (currency + doc clause).
       */
      convention: CdsConventionKey;
      /**
       * Reference entity name.
       */
      entity: string;
      /**
       * Unique identifier for the quote.
       */
      id: QuoteId;
      /**
       * Maturity pillar.
       */
      pillar: Pillar;
      /**
       * Recovery rate assumption (e.g. 0.40).
       */
      recovery_rate: number;
      /**
       * Par spread in basis points (e.g. 100.0).
       */
      spread_bp: number;
      type: "cds_par_spread";
    }
  | {
      /**
       * Convention key.
       */
      convention: CdsConventionKey;
      /**
       * Contractual running coupon in basis points (25.0, 100.0, 500.0 or 1000.0).
       */
      coupon_bp: number;
      /**
       * Reference entity name.
       */
      entity: string;
      /**
       * Unique identifier for the quote.
       */
      id: QuoteId;
      /**
       * Maturity pillar.
       */
      pillar: Pillar;
      /**
       * Recovery rate assumption.
       */
      recovery_rate: number;
      type: "cds_upfront";
      /**
       * Upfront payment percentage of notional (e.g. 0.01 for 1%).
       */
      upfront_pct: number;
    };
/**
 * Standard FX conversion strategies used to hint FX providers.
 *
 * The policy tells a provider *how* the rate will be applied so it can decide
 * between spot, forward, or averaged sources.
 */
export type FxConversionPolicy = "cashflow_date" | "period_end" | "period_average";
/**
 * Zero-coupon or year-on-year inflation swap quote.
 */
export type InflationQuote =
  | {
      inflation_swap: {
        /**
         * Per-instrument conventions
         */
        convention: InflationSwapConventionId;
        /**
         * Unique identifier for the quote.
         */
        id: QuoteId;
        /**
         * Inflation index identifier
         */
        index: string;
        /**
         * Swap maturity
         */
        maturity: valuations.DateWire;
        /**
         * Fixed rate (decimal)
         */
        rate: number;
      };
    }
  | {
      yoy_inflation_swap: {
        /**
         * Instrument-wide conventions
         */
        convention: InflationSwapConventionId;
        /**
         * Payment frequency
         */
        frequency: valuations.Tenor;
        /**
         * Unique identifier for the quote.
         */
        id: QuoteId;
        /**
         * Inflation index identifier
         */
        index: string;
        /**
         * Swap maturity
         */
        maturity: valuations.DateWire;
        /**
         * Fixed rate (decimal)
         */
        rate: number;
      };
    };
/**
 * Canonical tagged market quote.
 */
export type MarketQuote =
  | (
      | {
          class: "rates";
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Rate index identifier (e.g. "USD-SOFR-3M").
           */
          index: valuations.Id;
          /**
           * Maturity pillar; on the wire `{"tenor": {"count": 3, "unit": "months"}}`
           * or `{"date": "2024-01-01"}`.
           */
          pillar: Pillar;
          /**
           * Rate value (decimal).
           */
          rate: number;
          type: "deposit";
          [k: string]: unknown;
        }
      | {
          class: "rates";
          /**
           * End date pillar.
           */
          end: Pillar;
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Rate index identifier.
           */
          index: valuations.Id;
          /**
           * Rate value (decimal).
           */
          rate: number;
          /**
           * Start date pillar.
           */
          start: Pillar;
          type: "fra";
          [k: string]: unknown;
        }
      | {
          class: "rates";
          /**
           * Future contract identifier (e.g. "CME:SR3").
           */
          contract: IrFutureContractId;
          /**
           * Convexity adjustment as a decimal rate (Hull convention).
           *
           * The implied forward is
           * `forward = (100 - price) / 100 − convexity_adjustment`.
           * A positive adjustment lowers the futures-implied rate toward the true
           * forward. Callers that want no adjustment must pass `0.0` explicitly.
           */
          convexity_adjustment: number;
          /**
           * Last trading date of the future.
           *
           * The convention registry derives the underlying reference period from
           * this date. For in-arrears IMM contracts such as `CME:SR3`, pass the
           * business day before the ending IMM Wednesday, not the named contract
           * month's starting IMM date.
           */
          expiry: valuations.DateWire;
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Price of the future (e.g. 98.50).
           */
          price: number;
          type: "futures";
          [k: string]: unknown;
        }
      | {
          class: "rates";
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Rate index identifier (floating leg).
           */
          index: valuations.Id;
          /**
           * Maturity pillar of the swap.
           */
          pillar: Pillar;
          /**
           * Fixed rate (decimal) making the swap PV=0.
           */
          rate: number;
          /**
           * Optional spread over the index in decimal format (e.g., 0.0010 for 10 basis points).
           *
           * This spread is added to the floating leg rate. The value is in decimal format
           * and will be converted to basis points internally (multiplied by 10,000).
           */
          spread_decimal?: number | null;
          type: "swap";
          [k: string]: unknown;
        }
    )
  | (
      | {
          class: "cds";
          /**
           * Convention key (currency + doc clause).
           */
          convention: CdsConventionKey;
          /**
           * Reference entity name.
           */
          entity: string;
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Maturity pillar.
           */
          pillar: Pillar;
          /**
           * Recovery rate assumption (e.g. 0.40).
           */
          recovery_rate: number;
          /**
           * Par spread in basis points (e.g. 100.0).
           */
          spread_bp: number;
          type: "cds_par_spread";
          [k: string]: unknown;
        }
      | {
          class: "cds";
          /**
           * Convention key.
           */
          convention: CdsConventionKey;
          /**
           * Contractual running coupon in basis points (25.0, 100.0, 500.0 or 1000.0).
           */
          coupon_bp: number;
          /**
           * Reference entity name.
           */
          entity: string;
          /**
           * Unique identifier for the quote.
           */
          id: QuoteId;
          /**
           * Maturity pillar.
           */
          pillar: Pillar;
          /**
           * Recovery rate assumption.
           */
          recovery_rate: number;
          type: "cds_upfront";
          /**
           * Upfront payment percentage of notional (e.g. 0.01 for 1%).
           */
          upfront_pct: number;
          [k: string]: unknown;
        }
    )
  | {
      /**
       * Attachment point (decimal, e.g. 0.03).
       */
      attachment: number;
      class: "cds_tranche";
      /**
       * Convention key (currency + doc clause).
       */
      convention: CdsConventionKey;
      /**
       * Contractual running coupon of the tranche, in basis points.
       */
      coupon_bp: number;
      /**
       * Detachment point (decimal, e.g. 0.07).
       */
      detachment: number;
      /**
       * Unique identifier.
       */
      id: QuoteId;
      /**
       * Index identifier (e.g. CDX.NA.HY).
       */
      index: string;
      /**
       * Maturity date.
       */
      maturity: valuations.DateWire;
      /**
       * CDS index series number.
       */
      series: number;
      /**
       * Upfront payment as a decimal fraction of tranche notional (e.g., -0.025 for -2.5%).
       */
      upfront_pct: number;
    }
  | (
      | {
          class: "inflation";
          inflation_swap: {
            /**
             * Per-instrument conventions
             */
            convention: InflationSwapConventionId;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * Inflation index identifier
             */
            index: string;
            /**
             * Swap maturity
             */
            maturity: valuations.DateWire;
            /**
             * Fixed rate (decimal)
             */
            rate: number;
          };
          [k: string]: unknown;
        }
      | {
          class: "inflation";
          yoy_inflation_swap: {
            /**
             * Instrument-wide conventions
             */
            convention: InflationSwapConventionId;
            /**
             * Payment frequency
             */
            frequency: valuations.Tenor;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * Inflation index identifier
             */
            index: string;
            /**
             * Swap maturity
             */
            maturity: valuations.DateWire;
            /**
             * Fixed rate (decimal)
             */
            rate: number;
          };
          [k: string]: unknown;
        }
    )
  | (
      | {
          class: "vol";
          option_vol: {
            /**
             * Option expiry
             */
            expiry: valuations.DateWire;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * Option type (Call or Put).
             */
            option_type: OptionType;
            /**
             * Strike
             */
            strike: number;
            /**
             * Underlying identifier
             */
            underlying: valuations.Id;
            /**
             * Implied volatility in decimal units (for example, `0.20` for 20%).
             */
            vol: number;
          };
          [k: string]: unknown;
        }
      | {
          class: "vol";
          swaption_vol: {
            /**
             * Option exercise conventions
             */
            convention: SwaptionConventionId;
            /**
             * Option expiry
             */
            expiry: valuations.DateWire;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * Underlying swap maturity date
             */
            maturity: valuations.DateWire;
            /**
             * Volatility quoting convention.
             */
            quote_type: VolQuoteType;
            /**
             * Strike rate
             */
            strike: number;
            /**
             * Implied volatility in canonical decimal units: absolute rate
             * volatility for normal quotes and Black volatility for lognormal quotes.
             */
            vol: number;
          };
          [k: string]: unknown;
        }
      | {
          class: "vol";
          cap_floor_vol: {
            /**
             * Cap/floor maturity or caplet expiry.
             */
            expiry: valuations.DateWire;
            /**
             * Unique identifier for the quote.
             */
            id: QuoteId;
            /**
             * `true` for cap, `false` for floor.
             */
            is_cap: boolean;
            /**
             * Volatility quoting convention.
             */
            quote_type: VolQuoteType;
            /**
             * Strike rate.
             */
            strike: number;
            /**
             * Implied volatility in canonical decimal units: absolute rate
             * volatility for normal quotes and Black volatility for lognormal quotes.
             */
            vol: number;
          };
          [k: string]: unknown;
        }
    )
  | {
      /**
       * Basis spread in basis points on the base-currency leg.
       */
      basis_spread_bp: number;
      class: "xccy";
      /**
       * XCCY pair convention identifier (e.g., `EUR/USD-XCCY`).
       */
      convention: XccyConventionId;
      /**
       * Far-leg maturity pillar; near leg is the convention spot date.
       */
      far_pillar: Pillar;
      /**
       * Unique identifier for the quote.
       */
      id: QuoteId;
      /**
       * Optional spot FX quote (quote currency per 1 unit of base currency).
       */
      spot_fx?: number | null;
    };
/**
 * Deposit, FRA, futures or swap rate quote.
 */
export type RateQuote =
  | {
      /**
       * Unique identifier for the quote.
       */
      id: QuoteId;
      /**
       * Rate index identifier (e.g. "USD-SOFR-3M").
       */
      index: valuations.Id;
      /**
       * Maturity pillar; on the wire `{"tenor": {"count": 3, "unit": "months"}}`
       * or `{"date": "2024-01-01"}`.
       */
      pillar: Pillar;
      /**
       * Rate value (decimal).
       */
      rate: number;
      type: "deposit";
    }
  | {
      /**
       * End date pillar.
       */
      end: Pillar;
      /**
       * Unique identifier for the quote.
       */
      id: QuoteId;
      /**
       * Rate index identifier.
       */
      index: valuations.Id;
      /**
       * Rate value (decimal).
       */
      rate: number;
      /**
       * Start date pillar.
       */
      start: Pillar;
      type: "fra";
    }
  | {
      /**
       * Future contract identifier (e.g. "CME:SR3").
       */
      contract: IrFutureContractId;
      /**
       * Convexity adjustment as a decimal rate (Hull convention).
       *
       * The implied forward is
       * `forward = (100 - price) / 100 − convexity_adjustment`.
       * A positive adjustment lowers the futures-implied rate toward the true
       * forward. Callers that want no adjustment must pass `0.0` explicitly.
       */
      convexity_adjustment: number;
      /**
       * Last trading date of the future.
       *
       * The convention registry derives the underlying reference period from
       * this date. For in-arrears IMM contracts such as `CME:SR3`, pass the
       * business day before the ending IMM Wednesday, not the named contract
       * month's starting IMM date.
       */
      expiry: valuations.DateWire;
      /**
       * Unique identifier for the quote.
       */
      id: QuoteId;
      /**
       * Price of the future (e.g. 98.50).
       */
      price: number;
      type: "futures";
    }
  | {
      /**
       * Unique identifier for the quote.
       */
      id: QuoteId;
      /**
       * Rate index identifier (floating leg).
       */
      index: valuations.Id;
      /**
       * Maturity pillar of the swap.
       */
      pillar: Pillar;
      /**
       * Fixed rate (decimal) making the swap PV=0.
       */
      rate: number;
      /**
       * Optional spread over the index in decimal format (e.g., 0.0010 for 10 basis points).
       *
       * This spread is added to the floating leg rate. The value is in decimal format
       * and will be converted to basis points internally (multiplied by 10,000).
       */
      spread_decimal?: number | null;
      type: "swap";
    };
/**
 * Kind-tagged parameters of one calibration step, flattened into `CalibrationStep`.
 */
export type StepParams =
  | {
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Step-level conventions for pricing and curve time axis.
       */
      conventions?: RatesStepConventions;
      /**
       * Currency of the curve.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the discount curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Extrapolation policy for the curve.
       */
      extrapolation?: ExtrapolationPolicy;
      /**
       * Interpolation style for the constructed discount curve.
       *
       * Defaults to log-linear discount factors, preserving positive discount
       * factors and piecewise-constant continuously compounded forwards.
       * `Linear` remains available only when explicitly requested.
       */
      interpolation?: InterpStyle;
      kind: "discount";
      /**
       * Calibration method to use.
       */
      method?: CalibrationMethod;
      /**
       * Optional separate ID for pricing logic (defaults to curve_id).
       */
      pricing_discount_id?: valuations.Id | null;
      /**
       * Optional forward curve ID for pricing (if needed).
       */
      pricing_forward_id?: valuations.Id | null;
    }
  | {
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Step-level conventions for pricing and curve time axis.
       */
      conventions?: RatesStepConventions;
      /**
       * Currency of the curve.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the forward curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Identifier for the discount curve to use.
       */
      discount_curve_id: valuations.Id;
      /**
       * Interpolation style for the constructed forward curve.
       *
       * Defaults to Hagan-West monotone-convex interpolation for a smooth,
       * shape-preserving forward term structure. `Linear` remains explicitly
       * opt-in.
       */
      interpolation?: InterpStyle;
      kind: "forward";
      /**
       * Calibration method to use.
       *
       * Forward curves require [`CalibrationMethod::GlobalSolve`]. Sequential
       * bootstrap is rejected because contractual off-grid reset intervals
       * couple adjacent rates through projection discount factors.
       */
      method?: CalibrationMethod;
      /**
       * Tenor in years for the forward curve.
       */
      tenor_years: number;
    }
  | {
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Optional CDS valuation convention used by synthetic CDS instruments
       * during hazard calibration and rebootstrap.
       */
      cds_valuation_convention?: CdsValuationConvention | null;
      /**
       * Currency of the curve.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the hazard curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Identifier for the discount curve to use.
       */
      discount_curve_id: valuations.Id;
      /**
       * Optional CDS documentation-clause assertion.
       *
       * Hazard schedule conventions come from the quote `CdsConventionKey`. When
       * this field is set, it must be consistent with those quote-derived
       * conventions (same clause or the matching regional family).
       */
      doc_clause?: string | null;
      /**
       * Entity name.
       */
      entity: string;
      /**
       * Interpolation style for survival probabilities between pillars.
       *
       * Only log-linear survival interpolation is supported, preserving the
       * piecewise-constant hazard representation. Other styles are rejected
       * before calibration starts.
       */
      interpolation?: InterpStyle;
      kind: "hazard";
      /**
       * Calibration method to use.
       */
      method?: CalibrationMethod;
      /**
       * Notional used to price synthetic CDS instruments during calibration.
       *
       * Calibration normalizes residuals by notional, so this is typically left as
       * the unit-notional default unless you have a specific reason to change it.
       */
      notional?: number;
      /**
       * Interpolation method for par spreads reported by the calibrated curve.
       *
       * Note: this is used for *quoting/interpolation of stored par spreads* and does not affect
       * survival no-arbitrage, which is enforced via non-negative hazards together with the
       * required log-linear survival interpolation.
       */
      par_interp?: ParInterp;
      /**
       * Required recovery-rate assumption as a decimal fraction in `[0, 1]`.
       */
      recovery_rate: number;
      /**
       * Seniority of the debt.
       */
      seniority: Seniority;
    }
  | {
      /**
       * Base CPI level used as the curve's reference CPI at t=0.
       *
       * This is the contractual reference CPI at the start date after applying
       * observation lag and monthly interpolation. Its curve date is
       * `base_date - observation_lag`. Supplied index observations must reproduce
       * this value, including seasonality; a mismatch is rejected.
       */
      base_cpi: number;
      /**
       * Valuation and calibration-instrument start date. The output curve's
       * zero-time reference CPI date is this date minus `observation_lag`.
       */
      base_date: valuations.DateWire;
      /**
       * Currency of the curve.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the inflation curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Identifier for the discount curve to use.
       */
      discount_curve_id: valuations.Id;
      /**
       * Reference index (e.g. "USA-CPI-U").
       */
      index: string;
      /**
       * Interpolation style for the curve.
       */
      interpolation?: InterpStyle;
      kind: "inflation";
      /**
       * Calibration method to use.
       */
      method?: CalibrationMethod;
      /**
       * Notional used to price synthetic inflation swaps during calibration.
       *
       * Calibration normalizes residuals by notional, so this is typically left as
       * the unit-notional default unless you have a specific reason to change it.
       */
      notional?: number;
      /**
       * Observation lag (e.g. "3M").
       *
       * Overrides the quote convention's lag and must match any supplied index
       * lag. The same lag determines the output curve's reference-date origin and the dates
       * of the CPI observations consumed by calibration instruments.
       */
      observation_lag: string;
      /**
       * Optional seasonal adjustment factors for deseasonalizing CPI observations.
       *
       * When provided, the calibrator will:
       * 1. Deseasonalize input CPI levels using the monthly factors
       * 2. Fit the smooth zero-coupon curve to deseasonalized levels
       * 3. Reseasonalize the output CPI path
       *
       * Monthly adjustments are additive to log CPI level. They should approximately
       * sum to zero over 12 months.
       */
      seasonal_factors?: SeasonalFactors | null;
    }
  | {
      /**
       * Base date for the surface.
       */
      base_date: valuations.DateWire;
      /**
       * SABR Beta parameter.
       */
      beta?: number;
      /**
       * Discount curve ID.
       */
      discount_curve_id?: valuations.Id | null;
      /**
       * Optional dividend yield override.
       */
      dividend_yield_override?: number | null;
      /**
       * Extrapolation policy for total-variance fill across expiries.
       *
       * After each quoted expiry is calibrated, the published expiry×strike
       * grid is filled by interpolating total variance `w = σ²T` in expiry
       * (not by interpolating SABR α/ν/ρ and evaluating Hagan at the target
       * `T`). This policy controls targets that fall outside the calibrated
       * expiry range: `Error` rejects them; `Clamp` holds the nearest slice's
       * total variance flat.
       */
      expiry_extrapolation?: SurfaceExtrapolationPolicy;
      kind: "vol_surface";
      /**
       * Volatility model used for calibration.
       */
      model: VolSurfaceModel;
      /**
       * Optional spot price override.
       */
      spot_override?: number | null;
      /**
       * Target expiries for calibration.
       */
      target_expiries?: number[];
      /**
       * Target strikes for calibration.
       */
      target_strikes?: number[];
      /**
       * Identifier for the underlying instrument.
       */
      underlying_ticker: string;
      /**
       * Identifier for the volatility surface being built.
       */
      vol_surface_id: string;
    }
  | {
      /**
       * Allow deterministic fallbacks when SABR grid corners are missing during interpolation.
       *
       * If `false` (default), missing corner buckets are treated as an error instead of
       * silently substituting a nearby bucket.
       */
      allow_sabr_missing_bucket_fallback?: boolean;
      /**
       * Base date for the calibration.
       */
      base_date: valuations.DateWire;
      /**
       * Optional calendar identifier for date adjustments.
       */
      calendar_id?: string | null;
      /**
       * Currency for the swaption surface.
       */
      currency: valuations.Currency;
      /**
       * Discount curve identifier for pricing.
       */
      discount_curve_id: valuations.Id;
      /**
       * Optional day count convention for fixed leg calculations.
       */
      fixed_day_count?: valuations.DayCount | null;
      /**
       * Optional forward curve identifier (if different from discount curve).
       */
      forward_id?: string | null;
      kind: "swaption_vol";
      /**
       * SABR beta parameter (typically 0.0 for normal, 1.0 for lognormal).
       */
      sabr_beta?: number;
      /**
       * Extrapolation policy used when interpolating SABR parameters across the
       * expiry–tenor grid for target points that do not have a directly calibrated bucket.
       */
      sabr_extrapolation?: SurfaceExtrapolationPolicy;
      /**
       * SABR parameter interpolation method between expiries/tenors.
       */
      sabr_interpolation?: SabrInterpolationMethod;
      /**
       * Optional floating index identifier used to resolve market swap conventions.
       *
       * Swaption forward/par rate calculations require swap schedule conventions (fixed frequency,
       * day count, calendar, BDC) that are now indexed off a rate index conventions registry.
       *
       * If omitted, individual swaption quotes must provide `float_leg_conventions.index`.
       */
      swap_index?: valuations.Id | null;
      /**
       * Target expiry times (in years) for the surface grid.
       */
      target_expiries?: number[];
      /**
       * Target tenor times (in years) for the surface grid.
       */
      target_tenors?: number[];
      /**
       * Volatility quoting convention (normal or lognormal).
       */
      vol_convention?: SwaptionVolConvention;
      /**
       * Identifier for the volatility surface.
       */
      vol_surface_id: string;
      /**
       * Maximum absolute error of any fitted volatility quote; defaults to 0.0015.
       *
       * This is distinct from `plan.settings.tolerance` (solver tolerance). For swaption-vol
       * calibration, success should reflect whether the fitted smile residuals are within a
       * market-appropriate tolerance (e.g., 10–20 normal vol bp), not machine epsilon.
       * Normal quotes use decimal rate per square-root year; Black quotes use
       * dimensionless annual volatility. Errors are never scaled by vega.
       */
      vol_tolerance?: number | null;
    }
  | {
      /**
       * Base date for the calibration.
       */
      base_date: valuations.DateWire;
      /**
       * Business day convention for synthetic tranche schedule adjustments.
       */
      business_day_convention?: valuations.BusinessDayConvention | null;
      /**
       * Optional calendar identifier for schedule generation and date adjustments.
       */
      calendar_id?: string | null;
      /**
       * Currency used for synthetic tranche pricing.
       */
      currency: valuations.Currency;
      /**
       * Day count convention for synthetic tranche premium accrual.
       */
      day_count?: valuations.DayCount | null;
      /**
       * Detachment points (as percentages) for the tranches.
       */
      detachment_points?: number[];
      /**
       * Discount curve identifier for pricing.
       */
      discount_curve_id: valuations.Id;
      /**
       * Payment frequency for synthetic tranches (e.g., quarterly).
       */
      frequency?: valuations.Tenor | null;
      /**
       * Credit index identifier (e.g., CDX, iTraxx).
       */
      index_id: string;
      kind: "base_correlation";
      /**
       * Maturity of the tranches in years.
       */
      maturity_years: number;
      /**
       * Notional used to price synthetic tranches during calibration.
       *
       * Calibration can be expressed in upfront % terms, so this is typically left
       * as unit-notional unless you have a specific reason to change it.
       */
      notional?: number;
      /**
       * Coupon roll-date grid for the synthetic tranches: `cds_imm` for the
       * standard CDS roll dates (20th of Mar/Jun/Sep/Dec) or `none` (the
       * default) for a schedule generated from the convention frequency and
       * stub. `imm` is rejected.
       */
      roll_rule?: RollRule;
      /**
       * Series number of the credit index.
       */
      series: number;
    }
  | {
      /**
       * Identifier for the pre-calibrated base correlation curve.
       */
      base_correlation_curve_id: string;
      /**
       * Market-implied flat correlation for the tranche.
       */
      correlation?: number;
      /**
       * Feasible domain for `df` as `(lower_bound, upper_bound)`.
       *
       * `df` must be > 2 for finite variance. Typical range: `(2.1, 50.0)`.
       *
       * @minItems 2
       * @maxItems 2
       */
      df_bounds?: [unknown, unknown];
      /**
       * Discount curve identifier used to price the tranche.
       *
       * When omitted, calibration falls back to the only discount curve present
       * in the market context as a convenience default.
       */
      discount_curve_id?: valuations.Id | null;
      /**
       * Starting guess for degrees of freedom (typically 4-10).
       */
      initial_df?: number;
      kind: "student_t";
      /**
       * Identifier for the reference tranche instrument.
       */
      tranche_instrument_id: string;
    }
  | {
      /**
       * Base date for the calibration.
       */
      base_date: valuations.DateWire;
      /**
       * Currency for conventions.
       */
      currency: valuations.Currency;
      /**
       * Discount curve ID (must already exist in market context).
       */
      curve_id: valuations.Id;
      /**
       * Required positive maximum implied-quote error in quoted volatility units.
       * Normal quotes use decimal rate volatility; Black quotes use relative volatility.
       * This acceptance budget is independent of the numerical solver tolerance.
       */
      fit_tolerance: number;
      /**
       * Optional initial guess for mean reversion κ.
       */
      initial_kappa?: number | null;
      /**
       * Optional initial guess for short rate vol σ.
       */
      initial_sigma?: number | null;
      kind: "hull_white";
    }
  | {
      /**
       * Base date for the calibration.
       */
      base_date: valuations.DateWire;
      /**
       * Currency for conventions.
       */
      currency: valuations.Currency;
      /**
       * Discount curve ID (must already exist in market context).
       */
      discount_curve_id: valuations.Id;
      /**
       * Required positive maximum implied-quote error in quoted volatility units.
       * Normal quotes use decimal rate volatility; Black quotes use relative volatility.
       * This acceptance budget is independent of the numerical solver tolerance.
       */
      fit_tolerance: number;
      /**
       * Optional source mean reversion κ. Required for one-quote calibration.
       */
      fixed_kappa?: number | null;
      /**
       * Forward/projection curve ID. If equal to `discount_curve_id`, the
       * discount curve is used as the single-curve projection proxy.
       */
      forward_curve_id: valuations.Id;
      /**
       * Optional initial guess for mean reversion κ when solving both κ and σ.
       */
      initial_kappa?: number | null;
      /**
       * Optional initial guess for short-rate volatility σ when solving both κ and σ.
       */
      initial_sigma?: number | null;
      kind: "cap_floor_hull_white";
      /**
       * Payment frequency used to decompose quoted caps/floors into caplets.
       */
      payment_frequency?: SwapFrequency;
      /**
       * Scalar or expiry-bootstraped piecewise short-rate volatility calibration.
       */
      volatility_mode?: HullWhiteVolatilityMode;
    }
  | {
      /**
       * Base date for the surface.
       */
      base_date: valuations.DateWire;
      /**
       * Discount curve ID (optional).
       */
      discount_curve_id?: valuations.Id | null;
      /**
       * Optional continuous dividend yield; defaults to the market scalar
       * `"<underlying_ticker>-DIVYIELD"` or zero.
       */
      dividend_yield_override?: number | null;
      kind: "svi_surface";
      /**
       * Optional spot price override.
       */
      spot_override?: number | null;
      /**
       * Target expiries for calibration.
       */
      target_expiries?: number[];
      /**
       * Target strikes for calibration.
       */
      target_strikes?: number[];
      /**
       * Underlying instrument ticker.
       */
      underlying_ticker: string;
      /**
       * Identifier for the volatility surface being built.
       */
      vol_surface_id: string;
    }
  | {
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Optional ID for the byproduct basis spread curve.
       */
      basis_spread_curve_id?: valuations.Id | null;
      /**
       * Step-level conventions for pricing and curve time axis.
       */
      conventions?: RatesStepConventions;
      /**
       * Foreign currency being calibrated.
       */
      currency: valuations.Currency;
      /**
       * Identifier for the foreign discount curve being built.
       */
      curve_id: valuations.Id;
      /**
       * Identifier for the pre-calibrated domestic discount curve.
       */
      domestic_discount_id: valuations.Id;
      /**
       * Extrapolation policy for the foreign curve.
       */
      extrapolation?: ExtrapolationPolicy;
      /**
       * T+0 cash FX rate (domestic per foreign), used when a quote omits `spot_fx`.
       *
       * Covered-interest parity from today needs the cash FX, not the screen
       * "spot". Market spot is T+2 for most G10 pairs and T+1 for USD/CAD;
       * convert to T+0 using ON/TN points before passing the rate here.
       * Mixing T+2 screen spot with T+0 discounting biases long-tenor basis
       * by roughly 1–2 bp.
       */
      fx_spot: number;
      /**
       * Interpolation style for the constructed foreign discount curve.
       *
       * Defaults to log-linear discount factors. `Linear` remains available
       * only when explicitly requested.
       */
      interpolation?: InterpStyle;
      kind: "xccy_basis";
      /**
       * Calibration method to use.
       */
      method?: CalibrationMethod;
    }
  | {
      /**
       * Base date for the curve.
       */
      base_date: valuations.DateWire;
      /**
       * Identifier for the single discount curve being fitted. All calibration
       * instruments use this curve for discounting and implied projection.
       */
      curve_id: valuations.Id;
      /**
       * Optional initial parameter guesses.
       */
      initial_params?: NelsonSiegelModel | null;
      kind: "parametric";
      /**
       * Nelson-Siegel variant (NS or NSS).
       */
      model: NsVariant;
    };
/**
 * Option, swaption or cap/floor volatility quote.
 */
export type VolQuote =
  | {
      option_vol: {
        /**
         * Option expiry
         */
        expiry: valuations.DateWire;
        /**
         * Unique identifier for the quote.
         */
        id: QuoteId;
        /**
         * Option type (Call or Put).
         */
        option_type: OptionType;
        /**
         * Strike
         */
        strike: number;
        /**
         * Underlying identifier
         */
        underlying: valuations.Id;
        /**
         * Implied volatility in decimal units (for example, `0.20` for 20%).
         */
        vol: number;
      };
    }
  | {
      swaption_vol: {
        /**
         * Option exercise conventions
         */
        convention: SwaptionConventionId;
        /**
         * Option expiry
         */
        expiry: valuations.DateWire;
        /**
         * Unique identifier for the quote.
         */
        id: QuoteId;
        /**
         * Underlying swap maturity date
         */
        maturity: valuations.DateWire;
        /**
         * Volatility quoting convention.
         */
        quote_type: VolQuoteType;
        /**
         * Strike rate
         */
        strike: number;
        /**
         * Implied volatility in canonical decimal units: absolute rate
         * volatility for normal quotes and Black volatility for lognormal quotes.
         */
        vol: number;
      };
    }
  | {
      cap_floor_vol: {
        /**
         * Cap/floor maturity or caplet expiry.
         */
        expiry: valuations.DateWire;
        /**
         * Unique identifier for the quote.
         */
        id: QuoteId;
        /**
         * `true` for cap, `false` for floor.
         */
        is_cap: boolean;
        /**
         * Volatility quoting convention.
         */
        quote_type: VolQuoteType;
        /**
         * Strike rate.
         */
        strike: number;
        /**
         * Implied volatility in canonical decimal units: absolute rate
         * volatility for normal quotes and Black volatility for lognormal quotes.
         */
        vol: number;
      };
    };

/**
 * Global configuration for the calibration subsystem.
 *
 * This struct consolidates all settings for solvers, validation, and market-regime
 * specific bounds. It is typically derived from a `FinstackConfig` extension section.
 * Public callers should treat it as the behavioral contract for calibration
 * execution policy: solver choice, convergence settings, validation thresholds,
 * rate-bound policy, and curve-specific numerical guardrails.
 *
 * # Tolerance Semantics
 *
 * Calibration involves two distinct tolerance concepts:
 *
 * 1. **Solver Tolerance** ([`solver.tolerance()`](SolverConfig::tolerance)):
 *    Controls when the numerical solver (Brent/Newton) terminates. This is an
 *    algorithmic convergence criterion in x-space (parameter space). The solver
 *    stops when successive parameter estimates differ by less than this tolerance.
 *
 * 2. **Validation Tolerance** (e.g., [`discount_curve.validation_tolerance`](DiscountCurveSolveConfig::validation_tolerance)
 *    for PV-per-notional curve residuals, or [`vol_surface.validation_tolerance`](VolSurfaceSolveConfig::validation_tolerance)
 *    for decimal implied-vol residuals):
 *    Controls whether calibration is considered *successful*. After the solver
 *    converges, the final residuals are compared against this tolerance. If any
 *    residual exceeds `validation_tolerance`, the calibration is marked as failed
 *    even if the solver converged.
 *
 * **Why two tolerances?**
 * - Solver tolerance ensures numerical convergence but doesn't guarantee economic fit.
 * - Validation tolerance ensures the calibrated curve actually prices instruments correctly.
 * - For well-behaved problems, solver tolerance of `1e-12` with validation tolerance of
 *   `1e-8` works well: the solver finds a precise root, and we verify it prices accurately.
 *
 * # Configuration Hierarchy
 *
 * Settings can be specified at multiple levels with the following precedence:
 *
 * 1. **Step-level** (`CalibrationStep.params.method`): Per-instrument-type overrides
 * 2. **Plan-level** (`CalibrationPlan.settings`): Plan-wide defaults
 * 3. **Finstack config extensions** (`calibration.config.v1`): application defaults
 * 4. **Global defaults** (`CalibrationConfig::default()`): fallback values
 *
 * Step-level settings always take precedence over plan-level settings.
 * In other words, this struct provides default policy, but explicit plan steps
 * remain authoritative when both are supplied.
 *
 * # References
 *
 * - Multi-curve construction context: `docs/REFERENCES.md#andersen-piterbarg-interest-rate-modeling`
 * - Curve interpolation context: `docs/REFERENCES.md#hagan-west-monotone-convex`
 */
export interface CalibrationConfig {
  /**
   * High-level calibration method (bootstrap vs global solve).
   *
   * **Note**: When using the plan-driven API, this field is typically overwritten
   * by the step-level `params.method` for each calibration step. The step-level
   * method always takes precedence. This field serves as runtime state passed
   * from calibration targets to the underlying solvers.
   */
  calibration_method?: CalibrationMethod;
  /**
   * Whether to compute detailed calibration diagnostics (condition number,
   * per-quote quality metrics, singular values, R-squared, etc.).
   *
   * When `true`, the solver will perform additional post-solve analysis
   * including Jacobian-based condition number estimation. This adds
   * computational overhead and should typically be disabled in production
   * hot paths but enabled for calibration debugging, auditing, or
   * quality monitoring.
   *
   * Default: `false`.
   */
  compute_diagnostics?: boolean;
  /**
   * Discount-curve specific solver configuration.
   */
  discount_curve?: DiscountCurveSolveConfig;
  /**
   * When `true`, a calibration step whose solver reports
   * `report.success == false` is propagated as a
   * `finstack_quant_core::Error::Calibration` and its output is **not**
   * installed into the market context.
   *
   * Defaults to `true` — this is the safe production choice because
   * a non-converged solver would otherwise silently poison downstream
   * pricing. Diagnostic workflows that want to inspect the
   * report without aborting can set this to `false`.
   */
  fail_on_bad_fit?: boolean;
  /**
   * Forward-curve specific solver configuration.
   */
  forward_curve?: ForwardCurveSolveConfig;
  /**
   * FX matrix runtime config (pivot currency, triangulation, cache capacity).
   */
  fx?: FxConfig;
  /**
   * Hazard-curve specific solver configuration.
   */
  hazard_curve?: HazardCurveSolveConfig;
  /**
   * Optional market-data hierarchy snapshot.
   */
  hierarchy?: MarketDataHierarchy | null;
  /**
   * Inflation-curve specific solver configuration.
   */
  inflation_curve?: InflationCurveSolveConfig;
  /**
   * Snapshot timestamp, maximum age, and selected quote side.
   */
  market_freshness?: MarketFreshnessPolicy;
  /**
   * Rate bounds for forward/zero rate calibration (when policy is `Explicit`).
   */
  rate_bounds?: RateBounds;
  /**
   * Policy for selecting rate bounds (explicit vs currency-derived).
   */
  rate_bounds_policy?: RateBoundsPolicy;
  /**
   * Solver configuration including numerical method (e.g., Brent) and parameters (tolerance, iterations).
   */
  solver?: SolverConfig;
  /**
   * Use parallel processing when available (e.g., for independent curves).
   */
  use_parallel?: boolean;
  /**
   * Validation configuration with thresholds and quality checks.
   */
  validation?: ValidationConfig;
  /**
   * Runtime validation mode (warnings vs errors).
   */
  validation_mode?: ValidationMode;
  /**
   * Enable verbose logging of the calibration process.
   */
  verbose?: boolean;
  /**
   * Volatility-surface specific solver configuration (SABR and SVI).
   */
  vol_surface?: VolSurfaceSolveConfig;
}
/**
 * Discount-curve specific numerical solver configuration.
 *
 * Controls the search space and numerical stability of the discount curve
 * bootstrapping or global solve process.
 *
 * # Invariants
 * - `df_hard_min` > 0
 * - `scan_grid_points` > 0
 */
export interface DiscountCurveSolveConfig {
  /**
   * Override final-curve monotonicity enforcement (None = policy-driven).
   */
  allow_non_monotonic_final?: boolean | null;
  /**
   * Whether to use a sequential bootstrap to seed a global solve.
   */
  bootstrap_seed_global_solve?: boolean;
  /**
   * Absolute maximum allowed discount factor (prevents divergence).
   */
  df_hard_max?: number;
  /**
   * Absolute minimum allowed discount factor (prevents singularity).
   */
  df_hard_min?: number;
  /**
   * Step size (h) for finite-difference Jacobian calculation.
   */
  jacobian_step_size?: number;
  /**
   * Minimum required success points in scan before attempting polish.
   */
  min_scan_grid_points?: number;
  /**
   * Minimum time threshold for considering a knot at spot (t=0).
   */
  min_t_spot?: number;
  /**
   * Number of points in the initial geometric scan grid.
   */
  scan_grid_points?: number;
  /**
   * Initial step size for geometric scan grid.
   */
  scan_grid_step?: number;
  /**
   * Tolerance for determining calibration *success* (applied to residuals).
   *
   * After the solver converges, the final residuals are compared against this
   * tolerance. If `max_residual > validation_tolerance`, the calibration report
   * will have `success = false` even if the solver converged.
   *
   * This is distinct from `solver.tolerance()` which controls when the numerical
   * solver terminates. See [`CalibrationConfig`] for a full explanation.
   *
   * Default: `1e-8` (suitable for per-unit-notional residuals).
   */
  validation_tolerance?: number;
  /**
   * Weighting scheme for global solve residuals.
   */
  weighting_scheme?: ResidualWeightingScheme;
}
/**
 * Forward-curve specific numerical solver configuration.
 *
 * Controls residual weighting and post-solve success tolerance for
 * projection-curve global solves. Defaults match the values previously
 * borrowed from [`DiscountCurveSolveConfig`].
 */
export interface ForwardCurveSolveConfig {
  /**
   * Tolerance for determining calibration *success* (applied to residuals).
   *
   * After the solver converges, the final residuals are compared against this
   * tolerance. If `max_residual > validation_tolerance`, the calibration report
   * will have `success = false` even if the solver converged.
   *
   * This is distinct from `solver.tolerance()` which controls when the numerical
   * solver terminates. See [`CalibrationConfig`] for a full explanation.
   *
   * Default: `1e-8` (suitable for per-unit-notional residuals).
   */
  validation_tolerance?: number;
  /**
   * Weighting scheme for global solve residuals.
   */
  weighting_scheme?: ResidualWeightingScheme;
}
/**
 * Configuration for [`FxMatrix`](crate::money::fx::FxMatrix) behaviour.
 *
 * Controls triangulation and caching.
 */
export interface FxConfig {
  /**
   * Maximum number of provider-observed quotes retained in the LRU cache.
   */
  cache_capacity?: number;
  /**
   * Whether to enable automatic triangulation for missing rates.
   *
   * When enabled, the matrix will attempt `from -> pivot -> to` and will not
   * search multi-hop paths or alternative pivots.
   */
  enable_triangulation?: boolean;
  /**
   * Pivot currency for triangulation fallback (typically USD).
   *
   * Current limitation: triangulation uses this single configured pivot only.
   * If a market convention requires a different routing currency for a given
   * pair, callers must seed that direct quote explicitly instead of relying on
   * automatic cross construction.
   */
  pivot_currency?: valuations.Currency;
}
/**
 * Hazard-curve specific numerical solver configuration.
 *
 * Controls the search space and numerical stability of hazard curve
 * bootstrapping or global solve for credit curve calibration.
 *
 * # Invariants
 * - `hazard_hard_min` >= 0 (hazard rates must be non-negative)
 * - `hazard_hard_max` > `hazard_hard_min`
 */
export interface HazardCurveSolveConfig {
  /**
   * Maximum allowed hazard rate.
   *
   * The default of 10.0 corresponds to roughly 99.995% 1Y default probability.
   * For distressed/distressed sovereign scenarios, increase to 50.0 or 100.0.
   */
  hazard_hard_max?: number;
  /**
   * Minimum allowed hazard rate (must be non-negative for survival monotonicity).
   */
  hazard_hard_min?: number;
  /**
   * Tolerance for determining calibration *success* (applied to residuals).
   *
   * After the solver converges, the final residuals are compared against this
   * tolerance. If `max_residual > validation_tolerance`, the calibration report
   * will have `success = false` even if the solver converged.
   *
   * This is distinct from `solver.tolerance()` which controls when the numerical
   * solver terminates. See [`CalibrationConfig`] for a full explanation.
   *
   * Default: `1e-8` (suitable for per-unit-notional residuals).
   */
  validation_tolerance?: number;
  /**
   * Weighting scheme for global solve residuals.
   */
  weighting_scheme?: ResidualWeightingScheme;
}
/**
 * The top-level market data hierarchy containing root nodes.
 *
 * Each root represents a major asset class or category (e.g., "Rates", "Credit",
 * "FX", "Equity", "Volatility"). The hierarchy is fully serializable and can be
 * loaded from JSON configuration files.
 */
export interface MarketDataHierarchy {
  /**
   * Top-level nodes, keyed by their name. The key becomes each node's
   * `name` during deserialization. Insertion order is preserved.
   */
  roots: {
    [k: string]: HierarchyNode;
  };
  [k: string]: unknown;
}
/**
 * A single node in the market data hierarchy tree.
 *
 * Nodes form a tree: each has a name, optional key-value tags for cross-cutting
 * queries, ordered children, and leaf `CurveId` references.
 */
export interface HierarchyNode {
  /**
   * Child nodes, keyed by their name. The key becomes the child's `name`
   * during deserialization. Insertion order is preserved.
   */
  children?: {
    [k: string]: HierarchyNode;
  };
  /**
   * Curves attached directly to this node. Serialized as `curves`.
   */
  curves?: valuations.Id[];
  /**
   * Free-form key/value labels on this node, used for selection and
   * reporting. Insertion order is preserved.
   */
  tags?: {
    [k: string]: string;
  };
}
/**
 * Inflation-curve specific numerical solver configuration.
 *
 * Controls the numerical stability and success criteria of inflation curve
 * bootstrapping or global solve for CPI curve calibration.
 */
export interface InflationCurveSolveConfig {
  /**
   * Maximum allowed CPI level during calibration.
   *
   * Default: `10_000.0`. For hyperinflation currencies (TRY, ARS, VES),
   * increase this bound significantly (e.g. `1e9`).
   */
  cpi_hard_max?: number;
  /**
   * Minimum allowed CPI level during calibration.
   *
   * Default: `1.0`. For indices with different base conventions or rebased
   * indices, adjust accordingly.
   */
  cpi_hard_min?: number;
  /**
   * Tolerance for determining calibration *success* (applied to residuals).
   *
   * After the solver converges, the final residuals are compared against this
   * tolerance. If `max_residual > validation_tolerance`, the calibration report
   * will have `success = false` even if the solver converged.
   *
   * This is distinct from `solver.tolerance()` which controls when the numerical
   * solver terminates. See [`CalibrationConfig`] for a full explanation.
   *
   * Default: `1e-8` (suitable for per-unit-notional residuals).
   */
  validation_tolerance?: number;
  /**
   * Weighting scheme for global solve residuals.
   */
  weighting_scheme?: ResidualWeightingScheme;
}
/**
 * Audit metadata and freshness policy for calibration inputs.
 */
export interface MarketFreshnessPolicy {
  /**
   * Maximum permitted snapshot age in seconds.
   */
  max_age_seconds?: number | null;
  /**
   * Market side represented by quote values in the envelope.
   */
  quote_side?: MarketQuoteSide;
  /**
   * RFC3339 timestamp at which the quote snapshot was captured.
   */
  snapshot_timestamp?: string | null;
}
/**
 * Configurable bounds for forward/zero rates during calibration.
 *
 * Different market regimes require different rate bounds:
 * - Developed markets (USD, EUR, GBP): typically [-2%, 50%]
 * - Negative rate environments (EUR, JPY, CHF): [-5%, 20%]
 * - Emerging markets (TRY, ARS, BRL): [-5%, 200%]
 */
export interface RateBounds {
  /**
   * Maximum allowed rate (decimal, e.g., 0.50 for 50%)
   */
  max_rate: number;
  /**
   * Minimum allowed rate (decimal, e.g., -0.02 for -2%)
   */
  min_rate: number;
}
/**
 * Serializable convergence settings for calibration.
 *
 * Calibration owns its bracket search. This configuration contains only the
 * tolerance and iteration budget consumed by its numerical solvers.
 *
 * Deserialization runs [`SolverConfig::validate`], so a wire document with a
 * non-positive tolerance or a zero iteration budget is rejected at parse time.
 */
export interface SolverConfig {
  /**
   * Maximum iterations available to each solver invocation.
   */
  max_iterations?: number;
  /**
   * Numerical convergence tolerance; distinct from economic fit acceptance.
   */
  tolerance?: number;
}
/**
 * Validation configuration for curve and surface sanity checks.
 *
 * This structure defines the limits for various financial metrics
 * (forward rates, hazard rates, inflation growth) and toggles
 * for specific arbitrage and monotonicity checks.
 */
export interface ValidationConfig {
  /**
   * Allow negative rate environments (DF > 1.0 at short end)
   */
  allow_negative_rates?: boolean;
  /**
   * Butterfly spread convexity tolerance ratio (lower bound).
   * Actual variance must be >= interpolated * this ratio to pass.
   * Default 0.90 (10% tolerance); use values closer to 1.0 for stricter checking.
   */
  butterfly_lower_ratio?: number;
  /**
   * Butterfly spread convexity tolerance ratio (upper bound).
   * Actual variance must be <= interpolated * this ratio to pass.
   * Default 1.10 (10% tolerance); use values closer to 1.0 for stricter checking.
   */
  butterfly_upper_ratio?: number;
  /**
   * Enable arbitrage checks
   */
  check_arbitrage: boolean;
  /**
   * Enable forward rate positivity check
   */
  check_forward_positivity: boolean;
  /**
   * Enable monotonicity checks
   */
  check_monotonicity: boolean;
  /**
   * When true, arbitrage violations (calendar/butterfly) produce warnings instead of errors.
   * Default is false - arbitrage violations fail validation.
   * Set to true only for exploratory analysis or when arbitrage-free fitting is not required.
   */
  lenient_arbitrage?: boolean;
  /**
   * Maximum allowed annual CPI growth (default 0.50 = 50%)
   */
  max_cpi_growth: number;
  /**
   * Maximum allowed forward rate
   */
  max_forward_rate: number;
  /**
   * Maximum allowed forward inflation (default 0.50 = 50%)
   */
  max_fwd_inflation: number;
  /**
   * Maximum allowed hazard rate (default 0.5 = 50%)
   */
  max_hazard_rate: number;
  /**
   * Maximum allowed volatility (default 5.0 = 500%)
   */
  max_volatility: number;
  /**
   * Minimum allowed annual CPI growth (default -0.10 = -10%)
   */
  min_cpi_growth: number;
  /**
   * Minimum allowed forward rate (can be slightly negative)
   */
  min_forward_rate: number;
  /**
   * Minimum allowed forward inflation (default -0.20 = -20%)
   */
  min_fwd_inflation: number;
  /**
   * Minimum LGD denominator used for hazard-rate initial guesses.
   */
  minimum_lgd_for_hazard_guess?: number;
  /**
   * Absolute tolerance for comparing configured and quoted recovery rates.
   */
  recovery_rate_abs_tolerance?: number;
  /**
   * Numerical tolerance for comparisons
   */
  tolerance: number;
}
/**
 * Volatility-surface specific numerical solver configuration.
 *
 * Controls the success criterion for SABR and SVI surface calibration.
 * Residuals are in **decimal implied-vol units** (for example `0.001` is
 * 0.10 vol points), not PV-per-notional.
 */
export interface VolSurfaceSolveConfig {
  /**
   * Tolerance for determining calibration *success* (applied to vol residuals).
   *
   * After each expiry slice is calibrated, the final `|σ_model − σ_mkt|`
   * residuals are compared against this tolerance. If `max_residual >
   * validation_tolerance`, the calibration report will have `success = false`
   * even if the slice solver converged.
   *
   * This is distinct from `solver.tolerance()` which controls when the
   * numerical solver terminates. See [`CalibrationConfig`] for a full
   * explanation.
   *
   * Default: `1e-3` (0.10 vol points in decimal vol).
   */
  validation_tolerance?: number;
}
/**
 * Calibration diagnostics providing condition number, residual analysis, and fit quality.
 *
 * These diagnostics are only populated when `CalibrationConfig::compute_diagnostics`
 * is set to `true`. They are relatively expensive to compute (requiring Jacobian
 * analysis) and are intended for calibration debugging, auditing, and quality
 * monitoring rather than production hot paths.
 */
export interface CalibrationDiagnostics {
  /**
   * Condition number of the Jacobian's normal equations (J^T * J).
   *
   * A high condition number (e.g., > 1e10) indicates an ill-conditioned
   * calibration problem where small changes in market data can produce
   * large changes in calibrated parameters.
   */
  condition_number?: number | null;
  /**
   * Maximum absolute residual across all quotes.
   */
  max_residual: number;
  /**
   * Per-quote quality metrics for each calibration instrument.
   */
  per_quote: QuoteQuality[];
  /**
   * Coefficient of determination (R-squared) for the fit.
   *
   * Values close to 1.0 indicate a good fit. Only meaningful when
   * target values have meaningful variance.
   */
  r_squared?: number | null;
  /**
   * Root mean square residual across all quotes.
   */
  rms_residual: number;
  /**
   * Singular values of the Jacobian matrix (if computed).
   *
   * Useful for diagnosing rank deficiency and understanding which
   * parameter directions are well-determined vs poorly-determined.
   */
  singular_values?: number[] | null;
}
/**
 * Per-quote quality metrics from a calibration run.
 *
 * Captures the fitted vs target values for a single market quote,
 * along with the residual and a local sensitivity measure.
 */
export interface QuoteQuality {
  /**
   * Model-implied fitted value after calibration.
   */
  fitted_value: number;
  /**
   * Human-readable label identifying this quote (e.g., "USD-1Y-SWAP").
   */
  quote_label: string;
  /**
   * Residual (fitted - target) for this quote.
   */
  residual: number;
  /**
   * Local sensitivity: dOutput/dParam (via finite difference or Jacobian diagonal).
   */
  sensitivity: number;
  /**
   * Market-observed target value for this quote.
   */
  target_value: number;
}
/**
 * Canonical typed calibration request and result envelope.
 */
export interface CalibrationEnvelope {
  /**
   * Optional `$schema` URL/path for editor-side JSON Schema discovery.
   * Ignored at runtime; serialized when present.
   */
  $schema?: string | null;
  /**
   * Flat, id-addressable market data inputs.
   */
  market_data?: MarketDatum[];
  /**
   * The calibration plan containing steps and quote-set references.
   */
  plan: CalibrationPlan;
  /**
   * Pre-built calibrated objects from a prior run.
   */
  prior_market?: PriorMarketObject[];
  /**
   * Schema marker; current writers emit [`CALIBRATION_SCHEMA`].
   */
  schema: CalibrationSchema;
}
/**
 * Key to look up CDS conventions (Currency + DocClause).
 *
 * CDS conventions are identified by both currency and documentation clause, as different
 * clauses have different market conventions. Used to look up [`CdsConventionSpec`](crate::market::conventions::defs::CdsConventionSpec)
 * from the convention registry.
 */
export interface CdsConventionKey {
  /**
   * The currency of the CDS.
   */
  currency: valuations.Currency;
  /**
   * The documentation clause (e.g. CR14, MM14).
   */
  doc_clause: CdsDocClause;
  [k: string]: unknown;
}
/**
 * Dividend schedule for an equity or ETF underlying.
 *
 * Contains a time-ordered sequence of dividend events used for pricing equity
 * derivatives and calculating total return. The schedule can be referenced by
 * multiple instruments via its [`CurveId`] in the market context.
 *
 * # Usage in Pricing
 *
 * - **Discrete dividends**: Subtract PV of future dividends from spot for option pricing
 * - **Ex-dividend adjustments**: Reduce forward price by dividend amount
 * - **Dividend futures**: Sum dividends in contract period
 * - **Total return**: Include dividend reinvestment in performance
 */
export interface DividendSchedule {
  /**
   * Quote currency for cash dividends (optional metadata).
   */
  currency?: valuations.Currency | null;
  /**
   * Sorted events by date (ascending).
   */
  events: DividendEvent[];
  /**
   * Unique identifier of this schedule in the market context.
   */
  id: valuations.Id;
  /**
   * Optional display symbol/ticker for convenience.
   */
  underlying?: string | null;
}
/**
 * A dated dividend event.
 */
export interface DividendEvent {
  /**
   * Ex-dividend date.
   */
  date: valuations.DateWire;
  /**
   * Event kind.
   */
  kind: DividendKind;
  [k: string]: unknown;
}
/**
 * Data-only SABR parameters for one volatility-cube node.
 *
 * This type records calibrated market data. SABR evaluation, interpolation,
 * calibration, and convention conversion are owned by
 * `finstack-quant-models`.
 */
export interface SabrParameterData {
  alpha: number;
  beta: number;
  nu: number;
  rho: number;
  shift?: number | null;
}
/**
 * A calibration plan containing quote sets and execution steps.
 *
 * A plan organizes market data into named sets and defines a sequence of
 * [`CalibrationStep`] to be executed.
 */
export interface CalibrationPlan {
  /**
   * Optional human-readable description of the plan's purpose.
   */
  description?: string | null;
  /**
   * Unique identifier for the calibration plan.
   */
  id: string;
  /**
   * Named ID lists; each ID must resolve to a quote in `market_data`.
   */
  quote_sets?: {
    [k: string]: QuoteId[];
  };
  /**
   * Global settings for the calibration process.
   */
  settings?: CalibrationConfig;
  /**
   * Sequence of calibration steps to execute.
   */
  steps: CalibrationStep[];
}
/**
 * Step-level conventions for rates calibration (discount and forward curves).
 *
 * This is a Bloomberg/FinCad-style design: curve construction uses a small set of
 * *step-level* conventions (e.g., curve time-axis day count).
 */
export interface RatesStepConventions {
  /**
   * Day count used to map dates to year fractions for curve knot times.
   */
  curve_day_count?: valuations.DayCount | null;
  /**
   * Optional override for the OIS floating-leg compounding mode used by
   * the calibration's bootstrap-internal swaps.
   *
   * When unset, the bootstrap uses the registered per-index default
   * (e.g. SOFR → `CompoundedInArrears { lookback_days: 0 }`, the cleared
   * OIS plain in-arrears convention).
   * Set this to match a vendor convention that differs from the registry
   * default — e.g. Bloomberg SWPM SOFR uses
   * `CompoundedWithRateCutoff { cutoff_days: 1 }` for the daily-compounded
   * float leg, and a curve calibrated against SWPM screen rates needs to
   * price its bootstrap swaps with the same compounding to bit-match
   * Bloomberg's resulting DFs.
   */
  ois_compounding?: FloatingLegCompounding | null;
}
/**
 * Monthly seasonal adjustment factors for inflation curves.
 *
 * Used to deseasonalize CPI observations before fitting a smooth
 * zero-coupon inflation curve, then reseasonalize the output.
 * Monthly adjustments should approximately sum to zero.
 */
export interface SeasonalFactors {
  /**
   * Monthly adjustment factors (Jan=index 0 through Dec=index 11).
   * These are additive adjustments to the log CPI level.
   *
   * @minItems 12
   * @maxItems 12
   */
  monthly_adjustments: [number, number, number, number, number, number, number, number, number, number, number, number];
}
/**
 * Typed conventions required to replay a rate-curve calibration.
 */
export interface RateCalibrationRecipe {
  /**
   * Currency of the calibrated curve.
   */
  currency: valuations.Currency;
  /**
   * Day count used for the curve's time axis.
   */
  curve_day_count: valuations.DayCount;
  /**
   * Numerical calibration method.
   */
  method: RateCalibrationMethod;
  /**
   * Optional OIS floating-leg compounding override.
   */
  ois_compounding?: RateCalibrationOisCompounding | null;
  /**
   * Complete typed quote set required for exact replay.
   */
  quotes: RateCalibrationQuote[];
  /**
   * Discount/projection role and linked curve identifier.
   */
  role: RateCalibrationCurveRole;
}
/**
 * Exact valuation-layer inputs required to replay a hazard-curve calibration.
 *
 * The core market-data crate stores these payloads without interpreting them;
 * the calibration crate deserializes them back into its canonical
 * `HazardCurveParams`, typed CDS quotes, and `CalibrationConfig`. Keeping the
 * complete serde payloads avoids replacing date pillars with rounded tenors or
 * silently substituting current defaults for the original solver policy.
 */
export interface HazardCalibrationRecipe {
  /**
   * Exact serialized `CalibrationConfig`, including solver and validation policy.
   */
  calibration_config: {
    [k: string]: unknown;
  };
  /**
   * Original par-spread or upfront inputs used to calibrate the curve.
   */
  calibration_inputs: HazardCalibrationInput[];
  /**
   * Exact serialized `HazardCurveParams` used for the original solve.
   */
  hazard_params: {
    [k: string]: unknown;
  };
  /**
   * Par-spread-only inputs used for quote-space spread risk.
   */
  spread_risk_inputs: HazardCalibrationInput[];
  [k: string]: unknown;
}
/**
 * One atomic quote binding retained for hazard calibration replay.
 */
export interface HazardCalibrationInput {
  /**
   * Contractual pillar date resolved from the quote and CDS conventions.
   */
  pillar_date: valuations.DateWire;
  /**
   * Frozen year-fraction pillar time used by the original solve.
   */
  pillar_time: number;
  /**
   * Exact serialized typed CDS quote.
   */
  quote: {
    [k: string]: unknown;
  };
}
/**
 * Detailed report of a calibration exercise.
 *
 * Consolidates success status, residuals, convergence diagnostics, and optional
 * tracing information. Used by the calibration engine to return results and
 * by risk systems to audit calibration quality.
 */
export interface CalibrationReport {
  /**
   * Human-readable reason for convergence or failure.
   */
  convergence_reason: string;
  /**
   * Optional calibration diagnostics (condition number, per-quote quality, etc.).
   *
   * Only populated when `CalibrationConfig::compute_diagnostics` is `true`.
   * Provides detailed information about the quality and stability of the
   * calibration for debugging, auditing, and monitoring purposes.
   */
  diagnostics?: CalibrationDiagnostics | null;
  /**
   * Optional detailed trace of the calibration steps (enabled via config).
   */
  explanation?: ExplanationTrace | null;
  /**
   * Number of solver iterations or function evaluations.
   */
  iterations: number;
  /**
   * Maximum absolute residual across all instruments, in the residual
   * units of the calibrator that produced this report (raw units for a
   * step report; the largest step `max_residual` for a plan report).
   */
  max_residual: number;
  /**
   * Plan-level only: maximum `|residual| / step_tolerance` ratio across
   * every quote of every step. `None` on per-step reports.
   */
  max_residual_ratio?: number | null;
  /**
   * Domain-specific metadata (e.g., "type": "yield_curve").
   */
  metadata: {
    [k: string]: string;
  };
  /**
   * Optional model/methodology version used for this calibration.
   *
   * Used for audit trails and regulatory compliance. Examples:
   * - "ISDA Standard Model v1.8.2" for CDS hazard curve calibration
   * - "Multi-curve OIS discounting" for discount curve calibration
   * - "SABR v1.0" for volatility surface calibration
   */
  model_version?: string | null;
  /**
   * Final objective function value (usually RMSE).
   */
  objective_value: number;
  /**
   * Final residuals (fitting errors) by instrument identifier.
   */
  residuals: {
    [k: string]: number;
  };
  /**
   * Results metadata (timestamp, software version, etc.).
   */
  results_meta?: ResultsMeta;
  /**
   * Root mean square error of all residuals, in the same units as
   * [`Self::max_residual`].
   */
  rmse: number;
  /**
   * Plan-level only: root mean square of `|residual| / step_tolerance`
   * across every quote of every step. `None` on per-step reports.
   */
  rmse_ratio?: number | null;
  /**
   * Solver configuration used during this calibration run.
   */
  solver_config?: SolverConfig;
  /**
   * User-facing success flag. True only if both fitting and validation passed.
   */
  success: boolean;
  /**
   * Configured tolerance used to determine [`Self::success`] from residuals.
   *
   * Set by [`Self::for_type_with_tolerance`]; `None` for reports built via [`Self::new`]
   * directly (no tolerance gate applied). This is the typed source of truth for the
   * success-gate tolerance; it is distinct from `solver_config.tolerance()`, which controls
   * when the numerical root-finder stops.
   */
  success_tolerance?: number | null;
  /**
   * Optional details on validation failures.
   */
  validation_error?: string | null;
  /**
   * Whether the calibrated market object passed all validation checks.
   */
  validation_passed?: boolean;
  /**
   * Identifier of the quote with the largest absolute residual.
   *
   * Derived from `residuals`. `None` only when `residuals` is empty. This
   * is the quote a user should look at first when a step fails to
   * converge — the input most likely to fix.
   */
  worst_quote_id?: string | null;
  /**
   * Signed residual of [`Self::worst_quote_id`].
   */
  worst_quote_residual?: number | null;
}
/**
 * Container for detailed execution traces of financial computations.
 *
 * Traces are organized by type (calibration, pricing, waterfall) and contain
 * a sequence of domain-specific entries. Traces can be serialized to JSON for
 * inspection, debugging, or audit purposes.
 *
 * Mutation is intentionally single-threaded through `&mut self`. If multiple
 * workers need to append to one trace, wrap it in external synchronization and
 * keep ordering semantics explicit at the call site.
 */
export interface ExplanationTrace {
  /**
   * Sequence of trace entries
   */
  entries: TraceEntry[];
  /**
   * Whether the trace was truncated due to size limits
   */
  truncated?: boolean | null;
  /**
   * Type of trace (e.g., "calibration", "pricing", "waterfall")
   */
  type: string;
  [k: string]: unknown;
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
 * Complete calibration result with market snapshot and diagnostics.
 */
export interface CalibrationResult {
  /**
   * Final calibrated market context (all curves, surfaces, scalars, etc.)
   */
  final_market: MarketContextState;
  /**
   * Merged plan-level calibration report.
   */
  report: CalibrationReport;
  /**
   * Results metadata (timestamp, version, rounding context, etc.).
   */
  results_meta: ResultsMeta;
  /**
   * Per-step calibration reports keyed by step id.
   */
  step_reports: {
    [k: string]: CalibrationReport;
  };
}
/**
 * Complete serializable state of a MarketContext.
 *
 * Provides a stable, versioned snapshot of all market data that can be
 * persisted to JSON and reconstructed deterministically.
 */
export interface MarketContextState {
  /**
   * Collateral CSA mappings
   */
  collateral: {
    [k: string]: string;
  };
  /**
   * Credit index aggregates (references curves by ID)
   */
  credit_indices: CreditIndexState[];
  /**
   * All curves (discount, forward, hazard, inflation, base correlation)
   */
  curves: CurveState[];
  /**
   * Dividend schedules
   */
  dividends: DividendSchedule[];
  /**
   * FX matrix state (optional)
   */
  fx?: FxMatrixState | null;
  /**
   * FX delta-quoted volatility surfaces
   */
  fx_delta_vol_surfaces: FxDeltaVolSurface[];
  /**
   * Market data hierarchy snapshot, or `null` when no hierarchy is configured.
   *
   * The key is mandatory but its value is nullable: `deserialize_with` makes
   * serde demand the key, and the struct-level `required` extension keeps it
   * mandatory in the schema without stripping the `null` branch.
   */
  hierarchy: MarketDataHierarchy | null;
  /**
   * Inflation indices
   */
  inflation_indices: InflationIndex[];
  /**
   * Market scalars and prices
   */
  prices: {
    [k: string]: MarketScalar;
  };
  /**
   * Required schema version. Only version `1` is accepted.
   */
  schema_version: SchemaVersion;
  /**
   * Generic time series
   */
  series: ScalarTimeSeries[];
  /**
   * Volatility surfaces
   */
  surfaces: VolSurface[];
  /**
   * SABR volatility cubes
   */
  vol_cubes: VolCube[];
}
/**
 * Serializable state for credit index data.
 *
 * Instead of serializing `Arc<Curve>` directly, we store curve IDs that
 * reference curves present in the `MarketContextState`.
 */
export interface CreditIndexState {
  /**
   * ID of the base correlation curve (must exist in context curves)
   */
  base_correlation_curve_id: string;
  /**
   * Unique identifier for this credit index
   */
  id: string;
  /**
   * ID of the index hazard curve (must exist in context curves)
   */
  index_credit_curve_id: string;
  /**
   * Optional map of issuer ID → hazard curve ID
   */
  issuer_credit_curve_ids?: {
    [k: string]: string;
  } | null;
  /**
   * Optional map of issuer ID → recovery rate
   */
  issuer_recovery_rates?: {
    [k: string]: number;
  } | null;
  /**
   * Optional map of issuer ID → weight
   */
  issuer_weights?: {
    [k: string]: number;
  } | null;
  /**
   * Number of constituents
   */
  num_constituents: number;
  /**
   * Recovery rate
   */
  recovery_rate: number;
}
/**
 * Serializable state of an FxMatrix.
 * Contains the configuration and cached quotes that can be persisted and restored.
 */
export interface FxMatrixState {
  /**
   * Matrix configuration, including pivot and cache capacity.
   */
  config: FxConfig;
  /**
   * Pinned, date/policy-scoped quotes as `(from, to, on, policy, rate)`.
   *
   * Required: a snapshot that omits it would silently re-derive those
   * dates from the provider instead of restoring the pinned fixings.
   */
  pinned_quotes: [unknown, unknown, unknown, unknown, unknown][];
  /**
   * Captured provider quotes, below explicit global and date/policy-pinned
   * quotes in lookup priority. Market-context restoration uses these to
   * rebuild a quote-only provider; arbitrary live provider behavior is not
   * serialized. Required even when the provider has no snapshot quotes.
   */
  provider_quotes: [unknown, unknown, unknown][];
  /**
   * Pair-global quotes as source, target, rate tuples.
   */
  quotes: [unknown, unknown, unknown][];
}
/**
 * Delta-quoted FX volatility surface.
 *
 * Stores market-standard FX vol quotes (ATM DNS, 25-delta risk-reversal,
 * 25-delta butterfly) across multiple expiries. Models-layer functions
 * perform delta conversion and volatility evaluation.
 */
export interface FxDeltaVolSurface {
  /**
   * ATM delta-neutral straddle vols per expiry.
   */
  atm_vols: number[];
  /**
   * Optional 10-delta butterfly per expiry.
   */
  bf_10d?: number[] | null;
  /**
   * 25-delta butterfly per expiry.
   */
  bf_25d: number[];
  /**
   * Expiry times in years.
   */
  expiries: number[];
  /**
   * Surface identifier.
   */
  id: valuations.Id;
  /**
   * Optional 10-delta risk reversal per expiry.
   */
  rr_10d?: number[] | null;
  /**
   * 25-delta risk reversal per expiry.
   */
  rr_25d: number[];
}
/**
 * Inflation index time series with lagging and seasonality.
 *
 * Wraps historical CPI/RPI observations with market-standard conventions for
 * lag application and interpolation. Used for pricing TIPS, linkers, and
 * inflation derivatives.
 *
 * # Components
 *
 * - **Observations**: Historical index levels by publication date
 * - **Interpolation**: Daily interpolation between monthly observations
 * - **Lag**: Publication lag (typically 3 months for TIPS)
 * - **Seasonality**: Optional monthly adjustment factors
 *
 * # Interpolation Methods
 *
 * - **Step**: Last observation carried forward (conservative)
 * - **Linear**: Daily interpolation between months (TIPS standard)
 *
 * # Lag Application
 *
 * Reference index calculation applies lag before interpolation:
 * ```text
 * For settlement date T with 3-month lag:
 * 1. Reference date = T - 3 months
 * 2. Find bracketing CPI observations
 * 3. Interpolate linearly between them
 * ```
 *
 * # Thread Safety
 *
 * Immutable after construction; safe to share via `Arc<InflationIndex>`.
 *
 * # References
 *
 * - **TIPS Mechanics**:
 *   - US Treasury (2024). "TIPS In Depth." treasurydirect.gov. `docs/REFERENCES.md#deacon-derry-mirfendereski-2004`
 *   - Deacon, M., Derry, A., & Mirfendereski, D. (2004). *Inflation-Indexed Securities*
 *     (2nd ed.). Wiley Finance. Chapter 2 (Index-linked bond mechanics). `docs/REFERENCES.md#deacon-derry-mirfendereski-2004`
 *
 * - **Index Lagging**:
 *   - Kerkhof, J. (2005). "Inflation Derivatives Explained." *Journal of Derivatives
 *     Accounting*, 2(1), 1-19. `docs/REFERENCES.md#kerkhof-2005`
 *   - Hurd, M., & Relleen, J. (2006). "Estimating the Inflation Risk Premium."
 *     Bank of England Quarterly Bulletin, Q2 2006. `docs/REFERENCES.md#kerkhof-2005`
 */
export interface InflationIndex {
  /**
   * Currency
   */
  currency: valuations.Currency;
  /**
   * Unique identifier
   */
  id: string;
  /**
   * Interpolation method
   */
  interpolation: InflationInterpolation;
  /**
   * Lag policy
   */
  lag: InflationLag;
  /**
   * Observations as (date, value) pairs
   */
  observations: [unknown, unknown][];
  /**
   * Optional seasonality factors
   *
   * @minItems 12
   * @maxItems 12
   */
  seasonality?: [number, number, number, number, number, number, number, number, number, number, number, number] | null;
}
/**
 * Date-indexed time series with flexible interpolation.
 *
 * Provides lightweight storage for historical or forecast data with step or
 * linear interpolation. Unlike full term structures, this is optimized for
 * sparse, irregularly-spaced observations.
 *
 * # Storage
 *
 * Uses columnar format with parallel arrays:
 * - Dates stored as i32 (days since Unix epoch) for compact size
 * - Values stored as f64
 * - Binary search for O(log n) lookup
 *
 * # Interpolation
 *
 * - **Step**: Last observation carried forward (LOCF)
 * - **Linear**: Linear interpolation between observations
 *
 * # Use Cases
 *
 * - **Economic indicators**: GDP, unemployment rate, PMI
 * - **Credit metrics**: Historical credit spreads, CDS levels
 * - **Commodity fundamentals**: Inventory levels, production data
 * - **Any sparse time series**: Where full curve infrastructure is overkill
 */
export interface ScalarTimeSeries {
  /**
   * Optional currency
   */
  currency?: valuations.Currency | null;
  /**
   * Series identifier
   */
  id: string;
  /**
   * Interpolation method
   */
  interpolation: SeriesInterpolation;
  /**
   * Observations as (date, value) pairs
   */
  observations: [unknown, unknown][];
}
/**
 * Volatility surface defined on expiry × strike grid.
 *
 * Internally stores volatilities in row-major order as a boxed slice.
 */
export interface VolSurface {
  /**
   * Expiry times in years
   */
  expiries: number[];
  /**
   * Surface identifier
   */
  id: string;
  /**
   * Interpolation contract.
   */
  interpolation_mode: VolInterpolationMode;
  /**
   * Quote convention.
   */
  quote_type: VolQuoteType;
  /**
   * Semantic meaning of the secondary axis.
   */
  secondary_axis: VolSurfaceAxis;
  /**
   * Strike prices
   */
  strikes: number[];
  /**
   * Volatility values in row-major order
   */
  vols_row_major: number[];
}
/**
 * SABR volatility cube on an expiry x tenor grid.
 *
 * Each grid node stores a [`SabrParameterData`] and a forward rate. The
 * interpolation mode records how a models-layer evaluator should combine
 * nodes; core performs structural validation only.
 */
export interface VolCube {
  /**
   * Option expiries in years, strictly increasing. Indexes the first axis.
   */
  expiries: number[];
  /**
   * Row-major: forwards[expiry_idx * n_tenors + tenor_idx]
   */
  forwards: number[];
  /**
   * Identifier the cube is registered and looked up under.
   */
  id: string;
  /**
   * How volatilities are interpolated between the cube's grid points.
   */
  interpolation_mode: VolInterpolationMode;
  /**
   * Row-major: params[expiry_idx * n_tenors + tenor_idx]
   */
  params: SabrParameterData[];
  /**
   * Underlying swap tenors in years, strictly increasing. Indexes the
   * second axis.
   */
  tenors: number[];
}
/**
 * Canonical typed calibration result envelope.
 */
export interface CalibrationResultEnvelope {
  /**
   * The calibration result.
   */
  result: CalibrationResult;
  /**
   * Schema marker; current writers emit [`CALIBRATION_SCHEMA`].
   */
  schema: CalibrationSchema;
}
/**
 * Structural validation findings and the static step dependency graph.
 */
export interface CalibrationValidationReport {
  /**
   * Topological view of the steps' inputs and outputs.
   */
  dependency_graph: DependencyGraph;
  /**
   * All errors found in a single pass; empty if the envelope is valid.
   */
  errors: EnvelopeError[];
  [k: string]: unknown;
}
/**
 * Static dependency graph derived from a [`CalibrationEnvelope`].
 */
export interface DependencyGraph {
  /**
   * Curve / surface IDs available at the start of execution, contributed
   * by `market_data` and `prior_market`.
   */
  initial_ids: string[];
  /**
   * Per-step inputs and outputs in declared order.
   */
  nodes: DependencyNode[];
  [k: string]: unknown;
}
/**
 * A single step's view of the dependency graph.
 */
export interface DependencyNode {
  /**
   * Step kind (`"discount"`, `"forward"`, ...).
   */
  kind: string;
  /**
   * Curve / surface IDs the step depends on. Each must be either in
   * `initial_ids` or produced by an earlier step.
   */
  reads: string[];
  /**
   * Step identifier.
   */
  step_id: string;
  /**
   * Zero-based index in `plan.steps`.
   */
  step_index: number;
  /**
   * Curve / surface ID(s) the step produces.
   */
  writes: string[];
  [k: string]: unknown;
}
/**
 * One structured finding from the bounded strict loader.
 *
 * Mirrors the host-independent fields of
 * [`finstack_quant_core::contract::Diagnostic`] so the Python and WASM
 * bindings can attach the findings as plain records.
 */
export interface StrictLoadDiagnostic {
  /**
   * Version actually found, when the finding is version-related.
   */
  actual_version?: number | null;
  /**
   * Stable machine-readable code (e.g. `parse/invalid-json`,
   * `calibration/undefined-quote-set`).
   */
  code: string;
  /**
   * Contract identifier the finding was evaluated against, when known.
   */
  contract?: string | null;
  /**
   * Expected contract version, when the finding is version-related.
   */
  expected_version?: number | null;
  /**
   * Human-readable description of the finding.
   */
  message: string;
  /**
   * Load phase that produced the finding (`parse`, `version`, `structure`,
   * `semantic`, ...).
   */
  phase: string;
  /**
   * RFC 6901 JSON pointer into the request document, when known.
   */
  pointer?: string | null;
  /**
   * Severity label (`error` or `warning`).
   */
  severity: string;
  [k: string]: unknown;
}
/**
 * Settings of the scalar Hull-White fit to cap/floor quotes.
 */
export interface CapFloorCalibrationConfig {
  /**
   * Required positive maximum implied-quote error in quoted volatility units.
   * Normal quotes use decimal rate volatility; Black quotes use relative volatility.
   * This acceptance budget is independent of the numerical solver tolerance.
   */
  fit_tolerance: number;
  /**
   * Optional source mean reversion. Required when calibrating from a
   * single cap/floor quote because one quote cannot identify both κ and σ.
   */
  fixed_kappa?: number | null;
  /**
   * Payment frequency used to decompose full caps/floors into caplets.
   */
  frequency?: SwapFrequency;
  /**
   * Optional initial guess when solving both κ and σ.
   */
  initial_guess?: HullWhiteCalibrationParams | null;
}
/**
 * Validated constant-parameter Hull-White one-factor model.
 */
export interface HullWhiteCalibrationParams {
  kappa: number;
  sigma: number;
  [k: string]: unknown;
}
/**
 * Cap or floor volatility quote in year fractions for the direct Hull-White calibrators.
 */
export interface CapFloorQuote {
  /**
   * `true` for a cap, `false` for a floor.
   */
  is_cap: boolean;
  /**
   * `true` for normal (Bachelier) vol. Lognormal cap/floor quotes are rejected.
   */
  is_normal_vol: boolean;
  /**
   * Cap/floor maturity in years from the curve base date; finite and positive.
   */
  maturity: number;
  /**
   * Strike rate as a decimal (for example `0.03` for 3%).
   */
  strike: number;
  /**
   * Market-quoted flat volatility; normal vols use decimal rate units
   * (`0.0088` is 88 bp). Finite and positive.
   */
  volatility: number;
}
/**
 * Index tranche upfront quote.
 */
export interface CdsTrancheQuote {
  /**
   * Attachment point (decimal, e.g. 0.03).
   */
  attachment: number;
  /**
   * Convention key (currency + doc clause).
   */
  convention: CdsConventionKey;
  /**
   * Contractual running coupon of the tranche, in basis points.
   */
  coupon_bp: number;
  /**
   * Detachment point (decimal, e.g. 0.07).
   */
  detachment: number;
  /**
   * Unique identifier.
   */
  id: QuoteId;
  /**
   * Index identifier (e.g. CDX.NA.HY).
   */
  index: string;
  /**
   * Maturity date.
   */
  maturity: valuations.DateWire;
  /**
   * CDS index series number.
   */
  series: number;
  /**
   * Upfront payment as a decimal fraction of tranche notional (e.g., -0.025 for -2.5%).
   */
  upfront_pct: number;
}
/**
 * Settings of the piecewise-constant Hull-White sigma bootstrap to cap/floor quotes.
 */
export interface PiecewiseSigmaCalibrationConfig {
  /**
   * Required positive maximum implied-quote error in quoted volatility units.
   * Normal quotes use decimal rate volatility; Black quotes use relative volatility.
   * This acceptance budget is independent of the numerical solver tolerance.
   */
  fit_tolerance: number;
  /**
   * Mean reversion held fixed while bootstrapping the volatility schedule.
   */
  fixed_kappa: number;
  /**
   * Coupon frequency used to decompose each market cap/floor quote.
   */
  frequency?: SwapFrequency;
  /**
   * Inclusive upper short-rate volatility search bound.
   */
  sigma_max: number;
  /**
   * Inclusive lower short-rate volatility search bound.
   */
  sigma_min: number;
}
/**
 * ATM swaption volatility quote in year fractions for the direct Hull-White calibrator.
 */
export interface SwaptionQuote {
  /**
   * Swaption expiry in years (T₀); finite and positive.
   */
  expiry: number;
  /**
   * `true` for normal (Bachelier) vol, `false` for lognormal (Black-76) vol.
   */
  is_normal_vol: boolean;
  /**
   * Underlying swap tenor in years (e.g. 5.0 for a 5Y swap); finite and positive.
   */
  tenor: number;
  /**
   * Market-quoted volatility as a decimal: absolute rate volatility for
   * normal quotes, Black volatility for lognormal quotes; finite and positive.
   */
  volatility: number;
}
/**
 * Cross-currency basis swap quote.
 */
export interface XccyQuote {
  /**
   * Basis spread in basis points on the base-currency leg.
   */
  basis_spread_bp: number;
  /**
   * XCCY pair convention identifier (e.g., `EUR/USD-XCCY`).
   */
  convention: XccyConventionId;
  /**
   * Far-leg maturity pillar; near leg is the convention spot date.
   */
  far_pillar: Pillar;
  /**
   * Unique identifier for the quote.
   */
  id: QuoteId;
  /**
   * Optional spot FX quote (quote currency per 1 unit of base currency).
   */
  spot_fx?: number | null;
}
