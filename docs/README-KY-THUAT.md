# REVA tool — tài liệu kỹ thuật

> Tài liệu cho người phát triển / người muốn hiểu sâu. Nếu bạn chỉ muốn **dùng REVA**, xem [README chính](../README.md).

# REVA tool (`reva-tool`) v2.10

> Tiền thân: **Cánh đôi Anh–Việt** (`canh-doi-anh-viet`) — cùng công cụ, đổi tên từ bản 2.5.

Công cụ **ghép & sắp đoạn văn bản Anh–Việt** (chỉ ghép, không dịch) đóng gói thành **một chương trình Rust duy nhất**:
giao diện + API + bộ ghép nằm trong 1 file binary tự build, chạy **100% cục bộ** trên máy bạn — không gửi dữ liệu đi đâu.

```
[reva-tool] chạy tại http://localhost:3000 (giao diện + API, tất cả cục bộ)
[reva-tool] LaBSE (native, tự tải 1 lần ~450 MB): khởi động nền — xem huy hiệu trên đầu trang
```

## Lịch sử bản chính

- **2.10** — **[LABSE CỐT LÕI — NHÁNH `feat/labse-native`, MERGE `0ed9426`]** LaBSE từ “tuỳ chọn cần Python” thành **phần cốt lõi chạy ngay trong binary**: crate `ort` 2.0.0-rc.15 (load-dynamic, `ort::init_from` nạp động ONNX Runtime) + crate `tokenizers` 0.23 (feature `onig`) + `flate2`/`tar` (giải nén). **Tự tải 1 lần duy nhất** (HTTP Range resume + retry 6 lần, tự nối lại khi đứt): model `Xenova/LaBSE` `onnx/model_quantized.onnx` int8 ~450 MB + `tokenizer.json` ~13 MB từ huggingface.co → `data/models/labse/`; thư viện ONNX Runtime 1.31.1 từ github.com/microsoft/onnxruntime (win-x64 zip ~79 MB / linux-x64 tgz ~11 MB / osx-arm64 ~35 MB) → `data/models/ort/` — BẪY SYMLINK linux tgz đã sửa (bỏ EntryType::Link, chỉ lấy file thật >1 MB). Máy trạng thái Off → Downloading{pct} → Loading → Ready/Failed trên **thread nền không chặn server**; smoke test 2 câu trước khi Ready. `/api/model-status` thêm `labse.native` (phase/pct/detail/modelDir/ortVersion); route mới `POST /api/labse/retry` (chỉ chạy khi Failed/Off). UI: huy hiệu động cơ hiển thị tiến trình % + nút thử lại + poll 1,5 s tự ngừng. Env: `REVA_LABSE_URL` (ưu tiên HTTP cũ nếu đặt tường minh), `REVA_LABSE_AUTO=0` (tắt tự tải), `REVA_LABSE_MAXLEN` (mặc định 256). Embed: encode_batch truncate/pad → tensor i64 → output ưu tiên `embeddings` → `pooler_output` → mean-pool `last_hidden_state` theo attention mask; L2-chuẩn hoá; chia lô 16. Scoring giữ nguyên `max(lexical, 0.4·lexical + 0.6·LaBSE)`. Windows build qua cargo-zigbuild (binary 7,97 MB so với 4,8 MB bản trước — ort+tokenizers tĩnh). Selftest 56→**58** (57: status_json đủ khóa mọi pha + embed Err tiếng Việt khi chưa ready; 58: vector L2 thật cùng chiều ≥64 + L2≈1). KHÔNG cần Python nữa; script build-and-run rút từ 5 bước xuống 3 bước.
- **2.9 (UI)** — **[DỌN UI CHUYÊN NGHIỆP 2026-10-19 — nhánh `feat/professional-ui`]** làm lại toàn bộ lớp trình bày `static/index.html` **không đụng logic**: (1) hệ **biểu tượng SVG sprite** (26 symbol, vẽ dòng — 0 phụ thuộc mạng, đúng nguyên tắc cục bộ) thay toàn bộ emoji trong nút/tiêu đề/toast; (2) **bộ token màu 2 giao diện sáng/tối** — giữ nguyên MỌI tên biến cũ (`--teal`, `--muted`…) vì mã JS nhúng `var(…)` trực tiếp; dark theo `prefers-color-scheme` lần đầu + nút đổi + lưu `localStorage("reva-theme")`, áp TRƯỚC khi vẽ (script đầu `<head>`, không nhấp nháy) + `color-scheme` cho điều khiển gốc; (3) nút chuẩn hoá 3 cỡ (34/40/28 px) + `spin`/`bico` hoán đổi khi bận; (4) hàng kết quả sắc nhẹ phẳng (viền trái 3 px + tint 3–4% thay gradient), grip chấm hiện khi rê khối, `#` thay nhãn ROW; (5) modal/drawer/toast/busy làm lại (icon đầu đề, nút đóng icon, toast có huy hiệu icon theo loại); (6) hỗ trợ `prefers-reduced-motion`, focus ring thống nhất 2 px, `scrollbar-width:thin`. **Kiểm**: agent-browser đầy đủ (ghép bài mẫu → 10 hàng/19 khối; lọc chip; tìm "VinFast" 1 hàng/2 khối hit; thay "Hà Nội"→"HANOI" 2 chỗ + Ctrl+Z nguyên xi; stats/QA/replace modal; dark toggle + đo contrast chip active 14.84:1; mobile 390 px không tràn ngang; console + page errors sạch), selftest **56/56** giữ nguyên, `cargo fmt` sạch, clippy 0 cảnh báo mới. Hợp đồng DOM giữ nguyên 79/79 ID + mọi class/data-attr JS dùng — 0 thay đổi API, 0 thay đổi định dạng phiên.
- **2.9** — **[QoL 2026-10-19, nghiên cứu nhu cầu biên dịch viên — `docs/PLAN-QOL-2026-10-19.md`]** hai tính năng theo kết quả nghiên cứu: **📊 Thống kê số từ** (tính năng #2 điểm 7,7/10): nút `📊 Số từ` trên thanh công cụ + `GET /api/sessions/{id}/stats` (thuần đọc, 0 snapshot, 0 mạng) đếm **số từ / ký tự / đoạn mỗi phía** — căn cứ tính giá theo **số từ nguồn** (chuẩn ngành); kèm bảng theo 4 trạng thái, phân vùng cặp-đủ-hai-phía / chỉ-Anh / chỉ-Việt, nút **📋 Sao chép** chuỗi tiếng Việt cho thư báo giá; **số từ hiện trực tiếp trên đầu ô nhập ngay khi dán** (client-side, trước khi ghép). Quy ước công khai: từ = tách theo khoảng trắng Unicode (NBSP là khoảng trắng; emoji ZWJ = 1 từ); ký tự = scalar value **sau chuẩn hoá NFC** (crate `unicode-normalization` sẵn có cho fold — 0 phụ thuộc mới; NFD/NFC cùng con số). **🔍 Bảng soát lỗi QA** (tính năng #1 điểm 7,4/10 — `src/qa.rs`): nút `🔍 Soát lỗi` + `GET /api/sessions/{id}/qa` (thuần đọc, 0 snapshot, 0 mạng, KHÔNG đụng embedder — rescan cos=1 không đổi báo cáo) chạy **8 kiểm theo danh mục chuẩn ngành ApSIC Xbench**: `numeric-mismatch` (chuẩn hoá số THEO PHÍA: EN `1,234.5`≡VI `1.234,5`; bắt dời thập phân `3.5`→`35`; số sau chữ "tháng" được tha KHI phía Anh có tên tháng — "March 12" → "12 tháng 3" là chuyển ngữ đúng), `target-same-as-source` (bỏ dịch), `length-outlier` (EN ≥ 40 ký tự mà tỉ lệ VI/EN ngoài 0.35–2.5), `double-space`, `unpaired-symbol` (cặp `() [] {} “” ‘’ «»` + `"` lẻ; `'` KHÔNG kiểm vì "don't"), `alluppercase-mismatch` (NASA mất), `missing-diacritics` (bảng ~17 từ KHÔNG nhập nhằng — cua/lam là từ hợp lệ), `nfd-mixed`. Mức Lỗi/Cần xem/Ghi chú; bấm mục → cuộn tới hàng + nháy viền (tự mở lại lọc chip/“Hiện thêm” nếu hàng đang ẩn); KHÔNG tự sửa gì; hàng một phía: kiểm 4/5/6/8 chạy phía có mặt, kiểm 1/2/3 cần đủ hai phía (audit #2: kiểm số liệu trên hàng một phía chỉ tạo báo oan thuần tuý sau khi tách cặp). **🔎 Tìm / lọc / thay thế hàng loạt** (tính năng #3 điểm 6,1/10): ô **Tìm trong bảng** trên thanh công cụ — lọc tức thì thuần client-side (không phân biệt hoa thường, tô nền vàng khối chứa chuỗi, đếm n/tổng hàng, nút ✕ xoá lọc); nút **✏️ Thay thế** + `POST /api/sessions/{id}/replace` `{find, replace, column: vi|en, caseSensitive}` — thay trong MỌI khối của cột chọn, **MỘT snapshot hoàn tác cho cả lô** (Ctrl+Z một nhấn trả nguyên xi — kể cả tài liệu gốc đồng bộ lại qua `sync_docs_from_rows`); tìm/thay theo KÝ TỰ (an toàn UTF-8: dấu Việt/emoji ZWJ; tìm NFC trong chuỗi NFD không khớp — quy ước công khai), không-phân-biệt-hoa-thường qua bản đồ chữ-thường↔ký-tự-gốc (0 phụ thuộc mới, không ReDoS vì KHÔNG regex — ghi ROADMAP); hàng bị thay mất điểm cũ (mời Rescan); 0 chỗ khớp → 200 changed:0 KHÔNG snapshot; rỗng-thuần/tìm==thay/cột lạ → 400 tiếng Việt; chuỗi chỉ toàn khoảng trắng ĐƯỢC thay (sửa được lỗi "2 dấu cách" — [AUDIT #2]). Selftest 41→45 (thống kê) →51 (QA) →**56** (+5 bài: 3 chỗ đúng nội dung + sync tài liệu gốc + 1 snapshot + undo/redo nguyên xi; hoa thường The/the/THE + chọn cột; đầu vào xấu; lô 10 chỗ/6 khối + hàng gộp 2 khối; biên Unicode + 10.000 hàng ~27 ms ≪ 1 s).
- **2.8.1** — **[SỬA SAU PHÁT HÀNH 2026-10-19, người dùng báo từ máy thật]** (chi tiết `docs/AUDIT-LOG.md` — phụ lục): selftest bài 28 & 41 phụ thuộc môi trường — trên máy CÓ LaBSE chạy thật → **39/41** (điểm rescan bị hoà trộn LaBSE `0.904` ≠ `0.760` thuần từ vựng; `labse_used=true` sai kỳ vọng) → tách `rescan_row_with`/`rescan_all_with` TIÊM nguồn cosine: bài kiểm chạy xác định với `None` (không LaBSE) lẫn `Some(1.0)` (LaBSE hoàn hảo) — đạt trên MỌI máy. Thanh trạng thái UI lấy số bản TỪ SERVER (`bản 2.8.1 · REVA`, hết ghi tay "bản 2.6" lỗi thời); UA tải URL từ `CARGO_PKG_VERSION` (hết "reva-tool/2.7" cũ). Selftest vẫn 41 bài (2 bài hermetic hoá + kiểm mạnh hơn: đường hoà trộn qua rescan thật).
- **2.8** — **[AUDIT 2026-10-18]** (xem `docs/AUDIT-LOG.md`): mặc định bind `127.0.0.1` (mở LAN: `REVA_BIND=0.0.0.0`); chống SSRF khi tải URL (chặn IP literal/DNS nội bộ + tự theo redirect ≤5 hop có kiểm an toàn từng hop); CSV/TSV chống chèn công thức (`=` `+` `-` `@` đầu ô thêm `'`); xuất bỏ ký tự điều khiển (XLIFF/HTML luôn hợp lệ). **Tính năng**: xuất **TMX 1.2** (định dạng 8 — bộ nhớ dịch nhập thẳng OmegaT/Trados); **Rescan toàn bộ** (`POST /api/sessions/{id}/rescan-all`, 1 snapshot hoàn tác cho cả lô); kéo–thả file `.txt` vào ô nhập (FileReader cục bộ, chặn >4 MB). Selftest 35→**41**.
- **2.7** — xuất **Excel `.xlsx` tự tô màu ô theo trạng thái hàng** (xanh `E2EFDA` = khớp + đã xác nhận, vàng `FFF2CC` = cần soát, hồng `FADCE3` = không khớp) bằng `rust_xlsxwriter`: cột STT/Tiếng Anh/Tiếng Việt/Điểm (%)/Trạng thái tiếng Việt, đóng băng dòng đầu, autofilter, chú giải màu dưới bảng; hàng gộp giữ từng đoạn một dòng trong cùng một ô. `ExportResult` thêm `bytes: Option<Vec<u8>>` (xlsx nhị phân), UI chuyển sang `res.blob()`. Selftest 33→35 bài (đọc lại file zip kiểm fill màu TỪNG HÀNG).
- **2.6** — **bộ trích URL 3 tầng lọc chrom**: (1) class/id tổ tiên chứa từ khoá giao diện (menu, nav, modal, popup, comment, share, related, newslette…), (2) "link farm" — mật độ ký tự nằm trong thẻ `<a>`, (3) quy tắc caption/credit ảnh trong `<figure>/<figcaption>` (bỏ credit ngắn, GIỮ caption dài) + **lưới an toàn 2 lượt** (0 đoạn → nới lỏng). **LaBSE không bao giờ tải lại**: model lưu `labse/model/` (env `REVA_LABSE_MODEL_DIR`), service nạp offline, script build không nâng cấp PyTorch khi đã cài. Flag CLI `reva-tool --extract <file.html>`.
- **2.5** — **mô hình khối kéo–thả tự do + hoàn tác**: mỗi đoạn văn là một khối; kéo vào ô cùng ngôn ngữ của HÀNG BẤT KỲ, đổi thứ tự tuỳ ý, thả vào thân hàng (hoặc bấm `⇥`) để tách thành hàng riêng. **Ctrl+Z hoàn tác** (40 bước gần nhất) bao trùm mọi thao tác. Nút **`↻ Rescan LaBSE`** chấm lại độ khớp từng hàng sau khi sắp xếp lại. Đổi tên từ Cánh đôi Anh–Việt.
- **2.4** — gộp hàng liền kề (API `/merge` vẫn giữ).
- **2.3** — sửa trích `<li>` khi tải URL + tô màu 3 mức + 3 nút đánh dấu + xuất CSV/HTML/XLIFF.
- **2.2** — thêm LaBSE (tuỳ chọn nhưng rất nên có).

## Tính năng

- **Ghép đoạn song ngữ**: chấm điểm từng cặp đoạn theo từ vựng (từ điển Anh–Việt tích hợp ~500 mục),
  số liệu (2025, 4.000 = 4,000), danh từ riêng (Hà Nội = Hanoi), bigram nối và vị trí tương đối —
  sau đó ghép đơn điệu bằng quy hoạch động. Không cần GPU.
- **LaBSE (tuỳ chọn, bản 2.2)**: nếu dịch vụ LaBSE đang chạy, điểm mỗi cặp còn được **hoà trộn với cosine LaBSE**
  (công khai, chỉ nâng không hạ — xem bên dưới) → ghép đúng hơn hẳn với bài dịch thoáng/lược bớt.
- **4 trạng thái chuẩn** (giữ nguyên quy ước bản gốc): `auto_matched` (Tự khớp) · `needs_review` (Cần soát) ·
  `unmatched` (Không khớp) · `human_confirmed` (Đã xác nhận).
- **Mô hình khối kéo–thả tự do (bản 2.5)**: sau khi ghép, mỗi đoạn là **một khối 🧩** (co giãn theo độ lớn văn bản):
  * **Kéo sang ô cùng ngôn ngữ của hàng bất kỳ** — thả giữa hai khối có vạch chỉ vị trí; thả vào ô `none` trống để ghép cặp mồ côi;
  * **Đổi thứ tự tuỳ ý** trong cùng ô (kéo lên trước / sau) và giữa các hàng — thứ tự xuất file theo đúng thứ tự khối trên màn hình;
  * **Tách riêng**: thả khối vào thân hàng khác, bấm nút `⇥` trên khối, hoặc bấm chọn khối rồi bấm thân hàng (đường cảm ứng);
  * **Hoàn tác / làm lại**: `Ctrl+Z` / `Ctrl+Shift+Z` (hoặc nút `↩`/`↪` trên thanh công cụ) — 40 bước gần nhất,
    bao gồm mọi thao tác: kéo khối, tách, gộp, đánh dấu, rescan;
  * Hàng mất khối cuối cùng **tự biến mất**; hàng vừa đổi nội dung về `Cần soát` + xoá điểm cũ → luôn biết chỗ cần soát lại.
- **Rescan LaBSE từng hàng (bản 2.5)**: nút `↻ Rescan LaBSE` trên mỗi hàng chấm lại điểm khớp sau khi kéo–thả —
  ghép các khối mỗi phía thành 1 văn bản, chấm bằng bộ từ vựng (giữ idf toàn phiên) + hoà trộn cosine LaBSE nếu dịch vụ chạy,
  cập nhật điểm + trạng thái theo ngưỡng. Không có LaBSE thì vẫn chấm được bằng từ vựng (giao diện báo rõ).
- **Phiên làm việc**: mỗi lần ghép tạo **một phiên mới độc lập** — ghép bao nhiêu bài liên tiếp cũng không
  bị "bẩn" dữ liệu nhau. Phiên được lưu tự động vào `data/sessions/*.json` (phiên cũ của bản ≤2.4 vẫn mở được).
- **Nhả phiên / phiên mới**: nút "Phiên mới" để bắt đầu sạch; drawer "Các phiên" để mở lại hoặc nhả (xoá) phiên cũ;
  có "Nhả tất cả".
- **Sửa tay từng cặp**: xác nhận, đánh dấu cần soát, **bỏ ghép (tách cặp thành các hàng `none`)**, trở về tự động.
- **Kéo–thả gộp hàng liền kề (bản 2.4, API `/merge`)**: gộp hàng kế tiếp vào hàng này — vẫn dùng được qua API;
  trên giao diện bản 2.5 dùng thao tác kéo khối thay thế (linh hoạt hơn và hoàn tác được).
- **Xuất file 8 định dạng**: TSV (tab), Moses (`văn bản ||| văn bản`), JSON, **CSV (mở được bằng Excel, kèm BOM UTF-8 + chống chèn công thức: ô bắt đầu = + - @ thêm `'`)**, **HTML (bảng đối chiếu tô màu theo trạng thái)**, **XLIFF 1.2 (cho công cụ dịch/CAT)**, **Excel `.xlsx` (bản 2.7 — tự tô màu ô theo trạng thái)** và **TMX 1.2 (bản 2.8 — bộ nhớ dịch nhập thẳng OmegaT/Trados; trạng thái/điểm nằm trong `<prop>`)**. Mọi định dạng ghi ký tự điều khiển đã làm sạch (XLIFF/TMX luôn hợp lệ XML). Đoạn thiếu một phía ghi đúng
  chữ `none` — đúng quy ước bản gốc. **Hàng nhiều khối**: TSV/Moses/XLIFF nối đoạn bằng dấu cách; **CSV và .xlsx giữ
  từng đoạn một dòng trong cùng một ô** (Excel mở vẫn đúng); JSON kèm `enParts`/`viParts`; HTML ghi rõ từng đoạn gốc.
  Xuất theo đúng **thứ tự khối đã sắp** (đổi thứ tự tuỳ ý — duy trì bất biến "mỗi đoạn đúng 1 lần").
- **Tải bài từ URL** (`<p>` + mục danh sách `<li>` theo thứ tự tài liệu, đã lọc chrom trang): dán link bài tiếng Anh/tiếng Việt,
  công cụ tải và trích đoạn; tự **lược bỏ chrom giao diện trang** (dòng "Published…", "Share this with…", lời mời đăng ký
  bản tin, địa chỉ bưu chính…) — chỉ giữ nội dung thật của bài. **[AUDIT 2026-10-18] chống SSRF**: chỉ tải máy chủ công khai —
  chặn IP literal/DNS resolves về nội bộ (loopback, private, link-local, ULA IPv6, CGNAT, 0.0.0.0/8), tự theo redirect
  ≤5 hop và kiểm an toàn TỪNG hop (xem `docs/AUDIT-LOG.md`).
- **Tô màu 3 mức theo trạng thái** (bản 2.3): hàng **xanh** = khớp/khớp phần lớn (tự khớp + đã xác nhận), hàng **vàng** = cần soát/khớp một phần, hàng **đỏ** = không khớp hoàn toàn — áp cả trong giao diện lẫn file HTML xuất ra; từ bản 2.7 file **Excel `.xlsx`** cũng tự tô màu từng hàng (xanh nhạt/vàng nhạt/hồng nhạt).
- **Đúng 3 nút đánh dấu trên mỗi hàng** (bản 2.3): `✓ Khớp` (xanh) · `⚠ Soát` (vàng) · `✕ Không khớp` (đỏ, tách cặp thành các hàng `none`) — bấm xong xuất file là xong.
- **📊 Thống kê số từ (bản 2.9)**: đếm **số từ / ký tự / đoạn** mỗi phía cho căn cứ tính giá (chuẩn ngành trả phí theo
  **số từ nguồn**); bảng theo 4 trạng thái + phân vùng cặp đủ hai phía / chỉ-Anh / chỉ-Việt; nút **📋 Sao chép**
  chuỗi tiếng Việt cho thư báo giá; **số từ hiện trực tiếp trên đầu ô nhập khi gõ/dán** (client-side). Quy ước
  công khai: từ = tách theo khoảng trắng Unicode (NBSP tính là khoảng trắng, emoji ghép ZWJ = 1 từ);
  ký tự = scalar value **sau chuẩn hoá NFC** — NFD (macOS) và NFC (Windows) đếm ra cùng con số. Thuần đọc.
- **🔍 Bảng soát lỗi QA (bản 2.9)**: 8 kiểm theo danh mục chuẩn ngành ApSIC Xbench — sai số liệu (chuẩn hoá THEO PHÍA, bắt dời thập phân, tha số tháng 1..=12 khi EN có tên tháng — riêng "may" chỉ tính khi viết hoa), bỏ dịch (VI == EN), độ dài bất thường (ngoài 0.35–2.5 khi EN ≥ 40 ký tự), khoảng trắng thừa, ngoặc không cân, tên riêng viết hoa lệch, có thể thiếu dấu tiếng Việt (bảng từ không nhập nhằng), Unicode NFD lẫn. Mức **Lỗi / Cần xem / Ghi chú**; bấm mục → cuộn tới hàng + nháy viền; hàng một phía: kiểm 4/5/6/8 chạy phía có mặt, kiểm 1/2/3 cần đủ hai phía (audit #2: số liệu trên hàng một phía chỉ báo oan). Thuần đọc tuyệt đối: không snapshot, không đổi `updated_at`, **không đụng embedder** (rescan cos=1 không đổi báo cáo — kiểm trong selftest).
- **🔎 Tìm / lọc / thay thế hàng loạt (bản 2.9)**: ô **Tìm trong bảng** lọc tức thì (client-side, không phân biệt hoa thường, tô nền vàng khối khớp, đếm n/tổng hàng); **✏️ Thay thế** áp MỘT lần cho cả cột (vi/en, tuỳ chọn hoa thường) — **1 snapshot hoàn tác cả lô** (Ctrl+Z trả nguyên xi kể cả tài liệu gốc); tìm theo ký tự an toàn UTF-8 (0 phụ thuộc mới, không regex → không ReDoS); 0 khớp không đổi gì; đầu vào xấu → 400 tiếng Việt.
- **Tự kiểm tra (selftest)**: 58 bài kiểm nội bộ, gồm 4 bài **hồi quy** đúng 4 lỗi từng gặp
  (panic `index 61`, "không ghép được bài thứ hai", "nút Ghép bị mờ sau khi tải URL", "tải URL mất mục `<li>`"),
  1 bài **tích hợp LaBSE**, 4 bài **gộp hàng** (bản 2.4), 5 bài **mô hình khối** (bản 2.5),
  **3 bài bộ lọc chrom** (bản 2.6): mega-menu/ribbon/modal/comment theo class (tuoitre.vn),
  caption/credit ảnh, "link farm" không cần class, và **2 bài Excel `.xlsx`** (bản 2.7 — đọc lại file zip kiểm fill màu từng hàng + hàng gộp):
  kéo khối + hoàn tác/làm lại nguyên xi, tách khối + prune hàng rỗng, Rescan (công thức + ngưỡng + từ chối một phía),
  xuất sau khi tráo chéo khối (tha thứ đơn điệu, "mỗi đoạn 1 lần" vẫn gắt), đầu vào xấu của `move_block`,
  và **4 bài thống kê số từ** (bản 2.9 — đếm tay corpus, biên NFD/NFC/CRLF/ZWJ, phân vùng theo tách cặp, thuần đọc + 10.000 đoạn < 300 ms).

## LaBSE — mấu chốt để ghép chính xác

LaBSE là model nhúng câu đa ngôn ngữ của Google. Từ bản **2.10** LaBSE chạy **ngay trong binary**
(crate `ort` nạp động ONNX Runtime + crate `tokenizers`) — không cần Python, không cần cài gì thêm.
Bộ từ vựng giỏi bài báo giàu số/tên riêng; LaBSE giỏi **mọi văn bản** vì "hiểu nghĩa" câu.

- **Khi có LaBSE**: điểm mỗi cặp = `max(điểm_từ_vựng, 0.4×điểm_từ_vựng + 0.6×tin_cậy_LaBSE)` với
  `tin_cậy = (cosine − 0.50) / 0.30` kẹp vào [0,1] — đo thực tế: cặp dịch đúng 1:1 có cosine ~0.85–0.92,
  cặp "lược/gộp đoạn" ~0.5–0.65, cặp cùng chủ đề nhưng không dịch ~0.35–0.55.
  Nguyên tắc **"chỉ nâng, không hạ"**: điểm từ vựng là sàn — bài dịch lược bớt vẫn ghép được như thường,
  còn cặp dịch đúng được nâng lên "Tự khớp" và DP gán đúng cặp nhờ khoảng cách cosine rõ rệt.
- **Khi không có LaBSE** (chưa tải xong, không mạng, tải lỗi): app tự fallback về thuần từ vựng —
  huy hiệu trên đầu báo "Chỉ từ vựng" (chấm hổ phách) kèm tiến trình % hoặc nút thử lại.
  Ghép vẫn chạy 100%; nút **Rescan** cũng vẫn chấm được bằng từ vựng (giao diện báo rõ "từ vựng").
- **Native in-process (bản 2.10)**: `src/labse_native.rs` — tự tải model ONNX int8 (`Xenova/LaBSE`,
  ~450 MB) + `tokenizer.json` (~13 MB) + thư viện ONNX Runtime 1.31.1 vào `data/models/`
  (đúng 1 lần, HTTP Range resume, retry 6 lần); nạp trên **thread nền**, smoke test 2 câu rồi mới Ready.
  Embed: truncate/pad 256 token → tensor i64 → output ưu tiên `embeddings` → `pooler_output` →
  mean-pool `last_hidden_state` theo attention mask; **L2-chuẩn hoá**; chia lô 16.
  Env: `REVA_LABSE_AUTO=0` tắt tự tải; `REVA_LABSE_MAXLEN` đổi số token tối đa (16..=512);
  `POST /api/labse/retry` tải lại khi Failed; `/api/model-status` có `labse.native`.
- **Dịch vụ HTTP kiểu cũ vẫn được hỗ trợ** (ai đã có `labse/labse_service.py`): đặt
  `REVA_LABSE_URL` (vd `http://127.0.0.1:3031`) — khi biến này được đặt TƯỜNG MINH, app dùng
  đường HTTP đó và không tự tải native (tên cũ `CANH_DOI_LABSE_URL` vẫn được chấp nhận).
  Giao thức: `GET /health`, `POST /embed {"texts":[…]}` → `{"embeddings":[[…],…]}` (đã chuẩn hoá L2).
- **Không bao giờ tải lại model**: model nằm trong **`data/models/labse/`** — build lại mã nguồn bao
  nhiêu lần cũng không chạm mạng lại; xoá thư mục này nếu muốn tải lại từ đầu.
- Mỗi phiên lưu kèm `engine` (`lexical` / `lexical+labse`) — mở lại phiên cũ vẫn thấy đúng động cơ đã dùng.

*(Từ bản 2.10 không còn lưu ý cài PyTorch/CUDA: LaBSE native chạy CPU qua ONNX Runtime — không Python, không GPU.)*

## Build từ nguồn (thủ công, không dùng script)

Chỉ Rust là bắt buộc ([rustup](https://rustup.rs) 1.75+):

```bash
cd en-vi-align-rust
cargo run --release          # build rồi chạy tại http://localhost:3000
```

Đổi cổng:

```bash
PORT=1709 cargo run --release        # Linux/macOS
set PORT=1709 && cargo run --release # Windows (cmd)
```

**Đổi địa chỉ lắng nghe (bản 2.8)**: mặc định server chỉ mở `127.0.0.1` (máy của chính bạn — đúng
lời hứa "100% cục bộ"). Muốn máy khác trong mạng LAN dùng chung:

```bash
REVA_BIND=0.0.0.0 cargo run --release   # mở cho cả LAN (banner sẽ cảnh báo)
```

Thư mục dữ liệu (session) mặc định nằm cạnh binary (`data/`); ghi đè bằng biến môi trường
`REVA_DATA_DIR` (tên cũ `CANH_DOI_DATA_DIR` vẫn được chấp nhận).

Binary sau khi build nằm ở `target/release/reva-tool` (Linux/macOS)
hoặc `target/release/reva-tool.exe` (Windows) — copy đi đâu cũng chạy được, không kèm gì thêm.
Muốn thêm LaBSE thì chạy riêng dịch vụ:

```bash
python3 -m venv labse-venv && labse-venv/bin/pip install sentence-transformers
labse-venv/bin/python labse/labse_service.py    # giữ nguyên cửa sổ này, mở app ở cửa sổ khác
```

## Cấu trúc

```
en-vi-align-rust/
├── build-and-run-windows.bat  # file lệnh build+chạy cho Windows
├── build-and-run-macos.sh     # file lệnh build+chạy cho macOS
├── build-and-run-linux.sh     # file lệnh build+chạy cho Linux
├── Cargo.toml                 # khai báo package + dependencies (axum, tokio, serde, ureq…) — binary reva-tool
├── static/index.html          # giao diện tiếng Việt (nhúng sẵn vào binary khi build)
├── docs/
│   ├── README-KY-THUAT.md     # tài liệu kỹ thuật (file này)
│   └── images/                # ảnh chụp màn hình cho README
├── labse/                     # dịch vụ nhúng LaBSE (Python, tuỳ chọn)
│   ├── labse_service.py       #   lắng nghe 127.0.0.1:3031 — /health, /embed (ưu tiên nạp offline từ labse/model)
│   ├── download_model.py      #   đảm bảo model nằm trong labse/model/ — tải 1 LẦN duy nhất, có rồi thì thoát ngay
│   ├── wait_health.py         #   chờ service sẵn sàng (script Windows)
│   └── start-labse-windows.bat#   khởi động service kèm log (script Windows)
├── src/
│   ├── main.rs                # server axum + toàn bộ API + banner
│   ├── aligner.rs             # bộ ghép: chấm điểm + DP đơn điệu + hoà trộn LaBSE + rescore_pair (bản 2.5)
│   ├── embedder.rs            # LaBSE: HTTP client + cosine + pipeline /api/align & selftest + pair_cosine (rescan)
│   ├── session.rs             # phiên: tạo / mở / cập nhật / GỘP HÀNG / TÁCH / move_block / UNDO-REDO / rescan_row / THAY THẾ HÀNG LOẠT (bản 2.9) / NHẢ PHIÊN
│   ├── exporter.rs            # xuất TSV / Moses / JSON / CSV / HTML / XLIFF / Excel .xlsx (tô màu ô) / TMX 1.2 với chữ `none` nguyên văn (hỗ trợ nhiều khối/hàng)
│   ├── fetcher.rs             # tải URL + trích HTML
│   ├── selftest.rs            # 58 bài tự kiểm (4 hồi quy + LaBSE + xuất mới + gộp hàng + mô hình khối + xlsx màu + 4 audit + TMX + rescan-all + 4 thống kê số từ + 6 soát lỗi QA + 5 tìm/thay thế + 2 LaBSE native)
│   ├── lexicon.rs             # từ điển Anh→Việt + danh sách từ phổ biến
│   ├── stats.rs               # BẢN 2.9 — thống kê số từ/ký tự/đoạn (thuần đọc, đếm sau NFC)
│   ├── qa.rs                  # BẢN 2.9 — bảng soát lỗi QA 8 kiểm (thuần đọc, không đụng embedder)
│   └── fold.rs                # bỏ dấu tiếng Việt ("Hà Nội" → "ha noi"), chuẩn hoá số
└── data/sessions/             # (tạo lúc chạy) nơi lưu các phiên
```

## API (tất cả trả JSON `{ok:true,data:…}` hoặc `{ok:false,error:…}`)

| Phương thức | Đường dẫn | Chức năng |
|---|---|---|
| POST | `/api/align` | Ghép 2 văn bản → tạo PHIÊN MỚI (tự dùng LaBSE nếu đang chạy) |
| GET | `/api/sessions` | Danh sách phiên |
| GET | `/api/sessions/{id}` | Chi tiết 1 phiên (kèm `canUndo` / `canRedo`) |
| GET | `/api/sessions/{id}/stats` | **📊 Thống kê số từ / ký tự / đoạn** (thuần đọc; từ/ký tự/đoạn mỗi phía, byStatus, phân vùng paired/enOnly/viOnly) · bản 2.9 |
| GET | `/api/sessions/{id}/qa` | **🔍 Soát lỗi QA 8 kiểm** (thuần đọc; `{total, bySeverity, issues:[{row,severity,check,en,vi,detail}]}` — Lỗi/Cần xem/Ghi chú, bấm mục cuộn tới hàng) · bản 2.9 |
| POST | `/api/sessions/{id}/replace` | **✏️ Thay thế hàng loạt** `{find, replace, column: vi\|en, caseSensitive}` — 1 snapshot hoàn tác cả lô; 0 khớp → changed:0 không snapshot · bản 2.9 |
| DELETE | `/api/sessions/{id}` | **Nhả phiên** |
| DELETE | `/api/sessions` | Nhả tất cả |
| PATCH | `/api/sessions/{id}/rows/{index}` | Đổi trạng thái 1 hàng (bỏ ghép sẽ tách cặp) |
| POST | `/api/sessions/{id}/rows/{index}/merge` | Gộp hàng kế tiếp vào hàng này (API bản 2.4, vẫn giữ) |
| POST | `/api/sessions/{id}/rows/{index}/rescan` | **Rescan — chấm lại điểm hàng** (LaBSE nếu chạy + sàn từ vựng) · bản 2.5 |
| POST | `/api/sessions/{id}/rescan-all` | **Rescan toàn bộ** — chấm lại mọi hàng đủ hai phía, 1 snapshot hoàn tác · bản 2.8 |
| POST | `/api/sessions/{id}/blocks/move` | **Di chuyển khối** `{side, fromRow, fromPos, toRow, toPos, afterRow}` — `toRow: null` = tách thành hàng riêng · bản 2.5 |
| POST | `/api/sessions/{id}/undo` | **Hoàn tác** bước gần nhất (40 bước) · bản 2.5 |
| POST | `/api/sessions/{id}/redo` | **Làm lại** bước vừa hoàn tác · bản 2.5 |
| POST | `/api/export` | Xuất file (`tsv` / `moses` / `json` / `csv` / `html` / `xliff` / `xlsx` / `tmx`) |
| POST | `/api/fetch-url` | Tải bài từ URL |
| GET | `/api/selftest` | Chạy 58 bài tự kiểm |
| GET | `/api/model-status` | Trạng thái động cơ + LaBSE (`labse.ready`, `labse.url`) |
| GET | `/api/health` | Kiểm tra sống |

## 5 bất biến (được kiểm tra tự động)

1. Mỗi đoạn tiếng Anh được dùng **đúng 1 lần** trong kết quả (kể cả sau kéo–thả / gộp / tách).
2. Mỗi đoạn tiếng Việt được dùng **đúng 1 lần**.
3. Ghép **đơn điệu** — không giao nhau, giữ thứ tự bài gốc. *(Bản 2.5: khi người dùng CÓ CHỦ ĐÍCH kéo–thả đổi thứ tự
   khối giữa các hàng, phiên được đánh dấu `reordered` và duy nhất bất biến này được tha thứ lúc xuất — mọi bất biến
   khác vẫn kiểm gắt.)*
4. Trạng thái chỉ thuộc 4 giá trị chuẩn; `unmatched` ⟺ hàng thiếu đúng một phía
   (`human_confirmed` một phía = người dùng xác nhận "không có bản dịch").
5. Khi xuất, phía thiếu ghi đúng chữ **`none`** (nguyên văn) và **không bỏ sót đoạn nào**.

(LaBSE chỉ đổi **điểm gốc** trước khi vào DP — 5 bất biến và toàn bộ quy ước trạng thái giữ nguyên, được kiểm lại
trong selftest 58 bài kể cả khi có LaBSE. Rescan bản 2.5 dùng lại đúng lõi chấm điểm của bộ ghép — không thêm thuật toán mới.)

## Sửa lỗi so với các bản trước

| Lỗi | Nguyên nhân | Cách sửa |
|---|---|---|
| `panic: index out of bounds: the len is 61 but the index is 18446744073709551615` (bản 1.x) | Sentinel `-1` cast sang `usize` rồi dùng làm chỉ số mảng | Toàn bộ aligner viết lại với `Option<usize>`, ma trận phẳng, backtrack có nhánh riêng cho biên — **không còn phép trừ chỉ số nào có thể tràn**; kèm bài hồi quy `07. [REGRESSION] panic index 61` |
| Ghép bài 1 xuất tốt, bài 2 "Không ghép được." (bản 1.x) | Trạng thái rò rỉ giữa các lần ghép (panic trên worker thread làm chết request) | Mỗi lần ghép là **phiên mới độc lập**, bộ ghép **vô trạng thái**, panic được `catch_unwind` bắt lại trả lỗi JSON — server không bao giờ chết; kèm bài hồi quy `08. [REGRESSION]` |
| **Tải 2 bài từ URL xong, nút "⚡ Ghép đoạn" bị mờ, không bấm được** (bản 2.0) | Trong `fetchUrl()`, hàm đếm đoạn chạy lúc trạng thái "đang bận" vẫn bật → nút bị khoá; sau đó không còn đường dẫn nào mở khoá lại | Tách hàm `syncAlignBtn()` làm **một nguồn sự thật duy nhất** cho trạng thái nút; mọi vòng "bận" (tải URL, ghép…) kết thúc đều tự đồng bộ lại; kèm bài hồi quy `18. [REGRESSION]` và lọc chrom giao diện khi tải URL |
| **Bảng kết quả nhiều hàng → mỗi hàng bị nén cực hẹp (~12–16px), chữ bị cắt mất** (bản 2.1) | Khung `.rows` là flex-cột có `max-height` — các hàng mặc định `flex-shrink:1` nên bị **nén lại** cho vừa khung thay vì để khung cuộn | `.rows > *{flex:none}` — hàng giữ chiều cao tự nhiên, khung cuộn dọc như dự kiến; kèm chỉnh bố cục mobile (thanh giữa trải nguyên hàng, không còn rơi vào cột 28px) |
| **Kéo nhầm hàng không hoàn tác được** (bản 2.4 — thao tác gộp hàng là một chiều) | Gộp hàng cắt xé cấu trúc, không có ngăn xếp lịch sử | **Bản 2.5**: mô hình khối (mỗi đoạn = 1 khối kéo tự do) + **ngăn xếp hoàn tác 40 bước** (Ctrl+Z) bao trùm mọi thao tác; kéo nhầm chỉ là kéo lại — hoặc 1 phím Ctrl+Z |

## Giấy phép

MIT — xem `LICENSE` (ở gốc repo).
