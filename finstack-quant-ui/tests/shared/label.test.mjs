import { describe, expect, it } from "vitest";
import { labelFor } from "../../src/format/label.mjs";

describe("labelFor", () => {
  it.each([
    ["bucketed_dv01", "Bucketed DV01"],
    ["risky_pv01", "Risky PV01"],
    ["fx01", "FX01"],
    ["realized_var_method", "Realized var method"],
    ["contract-cds-option", "Contract CDS option"],
    ["ytm", "YTM"],
    ["duration_mod", "Duration mod"],
    ["tarn", "TARN"],
    ["ytw", "YTW"],
    ["implied_vol", "Implied VOL"],
    ["cmbs_dscr", "CMBS DSCR"],
    ["lp_irr", "LP IRR"],
    ["hvar", "HVAR"],
    ["cs_ref", "Cs ref"],
  ])("labels %s as %s", (name, label) => {
    expect(labelFor(name)).toBe(label);
  });
});
