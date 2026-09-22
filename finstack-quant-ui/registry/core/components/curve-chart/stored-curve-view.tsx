"use client";
import { useMemo } from "react";
import {
  useLinkedSelection,
  type LinkedSelection,
} from "@/hooks/shared/use-linked-selection/use-linked-selection";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import {
  CurveChart,
  curvePanels,
  type CurveChartProps,
  type CurvePoint,
} from "./curve-chart";
export const storedCurveKey = (point: CurvePoint) =>
  JSON.stringify([point.curve.type, point.curve.id, point.knot[0]]);
/** Stored nodes share the same accepted key in the existing table and curve chart. Non-knot variants retain their field views. */
export function StoredCurveView({
  link: external,
  ...props
}: Omit<CurveChartProps, "link"> & { link?: LinkedSelection }) {
  const own = useLinkedSelection(),
    link = external ?? own;
  const points = useMemo(
    () => curvePanels(props.curves).flatMap((panel) => panel.points),
    [props.curves],
  );
  const addresses = useMemo(
    () =>
      new Map(
        points.map((point) => [
          storedCurveKey(point),
          { rowId: storedCurveKey(point), columnId: "value" },
        ]),
      ),
    [points],
  );
  return (
    <div className="@container">
      <div className="finstack-stored-curve__layout grid min-w-0 gap-4 @min-[720px]:grid-cols-2">
        <div className="min-w-0">
          <CurveChart
            {...props}
            link={{ ...link, getPointKey: storedCurveKey }}
          />
        </div>
        <div className="min-w-0 max-h-[320px] overflow-auto">
          {points.length > 0 && (
            <FinstackTable
              caption="Stored curve knots"
              data={points}
              getRowId={storedCurveKey}
              link={link}
              getRowKey={storedCurveKey}
              getCellKey={(point, column) =>
                column === "value" ? storedCurveKey(point) : null
              }
              getActiveCell={(key) => addresses.get(key) ?? null}
              columns={[
                {
                  id: "curve",
                  header: "Curve",
                  accessorFn: (point) => point.curve.id,
                },
                {
                  id: "type",
                  header: "Variant",
                  accessorFn: (point) => point.curve.type,
                },
                {
                  id: "x",
                  header: "Stored x",
                  meta: { className: "text-right finstack-numeric" },
                  accessorFn: (point) => point.knot[0],
                },
                {
                  id: "value",
                  header: "Stored value",
                  meta: { className: "text-right finstack-numeric" },
                  accessorFn: (point) => point.knot[1],
                },
              ]}
            />
          )}
        </div>
      </div>
    </div>
  );
}
