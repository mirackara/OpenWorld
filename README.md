# OpenWorld

OpenWorld is a fast, privacy-first, desktop AI client built with Tauri, React, and Rust. It securely runs large language models (LLMs) entirely locally on your machine using an embedded Ollama engine, meaning your chats and data never leave your device.

## ✨ Features

- **100% Local & Private:** Chats are processed locally using an embedded Ollama daemon. No cloud APIs, no subscriptions, no tracking.
- **Embedded AI Engine:** Seamless experience — Ollama is bundled and managed automatically by the application. No separate installation required.
- **Model Management catalog:** Easily browse, download, and manage open-source models (like Llama 3, Mistral, Gemma 2) directly within the app.
- **Automatic Fact Extraction:** The AI automatically extracts and remembers personal facts from your conversations, building long-term memory across all your chats. fully configurable in Settings.
- **Encrypted Local Storage:** All your conversations, messages, and memories are stored in a local SQLite database encrypt with AES-256-GCM.
- **Rich Chat Interface:** Fluid token streaming, Markdown support, automatic syntax highlighting for code blocks, and a sleek, modern UI.
- **Cross-Platform Setup Wizard:** First-time launch wizard automatically verifies your system requirements, provisions the embedded engine, and downloads a high-quality default model to get you started effortlessly.

## 🛠️ Tech Stack

- **Frontend:** React, TypeScript, Vite, Zustand (state management), React Router, Vanilla CSS
- **Backend:** Rust, Tauri v2
- **Database:** SQLite (via `rusqlite`) encrypted with AES-GCM
- **AI Engine:** Ollama (managed via spawned subprocesses)

## 🚀 Getting Started

### Prerequisites

To build OpenWorld from source, you need the standard Tauri development environment:
- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- **macOS:** Xcode Command Line Tools (`xcode-select --install`)
- **Linux:** App build dependencies (e.g., `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`)
- **Windows:** Visual Studio C++ Build Tools

### Installation & Build

1. **Clone the repository:**
   ```bash
   git clone https://github.com/mirackara/OpenWorld.git
   cd OpenWorld
   ```

2. **Install frontend dependencies:**
   ```bash
   npm install
   ```

3. **Run in development mode:**
   This will start the Vite dev server and launch the Tauri app wrapper.
   ```bash
   npm run tauri dev
   ```

4. **Build for production:**
   To compile a native executable/installer for your OS (.app for macOS, .exe for Windows, .deb/.AppImage for Linux):
   ```bash
   npm run tauri build
   ```
   The compiled binaries will be located in `src-tauri/target/release/bundle/`.

## 🤝 Contributing
Contributions, issues, and feature requests are welcome! Feel free to check the [issues page](https://github.com/mirackara/OpenWorld/issues).

## 📝 License
This project is licensed under the MIT License.
