import { HashRouter, Route, Routes } from "react-router-dom";
import { WelcomePage } from "@/pages/WelcomePage";
import { StudentPage } from "@/features/student/StudentJoinPage";
import { TeacherGate } from "@/pages/teacher/TeacherGate";
import { DashboardPage } from "@/pages/teacher/DashboardPage";
import { SettingsPage } from "@/pages/teacher/SettingsPage";
import { ExamsPage } from "@/features/exams/ExamsPage";
import { ExamBuilderPage } from "@/features/exams/ExamBuilderPage";
import { ExamPreviewPage } from "@/features/exams/ExamPreviewPage";
import { SessionsPage } from "@/features/sessions/SessionsPage";
import { SessionLivePage } from "@/features/sessions/SessionLivePage";
import { ResultsPage } from "@/features/results/ResultsPage";
import { SessionResultsPage } from "@/features/results/SessionResultsPage";
import { AttemptDetailPage } from "@/features/results/AttemptDetailPage";
import { StudentsPage } from "@/features/results/StudentsPage";
import { BackupsPage } from "@/features/backups/BackupsPage";

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
          <Route path="sessions" element={<SessionsPage />} />
          <Route path="sessions/:id" element={<SessionLivePage />} />
          <Route path="students" element={<StudentsPage />} />
          <Route path="results" element={<ResultsPage />} />
          <Route path="results/attempt/:attemptId" element={<AttemptDetailPage />} />
          <Route path="results/:id" element={<SessionResultsPage />} />
          <Route path="backups" element={<BackupsPage />} />
          <Route path="settings" element={<SettingsPage />} />
        </Route>
        <Route path="/student" element={<StudentPage />} />
        <Route path="*" element={<WelcomePage />} />
      </Routes>
    </HashRouter>
  );
}
