import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { Check, Download, Loader2, Play } from "lucide-react";
import { CURRENT } from "@/lib/versions";
import { PROGRESS_LABELS, useGame } from "@/hooks/useGame";

const Home = () => {
  const navigate = useNavigate();
  const version = CURRENT;
  const { game, error, setError, launch, openMods } = useGame();
  const mine = game.version === version.id || game.version == null;
  const status = game.version === version.id ? game.state : "idle";
  const busy = status === "launching" || status === "starting" || status === "downloading";
  const locked = game.state !== "idle";

  const [login, setLogin] = useState("FestPlayer");
  const [warm, setWarm] = useState(false);

  useEffect(() => {
    setLogin(localStorage.getItem("user_login") || "FestPlayer");
    setWarm(localStorage.getItem("fest_instant") === "1");
  }, []);

  useEffect(() => {
    if (game.state === "running") {
      localStorage.setItem("fest_instant", "1");
      setWarm(true);
    }
  }, [game.state]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Enter" || event.repeat || event.metaKey || event.ctrlKey || event.altKey) return;
      const tag = (event.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      if (game.state !== "idle") return;
      launch(version.id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [game.state, launch, version.id]);

  const kicker = (() => {
    if (!mine && game.state !== "idle") return `Запущена ${game.version}`;
    if (status === "running") return "В игре";
    if (status === "starting") return "Запускаю клиент";
    if (status === "launching") return warm ? "Мгновенный запуск" : "Проверяю файлы";
    if (status === "downloading") {
      const what = PROGRESS_LABELS[game.type || ""] || "файлов";
      return `Загрузка ${what} · ${game.percent ?? 0}%`;
    }
    return warm ? "Мгновенный запуск" : "Готов к запуску";
  })();

  const label = status === "running" ? "В игре" : status === "downloading" ? "Загрузка" : busy ? "Секунду" : "Запустить";

  return (
    <div className="stage">
      <div className="stage-bar glass">
        <div className="stage-id">
          <div className="kicker">
            <span className={`live-dot${status === "running" ? " is-on" : busy ? " is-busy" : ""}`} />
            <span>{kicker}</span>
            <span className="tag">{version.tag}</span>
          </div>
          <div className="stage-title">
            <span className="iris-text stage-ver">{version.title}</span>
            <span className="stage-name">FestVisuals</span>
          </div>
          <button type="button" className="stage-who" onClick={() => navigate("/profile")}>
            игрок <b>{login}</b>
          </button>
        </div>
        <div className="stage-act">
          <button type="button" className="stage-mods" onClick={() => openMods(version.id)}>папка модов</button>
          <button
            type="button"
            className={`play gel${busy ? " is-busy" : ""}${status === "running" ? " is-running" : ""}`}
            disabled={locked}
            onClick={() => launch(version.id)}
            style={{ ["--pct" as string]: `${status === "downloading" ? game.percent ?? 0 : 0}%` }}
          >
            <span className="play-fill" />
            <span className="play-sheen" />
            <span className="play-label">
              {status === "idle" && mine && <Play size={18} fill="currentColor" />}
              {busy && status !== "downloading" && <Loader2 size={18} className="spin" />}
              {status === "downloading" && <Download size={18} />}
              {status === "running" && <Check size={18} />}
              <span>{label}</span>
              {status === "downloading" && <span className="play-pct">{game.percent ?? 0}%</span>}
              {status === "idle" && <kbd>Enter</kbd>}
            </span>
          </button>
        </div>
      </div>

      {error && (
        <button type="button" className="err-note stage-err" onClick={() => setError(null)}>{error}</button>
      )}

      <div className="stage-grid">
        <button type="button" className="plate plate-main" onClick={() => launch(version.id)} disabled={locked}>
          <img src="./art/sakura.jpg" alt="" />
          <span className="plate-scrim" />
          <span className="plate-cap"><b>Сакура</b> рассвет</span>
        </button>
        <button type="button" className="plate" onClick={() => launch(version.id)} disabled={locked}>
          <img src="./art/nether.jpg" alt="" />
          <span className="plate-scrim" />
          <span className="plate-cap"><b>Незер</b></span>
        </button>
        <button type="button" className="plate" onClick={() => launch(version.id)} disabled={locked}>
          <img src="./art/end.jpg" alt="" />
          <span className="plate-scrim" />
          <span className="plate-cap"><b>Энд</b></span>
        </button>
      </div>
    </div>
  );
};

export default Home;
