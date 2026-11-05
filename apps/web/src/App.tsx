import { lazy, Suspense } from "react";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import Home from "./pages/Home";
import NotFound from "./pages/NotFound";

// The app pages load on first visit, so the landing page does not carry the chat, markdown or admin code.
const Signed = lazy(() => import("./app/session"));
const Chat = lazy(() => import("./pages/Chat"));
const Settings = lazy(() => import("./pages/settings/Settings"));
const AdminLayout = lazy(() => import("./pages/admin/Layout"));
const Overview = lazy(() => import("./pages/admin/Overview"));
const Tools = lazy(() => import("./pages/admin/Tools"));
const Instructions = lazy(() => import("./pages/admin/Instructions"));
const Traces = lazy(() => import("./pages/admin/Traces"));
const Trace = lazy(() => import("./pages/admin/Trace"));
const Approvals = lazy(() => import("./pages/admin/Approvals"));
const History = lazy(() => import("./pages/admin/History"));
const Sources = lazy(() => import("./pages/admin/Sources"));
const Limits = lazy(() => import("./pages/admin/Limits"));
const Guardrails = lazy(() => import("./pages/admin/Guardrails"));

const Blank = () => <div className="min-h-screen bg-canvas" />;

const App = () => (
  <BrowserRouter>
    <Suspense fallback={<Blank />}>
      <Routes>
        <Route path="/" element={<Home />} />
        <Route element={<Signed />}>
          <Route path="/chat/:id?" element={<Chat />} />
          <Route path="/settings/:section?" element={<Settings />} />
          <Route path="/memories" element={<Navigate to="/settings/memories" replace />} />
          <Route path="/connections" element={<Navigate to="/settings/accounts" replace />} />
          <Route path="/admin" element={<AdminLayout />}>
            <Route index element={<Overview />} />
            <Route path="tools" element={<Tools />} />
            <Route path="instructions" element={<Instructions />} />
            <Route path="guardrails" element={<Guardrails />} />
            <Route path="traces" element={<Traces />} />
            <Route path="traces/:id" element={<Trace />} />
            <Route path="approvals" element={<Approvals />} />
            <Route path="history" element={<History />} />
            <Route path="sources" element={<Sources />} />
            <Route path="limits" element={<Limits />} />
          </Route>
        </Route>
        <Route path="*" element={<NotFound />} />
      </Routes>
    </Suspense>
  </BrowserRouter>
);

export default App;
