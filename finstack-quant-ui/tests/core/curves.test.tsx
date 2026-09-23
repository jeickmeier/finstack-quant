// @vitest-environment jsdom
import { afterEach, expect, it } from "vitest";
import { createRequire } from "node:module";
import { render, screen, cleanup } from "@testing-library/react";
import { createChartRuntime, type SceneNode } from "@tanstack/charts";
import { createWireCodec } from "../../src/codec.mjs";
import schema from "../../src/generated/schemas/market_context_state.json";
import fixture from "../../registry/core/components/curve-link-example/market.json";
import {
  CurveChart,
  curvePanels,
  curveDefinition,
  type CurveState,
  type CurvePoint,
} from "@/components/finstack/core/components/curve-chart/curve-chart";
import { getCurveView } from "../../src/views";
const curves = fixture.curves as CurveState[];
afterEach(cleanup);
it("validates all nine variants against the generated contract and native Market round trip", () => {
  expect(createWireCodec(schema).validator.safeParse(fixture).success).toBe(
    true,
  );
  const native = createRequire(import.meta.url)(
    "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
  );
  const market = new native.Market(JSON.stringify(fixture));
  try {
    expect(JSON.parse(market.toJson())).toEqual(fixture);
  } finally {
    market.free();
  }
  expect(new Set(curves.map((curve) => curve.type)).size).toBe(9);
});
it("plots every stored tuple unchanged, overlays only compatible variants and retains original references", () => {
  const panels = curvePanels(curves);
  expect(panels).toHaveLength(9);
  expect(panels.find((p) => p.type === "discount")!.states).toHaveLength(2);
  const flat = (nodes: readonly SceneNode[]): SceneNode[] =>
    nodes.flatMap((n) => (n.kind === "group" ? flat(n.children) : [n]));
  for (const panel of panels) {
    if (panel.route.route !== "stored-knots") {
      expect(panel.points).toEqual([]);
      continue;
    }
    const runtime = createChartRuntime<CurvePoint, number, number>();
    try {
      const scene = runtime.render(
        curveDefinition(panel.points, panel.type, {}),
        { width: 800, height: 420 },
      );
      const stored = panel.states.flatMap((c) =>
        "knot_points" in c ? c.knot_points : [],
      );
      expect(scene.points.map((p) => [p.xValue, p.yValue])).toEqual(stored);
      for (const point of scene.points) {
        expect(panel.states).toContain(point.datum.curve);
        expect(stored).toContain(point.datum.knot);
      }
      expect(
        flat(scene.nodes).filter(
          (n) => n.kind === "polyline" || n.kind === "area",
        ),
      ).toEqual([]);
      expect(
        flat(scene.nodes).filter((n) => n.kind === "dot").length,
      ).toBeGreaterThanOrEqual(stored.length);
    } finally {
      runtime.destroy();
    }
  }
});
it("renders actual no-knot arrays and model fields without manufacturing points", () => {
  const fields = curves.filter(
    (c) => c.type === "base_correlation" || c.type === "parametric",
  );
  const view = render(<CurveChart curves={fields} />);
  expect(view.container.querySelector("svg")).toBeNull();
  for (const curve of fields) {
    const projection = getCurveView(curve);
    if (projection.kind !== "fields") throw new Error("Expected field view");
    for (const [key, value] of Object.entries(projection.fields)) {
      expect(screen.getAllByText(key).length).toBeGreaterThan(0);
      expect(view.container.textContent).toContain(
        typeof value === "string" ? value : JSON.stringify(value, null, 2),
      );
    }
  }
});
it("rejects ambiguous overlay identities and handles empty input", () => {
  expect(() => curvePanels([curves[0], curves[0]])).toThrow(/Duplicate curve/);
  render(<CurveChart curves={[]} />);
  expect(screen.getByText("No curves supplied")).toBeTruthy();
});
it("reports unknown curve types instead of throwing during render", () => {
  const unknown = { ...curves[0], type: "unknown" } as unknown as CurveState;
  expect(() => curvePanels([unknown])).toThrow(/Unknown curve type: unknown/);
  render(<CurveChart curves={[unknown]} />);
  expect(screen.getByRole("alert").textContent).toBe(
    "Curves unavailable: Unknown curve type: unknown",
  );
});
