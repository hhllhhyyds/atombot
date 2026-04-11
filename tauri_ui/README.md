# Tauri UI Example

A desktop UI for Atombot using Tauri 2.0.

## Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (for Tauri CLI)
- Tauri CLI: `cargo install tauri-cli`

## Run

```bash
cd src-tauri
cargo tauri dev
```

Or use npm (if Node.js tooling is preferred):

```bash
npm install
npm run tauri dev
```

## Build

```bash
cargo tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.
