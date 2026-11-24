import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { ErrorBoundary } from "./ErrorBoundary";

function Draws({ broken }: { broken: boolean }) {
  if (broken) throw new Error("no such cell");
  return <p>the maze</p>;
}

describe("ErrorBoundary", () => {
  // React reports what a boundary catches on the console.
  beforeEach(() => { vi.spyOn(console, "error").mockImplementation(() => undefined); });
  afterEach(() => { vi.restoreAllMocks(); });

  test("says what failed to draw, and leaves what is around it", () => {
    render(<><p>the tree</p><ErrorBoundary label="The picture"><Draws broken /></ErrorBoundary></>);
    expect(screen.getByRole("alert")).toHaveTextContent("The picture could not be drawn. no such cell");
    expect(screen.getByText("the tree")).toBeInTheDocument();
  });

  test("draws again on what is next, or when asked", () => {
    const page = (step: number, broken: boolean) => <ErrorBoundary label="The picture" resetOn={step}><Draws broken={broken} /></ErrorBoundary>;
    const { rerender } = render(page(1, false));
    rerender(page(2, true));
    expect(screen.getByRole("alert")).toBeInTheDocument();
    rerender(page(3, false));
    expect(screen.getByText("the maze")).toBeInTheDocument();
    rerender(page(4, true));
    rerender(page(4, false));
    fireEvent.click(screen.getByRole("button", { name: "Try again" }));
    expect(screen.getByText("the maze")).toBeInTheDocument();
  });
});
