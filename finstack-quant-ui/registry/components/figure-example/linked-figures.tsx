"use client";
import { useMemo, useState, useCallback, type Ref } from "react";
import { defineChart, dot, lineY, crosshair } from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { createChartCursor, cursorHost } from "@tanstack/charts/cursor";
import { tooltip } from "@tanstack/charts/tooltip";
import { whenSelected } from "@tanstack/charts/selection";
import { useLinkedSelection } from "@/hooks/use-linked-selection/use-linked-selection";
import {
  FinstackChart,
  chartSelection,
  type FigureHandle,
} from "../../primitives/finstack-chart/finstack-chart";

const observations = [
  { id: "a", x: 1, y: 12, label: "First observation" },
  { id: "b", x: 2, y: 18, label: "Second observation" },
  { id: "c", x: 3, y: 15, label: "Third observation" },
];
type Observation = (typeof observations)[number];
const button =
  "rounded-sm border border-border px-2 py-1 focus-visible:outline-2 focus-visible:outline-ring";

/** Two compatible supplied-data charts, one accepted key and a parent-local native cursor. */
export function LinkedFigureExample({ ref }: { ref?: Ref<FigureHandle> } = {}) {
  const [accepted, setAccepted] = useState<string | null>(null);
  const [reject, setReject] = useState(false);
  const [reversed, setReversed] = useState(false);
  const [removed, setRemoved] = useState(false);
  const [activations, setActivations] = useState(0);
  const [proposals, setProposals] = useState(0);
  const [detail, setDetail] = useState<string | null>(null);
  const [focus, setFocus] = useState<string | null>(null);
  const [group, setGroup] = useState(0);
  const [cursor] = useState(() => createChartCursor<number, number>());
  const onSelectedKeyChange = useCallback(
    (key: string | null) => {
      setProposals((count) => count + 1);
      if (!reject) setAccepted(key);
    },
    [reject],
  );
  const selection = useLinkedSelection({
    selectedKey: accepted,
    onSelectedKeyChange,
  });
  const rows = useMemo(() => {
    const rows = observations.filter((row) => !removed || row.id !== "b");
    return reversed ? rows.reverse() : rows;
  }, [removed, reversed]);
  const native = useMemo(
    () =>
      chartSelection<Observation, number, number>(selection, (row) => row.id),
    [selection],
  );
  const definition = useMemo(
    () =>
      defineChart({
        marks: [
          lineY(rows, { x: "x", y: "y", key: "id", id: "line" }),
          dot(rows, { x: "x", y: "y", key: "id", id: "points", r: 5 }),
          whenSelected(
            dot(rows, {
              x: "x",
              y: "y",
              key: "id",
              id: "accepted",
              r: 9,
              fill: "none",
              stroke: "var(--primary)",
              strokeWidth: 2,
            }),
            native,
          ),
          crosshair({ x: true, y: false }),
        ],
        scales: {
          x: {
            scale: scaleLinear().domain([0.5, 3.5]),
            axis: { label: "Supplied coordinate" },
          },
          y: {
            scale: scaleLinear().domain([10, 20]),
            axis: { label: "Supplied value" },
          },
        },
        selection: native,
        focus: "group-x",
        cursor: {
          use: cursorHost,
          controller: cursor,
          mode: "focus",
          match: "x",
          pin: true,
        },
        tooltip: { use: tooltip },
      }),
    [rows, native, cursor],
  );
  return (
    <section
      aria-label="Linked figure example"
      className="space-y-2 font-sans text-foreground"
    >
      <h2 className="text-base font-semibold">Linked observations</h2>
      <div className="flex flex-wrap gap-2">
        <button className={button} onClick={() => selection.select("b")}>
          Select second observation
        </button>
        <button className={button} onClick={selection.clear}>
          Clear selection
        </button>
        <button className={button} onClick={() => setReversed(!reversed)}>
          Reverse observations
        </button>
        <button className={button} onClick={() => setRemoved(!removed)}>
          Toggle second observation
        </button>
        <label>
          <input
            type="checkbox"
            checked={reject}
            onChange={(event) => setReject(event.target.checked)}
          />{" "}
          Reject selection proposals
        </label>
      </div>
      <p data-selection-status="" aria-live="polite">
        Accepted: {accepted ?? "none"}; detail: {detail ?? "none"}
      </p>
      <p data-interaction-status="">
        Activations: {activations}; proposals: {proposals}; focus:{" "}
        {focus ?? "none"}; group: {group}
      </p>
      <FinstackChart
        definition={definition}
        ref={ref}
        ariaLabel="Interactive observations"
        height={340}
        onSelect={() => setActivations((count) => count + 1)}
        onFocusChange={(point) => setFocus(point?.datum.id ?? null)}
        onFocusGroupChange={(points) => setGroup(points.length)}
        renderTooltipBody={({ defaultBody, primaryPoint, pinned, dismiss }) => (
          <>
            {defaultBody}
            {pinned && primaryPoint && (
              <button
                className={button}
                onClick={() => {
                  setDetail(primaryPoint.datum.id);
                  dismiss();
                }}
              >
                Open observation details
              </button>
            )}
          </>
        )}
      />
      <FinstackChart
        definition={definition}
        ariaLabel="Linked comparison observations"
        height={300}
        onSelect={() => setActivations((count) => count + 1)}
      />
    </section>
  );
}
