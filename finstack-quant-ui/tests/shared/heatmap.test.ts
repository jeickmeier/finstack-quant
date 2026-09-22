import { expect, it } from "vitest";
import { createChartRuntime, type SceneNode } from "@tanstack/charts";
import { wcagContrast } from "culori";
import { heatmap } from "@/components/finstack/shared/chart/finstack-chart/heatmap";
import { composeFigure } from "@/components/finstack/shared/chart/finstack-chart/figure";
import tokens from "../../registry/theme/finstack-theme/tokens.json";
const flatten = (nodes: readonly SceneNode[]): SceneNode[] =>
  nodes.flatMap((node) =>
    node.kind === "group" ? flatten(node.children) : [node],
  );
const data = [
  { id: "negative", x: 0.5, y: "A", v: -2 },
  { id: "center", x: 1, y: "A", v: 0 },
  { id: "positive", x: 2, y: "A", v: 7 },
];
const options = {
  data,
  x: (d: (typeof data)[number]) => d.x,
  y: (d: (typeof data)[number]) => d.y,
  value: (d: (typeof data)[number]) => d.v,
  key: (d: (typeof data)[number]) => d.id,
  xLabel: "Supplied x",
  yLabel: "Supplied y",
  valueLabel: "Raw value",
};
function render(
  input: ReturnType<typeof heatmap<(typeof data)[number]>>,
  theme: "light" | "dark",
) {
  const t = tokens[theme];
  const ramps = {
    sequential: Array.from(
      { length: 9 },
      (_, i) => t[`sequential-${i + 1}` as keyof typeof t],
    ),
    diverging: [
      t["diverging-negative"],
      t["diverging-neutral"],
      t["diverging-positive"],
    ],
  };
  const figure = composeFigure(
    {
      ...input,
      ariaLabel: "Heatmap",
      title: "Supplied grid",
      caption: "Original values",
    },
    {
      theme: {
        foreground: t.foreground,
        muted: t["muted-foreground"],
        grid: t.border,
        background: t.background,
        palette: [t["chart-1"]],
      },
      cellText: { light: t["cell-light"], dark: t["cell-dark"], size: 11 },
      ramps,
      fontFamily: "sans-serif",
      titleSize: 16,
      bodySize: 14,
      noteSize: 12.5,
      spacing: 4,
      measureText: (text, o) => ({
        x: 0,
        y: 0,
        width: text.length * o.fontSize * 0.6,
        height: o.fontSize,
      }),
    },
  );
  const runtime = createChartRuntime<
    (typeof data)[number],
    string | number,
    string | number
  >();
  const scene = runtime.render(figure.definition, { width: 800, height: 600 });
  runtime.destroy();
  return {
    scene,
    nodes: flatten(scene.nodes),
    svg: figure.renderSvg(scene, { ariaLabel: "Heatmap" }),
  };
}
it.each(["light", "dark"] as const)(
  "retains original coordinates, rows, keys and contrast in %s",
  (theme) => {
    const result = render(
      heatmap({ ...options, palette: "diverging", domain: [-2, 0, 7] }),
      theme,
    );
    expect(result.scene.points).toHaveLength(3);
    result.scene.points.forEach((point, i) => {
      expect(point.datum).toBe(data[i]);
      expect([point.xValue, point.yValue]).toEqual([data[i]!.x, data[i]!.y]);
    });
    expect(
      wcagContrast(
        result.scene.colors.map(0),
        tokens[theme]["diverging-neutral"],
      ),
    ).toBe(1);
    for (const node of result.nodes.filter(
      (n) => n.kind === "label" && n.key?.includes("heatmap-values"),
    )) {
      if (node.kind !== "label") continue;
      expect(
        wcagContrast(
          String(node.style?.fill),
          result.scene.colors.map(Number(node.text)),
        ),
      ).toBeGreaterThanOrEqual(4.5);
    }
    expect(result.svg).toContain("Original values");
  },
);
it.each([12, 13])("prints values only up to 12 rows and columns (%i)", (n) => {
  const grid = Array.from({ length: n * n }, (_, i) => ({
    id: String(i),
    x: i % n,
    y: String(Math.floor(i / n)),
    v: i,
  }));
  const result = render(
    heatmap({
      ...options,
      data: grid,
      palette: "sequential",
      domain: [0, grid.length - 1],
    }),
    "light",
  );
  expect(result.scene.points).toHaveLength(n * n);
  expect(
    result.nodes.filter(
      (n) => n.kind === "label" && n.key?.includes("heatmap-values"),
    ),
  ).toHaveLength(n === 12 ? 144 : 0);
});
it("rejects ambiguous cell identities and misleading color extents", () => {
  expect(() =>
    heatmap({
      ...options,
      data: [data[0]!, data[0]!],
      palette: "diverging",
      domain: [-2, 0, 7],
    }),
  ).toThrow(/unique/);
  expect(() =>
    heatmap({ ...options, palette: "diverging", domain: [-2, 7, 0] }),
  ).toThrow(/increasing/);
  expect(() =>
    heatmap({ ...options, palette: "sequential", domain: [0, 7] }),
  ).toThrow(/include/);
});

it.each(["light", "dark"] as const)(
  "meets cell-text contrast between palette stops in %s",
  (theme) => {
    for (const colors of [
      { palette: "sequential", domain: [-2, 7] } as const,
      { palette: "diverging", domain: [-2, 0, 7] } as const,
    ]) {
      const { scene } = render(heatmap({ ...options, ...colors }), theme);
      for (let i = 0; i <= 1000; i++) {
        const fill = scene.colors.map(-2 + (9 * i) / 1000);
        expect(
          Math.max(
            wcagContrast(fill, tokens[theme]["cell-light"]),
            wcagContrast(fill, tokens[theme]["cell-dark"]),
          ),
        ).toBeGreaterThanOrEqual(4.5);
      }
    }
  },
);
