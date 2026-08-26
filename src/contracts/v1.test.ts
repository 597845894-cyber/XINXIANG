import fixture from "../../contracts/v1/app-bootstrap.json";

import { CONTRACT_VERSION, isAppBootstrapV1, type AppBootstrapV1 } from "./v1";

describe("application contract v1", () => {
  it("accepts the shared Rust/TypeScript bootstrap fixture", () => {
    expect(isAppBootstrapV1(fixture)).toBe(true);

    const contract = fixture as AppBootstrapV1;
    expect(contract.contractVersion).toBe(CONTRACT_VERSION);
    expect(contract.routes.map(({ id }) => id)).toEqual([
      "inbox",
      "quickImport",
      "review",
      "tasks",
      "settings",
    ]);
    expect(contract.commands).toContain("getModelResourceStatus");
    expect(contract.commands).toContain("installModelResources");
  });

  it("rejects an incompatible contract version", () => {
    expect(isAppBootstrapV1({ ...fixture, contractVersion: 2 })).toBe(false);
  });
});
