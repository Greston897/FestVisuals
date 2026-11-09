import { useEffect, useRef, useState } from "react";
import { Camera, User } from "lucide-react";
import { GlassPanel } from "@/components/Glass";

const Profile = () => {
  const [avatar, setAvatar] = useState<string | null>(null);
  const [login, setLogin] = useState("FestPlayer");
  const fileRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setAvatar(localStorage.getItem("user_avatar"));
    setLogin(localStorage.getItem("user_login") || "FestPlayer");
  }, []);

  useEffect(() => {
    localStorage.setItem("user_login", login);
  }, [login]);

  const onAvatar = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onloadend = () => {
      const base64 = String(reader.result || "");
      setAvatar(base64);
      localStorage.setItem("user_avatar", base64);
    };
    reader.readAsDataURL(file);
  };

  return (
    <div className="sheet">
      <header className="sheet-head">
        <p className="eyebrow">Аккаунт</p>
        <h1>Профиль</h1>
        <p>Ник уходит в игру как имя игрока. Аватар виден только в лаунчере.</p>
      </header>

      <div className="sheet-grid">
        <GlassPanel className="panel identity">
          <div className="avatar-ring">
            <button type="button" className="avatar" onClick={() => fileRef.current?.click()} aria-label="Изменить аватар">
              {avatar ? <img src={avatar} alt="" /> : <User size={42} strokeWidth={1.6} />}
              <span className="avatar-edit">
                <Camera size={16} />
              </span>
            </button>
          </div>
          <input ref={fileRef} type="file" accept="image/*" hidden onChange={onAvatar} />
          <div className="identity-name">{login || "FestPlayer"}</div>
          <div className="identity-role">Пользователь · навсегда</div>
        </GlassPanel>

        <GlassPanel strong className="panel">
          <h2>Данные</h2>
          <label className="field-row">
            <span>Ник</span>
            <input
              className="field"
              value={login}
              spellCheck={false}
              onChange={(event) => setLogin(event.target.value)}
            />
          </label>
          <div className="field-row">
            <span>Роль</span>
            <strong className="ember">Пользователь</strong>
          </div>
          <div className="field-row">
            <span>Подписка</span>
            <strong>Навсегда</strong>
          </div>
          <p className="hint">Этот ник лаунчер передаёт в игру при запуске.</p>
        </GlassPanel>
      </div>
    </div>
  );
};

export default Profile;
