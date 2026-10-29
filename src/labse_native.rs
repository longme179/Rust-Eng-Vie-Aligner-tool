//! labse_native.rs — LaBSE IN-PROCESS (PHẦN CỐT LÕI từ bản 2.10).
//!
//! LaBSE không còn là dịch vụ Python tuỳ chọn: binary tự
//!   1. TẢI model (Xenova/LaBSE — ONNX int8, ~450 MB) + thư viện ONNX Runtime
//!      (~11 MB Linux / ~79 MB Windows / ~35 MB macOS) về `<data>/models/` — CHỈ MỘT LẦN,
//!      tự nối lại nếu mạng đứt giữa chừng (HTTP Range);
//!   2. NẠP model ngay trong tiến trình (crate `ort`, dynamic-link — không Python,
//!      không Node, không cài đặt thêm bất kỳ thứ gì);
//!   3. Nhúng câu cho bộ ghép hybrid `max(từ vựng, 0.4·từ vựng + 0.6·LaBSE)`.
//!
//! Mọi bước đều báo tiến trình qua `phase()` cho `/api/model-status` + huy hiệu UI.
//! Thất bại nào (mạng chậm, đĩa đầy…) cũng trượt về bộ từ vựng như mọi bản trước —
//! app KHÔNG BAO GIỜ chết vì LaBSE.
//!
//! Biến môi trường:
//!   REVA_LABSE_URL    — nếu đặt tường minh: giữ đường HTTP cũ (service Python riêng), không tự tải
//!   REVA_LABSE_AUTO=0 — tắt tự tải (gọi tay / máy không có mạng)
//!   REVA_LABSE_MAXLEN — số token tối đa mỗi đoạn (mặc định 256, 16..=512)
//!   REVA_LABSE_DIR    — thư mục GỐC chứa model (labse/ + ort/); mặc định <data>/models.
//!                       BẢN 2.10.1: đĩa chỉ đọc (Vercel/serverless, Docker ro) → REVA tự trượt
//!                       xuống thư mục tạm của hệ điều hành (/tmp) thay vì chết với
//!                       "Read-only file system (os error 30)".

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ort::session::{builder::GraphOptimizationLevel, Session};
use tokenizers::{
    PaddingDirection, PaddingParams, PaddingStrategy, Tokenizer, TruncationDirection,
    TruncationParams, TruncationStrategy,
};

// ========================= Nguồn tải =========================

const HF_BASE: &str = "https://huggingface.co/Xenova/LaBSE/resolve/main";
const ORT_VER: &str = "1.31.1";

fn ort_archive_url() -> (&'static str, &'static str) {
    if cfg!(windows) {
        (
            "https://github.com/microsoft/onnxruntime/releases/download/v1.31.1/onnxruntime-win-x64-1.31.1.zip",
            "onnxruntime-win-x64.zip",
        )
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            (
                "https://github.com/microsoft/onnxruntime/releases/download/v1.31.1/onnxruntime-osx-arm64-1.31.1.tgz",
                "onnxruntime-osx.tgz",
            )
        } else {
            (
                "https://github.com/microsoft/onnxruntime/releases/download/v1.28.3/onnxruntime-osx-x64-1.28.3.tgz",
                "onnxruntime-osx.tgz",
            )
        }
    } else {
        (
            "https://github.com/microsoft/onnxruntime/releases/download/v1.31.1/onnxruntime-linux-x64-1.31.1.tgz",
            "onnxruntime-linux-x64.tgz",
        )
    }
}

fn dylib_name() -> &'static str {
    if cfg!(windows) {
        "onnxruntime.dll"
    } else if cfg!(target_os = "macos") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    }
}

// ========================= Trạng thái ( cho /api/model-status + UI) =========================

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Off,
    Downloading { file: String, pct: u8 },
    Loading,
    Ready,
    Failed(String),
}

static PHASE: Mutex<Phase> = Mutex::new(Phase::Off);
/// Engine sống cả quá trình server chạy; có thể thay khi retry.
static ENGINE: Mutex<Option<Arc<Mutex<Engine>>>> = Mutex::new(None);
static DATA_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
/// Chỉ MỘT bootstrap chạy tại một thời điểm.
static BOOTSTRAP_LOCK: Mutex<()> = Mutex::new(());

pub fn set_data_dir(d: PathBuf) {
    if let Ok(mut g) = DATA_DIR.lock() {
        *g = Some(d);
    }
    invalidate_root();
}

fn data_dir() -> PathBuf {
    DATA_DIR
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_else(|| PathBuf::from("data"))
}

// ========================= BẢN 2.10.1: chọn NƠI LƯU model =========================
//
// Vercel/serverless chạy binary trên hệ thống tệp CHỈ ĐỌC (chỉ /tmp được ghi):
// bản 2.10.0 chết ngay ở "tạo thư mục model: Read-only file system (os error 30)".
// Giờ REVA tự chọn nơi lưu, theo thứ tự:
//   1. REVA_LABSE_DIR (env tường minh) — nếu đã đủ file (chỉ cần đọc) hoặc ghi được;
//   2. <data>/models (mặc định)        — nếu đã đủ file hoặc ghi được;
//   3. thư mục tạm của hệ điều hành     — đĩa chỉ đọc: LaBSE VẪN CHẠY, nhưng mỗi lần
//      khởi động lại server sẽ tải lại model — status_json báo rõ cache:"tmp" + note.

static ROOT: Mutex<Option<(PathBuf, u8)>> = Mutex::new(None);

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

/// Đã đủ 3 file model hợp lệ ở gốc này? (chỉ cần QUYỀN ĐỌC — không cần ghi)
fn root_complete(root: &Path) -> bool {
    let model = root.join("labse").join("model.onnx");
    let tok = root.join("labse").join("tokenizer.json");
    let dylib = root.join("ort").join(dylib_name());
    model.is_file()
        && std::fs::metadata(&model)
            .map(|m| m.len() > 100 * 1024 * 1024)
            .unwrap_or(false)
        && tok.is_file()
        && dylib.is_file()
        && std::fs::metadata(&dylib)
            .map(|m| m.len() > 1024 * 1024)
            .unwrap_or(false)
}

fn root_writable(root: &Path) -> bool {
    if std::fs::create_dir_all(root).is_err() {
        return false;
    }
    let probe = root.join(".reva-ghi-duoc");
    let ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

/// (gốc, nguồn) — nguồn: 0 = <data>/models · 1 = REVA_LABSE_DIR · 2 = thư mục tạm.
fn resolve_root() -> (PathBuf, u8) {
    if let Some(p) = env_path("REVA_LABSE_DIR") {
        if root_complete(&p) || root_writable(&p) {
            return (p, 1);
        }
    }
    let d = data_dir().join("models");
    if root_complete(&d) || root_writable(&d) {
        return (d, 0);
    }
    // Đĩa chỉ đọc (Vercel/serverless, Docker read-only…) → thư mục tạm.
    (std::env::temp_dir().join("reva-models"), 2)
}

/// Gốc model hiện dùng (cache trong tiến trình; đổi khi set_data_dir / retry).
fn root() -> (PathBuf, u8) {
    if let Some(r) = ROOT.lock().ok().and_then(|g| g.clone()) {
        return r;
    }
    let r = resolve_root();
    if let Ok(mut g) = ROOT.lock() {
        *g = Some(r.clone());
    }
    r
}

fn invalidate_root() {
    if let Ok(mut g) = ROOT.lock() {
        *g = None;
    }
}

pub fn labse_dir() -> PathBuf {
    root().0.join("labse")
}

fn ort_dir() -> PathBuf {
    root().0.join("ort")
}

pub fn phase() -> Phase {
    PHASE.lock().map(|p| p.clone()).unwrap_or(Phase::Off)
}

fn set_phase(p: Phase) {
    if let Ok(mut g) = PHASE.lock() {
        *g = p;
    }
}

pub fn is_ready() -> bool {
    if phase() != Phase::Ready {
        return false;
    }
    ENGINE.lock().map(|g| g.is_some()).unwrap_or(false)
}

/// Trạng thái chi tiết cho /api/model-status.
pub fn status_json() -> serde_json::Value {
    let p = phase();
    let (state, pct, detail) = match &p {
        Phase::Off => ("off", 0u8, None),
        Phase::Downloading { file, pct } => ("downloading", *pct, Some(file.clone())),
        Phase::Loading => ("loading", 100u8, None),
        Phase::Ready => ("ready", 100u8, None),
        Phase::Failed(e) => ("failed", 0u8, Some(e.clone())),
    };
    let (root_path, src) = root();
    let cache = match src {
        1 => "env",
        2 => "tmp",
        _ => "data",
    };
    let mut v = serde_json::json!({
        "phase": state,
        "pct": pct,
        "detail": detail,
        "modelDir": root_path.join("labse").to_string_lossy(),
        "model": "Xenova/LaBSE (ONNX int8)",
        "ortVersion": ORT_VER,
        "cache": cache,
    });
    if src == 2 {
        v["note"] = serde_json::json!(
            "Đĩa của thư mục dữ liệu CHỈ ĐỌC — model lưu tạm trong thư mục tạm của hệ điều hành: \
             mỗi lần khởi động lại server sẽ TẢI LẠI model (~450 MB). Trên Vercel/serverless \
             nên dùng bản web (nhánh 'vercel') hoặc trỏ REVA_LABSE_URL sang dịch vụ nhúng riêng."
        );
    }
    v
}

// ========================= Tải file (resume được) =========================

fn http_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(900))
        .redirects(8)
        .build()
}

/// Diễn giải lỗi đĩa thành tiếng Việt người dùng cuối hiểu được (bản 2.10.1 —
/// trước đây chỉ in "Read-only file system (os error 30)" khô khan).
fn explain_io(e: &std::io::Error) -> String {
    let s = e.to_string();
    // EROFS = 30 trên Linux/macOS; một số nền tảng chỉ có chữ trong message.
    let ro = e.raw_os_error() == Some(30) || s.contains("Read-only file system");
    if ro {
        return format!(
            "{s} — hệ thống tệp chỉ đọc: server không ghi được vào thư mục dữ liệu \
             (Vercel/serverless chỉ ghi được trong /tmp). REVA sẽ tự dùng thư mục tạm, \
             hoặc đặt REVA_LABSE_DIR tới thư mục ghi được"
        );
    }
    // ENOSPC = 28 (Linux/macOS) · ERROR_DISK_FULL = 112 (Windows).
    let full = e.raw_os_error() == Some(28)
        || e.raw_os_error() == Some(112)
        || s.contains("No space left");
    if full {
        return format!("{s} — hết chỗ chứa: model LaBSE cần khoảng 500 MB trống");
    }
    s
}

fn download_file(url: &str, dest: &Path, label: &str) -> Result<(), String> {
    let part = dest.with_extension("part_file");
    let agent = http_agent();

    let mut tries = 0u8;
    loop {
        tries += 1;
        let have = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        let mut req = agent.get(url);
        if have > 0 {
            req = req.set("Range", &format!("bytes={}-", have));
        }
        let resp = req.call().map_err(|e| format!("tải {}: {}", label, e))?;
        let status = resp.status();
        let append = status == 206 && have > 0;
        if status != 200 && status != 206 {
            return Err(format!("tải {} trả HTTP {}", label, status));
        }
        let chunk_len: u64 = resp
            .header("Content-Length")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let grand_total = if append { have + chunk_len } else { chunk_len };

        let mut reader = resp.into_reader();
        let mut buf = [0u8; 256 * 1024];
        let mut downloaded = if append { have } else { 0u64 };
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&part)
            .map_err(|e| format!("mở file tạm: {}", explain_io(&e)))?;
        let mut since_update = 0u64;
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("đọc dữ liệu {}: {}", label, e))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])
                .map_err(|e| format!("ghi đĩa: {}", explain_io(&e)))?;
            downloaded += n as u64;
            since_update += n as u64;
            if since_update >= 1024 * 1024 {
                since_update = 0;
                let pct = if grand_total > 0 {
                    ((downloaded as f64 / grand_total as f64) * 100.0).clamp(0.0, 99.0) as u8
                } else {
                    0
                };
                set_phase(Phase::Downloading {
                    file: label.to_string(),
                    pct,
                });
            }
        }
        drop(out);
        let done = std::fs::metadata(&part).map(|m| m.len()).unwrap_or(0);
        if grand_total > 0 && done < grand_total && tries < 6 {
            // mạng đứt giữa chừng — ngủ 2s rồi tự NỐI lại từ chỗ dừng.
            set_phase(Phase::Downloading {
                file: format!("{} (nối lại…)", label),
                pct: ((done as f64 / grand_total as f64) * 100.0).clamp(0.0, 99.0) as u8,
            });
            std::thread::sleep(Duration::from_secs(2));
            continue;
        }
        std::fs::rename(&part, dest).map_err(|e| format!("lưu {}: {}", label, explain_io(&e)))?;
        return Ok(());
    }
}

// ========================= Giải nén thư viện ORT =========================

fn extract_dylib(archive: &Path, dylib_dest: &Path) -> Result<(), String> {
    if cfg!(windows) {
        let f = std::fs::File::open(archive).map_err(|e| format!("mở zip ORT: {}", e))?;
        let mut z = zip::ZipArchive::new(f).map_err(|e| format!("đọc zip ORT: {}", e))?;
        for i in 0..z.len() {
            let mut entry = z.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().to_string();
            // Bỏ qua thư mục + entry 0 byte (hardlink hoá thân trong zip hiếm gặp).
            if !name.ends_with('/')
                && name.ends_with("onnxruntime.dll")
                && entry.size() > 1024 * 1024
            {
                let mut out =
                    std::fs::File::create(dylib_dest).map_err(|e| format!("ghi dll: {}", e))?;
                std::io::copy(&mut entry, &mut out).map_err(|e| format!("giải nén dll: {}", e))?;
                return Ok(());
            }
        }
        Err("không thấy onnxruntime.dll trong zip".into())
    } else {
        // Gói ORT cho Linux/macOS dùng SYMLINK: libonnxruntime.so → libonnxruntime.so.1
        // → libonnxruntime.so.X.Y.Z (file thật). Lấy FILE THẬT (không link, > 1 MB)
        // tên khớp gốc, rồi lưu dưới tên chuẩn để `ort::init_from` dlopen được.
        let stem: &str = if cfg!(target_os = "macos") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        };
        let f = std::fs::File::open(archive).map_err(|e| format!("mở tgz ORT: {}", e))?;
        let gz = flate2::read::GzDecoder::new(f);
        let mut tar = tar::Archive::new(gz);
        let mut found = false;
        let mut best_size = 0u64;
        for entry in tar.entries().map_err(|e| format!("đọc tgz: {}", e))? {
            let mut entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path().map_err(|e| e.to_string())?.to_path_buf();
            let base = path
                .file_name()
                .map(|b| b.to_string_lossy().to_string())
                .unwrap_or_default();
            let is_link = matches!(
                entry.header().entry_type(),
                tar::EntryType::Link | tar::EntryType::Symlink
            );
            let size = entry.size();
            let matches = !is_link
                && size > 1024 * 1024
                && (base == stem || base.starts_with(&format!("{}.", stem)));
            if matches && size >= best_size {
                let mut out = std::fs::File::create(dylib_dest)
                    .map_err(|e| format!("ghi {}: {}", stem, e))?;
                std::io::copy(&mut entry, &mut out)
                    .map_err(|e| format!("giải nén {}: {}", stem, e))?;
                best_size = size;
                found = true;
            }
        }
        if found {
            Ok(())
        } else {
            Err(format!(
                "không thấy file thật {} trong tgz (chỉ symlink?)",
                stem
            ))
        }
    }
}

// ========================= Engine =========================

pub struct Engine {
    session: Session,
    tokenizer: Tokenizer,
    need_token_type: bool,
    /// 0 = output "embeddings" (pool sẵn) · 1 = "pooler_output" · 2 = mean-pool "last_hidden_state".
    out_kind: u8,
}

fn max_len() -> usize {
    std::env::var("REVA_LABSE_MAXLEN")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&v| (16..=512).contains(&v))
        .unwrap_or(256)
}

impl Engine {
    fn load(model: &Path, tokenizer_path: &Path, dylib: &Path) -> Result<Engine, String> {
        let env = ort::init_from(dylib)
            .map_err(|e| format!("nạp {}: {}", dylib.display(), e))?
            .build()
            .map_err(|e| format!("khởi tạo ONNX Runtime: {}", e))?;
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .max(1);
        let session = Session::builder(&env)
            .map_err(|e| format!("builder: {}", e))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| format!("cấu hình ORT: {}", e))?
            .with_intra_threads(threads)
            .map_err(|e| format!("cấu hình luồng ORT: {}", e))?
            .commit_from_file(model)
            .map_err(|e| format!("nạp model LaBSE: {}", e))?;

        let need_token_type = session
            .inputs()
            .iter()
            .any(|i| i.name().contains("token_type"));
        let mut out_kind = 2u8;
        for o in session.outputs() {
            let n = o.name();
            if n == "embeddings" {
                out_kind = 0;
                break;
            } else if n == "pooler_output" {
                out_kind = 1;
            }
        }

        let mut tokenizer =
            Tokenizer::from_file(tokenizer_path).map_err(|e| format!("nạp tokenizer: {}", e))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                direction: TruncationDirection::Right,
                max_length: max_len(),
                strategy: TruncationStrategy::LongestFirst,
                stride: 0,
            }))
            .map_err(|e| format!("cấu hình truncate: {}", e))?;
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            direction: PaddingDirection::Right,
            pad_to_multiple_of: None,
            pad_id: 0,
            pad_type_id: 0,
            pad_token: String::from("[PAD]"),
        }));

        Ok(Engine {
            session,
            tokenizer,
            need_token_type,
            out_kind,
        })
    }

    /// Nhúng (chia lô 16) → vector chuẩn hoá L2. Lỗi → chuỗi tiếng Việt.
    pub fn embed(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let mut out: Vec<Vec<f32>> = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(16) {
            let encodings = self
                .tokenizer
                .encode_batch(chunk.to_vec(), true)
                .map_err(|e| format!("tokenize: {}", e))?;
            let batch = encodings.len();
            let seq = encodings
                .iter()
                .map(|e| e.get_ids().len())
                .max()
                .unwrap_or(1);
            let mut input_ids = vec![0i64; batch * seq];
            let mut attention = vec![0i64; batch * seq];
            let mut type_ids = vec![0i64; batch * seq];
            for (b, e) in encodings.iter().enumerate() {
                let ids = e.get_ids();
                let mask = e.get_attention_mask();
                let types = e.get_type_ids();
                for s in 0..seq {
                    let k = b * seq + s;
                    if s < ids.len() {
                        input_ids[k] = ids[s] as i64;
                        attention[k] = if s < mask.len() { mask[s] as i64 } else { 1 };
                        type_ids[k] = if s < types.len() { types[s] as i64 } else { 0 };
                    }
                }
            }
            let ids_t = ort::value::Tensor::<i64>::from_array((vec![batch, seq], input_ids))
                .map_err(|e| format!("tensor input_ids: {}", e))?;
            let mask_t =
                ort::value::Tensor::<i64>::from_array((vec![batch, seq], attention.clone()))
                    .map_err(|e| format!("tensor mask: {}", e))?;
            let outputs = if self.need_token_type {
                let type_t = ort::value::Tensor::<i64>::from_array((vec![batch, seq], type_ids))
                    .map_err(|e| format!("tensor type: {}", e))?;
                self.session
                    .run(ort::inputs![
                        "input_ids" => &ids_t,
                        "attention_mask" => &mask_t,
                        "token_type_ids" => &type_t
                    ])
                    .map_err(|e| format!("chạy LaBSE: {}", e))?
            } else {
                self.session
                    .run(ort::inputs![
                        "input_ids" => &ids_t,
                        "attention_mask" => &mask_t
                    ])
                    .map_err(|e| format!("chạy LaBSE: {}", e))?
            };

            let pick = |name: &str| -> Option<Vec<f32>> {
                let v = outputs.get(name)?;
                let (_, data) = v.try_extract_tensor::<f32>().ok()?;
                Some(data.to_vec())
            };
            let raw = pick("embeddings")
                .or_else(|| pick("pooler_output"))
                .or_else(|| pick("last_hidden_state"))
                .ok_or_else(|| "LaBSE không trả tensor đầu ra".to_string())?;
            if raw.is_empty() || raw.len() % batch != 0 {
                return Err("kích thước đầu ra LaBSE lạ".to_string());
            }
            if self.out_kind == 2 {
                // [B, S, D] → mean-pool theo attention mask.
                let dim = raw.len() / batch;
                if dim == 0
                    || raw.len() % (batch * dim) != 0
                    || dim % 768 != 0 && dim % 384 != 0 && dim < 64
                {
                    return Err("đầu ra last_hidden có chiều lạ".to_string());
                }
                // dim = S*D → tách:
                let sd = dim; // seq*D
                              // suy D từ số token thật không chắc chắn — dùng attention đếm:
                              // D = sd / seq (seq đã pad BatchLongest → = max tokens). an toàn nhất:
                if sd % seq != 0 {
                    return Err("shape last_hidden không chia hết cho seq".to_string());
                }
                let d = sd / seq;
                for b in 0..batch {
                    let mut acc = vec![0f64; d];
                    let mut count = 0f64;
                    for s in 0..seq {
                        if attention[b * seq + s] == 0 {
                            continue;
                        }
                        count += 1.0;
                        let base = (b * seq + s) * d;
                        for k in 0..d {
                            acc[k] += raw[base + k] as f64;
                        }
                    }
                    let v: Vec<f32> = acc
                        .iter()
                        .map(|&x| if count > 0.0 { (x / count) as f32 } else { 0.0 })
                        .collect();
                    out.push(l2(v));
                }
            } else {
                let dim = raw.len() / batch;
                for b in 0..batch {
                    out.push(l2(raw[b * dim..(b + 1) * dim].to_vec()));
                }
            }
        }
        Ok(out)
    }
}

fn l2(mut v: Vec<f32>) -> Vec<f32> {
    let n: f64 = v.iter().map(|&x| (x * x) as f64).sum();
    if n > 1e-12 {
        let inv = (1.0 / n.sqrt()) as f32;
        for x in v.iter_mut() {
            *x *= inv;
        }
    }
    v
}

// ========================= Bootstrap: tự fetch + tự nạp =========================

fn files_ok() -> Option<(PathBuf, PathBuf, PathBuf)> {
    let model = labse_dir().join("model.onnx");
    let tok = labse_dir().join("tokenizer.json");
    let dylib = ort_dir().join(dylib_name());
    let ok = model.is_file()
        && model
            .metadata()
            .map(|m| m.len() > 100 * 1024 * 1024)
            .unwrap_or(false)
        && tok.is_file()
        && dylib.is_file()
        && dylib
            .metadata()
            .map(|m| m.len() > 1024 * 1024)
            .unwrap_or(false);
    if ok {
        Some((model, tok, dylib))
    } else {
        None
    }
}

fn install_files() -> Result<(), String> {
    let (root_path, src) = root();
    if src == 2 {
        // Đĩa chỉ đọc (Vercel/serverless…) — nói rõ nơi lưu + hệ quả cho log/banner.
        println!(
            "[reva-tool] LaBSE: đĩa dữ liệu chỉ đọc — lưu model vào thư mục tạm {} (mỗi lần khởi động lại sẽ tải lại ~450 MB)",
            root_path.display()
        );
    }
    std::fs::create_dir_all(labse_dir())
        .map_err(|e| format!("tạo thư mục model: {}", explain_io(&e)))?;
    std::fs::create_dir_all(ort_dir())
        .map_err(|e| format!("tạo thư mục ORT: {}", explain_io(&e)))?;

    let model = labse_dir().join("model.onnx");
    let tok = labse_dir().join("tokenizer.json");
    let model_ok = model.is_file()
        && model
            .metadata()
            .map(|m| m.len() > 100 * 1024 * 1024)
            .unwrap_or(false);
    if !model_ok {
        download_file(
            &format!("{}/onnx/model_quantized.onnx", HF_BASE),
            &model,
            "model LaBSE (ONNX int8, ~450 MB)",
        )?;
    }
    if !tok.is_file() {
        download_file(
            &format!("{}/tokenizer.json", HF_BASE),
            &tok,
            "tokenizer LaBSE",
        )?;
    }

    let dylib = ort_dir().join(dylib_name());
    let dylib_ok = dylib.is_file()
        && dylib
            .metadata()
            .map(|m| m.len() > 1024 * 1024)
            .unwrap_or(false);
    if !dylib_ok {
        // xoá bản 0 byte / hỏng từ lần trước nếu có
        let _ = std::fs::remove_file(&dylib);
        let (url, arch_name) = ort_archive_url();
        let archive = ort_dir().join(arch_name);
        download_file(url, &archive, "thư viện ONNX Runtime")?;
        extract_dylib(&archive, &dylib)?;
        let _ = std::fs::remove_file(&archive);
    }
    Ok(())
}

fn bootstrap_worker() {
    let _guard = BOOTSTRAP_LOCK.lock();
    if is_ready() {
        return;
    }
    // Người dùng chạy service HTTP riêng / tắt tự tải → không đụng.
    let http_url_set =
        std::env::var("REVA_LABSE_URL").is_ok() || std::env::var("CANH_DOI_LABSE_URL").is_ok();
    let auto_off = std::env::var("REVA_LABSE_AUTO")
        .map(|v| v == "0")
        .unwrap_or(false);
    if http_url_set || auto_off {
        set_phase(Phase::Off);
        return;
    }

    if files_ok().is_none() {
        set_phase(Phase::Downloading {
            file: String::from("LaBSE"),
            pct: 0,
        });
        if let Err(e) = install_files() {
            set_phase(Phase::Failed(e));
            if let Ok(mut g) = ENGINE.lock() {
                *g = None;
            }
            return;
        }
    }

    set_phase(Phase::Loading);
    if let Some((model, tok, dylib)) = files_ok() {
        match Engine::load(&model, &tok, &dylib) {
            Ok(mut eng) => {
                // Smoke test 2 câu: chặn model hỏng im lặng.
                let smoke = eng.embed(&[
                    String::from("Hello, this is a test."),
                    String::from("Xin chào, đây là bài kiểm tra."),
                ]);
                match smoke {
                    Ok(v) if v.len() == 2 && v[0].len() >= 64 => {
                        if let Ok(mut g) = ENGINE.lock() {
                            *g = Some(Arc::new(Mutex::new(eng)));
                        }
                        set_phase(Phase::Ready);
                    }
                    Ok(_) => {
                        set_phase(Phase::Failed(
                            "LaBSE trả vector lạ — model có thể hỏng".into(),
                        ));
                    }
                    Err(e) => set_phase(Phase::Failed(e)),
                }
            }
            Err(e) => set_phase(Phase::Failed(e)),
        }
    } else {
        set_phase(Phase::Failed("file model vẫn thiếu sau khi tải".into()));
    }
}

/// Gọi MỘT LẦN lúc server khởi động — nền, không chặn.
pub fn bootstrap() {
    std::thread::spawn(bootstrap_worker);
}

/// Thử lại từ đầu (nút trên UI khi Failed/Off).
pub fn retry() {
    if matches!(phase(), Phase::Failed(_)) || phase() == Phase::Off {
        // Có thể người dùng vừa sửa quyền/nối đĩa — chọn lại nơi lưu model.
        invalidate_root();
        std::thread::spawn(bootstrap_worker);
    }
}

/// Nhúng câu qua engine native. Err nếu chưa sẵn sàng.
pub fn embed(texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
    let guard = ENGINE.lock().map_err(|_| "khóa engine".to_string())?;
    if phase() != Phase::Ready {
        return Err(match phase() {
            Phase::Downloading { file, pct } => {
                format!("đang tải {} ({}%)", file, pct)
            }
            Phase::Loading => "đang nạp LaBSE".to_string(),
            Phase::Failed(e) => format!("LaBSE lỗi: {}", e),
            _ => "LaBSE chưa bật".to_string(),
        });
    }
    let eng = guard.as_ref().ok_or("engine LaBSE chưa nạp")?;
    let mut e = eng.lock().map_err(|_| "khóa LaBSE".to_string())?;
    e.embed(texts)
}
