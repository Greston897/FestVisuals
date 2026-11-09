/** Electron's node integration, or null in a plain browser preview. */
export function nodeRequire(module: string): any | null {
  const req = (window as Window & { require?: (id: string) => any }).require;
  if (!req) return null;
  try {
    return req(module);
  } catch {
    return null;
  }
}

export function electron(): null | {
  ipcRenderer: {
    on: (channel: string, listener: (event: unknown, payload: any) => void) => void;
    removeListener: (channel: string, listener: (event: unknown, payload: any) => void) => void;
    send: (channel: string, payload?: unknown) => void;
  };
  shell: { openPath: (path: string) => Promise<string> };
} {
  return nodeRequire("electron");
}
