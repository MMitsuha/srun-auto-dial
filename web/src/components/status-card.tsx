import type { StatusResult } from "@/lib/api";

interface Props {
  status: StatusResult | null;
  loading: boolean;
  error: string | null;
}

export function StatusCard({ status, loading, error }: Props) {
  if (loading) {
    return (
      <div className="rounded-xl border border-neutral-800 bg-neutral-900/50 p-6">
        <div className="flex items-center gap-3">
          <div className="h-3 w-3 animate-pulse rounded-full bg-neutral-600" />
          <span className="text-sm text-neutral-400">Loading status...</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-xl border border-red-900/50 bg-red-950/20 p-6">
        <p className="text-sm text-red-400">{error}</p>
      </div>
    );
  }

  if (!status) return null;

  const isOnline = !!status.online_user;

  return (
    <div className="rounded-xl border border-neutral-800 bg-neutral-900/50 p-6">
      <div className="flex items-center gap-3 mb-6">
        <div
          className={`h-3 w-3 rounded-full ${
            isOnline ? "bg-green-500 shadow-[0_0_8px_rgba(0,200,83,0.5)]" : "bg-neutral-600"
          }`}
        />
        <span className="text-sm font-medium">
          {isOnline ? "Online" : "Offline"}
        </span>
      </div>

      <div className="grid gap-4">
        <Field label="IP Address" value={status.ip} mono />
        <Field
          label="User"
          value={status.online_user || "-"}
        />
        <Field
          label="MAC Address"
          value={status.online_mac || "-"}
          mono
        />
      </div>
    </div>
  );
}

function Field({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-sm text-neutral-400">{label}</span>
      <span
        className={`text-sm ${
          mono ? "font-[family-name:var(--font-geist-mono)]" : ""
        } ${value === "-" ? "text-neutral-600" : "text-white"}`}
      >
        {value}
      </span>
    </div>
  );
}
