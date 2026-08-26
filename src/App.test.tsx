import { render, screen } from "@testing-library/react";

import { App } from "./App";

describe("App", () => {
  it("renders the product identity", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "校园信箱" })).toBeInTheDocument();
  });
});
