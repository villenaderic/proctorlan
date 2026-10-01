import { ShieldCheck } from "lucide-react";
export const Logo = ({ size = 56 }: { size?: number }) => (
  <div className="flex items-center justify-center rounded-2xl bg-brand-600 text-white" style={{ width: size, height: size }} aria-hidden>
    <ShieldCheck style={{ width: size * 0.6, height: size * 0.6 }} />
  </div>
);
