import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

export default defineConfig({
  base: "/fermiforge/",
  plugins: [svelte()],
  publicDir: "../data",
});
