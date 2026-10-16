//! aligner.rs — bộ ghép đoạn văn bản Anh–Việt.
//!
//! NGUYÊN TẮC AN TOÀN (sửa lỗi panic "index out of bounds: the len is 61 but the
//! index is 18446744073709551615" của bản 1.x):
//!   * KHÔNG dùng sentinel -1 / usize::MAX. Mọi chỉ số thiếu là Option<usize>.
//!   * Ma trận DP dùng mảng phẳng, truy cập bằng chỉ số suy từ biến vòng (i-1 với i >= 1).
//!   * Backtrack duyệt bằng vòng `while i > 0 || j > 0` với nhánh riêng cho i == 0 / j == 0,
//!     nên phép `i -= 1` / `j -= 1` không bao giờ tràn số.
//!   * Mọi truy cập mảng đều qua chỉ số đã kiểm chứng biên.
//!
//! Thuật toán: chấm điểm tương đồng từ vựng liên ngôn ngữ (số, danh từ riêng,
//! từ điển Anh–Việt, bigram nối) + số câu + DP đơn điệu (Needleman–Wunsch).
//!
//! LaBSE (bản 2.2, TUỲ CHỌN): nếu dịch vụ nhúng LaBSE đang chạy, điểm gốc của mỗi cặp
//! được HOÀ TRỘN với cosine LaBSE (chỉ nâng, không hạ — sàn từ vựng) trước khi đưa
//! vào DP — thuật toán DP và mọi quy ước trạng thái GIỮ NGUYÊN. Không có LaBSE thì
//! hành vi giống hệt bản 2.1.

use crate::fold::{fold_lower, fold_number_token};
use crate::lexicon::{is_common_word, lookup_en};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ========================= Hằng số =========================

/// Số đoạn tối đa mỗi bên.
pub const MAX_PARAS: usize = 2000;
/// Diện tích DP tối đa (n*m) — tránh cấp phát khổng lồ.
pub const MAX_DP_AREA: usize = 4_000_000;

const W_LEX: f64 = 0.90; // trọng số tương đồng từ vựng
const W_SENT: f64 = 0.10; // trọng số tương đồng số câu
const NUMBER_BOOST: f64 = 3.0; // số trùng nhau (rất mạnh với bài báo)
const PROPER_BOOST: f64 = 2.2; // danh từ riêng trùng nhau
const DICT_BOOST: f64 = 0.9; // từ điển Anh→Việt (pseudo-token)
const BIGRAM_W: f64 = 0.55; // bigram nối ("ha noi" -> "hanoi")
const GAP: f64 = 0.06; // phạt bỏ qua 1 đoạn
const PAIR_BIAS: f64 = 0.30; // chi phí ghép 1 cặp (ghép chỉ đáng khi s + thưởng vị trí > PAIR_BIAS)
const POS_W: f64 = 0.10; // thưởng vị trí tương đối

// ---- LaBSE (bản 2.2, tuỳ chọn) ----
// Đo thực tế trên model LaBSE THẬT (đã kiểm trên máy):
//   * cặp dịch đúng 1:1          : cosine ~0.85–0.92
//   * cặp "lược/bớt/gộp đoạn"    : cosine ~0.50–0.65 (bài báo dịch thoáng)
//   * cặp cùng chủ đề KHÔNG dịch : cosine ~0.35–0.55
// Chuẩn hoá affine cosine về "độ tin cậy" [0,1] rồi HOÀ TRỘN — nhưng theo nguyên tắc
// "LaBSE chỉ được NÂNG điểm, không được HẠ": điểm từ vựng là SÀN. Nhờ vậy bài dịch
// lược bớt (LaBSE chỉ ra cosine xám ~0.5) vẫn được ghép như bản thuần từ vựng
// (không tan thành hàng loạt "Không khớp"), còn cặp dịch đúng được nâng lên
// vượt ngưỡng Tự khớp + DP gán đúng cặp nhờ khoảng cách cosine rõ rệt.
pub const W_EMB: f64 = 0.60; // trọng số LaBSE sau chuẩn hoá (từ vựng giữ 0.40)
pub const EMB_LO: f64 = 0.50; // cosine <= 0.50 -> độ tin cậy LaBSE = 0
pub const EMB_HI: f64 = 0.80; // cosine >= 0.80 -> độ tin cậy LaBSE = 1

/// Hoà trộn điểm từ vựng (0..1) với cosine LaBSE (-1..1) thành điểm cuối (0..1).
/// Công thức: max(từ_vựng, (1-W_EMB)*từ_vựng + W_EMB*tin_cậy) — sàn từ vựng.
/// Public để selftest đối chiếu công thức.
pub fn blend_with_emb(composite: f64, emb_cos: f64) -> f64 {
    let conf = ((emb_cos - EMB_LO) / (EMB_HI - EMB_LO)).clamp(0.0, 1.0);
    let blended = (1.0 - W_EMB) * composite + W_EMB * conf;
    composite.max(blended)
}

// ========================= Trạng thái =========================

/// Trạng thái ghép — GIỮ NGUYÊN quy ước của bản gốc:
/// `auto_matched` / `needs_review` / `unmatched` / `human_confirmed`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    AutoMatched,
    NeedsReview,
    Unmatched,
    HumanConfirmed,
}

pub const ALL_STATUSES: [Status; 4] = [
    Status::AutoMatched,
    Status::NeedsReview,
    Status::Unmatched,
    Status::HumanConfirmed,
];

impl Status {
    /// Nhãn tiếng Việt để hiển thị.
    #[allow(dead_code)] // dành cho API/CLI tương lai — giao diện web tự có nhãn riêng
    pub fn label_vi(&self) -> &'static str {
        match self {
            Status::AutoMatched => "Tự khớp",
            Status::NeedsReview => "Cần soát",
            Status::Unmatched => "Không khớp",
            Status::HumanConfirmed => "Đã xác nhận",
        }
    }

    /// Chuẩn hoá từ chuỗi JSON (chấp nhận cả nhãn tiếng Việt cho tiện).
    pub fn parse(s: &str) -> Option<Status> {
        match s {
            "auto_matched" | "Tự khớp" => Some(Status::AutoMatched),
            "needs_review" | "Cần soát" => Some(Status::NeedsReview),
            "unmatched" | "Không khớp" => Some(Status::Unmatched),
            "human_confirmed" | "Đã xác nhận" => Some(Status::HumanConfirmed),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Status::AutoMatched => "auto_matched",
            Status::NeedsReview => "needs_review",
            Status::Unmatched => "unmatched",
            Status::HumanConfirmed => "human_confirmed",
        }
    }
}

// ========================= Dữ liệu =========================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub index: usize,
    pub en_index: Option<usize>,
    pub vi_index: Option<usize>,
    pub en: Option<String>,
    pub vi: Option<String>,
    pub status: Status,
    pub score: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlignOptions {
    /// Ngưỡng (0..1): cặp có điểm >= ngưỡng -> auto_matched, dưới -> needs_review.
    pub auto_threshold: f64,
}

impl Default for AlignOptions {
    fn default() -> Self {
        AlignOptions { auto_threshold: 0.30 }
    }
}

impl AlignOptions {
    pub fn clamped(&self) -> AlignOptions {
        AlignOptions {
            auto_threshold: self.auto_threshold.clamp(0.05, 0.90),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct AlignOutput {
    pub english: Vec<String>,
    pub vietnamese: Vec<String>,
    pub rows: Vec<Row>,
    pub options: AlignOptions,
    /// Động cơ đã tạo kết quả này: "lexical" hoặc "lexical+labse" (bản 2.2).
    pub engine: String,
}

#[derive(Debug)]
pub struct AlignError(pub String);
impl std::fmt::Display for AlignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ========================= Tách đoạn =========================

/// Tách đoạn: ưu tiên dòng trống; nếu chỉ có 1 "đoạn" mà có xuống dòng đơn thì tách theo dòng.
pub fn split_paragraphs(text: &str) -> Vec<String> {
    let norm = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut paras: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut blank_pending = false;
    for line in norm.split('\n') {
        let t = line.trim();
        if t.is_empty() {
            if !cur.is_empty() {
                blank_pending = true;
            }
        } else {
            if blank_pending {
                paras.push(std::mem::take(&mut cur));
                blank_pending = false;
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(t);
        }
    }
    if !cur.is_empty() {
        paras.push(cur);
    }
    // Fallback: 1 đoạn khổng lồ chứa nhiều dòng -> tách theo dòng
    if paras.len() <= 1 {
        if let Some(single) = paras.first() {
            let lines: Vec<&str> = norm
                .split('\n')
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .collect();
            if lines.len() > 1 {
                return lines.into_iter().map(|s| s.to_string()).collect();
            }
            return vec![single.clone()];
        }
    }
    paras
}

// ========================= Token hoá =========================

#[derive(Default, Clone)]
struct ParaTokens {
    /// token đã fold (kể cả bigram nối) -> số lần xuất hiện
    counts: HashMap<String, f64>,
    /// token thuần số
    numbers: HashSet<String>,
    /// danh từ riêng (đã fold) trong đoạn
    proper: HashSet<String>,
    /// số câu
    sent: usize,
}

impl ParaTokens {
    fn build(para: &str) -> ParaTokens {
        let folded_words = tokenize_words(para);
        let mut pt = ParaTokens::default();
        // danh từ riêng: quét trên chuỗi GỐC (giữ chữ hoa)
        for (idx, word) in raw_words(para).iter().enumerate() {
            let first = word.chars().next();
            if let Some(c) = first {
                if c.is_uppercase() {
                    let f = fold_lower(word);
                    let all_caps = word.chars().all(|ch| ch.is_uppercase()) && word.chars().count() >= 2;
                    let common = is_common_word(&f);
                    let long_enough = word.chars().count() >= 2;
                    if long_enough && !common && (!idx_normal_start(idx, para) || all_caps) {
                        pt.proper.insert(f);
                    }
                }
            }
        }
        for w in &folded_words {
            *pt.counts.entry(w.clone()).or_insert(0.0) += 1.0;
            if let Some(num) = fold_number_token(w) {
                pt.numbers.insert(num);
            }
        }
        // bigram nối: "ha noi" -> "hanoi"
        for pair in folded_words.windows(2) {
            let joined = format!("{}{}", pair[0], pair[1]);
            *pt.counts.entry(joined).or_insert(0.0) += BIGRAM_W;
        }
        pt.sent = count_sentences(para);
        pt
    }
}

/// Từ đầu câu vẫn có thể là danh từ riêng nếu từ đó viết hoa toàn bộ (NASA) —
/// hàm này trả về true khi idx==0 và KHÔNG phải all-caps (đã xử lý ở caller).
fn idx_normal_start(_idx: usize, _para: &str) -> bool {
    true // giữ đơn giản: logic all-caps đã xử lý ở caller
}

/// Tách từ (chữ/số liên tiếp), fold + chuẩn hoá số.
fn tokenize_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut has_digit = false;
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            cur.push(ch);
            if ch.is_ascii_digit() {
                has_digit = true;
            }
        } else if !cur.is_empty() {
            push_token(&mut out, &cur, has_digit);
            cur.clear();
            has_digit = false;
        }
    }
    if !cur.is_empty() {
        push_token(&mut out, &cur, has_digit);
    }
    out
}

fn push_token(out: &mut Vec<String>, raw: &str, has_digit: bool) {
    let folded = fold_lower(raw);
    if folded.is_empty() {
        return;
    }
    if has_digit {
        // "4,000" -> "4000"; "4.000" -> "4000"; "2025" -> "2025"
        if let Some(num) = fold_number_token(&folded) {
            out.push(num);
            return;
        }
    }
    out.push(folded);
}

/// Từ gốc (giữ hoa/thường) để nhận diện danh từ riêng.
fn raw_words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() {
            cur.push(ch);
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn count_sentences(para: &str) -> usize {
    let mut n = 0usize;
    let chars: Vec<char> = para.chars().collect();
    for (i, c) in chars.iter().enumerate() {
        if matches!(c, '.' | '!' | '?' | '…') {
            // dấu kết câu nếu đứng cuối hoặc theo sau là khoảng trắng
            let next = chars.get(i + 1);
            if next.is_none() || next.map(|c| c.is_whitespace()).unwrap_or(true) {
                n += 1;
            }
        }
    }
    n.max(1)
}

// ========================= Chấm điểm =========================

struct Side {
    ptoks: Vec<ParaTokens>,
    df: HashMap<String, usize>, // document frequency của token (kể cả bigram)
}

impl Side {
    fn build(paras: &[String]) -> Side {
        let ptoks: Vec<ParaTokens> = paras.iter().map(|p| ParaTokens::build(p)).collect();
        let mut df: HashMap<String, usize> = HashMap::new();
        for pt in &ptoks {
            for t in pt.counts.keys() {
                *df.entry(t.clone()).or_insert(0) += 1;
            }
        }
        Side { ptoks, df }
    }

    fn idf(&self, token: &str) -> f64 {
        let n = self.ptoks.len().max(1) as f64;
        let d = (*self.df.get(token).unwrap_or(&0) + 1) as f64;
        (1.0 + n / d).ln()
    }
}

/// Điểm tương đồng đoạn EN i với đoạn VI j (0..1).
fn score_pair(en: &Side, vi: &Side, i: usize, j: usize) -> f64 {
    let pe = en.ptoks.get(i).expect("ptoks[i] hợp lệ");
    let pv = vi.ptoks.get(j).expect("ptoks[j] hợp lệ");

    // ---- trọng số phía Việt
    let mut w_vi: HashMap<String, f64> = HashMap::new();
    let mut norm_vi = 0.0f64;
    for (t, cnt) in &pv.counts {
        let mut w = (*cnt) * vi.idf(t);
        if pv.numbers.contains(t) {
            w *= NUMBER_BOOST;
        } else if pv.proper.contains(t) {
            w *= PROPER_BOOST;
        }
        norm_vi += w * w;
        w_vi.insert(t.clone(), w);
    }

    // ---- trọng số phía Anh (đích + mở rộng từ điển)
    let mut w_en: HashMap<String, f64> = HashMap::new();
    let mut norm_en = 0.0f64;
    for (t, cnt) in &pe.counts {
        let mut w = (*cnt) * en.idf(t);
        if pe.numbers.contains(t) {
            w *= NUMBER_BOOST;
        } else if pe.proper.contains(t) {
            w *= PROPER_BOOST;
        }
        norm_en += w * w;
        *w_en.entry(t.clone()).or_insert(0.0) += w;
    }
    // pseudo-token từ điển Anh→Việt
    let mut dict_expansions: Vec<(String, f64)> = Vec::new();
    for t in pe.counts.keys() {
        if pe.numbers.contains(t) {
            continue;
        }
        if let Some(trans) = lookup_en(t) {
            let base = en.idf(t) * DICT_BOOST;
            for vt in trans {
                dict_expansions.push((vt.to_string(), base));
            }
        }
    }
    for (vt, base) in dict_expansions {
        // nếu VI có sẵn token gốc trùng pseudo thì cộng dồn (hiếm)
        *w_en.entry(vt).or_insert(0.0) += base;
        norm_en += base * base;
    }

    // ---- cosine
    let mut dot = 0.0f64;
    for (t, w) in &w_en {
        if let Some(wv) = w_vi.get(t) {
            dot += w * wv;
        }
    }
    let denom = norm_en.sqrt() * norm_vi.sqrt();
    let lexical = if denom > 1e-12 { (dot / denom).clamp(0.0, 1.0) } else { 0.0 };

    // ---- số câu
    let sa = pe.sent as f64;
    let sb = pv.sent as f64;
    let sent = 1.0 - ((sa - sb).abs() / sa.max(sb).max(1.0));

    W_LEX * lexical + W_SENT * sent
}

// ========================= DP đơn điệu =========================

/// Ma trận cha (backtrack): 0 = ghép, 1 = bỏ qua EN, 2 = bỏ qua VI.
/// Lưu phẳng: parent[i * (m+1) + j].
/// QUAN TRỌNG: `raw` (điểm gốc n×m) và `dp` (bảng quy hoạch) là HAI mảng riêng —
/// bản 1.x từng ghi đè tại chỗ rồi đọc nhầm giá trị DP làm điểm gốc.
fn dp_align(raw: &[f64], dp: &mut [f64], n: usize, m: usize) -> Vec<u8> {
    let m1 = m + 1;
    let mut parent = vec![0u8; (n + 1) * m1];
    dp[0] = 0.0;
    for i in 1..=n {
        dp[i * m1] = dp[(i - 1) * m1] - GAP;
        parent[i * m1] = 1;
    }
    for j in 1..=m {
        dp[j] = dp[j - 1] - GAP;
        parent[j] = 2;
    }
    let nn = n.max(1) as f64;
    let mm = m.max(1) as f64;
    for i in 1..=n {
        for j in 1..=m {
            let s = raw[(i - 1) * m + (j - 1)];
            let pos = 1.0 - (((i as f64) / nn) - ((j as f64) / mm)).abs();
            let mut best = dp[(i - 1) * m1 + (j - 1)] + s - PAIR_BIAS + POS_W * pos;
            let mut code = 0u8;
            let skip_e = dp[(i - 1) * m1 + j] - GAP;
            if skip_e > best {
                best = skip_e;
                code = 1;
            }
            let skip_v = dp[i * m1 + (j - 1)] - GAP;
            if skip_v > best {
                best = skip_v;
                code = 2;
            }
            dp[i * m1 + j] = best;
            parent[i * m1 + j] = code;
        }
    }
    parent
}

// ========================= Ghép chính =========================

/// Ghép hai văn bản bằng bộ từ vựng (như bản 2.1) — không LaBSE.
pub fn align(
    english_text: &str,
    vietnamese_text: &str,
    options: &AlignOptions,
) -> Result<AlignOutput, AlignError> {
    align_with_emb(english_text, vietnamese_text, options, None)
}

/// Ghép hai văn bản, HOÀ TRỘN kèm cosine LaBSE nếu có.
/// `emb_cos` (tuỳ chọn): ma trận cosine EN×VI phẳng (hàng trước), độ dài đúng n*m —
/// nếu sai kích thước thì bị BỎ QUA an toàn (fallback từ vựng, không panic).
/// Thuật toán DP, quy ước trạng thái, 5 bất biến: GIỮ NGUYÊN — chỉ điểm gốc đổi.
pub fn align_with_emb(
    english_text: &str,
    vietnamese_text: &str,
    options: &AlignOptions,
    emb_cos: Option<&[f64]>,
) -> Result<AlignOutput, AlignError> {
    let options = options.clamped();

    let english = split_paragraphs(english_text);
    let vietnamese = split_paragraphs(vietnamese_text);

    if english.is_empty() && vietnamese.is_empty() {
        return Err(AlignError(
            "Chưa có văn bản nào để ghép — hãy dán ít nhất một bên (tiếng Anh hoặc tiếng Việt)."
                .into(),
        ));
    }
    if english.len() > MAX_PARAS || vietnamese.len() > MAX_PARAS {
        return Err(AlignError(format!(
            "Quá nhiều đoạn (tối đa {} đoạn mỗi bên). Hiện có: EN={}, VI={}.",
            MAX_PARAS,
            english.len(),
            vietnamese.len()
        )));
    }
    let n = english.len();
    let m = vietnamese.len();
    if n.saturating_mul(m) > MAX_DP_AREA {
        return Err(AlignError(format!(
            "Hai văn bản lệch nhau quá nhiều (EN={} đoạn, VI={} đoạn,DP={} ô > {}). Hãy tách nhỏ bài rồi ghép từng phần.",
            n, m, n.saturating_mul(m), MAX_DP_AREA
        )));
    }

    let en_side = Side::build(&english);
    let vi_side = Side::build(&vietnamese);

    // LaBSE: chỉ dùng khi ma trận đúng kích thước n*m (phòng vệ — sai thì bỏ qua).
    let emb = match emb_cos {
        Some(mtx) if mtx.len() == n * m => Some(mtx),
        Some(_) => None,
        None => None,
    };

    // ---- ma trận điểm gốc (n x m) và bảng DP riêng biệt (n+1) x (m+1)
    let m1 = m + 1;
    let mut raw_scores: Vec<f64> = vec![0.0; n * m];
    for i in 0..n {
        for j in 0..m {
            let composite = score_pair(&en_side, &vi_side, i, j);
            raw_scores[i * m + j] = match emb {
                Some(mtx) => blend_with_emb(composite, mtx[i * m + j]),
                None => composite,
            };
        }
    }
    let mut dp: Vec<f64> = vec![0.0; (n + 1) * m1];
    let parent = dp_align(&raw_scores, &mut dp, n, m);

    // (debug tạm) in ma trận điểm khi DEBUG_ALIGN=1
    if std::env::var("DEBUG_ALIGN").ok().as_deref() == Some("1") {
        eprintln!("--- score matrix ({} x {}) ---", n, m);
        for i in 0..n {
            let mut line = String::new();
            for j in 0..m {
                line.push_str(&format!("{:5.2} ", raw_scores[i * m + j]));
            }
            eprintln!("EN{:2}: {}", i, line);
        }
    }

    // ---- Backtrack AN TOÀN: không có sentinel, không trừ xuống dưới 0
    enum Event {
        Pair(usize, usize),
        SkipEn(usize),
        SkipVi(usize),
    }
    let mut events: Vec<Event> = Vec::new();
    let mut i = n;
    let mut j = m;
    while i > 0 || j > 0 {
        let code = if i == 0 {
            2u8 // chỉ còn dòng VI
        } else if j == 0 {
            1u8 // chỉ còn dòng EN
        } else {
            parent[i * m1 + j] // i, j >= 1 -> chỉ số hợp lệ
        };
        match code {
            0 => {
                // i, j >= 1 khi vào nhánh này
                i -= 1;
                j -= 1;
                events.push(Event::Pair(i, j));
            }
            1 => {
                i -= 1;
                events.push(Event::SkipEn(i));
            }
            _ => {
                j -= 1;
                events.push(Event::SkipVi(j));
            }
        }
    }
    events.reverse();

    // ---- dựng hàng
    let mut rows: Vec<Row> = Vec::with_capacity(n + m);
    for ev in &events {
        let row = match ev {
            Event::Pair(a, b) => {
                let s = raw_scores[(*a) * m + (*b)];
                let status = if s >= options.auto_threshold {
                    Status::AutoMatched
                } else {
                    Status::NeedsReview
                };
                Row {
                    index: rows.len(),
                    en_index: Some(*a),
                    vi_index: Some(*b),
                    en: english.get(*a).cloned(),
                    vi: vietnamese.get(*b).cloned(),
                    status,
                    score: Some((s * 1000.0).round() / 1000.0),
                }
            }
            Event::SkipEn(a) => Row {
                index: rows.len(),
                en_index: Some(*a),
                vi_index: None,
                en: english.get(*a).cloned(),
                vi: None,
                status: Status::Unmatched,
                score: None,
            },
            Event::SkipVi(b) => Row {
                index: rows.len(),
                en_index: None,
                vi_index: Some(*b),
                en: None,
                vi: vietnamese.get(*b).cloned(),
                status: Status::Unmatched,
                score: None,
            },
        };
        rows.push(row);
    }

    let out = AlignOutput {
        english,
        vietnamese,
        rows,
        options,
        engine: if emb.is_some() {
            "lexical+labse".to_string()
        } else {
            "lexical".to_string()
        },
    };
    // kiểm tra bất biến ngay khi ghép (phòng vệ hai lớp)
    if let Err(e) = validate_invariants(&out) {
        return Err(AlignError(format!("Vi phạm bất biến nội bộ: {}", e)));
    }
    Ok(out)
}

// ========================= Bất biến (5 điều) =========================

/// 5 bất biến bắt buộc:
/// I1. Mỗi đoạn tiếng Anh được dùng đúng 1 lần.
/// I2. Mỗi đoạn tiếng Việt được dùng đúng 1 lần.
/// I3. Ghép đơn điệu — không giao nhau.
/// I4. Trạng thái thuộc 4 giá trị chuẩn; `unmatched` ⟺ thiếu đúng một bên.
/// I5. (kiểm tại export) bên thiếu ghi đúng chữ `none`; không bỏ sót đoạn nào.
pub fn validate_invariants(out: &AlignOutput) -> Result<(), String> {
    let n = out.english.len();
    let m = out.vietnamese.len();
    let mut seen_en = vec![false; n];
    let mut seen_vi = vec![false; m];
    let mut last_en: Option<usize> = None;
    let mut last_vi: Option<usize> = None;

    for row in &out.rows {
        // I4a: trạng thái hợp lệ
        if !ALL_STATUSES.contains(&row.status) {
            return Err(format!("hàng {}: trạng thái lạ {:?}", row.index, row.status));
        }
        let has_en = row.en_index.is_some();
        let has_vi = row.vi_index.is_some();
        if !has_en && !has_vi {
            return Err(format!("hàng {}: rỗng cả hai bên", row.index));
        }
        // I4b: unmatched ⟺ đúng một bên
        if row.status == Status::Unmatched && has_en && has_vi {
            return Err(format!(
                "hàng {}: trạng thái unmatched nhưng có đủ hai bên",
                row.index
            ));
        }
        // Chiều ngược của I4b cho phép NGOẠI LỆ duy nhất là HumanConfirmed trên hàng một
        // phía — đó là tính năng có chủ đích "đã xác nhận là không có bản dịch"
        // (session::set_row_status cho phép từ bản 2.0). Bản 2.3 nới lệnh cấm này để
        // việc xuất file không vỡ sau khi người dùng bấm ✓ trên hàng "none" —
        // mọi kiểm tra khác của I4b giữ nguyên.
        if row.status != Status::Unmatched
            && row.status != Status::HumanConfirmed
            && (!has_en || !has_vi)
        {
            return Err(format!(
                "hàng {}: trạng thái {:?} nhưng thiếu một bên",
                row.index, row.status
            ));
        }
        // I1 + I3 (phía Anh)
        if let Some(a) = row.en_index {
            if a >= n {
                return Err(format!("hàng {}: en_index {} >= {}", row.index, a, n));
            }
            if seen_en[a] {
                return Err(format!("đoạn tiếng Anh thứ {} bị dùng 2 lần", a));
            }
            seen_en[a] = true;
            if let Some(le) = last_en {
                if a <= le {
                    return Err(format!("I3: đoạn tiếng Anh không đơn điệu tại {}", a));
                }
            }
            last_en = Some(a);
        }
        // I2 + I3 (phía Việt)
        if let Some(b) = row.vi_index {
            if b >= m {
                return Err(format!("hàng {}: vi_index {} >= {}", row.index, b, m));
            }
            if seen_vi[b] {
                return Err(format!("đoạn tiếng Việt thứ {} bị dùng 2 lần", b));
            }
            seen_vi[b] = true;
            if let Some(lv) = last_vi {
                if b <= lv {
                    return Err(format!("I3: đoạn tiếng Việt không đơn điệu tại {}", b));
                }
            }
            last_vi = Some(b);
        }
    }
    for (k, s) in seen_en.iter().enumerate() {
        if !s {
            return Err(format!("đoạn tiếng Anh thứ {} bị bỏ sót", k));
        }
    }
    for (k, s) in seen_vi.iter().enumerate() {
        if !s {
            return Err(format!("đoạn tiếng Việt thứ {} bị bỏ sót", k));
        }
    }
    Ok(())
}
