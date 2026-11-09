import { Toaster } from "@/components/ui/toaster";
import { Toaster as Sonner } from "@/components/ui/sonner";
import { TooltipProvider } from "@/components/ui/tooltip";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { HashRouter, Navigate, Route, Routes, useLocation } from "react-router-dom";
import Sidebar from "./components/Sidebar";
import UpdateBanner from "./components/UpdateBanner";
import LiquidBackdrop from "./components/LiquidBackdrop";
import { GlassFx, useGlassPointer } from "./components/Glass";
import Home from "./pages/Home";
import Index from "./pages/Index";
import Profile from "./pages/Profile";
import Settings from "./pages/Settings";
import { Minus, X } from "lucide-react";
import { electron } from "@/lib/desktop";

const queryClient = new QueryClient();

const Topbar = () => {
  const brand = useGlassPointer<HTMLDivElement>();
  return (
    <header className="topbar window-drag">
      <div ref={brand.ref} className="brand glass no-drag" onPointerMove={brand.onPointerMove}>
        <GlassFx />
        <img className="brand-mark" src="./icon.png" alt="" />
        <span>FestVisuals</span>
      </div>
      <div className="win-controls no-drag">
        <button type="button" className="win-btn" aria-label="Свернуть" onClick={() => electron()?.ipcRenderer.send("minimize-window")}>
          <Minus size={15} />
        </button>
        <button type="button" className="win-btn win-close" aria-label="Закрыть" onClick={() => electron()?.ipcRenderer.send("close-window")}>
          <X size={15} />
        </button>
      </div>
    </header>
  );
};

const Shell = () => {
  const { pathname } = useLocation();
  return (
    <div className="app-shell">
      <LiquidBackdrop />
      <Topbar />
      <div className="workspace">
        <Sidebar />
        <main className="stage">
          <UpdateBanner />
          <div key={pathname} className="page page-enter">
            <Routes>
              <Route path="/" element={<Home />} />
              <Route path="/play/:id" element={<Index />} />
              <Route path="/profile" element={<Profile />} />
              <Route path="/settings" element={<Settings />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </div>
        </main>
      </div>
    </div>
  );
};

const App = () => (
  <QueryClientProvider client={queryClient}>
    <TooltipProvider>
      <Toaster />
      <Sonner theme="dark" />
      <HashRouter>
        <Shell />
      </HashRouter>
    </TooltipProvider>
  </QueryClientProvider>
);

export default App;
