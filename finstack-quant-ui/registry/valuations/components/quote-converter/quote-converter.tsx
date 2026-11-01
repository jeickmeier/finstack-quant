"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { MetricMetadata } from "finstack-quant-wasm";
import type { valuations } from "finstack-quant-wasm/types";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";

/** One market quote, as the schema's single-key object. */
export type QuoteInput = valuations.BondQuoteInput;
/** The whole quote set Rust returned. */
export type QuoteSet = valuations.BondQuoteSet;
/** The accepted quote kinds and what each means, from the BondQuoteInput schema. */
const kinds = contracts.bond_quote_input.schema.oneOf.map((branch) => ({
  value: branch.required[0],
  label: labelFor(branch.required[0]),
  description: branch.description,
}));
/** The metric id of a set field named differently from its metric; clean_price_pct has none. */
const fieldMetric: Readonly<Record<string, string>> = {
  clean_price_currency: "clean_price",
  dirty_price_currency: "dirty_price",
};
/** Metric ids whose units `metricMetadata` declares for the set's fields. */
export const quoteSetMetrics = [
  "clean_price",
  "dirty_price",
  "ytm",
  "ytw",
  "z_spread",
  "discount_margin",
  "oas",
  "asw_par",
  "asw_market",
  "i_spread",
  "japanese_simple_yield",
  "moosmuller_ytm",
] as const;
type Row = readonly [field: string, value: number | null | undefined];
/** An empty field is NaN, which Rust rejects, not the zero `Number("")` would send. */
const typed = (value: string) =>
  value.trim() === "" ? Number.NaN : Number(value);
export interface QuoteConverterProps {
  /** The quote the form starts from. */
  defaultInput: QuoteInput;
  /** Called with the entered quote, for Rust to solve the rest. */
  onSubmit(input: QuoteInput): void;
  /** The native computeQuotes result; null while none is available. */
  quotes: QuoteSet | null;
  /** Native metricMetadata of `quoteSetMetrics`: the units of the set's fields. */
  metadata?: readonly MetricMetadata[];
  loading?: boolean;
  /** The native error of the request (a failed solve, a quote kind the bond lacks), shown as returned. */
  error?: string;
  /** Display text of a returned quote; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Enter any one quote of a bond and see the whole set Rust returns for it. Nothing
 * is converted, compared or derived here; a quote that fails to solve shows the
 * native error.
 */
export function QuoteConverter({
  defaultInput,
  onSubmit,
  quotes,
  metadata,
  loading,
  error,
  formatValue = String,
  density = "compact",
}: QuoteConverterProps) {
  const [[defaultKind, defaultValue]] = Object.entries(defaultInput);
  const [kind, setKind] = useState(defaultKind);
  const [value, setValue] = useState(String(defaultValue));
  const columns = useMemo<ColumnDef<{}, Row, any>[]>(() => {
    const unit = (field: string) => {
      const declared = metadata?.find(
        (entry) => entry.key === (fieldMetric[field] ?? field),
      )?.unit;
      return declared ? ` (${declared})` : "";
    };
    return [
      {
        id: "field",
        header: "Quote",
        accessorFn: ([field]) => field,
        cell: ({ getValue }) => {
          const field = getValue() as string;
          return `${labelFor(field)}${unit(field)}`;
        },
        meta: { className: "", rowHeader: true },
      },
      {
        id: "value",
        header: "Value",
        accessorFn: ([, returned]) => returned,
        cell: ({ getValue }) => {
          const returned = getValue() as number | null | undefined;
          return returned == null ? (
            <span
              className="finstack-na"
              title="This quote does not apply to this bond, or it has no finite value"
            >
              No value
            </span>
          ) : (
            <span title={String(returned)}>{formatValue(returned)}</span>
          );
        },
        meta: { className: "text-right finstack-numeric" },
      },
    ];
  }, [metadata, formatValue]);
  const rows = useMemo<Row[]>(
    () => (quotes ? (Object.entries(quotes) as unknown as Row[]) : []),
    [quotes],
  );
  return (
    <section
      aria-label="Quote converter"
      data-density={density}
      className="@container min-w-0 max-w-full space-y-4 font-sans text-sm text-foreground"
    >
      <form
        aria-label="Quote"
        className="space-y-4"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit({ [kind]: typed(value) } as QuoteInput);
        }}
      >
        <div className="grid gap-4 @lg:grid-cols-2">
          <EnumField
            label="Quote kind"
            help={kinds.find((option) => option.value === kind)?.description}
            value={kind}
            onValueChange={setKind}
            options={kinds}
            layout="select"
          />
          <DecimalInput
            label="Quote value"
            help="Prices as the kind states; yields and spreads as decimals."
            value={value}
            onValueChange={setValue}
          />
        </div>
        <Button type="submit">Convert</Button>
      </form>
      {error ? (
        <p role="alert" className="text-error">
          {error}
        </p>
      ) : loading ? (
        <p role="status" className="text-muted-foreground">
          Solving quotes…
        </p>
      ) : !quotes ? (
        <p role="status" className="text-muted-foreground">
          No quote set
        </p>
      ) : (
        <FinstackTable
          data={rows}
          columns={columns}
          getRowId={([field]) => field}
          caption="The whole quote set"
          density={density}
          emptyState="Rust returned no quotes"
        />
      )}
    </section>
  );
}
