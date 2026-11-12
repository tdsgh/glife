import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

vi.mock("./view/GridView", () => ({ GridView: () => <div data-testid="grid-view" /> }));

import { App } from "./App";

test("mounts the grid view full-window", () => {
  render(<App />);
  expect(screen.getByTestId("app").contains(screen.getByTestId("grid-view"))).toBe(true);
});
