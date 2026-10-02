import { HashRouter, Route, Routes } from "react-router-dom";
import { WelcomePage } from "@/pages/WelcomePage";
import { ComingSoonPage } from "@/pages/ComingSoonPage";
import { TeacherGate } from "@/pages/teacher/TeacherGate";
import { DashboardPage } from "@/pages/teacher/DashboardPage";
import { SettingsPage } from "@/pages/teacher/SettingsPage";
import { SectionPlaceholder } from "@/pages/teacher/SectionPlaceholder";

// HashRouter: works from Tauri's bundled assets without server-side routing.
export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route path="/" element={<WelcomePage />} />
        <Route path="/teacher" element={<TeacherGate />}>
          <Route index element={<DashboardPage />} />
          <Route path="exams" element={<SectionPlaceholder title="Exams" phase="Phase 4 (Exam Builder)" />} />
          <Route path="sessions" element={<SectionPlaceholder title="Sessions" phase="Phase 5 (LAN Server)" />} />
          <Route path="students" element={<SectionPlaceholder title="Students" phase="Phase 10 (Results)" />} />
          <Route path="results" element={<SectionPlaceholder title="Results" phase="Phase 10 (Results)" />} />
          <Route path="backups" element={<SectionPlaceholder title="Backups" phase="Phase 11 (Backup/Export)" />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
        <Route path="/student" element={<ComingSoonPage title="Join an exam" phase="Phase 6 (Student Client)" />} />
        <Route path="*" element={<WelcomePage />} />
      </Routes>
    </HashRouter>
  );
}
