import Link from "next/link";
import { Card, PageHeader } from "@/components/ui";

export default function NotFound() {
  return (
    <div className="space-y-8">
      <PageHeader
        eyebrow="404"
        title="Page not found"
        description="The requested control page does not exist or has moved."
      />
      <Card className="p-5 sm:p-6">
        <Link
          href="/"
          className="inline-flex min-h-11 items-center rounded-xl bg-sky-300 px-4 py-2.5 text-sm font-semibold text-slate-950 transition hover:bg-sky-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sky-300 focus-visible:ring-offset-2 focus-visible:ring-offset-black"
        >
          Return to dashboard
        </Link>
      </Card>
    </div>
  );
}
