# 🌱 Digital Waste Index (DWI)

A desktop application that tracks your computer's real-time carbon footprint and rewards you for reducing digital waste.

Built with **Tauri v2** (Rust backend + HTML/CSS/JS frontend).

![DWI Dashboard](https://img.shields.io/badge/version-0.3.0-ccff00?style=flat-square&labelColor=111111)
![Tests](https://img.shields.io/badge/tests-18%20passed-ccff00?style=flat-square&labelColor=111111)

---

## Features

- **🏭 Real-time CO₂ Scoring** — Converts CPU usage and network traffic into grams of CO₂
- **💰 Eco-Credits** — Earn credits by resolving waste alerts (idle browser tabs, doomscrolling)
- **🔍 Cloud Cleaner** — Scan directories for duplicate files and estimate their carbon cost
- **🎁 Rewards Store** — Redeem Eco-Credits for gift cards (Apple, Amazon, Steam, Spotify)
- **📊 Daily Reports** — End-of-day summary of CO₂ generated, prevented, and credits earned
- **🚀 Splash Screen** — Animated startup with logo build-up effect

---

## Prerequisites

### All Platforms
- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (latest stable)

### Linux (Debian/Ubuntu)
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

### macOS
```bash
xcode-select --install
```

### Windows
- Install [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- Install [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

---

## Quick Start

```bash
# Clone
git clone https://github.com/nevinshine/dwi.git
cd dwi

# Install Node dependencies
npm install

# Run in development mode
npm run dev
```

The app will compile the Rust backend (~30s first time) and launch the DWI dashboard.

---

## Build for Production

```bash
npm run build
```

The compiled binary will be in `src-tauri/target/release/`.

---

## Project Structure

```
dwi/
├── src/                    # Frontend
│   ├── index.html          # Dashboard (bento grid layout)
│   ├── style.css           # Dark theme + neon green accent
│   ├── main.js             # Dashboard logic + Tauri IPC
│   └── redeem.html         # Gift card rewards store
├── src-tauri/              # Rust Backend
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          # Tauri commands + app setup
│       ├── main.rs         # Entry point
│       ├── daemon.rs       # Background polling (CPU, RAM, network)
│       ├── calculator.rs   # CO₂ calculation engine
│       ├── wallet.rs       # Eco-Credit system
│       ├── detectors.rs    # Waste detectors (tab tax, doomscroll)
│       ├── cleaner.rs      # Duplicate file scanner
│       └── db.rs           # SQLite database layer
└── package.json
```

---

## Running Tests

```bash
cd src-tauri
cargo test
```

All 18 tests should pass:
```
test result: ok. 18 passed; 0 failed; 0 ignored
```

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Framework | Tauri v2 |
| Backend | Rust |
| Frontend | HTML + CSS + JavaScript |
| Database | SQLite (rusqlite) |
| System Info | sysinfo crate |
| Hashing | xxHash (for duplicate detection) |

---

## License

MIT
