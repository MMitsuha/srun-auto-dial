"use client";

import { Alert, Button, PageHeader } from "@/components/ui";

export default function AppError({ error, reset }: { error: Error & { digest?: string }; reset: () => void }) {
  return (
    <div className="space-y-8">
      <PageHeader
        eyebrow="Application error"
        title="This view could not be loaded"
        description="An unexpected interface error occurred. Your backend operation may still be running, so verify its status before repeating a destructive action."
      />
      <Alert
        variant="error"
        title="Something went wrong"
        actions={
          <Button type="button" variant="secondary" onClick={reset}>
            Try this view again
          </Button>
        }
      >
        <p>If the problem continues, check the web-server logs for the matching error digest.</p>
        {error.digest && <p className="mt-1 font-mono text-xs opacity-60">Reference: {error.digest}</p>}
      </Alert>
    </div>
  );
}
