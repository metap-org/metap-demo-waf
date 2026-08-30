import path from "node:path";
import { defineConfig, searchForWorkspaceRoot } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  resolve: {
    // Same reason as `apps/crm-fe/vite.config.ts` in `metap`: `@metap/ui`/`@metap/platform-ui`
    // are real symlinks (`link:../../../design-system`, `link:../../../platform-ui`) to sibling
    // repos with their own independent `pnpm install` — dedupe forces every `react`/`react-dom`
    // import in the graph to resolve to this app's own copy, avoiding "Invalid hook call".
    dedupe: ["react", "react-dom"],
  },
  server: {
    // `platform-ui`/`design-system` live outside this workspace root — Vite's default `fs.allow`
    // would 403 every request for their files through `/@fs/...`.
    fs: {
      allow: [
        searchForWorkspaceRoot(process.cwd()),
        path.resolve(import.meta.dirname, "../../../platform-ui"),
        path.resolve(import.meta.dirname, "../../../design-system"),
      ],
    },
    proxy: {
      "/api": "http://localhost:3000",
      "/metadata": "http://localhost:3000",
      "/health": "http://localhost:3000",
      "/preferences": "http://localhost:3000",
      "/auth": "http://localhost:3000",
      "/admin": "http://localhost:3000",
    },
  },
});
