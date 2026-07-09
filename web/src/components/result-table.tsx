import type { RandomLoginAttempt, RandomLoginSummary } from "@/lib/api";
import { Alert, Card } from "@/components/ui";

interface Props {
  summary: RandomLoginSummary | null;
}

function attemptError(attempt: RandomLoginAttempt) {
  return attempt.error?.message || "The login attempt failed without an error message.";
}

export function ResultTable({ summary }: Props) {
  if (!summary) return null;

  return (
    <section aria-labelledby="batch-results-title" className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <h2 id="batch-results-title" className="text-lg font-semibold text-white">
            Batch results
          </h2>
          <p className="mt-1 text-sm text-neutral-500">
            Attempted {summary.attempted} of {summary.requested} requested connections.
          </p>
        </div>
        <dl className="grid grid-cols-3 gap-2 text-center">
          <Metric label="Attempted" value={summary.attempted} />
          <Metric label="Succeeded" value={summary.succeeded} tone="success" />
          <Metric label="Failed" value={summary.failed} tone="error" />
        </dl>
      </div>

      {summary.stopped_reason && (
        <Alert variant="warning" title="Batch stopped early">
          {summary.stopped_reason}
        </Alert>
      )}

      {summary.results.length === 0 ? (
        <Alert variant="info" title="No attempts were completed">
          The server returned an empty batch result.
        </Alert>
      ) : (
        <>
          <Card className="hidden overflow-x-auto sm:block" as="div">
            <table className="w-full min-w-[680px] text-left text-sm">
              <caption className="sr-only">Individual random MAC login results</caption>
              <thead className="border-b border-white/10 bg-white/[0.035] text-xs uppercase tracking-[0.1em] text-neutral-500">
                <tr>
                  <th scope="col" className="px-5 py-3.5 font-medium">MAC address</th>
                  <th scope="col" className="px-5 py-3.5 font-medium">Status</th>
                  <th scope="col" className="px-5 py-3.5 font-medium">User</th>
                  <th scope="col" className="px-5 py-3.5 font-medium">IP address</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-white/[0.07]">
                {summary.results.map((attempt, index) => (
                  <AttemptRow key={`${attempt.mac}-${index}`} attempt={attempt} />
                ))}
              </tbody>
            </table>
          </Card>

          <div className="grid gap-3 sm:hidden">
            {summary.results.map((attempt, index) => (
              <AttemptCard key={`${attempt.mac}-${index}`} attempt={attempt} />
            ))}
          </div>
        </>
      )}
    </section>
  );
}

function Metric({ label, value, tone }: { label: string; value: number; tone?: "success" | "error" }) {
  return (
    <div className="min-w-20 rounded-xl border border-white/10 bg-white/[0.035] px-3 py-2">
      <dt className="text-[10px] uppercase tracking-wider text-neutral-500">{label}</dt>
      <dd className={`mt-0.5 font-mono text-sm ${tone === "success" ? "text-emerald-300" : tone === "error" ? "text-red-300" : "text-white"}`}>
        {value}
      </dd>
    </div>
  );
}

function AttemptRow({ attempt }: { attempt: RandomLoginAttempt }) {
  return (
    <tr className="align-top text-neutral-300">
      <td className="px-5 py-4 font-mono text-xs">{attempt.mac}</td>
      <td className="max-w-xs px-5 py-4">
        {attempt.success ? (
          <span className="inline-flex items-center gap-2 font-medium text-emerald-300">
            <span aria-hidden="true" className="h-1.5 w-1.5 rounded-full bg-emerald-400" /> Success
          </span>
        ) : (
          <div>
            <span className="font-medium text-red-300">Failed</span>
            <p className="mt-1 break-words text-xs leading-5 text-red-200/70">{attemptError(attempt)}</p>
          </div>
        )}
      </td>
      <td className="px-5 py-4">{attempt.data?.username || "—"}</td>
      <td className="px-5 py-4 font-mono text-xs">{attempt.data?.ip || "—"}</td>
    </tr>
  );
}

function AttemptCard({ attempt }: { attempt: RandomLoginAttempt }) {
  return (
    <Card className="p-4" as="div">
      <div className="flex items-start justify-between gap-3">
        <p className="break-all font-mono text-xs text-neutral-300">{attempt.mac}</p>
        <span className={`shrink-0 rounded-full px-2.5 py-1 text-xs font-semibold ${attempt.success ? "bg-emerald-500/15 text-emerald-300" : "bg-red-500/15 text-red-300"}`}>
          {attempt.success ? "Success" : "Failed"}
        </span>
      </div>
      {attempt.success ? (
        <dl className="mt-4 grid grid-cols-2 gap-3 text-sm">
          <div><dt className="text-xs text-neutral-500">User</dt><dd className="mt-1 break-all text-neutral-200">{attempt.data?.username || "—"}</dd></div>
          <div><dt className="text-xs text-neutral-500">IP address</dt><dd className="mt-1 break-all font-mono text-xs text-neutral-200">{attempt.data?.ip || "—"}</dd></div>
        </dl>
      ) : (
        <p className="mt-3 break-words text-sm leading-5 text-red-200/75">{attemptError(attempt)}</p>
      )}
    </Card>
  );
}
