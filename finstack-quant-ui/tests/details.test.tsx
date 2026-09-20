// @vitest-environment jsdom
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { ValuationDetails } from "../registry/components/valuation-details/valuation-details";
import { CovenantReport } from "../registry/components/covenant-report/covenant-report";
import {
  adaptValuation,
  exportValuation,
  valuationCodec,
  type ValuationResult,
} from "../src/host";
import { serializeHost } from "../src/codec.mjs";
import { restore } from "./details/restore";
import fixture from "./details/cases.json";
const native = createRequire(import.meta.url)(
  "../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
);
afterEach(cleanup);
// Native meta timestamps are wall-clock stamps, including nested composite legs.
function withoutClock(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(withoutClock);
  if (value && typeof value === "object")
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => [
        key,
        key === "meta" && child && typeof child === "object"
          ? withoutClock(
              Object.fromEntries(
                Object.entries(child).filter(([name]) => name !== "timestamp"),
              ),
            )
          : withoutClock(child),
      ]),
    );
  return value;
}
it("pins fixture sources and restores exact current host transport for all five required variants", () => {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  for (const source of fixture.sources)
    expect(
      createHash("sha256")
        .update(readFileSync(resolve(root, source.path)))
        .digest("hex"),
    ).toBe(source.sha256);
  expect([...new Set(fixture.cases.map((c) => c.detailType))].sort()).toEqual([
    "composite",
    "credit_derivative",
    "fx",
    "monte_carlo",
    "structured_credit_stochastic",
  ]);
  for (const entry of fixture.cases) {
    const r = entry.request;
    const current = native.priceInstrument(
      r.instrumentJson,
      r.marketJson,
      r.asOf,
      r.model,
      r.metrics,
    );
    const restored = restore(entry);
    expect(current.details.type).toBe(entry.detailType);
    expect(withoutClock(restored)).toEqual(withoutClock(current));
    expect(adaptValuation(restored)).toEqual(restored);
    expect(structuredClone(restored)).toEqual(restored);
    expect(exportValuation(restored, native.validateValuationResultJson)).toBe(
      entry.resultJson,
    );
  }
});
it.each(fixture.cases)(
  "renders actual $type details without dropping nullable fields or stamps",
  async (entry) => {
    const result = restore(entry);
    const tree = (density: "compact" | "comfortable") => (
      <ValuationDetails result={result} density={density} />
    );
    const { rerender } = render(tree("compact"));
    if (entry.detailType === "monte_carlo") {
      const panel = await screen.findByRole("region", {
        name: "Monte Carlo diagnostics",
      });
      expect(
        within(panel).getByText(
          (result.details as { data: { seed: bigint } }).data.seed.toString(),
        ),
      ).toBeTruthy();
      expect(panel.textContent).toContain("frozen fitted exercise policy");
    } else {
      await waitFor(() =>
        expect(document.querySelectorAll("pre")[0]?.textContent).toBe(
          serializeHost(result.details!.data),
        ),
      );
    }
    for (const density of ["compact", "comfortable"] as const) {
      rerender(tree(density));
      expect(
        screen.getByRole("region", { name: "Valuation details" }).dataset
          .density,
      ).toBe(density);
      expect(
        screen
          .getByRole("region", {
            name: "Complete valuation result JSON",
            hidden: true,
          })
          .querySelector("pre")!.textContent,
      ).toBe(serializeHost(result));
    }
  },
);
it("retains full-width seed digits through display, clone, copy and native canonical export", async () => {
  const result = restore(fixture.cases.find((c) => c.type === "bond")!);
  if (result.details?.type !== "monte_carlo")
    throw new Error("Missing native diagnostics");
  expect(result.details.data.seed).toBeGreaterThan(
    BigInt(Number.MAX_SAFE_INTEGER),
  );
  // u64 boundary exercises transport only; it is not a repricing claim.
  result.details.data.seed = 18446744073709551615n;
  const copy = vi.fn().mockResolvedValue(undefined);
  Object.defineProperty(navigator, "clipboard", {
    configurable: true,
    value: { writeText: copy },
  });
  render(<ValuationDetails result={structuredClone(result)} />);
  expect(await screen.findByText("18446744073709551615")).toBeTruthy();
  fireEvent.click(screen.getByText("Complete result JSON"));
  const raw = screen.getByRole("region", {
    name: "Complete valuation result JSON",
  });
  fireEvent.click(within(raw).getByRole("button", { name: "Copy" }));
  await waitFor(() => expect(copy).toHaveBeenCalledWith(serializeHost(result)));
  const canonical = exportValuation(result, native.validateValuationResultJson);
  expect(
    (valuationCodec.parse(canonical) as { details: { data: { seed: bigint } } })
      .details.data.seed,
  ).toBe(18446744073709551615n);
});
it.each(["future_variant", "__proto__", "constructor"])(
  "warns and preserves unknown discriminator %s",
  async (type) => {
    const result = {
      ...restore(fixture.cases[0]),
      details: {
        type,
        data: {
          seed: 18446744073709551615n,
          empty: null,
          meta: { version: "future" },
        },
      },
    } as unknown as ValuationResult;
    render(<ValuationDetails result={result} />);
    expect(screen.getByRole("alert").textContent).toContain(type);
    const raw = await screen.findByRole("region", {
      name: "Unrecognized details JSON",
    });
    expect(raw.querySelector("pre")!.textContent).toBe(
      serializeHost(result.details!.data),
    );
  },
);
it("shows absence distinctly and preserves actual native covenant reports without computing verdicts", () => {
  const r = fixture.covenantRequest;
  const reports = native.evaluateEngine(r.engineJson, r.metricsJson, r.asOf);
  expect(serializeHost(reports)).toBe(fixture.covenantReportsJson);
  render(<CovenantReport value={reports} />);
  expect(document.querySelector("pre")!.textContent).toBe(
    fixture.covenantReportsJson,
  );
  cleanup();
  const absent = restore(fixture.cases[0]);
  delete absent.details;
  absent.explanation = null;
  absent.covenants = null;
  render(<ValuationDetails result={absent} />);
  expect(screen.getByText("No valuation details returned")).toBeTruthy();
  expect(screen.queryByText("Explanation")).toBeNull();
  expect(screen.queryByText("Covenants")).toBeNull();
});
it("preserves optional canonical trace and covenant sections as raw transport", async () => {
  // Core explain.rs test_trace_serialization supplies this trace shape and values.
  // Pricing's facade has no ExplainOpts input: this validates transport, not a generated pricing trace.
  const explanation = {
    type: "calibration",
    entries: [
      {
        kind: "calibration_iteration",
        iteration: 0,
        residual: 0.005,
        knots_updated: ["2025-01-15"],
        converged: false,
      },
    ],
  };
  const reports = native.evaluateEngine(
    fixture.covenantRequest.engineJson,
    fixture.covenantRequest.metricsJson,
    fixture.covenantRequest.asOf,
  );
  const result = {
    ...restore(fixture.cases[0]),
    explanation,
    covenants: reports,
  };
  expect(() =>
    native.validateValuationResultJson(serializeHost(result)),
  ).not.toThrow();
  render(<ValuationDetails result={result} />);
  const trace = await screen.findByRole("region", {
    name: "Explanation JSON",
    hidden: true,
  });
  expect(trace.querySelector("pre")!.textContent).toBe(
    serializeHost(explanation),
  );
  const covenants = await screen.findByRole("region", {
    name: "Covenant reports JSON",
    hidden: true,
  });
  expect(covenants.querySelector("pre")!.textContent).toBe(
    serializeHost(result.covenants),
  );
});
