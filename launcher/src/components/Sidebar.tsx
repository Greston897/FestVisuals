import { useLayoutEffect, useRef, useState } from "react";
import { Home, Settings, User } from "lucide-react";
import { NavLink, useLocation } from "react-router-dom";
import { GlassFx, useGlassPointer } from "./Glass";

const LINKS = [
  { to: "/", label: "Главная", icon: Home, end: true },
  { to: "/profile", label: "Профиль", icon: User, end: false },
  { to: "/settings", label: "Настройки", icon: Settings, end: false },
];

const Sidebar = () => {
  const { pathname } = useLocation();
  const glass = useGlassPointer<HTMLElement>();
  const linksRef = useRef<HTMLDivElement>(null);
  const [indicator, setIndicator] = useState({ x: 0, w: 132, visible: false });

  useLayoutEffect(() => {
    const root = linksRef.current;
    const active = root?.querySelector<HTMLElement>(".dock-link.active");
    if (!root || !active) {
      setIndicator((current) => ({ ...current, visible: false }));
      return;
    }
    setIndicator({ x: active.offsetLeft, w: active.offsetWidth, visible: true });
  }, [pathname]);

  return (
    <nav ref={glass.ref} className="dock glass" aria-label="Разделы" onPointerMove={glass.onPointerMove}>
      <GlassFx />
      <div className="dock-links" ref={linksRef}>
        <span
          className="dock-indicator"
          style={{ left: indicator.x, width: indicator.w, opacity: indicator.visible ? 1 : 0 }}
          aria-hidden
        />
        {LINKS.map(({ to, label, icon: Icon, end }) => (
          <NavLink key={to} to={to} end={end} className={({ isActive }) => `dock-link${isActive ? " active" : ""}`}>
            <Icon size={16} strokeWidth={2.2} />
            <span>{label}</span>
          </NavLink>
        ))}
      </div>
    </nav>
  );
};

export default Sidebar;
