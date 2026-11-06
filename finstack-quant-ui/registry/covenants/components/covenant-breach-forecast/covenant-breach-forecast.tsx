"use client";
import { useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import type { FutureBreachWire } from "@/lib/finstack/generated/types/future_breach";
import { formatExact } from "@/lib/finstack/format/format";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { covenantForecastConfigModule } from "./covenant-forecast-config";
import { passThrough } from "@/components/finstack/shared/primitives/native-display/native-display";

/** Numeric FutureBreach fields, in the metric's own units (the probability as a fraction) as Rust returns them. */
export type FutureBreachNumber =
  "projected_value" | "threshold" | "headroom" | "breach_probability";
export interface CovenantBreachForecastProps {
  /** Initial CovenantForecastConfig text; omitted starts empty, which takes Rust's deterministic maintenance-scope defaults. Remount to load another document. */
  defaultConfigJson?: string;
  /** Receives the config JSON on Forecast; connect it to the useForecastBreaches request. */
  onSubmit(json: string): void;
  /** Native FutureBreach rows (statements_analytics.forecastBreaches) in the order supplied; null before a forecast. */
  breaches?: readonly FutureBreachWire[] | null;
  loading?: boolean;
  /** The native error of the last forecast. */
  error?: string;
  /** Display text of a numeric field; defaults to every supplied digit. */
  formatValue?: (field: FutureBreachNumber, value: number) => string;
  density?: "compact" | "comfortable";
}
type Row = { key: string; breach: FutureBreachWire };
/**
 * Forecasts covenant breaches under a CovenantForecastConfig edited with the
 * generated form, and presents the returned FutureBreach rows as supplied
 * (Rust sorts them by date, then covenant id): date, covenant, Rust's
 * description, projected value, threshold, headroom and breach probability.
 * An empty config is Rust's deterministic default. Nothing is computed.
 */
export function CovenantBreachForecast({
  defaultConfigJson,
  onSubmit,
  breaches,
  loading,
  error,
  formatValue = (_field, value) => formatExact(value),
  density = "compact",
}: CovenantBreachForecastProps) {
  const [initial] = useState<Record<string, unknown> | undefined>(() =>
    defaultConfigJson === undefined
      ? undefined
      : (covenantForecastConfigModule.codec.parse(defaultConfigJson) as Record<
          string,
          unknown
        >),
  );
  const numeric = { className: "text-right finstack-numeric" };
  const number = (field: FutureBreachNumber) => (row: Row) => {
    const value = row.breach[field];
    return value == null ? "—" : formatValue(field, value);
  };
  const columns: ColumnDef<{}, Row, any>[] = [
    {
      id: "breach_date",
      header: "Date",
      accessorFn: (row) => row.breach.breach_date,
      meta: { className: "whitespace-nowrap tabular-nums", rowHeader: true },
    },
    {
      id: "covenant_id",
      header: "Covenant",
      accessorFn: (row) => row.breach.covenant_id,
    },
    {
      id: "covenant_description",
      header: "Test",
      accessorFn: (row) => row.breach.covenant_description,
    },
    {
      id: "projected_value",
      header: "Projected",
      accessorFn: number("projected_value"),
      meta: numeric,
    },
    {
      id: "threshold",
      header: "Threshold",
      accessorFn: number("threshold"),
      meta: numeric,
    },
    {
      id: "headroom",
      header: "Headroom",
      accessorFn: number("headroom"),
      meta: numeric,
    },
    {
      id: "breach_probability",
      header: "Probability",
      accessorFn: number("breach_probability"),
      meta: numeric,
    },
  ];
  return (
    <section
      aria-label="Covenant breach forecast"
      data-density={density}
      className="@container min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      <SchemaForm
        module={covenantForecastConfigModule}
        defaultValues={initial}
        layout="full"
        explicitNull={false}
        validate={passThrough}
        onSubmit={onSubmit}
        submitLabel="Forecast"
      />
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !breaches ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Forecasting breaches…" : "No breach forecast yet."}
        </p>
      ) : (
        <FinstackTable
          data={breaches.map((breach, index) => ({
            key: `${index}`,
            breach,
          }))}
          columns={columns}
          getRowId={(row) => row.key}
          caption="Forecast covenant breaches"
          emptyState="Rust forecast no breaches"
          density={density}
        />
      )}
    </section>
  );
}
