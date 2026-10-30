"use client";
import { useMemo, useState } from "react";
import {
  useFxDeltaSamples,
  type FxDeltaSampleRequest,
} from "@/hooks/models/use-fx-delta-samples/use-fx-delta-samples";
import {
  useFxStrikeSurface,
  type FxStrikeSurfaceRequest,
} from "@/hooks/models/use-fx-strike-surface/use-fx-strike-surface";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { FxDeltaQuotes } from "@/components/finstack/core/components/fx-delta-quotes/fx-delta-quotes";
import {
  EvaluatedSurfaceStatus,
  evaluatedSurfaceNodes,
  type SurfaceViewProps,
} from "@/components/finstack/core/components/vol-surface-chart/surface-view";
import {
  surfaceLabels,
  surfaceNodes,
  type SurfacePoint,
} from "@/components/finstack/core/components/vol-surface-chart/stored";
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
  /** Initial FX spot for the strike grid, domestic per unit of foreign. Omitted leaves the field empty. */
  initialSpot?: number;
  /** Initial continuously compounded domestic rate for the strike grid, as a decimal. */
  initialDomesticRate?: number;
  /** Initial continuously compounded foreign rate for the strike grid, as a decimal. */
  initialForeignRate?: number;
}
const decimal = new RegExp(contracts.decimal.schema.pattern);
/** Exact typed text as a number; null while the text is empty, incomplete or non-finite. */
function decimalValue(text: string): number | null {
  return decimal.test(text) && Number.isFinite(Number(text))
    ? Number(text)
    : null;
}
/** Rust-materialized strike-space grid for the explicit spot and rates; no local conversion. */
function FxStrikeGrid(props: FxSurfaceChartProps) {
  const [spot, setSpot] = useState(String(props.initialSpot ?? ""));
  const [domesticRate, setDomesticRate] = useState(
    String(props.initialDomesticRate ?? ""),
  );
  const [foreignRate, setForeignRate] = useState(
    String(props.initialForeignRate ?? ""),
  );
  const request = useMemo<FxStrikeSurfaceRequest | null>(() => {
    const inputs = [spot, domesticRate, foreignRate].map(decimalValue);
    if (inputs.some((value) => value === null)) return null;
    const [s, d, f] = inputs as number[];
    return {
      surface: props.surface,
      spot: s!,
      domesticRate: d!,
      foreignRate: f!,
    };
  }, [props.surface, spot, domesticRate, foreignRate]);
  const query = useFxStrikeSurface(request);
  const grid = query.data;
  const nodes = useMemo(() => (grid ? surfaceNodes(grid) : []), [grid]);
  const id = `${props.surface.id} strike grid`;
  return (
    <section aria-label={id} className="space-y-3">
      <h3 className="text-sm font-medium">Strike grid</h3>
      <div className="grid gap-3 sm:grid-cols-3">
        <DecimalInput
          label="Spot (domestic per unit of foreign)"
          value={spot}
          onValueChange={setSpot}
        />
        <DecimalInput
          label="Domestic rate (continuously compounded decimal)"
          value={domesticRate}
          onValueChange={setDomesticRate}
        />
        <DecimalInput
          label="Foreign rate (continuously compounded decimal)"
          value={foreignRate}
          onValueChange={setForeignRate}
        />
      </div>
      <EvaluatedSurfaceStatus
        request={request}
        error={query.error}
        data={grid}
        missing={
          <p>
            Enter a spot and both continuously compounded rates to materialize
            the strike grid.
          </p>
        }
        pending="Evaluating FX strike grid…"
        id={id}
        mode="evaluated"
        nodes={nodes}
        revision={JSON.stringify(request)}
        labels={
          grid
            ? surfaceLabels(grid)
            : { expiry: "Expiry (years)", secondary: "Strike", value: "" }
        }
        description={`Native models.volatility.materializeFxDeltaSurface output at the supplied spot and rates: the returned expiry by strike grid${grid ? ` (interpolation metadata: ${grid.interpolation_mode})` : ""}. Nodes are returned values; no local interpolation or delta-to-strike conversion.`}
        colorDomain={props.colorDomain}
        width={props.width}
        height={props.height}
      />
    </section>
  );
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
      <FxStrikeGrid {...props} />
    </section>
  );
}
