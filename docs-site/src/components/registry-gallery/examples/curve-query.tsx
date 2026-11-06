"use client";
import { useMemo, useState } from "react";
import { sixPlaces, type ExampleProps } from "./props";
import { MarketContextBrowser } from "@/components/finstack/core/components/market-context-browser/market-context-browser";
import { StoredCurveView } from "@/components/finstack/core/components/curve-chart/stored-curve-view";
import {
  CurveQuery,
  curveQueryTarget,
  type CurveLookup,
  type CurveQueryData,
  type CurveQueryTarget,
} from "@/components/finstack/core/components/curve-query/curve-query";
import { useCurveQuery } from "@/hooks/core/use-curve-query/use-curve-query";
import data from "../data.json";
import { curveQueryMarket } from "../fixtures";
import { WorkerPreview } from "../worker-preview";
const market = curveQueryMarket;
const marketJson = JSON.stringify(market);
const cases = data.curveQuery.cases;
// Starting text of each kind, from the fixture's lookups.
const startText = {
  discount: {
    points: cases.discount.request.points.join(", "),
    forwards: cases.discount.request.forwards
      .map((pair) => pair.join(", "))
      .join("\n"),
  },
  forward: { points: cases.forward.request.points.join(", ") },
  hazard: { points: cases.hazard.request.points.join(", ") },
  surface: {
    points: cases.surface.request.points
      .map(({ expiry, strike }) => `${expiry}, ${strike}`)
      .join("\n"),
  },
};
function Lookup({
  target,
  density,
}: {
  target: CurveQueryTarget;
  density: ExampleProps["density"];
}) {
  // Starts from the fixture's points on the selected object.
  const [lookup, setLookup] = useState<CurveLookup>(
    () =>
      ({
        ...cases[target.kind].request,
        id: target.id,
      }) as CurveLookup,
  );
  const request = useMemo(() => ({ ...lookup, marketJson }), [lookup]);
  const query = useCurveQuery(request);
  const text: { points: string; forwards?: string } = startText[target.kind];
  return (
    <CurveQuery
      target={target}
      defaultPoints={text.points}
      defaultForwards={text.forwards}
      onSubmit={setLookup}
      result={(query.data as CurveQueryData | undefined) ?? null}
      loading={query.isFetching}
      error={query.error?.message}
      formatValue={sixPlaces}
      density={density}
    />
  );
}
// The browser stays worker-free; this host mounts the query through its slot
// and keeps the stored curve chart the slot replaces.
export function Example({ density = "compact" }: ExampleProps) {
  return (
    <WorkerPreview>
      <MarketContextBrowser
        state={market}
        renderObject={(entry) => {
          const target = curveQueryTarget(entry);
          if (!target) return undefined;
          const position = entry.path[1] as number;
          return (
            <>
              {entry.path[0] === "curves" && (
                <StoredCurveView
                  curves={[market.curves[position]!]}
                  ariaLabel="Selected market curve"
                  height={230}
                />
              )}
              <Lookup
                key={`${target.kind}:${target.id}`}
                target={target}
                density={density}
              />
            </>
          );
        }}
      />
    </WorkerPreview>
  );
}
