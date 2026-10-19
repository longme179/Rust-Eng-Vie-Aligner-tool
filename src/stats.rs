//! stats.rs — BẢN 2.9 — thống kê số từ / ký tự / đoạn của một phiên.
//!
//! Mục đích sản phẩm (xem `docs/PLAN-QOL-2026-10-19.md` §1.2): tính phí dịch thuật theo
//! **số từ nguồn** là chuẩn ngành (biên dịch tự do + công ty dịch đều báo giá theo từ Anh),
//! nhưng trước giờ người dùng REVA phải mở Microsoft Word / công cụ đếm từ online để đếm —
//! trong khi REVA đã nắm toàn bộ văn bản. Module này đếm ngay trong máy, 100% cục bộ.
//!
//! Quy ước đếm (công khai trong tài liệu + ghi chú trong hộp thoại UI):
//!   * TỪ      = chuỗi con không khoảng trắng khi tách theo mọi khoảng trắng Unicode
//!     (kể cả NBSP). Chuỗi emoji ghép ZWJ tính 1 từ.
//!   * KÝ TỰ   = số scalar value Unicode SAU KHI chuẩn hoá NFC, bỏ khoảng trắng đầu/cuối
//!     đoạn. Chuẩn hoá NFC dùng crate `unicode-normalization` ĐÃ CÓ SẴN cho bộ
//!     fold (fold.rs) — không thêm phụ thuộc mới. Nhờ NFC, cùng một văn bản
//!     dán dạng NFD (macOS) hay NFC (Windows) cho CÙNG số từ lẫn số ký tự —
//!     không tranh cãi khi tính tiền theo ký tự.
//!   * ĐOẠN    = số đoạn văn mỗi phía của tài liệu GỐC (`session.english.len()`).
//!
//! Thuần ĐỌC: không đụng hàng, không snapshot hoàn tác, không gọi mạng, không LaBSE —
//! tự động hermetic trên mọi máy (bài học 39/41 của bản 2.8.0).

use crate::session::{Counts, Session};
use serde::Serialize;
use unicode_normalization::UnicodeNormalization;

/// Đếm số từ của một chuỗi (tách theo khoảng trắng Unicode, bỏ chuỗi rỗng).
pub fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Đếm số ký tự (scalar value sau NFC, bỏ khoảng trắng đầu/cuối).
/// NFC = dạng tổ hợp sẵn có ("ế" 1 scalar); văn bản NFD ("e" + 2 dấu tổ hợp)
/// cho cùng số ký tự — quy ước công khai để hai máy đếm ra cùng con số.
pub fn count_chars(text: &str) -> usize {
    text.trim().nfc().count()
}

/// Tổng của một phía ngôn ngữ.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SideTotals {
    pub words: usize,
    pub chars: usize,
    pub segments: usize,
}

fn side_totals(texts: &[String]) -> SideTotals {
    let mut t = SideTotals {
        segments: texts.len(),
        ..Default::default()
    };
    for s in texts {
        t.words += count_words(s);
        t.chars += count_chars(s);
    }
    t
}

/// Thống kê các cặp đã ghép đủ hai phía.
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairedTotals {
    pub rows: usize,
    pub en_words: usize,
    pub vi_words: usize,
}

/// Thống kê các hàng MỘT phía (chưa ghép / xác nhận không có bản dịch).
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OneSideTotals {
    pub rows: usize,
    pub words: usize,
}

/// Báo cáo đầy đủ của một phiên — cấu trúc trả về bởi `GET /api/sessions/{id}/stats`.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsReport {
    pub rows: usize,
    /// Toàn bộ tài liệu gốc tiếng Anh (căn cứ tính giá theo từ nguồn).
    pub en: SideTotals,
    /// Toàn bộ tài liệu gốc tiếng Việt.
    pub vi: SideTotals,
    /// Phân bố trạng thái hàng (đúng 4 giá trị chuẩn).
    pub by_status: Counts,
    /// Các hàng đủ hai phía (đã ghép).
    pub paired: PairedTotals,
    /// Hàng chỉ có tiếng Anh (chưa có bản dịch đi kèm).
    pub en_only: OneSideTotals,
    /// Hàng chỉ có tiếng Việt (bản dịch thừa / thiếu bản gốc).
    pub vi_only: OneSideTotals,
}

/// Tính báo cáo thống kê cho phiên. Thuần đọc — không thay đổi gì trong phiên.
pub fn report(s: &Session) -> StatsReport {
    let mut paired = PairedTotals::default();
    let mut en_only = OneSideTotals::default();
    let mut vi_only = OneSideTotals::default();
    for r in &s.rows {
        let has_en = r.has_en();
        let has_vi = r.has_vi();
        if has_en && has_vi {
            paired.rows += 1;
            for (_, t) in r.en_segments() {
                paired.en_words += count_words(t);
            }
            for (_, t) in r.vi_segments() {
                paired.vi_words += count_words(t);
            }
        } else if has_en {
            en_only.rows += 1;
            for (_, t) in r.en_segments() {
                en_only.words += count_words(t);
            }
        } else if has_vi {
            vi_only.rows += 1;
            for (_, t) in r.vi_segments() {
                vi_only.words += count_words(t);
            }
        }
    }
    StatsReport {
        rows: s.rows.len(),
        en: side_totals(&s.english),
        vi: side_totals(&s.vietnamese),
        by_status: s.counts(),
        paired,
        en_only,
        vi_only,
    }
}
