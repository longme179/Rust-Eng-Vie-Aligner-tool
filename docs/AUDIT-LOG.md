# AUDIT-LOG

> Mỗi lần audit ghi một mục mới nhất lên đầu. Mốc audit = commit `main` đã được đọc kiểm
> toàn bộ mã tính tới thời điểm đó (không phải thời điểm chạy — đọc lại diff khi cần).

---

## Audit #1 — 2026-10-18

- **Mốc audit**: repo chưa có `docs/AUDIT-LOG.md` → đây là lần audit **đầu tiên**. Tag mới nhất
  khi đó là `v2.7.1` = `91a5e22` = HEAD `main` (diff rỗng) → theo đúng quy trình, audit **toàn bộ
  repo**, trọng tâm các tính năng chưa từng được ai rà soát ad hoc (v2.5 khối kéo–thả/hoàn tác,
  v2.6 bộ lọc chrom khi tải URL, v2.7 xuất xlsx màu).
- **Kết quả test gốc (trước khi sửa)**: `cargo test` 2/2 unit test; `GET /api/selftest` **35/35**
  (bản v2.7.1, port 3010, chế độ từ vựng — sandbox không có LaBSE thật).
- **Công cụ**: `cargo build --release` sạch; `cargo fmt --check` 136 chỗ lệch (7 file — repo chưa
  từng fmt); `cargo clippy` 0 error / 59 warning style; `cargo audit`/`cargo deny` **chưa cài được**
  trong phiên (biên dịch cargo-audit quá lâu so với ngân sách phiên — ghi việc còn thiếu, không dựng số).
- **Phạm vi đọc mã**: `src/main.rs` (toàn file), `src/exporter.rs` (toàn file), `src/fetcher.rs`
  (phần fetch/SSRF + hằng số lọc), `src/session.rs` (store/undo/move_block/rescan), `src/aligner.rs`
  (score_tokens/dp_align/align_with_emb/backtrack/validate), `src/embedder.rs` (toàn file),
  `src/fold.rs` (toàn file), `static/index.html` (render hàng/escape/các handler), 3 script build
  (đọc, không chạy).

### Findings

#### Nghiêm trọng — (không có)

#### Nên sửa — 4 lỗi, đã sửa trong `audit/2026-10-18-fixes`

| # | Finding | Bằng chứng | Tác động | Sửa |
|---|---|---|---|---|
| A1 | Server bind `0.0.0.0:{port}` thay vì `127.0.0.1` — mâu thuẫn lời hứa "100% cục bộ" trên chính banner in ra | [ĐỌC MÃ] `src/main.rs:751` (bản v2.7.1): `let addr = format!("0.0.0.0:{}", port);` | Mọi máy trong LAN mở được API (đọc phiên, ghi phiên, dùng fetch-url như proxy); kèm DNS-rebinding từ trang web lạ | Mặc định `127.0.0.1`; mở LAN phải tự đặt `REVA_BIND=0.0.0.0` (banner cảnh báo). [ĐÃ CHẠY] `ss -tlnp` sau khi chạy: `127.0.0.1:3010` |
| A2 | SSRF khi tải URL: không chặn IP nội bộ; ureq tự theo redirect (≤5) không kiểm từng hop | [ĐỌC MÃ] `src/fetcher.rs:78-95` (bản 2.7.1): chỉ kiểm scheme http/https, không có kiểm địa chỉ | Lệnh `POST /api/fetch-url` với `http://127.0.0.1:3031/…` hay `http://169.254.169.254/…` đọc được dịch vụ nội bộ; public URL 302 về IP nội bộ cũng lọt | Chặn IP literal + DNS resolves nội bộ (loopback/private/link-local/ULA-IPv6/CGNAT/0.0.0.0/8…); tự theo redirect thủ công ≤5 hop, kiểm an toàn MỖI hop, chặn Location giao thức lạ. [ĐÃ CHẠY] API từ chối `127.0.0.1:3031` và `169.254.169.254` với thông báo tiếng Việt; theo đúng 301 `http://github.com` → `https://github.com` (24 đoạn). Test hồi quy: selftest 39 |
| A3 | CSV/TSV Formula Injection (OWASP "CSV Injection"): ô bắt đầu `=` `+` `-` `@` được ghi thẳng, Excel mở file sẽ chạy như công thức | [ĐỌC MÃ] `src/exporter.rs:148-154` (csv_field chỉ escape RFC4180) + nhánh TSV ghi en/vi thẳng; README lại KHUYẾN KHÍCH mở CSV bằng Excel | Văn bản chứa `=cmd|' /C calc'!A1` / `=WEBSERVICE(...)` (dán từ tài liệu người khác nhờ dịch) chạy mã khi mở file | Thêm dấu `'` trước ô nguy hiểm (chỉ CSV/TSV; xlsx ghi kiểu chuỗi). [ĐÃ CHẠY] xuất qua API: `'=cmd|' /C calc'!A1 …`. Test hồi quy: selftest 36 |
| A4 | Ký tự điều khiển (0x00–0x1F, 0x7F — dán từ PDF/Word) làm XLIFF invalid XML 1.0 | [ĐỌC MÃ] `xml_escape` (2.7.1) không lọc ký tự điều khiển; `one_line` chỉ thay \n\r\t | File XLIFF hỏng khi nhập vào CAT tool | `one_line`/`xml_escape`/`html_escape` thay ký tự điều khiển bằng space/lọc bỏ. Test hồi quy: selftest 38 |

#### Nhỏ — 3 đã sửa, 4 ghi nhận để lại

| # | Finding | Bằng chứng | Xử lý |
|---|---|---|---|
| M1 | Chuỗi UI ghi "33 bài" trong khi selftest khi đó là 35 | [ĐỌC MÃ] `static/index.html:414,1268` (2.7.1) | Đã sửa (bỏ số cứng) trong audit branch |
| M2 | Repo chưa chuẩn `cargo fmt` (136 chỗ) | [ĐÃ CHẠY] `cargo fmt --check` | Đã fmt toàn bộ (commit riêng, thuần định dạng, selftest vẫn xanh) |
| M3 | `guard_formula` viết theo kiểu bị clippy báo "redundant guard" | [ĐÃ CHẠY] clippy sau khi viết | Đã đổi `starts_with([...])` |
| M4 | Kéo–thả file .txt (feat mới) không giới hạn kích thước | [ĐỌC MÃ] khi tự phản biện mã mình | Đã chặn >4 MB + toast hướng dẫn tách file |
| M5 | TOCTOU DNS-rebinding: validate_public_url phân giải DNS TRƯỚC khi ureq tự kết nối lại — attacker kiểm soát DNS có thể đổi IP giữa 2 lần | [ĐỌC MÃ] bản chất thiết kế (đã ghi rõ trong comment mã) | **Còn để ngỏ** — rủi ro thấp (kẻ tấn công phải kiểm soát DNS của nạn nhân); sửa triệt để cần pin IP + ghi Host, phức tạp không xứng đáng với tool 1 người dùng. Ghi cho phiên sau nếu muốn |
| M6 | Hai tab cùng sửa 1 phiên: get→sửa→save last-writer-wins | [ĐỌC MÃ] `SessionStore::save` không khoá phiên; ghi atomic tmp+rename chống hỏng FILE nhưng không chống mất cập nhật | **Chấp nhận** (tool 1 người dùng; ghi rõ hạn chế). Không sửa |
| M7 | Clippy 59→~78 warning style (doc-list indent, map_or…) | [ĐÃ CHẠY] `cargo clippy` trước/sau | **Còn để ngỏ** — toàn style, không đúng đắn; fix dần trong các PR sau để tránh nhiễu diff |
| M8 | URL scheme chữ HOA (`HTTP://…`) bị từ chối sau A2 | [ĐỌC MÃ] `strip_prefix("http://")` phân biệt hoa thường | **Còn để ngỏ** — thông báo lỗi rõ ràng; trình duyệt luôn hạ thường scheme trước khi dán. Sửa nếu có người dùng báo |

#### Kiểm tra và xác nhận AN TOÀN (không có finding)

- **HTML escaping đầy đủ** (& < > " ') ở cả xuất file lẫn UI (`esc()` dùng cho mọi nội dung động) —
  [ĐỌC MÃ] `exporter.rs::html_escape`, `index.html:607` + mọi chỗ chèn `esc(s.text)`.
- **xlsx không thể bị chèn công thức**: `write_with_format(.., &str, ..)` ghi kiểu CHUỖI (sharedStrings) —
  [ĐÃ CHẠY] selftest 37: xuất `=1+1` → sheet không có phần tử `<f>`, chuỗi nằm trong sharedStrings.
- **Path traversal không thể**: `safe_id` lọc `[a-z0-9-]` trước khi ghép tên file — [ĐỌC MÃ] `session.rs::safe_id/persist/delete`.
- **Ghi phiên atomic** (tmp + rename) — [ĐỌC MÃ] `SessionStore::persist`. File hỏng lúc khởi động bị bỏ qua im lặng — [ĐỌC MÃ] `SessionStore::open`.
- **Panic containment**: mọi đường CPU/mạng (align/rescan/fetch) bọc `catch_unwind` + `spawn_blocking` — [ĐỌC MÃ] main.rs 4 handler.
- **Giới hạn đầu vào**: body 16 MB (axum DefaultBodyLimit), 800 đoạn/bên, DP ≤ MAX_DP_AREA, fetch 6 MB + timeout 25 s — [ĐỌC MÃ] aligner.rs/fetcher.rs/main.rs.
- **"Chỉ nâng không hạ" của LaBSE**: `blend_with_emb = max(từ_vựng, 0.4×từ_vựng + 0.6×tin_cậy)` — [ĐỌC MÃ] aligner.rs:58-62, công thức đúng như tài liệu.
- **NFC/NFD tiếng Việt**: fold dùng NFKD + strip combining marks → văn bản NFD (macOS) và NFC cho cùng kết quả — [ĐỌC MÃ] fold.rs:8-27.
- **CRLF**: `split_paragraphs` chuẩn hoá `\r\n`/`\r` — [ĐÃ CHẠY] selftest 01.
- **Undo 40 bước**: snapshot `Vec<Vec<Row>>` đóng gói đúng, reindex + tính lại cờ `reordered` sau undo/redo — [ĐỌC MÃ] session.rs::push_history/undo/redo.
- **Backtrack DP an toàn underflow** (không còn sentinel -1 của bản 1.x) — [ĐỌC MÃ] aligner.rs::align_with_emb.
- **Export filename** chỉ gồm id server-generated (`[a-z0-9-]`) + đuôi cố định → không chèn header — [ĐỌC MÃ] exporter.rs ExportResult.filename + session.rs::new_id.

### Script build/run (đọc, KHÔNG chạy — đúng quy trình)

- 3 script (linux/macos/windows) đọc kỹ: việc cài chỉ khi thiếu, không nâng cấp pip, model offline 1 lần,
  exit code rõ, `pause` cho Windows. Rủi ro đáng ghi: script chạy `cargo build` + venv **trong thư mục
  mã nguồn** (người dùng cần ~6 GB trống và có quyền ghi); `start-labse-windows.bat` mở cửa sổ thu nhỏ
  tồn_at sau khi app dừng (script có nhắc đóng tay — chấp nhận được cho non-developer). Không có finding mới.

### PR liên quan (cả stack base `main`, PAT không đủ quyền mở PR nên kèm link compare)

1. `audit/2026-10-18-fixes` — sửa A1–A4 + M1 + M2 (+ fmt). Link compare:
   https://github.com/longme179/Rust-Eng-Vie-Aligner-tool/compare/main...audit/2026-10-18-fixes
2. `feat/tmx-export` (stack trên 1) — nâng cấp #1 từ ROADMAP. Compare vs main:
   https://github.com/longme179/Rust-Eng-Vie-Aligner-tool/compare/main...feat/tmx-export
3. `feat/rescan-all` (stack trên 2). Compare: …/compare/main...feat/rescan-all
4. `feat/drag-drop-txt` (stack trên 3, chứa cả sửa self-audit M3+M4). Compare: …/compare/main...feat/drag-drop-txt
5. `docs/2026-10-18-roadmap` (từ main) — `docs/ROADMAP.md` + `THIRD_PARTY.md`. Compare: …/compare/main...docs/2026-10-18-roadmap
6. `docs/2026-10-18-audit-log` (stack trên 4) — file này + cập nhật số liệu README/docs. Compare: …/compare/main...docs/2026-10-18-audit-log

### Trạng thái cuối phiên

- Selftest **41/41** (35 cũ + 36–39 audit + 40 TMX + 41 rescan-all); `cargo test` 2/2; build sạch;
  fmt sạch; clippy 0 error (style warnings còn lại — M7).
- Mốc audit cho lần sau: commit trên nhánh `docs/2026-10-18-audit-log` (sau khi merge vào main,
  dùng SHA merge-commit).

### CẦN MÁY THẬT (không kiểm được trong sandbox)

1. LaBSE thật (Windows/macOS): chạy `bash build-and-run-*.sh` lần đầu, xác nhận huy hiệu đổi thành
   "Từ vựng + LaBSE", ghép lại bài mẫu, bấm `↻ Rescan tất cả` — toast phải ghi "(LaBSE + từ vựng)".
2. Mở file `.xlsx`/`.csv` xuất ra bằng Excel thật trên Windows/macOS: xác nhận màu hàng, và với CSV
   chứa ô `=cmd|…` thì Excel hiện `'=cmd|…` là VĂN BẢN (không chạy).
3. Nhập file `.tmx` xuất từ REVA vào OmegaT (File → Import Translation Memory) — phải nạp được 2 tu mỗi tu.
4. Kéo–thả 2 file .txt thật từ Explorer/Finder vào 2 ô — xác nhận toast + số đoạn.
5. Trên máy có mạng LAN khác: mặc định phải KHÔNG truy cập được từ máy khác (bind 127.0.0.1);
   đặt `REVA_BIND=0.0.0.0` rồi thử lại từ máy thứ hai.
