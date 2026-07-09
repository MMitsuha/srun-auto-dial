import { cpSync, existsSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const webRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const buildRoot = path.join(webRoot, ".next");
const standaloneRoot = path.join(buildRoot, "standalone");
const serverPath = path.join(standaloneRoot, "server.js");

if (!existsSync(serverPath)) {
  throw new Error("Standalone build not found. Run `bun run build` before `bun run start`.");
}

const nestedNext = path.join(standaloneRoot, ".next");
mkdirSync(nestedNext, { recursive: true });
cpSync(path.join(buildRoot, "static"), path.join(nestedNext, "static"), { recursive: true });

const publicDirectory = path.join(webRoot, "public");
if (existsSync(publicDirectory)) {
  cpSync(publicDirectory, path.join(standaloneRoot, "public"), { recursive: true });
}

process.env.PORT ||= "3001";
process.env.HOSTNAME ||= "0.0.0.0";
process.chdir(standaloneRoot);
await import(pathToFileURL(serverPath).href);
