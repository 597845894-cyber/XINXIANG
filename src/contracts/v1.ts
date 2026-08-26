export const CONTRACT_VERSION = 1 as const;

export type AppRouteId = "inbox" | "quickImport" | "review" | "tasks" | "settings";

export type CommandName = "getAppBootstrap" | "openQuickImport" | "quitApp";

export type EventName = "quickImportRequested" | "windowHiddenToTray";

export interface RouteDescriptorV1 {
  id: AppRouteId;
  label: string;
}

export interface AppBootstrapV1 {
  contractVersion: typeof CONTRACT_VERSION;
  appVersion: string;
  routes: RouteDescriptorV1[];
  commands: CommandName[];
  events: EventName[];
}

export function isAppBootstrapV1(value: unknown): value is AppBootstrapV1 {
  if (!value || typeof value !== "object") return false;

  const candidate = value as Partial<AppBootstrapV1>;
  return (
    candidate.contractVersion === CONTRACT_VERSION &&
    typeof candidate.appVersion === "string" &&
    Array.isArray(candidate.routes) &&
    Array.isArray(candidate.commands) &&
    Array.isArray(candidate.events)
  );
}
