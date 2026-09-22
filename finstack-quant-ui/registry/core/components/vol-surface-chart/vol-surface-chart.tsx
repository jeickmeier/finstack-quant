"use client";
import { useMemo } from "react";
import { SurfaceView, type SurfaceViewProps } from "./surface-view";
import {
  surfaceNodes,
  surfaceLabels,
  type SurfaceNode,
  type StoredSurface,
} from "./stored";
export { surfaceNodes, surfaceLabels, surfaceSlices } from "./stored";
export interface VolSurfaceChartProps extends Omit<
  SurfaceViewProps<SurfaceNode>,
  "id" | "mode" | "nodes" | "labels" | "description" | "revision"
> {
  /** Canonical validated stored grid; no off-grid evaluation is available here. */
  surface: StoredSurface;
}
/** Exact stored values with canonical metadata; shared presentation owns links and exports. */
export function VolSurfaceChart(props: VolSurfaceChartProps) {
  const nodes = useMemo(() => surfaceNodes(props.surface), [props.surface]);
  const labels = surfaceLabels(props.surface);
  return (
    <SurfaceView
      {...props}
      id={props.surface.id}
      mode="stored"
      nodes={nodes}
      labels={labels}
      revision={JSON.stringify(props.surface)}
      description={`${labels.value} · Interpolation metadata: ${props.surface.interpolation_mode}. Views show stored nodes only.`}
    />
  );
}
