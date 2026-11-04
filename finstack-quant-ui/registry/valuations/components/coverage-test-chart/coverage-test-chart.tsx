"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import {
  colorLegend,
  defineChart,
  dot,
  ruleX,
  ruleY,
  tickY,
} from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import { scaleUtc } from "d3-scale";
import type {
  MetricFieldUnit,
  SimulationFieldUnits,
  StructuredCredit,
} from "finstack-quant-wasm";
import { formatRaw, groupDecimal } from "@/lib/finstack/format/format";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import {
  utc,
  yesNo,
} from "@/components/finstack/shared/primitives/native-display/native-display";

type SimulationRun = ReturnType<StructuredCredit["runSimulation"]>;
type Diagnostics = SimulationRun["diagnostics"];
type Period = NonNullable<Diagnostics["periods"]>[number];
type Breach = Diagnostics["first_breaches"][number];
type TestRow = Period["coverage_tests"][number] & { payment_date: string };
type DiversionRow = NonNullable<
  Period["waterfall"]["diverted_amounts"]
>[number] & {
  payment_date: string;
};
const axisLabel = (unit: MetricFieldUnit) =>
  `${labelFor(unit.field)} (${unit.label})`;
const numeric = { className: "text-right finstack-numeric" };
/** What Rust's null ratio and cushion mean, per CoverageTestDiagnostic. */
const noClaim = "No claim outstanding";
const orNoClaim = (value: number | null | undefined) =>
  value == null ? noClaim : formatRaw(value);
/** The date scale all of a deal's panels share: Rust's first and last payment dates, so a test's missing dates stay visible. */
const dateScale = (periods: readonly Period[]) => ({
  scale: scaleUtc,
  domain: [utc(periods[0]!.payment_date), utc(periods.at(-1)!.payment_date)],
  axis: { label: "Payment date" },
});
function testChart(
  rows: readonly TestRow[],
  periods: readonly Period[],
  breach: string | null | undefined,
  field: "ratio" | "cushion",
  unit: MetricFieldUnit,
) {
  const channels = {
    x: (row: TestRow) => utc(row.payment_date),
    key: (row: TestRow) => row.payment_date,
  };
  return defineChart({
    marks: [
      ...(breach
        ? [
            ruleX([utc(breach)], {
              id: "first-breach",
              stroke: "var(--error)",
              strokeDasharray: "4 3",
            }),
          ]
        : []),
      field === "ratio"
        ? tickY(rows, {
            ...channels,
            y: (row: TestRow) => row.trigger_level,
            id: "trigger-level",
            stroke: "var(--foreground)",
            strokeWidth: 2,
            length: 14,
          })
        : ruleY([0], { id: "zero-cushion", stroke: "var(--foreground)" }),
      // A null is Rust's "no claim outstanding": a date with nothing to plot.
      dot(
        rows.filter((row) => row[field] != null),
        {
          ...channels,
          y: (row: TestRow) => row[field] as number,
          id: field,
          r: 4,
          // Rust's flag. Rows are not connected: a test can be absent between them.
          color: (row: TestRow) => (row.passing ? "Passed" : "Failed"),
        },
      ),
    ],
    scales: {
      x: dateScale(periods),
      y: { scale: scaleLinear, nice: true, axis: { label: axisLabel(unit) } },
    },
    // A fixed order, so a test that fails first keeps each flag's colour.
    color: { domain: ["Passed", "Failed"], legend: colorLegend() },
    tooltip: {
      use: tooltip,
      content: (marks) => {
        const row = marks
          .map((mark) => mark.datum as TestRow | Date | number)
          .find((datum): datum is TestRow => Object(datum).test_id);
        return {
          rows: row
            ? [
                { label: labelFor("payment_date"), value: row.payment_date },
                { label: labelFor("ratio"), value: orNoClaim(row.ratio) },
                {
                  label: labelFor("trigger_level"),
                  value: formatRaw(row.trigger_level),
                },
                { label: labelFor("cushion"), value: orNoClaim(row.cushion) },
              ]
            : [],
        };
      },
    },
  });
}
/** Rust's diverted cash on each period, zero included. */
function diversionChart(periods: readonly Period[], unit: MetricFieldUnit) {
  return defineChart({
    marks: [
      ruleY([0], { id: "no-diversion", stroke: "var(--foreground)" }),
      dot(periods, {
        x: (period: Period) => utc(period.payment_date),
        y: (period: Period) => Number(period.waterfall.diverted_cash.amount),
        key: (period: Period) => period.payment_date,
        id: "diverted-cash",
        r: 4,
        fill: "var(--primary)",
      }),
    ],
    scales: {
      x: dateScale(periods),
      y: { scale: scaleLinear, nice: true, axis: { label: axisLabel(unit) } },
    },
    tooltip: {
      use: tooltip,
      content: (marks) => ({
        rows: marks
          .map((mark) => mark.datum as Period | number)
          .filter((datum): datum is Period => Object(datum).waterfall)
          .map(({ payment_date, waterfall }) => ({
            label: payment_date,
            value: `${groupDecimal(
              waterfall.diverted_cash.amount,
            )} · coverage tier failed: ${yesNo(waterfall.had_diversions)}`,
          })),
      }),
    },
  });
}
const breachColumns: ColumnDef<{}, Breach, any>[] = [
  {
    id: "test_id",
    header: labelFor("test_id"),
    accessorFn: (row) => row.test_id,
    meta: { className: "", rowHeader: true },
  },
  {
    id: "first_breach",
    header: labelFor("first_breach"),
    accessorFn: (row) => row.first_breach ?? "Never failed",
  },
];
function testColumns(
  units: readonly MetricFieldUnit[],
): ColumnDef<{}, TestRow, any>[] {
  return [
    {
      id: "payment_date",
      header: labelFor("payment_date"),
      accessorFn: (row) => row.payment_date,
      meta: { className: "", rowHeader: true },
    },
    ...units.map((unit): ColumnDef<{}, TestRow, any> => ({
      id: unit.field,
      header: () => <span title={unit.source}>{axisLabel(unit)}</span>,
      accessorFn: (row) => orNoClaim(row[unit.field as "ratio"]),
      meta: numeric,
    })),
    {
      id: "passing",
      header: labelFor("passing"),
      accessorFn: (row) => yesNo(row.passing),
    },
  ];
}
function diversionColumns(
  unit: MetricFieldUnit,
): ColumnDef<{}, DiversionRow, any>[] {
  return [
    {
      id: "payment_date",
      header: labelFor("payment_date"),
      accessorFn: (row) => row.payment_date,
      meta: { className: "", rowHeader: true },
    },
    {
      id: "source_tier",
      header: labelFor("source_tier"),
      accessorFn: (row) => row.source_tier,
    },
    {
      id: "target_tranche",
      header: labelFor("target_tranche"),
      accessorFn: (row) => row.target_tranche,
    },
    {
      id: "amount",
      header: () => <span title={unit.source}>Amount ({unit.label})</span>,
      accessorFn: (row) => groupDecimal(row.amount.amount),
      meta: numeric,
    },
    {
      id: "reason",
      header: labelFor("reason"),
      accessorFn: (row) => row.reason,
      meta: { className: "whitespace-normal" },
    },
  ];
}
function Heading({ children }: { children: string }) {
  return <h4 className="m-0 text-sm font-medium">{children}</h4>;
}
function TestDetail({
  breach,
  allRows,
  periods,
  units,
  density,
}: {
  breach: Breach;
  allRows: readonly TestRow[];
  periods: readonly Period[];
  units: SimulationFieldUnits;
  density?: "compact" | "comfortable";
}) {
  const rows = useMemo(
    () => allRows.filter((row) => row.test_id === breach.test_id),
    [allRows, breach.test_id],
  );
  const [ratio, , cushion] = units.coverage_tests;
  const unclaimed = rows.filter((row) => row.ratio == null).length;
  const charts = useMemo(
    () =>
      periods[1] && rows[0]
        ? {
            ratio: testChart(
              rows,
              periods,
              breach.first_breach,
              "ratio",
              ratio!,
            ),
            cushion: testChart(
              rows,
              periods,
              breach.first_breach,
              "cushion",
              cushion!,
            ),
          }
        : null,
    [rows, periods, breach.first_breach, ratio, cushion],
  );
  return (
    <div className="flex flex-col gap-3">
      <p className="m-0 text-sm">
        {breach.first_breach
          ? `First breach ${breach.first_breach}. A later cure does not clear it.`
          : "This test never failed in this simulation."}
      </p>
      {rows.length < periods.length && (
        <p role="status" className="m-0 text-sm text-muted-foreground">
          Recorded on {rows.length} of {periods.length} dates.
        </p>
      )}
      {unclaimed > 0 && (
        <p role="status" className="m-0 text-sm text-muted-foreground">
          {noClaim} on {unclaimed} of the {rows.length} recorded dates, so no
          dot is drawn on them.
        </p>
      )}
      {rows[0] && (
        <>
          {charts && (
            <>
              <FinstackChart
                definition={charts.ratio}
                ariaLabel={`${breach.test_id} ratio against trigger by date`}
                subtitle="Dots: ratio, on the dates Rust recorded it. Dashes: trigger. Dashed line: first breach."
                height={240}
              />
              <FinstackChart
                definition={charts.cushion}
                ariaLabel={`${breach.test_id} cushion by date`}
                height={180}
              />
            </>
          )}
          <FinstackTable
            caption={`${breach.test_id} by payment date`}
            data={rows}
            columns={testColumns(units.coverage_tests)}
            getRowId={(row) => row.payment_date}
            density={density}
          />
        </>
      )}
    </div>
  );
}
export interface CoverageTestChartProps {
  /** Native StructuredCredit.runSimulation result; null while none is available. */
  run: SimulationRun | null;
  /** Native simulationFieldUnits for the deal currency; null while none is available. */
  units: SimulationFieldUnits | null;
  /** Test shown first; omitted is the first test Rust lists. */
  initialTest?: string;
  loading?: boolean;
  /** The native error of the simulation, shown as returned. */
  error?: string;
  density?: "compact" | "comfortable";
}
/**
 * One coverage test at a time from SimulationDiagnostics.first_breaches: ratio against
 * trigger and cushion on the dates Rust recorded it, with Rust's first breach date,
 * then the deal's diverted cash by date. Diversions belong to a waterfall tier, not to
 * one test, so they are not attributed to the selected test. Nothing is computed,
 * joined by date, sorted or classified.
 */
export function CoverageTestChart({
  run,
  units,
  initialTest,
  loading,
  error,
  density,
}: CoverageTestChartProps) {
  const [chosen, setChosen] = useState(initialTest);
  const periods = useMemo(() => run?.diagnostics.periods ?? [], [run]);
  const rows = useMemo(
    () =>
      periods.flatMap(({ payment_date, coverage_tests }) =>
        coverage_tests.map((test) => ({ payment_date, ...test })),
      ),
    [periods],
  );
  const diversions = useMemo(
    () =>
      periods.flatMap(
        ({ payment_date, waterfall }) =>
          waterfall.diverted_amounts?.map((diversion) => ({
            payment_date,
            ...diversion,
          })) ?? [],
      ),
    [periods],
  );
  const unrecorded = periods.filter(
    ({ waterfall }) => !waterfall.diverted_amounts,
  ).length;
  const cash = useMemo(
    () =>
      periods[1] && units ? diversionChart(periods, units.diverted_cash) : null,
    [periods, units],
  );
  if (error)
    return (
      <p role="alert" className="text-sm text-error">
        {error}
      </p>
    );
  if (!run)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        {loading ? "Calculating…" : "No simulation returned"}
      </p>
    );
  const breaches = run.diagnostics.first_breaches;
  if (!breaches[0])
    return (
      <p role="status" className="text-sm text-muted-foreground">
        No coverage test was evaluated.
      </p>
    );
  if (!periods[0])
    return (
      <p role="status" className="text-sm text-muted-foreground">
        The simulation returned no periods
      </p>
    );
  if (!units)
    return (
      <p role="status" className="text-sm text-muted-foreground">
        {loading ? "Calculating…" : "No units returned"}
      </p>
    );
  const selected =
    breaches.find((breach) => breach.test_id === chosen) ?? breaches[0];
  return (
    <section
      aria-label="Coverage tests"
      aria-busy={loading}
      className="flex min-w-0 flex-col gap-4 font-sans text-base text-foreground"
      data-density={density}
    >
      <FinstackTable
        caption="First breach by test"
        data={breaches}
        columns={breachColumns}
        getRowId={(row) => row.test_id}
        density={density}
      />
      <div className="w-72">
        <EnumField
          label="Test"
          value={selected.test_id}
          onValueChange={setChosen}
          options={breaches.map(({ test_id }) => ({
            value: test_id,
            label: test_id,
          }))}
          layout="select"
        />
      </div>
      {!periods[1] && (
        <p role="status" className="m-0 text-sm text-muted-foreground">
          The simulation returned one period, on {periods[0].payment_date}. A
          chart by date needs two, so what it returned is listed instead.
        </p>
      )}
      <TestDetail
        breach={selected}
        allRows={rows}
        periods={periods}
        units={units}
        density={density}
      />
      <div className="flex flex-col gap-3">
        <Heading>Cash diverted by the deal's coverage tiers</Heading>
        {cash && (
          <FinstackChart
            definition={cash}
            ariaLabel="Diverted cash by date"
            height={180}
          />
        )}
        {unrecorded > 0 && (
          <p role="status" className="m-0 text-sm text-muted-foreground">
            Diversion records were not returned on {unrecorded} of{" "}
            {periods.length} dates.
          </p>
        )}
        <FinstackTable
          caption="Diversions"
          data={diversions}
          columns={diversionColumns(units.diverted_cash)}
          getRowId={(row) => String(diversions.indexOf(row))}
          emptyState="No cash was diverted"
          density={density}
        />
      </div>
    </section>
  );
}
