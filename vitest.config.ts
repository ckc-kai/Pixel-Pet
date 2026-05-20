/**
 * Vitest config — frontend unit tests.
 *
 * jsdom environment so React hook tests can render. Kept minimal; no global
 * setup file yet (none of the current tests need DOM matchers).
 */

import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    globals: false,
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
  },
});
