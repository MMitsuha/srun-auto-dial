import { describe, expect, test } from "bun:test";
import { isRandomLoginSummary, parseApiResponse } from "../src/lib/api.ts";

const json = "application/json; charset=utf-8";

describe("parseApiResponse", () => {
  test("parses a successful envelope", () => {
    expect(parseApiResponse(200, json, JSON.stringify({ success: true, data: { value: 3 } }))).toEqual({
      ok: true,
      status: 200,
      data: { value: 3 },
    });
    expect(
      parseApiResponse(200, "application/problem+json", JSON.stringify({ success: true, data: "ok" })),
    ).toMatchObject({ ok: true, data: "ok" });
  });

  test("preserves structured backend errors", () => {
    const result = parseApiResponse(
      400,
      json,
      JSON.stringify({
        success: false,
        error: { code: "INVALID_MAC", message: "Invalid MAC address", field: "mac_address" },
      }),
    );
    expect(result.ok).toBeFalse();
    expect(result.error).toMatchObject({
      code: "INVALID_MAC",
      message: "Invalid MAC address",
      field: "mac_address",
      status: 400,
    });
  });

  test("supports legacy string errors while the backend migrates", () => {
    const result = parseApiResponse(
      409,
      json,
      JSON.stringify({ success: false, error: "session already online" }),
    );
    expect(result.ok).toBeFalse();
    expect(result.error.code).toBe("BACKEND_ERROR");
    expect(result.error.message).toBe("session already online");
  });

  test("normalizes non-JSON, invalid JSON, and malformed envelopes", () => {
    expect(parseApiResponse(502, "text/html", "<h1>Bad gateway</h1>").error.code).toBe(
      "NON_JSON_RESPONSE",
    );
    expect(parseApiResponse(500, json, "{").error.code).toBe("INVALID_JSON");
    expect(parseApiResponse(200, json, JSON.stringify({ data: [] })).error.code).toBe(
      "MALFORMED_RESPONSE",
    );
  });

  test("rejects success envelopes paired with error status codes", () => {
    const result = parseApiResponse(500, json, JSON.stringify({ success: true, data: null }));
    expect(result.ok).toBeFalse();
    expect(result.error.code).toBe("INVALID_SUCCESS_STATUS");
  });
});

describe("isRandomLoginSummary", () => {
  const success = {
    mac: "02:00:00:00:00:01",
    success: true,
    data: { ip: "10.0.0.2", username: "alice", mac: "02:00:00:00:00:01" },
  };
  const failure = {
    mac: "02:00:00:00:00:02",
    success: false,
    error: { code: "portal_timeout", message: "The portal timed out." },
  };

  test("accepts internally consistent batch data", () => {
    expect(
      isRandomLoginSummary({
        requested: 2,
        attempted: 2,
        succeeded: 1,
        failed: 1,
        results: [success, failure],
      }),
    ).toBeTrue();
  });

  test("rejects contradictory attempts and summary counts", () => {
    expect(
      isRandomLoginSummary({
        requested: 1,
        attempted: 1,
        succeeded: 1,
        failed: 0,
        results: [{ ...success, error: failure.error }],
      }),
    ).toBeFalse();
    expect(
      isRandomLoginSummary({
        requested: 2,
        attempted: 2,
        succeeded: 2,
        failed: 0,
        results: [success],
      }),
    ).toBeFalse();
    expect(
      isRandomLoginSummary({
        requested: 0,
        attempted: 0,
        succeeded: 0,
        failed: 0,
        results: [],
      }),
    ).toBeFalse();
  });
});
