import { describe, expect, test } from "bun:test";
import {
  normalizeMacAddress,
  normalizeServerPath,
  parseLoginCount,
  validateUsername,
} from "../src/lib/validation.ts";

describe("normalizeMacAddress", () => {
  test("normalizes common MAC formats", () => {
    expect(normalizeMacAddress("AA-BB-CC-DD-EE-FF")).toEqual({ value: "aa:bb:cc:dd:ee:ff" });
    expect(normalizeMacAddress("aabb.ccdd.eeff")).toEqual({ value: "aa:bb:cc:dd:ee:ff" });
    expect(normalizeMacAddress(" aabbccddeeff ")).toEqual({ value: "aa:bb:cc:dd:ee:ff" });
  });

  test("rejects incomplete and non-hex values", () => {
    expect(normalizeMacAddress("aa:bb").error).toBeTruthy();
    expect(normalizeMacAddress("zz:bb:cc:dd:ee:ff").error).toBeTruthy();
  });

  test("rejects reserved, broadcast, and multicast addresses", () => {
    expect(normalizeMacAddress("00:00:00:00:00:00").error).toContain("all-zero");
    expect(normalizeMacAddress("ff:ff:ff:ff:ff:ff").error).toContain("broadcast");
    expect(normalizeMacAddress("01:00:5e:00:00:01").error).toContain("Multicast");
    expect(normalizeMacAddress("02:00:00:00:00:01")).toEqual({ value: "02:00:00:00:00:01" });
  });
});

describe("form validation", () => {
  test("accepts only whole counts from 1 through 100", () => {
    expect(parseLoginCount("1")).toEqual({ value: 1 });
    expect(parseLoginCount("100")).toEqual({ value: 100 });
    expect(parseLoginCount("1.5").error).toBeTruthy();
    expect(parseLoginCount("0").error).toBeTruthy();
    expect(parseLoginCount("101").error).toBeTruthy();
  });

  test("trims usernames and optional server paths", () => {
    expect(validateUsername(" campus-user ")).toEqual({ value: "campus-user" });
    expect(validateUsername("   ").error).toBeTruthy();
    expect(normalizeServerPath(" userinfo.json ")).toBe("userinfo.json");
    expect(normalizeServerPath("   ")).toBeUndefined();
  });
});
