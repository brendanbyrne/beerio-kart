import { defineConfig } from 'vite';
import react, { reactCompilerPreset } from '@vitejs/plugin-react';
import babel from '@rolldown/plugin-babel';
import tailwindcss from '@tailwindcss/vite';

// `apiFetch` calls the backend with same-origin relative URLs (/api/...), so
// both the dev server and `vite preview` must forward /api to the Axum backend
// on :3000. `vite preview` does NOT inherit `server.proxy`, so the rule is
// shared here and applied to both — without the preview entry, a production
// build served by `bun run preview` can't reach the API.
const apiProxy = {
  '/api': {
    target: 'http://localhost:3000',
    changeOrigin: true,
  },
};

export default defineConfig({
  plugins: [
    react(),
    // The React Compiler (react.md § 2) runs via a Babel pass — plugin-react v6
    // transforms with oxc, not Babel, so the Compiler is wired in through
    // @rolldown/plugin-babel + the plugin's reactCompilerPreset helper (it
    // carries a preconfigured filter for React/hook files). The preset targets
    // React 19 by default, so no `target` option is needed. Active in every
    // mode, so dev gets the same memoization as prod.
    babel({ presets: [reactCompilerPreset()] }),
    tailwindcss(),
  ],
  server: {
    port: 5173,
    proxy: apiProxy,
  },
  preview: {
    proxy: apiProxy,
  },
});
