"use client";
import { useMemo } from "react";
import {
  useFxDeltaSamples,
  type FxDeltaSampleRequest,
} from "@/hooks/models/use-fx-delta-samples/use-fx-delta-samples";
import { FxDeltaQuotes } from "@/components/finstack/core/components/fx-delta-quotes/fx-delta-quotes";
import {
  EvaluatedSurfaceStatus,
  evaluatedSurfaceNodes,
  type SurfaceViewProps,
} from "@/components/finstack/core/components/vol-surface-chart/surface-view";
import type { SurfacePoint } from "@/components/finstack/core/components/vol-surface-chart/stored";
export interface FxSamplePoint extends SurfacePoint {
  coordinate: FxDeltaSampleRequest["coordinates"][number];
}
export interface FxSurfaceChartProps extends Omit<
  SurfaceViewProps<FxSamplePoint>,
  "id" | "mode" | "nodes" | "labels" | "description" | "revision"
> {
  surface: FxDeltaSampleRequest["surface"];
  /** Original expiry/strike pairs; each forward must be explicitly supplied in quote/base units. */
  coordinates?: readonly {
    expiry: number;
    strike: number;
    forward?: number | null;
  }[];
}
/** Native evaluated Black volatility, kept distinct from the raw ATM/RR/BF quote arrays. */
export function FxSurfaceChart(props: FxSurfaceChartProps) {
  const request = useMemo<FxDeltaSampleRequest | null>(() => {
    if (
      !props.coordinates?.length ||
      props.coordinates.some((c) => c.forward == null)
    )
      return null;
    return {
      surface: props.surface,
      coordinates: props.coordinates as FxDeltaSampleRequest["coordinates"],
    };
  }, [props.surface, props.coordinates]);
  const query = useFxDeltaSamples(request);
  const nodes = useMemo(
    () =>
      evaluatedSurfaceNodes(request, query.data, (coordinate, value) => ({
        coordinate,
        expiry: coordinate.expiry,
        secondary: coordinate.strike,
        value,
        key: JSON.stringify([
          props.surface.id,
          coordinate.expiry,
          coordinate.strike,
        ]),
      })),
    [request, query.data, props.surface.id],
  );
  return (
    <section
      aria-label={`${props.surface.id} FX surface`}
      className="space-y-4"
    >
      <FxDeltaQuotes surface={props.surface} />
      <EvaluatedSurfaceStatus
        {...props}
        request={request}
        error={query.error}
        data={query.data}
        missing={
          <p>
            Supply an explicit forward for every expiry/strike coordinate to
            evaluate volatility.
          </p>
        }
        pending="Evaluating FX volatility…"
        id={props.surface.id}
        mode="evaluated"
        nodes={nodes}
        revision={JSON.stringify(request)}
        labels={{
          expiry: "Expiry (years)",
          secondary: "Strike (quote currency)",
          value: "Evaluated Black/lognormal volatility (annualized decimal)",
        }}
        description="Native FX delta-surface evaluation at the supplied coordinates and explicit quote/base forwards. These values are evaluated samples, not stored quote nodes."
      />
    </section>
  );
}
