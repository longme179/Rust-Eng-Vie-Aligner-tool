# HƯỚNG DẪN TRIỂN KHAI — REVA tool v2.5

> Công cụ ghép & sắp đoạn văn bản Anh–Việt (tiền thân: **Cánh đôi Anh–Việt**), 1 chương trình duy nhất, chạy cục bộ 100%.
> **Từ v2.2 không còn binary dựng sẵn** — mỗi hệ điều hành có **một file lệnh duy nhất**
> đem vào terminal là tự build + tự chạy (người dùng tự compile từ mã nguồn, nhờ đó
> **LaBSE — mấu chốt để ghép chính xác — luôn đầy đủ**).
>
> **v2.5 — MÔ HÌNH KHỐI**: sau khi ghép, **mỗi đoạn văn là một khối 🧩** co giãn theo độ lớn chữ.
> Kéo khối vào ô cùng ngôn ngữ của HÀNG BẤT KỲ (ví dụ kéo D1, E1 lên chung sau C1), **đổi thứ tự tuỳ ý**,
> thả vào thân hàng (hoặc bấm `⇥`) để **tách thành hàng riêng**. Kéo nhầm thì **Ctrl+Z hoàn tác**
> (40 bước gần nhất, bao trùm mọi thao tác). Mỗi hàng có nút **`↻ Rescan LaBSE`** chấm lại độ khớp sau khi sắp xếp.
> **v2.4**: gộp hàng liền kề (API `/merge` vẫn giữ). **v2.3**: tải URL lấy đủ mục danh sách `<li>`,
> tô màu hàng 3 mức (xanh/vàng/đỏ), đúng 3 nút đánh dấu, xuất CSV/HTML/XLIFF. **v2.2**: LaBSE tuỳ chọn
> (huy hiệu động cơ trên đầu trang), script build cho Windows/macOS/Linux, hết lỗi hàng bị nén hẹp.

---

## A. CHẠY BẰNG FILE LỆNH (khuyên dùng — 1 lệnh duy nhất)

Giải nén mã nguồn, mở terminal **ngay trong thư mục `en-vi-align-rust`**, chạy:

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

File lệnh tự làm 5 việc:

1. Kiểm tra + hướng dẫn cài **Rust** (nếu thiếu — script in sẵn đường dẫn rustup).
2. `cargo build --release` — biên dịch app (lần đầu ~2–5 phút).
3. Tạo môi trường Python riêng `labse-venv` + cài `sentence-transformers`
   (lần đầu tải ~2 GB — cần mạng; lần sau bỏ qua).
4. Tải **model LaBSE** về cache máy (chỉ lần đầu) rồi khởi động dịch vụ LaBSE tại `127.0.0.1:3031`.
5. Chạy app + tự mở trình duyệt **http://localhost:3000**. Nhấn **Ctrl+C** để dừng.

Chạy đúng thì thấy:

```
[reva-tool] chạy tại http://localhost:3000 (giao diện + API, tất cả cục bộ)
[reva-tool] LaBSE (tuỳ chọn): http://127.0.0.1:3031 — …
```

và huy hiệu trên đầu trang là **“Từ vựng + LaBSE — chính xác cao”** (chấm xanh lá).

### Chạy lại lần sau

Chạy lại đúng file lệnh đó — gần như tức thì (build tăng dần, Python/model đã có sẵn).
Muốn KHÔNG dùng LaBSE (máy yếu, không muốn cài Python): chạy thẳng binary
`target/release/reva-tool` — app vẫn hoạt động, huy hiệu báo
“Chỉ từ vựng — chưa bật LaBSE” (chấm hổ phách) và ghép bằng bộ từ vựng
(nút **Rescan** cũng vẫn chấm được bằng từ vựng).

### Yêu cầu máy

| | Tối thiểu | Khuyên dùng |
|---|---|---|
| RAM | 4 GB | 8 GB (LaBSE ăn ~1.5–2 GB) |
| Đĩa trống | 1 GB (không LaBSE) | ~4 GB (có LaBSE: Python + PyTorch + model 1.8 GB) |
| Mạng | không cần khi chạy | cần lần ĐẦU để tải dependencies + model LaBSE |

**Linux không có card NVIDIA**: muốn tiết kiệm ~2 GB tải, chạy trước
`labse-venv/bin/pip install torch --index-url https://download.pytorch.org/whl/cpu`
rồi chạy lại script.

### Đổi cổng (mặc định 3000)

```bash
PORT=1709 ./target/release/reva-tool       # Linux/macOS
```

```powershell
$env:PORT=1709; .\target\release\reva-tool.exe   # Windows PowerShell
```

### Dữ liệu nằm ở đâu?

- Phiên làm việc lưu tại **thư mục `data/sessions/`** (mỗi phiên 1 file JSON — phiên cũ của bản ≤2.4 vẫn mở được).
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
Muốn LaBSE, chạy riêng dịch vụ (một cửa sổ terminal khác):

```bash
python3 -m venv labse-venv
labse-venv/bin/pip install sentence-transformers
labse-venv/bin/python labse/labse_service.py
```

Binary nằm tại:

| Hệ điều hành | Đường dẫn binary |
|---|---|
| Linux | `target/release/reva-tool` |
| Windows | `target/release/reva-tool.exe` |
| macOS | `target/release/reva-tool` |

Copy duy nhất file binary đó sang máy khác là chạy được — **không kèm thư mục nào cả**
(giao diện đã nhúng sẵn bên trong binary). Muốn có LaBSE trên máy đó thì cài Python
+ chạy `labse_service.py` như trên.

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
5. Chọn định dạng rồi bấm **⬇ Xuất file**.

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
  chấm lại điểm khớp (LaBSE nếu dịch vụ chạy, luôn có sàn từ vựng) và cập nhật trạng thái theo ngưỡng.
- Hàng mất khối cuối cùng **tự biến mất**; hàng vừa đổi nội dung về **Cần soát** (điểm cũ bị xoá) → biết chỗ cần rescan.

### LaBSE ảnh hưởng thế nào?

- **Có LaBSE** (huy hiệu xanh “Từ vựng + LaBSE — chính xác cao”): cặp dịch đúng được
  nâng điểm rõ rệt → nhiều cặp 🟢 **Tự khớp** hơn và gán cặp đúng hơn với bài dịch thoáng.
  Nút **Rescan** lúc này chấm bằng cả LaBSE + từ vựng. Bấm vào huy hiệu để xem trạng thái chi tiết.
- **Không LaBSE** (huy hiệu hổ phách “Chỉ từ vựng”): chất lượng như bản 2.1 — vẫn dùng
  được bình thường; **Rescan** vẫn chấm bằng bộ từ vựng (giao diện báo rõ “từ vựng”).
  Bấm huy hiệu để xem cách bật.
- Mỗi phiên ghi lại động cơ đã dùng (`lexical` / `lexical+labse`) — mở lại phiên cũ vẫn thấy đúng.

### PHIÊN MỚI / NHẢ PHIÊN (câu hỏi thường gặp)

- **Bắt đầu 1 phiên mới**: bấm nút **✨ Phiên mới** — kết quả cũ được dọn sạch, văn bản hai ô được
  giữ lại để bạn chỉnh rồi ghép lại. Mỗi lần bấm **Ghép đoạn** cũng tự tạo một phiên mới.
- **Nhả (xoá) phiên trước**: bấm **🗑 Nhả phiên này** ở thanh công cụ kết quả, hoặc mở
  **📂 Các phiên** → nút **🗑 Nhả** bên mỗi phiên. Có cả **Nhả tất cả phiên**.
- Các phiên cũ nằm trong drawer **📂 Các phiên**, bấm **📂 Mở** để đưa trở lại màn hình
  (kèm văn bản gốc để ghép lại; lịch sử hoàn tác không lưu qua lần mở lại).

### Tự kiểm tra

Nút **🧪 Tự kiểm tra** chạy 30 bài kiểm nội bộ (không cần mạng). Bốn bài `[REGRESSION]` chính là
4 lỗi từng gặp ở các bản trước (panic `index 61`, “không ghép được bài thứ hai”, “tải 2 bài từ URL
xong nút Ghép bị mờ”, “tải URL mất mục danh sách”); bài `[TÍCH HỢP]` kiểm toàn bộ đường ống LaBSE bằng
dịch vụ nhúng giả lập (không cần tải model); 4 bài gộp hàng (bản 2.4) và 5 bài mô hình khối (bản 2.5:
kéo khối + hoàn tác/làm lại, tách khối, Rescan, xuất sau đổi thứ tự, đầu vào xấu). Nếu cả 30 bài xanh
thì máy bạn đang chạy đúng bản đã sửa.

---

## D. XỬ LÝ SỰ CỐ

| Triệu chứng | Nguyên nhân & cách xử lý |
|---|---|
| "KHÔNG mở được cổng 3000" | Cổng đang bận. Đổi cổng: `PORT=3001 ./target/release/reva-tool` |
| Script báo "Chưa có Rust" | Cài tại https://rustup.rs (Windows chạy `rustup-init.exe`, chọn 1) rồi **mở terminal MỚI** chạy lại script |
| Script báo "Không tìm thấy Python 3" | Cài Python 3.9+ (Windows: tick **Add python.exe to PATH** lúc cài) rồi chạy lại script |
| Tải model LaBSE lỗi | Kiểm tra mạng + ~2 GB đĩa trống; chạy lại script (model có cache, không phải tải lại từ đầu) |
| Huy hiệu vẫn "Chỉ từ vựng" dù đã chạy script | Dịch vụ LaBSE chưa lên (CPU chậm lần đầu) — chờ dòng `LaBSE OK` trong script; xem `labse-service.log` nếu treo lâu |
| Trình duyệt không mở được | Chờ 1–2 giây sau khi thấy dòng `chạy tại http://localhost:3000`; chắc chắn gõ đúng `http://localhost:3000` |
| "Không tải được trang" khi dùng ⬇ Tải | Trang đó chặn robot hoặc cần JavaScript — copy văn bản thủ công vào ô |
| Ghép ra toàn "Cần soát" | Bài ít số/tên riêng → mở **⚙ Nâng cao**, hạ **Ngưỡng tự khớp** xuống ~0.2 rồi ghép lại; hoặc bật LaBSE để nâng chất lượng |
| Quá nhiều đoạn | Tối đa 2000 đoạn mỗi bên; bài dài thì tách làm 2 lần ghép |
| Kéo thả không chạy trên điện thoại | HTML5 drag chỉ có trên desktop — dùng **bấm chọn khối → bấm ô đích** (hoặc nút ⇥ để tách) như mô tả mục C |
| Muốn xoá mọi dữ liệu | **📂 Các phiên → 🗑 Nhả tất cả phiên**, hoặc xoá thư mục `data/` |

---

## E. KIỂM TRA TOÀN VẸN FILE TẢI VỀ

Gói nguồn kèm `SHA256SUMS.txt`. Kiểm tra:

```bash
sha256sum -c SHA256SUMS.txt        # Linux/macOS
certutil -hashfile en-vi-align-rust-source-v2.5.zip SHA256   # Windows
```
