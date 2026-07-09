"use client";

import Link from "next/link";
import { useEffect, useRef, useState } from "react";
import {
  loginLocal,
  loginMacvlan,
  loginRandom,
  type ApiError,
  type LoginResult,
  type RandomLoginSummary,
} from "@/lib/api";
import {
  normalizeMacAddress,
  normalizeServerPath,
  parseLoginCount,
  validateUsername,
} from "@/lib/validation";
import { InterfaceSelect } from "@/components/interface-select";
import { ResultTable } from "@/components/result-table";
import {
  Alert,
  Button,
  Card,
  PageHeader,
  SectionHeading,
  SegmentedControl,
  TextField,
} from "@/components/ui";

type Mode = "local" | "macvlan" | "random";

const modes = [
  { value: "local" as const, label: "Local" },
  { value: "macvlan" as const, label: "Macvlan" },
  { value: "random" as const, label: "Random batch" },
];

interface FieldErrors {
  username?: string;
  password?: string;
  macAddress?: string;
  count?: string;
}

export default function LoginPage() {
  const [mode, setMode] = useState<Mode>("local");
  const [iface, setIface] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [macAddress, setMacAddress] = useState("");
  const [useFile, setUseFile] = useState(false);
  const [userinfoPath, setUserinfoPath] = useState("userinfo.json");
  const [count, setCount] = useState("1");
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<LoginResult | null>(null);
  const [batchSummary, setBatchSummary] = useState<RandomLoginSummary | null>(null);
  const [error, setError] = useState<ApiError | null>(null);
  const [fieldErrors, setFieldErrors] = useState<FieldErrors>({});
  const controllerRef = useRef<AbortController | null>(null);

  useEffect(() => () => controllerRef.current?.abort(), []);

  const changeMode = (nextMode: Mode) => {
    controllerRef.current?.abort();
    setMode(nextMode);
    setResult(null);
    setBatchSummary(null);
    setError(null);
    setFieldErrors({});
  };

  const clearFieldError = (field: keyof FieldErrors) => {
    setFieldErrors((current) => (current[field] ? { ...current, [field]: undefined } : current));
  };

  const showActionError = (actionError: ApiError) => {
    setError(actionError);
    const fieldMap: Record<string, keyof FieldErrors | undefined> = {
      username: "username",
      password: "password",
      mac_address: "macAddress",
      count: "count",
    };
    const field = actionError.field ? fieldMap[actionError.field] : undefined;
    if (field) setFieldErrors((current) => ({ ...current, [field]: actionError.message }));
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!iface || loading) return;

    const nextErrors: FieldErrors = {};
    let normalizedMac: string | undefined;
    let parsedCount: number | undefined;
    let normalizedUsername: string | undefined;

    if (mode === "macvlan") {
      const mac = normalizeMacAddress(macAddress);
      normalizedMac = mac.value;
      nextErrors.macAddress = mac.error;
    }
    if (mode === "random") {
      const parsed = parseLoginCount(count);
      parsedCount = parsed.value;
      nextErrors.count = parsed.error;
    } else if (!useFile) {
      const user = validateUsername(username);
      normalizedUsername = user.value;
      nextErrors.username = user.error;
      if (!password) nextErrors.password = "Enter a password.";
    }

    setFieldErrors(nextErrors);
    if (Object.values(nextErrors).some(Boolean)) return;

    if (normalizedMac) setMacAddress(normalizedMac);
    if (normalizedUsername) setUsername(normalizedUsername);
    const path = normalizeServerPath(userinfoPath);
    const controller = new AbortController();
    controllerRef.current?.abort();
    controllerRef.current = controller;
    setLoading(true);
    setError(null);
    setResult(null);
    setBatchSummary(null);

    try {
      if (mode === "random") {
        const response = await loginRandom(iface, parsedCount!, path, { signal: controller.signal });
        if (controller.signal.aborted) return;
        if (response.ok) setBatchSummary(response.data);
        else showActionError(response.error);
        return;
      }

      const response =
        mode === "local"
          ? await loginLocal(
              iface,
              useFile ? undefined : normalizedUsername,
              useFile ? undefined : password,
              useFile ? path : undefined,
              { signal: controller.signal },
            )
          : await loginMacvlan(
              iface,
              normalizedMac!,
              useFile ? undefined : normalizedUsername,
              useFile ? undefined : password,
              useFile ? path : undefined,
              { signal: controller.signal },
            );

      if (controller.signal.aborted) return;
      if (response.ok) setResult(response.data);
      else showActionError(response.error);
    } finally {
      if (!controller.signal.aborted) setLoading(false);
    }
  };

  return (
    <div className="space-y-8 sm:space-y-10">
      <PageHeader
        eyebrow="Authentication"
        title="Connect to the network"
        description="Start a local session, use a specific macvlan identity, or run a controlled batch of random MAC logins."
      />

      <Card className="overflow-hidden">
        <div className="border-b border-white/10 p-5 sm:p-6">
          <SectionHeading
            title="Connection method"
            description="Choose how this session should reach the campus portal."
          />
          <div className="mt-4">
            <SegmentedControl
              legend="Connection method"
              value={mode}
              options={modes}
              onChange={changeMode}
              disabled={loading}
            />
          </div>
        </div>

        <form onSubmit={handleSubmit} noValidate className="space-y-6 p-5 sm:p-6">
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
                clearFieldError("macAddress");
              }}
              onBlur={() => {
                if (macAddress) {
                  setFieldErrors((current) => ({
                    ...current,
                    macAddress: normalizeMacAddress(macAddress).error,
                  }));
                }
              }}
              autoComplete="off"
              spellCheck={false}
              disabled={loading}
              error={fieldErrors.macAddress}
              hint="Common colon, hyphen, and compact formats are accepted."
              mono
            />
          )}

          {mode === "random" ? (
            <div className="grid gap-5 sm:grid-cols-2">
              <TextField
                label="Connection count"
                name="count"
                type="number"
                min={1}
                max={100}
                step={1}
                value={count}
                onChange={(event) => {
                  setCount(event.target.value);
                  clearFieldError("count");
                }}
                onBlur={() => setFieldErrors((current) => ({ ...current, count: parseLoginCount(count).error }))}
                disabled={loading}
                error={fieldErrors.count}
                hint="Between 1 and 100 sequential attempts. Large batches can take several minutes."
                mono
              />
              <TextField
                label="User-info JSON path"
                name="userinfo-path"
                value={userinfoPath}
                onChange={(event) => setUserinfoPath(event.target.value)}
                placeholder="userinfo.json"
                disabled={loading}
                hint="A server-side file containing one JSON array of username/password objects."
                mono
              />
            </div>
          ) : (
            <div className="space-y-5">
              <label
                htmlFor="credential-source"
                className="flex cursor-pointer items-start gap-3 rounded-xl border border-white/10 bg-white/[0.025] p-4 transition hover:border-white/20"
              >
                <input
                  id="credential-source"
                  type="checkbox"
                  checked={useFile}
                  onChange={(event) => {
                    setUseFile(event.target.checked);
                    setFieldErrors({});
                  }}
                  disabled={loading}
                  className="peer sr-only"
                />
                <span
                  aria-hidden="true"
                  className="relative mt-0.5 h-6 w-10 shrink-0 rounded-full bg-neutral-700 transition after:absolute after:left-1 after:top-1 after:h-4 after:w-4 after:rounded-full after:bg-white after:transition-transform peer-checked:bg-sky-300 peer-checked:after:translate-x-4 peer-checked:after:bg-slate-950 peer-focus-visible:ring-2 peer-focus-visible:ring-sky-300 peer-focus-visible:ring-offset-2 peer-focus-visible:ring-offset-black peer-disabled:opacity-50"
                />
                <span>
                  <span className="block text-sm font-medium text-neutral-200">Use a server-side credential file</span>
                  <span className="mt-1 block text-xs leading-5 text-neutral-500">
                    The server selects an account from the configured JSON array instead of sending credentials from this form.
                  </span>
                </span>
              </label>

              {useFile ? (
                <TextField
                  label="User-info JSON path"
                  name="userinfo-path"
                  value={userinfoPath}
                  onChange={(event) => setUserinfoPath(event.target.value)}
                  placeholder="userinfo.json"
                  disabled={loading}
                  hint="Leave blank to use the backend's configured default file."
                  mono
                />
              ) : (
                <div className="grid gap-5 sm:grid-cols-2">
                  <TextField
                    label="Username"
                    name="username"
                    value={username}
                    onChange={(event) => {
                      setUsername(event.target.value);
                      clearFieldError("username");
                    }}
                    autoComplete="username"
                    placeholder="Campus username"
                    disabled={loading}
                    error={fieldErrors.username}
                  />
                  <TextField
                    label="Password"
                    name="password"
                    type="password"
                    value={password}
                    onChange={(event) => {
                      setPassword(event.target.value);
                      clearFieldError("password");
                    }}
                    autoComplete="current-password"
                    placeholder="Campus password"
                    disabled={loading}
                    error={fieldErrors.password}
                  />
                </div>
              )}
            </div>
          )}

          <div className="flex flex-col gap-3 border-t border-white/10 pt-6 sm:flex-row sm:items-center sm:justify-between">
            <p className="text-xs leading-5 text-neutral-500">
              {mode === "random"
                ? "Attempts run sequentially and may stop early when account limits are reached."
                : "Credentials are sent only through the same-origin server proxy."}
            </p>
            <Button
              type="submit"
              variant="primary"
              busy={loading}
              disabled={!iface}
              className="w-full shrink-0 sm:w-auto"
            >
              {loading ? (mode === "random" ? "Running batch…" : "Connecting…") : mode === "random" ? "Start batch" : "Connect"}
            </Button>
          </div>
        </form>
      </Card>

      {result && (
        <Alert
          variant="success"
          title="Connection established"
          actions={
            <Link className="text-sm font-semibold underline decoration-white/30 underline-offset-4 hover:decoration-white" href="/">
              View connection status
            </Link>
          }
        >
          <dl className="mt-2 grid gap-2 sm:grid-cols-3">
            <div><dt className="text-xs opacity-60">IP address</dt><dd className="break-all font-mono">{result.ip}</dd></div>
            <div><dt className="text-xs opacity-60">User</dt><dd className="break-all">{result.username}</dd></div>
            {result.mac && <div><dt className="text-xs opacity-60">MAC address</dt><dd className="break-all font-mono">{result.mac}</dd></div>}
          </dl>
        </Alert>
      )}

      {error && (
        <Alert
          variant={error.code === "cleanup_failed_after_success" ? "warning" : "error"}
          title={
            error.code === "cleanup_failed_after_success"
              ? "Connected, cleanup incomplete"
              : "Connection failed"
          }
        >
          <p>{error.message}</p>
          <p className="mt-1 font-mono text-xs opacity-60">{error.code}</p>
        </Alert>
      )}

      <ResultTable summary={batchSummary} />
    </div>
  );
}
