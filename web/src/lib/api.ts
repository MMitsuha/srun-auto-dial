const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://127.0.0.1:3000";
const API_KEY = process.env.NEXT_PUBLIC_API_KEY || "";

export interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

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

export interface RandomLoginResult {
  mac: string;
  result: { Ok: LoginResult } | { Err: string };
}

export function isLoginOk(
  result: RandomLoginResult["result"]
): result is { Ok: LoginResult } {
  return "Ok" in result;
}

async function request<T>(
  path: string,
  options?: RequestInit
): Promise<ApiResponse<T>> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options?.headers as Record<string, string>),
  };
  if (API_KEY) {
    headers["X-API-Key"] = API_KEY;
  }

  const res = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers,
  });
  return res.json();
}

export async function getHealth() {
  return request<string>("/api/health");
}

export async function getInterfaces() {
  return request<InterfaceInfo[]>("/api/interfaces");
}

export async function getStatus(iface: string) {
  return request<StatusResult>(`/api/status?interface=${encodeURIComponent(iface)}`);
}

export async function loginLocal(iface: string, username: string, password: string) {
  return request<LoginResult>("/api/login/local", {
    method: "POST",
    body: JSON.stringify({ interface: iface, username, password }),
  });
}

export async function logoutLocal(iface: string) {
  return request<void>("/api/logout/local", {
    method: "POST",
    body: JSON.stringify({ interface: iface }),
  });
}

export async function loginMacvlan(
  parentInterface: string,
  macAddress: string,
  username: string,
  password: string
) {
  return request<LoginResult>("/api/login/macvlan", {
    method: "POST",
    body: JSON.stringify({
      parent_interface: parentInterface,
      mac_address: macAddress,
      username,
      password,
    }),
  });
}

export async function logoutMacvlan(parentInterface: string) {
  return request<void>("/api/logout/macvlan", {
    method: "POST",
    body: JSON.stringify({ parent_interface: parentInterface }),
  });
}

export async function loginRandom(parentInterface: string, count: number) {
  return request<RandomLoginResult[]>("/api/login/random", {
    method: "POST",
    body: JSON.stringify({ parent_interface: parentInterface, count }),
  });
}
