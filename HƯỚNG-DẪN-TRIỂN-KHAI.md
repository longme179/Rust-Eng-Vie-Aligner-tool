# HƯỚNG DẪN TRIỂN KHAI — Cánh đôi Anh–Việt v2.3

> Công cụ ghép đoạn văn bản Anh–Việt, 1 chương trình duy nhất, chạy cục bộ 100%.
> **Từ v2.2 không còn binary dựng sẵn** — mỗi hệ điều hành có **một file lệnh duy nhất**
> đem vào terminal là tự build + tự chạy (người dùng tự compile từ mã nguồn, nhờ đó
> **LaBSE — mấu chốt để ghép chính xác — luôn đầy đủ**).
>
> **v2.3 sửa**: tải URL giờ lấy đủ mục danh sách (gạch đầu dòng) — trước đây chỉ lấy
> được câu dẫn (vd bài SBS chỉ còn "bao gồm:" mà mất 3 gạch đầu dòng). **v2.3 thêm**:
> tô màu hàng 3 mức (xanh/vàng/đỏ), đúng 3 nút đánh dấu, xuất CSV/HTML/XLIFF.
> **v2.2 sửa**: bảng kết quả nhiều hàng bị nén cực hẹp, chữ bị cắt → đã hết
> (hàng giữ chiều cao tự nhiên, khung cuộn dọc). **v2.2 thêm**: LaBSE tuỳ chọn
> (huy hiệu động cơ trên đầu trang), script build cho Windows/macOS/Linux.

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
[canh-doi-anh-viet] chạy tại http://localhost:3000 (giao diện + API, tất cả cục bộ)
[canh-doi-anh-viet] LaBSE (tuỳ chọn): http://127.0.0.1:3031 — …
```

và huy hiệu trên đầu trang là **“Từ vựng + LaBSE — chính xác cao”** (chấm xanh lá).

### Chạy lại lần sau

Chạy lại đúng file lệnh đó — gần như tức thì (build tăng dần, Python/model đã có sẵn).
Muốn KHÔNG dùng LaBSE (máy yếu, không muốn cài Python): chạy thẳng binary
`target/release/canh-doi-anh-viet` — app vẫn hoạt động, huy hiệu báo
“Chỉ từ vựng — chưa bật LaBSE” (chấm hổ phách) và ghép bằng bộ từ vựng.

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
PORT=1709 ./target/release/canh-doi-anh-viet       # Linux/macOS
```

```powershell
$env:PORT=1709; .\target\release\canh-doi-anh-viet.exe   # Windows PowerShell
```

### Dữ liệu nằm ở đâu?

- Phiên làm việc lưu tại **thư mục `data/sessions/`** (mỗi phiên 1 file JSON).
- Muốn đặt chỗ khác: đặt biến môi trường `CANH_DOI_DATA_DIR=/duong/dan`.
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
| Linux | `target/release/canh-doi-anh-viet` |
| Windows | `target/release/canh-doi-anh-viet.exe` |
| macOS | `target/release/canh-doi-anh-viet` |

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
4. Sửa tay nếu cần: mỗi hàng có nút ✓ (xác nhận), ⚠ (cần soát), ✕ (bỏ ghép — tách thành 2 hàng `none`), ↺ (về tự động).
5. Chọn định dạng rồi bấm **⬇ Xuất file**.

### LaBSE ảnh hưởng thế nào?

- **Có LaBSE** (huy hiệu xanh “Từ vựng + LaBSE — chính xác cao”): cặp dịch đúng được
  nâng điểm rõ rệt → nhiều cặp 🟢 **Tự khớp** hơn và gán cặp đúng hơn với bài dịch thoáng.
  Bấm vào huy hiệu để xem trạng thái chi tiết.
- **Không LaBSE** (huy hiệu hổ phách “Chỉ từ vựng”): chất lượng như bản 2.1 — vẫn dùng
  được bình thường. Bấm huy hiệu để xem cách bật.
- Mỗi phiên ghi lại động cơ đã dùng (`lexical` / `lexical+labse`) — mở lại phiên cũ vẫn thấy đúng.

### PHIÊN MỚI / NHẢ PHIÊN (câu hỏi thường gặp)

- **Bắt đầu 1 phiên mới**: bấm nút **✨ Phiên mới** — kết quả cũ được dọn sạch, văn bản hai ô được
  giữ lại để bạn chỉnh rồi ghép lại. Mỗi lần bấm **Ghép đoạn** cũng tự tạo một phiên mới.
- **Nhả (xoá) phiên trước**: bấm **🗑 Nhả phiên này** ở thanh công cụ kết quả, hoặc mở
  **📂 Các phiên** → nút **🗑 Nhả** bên mỗi phiên. Có cả **Nhả tất cả phiên**.
- Các phiên cũ nằm trong drawer **📂 Các phiên**, bấm **📂 Mở** để đưa trở lại màn hình
  (kèm văn bản gốc để ghép lại).

### Tự kiểm tra

Nút **🧪 Tự kiểm tra** chạy 21 bài kiểm nội bộ (không cần mạng). Bốn bài `[REGRESSION]` chính là
3 lỗi từng gặp ở các bản trước (panic `index 61`, “không ghép được bài thứ hai”, “tải 2 bài từ URL
xong nút Ghép bị mờ”); bài `[TÍCH HỢP]` cuối kiểm toàn bộ đường ống LaBSE bằng dịch vụ nhúng
giả lập (không cần tải model), bài 20 hồi quy "tải URL mất mục danh sách" và bài 21 xuất CSV/HTML/XLIFF. Nếu cả 21 bài xanh thì máy bạn đang chạy đúng bản đã sửa.

---

## D. XỬ LÝ SỰ CỐ

| Triệu chứng | Nguyên nhân & cách xử lý |
|---|---|
| "KHÔNG mở được cổng 3000" | Cổng đang bận. Đổi cổng: `PORT=3001 ./target/release/canh-doi-anh-viet` |
| Script báo "Chưa có Rust" | Cài tại https://rustup.rs (Windows chạy `rustup-init.exe`, chọn 1) rồi **mở terminal MỚI** chạy lại script |
| Script báo "Không tìm thấy Python 3" | Cài Python 3.9+ (Windows: tick **Add python.exe to PATH** lúc cài) rồi chạy lại script |
| Tải model LaBSE lỗi | Kiểm tra mạng + ~2 GB đĩa trống; chạy lại script (model có cache, không phải tải lại từ đầu) |
| Huy hiệu vẫn "Chỉ từ vựng" dù đã chạy script | Dịch vụ LaBSE chưa lên (CPU chậm lần đầu) — chờ dòng `LaBSE OK` trong script; xem `labse-service.log` nếu treo lâu |
| Trình duyệt không mở được | Chờ 1–2 giây sau khi thấy dòng `chạy tại http://localhost:3000`; chắc chắn gõ đúng `http://localhost:3000` |
| "Không tải được trang" khi dùng ⬇ Tải | Trang đó chặn robot hoặc cần JavaScript — copy văn bản thủ công vào ô |
| Ghép ra toàn "Cần soát" | Bài ít số/tên riêng → mở **⚙ Nâng cao**, hạ **Ngưỡng tự khớp** xuống ~0.2 rồi ghép lại; hoặc bật LaBSE để nâng chất lượng |
| Quá nhiều đoạn | Tối đa 2000 đoạn mỗi bên; bài dài thì tách làm 2 lần ghép |
| Muốn xoá mọi dữ liệu | **📂 Các phiên → 🗑 Nhả tất cả phiên**, hoặc xoá thư mục `data/` |

---

## E. KIỂM TRA TOÀN VẸN FILE TẢI VỀ

Gói nguồn kèm `SHA256SUMS.txt`. Kiểm tra:

```bash
sha256sum -c SHA256SUMS.txt        # Linux/macOS
certutil -hashfile en-vi-align-rust-source-v2.3.zip SHA256   # Windows
```
