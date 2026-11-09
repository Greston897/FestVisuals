import { useRef, type HTMLAttributes, type PointerEvent, type ReactNode, type RefObject } from "react";

/** Hairline rim, lit crescent and pointer glare. The blur itself lives in CSS. */
export function GlassFx() {
  return (
    <>
      <span className="glass-prism" aria-hidden />
      <span className="glass-rim" aria-hidden />
      <span className="glass-crescent" aria-hidden />
      <span className="glass-glare" aria-hidden />
    </>
  );
}

export function useGlassPointer<T extends HTMLElement>(): {
  ref: RefObject<T>;
  onPointerMove: (event: PointerEvent<T>) => void;
} {
  const ref = useRef<T>(null);
  const onPointerMove = (event: PointerEvent<T>) => {
    const el = ref.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    el.style.setProperty("--lx", `${event.clientX - rect.left}px`);
    el.style.setProperty("--ly", `${event.clientY - rect.top}px`);
  };
  return { ref, onPointerMove };
}

type PanelProps = HTMLAttributes<HTMLDivElement> & {
  strong?: boolean;
  children?: ReactNode;
};

/** A liquid-glass surface. Do not transform this element — that breaks backdrop-filter. */
export function GlassPanel({ strong, className, children, onPointerMove, ...rest }: PanelProps) {
  const glass = useGlassPointer<HTMLDivElement>();
  return (
    <div
      ref={glass.ref}
      className={["glass", strong ? "glass-strong" : "", className].filter(Boolean).join(" ")}
      onPointerMove={(event) => {
        glass.onPointerMove(event);
        onPointerMove?.(event);
      }}
      {...rest}
    >
      <GlassFx />
      {children}
    </div>
  );
}
