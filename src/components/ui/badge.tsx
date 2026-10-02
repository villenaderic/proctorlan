import type { HTMLAttributes } from "react";
import { cn } from "@/utils/cn";

const tones = {
  green: "bg-green-100 text-green-800 dark:bg-green-950 dark:text-green-300",
  gray: "bg-slate-200 text-slate-700 dark:bg-slate-700 dark:text-slate-200",
  red: "bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-300",
  blue: "bg-brand-50 text-brand-700 dark:bg-navy-800 dark:text-blue-300",
  amber: "bg-amber-100 text-amber-800 dark:bg-amber-950 dark:text-amber-300",
};

/** Status badge. Always carries text (and optionally an icon), never colour alone. */
export const Badge = ({ tone = "gray", className, ...p }: HTMLAttributes<HTMLSpanElement> & { tone?: keyof typeof tones }) => (
  <span className={cn("inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium", tones[tone], className)} {...p} />
);
