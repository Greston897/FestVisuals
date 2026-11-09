import { Navigate } from "react-router-dom";

/** Old per-version route. The launcher is a single 26.2 cockpit now. */
const Index = () => <Navigate to="/" replace />;

export default Index;
