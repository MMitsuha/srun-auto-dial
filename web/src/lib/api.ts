export interface ApiError {
  code: string;
  message: string;
  field?: string;
  status?: number;
  retryable?: boolean;
}

export type ApiResult<T> =
  | { ok: true; data: T; status: number }
  | { ok: false; error: ApiError; status: number };

export interface InterfaceInfo {
  index: number;
  name: string;
}

export interface LoginResult {
  ip: string;
  username: string;
  mac: string | null;
}

export interface StatusResult {
  ip: string;
  online_user: string | null;
  online_mac: string | null;
}

export interface RandomLoginAttempt {
  mac: string;
  success: boolean;
  data?: LoginResult;
  error?: ApiError;
}

export interface RandomLoginSummary {
  requested: number;
  attempted: number;
  succeeded: number;
  failed: number;
  stopped_reason?: string;
  results: RandomLoginAttempt[];
}

interface RequestOptions extends RequestInit {
  timeoutMs?: number;
}

type Validator<T> = (value: unknown) => value is T;

const DEFAULT_TIMEOUT_MS = 80_000;
const BATCH_TIMEOUT_MS = 2 * 60 * 60_000 + 5_000;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isNullableString(value: unknown): value is string | null {
  return typeof value === "string" || value === null;
}

const isLoginResult: Validator<LoginResult> = (value): value is LoginResult =>
  isRecord(value) &&
  typeof value.ip === "string" &&
  typeof value.username === "string" &&
  isNullableString(value.mac);

const isStatusResult: Validator<StatusResult> = (value): value is StatusResult =>
  isRecord(value) &&
  typeof value.ip === "string" &&
  isNullableString(value.online_user) &&
  isNullableString(value.online_mac);

const isInterfaceList: Validator<InterfaceInfo[]> = (value): value is InterfaceInfo[] =>
  Array.isArray(value) &&
  value.every(
    (item) => isRecord(item) && typeof item.index === "number" && typeof item.name === "string",
  );

const isApiError = (value: unknown): value is ApiError =>
  isRecord(value) && typeof value.code === "string" && typeof value.message === "string";

export const isRandomLoginSummary: Validator<RandomLoginSummary> = (value): value is RandomLoginSummary =>
  isRecord(value) &&
  Number.isInteger(value.requested) &&
  Number.isInteger(value.attempted) &&
  Number.isInteger(value.succeeded) &&
  Number.isInteger(value.failed) &&
  (value.requested as number) >= 1 &&
  (value.requested as number) <= 100 &&
  (value.attempted as number) >= 0 &&
  (value.attempted as number) <= (value.requested as number) &&
  (value.succeeded as number) >= 0 &&
  (value.failed as number) >= 0 &&
  (value.succeeded as number) + (value.failed as number) === (value.attempted as number) &&
  (value.stopped_reason === undefined || typeof value.stopped_reason === "string") &&
  Array.isArray(value.results) &&
  value.results.length === value.attempted &&
  value.results.every(
    (attempt) =>
      isRecord(attempt) &&
      typeof attempt.mac === "string" &&
      typeof attempt.success === "boolean" &&
      (attempt.success
        ? isLoginResult(attempt.data) && attempt.error === undefined
        : attempt.data === undefined && isApiError(attempt.error)),
  );

function errorFromUnknown(value: unknown, status: number): ApiError {
  if (typeof value === "string" && value.trim()) {
    return { code: "BACKEND_ERROR", message: value, status };
  }

  if (isRecord(value)) {
    const message = typeof value.message === "string" ? value.message : "The request failed.";
    return {
      code: typeof value.code === "string" ? value.code : "BACKEND_ERROR",
      message,
      ...(typeof value.field === "string" ? { field: value.field } : {}),
      status,
      retryable: status >= 500,
    };
  }

  return {
    code: "MALFORMED_ERROR",
    message: "The server returned an invalid error response.",
    status,
    retryable: status >= 500,
  };
}

/** Pure response parser, exported for dependency-free Bun tests. */
export function parseApiResponse<T>(
  status: number,
  contentType: string | null,
  body: string,
): ApiResult<T> {
  const normalizedContentType = contentType?.toLowerCase() || "";
  const isJson =
    normalizedContentType.includes("application/json") || normalizedContentType.includes("+json");
  if (!isJson) {
    return {
      ok: false,
      status,
      error: {
        code: "NON_JSON_RESPONSE",
        message:
          status >= 500
            ? "The service returned an unreadable response. Please try again."
            : "The server returned an unexpected response.",
        status,
        retryable: status >= 500,
      },
    };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(body);
  } catch {
    return {
      ok: false,
      status,
      error: {
        code: "INVALID_JSON",
        message: "The server returned malformed JSON.",
        status,
        retryable: status >= 500,
      },
    };
  }

  if (!isRecord(parsed) || typeof parsed.success !== "boolean") {
    return {
      ok: false,
      status,
      error: {
        code: "MALFORMED_RESPONSE",
        message: "The server response did not match the expected format.",
        status,
      },
    };
  }

  if (parsed.success) {
    if (status < 200 || status >= 300) {
      return {
        ok: false,
        status,
        error: {
          code: "INVALID_SUCCESS_STATUS",
          message: "The server reported success with an invalid HTTP status.",
          status,
        },
      };
    }
    return { ok: true, status, data: parsed.data as T };
  }

  return { ok: false, status, error: errorFromUnknown(parsed.error, status) };
}

async function request<T>(
  path: string,
  options: RequestOptions = {},
  validate?: Validator<T>,
): Promise<ApiResult<T>> {
  const { timeoutMs = DEFAULT_TIMEOUT_MS, signal, headers: suppliedHeaders, ...init } = options;
  const controller = new AbortController();
  let didTimeout = false;
  const timeout = setTimeout(() => {
    didTimeout = true;
    controller.abort();
  }, timeoutMs);
  const abortFromCaller = () => controller.abort();
  if (signal?.aborted) controller.abort();
  else signal?.addEventListener("abort", abortFromCaller, { once: true });

  const headers = new Headers(suppliedHeaders);
  headers.set("Accept", "application/json");
  if (init.body && !headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  try {
    const response = await fetch(`/api/backend${path}`, {
      ...init,
      headers,
      cache: "no-store",
      signal: controller.signal,
    });
    const body = await response.text();
    const result = parseApiResponse<T>(response.status, response.headers.get("content-type"), body);
    if (result.ok && validate && !validate(result.data)) {
      return {
        ok: false,
        status: result.status,
        error: {
          code: "MALFORMED_DATA",
          message: "The server returned data in an unexpected format.",
          status: result.status,
        },
      };
    }
    return result;
  } catch {
    const aborted = controller.signal.aborted;
    return {
      ok: false,
      status: 0,
      error: {
        code: didTimeout ? "REQUEST_TIMEOUT" : aborted ? "REQUEST_CANCELLED" : "NETWORK_ERROR",
        message: didTimeout
          ? "The request took too long. Please try again."
          : aborted
            ? "The request was cancelled."
            : "Could not reach the service. Check the server connection and try again.",
        status: 0,
        retryable: !aborted || didTimeout,
      },
    };
  } finally {
    clearTimeout(timeout);
    signal?.removeEventListener("abort", abortFromCaller);
  }
}

function post<T>(
  path: string,
  body: Record<string, unknown>,
  options?: RequestOptions,
  validate?: Validator<T>,
) {
  return request<T>(path, { ...options, method: "POST", body: JSON.stringify(body) }, validate);
}

export function getHealth(options?: RequestOptions) {
  return request<string>("/health", options, (value): value is string => typeof value === "string");
}

export function getInterfaces(options?: RequestOptions) {
  return request<InterfaceInfo[]>("/interfaces", options, isInterfaceList);
}

export function getStatus(iface: string, options?: RequestOptions) {
  return request<StatusResult>(`/status?interface=${encodeURIComponent(iface)}`, options, isStatusResult);
}

export function getStatusMacvlan(parentInterface: string, macAddress: string, options?: RequestOptions) {
  return post<StatusResult>(
    "/status/macvlan",
    { parent_interface: parentInterface, mac_address: macAddress },
    options,
    isStatusResult,
  );
}

export function loginLocal(
  iface: string,
  username?: string,
  password?: string,
  userinfoPath?: string,
  options?: RequestOptions,
) {
  return post<LoginResult>(
    "/login/local",
    {
      interface: iface,
      ...(username !== undefined && password !== undefined ? { username, password } : {}),
      ...(userinfoPath ? { userinfo_path: userinfoPath } : {}),
    },
    options,
    isLoginResult,
  );
}

export function logoutLocal(iface: string, options?: RequestOptions) {
  return post<void>("/logout/local", { interface: iface }, options);
}

export function loginMacvlan(
  parentInterface: string,
  macAddress: string,
  username?: string,
  password?: string,
  userinfoPath?: string,
  options?: RequestOptions,
) {
  return post<LoginResult>(
    "/login/macvlan",
    {
      parent_interface: parentInterface,
      mac_address: macAddress,
      ...(username !== undefined && password !== undefined ? { username, password } : {}),
      ...(userinfoPath ? { userinfo_path: userinfoPath } : {}),
    },
    options,
    isLoginResult,
  );
}

export function logoutMacvlan(
  parentInterface: string,
  macAddress: string,
  options?: RequestOptions,
) {
  return post<void>(
    "/logout/macvlan",
    { parent_interface: parentInterface, mac_address: macAddress },
    options,
  );
}

export function loginRandom(
  parentInterface: string,
  count: number,
  userinfoPath?: string,
  options?: RequestOptions,
) {
  return post<RandomLoginSummary>(
    "/login/random",
    {
      parent_interface: parentInterface,
      count,
      ...(userinfoPath ? { userinfo_path: userinfoPath } : {}),
    },
    { timeoutMs: BATCH_TIMEOUT_MS, ...options },
    isRandomLoginSummary,
  );
}
