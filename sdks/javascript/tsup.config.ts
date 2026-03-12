import { defineConfig } from "tsup";

export default defineConfig({
  entry: ["src/index.ts"],
  format: ["cjs", "esm"],
  dts: true,
  splitting: false,
  sourcemap: true,
  clean: true,
  external: ["better-sqlite3"],
  esbuildOptions(options) {
    options.external = [...(options.external ?? []), "../wasm/bandito_engine"];
  },
});
