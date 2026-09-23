import { test, expect } from "playwright/test";

const examples = [
  { type: "bond", id: "US912828XG33", model: "discounting", featured: 3 },
  {
    type: "fx_swap",
    id: "FXSWAP-EURUSD-6M",
    model: "discounting",
    featured: 2,
  },
  {
    type: "xccy_swap",
    id: "XCCY-USDEUR-5Y",
    model: "discounting",
    featured: 2,
  },
  {
    type: "interest_rate_swap",
    id: "IRS-5Y-USD-STD",
    model: "discounting",
    featured: 2,
  },
  { type: "equity_option", id: "SPX-CALL-4500", model: "black76", featured: 3 },
  {
    type: "commodity_option",
    id: "WTI-OPT-2025M06",
    model: "monte_carlo_schwartz_smith",
    featured: 1,
  },
  {
    type: "composite",
    id: "COMPOSITE-EXAMPLE",
    model: "discounting",
    featured: 0,
  },
  {
    type: "credit_default_swap",
    id: "CDS-CORP-5Y",
    model: "hazard_rate",
    featured: 3,
  },
  { type: "fx_option", id: "FXOPT-EURUSD-CALL", model: "black76", featured: 3 },
  {
    type: "structured_credit",
    id: "CLO-EXAMPLE",
    model: "structured_credit_stochastic",
    featured: 2,
  },
] as const;

for (const { type, id, model, featured } of examples)
  test(`gallery ${type} deep link prices its complete request`, async ({
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
    await expect(workbench.locator(".finstack-workbench__id")).toHaveText(id);
    await expect(
      workbench.getByRole("combobox", { name: "Pricing model", exact: true }),
    ).toContainText(model);
    await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
      "Priced",
    );
    await expect(workbench).not.toHaveAttribute("data-stale");
    await expect(
      workbench.getByRole("region", { name: "Results", exact: true }),
    ).toContainText("Present value");
    const results = workbench.getByRole("region", {
      name: "Results",
      exact: true,
    });
    await expect(
      results.getByRole("tab", {
        name: ["bond", "equity_option"].includes(type)
          ? /Buckets & surfaces/
          : ["fx_swap", "xccy_swap"].includes(type)
            ? /Cashflows$/
            : "Measures",
      }),
    ).toHaveAttribute("aria-selected", "true");
    await expect(
      workbench.locator('[aria-label="Key measures"] > div'),
    ).toHaveCount(featured);
    if (type === "bond") {
      await expect(
        results.getByRole("heading", { name: "Bucketed DV01" }),
      ).toBeVisible();
      await expect(
        results.getByRole("table", {
          name: "Returned bucket values for USD-OIS",
        }),
      ).toContainText("30y");
    }
    if (type === "equity_option") {
      await expect(
        results.getByRole("heading", { name: "Bucketed Vega" }),
      ).toBeVisible();
      await expect(
        results.getByRole("table", {
          name: "Returned two-coordinate measure matrix",
        }),
      ).toContainText("EQUITY-VOL::0.01y");
    }
    for (const viewport of [
      { width: 1440, height: 900 },
      { width: 1405, height: 768 },
      { width: 1280, height: 800 },
    ]) {
      await page.setViewportSize(viewport);
      const instrumentForm = workbench.getByRole("region", {
        name: "Instrument form",
        exact: true,
      });
      await expect(
        instrumentForm.getByRole("button", {
          name: "Load example",
          exact: true,
        }),
      ).toHaveCount(0);
      await expect(
        instrumentForm.getByPlaceholder("Search instruments…", { exact: true }),
      ).toBeInViewport();
      await expect(
        workbench.getByRole("combobox", {
          name: "Pricing model",
          exact: true,
        }),
      ).toBeInViewport();
      const definingFields =
        type === "bond"
          ? ["Maturity", "Rate"]
          : type === "equity_option"
            ? ["Expiry", "Strike"]
            : [];
      for (const label of definingFields)
        await expect(
          instrumentForm.getByLabel(label, { exact: true }),
        ).toBeInViewport();
      if (type === "structured_credit")
        await expect(
          instrumentForm.getByLabel("Maturity", { exact: true }).nth(1),
        ).toBeInViewport();
      await expect(
        workbench.getByText("Present value", { exact: true }),
      ).toBeInViewport();
      if (featured > 0)
        await expect(
          workbench.locator('[aria-label="Key measures"]'),
        ).toBeInViewport({ ratio: 1 });
      expect(
        await page.evaluate(
          () =>
            document.documentElement.scrollWidth <= window.innerWidth &&
            document.body.scrollWidth <= window.innerWidth,
        ),
        `${type} at ${viewport.width}×${viewport.height} must not scroll horizontally`,
      ).toBe(true);
      expect(
        await page
          .locator(".finstack-workbench-container")
          .evaluate(
            (element) => element.scrollWidth <= element.clientWidth + 1,
          ),
        `${type} workbench at ${viewport.width}×${viewport.height} must contain its overflow`,
      ).toBe(true);
    }
    expect(errors).toEqual([]);
  });

test("switching instrument loads its model and market, and gallery links retain it", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=pricing-workbench&theme=light&density=compact",
  );
  const workbench = page.getByRole("region", {
    name: "Pricing workbench",
    exact: true,
  });
  await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
    "Priced",
  );
  await workbench
    .getByRole("combobox", { name: "Instrument type", exact: true })
    .click();
  await page.getByRole("option", { name: "equity_option" }).click();
  await expect(page).toHaveURL(/instrument=equity_option/);
  await expect(workbench.locator(".finstack-workbench__id")).toHaveText(
    "SPX-CALL-4500",
  );
  await expect(
    workbench.getByRole("combobox", { name: "Pricing model", exact: true }),
  ).toContainText("black76");
  await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
    "Priced",
  );
  await expect(workbench).not.toHaveAttribute("data-stale");
  await expect(
    workbench.getByRole("tab", { name: /Buckets & surfaces/ }),
  ).toHaveAttribute("aria-selected", "true");
  await page.getByRole("link", { name: "Dark", exact: true }).click();
  await expect(page).toHaveURL(/instrument=equity_option/);
  await expect(page).toHaveURL(/theme=dark/);
  await expect(workbench.locator(".finstack-workbench__id")).toHaveText(
    "SPX-CALL-4500",
  );
  await page.getByRole("link", { name: "Comfortable", exact: true }).click();
  await expect(page).toHaveURL(/instrument=equity_option/);
  await expect(page).toHaveURL(/density=comfortable/);
  await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
    "Priced",
  );
  await workbench
    .getByRole("combobox", { name: "Instrument type", exact: true })
    .click();
  await page.getByRole("option", { name: "fx_swap" }).click();
  await expect(page).toHaveURL(/instrument=fx_swap/);
  await expect(workbench.locator(".finstack-workbench__state")).toHaveText(
    "Priced",
  );
  await expect(
    workbench.getByRole("tab", { name: /Cashflows$/ }),
  ).toHaveAttribute("aria-selected", "true");
  expect(errors).toEqual([]);
});

test("structured-credit scenario prices use the completed native request", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=pricing-workbench&theme=light&density=compact&focus=1&instrument=structured_credit",
  );
  const results = page.getByRole("region", { name: "Results", exact: true });
  await expect(page.locator(".finstack-workbench__state")).toHaveText("Priced");
  await results.getByRole("tab", { name: "Scenarios" }).click();
  await results.getByText("Scenario prices", { exact: true }).click();
  const scenario = results.getByRole("region", {
    name: "CLONOTES-A scenario prices",
  });
  await expect(scenario.getByRole("grid")).toContainText("126.336");
  await expect(scenario).toContainText("structuredCreditTrancheScenarioTable");
  expect(errors).toEqual([]);
});
