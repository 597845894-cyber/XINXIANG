import { fireEvent, render, screen } from "@testing-library/react";

import { App } from "./App";

describe("App", () => {
  it("renders the inbox as the initial workspace", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "收件箱" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "主导航" })).toBeInTheDocument();
  });

  it.each([
    ["快速导入", "导入微信通知"],
    ["任务核对", "核对任务候选"],
    ["任务表", "我的任务"],
    ["设置", "设置"],
  ])("opens the %s top-level view", (navigationLabel, heading) => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: new RegExp(navigationLabel) }));
    expect(screen.getByRole("heading", { name: heading, level: 2 })).toBeInTheDocument();
  });
});
