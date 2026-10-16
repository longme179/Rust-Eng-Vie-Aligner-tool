//! session.rs — quản lý phiên ghép (tạo / mở / cập nhật / NHẢ PHIÊN).
//!
//! SỬA LỖI "chạy bài thứ hai bị 'Không ghép được.'" của bản 1.x:
//!   * Mỗi lần ghép là MỘT PHIÊN MỚI hoàn toàn độc lập — không tái dùng trạng thái cũ,
//!     không giữ biến toàn cục của bộ ghép.
//!   * Bộ ghép (aligner) thuần vô trạng thái; mọi dữ liệu nằm trong Session.

use crate::aligner::{AlignOptions, Row, Seg, Status, ALL_STATUSES};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(1);

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn new_id() -> String {
    let t = now_ms();
    let c = COUNTER.fetch_add(1, Ordering::SeqCst);
    format!("p{:x}-{:04}", t, c)
}

/// Giá trị mặc định cho `Session::engine` — các phiên cũ (bản ≤2.1) không có trường này
/// nên serde cần default để nạp được từ đĩa.
fn default_engine() -> String {
    "lexical".to_string()
}

// ========================= Session =========================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Counts {
    pub total: usize,
    pub auto_matched: usize,
    pub needs_review: usize,
    pub unmatched: usize,
    pub human_confirmed: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub english: Vec<String>,
    pub vietnamese: Vec<String>,
    pub rows: Vec<Row>,
    pub options: AlignOptions,
    /// Động cơ đã ghép phiên này: "lexical" hoặc "lexical+labse" (bản 2.2).
    #[serde(default = "default_engine")]
    pub engine: String,
}

impl Session {
    pub fn from_align(
        id: String,
        title: String,
        english: Vec<String>,
        vietnamese: Vec<String>,
        rows: Vec<Row>,
        options: AlignOptions,
    ) -> Session {
        let now = now_ms();
        Session {
            id,
            title,
            created_at: now,
            updated_at: now,
            english,
            vietnamese,
            rows,
            options,
            engine: default_engine(), // người gọi (handler/selftest) ghi đè theo AlignOutput.engine
        }
    }

    pub fn counts(&self) -> Counts {
        let mut c = Counts {
            total: self.rows.len(),
            auto_matched: 0,
            needs_review: 0,
            unmatched: 0,
            human_confirmed: 0,
        };
        for r in &self.rows {
            match r.status {
                Status::AutoMatched => c.auto_matched += 1,
                Status::NeedsReview => c.needs_review += 1,
                Status::Unmatched => c.unmatched += 1,
                Status::HumanConfirmed => c.human_confirmed += 1,
            }
        }
        c
    }

    /// Đặt trạng thái cho một hàng.
    /// * Với hàng ĐỦ hai bên: cho phép auto_matched / needs_review / human_confirmed.
    ///   Chọn `unmatched` sẽ TÁCH hàng thành các hàng đơn phía (bỏ ghép).
    /// * Với hàng MỘT phía: cho phép human_confirmed (xác nhận "không có bản dịch")
    ///   và unmatched (trở lại).
    /// * Hàng GỘP (n EN · m VI): tách trả về đúng n + m hàng đơn phía, giữ thứ tự gốc
    ///   (bản 2.4 — cặp đơn giản vẫn ra 2 hàng như bản cũ).
    pub fn set_row_status(&mut self, row_index: usize, status: Status) -> Result<(), String> {
        if row_index >= self.rows.len() {
            return Err(format!("Hàng {} không tồn tại (có {} hàng).", row_index, self.rows.len()));
        }
        if !ALL_STATUSES.contains(&status) {
            return Err("Trạng thái không hợp lệ.".into());
        }
        let row = self.rows[row_index].clone();
        let has_en = row.has_en();
        let has_vi = row.has_vi();

        if status == Status::Unmatched {
            if has_en && has_vi {
                // TÁCH hoàn toàn: mỗi đoạn một hàng đơn phía, chèn tại chỗ, giữ thứ tự
                let mut pieces: Vec<Row> = Vec::new();
                for (i, t) in row.en_segments() {
                    pieces.push(Row::single_side(Some((i, t.to_string())), None, Status::Unmatched));
                }
                for (i, t) in row.vi_segments() {
                    pieces.push(Row::single_side(None, Some((i, t.to_string())), Status::Unmatched));
                }
                self.rows.remove(row_index);
                for (k, p) in pieces.into_iter().enumerate() {
                    self.rows.insert(row_index + k, p);
                }
                self.reindex();
            } else {
                // đã một phía — chỉ đổi trạng thái
                self.rows[row_index].status = Status::Unmatched;
            }
        } else {
            if !has_en || !has_vi {
                if status == Status::HumanConfirmed {
                    self.rows[row_index].status = Status::HumanConfirmed;
                } else {
                    return Err(
                        "Hàng chỉ có một phía — chỉ hỗ trợ 'Đã xác nhận' (human_confirmed) hoặc 'Không khớp' (unmatched)."
                            .into(),
                    );
                }
            } else {
                self.rows[row_index].status = status;
            }
        }
        self.updated_at = now_ms();
        Ok(())
    }

    /// GỘP hàng kế tiếp (row_index + 1) VÀO hàng `row_index` — nền tảng cho
    /// thao tác kéo–thả "đem các hàng VI lên chung hàng EN" (và ngược lại). (Bản 2.4)
    ///
    /// Nguyên tắc an toàn:
    /// * Chỉ cho phép gộp hàng LIỀN KỀ → tính đơn điệu (I3) luôn được bảo toàn,
    ///   không bao giờ tạo giao chéo giữa các hàng còn lại.
    /// * Mọi đoạn của hàng dưới được chuyển sang hàng trên: nếu hàng trên còn
    ///   trống phía đó thì thành đoạn chính, ngược lại vào `en_extra`/`vi_extra`
    ///   (luôn giữ chỉ số đoạn gốc để kiểm tra "mỗi đoạn dùng đúng 1 lần").
    /// * Trạng thái sau gộp: nếu chỉ định thì dùng (trừ `unmatched` — muốn tách
    ///   thì PATCH sau); nếu không chỉ định → `needs_review` (an toàn: người
    ///   dùng tự bấm Khớp / Cần soát sau khi gộp).
    /// * Điểm tương đồng cũ không còn đúng ý nghĩa sau gộp → đặt lại `None`.
    pub fn merge_next(&mut self, row_index: usize, status: Option<Status>) -> Result<(), String> {
        if row_index >= self.rows.len() {
            return Err(format!("Hàng {} không tồn tại (có {} hàng).", row_index, self.rows.len()));
        }
        if row_index + 1 >= self.rows.len() {
            return Err("Đây đã là hàng cuối — không còn hàng phía dưới để gộp.".into());
        }
        if let Some(st) = status {
            if !ALL_STATUSES.contains(&st) {
                return Err("Trạng thái không hợp lệ.".into());
            }
            if st == Status::Unmatched {
                return Err(
                    "Không đặt 'unmatched' khi gộp — nếu muốn tách, bấm nút \"Không khớp\" sau khi gộp.".into(),
                );
            }
        }

        let src = self.rows.remove(row_index + 1);
        let dst = &mut self.rows[row_index];

        // ---- phía Anh: đoạn chính của hàng dưới
        if let (Some(i), Some(t)) = (src.en_index, src.en) {
            if dst.en.is_none() && dst.en_extra.is_empty() {
                dst.en_index = Some(i);
                dst.en = Some(t);
            } else {
                dst.en_extra.push(Seg::new(i, t));
            }
        }
        // ---- phía Việt: đoạn chính của hàng dưới
        if let (Some(i), Some(t)) = (src.vi_index, src.vi) {
            if dst.vi.is_none() && dst.vi_extra.is_empty() {
                dst.vi_index = Some(i);
                dst.vi = Some(t);
            } else {
                dst.vi_extra.push(Seg::new(i, t));
            }
        }
        // ---- các đoạn đã gộp trước đó của hàng dưới (nếu hàng dưới vốn là hàng gộp)
        for s in src.en_extra {
            dst.en_extra.push(s);
        }
        for s in src.vi_extra {
            dst.vi_extra.push(s);
        }

        // ---- trạng thái sau gộp
        if dst.has_en() && dst.has_vi() {
            dst.status = status.unwrap_or(Status::NeedsReview);
        } else {
            // gộp xong vẫn một phía (gộp 2 hàng cùng phía) → chỉ 2 trạng thái hợp lệ
            dst.status = match status {
                Some(Status::HumanConfirmed) => Status::HumanConfirmed,
                _ => Status::Unmatched,
            };
        }
        dst.score = None;
        self.reindex();
        self.updated_at = now_ms();
        Ok(())
    }

    fn reindex(&mut self) {
        for (i, r) in self.rows.iter_mut().enumerate() {
            r.index = i;
        }
    }
}

/// Tự đặt tiêu đề phiên từ nội dung.
pub fn derive_title(english: &[String], vietnamese: &[String]) -> String {
    let src = english
        .first()
        .or_else(|| vietnamese.first())
        .map(|s| s.as_str())
        .unwrap_or("");
    let words: Vec<&str> = src.split_whitespace().take(9).collect();
    if words.is_empty() {
        return "Phiên không tên".into();
    }
    let mut t = words.join(" ");
    if words.len() == 9 {
        t.push('…');
    }
    if t.chars().count() > 90 {
        t = t.chars().take(90).collect();
        t.push('…');
    }
    t
}

// ========================= Store =========================

pub struct SessionStore {
    dir: PathBuf,
    sessions: Mutex<HashMap<String, Session>>,
}

impl SessionStore {
    /// Khởi tạo store; nạp các phiên cũ từ đĩa (nếu có).
    pub fn open(dir: PathBuf) -> SessionStore {
        let _ = std::fs::create_dir_all(&dir);
        let mut map: HashMap<String, Session> = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&p) {
                    if let Ok(s) = serde_json::from_str::<Session>(&text) {
                        map.insert(s.id.clone(), s);
                    }
                }
            }
        }
        SessionStore {
            dir,
            sessions: Mutex::new(map),
        }
    }

    #[allow(dead_code)] // API tiện ích — hiện chưa dùng trong handler
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Tạo phiên mới trong store (lưu đĩa + bộ nhớ).
    pub fn create(&self, session: &Session) -> std::io::Result<Session> {
        self.persist(session)?;
        self.sessions
            .lock()
            .expect("session lock")
            .insert(session.id.clone(), session.clone());
        self.prune_locked();
        Ok(session.clone())
    }

    pub fn get(&self, id: &str) -> Option<Session> {
        self.sessions
            .lock()
            .expect("session lock")
            .get(id)
            .cloned()
    }

    /// Cập nhật phiên (đã thay đổi) — lưu lại đĩa.
    pub fn save(&self, session: &Session) -> std::io::Result<()> {
        self.persist(session)?;
        self.sessions
            .lock()
            .expect("session lock")
            .insert(session.id.clone(), session.clone());
        Ok(())
    }

    /// NHẢ PHIÊN — xoá khỏi bộ nhớ và đĩa. Trả về true nếu tìm thấy.
    pub fn delete(&self, id: &str) -> bool {
        let removed = {
            let mut guard = self.sessions.lock().expect("session lock");
            guard.remove(id)
        };
        if removed.is_some() {
            let _ = std::fs::remove_file(self.dir.join(format!("{}.json", safe_id(id))));
        }
        removed.is_some()
    }

    /// Nhả tất cả các phiên.
    pub fn delete_all(&self) -> usize {
        let removed: Vec<Session> = {
            let mut guard = self.sessions.lock().expect("session lock");
            let ids: Vec<Session> = guard.drain().map(|(_, v)| v).collect();
            ids
        };
        let n = removed.len();
        if let Ok(entries) = std::fs::read_dir(&self.dir) {
            for e in entries.flatten() {
                let _ = std::fs::remove_file(e.path());
            }
        }
        n
    }

    /// Danh sách phiên (mới nhất trước), dạng nhẹ.
    pub fn list(&self) -> Vec<Session> {
        let guard = self.sessions.lock().expect("session lock");
        let mut v: Vec<Session> = guard.values().cloned().collect();
        drop(guard);
        v.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        v
    }

    pub fn count(&self) -> usize {
        self.sessions.lock().expect("session lock").len()
    }

    fn persist(&self, s: &Session) -> std::io::Result<()> {
        let name = format!("{}.json", safe_id(&s.id));
        let tmp = self.dir.join(format!("{}.tmp", name));
        let fin = self.dir.join(name);
        let text = serde_json::to_string_pretty(s)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &fin)?;
        Ok(())
    }

    /// Giữ tối đa 300 phiên trên đĩa để tránh phình to.
    fn prune_locked(&self) {
        const MAX_KEEP: usize = 300;
        let excess_ids: Vec<String> = {
            let guard = self.sessions.lock().expect("session lock");
            if guard.len() <= MAX_KEEP {
                Vec::new()
            } else {
                let mut v: Vec<&Session> = guard.values().collect();
                v.sort_by(|a, b| a.created_at.cmp(&b.created_at));
                v.iter()
                    .take(guard.len() - MAX_KEEP)
                    .map(|s| s.id.clone())
                    .collect()
            }
        };
        for id in excess_ids {
            self.delete(&id);
        }
    }
}

/// ID chỉ gồm [a-z0-9-] — an toàn cho tên file.
fn safe_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

/// Tạo id phiên mới.
pub fn generate_id() -> String {
    new_id()
}
