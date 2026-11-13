import { shortPoint } from "./PackagesApp";

test("a point reads briefly as its run's last digits and its step", () => {
  expect(shortPoint("run-000000000000feed0000000000000007:12")).toBe("run-…000007:12");
});
