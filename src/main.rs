//! main.rs — REVA tool (reva-tool) v2.10
//! Web server 1-binary: giao diện + API, chạy 100% cục bộ.
//! Bản 2.10: LaBSE CỐT LÕI — tự tải model ONNX int8 (~450 MB, 1 lần duy nhất) + thư viện
//!           ONNX Runtime, nạp IN-PROCESS (không Python, không cài thêm gì), tự dùng cho
//!           mọi lượt ghép; thất bại thì trượt về bộ từ vựng như cũ.
//!           Hướng web: nhánh `vercel` trong repo (REVA Web + LaBSE transformers.js).
//! Bản 2.7: xuất Excel THẬT (.xlsx) tự tô màu ô theo trạng thái hàng (xanh/vàng/hồng
//!           nhạt) + đóng băng dòng đầu + autofilter + chú giải màu — CSV/TSV là văn
//!           bản thuần nên không mang màu được.
//! Bản 2.6: bộ trích URL 3 tầng lọc chrom (class/id tổ tiên, mật độ link,
//!           caption/credit ảnh) + flag dòng lệnh `--extract <file.html>` để soi
//!           nhanh các đoạn sẽ lấy từ 1 file HTML (kiểm tra/hủy trừ khi phát triển).
//!           LaBSE giờ lưu trong labse/model/ (tải 1 lần, build lại không tải lại).
//! Bản 2.5: mô hình KHỐI kéo–thả tự do (mỗi đoạn = 1 khối, đổi chỗ tuỳ ý) +
//!           hoàn tác/làm lại (Ctrl+Z) + Rescan LaBSE từng hàng + đổi tên REVA tool.
//! Bản 2.4: kéo–thả gộp hàng (EN ↔ VI) + API gộp hàng.
//! Bản 2.3: trích <li> khi tải URL + tô màu 3 mức + xuất CSV/HTML/XLIFF + LaBSE tuỳ chọn.

mod aligner;
mod embedder;
mod exporter;
mod fetcher;
mod fold;
mod labse_native;
mod lexicon;
mod qa;
mod selftest;
mod session;
mod stats;

use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const INDEX_HTML: &str = include_str!("../static/index.html");
const FAVICON_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64"><rect x="4" y="12" width="26" height="40" rx="6" fill="#0d9488"/><rect x="34" y="12" width="26" height="40" rx="6" fill="#d97706"/><path d="M22 32h20m0 0-6-6m6 6-6 6" stroke="#fff" stroke-width="4" fill="none" stroke-linecap="round" stroke-linejoin="round"/></svg>"##;

struct AppState {
    store: session::SessionStore,
    started_at: u64,
}

type SharedState = Arc<AppState>;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn ok_json(data: serde_json::Value) -> Response {
    Json(json!({ "ok": true, "data": data })).into_response()
}

fn err_json(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({ "ok": false, "error": msg }))).into_response()
}

// ========================= Handlers =========================

async fn root() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn favicon() -> Response {
    ([(header::CONTENT_TYPE, "image/svg+xml")], FAVICON_SVG).into_response()
}

async fn api_health() -> Response {
    ok_json(json!({ "status": "ok", "time": now_ms() }))
}

async fn api_model_status(State(state): State<SharedState>) -> Response {
    // Trạng thái LaBSE thật (native in-process ưu tiên; HTTP đệm 30s phía embedder).
    let labse_ready = tokio::task::spawn_blocking(embedder::labse_ready)
        .await
        .unwrap_or(false);
    let native = labse_native::status_json();
    let native_phase = native
        .get("phase")
        .and_then(|v| v.as_str())
        .unwrap_or("off");
    // BẢN 2.10.1 — đĩa chỉ đọc (Vercel/serverless): model lưu tạm /tmp → nói đúng
    // hệ quả "khởi động lại sẽ tải lại" thay vì hứa "1 lần duy nhất".
    let native_tmp = native.get("cache").and_then(|v| v.as_str()) == Some("tmp");
    let engine = if labse_ready {
        "lexical+labse"
    } else {
        "lexical"
    };
    let mode = if labse_ready {
        "Từ vựng + LaBSE — chế độ chính xác cao"
    } else if native_phase == "downloading" {
        if native_tmp {
            "Đang tải LaBSE vào đĩa tạm /tmp (đĩa hệ thống chỉ đọc — mỗi lần khởi động lại sẽ tải lại) — tạm dùng bộ từ vựng"
        } else {
            "Đang tải LaBSE (phần cốt lõi, 1 lần duy nhất) — tạm dùng bộ từ vựng"
        }
    } else if native_phase == "loading" {
        "Đang nạp LaBSE vào bộ nhớ — tạm dùng bộ từ vựng"
    } else if native_phase == "failed" {
        "Bộ từ vựng — LaBSE gặp lỗi (bấm huy hiệu để thử lại)"
    } else {
        "Từ vựng + số học + vị trí (offline — chưa bật LaBSE)"
    };
    ok_json(json!({
        "engine": engine,
        "mode": mode,
        "ready": true,
        "labse": {
            "ready": labse_ready,
            "url": embedder::labse_url(),
            "native": native,
        },
        "version": env!("CARGO_PKG_VERSION"),
        "uptimeSec": now_ms().saturating_sub(state.started_at) / 1000,
        "sessions": state.store.count(),
    }))
}

/// Thử tải/nạp lại LaBSE (nút trên UI khi lỗi).
async fn api_labse_retry() -> Response {
    let phase = labse_native::phase();
    match phase {
        labse_native::Phase::Failed(_) | labse_native::Phase::Off => {
            labse_native::retry();
            // Kèm phase (gồm cache: "tmp"/"env"/"data") để UI nói đúng nơi lưu model.
            ok_json(json!({ "ok": true, "started": true, "phase": labse_native::status_json() }))
        }
        _ => ok_json(json!({ "ok": true, "started": false, "phase": labse_native::status_json() })),
    }
}

#[derive(Deserialize)]
struct AlignRequest {
    #[serde(alias = "englishText", alias = "text_en", alias = "en")]
    english_text: String,
    #[serde(alias = "vietnameseText", alias = "text_vi", alias = "vi")]
    vietnamese_text: String,
    #[serde(alias = "autoThreshold", default)]
    auto_threshold: Option<f64>,
}

async fn api_align(
    State(state): State<SharedState>,
    body: Result<Json<AlignRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let req = match body {
        Ok(Json(r)) => r,
        Err(e) => {
            return err_json(
                StatusCode::BAD_REQUEST,
                &format!("Dữ liệu gửi lên không đúng: {}", e),
            )
        }
    };
    // Chạy bộ ghép (CPU) ngoài runtime — và BẮT PANIC để server không bao giờ chết
    // vì một bài văn lạ (đây là lớp phòng vệ 2, lớp 1 là aligner an toàn index).
    // align_pipeline TỰ dùng LaBSE nếu dịch vụ đang chạy, fallback từ vựng nếu không.
    let en_text = req.english_text.clone();
    let vi_text = req.vietnamese_text.clone();
    let opts = aligner::AlignOptions {
        auto_threshold: req.auto_threshold.unwrap_or(0.30),
    };
    let t0 = Instant::now();
    let result = tokio::task::spawn_blocking(move || {
        catch_unwind(AssertUnwindSafe(|| {
            embedder::align_pipeline(&en_text, &vi_text, &opts)
        }))
    })
    .await;
    let outcome = match result {
        Ok(Ok(Ok(outcome))) => outcome,
        Ok(Ok(Err(e))) => return err_json(StatusCode::BAD_REQUEST, &e.0),
        Ok(Err(panic)) => {
            let msg = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "lỗi không rõ".into());
            return err_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!(
                    "Bộ ghép gặp lỗi nội bộ (panic đã được bắt, server vẫn sống): {}",
                    msg
                ),
            );
        }
        Err(_) => return err_json(StatusCode::INTERNAL_SERVER_ERROR, "Tác vụ ghép bị hủy."),
    };
    let embedder::PipelineOutcome {
        output: aligned,
        engine,
        labse_error,
        labse_ms,
    } = outcome;

    // PHIÊN MỚI cho MỖI lần ghép — không tái dùng trạng thái cũ (sửa lỗi bài thứ 2)
    let title = session::derive_title(&aligned.english, &aligned.vietnamese);
    let mut s = session::Session::from_align(
        session::generate_id(),
        title,
        aligned.english,
        aligned.vietnamese,
        aligned.rows,
        aligned.options,
    );
    s.engine = engine; // "lexical" hoặc "lexical+labse" — lưu kèm phiên
    match state.store.create(&s) {
        Ok(created) => {
            println!(
                "[reva-tool] ghép xong phiên {} ({} hàng, {} ms, động cơ {}, LaBSE {} ms)",
                created.id,
                created.rows.len(),
                t0.elapsed().as_millis(),
                created.engine,
                labse_ms
            );
            let mut v = session_value(&created, true);
            if let Some(e) = &labse_error {
                v["labseError"] = json!(e); // thông báo fallback cho giao diện
            }
            ok_json(v)
        }
        Err(e) => err_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không lưu được phiên: {}", e),
        ),
    }
}

/// Chuyển Session thành JSON (kèm rows + counts + khả năng hoàn tác).
fn session_value(s: &session::Session, with_text: bool) -> serde_json::Value {
    let counts = s.counts();
    let mut v = json!({
        "id": s.id,
        "title": s.title,
        "createdAt": s.created_at,
        "updatedAt": s.updated_at,
        "counts": counts,
        "options": { "autoThreshold": s.options.auto_threshold },
        "engine": s.engine,
        "reordered": s.reordered,
        "canUndo": !s.undo_stack.is_empty(),
        "canRedo": !s.redo_stack.is_empty(),
        "rows": s.rows,
    });
    if with_text {
        v["english"] = json!(s.english);
        v["vietnamese"] = json!(s.vietnamese);
    }
    v
}

async fn api_sessions(State(state): State<SharedState>) -> Response {
    let list: Vec<serde_json::Value> = state
        .store
        .list()
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "title": s.title,
                "createdAt": s.created_at,
                "updatedAt": s.updated_at,
                "counts": s.counts(),
            })
        })
        .collect();
    ok_json(json!({ "sessions": list }))
}

async fn api_session_get(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    match state.store.get(&id) {
        Some(s) => ok_json(session_value(&s, true)),
        None => err_json(
            StatusCode::NOT_FOUND,
            "Không tìm thấy phiên (có thể đã bị nhả/xoá).",
        ),
    }
}

async fn api_session_delete(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    if state.store.delete(&id) {
        println!("[reva-tool] đã nhả phiên {}", id);
        ok_json(json!({ "released": id }))
    } else {
        err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên để nhả.")
    }
}

/// BẢN 2.9 — THỐNG KÊ SỐ TỪ / KÝ TỰ / ĐOẠN của một phiên (thuần đọc — không đụng
/// hàng, không snapshot hoàn tác). Căn cứ tính giá dịch thuật theo số từ nguồn
/// (chuẩn ngành — xem docs/PLAN-QOL-2026-10-19.md §1.2). Quy ước đếm: từ = tách
/// theo khoảng trắng Unicode; ký tự = scalar value sau NFC (NFD/NFC cùng con số).
async fn api_session_stats(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let s = match state.store.get(&id) {
        Some(s) => s,
        None => {
            return err_json(
                StatusCode::NOT_FOUND,
                "Không tìm thấy phiên để thống kê (có thể đã bị nhả/xoá).",
            )
        }
    };
    let rep = stats::report(&s);
    ok_json(serde_json::to_value(&rep).unwrap_or(serde_json::Value::Null))
}

/// BẢN 2.9 — BẢNG SOÁT LỖI QA (8 kiểm theo chuẩn ngành — Xbench): sai số liệu,
/// bỏ dịch (VI==EN), tỉ lệ độ dài bất thường, khoảng trắng thừa, ngoặc không cân,
/// tên riêng viết hoa mất, thiếu dấu tiếng Việt, NFD lẫn. Thuần đọc — KHÔNG tự
/// sửa gì; trả về danh sách {row, severity, check, en, vi, detail} để giao diện
/// cuộn tới từng hàng. Xem docs/PLAN-QOL-2026-10-19.md §3.2.
async fn api_session_qa(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let s = match state.store.get(&id) {
        Some(s) => s,
        None => {
            return err_json(
                StatusCode::NOT_FOUND,
                "Không tìm thấy phiên để soát lỗi (có thể đã bị nhả/xoá).",
            )
        }
    };
    let rep = qa::report(&s);
    ok_json(serde_json::to_value(&rep).unwrap_or(serde_json::Value::Null))
}

/// Body của POST /api/sessions/{id}/replace (BẢN 2.9 — F3 tìm/thay hàng loạt).
#[derive(Deserialize, Default)]
struct ReplaceRequest {
    find: String,
    #[serde(default)]
    replace: String,
    /// Cột áp dụng: "vi" (mặc định người dùng sửa bản dịch) hoặc "en".
    #[serde(default)]
    column: String,
    #[serde(default)]
    case_sensitive: bool,
}

/// BẢN 2.9 — THAY THẾ HÀNG LOẠT (F3): thay `find` → `replace` trong MỌI khối của
/// cột chọn, MỘT snapshot hoàn tác cho cả lô (Ctrl+Z trả nguyên xi); 0 chỗ khớp
/// → 200 changed:0, KHÔNG snapshot. Tài liệu gốc được đồng bộ theo hàng — xuất
/// file kiểm bất biến vẫn OK. Xem docs/PLAN-QOL-2026-10-19.md §3.3.
async fn api_session_replace(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    body: Result<Json<ReplaceRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let req = match body {
        Ok(Json(r)) => r,
        Err(e) => {
            return err_json(
                StatusCode::BAD_REQUEST,
                &format!("Dữ liệu không đúng: {}", e),
            )
        }
    };
    let side = match session::Side::parse(&req.column) {
        Some(s) => s,
        None => {
            return err_json(
                StatusCode::BAD_REQUEST,
                "Cột phải là \"vi\" (tiếng Việt) hoặc \"en\" (tiếng Anh).",
            )
        }
    };
    let mut s = match state.store.get(&id) {
        Some(s) => s,
        None => return err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên để thay thế."),
    };
    let out = match s.replace_all(side, &req.find, &req.replace, req.case_sensitive) {
        Ok(o) => o,
        Err(e) => return err_json(StatusCode::BAD_REQUEST, &e),
    };
    if out.changed > 0 {
        if let Err(e) = state.store.save(&s) {
            return err_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Không lưu được: {}", e),
            );
        }
    }
    let mut v = session_value(&s, false);
    v["changed"] = json!(out.changed);
    v["blocks"] = json!(out.blocks);
    v["rowsAffected"] = json!(out.rows);
    v["column"] = json!(side.as_str());
    ok_json(v)
}

async fn api_sessions_delete_all(State(state): State<SharedState>) -> Response {
    let n = state.store.delete_all();
    println!("[reva-tool] đã nhả tất cả {} phiên", n);
    ok_json(json!({ "releasedAll": n }))
}

#[derive(Deserialize)]
struct RowStatusRequest {
    #[serde(alias = "rowStatus")]
    status: String,
}

async fn api_row_status(
    State(state): State<SharedState>,
    Path((id, index)): Path<(String, usize)>,
    body: Result<Json<RowStatusRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let req = match body {
        Ok(Json(r)) => r,
        Err(e) => {
            return err_json(
                StatusCode::BAD_REQUEST,
                &format!("Dữ liệu không đúng: {}", e),
            )
        }
    };
    let status = match aligner::Status::parse(&req.status) {
        Some(s) => s,
        None => return err_json(
            StatusCode::BAD_REQUEST,
            "Trạng thái phải là một trong: auto_matched, needs_review, unmatched, human_confirmed.",
        ),
    };
    let mut s = match state.store.get(&id) {
        Some(s) => s,
        None => return err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên."),
    };
    if let Err(e) = s.set_row_status(index, status) {
        return err_json(StatusCode::BAD_REQUEST, &e);
    }
    if let Err(e) = state.store.save(&s) {
        return err_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không lưu được: {}", e),
        );
    }
    ok_json(session_value(&s, false))
}

#[derive(Deserialize, Default)]
struct RowMergeRequest {
    /// Tuỳ chọn: trạng thái đặt cho hàng sau khi gộp (mặc định needs_review).
    #[serde(default)]
    status: Option<String>,
}

/// GỘP hàng kế tiếp vào hàng `index` (kéo–thả gộp hàng trên giao diện — bản 2.4).
/// Chấp nhận body rỗng hoặc `{"status": null}` — mặc định needs_review.
async fn api_row_merge(
    State(state): State<SharedState>,
    Path((id, index)): Path<(String, usize)>,
    body: Option<Json<RowMergeRequest>>,
) -> Response {
    let req = body.map(|Json(r)| r).unwrap_or_default();
    let status = match req.status.as_deref() {
        None => None,
        Some(s) => match aligner::Status::parse(s) {
            Some(st) => Some(st),
            None => {
                return err_json(
                    StatusCode::BAD_REQUEST,
                    "Trạng thái phải là một trong: auto_matched, needs_review, unmatched, human_confirmed.",
                )
            }
        },
    };
    let mut s = match state.store.get(&id) {
        Some(s) => s,
        None => return err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên."),
    };
    if let Err(e) = s.merge_next(index, status) {
        return err_json(StatusCode::BAD_REQUEST, &e);
    }
    if let Err(e) = state.store.save(&s) {
        return err_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không lưu được: {}", e),
        );
    }
    println!(
        "[reva-tool] gộp hàng {} vào hàng {} (phiên {}, còn {} hàng)",
        index + 1,
        index,
        id,
        s.rows.len()
    );
    ok_json(session_value(&s, false))
}

// ========================= BẢN 2.5: khối kéo–thả + hoàn tác + rescan =========================

#[derive(Deserialize, Default)]
struct BlockMoveRequest {
    /// "en" hoặc "vi" — phía ngôn ngữ của khối.
    #[serde(alias = "blockSide", alias = "lang", alias = "block_side")]
    side: String,
    /// Hàng chứa khối đang cầm (0-based).
    #[serde(alias = "fromRow", alias = "fromRowIndex", alias = "from_row_index")]
    from_row: usize,
    /// Vị trí khối trong ô (0-based).
    #[serde(alias = "fromPos", alias = "from_pos")]
    from_pos: usize,
    /// Hàng đích (0-based). `null` = TÁCH thành hàng riêng mới sau `afterRow`.
    #[serde(default, alias = "toRow", alias = "to_row")]
    to_row: Option<usize>,
    /// Vị trí chèn trong ô đích (0..=số khối; thiếu/qua lớn = nối cuối).
    #[serde(default, alias = "toPos", alias = "to_pos")]
    to_pos: Option<usize>,
    /// Khi tách thành hàng riêng: chèn sau hàng này (mặc định = hàng nguồn).
    #[serde(default, alias = "afterRow", alias = "after_row")]
    after_row: Option<usize>,
}

/// DI CHUYỂN KHỐI: kéo–thả 1 đoạn sang ô cùng ngôn ngữ của hàng khác, đổi chỗ trong
/// cùng ô, hoặc tách thành hàng riêng — nền tảng mô hình khối bản 2.5 (đảo ngược
/// hoàn toàn được bằng /undo).
async fn api_block_move(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    body: Result<Json<BlockMoveRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let req = match body {
        Ok(Json(r)) => r,
        Err(e) => {
            return err_json(
                StatusCode::BAD_REQUEST,
                &format!("Dữ liệu không đúng: {}", e),
            )
        }
    };
    let side =
        match session::Side::parse(&req.side) {
            Some(s) => s,
            None => return err_json(
                StatusCode::BAD_REQUEST,
                "'side' phải là \"en\" hoặc \"vi\" — khối chỉ chuyển được sang ô cùng ngôn ngữ.",
            ),
        };
    let to_pos = req.to_pos.unwrap_or(usize::MAX); // thiếu = nối cuối ô đích
    let mut s = match state.store.get(&id) {
        Some(s) => s,
        None => return err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên."),
    };
    let focus = match s.move_block(
        side,
        req.from_row,
        req.from_pos,
        req.to_row,
        to_pos,
        req.after_row,
    ) {
        Ok(f) => f,
        Err(e) => return err_json(StatusCode::BAD_REQUEST, &e),
    };
    if let Err(e) = state.store.save(&s) {
        return err_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không lưu được: {}", e),
        );
    }
    println!(
        "[reva-tool] di chuyển khối {} phía {} từ hàng {} → {} (phiên {}, còn {} hàng)",
        req.from_pos,
        side.as_str(),
        req.from_row,
        req.to_row
            .map(|r| r.to_string())
            .unwrap_or_else(|| "hàng mới".into()),
        id,
        s.rows.len()
    );
    let mut v = session_value(&s, false);
    v["focusRow"] = json!(focus);
    ok_json(v)
}

/// HOÀN TÁC thao tác gần nhất (kéo khối / gộp / đổi trạng thái / rescan).
async fn api_undo(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let mut s = match state.store.get(&id) {
        Some(s) => s,
        None => return err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên."),
    };
    if let Err(e) = s.undo() {
        return err_json(StatusCode::BAD_REQUEST, &e);
    }
    if let Err(e) = state.store.save(&s) {
        return err_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không lưu được: {}", e),
        );
    }
    println!(
        "[reva-tool] hoàn tác (phiên {}, còn {} bước, {} hàng)",
        id,
        s.undo_stack.len(),
        s.rows.len()
    );
    ok_json(session_value(&s, false))
}

/// LÀM LẠI thao tác vừa hoàn tác.
async fn api_redo(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let mut s = match state.store.get(&id) {
        Some(s) => s,
        None => return err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên."),
    };
    if let Err(e) = s.redo() {
        return err_json(StatusCode::BAD_REQUEST, &e);
    }
    if let Err(e) = state.store.save(&s) {
        return err_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không lưu được: {}", e),
        );
    }
    println!(
        "[reva-tool] làm lại (phiên {}, còn {} bước, {} hàng)",
        id,
        s.redo_stack.len(),
        s.rows.len()
    );
    ok_json(session_value(&s, false))
}

/// RESCAN một hàng: chấm lại điểm khớp sau khi kéo–thả (LaBSE nếu dịch vụ chạy,
/// kèm sàn từ vựng — không LaBSE thì vẫn chấm được bằng từ vựng).
async fn api_row_rescan(
    State(state): State<SharedState>,
    Path((id, index)): Path<(String, usize)>,
) -> Response {
    let s = match state.store.get(&id) {
        Some(s) => s,
        None => return err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên."),
    };
    // chấm điểm ngoài runtime async (có thể gọi HTTP tới LaBSE + chạy CPU)
    let result = tokio::task::spawn_blocking(move || {
        catch_unwind(AssertUnwindSafe(|| {
            let mut s = s;
            let info: (f64, aligner::Status, bool) = s.rescan_row(index)?;
            Ok::<(session::Session, (f64, aligner::Status, bool)), String>((s, info))
        }))
    })
    .await;
    let (s, (score, status, labse_used)) = match result {
        Ok(Ok(Ok(pair))) => pair,
        Ok(Ok(Err(e))) => return err_json(StatusCode::BAD_REQUEST, &e),
        Ok(Err(panic)) => {
            let msg = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|x| x.to_string()))
                .unwrap_or_else(|| "lỗi không rõ".into());
            return err_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!(
                    "Rescan gặp lỗi nội bộ (panic đã được bắt, server vẫn sống): {}",
                    msg
                ),
            );
        }
        Err(_) => return err_json(StatusCode::INTERNAL_SERVER_ERROR, "Tác vụ rescan bị hủy."),
    };
    if let Err(e) = state.store.save(&s) {
        return err_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không lưu được: {}", e),
        );
    }
    println!(
        "[reva-tool] rescan hàng {} (phiên {}): {:.3} — {} ({})",
        index,
        id,
        score,
        status.as_str(),
        if labse_used {
            "LaBSE + từ vựng"
        } else {
            "từ vựng"
        }
    );
    let mut v = session_value(&s, false);
    v["rescan"] = json!({
        "row": index,
        "score": score,
        "status": status.as_str(),
        "labseUsed": labse_used,
    });
    ok_json(v)
}

/// BẢN 2.8 — RESCAN TOÀN BỘ: chấm lại điểm mọi hàng đủ hai phía (một thao tác
/// thay cho việc bấm Rescan từng hàng sau khi kéo–thả). Hoàn tác được 1 nhấn.
async fn api_rescan_all(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let s = match state.store.get(&id) {
        Some(s) => s,
        None => return err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên."),
    };
    let result = tokio::task::spawn_blocking(move || {
        catch_unwind(AssertUnwindSafe(|| {
            let mut s = s;
            let info: (usize, bool) = s.rescan_all()?;
            Ok::<(session::Session, (usize, bool)), String>((s, info))
        }))
    })
    .await;
    let (s, (count, labse_used)) = match result {
        Ok(Ok(Ok(pair))) => pair,
        Ok(Ok(Err(e))) => return err_json(StatusCode::BAD_REQUEST, &e),
        Ok(Err(panic)) => {
            let msg = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|x| x.to_string()))
                .unwrap_or_else(|| "lỗi không rõ".into());
            return err_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!(
                    "Rescan toàn bộ gặp lỗi nội bộ (panic đã được bắt, server vẫn sống): {}",
                    msg
                ),
            );
        }
        Err(_) => {
            return err_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Tác vụ rescan toàn bộ bị hủy.",
            )
        }
    };
    if let Err(e) = state.store.save(&s) {
        return err_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Không lưu được: {}", e),
        );
    }
    println!(
        "[reva-tool] rescan toàn bộ (phiên {}): {} hàng — {}",
        id,
        count,
        if labse_used {
            "LaBSE + từ vựng"
        } else {
            "từ vựng"
        }
    );
    let mut v = session_value(&s, false);
    v["rescanAll"] = json!({
        "count": count,
        "labseUsed": labse_used,
    });
    ok_json(v)
}

#[derive(Deserialize)]
struct ExportRequest {
    #[serde(alias = "sessionId")]
    session_id: String,
    #[serde(default)]
    format: Option<String>,
}

async fn api_export(
    State(state): State<SharedState>,
    body: Result<Json<ExportRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let req = match body {
        Ok(Json(r)) => r,
        Err(e) => {
            return err_json(
                StatusCode::BAD_REQUEST,
                &format!("Dữ liệu không đúng: {}", e),
            )
        }
    };
    let fmt = exporter::ExportFormat::parse(req.format.as_deref().unwrap_or("tsv"))
        .unwrap_or(exporter::ExportFormat::Tsv);
    let s = match state.store.get(&req.session_id) {
        Some(s) => s,
        None => return err_json(StatusCode::NOT_FOUND, "Không tìm thấy phiên để xuất."),
    };
    match exporter::export_session(&s, fmt) {
        Ok(r) => {
            println!(
                "[reva-tool] xuất {} ({} hàng, {} 'none')",
                r.filename, r.rows, r.none_count
            );
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, r.mime.clone()),
                    (
                        header::CONTENT_DISPOSITION,
                        format!("attachment; filename=\"{}\"", r.filename),
                    ),
                    (
                        header::HeaderName::from_static("x-rows"),
                        r.rows.to_string(),
                    ),
                    (
                        header::HeaderName::from_static("x-none-count"),
                        r.none_count.to_string(),
                    ),
                    (
                        header::HeaderName::from_static("x-session-id"),
                        s.id.clone(),
                    ),
                ],
                // xlsx là nhị phân; mọi định dạng khác là văn bản (exporter 2.7)
                match r.bytes {
                    Some(b) => b,
                    None => r.content.into_bytes(),
                },
            )
                .into_response()
        }
        Err(e) => err_json(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

#[derive(Deserialize)]
struct FetchUrlRequest {
    url: String,
}

async fn api_fetch_url(
    body: Result<Json<FetchUrlRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let req = match body {
        Ok(Json(r)) => r,
        Err(e) => {
            return err_json(
                StatusCode::BAD_REQUEST,
                &format!("Dữ liệu không đúng: {}", e),
            )
        }
    };
    let url = req.url.clone();
    let t0 = Instant::now();
    let result = tokio::task::spawn_blocking(move || {
        catch_unwind(AssertUnwindSafe(|| fetcher::fetch(&url)))
    })
    .await;
    match result {
        Ok(Ok(Ok(page))) => {
            println!(
                "[reva-tool] tải {} ({} đoạn, {} KB, {} ms)",
                page.url,
                page.paragraphs.len(),
                page.bytes / 1024,
                t0.elapsed().as_millis()
            );
            ok_json(json!({
                "url": page.url,
                "title": page.title,
                "paragraphs": page.paragraphs,
                "paragraphCount": page.paragraphs.len(),
                "bytes": page.bytes,
            }))
        }
        Ok(Ok(Err(e))) => err_json(StatusCode::BAD_GATEWAY, &e),
        Ok(Err(panic)) => {
            let msg = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "lỗi không rõ".into());
            err_json(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Lỗi nội bộ khi tải trang: {}", msg),
            )
        }
        Err(_) => err_json(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Tác vụ tải trang bị hủy.",
        ),
    }
}

async fn api_selftest() -> Response {
    let t0 = Instant::now();
    let results = tokio::task::spawn_blocking(selftest::run_all)
        .await
        .unwrap_or_default();
    let total = results.len();
    let passed = results.iter().filter(|r| r.passed).count();
    let failed = total - passed;
    println!(
        "[reva-tool] selftest: {}/{} đạt ({} ms)",
        passed,
        total,
        t0.elapsed().as_millis()
    );
    ok_json(json!({
        "total": total,
        "passed": passed,
        "failed": failed,
        "durationMs": t0.elapsed().as_millis() as u64,
        "tests": results,
    }))
}

#[derive(Deserialize, Default)]
struct DebugQuery {
    #[allow(dead_code)]
    #[serde(default)]
    _q: Option<String>,
}

async fn api_not_found(Query(_q): Query<DebugQuery>) -> Response {
    err_json(StatusCode::NOT_FOUND, "Không có API này. Các API: /api/align, /api/sessions, /api/export, /api/fetch-url, /api/selftest, /api/model-status, /api/health, /api/sessions/{id} (GET thông tin phiên · GET /stats thống kê số từ · GET /qa soát lỗi QA · DELETE nhả), /api/sessions/{id}/replace (POST thay thế hàng loạt), /api/sessions/{id}/rows/{index} (PATCH đổi trạng thái), /api/sessions/{id}/rows/{index}/merge (POST gộp hàng), /api/sessions/{id}/rows/{index}/rescan (POST chấm lại điểm), /api/sessions/{id}/rescan-all (POST chấm lại toàn bộ), /api/sessions/{id}/blocks/move (POST di chuyển khối), /api/sessions/{id}/undo và /api/sessions/{id}/redo (POST hoàn tác / làm lại).")
}

// ========================= Main =========================

fn resolve_data_dir() -> std::path::PathBuf {
    if let Ok(d) = std::env::var("REVA_DATA_DIR") {
        if !d.trim().is_empty() {
            return std::path::PathBuf::from(d);
        }
    }
    // tên CŨ (bản ≤2.4) — vẫn tôn trọng nếu người dùng đã đặt sẵn
    if let Ok(d) = std::env::var("CANH_DOI_DATA_DIR") {
        if !d.trim().is_empty() {
            return std::path::PathBuf::from(d);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let d = parent.join("data");
            if std::fs::create_dir_all(&d).is_ok() {
                return d;
            }
        }
    }
    let d = std::path::PathBuf::from("data");
    let _ = std::fs::create_dir_all(&d);
    d
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    println!("\n[reva-tool] đang dừng…");
}

#[tokio::main]
async fn main() {
    // ---- tiện ích dòng lệnh (bản 2.6) — chạy xong là thoát, không mở server ----
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!(
            "reva-tool {} (REVA tool — ghép & sắp đoạn Anh–Việt)",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }
    if let Some(pos) = args.iter().position(|a| a == "--extract") {
        // reva-tool --extract <file.html> : in tiêu đề + các đoạn sẽ lấy từ file HTML.
        // Dùng để kiểm tra bộ trích trên trang đã lưu (không cần mạng, không cần server).
        let path = args.get(pos + 1).cloned().unwrap_or_default();
        if path.is_empty() {
            eprintln!("Dùng: reva-tool --extract <file.html>");
            std::process::exit(2);
        }
        match std::fs::read_to_string(&path) {
            Ok(html) => {
                let (title, paras) = fetcher::extract(&html);
                println!("FILE:       {}", path);
                println!("TITLE:      {}", title);
                println!("PARAGRAPHS: {}", paras.len());
                for (i, p) in paras.iter().enumerate() {
                    println!("{:>3}| {}", i + 1, p);
                }
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Không đọc được {}: {}", path, e);
                std::process::exit(2);
            }
        }
    }

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.trim().parse().ok())
        .unwrap_or(3000);

    // [AUDIT 2026-10-18] mặc định CHỈ mở trên máy của chính mình (127.0.0.1) — đúng
    // lời hứa "chạy 100% cục bộ". Muốn chia sẻ trong mạng LAN: REVA_BIND=0.0.0.0.
    let bind_host = std::env::var("REVA_BIND")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "127.0.0.1".to_string());

    let data_dir = resolve_data_dir();
    let sessions_dir = data_dir.join("sessions");
    // LaBSE cốt lõi: truyền thư mục dữ liệu rồi bật tiến trình tự-tải + tự-nạp nền.
    labse_native::set_data_dir(data_dir.clone());
    labse_native::bootstrap();
    let store = session::SessionStore::open(sessions_dir);
    let state = Arc::new(AppState {
        store,
        started_at: now_ms(),
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/favicon.svg", get(favicon))
        .route("/api/health", get(api_health))
        .route("/api/model-status", get(api_model_status))
        .route("/api/labse/retry", post(api_labse_retry))
        .route("/api/align", post(api_align))
        .route(
            "/api/sessions",
            get(api_sessions).delete(api_sessions_delete_all),
        )
        .route(
            "/api/sessions/{id}",
            get(api_session_get).delete(api_session_delete),
        )
        // BẢN 2.9 — thống kê số từ (căn cứ tính giá, thuần đọc)
        .route("/api/sessions/{id}/stats", get(api_session_stats))
        // BẢN 2.9 — bảng soát lỗi QA 8 kiểm (thuần đọc, không tự sửa)
        .route("/api/sessions/{id}/qa", get(api_session_qa))
        // BẢN 2.9 — tìm/thay thế hàng loạt (1 snapshot cho cả lô, Ctrl+Z hoàn tác)
        .route("/api/sessions/{id}/replace", post(api_session_replace))
        .route("/api/sessions/{id}/rows/{index}", patch(api_row_status))
        .route("/api/sessions/{id}/rows/{index}/merge", post(api_row_merge))
        .route(
            "/api/sessions/{id}/rows/{index}/rescan",
            post(api_row_rescan),
        )
        .route("/api/sessions/{id}/rescan-all", post(api_rescan_all))
        .route("/api/sessions/{id}/blocks/move", post(api_block_move))
        .route("/api/sessions/{id}/undo", post(api_undo))
        .route("/api/sessions/{id}/redo", post(api_redo))
        .route("/api/export", post(api_export))
        .route("/api/fetch-url", post(api_fetch_url))
        .route("/api/selftest", get(api_selftest))
        .fallback(api_not_found)
        .layer(DefaultBodyLimit::max(16 * 1024 * 1024))
        .with_state(state);

    let addr = format!("{}:{}", bind_host, port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[reva-tool] KHÔNG mở được {}: {}", addr, e);
            eprintln!("[reva-tool] cổng có thể đang bận — thử: PORT=3001 ./reva-tool");
            std::process::exit(1);
        }
    };

    println!("[reva-tool] v{} — REVA tool, ghép & sắp đoạn văn bản Anh–Việt (tiền thân: Cánh đôi Anh–Việt)", env!("CARGO_PKG_VERSION"));
    println!(
        "[reva-tool] chạy tại http://localhost:{} (giao diện + API, tất cả cục bộ)",
        port
    );
    if bind_host != "127.0.0.1" {
        println!("[reva-tool] chú ý: đang lắng nghe trên {} — máy khác trong mạng có thể truy cập. Muốn chỉ mình mình dùng thì bỏ biến REVA_BIND rồi chạy lại.", bind_host);
    }
    println!("[reva-tool] thư mục dữ liệu: {}", data_dir.display());
    // Banner LaBSE: đúng theo chế độ thật — HTTP ngoài (REVA_LABSE_URL đặt tường minh)
    // hay native in-process (tự tải 1 lần duy nhất, chạy nền không chặn server).
    let labse_http_set =
        std::env::var("REVA_LABSE_URL").is_ok() || std::env::var("CANH_DOI_LABSE_URL").is_ok();
    if labse_http_set {
        println!(
            "[reva-tool] LaBSE (HTTP ngoài): {} — dùng dịch vụ riêng theo biến REVA_LABSE_URL",
            embedder::labse_url()
        );
    } else {
        println!("[reva-tool] LaBSE (native, phần cốt lõi): tự tải model ~450 MB đúng 1 lần rồi nạp chạy nền — xem huy hiệu trên đầu trang");
    }
    println!("[reva-tool] nhấn Ctrl+C để dừng");

    if let Err(e) = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
    {
        eprintln!("[reva-tool] lỗi server: {}", e);
        std::process::exit(1);
    }
    println!("[reva-tool] đã dừng hẳn.");
}
