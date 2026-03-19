"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { loginLocal, loginMacvlan, type LoginResult } from "@/lib/api";
import { InterfaceSelect } from "@/components/interface-select";

type Mode = "local" | "macvlan";

export default function LoginPage() {
  const router = useRouter();
  const [mode, setMode] = useState<Mode>("local");
  const [iface, setIface] = useState("");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [macAddress, setMacAddress] = useState("");
  const [useFile, setUseFile] = useState(false);
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<LoginResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setError(null);
    setResult(null);

    const u = useFile ? undefined : username;
    const p = useFile ? undefined : password;

    const res =
      mode === "local"
        ? await loginLocal(iface, u, p)
        : await loginMacvlan(iface, macAddress, u, p);

    setLoading(false);
    if (res.success && res.data) {
      setResult(res.data);
    } else {
      setError(res.error || "Login failed");
    }
  };

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">Login</h1>
        <p className="mt-2 text-sm text-neutral-400">
          Authenticate to the campus network.
        </p>
      </div>

      {/* Mode toggle */}
      <div className="flex gap-1 rounded-lg border border-neutral-800 bg-neutral-900/50 p-1 w-fit">
        {(["local", "macvlan"] as Mode[]).map((m) => (
          <button
            key={m}
            onClick={() => setMode(m)}
            className={`rounded-md px-4 py-2 text-sm font-medium transition-colors ${
              mode === m
                ? "bg-neutral-800 text-white"
                : "text-neutral-400 hover:text-white"
            }`}
          >
            {m === "local" ? "Local" : "Macvlan"}
          </button>
        ))}
      </div>

      <form onSubmit={handleSubmit} className="space-y-5">
        <InterfaceSelect
          value={iface}
          onChange={setIface}
          label={mode === "local" ? "Network Interface" : "Parent Interface"}
        />

        {mode === "macvlan" && (
          <InputField
            label="MAC Address"
            placeholder="AA:BB:CC:DD:EE:FF"
            value={macAddress}
            onChange={setMacAddress}
            mono
          />
        )}

        {/* Credential source toggle */}
        <div className="flex items-center gap-3">
          <label className="relative inline-flex cursor-pointer items-center">
            <input
              type="checkbox"
              checked={useFile}
              onChange={(e) => setUseFile(e.target.checked)}
              className="peer sr-only"
            />
            <div className="h-5 w-9 rounded-full bg-neutral-700 after:absolute after:left-[2px] after:top-[2px] after:h-4 after:w-4 after:rounded-full after:bg-white after:transition-all peer-checked:bg-white peer-checked:after:translate-x-full peer-checked:after:bg-black" />
          </label>
          <span className="text-sm text-neutral-400">
            Use credentials from <code className="font-[family-name:var(--font-geist-mono)] text-neutral-300">userinfo.json</code>
          </span>
        </div>

        {!useFile && (
          <>
            <InputField
              label="Username"
              placeholder="Enter username"
              value={username}
              onChange={setUsername}
            />

            <InputField
              label="Password"
              placeholder="Enter password"
              value={password}
              onChange={setPassword}
              type="password"
            />
          </>
        )}

        <button
          type="submit"
          disabled={loading || !iface || (!useFile && (!username || !password))}
          className="rounded-lg bg-white px-5 py-2.5 text-sm font-medium text-black transition-colors hover:bg-neutral-200 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {loading ? "Logging in..." : "Login"}
        </button>
      </form>

      {result && (
        <div className="rounded-xl border border-green-900/50 bg-green-950/20 p-6 space-y-2">
          <p className="text-sm font-medium text-green-400">Login successful</p>
          <p className="text-sm text-neutral-300">
            IP: <span className="font-[family-name:var(--font-geist-mono)]">{result.ip}</span>
          </p>
          <p className="text-sm text-neutral-300">User: {result.username}</p>
          {result.mac && (
            <p className="text-sm text-neutral-300">
              MAC: <span className="font-[family-name:var(--font-geist-mono)]">{result.mac}</span>
            </p>
          )}
          <button
            onClick={() => router.push("/")}
            className="mt-2 text-sm text-neutral-400 underline underline-offset-4 hover:text-white transition-colors"
          >
            Go to Dashboard
          </button>
        </div>
      )}

      {error && (
        <div className="rounded-xl border border-red-900/50 bg-red-950/20 p-6">
          <p className="text-sm text-red-400">{error}</p>
        </div>
      )}
    </div>
  );
}

function InputField({
  label,
  placeholder,
  value,
  onChange,
  type = "text",
  mono,
}: {
  label: string;
  placeholder: string;
  value: string;
  onChange: (v: string) => void;
  type?: string;
  mono?: boolean;
}) {
  return (
    <div className="flex flex-col gap-2">
      <label className="text-sm text-neutral-400">{label}</label>
      <input
        type={type}
        placeholder={placeholder}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={`rounded-lg border border-neutral-800 bg-neutral-900 px-4 py-2.5 text-sm text-white placeholder-neutral-600 outline-none transition-colors focus:border-neutral-600 ${
          mono ? "font-[family-name:var(--font-geist-mono)]" : ""
        }`}
      />
    </div>
  );
}
