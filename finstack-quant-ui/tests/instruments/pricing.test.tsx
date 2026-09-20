// @vitest-environment jsdom
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { beforeAll, afterAll, afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import Big from "big.js";
import init, { valuations } from "../../../finstack-quant-wasm/index.js";
import { instruments } from "../../src/generated/instruments";
import { SchemaForm } from "../../registry/components/schema-form/schema-form";
import type { InstrumentModule } from "../../registry/components/schema-form/schema";
import {
  unwrap,
  errorValue,
  type PriceRequest,
  type WorkerApi,
} from "../../registry/workers/finstack-contract";
import { startWorker } from "../worker/harness.mjs";
import manifest from "./pricing-cases.json";

type Case = (typeof manifest.cases)[number];
const repository = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
let worker: { proxy: WorkerApi; close(): Promise<void> };
beforeAll(async () => {
  await init({
    module_or_path: readFileSync(
      resolve(
        repository,
        "finstack-quant-wasm/pkg/finstack_quant_wasm_bg.wasm",
      ),
    ),
  });
  worker = await startWorker();
}, 30000);
afterAll(async () => worker?.close());
afterEach(cleanup);

type ObjectValue = Record<string | number, unknown>;
function at(value: unknown, path: readonly (string | number)[]): unknown {
  for (const key of path) value = (value as ObjectValue)[key];
  return value;
}
function set(
  value: unknown,
  path: readonly (string | number)[],
  next: unknown,
) {
  const parent = at(value, path.slice(0, -1)) as ObjectValue;
  parent[path.at(-1)!] = structuredClone(next);
}
function read(source: Case["instrumentSource"]) {
  const bytes = readFileSync(resolve(repository, source.path));
  expect(createHash("sha256").update(bytes).digest("hex"), source.path).toBe(
    source.sha256,
  );
  return at(JSON.parse(bytes.toString()), source.pointer);
}
function requestFor(entry: Case): PriceRequest {
  const instrument = read(entry.instrumentSource);
  for (const patch of entry.instrumentPatches)
    set(instrument, patch.path, patch.value);
  const market = read(entry.marketSource);
  for (const patch of entry.marketPatches) set(market, patch.path, patch.value);
  return {
    ...entry.request,
    instrumentJson: JSON.stringify(instrument),
    marketJson: JSON.stringify(market),
  };
}
function direct(request: PriceRequest) {
  return valuations.instruments.priceInstrument(
    request.instrumentJson,
    request.marketJson,
    request.asOf,
    request.model ?? undefined,
    request.metrics ? [...request.metrics] : undefined,
    request.pricingOptions ?? undefined,
    request.marketHistory ?? undefined,
  );
}
function same(
  actual: ReturnType<typeof direct>,
  expected: ReturnType<typeof direct>,
) {
  function withoutClocks(value: unknown): unknown {
    if (Array.isArray(value)) return value.map(withoutClocks);
    if (!value || typeof value !== "object") return value;
    return Object.fromEntries(
      Object.entries(value).map(([key, child]) => {
        if (
          key === "meta" &&
          child &&
          typeof child === "object" &&
          "timestamp" in child
        ) {
          expect(typeof child.timestamp).toBe("string");
          return [key, { ...child, timestamp: "<clock>" }];
        }
        return [key, withoutClocks(child)];
      }),
    );
  }
  expect(withoutClocks(actual)).toEqual(withoutClocks(expected));
}
function reference(
  actual: ReturnType<typeof direct>["value"],
  expected: Case["reference"]["baseline"],
) {
  expect(actual.currency).toBe(expected.currency);
  // Captured native values constrain the case; direct worker/facade comparison is exact.
  expect(
    new Big(actual.amount)
      .minus(expected.amount)
      .abs()
      .lte(new Big(expected.amount).abs().times("1e-10").plus("1e-7")),
  ).toBe(true);
}

it("pins exactly one price-changing case per canonical instrument and detects source drift", () => {
  expect(manifest.cases.map((c) => c.type).sort()).toEqual(
    instruments.map((i) => i.type).sort(),
  );
  expect(manifest.cases).toHaveLength(78);
  for (const source of [
    ...manifest.marketProvenance,
    ...manifest.instrumentDependencies,
  ])
    read({ ...source, pointer: [] });
  for (const entry of manifest.cases) {
    const request = requestFor(entry);
    expect(at(JSON.parse(request.instrumentJson), entry.edit.path)).toEqual(
      entry.edit.from,
    );
    expect(entry.reference.baseline.amount).not.toBe(
      entry.reference.edited.amount,
    );
  }
});
it.each(manifest.cases)(
  "$type edits a financial control and prices through the worker",
  async (entry) => {
    const module = (await instruments
      .find((i) => i.type === entry.type)!
      .loader()) as InstrumentModule;
    const request = requestFor(entry);
    const baseline = direct(request);
    reference(baseline.value, entry.reference.baseline);
    same(unwrap(await worker.proxy.price(request)), baseline);
    const validated = valuations.instruments.validateInstrumentJson(
      request.instrumentJson,
    );
    const initial = module.codec.parse(validated) as Record<string, unknown>;
    const submit = vi.fn();
    const validate = async (instrumentJson: string) =>
      unwrap(
        await worker.proxy.validate({ instrumentJson, revision: entry.type }),
      ).json;
    const { container } = render(
      <SchemaForm
        module={module}
        defaultValues={initial}
        validate={validate}
        onSubmit={submit}
        layout="full"
      />,
    );
    const fieldPath = entry.edit.path.reduce<string>(
      (path, segment) =>
        typeof segment === "number"
          ? `${path}[${segment}]`
          : path
            ? `${path}.${segment}`
            : segment,
      "",
    );
    const control = container.querySelector(
      `[data-field-path="${fieldPath}"] input`,
    ) as HTMLInputElement;
    expect(control, fieldPath).not.toBeNull();
    fireEvent.change(control, { target: { value: String(entry.edit.to) } });
    await waitFor(
      () =>
        expect(
          (
            container.querySelector(
              'button[type="submit"]',
            ) as HTMLButtonElement
          ).disabled,
        ).toBe(false),
      { timeout: 10000 },
    );
    fireEvent.submit(container.querySelector("form")!);
    await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
    const instrumentJson = submit.mock.calls[0][0] as string;
    const expected = structuredClone(initial);
    set(expected, entry.edit.path, entry.edit.to);
    expect(module.codec.parse(instrumentJson)).toEqual(expected);
    const editedRequest = { ...request, instrumentJson };
    const snapshot = structuredClone(editedRequest);
    const native = direct(editedRequest);
    const actual = unwrap(await worker.proxy.price(editedRequest));
    expect(editedRequest).toEqual(snapshot);
    same(actual, native);
    reference(actual.value, entry.reference.edited);
    expect(new Big(actual.value.amount).eq(baseline.value.amount)).toBe(false);
    expect(actual.instrument_id).toBe(baseline.instrument_id);
    const exported = unwrap(await worker.proxy.exportResult(actual));
    expect(valuations.validateValuationResultJson(exported)).toBe(exported);
  },
  30000,
);

it.each([
  { type: "equity", model: "hull_white_1f" },
  { type: "bond", model: "black76" },
  { type: "snowball", model: "discounting" },
])(
  "preserves the explicit unsupported $type / $model error",
  async ({ type, model }) => {
    const request = {
      ...requestFor(manifest.cases.find((c) => c.type === type)!),
      model,
    };
    let expected;
    try {
      direct(request);
    } catch (error) {
      expected = errorValue(error);
    }
    expect(expected).toBeDefined();
    expect(await worker.proxy.price(request)).toEqual({
      ok: false,
      error: expected,
    });
  },
);
