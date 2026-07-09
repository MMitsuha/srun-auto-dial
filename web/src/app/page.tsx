"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { getStatus, getStatusMacvlan, type ApiError, type StatusResult } from "@/lib/api";
import { normalizeMacAddress } from "@/lib/validation";
import { InterfaceSelect } from "@/components/interface-select";
import { StatusCard } from "@/components/status-card";
import { Button, Card, PageHeader, SectionHeading, SegmentedControl, TextField } from "@/components/ui";

type Mode = "local" | "macvlan";

const modes = [
  { value: "local" as const, label: "Local" },
  { value: "macvlan" as const, label: "Macvlan" },
];

export default function Dashboard() {
  const [mode, setMode] = useState<Mode>("local");
  const [iface, setIface] = useState("");
  const [macAddress, setMacAddress] = useState("");
  const [macError, setMacError] = useState<string>();
  const [status, setStatus] = useState<StatusResult | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);
  const activeRequest = useRef<AbortController | null>(null);
  const requestSequence = useRef(0);

  const runLocalStatus = useCallback(async () => {
    if (!iface) return;
    activeRequest.current?.abort();
    const controller = new AbortController();
    activeRequest.current = controller;
    const sequence = ++requestSequence.current;
    setLoading(true);
    setError(null);

    try {
      const result = await getStatus(iface, { signal: controller.signal });
      if (controller.signal.aborted || sequence !== requestSequence.current) return;
      if (result.ok) {
        setStatus(result.data);
        setLastUpdated(new Date());
      } else {
        setStatus(null);
        setError(result.error);
      }
    } finally {
      if (sequence === requestSequence.current) setLoading(false);
    }
  }, [iface]);

  useEffect(() => {
    if (mode === "local" && iface) void runLocalStatus();
    return () => activeRequest.current?.abort();
  }, [iface, mode, runLocalStatus]);

  useEffect(() => () => activeRequest.current?.abort(), []);

  const changeMode = (nextMode: Mode) => {
    activeRequest.current?.abort();
    requestSequence.current += 1;
    setMode(nextMode);
    setStatus(null);
    setError(null);
    setMacError(undefined);
    setLastUpdated(null);
    setLoading(false);
  };

  const checkMacvlanStatus = async (event: React.FormEvent) => {
    event.preventDefault();
    const normalized = normalizeMacAddress(macAddress);
    if (!normalized.value) {
      setMacError(normalized.error);
      return;
    }
    if (!iface) return;

    setMacAddress(normalized.value);
    setMacError(undefined);
    activeRequest.current?.abort();
    const controller = new AbortController();
    activeRequest.current = controller;
    const sequence = ++requestSequence.current;
    setLoading(true);
    setError(null);

    try {
      const result = await getStatusMacvlan(iface, normalized.value, { signal: controller.signal });
      if (controller.signal.aborted || sequence !== requestSequence.current) return;
      if (result.ok) {
        setStatus(result.data);
        setLastUpdated(new Date());
      } else {
        setStatus(null);
        setError(result.error);
        if (result.error.field === "mac_address") setMacError(result.error.message);
      }
    } finally {
      if (sequence === requestSequence.current) setLoading(false);
    }
  };

  return (
    <div className="space-y-8 sm:space-y-10">
      <PageHeader
        eyebrow="Network overview"
        title="Connection dashboard"
        description="Inspect the active campus-network session through a local interface or a specific macvlan identity."
      />

      <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,0.8fr)_minmax(0,1.2fr)]">
        <Card className="p-5 sm:p-6">
          <div className="space-y-6">
            <SectionHeading
              title="Status source"
              description={
                mode === "local"
                  ? "Local status refreshes when the selected interface changes."
                  : "Macvlan checks run only after you submit a complete MAC address."
              }
            />
            <SegmentedControl
              legend="Status mode"
              value={mode}
              options={modes}
              onChange={changeMode}
              disabled={loading}
            />

            {mode === "local" ? (
              <div className="space-y-5">
                <InterfaceSelect value={iface} onChange={setIface} disabled={loading} />
                <Button
                  type="button"
                  variant="secondary"
                  busy={loading}
                  disabled={!iface}
                  onClick={() => void runLocalStatus()}
                  className="w-full sm:w-auto"
                >
                  {loading ? "Checking…" : "Refresh status"}
                </Button>
              </div>
            ) : (
              <form onSubmit={checkMacvlanStatus} noValidate className="space-y-5">
                <InterfaceSelect
                  value={iface}
                  onChange={setIface}
                  label="Parent interface"
                  disabled={loading}
                />
                <TextField
                  label="MAC address"
                  name="mac-address"
                  value={macAddress}
                  onChange={(event) => {
                    setMacAddress(event.target.value);
                    if (macError) setMacError(undefined);
                  }}
                  onBlur={() => {
                    if (macAddress) setMacError(normalizeMacAddress(macAddress).error);
                  }}
                  placeholder="AA:BB:CC:DD:EE:FF"
                  autoComplete="off"
                  spellCheck={false}
                  disabled={loading}
                  error={macError}
                  hint="Common colon, hyphen, and compact formats are accepted."
                  mono
                />
                <Button
                  type="submit"
                  variant="primary"
                  busy={loading}
                  disabled={!iface || !macAddress.trim()}
                  className="w-full sm:w-auto"
                >
                  {loading ? "Checking…" : "Check macvlan status"}
                </Button>
              </form>
            )}
          </div>
        </Card>

        <StatusCard status={status} loading={loading} error={error} lastUpdated={lastUpdated} />
      </div>
    </div>
  );
}
