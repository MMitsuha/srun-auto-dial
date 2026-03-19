"use client";

import { useState } from "react";
import { loginRandom, type RandomLoginResult } from "@/lib/api";
import { InterfaceSelect } from "@/components/interface-select";
import { ResultTable } from "@/components/result-table";

export default function RandomPage() {
  const [iface, setIface] = useState("");
  const [count, setCount] = useState("1");
  const [loading, setLoading] = useState(false);
  const [results, setResults] = useState<RandomLoginResult[]>([]);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const n = parseInt(count, 10);
    if (isNaN(n) || n < 1 || n > 100) {
      setError("Count must be between 1 and 100");
      return;
    }

    setLoading(true);
    setError(null);
    setResults([]);

    const res = await loginRandom(iface, n);
    setLoading(false);
    if (res.success && res.data) {
      setResults(res.data);
    } else {
      setError(res.error || "Random login failed");
    }
  };

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">
          Random MAC Login
        </h1>
        <p className="mt-2 text-sm text-neutral-400">
          Batch login with randomly generated MAC addresses. User credentials
          are read from <code className="font-[family-name:var(--font-geist-mono)] text-neutral-300">userinfo.json</code> on the
          server.
        </p>
      </div>

      <form onSubmit={handleSubmit} className="space-y-5">
        <InterfaceSelect
          value={iface}
          onChange={setIface}
          label="Parent Interface"
        />

        <div className="flex flex-col gap-2">
          <label className="text-sm text-neutral-400">Count (1-100)</label>
          <input
            type="number"
            min={1}
            max={100}
            value={count}
            onChange={(e) => setCount(e.target.value)}
            className="rounded-lg border border-neutral-800 bg-neutral-900 px-4 py-2.5 text-sm text-white placeholder-neutral-600 outline-none transition-colors focus:border-neutral-600 w-32 font-[family-name:var(--font-geist-mono)]"
          />
        </div>

        <button
          type="submit"
          disabled={loading || !iface}
          className="rounded-lg bg-white px-5 py-2.5 text-sm font-medium text-black transition-colors hover:bg-neutral-200 disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {loading ? "Running..." : "Start"}
        </button>
      </form>

      {error && (
        <div className="rounded-xl border border-red-900/50 bg-red-950/20 p-6">
          <p className="text-sm text-red-400">{error}</p>
        </div>
      )}

      <ResultTable results={results} />
    </div>
  );
}
