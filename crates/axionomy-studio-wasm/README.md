# axionomy-studio-wasm

The Web Worker adapter for Axionomy Studio. It exports the same Rust-owned
problem catalog, run request, solver observations, and replay artifact used by
the native HTTP service. JavaScript schedules and transports the computation;
it does not reimplement any economic semantics.

Build it through `pnpm build:wasm` in `studio/`. The generated bindings are
consumed by Vite and intentionally remain build output.
