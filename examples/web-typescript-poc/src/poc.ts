/**
 * PoC: bit in-memory git + file enumeration
 *
 * Phase 1: Validate bit's in-memory backend works (sync operations only)
 * Phase 2: (future) Add fetch() for remote clone
 *
 * Findings so far:
 * - bit's fetch() requires MoonBit coroutine runtime context
 * - Calling rawFetch directly panics with "current_coroutine undefined"
 * - The lib.js wrapper's async fetch() triggers the same panic
 * - Remote fetch may need a specific runtime setup or be browser-only
 *
 * This PoC validates the file walk + repoask integration pattern
 * using locally created files instead of a remote clone.
 *
 * Usage: npm run poc
 */

import {
  createMemoryHost,
  init,
  add,
  commit,
  status,
  writeString,
  readString,
  type BitGitBackend,
} from "@mizchi/bit/lib";

const REPO_ROOT = "/repo";

// Extensions that repoask-parser handles (oxc + markdown)
const REPOASK_EXTENSIONS = new Set([
  "ts", "tsx", "js", "jsx", "mts", "cts", "mjs", "cjs",
  "md", "mdx",
]);

function main() {
  console.log("[poc] Creating in-memory git repo...");

  const backend = createMemoryHost();
  init(backend, REPO_ROOT, "main");

  // Simulate a repo with TS/JS and markdown files
  const files: Record<string, string> = {
    "src/auth.ts": `
export interface User {
  id: string;
  name: string;
  email: string;
}

/** Validate a JWT token and return the user payload. */
export function validateToken(token: string): User {
  const decoded = decodeJWT(token);
  return decoded.payload as User;
}

export class AuthService {
  constructor(private secret: string) {}

  signIn(email: string, password: string): string {
    return createJWT({ email }, this.secret);
  }
}
`,
    "src/utils/parse.ts": `
export type Result<T> = { ok: true; value: T } | { ok: false; error: Error };

export function safeParse<T>(fn: () => T): Result<T> {
  try {
    return { ok: true, value: fn() };
  } catch (e) {
    return { ok: false, error: e as Error };
  }
}
`,
    "README.md": `
# My Auth Library

## Installation

\`\`\`bash
npm install my-auth
\`\`\`

## Quick Start

\`\`\`typescript
import { validateToken, AuthService } from 'my-auth';

const service = new AuthService('secret');
const token = service.signIn('user@example.com', 'pass');
const user = validateToken(token);
\`\`\`

## API Reference

### validateToken(token)

Validates a JWT token and returns the user payload.

### AuthService

Class for managing authentication.
`,
    "examples/basic.ts": `
import { AuthService, validateToken } from '../src/auth';

const service = new AuthService('my-secret');
const token = service.signIn('test@test.com', '1234');
console.log('Token:', token);

const user = validateToken(token);
console.log('User:', user);
`,
  };

  // Write files into bit's in-memory FS
  const t0 = performance.now();
  for (const [filepath, content] of Object.entries(files)) {
    writeString(backend, `${REPO_ROOT}/${filepath}`, content);
  }

  // Git add + commit
  add(backend, REPO_ROOT, Object.keys(files));
  const commitId = commit(
    backend,
    REPO_ROOT,
    "initial commit",
    "Test <test@test.com>",
  );
  const t1 = performance.now();

  console.log(`[poc] Repo created in ${(t1 - t0).toFixed(0)}ms`);
  console.log(`[poc] Commit: ${commitId}`);
  console.log(`[poc] Status: ${JSON.stringify(status(backend, REPO_ROOT))}`);

  // Walk filesystem and enumerate files
  const allFiles = walkDirectory(backend, REPO_ROOT, "");
  console.log(`\n[poc] Files in repo: ${allFiles.length}`);

  // Classify and read files
  const supported: string[] = [];
  for (const filepath of allFiles) {
    const ext = filepath.split(".").pop() ?? "";
    if (REPOASK_EXTENSIONS.has(ext)) {
      supported.push(filepath);
    }
  }

  console.log(`[poc] Supported by repoask-parser: ${supported.length}`);

  // Simulate feeding to repoask-wasm
  console.log("\n[poc] Files that would be fed to repoask-wasm addFile():");
  let totalBytes = 0;
  for (const filepath of supported) {
    const content = readFileAsString(backend, `${REPO_ROOT}/${filepath}`);
    totalBytes += content.length;
    console.log(`  ${filepath} (${content.length} bytes)`);

    // Here we'd call: repoaskIndex.addFile(filepath, content)
  }

  console.log(`\n[poc] Total: ${supported.length} files, ${totalBytes} bytes`);
  console.log("[poc] Integration pattern validated.");
  console.log("\n[poc] Next steps:");
  console.log("  1. Build repoask-wasm with wasm-pack");
  console.log("  2. Import and call addFile() / build() / search()");
  console.log("  3. Solve bit fetch() coroutine runtime issue for remote clone");
}

/** Recursively walk a directory in the bit in-memory backend. */
function walkDirectory(
  backend: BitGitBackend,
  root: string,
  relativePath: string,
): string[] {
  const fullPath = relativePath ? `${root}/${relativePath}` : root;
  const entries: string[] = [];

  let children: string[];
  try {
    children = Array.from(backend.readdir(fullPath));
  } catch {
    return entries;
  }

  for (const name of children) {
    if (name === ".git") continue;

    const childRelative = relativePath ? `${relativePath}/${name}` : name;
    const childFull = `${root}/${childRelative}`;

    if (backend.isDir(childFull)) {
      entries.push(...walkDirectory(backend, root, childRelative));
    } else if (backend.isFile(childFull)) {
      entries.push(childRelative);
    }
  }

  return entries;
}

/** Read a file from the backend as a UTF-8 string. */
function readFileAsString(backend: BitGitBackend, path: string): string {
  const data = backend.readFile(path);
  if (typeof data === "string") return data;
  if (data instanceof Uint8Array) return new TextDecoder().decode(data);
  if (data instanceof ArrayBuffer) return new TextDecoder().decode(data);
  return new TextDecoder().decode(Uint8Array.from(data as ArrayLike<number>));
}

main();
