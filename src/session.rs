//! session.rs — quản lý phiên ghép (tạo / mở / cập nhật / NHẢ PHIÊN).
//!
//! SỬA LỖI "chạy bài thứ hai bị 'Không ghép được.'" của bản 1.x:
//!   * Mỗi lần ghép là MỘT PHIÊN MỚI hoàn toàn độc lập — không tái dùng trạng thái cũ,
//!     không giữ biến toàn cục của bộ ghép.
//!   * Bộ ghép (aligner) thuần vô trạng thái; mọi dữ liệu nằm trong Session.
//!
//! BẢN 2.5 — MÔ HÌNH KHỐI (block) cho kéo–thả tự do:
//!   * Mỗi đoạn văn là MỘT KHỐI, identity = chỉ số đoạn gốc trong `english`/`vietnamese`
//!     (bất biến I1 bảo đảm mỗi đoạn dùng đúng 1 lần → chỉ số là mã khối duy nhất).
//!   * `move_block` chuyển khối sang ô cùng ngôn ngữ của HÀNG BẤT KỲ (chèn tại vị trí
//!     tuỳ ý), hoặc TÁCH khối thành hàng riêng mới — hoàn toàn đảo ngược được:
//!     mọi thay đổi được đẩy vào NGĂN XẾP HOÀN TÁC (snapshot, Ctrl+Z trên giao diện).
//!   * Hàng rỗng sau khi mất khối cuối cùng tự động bị xoá (prune).
//!   * `rescan_row` chấm LẠI điểm khớp của một hàng (LaBSE nếu dịch vụ chạy,
//!     kèm sàn từ vựng) — dùng sau khi kéo–thả để "xác định lại độ khớp".
//!   * Cờ `reordered`: đúng khi người dùng đã đổi thứ tự khối giữa các hàng —
//!     khi đó bất biến đơn điệu (I3) được tha thứ lúc xuất (I1/I2/I4/I5 vẫn gắt).

use crate::aligner::{AlignOptions, Row, Seg, Status, ALL_STATUSES};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(1);

/// Số bước hoàn tác giữ lại cho mỗi phiên (đủ cho một phiên làm việc dài).
const HISTORY_CAP: usize = 40;

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

/// Giá trị mặc định cho `Session::reordered` — các phiên ≤2.4 không có trường này.
fn default_false() -> bool {
    false
}

// ========================= Phía ngôn ngữ (Side) =========================

/// Phía ngôn ngữ của một khối — khối EN chỉ nằm được trong ô EN, VI chỉ trong ô VI.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    En,
    Vi,
}

impl Side {
    pub fn parse(s: &str) -> Option<Side> {
        match s.trim().to_ascii_lowercase().as_str() {
            "en" | "english" | "anh" => Some(Side::En),
            "vi" | "vie" | "vietnamese" | "vietnam" => Some(Side::Vi),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Side::En => "en",
            Side::Vi => "vi",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Side::En => "Anh",
            Side::Vi => "Việt",
        }
    }
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
    /// BẢN 2.5 — đúng khi người dùng đã kéo–thả đổi thứ tự khối giữa các hàng
    /// (làm mất tính đơn điệu I3 một cách CÓ CHỦ ĐÍCH). Xuất file lúc đó chỉ bỏ
    /// kiểm I3; mọi bất biến khác (mỗi đoạn 1 lần, trạng thái chuẩn, `none`) giữ nguyên.
    #[serde(default = "default_false")]
    pub reordered: bool,
    /// Ngăn xếp HOÀN TÁC (snapshot hàng, bản 2.5) — không lưu ra đĩa.
    #[serde(skip)]
    pub undo_stack: Vec<Vec<Row>>,
    /// Ngăn xếp LÀM LẠI (bản 2.5) — không lưu ra đĩa.
    #[serde(skip)]
    pub redo_stack: Vec<Vec<Row>>,
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
            reordered: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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

    // ========================= Hoàn tác / làm lại (bản 2.5) =========================

    /// Đẩy snapshot hàng hiện tại vào ngăn hoàn tác (xoá redo) — gọi TRƯỚC khi đổi.
    fn push_history(&mut self) {
        self.undo_stack.push(self.rows.clone());
        if self.undo_stack.len() > HISTORY_CAP {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// HOÀN TÁC bước gần nhất (snapshot khôi phục nguyên xi trạng thái hàng).
    pub fn undo(&mut self) -> Result<(), String> {
        match self.undo_stack.pop() {
            None => Err("Không còn thao tác nào để hoàn tác.".into()),
            Some(prev) => {
                self.redo_stack
                    .push(std::mem::replace(&mut self.rows, prev));
                self.reindex();
                self.reordered = !self.is_monotonic();
                self.updated_at = now_ms();
                Ok(())
            }
        }
    }

    /// LÀM LẠI bước vừa hoàn tác.
    pub fn redo(&mut self) -> Result<(), String> {
        match self.redo_stack.pop() {
            None => Err("Không còn thao tác nào để làm lại.".into()),
            Some(next) => {
                self.undo_stack
                    .push(std::mem::replace(&mut self.rows, next));
                self.reindex();
                self.reordered = !self.is_monotonic();
                self.updated_at = now_ms();
                Ok(())
            }
        }
    }

    /// Chuỗi chỉ số đoạn mỗi phía có còn đơn điệu (tăng nghiêm) theo thứ tự hàng không.
    fn is_monotonic(&self) -> bool {
        let mut last_en: Option<usize> = None;
        let mut last_vi: Option<usize> = None;
        for r in &self.rows {
            for (a, _) in r.en_segments() {
                if let Some(le) = last_en {
                    if a <= le {
                        return false;
                    }
                }
                last_en = Some(a);
            }
            for (b, _) in r.vi_segments() {
                if let Some(lv) = last_vi {
                    if b <= lv {
                        return false;
                    }
                }
                last_vi = Some(b);
            }
        }
        true
    }

    // ========================= Kéo–thả KHỐI (bản 2.5) =========================

    /// Chuyển khối `from_pos` (ô `side` của hàng `from_row`) sang ô cùng ngôn ngữ
    /// của hàng `to_row` tại vị trí chèn `to_pos` (0..=số khối, quá lớn = nối cuối);
    /// hoặc — khi `to_row` là None — TÁCH khối thành HÀNG RIÊNG mới chèn ngay sau
    /// `after_row` (mặc định = hàng nguồn). Hàng nguồn mất khối cuối cùng sẽ tự xoá.
    ///
    /// Trả về chỉ số hàng chứa khối sau khi di chuyển (để giao diện nhấp nháy đúng chỗ).
    /// Thao tác hoàn toàn đảo ngược được: Ctrl+Z / nút Hoàn tác trả về đúng trạng thái cũ.
    pub fn move_block(
        &mut self,
        side: Side,
        from_row: usize,
        from_pos: usize,
        to_row: Option<usize>,
        to_pos: usize,
        after_row: Option<usize>,
    ) -> Result<usize, String> {
        if from_row >= self.rows.len() {
            return Err(format!(
                "Hàng nguồn {} không tồn tại (phiên có {} hàng).",
                from_row,
                self.rows.len()
            ));
        }
        let src_blocks = row_side_blocks(&self.rows[from_row], side);
        if from_pos >= src_blocks.len() {
            return Err(format!(
                "Khối thứ {} không tồn tại ở phía {} của hàng {} (phía này có {} khối).",
                from_pos,
                side.label(),
                from_row,
                src_blocks.len()
            ));
        }
        if let Some(tr) = to_row {
            if tr >= self.rows.len() {
                return Err(format!(
                    "Hàng đích {} không tồn tại (phiên có {} hàng).",
                    tr,
                    self.rows.len()
                ));
            }
        }
        let after_row = match (to_row, after_row) {
            (None, Some(a)) => {
                if a >= self.rows.len() {
                    return Err(format!(
                        "Hàng mốc {} không tồn tại (phiên có {} hàng).",
                        a,
                        self.rows.len()
                    ));
                }
                a
            }
            (None, None) => from_row,
            (Some(_), _) => from_row, // không dùng khi đích là hàng sẵn có
        };

        // ---- trả về sớm nếu là di chuyển tại chỗ (no-op): không đẩy history
        if let Some(tr) = to_row {
            if tr == from_row {
                let tp = to_pos.min(src_blocks.len());
                let eff = if from_pos < tp { tp - 1 } else { tp };
                if eff == from_pos {
                    // khối về đúng chỗ cũ — không đổi gì
                    return Ok(from_row);
                }
            }
        }

        let block = src_blocks[from_pos].clone();
        let block_orig_index = block.index;
        self.push_history();

        // ---- 1) gỡ khối khỏi hàng nguồn
        let mut remain = src_blocks.clone();
        remain.remove(from_pos);
        set_row_side_blocks(&mut self.rows[from_row], side, remain);

        if let Some(tr) = to_row {
            // ---- 2a) chèn vào hàng đích (hiện có)
            let mut target_blocks = row_side_blocks(&self.rows[tr], side);
            let tp = to_pos.min(target_blocks.len());
            let tp = if tr == from_row && from_pos < tp {
                tp - 1
            } else {
                tp
            };
            target_blocks.insert(tp, block);
            set_row_side_blocks(&mut self.rows[tr], side, target_blocks);
        } else {
            // ---- 2b) tách thành hàng riêng mới, chèn sau `after_row`
            let insert_at = after_row + 1;
            let new_row = match side {
                Side::En => Row::single_side(
                    Some((block.index, block.text.clone())),
                    None,
                    Status::Unmatched,
                ),
                Side::Vi => Row::single_side(
                    None,
                    Some((block.index, block.text.clone())),
                    Status::Unmatched,
                ),
            };
            self.rows.insert(insert_at, new_row);
        }

        // ---- 3) đặt lại trạng thái/điểm của các hàng liên quan (nội dung đổi → chấm lại)
        //     hàng nguồn (nếu còn sót) và hàng đích (nếu là hàng hiện có)
        let mut prune_at: Option<usize> = None;
        if to_row != Some(from_row) {
            let src = &self.rows[from_row];
            if src.en_segments().is_empty() && src.vi_segments().is_empty() {
                prune_at = Some(from_row);
            } else {
                refresh_row_after_edit(&mut self.rows[from_row]);
            }
        }
        if let Some(tr) = to_row {
            if tr != from_row {
                refresh_row_after_edit(&mut self.rows[tr]);
            }
        }

        // ---- 4) xoá hàng nguồn nếu rỗng (sau khi chèn, chỉ số có thể đã dịch)
        if let Some(p) = prune_at {
            self.rows.remove(p);
        }

        // ---- 5) dọn thứ tự + cờ reordered + mốc focus
        //     khối mang chỉ số đoạn GỐC duy nhất (I1) → tìm hàng đang chứa nó là ra focus
        let focus = self
            .rows
            .iter()
            .position(|r| {
                row_side_blocks(r, side)
                    .iter()
                    .any(|s| s.index == block_orig_index)
            })
            .unwrap_or(0);
        self.reindex();
        self.reordered = !self.is_monotonic();
        self.updated_at = now_ms();
        Ok(focus)
    }

    // ========================= Chấm lại điểm (bản 2.5) =========================

    /// RESCAN một hàng: ghép các khối mỗi phía thành 1 văn bản, chấm lại điểm
    /// tương đồng bằng bộ từ vựng (giữ idf của TOÀN phiên như lúc ghép gốc) và
    /// HOÀ TRỘN cosine LaBSE nếu dịch vụ nhúng đang chạy. Cập nhật điểm + trạng thái
    /// theo ngưỡng của phiên (≥ ngưỡng → tự khớp, dưới → cần soát).
    /// Trả về (điểm, trạng thái mới, LaBSE đã dùng?).
    pub fn rescan_row(&mut self, row_index: usize) -> Result<(f64, Status, bool), String> {
        if row_index >= self.rows.len() {
            return Err(format!(
                "Hàng {} không tồn tại (phiên có {} hàng).",
                row_index,
                self.rows.len()
            ));
        }
        {
            let row = &self.rows[row_index];
            if !row.has_en() || !row.has_vi() {
                return Err(
                    "Hàng chỉ có một phía — chưa chấm điểm được. Kéo khối vào phía thiếu (hoặc tách khối ra hàng khác) rồi bấm Rescan lại."
                        .into(),
                );
            }
        }
        let en_text = self.rows[row_index]
            .en_segments()
            .iter()
            .map(|(_, t)| *t)
            .collect::<Vec<&str>>()
            .join(" ");
        let vi_text = self.rows[row_index]
            .vi_segments()
            .iter()
            .map(|(_, t)| *t)
            .collect::<Vec<&str>>()
            .join(" ");
        let emb_cos = crate::embedder::pair_cosine(&en_text, &vi_text);
        let raw = crate::aligner::rescore_pair(
            &self.english,
            &self.vietnamese,
            &en_text,
            &vi_text,
            emb_cos,
        );
        let score = (raw * 1000.0).round() / 1000.0;
        let status = if score >= self.options.auto_threshold {
            Status::AutoMatched
        } else {
            Status::NeedsReview
        };
        self.push_history();
        let row = &mut self.rows[row_index];
        row.score = Some(score);
        row.status = status;
        self.updated_at = now_ms();
        Ok((score, status, emb_cos.is_some()))
    }

    // ========================= Trạng thái / gộp hàng =========================

    /// Đặt trạng thái cho một hàng.
    /// * Với hàng ĐỦ hai bên: cho phép auto_matched / needs_review / human_confirmed.
    ///   Chọn `unmatched` sẽ TÁCH hàng thành các hàng đơn phía (bỏ ghép).
    /// * Với hàng MỘT phía: cho phép human_confirmed (xác nhận "không có bản dịch")
    ///   và unmatched (trở lại).
    /// * Hàng GỘP (n EN · m VI): tách trả về đúng n + m hàng đơn phía, giữ thứ tự gốc
    ///   (cặp đơn giản vẫn ra 2 hàng như bản cũ). Hỗ trợ HOÀN TÁC từ bản 2.5.
    pub fn set_row_status(&mut self, row_index: usize, status: Status) -> Result<(), String> {
        if row_index >= self.rows.len() {
            return Err(format!(
                "Hàng {} không tồn tại (có {} hàng).",
                row_index,
                self.rows.len()
            ));
        }
        if !ALL_STATUSES.contains(&status) {
            return Err("Trạng thái không hợp lệ.".into());
        }
        let row = self.rows[row_index].clone();
        let has_en = row.has_en();
        let has_vi = row.has_vi();

        if status == Status::Unmatched {
            if has_en && has_vi {
                self.push_history();
                // TÁCH hoàn toàn: mỗi đoạn một hàng đơn phía, chèn tại chỗ, giữ thứ tự
                let mut pieces: Vec<Row> = Vec::new();
                for (i, t) in row.en_segments() {
                    pieces.push(Row::single_side(
                        Some((i, t.to_string())),
                        None,
                        Status::Unmatched,
                    ));
                }
                for (i, t) in row.vi_segments() {
                    pieces.push(Row::single_side(
                        None,
                        Some((i, t.to_string())),
                        Status::Unmatched,
                    ));
                }
                self.rows.remove(row_index);
                for (k, p) in pieces.into_iter().enumerate() {
                    self.rows.insert(row_index + k, p);
                }
                self.reindex();
            } else {
                self.push_history();
                self.rows[row_index].status = Status::Unmatched;
            }
        } else {
            if !has_en || !has_vi {
                if status == Status::HumanConfirmed {
                    self.push_history();
                    self.rows[row_index].status = Status::HumanConfirmed;
                } else {
                    return Err(
                        "Hàng chỉ có một phía — chỉ hỗ trợ 'Đã xác nhận' (human_confirmed) hoặc 'Không khớp' (unmatched)."
                            .into(),
                    );
                }
            } else {
                self.push_history();
                self.rows[row_index].status = status;
            }
        }
        self.updated_at = now_ms();
        Ok(())
    }

    /// GỘP hàng kế tiếp (row_index + 1) VÀO hàng `row_index` (bản 2.4, giữ cho API cũ).
    ///
    /// Nguyên tắc an toàn:
    /// * Chỉ cho phép gộp hàng LIỀN KỀ → tính đơn điệu (I3) luôn được bảo toàn.
    /// * Mọi đoạn của hàng dưới được chuyển sang hàng trên: nếu hàng trên còn
    ///   trống phía đó thì thành đoạn chính, ngược lại vào `en_extra`/`vi_extra`.
    /// * Trạng thái sau gộp: nếu chỉ định thì dùng (trừ `unmatched`); mặc định
    ///   `needs_review`. Điểm cũ đặt lại `None`. Hỗ trợ HOÀN TÁC từ bản 2.5.
    pub fn merge_next(&mut self, row_index: usize, status: Option<Status>) -> Result<(), String> {
        if row_index >= self.rows.len() {
            return Err(format!(
                "Hàng {} không tồn tại (có {} hàng).",
                row_index,
                self.rows.len()
            ));
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

        self.push_history();
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

// ========================= Helper khối (bản 2.5) =========================

/// Danh sách khối của một phía hàng (sao chép, thứ tự hiển thị = thứ tự xuất).
fn row_side_blocks(row: &Row, side: Side) -> Vec<Seg> {
    let segs = match side {
        Side::En => row.en_segments(),
        Side::Vi => row.vi_segments(),
    };
    segs.iter()
        .map(|(i, t)| Seg::new(*i, t.to_string()))
        .collect()
}

/// Ghi lại danh sách khối cho một phía hàng: khối đầu thành đoạn chính, còn lại vào extra.
fn set_row_side_blocks(row: &mut Row, side: Side, blocks: Vec<Seg>) {
    match side {
        Side::En => match blocks.split_first() {
            Some((first, rest)) => {
                row.en_index = Some(first.index);
                row.en = Some(first.text.clone());
                row.en_extra = rest.to_vec();
            }
            None => {
                row.en_index = None;
                row.en = None;
                row.en_extra = Vec::new();
            }
        },
        Side::Vi => match blocks.split_first() {
            Some((first, rest)) => {
                row.vi_index = Some(first.index);
                row.vi = Some(first.text.clone());
                row.vi_extra = rest.to_vec();
            }
            None => {
                row.vi_index = None;
                row.vi = None;
                row.vi_extra = Vec::new();
            }
        },
    }
}

/// Nội dung hàng vừa đổi (kéo khối vào/ra) → đặt lại trạng thái + xoá điểm cũ.
fn refresh_row_after_edit(row: &mut Row) {
    if row.has_en() && row.has_vi() {
        row.status = Status::NeedsReview;
        row.score = None;
    } else if row.has_en() || row.has_vi() {
        row.status = Status::Unmatched;
        row.score = None;
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
        self.sessions.lock().expect("session lock").get(id).cloned()
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
