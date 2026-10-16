//! exporter.rs — xuất kết quả ghép.
//!
//! QUY ƯỚC BẮT BUỘC (giữ nguyên của bản gốc): khi một phía thiếu, xuất ĐÚNG CHỮ `none`
//! (không phải chuỗi rỗng, không phải null) — áp cho MỌI định dạng. Mọi đoạn đầu vào
//! đều xuất đúng 1 lần.
//!
//! BẢN 2.3 — thêm 3 định dạng theo yêu cầu: CSV (Excel mở đúng tiếng Việt nhờ BOM +
//! escape RFC 4180), HTML (bảng đối chiếu tô màu theo trạng thái), XLIFF 1.2 (dùng cho
//! công cụ dịch/CAT). Các định dạng cũ (TSV / Moses / JSON) giữ nguyên từng byte.
//!
//! BẢN 2.4 — hỗ trợ HÀNG GỘP (kéo–thả 1 EN ↔ nhiều VI):
//! * TSV / Moses / XLIFF: các đoạn gộp nối bằng dấu cách.
//! * CSV: các đoạn gộp nằm trong CÙNG MỘT Ô, mỗi đoạn một dòng (trích dẫn RFC 4180).
//! * JSON: `en`/`vi` nối bằng dấu cách + kèm `enParts`/`viParts` khi >1 đoạn.
//! * HTML: mỗi đoạn gộp một dòng trong ô, kèm nhãn số đoạn gốc.
//!
//! BẢN 2.5 — phiên đã kéo–thả ĐỔI THỨ TỰ khối (cờ `reordered`): bỏ kiểm đơn điệu I3
//! khi xuất (người dùng sắp xếp lại CÓ CHỦ ĐÍCH) — I1/I2/I4/I5 và quy ước `none`
//! vẫn kiểm gắt như cũ. Thứ tự xuất = thứ tự khối hiển thị trên giao diện.

use crate::aligner::{validate_invariants_opts, AlignOutput, Row, Status};
use crate::session::Session;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportFormat {
    Tsv,
    Moses,
    Json,
    Csv,
    Html,
    Xliff,
}

impl ExportFormat {
    pub fn parse(s: &str) -> Option<ExportFormat> {
        match s {
            "tsv" => Some(ExportFormat::Tsv),
            "moses" | "txt" | " ||| " => Some(ExportFormat::Moses),
            "json" => Some(ExportFormat::Json),
            "csv" => Some(ExportFormat::Csv),
            "html" | "htm" => Some(ExportFormat::Html),
            "xliff" | "xlf" => Some(ExportFormat::Xliff),
            _ => None,
        }
    }

    pub fn ext(&self) -> &'static str {
        match self {
            ExportFormat::Tsv => "tsv",
            ExportFormat::Moses => "txt",
            ExportFormat::Json => "json",
            ExportFormat::Csv => "csv",
            ExportFormat::Html => "html",
            ExportFormat::Xliff => "xlf",
        }
    }

    pub fn mime(&self) -> &'static str {
        match self {
            ExportFormat::Tsv => "text/tab-separated-values; charset=utf-8",
            ExportFormat::Moses => "text/plain; charset=utf-8",
            ExportFormat::Json => "application/json; charset=utf-8",
            ExportFormat::Csv => "text/csv; charset=utf-8",
            ExportFormat::Html => "text/html; charset=utf-8",
            ExportFormat::Xliff => "application/x-xliff+xml; charset=utf-8",
        }
    }
}

/// Chuẩn hoá 1 dòng để không phá định dạng: bỏ xuống dòng, gộp khoảng trắng.
fn one_line(s: &str) -> String {
    s.replace(['\n', '\r', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Ghép các đoạn phía Anh của một hàng thành 1 chuỗi; None khi phía đó thiếu (→ `none`).
fn en_joined(r: &Row, sep: &str) -> Option<String> {
    let segs = r.en_segments();
    if segs.is_empty() {
        return None;
    }
    Some(
        segs.iter()
            .map(|(_, t)| one_line(t))
            .collect::<Vec<_>>()
            .join(sep),
    )
}

/// Ghép các đoạn phía Việt của một hàng thành 1 chuỗi; None khi phía đó thiếu (→ `none`).
fn vi_joined(r: &Row, sep: &str) -> Option<String> {
    let segs = r.vi_segments();
    if segs.is_empty() {
        return None;
    }
    Some(
        segs.iter()
            .map(|(_, t)| one_line(t))
            .collect::<Vec<_>>()
            .join(sep),
    )
}

/// Chuẩn hoá một hàng thành 2 ô (en, vi): phía thiếu → ĐÚNG CHỮ `none` (quy ước gốc).
/// Trả về (en, vi, số phía thiếu 0..2). Hàng gộp: các đoạn nối bằng `sep`.
fn row_cells_sep(r: &Row, sep: &str) -> (String, String, usize) {
    let mut none = 0usize;
    let en = match en_joined(r, sep) {
        Some(t) => t,
        None => {
            none += 1;
            "none".to_string()
        }
    };
    let vi = match vi_joined(r, sep) {
        Some(t) => t,
        None => {
            none += 1;
            "none".to_string()
        }
    };
    (en, vi, none)
}

/// Chuẩn hoá một hàng thành 2 ô nối bằng dấu cách (TSV / Moses / XLIFF / JSON.en-vi).
fn row_cells(r: &Row) -> (String, String, usize) {
    row_cells_sep(r, " ")
}

/// Escape một trường CSV theo RFC 4180: chứa `,` `"` `\n` `\r` → bọc `"`, `"` nội bộ → `""`.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Escape văn bản cho HTML.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Escape văn bản cho XML.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Nhãn tiếng Việt của trạng thái (dùng trong CSV/HTML).
fn status_label_vi(st: &Status) -> &'static str {
    match st {
        Status::AutoMatched => "Tự khớp",
        Status::NeedsReview => "Cần soát",
        Status::Unmatched => "Không khớp",
        Status::HumanConfirmed => "Đã xác nhận",
    }
}

/// Nhóm màu theo quy ước 3 màu của người dùng (bản 2.3):
/// khớp / khớp phần lớn → xanh lá · cần soát / khớp 1 phần → vàng · không khớp hoàn toàn → đỏ.
fn status_color_key(st: &Status) -> &'static str {
    match st {
        Status::AutoMatched | Status::HumanConfirmed => "green",
        Status::NeedsReview => "yellow",
        Status::Unmatched => "red",
    }
}

pub struct ExportResult {
    pub content: String,
    pub filename: String,
    pub mime: String,
    pub rows: usize,
    pub none_count: usize,
}

/// Xuất phiên hiện tại. Kiểm tra bất biến trước khi xuất:
/// I1/I2/I4/I5 luôn gắt; riêng I3 (đơn điệu) chỉ bắt khi phiên CHƯA bị kéo–thả
/// đổi thứ tự khối (bản 2.5 — sắp xếp lại là ý đồ người dùng, không phải lỗi).
pub fn export_session(session: &Session, format: ExportFormat) -> Result<ExportResult, String> {
    // ---- I5 + I1..I4: kiểm tra trước khi xuất
    let out = AlignOutput {
        english: session.english.clone(),
        vietnamese: session.vietnamese.clone(),
        rows: session.rows.clone(),
        options: session.options.clone(),
        engine: session.engine.clone(),
    };
    validate_invariants_opts(&out, !session.reordered)?;

    let mut none_count = 0usize;
    let mut body = String::new();

    match format {
        ExportFormat::Tsv => {
            for r in &session.rows {
                let (en, vi, none) = row_cells(r);
                none_count += none;
                body.push_str(&en);
                body.push('\t');
                body.push_str(&vi);
                body.push('\n');
            }
        }
        ExportFormat::Moses => {
            for r in &session.rows {
                let (en, vi, none) = row_cells(r);
                none_count += none;
                body.push_str(&en);
                body.push_str(" ||| ");
                body.push_str(&vi);
                body.push('\n');
            }
        }
        ExportFormat::Csv => {
            // BOM UTF-8 để Excel nhận đúng tiếng Việt ngay khi mở.
            // Hàng gộp: các đoạn nằm trong cùng một ô, mỗi đoạn một dòng (RFC 4180).
            body.push('\u{FEFF}');
            body.push_str("index,status,score,en,vi\n");
            for r in &session.rows {
                let (en, vi, none) = row_cells_sep(r, "\n");
                none_count += none;
                let score = match r.score {
                    Some(s) => format!("{:.3}", s),
                    None => String::new(),
                };
                body.push_str(&format!(
                    "{},{},{},{},{}\n",
                    r.index,
                    r.status.as_str(),
                    score,
                    csv_field(&en),
                    csv_field(&vi)
                ));
            }
        }
        ExportFormat::Json => {
            let rows_json: Vec<serde_json::Value> = session
                .rows
                .iter()
                .map(|r| {
                    let (en, vi, none) = row_cells(r);
                    none_count += none;
                    let mut obj = serde_json::json!({
                        "index": r.index,
                        "status": r.status.as_str(),
                        "score": r.score,
                        "en": en,
                        "vi": vi,
                    });
                    // hàng gộp (bản 2.4): kèm danh sách đoạn gốc để dùng máy được
                    let en_segs = r.en_segments();
                    if en_segs.len() > 1 {
                        obj["enParts"] = serde_json::json!(
                            en_segs.iter().map(|(_, t)| one_line(t)).collect::<Vec<_>>()
                        );
                    }
                    let vi_segs = r.vi_segments();
                    if vi_segs.len() > 1 {
                        obj["viParts"] = serde_json::json!(
                            vi_segs.iter().map(|(_, t)| one_line(t)).collect::<Vec<_>>()
                        );
                    }
                    obj
                })
                .collect();
            let doc = serde_json::json!({
                "tool": "reva-tool",
                "version": env!("CARGO_PKG_VERSION"),
                "session": {
                    "id": session.id,
                    "title": session.title,
                    "createdAt": session.created_at,
                    "updatedAt": session.updated_at,
                },
                "counts": {
                    "total": session.rows.len(),
                    "auto_matched": session.rows.iter().filter(|r| r.status == Status::AutoMatched).count(),
                    "needs_review": session.rows.iter().filter(|r| r.status == Status::NeedsReview).count(),
                    "unmatched": session.rows.iter().filter(|r| r.status == Status::Unmatched).count(),
                    "human_confirmed": session.rows.iter().filter(|r| r.status == Status::HumanConfirmed).count(),
                },
                "rows": rows_json,
            });
            body = serde_json::to_string_pretty(&doc)
                .map_err(|e| format!("Lỗi tạo JSON: {}", e))?;
            body.push('\n');
        }
        ExportFormat::Html => {
            let c = session.counts();
            body.push_str(&format!(
                concat!(
                    "<!DOCTYPE html>\n",
                    "<html lang=\"vi\">\n<head>\n<meta charset=\"utf-8\">\n",
                    "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n",
                    "<title>REVA tool — Phiên {sid}</title>\n",
                    "<style>\n",
                    ":root{{--green:#047857;--green-bg:#ecfdf5;--yellow:#b45309;--yellow-bg:#fffbeb;--red:#be123c;--red-bg:#fff1f2;--line:#e5e7eb;--muted:#6b7280}}\n",
                    "*{{box-sizing:border-box}}\n",
                    "body{{margin:0;font:14px/1.6 -apple-system,BlinkMacSystemFont,\"Segoe UI\",Roboto,Arial,sans-serif;color:#1c2b28;background:#fff}}\n",
                    "main{{max-width:1100px;margin:0 auto;padding:26px 18px 60px}}\n",
                    "h1{{font-size:19px;margin:0 0 4px}}\n",
                    ".meta{{color:var(--muted);font-size:12.5px;margin-bottom:18px}}\n",
                    ".legend{{display:flex;gap:10px;flex-wrap:wrap;margin:0 0 14px;font-size:12.5px}}\n",
                    ".legend span{{display:inline-flex;align-items:center;gap:6px;padding:3px 10px;border-radius:99px}}\n",
                    ".lg-g{{background:var(--green-bg);color:var(--green)}}.lg-y{{background:var(--yellow-bg);color:var(--yellow)}}.lg-r{{background:var(--red-bg);color:var(--red)}}\n",
                    "table{{border-collapse:collapse;width:100%}}\n",
                    "th,td{{border:1px solid var(--line);padding:9px 12px;text-align:left;vertical-align:top;overflow-wrap:anywhere}}\n",
                    "thead th{{background:#f8fafa;font-size:12px;letter-spacing:.4px;text-transform:uppercase;color:var(--muted)}}\n",
                    "tr.g td{{background:var(--green-bg)}}\n",
                    "tr.y td{{background:var(--yellow-bg)}}\n",
                    "tr.r td{{background:var(--red-bg)}}\n",
                    "td.idx{{width:36px;color:var(--muted);font-variant-numeric:tabular-nums}}\n",
                    "td.st{{width:104px;font-size:12px;font-weight:700;white-space:nowrap}}\n",
                    "td.st .pct{{display:block;font-weight:400;color:var(--muted)}}\n",
                    "td.miss{{color:#9ca3af;font-style:italic}}\n",
                    "td.miss code{{font-style:normal;font-family:ui-monospace,Menlo,Consolas,monospace;background:#f3f4f6;border-radius:5px;padding:1px 7px;font-size:12px;color:#4b5563}}\n",
                    "span.sg{{font-family:ui-monospace,Menlo,Consolas,monospace;font-size:10.5px;color:#9ca3af;margin-left:7px;white-space:nowrap}}\n",
                    "span.mg{{display:inline-block;font-size:10.5px;font-weight:800;color:#7c2d12;background:#ffedd5;border:1px solid #fed7aa;border-radius:99px;padding:1px 9px;margin-bottom:4px}}\n",
                    "footer{{margin-top:18px;color:var(--muted);font-size:12px}}\n",
                    "</style>\n</head>\n<body>\n<main>\n",
                    "<h1>REVA tool — kết quả ghép đoạn</h1>\n",
                    "<p class=\"meta\">Phiên <b>{sid}</b> · {title} · xuất lúc {when} · công cụ reva-tool v{ver}</p>\n",
                    "<p class=\"legend\"><span class=\"lg-g\">■ Khớp / khớp phần lớn ({ng})</span><span class=\"lg-y\">■ Cần soát / khớp một phần ({ny})</span><span class=\"lg-r\">■ Không khớp hoàn toàn ({nr})</span></p>\n",
                    "<table>\n<thead><tr><th>#</th><th>Tiếng Anh</th><th>Tiếng Việt</th><th>Trạng thái</th></tr></thead>\n<tbody>\n"
                ),
                sid = html_escape(&session.id),
                title = html_escape(&session.title),
                when = html_escape(&format_time(session.updated_at)),
                ver = env!("CARGO_PKG_VERSION"),
                ng = c.auto_matched + c.human_confirmed,
                ny = c.needs_review,
                nr = c.unmatched,
            ));
            for r in &session.rows {
                // Hàng gộp (bản 2.4): mỗi đoạn một dòng, kèm nhãn số đoạn gốc.
                let en_segs = r.en_segments();
                let vi_segs = r.vi_segments();
                none_count += (if en_segs.is_empty() { 1 } else { 0 }) + (if vi_segs.is_empty() { 1 } else { 0 });
                let segs_html = |segs: &Vec<(usize, &str)>| {
                    segs.iter()
                        .map(|(i, t)| {
                            if segs.len() == 1 {
                                html_escape(&one_line(t))
                            } else {
                                // hàng gộp: ghi rõ từng đoạn gốc số mấy để đối chiếu
                                format!(
                                    "{}<span class=\"sg\">·đoạn {}</span>",
                                    html_escape(&one_line(t)),
                                    i + 1
                                )
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("<br>")
                };
                let merged_badge = |n_en: usize, n_vi: usize| {
                    if n_en > 1 || n_vi > 1 {
                        format!("<span class=\"mg\">🔗 {} EN · {} VI — đã gộp</span>", n_en, n_vi)
                    } else {
                        String::new()
                    }
                };
                let en_cell = if !en_segs.is_empty() {
                    let badge = merged_badge(en_segs.len(), vi_segs.len());
                    let inner = segs_html(&en_segs);
                    if badge.is_empty() {
                        format!("<td>{}</td>", inner)
                    } else {
                        format!("<td>{}<br>{}</td>", badge, inner)
                    }
                } else {
                    format!("<td class=\"miss\"><code>none</code> — không có đoạn tiếng Anh</td>")
                };
                let vi_cell = if !vi_segs.is_empty() {
                    let inner = segs_html(&vi_segs);
                    format!("<td>{}</td>", inner)
                } else {
                    format!("<td class=\"miss\"><code>none</code> — không có bản dịch</td>")
                };
                let pct = match r.score {
                    Some(s) => format!("<span class=\"pct\">{:.0}%</span>", (s * 100.0).round()),
                    None => String::new(),
                };
                body.push_str(&format!(
                    "<tr class=\"{}\"><td class=\"idx\">{}</td>{}{}<td class=\"st\">{}{}</td></tr>\n",
                    status_color_key(&r.status),
                    r.index + 1,
                    en_cell,
                    vi_cell,
                    html_escape(status_label_vi(&r.status)),
                    pct,
                ));
            }
            body.push_str("</tbody>\n</table>\n");
            body.push_str(&format!(
                "<footer>Tổng {t} hàng · {a} tự khớp · {rv} cần soát · {u} không khớp · {h} đã xác nhận · {nc} ô ghi chữ “none”.</footer>\n",
                t = c.total,
                a = c.auto_matched,
                rv = c.needs_review,
                u = c.unmatched,
                h = c.human_confirmed,
                nc = none_count,
            ));
            body.push_str("</main>\n</body>\n</html>\n");
        }
        ExportFormat::Xliff => {
            // XLIFF 1.2 cho công cụ dịch/CAT. Quy ước `none` vẫn giữ nguyên:
            // phía thiếu ghi đúng chữ none trong source/target tương ứng.
            body.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            body.push_str(&format!(
                "<xliff xmlns=\"urn:oasis:names:tc:xliff:document:1.2\" version=\"1.2\">\n"
            ));
            body.push_str(&format!(
                " <file original=\"reva-tool-{id}\" source-language=\"en\" target-language=\"vi\" datatype=\"plaintext\" tool=\"reva-tool v{ver}\">\n",
                id = xml_escape(&session.id),
                ver = env!("CARGO_PKG_VERSION"),
            ));
            body.push_str("  <body>\n");
            for r in &session.rows {
                let (en, vi, none) = row_cells(r);
                none_count += none;
                let approved = matches!(r.status, Status::AutoMatched | Status::HumanConfirmed)
                    .then_some("yes")
                    .unwrap_or("no");
                let state = match r.status {
                    Status::AutoMatched => "translated",
                    Status::NeedsReview => "needs-review-translation",
                    Status::HumanConfirmed => "final",
                    Status::Unmatched => "missing",
                };
                body.push_str(&format!(
                    "   <trans-unit id=\"{}\" approved=\"{}\" resname=\"{}\">\n",
                    r.index + 1,
                    approved,
                    xml_escape(r.status.as_str())
                ));
                body.push_str(&format!(
                    "    <source xml:lang=\"en\">{}</source>\n",
                    xml_escape(&en)
                ));
                body.push_str(&format!(
                    "    <target xml:lang=\"vi\" state=\"{}\">{}</target>\n",
                    state,
                    xml_escape(&vi)
                ));
                body.push_str("   </trans-unit>\n");
            }
            body.push_str("  </body>\n");
            body.push_str(" </file>\n");
            body.push_str("</xliff>\n");
        }
    }

    Ok(ExportResult {
        filename: format!("reva-tool-{}.{}", session.id, format.ext()),
        mime: format.mime().to_string(),
        content: body,
        rows: session.rows.len(),
        none_count,
    })
}

/// Định dạng thời gian cho phần meta HTML (ISO-8601, không phụ thuộc crate ngoài).
fn format_time(ms: u64) -> String {
    // đổi ms → ngày/tháng/năm + giờ:phút UTC (đủ dùng cho nhãn xuất file)
    let secs = ms / 1000;
    let days = secs / 86400;
    let rem = secs % 86400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    // ngày từ epoch (thuật toán Howard Hinnant, không cần chrono)
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", y, m, d, h, mi, s)
}
