import { it, expect } from "vitest";
import {
  createChartRuntime,
  type SceneNode,
  type ChartSpecDatum,
} from "@tanstack/charts";
import { composeFigure } from "@/components/finstack/shared/chart/finstack-chart/figure";
import {
  figureLayout,
  type FigurePresentation,
} from "@/components/finstack/shared/chart/finstack-chart/presentation";
import { figureExample } from "@/components/finstack/shared/chart/figure-example/figure-data";
import tokens from "../../registry/theme/finstack-theme/tokens.json";
const presentation: FigurePresentation = {
  theme: {
    foreground: tokens.light.foreground,
    muted: tokens.light["muted-foreground"],
    grid: tokens.light.border,
    background: tokens.light.background,
    palette: [tokens.light["chart-1"]],
  },
  cellText: {
    light: tokens.light["cell-light"],
    dark: tokens.light["cell-dark"],
    size: 11,
  },
  ramps: { sequential: [], diverging: [] },
  fontFamily: tokens.theme["font-sans"],
  titleSize: 16,
  bodySize: 14,
  noteSize: 12.5,
  spacing: 4,
  measureText: (text, options) => ({
    x: 0,
    y: 0,
    width: text.length * options.fontSize * 0.6,
    height: options.fontSize,
  }),
};
it.each([340, 900])(
  "keeps supplied coordinates and every publication section at width %i",
  (width) => {
    const figure = composeFigure(figureExample, presentation);
    const runtime = createChartRuntime<
      ChartSpecDatum<typeof figureExample.definition>,
      number,
      number
    >();
    const native = runtime.render(figureExample.definition, {
      width,
      height: 600,
    });
    const scene = runtime.render(
      figure.definition,
      { width, height: 600 },
      { measureText: presentation.measureText },
    );
    expect(scene.points.map((point) => [point.xValue, point.yValue])).toEqual(
      native.points.map((point) => [point.xValue, point.yValue]),
    );
    const svg = figure.renderSvg(scene, { ariaLabel: figureExample.ariaLabel });
    for (const value of [
      "Supplied callout",
      "Supplied coordinate",
      "Stored value (raw)",
      "Registry illustration",
      "https://example.com/source",
      "caller",
    ])
      expect(svg).toContain(value);
    expect(svg).toContain("<text");
    expect(svg).toContain("<path");
    expect(svg).not.toContain("foreignObject");
    expect(svg).not.toContain("<image");
    const layout = figureLayout(figureExample, width, 600, presentation);
    const labels = (nodes: readonly SceneNode[]): SceneNode[] =>
      nodes.flatMap((node) =>
        node.kind === "group" ? labels(node.children) : [node],
      );
    const legend = labels(scene.nodes).find(
      (node) => node.kind === "label" && node.text === "Supplied series",
    );
    expect(legend && "y" in legend && legend.y).toBeGreaterThanOrEqual(
      layout.top,
    );
    expect(scene.chart.y).toBeGreaterThanOrEqual(layout.top);
    expect(scene.chart.y + scene.chart.height).toBeLessThanOrEqual(
      600 - layout.bottom,
    );
    runtime.destroy();
  },
);
it("omits empty bands and preserves all long text without ellipsis", () => {
  expect(figureLayout({}, 300, 300, presentation)).toEqual({
    top: 0,
    bottom: 0,
    lines: [],
  });
  const title = "A heading with explicit\nparagraphs andaverylongsingleword";
  const layout = figureLayout({ title }, 180, 600, presentation);
  expect(
    layout.lines
      .map((line) => line.text)
      .join("")
      .replaceAll(/\s/g, ""),
  ).toBe(title.replaceAll(/\s/g, ""));
  expect(() => figureLayout({ title }, 120, 120, presentation)).toThrow(
    /cannot fit/,
  );
});
it("places figure-relative notes without touching native data values and escapes markup", () => {
  const props = {
    ...figureExample,
    title: '<script>alert("text")</script>',
    figureAnnotations: [{ text: "Figure note", x: 0.5, y: 0.1, dx: 8, dy: 4 }],
  };
  const figure = composeFigure(props, presentation);
  const runtime = createChartRuntime<
    ChartSpecDatum<typeof figureExample.definition>,
    number,
    number
  >();
  const scene = runtime.render(figure.definition, { width: 900, height: 600 });
  const svg = figure.renderSvg(scene, { ariaLabel: props.ariaLabel });
  expect(svg).not.toContain("<script>");
  expect(svg).toContain("&lt;script&gt;");
  expect(
    figureLayout(props, 900, 600, presentation).lines.at(-1),
  ).toMatchObject({ text: "Figure note", x: 458, y: 64 });
  runtime.destroy();
});
