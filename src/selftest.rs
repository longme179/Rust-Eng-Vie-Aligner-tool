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

use crate::aligner::{self, AlignOptions, Status};
use crate::exporter::{export_session, ExportFormat};
use crate::fetcher;
use crate::session::{derive_title, generate_id, Session, SessionStore};
use serde::Serialize;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

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
    *state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    *state >> 33
}

fn synthetic_paragraphs(seed: u64, count: usize) -> String {
    let mut state = seed;
    let mut out: Vec<String> = Vec::with_capacity(count);
    for k in 0..count {
        let year = 1990 + (lcg_next(&mut state) % 40);
        let money = 10 + (lcg_next(&mut state) % 900);
        let name_idx = lcg_next(&mut state) % 5;
        let names = ["Alpha Corp", "Beta Group", "Gamma Ltd", "Delta Fund", "Epsilon AG"];
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
    let out = aligner::align(&corpus_en(), &corpus_vi(), &AlignOptions::default())
        .map_err(|e| e.0)?;
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
    let out = aligner::align(&corpus_en(), &corpus_vi(), &AlignOptions::default())
        .map_err(|e| e.0)?;
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
            return Err(format!("{:?}: {} dòng ≠ {} hàng", fmt, lines.len(), session.rows.len()));
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
                return Err(format!("{:?}: dòng {} có phía rỗng (phải là 'none')", fmt, li));
            }
            let row = &session.rows[li];
            if row.en.is_none() && en != "none" {
                return Err(format!("{:?}: dòng {} phía Anh phải là 'none', được '{}'", fmt, li, en));
            }
            if row.vi.is_none() && vi != "none" {
                return Err(format!("{:?}: dòng {} phía Việt phải là 'none', được '{}'", fmt, li, vi));
            }
            if row.en.is_some() && en == "none" {
                return Err(format!("{:?}: dòng {} phía Anh là văn bản nhưng xuất 'none'", fmt, li));
            }
            let _ = sep;
        }
        if r.none_count != 1 {
            return Err(format!("{:?}: kỳ vọng đúng 1 'none', được {}", fmt, r.none_count));
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
        .filter(|x| x.get("en").and_then(|e| e.as_str()) == Some("none") || x.get("vi").and_then(|e| e.as_str()) == Some("none"))
        .count();
    if none_count != 1 {
        return Err(format!("JSON: kỳ vọng 1 'none', được {}", none_count));
    }
    Ok(())
}

fn t_export_single_side() -> Result<(), String> {
    let out = aligner::align("Alpha beta gamma.", "", &AlignOptions::default())
        .map_err(|e| e.0)?;
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
    s.set_row_status(0, Status::Unmatched).map_err(|e| e.to_string())?;
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
    s.set_row_status(0, Status::HumanConfirmed).map_err(|e| e.to_string())?;
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
            return Err(format!("khởi động lại phải thấy 2 phiên, được {}", store2.count()));
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
    let opts = AlignOptions { auto_threshold: 99.0 }.clamped();
    if (opts.auto_threshold - 0.90).abs() > 1e-9 {
        return Err(format!("clamp trên sai: {}", opts.auto_threshold));
    }
    let opts2 = AlignOptions { auto_threshold: -5.0 }.clamped();
    if (opts2.auto_threshold - 0.05).abs() > 1e-9 {
        return Err(format!("clamp dưới sai: {}", opts2.auto_threshold));
    }
    // ngưỡng cao -> mọi cặp thành needs_review; ngưỡng thấp -> auto
    let out_hi = aligner::align(&corpus_en(), &corpus_vi(), &AlignOptions { auto_threshold: 0.9 })
        .map_err(|e| e.0)?;
    let out_lo = aligner::align(&corpus_en(), &corpus_vi(), &AlignOptions { auto_threshold: 0.05 })
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
        return Err(format!("đoạn nhiều số phải điểm cao hơn ({} vs {})", s0, s1));
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
        return Err(format!("kỳ vọng 3 đoạn (bỏ 'Ngắn.'), được {}: {:?}", paras.len(), paras));
    }
    if paras[0].contains("<b>") || paras[0].contains("bỏ tôi") || paras[0].contains("color:red") {
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
            return Err(format!("đoạn thật '{}' bị lọc nhầm — không được phép", must_keep));
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
        return Err(format!("kỳ vọng đúng 3 đoạn nội dung thật, được {}: {:?}", paras.len(), paras));
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
    let expect = ["auto_matched", "needs_review", "unmatched", "human_confirmed"];
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
    s.set_row_status(0, Status::Unmatched).map_err(|e| e.to_string())?;
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
    let mut c = Counts { auto: 0, needs: 0, unmat: 0, human: 0 };
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
    let s_zero = out_zero.rows[0].score.ok_or("hàng 0 (cos=0) phải có điểm")?;
    // Công thức công khai (bản 2.2): điểm = max(từ_vựng, 0.4*từ_vựng + 0.6*tin_cậy)
    //   * cos=1.0 -> tin_cậy=1 -> 0.4*s_lex + 0.6  (được NÂNG lên)
    //   * cos=0.0 -> tin_cậy=0 -> sàn từ vựng s_lex (GIỮ NGUYÊN — LaBSE không được HẠ)
    let expect_one = (1.0 - aligner::W_EMB) * s_lex + aligner::W_EMB;
    let expect_zero = s_lex;
    if (s_one - expect_one).abs() > 0.003 {
        return Err(format!("cos=1: kỳ vọng ~{:.3}, được {:.3}", expect_one, s_one));
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
    let out_bad = aligner::align_with_emb(en, vi, &opts, Some(&[0.9, 0.9, 0.9])).map_err(|e| e.0)?;
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
    let mapping: Arc<StdMutex<HashMap<String, Vec<f32>>>> =
        Arc::new(StdMutex::new(HashMap::new()));
    for i in 0..5 {
        let mut v = vec![0.0f32; dim];
        v[i] = 1.0;
        mapping.lock().unwrap().insert(en_paras[i].clone(), v.clone());
        mapping.lock().unwrap().insert(vi_paras[i].clone(), v);
    }

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("bind mock: {}", e))?;
    let addr = listener.local_addr().map_err(|e| format!("addr mock: {}", e))?;
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
                (200, "OK", serde_json::json!({ "embeddings": emb }).to_string())
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
            return Err(format!("đoạn {}: được {:?}, kỳ vọng {:?}", i + 1, got, want));
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
    let vi1 = "Thị trưởng nói \"năng lượng sạch, việc làm và tăng trưởng\" sẽ dẫn dắt chương trình.";
    let vi2 = "Đây là đoạn tiếng Việt thừa, không có bản tiếng Anh tương ứng.";
    let rows = vec![
        Row {
            index: 0,
            en_index: Some(0),
            vi_index: Some(0),
            en: Some(en0.into()),
            vi: Some(vi0.into()),
            status: Status::AutoMatched,
            score: Some(0.85),
        },
        Row {
            index: 1,
            en_index: Some(1),
            vi_index: Some(1),
            en: Some(en1.into()),
            vi: Some(vi1.into()),
            status: Status::NeedsReview,
            score: Some(0.5),
        },
        Row {
            index: 2,
            en_index: Some(2),
            vi_index: None,
            en: Some(en2.into()),
            vi: None,
            status: Status::Unmatched,
            score: None,
        },
        Row {
            index: 3,
            en_index: None,
            vi_index: Some(2),
            en: None,
            vi: Some(vi2.into()),
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
        options: AlignOptions { auto_threshold: 0.30 },
        engine: "lexical".into(),
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
        return Err(format!("CSV phải có 1 header + 4 hàng, được {} dòng", lines.len()));
    }
    // escape RFC 4180: nháy kép nội bộ nhân đôi, cả ô bọc trong "..."
    if !r
        .content
        .contains("\"The mayor said \"\"clean energy, jobs and growth\"\" will lead the agenda.\"")
    {
        return Err(format!("CSV escape RFC 4180 sai: {}", r.content));
    }
    if r.content.lines().nth(3).map(|l| !l.ends_with(",none")).unwrap_or(true) {
        return Err(format!("CSV hàng 3 (EN-only) phải kết thúc bằng 'none': {}", r.content));
    }
    if !r.content.contains("\n3,human_confirmed,,none,") {
        return Err(format!("CSV hàng 4 (VI-only) phải có 'none' ở cột EN: {}", r.content));
    }
    if r.none_count != 2 || r.rows != 4 {
        return Err(format!("CSV none_count phải 2, rows 4 — được {}/{}", r.none_count, r.rows));
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

/// Chạy toàn bộ bộ kiểm. Mỗi bài chạy trong thread riêng để panic (nếu có)
/// không làm sập cả tiến trình — panics được coi là FAIL có thông báo.
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
    ];

    let mut results = Vec::with_capacity(tests.len());
    for (name, f) in tests {
        let t0 = Instant::now();
        let handle = std::thread::spawn(move || f());
        let (passed, detail) = match handle.join() {
            Ok(Ok(())) => (true, "đạt".to_string()),
            Ok(Err(e)) => (false, e),
            Err(_) => (false, "PANIC — bài kiểm làm sập thread (đây chính là lỗi bản 1.x)".to_string()),
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
