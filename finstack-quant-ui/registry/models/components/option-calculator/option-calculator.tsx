"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { models as modelsWire } from "finstack-quant-wasm/types";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import {
  shownValue,
  typedNumber,
} from "@/components/finstack/shared/primitives/native-display/native-display";

/** A native result or the native error of that one call. */
type Outcome<T> =
  { ok: true; value: T } | { ok: false; error: { message: string } };
/** The pricing model; each takes its own terms. */
export type OptionModel = "black_scholes" | "black76" | "bachelier";
type OptionTerms =
  | {
      model: "black_scholes";
      spot: number;
      strike: number;
      rate: number;
      divYield: number;
      expiry: number;
      isCall: boolean;
      /** Omitted leaves the theta denominator to Rust. */
      thetaDaysPerYear?: number | null;
    }
  | {
      model: "black76";
      forward: number;
      strike: number;
      df: number;
      expiry: number;
      isCall: boolean;
    }
  | {
      model: "bachelier";
      forward: number;
      strike: number;
      expiry: number;
      isCall: boolean;
    };
/** The request the calculator submits; the worker's OptionCalculatorRequest. */
export type OptionCalculation = OptionTerms &
  ({ mode: "price"; vol: number } | { mode: "implied_vol"; price: number });
/** What the worker's optionCalculator returns. */
export type OptionCalculationResult = {
  model: OptionModel;
  /** The returned Greeks, or Rust's error of that one call. */
  greeks: Outcome<modelsWire.BsGreeks | modelsWire.ForwardGreeks>;
} & (
  { mode: "price"; price: number } | { mode: "implied_vol"; impliedVol: number }
);
export interface OptionCalculatorProps {
  /** The initial inputs; remount to load another request. */
  defaultRequest: OptionCalculation;
  /** Receives the request on Calculate; connect it to the useOptionCalculator request. */
  onSubmit(request: OptionCalculation): void;
  /** The returned result; null while none is available. */
  result: OptionCalculationResult | null;
  loading?: boolean;
  /** The native error of the whole calculation, shown as returned. */
  error?: string;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
const strikeField = [
  "strike",
  "Strike",
  "Same price units as the underlying.",
] as const;
const expiryField = ["expiry", "Expiry", "Years to expiry."] as const;
/** The inputs, units and output conventions each facade function documents. */
const optionModels = {
  black_scholes: {
    label: "Black-Scholes",
    fields: [
      ["spot", "Spot", "Spot price of the underlying."],
      strikeField,
      ["rate", "Rate", "Risk-free rate, decimal, continuously compounded."],
      [
        "divYield",
        "Dividend yield",
        "Dividend yield or foreign rate, decimal, continuously compounded.",
      ],
      expiryField,
    ],
    vol: ["Volatility", "Annualized, decimal."],
    priceHelp: "Observed per-unit option price.",
    priceNote: "Per-unit option price.",
    greeksNote:
      "Vega and both rhos are per 1% move; theta is per day under the theta denominator.",
  },
  black76: {
    label: "Black-76",
    fields: [
      ["forward", "Forward", "Forward price or rate at expiry."],
      strikeField,
      ["df", "Discount factor", "Valuation to expiry, positive decimal."],
      expiryField,
    ],
    vol: ["Volatility", "Annualized lognormal (Black) volatility, decimal."],
    priceHelp:
      "Observed discounted per-unit price, in the units of the forward.",
    priceNote: "Discounted per-unit price, in the units of the forward.",
    greeksNote:
      "Undiscounted forward Greeks: delta and gamma with respect to the forward, vega per unit (1.0) change in volatility.",
  },
  bachelier: {
    label: "Bachelier",
    fields: [
      [
        "forward",
        "Forward",
        "Forward price or rate at expiry; may be negative.",
      ],
      strikeField,
      expiryField,
    ],
    vol: [
      "Normal volatility",
      "Annualized and absolute, in the units of the forward.",
    ],
    priceHelp:
      "Observed undiscounted per-unit price, in the units of the forward.",
    priceNote: "Undiscounted per-unit price, in the units of the forward.",
    greeksNote:
      "Undiscounted forward Greeks: vega per unit (1.0) change in normal volatility.",
  },
} as const satisfies Record<
  OptionModel,
  {
    label: string;
    fields: readonly (readonly [string, string, string])[];
    vol: readonly [string, string];
    priceHelp: string;
    priceNote: string;
    greeksNote: string;
  }
>;
/** The supplied numbers as field text; a model the request does not use starts empty. */
function fieldText(request: OptionCalculation) {
  return Object.fromEntries(
    Object.entries(request).flatMap(([key, value]) =>
      typeof value === "number" ? [[key, String(value)]] : [],
    ),
  );
}
/**
 * Prices a European option or solves its implied volatility under Black-Scholes,
 * Black-76 or Bachelier, with the Greeks at that volatility. Rust validates every
 * input and computes every number; the caller's callback runs the calculation.
 */
export function OptionCalculator({
  defaultRequest,
  onSubmit,
  result,
  loading,
  error,
  formatValue = String,
  density = "compact",
}: OptionCalculatorProps) {
  const [model, setModel] = useState<OptionModel>(defaultRequest.model);
  const [mode, setMode] = useState(defaultRequest.mode);
  const [side, setSide] = useState(defaultRequest.isCall ? "call" : "put");
  const [values, setValues] = useState<Record<string, string>>(() =>
    fieldText(defaultRequest),
  );
  const field = (key: string, label: string, help: string) => (
    <DecimalInput
      key={key}
      label={label}
      help={help}
      value={values[key] ?? ""}
      onValueChange={(value) =>
        setValues((prior) => ({ ...prior, [key]: value }))
      }
    />
  );
  const definition = optionModels[model];
  const submit = () => {
    const terms = Object.fromEntries(
      definition.fields.map(([key]) => [key, typedNumber(values[key])]),
    );
    // An empty theta denominator stays unset so Rust applies its own.
    const theta = values.thetaDaysPerYear?.trim()
      ? { thetaDaysPerYear: typedNumber(values.thetaDaysPerYear) }
      : {};
    onSubmit({
      model,
      mode,
      isCall: side === "call",
      ...terms,
      ...(model === "black_scholes" ? theta : {}),
      ...(mode === "price"
        ? { vol: typedNumber(values.vol) }
        : { price: typedNumber(values.price) }),
    } as OptionCalculation);
  };
  return (
    <section
      aria-label="Option calculator"
      data-density={density}
      className="@container space-y-6 font-sans text-sm text-foreground"
    >
      <form
        aria-label="Option terms"
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <div className="grid gap-4 @lg:grid-cols-3">
          <EnumField
            label="Model"
            value={model}
            onValueChange={(value) => setModel(value as OptionModel)}
            options={Object.entries(optionModels).map(([value, { label }]) => ({
              value,
              label,
            }))}
            layout="radio"
          />
          <EnumField
            label="Solve for"
            value={mode}
            onValueChange={(value) => setMode(value as typeof mode)}
            options={[
              { value: "price", label: "Price" },
              { value: "implied_vol", label: "Implied volatility" },
            ]}
            layout="radio"
          />
          <EnumField
            label="Option"
            value={side}
            onValueChange={setSide}
            options={[
              { value: "call", label: "Call" },
              { value: "put", label: "Put" },
            ]}
            layout="radio"
          />
        </div>
        <div className="grid gap-4 @lg:grid-cols-3">
          {definition.fields.map(([key, label, help]) =>
            field(key, label, help),
          )}
          {mode === "price"
            ? field("vol", definition.vol[0], definition.vol[1])
            : field("price", "Option price", definition.priceHelp)}
          {model === "black_scholes" &&
            field(
              "thetaDaysPerYear",
              "Theta days per year",
              "Day-count denominator for theta; empty uses the Rust default of 365.",
            )}
        </div>
        <Button type="submit">Calculate</Button>
      </form>
      <OptionResultView
        result={result}
        loading={loading}
        error={error}
        format={formatValue}
        density={density}
      />
    </section>
  );
}
interface GreekRow {
  name: string;
  value: unknown;
}
function OptionResultView({
  result,
  loading,
  error,
  format,
  density,
}: {
  result: OptionCalculationResult | null;
  loading?: boolean;
  error?: string;
  format: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  const columns = useMemo<ColumnDef<{}, GreekRow, any>[]>(
    () => [
      {
        id: "name",
        header: "Greek",
        accessorFn: (row) => row.name,
        meta: { className: "text-left" },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: (row) => row.value,
        cell: ({ getValue }) => shownValue(getValue(), format),
        meta: { className: "text-right finstack-numeric" },
      },
    ],
    [format],
  );
  if (error)
    return (
      <p role="alert" className="text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="text-muted-foreground">
        Calculating option…
      </p>
    );
  if (!result)
    return (
      <p role="status" className="text-muted-foreground">
        No option result
      </p>
    );
  // The notes follow the returned model, not the current selection.
  const definition = optionModels[result.model];
  const [title, value, note] =
    result.mode === "price"
      ? ["Price", result.price, definition.priceNote]
      : [
          `Implied ${definition.vol[0].toLowerCase()}`,
          result.impliedVol,
          definition.vol[1],
        ];
  return (
    <div className="space-y-4">
      <section
        aria-label={title}
        className="flex min-w-0 flex-col gap-1 rounded-md border border-border p-2"
      >
        <h3 className="text-xs font-medium text-muted-foreground">{title}</h3>
        <p className="finstack-numeric text-base">
          {shownValue(value, format)}
        </p>
        <p className="text-xs text-muted-foreground">{note}</p>
      </section>
      <section aria-label="Greeks" className="min-w-0 space-y-2">
        <h3 className="text-sm font-medium">Greeks</h3>
        {result.greeks.ok ? (
          <>
            <FinstackTable
              data={Object.entries(result.greeks.value).map(
                ([name, greek]) => ({ name: labelFor(name), value: greek }),
              )}
              columns={columns}
              getRowId={(row) => row.name}
              caption="Option Greeks"
              captionVisibility="sr-only"
              density={density}
              emptyState="No Greeks returned"
            />
            <p className="text-xs text-muted-foreground">
              {definition.greeksNote}
            </p>
          </>
        ) : (
          <p role="alert" className="text-xs text-error">
            {result.greeks.error.message}
          </p>
        )}
      </section>
    </div>
  );
}
