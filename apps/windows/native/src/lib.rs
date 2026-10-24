//! Saga native Windows host container.

mod database;
mod ingestion;
mod paths;
#[cfg(test)]
mod regression;
mod review;
mod storage;

use std::path::Path;

#[tauri::command]
fn ping() -> String {
    let sample = b"saga native host";
    let hash = protocol::hash(sample);
    format!("pong from native host; sample hash: {}", hash)
}

use protocol::{ImportProgress, NovelImportResult};
use tauri::{Emitter, Manager};

fn ingest(app: &tauri::AppHandle, path_str: String) -> Result<NovelImportResult, String> {
    // Emit at most one event per percentage point or phase transition.
    let mut previous = None;
    let mut report = |phase: &'static str, detail: &str, percent: u32| {
        if previous == Some((phase, percent)) {
            return;
        }
        previous = Some((phase, percent));
        let title = match phase {
            "parsing" => "正在分析",
            "validating" => "正在校验",
            "persisting" => "正在保存",
            "completed" => "正在准备阅读",
            _ => "正在导入",
        };
        let _ = app.emit(
            "import-progress",
            ImportProgress {
                phase: phase.into(),
                title: title.into(),
                detail: detail.into(),
                percent,
            },
        );
    };

    report("reading", "正在打开文件...", 0);
    let file_path = Path::new(&path_str);
    ingestion::validate(file_path)?;

    let is_saga = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("saga"));

    if is_saga {
        report("reading", "正在验证 .saga 归档格式与签名...", 20);
        let books_dir = paths::directory(app, "books")?;
        report("parsing", "正在解析 SGMF 调度元数据与章节分片...", 50);
        report("persisting", "正在挂载书籍与章节索引...", 80);
        let result = app
            .state::<database::Database>()
            .execute(|connection| storage::import_saga(connection, file_path, &books_dir))?;
        report("completed", "导入完成", 100);
        return Ok(result);
    }

    let file = std::fs::File::open(file_path).map_err(|e| format!("打开文件失败: {}", e))?;
    let size = file.metadata().map_err(|e| format!("读取文件信息失败: {}", e))?.len();
    let bytes = ingestion::read(file, size, |done, total| {
        let percent = ((u128::from(done.min(total)) * 20).checked_div(u128::from(total)).unwrap_or(0)) as u32;
        report(
            "reading",
            &format!("已读取 {:.1} / {:.1} MB", done as f64 / 1_048_576.0, total as f64 / 1_048_576.0),
            percent,
        );
    })
    .map_err(|e| format!("读取失败: {}", e))?;
    let title = file_path.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string());

    report("hashing", "正在计算文件校验值...", 20);
    let book_id = protocol::hash(&bytes);

    report("decoding", "正在识别文字编码...", 30);
    let decoded = reader::decode_bytes(&bytes).map_err(|e| e.to_string())?;
    drop(bytes);
    let novel = reader::parse_txt_str_with_progress(&decoded, &book_id, title.as_deref(), |done, total| {
        let percent = 35 + if total == 0 { 0 } else { (done as u64 * 40 / total as u64) as u32 };
        report(
            "parsing",
            &format!("已分析 {:.1} / {:.1} MB 文本", done as f64 / 1_048_576.0, total as f64 / 1_048_576.0),
            percent,
        );
    })
    .map_err(|e| e.to_string())?;
    drop(decoded);

    report("persisting", "正在打包 .saga 归档并持久化...", 75);
    let books_dir = paths::directory(app, "books")?;
    let validation = app
        .state::<database::Database>()
        .execute(|connection| storage::save(connection, &novel, "TXT", &books_dir))?;
    report("completed", "导入完成，正在进入...", 100);

    Ok(NovelImportResult { novel, validation })
}

#[tauri::command]
fn parse_txt(bytes: Vec<u8>, title: Option<String>) -> Result<protocol::Novel, String> {
    reader::parse_txt(&bytes, title.as_deref()).map_err(|e| e.to_string())
}

#[tauri::command]
async fn import_txt_file(app: tauri::AppHandle, path: String) -> Result<NovelImportResult, String> {
    tokio::task::spawn_blocking(move || ingest(&app, path))
        .await
        .map_err(|e| format!("后台导入任务异常: {}", e))?
}

#[tauri::command]
async fn pick_novel_file(window: tauri::Window) -> Result<Option<String>, String> {
    // The blocking join completes after the native dialog and COM objects are released.
    // Avoid rfd's custom asynchronous wakeup handoff on Windows.
    tauri::async_runtime::spawn_blocking(move || {
        rfd::FileDialog::new()
            .set_parent(&window)
            .set_title("选择电子书文件")
            .add_filter("支持的书籍文件 (*.txt, *.saga)", &["txt", "saga"])
            .pick_file()
            .map(|path| path.to_string_lossy().into_owned())
    })
    .await
    .map_err(|e| format!("文件选择任务异常: {}", e))
}

#[tauri::command]
async fn pick_file_and_import(app: tauri::AppHandle, window: tauri::Window) -> Result<Option<NovelImportResult>, String> {
    let Some(path_str) = pick_novel_file(window).await? else {
        return Ok(None);
    };
    let res = tokio::task::spawn_blocking(move || ingest(&app, path_str))
        .await
        .map_err(|e| format!("后台导入任务异常: {}", e))??;

    Ok(Some(res))
}

#[tauri::command]
async fn import_txt_bytes(app: tauri::AppHandle, bytes: Vec<u8>, title: Option<String>) -> Result<NovelImportResult, String> {
    if bytes.is_empty() {
        return Err("provided byte buffer is empty".to_string());
    }

    tokio::task::spawn_blocking(move || {
        let books_dir = paths::directory(&app, "books")?;

        let novel = reader::parse_txt(&bytes, title.as_deref()).map_err(|e| e.to_string())?;
        let validation = app
            .state::<database::Database>()
            .execute(|connection| storage::save(connection, &novel, "TXT", &books_dir))?;

        Ok(NovelImportResult { novel, validation })
    })
    .await
    .map_err(|e| format!("后台导入任务异常: {}", e))?
}

#[tauri::command]
async fn delete_novel(app: tauri::AppHandle, id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let books_dir = paths::directory(&app, "books")?;
        app.state::<database::Database>()
            .execute(|connection| storage::delete(connection, &id, &books_dir))
    })
    .await
    .map_err(|e| format!("后台删除失败: {}", e))?
}

#[tauri::command]
async fn load_book_summaries(app: tauri::AppHandle) -> Result<Vec<protocol::BookSummary>, String> {
    tokio::task::spawn_blocking(move || app.state::<database::Database>().execute(storage::summaries))
        .await
        .map_err(|e| format!("后台查询书籍失败: {}", e))?
}

#[tauri::command]
async fn load_novel(app: tauri::AppHandle, id: String) -> Result<Option<protocol::Novel>, String> {
    tokio::task::spawn_blocking(move || {
        let books_dir = paths::directory(&app, "books")?;
        app.state::<database::Database>()
            .execute(|connection| storage::load(connection, &id, &books_dir))
    })
    .await
    .map_err(|e| format!("后台读取小说详情失败: {}", e))?
}

#[tauri::command]
async fn load_novels(app: tauri::AppHandle) -> Result<Vec<protocol::Novel>, String> {
    tokio::task::spawn_blocking(move || {
        let books_dir = paths::directory(&app, "books")?;
        app.state::<database::Database>().execute(|connection| storage::all(connection, &books_dir))
    })
    .await
    .map_err(|e| format!("后台读取全部小说失败: {}", e))?
}

#[tauri::command]
async fn inspect_novel(app: tauri::AppHandle, id: String) -> Result<NovelImportResult, String> {
    tokio::task::spawn_blocking(move || {
        let books_dir = paths::directory(&app, "books")?;
        app.state::<database::Database>().execute(|connection| {
            let novel = storage::load(connection, &id, &books_dir)?.ok_or("book no longer exists")?;
            let validation = review::inspect(connection, &id, &novel.sections)?;
            Ok(NovelImportResult { novel, validation })
        })
    })
    .await
    .map_err(|error| format!("后台核验失败: {}", error))?
}

#[tauri::command]
async fn update_novel_sections(
    app: tauri::AppHandle,
    book_id: String,
    sections: Vec<protocol::Section>,
    retained: Vec<String>,
) -> Result<protocol::NovelValidation, String> {
    tokio::task::spawn_blocking(move || {
        let books_dir = paths::directory(&app, "books")?;
        app.state::<database::Database>()
            .execute(|connection| storage::update(connection, &book_id, &sections, &retained, &books_dir))
    })
    .await
    .map_err(|e| format!("后台更新章节失败: {}", e))?
}

#[tauri::command]
async fn update_reading_progress(app: tauri::AppHandle, book_id: String, chapter_index: u32, paragraph_index: u32, scroll_ratio: f64) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        app.state::<database::Database>()
            .execute(|connection| storage::update_progress(connection, &book_id, chapter_index, paragraph_index, scroll_ratio))
    })
    .await
    .map_err(|e| format!("后台更新阅读进度失败: {}", e))?
}

#[tauri::command]
async fn export_novel_saga(app: tauri::AppHandle, book_id: String, output_path: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let books_dir = paths::directory(&app, "books")?;
        app.state::<database::Database>()
            .execute(|connection| storage::export_saga(connection, &book_id, &books_dir, Path::new(&output_path)))
    })
    .await
    .map_err(|e| format!("后台导出任务异常: {}", e))?
}

#[tauri::command]
fn storage_location(app: tauri::AppHandle) -> Result<String, String> {
    Ok(paths::root(&app)?.to_string_lossy().into_owned())
}

/// Exports a consistent readable snapshot only after the user selects a JSON destination.
#[tauri::command]
async fn export_library_state(app: tauri::AppHandle, window: tauri::Window) -> Result<Option<String>, String> {
    let destination = tokio::task::spawn_blocking(move || {
        rfd::FileDialog::new()
            .set_parent(&window)
            .set_title("导出书架与阅读记录")
            .add_filter("JSON 文件", &["json"])
            .set_file_name("saga-library.json")
            .save_file()
    })
    .await
    .map_err(|error| format!("导出文件选择失败: {error}"))?;
    let Some(mut destination) = destination else {
        return Ok(None);
    };
    if destination.extension().is_none() {
        destination.set_extension("json");
    }
    tokio::task::spawn_blocking(move || {
        let bytes = app.state::<database::Database>().execute(storage::snapshot)?;
        storage::export(&bytes, &destination)?;
        Ok(Some(destination.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|error| format!("导出失败: {error}"))?
}

/// Runs the native host desktop application lifecycle.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let path = paths::directory(app.handle(), "storage")?.join("saga.db");
            app.manage(database::Database::open(&path)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            ping,
            storage_location,
            parse_txt,
            import_txt_file,
            import_txt_bytes,
            pick_file_and_import,
            pick_novel_file,
            delete_novel,
            update_novel_sections,
            inspect_novel,
            load_book_summaries,
            load_novel,
            load_novels,
            update_reading_progress,
            export_library_state,
            export_novel_saga
        ])
        .run(tauri::generate_context!())
        .expect("failed to execute saga desktop application");
}
