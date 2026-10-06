# Моя бухгалтерия (Tauri v2 + SvelteKit + SQLite + Ollama)

Требуется: Rust (stable), Node.js 20+, Ollama с загруженной моделью (например `ollama pull qwen2.5:7b`).
На Windows также нужны MSVC Build Tools и WebView2 (на Windows 11 уже есть).

    npm install
    npx tauri icon app-icon.png     # один раз, генерирует иконки
    npm run tauri dev               # разработка
    npm run tauri build             # установщик

База: `finance.db` в папке данных приложения (Windows: `%APPDATA%\local.moya.buhgalteriya`).
Суммы хранятся в копейках (INTEGER). Все запросы к LLM идут только на http://localhost:11434.

## Установщик и автообновление

Один раз:
1. `npx tauri signer generate -w $HOME/.tauri/finance.key` — создаёт ключ подписи обновлений. Приватный ключ никому не отдавайте и не теряйте.
2. Публичный ключ из вывода вставьте в `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.
3. Там же замените `OWNER/REPO` в `endpoints` на свой публичный репозиторий GitHub.
4. В репозитории: Settings → Secrets → Actions → добавьте `TAURI_SIGNING_PRIVATE_KEY` (содержимое файла ключа) и `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

Локальная сборка установщика (PowerShell):

    $env:TAURI_SIGNING_PRIVATE_KEY = "$HOME\.tauri\finance.key"
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "ваш пароль"
    npm run tauri build

Готовый `*-setup.exe` лежит в `src-tauri/target/release/bundle/nsis/`.

Выпуск новой версии: поднимите `version` в `src-tauri/tauri.conf.json`, закоммитьте и выполните
`git tag v0.2.0 && git push --tags`. GitHub Actions соберёт установщик и опубликует релиз с `latest.json`.
Установленные приложения при запуске увидят новую версию и предложат обновиться.
