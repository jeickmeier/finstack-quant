// @vitest-environment jsdom
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { InstrumentForm } from "../../registry/components/instrument-form/instrument-form";
import { InstrumentSelector } from "../../registry/components/instrument-form/instrument-selector";
import { instruments } from "../../src/generated/instruments";
import inventory from "../../src/generated/catalogue.json";
import pricing from "./pricing-cases.json";
import encodings from "./encoding-report.json";
const counts = vi.hoisted(() => new Map<string, number>());
vi.mock("../../src/codec.mjs", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../src/codec.mjs")>();
  return {
    ...actual,
    createWireCodec: (schema: Record<string, unknown>) => {
      const name = String(schema.title);
      counts.set(name, (counts.get(name) ?? 0) + 1);
      return actual.createWireCodec(schema);
    },
  };
});
afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});
beforeEach(() => localStorage.clear());
const validate = async (json: string) => json;
function Form() {
  const [type, setType] = useState("bond");
  return (
    <InstrumentForm
      type={type}
      onTypeChange={setType}
      validate={validate}
      onSubmit={() => {}}
    />
  );
}
async function select(type: string) {
  const input = screen.getByRole("combobox", { name: "Instrument type" });
  await userEvent.clear(input);
  await userEvent.type(input, type);
  await userEvent.click(await screen.findByRole("option", { name: type }));
  await waitFor(() =>
    expect(
      (
        document.querySelector(
          '[data-field-path="instrument.spec.id"] input',
        ) as HTMLInputElement
      )?.value,
    ).toBe(inventory.find((entry) => entry.type === type)!.exampleId),
  );
}
it("reconciles all instrument tags with positive encoding and pricing coverage", () => {
  const types = instruments.map((entry) => entry.type).sort();
  expect(types).toHaveLength(78);
  expect(inventory.map((entry) => entry.type).sort()).toEqual(types);
  expect(pricing.cases.map((entry) => entry.type).sort()).toEqual(types);
  expect(encodings.instruments.map((entry) => entry.type).sort()).toEqual(
    types,
  );
  expect([...new Set(inventory.map((entry) => entry.group))].sort()).toEqual([
    "commodity",
    "composite",
    "credit",
    "equity",
    "exotics",
    "fixed_income",
    "fx",
    "rates",
    "real_assets",
  ]);
});
it("searches grouped choices, converts only selected modules once and reuses them on repeat selection", async () => {
  render(<Form />);
  await screen.findByRole("textbox", { name: "Amount" });
  await select("equity");
  await select("commodity_option");
  await select("bond");
  for (const type of ["bond", "equity", "commodity_option"])
    expect(counts.get(type)).toBe(1);
  expect(
    [...counts.keys()]
      .filter((type) => instruments.some((entry) => entry.type === type))
      .sort(),
  ).toEqual(["bond", "commodity_option", "equity"]);
  expect(
    JSON.parse(localStorage.getItem("finstack.recent-instruments")!),
  ).toEqual(["bond", "commodity_option", "equity"]);
  expect(
    screen.getByRole("navigation", { name: "Recently used instruments" }),
  ).toBeTruthy();
});
it("accepts only known unique recent identifiers and remains usable without storage", async () => {
  localStorage.setItem(
    "finstack.recent-instruments",
    JSON.stringify(["equity", "equity", "__proto__", {}, null, "bond"]),
  );
  const changed = vi.fn();
  render(<InstrumentSelector value="bond" onValueChange={changed} />);
  const recent = await screen.findByRole("navigation", {
    name: "Recently used instruments",
  });
  expect(
    [...recent.querySelectorAll("button")].map((node) => node.textContent),
  ).toEqual(["equity", "bond"]);
  cleanup();
  vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
    throw new Error("Storage unavailable");
  });
  vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
    throw new Error("Quota exceeded");
  });
  render(<InstrumentSelector value="bond" onValueChange={changed} />);
  await userEvent.click(
    screen.getByRole("combobox", { name: "Instrument type" }),
  );
  await userEvent.click(await screen.findByRole("option", { name: "equity" }));
  expect(changed).toHaveBeenLastCalledWith("equity");
});
it("rejects unknown types without loading a fallback schema", () => {
  render(
    <InstrumentForm
      type="future_type"
      onTypeChange={() => {}}
      validate={validate}
      onSubmit={() => {}}
    />,
  );
  expect(screen.getByRole("alert").textContent).toBe(
    "Unknown instrument type: future_type",
  );
  expect(screen.queryByRole("button", { name: "Load example" })).toBeNull();
});
