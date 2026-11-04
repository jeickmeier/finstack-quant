"use client";
import { useMemo, useState } from "react";
import type { ColumnDef } from "@tanstack/react-table";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { formatRaw } from "@/lib/finstack/format/format";
import { FieldFrame } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { typedNumber } from "@/components/finstack/shared/primitives/native-display/native-display";

/** A native result or the native error of that one point. */
type Outcome<T> =
  { ok: true; value: T } | { ok: false; error: { message: string } };
/** Years from the curve base date, or an ISO date Rust converts with the curve day count. */
export type CurvePoint = number | string;
/** The stored object the lookup reads. */
export interface CurveQueryTarget {
  kind: "discount" | "forward" | "hazard" | "surface";
  /** The stored id, the MarketContext lookup key. */
  id: string;
}
/** The points the form submits; with a market, the worker's CurveQueryRequest. */
export type CurveLookup =
  | {
      kind: "discount";
      id: string;
      points: CurvePoint[];
      /** Start and end of each forward rate, in years: the native forward takes no dates. */
      forwards: [number, number][];
    }
  | { kind: "forward" | "hazard"; id: string; points: CurvePoint[] }
  | {
      kind: "surface";
      id: string;
      points: { expiry: number; strike: number }[];
    };
/** What the worker's curveQuery returns: each native value or its own native error. */
export type CurveQueryData =
  | {
      kind: "discount";
      points: {
        t: CurvePoint;
        df: Outcome<number>;
        zeroRate: Outcome<number>;
      }[];
      forwards: { t1: number; t2: number; rate: Outcome<number> }[];
    }
  | { kind: "forward"; points: { t: CurvePoint; rate: Outcome<number> }[] }
  | {
      kind: "hazard";
      points: {
        t: CurvePoint;
        sp: Outcome<number>;
        hazardRate: Outcome<number>;
      }[];
    }
  | {
      kind: "surface";
      points: { expiry: number; strike: number; vol: Outcome<number> }[];
    };
export interface CurveQueryProps {
  /** The stored curve or surface; remount with a key to load another. */
  target: CurveQueryTarget;
  /** Starting point text: comma-separated for a curve, one "expiry, strike" per line for a surface. */
  defaultPoints?: string;
  /** Starting forward intervals of a discount curve, one "start, end" per line. */
  defaultForwards?: string;
  /** Receives the lookup on Look up; connect it, with the market, to the useCurveQuery request. */
  onSubmit(lookup: CurveLookup): void;
  /** The returned values; null while none is available. */
  result: CurveQueryData | null;
  loading?: boolean;
  /** The native error of the whole lookup (an unknown id, say), shown as returned. */
  error?: string;
  /** Display text of a finite returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/** The queryable target a market-context-browser entry holds, or null: a stored surface, or a discount, forward or hazard curve. */
export function curveQueryTarget(entry: {
  path: readonly (string | number)[];
  value: unknown;
}): CurveQueryTarget | null {
  const stored = entry.value as { id?: unknown; type?: unknown } | null;
  if (typeof stored?.id !== "string") return null;
  if (entry.path[0] === "surfaces") return { kind: "surface", id: stored.id };
  if (
    entry.path[0] === "curves" &&
    (stored.type === "discount" ||
      stored.type === "forward" ||
      stored.type === "hazard")
  )
    return { kind: stored.type, id: stored.id };
  return null;
}
/** Text a number takes as a number; anything else stays text, for Rust to read as a date or reject. */
function curvePoint(text: string): CurvePoint {
  const number = Number(text);
  return Number.isNaN(number) ? text : number;
}
const pointList = (text: string) =>
  text
    .split(",")
    .map((part) => part.trim())
    .filter((part) => part !== "")
    .map(curvePoint);
const pairs = (text: string) =>
  text
    .split("\n")
    .filter((line) => line.trim() !== "")
    .map((line): [number, number] => {
      const [first, second] = line.split(",");
      return [typedNumber(first), typedNumber(second)];
    });
type Column = readonly [key: string, header: string];
/** The input and measure columns of one table; the headers state the units the facade documents. */
const tables = {
  discount: {
    inputs: [["t", "Point"]],
    measures: [
      ["df", "Discount factor"],
      ["zeroRate", "Zero rate (continuously compounded)"],
    ],
  },
  discountForward: {
    inputs: [
      ["t1", "Start (years)"],
      ["t2", "End (years)"],
    ],
    measures: [["rate", "Forward rate (continuously compounded)"]],
  },
  forward: {
    inputs: [["t", "Point"]],
    measures: [["rate", "Forward rate (simple, on the curve day count)"]],
  },
  hazard: {
    inputs: [["t", "Point"]],
    measures: [
      ["sp", "Survival probability"],
      ["hazardRate", "Hazard rate (annualized)"],
    ],
  },
  surface: {
    inputs: [
      ["expiry", "Expiry (years)"],
      ["strike", "Strike"],
    ],
    measures: [["vol", "Volatility"]],
  },
} as const satisfies Record<
  string,
  { inputs: readonly Column[]; measures: readonly Column[] }
>;
/**
 * Reads one stored curve or surface at the supplied points. Dates pass through
 * as entered for Rust to convert with the curve day count; a point Rust rejects
 * shows its native error in its own cell. Nothing is computed here.
 */
export function CurveQuery({
  target,
  defaultPoints = "",
  defaultForwards = "",
  onSubmit,
  result,
  loading,
  error,
  formatValue = String,
  density = "compact",
}: CurveQueryProps) {
  const [points, setPoints] = useState(defaultPoints);
  const [forwards, setForwards] = useState(defaultForwards);
  const lookup = (): CurveLookup => {
    const { kind, id } = target;
    if (kind === "surface")
      return {
        kind,
        id,
        points: pairs(points).map(([expiry, strike]) => ({ expiry, strike })),
      };
    if (kind === "discount")
      return { kind, id, points: pointList(points), forwards: pairs(forwards) };
    return { kind, id, points: pointList(points) };
  };
  return (
    <section
      aria-label={`Point lookup for ${target.id}`}
      data-density={density}
      className="@container mt-3 space-y-4 font-sans text-sm text-foreground"
    >
      <form
        aria-label="Lookup points"
        className="space-y-3"
        onSubmit={(event) => {
          event.preventDefault();
          onSubmit(lookup());
        }}
      >
        <div className="grid gap-4 @lg:grid-cols-2">
          {target.kind === "surface" ? (
            <FieldFrame
              label="Expiry and strike"
              help="One expiry, strike per line: expiry in years, strike in the surface's own units. Rust rejects a coordinate outside the grid."
            >
              {(control) => (
                <Textarea
                  {...control}
                  value={points}
                  onChange={(event) => setPoints(event.target.value)}
                />
              )}
            </FieldFrame>
          ) : (
            <FieldFrame
              label="Points"
              help="Comma-separated: years from the curve base date, or ISO dates (YYYY-MM-DD) Rust converts with the curve day count."
            >
              {(control) => (
                <Input
                  {...control}
                  value={points}
                  onChange={(event) => setPoints(event.target.value)}
                />
              )}
            </FieldFrame>
          )}
          {target.kind === "discount" && (
            <FieldFrame
              label="Forward intervals"
              help="One start, end per line, in years: the native forward rate takes year fractions, not dates."
            >
              {(control) => (
                <Textarea
                  {...control}
                  value={forwards}
                  onChange={(event) => setForwards(event.target.value)}
                />
              )}
            </FieldFrame>
          )}
        </div>
        <Button type="submit">Look up</Button>
      </form>
      <CurveQueryView
        result={result}
        loading={loading}
        error={error}
        format={formatValue}
        density={density}
      />
    </section>
  );
}
type Row = Record<string, unknown>;
function LookupTable({
  caption,
  table,
  rows,
  format,
  density,
}: {
  caption: string;
  table: { inputs: readonly Column[]; measures: readonly Column[] };
  rows: readonly object[];
  format: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  const columns = useMemo<ColumnDef<{}, Row, any>[]>(
    () => [
      // Each point echoes as entered rather than host-formatted.
      ...table.inputs.map(([key, header]): ColumnDef<{}, Row, any> => ({
        id: key,
        header,
        accessorFn: (row) => row[key],
        cell: ({ getValue }) => formatRaw(getValue() as string | number),
        meta: { className: "text-left finstack-numeric" },
      })),
      ...table.measures.map(([key, header]): ColumnDef<{}, Row, any> => ({
        id: key,
        header,
        accessorFn: (row) => row[key],
        cell: ({ getValue }) => {
          const outcome = getValue() as Outcome<number>;
          return outcome.ok ? (
            Number.isFinite(outcome.value) ? (
              format(outcome.value)
            ) : (
              formatRaw(outcome.value)
            )
          ) : (
            <span role="alert" className="text-xs text-error">
              {outcome.error.message}
            </span>
          );
        },
        meta: { className: "text-right finstack-numeric" },
      })),
    ],
    [table, format],
  );
  return (
    <FinstackTable
      data={rows as Row[]}
      columns={columns}
      getRowId={(row) => String(rows.indexOf(row))}
      caption={caption}
      density={density}
      emptyState="No points supplied"
    />
  );
}
function CurveQueryView({
  result,
  loading,
  error,
  format,
  density,
}: {
  result: CurveQueryData | null;
  loading?: boolean;
  error?: string;
  format: (value: number) => string;
  density: "compact" | "comfortable";
}) {
  if (error)
    return (
      <p role="alert" className="text-error">
        {error}
      </p>
    );
  if (loading)
    return (
      <p role="status" className="text-muted-foreground">
        Looking up points…
      </p>
    );
  if (!result)
    return (
      <p role="status" className="text-muted-foreground">
        No lookup result
      </p>
    );
  const shared = { format, density };
  return (
    <div className="space-y-4">
      <LookupTable
        caption={result.kind === "surface" ? "Surface values" : "Curve values"}
        table={tables[result.kind]}
        rows={result.points}
        {...shared}
      />
      {result.kind === "discount" && (
        <LookupTable
          caption="Forward rates"
          table={tables.discountForward}
          rows={result.forwards}
          {...shared}
        />
      )}
      <p className="text-xs text-muted-foreground">
        Decimals as Rust returned them.
      </p>
    </div>
  );
}
