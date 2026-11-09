import { describe, expect, it } from "vitest";
import { formatQuantity, type QuantityFormat } from "./Quantity";

// The same cases as the view's Rust `formats_write_exact_values_and_never_round`.
describe("formatQuantity", () => {
  const usd: QuantityFormat = { scale: 2, digits: 2, unit: "USD" };
  const kilograms: QuantityFormat = { scale: 3, unit: "kg" };
  const percent: QuantityFormat = { scale: 16, unit: "%" };

  it("writes counts as they are", () => {
    expect(formatQuantity("0")).toBe("0");
    expect(formatQuantity("1250", { scale: 0 })).toBe("1250");
  });

  it("keeps the digits a format asks for", () => {
    expect(formatQuantity("1250", usd)).toBe("12.50 USD");
    expect(formatQuantity("5", usd)).toBe("0.05 USD");
    expect(formatQuantity("0", usd)).toBe("0.00 USD");
    expect(formatQuantity("7", { scale: 0, digits: 2 })).toBe("7.00");
  });

  it("trims zeros and never rounds", () => {
    expect(formatQuantity("1000", kilograms)).toBe("1 kg");
    expect(formatQuantity("900", kilograms)).toBe("0.9 kg");
    expect(formatQuantity("1234567", kilograms)).toBe("1234.567 kg");
    expect(formatQuantity("50000000000000000", percent)).toBe("5 %");
    expect(formatQuantity("1", percent)).toBe("0.0000000000000001 %");
  });

  it("is exact beyond floating point", () => {
    const max = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
    expect(formatQuantity(max, { scale: 18 })).toBe(`${max.slice(0, -18)}.${max.slice(-18).replace(/0+$/, "")}`);
  });
});
