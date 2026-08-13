/**
 * `vitest` + Testing Library, per `ARCHITECTURE_SPEC.md` §12's last row.
 *
 * Kept apart from `vite.config.ts` so the production build has no idea the
 * tests exist. The environment is `jsdom` because the store is bound to React
 * through `useSyncExternalStore`, and the reconnect criterion is a claim about
 * what is *rendered* after a daemon restarts — which cannot be asserted without
 * something to render into.
 *
 * The end-to-end tests are Playwright's (`playwright.config.ts`) and are
 * excluded here: they need a daemon, and everything in this run needs nothing.
 */

import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["src/testing/setup.ts"],
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    coverage: {
      provider: "v8",
      reporter: ["text", "json-summary"],
      reportsDirectory: "coverage",
      include: ["src/**/*.ts", "src/**/*.tsx"],
      // `bindings/` is generated from Rust and checked there; `main.tsx` is the
      // three lines that put a root on the page, and a test of it would be a
      // test of `createRoot`.
      exclude: [
        "src/bindings/**",
        "src/main.tsx",
        "src/testing/**",
        "src/**/*.test.ts",
        "src/**/*.test.tsx",
      ],
    },
  },
});
