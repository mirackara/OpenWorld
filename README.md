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

### 📦 For Non-Technical Users (No coding required)

The easiest way to get OpenWorld is to download a pre-built version. You don't need to know anything about terminals, code, or Ollama to run this app.

1. Go to the **Releases** page (click the "Releases" link on the right side of this GitHub page).
2. Look for the latest release (e.g., `v0.1.0`).
3. Under **Assets**, download the installer for your computer:
   - **Mac:** Download the `.dmg` file and drag the app into your Applications folder.
   - **Windows:** Download the `.msi` file and click it to install. 
4. Open the OpenWorld app. The Setup Wizard will automatically download the AI models and get everything running in the background for you.

> **Note for Mac Users:** If you get an error saying *"OpenWorld is damaged and can't be opened. You should move it to the Trash,"* this is a standard macOS security warning for apps downloaded outside the App Store that aren't digitally signed yet. 
> **To fix it:** Open your Terminal app, paste this exact command, press Enter, and then open the app normally:
> ```bash
> xattr -d com.apple.quarantine /Applications/OpenWorld.app
> ```

---

### 💻 For Developers (Building from source)

If you want to modify the app or build it yourself from source:

#### Prerequisites

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
