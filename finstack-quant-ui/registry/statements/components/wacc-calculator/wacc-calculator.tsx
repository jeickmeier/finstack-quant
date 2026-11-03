"use client";
import { useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";

/** The five statements_analytics.wacc arguments, each a decimal as Rust takes it. */
export interface WaccInputs {
  equityWeight: number;
  costOfEquity: number;
  debtWeight: number;
  costOfDebt: number;
  taxRate: number;
}
/** The arguments in Rust's order, with the wire name that labels each and what it means. */
const fields: readonly (readonly [keyof WaccInputs, string, string])[] = [
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
/** An empty field is NaN, which Rust rejects, not the zero `Number("")` would send. */
const typed = (value: string) =>
  value.trim() === "" ? Number.NaN : Number(value);
export interface WaccCalculatorProps {
  /** The inputs the form starts from; omitted starts empty. */
  defaultInputs?: WaccInputs;
  /** Called with the five entered values, for Rust to blend. */
  onSubmit(inputs: WaccInputs): void;
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
  const [values, setValues] = useState<Record<string, string>>(() =>
    Object.fromEntries(
      fields.map(([name]) => [
        name,
        defaultInputs ? String(defaultInputs[name]) : "",
      ]),
    ),
  );
  // The rate shown answers the last submission, so an edit since then cannot be applied through it.
  const [edited, setEdited] = useState(false);
  const submit = (event: FormEvent) => {
    event.preventDefault();
    setEdited(false);
    onSubmit(
      Object.fromEntries(
        fields.map(([name]) => [name, typed(values[name] ?? "")]),
      ) as unknown as WaccInputs,
    );
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
              value={values[name] ?? ""}
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
      {error ? (
        <p role="alert" className="text-error">
          {error}
        </p>
      ) : loading ? (
        <p role="status" className="text-muted-foreground">
          Calculating…
        </p>
      ) : rate === null ? (
        <p role="status" className="text-muted-foreground">
          No rate calculated
        </p>
      ) : (
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
    </section>
  );
}
