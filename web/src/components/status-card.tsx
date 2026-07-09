import type { ApiError, StatusResult } from "@/lib/api";
import { Alert, Card } from "@/components/ui";

interface Props {
  status: StatusResult | null;
  loading: boolean;
  error: ApiError | null;
  lastUpdated?: Date | null;
}

export function StatusCard({ status, loading, error, lastUpdated }: Props) {
  if (loading) {
    return (
      <Card className="p-5 sm:p-6" as="div">
        <div role="status" aria-live="polite" className="animate-pulse space-y-5">
          <span className="sr-only">Checking connection status…</span>
          <div className="h-6 w-36 rounded bg-white/10" />
          <div className="grid gap-3 sm:grid-cols-3">
            {[0, 1, 2].map((item) => (
              <div key={item} className="h-20 rounded-xl bg-white/[0.05]" />
            ))}
          </div>
        </div>
      </Card>
    );
  }

  if (error) {
    const cleanupIncomplete = error.code === "cleanup_failed_after_success";
    return (
      <Alert
        variant={cleanupIncomplete ? "warning" : "error"}
        title={cleanupIncomplete ? "Status retrieved, cleanup incomplete" : "Status check failed"}
      >
        <p>{error.message}</p>
        <p className="mt-1 font-mono text-xs opacity-70">{error.code}</p>
      </Alert>
    );
  }

  if (!status) {
    return (
      <Card className="grid min-h-48 place-items-center p-6 text-center" as="div">
        <div className="max-w-sm">
          <div aria-hidden="true" className="mx-auto grid h-11 w-11 place-items-center rounded-xl bg-white/[0.06] text-neutral-400">
            ↻
          </div>
          <p className="mt-4 text-sm font-medium text-neutral-200">No status checked yet</p>
          <p className="mt-1 text-sm leading-5 text-neutral-500">
            Choose an interface and check its current campus-network session.
          </p>
        </div>
      </Card>
    );
  }

  const isOnline = Boolean(status.online_user);
  return (
    <Card className="overflow-hidden" as="div">
      <div className="flex flex-wrap items-center justify-between gap-3 border-b border-white/10 px-5 py-4 sm:px-6">
        <div className="flex items-center gap-3">
          <span
            aria-hidden="true"
            className={`h-2.5 w-2.5 rounded-full ${
              isOnline
                ? "bg-emerald-400 shadow-[0_0_14px_rgba(52,211,153,0.8)]"
                : "bg-neutral-600"
            }`}
          />
          <div>
            <p className="text-sm font-semibold text-white">{isOnline ? "Online" : "Offline"}</p>
            <p className="text-xs text-neutral-500">Campus network session</p>
          </div>
        </div>
        {lastUpdated && (
          <time dateTime={lastUpdated.toISOString()} className="text-xs text-neutral-500">
            Checked {lastUpdated.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
          </time>
        )}
      </div>
      <dl className="grid gap-px bg-white/10 sm:grid-cols-3">
        <StatusField label="IP address" value={status.ip} mono />
        <StatusField label="User" value={status.online_user || "Not connected"} muted={!status.online_user} />
        <StatusField label="MAC address" value={status.online_mac || "Not available"} mono muted={!status.online_mac} />
      </dl>
    </Card>
  );
}

function StatusField({
  label,
  value,
  mono,
  muted,
}: {
  label: string;
  value: string;
  mono?: boolean;
  muted?: boolean;
}) {
  return (
    <div className="min-w-0 bg-neutral-950/90 px-5 py-4 sm:px-6">
      <dt className="text-xs font-medium uppercase tracking-[0.12em] text-neutral-500">{label}</dt>
      <dd
        className={`mt-2 break-all text-sm ${mono ? "font-[family-name:var(--font-geist-mono)]" : ""} ${
          muted ? "text-neutral-500" : "text-neutral-100"
        }`}
      >
        {value}
      </dd>
    </div>
  );
}
