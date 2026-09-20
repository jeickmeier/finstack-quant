import { createRoot } from "react-dom/client";
import { useRef, useState, useMemo } from "react";
import { text } from "@tanstack/charts";
import {
  CurveChart,
  type CurveState,
  type CurvePoint,
} from "./components/finstack/components/curve-chart/curve-chart";
import type { FigureHandle } from "./components/finstack/primitives/finstack-chart/finstack-chart";
import fixture from "./fixture.json";
const curves = fixture.curves as CurveState[];
const overlay = curves.find((c) => c.id === "OVERLAY")!;
const annotations = {
  discount: [
    text<CurvePoint>([{ curve: overlay, knot: [1, 0.9512345678901234] }], {
      x: (p) => p.knot[0],
      y: (p) => p.knot[1],
      text: () => "Supplied knot note",
      dy: -16,
    }),
  ],
};
const key = (p: CurvePoint) => `${p.curve.id}/${p.knot[0]}`;
const sources = [{ label: "Retained native fixture" }];
const figureAnnotations = [{ text: "Publication note", x: 0.5, y: 0.65 }];
function App() {
  const ref = useRef<FigureHandle>(null);
  const [selectedKey, select] = useState<string | null>(null);
  const [activations, count] = useState(0);
  const [focus, setFocus] = useState<string | null>(null);
  const [group, setGroup] = useState(0);
  const [custom, setCustom] = useState(false);
  const link = useMemo(
    () => ({ selectedKey, select, getPointKey: key }),
    [selectedKey],
  );
  (window as any).exportCurve = async () =>
    (
      await ref.current!.exportSvg({ width: 900, height: 600, theme: "light" })
    ).text();
  return (
    <main className="mx-auto max-w-5xl space-y-4 p-4">
      <h1 className="text-xl">Stored market curves</h1>
      <button onClick={() => setCustom(!custom)}>Toggle custom tooltip</button>
      <p data-status="">
        Selected: {selectedKey ?? "none"}; activations: {activations}; focus:{" "}
        {focus ?? "none"}; group: {group}
      </p>
      <CurveChart
        curves={curves}
        link={link}
        height={600}
        figureRefs={{ discount: ref }}
        annotations={annotations}
        title="Stored market observations"
        subtitle="Canonical state supplied by native Market"
        caption="Each point is an original stored tuple."
        sources={sources}
        figureAnnotations={figureAnnotations}
        onSelect={() => count((n) => n + 1)}
        onFocusChange={(p) => setFocus(p ? key(p.datum) : null)}
        onFocusGroupChange={(p) => setGroup(p.length)}
        renderTooltipBody={
          custom
            ? ({ defaultBody, pinned, primaryPoint, dismiss }) => (
                <>
                  {defaultBody}
                  {pinned && primaryPoint && (
                    <button onClick={dismiss}>Open curve details</button>
                  )}
                </>
              )
            : undefined
        }
      />
    </main>
  );
}
createRoot(document.getElementById("root")!).render(<App />);
