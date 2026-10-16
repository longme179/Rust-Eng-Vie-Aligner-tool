# REVA tool (`reva-tool`) v2.6

> Tiền thân: **Cánh đôi Anh–Việt** (`canh-doi-anh-viet`) — cùng công cụ, đổi tên từ bản 2.5.

Công cụ **ghép & sắp đoạn văn bản Anh–Việt** (chỉ ghép, không dịch) đóng gói thành **một chương trình Rust duy nhất**:
giao diện + API + bộ ghép nằm trong 1 file binary tự build, chạy **100% cục bộ** trên máy bạn — không gửi dữ liệu đi đâu.

**Bản 2.6 — bộ trích URL 3 tầng lọc chrom + LaBSE không bao giờ tải lại**: khi tải bài từ URL, mọi rác giao diện
trang báo (ribbon/mega-menu, khối modal/đăng nhập/bình luận, credit ảnh "Ảnh: X…", tin liên quan) bị loại bằng
3 lớp tổng quát: **(1)** class/id tổ tiên chứa từ khoá giao diện (menu, nav, modal, popup, comment, share,
related, newslette…), **(2)** "link farm" — mật độ ký tự nằm trong thẻ `<a>` (menu gần như toàn link),
**(3)** quy tắc caption/credit ảnh trong `<figure>/<figcaption>` (bỏ credit ngắn, GIỮ caption dài).
Model LaBSE giờ được **lưu trong `labse/model/`** — tải đúng 1 lần; build/cập nhật mã nguồn bao nhiêu lần
cũng **không tải lại**, script build cũng không nâng cấp PyTorch khi đã cài. Thêm flag dòng lệnh
`reva-tool --extract <file.html>` để soi nhanh các đoạn sẽ lấy từ 1 file HTML.

**Bản 2.5 — MÔ HÌNH KHỐI kéo–thả tự do + hoàn tác**: sau khi ghép, **mỗi đoạn văn là một khối** co giãn theo độ lớn chữ.
Kéo khối vào ô cùng ngôn ngữ của HÀNG BẤT KỲ (ví dụ EN có A B C, VI có A1 B1 C1 D1 E1 → kéo D1, E1 lên chung sau C1),
**đổi thứ tự tuỳ ý**, thả vào thân hàng (hoặc bấm `⇥`) để **tách thành hàng riêng**. Kéo nhầm? **Ctrl+Z hoàn tác ngay**
(40 bước gần nhất). Mỗi hàng có nút **`↻ Rescan LaBSE`** chấm lại độ khớp sau khi sắp xếp lại.

**Bản 2.4 thêm gộp hàng liền kề** (API `/merge` vẫn giữ), **bản 2.3** sửa trích `<li>` khi tải URL + tô màu 3 mức +
3 nút đánh dấu + xuất CSV/HTML/XLIFF, **bản 2.2** thêm LaBSE (tuỳ chọn nhưng rất nên có — xem [LaBSE](#labse-mấu-chốt-để-ghép-chính-xác)).

```
[reva-tool] chạy tại http://localhost:3000 (giao diện + API, tất cả cục bộ)
[reva-tool] LaBSE (tuỳ chọn): http://127.0.0.1:3031 — chạy labse/labse_service.py để bật chế độ chính xác cao
```

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
- **Xuất file 7 định dạng**: TSV (tab), Moses (`văn bản ||| văn bản`), JSON, **CSV (mở được bằng Excel, kèm BOM UTF-8)**, **HTML (bảng đối chiếu tô màu theo trạng thái)**, **XLIFF 1.2 (cho công cụ dịch/CAT)**, và **Excel `.xlsx` (bản 2.7) — tự tô màu ô theo trạng thái**: hàng
  **xanh nhạt** = khớp (tự khớp + đã xác nhận), **vàng nhạt** = cần soát/khớp một phần, **hồng nhạt** = không khớp; kèm cột STT/Điểm (%)/Trạng thái tiếng Việt, đóng băng dòng đầu, autofilter, chú giải màu dưới bảng (CSV/TSV là văn bản thuần nên không mang màu được — muốn màu trong Excel thì chọn `.xlsx`). Đoạn thiếu một phía ghi đúng
  chữ `none` — đúng quy ước bản gốc. **Hàng nhiều khối**: TSV/Moses/XLIFF nối đoạn bằng dấu cách; **CSV và .xlsx giữ
  từng đoạn một dòng trong cùng một ô** (Excel mở vẫn đúng); JSON kèm `enParts`/`viParts`; HTML ghi rõ từng đoạn gốc.
  Xuất theo đúng **thứ tự khối đã sắp** (đổi thứ tự tuỳ ý — duy trì bất biến "mỗi đoạn đúng 1 lần").
- **Tải bài từ URL** (`<p>` + mục danh sách `<li>` theo thứ tự tài liệu, đã lọc chrom trang): dán link bài tiếng Anh/tiếng Việt,
  công cụ tải và trích đoạn; tự **lược bỏ chrom giao diện trang** (dòng "Published…", "Share this with…", lời mời đăng ký
  bản tin, địa chỉ bưu điện…) — chỉ giữ nội dung thật của bài.
- **Tô màu 3 mức theo trạng thái** (bản 2.3): hàng **xanh** = khớp/khớp phần lớn (tự khớp + đã xác nhận), hàng **vàng** = cần soát/khớp một phần, hàng **đỏ** = không khớp hoàn toàn — áp cả trong giao diện lẫn file HTML xuất ra; từ bản 2.7 file **Excel `.xlsx`** cũng tự tô màu từng hàng (xanh nhạt/vàng nhạt/hồng nhạt).
- **Đúng 3 nút đánh dấu trên mỗi hàng** (bản 2.3): `✓ Khớp` (xanh) · `⚠ Soát` (vàng) · `✕ Không khớp` (đỏ, tách cặp thành các hàng `none`) — bấm xong xuất file là xong.
- **Tự kiểm tra (selftest)**: 35 bài kiểm nội bộ, gồm 4 bài **hồi quy** đúng 4 lỗi từng gặp
  (panic `index 61`, "không ghép được bài thứ hai", "nút Ghép bị mờ sau khi tải URL", "tải URL mất mục `<li>`"),
  1 bài **tích hợp LaBSE**, 4 bài **gộp hàng** (bản 2.4), 5 bài **mô hình khối** (bản 2.5),
  **3 bài bộ lọc chrom** (bản 2.6): mega-menu/ribbon/modal/comment theo class (tuoitre.vn),
  caption/credit ảnh, "link farm" không cần class, và **2 bài Excel `.xlsx`** (bản 2.7 — đọc lại file zip kiểm fill màu từng hàng + hàng gộp):
  kéo khối + hoàn tác/làm lại nguyên xi, tách khối + prune hàng rỗng, Rescan (công thức + ngưỡng + từ chối một phía),
  xuất sau khi tráo chéo khối (tha thứ đơn điệu, "mỗi đoạn 1 lần" vẫn gắt), đầu vào xấu của `move_block`.

## Chạy nhanh — 1 lệnh cho mỗi hệ điều hành

Người dùng chỉ cần mang thư mục mã nguồn này vào terminal và chạy **file lệnh** tương ứng hệ điều hành:

| Hệ điều hành | Lệnh (trong thư mục `en-vi-align-rust`) |
|---|---|
| **Windows** | nháy đúp `build-and-run-windows.bat` (hoặc chạy nó trong CMD/PowerShell) |
| **macOS** | `bash build-and-run-macos.sh` |
| **Linux** | `bash build-and-run-linux.sh` |

File lệnh tự làm 5 việc: cài Rust nếu thiếu → biên dịch app → tạo môi trường Python riêng (`labse-venv`)
+ cài `sentence-transformers` (**chỉ khi chưa có** — không nâng cấp, để không kéo PyTorch mới mỗi lần) →
kiểm tra model LaBSE trong `labse/model/` (**đã có thì bỏ qua ngay, không cần mạng**; chưa có mới tải ~2 GB
**đúng 1 lần** rồi lưu vào đó) → khởi động LaBSE + app và mở sẵn `http://localhost:3000`.
Chạy lại lần sau thì gần như tức thì (bỏ qua phần đã cài).
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
  huy hiệu trên đầu báo "Chỉ từ vựng — chưa bật LaBSE" (chấm hổ phách). Ghép vẫn chạy 100%;
  nút **Rescan** cũng vẫn chấm được bằng từ vựng (giao diện báo rõ "từ vựng").
- Giao thức: `labse/labse_service.py` lắng nghe tại `127.0.0.1:3031`;
  `GET /health`, `POST /embed {"texts":[…]}` → `{"embeddings":[[…],…]}` (đã chuẩn hoá L2).
  Đổi địa chỉ bằng biến môi trường `REVA_LABSE_URL` (tên cũ `CANH_DOI_LABSE_URL` vẫn được chấp nhận).
- **Không bao giờ tải lại model (bản 2.6)**: model nằm trong **`labse/model/`** (do `labse/download_model.py`
  tạo đúng 1 lần; đổi vị trí bằng biến `REVA_LABSE_MODEL_DIR`). Dịch vụ nạp từ thư mục này ở **chế độ
  offline** (`HF_HUB_OFFLINE=1`) — không phụ thuộc cache HuggingFace, không chạm mạng, cập nhật
  mã nguồn/build lại không ảnh hưởng gì. Xoá `~/.cache/huggingface` cũng không mất model.
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
├── labse/                     # dịch vụ nhúng LaBSE (Python, tuỳ chọn)
│   ├── labse_service.py       #   lắng nghe 127.0.0.1:3031 — /health, /embed (ưu tiên nạp offline từ labse/model)
│   ├── download_model.py      #   đảm bảo model nằm trong labse/model/ — tải 1 LẦN duy nhất, có rồi thì thoát ngay
│   ├── wait_health.py         #   chờ service sẵn sàng (script Windows)
│   └── start-labse-windows.bat#   khởi động service kèm log (script Windows)
├── src/
│   ├── main.rs                # server axum + toàn bộ API + banner
│   ├── aligner.rs             # bộ ghép: chấm điểm + DP đơn điệu + hoà trộn LaBSE + rescore_pair (bản 2.5)
│   ├── embedder.rs            # LaBSE: HTTP client + cosine + pipeline /api/align & selftest + pair_cosine (rescan)
│   ├── session.rs             # phiên: tạo / mở / cập nhật / GỘP HÀNG / TÁCH / move_block / UNDO-REDO / rescan_row / NHẢ PHIÊN
│   ├── exporter.rs            # xuất TSV / Moses / JSON / CSV / HTML / XLIFF / Excel .xlsx (tô màu ô) với chữ `none` nguyên văn (hỗ trợ nhiều khối/hàng)
│   ├── fetcher.rs             # tải URL + trích HTML
│   ├── selftest.rs            # 35 bài tự kiểm (4 hồi quy + LaBSE + xuất mới + gộp hàng + mô hình khối + xlsx màu)
│   ├── lexicon.rs             # từ điển Anh→Việt + danh sách từ phổ biến
│   └── fold.rs                # bỏ dấu tiếng Việt ("Hà Nội" → "ha noi"), chuẩn hoá số
└── data/sessions/             # (tạo lúc chạy) nơi lưu các phiên
```

## API (tất cả trả JSON `{ok:true,data:…}` hoặc `{ok:false,error:…}`)

| Phương thức | Đường dẫn | Chức năng |
|---|---|---|
| POST | `/api/align` | Ghép 2 văn bản → tạo PHIÊN MỚI (tự dùng LaBSE nếu đang chạy) |
| GET | `/api/sessions` | Danh sách phiên |
| GET | `/api/sessions/{id}` | Chi tiết 1 phiên (kèm `canUndo` / `canRedo`) |
| DELETE | `/api/sessions/{id}` | **Nhả phiên** |
| DELETE | `/api/sessions` | Nhả tất cả |
| PATCH | `/api/sessions/{id}/rows/{index}` | Đổi trạng thái 1 hàng (bỏ ghép sẽ tách cặp) |
| POST | `/api/sessions/{id}/rows/{index}/merge` | Gộp hàng kế tiếp vào hàng này (API bản 2.4, vẫn giữ) |
| POST | `/api/sessions/{id}/rows/{index}/rescan` | **Rescan — chấm lại điểm hàng** (LaBSE nếu chạy + sàn từ vựng) · bản 2.5 |
| POST | `/api/sessions/{id}/blocks/move` | **Di chuyển khối** `{side, fromRow, fromPos, toRow, toPos, afterRow}` — `toRow: null` = tách thành hàng riêng · bản 2.5 |
| POST | `/api/sessions/{id}/undo` | **Hoàn tác** bước gần nhất (40 bước) · bản 2.5 |
| POST | `/api/sessions/{id}/redo` | **Làm lại** bước vừa hoàn tác · bản 2.5 |
| POST | `/api/export` | Xuất file (`tsv` / `moses` / `json` / `csv` / `html` / `xliff`) |
| POST | `/api/fetch-url` | Tải bài từ URL |
| GET | `/api/selftest` | Chạy 30 bài tự kiểm |
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
trong selftest 30 bài kể cả khi có LaBSE. Rescan bản 2.5 dùng lại đúng lõi chấm điểm của bộ ghép — không thêm thuật toán mới.)

## Sửa lỗi so với các bản trước

| Lỗi | Nguyên nhân | Cách sửa |
|---|---|---|
| `panic: index out of bounds: the len is 61 but the index is 18446744073709551615` (bản 1.x) | Sentinel `-1` cast sang `usize` rồi dùng làm chỉ số mảng | Toàn bộ aligner viết lại với `Option<usize>`, ma trận phẳng, backtrack có nhánh riêng cho biên — **không còn phép trừ chỉ số nào có thể tràn**; kèm bài hồi quy `07. [REGRESSION] panic index 61` |
| Ghép bài 1 xuất tốt, bài 2 "Không ghép được." (bản 1.x) | Trạng thái rò rỉ giữa các lần ghép (panic trên worker thread làm chết request) | Mỗi lần ghép là **phiên mới độc lập**, bộ ghép **vô trạng thái**, panic được `catch_unwind` bắt lại trả lỗi JSON — server không bao giờ chết; kèm bài hồi quy `08. [REGRESSION]` |
| **Tải 2 bài từ URL xong, nút "⚡ Ghép đoạn" bị mờ, không bấm được** (bản 2.0) | Trong `fetchUrl()`, hàm đếm đoạn chạy lúc trạng thái "đang bận" vẫn bật → nút bị khoá; sau đó không còn đường dẫn nào mở khoá lại | Tách hàm `syncAlignBtn()` làm **một nguồn sự thật duy nhất** cho trạng thái nút; mọi vòng "bận" (tải URL, ghép…) kết thúc đều tự đồng bộ lại; kèm bài hồi quy `18. [REGRESSION]` và lọc chrom giao diện khi tải URL |
| **Bảng kết quả nhiều hàng → mỗi hàng bị nén cực hẹp (~12–16px), chữ bị cắt mất** (bản 2.1) | Khung `.rows` là flex-cột có `max-height` — các hàng mặc định `flex-shrink:1` nên bị **nén lại** cho vừa khung thay vì để khung cuộn | `.rows > *{flex:none}` — hàng giữ chiều cao tự nhiên, khung cuộn dọc như dự kiến; kèm chỉnh bố cục mobile (thanh giữa trải nguyên hàng, không còn rơi vào cột 28px) |
| **Kéo nhầm hàng không hoàn tác được** (bản 2.4 — thao tác gộp hàng là một chiều) | Gộp hàng cắt xé cấu trúc, không có ngăn xếp lịch sử | **Bản 2.5**: mô hình khối (mỗi đoạn = 1 khối kéo tự do) + **ngăn xếp hoàn tác 40 bước** (Ctrl+Z) bao trùm mọi thao tác; kéo nhầm chỉ là kéo lại — hoặc 1 phím Ctrl+Z |

## Giấy phép

MIT — xem `LICENSE`.
