import { expect, it } from "vitest";
import { datedPoints } from "@/components/finstack/analytics/lib/analytics-report-presentation";

it("keeps native values and date order while omitting non-finite or missing observations", () => {
  const dates = [
    "2025-03-10",
    "2025-03-09",
    "2025-03-08",
    "2025-03-07",
    "2025-03-06",
    "2025-03-05",
    "2025-03-04",
  ];
  const values = [0, NaN, -0.125, Infinity, 0.25, -Infinity];
  const points = datedPoints("Fund", dates, values);
  expect(
    points.map(({ series, iso, date, value }) => [
      series,
      iso,
      date.toISOString(),
      value,
    ]),
  ).toEqual([
    ["Fund", "2025-03-10", "2025-03-10T00:00:00.000Z", 0],
    ["Fund", "2025-03-08", "2025-03-08T00:00:00.000Z", -0.125],
    ["Fund", "2025-03-06", "2025-03-06T00:00:00.000Z", 0.25],
  ]);
  expect(values).toEqual([0, NaN, -0.125, Infinity, 0.25, -Infinity]);
});
