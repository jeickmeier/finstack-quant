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

/** The pricing model; each takes its own terms. */
type Kind = modelsWire.OptionModel["kind"];
export interface OptionCalculatorProps {
  /** The initial inputs, and the request a result answers until Calculate; remount to load another request. */
  defaultRequest: modelsWire.OptionCalculation;
  /** Receives the request on Calculate; connect it to the useOptionCalculator request. */
  onSubmit(request: modelsWire.OptionCalculation): void;
  /** The returned result of the last request; null while none is available. */
  result: modelsWire.OptionCalculationResult | null;
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
        "div_yield",
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
  Kind,
  {
    label: string;
    fields: readonly (readonly [string, string, string])[];
    vol: readonly [string, string];
    priceHelp: string;
    priceNote: string;
    greeksNote: string;
  }
>;
/** The supplied numbers as field text by wire name; a model the request does not use starts empty. */
function fieldText(request: modelsWire.OptionCalculation) {
  return Object.fromEntries(
    Object.entries({ ...request.model, ...request.solve }).flatMap(
      ([key, value]) =>
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
  const [model, setModel] = useState<Kind>(defaultRequest.model.kind);
  const [mode, setMode] = useState(defaultRequest.solve.kind);
  const [side, setSide] = useState(
    defaultRequest.model.is_call ? "call" : "put",
  );
  // The request the shown result answers: the last one submitted.
  const [answered, setAnswered] = useState(defaultRequest);
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
    const theta = values.theta_days_per_year?.trim()
      ? { theta_days_per_year: typedNumber(values.theta_days_per_year) }
      : {};
    const request = {
      model: {
        kind: model,
        is_call: side === "call",
        ...terms,
        ...(model === "black_scholes" ? theta : {}),
      },
      solve:
        mode === "price"
          ? { kind: "price", vol: typedNumber(values.vol) }
          : { kind: "implied_vol", price: typedNumber(values.price) },
    } as modelsWire.OptionCalculation;
    setAnswered(request);
    onSubmit(request);
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
            onValueChange={(value) => setModel(value as Kind)}
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
              "theta_days_per_year",
              "Theta days per year",
              "Day-count denominator for theta; empty uses the Rust default of 365.",
            )}
        </div>
        <Button type="submit">Calculate</Button>
      </form>
      <OptionResultView
        request={answered}
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
  request,
  result,
  loading,
  error,
  format,
  density,
}: {
  request: modelsWire.OptionCalculation;
  result: modelsWire.OptionCalculationResult | null;
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
  // The notes follow the answered request's model, not the current selection.
  const definition = optionModels[request.model.kind];
  const [title, value, note] =
    request.solve.kind === "price"
      ? ["Price", result.price, definition.priceNote]
      : [
          `Implied ${definition.vol[0].toLowerCase()}`,
          result.implied_vol,
          definition.vol[1],
        ];
  const { kind: _family, ...greeks } = result.greeks;
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
        <FinstackTable
          data={Object.entries(greeks).map(([name, greek]) => ({
            name: labelFor(name),
            value: greek,
          }))}
          columns={columns}
          getRowId={(row) => row.name}
          caption="Option Greeks"
          captionVisibility="sr-only"
          density={density}
          emptyState="No Greeks returned"
        />
        <p className="text-xs text-muted-foreground">{definition.greeksNote}</p>
      </section>
    </div>
  );
}
