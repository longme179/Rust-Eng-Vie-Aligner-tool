# HƯỚNG DẪN TRIỂN KHAI — REVA tool v2.10

> Công cụ ghép & sắp đoạn văn bản Anh–Việt (tiền thân: **Cánh đôi Anh–Việt**), 1 chương trình duy nhất, chạy cục bộ 100%.
>
> **v2.10 — LABSE TRỞ THÀNH PHẦN CỐT LÕI, KHÔNG CẦN PYTHON**: LaBSE giờ chạy **ngay trong
> chương trình** (crate `ort` nạp động ONNX Runtime). Lần đầu chạy, app **tự tải** model LaBSE
> ONNX int8 (~450 MB) + thư viện ONNX Runtime vào `data/models/` — đúng **1 lần duy nhất**,
> tự nối lại khi mạng đứt; trong lúc chờ vẫn ghép bình thường bằng bộ từ vựng (huy hiệu góc trái
> hiện tiến trình %, tải lỗi thì bấm huy hiệu để thử lại). Windows còn có **bản dựng sẵn**
> `reva-tool-vX.Y-windows-x64.zip` ở Releases — giải nén là chạy, không cần Rust.
>
> **v2.9 — GIAO DIỆN CHUYÊN NGHIỆP + QoL**: biểu tượng SVG, chế độ tối, thống kê số từ,
> soát lỗi QA 8 phép (chuẩn Xbench), tìm/lọc/thay thế hàng loạt.
>
> **v2.6 — TẢI URL SẠCH**: khi tải bài từ URL, các rác giao diện trang báo
> (ribbon/mega-menu, khối modal/đăng nhập/bình luận, credit ảnh “Ảnh: X…”, tin liên quan) bị loại
> bằng 3 tầng lọc tổng quát (class/id tổ tiên, “link farm” mật độ link, caption/credit ảnh).
> Soi nhanh bộ trích bằng `./target/release/reva-tool --extract file.html`.
>
> **v2.5 — MÔ HÌNH KHỐI**: sau khi ghép, **mỗi đoạn văn là một khối 🧩** co giãn theo độ lớn chữ.
> Kéo khối vào ô cùng ngôn ngữ của HÀNG BẤT KỲ, **đổi thứ tự tuỳ ý**, thả vào thân hàng (hoặc bấm
> `⇥`) để **tách thành hàng riêng**. Kéo nhầm thì **Ctrl+Z hoàn tác** (40 bước gần nhất).
> Mỗi hàng có nút **`↻ Rescan LaBSE`** chấm lại độ khớp sau khi sắp xếp.
> **v2.4**: gộp hàng liền kề (API `/merge` vẫn giữ). **v2.3**: tải URL lấy đủ `<li>`,
> tô màu hàng 3 mức, đúng 3 nút đánh dấu, xuất CSV/HTML/XLIFF. **v2.2**: LaBSE tuỳ chọn.

---

## A. CHẠY BẰNG FILE LỆNH (khuyên dùng — 1 lệnh duy nhất)

**Windows nhanh nhất**: tải **`reva-tool-vX.Y-windows-x64.zip`** ở [Releases](https://github.com/longme179/Rust-Eng-Vie-Aligner-tool/releases) → giải nén → nháy đúp `reva-tool.exe`. Không cần Rust, không cần Python.

Hoặc build từ mã nguồn — giải nén mã nguồn, mở terminal **ngay trong thư mục `en-vi-align-rust`**, chạy:

### Windows

Nháy đúp **`build-and-run-windows.bat`**
(hoặc mở CMD/PowerShell trong thư mục rồi gõ `build-and-run-windows.bat`).

### macOS

```bash
bash build-and-run-macos.sh
```

### Linux

```bash
bash build-and-run-linux.sh
```

File lệnh tự làm 3 việc:

1. Kiểm tra + hướng dẫn cài **Rust** (nếu thiếu — script in sẵn đường dẫn rustup).
2. `cargo build --release` — biên dịch app (lần đầu ~2–5 phút).
3. Chạy app + tự mở trình duyệt **http://localhost:3000**. Nhấn **Ctrl+C** để dừng.

Sau đó app **tự chuẩn bị LaBSE trên nền** (không cần bạn làm gì): lần đầu tải model
ONNX int8 (~450 MB) + thư viện ONNX Runtime vào `data/models/` — đúng 1 lần,
có nối lại khi mạng đứt — rồi nạp vào RAM (~10–15 giây) và chuyển huy hiệu
“Từ vựng + LaBSE — chính xác cao”.

Chạy đúng thì thấy:

```
[reva-tool] chạy tại http://localhost:3000 (giao diện + API, tất cả cục bộ)
[reva-tool] LaBSE: đang tải model lần đầu… (hoặc đã sẵn sàng từ lần chạy trước)
```

và huy hiệu trên đầu trang là **“Từ vựng + LaBSE — chính xác cao”** (chấm xanh lá)
— hoặc đang hiện **tiến trình tải %** trong lần đầu.

### Chạy lại lần sau

Chạy lại đúng file lệnh đó — build tăng dần nhanh, model nằm sẵn trong
**`data/models/`** nên **không bao giờ tải lại** — kể cả khi bạn cập nhật mã nguồn
hay build lại. App vẫn dùng được ngay cả khi không có mạng (bộ từ vựng chạy trước,
LaBSE nạp sau). Muốn tắt hẳn tự tải: đặt biến `REVA_LABSE_AUTO=0`.

> Mẹo kiểm tra bộ trích văn bản (bản 2.6): lưu trang web thành file `.html` rồi chạy
> `./target/release/reva-tool --extract trang.html` — in ra đúng các đoạn công cụ sẽ lấy,
> không cần mở app hay mạng.

### Yêu cầu máy

| | Tối thiểu | Khuyên dùng |
|---|---|---|
| RAM | 4 GB | 8 GB (LaBSE native ăn ~1–1,5 GB RAM khi nạp) |
| Đĩa trống | 0,2 GB (chưa có model) | ~0,8 GB (model ~450 MB + thư viện ONNX Runtime + binary) |
| Mạng | không cần khi chạy | cần lần ĐẦU để tự tải model LaBSE (~450 MB) + thư viện ONNX Runtime |
| Python | ❌ KHÔNG cần (từ 2.10) | — |

### Đổi cổng (mặc định 3000)

```bash
PORT=1709 ./target/release/reva-tool       # Linux/macOS
```

```powershell
$env:PORT=1709; .\target\release\reva-tool.exe   # Windows PowerShell
```

### Dữ liệu nằm ở đâu?

- Phiên làm việc lưu tại **thư mục `data/sessions/`** (mỗi phiên 1 file JSON — phiên cũ của bản ≤2.4 vẫn mở được).
- Model LaBSE + thư viện ONNX Runtime tại **`data/models/`** (tự tạo lần đầu).
- Muốn đặt chỗ khác: đặt biến môi trường `REVA_DATA_DIR=/duong/dan` (tên cũ `CANH_DOI_DATA_DIR` vẫn được chấp nhận).
- Muốn xoá sạch: trong app bấm **Các phiên → Nhả tất cả**, hoặc xoá thư mục `data/`.

---

## B. BUILD THỦ CÔNG (không dùng file lệnh)

Chỉ Rust là bắt buộc (**Rust 1.75+**, cài tại https://rustup.rs — Windows tải `rustup-init.exe`).

```bash
cd en-vi-align-rust
cargo run --release
```

Lần đầu build mất vài phút (tải dependencies). Xong tự mở server tại http://localhost:3000.
LaBSE tự tải + tự nạp ngay trong app (xem mục A) — **không cần Python, không cần bước nào thêm**.
Ai vẫn muốn chạy dịch vụ LaBSE HTTP riêng kiểu cũ: đặt `REVA_LABSE_URL=http://127.0.0.1:3031`
khi khởi động app (mã nguồn dịch vụ cũ còn trong thư mục `labse/`).

Binary nằm tại:

| Hệ điều hành | Đường dẫn binary |
|---|---|
| Linux | `target/release/reva-tool` |
| Windows | `target/release/reva-tool.exe` |
| macOS | `target/release/reva-tool` |

Copy duy nhất file binary đó sang máy khác là chạy được — **không kèm thư mục nào cả**
(giao diện đã nhúng sẵn bên trong binary). Lần chạy đầu trên máy mới sẽ tự tải model
LaBSE vào `data/models/` cạnh binary (hoặc đặt `REVA_DATA_DIR` để chọn chỗ khác);
muốn dùng lại model đã tải thì chép theo thư mục `data/models/`.

---

## C. SỬ DỤNG HÀNG NGÀY

### Ghép 1 bài

1. Dán bài tiếng Anh vào ô trái, bài tiếng Việt vào ô phải
   (hoặc dán URL rồi bấm **⬇ Tải** để tự đổ nội dung vào ô).
   *Mỗi đoạn cách nhau một dòng trống; nếu bài viết liền dòng thì mỗi dòng tính là 1 đoạn.*
2. Bấm **⚡ Ghép đoạn** (hoặc `Ctrl+Enter`).
3. Kiểm tra kết quả:
   - 🟢 **Tự khớp** — tin được.
   - 🟡 **Cần soát** — nên đọc lại, nhấn ✓ nếu đúng.
   - ⚪ **Không khớp** — một phía không có đối tác, khi xuất sẽ ghi chữ `none`.
4. Sửa tay nếu cần: mỗi hàng có nút ✓ (xác nhận), ⚠ (cần soát), ✕ (bỏ ghép — tách thành các hàng `none`).
5. Chọn định dạng rồi bấm **⬇ Xuất file**. Chọn **Excel (.xlsx)** để file tự tô màu từng hàng:
   xanh nhạt = khớp (tự khớp + đã xác nhận), vàng nhạt = cần soát/khớp một phần, hồng nhạt = không khớp —
   kèm cột Điểm (%) và Trạng thái, đóng băng dòng đầu, autofilter, chú giải màu dưới bảng.
   *(CSV/TSV là văn bản thuần nên không mang màu được — muốn màu trong Excel thì chọn .xlsx.)*

### KÉO–THẢ KHỐI (bản 2.5 — cách sửa nhanh cấu trúc)

Sau khi ghép, **mỗi đoạn là một khối** (nhãn `EN·n` / `VI·n` = số đoạn gốc):

- **Gộp nội dung**: kéo khối (vd `VI·16`, `VI·17`) thả vào ô VI của hàng khác (vd hàng có `C1`)
  — thả giữa hai khối sẽ có **vạch chỉ vị trí**; thả vào ô `none` trống để ghép cặp mồ côi.
  *Ví dụ:* EN có A B C, VI có A1 B1 C1 D1 E1 → kéo D1, E1 lên xếp sau C1 → hàng C thành `1 EN · 3 VI`.
- **Đổi thứ tự**: kéo khối lên trước / sau khối khác trong cùng ô — thoải mái, xuất file theo đúng thứ tự khối.
- **Tách riêng**: thả khối vào **thân hàng** khác (hiện nhãn “⇥ Thả đây để TÁCH”), bấm nút **⇥** trên khối,
  hoặc (cảm ứng) bấm chọn khối rồi bấm thân hàng.
- **Hoàn tác**: **Ctrl+Z** (làm lại: Ctrl+Shift+Z) hoặc nút **↩ Hoàn tác / ↪ Làm lại** trên thanh công cụ —
  40 bước gần nhất, gồm cả kéo khối, tách, gộp, đánh dấu, rescan.
- **Rescan**: bấm **↻ Rescan LaBSE** trên hàng vừa sắp xếp lại — công cụ nối các khối mỗi phía thành 1 văn bản,
  chấm lại điểm khớp (LaBSE nếu sẵn sàng, luôn có sàn từ vựng) và cập nhật trạng thái theo ngưỡng.
- Hàng mất khối cuối cùng **tự biến mất**; hàng vừa đổi nội dung về **Cần soát** (điểm cũ bị xoá) → biết chỗ cần rescan.

### LaBSE ảnh hưởng thế nào?

- **Có LaBSE** (huy hiệu xanh “Từ vựng + LaBSE — chính xác cao”): cặp dịch đúng được
  nâng điểm rõ rệt → nhiều cặp 🟢 **Tự khớp** hơn và gán cặp đúng hơn với bài dịch thoáng.
  Nút **Rescan** lúc này chấm bằng cả LaBSE + từ vựng. Bấm vào huy hiệu để xem trạng thái chi tiết.
- **Không LaBSE** (huy hiệu hổ phách “Chỉ từ vựng”): chất lượng như bản 2.1 — vẫn dùng
  được bình thường; **Rescan** vẫn chấm bằng bộ từ vựng (giao diện báo rõ “từ vựng”).
  Bấm huy hiệu để thử tải LaBSE ngay (từ bản 2.10 không cần cài thêm gì cả).
- Mỗi phiên ghi lại động cơ đã dùng (`lexical` / `lexical+labse`) — mở lại phiên cũ vẫn thấy đúng.

### PHIÊN MỚI / NHẢ PHIÊN (câu hỏi thường gặp)

- **Bắt đầu 1 phiên mới**: bấm nút **✨ Phiên mới** — kết quả cũ được dọn sạch, văn bản hai ô được
  giữ lại để bạn chỉnh rồi ghép lại. Mỗi lần bấm **Ghép đoạn** cũng tự tạo một phiên mới.
- **Nhả (xoá) phiên trước**: bấm **🗑 Nhả phiên này** ở thanh công cụ kết quả, hoặc mở
  **📂 Các phiên** → nút **🗑 Nhả** bên mỗi phiên. Có cả **Nhả tất cả phiên**.
- Các phiên cũ nằm trong drawer **📂 Các phiên**, bấm **📂 Mở** để đưa trở lại màn hình
  (kèm văn bản gốc để ghép lại; lịch sử hoàn tác không lưu qua lần mở lại).

### Tự kiểm tra

Nút **🧪 Tự kiểm tra** chạy **58 bài** kiểm nội bộ (không cần mạng, không cần model).
Bốn bài `[REGRESSION]` chính là 4 lỗi từng gặp ở các bản trước (panic `index 61`, “không ghép
được bài thứ hai”, “tải 2 bài từ URL xong nút Ghép bị mờ”, “tải URL mất mục danh sách”);
bài `[TÍCH HỢP]` kiểm toàn bộ đường ống LaBSE bằng dịch vụ nhúng giả lập (không cần tải
model); bài 57–58 kiểm trạng thái LaBSE native mọi pha + vector nhúng thật. Nếu cả 58 bài
xanh thì máy bạn đang chạy đúng bản đã sửa.

---

## D. XỬ LÝ SỰ CỐ

| Triệu chứng | Nguyên nhân & cách xử lý |
|---|---|
| "KHÔNG mở được cổng 3000" | Cổng đang bận. Đổi cổng: `PORT=3001 ./target/release/reva-tool` |
| Script báo "Chưa có Rust" | Cài tại https://rustup.rs (Windows chạy `rustup-init.exe`, chọn 1) rồi **mở terminal MỚI** chạy lại script |
| Huy hiệu hiện "Đang tải LaBSE… N%" lâu | Mạng chậm (~450 MB lần đầu) — cứ để app chạy, tải có nối lại khi đứt; muốn xem nhanh nhất bấm huy hiệu |
| Huy hiệu "LaBSE lỗi — bấm thử lại" | Mạng lỗi lúc tải — bấm vào huy hiệu để tải lại (KHÔNG phải tải lại từ đầu, có resume); vẫn kiểm tra ~0,6 GB đĩa trống |
| Huy hiệu "Chỉ từ vựng" mãi | Chưa có mạng lần đầu (bộ từ vựng vẫn chạy 100%) — khi có mạng, bấm huy hiệu để tải LaBSE ngay |
| Trình duyệt không mở được | Chờ 1–2 giây sau khi thấy dòng `chạy tại http://localhost:3000`; chắc chắn gõ đúng `http://localhost:3000` |
| "Không tải được trang" khi dùng ⬇ Tải | Trang đó chặn robot hoặc cần JavaScript — copy văn bản thủ công vào ô |
| Ghép ra toàn "Cần soát" | Bài ít số/tên riêng → mở **⚙ Nâng cao**, hạ **Ngưỡng tự khớp** xuống ~0.2 rồi ghép lại; hoặc đợi LaBSE sẵn sàng rồi bấm **Rescan tất cả** |
| Quá nhiều đoạn | Tối đa 2000 đoạn mỗi bên; bài dài thì tách làm 2 lần ghép |
| Kéo thả không chạy trên điện thoại | HTML5 drag chỉ có trên desktop — dùng **bấm chọn khối → bấm ô đích** (hoặc nút ⇥ để tách) như mô tả mục C |
| Muốn xoá mọi dữ liệu | **📂 Các phiên → 🗑 Nhả tất cả phiên**, hoặc xoá thư mục `data/` (cả model nằm trong `data/models/` — xoá thì lần sau tải lại) |

---

## E. KIỂM TRA TOÀN VẸN FILE TẢI VỀ

Gói nguồn và gói Windows đều kèm `SHA256SUMS.txt`. Kiểm tra:

```bash
sha256sum -c SHA256SUMS.txt        # Linux/macOS
certutil -hashfile reva-tool-v2.10.0-windows-x64.zip SHA256   # Windows
```

---

## F. DEPLOY LÊN VERCEL (bản web — khuyên dùng khi cần dùng chung trên mạng)

**Cách đúng — dùng nhánh `vercel` (bản web tự chứa):**

1. Trên [vercel.com](https://vercel.com): **Add New → Project → Import Git Repository** → chọn repo REVA.
2. Mục **Root Directory**: chọn **`web-src/`**; Framework tự nhận **Next.js**.
3. Bấm **Deploy** — không cần biến môi trường, không cần database.

Bản web chạy **100% trong trình duyệt người dùng**: LaBSE (cùng file ONNX int8
~450 MB với bản desktop) tự tải đúng 1 lần rồi nằm trong **OPFS** của trình
duyệt (mở lại trang ~10 giây); thuật toán ghép port 1:1 từ `src/aligner.rs` —
kết quả khớp bản desktop. Xem `web-src/README.md` chi tiết.

**Chạy bản Rust trên serverless (Vercel/Docker đĩa chỉ đọc) — nâng cao:**

Bản desktop Rust **không sinh ra cho serverless** (model 450 MB không thể nằm
trong gói hàm; đĩa chỉ đọc). Từ v2.10.1 nếu bạn vẫn chạy được binary ở môi
trường như vậy, REVA tự xoay:

| Biến môi trường | Ý nghĩa |
|---|---|
| `REVA_LABSE_DIR=/tmp/reva-models` | Chỉ định thư mục gốc model ghi được |
| *(không đặt gì)* | REVA tự chọn: `<data>/models` → nếu đĩa chỉ đọc thì **thư mục tạm /tmp** (huy hiệu ghi rõ `cache: "tmp"` + lưu ý *mỗi lần khởi động lại sẽ tải lại ~450 MB*) |
| `REVA_LABSE_AUTO=0` | Tắt tự tải (chỉ bộ từ vựng — nhẹ nhất cho serverless) |
| `REVA_LABSE_URL=...` | Trỏ sang dịch vụ nhúng HTTP riêng nếu có |

Phiên làm việc cũng tự hạ cấp sang **bộ nhớ** khi thư mục phiên chỉ đọc (từ
v2.10.1 — trước đây `/api/align` chết với "Không lưu được phiên: Read-only file
system").
