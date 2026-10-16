//! fold.rs — "ASCII fold": bỏ dấu tiếng Việt để so khớp không phân biệt dấu.
//! "Hà Nội" -> "ha noi", "Đà Nẵng" -> "da nang", "trường" -> "truong".

use unicode_normalization::UnicodeNormalization;

/// Bỏ dấu một chuỗi (giữ chữ cái/số, lowercase).
/// Dùng NFKD rồi lọc bỏ các combining mark (U+0300..U+036F), và map riêng đ/Đ -> d.
pub fn fold_lower(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.nfkd() {
        let cp = c as u32;
        if (0x0300..=0x036F).contains(&cp) {
            continue; // combining diacritics
        }
        for lc in c.to_lowercase() {
            if lc == 'đ' {
                out.push('d');
            } else if lc == 'Đ' {
                // (đã lowercase nên không gặp, nhưng phòng hờ)
                out.push('d');
            } else {
                out.push(lc);
            }
        }
    }
    out
}

/// Chuẩn hoá token số: bỏ dấu phân cách hàng nghìn/thập phân.
/// "4,000" / "4.000" -> "4000"; "1,5" -> "15"? — KHÔNG: ta chỉ gộp khi CÙNG là nhóm số.
/// Quy ước: nếu token toàn chữ số và có dấu , hoặc . bên trong thì bỏ hết dấu -> "4000".
/// Đây là heuristic hợp lệ cho bài báo (số nguyên lớn); số thập phân hiếm khi quyết định ghép.
pub fn fold_number_token(tok: &str) -> Option<String> {
    let bytes = tok.as_bytes();
    if bytes.is_empty() {
        return None;
    }
    let mut all_digits = true;
    let mut has_sep = false;
    let mut digit_run = 0usize;
    for &b in bytes {
        if b.is_ascii_digit() {
            digit_run += 1;
        } else if b == b',' || b == b'.' {
            if digit_run == 0 {
                return None; // ".5" hay "5." lệch dạng
            }
            has_sep = true;
        } else {
            all_digits = false;
            break;
        }
    }
    if !all_digits || digit_run == 0 {
        return None;
    }
    if has_sep {
        Some(tok.chars().filter(|c| c.is_ascii_digit()).collect())
    } else {
        Some(tok.to_string())
    }
}

#[cfg(test)]
mod tests_fold {
    use super::*;

    #[test]
    fn test_fold_basic() {
        assert_eq!(fold_lower("Hà Nội"), "ha noi");
        assert_eq!(fold_lower("Đà Nẵng"), "da nang");
        assert_eq!(fold_lower("trường học"), "truong hoc");
        assert_eq!(fold_lower("PHẠM Minh Chính"), "pham minh chinh");
        assert_eq!(fold_lower("Hello World"), "hello world");
    }

    #[test]
    fn test_number() {
        assert_eq!(fold_number_token("4,000").unwrap(), "4000");
        assert_eq!(fold_number_token("4.000").unwrap(), "4000");
        assert_eq!(fold_number_token("2025").unwrap(), "2025");
        assert_eq!(fold_number_token("abc"), None);
        assert_eq!(fold_number_token(""), None);
    }
}
