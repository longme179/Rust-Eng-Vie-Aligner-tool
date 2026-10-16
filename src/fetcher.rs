//! fetcher.rs — tải bài từ URL và trích đoạn văn bản.
//! Hoạt động offline-safe: không tải gì thêm ngoài URL người dùng nhập.
//!
//! BẢN 2.3 — SỬA LỖI "thiếu mục danh sách": bản ≤2.2 chỉ quét thẻ <p>, nên các mục
//! danh sách <li> trong <ul>/<ol> của bài bị BỎ SÓT toàn bộ (vd: bài SBS có
//! "Những thay đổi được đề xuất bao gồm:" + 3 gạch đầu dòng → chỉ lấy được câu dẫn).
//! Giờ bộ trích quét THEO THỨ TỰ TÀI LIỆU cả <p> lẫn <li> (trong <ul>/<ol>) bằng
//! máy quét thẻ có stack, kèm các quy tắc lọc chrom đã kiểm chứng trên 2 bài SBS thật:
//!   * <p>  : nhận khi không nằm trong <li> (tránh trùng lặp <li><p>…</p></li>).
//!   * <li> : nhận khi nằm trong <ul>/<ol>, không lồng trong <li> khác.
//!   * chrom container: mọi block bên trong <nav>/<aside>/<footer> bị bỏ.
//!   * li "chỉ toàn link" (text NGOÀI thẻ <a> < 8 ký tự) bị bỏ — thẻ link-embed/
//!     nút chia sẻ. Mục thật luôn có văn bản ngoài link.
//!   * li "icon/card giao diện" (có <img>/<svg> mà văn bản < 50 ký tự) bị bỏ.
//!   * độ dài tối thiểu: p ≥ 30 ký tự (như cũ), li ≥ 12 ký tự (mục danh sách
//!     thường ngắn hơn đoạn văn).
//! Đối chiếu trên 2 bài SBS thật: VI 27→47 đoạn (thêm đúng 20 mục danh sách thật,
//! giữ nguyên 27 đoạn cũ đúng thứ tự), EN 33→33 (không đổi, mọi li rác bị lọc đúng).

use std::time::Duration;

const MAX_BYTES: usize = 6 * 1024 * 1024; // 6 MB
const MAX_PARAGRAPHS: usize = 800;
const MIN_PARA_LEN: usize = 30; // bỏ đoạn quá ngắn (menu, nút…)
const MIN_ITEM_LEN: usize = 12; // mục danh sách được ngắn hơn đoạn văn
/// li mà phần văn bản NGOÀI thẻ <a> ngắn hơn ngưỡng này ⇒ li "chỉ toàn link" (link-embed,
/// nút chia sẻ, thẻ bài liên quan) — không phải nội dung bài.
const LI_MIN_NON_ANCHOR_TEXT: usize = 8;
/// li có <img>/<svg> mà toàn bộ văn bản ngắn hơn ngưỡng này ⇒ icon/card giao diện.
const LI_ICON_CARD_MAX_TEXT: usize = 50;

pub struct FetchedPage {
    pub url: String,
    pub title: String,
    pub paragraphs: Vec<String>,
    pub bytes: usize,
}

pub fn fetch(url: &str) -> Result<FetchedPage, String> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("URL phải bắt đầu bằng http:// hoặc https://".into());
    }
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(25))
        .redirects(5)
        .build();
    let resp = agent
        .get(url)
        .set(
            "User-Agent",
            "Mozilla/5.0 (compatible; canh-doi-anh-viet/2.0; +local-aligner)",
        )
        .set("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
        .set("Accept-Language", "en,vi;q=0.9")
        .call()
        .map_err(|e| format!("Không tải được trang: {}", e))?;

    let mut reader = resp.into_reader();
    let mut raw: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut buf = [0u8; 32 * 1024];
    use std::io::Read;
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(k) => {
                if raw.len() + k > MAX_BYTES {
                    raw.extend_from_slice(&buf[..MAX_BYTES.saturating_sub(raw.len())]);
                    break; // đủ 6MB thì dừng
                }
                raw.extend_from_slice(&buf[..k]);
            }
            Err(e) => return Err(format!("Lỗi khi đọc nội dung: {}", e)),
        }
    }
    if raw.is_empty() {
        return Err("Trang trả về rỗng.".into());
    }
    let html = String::from_utf8_lossy(&raw).to_string();
    let (title, paragraphs) = extract(&html);
    if paragraphs.is_empty() {
        return Err("Không tìm thấy đoạn văn bản (<p>) hay mục danh sách (<li>) nào trong trang — có thể trang cần JavaScript hoặc chặn truy cập.".into());
    }
    Ok(FetchedPage {
        url: url.to_string(),
        title,
        paragraphs,
        bytes: raw.len(),
    })
}

// ========================= Trích HTML (thủ công, không phụ thuộc ngoài) =========================

/// Trả về (title, các đoạn <p> + mục <li> đã làm sạch — theo đúng thứ tự tài liệu).
///
/// BẢN 2.3: từ máy quét thẻ có stack — vừa lấy <p> vừa lấy <li> của <ul>/<ol>
/// (sửa lỗi "chỉ lấy được câu dẫn, mất hết mục gạch đầu dòng").
/// Dùng `to_ascii_lowercase` (không phải `to_lowercase`) để vị trí byte KHỚP TUYỆT ĐỐI
/// với HTML gốc khi cắt slice — mọi pattern cần so khớp đều là thẻ ASCII.
pub fn extract(html: &str) -> (String, Vec<String>) {
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();

    let mut title = String::new();
    // <title>...</title>
    if let Some(a) = find_tag(&lower, "title") {
        let content_start = a + 7;
        if let Some(rel) = lower[content_start..].find("</title>") {
            let e = content_start + rel;
            if e <= bytes.len() {
                title = collapse(&decode_entities(&html[content_start..e]));
            }
        }
    }
    // fallback: <h1>...</h1>
    if title.is_empty() {
        if let Some(h1) = find_tag(&lower, "h1") {
            let open_end = lower[h1..].find('>').map(|k| h1 + k + 1);
            let close = lower[h1..].find("</h1>").map(|k| h1 + k);
            if let (Some(s), Some(e)) = (open_end, close) {
                if s < e && e <= bytes.len() {
                    title = collapse(&decode_entities(&strip_tags(&html[s..e])));
                }
            }
        }
    }

    // ---- quét block p/li theo thứ tự tài liệu (bản 2.3) ----
    let mut paras: Vec<String> = Vec::new();
    for block in collect_blocks(&lower) {
        let inner = &html[block.inner_start..block.inner_end];
        let text = collapse(&decode_entities(&strip_tags(inner)));
        if is_boilerplate(&text) {
            continue; // chrom giao diện trang (timestamp, nút chia sẻ, newsletter…) — không phải nội dung bài
        }
        if block.kind == "li" {
            // li "chỉ toàn link": phần văn bản NGOÀI mọi thẻ <a> gần như rỗng —
            // đó là link-embed/nút chia sẻ/thẻ bài liên quan, không phải nội dung.
            if non_anchor_text(inner).chars().count() < LI_MIN_NON_ANCHOR_TEXT {
                continue;
            }
            // li icon/card giao diện (vd: khối "tải app" có <img>+nhãn ngắn).
            let frag = &lower[block.inner_start..block.inner_end];
            if (has_tag_open(frag, "img") || has_tag_open(frag, "svg"))
                && text.chars().count() < LI_ICON_CARD_MAX_TEXT
            {
                continue;
            }
        }
        let min_len = if block.kind == "p" { MIN_PARA_LEN } else { MIN_ITEM_LEN };
        if text.chars().count() >= min_len {
            if paras.len() < MAX_PARAGRAPHS {
                paras.push(text);
            } else {
                break;
            }
        }
    }
    (title, paras)
}

// ========================= Máy quét thẻ (bản 2.3) =========================

/// Thẻ "void" không bao giờ có thẻ đóng (HTML5) — không đưa vào stack.
fn is_void_tag(name: &str) -> bool {
    matches!(
        name,
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta"
            | "param" | "source" | "track" | "wbr"
    )
}

/// Container "chrom giao diện" — mọi block p/li bên trong bị bỏ khi tải URL.
fn is_chrome_container(name: &str) -> bool {
    matches!(name, "nav" | "aside" | "footer")
}

/// Một thẻ đã quét được (chỉ chứa thông tin vị trí + tên).
#[derive(Debug)]
struct Tag {
    /// vị trí ký tự '<'
    start: usize,
    /// vị trí ngay sau '>' của thẻ
    end: usize,
    name: String,
    is_close: bool,
    is_self_close: bool,
}

/// Quét TẤT CẢ thẻ theo thứ tự tài liệu. Chịu được:
/// * '>' nằm trong giá trị thuộc tính được bọc nháy ("…>…");
/// * comment/DOCTYPE (bỏ qua — sau '<' không phải chữ cái);
/// * '<' rơi trong văn bản (vd "5 < 6") — bỏ qua từng ký tự.
/// * thẻ tự đóng `<br/>`.
fn scan_tags(lower: &str) -> Vec<Tag> {
    let b = lower.as_bytes();
    let n = b.len();
    let mut tags = Vec::new();
    let mut i = 0usize;
    while i < n {
        // tìm '<'
        match lower[i..].find('<') {
            Some(rel) => i += rel,
            None => break,
        }
        let after = lower.get(i + 1..).and_then(|s| s.chars().next());
        let is_close = after == Some('/');
        let name_start = if is_close { i + 2 } else { i + 1 };
        let first = lower.get(name_start..).and_then(|s| s.chars().next());
        if !first.map_or(false, |c| c.is_ascii_alphabetic()) {
            i += 1; // <!-- … -->, <!DOCTYPE, <?xml…, hoặc '<' rơi trong văn bản
            continue;
        }
        let name_end = lower[name_start..]
            .find(|c: char| !(c.is_ascii_alphanumeric()))
            .map(|k| name_start + k)
            .unwrap_or(n);
        let name = lower[name_start..name_end].to_string();
        // tìm '>' kết thúc thẻ — tôn trọng attr bọc trong nháy kép/đơn
        let mut j = name_end;
        let mut quote: Option<u8> = None;
        let mut gt = None;
        while j < n {
            let ch = b[j];
            match quote {
                Some(q) => {
                    if ch == q {
                        quote = None;
                    }
                }
                None => {
                    if ch == b'"' || ch == b'\'' {
                        quote = Some(ch);
                    } else if ch == b'>' {
                        gt = Some(j);
                        break;
                    }
                }
            }
            j += 1;
        }
        let gt = match gt {
            Some(g) => g,
            None => break, // thẻ hỏng không đóng — phần còn lại không dùng được
        };
        let is_self_close = gt > 0 && b[gt - 1] == b'/';
        tags.push(Tag {
            start: i,
            end: gt + 1,
            name,
            is_close,
            is_self_close,
        });
        i = gt + 1;
    }
    tags
}

/// Một khối nội dung thu được (p hoặc li).
struct Block {
    /// "p" hoặc "li"
    kind: &'static str,
    /// vị trí bắt đầu nội dung (ngay sau '>' của thẻ mở)
    inner_start: usize,
    /// vị trí '<' của thẻ đóng tương ứng
    inner_end: usize,
}

/// Duyệt stack thẻ, thu các block p/li THEO THỨ TỰ TÀI LIỆU, áp quy tắc lọc chrom:
/// * bỏ mọi block nằm trong <nav>/<aside>/<footer>;
/// * <p> chỉ nhận khi không nằm trong <li> (tránh trùng lặp <li><p>…</p></li>);
/// * <li> chỉ nhận khi nằm trong <ul>/<ol>, không lồng trong <li> khác
///   (mục lồng cấp 2 như cũ vẫn bị bỏ — hiếm và thường là giao diện).
/// Chịu HTML không khớp hoàn hảo: thẻ đóng khớp với thẻ mở GẦN NHẤT cùng tên
/// (thẻ con không đóng sẽ được đóng ngầm — đúng cách trình duyệt xử lý).
fn collect_blocks(lower: &str) -> Vec<Block> {
    let tags = scan_tags(lower);
    // stack: (tên thẻ, capture_start nếu là p/li ĐƯỢC nhận)
    let mut stack: Vec<(String, Option<usize>)> = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    for tag in tags {
        if tag.is_close {
            if let Some(k) = stack.iter().rposition(|(nm, _)| *nm == tag.name) {
                for (nm, cap) in stack.split_off(k) {
                    if let Some(inner_start) = cap {
                        blocks.push(Block {
                            kind: if nm == "p" { "p" } else { "li" },
                            inner_start,
                            inner_end: tag.start,
                        });
                    }
                }
            }
            continue;
        }
        if is_void_tag(&tag.name) || tag.is_self_close {
            continue;
        }
        let in_li = stack.iter().any(|(nm, _)| nm == "li");
        let in_p = stack.iter().any(|(nm, _)| nm == "p");
        let in_chrome = stack.iter().any(|(nm, _)| is_chrome_container(nm));
        let in_list = stack.iter().any(|(nm, _)| nm == "ul" || nm == "ol");
        let cap = if tag.name == "p" && !in_li && !in_p && !in_chrome {
            Some(tag.end)
        } else if tag.name == "li" && in_list && !in_li && !in_p && !in_chrome {
            Some(tag.end)
        } else {
            None
        };
        stack.push((tag.name, cap));
    }
    blocks
}

/// Toàn bộ văn bản nằm NGOÀI mọi thẻ <a>…</a> của một đoạn HTML
/// (đã strip thẻ, giải mã thực thể, gộp khoảng trắng).
/// Dùng để nhận diện li "chỉ toàn link": mục thật luôn có văn bản ngoài link,
/// còn link-embed/nút chia sẻ thì gần như không có.
fn non_anchor_text(inner: &str) -> String {
    let lower = inner.to_ascii_lowercase();
    let tags = scan_tags(&lower);
    let mut out = String::new();
    let mut depth = 0usize; // độ sâu thẻ <a> đang mở
    let mut last = 0usize; // vị trí đã sao chép tới
    for tag in tags {
        if tag.name != "a" {
            continue;
        }
        if tag.is_close {
            if depth > 0 {
                depth -= 1;
                if depth == 0 {
                    // bỏ qua toàn bộ văn bản bên trong link — tiếp tục sao chép từ sau </a>
                    last = tag.end;
                }
            }
            continue;
        }
        // thẻ mở <a …> ở tầng ngoài cùng: giữ lại văn bản TRƯỚC link
        if depth == 0 {
            if tag.start > last {
                out.push_str(&inner[last..tag.start]);
            }
            last = tag.end;
        }
        depth += 1;
    }
    if last < inner.len() {
        out.push_str(&inner[last..]);
    }
    collapse(&decode_entities(&strip_tags(&out)))
}

/// Đoạn `frag` (đã thường hoá) có chứa thẻ mở `<name` hợp lệ không
/// (theo sau là khoảng trắng, '>' hoặc '/').
fn has_tag_open(frag: &str, name: &str) -> bool {
    let pat = format!("<{}", name);
    let mut pos = 0usize;
    while let Some(rel) = frag[pos..].find(&pat) {
        let at = pos + rel;
        let after = frag[at + pat.len()..].chars().next();
        if matches!(after, Some(' ') | Some('>') | Some('\t') | Some('\n') | Some('\r') | Some('/')) {
            return true;
        }
        pos = at + pat.len();
    }
    false
}

/// Nhận diện đoạn "chrom giao diện" của trang tin — KHÔNG phải nội dung bài viết
/// (dòng thời gian, nút chia sẻ, mời đăng ký bản tin, lời cảm ơn truyền thống,
/// địa chỉ bưu điện…). Chỉ áp cho nội dung tải từ URL; văn bản người dùng tự dán
/// không đi qua hàm này. Quy tắc rất kĩ tính:
/// - timestamp: chỉ drop khi ngay sau nhãn là CHỮ SỐ ("Published 3 …", "Cập nhật 12/2 …")
///   — "Published reports suggest…" (văn thật) KHÔNG bị drop.
/// - các mẫu còn lại là câu cửa miệng đậm tính chrom, gần như không thể mở đầu đoạn văn thật.
fn is_boilerplate(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    if t.is_empty() {
        return true;
    }
    // ---- dòng thời gian / nhãn xuất bản (EN + VI) ----
    for p in ["published ", "updated ", "last updated ", "cập nhật ", "đăng ngày "] {
        if let Some(rest) = t.strip_prefix(p) {
            if rest.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    if t.starts_with("đã xuất bản") {
        return true;
    }
    // ---- nút chia sẻ / mạng xã hội ----
    if t.starts_with("share this with")
        || t.starts_with("chia sẻ bài viết")
        || t.starts_with("chia sẻ với")
    {
        return true;
    }
    // ---- mời đăng ký bản tin / điều khoản ----
    if t.starts_with("by subscribing")
        || t.starts_with("bằng việc đăng ký")
        || t.starts_with("sign up now for")
        || t.starts_with("đăng ký ngay")
        || t.starts_with("your trusted source for")
    {
        return true;
    }
    // ---- khối quảng bá podcast / bản tin (chrom quen thuộc của trang tin) ----
    if t.starts_with("get the latest with our")
        || t.starts_with("take a global view")
        || t.starts_with("watch the latest news videos")
    {
        return true;
    }
    // ---- nghe bài / ứng dụng âm thanh ----
    if t.starts_with("listen to this article")
        || t.starts_with("nghe bài viết")
        || t.starts_with("nghe sbs tiếng việt")
        || t.starts_with("đồng hành cùng chúng tôi tại")
    {
        return true;
    }
    // ---- lời ghi nhận truyền thống lặp lại trên mọi trang của trang web ----
    if t.contains("acknowledges the traditional custodians") {
        return true;
    }
    // ---- địa chỉ bưu điện (chân trang) ----
    if t.contains("locked bag") || t.contains("p.o. box") || t.contains("po box") || t.contains("gpo box") {
        return true;
    }
    false
}

/// Tìm vị trí mở đầu thẻ `<name` (case-insensitive) — trả về chỉ số của '<'.
fn find_tag(lower: &str, name: &str) -> Option<usize> {
    let pat = format!("<{}", name);
    let a = lower.find(&pat)?;
    let after = lower.get(a + pat.len()..).and_then(|s| s.chars().next());
    match after {
        Some(' ') | Some('>') | Some('\n') | Some('\t') | Some('\r') | Some('/') => Some(a),
        _ => {
            // thử thẻ kế tiếp
            let rest = &lower[a + pat.len()..];
            rest.find(&pat).map(|k| a + pat.len() + k)
        }
    }
}

/// Bỏ mọi thẻ HTML, giữ lại text (đã bỏ nội dung script/style).
fn strip_tags(fragment: &str) -> String {
    let lower = fragment.to_lowercase();
    let mut out = String::with_capacity(fragment.len());
    let mut i = 0usize;
    let n = fragment.len();
    while i < n {
        match lower[i..].find('<') {
            Some(rel) => {
                let start = i + rel;
                if start > i {
                    out.push_str(&fragment[i..start]);
                }
                // tên thẻ
                let after = &lower[start + 1..];
                let name_end = after
                    .find(|c: char| c == ' ' || c == '>' || c == '/')
                    .unwrap_or(after.len());
                let name = &after[..name_end];
                let close_rel = lower[start..].find('>').unwrap_or(n - start);
                let close = start + close_rel;
                if name == "script" || name == "style" || name == "noscript" {
                    // bỏ tới </script>/<style>
                    let end_pat = format!("</{}", name);
                    if let Some(k) = lower[close..].find(&end_pat) {
                        i = close + k + end_pat.len();
                        // nhảy qua '>' của thẻ đóng
                        if let Some(g) = lower[i..].find('>') {
                            i += g + 1;
                        }
                        continue;
                    } else {
                        i = n;
                        continue;
                    }
                }
                i = close + 1;
            }
            None => {
                out.push_str(&fragment[i..]);
                break;
            }
        }
    }
    out
}

/// Giải mã các thực thể HTML phổ biến.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '&' {
            // tìm ';'
            let mut j = i + 1;
            let mut ent = String::new();
            let mut found = false;
            while j < s.len() {
                let ch = s[j..].chars().next().unwrap();
                if ch == ';' {
                    found = true;
                    break;
                }
                if ch.is_ascii_alphanumeric() || ch == '#' {
                    ent.push(ch);
                    j += ch.len_utf8();
                    if ent.len() > 10 {
                        break;
                    }
                } else {
                    break;
                }
            }
            if found && !ent.is_empty() {
                let rep = match ent.as_str() {
                    "amp" => Some('&'),
                    "lt" => Some('<'),
                    "gt" => Some('>'),
                    "quot" => Some('"'),
                    "apos" => Some('\''),
                    "nbsp" => Some(' '),
                    _ => {
                        if let Some(num) = ent.strip_prefix('#') {
                            let cp = if let Some(hex) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
                                u32::from_str_radix(hex, 16).ok()
                            } else {
                                num.parse::<u32>().ok()
                            };
                            cp.and_then(char::from_u32)
                        } else {
                            None
                        }
                    }
                };
                if let Some(r) = rep {
                    out.push(r);
                    // nhảy tới sau ';'
                    while let Some(&(k, _)) = chars.peek() {
                        if k <= j {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    continue;
                }
            }
        }
        out.push(c);
    }
    out
}

/// Gộp khoảng trắng, chuẩn hoá 1 dòng.
fn collapse(s: &str) -> String {
    s.replace(['\n', '\r', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}
