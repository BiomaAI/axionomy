import { render, screen } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import { App } from "./App";

vi.mock("echarts/core", () => ({
  use: vi.fn(),
  init: () => ({ setOption: vi.fn(), resize: vi.fn(), dispose: vi.fn() }),
}));

beforeEach(() => {
  vi.stubGlobal("fetch", vi.fn(async (input: RequestInfo | URL) => {
    if (String(input).includes("/api/examples")) {
      return new Response(JSON.stringify({ examples: [] }), { status: 200, headers: { "content-type": "application/json" } });
    }
    return new Response(JSON.stringify({
      id: "fixture",
      title: "Fixture economy",
      description: "Replay fixture",
      source: { key: "fixture", label: "Fixture" },
      initial: { index: 0, accounts: [], scene: null },
      frames: [],
      objectives: [],
      pareto_fronts: [],
    }), { status: 200 });
  }));
});

afterEach(() => vi.unstubAllGlobals());

test("loads a portable document without requiring the server", async () => {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(<QueryClientProvider client={client}><App /></QueryClientProvider>);
  expect(await screen.findByRole("heading", { name: "Fixture economy" })).toBeInTheDocument();
  expect(screen.getByText("replay verified")).toBeInTheDocument();
});
