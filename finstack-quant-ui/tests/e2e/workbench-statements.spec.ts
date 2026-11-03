import { test, expect } from "playwright/test";

test("statements-workbench exports the result as a table and a CSV of those cells, in a real worker", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=statements-workbench&theme=light&density=compact&focus=1",
  );
  await page.getByText("Export table and CSV", { exact: true }).click();
  const region = page.getByRole("region", { name: "Statement export" });
  // A real worker delivers a missing value as NaN; it never shows as NaN.
  await expect(region.getByText("Undefined").first()).toBeVisible();
  await expect(region.getByText("NaN", { exact: true })).toHaveCount(0);
  const saved = page.waitForEvent("download");
  await region.getByRole("button", { name: "Download CSV" }).click();
  const file = await saved;
  expect(file.suggestedFilename()).toBe("statement-wide.csv");
  const stream = await file.createReadStream();
  const chunks: Buffer[] = [];
  for await (const chunk of stream) chunks.push(chunk as Buffer);
  const csv = Buffer.concat(chunks).toString("utf8");
  const [header, first] = csv.split("\r\n");
  const names = header!.split(",");
  const cells = first!.split(",");
  expect(names[0]).toBe("period_id");
  expect(cells[names.indexOf("revenue_growth_qoq")]).toBe("");
  expect(csv).not.toContain("NaN");
  await region.getByRole("radio", { name: "Long" }).click();
  await expect(
    region.getByText("Statement result, long layout"),
  ).toBeAttached();
  expect(errors).toEqual([]);
});

test("statements-workbench WACC calculator sets the DCF field on the click, in a real worker", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=statements-workbench&theme=light&density=compact&focus=1",
  );
  await page.getByRole("tab", { name: "Valuation" }).click();
  const form = page.getByRole("region", { name: "DCF assumptions" });
  const waccField = form.getByRole("textbox", { name: "WACC", exact: true });
  await expect(waccField).toHaveValue("");
  await page.getByText("WACC calculator", { exact: true }).click();
  const calculator = page.getByRole("region", { name: "WACC calculator" });
  const enter = (name: string, value: string) =>
    calculator.getByRole("textbox", { name, exact: true }).fill(value);
  await enter("Equity weight", "0.6");
  await enter("Cost of equity", "0.115");
  await enter("Debt weight", "0.5");
  await enter("Cost of debt", "0.06");
  await enter("Tax rate", "0.25");
  await calculator.getByRole("button", { name: "Calculate" }).click();
  // Rust rejects the weights; no one here sums them.
  await expect(calculator.getByRole("alert")).toContainText("summing to 1.0");
  await enter("Debt weight", "0.4");
  await calculator.getByRole("button", { name: "Calculate" }).click();
  const apply = calculator.getByRole("button", { name: "Use as WACC" });
  await expect(apply).toBeEnabled();
  await expect(waccField).toHaveValue("");
  await apply.click();
  await expect(waccField).toHaveValue(/^0\.08[0-9]*$/);
  expect(errors).toEqual([]);
});

test("statements-workbench normalizes EBITDA and bridges it from the native steps, in a real worker", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=statements-workbench&theme=light&density=compact&focus=1",
  );
  await page.getByRole("tab", { name: "Metrics & ratios" }).click();
  await page
    .getByText("Adjusted EBITDA and normalization", { exact: true })
    .click();
  const editor = page.getByRole("region", { name: "Normalization" });
  const target = editor.getByRole("combobox", { name: "Target node" });
  await target.fill("ebitda");
  await page.getByRole("option", { name: "ebitda", exact: true }).click();
  await expect(target).toHaveValue("ebitda");
  await editor.getByRole("button", { name: "Normalize" }).click();
  const table = editor.getByRole("table", {
    name: "Adjustments by period (USD)",
  });
  await expect(table.getByRole("row")).toHaveCount(3);
  await expect(
    editor.getByRole("img", { name: /^Base to adjusted, / }),
  ).toBeAttached();
  expect(errors).toEqual([]);
});

test("capital-structure-report summarizes the debt stack and leaves a short window's ratios undefined, in a real worker", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=capital-structure-report&theme=light&density=compact&focus=1",
  );
  const report = page.getByRole("region", { name: "Capital structure" });
  const stack = report.getByRole("table", { name: "Debt stack at 2025Q4" });
  await expect(stack.getByRole("rowheader")).toHaveText([
    "SR-A",
    "SR-B",
    "SUB",
  ]);
  await expect(
    report.getByRole("img", { name: "Maturity wall at 2025Q4" }),
  ).toBeAttached();
  await report.getByRole("combobox", { name: "Period" }).click();
  await page.getByRole("option", { name: "2025Q3", exact: true }).click();
  await report.getByRole("button", { name: "Summarize" }).click();
  await expect(
    report.getByRole("table", { name: "Debt stack at 2025Q3" }),
  ).toBeVisible();
  // Rust returns no LTM EBITDA or ratio for three of four quarters.
  await expect(report.getByText("3 of 4 periods found")).toBeVisible();
  await expect(report.getByText("Undefined").first()).toBeVisible();
  await expect(report.getByText("NaN", { exact: true })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test("statements-workbench shows Rust's error for a model with no capital structure, in a real worker", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(
    "/registry-gallery/?item=statements-workbench&theme=light&density=compact&focus=1",
  );
  await page.getByRole("tab", { name: "Metrics & ratios" }).click();
  await page
    .getByText("Capital structure and debt stack", { exact: true })
    .click();
  const report = page.getByRole("region", { name: "Capital structure" });
  await report.getByRole("combobox", { name: "Period" }).click();
  await page.getByRole("option", { name: "2025Q4", exact: true }).click();
  const node = report.getByRole("combobox", { name: "EBITDA node" });
  await node.fill("ebitda");
  await page.getByRole("option", { name: "ebitda", exact: true }).click();
  await expect(node).toHaveValue("ebitda");
  await report.getByRole("button", { name: "Summarize" }).click();
  await expect(report.getByRole("alert")).toContainText(
    "has no capital structure",
  );
  expect(errors).toEqual([]);
});
