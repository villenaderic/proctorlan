import { HashRouter, Route, Routes } from "react-router-dom";
import { WelcomePage } from "@/pages/WelcomePage";
import { ComingSoonPage } from "@/pages/ComingSoonPage";

// HashRouter: works from Tauri's bundled assets without server-side routing.
export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route path="/" element={<WelcomePage />} />
        <Route path="/teacher" element={<ComingSoonPage title="Teacher setup & login" phase="Phase 3 (Authentication)" />} />
        <Route path="/student" element={<ComingSoonPage title="Join an exam" phase="Phase 6 (Student Client)" />} />
        <Route path="*" element={<WelcomePage />} />
      </Routes>
    </HashRouter>
  );
}
