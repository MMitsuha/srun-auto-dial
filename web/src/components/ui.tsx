"use client";

import {
  forwardRef,
  useId,
  type ButtonHTMLAttributes,
  type InputHTMLAttributes,
  type ReactNode,
} from "react";

function cx(...classes: Array<string | false | null | undefined>) {
  return classes.filter(Boolean).join(" ");
}

export function PageHeader({
  eyebrow,
  title,
  description,
  actions,
}: {
  eyebrow?: string;
  title: string;
  description: string;
  actions?: ReactNode;
}) {
  return (
    <header className="flex flex-col gap-5 sm:flex-row sm:items-end sm:justify-between">
      <div className="max-w-2xl">
        {eyebrow && (
          <p className="mb-2 text-xs font-semibold uppercase tracking-[0.18em] text-sky-400">
            {eyebrow}
          </p>
        )}
        <h1 className="text-3xl font-semibold tracking-[-0.035em] text-white sm:text-4xl">
          {title}
        </h1>
        <p className="mt-3 max-w-xl text-sm leading-6 text-neutral-400 sm:text-base">
          {description}
        </p>
      </div>
      {actions}
    </header>
  );
}

export function Card({
  children,
  className,
  as = "section",
}: {
  children: ReactNode;
  className?: string;
  as?: "section" | "div";
}) {
  const Component = as;
  return (
    <Component
      className={cx(
        "rounded-2xl border border-white/10 bg-neutral-950/75 shadow-[0_20px_70px_-36px_rgba(56,189,248,0.25)] backdrop-blur",
        className,
      )}
    >
      {children}
    </Component>
  );
}

type AlertVariant = "error" | "success" | "info" | "warning";

const alertStyles: Record<AlertVariant, string> = {
  error: "border-red-500/25 bg-red-500/10 text-red-100",
  success: "border-emerald-500/25 bg-emerald-500/10 text-emerald-100",
  info: "border-sky-500/25 bg-sky-500/10 text-sky-100",
  warning: "border-amber-500/25 bg-amber-500/10 text-amber-100",
};

export function Alert({
  variant,
  title,
  children,
  actions,
}: {
  variant: AlertVariant;
  title: string;
  children?: ReactNode;
  actions?: ReactNode;
}) {
  const isError = variant === "error";
  return (
    <div
      role={isError ? "alert" : "status"}
      aria-live={isError ? "assertive" : "polite"}
      className={cx("rounded-xl border px-4 py-3.5", alertStyles[variant])}
    >
      <div className="flex gap-3">
        <span aria-hidden="true" className="mt-0.5 text-base">
          {variant === "success" ? "✓" : variant === "error" ? "!" : variant === "warning" ? "△" : "i"}
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-sm font-semibold">{title}</p>
          {children && <div className="mt-1 text-sm leading-5 opacity-80">{children}</div>}
          {actions && <div className="mt-3">{actions}</div>}
        </div>
      </div>
    </div>
  );
}

type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";

const buttonStyles: Record<ButtonVariant, string> = {
  primary:
    "border-sky-300/70 bg-sky-300 text-slate-950 shadow-[0_10px_30px_-12px_rgba(125,211,252,0.8)] hover:bg-sky-200",
  secondary: "border-white/15 bg-white/[0.04] text-neutral-100 hover:border-white/25 hover:bg-white/[0.08]",
  danger: "border-red-400/30 bg-red-500/15 text-red-100 hover:border-red-400/50 hover:bg-red-500/25",
  ghost: "border-transparent bg-transparent text-neutral-300 hover:bg-white/[0.06] hover:text-white",
};

export const Button = forwardRef<
  HTMLButtonElement,
  ButtonHTMLAttributes<HTMLButtonElement> & { variant?: ButtonVariant; busy?: boolean }
>(function Button({ variant = "secondary", busy = false, className, children, disabled, ...props }, ref) {
  return (
    <button
      ref={ref}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      className={cx(
        "inline-flex min-h-11 items-center justify-center gap-2 rounded-xl border px-4 py-2.5 text-sm font-semibold transition focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-300 focus-visible:ring-offset-2 focus-visible:ring-offset-black disabled:cursor-not-allowed disabled:opacity-45",
        buttonStyles[variant],
        className,
      )}
      {...props}
    >
      {busy && (
        <span
          aria-hidden="true"
          className="h-4 w-4 animate-spin rounded-full border-2 border-current border-r-transparent"
        />
      )}
      {children}
    </button>
  );
});

export function TextField({
  label,
  hint,
  error,
  mono,
  className,
  id: suppliedId,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & {
  label: string;
  hint?: string;
  error?: string;
  mono?: boolean;
}) {
  const generatedId = useId();
  const id = suppliedId || generatedId;
  const descriptionId = hint || error ? `${id}-description` : undefined;
  return (
    <div className={cx("space-y-2", className)}>
      <label htmlFor={id} className="block text-sm font-medium text-neutral-200">
        {label}
      </label>
      <input
        id={id}
        aria-invalid={Boolean(error)}
        aria-describedby={descriptionId}
        className={cx(
          "min-h-11 w-full rounded-xl border bg-black/30 px-3.5 py-2.5 text-sm text-white outline-none transition placeholder:text-neutral-600 focus:border-sky-400/70 focus:ring-2 focus:ring-sky-400/20 disabled:cursor-not-allowed disabled:opacity-50",
          error ? "border-red-400/60" : "border-white/10 hover:border-white/20",
          mono && "font-[family-name:var(--font-geist-mono)]",
        )}
        {...props}
      />
      {(error || hint) && (
        <p id={descriptionId} className={cx("text-xs leading-5", error ? "text-red-300" : "text-neutral-500")}>
          {error || hint}
        </p>
      )}
    </div>
  );
}

export function SegmentedControl<T extends string>({
  legend,
  value,
  options,
  onChange,
  disabled,
}: {
  legend: string;
  value: T;
  options: ReadonlyArray<{ value: T; label: string; description?: string }>;
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  const name = useId();
  return (
    <fieldset disabled={disabled}>
      <legend className="sr-only">{legend}</legend>
      <div className="inline-flex max-w-full gap-1 overflow-x-auto rounded-xl border border-white/10 bg-black/30 p-1">
        {options.map((option) => (
          <label key={option.value} className="relative shrink-0 cursor-pointer">
            <input
              className="peer sr-only"
              type="radio"
              name={name}
              value={option.value}
              checked={value === option.value}
              onChange={() => onChange(option.value)}
            />
            <span className="flex min-h-10 items-center rounded-lg px-3.5 text-sm font-medium text-neutral-400 transition peer-checked:bg-white/10 peer-checked:text-white peer-focus-visible:ring-2 peer-focus-visible:ring-sky-300 peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-black peer-disabled:cursor-not-allowed peer-disabled:opacity-45">
              {option.label}
            </span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}

export function SectionHeading({ title, description }: { title: string; description?: string }) {
  return (
    <div>
      <h2 className="text-base font-semibold text-white">{title}</h2>
      {description && <p className="mt-1 text-sm leading-5 text-neutral-500">{description}</p>}
    </div>
  );
}
