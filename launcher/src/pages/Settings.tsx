import { useEffect, useState } from "react";
import { Folder, HardDrive, Monitor } from "lucide-react";
import { Switch } from "@/components/ui/switch";
import { gameDir as savedGameDir, totalRamMb } from "@/lib/gameDir";
import { electron, nodeRequire } from "@/lib/desktop";
import { GlassPanel } from "@/components/Glass";

const Settings = () => {
  const maxRam = totalRamMb();
  const [ram, setRam] = useState(() => Math.min(Number(localStorage.getItem("settings_ram")) || 4096, maxRam));
  const [dir, setDir] = useState(() => savedGameDir());
  const [resW, setResW] = useState(() => localStorage.getItem("settings_res_w") || "1920");
  const [resH, setResH] = useState(() => localStorage.getItem("settings_res_h") || "1080");
  const [fullscreen, setFullscreen] = useState(() => localStorage.getItem("settings_fullscreen") === "true");
  const [vulkanEnabled, setVulkanEnabled] = useState(() => localStorage.getItem("settings_vulkan") !== "false");
  const [closeOnLaunch, setCloseOnLaunch] = useState(() => localStorage.getItem("settings_close_launch") === "true");
  const [note, setNote] = useState<string | null>(null);

  useEffect(() => {
    localStorage.setItem("settings_ram", String(ram));
    localStorage.setItem("settings_dir", dir);
    localStorage.setItem("settings_res_w", resW);
    localStorage.setItem("settings_res_h", resH);
    localStorage.setItem("settings_fullscreen", String(fullscreen));
    localStorage.setItem("settings_close_launch", String(closeOnLaunch));
    localStorage.setItem("settings_vulkan", String(vulkanEnabled));
  }, [ram, dir, resW, resH, fullscreen, closeOnLaunch, vulkanEnabled]);

  const openPath = (target: string) => {
    const ipc = electron();
    const fs = nodeRequire("fs");
    if (!ipc || !fs) {
      setNote("Папки открываются в приложении лаунчера");
      return;
    }
    try {
      if (!fs.existsSync(target)) fs.mkdirSync(target, { recursive: true });
      ipc.shell.openPath(target);
      setNote(null);
    } catch (e) {
      setNote(e instanceof Error ? e.message : "Не удалось открыть папку");
    }
  };

  const pct = maxRam <= 1024 ? 100 : ((ram - 1024) / (maxRam - 1024)) * 100;
  const gb = ram / 1024;

  return (
    <div className="sheet settings-page">
      <header className="sheet-head">
        <p className="eyebrow">Клиент 26.2</p>
        <h1>Настройки</h1>
        <p>Память, окно и папка игры. Моды, миры и конфиги лежат в instances/26.2.</p>
      </header>

      {note && (
        <button type="button" className="err-note sheet-note" onClick={() => setNote(null)}>
          {note}
        </button>
      )}

      <div className="sheet-grid">
        <GlassPanel className="panel">
          <div className="panel-title">
            <HardDrive size={18} />
            <h2>Память</h2>
            <strong className="ember">{Number.isInteger(gb) ? `${gb} ГБ` : `${ram} МБ`}</strong>
          </div>
          <p className="hint">Сколько оперативной памяти отдать игре. Верхняя граница — память этого компьютера.</p>
          <div className="range-labels">
            <span>1 ГБ</span>
            <span>{Math.round(maxRam / 1024)} ГБ</span>
          </div>
          <input
            className="liquid"
            type="range"
            min={1024}
            max={maxRam}
            step={512}
            value={ram}
            style={{ ["--p" as string]: `${pct}%` }}
            aria-label="Выделение ОЗУ"
            onChange={(event) => setRam(Number(event.target.value))}
          />
          <label className="field-line">
            <span>Точно, МБ</span>
            <input
              className="field field-num"
              type="number"
              min={1024}
              max={maxRam}
              value={ram}
              onChange={(event) => setRam(Math.max(1024, Math.min(maxRam, Number(event.target.value) || 1024)))}
            />
          </label>
        </GlassPanel>

        <GlassPanel className="panel">
          <div className="panel-title">
            <Folder size={18} />
            <h2>Папка игры</h2>
          </div>
          <p className="hint">Сюда скачиваются общие ресурсы. У клиента своя папка instances/26.2 — моды, миры, настройки.</p>
          <input className="field" value={dir} spellCheck={false} onChange={(event) => setDir(event.target.value)} />
          <div className="btn-row">
            <button type="button" className="gel gel-ghost" onClick={() => openPath(dir)}>
              Папка игры
            </button>
            <button
              type="button"
              className="gel gel-ghost"
              onClick={() => {
                const path = nodeRequire("path");
                if (!path) {
                  setNote("Папки открываются в приложении лаунчера");
                  return;
                }
                openPath(path.join(dir, "instances", "26.2"));
              }}
            >
              Папка 26.2
            </button>
          </div>
        </GlassPanel>

        <GlassPanel className="panel">
          <div className="panel-title">
            <Monitor size={18} />
            <h2>Окно</h2>
          </div>
          <p className="hint">Размер окна при запуске. В полноэкранном режиме эти числа не используются.</p>
          <div className="res-row">
            <label>
              <span>Ширина</span>
              <input className="field" type="number" min={640} max={7680} value={resW} onChange={(event) => setResW(event.target.value)} />
            </label>
            <label>
              <span>Высота</span>
              <input className="field" type="number" min={480} max={4320} value={resH} onChange={(event) => setResH(event.target.value)} />
            </label>
          </div>
          <div className="presets">
            {[["1280", "720"], ["1600", "900"], ["1920", "1080"]].map(([w, h]) => (
              <button key={w} type="button" className="preset" onClick={() => { setResW(w); setResH(h); }}>
                {w}×{h}
              </button>
            ))}
          </div>
        </GlassPanel>

        <GlassPanel className="panel">
          <div className="panel-title">
            <h2>Дополнительно</h2>
          </div>
          <div className="toggle-row">
            <div>
              <strong>Полный экран</strong>
              <span>Запускать игру на весь монитор</span>
            </div>
            <Switch checked={fullscreen} onCheckedChange={setFullscreen} />
          </div>
          <div className="toggle-row">
            <div>
              <strong>Закрывать лаунчер</strong>
              <span>Прятать окно, когда игра уже запущена</span>
            </div>
            <Switch checked={closeOnLaunch} onCheckedChange={setCloseOnLaunch} />
          </div>
          <div className="toggle-row is-locked">
            <div>
              <strong>Vulkan</strong>
              <span>Замена Sodium — появится позже, переключатель пока закрыт</span>
            </div>
            <Switch checked={vulkanEnabled} onCheckedChange={setVulkanEnabled} disabled />
          </div>
        </GlassPanel>
      </div>
    </div>
  );
};

export default Settings;
