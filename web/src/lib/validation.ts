export interface ValidationResult<T> {
  value?: T;
  error?: string;
}

export function normalizeMacAddress(input: string): ValidationResult<string> {
  const compact = input.trim().replace(/[-.]/g, "").replace(/:/g, "");
  if (!/^[0-9a-fA-F]{12}$/.test(compact)) {
    return { error: "Enter a valid 12-digit MAC address, for example AA:BB:CC:DD:EE:FF." };
  }

  const octets = compact.match(/.{2}/g);
  if (!octets) return { error: "Enter a valid MAC address." };
  const bytes = octets.map((octet) => Number.parseInt(octet, 16));
  if (bytes.every((byte) => byte === 0)) {
    return { error: "The all-zero MAC address is reserved and cannot be used." };
  }
  if (bytes.every((byte) => byte === 0xff)) {
    return { error: "The broadcast MAC address cannot be used." };
  }
  if ((bytes[0] & 1) !== 0) {
    return { error: "Multicast MAC addresses cannot be used for a network session." };
  }
  return { value: octets.join(":").toLowerCase() };
}

export function parseLoginCount(input: string): ValidationResult<number> {
  const value = Number(input);
  if (!Number.isInteger(value) || value < 1 || value > 100) {
    return { error: "Enter a whole number between 1 and 100." };
  }
  return { value };
}

export function validateUsername(input: string): ValidationResult<string> {
  const value = input.trim();
  return value ? { value } : { error: "Enter a username." };
}

export function normalizeServerPath(input: string): string | undefined {
  return input.trim() || undefined;
}
