"use client";
import {
  MetricValuesTable,
  type MetricValuesTableProps,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import {
  metricOverrides,
  overrideValues,
} from "@/components/finstack/valuations/components/pricing-params-form/metric-overrides";

/** The ids the card shows, in display order: spreads, the option, durations, risk, carry and breakeven. */
export const bondCreditMetrics = [
  "z_spread",
  "i_spread",
  "g_spread",
  "oas",
  "asw_par",
  "asw_market",
  "discount_margin",
  "embedded_option_value",
  "duration_mod",
  "duration_mac",
  "convexity",
  "dv01",
  "cs01",
  "recovery01",
  "jump_to_default",
  "theta",
  "theta_carry",
  "theta_roll_down",
  "carry_total",
  "coupon_income",
  "pull_to_par",
  "roll_down",
  "funding_cost",
  "theta_period_days",
  "breakeven",
] as const;
/** The MetricPricingOverrides fields the card edits; the form keeps the others as supplied. */
const horizonFields = { allow: ["theta_period", "breakeven_config"] };
export interface BondCreditCardProps extends Omit<
  MetricValuesTableProps,
  "label" | "note"
> {
  /** The request's MetricPricingOverrides JSON the form starts from; omitted starts empty. */
  overrides?: string | null;
  /** Receives complete MetricPricingOverrides JSON when the form is applied; omit to hide the form. */
  onOverridesChange?: (json: string) => void;
}
/**
 * Spreads, option value, durations, default risk, carry and breakeven of a bond,
 * as Rust returned them by metric id. An id Rust rejects shows its native error
 * in its own row and leaves the others. The carry horizon and BreakevenConfig
 * come from the schema's form; no target, mode or horizon is assumed here.
 */
export function BondCreditCard({
  overrides,
  onOverridesChange,
  ...table
}: BondCreditCardProps) {
  return (
    <div className="min-w-0 max-w-full space-y-3">
      {onOverridesChange && (
        <details className="font-sans text-sm text-foreground">
          <summary>Carry horizon and breakeven</summary>
          <SchemaForm
            module={metricOverrides}
            fields={horizonFields}
            layout="full"
            explicitNull={false}
            defaultValues={overrideValues(overrides)}
            submitLabel="Apply"
            submitVariant="outline"
            validate={async (json) => json}
            onSubmit={onOverridesChange}
          />
        </details>
      )}
      <MetricValuesTable
        {...table}
        label="Bond credit"
        note={
          <>
            Values as Rust returned them. Spreads and durations solve to a
            quoted clean price when the bond carries one, otherwise to the model
            price. Z-spread compounds at the bond&apos;s coupon frequency and
            OAS on its OAS quote compounding, continuous by default, so the two
            differ even when they price the same flows. ASW par is the coupon
            less the par rate and does not move with the quoted price; ASW
            market does. Carry metrics cover the horizon Theta period days
            reports. Breakeven is the move in the configured target that offsets
            carry over that horizon: basis points for z-spread, OAS and yield
            targets, vol points for implied vol and correlation points for base
            correlation.
          </>
        }
      />
    </div>
  );
}
