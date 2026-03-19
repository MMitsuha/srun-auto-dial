import { type RandomLoginResult, isLoginOk } from "@/lib/api";

interface Props {
  results: RandomLoginResult[];
}

export function ResultTable({ results }: Props) {
  if (results.length === 0) return null;

  const successCount = results.filter((r) => isLoginOk(r.result)).length;

  return (
    <div className="space-y-4">
      <div className="flex gap-4 text-sm">
        <span className="text-neutral-400">
          Total: <span className="text-white">{results.length}</span>
        </span>
        <span className="text-neutral-400">
          Success: <span className="text-green-400">{successCount}</span>
        </span>
        <span className="text-neutral-400">
          Failed:{" "}
          <span className="text-red-400">{results.length - successCount}</span>
        </span>
      </div>

      <div className="overflow-hidden rounded-xl border border-neutral-800">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-neutral-800 bg-neutral-900/50">
              <th className="px-4 py-3 text-left font-medium text-neutral-400">
                MAC Address
              </th>
              <th className="px-4 py-3 text-left font-medium text-neutral-400">
                Status
              </th>
              <th className="px-4 py-3 text-left font-medium text-neutral-400">
                User
              </th>
              <th className="px-4 py-3 text-left font-medium text-neutral-400">
                IP
              </th>
            </tr>
          </thead>
          <tbody>
            {results.map((r, i) => {
              const ok = isLoginOk(r.result);
              const data = "Ok" in r.result ? r.result.Ok : null;
              return (
                <tr
                  key={i}
                  className="border-b border-neutral-800/50 last:border-0"
                >
                  <td className="px-4 py-3 font-[family-name:var(--font-geist-mono)] text-neutral-300">
                    {r.mac}
                  </td>
                  <td className="px-4 py-3">
                    {ok ? (
                      <span className="inline-flex items-center gap-1.5 text-green-400">
                        <span className="h-1.5 w-1.5 rounded-full bg-green-400" />
                        OK
                      </span>
                    ) : (
                      <span className="text-red-400 truncate max-w-48 inline-block">
                        {"Err" in r.result ? r.result.Err : "Unknown error"}
                      </span>
                    )}
                  </td>
                  <td className="px-4 py-3 text-neutral-300">
                    {data?.username || "-"}
                  </td>
                  <td className="px-4 py-3 font-[family-name:var(--font-geist-mono)] text-neutral-300">
                    {data?.ip || "-"}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}
