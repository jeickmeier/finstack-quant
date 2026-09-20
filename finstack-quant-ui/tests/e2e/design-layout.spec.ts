import { test, expect } from "playwright/test";
import { converter, wcagContrast } from "culori";

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
    const model = await workbench
      .getByRole("combobox", { name: "Pricing model", exact: true })
      .boundingBox();
    const price = await workbench
      .getByRole("button", { name: "Price", exact: true })
      .boundingBox();
    expect(model).not.toBeNull();
    expect(price).not.toBeNull();
    const overlapWidth = Math.max(
      0,
      Math.min(model!.x + model!.width, price!.x + price!.width) -
        Math.max(model!.x, price!.x),
    );
    const overlapHeight = Math.max(
      0,
      Math.min(model!.y + model!.height, price!.y + price!.height) -
        Math.max(model!.y, price!.y),
    );
    expect(
      overlapWidth * overlapHeight,
      "The model selector must not overlap Price",
    ).toBeLessThanOrEqual(0.5);

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
        .getByRole("button", { name: "Settings", exact: true })
        .click();
      await page
        .getByRole("button", { name: "Toggle theme", exact: true })
        .click();
    }
    await expect(formatted).toHaveAttribute("aria-pressed", "true");
    await expect(async () => {
      const colors = await formatted.evaluate((node) => {
        const foreground = getComputedStyle(node).color;
        let painted: Element | null = node;
        while (painted) {
          const background = getComputedStyle(painted).backgroundColor;
          if (background !== "rgba(0, 0, 0, 0)" && background !== "transparent")
            return { foreground, background };
          painted = painted.parentElement;
        }
        throw new Error("No painted background for stock control");
      });
      expect(
        wcagContrast(colors.foreground, colors.background),
        `${theme}: ${JSON.stringify(colors)}`,
      ).toBeGreaterThanOrEqual(4.5);
    }).toPass({ timeout: 2000 });
  }
});

for (const theme of ["light", "dark"])
  test(`stock inactive Tabs preserve text contrast in ${theme}`, async ({
    page,
  }) => {
    await page.goto(
      `/registry-gallery/?item=pricing-workbench&focus=1&theme=${theme}`,
    );
    await page.waitForLoadState("networkidle");
    const tab = page.getByRole("tab", { name: "2 Market", exact: true });
    await expect(tab).toBeVisible();
    await expect(tab).toHaveAttribute("aria-selected", "false");
    const colors = await tab.evaluate((node) => {
      const foreground = getComputedStyle(node).color;
      let painted: Element | null = node;
      while (painted) {
        const background = getComputedStyle(painted).backgroundColor;
        if (background !== "rgba(0, 0, 0, 0)" && background !== "transparent")
          return { foreground, background };
        painted = painted.parentElement;
      }
      throw new Error("No painted background for inactive tab");
    });
    const rgb = converter("rgb");
    const foreground = rgb(colors.foreground)!;
    const background = rgb(colors.background)!;
    const alpha = foreground.alpha ?? 1;
    const painted = {
      mode: "rgb" as const,
      r: foreground.r * alpha + background.r * (1 - alpha),
      g: foreground.g * alpha + background.g * (1 - alpha),
      b: foreground.b * alpha + background.b * (1 - alpha),
    };
    expect(
      wcagContrast(painted, background),
      JSON.stringify(colors),
    ).toBeGreaterThanOrEqual(4.5);
  });
