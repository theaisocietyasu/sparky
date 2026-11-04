import { BrowserRouter, Route, Routes } from "react-router-dom";
import Admin from "./pages/admin/Admin";
import Chat from "./pages/Chat";
import Home from "./pages/Home";
import Memories from "./pages/Memories";
import NotFound from "./pages/NotFound";

const App = () => (
  <BrowserRouter>
    <Routes>
      <Route path="/" element={<Home />} />
      <Route path="/admin" element={<Admin />} />
      <Route path="/admin/:tab" element={<Admin />} />
      <Route path="/chat" element={<Chat />} />
      <Route path="/memories" element={<Memories />} />
      <Route path="*" element={<NotFound />} />
    </Routes>
  </BrowserRouter>
);

export default App;
