//! fetcher.rs — tải bài từ URL và trích đoạn văn bản.
//! Hoạt động offline-safe: không tải gì thêm ngoài URL người dùng nhập.
//!
//! BẢN 2.6 — SỬA LỖI "hớt cả ribbon/menu vào văn bản" (báo cáo trên tuoitre.vn):
//! bản ≤2.5 chỉ bỏ subtree <nav>/<aside>/<footer>, nhưng ribbon/mega-menu/modal
//! của nhiều báo nằm trong <div class="popup-megamenu">, <ul class="menu-nav">…
//! (không dùng thẻ nav) nên tràn hết vào kết quả. Bộ lọc mới 3 tầng, tổng quát
//! cho mọi trang tin (không hard-code theo tên site):
//!   1. CHROME THEO THUỘC TÍNH: mọi <p>/<li> có TỔ TIÊN bất kỳ (hoặc chính nó)
//!      mang class/id chứa từ khoá giao diện (menu, nav, mega, ribbon, modal,
//!      popup, comment, share, sidebar, breadcrumb, newslette(r), related,
//!      login, banner…) bị bỏ — so KHÔP CHUỖI con sau khi bỏ ký tự ngăn cách.
//!   2. MẬT ĐỘ LINK ("link farm"): đoạn có ≥5 link và ≥55% ký tự nằm trong
//!      thẻ <a> (hoặc ≥2 link với ≥80%) là menu/danh sách bài liên quan — bỏ.
//!      Đoạn văn thật gần như không bao giờ toàn link.
//!   3. CAPTION/CRÉDIT ẢNH: <p> nằm trong <figure>/<figcaption> là chú thích —
//!      bỏ khi ngắn (<100 ký tự) hoặc là dòng credit ("Ảnh: X", "Nguồn: Y"…).
//! Bên cạnh đó, mọi quy tắc của bản 2.3 (li, boilerplate, icon-card…) giữ nguyên.
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

use std::net::{IpAddr, ToSocketAddrs};
use std::time::Duration;

const MAX_BYTES: usize = 6 * 1024 * 1024; // 6 MB
const MAX_PARAGRAPHS: usize = 800;
/// Số bước chuyển hướng (redirect) tối đa — REVA tự theo TỪNG BƯỚC và kiểm tra
/// an toàn địa chỉ trước khi đi tiếp (chống redirect về địa chỉ nội bộ).
const MAX_REDIRECTS: usize = 5;
const MIN_PARA_LEN: usize = 30; // bỏ đoạn quá ngắn (menu, nút…)
const MIN_ITEM_LEN: usize = 12; // mục danh sách được ngắn hơn đoạn văn
/// li mà phần văn bản NGOÀI thẻ <a> ngắn hơn ngưỡng này ⇒ li "chỉ toàn link" (link-embed,
/// nút chia sẻ, thẻ bài liên quan) — không phải nội dung bài.
const LI_MIN_NON_ANCHOR_TEXT: usize = 8;
/// li có <img>/<svg> mà toàn bộ văn bản ngắn hơn ngưỡng này ⇒ icon/card giao diện.
const LI_ICON_CARD_MAX_TEXT: usize = 50;
/// <p> trong <figure>/<figcaption> ngắn hơn ngưỡng này ⇒ caption/credit ảnh, bỏ.
const CAPTION_MAX_LEN: usize = 100;
/// "Link farm": từ ngưỡng link + mật độ ký tự nằm trong <a> trở lên ⇒ menu/danh sách liên kết.
const LINKFARM_MIN_LINKS: usize = 5;
const LINKFARM_MIN_RATIO: f64 = 0.55;
/// Dạng cực đoan: gần như TOÀN link (teaser "Đọc thêm", khối bài liên quan dày).
const LINKFARM_STRICT_LINKS: usize = 2;
const LINKFARM_STRICT_RATIO: f64 = 0.80;

/// Từ khoá "giao diện trang" trong class/id của thẻ HTML (thường hoá ascii, so chuỗi con
/// trên chuỗi đã bỏ mọi ký tự không phải chữ/số — nên "menu-nav", "sub_all_menu_main",
/// "top-bar", "popup-megamenu", "detail__newslette"… đều trúng).
/// KỸ (rút từ thực tế các báo lớn):
/// * KHÔNG dùng "form" — đánh trúng "format-standard" của WordPress;
/// * KHÔNG dùng "header" — bài viết thường nằm trong article-header;
/// * KHÔNG dùng "sidebar" — vnexpress.net đặt CẢ BÀI VIẾT trong div.sidebar-1
///   (tên cột bố cục, không phải thanh bên!) — giết nhầm toàn bộ bài;
/// * KHÔNG dùng "banner" — hero-banner có thể chứa đoạn mở đầu bài.
/// Ngoài ra có LƯỚI AN TOÀN: nếu lọc chặt mà trăng tay (0 đoạn) thì tự chạy lại
/// ở chế độ lỏng (bỏ lọc class/id tổ tiên) — xem `extract`.
const CHROME_ATTR_KEYWORDS: &[&str] = &[
    "menu", "nav", "mega", "ribbon", "breadcrumb", "modal", "popup", "comment", "share",
    "social", "topbar", "newslette", "login", "signin", "signup", "register",
    "related", "sitemap", "toolbar", "subscribe",
];

pub struct FetchedPage {
    pub url: String,
    pub title: String,
    pub paragraphs: Vec<String>,
    pub bytes: usize,
}

/// Địa chỉ IP có thuộc dải "nội bộ/không nên tự ý tải" không:
/// loopback, private (10/8, 172.16/12, 192.168/16), link-local, unique-local IPv6,
/// multicast, broadcast, unspecified, 0.0.0.0/8, CGNAT 100.64/10, dải tài liệu.
/// [AUDIT 2026-10-18] chống SSRF: chỉ tải trang công khai.
fn ip_is_internal(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()
                || o[0] == 0 // 0.0.0.0/8 "this network"
                || (o[0] == 100 && (o[1] & 0xc0) == 64) // 100.64.0.0/10 CGNAT
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                || (s[0] & 0xfe00) == 0xfc00 // fc00::/7 unique-local
                || (s[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
        }
    }
}

/// Tách (host, port) từ URL http/https — hỗ trợ IPv6 trong ngoặc vuông, bỏ userinfo.
/// KHÔNG tải gì cả — chỉ để kiểm tra an toàn trước khi gọi mạng.
fn url_host_port(url: &str) -> Result<(String, u16), String> {
    let (scheme, rest) = if let Some(r) = url.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = url.strip_prefix("http://") {
        ("http", r)
    } else {
        return Err("URL phải bắt đầu bằng http:// hoặc https://".into());
    };
    // cắt phần path/query/fragment
    let authority_end = rest
        .find(['/', '?', '#'])
        .unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    // bỏ userinfo "user:pass@host" (hiếm nhưng hợp lệ theo RFC)
    let hostport = match authority.rfind('@') {
        Some(p) => &authority[p + 1..],
        None => authority,
    };
    let default_port = if scheme == "https" { 443 } else { 80 };
    let (host, port) = if let Some(stripped) = hostport.strip_prefix('[') {
        // IPv6 [::1]:8080
        match stripped.find(']') {
            Some(e) => {
                let h = &stripped[..e];
                let after = &stripped[e + 1..];
                let p = match after.strip_prefix(':') {
                    Some(pstr) => pstr
                        .parse::<u16>()
                        .map_err(|_| "Cổng trong URL không hợp lệ.".to_string())?,
                    None => default_port,
                };
                (h.to_string(), p)
            }
            None => return Err("URL IPv6 thiếu dấu ']'.".into()),
        }
    } else {
        match hostport.rfind(':') {
            Some(pos) => {
                let pstr = &hostport[pos + 1..];
                if pstr.is_empty() {
                    (hostport[..pos].to_string(), default_port)
                } else {
                    let port = pstr
                        .parse::<u16>()
                        .map_err(|_| "Cổng trong URL không hợp lệ.".to_string())?;
                    (hostport[..pos].to_string(), port)
                }
            }
            None => (hostport.to_string(), default_port),
        }
    };
    if host.is_empty() {
        return Err("URL thiếu tên máy chủ.".into());
    }
    Ok((host, port))
}

/// Kiểm tra URL có trỏ tới máy chủ CÔNG KHAI không (chống SSRF):
/// host là IP literal nội bộ → chặn; host là tên miền → phân giải DNS và chặn
/// nếu CÓ ÍT NHẤT MỘT địa chỉ trả về là địa chỉ nội bộ.
fn validate_public_url(url: &str) -> Result<(), String> {
    let (host, port) = url_host_port(url)?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        if ip_is_internal(ip) {
            return Err(format!(
                "Không tải từ địa chỉ nội bộ {} — REVA chỉ tải trang web công khai.",
                ip
            ));
        }
        return Ok(());
    }
    let addrs: Vec<std::net::SocketAddr> = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("Không phân giải được tên máy chủ {}: {}", host, e))?
        .collect();
    if addrs.is_empty() {
        return Err(format!("Không tìm thấy địa chỉ cho máy chủ {}.", host));
    }
    for a in &addrs {
        if ip_is_internal(a.ip()) {
            return Err(format!(
                "Máy chủ {} trỏ về địa chỉ nội bộ {} — không tải.",
                host, a.ip()
            ));
        }
    }
    Ok(())
}

/// Nối Location của redirect với URL hiện tại: hỗ trợ tuyệt đối (http/https),
/// "//host/…" (scheme-relative), "/đường-dẫn" (root-relative) và tương đối
/// (kèm làm sạch "."/".." của segment).
fn resolve_redirect(base: &str, location: &str) -> Option<String> {
    let loc = location.trim();
    if loc.is_empty() {
        return None;
    }
    if loc.starts_with("http://") || loc.starts_with("https://") {
        return Some(loc.to_string());
    }
    if loc.starts_with("//") {
        let scheme = if base.starts_with("http://") { "http" } else { "https" };
        return Some(format!("{}:{}", scheme, loc));
    }
    // origin = scheme://authority
    let after = base.find("://")? + 3;
    let authority_end = base[after..]
        .find(['/', '?', '#'])
        .map(|p| after + p)
        .unwrap_or(base.len());
    let origin = &base[..authority_end];
    if loc.starts_with('/') {
        return Some(format!("{}{}", origin, loc));
    }
    // tương đối: nối vào "thư mục" của path hiện tại rồi làm sạch . / ..
    let path_start = authority_end;
    let dir_end = base[path_start..]
        .rfind('/')
        .map(|p| path_start + p + 1)
        .unwrap_or(path_start);
    let joined = format!("{}{}", &base[..dir_end], loc);
    // làm sạch segment "." và ".."
    let (j_after, j_rest) = {
        let a = joined.find("://")? + 3;
        let end = joined[a..]
            .find(['/', '?', '#'])
            .map(|p| a + p)
            .unwrap_or(joined.len());
        (end, &joined[end..])
    };
    let mut segs: Vec<&str> = Vec::new();
    for s in j_rest.split('/') {
        match s {
            "." => {}
            ".." => {
                segs.pop();
            }
            _ => segs.push(s),
        }
    }
    Some(format!("{}{}", &joined[..j_after], segs.join("/")))
}

/// Cho selftest bài 39 — truy cập hàm nội bộ (phân giải redirect, tách host:port).
pub fn resolve_redirect_for_test(base: &str, location: &str) -> Option<String> {
    resolve_redirect(base, location)
}

/// Cho selftest bài 39.
pub fn url_host_port_for_test(url: &str) -> Result<(String, u16), String> {
    url_host_port(url)
}

pub fn fetch(url: &str) -> Result<FetchedPage, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(25))
        .redirects(0) // tự theo redirect để KIỂM TRA AN TOÀN từng bước đi tới
        .build();
    let mut current = url.to_string();
    let mut hops = 0usize;
    let resp = loop {
        validate_public_url(&current)?;
        let r = agent
            .get(&current)
            .set(
                "User-Agent",
                "Mozilla/5.0 (compatible; reva-tool/2.7; +local-aligner)",
            )
            .set("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
            .set("Accept-Language", "en,vi;q=0.9")
            .call()
            .map_err(|e| format!("Không tải được trang: {}", e))?;
        let status = r.status();
        if (300..400).contains(&status) {
            let loc = r.header("location").map(|s| s.to_string());
            match loc {
                Some(loc) => {
                    if loc.contains("://")
                        && !loc.starts_with("http://")
                        && !loc.starts_with("https://")
                    {
                        return Err(format!(
                            "Trang chuyển hướng tới giao thức không hỗ trợ: {}",
                            loc
                        ));
                    }
                    if hops >= MAX_REDIRECTS {
                        return Err("Quá nhiều bước chuyển hướng (redirect) — từ bỏ.".into());
                    }
                    hops += 1;
                    current = resolve_redirect(&current, &loc)
                        .ok_or_else(|| "Đường dẫn chuyển hướng không đọc được.".to_string())?;
                    continue;
                }
                None => {
                    return Err(format!(
                        "Trang trả về mã chuyển hướng {} mà không có địa chỉ mới.",
                        status
                    ));
                }
            }
        }
        break r;
    };

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
        url: current,
        title,
        paragraphs,
        bytes: raw.len(),
    })
}

// ========================= Trích HTML (thủ công, không phụ thuộc ngoài) =========================

/// Trả về (title, các đoạn <p> + mục <li> đã làm sạch — theo đúng thứ tự tài liệu).
///
/// BẢN 2.6: thêm 3 tầng lọc — chrome theo class/id tổ tiên, mật độ link,
/// caption/credit ảnh (xem đầu file). Bản 2.3: máy quét thẻ có stack — vừa lấy
/// <p> vừa lấy <li> của <ul>/<ol>.
///
/// LƯỚI AN TOÀN 2 LƯỢT: lượt 1 (chặt) áp đủ mọi tầng lọc; nếu kết quả TRẮNG TAY
/// mà trang rõ ràng có nội dung, lượt 2 (lỏng) bỏ duy nhất tầng lọc class/id tổ tiên
/// (tầng dễ gặp cấu trúc lạ nhất — vd vnexpress.net đặt cả bài trong div.sidebar-1)
/// nhưng vẫn giữ nav/aside/footer + boilerplate + link-farm + caption. Thanks đó,
/// một trang "đặt tên class lạ" tối đa cho lẫn vài khối chrom — không bao giờ lỗi
/// "không tìm thấy đoạn" như khi bỏ lưới an toàn.
pub fn extract(html: &str) -> (String, Vec<String>) {
    let (title, strict) = extract_pass(html, true);
    if !strict.is_empty() {
        return (title, strict);
    }
    let (title, relaxed) = extract_pass(html, false);
    (title, relaxed)
}

/// Một lượt trích. `strict = true` áp đủ 3 tầng lọc chrom; `strict = false` bỏ
/// lọc theo class/id tổ tiên (lưới an toàn cho cấu trúc đặt tên lạ).
fn extract_pass(html: &str, strict: bool) -> (String, Vec<String>) {
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

    // ---- quét block p/li theo thứ tự tài liệu (bản 2.3 + lọc 2.6) ----
    let mut paras: Vec<String> = Vec::new();
    for block in collect_blocks(&lower, strict) {
        let inner = &html[block.inner_start..block.inner_end];
        let text = collapse(&decode_entities(&strip_tags(inner)));
        if is_boilerplate(&text) {
            continue; // chrom giao diện trang (timestamp, nút chia sẻ, newsletter…) — không phải nội dung bài
        }
        // ---- TẦNG 3 (2.6): caption/credit ảnh ----
        if block.in_caption
            && (text.chars().count() < CAPTION_MAX_LEN || is_photo_credit(&text))
        {
            continue;
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
        // ---- TẦNG 2 (2.6): "link farm" — menu/danh sách bài liên quan ----
        // Áp cho CẢ p lẫn li: đoạn văn thật gần như không bao giờ có ≥5 link chiếm
        // ≥55% ký tự; menu thì link là gần như toàn bộ.
        let total = text.chars().count();
        let (links, anchor_chars) = link_stats(inner);
        if total > 0 {
            let ratio = anchor_chars as f64 / total as f64;
            if (links >= LINKFARM_MIN_LINKS && ratio >= LINKFARM_MIN_RATIO)
                || (links >= LINKFARM_STRICT_LINKS && ratio >= LINKFARM_STRICT_RATIO)
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

// ========================= Máy quét thẻ (bản 2.3 + attr 2.6) =========================

/// Thẻ "void" không bao giờ có thẻ đóng (HTML5) — không đưa vào stack.
fn is_void_tag(name: &str) -> bool {
    matches!(
        name,
        "area" | "base" | "br" | "col" | "embed" | "hr" | "img" | "input" | "link" | "meta"
            | "param" | "source" | "track" | "wbr"
    )
}

/// Container "chrom giao diện" theo TÊN THẺ — mọi block p/li bên trong bị bỏ khi tải URL.
fn is_chrome_container(name: &str) -> bool {
    matches!(name, "nav" | "aside" | "footer")
}

/// Container "chú thích ảnh" — <p>/<li> bên trong là caption/credit.
fn is_caption_container(name: &str) -> bool {
    matches!(name, "figure" | "figcaption")
}

/// Chuẩn hoá để so khớp từ khoá chrome: bỏ mọi ký tự không phải chữ/số
/// ("menu-nav"→"menunav", "top-bar"→"topbar", "detail__newslette"→"detailnewslette").
fn norm_attr(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect()
}

/// class/id của thẻ có phải "giao diện trang" (menu, modal, comment…) không.
fn has_chrome_attr(class_id: &str) -> bool {
    if class_id.is_empty() {
        return false;
    }
    let normed = norm_attr(class_id);
    CHROME_ATTR_KEYWORDS.iter().any(|kw| normed.contains(kw))
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
    /// giá trị class + id của thẻ (đã thường hoá ascii, nối bằng dấu cách) — bản 2.6
    class_id: String,
}

/// Trích giá trị thuộc tính class/id từ phần chữ của thẻ `<div class="x" id='y'>`.
/// Chấp nhận nháy kép/nhơn/không nháy; từ khoá phải đứng đầu tên thuộc tính
/// (để "data-id=" không bị nhầm là "id=").
fn attr_class_id(tag_text: &str) -> String {
    let mut out = String::new();
    for key in ["class", "id"] {
        let pat = format!("{}=", key);
        let mut pos = 0usize;
        while let Some(rel) = tag_text[pos..].find(&pat) {
            let at = pos + rel;
            // ký tự TRƯỚC phải là ranh giới tên thuộc tính (không phải chữ/số/-/_)
            let before_ok = tag_text[..at]
                .chars()
                .last()
                .map_or(true, |c| !(c.is_alphanumeric() || c == '-' || c == '_' || c == ':'));
            if !before_ok {
                pos = at + pat.len();
                continue;
            }
            let rest = &tag_text[at + pat.len()..];
            let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'');
            let (val, adv) = match quote {
                Some(q) => match rest[1..].find(q) {
                    Some(endrel) => (&rest[1..1 + endrel], endrel + 2),
                    None => break, // hỏng nháy — bỏ tên này
                },
                None => {
                    let endrel = rest
                        .find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
                        .unwrap_or(rest.len());
                    (&rest[..endrel], endrel)
                }
            };
            if !val.is_empty() {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(val);
            }
            pos = at + pat.len() + adv;
        }
    }
    out
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
        // bản 2.6: lấy class/id của thẻ (chỉ thẻ MỞ cần; thẻ đóng bỏ qua cho nhanh)
        let class_id = if is_close {
            String::new()
        } else {
            attr_class_id(&lower[i + 1..gt])
        };
        tags.push(Tag {
            start: i,
            end: gt + 1,
            name,
            is_close,
            is_self_close,
            class_id,
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
    /// nằm trong <figure>/<figcaption> (bản 2.6 — quy tắc caption)
    in_caption: bool,
}

/// Duyệt stack thẻ, thu các block p/li THEO THỨ TỰ TÀI LIỆU, áp quy tắc lọc chrom:
/// * bỏ mọi block nằm trong <nav>/<aside>/<footer> (tên thẻ) — bản 2.3;
/// * bỏ mọi block có TỔ TIÊN hoặc CHÍNH NÓ mang class/id chrome (menu, modal,
///   comment, related…) — bản 2.6 (tầng 1);
/// * <p> chỉ nhận khi không nằm trong <li> (tránh trùng lặp <li><p>…</p></li>);
/// * <li> chỉ nhận khi nằm trong <ul>/<ol>, không lồng trong <li> khác
///   (mục lồng cấp 2 như cũ vẫn bị bỏ — hiếm và thường là giao diện).
/// Chịu HTML không khớp hoàn hảo: thẻ đóng khớp với thẻ mở GẦN NHẤT cùng tên
/// (thẻ con không đóng sẽ được đóng ngầm — đúng cách trình duyệt xử lý).
fn collect_blocks(lower: &str, strict: bool) -> Vec<Block> {
    let tags = scan_tags(lower);
    // stack: (tên thẻ, capture_start nếu là p/li ĐƯỢC nhận, class/id chrome?,
    //         nằm trong figure/figcaption? — ghi lúc MỞ thẻ)
    let mut stack: Vec<(String, Option<usize>, bool, bool)> = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    for tag in tags {
        if tag.is_close {
            if let Some(k) = stack.iter().rposition(|(nm, _, _, _)| *nm == tag.name) {
                for (nm, cap, _, in_cap) in stack.split_off(k) {
                    if let Some(inner_start) = cap {
                        blocks.push(Block {
                            kind: if nm == "p" { "p" } else { "li" },
                            inner_start,
                            inner_end: tag.start,
                            in_caption: in_cap,
                        });
                    }
                }
            }
            continue;
        }
        if is_void_tag(&tag.name) || tag.is_self_close {
            continue;
        }
        let in_li = stack.iter().any(|(nm, _, _, _)| *nm == "li");
        let in_p = stack.iter().any(|(nm, _, _, _)| *nm == "p");
        let in_chrome = stack.iter().any(|(nm, _, _, _)| is_chrome_container(nm));
        let in_caption = stack.iter().any(|(nm, _, _, _)| is_caption_container(nm));
        let in_list = stack.iter().any(|(nm, _, _, _)| *nm == "ul" || *nm == "ol");
        let own_chrome = strict && has_chrome_attr(&tag.class_id);
        let in_attr_chrome = strict && stack.iter().any(|(_, _, c, _)| *c);
        let cap = if tag.name == "p" && !in_li && !in_p && !in_chrome && !in_attr_chrome && !own_chrome
        {
            Some(tag.end)
        } else if tag.name == "li"
            && in_list
            && !in_li
            && !in_p
            && !in_chrome
            && !in_attr_chrome
            && !own_chrome
        {
            Some(tag.end)
        } else {
            None
        };
        stack.push((tag.name, cap, own_chrome, in_caption));
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

/// Thống kê link của một đoạn HTML: (số thẻ <a> mở ở tầng ngoài, tổng ký tự văn bản
/// nằm TRONG link). Bản 2.6 — dùng cho quy tắc "link farm".
fn link_stats(inner: &str) -> (usize, usize) {
    let lower = inner.to_ascii_lowercase();
    let tags = scan_tags(&lower);
    let mut links = 0usize;
    let mut anchor_chars = 0usize;
    let mut depth = 0usize;
    let mut seg_start: Option<usize> = None;
    for tag in tags {
        if tag.name != "a" {
            continue;
        }
        if tag.is_close {
            if depth > 0 {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = seg_start.take() {
                        if tag.start > s {
                            let frag = &inner[s..tag.start];
                            anchor_chars +=
                                collapse(&decode_entities(&strip_tags(frag))).chars().count();
                        }
                    }
                }
            }
            continue;
        }
        if depth == 0 {
            seg_start = Some(tag.end);
            links += 1;
        }
        depth += 1;
    }
    (links, anchor_chars)
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

/// Dòng credit/cung cấp ảnh — "Ảnh: C.DŨNG", "Nguồn: Reuters", "Photo: AFP"…
fn is_photo_credit(text: &str) -> bool {
    let t = text.to_lowercase();
    for marker in ["ảnh:", "photo:", "credit:", "nguồn:", "source:", "foto:"] {
        if t.contains(marker) {
            return true;
        }
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
