import { forwardRef, type InputHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input
      ref={ref}
      className={cn(
        "h-9 w-full rounded-md border border-line bg-canvas px-3 text-sm text-ink outline-none transition-colors",
        "placeholder:text-muted/70 focus:border-brand/50 focus:ring-2 focus:ring-brand/25",
        className,
      )}
      {...props}
    />
  ),
);
Input.displayName = "Input";
