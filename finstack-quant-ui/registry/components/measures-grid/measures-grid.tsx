"use client";
import { FinstackTable } from "../../primitives/finstack-table/finstack-table";
import { MeasureValue } from "../../primitives/measure-value/measure-value";
import {
  ValuationSummary,
  type ValuationSummaryProps,
} from "../valuation-summary/valuation-summary";
import type { ColumnDef } from "@tanstack/react-table";
export interface MeasuresGridProps extends ValuationSummaryProps {
  /** Pass the groups returned by listStandardMetricsGrouped; no local financial taxonomy. */
  groups: Readonly<Record<string, readonly string[]>>;
  /** Optional source-backed units by complete metric key. Values are always displayed raw. */
  units?: Readonly<Record<string, { label: string; source: string }>>;
  comparisonUnits?: Readonly<Record<string, { label: string; source: string }>>;
}
type MeasureRow = {
  key: string;
  value: number | undefined;
  comparison: number | undefined;
};
/** Group by the canonical metric family prefix while retaining the complete returned key. */
export function groupMeasures(
  props: Pick<MeasuresGridProps, "result" | "compareTo" | "groups">,
) {
  const grouped = new Map<string, MeasureRow[]>();
  const keys = new Set([
    ...Object.keys(props.result?.measures ?? {}),
    ...Object.keys(props.compareTo?.measures ?? {}),
  ]);
  for (const key of keys) {
    const matches = Object.entries(props.groups).filter(([, metrics]) =>
      metrics.some((metric) => key === metric || key.startsWith(`${metric}::`)),
    );
    const group = matches.length === 1 ? matches[0][0] : "Group unavailable";
    const rows = grouped.get(group) ?? [];
    rows.push({
      key,
      value: props.result?.measures[key],
      comparison: props.compareTo?.measures[key],
    });
    grouped.set(group, rows);
  }
  return [...new Set([...Object.keys(props.groups), "Group unavailable"])]
    .filter((group) => grouped.has(group))
    .map((group) => ({ group, rows: grouped.get(group)! }));
}
/** Supplied measures and independent comparison context, using the unlinked core table. */
export function MeasuresGrid(props: MeasuresGridProps) {
  const grouped = groupMeasures(props);
  const noUnits =
    !Object.keys(props.units ?? {}).length &&
    !Object.keys(props.comparisonUnits ?? {}).length;
  const columns = (group: string): ColumnDef<{}, MeasureRow, any>[] => [
    {
      id: "key",
      accessorFn: (row) => row.key,
      header: () => <span className="font-sans">{group}</span>,
      meta: { className: "text-left" },
      cell: (context) => (
        <span className="finstack-measure-key font-mono">
          {context.getValue()}
        </span>
      ),
    },
    {
      id: "value",
      header:
        props.compareTo === undefined ? "Raw value" : "Valuation · raw value",
      accessorFn: (row) => row.value,
      meta: { className: "text-right finstack-numeric" },
      cell: (context) => (
        <MeasureValue
          value={context.row.original.value}
          unit={props.units?.[context.row.original.key]}
          showUnavailableUnit={!noUnits}
        />
      ),
    },
    ...(props.compareTo === undefined
      ? []
      : [
          {
            id: "comparison",
            header: "Comparison · raw value",
            accessorFn: (row: MeasureRow) => row.comparison,
            meta: { className: "text-right finstack-numeric" },
            cell: (context: { row: { original: MeasureRow } }) => (
              <MeasureValue
                value={context.row.original.comparison}
                unit={props.comparisonUnits?.[context.row.original.key]}
                showUnavailableUnit={!noUnits}
              />
            ),
          },
        ]),
  ];
  return (
    <div
      className="finstack-measures font-sans text-foreground"
      data-density={props.density}
    >
      <ValuationSummary
        result={props.result}
        compareTo={props.compareTo}
        compact
        model={props.model}
        comparisonModel={props.comparisonModel}
        density={props.density}
        loading={props.loading}
        error={props.error}
      />
      {grouped.length ? (
        grouped.map(({ group, rows }) => (
          <section
            key={group}
            className="finstack-measures__group"
            aria-label={`${group} measures`}
          >
            <FinstackTable
              data={rows}
              columns={columns(group)}
              getRowId={(row) => row.key}
              caption={`${group} measures`}
              captionVisibility="sr-only"
            />
          </section>
        ))
      ) : (
        <p role="status" className="text-sm text-muted-foreground">
          No measures supplied
        </p>
      )}
      {grouped.length > 0 && noUnits && (
        <p className="text-xs text-muted-foreground">
          Raw measure values · units unavailable
        </p>
      )}
    </div>
  );
}
