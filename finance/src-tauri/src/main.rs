#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State};

const OLLAMA: &str = "http://localhost:11434";
const NO_OLLAMA: &str = "Ollama недоступна на localhost:11434. Запустите её и повторите.";

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    amount INTEGER NOT NULL,
    type TEXT NOT NULL CHECK(type IN ('income', 'expense')),
    category TEXT NOT NULL,
    description TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS categories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    type TEXT NOT NULL CHECK(type IN ('income', 'expense')),
    color TEXT,
    icon TEXT
);
CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date);
INSERT OR IGNORE INTO categories(name, type, color, icon) VALUES
 ('Еда','expense','#639922','shopping-cart'),
 ('Жильё','expense','#534AB7','home'),
 ('Транспорт','expense','#378ADD','bus'),
 ('Кафе','expense','#D85A30','coffee'),
 ('Здоровье','expense','#D4537E','heart'),
 ('Развлечения','expense','#BA7517','ticket'),
 ('Прочее','expense','#888780','dots'),
 ('Зарплата','income','#1D9E75','cash'),
 ('Прочие доходы','income','#0F6E56','plus');
";

struct Db(Mutex<Connection>);

fn e<E: ToString>(x: E) -> String {
    x.to_string()
}

fn with_db<T>(db: &Db, f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    let conn = db.0.lock().map_err(|_| "Ошибка доступа к базе".to_string())?;
    f(&conn)
}

#[derive(Serialize)]
struct Transaction {
    id: i64,
    date: String,
    amount: i64,
    #[serde(rename = "type")]
    kind: String,
    category: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct NewTransaction {
    date: String,
    amount: String,
    #[serde(rename = "type")]
    kind: String,
    category: String,
    description: Option<String>,
}

#[derive(Serialize)]
struct Category {
    id: i64,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    color: Option<String>,
    icon: Option<String>,
}

#[derive(Serialize)]
struct CatTotal {
    category: String,
    total: i64,
    color: Option<String>,
}

#[derive(Serialize)]
struct Summary {
    income: i64,
    expense: i64,
    by_category: Vec<CatTotal>,
}

#[derive(Deserialize, Default)]
struct Plan {
    #[serde(rename = "type")]
    kind: Option<String>,
    category: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
}

fn valid_date(d: &str) -> bool {
    let b = d.as_bytes();
    d.len() == 10 && b[4] == b'-' && b[7] == b'-' && d.chars().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

/// "1500,50" -> 150050. Работает только с целыми, без float.
fn parse_kopecks(input: &str) -> Result<i64, String> {
    let s: String = input.trim().replace(',', ".").chars().filter(|c| !c.is_whitespace()).collect();
    let (whole, frac) = s.split_once('.').unwrap_or((s.as_str(), ""));
    let digits = |p: &str| p.chars().all(|c| c.is_ascii_digit());
    if (whole.is_empty() && frac.is_empty()) || !digits(whole) || !digits(frac) || frac.len() > 2 {
        return Err("Введите сумму, например 1500,50".into());
    }
    let w: i64 = if whole.is_empty() { 0 } else { whole.parse().map_err(|_| "Сумма слишком большая".to_string())? };
    let f: i64 = format!("{:0<2}", frac).parse().unwrap_or(0);
    let total = w.checked_mul(100).and_then(|v| v.checked_add(f)).ok_or("Сумма слишком большая")?;
    if total <= 0 {
        return Err("Сумма должна быть больше нуля".into());
    }
    Ok(total)
}

fn rub(k: i64) -> String {
    format!("{}.{:02}", k / 100, k % 100)
}

#[tauri::command]
fn list_transactions(db: State<Db>, month: String) -> Result<Vec<Transaction>, String> {
    with_db(&db, |c| {
        let mut s = c
            .prepare("SELECT id, date, amount, type, category, description FROM transactions WHERE substr(date,1,7)=?1 ORDER BY date DESC, id DESC")
            .map_err(e)?;
        let rows = s
            .query_map(params![month], |r| {
                Ok(Transaction { id: r.get(0)?, date: r.get(1)?, amount: r.get(2)?, kind: r.get(3)?, category: r.get(4)?, description: r.get(5)? })
            })
            .map_err(e)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(e)?;
        Ok(rows)
    })
}

#[tauri::command]
fn add_transaction(db: State<Db>, t: NewTransaction) -> Result<i64, String> {
    if t.kind != "income" && t.kind != "expense" {
        return Err("Неверный тип операции".into());
    }
    if !valid_date(&t.date) {
        return Err("Неверная дата".into());
    }
    let category = t.category.trim().to_string();
    if category.is_empty() {
        return Err("Укажите категорию".into());
    }
    let amount = parse_kopecks(&t.amount)?;
    let desc = t.description.map(|d| d.trim().to_string()).filter(|d| !d.is_empty());
    with_db(&db, |c| {
        c.execute("INSERT OR IGNORE INTO categories(name, type, color) VALUES(?1, ?2, '#888780')", params![category, t.kind]).map_err(e)?;
        c.execute(
            "INSERT INTO transactions(date, amount, type, category, description) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![t.date, amount, t.kind, category, desc],
        )
        .map_err(e)?;
        Ok(c.last_insert_rowid())
    })
}

#[tauri::command]
fn delete_transaction(db: State<Db>, id: i64) -> Result<(), String> {
    with_db(&db, |c| c.execute("DELETE FROM transactions WHERE id=?1", params![id]).map(|_| ()).map_err(e))
}

#[tauri::command]
fn list_categories(db: State<Db>) -> Result<Vec<Category>, String> {
    with_db(&db, |c| {
        let mut s = c.prepare("SELECT id, name, type, color, icon FROM categories ORDER BY name").map_err(e)?;
        let rows = s
            .query_map([], |r| Ok(Category { id: r.get(0)?, name: r.get(1)?, kind: r.get(2)?, color: r.get(3)?, icon: r.get(4)? }))
            .map_err(e)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(e)?;
        Ok(rows)
    })
}

#[tauri::command]
fn summary(db: State<Db>, month: String) -> Result<Summary, String> {
    with_db(&db, |c| {
        let (income, expense): (i64, i64) = c
            .query_row(
                "SELECT COALESCE(SUM(CASE WHEN type='income' THEN amount END),0), COALESCE(SUM(CASE WHEN type='expense' THEN amount END),0) FROM transactions WHERE substr(date,1,7)=?1",
                params![month],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(e)?;
        let mut s = c
            .prepare("SELECT t.category, SUM(t.amount), k.color FROM transactions t LEFT JOIN categories k ON k.name=t.category WHERE t.type='expense' AND substr(t.date,1,7)=?1 GROUP BY t.category ORDER BY 2 DESC")
            .map_err(e)?;
        let by_category = s
            .query_map(params![month], |r| Ok(CatTotal { category: r.get(0)?, total: r.get(1)?, color: r.get(2)? }))
            .map_err(e)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(e)?;
        Ok(Summary { income, expense, by_category })
    })
}

async fn chat(model: &str, system: &str, user: &str, json: bool) -> Result<String, String> {
    let mut body = serde_json::json!({
        "model": model,
        "stream": false,
        "options": { "temperature": 0 },
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });
    if json {
        body["format"] = "json".into();
    }
    let resp = reqwest::Client::new()
        .post(format!("{OLLAMA}/api/chat"))
        .json(&body)
        .send()
        .await
        .map_err(|_| NO_OLLAMA.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Ollama вернула ошибку {}. Проверьте, что модель «{model}» загружена.", resp.status()));
    }
    let v: serde_json::Value = resp.json().await.map_err(e)?;
    Ok(v["message"]["content"].as_str().unwrap_or("").trim().to_string())
}

#[tauri::command]
async fn ollama_models() -> Result<Vec<String>, String> {
    let v: serde_json::Value = reqwest::get(format!("{OLLAMA}/api/tags"))
        .await
        .map_err(|_| NO_OLLAMA.to_string())?
        .json()
        .await
        .map_err(e)?;
    Ok(v["models"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| m["name"].as_str().map(String::from)).collect())
        .unwrap_or_default())
}

/// Вопрос -> (LLM) параметры запроса -> (Rust) безопасный SQL -> (LLM) ответ по готовым цифрам.
#[tauri::command]
async fn ask(db: State<'_, Db>, question: String, model: String, today: String) -> Result<String, String> {
    let cats: Vec<(String, String)> = with_db(&db, |c| {
        let mut s = c.prepare("SELECT name, type FROM categories ORDER BY name").map_err(e)?;
        let rows = s
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(e)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(e)?;
        Ok(rows)
    })?;

    let list = cats.iter().map(|(n, t)| format!("{n} ({t})")).collect::<Vec<_>>().join(", ");
    let planner = format!(
        "Ты превращаешь вопрос о личных финансах в JSON-запрос. Сегодня {today}. Категории: {list}. \
         Верни ТОЛЬКО JSON: {{\"type\": \"income\"|\"expense\"|null, \"category\": \"<точное название из списка>\"|null, \
         \"date_from\": \"YYYY-MM-DD\"|null, \"date_to\": \"YYYY-MM-DD\"|null}}. \
         Месяц без года — текущий год: date_from первый день месяца, date_to последний. Период не указан — null. \
         Вопрос про траты — type expense, про заработок — income."
    );
    let raw = chat(&model, &planner, &question, true).await?;
    let plan: Plan = serde_json::from_str(&raw).map_err(|_| "Модель вернула непонятный запрос. Переформулируйте вопрос.".to_string())?;

    let kind = plan.kind.filter(|k| k == "income" || k == "expense");
    let category = plan
        .category
        .and_then(|c| cats.iter().find(|(n, _)| n.to_lowercase() == c.to_lowercase()).map(|(n, _)| n.clone()));
    let from = plan.date_from.filter(|d| valid_date(d)).unwrap_or_else(|| "0000-01-01".into());
    let to = plan.date_to.filter(|d| valid_date(d)).unwrap_or_else(|| "9999-12-31".into());

    let rows: Vec<(String, String, i64, i64)> = with_db(&db, |c| {
        let mut s = c
            .prepare(
                "SELECT category, type, SUM(amount), COUNT(*) FROM transactions \
                 WHERE date>=?1 AND date<=?2 AND (?3 IS NULL OR type=?3) AND (?4 IS NULL OR category=?4) \
                 GROUP BY category, type ORDER BY 3 DESC",
            )
            .map_err(e)?;
        let rows = s
            .query_map(params![from, to, kind, category], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .map_err(e)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(e)?;
        Ok(rows)
    })?;

    let sum_of = |t: &str| rows.iter().filter(|r| r.1 == t).map(|r| r.2).sum::<i64>();
    let mut facts = format!("Период: {from} — {to}\n");
    for (cat, k, total, n) in &rows {
        let label = if k == "income" { "доход" } else { "расход" };
        facts += &format!("{label} / {cat}: {} руб., операций: {n}\n", rub(*total));
    }
    facts += &format!("Итого доходы: {} руб., расходы: {} руб.", rub(sum_of("income")), rub(sum_of("expense")));
    if rows.is_empty() {
        facts += "\nПодходящих операций нет.";
    }

    let writer = "Ты помощник по личным финансам. Отвечай по-русски, коротко, используя ТОЛЬКО цифры из данных. \
                  Ничего не выдумывай. Если операций нет — так и скажи.";
    chat(&model, writer, &format!("Вопрос: {question}\n\nДанные:\n{facts}"), false).await
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            let conn = Connection::open(dir.join("finance.db"))?;
            conn.execute_batch(SCHEMA)?;
            app.manage(Db(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_transactions,
            add_transaction,
            delete_transaction,
            list_categories,
            summary,
            ollama_models,
            ask
        ])
        .run(tauri::generate_context!())
        .expect("ошибка запуска приложения");
}
