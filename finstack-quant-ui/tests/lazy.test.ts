import { expect, it, vi } from "vitest";
import { z } from "zod";

vi.mock("zod", async () => {
  const actual = await vi.importActual<typeof import("zod")>("zod");
  return {
    ...actual,
    z: { ...actual.z, fromJSONSchema: vi.fn(actual.z.fromJSONSchema) },
  };
});

it("converts no schema on catalogue import and converts a selected instrument once", async () => {
  const { instruments } = await import("../src/generated/instruments");
  expect(z.fromJSONSchema).not.toHaveBeenCalled();
  const entry = instruments.find((item) => item.type === "bond")!;
  const first = await entry.loader();
  expect(z.fromJSONSchema).toHaveBeenCalledTimes(1);
  const second = await entry.loader();
  expect(z.fromJSONSchema).toHaveBeenCalledTimes(1);
  expect(first.validator).toBe(second.validator);
});
