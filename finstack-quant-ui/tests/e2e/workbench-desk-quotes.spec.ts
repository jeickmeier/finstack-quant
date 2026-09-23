import { test, expect } from "playwright/test";

const cases = [
  {
    type: "bond",
    label: "Bond clean price",
    value: "98.5",
    path: [
      "instrument_pricing_overrides",
      "market_quotes",
      "quoted_clean_price",
    ],
  },
  {
    type: "interest_rate_swap",
    label: "Fixed coupon",
    value: "0.045",
    path: ["fixed", "rate"],
  },
  {
    type: "credit_default_swap",
    label: "Running spread",
    value: "110",
    path: ["premium", "spread_bp"],
  },
  {
    type: "equity_option",
    label: "Implied volatility",
    value: "0.25",
    path: [
      "instrument_pricing_overrides",
      "market_quotes",
      "implied_volatility",
    ],
  },
  {
    type: "structured_credit",
    label: "Tranche coupon",
    value: "0.07",
    path: ["tranches", "tranches", 0, "coupon", "fixed", "rate"],
  },
] as const;

for (const { type, label, value, path } of cases)
  test(`${type} desk quote updates the native priced instrument`, async ({
    page,
  }) => {
    const errors: string[] = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(
      `/registry-gallery/?item=pricing-workbench&focus=1&theme=light&density=compact&instrument=${type}`,
    );
    const workbench = page.getByRole("region", {
      name: "Pricing workbench",
      exact: true,
    });
    const form = workbench.getByRole("region", {
      name: "Instrument form",
      exact: true,
    });
    const results = workbench.getByRole("region", {
      name: "Results",
      exact: true,
    });
    const presentValue = results.locator(".finstack-valuation__value");
    await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
      "Priced",
    );
    const original = await presentValue.innerText();
    const input = form.getByRole("textbox", { name: label, exact: true });
    await expect(input).toBeInViewport();
    await input.fill(value);
    await expect(
      workbench.getByRole("button", { name: "Price", exact: true }),
    ).toBeEnabled();
    await workbench.getByRole("button", { name: "Price", exact: true }).click();
    await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
      "Priced",
    );
    await expect(presentValue).not.toHaveText(original);

    await results.getByRole("tab", { name: "Request" }).click();
    const requestText = await results
      .getByRole("region", { name: "Last priced request JSON" })
      .locator("pre")
      .textContent();
    const request = JSON.parse(requestText ?? "null") as {
      instrumentJson: string;
    };
    const instrument = JSON.parse(request.instrumentJson) as {
      instrument: { spec: Record<string, unknown> };
    };
    let field: unknown = instrument.instrument.spec;
    for (const key of path)
      field = (field as Record<string | number, unknown>)[key];
    expect(String(field)).toBe(value);
    expect(errors).toEqual([]);
  });
