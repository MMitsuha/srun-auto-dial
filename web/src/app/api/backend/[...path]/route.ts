import { NextRequest, NextResponse } from "next/server";

const DEFAULT_API_URL = "http://127.0.0.1:3000";
const DEFAULT_TIMEOUT_MS = 75_000;
const DEFAULT_BATCH_TIMEOUT_MS = 2 * 60 * 60_000;
const MAX_BODY_BYTES = 1_048_576;

type RouteContext = { params: Promise<{ path: string[] }> };

function proxyError(status: number, code: string, message: string) {
  return NextResponse.json(
    { success: false, error: { code, message } },
    { status, headers: { "Cache-Control": "no-store" } },
  );
}

function timeoutMs(isBatch: boolean) {
  const configured = Number(
    isBatch ? process.env.API_BATCH_TIMEOUT_MS : process.env.API_TIMEOUT_MS,
  );
  return Number.isFinite(configured) && configured > 0
    ? configured
    : isBatch
      ? DEFAULT_BATCH_TIMEOUT_MS
      : DEFAULT_TIMEOUT_MS;
}

async function forward(request: NextRequest, context: RouteContext) {
  const { path } = await context.params;
  if (!path.length || path.some((segment) => !segment || segment === "." || segment === "..")) {
    return proxyError(400, "INVALID_PROXY_PATH", "The backend path is invalid.");
  }

  let baseUrl: URL;
  try {
    baseUrl = new URL(process.env.API_URL || DEFAULT_API_URL);
    if (!['http:', 'https:'].includes(baseUrl.protocol)) throw new Error("Unsupported protocol");
  } catch {
    return proxyError(500, "INVALID_API_CONFIGURATION", "The backend URL is not configured correctly.");
  }

  const target = new URL(`/api/${path.map(encodeURIComponent).join("/")}`, baseUrl);
  target.search = request.nextUrl.search;

  const headers = new Headers({ Accept: "application/json" });
  const apiKey = process.env.API_KEY;
  if (apiKey) headers.set("X-API-Key", apiKey);

  let body: string | undefined;
  if (request.method === "POST") {
    if (!request.headers.get("content-type")?.toLowerCase().includes("application/json")) {
      return proxyError(415, "UNSUPPORTED_CONTENT_TYPE", "Requests must use application/json.");
    }
    const declaredLength = Number(request.headers.get("content-length"));
    if (Number.isFinite(declaredLength) && declaredLength > MAX_BODY_BYTES) {
      return proxyError(413, "REQUEST_TOO_LARGE", "The request body is too large.");
    }
    try {
      body = await request.text();
    } catch {
      return proxyError(400, "INVALID_REQUEST_BODY", "The request body could not be read.");
    }
    if (new TextEncoder().encode(body).byteLength > MAX_BODY_BYTES) {
      return proxyError(413, "REQUEST_TOO_LARGE", "The request body is too large.");
    }
    headers.set("Content-Type", "application/json");
  }

  const controller = new AbortController();
  let didTimeout = false;
  const abortFromClient = () => controller.abort();
  if (request.signal.aborted) controller.abort();
  else request.signal.addEventListener("abort", abortFromClient, { once: true });
  const timer = setTimeout(
    () => {
      didTimeout = true;
      controller.abort();
    },
    timeoutMs(path.join("/") === "login/random"),
  );
  try {
    const response = await fetch(target, {
      method: request.method,
      headers,
      body,
      cache: "no-store",
      signal: controller.signal,
    });
    const responseBody = await response.text();
    return new NextResponse(responseBody, {
      status: response.status,
      headers: {
        "Content-Type": response.headers.get("content-type") || "application/octet-stream",
        "Cache-Control": "no-store",
      },
    });
  } catch {
    if (didTimeout) {
      return proxyError(504, "BACKEND_TIMEOUT", "The backend did not respond in time.");
    }
    if (request.signal.aborted) {
      return proxyError(499, "CLIENT_CANCELLED", "The client cancelled the request.");
    }
    return proxyError(502, "BACKEND_UNREACHABLE", "The backend service could not be reached.");
  } finally {
    clearTimeout(timer);
    request.signal.removeEventListener("abort", abortFromClient);
  }
}

export const dynamic = "force-dynamic";

export function GET(request: NextRequest, context: RouteContext) {
  return forward(request, context);
}

export function POST(request: NextRequest, context: RouteContext) {
  return forward(request, context);
}
