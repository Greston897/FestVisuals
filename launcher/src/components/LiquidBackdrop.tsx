import { useEffect } from "react";
import { CURRENT } from "@/lib/versions";

const MOTES = [8, 18, 29, 41, 52, 63, 74, 86, 93];

/**
 * Living backdrop the glass refracts. The art is the 26.2 key, orbs carry the
 * переливание (warm lava against a cool prism), motes rise like embers.
 */
export default function LiquidBackdrop() {
  useEffect(() => {
    const root = document.documentElement;
    let frame = 0;
    const onMove = (event: PointerEvent) => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const x = event.clientX / Math.max(window.innerWidth, 1) - 0.5;
        const y = event.clientY / Math.max(window.innerHeight, 1) - 0.5;
        root.style.setProperty("--mx", x.toFixed(4));
        root.style.setProperty("--my", y.toFixed(4));
      });
    };
    window.addEventListener("pointermove", onMove, { passive: true });
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener("pointermove", onMove);
    };
  }, []);

  return (
    <div className="scene" aria-hidden>
      <img className="scene-art" src={CURRENT.image} alt="" />
      <div className="scene-veil" />
      <div className="orb orb-ember" />
      <div className="orb orb-magma" />
      <div className="orb orb-prism" />
      <div className="orb orb-gold" />
      <div className="caustics" />
      {MOTES.map((left, index) => (
        <span key={left} className="mote" style={{ left: `${left}%`, animationDelay: `${-index * 1.7}s`, animationDuration: `${12 + (index % 4) * 3}s` }} />
      ))}
      <div className="grain" />
      <div className="vignette" />
    </div>
  );
}
