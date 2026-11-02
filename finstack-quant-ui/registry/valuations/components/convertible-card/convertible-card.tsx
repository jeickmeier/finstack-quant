"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { colorLegend, defineChart, dot, lineY, ruleX } from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import type { MetricMetadata, valuations } from "finstack-quant-wasm";
import { formatRaw, groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import {
  MetricValuesTable,
  type MetricValuesTableProps,
} from "@/components/finstack/valuations/components/metric-values-table/metric-values-table";

type Ladder = ReturnType<typeof valuations.instruments.convertibleSpotLadder>;
type Row = Ladder["rows"][number];
/** The ids the card shows, in display order: conversion terms, Greeks, then spreads and risk. */
export const convertibleMetrics = [
  "parity",
  "conversion_value",
  "conversion_premium",
  "bond_floor",
  "delta",
  "gamma",
  "vega",
  "rho",
  "theta",
  "implied_vol",
  "dv01",
  "cs01",
  "oas",
  "dividend01",
  "conversion01",
  "accrued",
  "clean_price",
] as const;
/** Metric ids whose units `metricMetadata` declares for the ladder's ratio columns. */
export const ladderMetrics = ["parity", "conversion_premium"] as const;
export interface ConvertibleCardProps extends Omit<
  MetricValuesTableProps,
  "label" | "note"
> {
  /** The native convertibleSpotLadder result; null while none is available. */
  ladder: Ladder | null;
  /** Native metricMetadata of `ladderMetrics`: the column units. */
  ladderMetadata?: readonly MetricMetadata[];
  ladderLoading?: boolean;
  /** The native error of the ladder (a share price Rust rejects, a missing curve), shown as returned. */
  ladderError?: string;
  /** Receives the share prices entered, or null for Rust's default ladder; omit to hide the form. */
  onSpotsChange?: (spots: readonly number[] | null) => void;
}
const undefinedValue = (
  <span className="finstack-na" title="Rust returned no value for this cell">
    Undefined
  </span>
);
/** Share prices as entered, unchecked: Rust rejects, sorts and dedupes them. */
const parseSpots = (text: string) => {
  const entries = text.split(/[\s,]+/).filter(Boolean);
  return entries.length ? entries.map(Number) : null;
};
const series = {
  price: labelFor("price"),
  conversionValue: labelFor("conversion_value"),
  bondFloor: labelFor("bond_floor"),
  parity: labelFor("parity"),
  premium: labelFor("conversion_premium"),
} as const;
const spotScale = (rows: readonly Row[], label: string) => ({
  scale: scaleLinear,
  domain: [rows[0]!.spot, rows.at(-1)!.spot],
  axis: { label },
});
const marketRule = (rows: readonly Row[]) =>
  ruleX(
    rows.filter((row) => row.is_market_spot).map((row) => row.spot),
    {
      id: "market-spot",
      stroke: "var(--foreground)",
      strokeDasharray: "4 3",
    },
  );
/** Price, conversion value and floor by share price, each dot on the price the regime Rust returned. */
function valueChart(ladder: Ladder) {
  const { rows } = ladder;
  const points = rows.flatMap((row) => [
    { spot: row.spot, value: row.price, line: series.price },
    {
      spot: row.spot,
      value: row.conversion_value,
      line: series.conversionValue,
    },
    { spot: row.spot, value: row.bond_floor, line: series.bondFloor },
  ]);
  const regimes = [
    ...new Set(rows.flatMap((row) => (row.regime ? [row.regime] : []))),
  ].map((regime) => labelFor(regime));
  return defineChart({
    marks: [
      marketRule(rows),
      lineY(points, {
        x: (point) => point.spot,
        y: (point) => point.value,
        z: (point) => point.line,
        color: (point) => point.line,
        key: (point) => `${point.line}|${point.spot}`,
        id: "value-lines",
      }),
      dot(
        rows.filter((row) => row.regime),
        {
          x: (row: Row) => row.spot,
          y: (row: Row) => row.price,
          color: (row: Row) => labelFor(row.regime as string),
          key: (row: Row) => row.spot,
          id: "regime",
          r: 5,
        },
      ),
    ],
    scales: {
      x: spotScale(rows, labelFor("spot")),
      y: {
        scale: scaleLinear,
        nice: true,
        axis: { label: ladder.currency },
      },
    },
    color: {
      domain: [
        series.price,
        series.conversionValue,
        series.bondFloor,
        ...regimes,
      ],
      legend: colorLegend(),
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks.flatMap((mark) => {
          const row = mark.datum as Row;
          return row.regime
            ? [
                { label: labelFor("spot"), value: formatRaw(row.spot) },
                { label: labelFor("price"), value: formatRaw(row.price) },
                { label: labelFor("regime"), value: labelFor(row.regime) },
              ]
            : [];
        }),
      }),
    },
  });
}
/** Parity and conversion premium by share price, a null premium left out. */
function ratioChart(rows: readonly Row[]) {
  const points = rows.flatMap((row) => [
    { spot: row.spot, value: row.parity, line: series.parity },
    ...(row.conversion_premium == null
      ? []
      : [
          {
            spot: row.spot,
            value: row.conversion_premium,
            line: series.premium,
          },
        ]),
  ]);
  return defineChart({
    marks: [
      marketRule(rows),
      lineY(points, {
        x: (point) => point.spot,
        y: (point) => point.value,
        z: (point) => point.line,
        color: (point) => point.line,
        key: (point) => `${point.line}|${point.spot}`,
        id: "ratio-lines",
      }),
    ],
    scales: {
      x: spotScale(rows, labelFor("spot")),
      y: { scale: scaleLinear, nice: true, axis: { label: "Ratio" } },
    },
    color: {
      domain: [series.parity, series.premium],
      legend: colorLegend(),
    },
  });
}
/**
 * A convertible by share price: price, parity, floor, premium and regime from one
 * native convertibleSpotLadder, then metrics by id. The share prices are the ones
 * entered, or Rust's default ladder. Nothing is classified, compared or derived:
 * the regime, the premium and the market row are what Rust returned.
 */
export function ConvertibleCard({
  ladder,
  ladderMetadata,
  ladderLoading,
  ladderError,
  onSpotsChange,
  formatValue = String,
  ...table
}: ConvertibleCardProps) {
  const [spots, setSpots] = useState("");
  const unit = (metric: string) => {
    const declared = ladderMetadata?.find(
      (entry) => entry.key === metric,
    )?.unit;
    return declared ? ` (${declared})` : "";
  };
  const columns = useMemo<ColumnDef<{}, Row, any>[]>(() => {
    const number = (value: number | null | undefined) =>
      value == null ? (
        undefinedValue
      ) : (
        <span title={String(value)}>{formatValue(value)}</span>
      );
    const money = (value: number) => (
      <span title={String(value)}>{groupDecimal(formatValue(value))}</span>
    );
    const numeric = { className: "text-right finstack-numeric" };
    return [
      {
        id: "spot",
        header: labelFor("spot"),
        accessorFn: (row) => row,
        cell: ({ getValue }) => {
          const row = getValue() as Row;
          return row.is_market_spot ? (
            <strong className="font-semibold">{row.spot} (market)</strong>
          ) : (
            row.spot
          );
        },
        meta: { ...numeric, rowHeader: true },
      },
      {
        id: "price",
        header: labelFor("price"),
        accessorFn: (row) => row.price,
        cell: ({ getValue }) => money(getValue() as number),
        meta: numeric,
      },
      {
        id: "delta",
        header: labelFor("delta"),
        accessorFn: (row) => row.delta,
        cell: ({ getValue }) => number(getValue() as number | null),
        meta: numeric,
      },
      {
        id: "hedge_ratio",
        header: labelFor("hedge_ratio"),
        accessorFn: (row) => row.hedge_ratio,
        cell: ({ getValue }) => number(getValue() as number | null),
        meta: numeric,
      },
      {
        id: "regime",
        header: labelFor("regime"),
        accessorFn: (row) => row.regime,
        cell: ({ getValue }) => {
          const regime = getValue() as string | null;
          return regime ? labelFor(regime) : undefinedValue;
        },
        meta: { className: "" },
      },
      {
        id: "parity",
        header: `${labelFor("parity")}${unit("parity")}`,
        accessorFn: (row) => row.parity,
        cell: ({ getValue }) => number(getValue() as number),
        meta: numeric,
      },
      {
        id: "conversion_premium",
        header: `${labelFor("conversion_premium")}${unit("conversion_premium")}`,
        accessorFn: (row) => row.conversion_premium,
        cell: ({ getValue }) => number(getValue() as number | null),
        meta: numeric,
      },
    ];
    // `unit` reads only ladderMetadata.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [formatValue, ladderMetadata]);
  const charts = useMemo(
    () =>
      ladder && ladder.rows.length > 1
        ? {
            value: valueChart(ladder),
            ratio: ratioChart(ladder.rows),
          }
        : null,
    [ladder],
  );
  const heading = "Convertible by share price";
  return (
    <div className="min-w-0 max-w-full space-y-3">
      <section
        aria-label={heading}
        className="space-y-3 font-sans text-sm text-foreground"
      >
        {onSpotsChange && (
          <form
            className="flex flex-wrap items-end gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              onSpotsChange(parseSpots(spots));
            }}
          >
            <label className="grid gap-1">
              <span>
                Share prices, comma separated; empty uses Rust&apos;s ladder
              </span>
              <Input
                value={spots}
                onChange={(event) => setSpots(event.target.value)}
                inputMode="decimal"
                className="w-72"
              />
            </label>
            <Button type="submit" variant="outline">
              Apply
            </Button>
          </form>
        )}
        {ladderError ? (
          <p role="alert" className="m-0 text-destructive">
            {ladderError}
          </p>
        ) : ladder ? (
          <>
            {charts && (
              <>
                <FinstackChart
                  definition={charts.value}
                  ariaLabel="Price, conversion value and bond floor by share price, with each regime"
                  subtitle={`${ladder.currency}; the dashed line is the market share price of ${ladder.market_spot}`}
                  height={300}
                />
                <FinstackChart
                  definition={charts.ratio}
                  ariaLabel="Parity and conversion premium by share price"
                  subtitle={`${series.parity}${unit("parity")} and ${series.premium}${unit("conversion_premium")}`}
                  height={240}
                />
              </>
            )}
            <FinstackTable
              columns={columns}
              data={ladder.rows as Row[]}
              getRowId={(row) => String(row.spot)}
              caption={`${ladder.instrument_id} by share price, as of ${ladder.as_of}; price in ${ladder.currency}, delta per unit of share price`}
              density={table.density}
            />
            <p className="m-0 text-sm text-muted-foreground">
              Each row reprices with only the share price moved: volatility,
              rates and credit stay as supplied. The floor does not move with
              the share price, so the chart and the metric row below carry it.
              Hedge ratio is delta over the conversion ratio, a decimal. Regime
              reads it on the 40 and 80 delta bands, one market convention. A
              mandatory-variable bond delivers a moving share count, so it has
              neither.
            </p>
          </>
        ) : (
          ladderLoading && (
            <p role="status" className="m-0 text-muted-foreground">
              Valuing by share price…
            </p>
          )
        )}
      </section>
      <MetricValuesTable
        {...table}
        formatValue={formatValue}
        label="Convertible"
        note={
          <>
            Values as Rust returned them on the tree model. OAS and implied vol
            solve to a quoted clean price; CS01 needs a hazard curve with a
            calibration recipe, so its row otherwise shows Rust&apos;s error.
            Parity is conversion value over notional: 1.0 is par, not one
            dollar.
          </>
        }
      />
    </div>
  );
}
