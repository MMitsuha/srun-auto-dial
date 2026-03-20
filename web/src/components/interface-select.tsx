"use client";

import { useEffect, useState } from "react";
import { getInterfaces, type InterfaceInfo } from "@/lib/api";

interface Props {
  value: string;
  onChange: (value: string) => void;
  label?: string;
}

export function InterfaceSelect({ value, onChange, label = "Network Interface" }: Props) {
  const [interfaces, setInterfaces] = useState<InterfaceInfo[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    getInterfaces().then((res) => {
      if (res.success && res.data) {
        setInterfaces(res.data);
        if (!value && res.data.length > 0) {
          onChange(res.data[0].name);
        }
      }
      setLoading(false);
    });
  }, []);

  return (
    <div className="flex flex-col gap-2">
      <label className="text-sm text-neutral-400">{label}</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={loading}
        className="rounded-lg border border-neutral-800 bg-neutral-900 px-4 py-2.5 text-sm text-white outline-none transition-colors focus:border-neutral-600 disabled:opacity-50"
      >
        {loading ? (
          <option>Loading...</option>
        ) : (
          interfaces.map((iface) => (
            <option key={iface.index} value={iface.name}>
              {iface.name} (index {iface.index})
            </option>
          ))
        )}
      </select>
    </div>
  );
}
