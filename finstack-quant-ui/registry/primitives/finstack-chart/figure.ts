import {
  createChartScene,
  renderChartSvg,
  type ChartValue,
  type DomChartDefinition,
  type ChartSvgRenderer,
  type ChartColorLegendContext,
  type StaticChartDefinition,
} from "@tanstack/charts";
import {
  figureLayout,
  type FigureText,
  type FigurePresentation,
} from "./presentation";
export interface FigureSpec<
  T,
  X extends ChartValue,
  Y extends ChartValue,
> extends FigureText {
  /** Native scales, axis/unit labels, legend and data-coordinate annotation marks; no alternate grammar. */
  definition: DomChartDefinition<T, X, Y>;
  ariaLabel: string;
  ariaDescription?: string;
}
/** One figure composition used by the native React host and all export sizes. */
export function composeFigure<T, X extends ChartValue, Y extends ChartValue>(
  props: FigureSpec<T, X, Y>,
  presentation: FigurePresentation,
) {
  // Responsive runtime options override the returned spec. Carry only native
  // interaction options forward, so original static scales/color cannot undo layout.
  const {
    marks: _marks,
    scales: _scales,
    color: _color,
    gradients: _gradients,
    clip: _clip,
    margin: _margin,
    theme: _theme,
    guides: _guides,
    ...options
  } = props.definition as StaticChartDefinition<T, X, Y, "dom">;
  const definition: DomChartDefinition<T, X, Y> = {
    ...options,
    chart(context) {
      const layout = figureLayout(
        props,
        context.width,
        context.height,
        presentation,
      );
      const size = {
        width: context.width,
        height: context.height - layout.top - layout.bottom,
      };
      const source =
        "chart" in props.definition
          ? props.definition.chart({ ...context, ...size })
          : props.definition;
      const spec = {
        ...source,
        scales: Object.fromEntries(
          Object.entries(source.scales).map(([name, scale]) => {
            if (!scale || scale.axis === false) return [name, scale];
            const axis = scale.axis ?? {};
            const label =
              typeof axis.label === "string"
                ? { text: axis.label }
                : axis.label;
            return [
              name,
              {
                ...scale,
                axis: {
                  ...axis,
                  tickLabels:
                    axis.tickLabels === false
                      ? false
                      : {
                          fontSize: presentation.noteSize,
                          opacity: 1,
                          ...axis.tickLabels,
                        },
                  label: label
                    ? { fontSize: presentation.bodySize, opacity: 1, ...label }
                    : undefined,
                },
              },
            ];
          }),
        ),
        theme: { ...presentation.theme, ...source.theme },
      };
      // Native layout reserves axes, labels, legend and annotation overhang. Add only prose bands.
      const plot = createChartScene(spec, size, {
        measureText: presentation.measureText,
        typography: { fontFamily: presentation.fontFamily },
      });
      const legend = spec.color?.legend;
      const legendContext = (
        context: ChartColorLegendContext,
      ): ChartColorLegendContext => ({
        ...context,
        bounds: {
          ...context.bounds,
          y:
            context.bounds.y +
            (legend?.placement === "bottom" ? -layout.bottom : layout.top),
        },
      });
      return {
        ...spec,
        color: legend
          ? {
              ...spec.color,
              legend: {
                ...legend,
                render: (context) => legend.render(legendContext(context)),
                control: legend.control
                  ? (context) => legend.control!(legendContext(context))
                  : undefined,
              },
            }
          : spec.color,
        margin: {
          ...plot.margin,
          top: plot.margin.top + layout.top,
          bottom: plot.margin.bottom + layout.bottom,
        },
      };
    },
  };
  return {
    definition,
    renderSvg: figureRenderer<T, X, Y>(props, presentation),
  };
}
/** Renderer identity depends only on prose and presentation, never on interaction state. */
export function figureRenderer<T, X extends ChartValue, Y extends ChartValue>(
  props: FigureText,
  presentation: FigurePresentation,
): ChartSvgRenderer<T, X, Y> {
  return (scene, options) => {
    const layout = figureLayout(props, scene.width, scene.height, presentation);
    return renderChartSvg(
      { ...scene, nodes: [...scene.nodes, ...layout.lines] },
      options,
    );
  };
}
