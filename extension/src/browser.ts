// Firefox exposes promise-based `browser`; Chrome's MV3 `chrome` returns
// promises too. Use whichever exists with Chrome's types.
export const ext: typeof chrome = (globalThis as { browser?: typeof chrome }).browser ?? chrome;

/** Browser family, matching what the desktop app derives from the exe name. */
export function browserName(): string {
  const ua = navigator.userAgent;
  if (ua.includes("Firefox/")) return "firefox";
  if (ua.includes("Edg/")) return "edge";
  if (ua.includes("OPR/")) return "opera";
  if (ua.includes("Vivaldi")) return "vivaldi";
  if ((navigator as { brave?: unknown }).brave) return "brave";
  return "chrome";
}

export interface Settings {
  clientId: string;
  token: string | null;
  port: number | null;
}

export async function settings(): Promise<Settings> {
  const s = await ext.storage.local.get(["clientId", "token", "port"]);
  let clientId = s.clientId as string | undefined;
  if (!clientId) {
    clientId = crypto.randomUUID();
    await ext.storage.local.set({ clientId });
  }
  return {
    clientId,
    token: (s.token as string | undefined) ?? null,
    port: (s.port as number | undefined) ?? null,
  };
}
