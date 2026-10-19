//! qa.rs — BẢN 2.9 — bảng soát lỗi QA (8 kiểm) cho phiên đã ghép.
//!
//! Mục đích sản phẩm (xem `docs/PLAN-QOL-2026-10-19.md` §1.3): soát lỗi là bước CHUẨN
//! NGÀNH (danh mục kiểm ApSIC Xbench: Numeric Mismatch, Target same as Source,
//! Unpaired Symbol/Quotes, ALLUPPERCASE Mismatch…; VN: "hiệu đính — đối chiếu bản
//! dịch và bản gốc", báo chí "nhặt sạn" sách dịch) — nhưng REVA trước giờ chỉ HIỂN
//! THỊ từng cặp, mọi kiểm tra là mắt người dùng tự dò. Module này tự dò các lỗi cơ
//! học để người soát chỉ cần nhìn vào chỗ có vấn đề. KHÔNG tự sửa gì bao giờ.
//!
//! 8 kiểm v1 (đặt tên theo ngành để người dùng CAT nhận ra):
//!   1. `numeric-mismatch` (error) — dãy số mỗi phía chuẩn hoá THEO PHÍA rồi so multiset: EN `1,234.5`→`1234.5` (`,`=nghìn, `.`=thập phân); VI `1.234,5`→`1234.5` (ngược lại). `4,000`==`4.000`; `3.5`≠`35` (lỗi đổi vị trí thập phân — phát hiện được).
//!   2. `target-same-as-source` (error) — VI nguyên văn == EN sau trim (dấu hiệu bỏ dịch).
//!   3. `length-outlier` (warning) — EN ≥ 40 ký tự mà tỉ lệ dài VI/EN < 0.35 hoặc > 2.5.
//!   4. `double-space` (warning) — 2 dấu cách liền / khoảng trắng đầu-cuối một phía.
//!   5. `unpaired-symbol` (warning) — cặp `() [] {} “” ‘’ «»` không cân; dấu `"` lẻ (dấu `'` KHÔNG kiểm — apostrophe trong "don't" gây báo oan).
//!   6. `alluppercase-mismatch` (warning) — từ viết hoa toàn bộ ≥ 3 ký tự chỉ có một phía (NASA mất) — so không phân biệt hoa thường.
//!   7. `missing-diacritics` (info) — từ VI không dấu nằm trong bảng ~17 từ KHÔNG NHẬP NHẰNG (precision-first: đã loại cua/roi/nhung/cung/lam/nho… vì đó là từ tiếng Việt hợp lệ).
//!   8. `nfd-mixed` (info) — VI chứa dấu tổ hợp NFD → nhắc chuẩn hoá.
//!
//! THUẦN ĐỌC: hàm thuần `report(&Session)` — không đổi hàng, không snapshot, không
//! trạng thái, không mạng, KHÔNG đụng embedder → kết quả xác định trên mọi máy
//! (LaBSE chạy hay không không đổi kết quả QA — hermetic tự nhiên).
//!
//! Quy ước biên: hàng MỘT phía (unmatched) — kiểm 4, 5, 6, 8 chạy trên phía có mặt
//! (mức soát đúng nghĩa); kiểm 1, 2, 3 cần đủ hai phía nên bỏ qua. [AUDIT #2 2026-10-19]
//! Kiểm 1 (số liệu) cũng bỏ trên hàng một phía: tách cặp xong thì mọi số phía có mặt
//! thành “thiếu/thừa” oan — nhiễu nhiều hơn giá trị (số bên kia thiếu là đương nhiên
//! đã thấy ở chữ “none”). Phiên rỗng → 0 vấn đề.

use crate::session::Session;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Mức nghiêm trọng — mã chuẩn cho API + nhãn tiếng Việt.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

/// Một vấn đề phát hiện được. `en`/`vi` là văn bản hàng (cắt ngắn để hiển thị).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    /// Chỉ số hàng (0-based) trong phiên — giao diện dùng để cuộn tới + nhấp nháy.
    pub row: usize,
    pub severity: Severity,
    /// Tên kiểm theo ngành (numeric-mismatch, target-same-as-source…).
    pub check: &'static str,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub en: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub vi: String,
    /// Mô tả ngắn tiếng Việt kèm hướng dẫn soát.
    pub detail: String,
}

/// Đếm theo mức + danh sách vấn đề.
/// `by_severity` dùng BTreeMap để JSON ra CÙNG THỨ TỰ khoá mọi lần gọi
/// (HashMap std sẽ ngẫu nhiên thứ tự — khó so sánh/kểm định chuỗi JSON).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QaReport {
    pub total: usize,
    pub by_severity: BTreeMap<&'static str, usize>,
    /// Danh sách vấn đề theo thứ tự hàng → tên kiểm (ổn định để kiểm định).
    pub issues: Vec<Issue>,
}

// ========================= Bảng từ thiếu dấu (kiểm 7) =========================
//
// Precision-first: CHỈ dạng không-dấu gần như chắc chắn là lỗi (chuỗi đó KHÔNG
// phải từ tiếng Việt hợp lệ). Đã loại các dạng nhập nhằng vì là từ hợp lệ:
// cua (con cua) · roi (cây roi) · nhung (nhung) · cung (cung điện) · lam (màu lam)
// · nho (quả nho) · den (đen) · voi (voi) · da (da) · bi (bi) · vay (vay) · chua
// (chua) · chung (chung) · lan (lan) · nay (nay) · rat (chuột) · day (đây/dạy/đầy)
// · the (thế/thể) · vi (vi)…

const MISSING_DIACRITICS: &[(&str, &str)] = &[
    ("khong", "không"),
    ("giua", "giữa"),
    ("nguoi", "người"),
    ("duoc", "được"),
    ("truoc", "trước"),
    ("hon", "hơn"),
    ("nua", "nữa"),
    ("hien", "hiện"),
    ("dang", "đang"),
    ("gi", "gì"),
    ("dau", "đâu"),
    ("tuyet", "tuyệt"),
    ("toi", "tôi"),
    ("vao", "vào"),
    ("nen", "nên"),
    ("neu", "nếu"),
    ("hoac", "hoặc"),
];

// ========================= Trích số liệu (kiểm 1) =========================

/// Trích các "token số" của một chuỗi: cụm dài nhất gồm chữ số nối bằng `.` hoặc `,`
/// theo đúng định dạng phía đó (EN: `4,000` · `1,234.5`; VI: `4.000` · `1.234,5`).
/// Trả dạng CHUẨN HOÁ theo phía: EN bỏ `,`, VI bỏ `.` rồi đổi `,` thành `.` —
/// hai bên cùng giá trị sẽ ra cùng chuỗi (`4,000`≡`4.000`→`4000`; `1,234.5`≡`1.234,5`
/// →`1234.5`), còn `3.5`≠`35` vẫn bị bắt (lỗi dời dấu thập phân). Ký tự đơn vị
/// ($ % đ €) nằm ngoài token nên tự loại.
fn extract_numbers(text: &str, vi_side: bool) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            // cụm số: chữ số, xen `.`/`,` nhưng sau dấu phải là chữ số
            let start = i;
            let mut j = i + 1;
            while j < chars.len() {
                let c = chars[j];
                let cont = c.is_ascii_digit()
                    || ((c == '.' || c == ',')
                        && chars
                            .get(j + 1)
                            .map(|n| n.is_ascii_digit())
                            .unwrap_or(false));
                if !cont {
                    break;
                }
                j += 1;
            }
            let raw: String = chars[start..j].iter().collect();
            let norm = if vi_side {
                // VI: `.` = nghìn (bỏ), `,` = thập phân (thành `.`)
                let no_thou: String = raw.chars().filter(|c| *c != '.').collect();
                no_thou.replace(',', ".")
            } else {
                // EN: `,` = nghìn (bỏ), `.` = thập phân (giữ)
                raw.chars().filter(|c| *c != ',').collect()
            };
            out.push(norm);
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// So multiset số liệu EN vs VI; trả mô tả các lệch (thiếu/thừa từng con số).
fn numeric_mismatch_detail(en: &str, vi: &str) -> Option<String> {
    let en_count = multiset(&extract_numbers(en, false));
    let mut vi_count = multiset(&extract_numbers(vi, true));
    // Tiếng Việt viết TÊN THÁNG thành số ("March 12" → "12 tháng 3") — con số
    // tháng là chuyển ngữ đúng, không phải sai số liệu. Nếu phía Anh có tên tháng
    // tiếng Anh thì bỏ từng số đứng sau chữ "tháng" khỏi multiset VI trước khi so
    // (chỉ số SAU "tháng" — "3 tháng" = thời lượng giữ nguyên). Phát hiện khi QA
    // agent-browser trên chính bài mẫu demo: 2 lỗi oan do "tháng 3".
    if has_english_month(en) {
        for d in month_digits_after_thang(vi) {
            if let Some(c) = vi_count.get_mut(&d) {
                if *c > 0 {
                    *c -= 1;
                    if *c == 0 {
                        vi_count.remove(&d);
                    }
                }
            }
        }
    }
    let mut missing: Vec<String> = Vec::new(); // có ở EN, thiếu ở VI
    let mut extra: Vec<String> = Vec::new(); // có ở VI, không có ở EN
    for (n, c) in &en_count {
        let v = vi_count.get(n).copied().unwrap_or(0);
        if v < *c {
            missing.push(if v == 0 {
                (*n).clone()
            } else {
                format!("{} (thiếu {} lần)", n, c - v)
            });
        }
    }
    for (n, c) in &vi_count {
        let e = en_count.get(n).copied().unwrap_or(0);
        if e < *c {
            extra.push(if e == 0 {
                (*n).clone()
            } else {
                format!("{} (thừa {} lần)", n, c - e)
            });
        }
    }
    if missing.is_empty() && extra.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    if !missing.is_empty() {
        parts.push(format!(
            "bản gốc có mà bản dịch thiếu: {}",
            missing.join(", ")
        ));
    }
    if !extra.is_empty() {
        parts.push(format!(
            "bản dịch có mà bản gốc không có: {}",
            extra.join(", ")
        ));
    }
    Some(format!(
        "Lệch số liệu — {}. Soát chỗ đổi vị trí dấu phẩy/thập phân (3.5% → 35%) hoặc bỏ sót.",
        parts.join("; ")
    ))
}

fn multiset(items: &[String]) -> HashMap<String, usize> {
    let mut m: HashMap<String, usize> = HashMap::new();
    for i in items {
        *m.entry(i.clone()).or_insert(0) += 1;
    }
    m
}

/// Phía Anh có tên tháng tiếng Anh (so TỪ NGUYÊN VẸN, không phân biệt hoa thường).
fn has_english_month(en: &str) -> bool {
    // "may" NHẬP NHẰNG với động từ ("you may go") nên CHỈ tính khi viết hoa chữ
    // đầu: "May"/"MAY" là tháng, "may" là động từ. [AUDIT #2 2026-10-19]
    const MONTHS: &[&str] = &[
        "january",
        "february",
        "march",
        "april",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ];
    for w in en.split_whitespace() {
        let t = word_trim(w);
        let low = t.to_lowercase();
        if MONTHS.contains(&low.as_str()) {
            return true;
        }
        if low == "may" && t.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
            return true;
        }
    }
    false
}

/// Các token số đứng ngay sau chữ "tháng" (dạng đã chuẩn hoá như phía VI):
/// "đến 15 tháng 3, thu hút" → ["3"].
fn month_digits_after_thang(vi: &str) -> Vec<String> {
    let words: Vec<&str> = vi.split_whitespace().collect();
    let mut out = Vec::new();
    for i in 0..words.len().saturating_sub(1) {
        // gọt dấu câu dính HAI đầu — "(tháng" vẫn tính là "tháng" [AUDIT #2]
        if word_trim(words[i]).to_lowercase() == "tháng" {
            // lấy dãy chữ số ĐẦU của từ kế tiếp (gọt dấu câu dính theo: "3,", "10.")
            let digits: String = words[i + 1]
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            // CHỈ tha số 1..=12 — tên tháng phải vậy; "tháng 2025" không phải tên tháng
            if let Ok(n) = digits.parse::<usize>() {
                if (1..=12).contains(&n) {
                    out.push(digits);
                }
            }
        }
    }
    out
}

// ========================= Tách từ (kiểm 6, 7) =========================

/// Tách từ thô theo khoảng trắng, gọt ký tự không chữ/số hai đầu (`(NASA)` → `NASA`).
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split_whitespace().map(word_trim)
}

fn word_trim(w: &str) -> &str {
    let mut s = w;
    while let Some(c) = s.chars().next() {
        if c.is_alphanumeric() {
            break;
        }
        s = &s[c.len_utf8()..];
    }
    let mut e = s;
    while let Some(c) = e.chars().next_back() {
        if c.is_alphanumeric() {
            break;
        }
        e = &e[..e.len() - c.len_utf8()];
    }
    e
}

/// Từ viết hoa TOÀN BỘ, ≥ 3 ký tự, thuần chữ (`COVID-19` có số/gạch → bỏ qua).
fn all_caps_word(w: &str) -> Option<String> {
    if w.chars().count() < 3 || !w.chars().all(|c| c.is_alphabetic()) {
        return None;
    }
    if w.chars().all(|c| c.is_uppercase()) {
        Some(w.to_lowercase())
    } else {
        None
    }
}

/// Kiểm 6 — so 2 tập từ viết hoa (không phân biệt hoa thường).
fn allcaps_mismatch_detail(en: &str, vi: &str) -> Option<String> {
    let en_caps: HashSet<String> = words(en).filter_map(all_caps_word).collect();
    let vi_caps: HashSet<String> = words(vi).filter_map(all_caps_word).collect();
    let lost: Vec<&str> = en_caps.difference(&vi_caps).map(|s| s.as_str()).collect();
    let added: Vec<&str> = vi_caps.difference(&en_caps).map(|s| s.as_str()).collect();
    if lost.is_empty() && added.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    if !lost.is_empty() {
        parts.push(format!("mất ở bản dịch: {}", lost.join(", ")));
    }
    if !added.is_empty() {
        parts.push(format!("chỉ có ở bản dịch: {}", added.join(", ")));
    }
    Some(format!(
        "Tên riêng/viết tắt viết hoa không đối — {}. Kiểm tra đã dịch hay bỏ tên riêng.",
        parts.join("; ")
    ))
}

/// Kiểm 5 — ngoặc không cân từng phía.
fn unpaired_symbol_detail(en: &str, vi: &str) -> Option<String> {
    let pairs: &[(char, char, &str)] = &[
        ('(', ')', "()"),
        ('[', ']', "[]"),
        ('{', '}', "{}"),
        ('\u{201c}', '\u{201d}', "“”"),
        ('\u{2018}', '\u{2019}', "‘’"),
        ('\u{ab}', '\u{bb}', "«»"),
    ];
    let mut parts: Vec<String> = Vec::new();
    for (side_name, text) in [("bản gốc", en), ("bản dịch", vi)] {
        let mut unbalanced: Vec<&str> = Vec::new();
        for (open, close, name) in pairs {
            if text.matches(*open).count() != text.matches(*close).count() {
                unbalanced.push(name);
            }
        }
        if text.matches('"').count() % 2 == 1 {
            unbalanced.push("\" lẻ");
        }
        if !unbalanced.is_empty() {
            parts.push(format!("{} lệch cặp {}", side_name, unbalanced.join(", ")));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!(
            "Dấu ngoặc không cân — {}. Bấm vào hàng để soi chỗ lửng.",
            parts.join("; ")
        ))
    }
}

/// Kiểm 4 — khoảng trắng thừa một phía.
fn whitespace_detail(en: &str, vi: &str) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for (side_name, text) in [("bản gốc", en), ("bản dịch", vi)] {
        if text.contains("  ") {
            parts.push(format!("{} có 2 dấu cách liền", side_name));
        }
        if !text.is_empty() && text.trim() != text {
            parts.push(format!("{} có khoảng trắng đầu/cuối", side_name));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(format!("Khoảng trắng thừa — {}.", parts.join("; ")))
    }
}

/// Kiểm 7 — từ VI không dấu nằm trong bảng.
fn missing_diacritics_detail(vi: &str) -> Option<String> {
    let hits: Vec<String> = words(vi)
        .filter_map(|w| {
            let low = w.to_lowercase();
            MISSING_DIACRITICS
                .iter()
                .find(|(k, _)| *k == low)
                .map(|(k, v)| format!("{} → {}", k, v))
        })
        .collect();
    if hits.is_empty() {
        None
    } else {
        Some(format!(
            "Có thể thiếu dấu tiếng Việt: {}. Từ hợp lệ trùng dạng thì bỏ qua.",
            hits.join(", ")
        ))
    }
}

/// Kiểm 8 — VI chứa dấu tổ hợp NFD.
fn nfd_mixed_detail(vi: &str) -> Option<String> {
    let has_combining = vi.chars().any(|c| ('\u{0300}'..='\u{036f}').contains(&c));
    if has_combining {
        Some(
            "Bản dịch chứa ký tự tổ hợp NFD (dấu ghép riêng) — nên dán lại dạng NFC để công cụ khác không nhảy chữ."
                .into(),
        )
    } else {
        None
    }
}

/// Cắt ngắn văn bản hiển thị trong danh sách vấn đề.
fn short(text: &str, max: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= max {
        t.to_string()
    } else {
        let s: String = t.chars().take(max).collect();
        format!("{}…", s)
    }
}

// ========================= Báo cáo =========================

/// Ngưỡng EN (ký tự) để kiểm độ dài hoạt động — câu ngắn tỉ lệ giãn tự nhiên lớn.
const LENGTH_MIN_EN_CHARS: usize = 40;
/// Tỉ lệ dài VI/EN ngoài khoảng này là bất thường (tham chiếu bộ ngữ liệu selftest).
const LENGTH_RATIO_LOW: f64 = 0.35;
const LENGTH_RATIO_HIGH: f64 = 2.5;

/// Chạy 8 kiểm QA trên một phiên. THUẦN ĐỌC — không đổi gì trong phiên.
pub fn report(s: &Session) -> QaReport {
    let mut issues: Vec<Issue> = Vec::new();
    for (i, r) in s.rows.iter().enumerate() {
        let en_segs = r.en_segments();
        let vi_segs = r.vi_segments();
        let en: String = en_segs
            .iter()
            .map(|(_, t)| *t)
            .collect::<Vec<&str>>()
            .join(" ");
        let vi: String = vi_segs
            .iter()
            .map(|(_, t)| *t)
            .collect::<Vec<&str>>()
            .join(" ");
        let has_en = !en_segs.is_empty();
        let has_vi = !vi_segs.is_empty();
        let en_s = short(&en, 70);
        let vi_s = short(&vi, 70);

        let mut push = |severity: Severity, check: &'static str, detail: String| {
            issues.push(Issue {
                row: i,
                severity,
                check,
                en: en_s.clone(),
                vi: vi_s.clone(),
                detail,
            });
        };

        // ---- kiểm chạy được trên phía có mặt (4, 5, 6 + 7, 8 phía VI) ----
        // [AUDIT #2] kiểm 1 (số liệu) dời xuống nhóm đủ-hai-phía: trên hàng một phía,
        // mọi số phía có mặt thành “thiếu/thừa” oan — nhiễu hơn giá trị (chữ “none”
        // đã tự nói bên kia thiếu).
        if has_en || has_vi {
            if let Some(d) =
                whitespace_detail(if has_en { &en } else { "" }, if has_vi { &vi } else { "" })
            {
                push(Severity::Warning, "double-space", d);
            }
            if let Some(d) =
                unpaired_symbol_detail(if has_en { &en } else { "" }, if has_vi { &vi } else { "" })
            {
                push(Severity::Warning, "unpaired-symbol", d);
            }
            if let Some(d) = allcaps_mismatch_detail(
                if has_en { &en } else { "" },
                if has_vi { &vi } else { "" },
            ) {
                push(Severity::Warning, "alluppercase-mismatch", d);
            }
        }
        if has_vi {
            if let Some(d) = missing_diacritics_detail(&vi) {
                push(Severity::Info, "missing-diacritics", d);
            }
            if let Some(d) = nfd_mixed_detail(&vi) {
                push(Severity::Info, "nfd-mixed", d);
            }
        }

        // ---- kiểm cần ĐỦ hai phía (1, 2, 3) ----
        if has_en && has_vi {
            if let Some(d) = numeric_mismatch_detail(&en, &vi) {
                push(Severity::Error, "numeric-mismatch", d);
            }
            if !en.trim().is_empty() && en.trim() == vi.trim() {
                push(
                    Severity::Error,
                    "target-same-as-source",
                    "Bản dịch giống hệt bản gốc — có thể chưa dịch hàng này (so sau khi bỏ khoảng trắng hai đầu)."
                        .into(),
                );
            }
            let en_chars = crate::stats::count_chars(&en);
            let vi_chars = crate::stats::count_chars(&vi);
            if en_chars >= LENGTH_MIN_EN_CHARS {
                let ratio = vi_chars as f64 / en_chars as f64;
                if ratio < LENGTH_RATIO_LOW || ratio > LENGTH_RATIO_HIGH {
                    push(
                        Severity::Warning,
                        "length-outlier",
                        format!(
                            "Độ dài lệch bất thường: bản dịch {} ký tự so với bản gốc {} (tỉ lệ {:.2}, bình thường 0.35–2.5). Kiểm tra dịch thiếu/thừa đoạn.",
                            vi_chars, en_chars, ratio
                        ),
                    );
                }
            }
        }
    }

    let mut by_severity: BTreeMap<&'static str, usize> = BTreeMap::new();
    for i in &issues {
        *by_severity.entry(i.severity.as_str()).or_insert(0) += 1;
    }
    QaReport {
        total: issues.len(),
        by_severity,
        issues,
    }
}
