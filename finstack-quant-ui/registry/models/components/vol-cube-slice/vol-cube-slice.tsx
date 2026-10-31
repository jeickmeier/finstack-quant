"use client";
import { useMemo, useState } from "react";
import { defineChart, colorLegend, lineY } from "@tanstack/charts";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import {
  useCubeSlice,
  type CubeSlice,
  type CubeSliceRequest,
} from "@/hooks/models/use-cube-slice/use-cube-slice";
import {
  useCubeSmile,
  type CubeSmileRequest,
} from "@/hooks/models/use-cube-smile/use-cube-smile";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
import { serializeHost } from "@/lib/finstack/codec.mjs";
import {
  EvaluatedSurfaceStatus,
  type SurfaceViewProps,
} from "@/components/finstack/core/components/vol-surface-chart/surface-view";
import {
  surfaceLabels,
  surfaceNodes,
  type SurfacePoint,
} from "@/components/finstack/core/components/vol-surface-chart/stored";
import { FinstackChart } from "@/components/finstack/shared/chart/finstack-chart/finstack-chart";
import { FinstackTable } from "@/components/finstack/shared/table/finstack-table/finstack-table";
import { JsonViewer } from "@/components/finstack/shared/primitives/json-viewer/json-viewer";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
export interface VolCubeSliceProps extends Pick<
  SurfaceViewProps<SurfacePoint>,
  "width" | "height"
> {
  cube: CubeSliceRequest["cube"];
  /** Absolute strikes in the stored forwards' rate units, in the order given; Rust requires them strictly increasing and inside the SABR domain. */
  initialStrikes: readonly number[];
  initialConvention: CubeSliceRequest["convention"];
  /** Fixed axis shown first; omitted fixes the underlying tenor. */
  initialAxis?: CubeSliceRequest["axis"];
  /** Continuously compounded decimal rate for the smile's arbitrage check. Omitted leaves the field empty. */
  initialRate?: number;
  /** Repair passes for SabrSmile#repairArbitrage. Omitted runs no repair. */
  repairIterations?: number;
  /** Explicit extents in each evaluator's output units. */
  colorDomains: Record<
    CubeSliceRequest["convention"],
    readonly [number, number]
  >;
}
const decimal = new RegExp(contracts.decimal.schema.pattern);
/** Exact typed text as a number; null while the text is empty, incomplete or non-finite. */
function decimalValue(text: string): number | null {
  return decimal.test(text) && Number.isFinite(Number(text))
    ? Number(text)
    : null;
}
/** Returned slice values as surface points; the row axis is expiry (tenor slice) or tenor (expiry slice). */
function sliceView(result: CubeSlice) {
  if (result.axis === "tenor")
    return {
      nodes: surfaceNodes(result.surface),
      labels: surfaceLabels(result.surface),
      displacements: result.surface.displacements,
    };
  const { slice } = result;
  return {
    nodes: slice.tenors.flatMap((tenor, row) =>
      slice.strikes.map((strike, column) => ({
        expiry: tenor,
        secondary: strike,
        // finstack-arith: row-major index into the supplied flat grid
        value: slice.vols_row_major[row * slice.strikes.length + column]!,
        key: JSON.stringify([slice.id, slice.expiry, tenor, strike]),
      })),
    ),
    labels: {
      expiry: "Tenor (years)",
      secondary: "Strike",
      value:
        slice.quote_type === "normal"
          ? "Normal volatility (raw)"
          : "Black/lognormal volatility (raw)",
    },
    displacements: slice.displacements,
  };
}
interface SmilePoint {
  series: string;
  strike: number;
  vol: number;
}
/** Rust-materialized cube slice at explicit strikes, plus the native SABR smile of one cube node. */
export function VolCubeSlice(props: VolCubeSliceProps) {
  const { cube } = props;
  const [axis, setAxis] = useState(props.initialAxis ?? "tenor");
  const [convention, setConvention] = useState(props.initialConvention);
  const [tenorIndex, setTenorIndex] = useState(0);
  const [expiryIndex, setExpiryIndex] = useState(0);
  const [strikes, setStrikes] = useState(() =>
    props.initialStrikes.map(String),
  );
  const [rate, setRate] = useState(String(props.initialRate ?? ""));
  const values = useMemo(() => {
    const parsed = strikes.map(decimalValue);
    return parsed.length && parsed.every((value) => value !== null)
      ? (parsed as number[])
      : null;
  }, [strikes]);
  const sliceRequest = useMemo<CubeSliceRequest | null>(
    () =>
      values && {
        cube,
        axis,
        at:
          axis === "tenor"
            ? cube.tenors[tenorIndex]!
            : cube.expiries[expiryIndex]!,
        strikes: values,
        convention,
      },
    [cube, axis, tenorIndex, expiryIndex, values, convention],
  );
  const smileRequest = useMemo<CubeSmileRequest | null>(() => {
    const r = decimalValue(rate);
    return values && r !== null
      ? {
          cube,
          expiryIndex,
          tenorIndex,
          strikes: values,
          rate: r,
          repairIterations: props.repairIterations,
        }
      : null;
  }, [cube, expiryIndex, tenorIndex, values, rate, props.repairIterations]);
  const slice = useCubeSlice(sliceRequest);
  const smile = useCubeSmile(smileRequest);
  const view = useMemo(
    () => (slice.data ? sliceView(slice.data) : null),
    [slice.data],
  );
  const smilePoints = useMemo<SmilePoint[]>(() => {
    if (!smileRequest || !smile.data) return [];
    const { vols, repaired } = smile.data;
    return [
      ...smileRequest.strikes.map((strike, i) => ({
        series: "SABR",
        strike,
        vol: vols[i]!,
      })),
      ...(repaired ?? []).map((vol, i) => ({
        series: "Repaired",
        strike: smileRequest.strikes[i]!,
        vol,
      })),
    ];
  }, [smileRequest, smile.data]);
  const smileValue = "SABR implied volatility (raw)";
  const definition = useMemo(
    () =>
      defineChart({
        marks: [
          lineY(smilePoints, {
            x: (p) => p.strike,
            y: (p) => p.vol,
            z: (p) => p.series,
            key: (p) => JSON.stringify([p.series, p.strike]),
            points: true,
          }),
        ],
        scales: {
          x: { scale: scaleLinear, axis: { label: "Strike (rate units)" } },
          y: { scale: scaleLinear, axis: { label: smileValue } },
        },
        color: { legend: colorLegend() },
        tooltip: {
          use: tooltip,
          content: (points) => ({
            rows: points.map((point) => ({
              label: `${point.datum.series} · strike ${point.datum.strike}`,
              value: String(point.datum.vol),
            })),
          }),
        },
      }),
    [smilePoints],
  );
  const fixed = axis === "tenor" ? "tenor" : "expiry";
  const node = {
    expiry: cube.expiries[expiryIndex]!,
    tenor: cube.tenors[tenorIndex]!,
  };
  const id = `${cube.id} ${fixed} slice`;
  const axisOptions = (axisValues: readonly number[]) =>
    axisValues.map((value, index) => ({
      value: String(index),
      label: String(value),
    }));
  return (
    <section
      aria-label={`${cube.id} cube slice`}
      className="space-y-4 font-sans text-sm text-foreground"
    >
      <h3 className="text-sm font-medium">Cube slice and node smile</h3>
      <div className="grid gap-3 sm:grid-cols-2">
        <EnumField
          label="Fixed axis"
          value={axis}
          onValueChange={(value) => setAxis(value as CubeSliceRequest["axis"])}
          options={[
            { value: "tenor", label: "Underlying tenor" },
            { value: "expiry", label: "Option expiry" },
          ]}
        />
        <EnumField
          label="Slice convention"
          value={convention}
          onValueChange={(value) =>
            setConvention(value as CubeSliceRequest["convention"])
          }
          options={[
            { value: "normal", label: "Normal/Bachelier" },
            { value: "black_lognormal", label: "Black/lognormal" },
          ]}
        />
        <EnumField
          label="Tenor node (years)"
          layout="select"
          value={String(tenorIndex)}
          onValueChange={(value) => setTenorIndex(Number(value))}
          options={axisOptions(cube.tenors)}
        />
        <EnumField
          label="Expiry node (years)"
          layout="select"
          value={String(expiryIndex)}
          onValueChange={(value) => setExpiryIndex(Number(value))}
          options={axisOptions(cube.expiries)}
        />
      </div>
      <fieldset className="space-y-2">
        <legend>Absolute strikes (rate units), strictly increasing</legend>
        <div className="grid gap-3 sm:grid-cols-4">
          {strikes.map((text, index) => (
            <DecimalInput
              key={index}
              // finstack-arith: 1-based ordinal for a label
              label={`Strike ${index + 1}`}
              value={text}
              onValueChange={(next) =>
                setStrikes((current) =>
                  current.map((entry, i) => (i === index ? next : entry)),
                )
              }
            />
          ))}
        </div>
      </fieldset>
      <EvaluatedSurfaceStatus
        request={sliceRequest}
        error={slice.error}
        data={slice.data}
        missing={
          <p>Enter every strike as a finite decimal to slice the cube.</p>
        }
        pending="Materializing cube slice…"
        id={id}
        mode="evaluated"
        nodes={view?.nodes ?? []}
        revision={JSON.stringify(sliceRequest)}
        labels={
          view?.labels ?? {
            expiry: "Expiry (years)",
            secondary: "Strike",
            value: "",
          }
        }
        description={`Native models.volatility.materializeCube${axis === "tenor" ? "Tenor" : "Expiry"}Slice${convention === "normal" ? "Normal" : ""} output at fixed ${fixed} ${axis === "tenor" ? node.tenor : node.expiry} years: the returned grid at the supplied strikes. Values off the cube's nodes are Rust-interpolated; no local interpolation or conversion.${view?.displacements ? ` Shifted-Black displacements (rate units): ${view.displacements.join(", ")}.` : ""}`}
        colorDomain={props.colorDomains[convention]}
        width={props.width}
        height={props.height}
      />
      <section aria-label={`${cube.id} node smile`} className="space-y-3">
        <h4 className="text-sm font-medium">
          SABR smile at expiry {node.expiry}, tenor {node.tenor} (years)
        </h4>
        <DecimalInput
          label="Rate for the arbitrage check (continuously compounded decimal)"
          value={rate}
          onValueChange={setRate}
        />
        {!smileRequest ? (
          <p>
            Enter every strike and a continuously compounded rate to evaluate
            the node smile.
          </p>
        ) : smile.error ? (
          <p role="alert">{smile.error.message}</p>
        ) : !smile.data ? (
          <p role="status">Evaluating SABR smile…</p>
        ) : (
          <>
            <p className="text-xs text-muted-foreground">
              Native SabrSmile values for the node&apos;s parameters and forward{" "}
              {smile.data.forward} (rate units) at expiry {smile.data.expiry}{" "}
              years: normal volatility when beta &lt; 1e-4, Black volatility of
              the (shifted) forward otherwise.{" "}
              {smile.data.repaired
                ? `Repaired values come from repairArbitrage with ${props.repairIterations} passes.`
                : "No repair was requested."}
            </p>
            <p role="status">
              {smile.data.arbitrage.arbitrage_free
                ? "Rust arbitrage check: no butterfly or monotonicity violations."
                : `Rust arbitrage check: ${smile.data.arbitrage.butterfly_violations.length} butterfly and ${smile.data.arbitrage.monotonicity_violations.length} monotonicity violations.`}
            </p>
            <FinstackChart
              definition={definition}
              ariaLabel={`${cube.id} node smile`}
              title={`${cube.id} SABR smile`}
              width={props.width}
              height={320}
            />
            <FinstackTable
              data={smileRequest.strikes.map((strike, i) => ({
                strike,
                sabr: smile.data!.vols[i]!,
                repaired: smile.data!.repaired?.[i],
              }))}
              getRowId={(row) => String(row.strike)}
              caption="Node smile values"
              columns={[
                {
                  id: "strike",
                  header: "Strike (rate units)",
                  accessorFn: (row) => row.strike,
                },
                {
                  id: "sabr",
                  header: smileValue,
                  accessorFn: (row) => row.sabr,
                },
                {
                  id: "repaired",
                  header: "Repaired volatility (raw)",
                  accessorFn: (row) => row.repaired ?? "Not requested",
                },
              ]}
            />
            <FinstackTable
              data={[smile.data.parameters]}
              getRowId={() => "parameters"}
              caption="Node SABR parameters"
              columns={(["alpha", "beta", "rho", "nu", "shift"] as const).map(
                (key) => ({
                  id: key,
                  header: key,
                  accessorFn: (row: CubeSmileParameters) =>
                    row[key] ?? "Not supplied",
                }),
              )}
            />
            <JsonViewer
              label="Native arbitrage report"
              text={serializeHost(smile.data.arbitrage)}
            />
          </>
        )}
      </section>
    </section>
  );
}
type CubeSmileParameters = CubeSliceRequest["cube"]["params"][number];
