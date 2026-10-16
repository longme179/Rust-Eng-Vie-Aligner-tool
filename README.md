# Cánh đôi Anh–Việt (`canh-doi-anh-viet`) v2.3

Công cụ **ghép đoạn văn bản Anh–Việt** (chỉ ghép, không dịch) đóng gói thành **một chương trình Rust duy nhất**:
giao diện + API + bộ ghép nằm trong 1 file binary tự build, chạy **100% cục bộ** trên máy bạn — không gửi dữ liệu đi đâu.

**Bản 2.3 sửa lỗi lớn khi tải URL: mục danh sách (gạch đầu dòng `<li>`) giờ được lấy đủ**, kèm tô màu 3 mức và 3 nút đánh dấu, xuất thêm CSV/HTML/XLIFF.

**Bản 2.2 thêm LaBSE (tuỳ chọn nhưng rất nên có)**: mấu chốt cho độ chính xác ghép cặp. Xem mục [LaBSE](#labse-mấu-chốt-để-ghép-chính-xác) bên dưới.

```
[canh-doi-anh-viet] chạy tại http://localhost:3000 (giao diện + API, tất cả cục bộ)
[canh-doi-anh-viet] LaBSE (tuỳ chọn): http://127.0.0.1:3031 — chạy labse/labse_service.py để bật chế độ chính xác cao
```

## Tính năng

- **Ghép đoạn song ngữ**: chấm điểm từng cặp đoạn theo từ vựng (từ điển Anh–Việt tích hợp ~500 mục),
  số liệu (2025, 4.000 = 4,000), danh từ riêng (Hà Nội = Hanoi), bigram nối và vị trí tương đối —
  sau đó ghép đơn điệu bằng quy hoạch động. Không cần GPU.
- **LaBSE (tuỳ chọn, bản 2.2)**: nếu dịch vụ LaBSE đang chạy, điểm mỗi cặp còn được **hoà trộn với cosine LaBSE**
  (công khai, chỉ nâng không hạ — xem bên dưới) → ghép đúng hơn hẳn với bài dịch thoáng/lược bớt.
- **4 trạng thái chuẩn** (giữ nguyên quy ước bản gốc): `auto_matched` (Tự khớp) · `needs_review` (Cần soát) ·
  `unmatched` (Không khớp) · `human_confirmed` (Đã xác nhận).
- **Phiên làm việc**: mỗi lần ghép tạo **một phiên mới độc lập** — ghép bao nhiêu bài liên tiếp cũng không
  bị "bẩn" dữ liệu nhau. Phiên được lưu tự động vào `data/sessions/*.json`.
- **Nhả phiên / phiên mới**: nút "Phiên mới" để bắt đầu sạch; drawer "Các phiên" để mở lại hoặc nhả (xoá) phiên cũ;
  có "Nhả tất cả".
- **Sửa tay từng cặp**: xác nhận, đánh dấu cần soát, **bỏ ghép (tách cặp thành 2 hàng `none`)**, trở về tự động.
- **Xuất file 6 định dạng**: TSV (tab), Moses (`văn bản ||| văn bản`), JSON, **CSV (mở được bằng Excel, kèm BOM UTF-8)**, **HTML (bảng đối chiếu tô màu theo trạng thái)**, **XLIFF 1.2 (cho công cụ dịch/CAT)**. Đoạn thiếu một phía ghi đúng
  chữ `none` — đúng quy ước bản gốc.
- **Tải bài từ URL** (p + mục danh sách `<li>` theo thứ tự tài liệu, đã lọc chrom trang): dán link bài tiếng Anh/tiếng Việt, công cụ tải và trích các đoạn `<p>`;
  tự **lược bỏ chrom giao diện trang** (dòng “Published…”, “Share this with…”, lời mời đăng ký bản tin,
  địa chỉ bưu điện…) — chỉ giữ nội dung thật của bài.
- **Tô màu 3 mức theo trạng thái** (bản 2.3): hàng **xanh** = khớp/khớp phần lớn (tự khớp + đã xác nhận), hàng **vàng** = cần soát/khớp một phần, hàng **đỏ** = không khớp hoàn toàn — áp cả trong giao diện lẫn file HTML xuất ra.
- **Đúng 3 nút đánh dấu trên mỗi hàng** (bản 2.3): `✓ Khớp` (xanh) · `⚠ Soát` (vàng) · `✕ Không khớp` (đỏ, tách cặp thành 2 hàng `none`) — bấm xong xuất file là xong.
- **Tự kiểm tra (selftest)**: 21 bài kiểm nội bộ, gồm 4 bài **hồi quy** đúng 4 lỗi từng gặp
  (panic `index 61`, "không ghép được bài thứ hai", "nút Ghép bị mờ sau khi tải URL") và 1 bài **tích hợp LaBSE**.

## Chạy nhanh — 1 lệnh cho mỗi hệ điều hành

Người dùng chỉ cần mang thư mục mã nguồn này vào terminal và chạy **file lệnh** tương ứng hệ điều hành:

| Hệ điều hành | Lệnh (trong thư mục `en-vi-align-rust`) |
|---|---|
| **Windows** | nháy đúp `build-and-run-windows.bat` (hoặc chạy nó trong CMD/PowerShell) |
| **macOS** | `bash build-and-run-macos.sh` |
| **Linux** | `bash build-and-run-linux.sh` |

File lệnh tự làm 5 việc: cài Rust nếu thiếu → biên dịch app → tạo môi trường Python riêng (`labse-venv`)
+ cài `sentence-transformers` → tải model LaBSE (~2 GB, chỉ lần đầu, có cache) → khởi động LaBSE + app
và mở sẵn `http://localhost:3000`. Chạy lại lần sau thì gần như tức thì (bỏ qua phần đã cài).
Nhấn **Ctrl+C** để dừng. Không muốn LaBSE? Vẫn chạy bình thường — xem bên dưới.

## LaBSE — mấu chốt để ghép chính xác

LaBSE là model nhúng câu đa ngôn ngữ của Google (chạy cục bộ qua `sentence-transformers`).
Bộ từ vựng giỏi bài báo giàu số/tên riêng; LaBSE giỏi **mọi văn bản** vì "hiểu nghĩa" câu.

- **Khi có LaBSE**: điểm mỗi cặp = `max(điểm_từ_vựng, 0.4×điểm_từ_vựng + 0.6×tin_cậy_LaBSE)` với
  `tin_cậy = (cosine − 0.50) / 0.30` kẹp vào [0,1] — đo thực tế: cặp dịch đúng 1:1 có cosine ~0.85–0.92,
  cặp "lược/gộp đoạn" ~0.5–0.65, cặp cùng chủ đề nhưng không dịch ~0.35–0.55.
  Nguyên tắc **"chỉ nâng, không hạ"**: điểm từ vựng là sàn — bài dịch lược bớt vẫn ghép được như thường,
  còn cặp dịch đúng được nâng lên "Tự khớp" và DP gán đúng cặp nhờ khoảng cách cosine rõ rệt.
- **Khi không có LaBSE** (không cài Python, không mạng…): app tự fallback về thuần từ vựng —
  huy hiệu trên đầu báo "Chỉ từ vựng — chưa bật LaBSE" (chấm hổ phách). Ghép vẫn chạy 100%.
- Giao thức: `labse/labse_service.py` lắng nghe tại `127.0.0.1:3031`;
  `GET /health`, `POST /embed {"texts":[…]}` → `{"embeddings":[[…],…]}` (đã chuẩn hoá L2).
  Đổi địa chỉ bằng biến môi trường `CANH_DOI_LABSE_URL`.
- Mỗi phiên lưu kèm `engine` (`lexical` / `lexical+labse`) — mở lại phiên cũ vẫn thấy đúng động cơ đã dùng.

**Lưu ý Linux không có card NVIDIA**: `pip install sentence-transformers` kéo bản PyTorch kèm CUDA (~2.5 GB).
Muốn gọn hơn, chạy trước `labse-venv/bin/pip install torch --index-url https://download.pytorch.org/whl/cpu`
rồi chạy lại script — LaBSE chạy CPU được, chỉ chậm hơn.

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

Binary sau khi build nằm ở `target/release/canh-doi-anh-viet` (Linux/macOS)
hoặc `target/release/canh-doi-anh-viet.exe` (Windows) — copy đi đâu cũng chạy được, không kèm gì thêm.
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
├── Cargo.toml                 # khai báo package + dependencies (axum, tokio, serde, ureq…)
├── static/index.html          # giao diện tiếng Việt (nhúng sẵn vào binary khi build)
├── labse/                     # dịch vụ nhúng LaBSE (Python, tuỳ chọn)
│   ├── labse_service.py       #   lắng nghe 127.0.0.1:3031 — /health, /embed
│   ├── download_model.py      #   tải model LaBSE về cache (gọi bởi script build)
│   ├── wait_health.py         #   chờ service sẵn sàng (script Windows)
│   └── start-labse-windows.bat#   khởi động service kèm log (script Windows)
├── src/
│   ├── main.rs                # server axum + toàn bộ API + banner
│   ├── aligner.rs             # bộ ghép: chấm điểm + DP đơn điệu + hoà trộn LaBSE (an toàn index tuyệt đối)
│   ├── embedder.rs            # LaBSE: HTTP client + cosine + pipeline dùng chung /api/align & selftest
│   ├── session.rs             # phiên: tạo / mở / cập nhật / NHẢ PHIÊN / lưu JSON (kèm engine)
│   ├── exporter.rs            # xuất TSV / Moses / JSON với chữ `none` nguyên văn
│   ├── fetcher.rs             # tải URL + trích HTML
│   ├── selftest.rs            # 21 bài tự kiểm (4 hồi quy + LaBSE + xuất mới)
│   ├── lexicon.rs             # từ điển Anh→Việt + danh sách từ phổ biến
│   └── fold.rs                # bỏ dấu tiếng Việt ("Hà Nội" → "ha noi"), chuẩn hoá số
└── data/sessions/             # (tạo lúc chạy) nơi lưu các phiên
```

## API (tất cả trả JSON `{ok:true,data:…}` hoặc `{ok:false,error:…}`)

| Phương thức | Đường dẫn | Chức năng |
|---|---|---|
| POST | `/api/align` | Ghép 2 văn bản → tạo PHIÊN MỚI (tự dùng LaBSE nếu đang chạy) |
| GET | `/api/sessions` | Danh sách phiên |
| GET | `/api/sessions/{id}` | Chi tiết 1 phiên |
| DELETE | `/api/sessions/{id}` | **Nhả phiên** |
| DELETE | `/api/sessions` | Nhả tất cả |
| PATCH | `/api/sessions/{id}/rows/{index}` | Đổi trạng thái 1 hàng (bỏ ghép sẽ tách cặp) |
| POST | `/api/export` | Xuất file (`tsv` / `moses` / `json`) |
| POST | `/api/fetch-url` | Tải bài từ URL |
| GET | `/api/selftest` | Chạy 21 bài tự kiểm |
| GET | `/api/model-status` | Trạng thái động cơ + LaBSE (`labse.ready`, `labse.url`) |
| GET | `/api/health` | Kiểm tra sống |

## 5 bất biến (được kiểm tra tự động)

1. Mỗi đoạn tiếng Anh được dùng **đúng 1 lần** trong kết quả.
2. Mỗi đoạn tiếng Việt được dùng **đúng 1 lần**.
3. Ghép **đơn điệu** — không giao nhau, giữ thứ tự bài gốc.
4. Trạng thái chỉ thuộc 4 giá trị chuẩn; `unmatched` ⟺ hàng thiếu đúng một phía.
5. Khi xuất, phía thiếu ghi đúng chữ **`none`** (nguyên văn) và **không bỏ sót đoạn nào**.

(LaBSE chỉ đổi **điểm gốc** trước khi vào DP — 5 bất biến và toàn bộ quy ước trạng thái giữ nguyên, được kiểm lại trong selftest 21 bài kể cả khi có LaBSE.)

## Sửa lỗi so với các bản trước

| Lỗi | Nguyên nhân | Cách sửa |
|---|---|---|
| `panic: index out of bounds: the len is 61 but the index is 18446744073709551615` (bản 1.x) | Sentinel `-1` cast sang `usize` rồi dùng làm chỉ số mảng | Toàn bộ aligner viết lại với `Option<usize>`, ma trận phẳng, backtrack có nhánh riêng cho biên — **không còn phép trừ chỉ số nào có thể tràn**; kèm bài hồi quy `07. [REGRESSION] panic index 61` |
| Ghép bài 1 xuất tốt, bài 2 "Không ghép được." (bản 1.x) | Trạng thái rò rỉ giữa các lần ghép (panic trên worker thread làm chết request) | Mỗi lần ghép là **phiên mới độc lập**, bộ ghép **vô trạng thái**, panic được `catch_unwind` bắt lại trả lỗi JSON — server không bao giờ chết; kèm bài hồi quy `08. [REGRESSION]` |
| **Tải 2 bài từ URL xong, nút “⚡ Ghép đoạn" bị mờ, không bấm được** (bản 2.0) | Trong `fetchUrl()`, hàm đếm đoạn chạy lúc trạng thái "đang bận" vẫn bật → nút bị khoá; sau đó không còn đường dẫn nào mở khoá lại | Tách hàm `syncAlignBtn()` làm **một nguồn sự thật duy nhất** cho trạng thái nút; mọi vòng "bận" (tải URL, ghép…) kết thúc đều tự đồng bộ lại; kèm bài hồi quy `18. [REGRESSION]` và lọc chrom giao diện khi tải URL |
| **Bảng kết quả nhiều hàng → mỗi hàng bị nén cực hẹp (~12–16px), chữ bị cắt mất** (bản 2.1) | Khung `.rows` là flex-cột có `max-height` — các hàng mặc định `flex-shrink:1` nên bị **nén lại** cho vừa khung thay vì để khung cuộn | `.rows > *{flex:none}` — hàng giữ chiều cao tự nhiên, khung cuộn dọc như dự kiến; kèm chỉnh bố cục mobile (thanh giữa trải nguyên hàng, không còn rơi vào cột 28px) |

## Giấy phép

MIT — xem `LICENSE`.
