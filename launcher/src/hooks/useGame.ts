import { useCallback, useEffect, useState } from "react";
import { gameDir, instanceDir, totalRamMb } from "@/lib/gameDir";
import { electron, nodeRequire } from "@/lib/desktop";

export type GameState = {
  state: "idle" | "launching" | "downloading" | "starting" | "running";
  version: string | null;
  type?: string;
  percent?: number;
};

export const PROGRESS_LABELS: Record<string, string> = {
  assets: "ассетов",
  "assets-copy": "ассетов",
  natives: "движка",
  classes: "библиотек",
  libraries: "библиотек",
  "classes-custom": "Fabric",
  "classes-maven-custom": "Fabric",
  "version-jar": "игры",
  java: "Java",
  mods: "модов",
  festvisuals: "клиента",
  instant: "мгновенно",
};

const idle: GameState = { state: "idle", version: null };

export function useGame() {
  const [game, setGame] = useState<GameState>(idle);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const ipc = electron()?.ipcRenderer;
    if (!ipc) return;
    const onState = (_: unknown, next: GameState) => setGame(next);
    const onError = (_: unknown, payload: { message?: string }) => {
      setError(payload?.message || "Не удалось запустить игру");
      setGame(idle);
    };
    ipc.on("game-state", onState);
    ipc.on("launch-error", onError);
    ipc.send("get-game-state");
    return () => {
      ipc.removeListener("game-state", onState);
      ipc.removeListener("launch-error", onError);
    };
  }, []);

  useEffect(() => {
    if (game.state === "running") localStorage.setItem("last_launch", String(Date.now()));
  }, [game.state]);

  const launch = useCallback((versionId: string) => {
    setError(null);
    const ipc = electron()?.ipcRenderer;
    if (!ipc) {
      setError("Запуск доступен в приложении лаунчера");
      return;
    }
    if (game.state !== "idle") return;
    const ram = Math.min(Number(localStorage.getItem("settings_ram")) || 4096, totalRamMb());
    const width = Math.min(Number(localStorage.getItem("settings_res_w")) || 1280, window.screen.availWidth);
    const height = Math.min(Number(localStorage.getItem("settings_res_h")) || 720, window.screen.availHeight);
    ipc.send("launch-game", {
      version: versionId,
      root: gameDir(),
      ram,
      username: localStorage.getItem("user_login") || "FestPlayer",
      closeOnLaunch: localStorage.getItem("settings_close_launch") === "true",
      vulkan: localStorage.getItem("settings_vulkan") !== "false",
      window: { width, height, fullscreen: localStorage.getItem("settings_fullscreen") === "true" },
    });
  }, [game.state]);

  const openMods = useCallback((versionId: string) => {
    const ipc = electron();
    const fs = nodeRequire("fs");
    const path = nodeRequire("path");
    if (!ipc || !fs || !path) {
      setError("Папка модов открывается в приложении лаунчера");
      return;
    }
    try {
      const mods = path.join(instanceDir(versionId), "mods");
      fs.mkdirSync(mods, { recursive: true });
      ipc.shell.openPath(mods);
    } catch (e) {
      setError(e instanceof Error ? e.message : "Не удалось открыть папку модов");
    }
  }, []);

  return { game, error, setError, launch, openMods };
}
