"use client";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  useId,
  useImperativeHandle,
  type Ref,
} from "react";
import { Chart, type ChartCommonProps } from "@tanstack/charts/react/tooltip";
import type { ChartValue } from "@tanstack/charts";
import { composeFigure, figureRenderer, type FigureSpec } from "./figure";
import { readPresentation, type FigurePresentation } from "./presentation";
import { exportFigure, type FigureExportOptions } from "./export";
export type { FigureSpec, FigureExportOptions };
export { heatmap } from "./heatmap";
export { chartSelection } from "./selection";
/** Native notifications preserve original data; onSelect reports activation, not acceptance. */
export type FigureInteractions<
  T,
  X extends ChartValue,
  Y extends ChartValue,
> = Pick<
  ChartCommonProps<T, X, Y>,
  "onFocusChange" | "onFocusGroupChange" | "onSelect" | "renderTooltipBody"
>;
export interface FigureHandle {
  exportSvg(options: FigureExportOptions): Promise<Blob>;
  exportPng(options: FigureExportOptions): Promise<Blob>;
}
/** Complete supplied-data SVG figure. Native chart definitions own axes and annotation marks. */
export function FinstackChart<T, X extends ChartValue, Y extends ChartValue>(
  props: FigureSpec<T, X, Y> &
    FigureInteractions<T, X, Y> & {
      height?: number;
      width?: number;
      ref?: Ref<FigureHandle>;
      /** Put source links in an on-screen disclosure; exports retain full source text. */
      sourceDisplay?: "inline" | "disclosure";
    },
) {
  const host = useRef<HTMLDivElement>(null);
  const id = useId();
  const [presentation, setPresentation] = useState<FigurePresentation | null>(
    null,
  );
  useEffect(() => {
    let active = true;
    let signature: string;
    const refresh = () => {
      if (active && host.current) {
        const next = readPresentation(host.current);
        // Exported embedded fonts can emit loadingdone without changing the
        // screen's typography. Avoid replacing its renderer and focused control.
        const nextSignature = JSON.stringify({
          ...next,
          metrics: next.measureText(
            "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
            {
              fontSize: next.bodySize,
              fontFamily: next.fontFamily,
              fontStyle: "normal",
              fontStretch: "normal",
              letterSpacing: 0,
              direction: "ltr",
              anchor: "start",
              baseline: "hanging",
              fontScale: 1,
            },
          ),
        });
        if (signature !== nextSignature) {
          signature = nextSignature;
          setPresentation(next);
        }
      }
    };
    void document.fonts.ready.then(refresh);
    document.fonts.addEventListener("loadingdone", refresh);
    const observer = new MutationObserver(refresh);
    for (
      let ancestor = host.current?.parentElement;
      ancestor;
      ancestor = ancestor.parentElement
    )
      observer.observe(ancestor, {
        attributes: true,
        attributeFilter: ["data-theme", "data-density", "class", "style"],
      });
    return () => {
      active = false;
      observer.disconnect();
      document.fonts.removeEventListener("loadingdone", refresh);
    };
  }, []);
  const { definition, title, subtitle, caption, sources, figureAnnotations } =
    props;
  const prose = useMemo(
    () => ({
      title,
      subtitle,
      caption,
      sources: props.sourceDisplay === "disclosure" ? undefined : sources,
      figureAnnotations,
    }),
    [title, subtitle, caption, sources, figureAnnotations, props.sourceDisplay],
  );
  const figure = useMemo(
    () =>
      presentation
        ? composeFigure(
            {
              ...prose,
              definition,
              ariaLabel: props.ariaLabel,
            },
            presentation,
          )
        : null,
    [definition, prose, props.ariaLabel, presentation],
  );
  const renderSvg = useMemo(
    () =>
      presentation ? figureRenderer<T, X, Y>(prose, presentation) : undefined,
    [prose, presentation],
  );
  useImperativeHandle(
    props.ref,
    () => ({
      exportSvg: (options) =>
        exportFigure(host.current!, props, options, "svg"),
      exportPng: (options) =>
        exportFigure(host.current!, props, options, "png"),
    }),
    [props],
  );
  return (
    <div
      ref={host}
      className="font-sans text-sm text-foreground"
      data-finstack-figure=""
    >
      {figure && presentation ? (
        <Chart
          {...figure}
          renderSvg={renderSvg}
          ariaLabel={props.ariaLabel}
          ariaDescription={props.ariaDescription}
          idPrefix={id}
          width={props.width}
          height={props.height ?? 420}
          measureText={presentation.measureText}
          onFocusChange={props.onFocusChange}
          onFocusGroupChange={props.onFocusGroupChange}
          onSelect={props.onSelect}
          renderTooltipBody={
            props.renderTooltipBody
              ? (context) => (
                  <div
                    onKeyDown={(event) => {
                      // The native host handles Enter/arrows on its container. Keep custom
                      // controls' browser defaults; Escape still reaches native dismissal.
                      if (event.key !== "Escape") event.stopPropagation();
                    }}
                  >
                    {props.renderTooltipBody!(context)}
                  </div>
                )
              : undefined
          }
        />
      ) : (
        <div role="status" aria-label={props.ariaLabel}>
          Loading figure…
        </div>
      )}
      {props.sourceDisplay === "disclosure" && sources?.length ? (
        <details className="mt-2 text-xs text-muted-foreground">
          <summary className="cursor-pointer">Sources</summary>
          <ul className="mt-2 space-y-1">
            {sources.map((source, index) => (
              <li key={index}>
                {source.url ? (
                  <a href={source.url} className="break-all underline">
                    {source.label}
                  </a>
                ) : (
                  source.label
                )}
              </li>
            ))}
          </ul>
        </details>
      ) : null}
    </div>
  );
}
