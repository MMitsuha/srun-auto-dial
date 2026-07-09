"use client";

import { useCallback, useEffect, useId, useRef, useState } from "react";
import { getInterfaces, type ApiError, type InterfaceInfo } from "@/lib/api";
import { Button } from "@/components/ui";

interface Props {
  value: string;
  onChange: (value: string) => void;
  label?: string;
  disabled?: boolean;
}

type LoadState =
  | { status: "loading" }
  | { status: "ready"; interfaces: InterfaceInfo[] }
  | { status: "empty" }
  | { status: "error"; error: ApiError };

function preferredInterfaces(interfaces: InterfaceInfo[]) {
  const sorted = [...interfaces].sort((a, b) => a.index - b.index);
  return sorted.filter(({ name }) => name !== "lo" && name !== "srun");
}

export function InterfaceSelect({ value, onChange, label = "Network interface", disabled }: Props) {
  const id = useId();
  const [state, setState] = useState<LoadState>({ status: "loading" });
  const controllerRef = useRef<AbortController | null>(null);
  const valueRef = useRef(value);
  const onChangeRef = useRef(onChange);
  valueRef.current = value;
  onChangeRef.current = onChange;

  const load = useCallback(async () => {
    controllerRef.current?.abort();
    const controller = new AbortController();
    controllerRef.current = controller;
    setState({ status: "loading" });

    const result = await getInterfaces({ signal: controller.signal });
    if (controller.signal.aborted) return;
    if (!result.ok) {
      setState({ status: "error", error: result.error });
      return;
    }

    const interfaces = preferredInterfaces(result.data);
    if (interfaces.length === 0) {
      setState({ status: "empty" });
      if (valueRef.current) onChangeRef.current("");
      return;
    }

    setState({ status: "ready", interfaces });
    if (!interfaces.some((item) => item.name === valueRef.current)) {
      onChangeRef.current(interfaces[0].name);
    }
  }, []);

  useEffect(() => {
    void load();
    return () => controllerRef.current?.abort();
  }, [load]);

  const isLoading = state.status === "loading";
  const options = state.status === "ready" ? state.interfaces : [];
  const descriptionId = state.status === "error" || state.status === "empty" ? `${id}-message` : undefined;

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-3">
        <label htmlFor={id} className="text-sm font-medium text-neutral-200">
          {label}
        </label>
        {state.status === "error" && (
          <Button type="button" variant="ghost" className="min-h-8 px-2 py-1 text-xs" onClick={load}>
            Retry
          </Button>
        )}
      </div>
      <div className="relative">
        <select
          id={id}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          disabled={disabled || state.status !== "ready"}
          aria-busy={isLoading || undefined}
          aria-invalid={state.status === "error"}
          aria-describedby={descriptionId}
          className="min-h-11 w-full appearance-none rounded-xl border border-white/10 bg-black/30 px-3.5 py-2.5 pr-10 text-sm text-white outline-none transition hover:border-white/20 focus:border-sky-400/70 focus:ring-2 focus:ring-sky-400/20 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {isLoading && <option value="">Loading interfaces…</option>}
          {state.status === "error" && <option value="">Interfaces unavailable</option>}
          {state.status === "empty" && <option value="">No usable interfaces</option>}
          {options.map((item) => (
            <option key={item.index} value={item.name}>
              {item.name} · index {item.index}
            </option>
          ))}
        </select>
        <span aria-hidden="true" className="pointer-events-none absolute right-3.5 top-1/2 -translate-y-1/2 text-neutral-500">
          {isLoading ? "···" : "⌄"}
        </span>
      </div>
      {state.status === "error" && (
        <p id={descriptionId} role="alert" className="text-xs leading-5 text-red-300">
          {state.error.message}
        </p>
      )}
      {state.status === "empty" && (
        <p id={descriptionId} role="status" className="text-xs leading-5 text-amber-300">
          No usable network interfaces were reported by the server.
        </p>
      )}
    </div>
  );
}
