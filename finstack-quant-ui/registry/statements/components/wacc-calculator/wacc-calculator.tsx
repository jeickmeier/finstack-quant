"use client";
import { useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import {
  Outcome,
  typedNumber,
} from "@/components/finstack/shared/primitives/native-display/native-display";
import type { WaccRequest } from "@/workers/finstack-contract";

/** The arguments in Rust's order, with the wire name that labels each and what it means. */
const fields: readonly (readonly [keyof WaccRequest, string, string])[] = [
  [
    "equityWeight",
    "equity_weight",
    "Equity share of total capital, decimal (0.6 = 60%).",
  ],
  [
    "costOfEquity",
    "cost_of_equity",
    "Required return on equity, decimal (0.115 = 11.5%).",
  ],
  [
    "debtWeight",
    "debt_weight",
    "Debt share of total capital, decimal; must sum with the equity weight to one.",
  ],
  [
    "costOfDebt",
    "cost_of_debt",
    "Pre-tax borrowing yield, decimal (0.06 = 6%).",
  ],
  [
    "taxRate",
    "tax_rate",
    "Marginal tax rate, decimal from 0 to 1 (0.25 = 25%).",
  ],
];
export interface WaccCalculatorProps {
  /** The inputs the form starts from; omitted starts empty. */
  defaultInputs?: WaccRequest;
  /** Called with the five entered values, for Rust to blend. */
  onSubmit(inputs: WaccRequest): void;
  /** The native statements_analytics.wacc rate, a decimal; null while none is available. */
  rate: number | null;
  loading?: boolean;
  /** The native error of the request (weights that do not sum to one, a tax rate out of range), shown as returned. */
  error?: string;
  /** Receives the rate shown, on the user's click only; omitted hides the button. */
  onApply?: (rate: number) => void;
  /** Text of the apply button. */
  applyLabel?: string;
  /** Display text of the returned rate; omitted shows every digit. */
  formatValue?: (value: number) => string;
  disabled?: boolean;
  density?: "compact" | "comfortable";
}
/**
 * Enter the capital weights and costs and see the WACC Rust returns. The two
 * weights are separate inputs: nothing is blended, defaulted or checked here,
 * and weights that do not sum to one show the native error.
 */
export function WaccCalculator({
  defaultInputs,
  onSubmit,
  rate,
  loading,
  error,
  onApply,
  applyLabel = "Use as WACC",
  formatValue = String,
  disabled,
  density = "compact",
}: WaccCalculatorProps) {
  const [values, setValues] = useState<Record<keyof WaccRequest, string>>(
    () => {
      const text = (name: keyof WaccRequest) =>
        defaultInputs ? String(defaultInputs[name]) : "";
      return {
        equityWeight: text("equityWeight"),
        costOfEquity: text("costOfEquity"),
        debtWeight: text("debtWeight"),
        costOfDebt: text("costOfDebt"),
        taxRate: text("taxRate"),
      };
    },
  );
  // The rate shown answers the last submission, so an edit since then cannot be applied through it.
  const [edited, setEdited] = useState(false);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    setEdited(false);
    onSubmit({
      equityWeight: typedNumber(values.equityWeight),
      costOfEquity: typedNumber(values.costOfEquity),
      debtWeight: typedNumber(values.debtWeight),
      costOfDebt: typedNumber(values.costOfDebt),
      taxRate: typedNumber(values.taxRate),
    });
  };
  return (
    <section
      aria-label="WACC calculator"
      data-density={density}
      className="@container min-w-0 max-w-full space-y-4 font-sans text-sm text-foreground"
    >
      <form aria-label="WACC inputs" className="space-y-4" onSubmit={submit}>
        <p className="text-xs text-muted-foreground">
          Enter every value as a decimal: 0.10 means 10%.
        </p>
        <div className="grid gap-4 @lg:grid-cols-2">
          {fields.map(([name, wire, help]) => (
            <DecimalInput
              key={name}
              label={labelFor(wire)}
              help={help}
              value={values[name]}
              onValueChange={(value) => {
                setEdited(true);
                setValues((prior) => ({ ...prior, [name]: value }));
              }}
            />
          ))}
        </div>
        <Button type="submit" disabled={disabled}>
          Calculate
        </Button>
      </form>
      <Outcome
        state={{ result: rate, loading, error }}
        empty="No rate calculated"
        working="Calculating…"
      >
        {(rate) => (
          <div className="flex flex-wrap items-end gap-4">
            <dl>
              <dt className="text-xs text-muted-foreground">WACC, decimal</dt>
              <dd className="finstack-numeric text-base" title={String(rate)}>
                {formatValue(rate)}
              </dd>
            </dl>
            {onApply && (
              <Button
                type="button"
                variant="outline"
                disabled={edited}
                onClick={() => onApply(rate)}
              >
                {applyLabel}
              </Button>
            )}
            {edited && (
              <p role="status" className="text-xs text-muted-foreground">
                Inputs changed since this rate; calculate again.
              </p>
            )}
          </div>
        )}
      </Outcome>
    </section>
  );
}
