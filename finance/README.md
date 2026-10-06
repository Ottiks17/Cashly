# Моя бухгалтерия (Tauri v2 + SvelteKit + SQLite + Ollama)

Требуется: Rust (stable), Node.js 20+, Ollama с загруженной моделью (например `ollama pull qwen2.5:7b`).
На Windows также нужны MSVC Build Tools и WebView2 (на Windows 11 уже есть).

    npm install
    npx tauri icon app-icon.png     # один раз, генерирует иконки
    npm run tauri dev               # разработка
    npm run tauri build             # установщик

База: `finance.db` в папке данных приложения (Windows: `%APPDATA%\local.moya.buhgalteriya`).
Суммы хранятся в копейках (INTEGER). Все запросы к LLM идут только на http://localhost:11434.
