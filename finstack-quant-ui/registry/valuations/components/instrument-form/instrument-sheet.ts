import {
  LESS_COMMON_TERMS,
  type Disclosure,
  type SheetRoot,
} from "@/components/finstack/core/components/schema-form/disclosure";

/** The instrument spec renders as the two-column term sheet. */
export const INSTRUMENT_SHEET: SheetRoot = {
  path: "instrument.spec",
  label: "Instrument terms",
};

/** Collapsed group the desk quote's "More pricing overrides" action reveals. */
export const PRICING_OVERRIDES = "Pricing overrides";

/** Collapsed groups, in sheet order. Common deal terms stay outside these groups. */
const GROUP_ORDER = [
  "Schedule conventions",
  "Market links",
  PRICING_OVERRIDES,
  "Attributes",
  "Deal structure",
  "Facility terms",
  LESS_COMMON_TERMS,
] as const;

const SCHEDULE = new Set([
  "day_count",
  "calendar_id",
  "payment_calendar_id",
  "fixing_calendar_id",
  "ex_coupon_calendar_id",
  "base_calendar_id",
  "quote_calendar_id",
  "business_day_convention",
  "end_of_month",
  "stub",
  "payment_lag_days",
  "stated_delay_days",
  "settlement_days",
  "settlement_delay",
  "ex_coupon_days",
  "reset_lag_days",
  "index_lag_days",
  "accrual_method",
  "roll_rule",
  "protection_start_convention",
  "delta_convention",
  "underlying_convention",
  "valuation_convention",
]);

const PRICING = new Set([
  "instrument_pricing_overrides",
  "metric_pricing_overrides",
  "scenario_pricing_overrides",
  "vol_shift",
  "vol_type",
  "vol_model",
  "path_model",
  "sabr_params",
  "margin_spec",
]);

const LESS_COMMON = new Set([
  "recovery_rate",
  "settlement",
  "discrete_dividends",
  "index_floor_bp",
  "knockout",
]);

/** Spec-root keys whose filled examples are still uncommon for a desk user. */
export const SPEC_GROUPS: Readonly<
  Record<string, Readonly<Record<string, string>>>
> = {
  structured_credit: {
    pool: "Deal structure",
    market_conditions: "Deal structure",
    default_spec: "Deal structure",
    prepayment_spec: "Deal structure",
    recovery_spec: "Deal structure",
    deal_metadata: "Deal structure",
    hedge_swaps: "Deal structure",
    fees: "Deal structure",
    coverage_rules: "Deal structure",
    coverage_triggers: "Deal structure",
    correlation_structure: "Deal structure",
    delinquency: "Deal structure",
    loss_allocation: "Deal structure",
    loss_recognition: "Deal structure",
    liquidation_price_pct: "Deal structure",
    card: "Deal structure",
    call_assumption: "Deal structure",
    cleanup_call_decimal: "Deal structure",
    stochastic_default_spec: "Deal structure",
    stochastic_prepay_spec: "Deal structure",
    stochastic_recovery_spec: "Deal structure",
    tranche_draws: "Deal structure",
    tranche_readvance: "Deal structure",
    waterfall: "Deal structure",
    waterfall_rules: "Deal structure",
    principal_covers_senior_interest: "Deal structure",
    quote_settlement_date: "Deal structure",
  },
  revolving_credit: {
    fees: "Facility terms",
    margin_steps: "Facility terms",
    leq: "Facility terms",
    draw_repay_spec: "Facility terms",
    commitment_steps: "Facility terms",
    scheduled_fees: "Facility terms",
    lc: "Facility terms",
    oid_eir: "Facility terms",
  },
  inflation_linked_bond: {
    lag: "Schedule conventions",
  },
  autocallable: {
    past_fixings: LESS_COMMON_TERMS,
  },
  composite: {
    state: LESS_COMMON_TERMS,
  },
};

function marketLink(key: string, siblingKeys: readonly string[]): boolean {
  if (
    key.endsWith("_curve_id") ||
    key === "vol_surface_id" ||
    key === "div_yield_id"
  )
    return true;
  return (
    key === "spot_id" &&
    (siblingKeys.includes("underlying_ticker") ||
      siblingKeys.includes("ticker"))
  );
}

/** Deal terms that stay on the first screen even when they match a schema default. */
const PINNED = new Set(["exercise_style"]);

/** Group for a filled convention, or null when the term stays on the first screen. */
export function termDisclosure(
  key: string,
  options: {
    atSpec: boolean;
    instrumentType?: string;
    siblingKeys: readonly string[];
  },
): string | null {
  if (options.atSpec && options.instrumentType) {
    const grouped = SPEC_GROUPS[options.instrumentType]?.[key];
    if (grouped) return grouped;
  }
  if (SCHEDULE.has(key)) return "Schedule conventions";
  if (marketLink(key, options.siblingKeys)) return "Market links";
  if (PRICING.has(key)) return PRICING_OVERRIDES;
  if (key === "attributes") return "Attributes";
  if (LESS_COMMON.has(key)) return LESS_COMMON_TERMS;
  return null;
}

/**
 * Instrument term-sheet grouping for one instrument type. Within the spec a
 * pinned term stays on the first screen, then a named convention group
 * applies; everything else falls back to Less common terms when defaulted or
 * empty.
 */
export function instrumentDisclosure(type: string): Disclosure {
  return {
    order: GROUP_ORDER,
    group({ path, key, siblingKeys, secondary }) {
      if (
        path === INSTRUMENT_SHEET.path ||
        path.startsWith(`${INSTRUMENT_SHEET.path}.`)
      ) {
        if (PINNED.has(key)) return null;
        const named = termDisclosure(key, {
          atSpec: path === INSTRUMENT_SHEET.path,
          instrumentType: type,
          siblingKeys,
        });
        if (named) return named;
      }
      return secondary ? LESS_COMMON_TERMS : null;
    },
  };
}
