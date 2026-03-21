# Digital Waste Index (DWI)

A desktop application that tracks your computer's real-time carbon footprint and rewards you for reducing digital waste.

Built with Tauri v2 (Rust backend, HTML/CSS/JS frontend).

---

## What It Does

- **Carbon Scoring** — Converts CPU usage and network traffic into estimated grams of CO2
- **Eco-Credits** — Earn credits by fixing waste alerts like idle browser tabs
- **Cloud Cleaner** — Scan directories for duplicate files and estimate their carbon cost
- **Rewards Store** — Redeem credits for gift cards (Apple, Amazon, Steam, Spotify)
- **Daily Reports** — End-of-day summary of carbon generated and prevented

---

## Prerequisites

**All Platforms:**
- [Node.js](https://nodejs.org/) v18+
- [Rust](https://rustup.rs/) (latest stable)

**Linux (Debian/Ubuntu):**
```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  patchelf \
  build-essential \
  curl \
  wget \
  file \
  libssl-dev \
  libxdo-dev
```

**macOS:**
```bash
xcode-select --install
```

**Windows:**
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

---

## Quick Start

```bash
git clone https://github.com/nevinshine/dwi.git
cd dwi
npm install
npm run dev
```

First build takes ~30s to compile the Rust backend. Subsequent runs are instant.

---

## Build

```bash
npm run build
```

Output binary: `src-tauri/target/release/dwi`

---

## Tests

```bash
cd src-tauri
cargo test
```

```
test result: ok. 18 passed; 0 failed; 0 ignored
```

---

## Project Structure

```
src/                     Frontend (HTML/CSS/JS)
  index.html             Dashboard
  style.css              Dark theme styling
  main.js                UI logic and Tauri IPC
  redeem.html            Rewards store

src-tauri/src/           Rust Backend
  lib.rs                 Tauri commands and app setup
  daemon.rs              Background system polling
  calculator.rs          CO2 calculation engine
  wallet.rs              Eco-Credit system
  detectors.rs           Waste detection (tab tax, doomscroll)
  cleaner.rs             Duplicate file scanner
  db.rs                  SQLite database layer
```

---

## Stack

| Layer | Tech |
|-------|------|
| Framework | Tauri v2 |
| Backend | Rust |
| Frontend | Vanilla HTML/CSS/JS |
| Database | SQLite |
| System Metrics | sysinfo |
| File Hashing | xxHash |

---

## Status

v0.1 — Prototype. Core features functional, UI complete. Not production-ready.

## License

MIT
