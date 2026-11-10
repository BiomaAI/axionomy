# axionomy-studio-wasm

The Web Worker adapter for Axionomy Studio. It exports the same Rust-owned
problem catalog, study request, solver observations, and study used by the
native HTTP service: `study()` computes one and emits the same Studio events.
JavaScript schedules and transports the computation; it does not reimplement
any economic semantics.

Build it through `pnpm build:wasm` in `studio/`. The generated bindings are
consumed by Vite and intentionally remain build output.
