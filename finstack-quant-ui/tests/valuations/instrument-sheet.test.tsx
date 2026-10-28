// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createRequire } from "node:module";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import {
  INSTRUMENT_SHEET,
  instrumentDisclosure,
  termDisclosure,
} from "@/components/finstack/valuations/components/instrument-form/instrument-sheet";
import * as bond from "../../src/generated/instrument/bond";
import * as equityOption from "../../src/generated/instrument/equity_option";
import { createWireCodec } from "../../src/codec.mjs";
afterEach(cleanup);
const native = createRequire(import.meta.url)(
  "../../../finstack-quant-wasm/pkg-node/finstack_quant_wasm.js",
) as { validateInstrumentJson(json: string): string };
const validate = async (json: string) => native.validateInstrumentJson(json);
function summaries(name: RegExp) {
  return [...document.querySelectorAll("summary")].filter((node) =>
    name.test(node.textContent ?? ""),
  );
}
it("keeps common bond terms on the sheet and conventions in collapsed disclosures", () => {
  expect(termDisclosure("day_count", { atSpec: true, siblingKeys: [] })).toBe(
    "Schedule conventions",
  );
  expect(
    termDisclosure("discount_curve_id", { atSpec: true, siblingKeys: [] }),
  ).toBe("Market links");
  expect(
    termDisclosure("spot_id", {
      atSpec: true,
      siblingKeys: ["underlying_ticker"],
    }),
  ).toBe("Market links");
  expect(
    termDisclosure("spot_id", { atSpec: true, siblingKeys: [] }),
  ).toBeNull();
  expect(
    termDisclosure("pool", {
      atSpec: true,
      instrumentType: "structured_credit",
      siblingKeys: [],
    }),
  ).toBe("Deal structure");
  expect(
    termDisclosure("fees", {
      atSpec: true,
      instrumentType: "revolving_credit",
      siblingKeys: [],
    }),
  ).toBe("Facility terms");
  expect(termDisclosure("rate", { atSpec: true, siblingKeys: [] })).toBeNull();
  expect(
    termDisclosure("id", {
      atSpec: true,
      instrumentType: "cds_option",
      siblingKeys: [],
    }),
  ).toBeNull();
});

it("edits generated bond controls, shows decimal errors and emits native canonical JSON without replacing edits", async () => {
  const submit = vi.fn();
  render(
    <SchemaForm
      module={bond}
      sheet={INSTRUMENT_SHEET}
      disclosure={instrumentDisclosure("bond")}
      validate={validate}
      onSubmit={submit}
    />,
  );
  const addCredit = screen.getByRole("button", { name: "Add credit curve id" });
  expect(addCredit.closest("details")?.open).toBe(false);
  expect(
    addCredit.closest("details")?.querySelector("summary")?.textContent,
  ).toMatch(/^Market links/);
  expect(screen.getByRole("textbox", { name: "Rate" })).toBeTruthy();
  expect(summaries(/^More /)).toEqual([]);
  const settlement = screen.getByRole("textbox", {
    name: "Settlement days",
  }) as HTMLInputElement;
  expect(settlement.closest("details")?.open).toBe(false);
  expect(settlement.value).toBe("1");
  const amount = within(
    screen.getByRole("group", { name: "Notional" }),
  ).getByRole("textbox", { name: "Amount" });
  fireEvent.focus(amount);
  fireEvent.change(amount, { target: { value: "abc" } });
  await waitFor(() => expect(amount.getAttribute("aria-invalid")).toBe("true"));
  expect(
    screen
      .getAllByRole("alert")
      .map((node) => node.textContent)
      .join(" "),
  ).toMatch(/pattern|decimal/i);
  fireEvent.change(amount, { target: { value: "1234567.1234567890" } });
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Apply instrument",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  fireEvent.submit(amount.closest("form")!);
  await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  const canonical = submit.mock.calls[0][0];
  expect(native.validateInstrumentJson(canonical)).toBe(canonical);
  expect(JSON.parse(canonical).instrument.spec.notional.amount).toBe(
    "1234567.1234567890",
  );
  expect(JSON.parse(canonical).instrument.spec.settlement_days).toBe(1);
  expect(
    JSON.parse(canonical).instrument.spec.cashflow_spec.fixed.day_count,
  ).toBe("act_act_isma");
  expect((amount as HTMLInputElement).value).toBe("1234567.1234567890");
});

it("keeps a defaulted exercise style on the equity option sheet", () => {
  render(
    <SchemaForm
      module={equityOption}
      sheet={INSTRUMENT_SHEET}
      disclosure={instrumentDisclosure("equity_option")}
      validate={validate}
      onSubmit={() => {}}
    />,
  );
  const style = screen.getByRole("combobox", { name: "Exercise style" });
  expect(style.closest("details")).toBeNull();
});

it("opens a collapsed disclosure when its field is invalid", async () => {
  render(
    <SchemaForm
      module={bond}
      sheet={INSTRUMENT_SHEET}
      disclosure={instrumentDisclosure("bond")}
      validate={validate}
      onSubmit={() => {}}
    />,
  );
  const settlement = screen.getByRole("textbox", {
    name: "Settlement days",
  }) as HTMLInputElement;
  const details = settlement.closest("details");
  expect(details?.open).toBe(false);
  fireEvent.change(settlement, { target: { value: "nope" } });
  await waitFor(() =>
    expect(settlement.getAttribute("aria-invalid")).toBe("true"),
  );
  expect(details?.open).toBe(true);
  expect(document.activeElement).toBe(settlement);
  details!.open = false;
  details!.dispatchEvent(new Event("toggle", { bubbles: true }));
  expect(details?.open).toBe(true);
  expect(settlement.value).toBe("nope");
});

it("switches canonical external coupon branches, and exposes defaulted fields only on request", async () => {
  const user = userEvent.setup();
  render(
    <SchemaForm
      module={bond}
      sheet={INSTRUMENT_SHEET}
      disclosure={instrumentDisclosure("bond")}
      validate={validate}
      onSubmit={() => {}}
    />,
  );
  const dayCount = screen.getByRole("combobox", { name: "Day count" });
  expect(dayCount.closest("details")?.open).toBe(false);
  for (const summary of summaries(/^Schedule conventions/))
    await user.click(summary);
  expect(
    screen.getByRole("textbox", { name: "Settlement days" }).closest("details")
      ?.open,
  ).toBe(true);
  expect(dayCount.closest("details")?.open).toBe(true);
  const types = screen.getByRole("radiogroup", { name: "Cashflow spec type" });
  await user.click(within(types).getByRole("radio", { name: "Floating" }));
  expect(
    within(types)
      .getByRole("radio", { name: "Floating" })
      .getAttribute("aria-checked"),
  ).toBe("true");
  expect(screen.queryByRole("group", { name: "Fixed" })).toBeNull();
  await user.click(within(types).getByRole("radio", { name: "Fixed" }));
  expect(
    within(types)
      .getByRole("radio", { name: "Fixed" })
      .getAttribute("aria-checked"),
  ).toBe("true");
});

it("supports nullable references without inventing IDs and ignores stale native validation errors", async () => {
  const pending: {
    json: string;
    resolve(value: string): void;
    reject(error: Error): void;
  }[] = [];
  render(
    <SchemaForm
      module={bond}
      sheet={INSTRUMENT_SHEET}
      disclosure={instrumentDisclosure("bond")}
      validate={(json) =>
        new Promise((resolve, reject) =>
          pending.push({ json, resolve, reject }),
        )
      }
      onSubmit={() => {}}
    />,
  );
  fireEvent.click(summaries(/^Market links/)[0]!);
  fireEvent.click(screen.getByRole("button", { name: "Add credit curve id" }));
  const credit = screen.getByRole("textbox", { name: "Credit curve id" });
  expect((credit as HTMLInputElement).value).toBe("");
  fireEvent.change(credit, { target: { value: "OLD" } });
  await waitFor(() => expect(pending.length).toBeGreaterThan(0));
  fireEvent.change(credit, { target: { value: "NEW" } });
  await waitFor(() => expect(pending.length).toBe(2));
  pending[1].resolve(native.validateInstrumentJson(pending[1].json));
  pending[0].reject(new Error("Obsolete native error"));
  await waitFor(() =>
    expect(
      (
        screen.getByRole("button", {
          name: "Apply instrument",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false),
  );
  expect(screen.queryByText("Obsolete native error")).toBeNull();
  fireEvent.click(
    screen.getByRole("button", { name: "Credit curve id options" }),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: "Clear credit curve id" }),
  );
  expect(screen.queryByRole("textbox", { name: "Credit curve id" })).toBeNull();
});

it("normalizes each immutable edit once across errors, native validation and submission", async () => {
  const codec = createWireCodec(bond.schema);
  const normalize = vi.spyOn(codec.validator, "parse");
  const stringify = vi.spyOn(codec, "stringify");
  const module = { ...bond, codec };
  const nativeValidate = vi.fn(validate);
  const submit = vi.fn();
  render(
    <SchemaForm
      module={module}
      sheet={INSTRUMENT_SHEET}
      disclosure={instrumentDisclosure("bond")}
      validate={nativeValidate}
      onSubmit={submit}
    />,
  );
  await waitFor(() => expect(nativeValidate).toHaveBeenCalledTimes(1));
  expect(normalize).toHaveBeenCalledTimes(1);
  const input = screen.getByRole("textbox", {
    name: "Settlement days",
    hidden: true,
  });
  fireEvent.click(input.closest("details")!.querySelector("summary")!);
  fireEvent.change(input, { target: { value: "2" } });
  await waitFor(() => expect(nativeValidate).toHaveBeenCalledTimes(2));
  expect(normalize).toHaveBeenCalledTimes(2);
  fireEvent.submit(input.closest("form")!);
  await waitFor(() => expect(submit).toHaveBeenCalledTimes(1));
  expect(normalize).toHaveBeenCalledTimes(2);
  expect(nativeValidate).toHaveBeenCalledTimes(2);
  expect(stringify).not.toHaveBeenCalled();
});
