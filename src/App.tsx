import { HashRouter, Route, Routes } from "react-router-dom";
import { WelcomePage } from "@/pages/WelcomePage";
import { ComingSoonPage } from "@/pages/ComingSoonPage";
import { TeacherGate } from "@/pages/teacher/TeacherGate";
import { DashboardPage } from "@/pages/teacher/DashboardPage";
import { SettingsPage } from "@/pages/teacher/SettingsPage";
import { ExamsPage } from "@/features/exams/ExamsPage";
import { ExamBuilderPage } from "@/features/exams/ExamBuilderPage";
import { ExamPreviewPage } from "@/features/exams/ExamPreviewPage";
import { SectionPlaceholder } from "@/pages/teacher/SectionPlaceholder";

// HashRouter: works from Tauri's bundled assets without server-side routing.
export default function App() {
  return (
    <HashRouter>
      <Routes>
        <Route path="/" element={<WelcomePage />} />
        <Route path="/teacher" element={<TeacherGate />}>
          <Route index element={<DashboardPage />} />
          <Route path="exams" element={<ExamsPage />} />
          <Route path="exams/new" element={<ExamBuilderPage />} />
          <Route path="exams/:id/edit" element={<ExamBuilderPage />} />
          <Route path="exams/:id/preview" element={<ExamPreviewPage />} />
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
