import { linkedWorkspace } from "./Studio";
import { shortPoint } from "./PackagesApp";

test("a link opens studies when it names a problem or asks for them, and packages otherwise", () => {
  expect(linkedWorkspace("")).toBe("packages");
  expect(linkedWorkspace("?package=packages%2Fmaze.zip&ref=maze%2Fmicro")).toBe("packages");
  expect(linkedWorkspace("?problem=maze&instance=showcase")).toBe("studies");
  expect(linkedWorkspace("?studies")).toBe("studies");
});

test("a point reads briefly as its run's last digits and its step", () => {
  expect(shortPoint("run-000000000000feed0000000000000007:12")).toBe("run-…000007:12");
});
