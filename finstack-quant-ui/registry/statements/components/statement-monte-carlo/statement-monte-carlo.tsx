"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { areaY, colorLegend, defineChart, lineY } from "@tanstack/charts";
import { scalePoint } from "@tanstack/charts/scales/point";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { MonteCarloResults } from "finstack-quant-wasm";
import { Button } from "@/components/ui/button";
import { formatRate, groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { monteCarloConfigModule } from "./monte-carlo-config";

/** Rust documents MonteCarloConfig percentiles and breach probabilities as fractions in [0, 1]; shown as percents. */
const fraction = {
  source: "statements::MonteCarloConfig::percentiles (fraction)",
  wire: "decimal",
  display: "percent",
} as const;
const percent = (value: number) => `${formatRate(value, fraction).text}%`;
/** Every supplied digit, grouped; a non-finite native value shows as itself. */
function exactValue(value: number): string {
  return Number.isFinite(value) ? groupDecimal(String(value)) : String(value);
}
/**
 * No standalone native MonteCarloConfig parse exists; Rust validates the
 * config (unknown keys included) when it simulates and the caller shows its
 * error. Kept stable so the form does not revalidate on every render.
 */
const passThrough = async (json: string) => json;
type Pairs = readonly (readonly [number, number])[];
/** The value Rust returned for one percentile, or undefined. */
function at(pairs: Pairs | undefined, percentile: number) {
  return pairs?.find(([p]) => p === percentile)?.[1];
}
interface BandPoint {
  band: string;
  period: string;
  low: number;
  high: number;
}
interface LinePoint {
  percentile: string;
  period: string;
  value: number;
}
interface Row {
  period: string;
  pairs: Pairs;
}
/** One returned warning, flattened from its externally tagged variant. */
function warningText(warning: unknown): string {
  const [kind, fields] = Object.entries(warning as object)[0] ?? ["", {}];
  const detail = Object.entries(fields as object)
    .map(([name, value]) => `${name}: ${String(value)}`)
    .join(", ");
  return `${kind}${detail ? ` · ${detail}` : ""}`;
}
/** A metric's breach test, as requested from the caller. */
export interface BreachRequest {
  metric: string;
  threshold: number;
}
export interface StatementMonteCarloProps {
  /** Initial MonteCarloConfig; remount to load another. Omitted starts empty. */
  defaultConfig?: Record<string, unknown>;
  /** Receives the structurally valid MonteCarloConfig JSON on Simulate; Rust validates it. */
  onSubmit(configJson: string): void;
  /** Native MonteCarloResults; null while none is available. */
  results: MonteCarloResults | null | undefined;
  loading?: boolean;
  /** Native error, shown as returned. */
  error?: string;
  /** Initial metric and threshold of the breach test; omitted starts with the first returned metric and no threshold. */
  defaultBreach?: BreachRequest;
  /** Receives a breach test on Test breach; omitted hides the breach readout. */
  onBreach?: (request: BreachRequest) => void;
  /** Native breach probability: a fraction, null when the results carry no path data for the metric. */
  breachProbability?: number | null;
  breachLoading?: boolean;
  breachError?: string;
  /** Display text of a returned value; defaults to every supplied digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Edits a Rust MonteCarloConfig with the schema form over the generated
 * contract (the uint64 seed stays an exact integer) and presents native
 * MonteCarloResults as returned: for the chosen metric, a fan of bands
 * between the returned percentiles paired from the outside in, with the
 * unpaired middle percentile as a line, across the forecast periods, the percentile table, the breach
 * probability Rust reports for a threshold, and the warnings. Nothing is
 * simulated, interpolated or summed here.
 */
export function StatementMonteCarlo({
  defaultConfig,
  onSubmit,
  results,
  loading,
  error,
  defaultBreach,
  onBreach,
  breachProbability,
  breachLoading,
  breachError,
  formatValue = exactValue,
  density = "compact",
}: StatementMonteCarloProps) {
  const [initial] = useState(() => defaultConfig);
  const metrics = Object.keys(results?.percentile_results ?? {});
  const [chosen, setChosen] = useState(defaultBreach?.metric ?? "");
  const metric = metrics.includes(chosen) ? chosen : (metrics[0] ?? "");
  const [threshold, setThreshold] = useState(
    defaultBreach === undefined ? "" : String(defaultBreach.threshold),
  );
  const [showWarnings, setShowWarnings] = useState(false);
  const series = results?.percentile_results[metric];
  const periods = results?.forecast_periods ?? [];
  const percentiles = results?.percentiles ?? [];
  const chart = useMemo(() => {
    // Bands pair the lowest returned percentile with the highest, inwards.
    const ordered = [...percentiles].sort((a, b) => a - b);
    const bands: BandPoint[] = [];
    const lines: LinePoint[] = [];
    for (const period of periods) {
      const pairs = series?.values[period] as Pairs | undefined;
      for (let i = 0; i < Math.floor(ordered.length / 2); i += 1) {
        const low = at(pairs, ordered[i]!);
        const high = at(pairs, ordered[ordered.length - 1 - i]!);
        if (
          low !== undefined &&
          high !== undefined &&
          Number.isFinite(low) &&
          Number.isFinite(high)
        )
          bands.push({
            band: `${percent(ordered[i]!)}–${percent(ordered[ordered.length - 1 - i]!)}`,
            period,
            low,
            high,
          });
      }
      // An odd count leaves one middle percentile unpaired; it is drawn as a line.
      const middle =
        ordered.length % 2 === 1
          ? ordered[(ordered.length - 1) / 2]
          : undefined;
      const value = middle === undefined ? undefined : at(pairs, middle);
      if (value !== undefined && Number.isFinite(value))
        lines.push({ percentile: percent(middle!), period, value });
    }
    return defineChart({
      marks: [
        areaY(bands, {
          x: (p: BandPoint) => p.period,
          y1: (p: BandPoint) => p.low,
          y2: (p: BandPoint) => p.high,
          z: (p: BandPoint) => p.band,
          fillOpacity: 0.25,
          id: "monte-carlo-bands",
        }),
        lineY(lines, {
          x: (p: LinePoint) => p.period,
          y: (p: LinePoint) => p.value,
          z: (p: LinePoint) => p.percentile,
          key: (p: LinePoint) => `${p.percentile}|${p.period}`,
          id: "monte-carlo-percentiles",
        }),
      ],
      scales: {
        x: {
          scale: () => scalePoint<string>().domain(periods).padding(0.5),
          axis: { label: "Forecast period (returned order)" },
        },
        y: {
          scale: scaleLinear,
          nice: true,
          axis: { label: labelFor(metric) },
        },
      },
      color: { legend: colorLegend() },
      tooltip: {
        use: tooltip,
        content: (marks) => ({
          rows: marks.map((m) => ({
            label:
              "percentile" in m.datum
                ? `${m.datum.percentile} · ${m.datum.period}`
                : `${m.datum.band} · ${m.datum.period}`,
            value:
              "value" in m.datum
                ? formatValue(m.datum.value)
                : `${formatValue(m.datum.low)} – ${formatValue(m.datum.high)}`,
          })),
        }),
      },
    });
  }, [series, periods, percentiles, metric, formatValue]);
  const rows: Row[] = periods.map((period) => ({
    period,
    pairs: (series?.values[period] ?? []) as Pairs,
  }));
  const columns: ColumnDef<{}, Row, any>[] = [
    {
      id: "period",
      header: labelFor("period"),
      accessorFn: (row) => row.period,
      meta: { className: "font-mono", rowHeader: true },
    },
    ...percentiles.map((percentile): ColumnDef<{}, Row, any> => ({
      id: String(percentile),
      header: percent(percentile),
      accessorFn: (row) => at(row.pairs, percentile),
      meta: { className: "text-right finstack-numeric" },
      cell: (context) => {
        const value = context.getValue() as number | undefined;
        return value === undefined ? "—" : formatValue(value);
      },
    })),
  ];
  const warnings = results?.warnings ?? [];
  return (
    <section
      aria-label="Monte Carlo simulation"
      data-density={density}
      className="@container min-w-0 space-y-6 font-sans text-sm text-foreground"
    >
      <SchemaForm
        module={monteCarloConfigModule}
        defaultValues={initial}
        layout="full"
        explicitNull={false}
        validate={passThrough}
        onSubmit={onSubmit}
        submitLabel="Simulate"
      />
      {error ? (
        <p role="alert" className="text-sm text-error">
          {error}
        </p>
      ) : !results ? (
        <p role="status" className="text-sm text-muted-foreground">
          {loading ? "Simulating paths…" : "No Monte Carlo results available."}
        </p>
      ) : (
        <div
          role="group"
          aria-label="Returned Monte Carlo results"
          className="min-w-0 space-y-6"
        >
          <p className="text-xs text-muted-foreground">
            {results.n_paths} paths over {periods.join(", ")}. Only forecasts
            with a random draw vary across paths.
          </p>
          <div className="max-w-md">
            <EnumField
              label="Metric"
              layout="select"
              value={metric}
              onValueChange={setChosen}
              options={metrics.map((id) => ({ value: id, label: id }))}
            />
          </div>
          {series && periods.length > 0 && (
            <figure className="min-w-0">
              <FinstackChart
                definition={chart}
                ariaLabel={`${labelFor(metric)} percentile fan`}
              />
            </figure>
          )}
          <FinstackTable
            data={rows}
            columns={columns}
            getRowId={(row) => row.period}
            caption={`${labelFor(metric)} percentiles by forecast period`}
            density={density}
            emptyState="No forecast periods returned"
          />
          {onBreach && (
            <section
              aria-label="Breach probability"
              className="space-y-3 rounded-md border border-border p-3"
            >
              <h3 className="text-sm font-medium">Breach probability</h3>
              <div className="flex flex-wrap items-end gap-3">
                <div className="w-56">
                  <DecimalInput
                    label="Threshold"
                    help={`Breach level of ${metric} in its own units: Rust counts the paths above it in any forecast period.`}
                    value={threshold}
                    onValueChange={setThreshold}
                  />
                </div>
                <Button
                  type="button"
                  variant="outline"
                  disabled={!metric}
                  onClick={() =>
                    onBreach({
                      metric,
                      // A cleared threshold is NaN, never 0, so Rust rejects it.
                      threshold:
                        threshold.trim() === ""
                          ? Number.NaN
                          : Number(threshold),
                    })
                  }
                >
                  Test breach
                </Button>
              </div>
              {breachError ? (
                <p role="alert" className="text-sm text-error">
                  {breachError}
                </p>
              ) : breachLoading ? (
                <p role="status" className="text-sm text-muted-foreground">
                  Testing the paths…
                </p>
              ) : breachProbability === null ? (
                <p className="text-sm text-muted-foreground">
                  Rust reports no probability: these results carry no path data
                  for the metric (simulate with include path data).
                </p>
              ) : breachProbability !== undefined ? (
                <p className="text-sm">
                  <span className="finstack-numeric text-base font-medium">
                    {percent(breachProbability)}
                  </span>{" "}
                  <span className="text-muted-foreground">
                    of paths ({String(breachProbability)})
                  </span>
                </p>
              ) : null}
            </section>
          )}
          <details
            onToggle={(event) =>
              setShowWarnings((event.target as HTMLDetailsElement).open)
            }
          >
            <summary>Simulation warnings ({warnings.length})</summary>
            {showWarnings && (
              <ul
                aria-label="Simulation warnings"
                className="mt-2 max-h-64 space-y-0.5 overflow-y-auto text-xs text-muted-foreground"
              >
                {warnings.map((warning, index) => (
                  <li key={index}>{warningText(warning)}</li>
                ))}
              </ul>
            )}
          </details>
        </div>
      )}
    </section>
  );
}
