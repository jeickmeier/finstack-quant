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
  /** Expiry interpolation used to materialize the strike grid. */
  interpolationMode: FxStrikeSurfaceRequest["interpolationMode"];
  /** Initial qualified delivery forward rates aligned with `surface.expiries` (quote per base). Omitted leaves fields empty. */
  initialDeliveryForwards?: readonly number[];
}
const decimal = new RegExp(contracts.decimal.schema.pattern);
/** Exact typed text as a number; null while the text is empty, incomplete or non-finite. */
function decimalValue(text: string): number | null {
  return decimal.test(text) && Number.isFinite(Number(text))
    ? Number(text)
    : null;
}
function initialForwardValues(
  expiries: readonly number[],
  initial?: readonly number[],
): string[] {
  return expiries.map((_, index) => {
    const value = initial?.[index];
    return value === undefined ? "" : String(value);
  });
}

/** Rust-materialized strike-space grid for the explicit delivery forwards; no local conversion. */
function FxStrikeGrid(props: FxSurfaceChartProps) {
  const [forwardInputs, setForwardInputs] = useState<string[]>(() =>
    initialForwardValues(props.surface.expiries, props.initialDeliveryForwards),
  );
  const alignedInputs = useMemo(
    () => props.surface.expiries.map((_, index) => forwardInputs[index] ?? ""),
    [props.surface.expiries, forwardInputs],
  );
  const request = useMemo<FxStrikeSurfaceRequest | null>(() => {
    if (props.surface.expiries.length === 0) return null;
    const parsed = alignedInputs.map(decimalValue);
    if (parsed.some((value) => value === null)) return null;
    return {
      surface: props.surface,
      interpolationMode: props.interpolationMode,
      deliveryForwards: parsed as number[],
    };
  }, [props.surface, props.interpolationMode, alignedInputs]);
  const query = useFxStrikeSurface(request);
  const grid = query.data;
  const nodes = useMemo(() => (grid ? surfaceNodes(grid) : []), [grid]);
  const id = `${props.surface.id} strike grid`;
  return (
    <section aria-label={id} className="space-y-3">
      <h3 className="text-sm font-medium">Strike grid</h3>
      <div className="grid gap-3 sm:grid-cols-3">
        {props.surface.expiries.map((expiry, index) => (
          <DecimalInput
            key={`${index}:${expiry}`}
            label={`Delivery forward (${expiry}y)`}
            value={alignedInputs[index] ?? ""}
            onValueChange={(next) =>
              setForwardInputs((current) =>
                props.surface.expiries.map((_, i) =>
                  i === index ? next : (current[i] ?? ""),
                ),
              )
            }
          />
        ))}
      </div>
      <EvaluatedSurfaceStatus
        request={request}
        error={query.error}
        data={grid}
        missing={
          <p>
            Enter a delivery forward for every expiry to materialize the strike
            grid.
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
        description={`Native models.volatility.materializeFxDeltaSurface output at the supplied delivery forwards: the returned expiry by strike grid${grid ? ` (interpolation metadata: ${grid.interpolation_mode})` : ""}. Nodes are returned values; no local interpolation or delta-to-strike conversion.`}
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
