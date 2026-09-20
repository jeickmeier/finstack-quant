import { test, expect } from "playwright/test";
import { wcagContrast } from "culori";

for (const variant of ["default", "equity-option", "structured-credit"])
  test(`workbench viewport composition ${variant}`, async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto(
      `/registry-gallery/?item=pricing-workbench&focus=1&variant=${variant}`,
    );
    await page.waitForLoadState("networkidle");
    const workbench = page.getByRole("region", {
      name: "Pricing workbench",
      exact: true,
    });
    await expect(workbench.getByText("Priced", { exact: true })).toBeVisible();
    await page.evaluate(() => document.fonts.ready);
    const inputs = workbench.getByRole("region", {
      name: "Inputs",
      exact: true,
    });
    const results = workbench.getByRole("region", {
      name: "Results",
      exact: true,
    });
    const bounds = await workbench.boundingBox();
    const upper = await inputs.boundingBox();
    const lower = await results.boundingBox();
    expect(bounds).not.toBeNull();
    expect(upper).not.toBeNull();
    expect(lower).not.toBeNull();
    expect(bounds!.y + bounds!.height).toBeLessThanOrEqual(901);
    expect(bounds!.y + bounds!.height).toBeGreaterThanOrEqual(899);
    expect(upper!.height).toBeGreaterThan(250);
    expect(upper!.height).toBeLessThan(500);
    expect(lower!.height).toBeGreaterThan(250);
    expect(lower!.y).toBeGreaterThan(upper!.y);
    expect(lower!.y + lower!.height).toBeLessThanOrEqual(901);
    await expect(
      results.getByText("Present value", { exact: true }),
    ).toBeVisible();
    await expect(results.getByRole("tablist")).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
  });

test("workbench stacks on mobile without horizontal page scrolling", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/registry-gallery/?item=pricing-workbench&focus=1");
  await page.waitForLoadState("networkidle");
  const workbench = page.getByRole("region", {
    name: "Pricing workbench",
    exact: true,
  });
  await expect(workbench.getByText("Priced", { exact: true })).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await expect(
    workbench.getByRole("region", { name: "Results", exact: true }),
  ).toBeVisible();
});

test("docs host preserves registry selected-control contrast", async ({
  page,
}) => {
  await page.goto("/docs/registry/workbench/");
  await page.waitForLoadState("networkidle");
  const workbench = page.getByRole("region", {
    name: "Pricing workbench",
    exact: true,
  });
  await expect(
    workbench.getByRole("button", { name: "Print report", exact: true }),
  ).toBeEnabled();
  const formatted = workbench.getByRole("button", {
    name: "Formatted",
    exact: true,
  });
  for (const theme of ["light", "dark"]) {
    if (theme === "dark") {
      await workbench
        .locator("summary")
        .filter({ hasText: /^Settings$/ })
        .click();
      await workbench
        .getByRole("button", { name: "Toggle theme", exact: true })
        .click();
    }
    await expect(formatted).toHaveAttribute("aria-pressed", "true");
    const colors = await formatted.evaluate((node) => {
      const style = getComputedStyle(node);
      return { foreground: style.color, background: style.backgroundColor };
    });
    expect(
      wcagContrast(colors.foreground, colors.background),
      `${theme}: ${JSON.stringify(colors)}`,
    ).toBeGreaterThanOrEqual(4.5);
  }
});
