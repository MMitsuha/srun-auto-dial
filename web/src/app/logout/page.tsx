"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import { logoutLocal, logoutMacvlan, type ApiError } from "@/lib/api";
import { normalizeMacAddress } from "@/lib/validation";
import { InterfaceSelect } from "@/components/interface-select";
import {
  Alert,
  Button,
  Card,
  PageHeader,
  SectionHeading,
  SegmentedControl,
  TextField,
} from "@/components/ui";

type Mode = "local" | "macvlan";

const modes = [
  { value: "local" as const, label: "Local" },
  { value: "macvlan" as const, label: "Macvlan" },
];

export default function LogoutPage() {
  const [mode, setMode] = useState<Mode>("local");
  const [iface, setIface] = useState("");
  const [macAddress, setMacAddress] = useState("");
  const [macError, setMacError] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [success, setSuccess] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const controllerRef = useRef<AbortController | null>(null);

  useEffect(() => () => controllerRef.current?.abort(), []);

  const changeMode = (nextMode: Mode) => {
    controllerRef.current?.abort();
    setMode(nextMode);
    setSuccess(false);
    setError(null);
    setMacError(undefined);
  };

  const handleLogout = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!iface || loading) return;

    let normalizedMac: string | undefined;
    if (mode === "macvlan") {
      const mac = normalizeMacAddress(macAddress);
      if (!mac.value) {
        setMacError(mac.error);
        return;
      }
      normalizedMac = mac.value;
      setMacAddress(mac.value);
      setMacError(undefined);
    }

    controllerRef.current?.abort();
    const controller = new AbortController();
    controllerRef.current = controller;
    setLoading(true);
    setError(null);
    setSuccess(false);

    try {
      const response =
        mode === "local"
          ? await logoutLocal(iface, { signal: controller.signal })
          : await logoutMacvlan(iface, normalizedMac!, { signal: controller.signal });
      if (controller.signal.aborted) return;
      if (response.ok) setSuccess(true);
      else {
        setError(response.error);
        if (response.error.field === "mac_address") setMacError(response.error.message);
      }
    } finally {
      if (!controller.signal.aborted) setLoading(false);
    }
  };

  return (
    <div className="space-y-8 sm:space-y-10">
      <PageHeader
        eyebrow="Session control"
        title="Disconnect a session"
        description="End the active campus-network session on a local interface or through a specific macvlan identity."
      />

      <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,1fr)_320px]">
        <Card className="overflow-hidden">
          <div className="border-b border-white/10 p-5 sm:p-6">
            <SectionHeading
              title="Session source"
              description="Select the same interface identity that was used to connect."
            />
            <div className="mt-4">
              <SegmentedControl
                legend="Disconnect mode"
                value={mode}
                options={modes}
                onChange={changeMode}
                disabled={loading}
              />
            </div>
          </div>

          <form onSubmit={handleLogout} noValidate className="space-y-6 p-5 sm:p-6">
            <InterfaceSelect
              value={iface}
              onChange={setIface}
              label={mode === "local" ? "Network interface" : "Parent interface"}
              disabled={loading}
            />

            {mode === "macvlan" && (
              <TextField
                label="MAC address"
                name="mac-address"
                placeholder="AA:BB:CC:DD:EE:FF"
                value={macAddress}
                onChange={(event) => {
                  setMacAddress(event.target.value);
                  if (macError) setMacError(undefined);
                }}
                onBlur={() => {
                  if (macAddress) setMacError(normalizeMacAddress(macAddress).error);
                }}
                autoComplete="off"
                spellCheck={false}
                disabled={loading}
                error={macError}
                hint="Use the MAC address associated with the session you want to end."
                mono
              />
            )}

            <div className="flex flex-col gap-3 border-t border-white/10 pt-6 sm:flex-row sm:items-center sm:justify-between">
              <p className="text-xs leading-5 text-neutral-500">
                Disconnecting ends the current portal session but does not change saved credentials.
              </p>
              <Button
                type="submit"
                variant="danger"
                busy={loading}
                disabled={!iface || (mode === "macvlan" && !macAddress.trim())}
                className="w-full shrink-0 sm:w-auto"
              >
                {loading ? "Disconnecting…" : "Disconnect session"}
              </Button>
            </div>
          </form>
        </Card>

        <Card className="p-5 sm:p-6" as="div">
          <p className="text-sm font-semibold text-white">Before disconnecting</p>
          <ul className="mt-3 space-y-3 text-sm leading-5 text-neutral-400">
            <li className="flex gap-2"><span aria-hidden="true" className="text-sky-300">•</span> Verify the selected interface.</li>
            <li className="flex gap-2"><span aria-hidden="true" className="text-sky-300">•</span> Macvlan sessions require the exact MAC identity.</li>
            <li className="flex gap-2"><span aria-hidden="true" className="text-sky-300">•</span> “No user online” means the session is already inactive.</li>
          </ul>
        </Card>
      </div>

      {success && (
        <Alert
          variant="success"
          title="Session disconnected"
          actions={
            <Link className="text-sm font-semibold underline decoration-white/30 underline-offset-4 hover:decoration-white" href="/">
              Return to dashboard
            </Link>
          }
        >
          The selected campus-network session ended successfully.
        </Alert>
      )}

      {error && (
        <Alert
          variant={error.code === "cleanup_failed_after_success" ? "warning" : "error"}
          title={
            error.code === "cleanup_failed_after_success"
              ? "Disconnected, cleanup incomplete"
              : "Disconnect failed"
          }
        >
          <p>{error.message}</p>
          <p className="mt-1 font-mono text-xs opacity-60">{error.code}</p>
        </Alert>
      )}
    </div>
  );
}
