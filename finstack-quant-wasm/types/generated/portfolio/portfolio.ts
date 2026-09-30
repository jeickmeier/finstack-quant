// Generated from the finstack-quant-portfolio JSON schemas by scripts/generate-contract-types.mjs. Do not edit.
import type * as valuations from '../valuations/index.js';

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
 * Book identifier.
 */
export type BookId = string;
/**
 * Position identifier.
 */
export type PositionId = string;
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
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
/**
 * Stage of persisted artifact loading that produced a diagnostic.
 */
export type LoadPhase = "parse" | "version" | "structure" | "semantic" | "canonicalize" | "hash" | "build";
/**
 * Severity assigned to a persisted artifact diagnostic.
 */
export type Severity = "error" | "warning";
/**
 * Entity identifier (company, fund, etc.)
 */
export type EntityId = string;
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
 * # Examples
 *
 * ```rust
 * use finstack_quant_core::types::{CurveId, InstrumentId};
 *
 * // Create IDs with different type tags
 * let curve = CurveId::from("USD-SOFR");
 * let bond = InstrumentId::from("ISIN:US912828XG60");
 *
 * // Can compare IDs of the same type
 * assert_eq!(curve, CurveId::from("USD-SOFR"));
 * assert_ne!(curve, CurveId::from("EUR-ESTR"));
 *
 * // Cannot compare IDs of different types (compile error):
 * // let _ = curve == bond;  // Error: mismatched types
 * ```
 *
 * # Thread Safety
 *
 * `Id<T>` is `Send + Sync` as it wraps an `Arc<str>`. Multiple threads can
 * safely share and clone IDs with minimal synchronization overhead.
 */
export type Id = string;
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
