# termagent web client

The browser UI served by `termagent --web`. React 19 + TypeScript on Vite, styled with
Tailwind v4 and shadcn primitives, using `@aicss/react` for chat pieces and Phosphor
icons. Motion comes from `motion/react`.

```sh
npm ci
npm run build      # writes dist/, no source maps
npm run dev        # needs a running `termagent --web` for the API
```

The Rust build embeds `dist/` into the binary (see `crates/web/build.rs`), so run
`npm run build` and then `cargo build --release` after changing anything here.

The API requires the per-boot token from `window.__TERMAGENT__`, injected into
`index.html` by the server. Opening the Vite dev server directly shows a setup screen
because no token exists there; use the binary's URL instead.
