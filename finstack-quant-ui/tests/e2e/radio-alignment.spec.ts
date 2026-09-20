import { test, expect } from "playwright/test";

for (const width of [1440, 375])
  for (const theme of ["light", "dark"])
    test(`stock radio indicator centered at ${width}px ${theme}`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 900 });
      await page.goto(`/registry-gallery/?item=enum-field&theme=${theme}`);
      await page.waitForLoadState("networkidle");
      await page.evaluate(() => document.fonts.ready);
      const group = page.getByRole("radiogroup").first();
      const radios = group.getByRole("radio");
      await expect(radios.first()).toBeVisible();
      for (let index = 0; index < (await radios.count()); index++) {
        const radio = radios.nth(index);
        if ((await radio.getAttribute("aria-disabled")) === "true") continue;
        await radio.click();
        await expect(radio).toHaveAttribute("aria-checked", "true");
        const offset = await radio.evaluate((node) => {
          const indicator = node.querySelector(
            "[data-slot=radio-group-indicator] > span",
          );
          if (!indicator) throw new Error("Missing stock radio indicator");
          const outer = node.getBoundingClientRect();
          const inner = indicator.getBoundingClientRect();
          return {
            x: inner.x + inner.width / 2 - outer.x - outer.width / 2,
            y: inner.y + inner.height / 2 - outer.y - outer.height / 2,
          };
        });
        expect(Math.abs(offset.x)).toBeLessThanOrEqual(0.5);
        expect(Math.abs(offset.y)).toBeLessThanOrEqual(0.5);
      }
      await radios.first().focus();
      await page.keyboard.press("ArrowRight");
      await expect(radios.nth(1)).toBeFocused();
      await expect(radios.nth(1)).toHaveAttribute("aria-checked", "true");
    });
