// @vitest-environment jsdom
import { createRequire } from "node:module";
import { afterEach, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import { instruments } from "../../../src/generated/instruments";
import report from "./encoding-report.json";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import type { InstrumentModule } from "@/components/finstack/core/components/schema-form/schema";
import {
  editValue,
  structuralValidator,
  workingValue,
} from "@/components/finstack/core/components/schema-form/schema-walk";
const native = createRequire(import.meta.url)(
  "../../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
) as { validateInstrumentJson(json: string): string };
const validate = async (json: string) => native.validateInstrumentJson(json);
afterEach(cleanup);
function identifierPath(value: unknown): string[] {
  const queue = [{ value, path: [] as string[] }];
  while (queue.length) {
    const { value, path } = queue.shift()!;
    if (!value || typeof value !== "object" || Array.isArray(value)) continue;
    if ("id" in value && typeof value.id === "string") return [...path, "id"];
    for (const [key, child] of Object.entries(value))
      queue.push({ value: child, path: [...path, key] });
  }
  throw new Error("Fixture has no editable identifier");
}

it("covers the exact canonical instrument inventory and its source fixtures", () => {
  expect(instruments.map((i) => i.type).sort()).toEqual(
    report.instruments.map((i) => i.type).sort(),
  );
  expect(report.instrumentCount).toBe(78);
  expect(report.instruments.every((i) => i.fixtures.length > 0)).toBe(true);
});
it.each(instruments)(
  "$type renders and edits an actual control through native acceptance",
  async (entry) => {
    const module = (await entry.loader()) as InstrumentModule;
    const location = { schema: module.schema, pointer: "#" };
    const initial = module.codec.parse(JSON.stringify(module.example));
    const working = editValue(module.schema, location, initial);
    expect(workingValue(module.schema, location, working)).toEqual(initial);
    const baseline = native.validateInstrumentJson(
      module.codec.stringify(initial),
    );
    const submit = vi.fn();
    const { container } = render(
      <SchemaForm
        module={module}
        validate={validate}
        onSubmit={submit}
        layout="full"
      />,
    );
    expect(container.textContent).not.toContain(
      "unsupported generated schema construct",
    );
    const idPath = identifierPath(initial);
    const control = container.querySelector(
      `[data-field-path="${idPath.join(".")}"] input`,
    ) as HTMLInputElement;
    expect(control, entry.type).not.toBeNull();
    const edited = `${control.value}-edited`;
    fireEvent.change(control, { target: { value: edited } });
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
    const output = submit.mock.calls[0][0] as string;
    expect(output).not.toBe(baseline);
    expect(native.validateInstrumentJson(output)).toBe(output);
    const parsed = module.codec.parse(output) as Record<string, unknown>;
    const expected = module.codec.parse(baseline) as Record<string, unknown>;
    let parent = expected;
    for (const key of idPath.slice(0, -1))
      parent = parent[key] as Record<string, unknown>;
    parent[idPath.at(-1)!] = edited;
    expect(parsed).toEqual(expected);
    // Structural validation still rejects an unrelated broken envelope after an edit.
    const invalid = structuredClone(working) as Record<string, unknown>;
    invalid.schema_version = "not-an-integer";
    expect(structuralValidator(module).safeParse(invalid).success).toBe(false);
  },
  30000,
);
