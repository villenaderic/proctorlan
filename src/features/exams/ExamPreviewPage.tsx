import { ArrowLeft } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { toMessage } from "@/services/auth";
import { examsApi } from "@/services/exams";
import type { ExamDraft } from "@/types/exam";
import { fromExam } from "./draft";
import { ExamPreview } from "./ExamPreview";

export function ExamPreviewPage() {
  const { id = "" } = useParams();
  const [draft, setDraft] = useState<ExamDraft | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { examsApi.get(id).then((e) => setDraft(fromExam(e))).catch((e) => setError(toMessage(e))); }, [id]);

  return (
    <div className="space-y-4">
      <Link to="/teacher/exams" className="inline-flex items-center gap-1 text-sm text-slate-600 hover:underline dark:text-slate-300"><ArrowLeft className="h-4 w-4" aria-hidden /> Back to exams</Link>
      <h1 className="text-2xl font-semibold">Exam preview</h1>
      {error ? <p role="alert" className="text-red-600">{error}</p> : draft ? <ExamPreview draft={draft} /> : <p className="text-slate-500" role="status">Loading…</p>}
    </div>
  );
}
