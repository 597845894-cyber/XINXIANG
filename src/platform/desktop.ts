import { invoke } from "@tauri-apps/api/core";
import type { ModelResourceStatusV1, SecurityStatusV1 } from "../contracts/v1";

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export function isDesktopRuntime() {
  return typeof window !== "undefined" && Boolean(window.__TAURI_INTERNALS__);
}

export async function quitDesktopApp() {
  if (!isDesktopRuntime()) return;
  await invoke("quit_app");
}

export async function getSecurityStatus(): Promise<SecurityStatusV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<SecurityStatusV1>("get_security_status");
}

export async function getModelResourceStatus(): Promise<ModelResourceStatusV1 | null> {
  if (!isDesktopRuntime()) return null;
  return invoke<ModelResourceStatusV1>("get_model_resource_status");
}

export async function installModelResources(
  sourceDirectory: string,
): Promise<ModelResourceStatusV1> {
  return invoke<ModelResourceStatusV1>("install_model_resources", { sourceDirectory });
}
