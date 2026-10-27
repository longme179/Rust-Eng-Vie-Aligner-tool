//! embedder.rs — LaBSE (CỐT LÕI từ 2.10): nhúng câu bằng engine IN-PROCESS tự tải
//! (xem `labse_native.rs`), tự động dùng cho mọi lượt ghép.
//! Nếu người dùng đặt `REVA_LABSE_URL` (dịch vụ Python riêng, giao thức cũ) thì tôn trọng
//! đường HTTP đó; nếu không — engine native là mặc định.
//!
//! Giao thức HTTP (tuân phương bản trước, để tương thích):
//!   GET  /health                  -> {"status":"ok","model":"LaBSE","dim":768}
//!   POST /embed  {"texts":[...]}  -> {"embeddings":[[...], ...]}   (đã chuẩn hoá L2)
//!
//! Mọi thất bại (chưa tải xong / lỗi model / lỗi mạng) đều fallback êm ái về bộ từ vựng,
//! kèm `labse_error` để giao diện thông báo cho người dùng.

use crate::aligner::{self, AlignOptions, AlignOutput};
use crate::labse_native;
use serde::Deserialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const HEALTH_TTL: Duration = Duration::from_secs(30);
const HEALTH_TIMEOUT: Duration = Duration::from_secs(2);
const EMBED_TIMEOUT: Duration = Duration::from_secs(600); // CPU chậm + lô 32 đoạn vẫn đủ thời gian
const EMBED_BATCH: usize = 32; // mỗi request HTTP gửi tối đa 32 đoạn (đường service cũ)

/// Người dùng CÓ đặt REVA_LABSE_URL / CANH_DOI_LABSE_URL tường minh không?
fn http_url_overridden() -> bool {
    std::env::var("REVA_LABSE_URL").is_ok() || std::env::var("CANH_DOI_LABSE_URL").is_ok()
}

/// Địa chỉ dịch vụ LaBSE HTTP (đọc biến môi trường, bỏ dấu `/` cuối).
/// Ưu tiên REVA_LABSE_URL (bản 2.5+), giữ tương thích CANH_DOI_LABSE_URL cũ.
pub fn labse_url() -> String {
    std::env::var("REVA_LABSE_URL")
        .ok()
        .or_else(|| std::env::var("CANH_DOI_LABSE_URL").ok())
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:3031".to_string())
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout(timeout).build()
}

// ========================= Health check (có bộ đệm 30s) =========================
// Mỗi lần ghép không phải chịu timeout 2s khi LaBSE không chạy — kết quả được đệm 30 giây.

static HEALTH_CACHE: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

/// LaBSE có sẵn sàng không — engine native (in-process) hoặc dịch vụ HTTP (nếu người dùng
/// đặt URL tường minh). Thứ tự ưu tiên: native → HTTP (đệm 30s).
pub fn labse_ready() -> bool {
    if !http_url_overridden() {
        return labse_native::is_ready();
    }
    {
        let guard = HEALTH_CACHE.lock().expect("health cache lock");
        if let Some((at, ok)) = *guard {
            if at.elapsed() < HEALTH_TTL {
                return ok;
            }
        }
    }
    let ok = probe_health();
    *HEALTH_CACHE.lock().expect("health cache lock") = Some((Instant::now(), ok));
    ok
}

fn probe_health() -> bool {
    let url = format!("{}/health", labse_url());
    match agent(HEALTH_TIMEOUT).get(&url).call() {
        Ok(resp) => resp.status() == 200,
        Err(_) => false,
    }
}

/// Xoá bộ đệm health (dùng khi đổi `CANH_DOI_LABSE_URL` — selftest / tự kiểm).
pub fn invalidate_health_cache() {
    *HEALTH_CACHE.lock().expect("health cache lock") = None;
}

// ========================= Embed =========================

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

/// Nhúng danh sách đoạn — native trước (lô 16 trong engine), HTTP sau (lô 32, đường cũ).
/// Lỗi cả hai đường -> Err(chuỗi tiếng Việt).
pub fn embed(texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    if !http_url_overridden() {
        return labse_native::embed(texts);
    }
    embed_http(texts)
}

fn embed_http(texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    let a = agent(EMBED_TIMEOUT);
    let url = format!("{}/embed", labse_url());
    let mut out: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
    for chunk in texts.chunks(EMBED_BATCH) {
        let body = serde_json::json!({ "texts": chunk });
        let resp = a
            .post(&url)
            .send_json(body)
            .map_err(|e| format!("không gọi được dịch vụ LaBSE ({})", e))?;
        if resp.status() != 200 {
            return Err(format!("dịch vụ LaBSE trả mã HTTP {}", resp.status()));
        }
        let parsed: EmbedResponse = resp
            .into_json()
            .map_err(|e| format!("đọc phản hồi LaBSE không được ({})", e))?;
        if parsed.embeddings.len() != chunk.len() {
            return Err(format!(
                "LaBSE trả {} vector cho {} đoạn",
                parsed.embeddings.len(),
                chunk.len()
            ));
        }
        out.extend(parsed.embeddings);
    }
    Ok(out)
}

// ========================= Cosine =========================

fn cosine(a: &[f32], b: &[f32]) -> f64 {
    let n = a.len().min(b.len());
    let mut dot = 0.0f64;
    for i in 0..n {
        dot += a[i] as f64 * b[i] as f64;
    }
    let na: f64 = a.iter().map(|&x| (x * x) as f64).sum();
    let nb: f64 = b.iter().map(|&x| (x * x) as f64).sum();
    if na < 1e-12 || nb < 1e-12 {
        return 0.0;
    }
    (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
}

/// Ma trận cosine EN×VI (phẳng, hàng trước — cùng bố cục với `raw_scores` của aligner).
pub fn cosine_matrix(en: &[Vec<f32>], vi: &[Vec<f32>]) -> Vec<f64> {
    let mut m = vec![0.0f64; en.len() * vi.len()];
    for (i, a) in en.iter().enumerate() {
        for (j, b) in vi.iter().enumerate() {
            m[i * vi.len() + j] = cosine(a, b);
        }
    }
    m
}

// ========================= Pipeline dùng chung (/api/align + selftest) =========================

/// BẢN 2.5 — RESCAN: nhúng 2 văn bản (các khối của một hàng đã nối lại) và trả về
/// cosine giữa chúng. Trả None nếu LaBSE không chạy / lỗi (gọi rơi về điểm từ vựng).
pub fn pair_cosine(en_text: &str, vi_text: &str) -> Option<f64> {
    if !labse_ready() {
        return None;
    }
    let texts = vec![en_text.to_string(), vi_text.to_string()];
    match embed(&texts) {
        Ok(emb) if emb.len() == 2 => Some(cosine(&emb[0], &emb[1])),
        Ok(_) => None,
        Err(_) => {
            invalidate_health_cache(); // lần sau thử lại
            None
        }
    }
}

pub struct PipelineOutcome {
    pub output: AlignOutput,
    /// "lexical" hoặc "lexical+labse".
    pub engine: String,
    /// Nếu LaBSE từng được thử nhưng gặp lỗi (đã fallback về từ vựng) — mô tả lỗi.
    pub labse_error: Option<String>,
    /// Số ms dành cho phần LaBSE (0 nếu không chạy).
    pub labse_ms: u64,
}

/// Ghép hai văn bản: tự động dùng LaBSE nếu dịch vụ đang chạy, ngược lại thuần từ vựng.
/// Đây là MỘT nguồn sự thật cho cả /api/align và selftest — đảm bảo hai đường giống hệt nhau.
pub fn align_pipeline(
    english_text: &str,
    vietnamese_text: &str,
    options: &AlignOptions,
) -> Result<PipelineOutcome, aligner::AlignError> {
    let t0 = Instant::now();
    let mut engine = "lexical".to_string();
    let mut labse_error: Option<String> = None;
    let mut emb_cos: Option<Vec<f64>> = None;

    if labse_ready() {
        // Chỉ nhúng khi CẢ HAI bên đều có đoạn (một bên rỗng thì mọi hàng đều unmatched,
        // embedding không mang lại gì).
        let en_paras = aligner::split_paragraphs(english_text);
        let vi_paras = aligner::split_paragraphs(vietnamese_text);
        if !en_paras.is_empty() && !vi_paras.is_empty() {
            match (embed(&en_paras), embed(&vi_paras)) {
                (Ok(en_emb), Ok(vi_emb)) => {
                    emb_cos = Some(cosine_matrix(&en_emb, &vi_emb));
                    engine = "lexical+labse".to_string();
                }
                (Err(e), _) | (_, Err(e)) => {
                    labse_error = Some(e);
                    invalidate_health_cache(); // lần ghép sau thử lại
                }
            }
        }
    }
    let labse_ms = t0.elapsed().as_millis() as u64;
    let output =
        aligner::align_with_emb(english_text, vietnamese_text, options, emb_cos.as_deref())?;
    Ok(PipelineOutcome {
        output,
        engine,
        labse_error,
        labse_ms,
    })
}
