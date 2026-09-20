"use client";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Checkbox } from "@/components/ui/checkbox";
import { useMemo, useState } from "react";
import { dot } from "@tanstack/charts";
import { decorative } from "@tanstack/charts/mark/decorative";
import type { ColumnDef } from "@tanstack/react-table";
import { createWireCodec } from "@/lib/finstack/codec.mjs";
import marketSchema from "@/lib/finstack/generated/schemas/market_context_state.json";
import fixture from "@/lib/finstack/fixtures/curves/market.json";
import type { MarketContextStateWire } from "@/lib/finstack/generated/types/market_context_state";
import { useLinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";
import { FinstackTable } from "../../primitives/finstack-table/finstack-table";
import {
  CurveChart,
  curvePanels,
  type CurvePoint,
  type CurveState,
} from "../curve-chart/curve-chart";
const market = createWireCodec(marketSchema).parse(
  JSON.stringify(fixture),
) as MarketContextStateWire;
type Discount = Extract<CurveState, { type: "discount" }>;
const original = market.curves.filter(
  (curve): curve is Discount => curve.type === "discount",
);
const rowId = (curve: Discount) => JSON.stringify([curve.type, curve.id]);
const seriesKey = (curve: Discount) => JSON.stringify(["series", curve.id]);
const pointKey = (point: CurvePoint) =>
  JSON.stringify(["point", point.curve.id, point.knot[0]]);
const coordinateColumns = [
  ...new Set(
    original.flatMap((curve) => curve.knot_points.map((knot) => knot[0])),
  ),
].map((coordinate) => ({
  id: JSON.stringify(["coordinate", coordinate]),
  coordinate,
}));
function cellKey(curve: Discount, id: string) {
  const column = coordinateColumns.find((column) => column.id === id);
  const knot =
    column && curve.knot_points.find((knot) => knot[0] === column.coordinate);
  return knot ? pointKey({ curve, knot }) : null;
}
const addresses = new Map(
  original.flatMap((curve) =>
    coordinateColumns.flatMap((column) => {
      const key = cellKey(curve, column.id);
      return key === null
        ? []
        : [[key, { rowId: rowId(curve), columnId: column.id }] as const];
    }),
  ),
);
/** Explicit correspondence over original stored knots, with no curve evaluation. */
export function CurveLinkExample({
  label = "Linked stored curves",
}: { label?: string } = {}) {
  const [accepted, setAccepted] = useState<string | null>(null),
    [reject, setReject] = useState(false);
  const [reverse, setReverse] = useState(false),
    [remove, setRemove] = useState(false);
  const [proposals, setProposals] = useState(0),
    [activations, setActivations] = useState(0);
  const [detail, setDetail] = useState("none");
  const link = useLinkedSelection({
    selectedKey: accepted,
    onSelectedKeyChange(key) {
      setProposals((count) => count + 1);
      if (!reject) setAccepted(key);
    },
  });
  const curves = useMemo(() => {
    const rows = original.filter((curve) => !remove || curve.id !== "OVERLAY");
    return reverse ? rows.reverse() : rows;
  }, [reverse, remove]);
  const notify = (value: string) => {
    setActivations((count) => count + 1);
    setDetail(value);
  };
  const columns: ColumnDef<{}, Discount, any>[] = useMemo(
    () => [
      { id: "series", accessorKey: "id", header: "Curve" },
      ...coordinateColumns.map((column) => ({
        id: column.id,
        header: `Coordinate ${column.coordinate}`,
        accessorFn: (curve: Discount) =>
          curve.knot_points.find((knot) => knot[0] === column.coordinate)?.[1],
        cell: ({ getValue }: { getValue: () => unknown }) =>
          getValue() === undefined ? "—" : String(getValue()),
      })),
      {
        id: "action",
        header: "Details",
        cell: ({ row }: { row: { original: Discount } }) => (
          <Button
            variant="outline"
            size="sm"

            onClick={() => setDetail(`Inspect ${row.original.id}`)}
          >
            Inspect {row.original.id}
          </Button>
        ),
      },
    ],
    [],
  );
  const annotations = useMemo(() => {
    const selected = curves.filter((curve) => seriesKey(curve) === accepted);
    const points = selected.length ? curvePanels(selected)[0].points : [];
    return {
      discount: [
        decorative(
          dot(points, {
            id: "selected-series",
            x: (point: CurvePoint) => point.knot[0],
            y: (point: CurvePoint) => point.knot[1],
            key: (point: CurvePoint) => pointKey(point),
            r: 9,
            fill: "none",
            stroke: "var(--primary)",
            strokeWidth: 2,
          }),
        ),
      ],
    };
  }, [curves, accepted]);
  return (
    <section aria-label={label} className="space-y-3">
      <h2 className="text-lg font-semibold">Stored curve correspondence</h2>
      <div className="flex flex-wrap gap-2">
        <Button
          variant="outline"
          size="sm"

          onClick={() =>
            link.select(
              pointKey({
                curve: original[0],
                knot: original[0].knot_points[1],
              }),
            )
          }
        >
          Select overlay point
        </Button>
        <Button variant="outline" size="sm" onClick={link.clear}>
          Clear
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => setReverse(!reverse)}
        >
          Reverse rows
        </Button>
        <Button variant="outline" size="sm" onClick={() => setRemove(!remove)}>
          Toggle overlay
        </Button>
        <Label>
          <Checkbox
            checked={reject}
            onCheckedChange={(checked) => setReject(checked === true)}
          />{" "}
          Reject proposals
        </Label>
      </div>
      <output data-link-status>
        Accepted: {accepted ?? "none"}; proposals: {proposals}; activations:{" "}
        {activations}; detail: {detail}
      </output>
      <FinstackTable<Discount, number | undefined>
        caption="Stored discount knots"
        columns={columns}
        data={curves}
        getRowId={rowId}
        link={link}
        getRowKey={seriesKey}
        getCellKey={cellKey}
        getActiveCell={(key) => addresses.get(key) ?? null}
        onRowActivate={(curve) => notify(`Series ${curve.id}`)}
        onCellActivate={(cell) =>
          notify(`${cell.row.id} ${cell.columnId}: ${String(cell.value)}`)
        }
      />
      <CurveChart
        ariaLabel={label}
        curves={curves}
        title="Stored discount factors"
        height={340}
        annotations={annotations}
        link={{ ...link, getPointKey: pointKey }}
        onSelect={(point) =>
          notify(
            point
              ? `Point ${point.datum.curve.id} ${point.datum.knot[0]}`
              : "Chart cleared",
          )
        }
      />
    </section>
  );
}
