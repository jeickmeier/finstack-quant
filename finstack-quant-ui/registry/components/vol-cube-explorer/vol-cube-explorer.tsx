"use client";
import { useMemo, useState } from "react";
import {
  useCubeSamples,
  type CubeSampleRequest,
} from "@/hooks/use-cube-samples/use-cube-samples";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
import {
  EvaluatedSurfaceStatus,
  evaluatedSurfaceNodes,
  type SurfaceViewProps,
} from "../vol-surface-chart/surface-view";
import type { SurfacePoint } from "../vol-surface-chart/stored";
import { JsonViewer } from "../../primitives/json-viewer/json-viewer";
import { FinstackTable } from "../../primitives/finstack-table/finstack-table";
import { DecimalInput } from "../../primitives/decimal-input/decimal-input";
import { EnumField } from "../../primitives/enum-field/enum-field";
export interface CubePoint extends SurfacePoint {
  coordinate: CubeSampleRequest["coordinates"][number];
}
export interface VolCubeExplorerProps extends Omit<
  SurfaceViewProps<CubePoint>,
  | "id"
  | "mode"
  | "nodes"
  | "labels"
  | "description"
  | "revision"
  | "colorDomain"
> {
  cube: CubeSampleRequest["cube"];
  /** Absolute strike in the same rate units as stored forwards; never an offset. */
  initialStrike: number;
  initialConvention: CubeSampleRequest["convention"];
  /** Explicit extents in each evaluator's output units. */
  colorDomains: Record<
    CubeSampleRequest["convention"],
    readonly [number, number]
  >;
  /** Supplied sampling coordinates. Omit to sample the canonical expiry/tenor nodes. */
  coordinates?: readonly { expiry: number; tenor: number }[];
}
/** Checked native SABR evaluation with an explicit strike/convention and separate original parameters. */
export function VolCubeExplorer(props: VolCubeExplorerProps) {
  const [strike, setStrike] = useState(String(props.initialStrike));
  const [convention, setConvention] = useState(props.initialConvention);
  const request = useMemo<CubeSampleRequest | null>(() => {
    if (
      !new RegExp(contracts.decimal.schema.pattern).test(strike) ||
      !Number.isFinite(Number(strike))
    )
      return null;
    const coordinates =
      props.coordinates ??
      props.cube.expiries.flatMap((expiry) =>
        props.cube.tenors.map((tenor) => ({ expiry, tenor })),
      );
    return {
      cube: props.cube,
      convention,
      coordinates: coordinates.map((c) => ({ ...c, strike: Number(strike) })),
    };
  }, [props.cube, props.coordinates, convention, strike]);
  const query = useCubeSamples(request);
  const nodes = useMemo(
    () =>
      evaluatedSurfaceNodes(request, query.data, (coordinate, value) => ({
        coordinate,
        expiry: coordinate.expiry,
        secondary: coordinate.tenor,
        value,
        key: JSON.stringify([
          props.cube.id,
          coordinate.expiry,
          coordinate.tenor,
        ]),
      })),
    [request, query.data, props.cube.id],
  );
  const raw = useMemo(
    () =>
      props.cube.expiries.flatMap((expiry, row) =>
        props.cube.tenors.map((tenor, col) => {
          const index = row * props.cube.tenors.length + col;
          return {
            expiry,
            tenor,
            parameters: props.cube.params[index],
            forward: props.cube.forwards[index],
          };
        }),
      ),
    [props.cube],
  );
  const valueLabel =
    convention === "normal"
      ? "Normal volatility (annualized absolute rate)"
      : "Black/lognormal volatility (annualized decimal)";
  return (
    <section
      aria-label={`${props.cube.id} cube explorer`}
      className="space-y-4 font-sans text-sm text-foreground"
    >
      <h2>{props.cube.id} volatility cube</h2>
      <DecimalInput
        label="Absolute strike (rate units)"
        value={strike}
        onValueChange={setStrike}
      />
      <EnumField
        label="Output convention"
        value={convention}
        onValueChange={(value) =>
          setConvention(value as CubeSampleRequest["convention"])
        }
        options={[
          { value: "normal", label: "Normal/Bachelier" },
          { value: "black_lognormal", label: "Black/lognormal" },
        ]}
      />
      <p>
        Raw SABR parameters and forwards. Interpolation metadata:{" "}
        {props.cube.interpolation_mode}. Missing shifts remain absent.
      </p>
      <FinstackTable
        data={raw}
        getRowId={(row) => JSON.stringify([row.expiry, row.tenor])}
        caption="Raw cube nodes"
        columns={[
          {
            id: "expiry",
            header: "Expiry (years)",
            accessorFn: (row) => row.expiry,
          },
          {
            id: "tenor",
            header: "Tenor (years)",
            accessorFn: (row) => row.tenor,
          },
          {
            id: "forward",
            header: "Forward (rate units)",
            accessorFn: (row) => row.forward,
          },
          ...(["alpha", "beta", "rho", "nu", "shift"] as const).map((key) => ({
            id: key,
            header: key,
            accessorFn: (row: (typeof raw)[number]) =>
              row.parameters?.[key] ?? "Not supplied",
          })),
        ]}
      />
      <JsonViewer
        label="Complete cube state"
        text={serializeHost(props.cube)}
      />
      <EvaluatedSurfaceStatus
        {...props}
        request={request}
        error={query.error}
        data={query.data}
        missing={<p>Enter a finite absolute strike to evaluate the cube.</p>}
        pending="Evaluating volatility cube…"
        id={props.cube.id}
        mode="evaluated"
        nodes={nodes}
        revision={JSON.stringify(request)}
        colorDomain={props.colorDomains[convention]}
        labels={{
          expiry: "Expiry (years)",
          secondary: "Tenor (years)",
          value: valueLabel,
        }}
        description={`Native ${valueLabel} at absolute strike ${strike}. Samples use the selected checked evaluator; no coordinate clamping or forward-offset conversion.`}
      />
    </section>
  );
}
