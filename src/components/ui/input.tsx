import type { InputHTMLAttributes } from "react";
import { cn } from "@/utils/cn";

export const Input = ({ className, ...p }: InputHTMLAttributes<HTMLInputElement>) => (
  <input
    className={cn(
      "h-10 w-full rounded-md border border-slate-300 bg-white px-3 text-sm placeholder:text-slate-400 disabled:opacity-60 dark:border-slate-600 dark:bg-navy-950",
      className,
    )}
    {...p}
  />
);
