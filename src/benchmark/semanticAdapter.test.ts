import { isSemanticResult } from "./semanticAdapter";

describe("semantic adapter contract", () => {
  it("accepts a constrained structured result", () => {
    expect(
      isSemanticResult({
        category: "required-action",
        changeIntent: "none",
        tasks: [
          {
            title: "提交材料",
            timeExpression: "明天前",
            locationOrEntry: null,
            materials: [],
            audience: null,
            required: true,
            evidence: ["明天前提交材料"],
          },
        ],
        uncertainties: [],
      }),
    ).toBe(true);
  });

  it("rejects a category outside the contract", () => {
    expect(
      isSemanticResult({ category: "other", changeIntent: "none", tasks: [], uncertainties: [] }),
    ).toBe(false);
  });
});
