"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { getStatus, logoutLocal, type StatusResult } from "@/lib/api";
import { InterfaceSelect } from "@/components/interface-select";
import { StatusCard } from "@/components/status-card";

export default function Dashboard() {
  const [iface, setIface] = useState("");
  const [status, setStatus] = useState<StatusResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [logoutLoading, setLogoutLoading] = useState(false);

  const fetchStatus = useCallback(async () => {
    if (!iface) return;
    setLoading(true);
    setError(null);
    const res = await getStatus(iface);
    if (res.success && res.data) {
      setStatus(res.data);
    } else {
      setError(res.error || "Failed to fetch status");
    }
    setLoading(false);
  }, [iface]);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  const handleLogout = async () => {
    if (!iface) return;
    setLogoutLoading(true);
    const res = await logoutLocal(iface);
    setLogoutLoading(false);
    if (res.success) {
      fetchStatus();
    } else {
      setError(res.error || "Logout failed");
    }
  };

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Dashboard</h1>
        <p className="mt-2 text-sm text-neutral-400">
          Monitor and manage your campus network connection.
        </p>
      </div>

      <InterfaceSelect value={iface} onChange={setIface} />

      <StatusCard status={status} loading={loading} error={error} />

      <div className="flex gap-3">
        <Link
          href="/login"
          className="rounded-lg bg-white px-4 py-2.5 text-sm font-medium text-black transition-colors hover:bg-neutral-200"
        >
          Login
        </Link>
        <button
          onClick={handleLogout}
          disabled={logoutLoading || !status?.online_user}
          className="rounded-lg border border-neutral-700 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:border-neutral-500 hover:bg-neutral-900 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {logoutLoading ? "Logging out..." : "Logout"}
        </button>
        <button
          onClick={fetchStatus}
          disabled={loading}
          className="rounded-lg border border-neutral-700 px-4 py-2.5 text-sm font-medium text-white transition-colors hover:border-neutral-500 hover:bg-neutral-900 disabled:opacity-40"
        >
          Refresh
        </button>
      </div>
    </div>
  );
}
