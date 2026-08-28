import { fireEvent, render, screen } from "@testing-library/react";

import { App } from "./App";

describe("App", () => {
  it("renders the inbox as the initial workspace", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "收件箱" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "主导航" })).toBeInTheDocument();
    expect(screen.getByText("这里还没有通知。导入文字后会显示在这里。")).toBeInTheDocument();
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

  it("provides a text-only import form", () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: /快速导入/ }));

    expect(screen.getByRole("textbox", { name: "通知原文" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /上传截图/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /粘贴剪贴板图片/ })).not.toBeInTheDocument();
  });

  it("shows independent candidate evidence in the review workspace", async () => {
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: /任务核对/ }));

    expect(await screen.findByRole("heading", { name: "核对任务候选" })).toBeInTheDocument();
    expect(screen.getByText(/请于 8 月 28 日 17:00 前完成实验室安全准入考试/)).toBeInTheDocument();
    expect(screen.getByText("提交报名材料")).toBeInTheDocument();
  });
});
