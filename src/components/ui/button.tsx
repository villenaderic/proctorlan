import { cva, type VariantProps } from "class-variance-authority";
import type { ButtonHTMLAttributes } from "react";
import { cn } from "@/utils/cn";

const button = cva(
  "inline-flex items-center justify-center gap-2 rounded-md text-sm font-medium transition-colors disabled:pointer-events-none disabled:opacity-50",
  {
    variants: {
      variant: {
        primary: "bg-brand-600 text-white hover:bg-brand-700",
        outline: "border border-slate-300 bg-white hover:bg-slate-100 dark:border-slate-600 dark:bg-transparent dark:hover:bg-navy-800",
        ghost: "hover:bg-slate-200/70 dark:hover:bg-navy-800",
      },
      size: { md: "h-10 px-4", sm: "h-8 px-3", icon: "h-9 w-9" },
    },
    defaultVariants: { variant: "primary", size: "md" },
  },
);

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement>, VariantProps<typeof button> {}

export const Button = ({ className, variant, size, ...props }: ButtonProps) => (
  <button className={cn(button({ variant, size }), className)} {...props} />
);
