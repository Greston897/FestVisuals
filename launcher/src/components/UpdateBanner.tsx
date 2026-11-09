import { useEffect, useState } from "react";
import { Download, RefreshCw } from "lucide-react";
import { electron } from "@/lib/desktop";
import { GlassFx } from "./Glass";

type UpdateState = { state: "idle" | "downloading" | "ready"; version?: string; percent?: number };

/** Floating glass pill while the launcher replaces itself. */
const UpdateBanner = () => {
  const [update, setUpdate] = useState<UpdateState>({ state: "idle" });

  useEffect(() => {
    const ipc = electron()?.ipcRenderer;
    if (!ipc) return;
    const onUpdate = (_: unknown, next: UpdateState) =>
      setUpdate((current) => ({ ...current, ...next, version: next.version || current.version }));
    ipc.on("updater", onUpdate);
    return () => ipc.removeListener("updater", onUpdate);
  }, []);

  if (update.state === "idle") return null;

  return (
    <div className="update-float" role="status">
      <div className="update-pill glass">
        <GlassFx />
        {update.state === "downloading" ? (
          <>
            <Download size={14} className="spin-soft" />
            <span>Обновление{update.version ? ` ${update.version}` : ""}</span>
            <div className="update-track">
              <div style={{ width: `${update.percent ?? 0}%` }} />
            </div>
            <span className="update-pct">{update.percent ?? 0}%</span>
          </>
        ) : (
          <>
            <RefreshCw size={14} className="spin" />
            <span>Обновление {update.version} готово — перезапускаю</span>
          </>
        )}
      </div>
    </div>
  );
};

export default UpdateBanner;
