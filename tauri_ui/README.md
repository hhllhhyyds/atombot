# Tauri UI

A desktop UI for Atombot using Tauri 2.0.

## Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Tauri CLI](https://tauri.app/start/) `cargo install tauri-cli`

## Setup

1. Create a `.env` file in the project root (one level up):

```bash
echo "OPENAI_API_KEY=your_key" > ../.env
```

2. Start the development server:

```bash
./dev.sh
```

This script automatically:
- Kills any process on port 8080
- Starts an HTTP server for the frontend
- Launches `cargo tauri dev`

## Manual Setup

If you prefer running manually:

**Terminal 1:** Start HTTP server
```bash
cd ui
python3 -m http.server 8080
```

**Terminal 2:** Run Tauri
```bash
cd src-tauri
cargo tauri dev
```

## Build

```bash
cd src-tauri
cargo tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.
