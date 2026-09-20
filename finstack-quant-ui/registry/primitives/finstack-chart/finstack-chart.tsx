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
import { Chart } from "@tanstack/charts/react";
import type { ChartValue } from "@tanstack/charts";
import { composeFigure, type FigureSpec } from "./figure";
import { readPresentation, type FigurePresentation } from "./presentation";
import { exportFigure, type FigureExportOptions } from "./export";
export type { FigureSpec, FigureExportOptions };
export interface FigureHandle {
  exportSvg(options: FigureExportOptions): Promise<Blob>;
  exportPng(options: FigureExportOptions): Promise<Blob>;
}
/** Complete supplied-data SVG figure. Native chart definitions own axes and annotation marks. */
export function FinstackChart<T, X extends ChartValue, Y extends ChartValue>(
  props: FigureSpec<T, X, Y> & {
    height?: number;
    width?: number;
    ref?: Ref<FigureHandle>;
  },
) {
  const host = useRef<HTMLDivElement>(null);
  const id = useId();
  const [presentation, setPresentation] = useState<FigurePresentation | null>(
    null,
  );
  useEffect(() => {
    let active = true;
    const refresh = () => {
      if (active && host.current)
        setPresentation(readPresentation(host.current));
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
  const figure = useMemo(
    () => (presentation ? composeFigure(props, presentation) : null),
    [props, presentation],
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
          ariaLabel={props.ariaLabel}
          ariaDescription={props.ariaDescription}
          idPrefix={id}
          width={props.width}
          height={props.height ?? 420}
          measureText={presentation.measureText}
        />
      ) : (
        <div role="status" aria-label={props.ariaLabel}>
          Loading figure…
        </div>
      )}
    </div>
  );
}
