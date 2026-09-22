import { test, expect, type Locator } from "playwright/test";

async function expectOpaque(control: Locator) {
  await expect(control).toBeVisible();
  await expect(control).toHaveCSS("background-color", /^rgb\(/);
}

for (const theme of ["light", "dark"])
  for (const density of ["compact", "comfortable"])
    test(`workbench controls ${theme} ${density}`, async ({ page }) => {
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      await page.goto(
        `/registry-gallery/?item=pricing-workbench&theme=${theme}&density=${density}`,
      );
      const workbench = page.locator('[data-item="pricing-workbench"]');
      await expect(
        workbench.getByText("Priced", { exact: true }),
      ).toBeVisible();

      // A successful click is insufficient: missing Tailwind sources can leave
      // both the popup and the selected radio visually transparent.
      const model = workbench.getByRole("combobox", {
        name: "Pricing model",
        exact: true,
      });
      await model.click();
      await expectOpaque(page.locator('[data-slot="select-content"]'));
      await page
        .getByRole("option", { name: "discounting", exact: true })
        .click();
      await expect(model).toContainText("discounting");

      const currency = workbench.getByRole("combobox", {
        name: "Currency",
        exact: true,
      });
      await currency.click();
      await page.getByRole("option", { name: "EUR", exact: true }).click();
      await expect(currency).toContainText("EUR");
      await currency.click();
      await page.getByRole("option", { name: "USD", exact: true }).click();
      await expect(currency).toContainText("USD");

      const group = workbench.getByRole("radiogroup", {
        name: "Cashflow spec type",
        exact: true,
      });
      const fixed = group.getByRole("radio", { name: "Fixed", exact: true });
      const floating = group.getByRole("radio", {
        name: "Floating",
        exact: true,
      });
      await expectOpaque(fixed);
      await group
        .locator("label")
        .filter({ hasText: /^Floating$/ })
        .click();
      await expect(floating).toHaveAttribute("aria-checked", "true");
      await expect(fixed).toHaveAttribute("aria-checked", "false");
      await expectOpaque(floating);
      await workbench
        .locator("summary")
        .filter({ hasText: "Schedule conventions (6)" })
        .click();
      await expect(
        workbench.getByRole("combobox", { name: "Day count", exact: true }),
      ).toBeVisible();
      await fixed.click();
      await expect(fixed).toHaveAttribute("aria-checked", "true");
      await fixed.focus();
      await page.keyboard.press("ArrowDown");
      await expect(floating).toBeFocused();
      await expect(floating).toHaveAttribute("aria-checked", "true");
      await expectOpaque(floating);
      const indicator = floating.locator(
        '[data-slot="radio-group-indicator"] > span',
      );
      await expectOpaque(indicator);
      expect(
        await indicator.evaluate(
          (node) => getComputedStyle(node).backgroundColor,
        ),
      ).not.toBe(
        await floating.evaluate(
          (node) => getComputedStyle(node).backgroundColor,
        ),
      );

      await workbench
        .getByRole("button", { name: "Issue date calendar", exact: true })
        .click();
      const calendar = page.locator('[data-slot="popover-content"]');
      await expectOpaque(calendar);
      await calendar.getByRole("button", { name: /January 2nd, 2025/ }).click();
      await expect(
        workbench.getByRole("textbox", { name: "Issue date", exact: true }),
      ).toHaveValue("2025-01-02");
      await expect(calendar).toBeHidden();
      expect(errors).toEqual([]);
    });
