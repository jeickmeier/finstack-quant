import {
  cell,
  text,
  colorLegend,
  type ChartKey,
  type ChartMark,
  type ChartAxisPresentationOptions,
  type DomChartDefinition,
} from "@tanstack/charts";
import { decorative } from "@tanstack/charts/mark/decorative";
import { scaleBand } from "@tanstack/charts/scales/band";
import { whenSelected } from "@tanstack/charts/selection";
import { tooltip } from "@tanstack/charts/tooltip";
import { scaleLinear } from "d3-scale";
import { wcagContrast } from "culori";
import type { FigurePresentation } from "./presentation";
import { chartSelection } from "./selection";

export interface HeatmapOptions<T> {
  /** Original caller rows; coordinates and values are never transformed. */
  data: readonly T[];
  x(row: T): ChartKey;
  y(row: T): ChartKey;
  value(row: T): number;
  /** Unique stable cell identifier, independent of array position. */
  key(row: T): string;
  xLabel: string;
  yLabel: string;
  valueLabel: string;
  /** Display text only. The default is the original number's string representation. */
  formatValue?(value: number): string;
  axes?: {
    x?: ChartAxisPresentationOptions<ChartKey>;
    y?: ChartAxisPresentationOptions<ChartKey>;
  };
  annotations?: readonly ChartMark<T, ChartKey, ChartKey>[];
  link?: { selectedKey: string | null; select(key: string | null): void };
}
/** Explicit extents; a diverging ramp requires a meaningful caller-supplied center. */
export type HeatmapColors =
  | { palette: "sequential"; domain: readonly [number, number] }
  | { palette: "diverging"; domain: readonly [number, number, number] };

/** Native cell/text marks for supplied grids. Spread the result into FinstackChart. */
export function heatmap<T>(options: HeatmapOptions<T> & HeatmapColors) {
  const { data, x, y, value, key, domain, palette } = options;
  if (
    domain.some(
      (stop, i) => !Number.isFinite(stop) || (i > 0 && stop <= domain[i - 1]!),
    )
  )
    throw new TypeError(
      "Heatmap color domain must contain finite, strictly increasing stops",
    );
  const ids = new Set<string>(),
    coordinates = new Set<string>();
  for (const row of data) {
    const coordinate = JSON.stringify([x(row), y(row)]);
    if (
      ![x(row), y(row)].every(
        (v) => typeof v === "string" || Number.isFinite(v),
      ) ||
      !Number.isFinite(value(row))
    )
      throw new TypeError(
        "Heatmap coordinates and values must be finite numbers or string coordinates",
      );
    if (value(row) < domain[0] || value(row) > domain[domain.length - 1]!)
      throw new TypeError(
        "Heatmap color domain must include every supplied value",
      );
    if (ids.has(key(row)) || coordinates.has(coordinate))
      throw new TypeError("Heatmap keys and coordinate pairs must be unique");
    ids.add(key(row));
    coordinates.add(coordinate);
  }
  const xs = [...new Set(data.map(x))],
    ys = [...new Set(data.map(y))];
  const format = options.formatValue ?? String;
  const selection = options.link
    ? chartSelection<T, ChartKey, ChartKey>(options.link, key)
    : undefined;
  const channels = { x, y, key, color: value };
  const definition = (
    presentation: FigurePresentation,
  ): DomChartDefinition<T, ChartKey, ChartKey> => ({
    chart() {
      const defaultTheme = presentation.theme;
      const range = presentation.ramps[palette];
      const { light, dark, size } = presentation.cellText;
      const stops =
        palette === "diverging"
          ? [...domain]
          : range.map(
              (_, i) =>
                domain[0] + ((domain[1] - domain[0]) * i) / (range.length - 1),
            );
      const colors = scaleLinear<string>().domain(stops).range(range);
      return {
        marks: [
          cell(data, {
            ...channels,
            id: "heatmap-cells",
            stroke: defaultTheme.grid,
            strokeWidth: 1,
          }),
          ...(xs.length <= 12 && ys.length <= 12
            ? [
                decorative(
                  text(data, {
                    ...channels,
                    id: "heatmap-values",
                    color: undefined,
                    text: (row) => format(value(row)),
                    fontSize: size,
                    fill: (row) =>
                      wcagContrast(colors(value(row)), dark) >=
                      wcagContrast(colors(value(row)), light)
                        ? dark
                        : light,
                  }),
                ),
              ]
            : []),
          ...(selection
            ? [
                whenSelected(
                  cell(data, {
                    ...channels,
                    id: "heatmap-accepted",
                    color: undefined,
                    fill: "none",
                    stroke: defaultTheme.foreground,
                    strokeWidth: 3,
                    inset: 2,
                  }),
                  selection,
                ),
              ]
            : []),
          ...(options.annotations ?? []),
        ],
        scales: {
          x: {
            scale: scaleBand,
            domain: xs,
            axis: { label: options.xLabel, ...options.axes?.x },
          },
          y: {
            scale: scaleBand,
            domain: ys,
            axis: { label: options.yLabel, ...options.axes?.y },
          },
        },
        color: {
          scale: colors,
          legend: colorLegend({ label: options.valueLabel, format }),
        },
      };
    },
    selection,
    tooltip: {
      use: tooltip,
      content: (points) => ({
        rows: points.map((point) => ({
          label: `${options.xLabel}: ${String(x(point.datum))} · ${options.yLabel}: ${String(y(point.datum))}`,
          value: `${options.valueLabel}: ${format(value(point.datum))}`,
        })),
      }),
    },
  });
  return { definition };
}
