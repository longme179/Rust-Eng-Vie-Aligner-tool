# REVA — công cụ ghép đoạn song ngữ Anh–Việt

![REVA — giao diện chính](docs/images/reva-ui.png)

**REVA** giúp bạn đi tìm *"đoạn tiếng Anh này tương ứng với đoạn tiếng Việt nào"* giữa một bài gốc và bản dịch — rồi xuất ra **bảng đối chiếu Excel/CSV/HTML…** để soát dịch. REVA **không dịch** — chỉ ghép và sắp xếp.

- 🏠 **Chạy 100% trên máy bạn** — văn bản của bạn không bị gửi lên Internet đâu cả.
- 🧠 **LaBSE chạy sẵn trong chương trình** (bản 2.10): tự tải model ~450 MB đúng 1 lần, tự nạp ngay trong binary — không cần Python, không cần cài gì thêm.
- 🖱️ **Kéo–thả từng đoạn** để sắp xếp lại; làm sai thì **Ctrl+Z hoàn tác ngay**.
- 🌙 **Giao diện sáng/tối** (bản 2.9): bấm nút mặt trời/mặt trăng ở góc phải thanh đầu — lựa chọn được ghi nhớ trên máy bạn.
- 🎨 **Xuất Excel tự tô màu**: <span>🟩</span> xanh = khớp · 🟨 vàng = cần soát · 🩷 hồng = không khớp.

> Dùng tốt cho: biên dịch viên, dịch giả sách/truyện, đội dịch nội dung, người học dịch — bất kỳ ai có **2 văn bản song ngữ** cần ghép cặp từng đoạn.

---

## Mục lục

1. [Bạn cần cài gì? (chỉ 1 phần mềm — hoặc không cần gì)](#1-bạn-cần-cài-gì-chỉ-1-phần-mềm--hoặc-không-cần-gì)
2. [Cài đặt từng bước — Windows](#2-cài-đặt-từng-bước--windows)
3. [Cài đặt từng bước — macOS](#3-cài-đặt-từng-bước--macos)
4. [Chạy REVA](#4-chạy-reva)
5. [Khi làm đúng bạn sẽ thấy gì?](#5-khi-làm-đúng-bạn-sẽ-thấy-gì)
6. [Chạy lại lần sau (rất nhanh)](#6-chạy-lại-lần-sau-rất-nhanh)
7. [Câu hỏi thường gặp](#7-câu-hỏi-thường-gặp)
8. [Tính năng chính](#8-tính-năng-chính)
9. [Cho người phát triển](#9-cho-người-phát-triển)

---

## 1. Bạn cần cài gì? (chỉ 1 phần mềm — hoặc không cần gì)

| Phần mềm | Bắt buộc? | Để làm gì | Dung lượng |
|---|---|---|---|
| **Không cần gì hết** | ✅ Cách dễ nhất (Windows) | Tải bản dựng sẵn `reva-tool-vX.Y-windows-x64.zip` ở [Releases](https://github.com/longme179/Rust-Eng-Vie-Aligner-tool/releases) — giải nén là chạy | ~0,5 GB (tính cả model lần đầu) |
| **Rust** (bộ công cụ biên dịch) | Tuỳ chọn (build từ mã nguồn) | Biên dịch REVA thành chương trình chạy được | ~1,5 GB (Windows cần thêm *Visual Studio C++ Build Tools* ~6 GB) |
| Python 3 | ❌ KHÔNG cần nữa (từ bản 2.10) | LaBSE giờ chạy **ngay trong chương trình** — không cần Python | — |

**Sau đó bạn KHÔNG phải cài thêm gì thủ công nữa.** Từ bản **2.10**, lần đầu chạy REVA sẽ TỰ TẢI (chỉ đúng 1 lần, có nối lại nếu mạng đứt):

- model **LaBSE** dạng ONNX tinh gọn (~450 MB) — lưu trong `data/models/labse/`;
- thư viện chạy ONNX Runtime (~11 MB Linux / ~79 MB Windows) — lưu trong `data/models/ort/`.

Trong lúc chờ tải, REVA vẫn ghép bình thường bằng bộ từ vựng; tải xong tự nạp và mọi lượt ghép sau dùng kèm LaBSE (huy hiệu góc trái màn hình hiện tiến trình). Về sau build lại mã nguồn bao nhiêu lần cũng **không bao giờ tải lại**.

**Tổng dung lượng cần chuẩn bị: ~1,5 GB** (Windows: ~7 GB tính cả Visual Studio Build Tools) — giảm từ ~6 GB của các bản trước.
**Không có mạng lần đầu?** REVA vẫn chạy được bình thường bằng bộ từ vựng — bấm huy hiệu để thử tải LaBSE sau.

---

## 2. Cài đặt từng bước — Windows

### Cách A — Dùng bản dựng sẵn (KHÔNG cần cài gì) ✅ khuyên dùng

1. Vào trang [**Releases**](https://github.com/longme179/Rust-Eng-Vie-Aligner-tool/releases) → tìm bản mới nhất → tải file **`reva-tool-vX.Y-windows-x64.zip`**.
2. Giải nén ra thư mục dễ nhớ, ví dụ `C:\reva`.
3. Nháy đúp **`reva-tool.exe`** — nếu Windows hiện bảng xanh *"Windows protected your PC"* → bấm **More info** → **Run anyway** (bình thường với phần mềm mã nguồn mở chưa kí số).
4. Trình duyệt tự mở tại **http://localhost:3000**. Xong!

> Lần đầu chạy, app **tự tải** model LaBSE (~450 MB) + thư viện ONNX Runtime (~17 MB) — đúng 1 lần duy nhất, có nối lại nếu mạng đứt. Trong lúc chờ, ghép vẫn chạy bằng bộ từ vựng; tải xong tự nâng cấp lên LaBSE (xem huy hiệu góc trái màn hình).

### Cách B — Build từ mã nguồn

#### Bước 0 — Tải mã nguồn REVA

Vào trang [**Releases**](https://github.com/longme179/Rust-Eng-Vie-Aligner-tool/releases) của repo → tìm bản mới nhất → tải file **`en-vi-align-rust-source-vX.Y.zip`** → giải nén ra một thư mục dễ nhớ, ví dụ `C:\reva\en-vi-align-rust`.

#### Bước 1 — Cài Rust

1. Mở trình duyệt, vào **https://rustup.rs** → tải file **`rustup-init.exe`**.
2. Chạy file đó. Nếu Windows hiện bảng xanh *"Windows protected your PC"* → bấm **More info** → **Run anyway** (bình thường với phần mềm mã nguồn mở chưa kí số).
3. Trong cửa sổ đen hiện ra, nó sẽ hỏi chọn `1` / `2` / `3` → **gõ `1` rồi Enter** (cài mặc định).
4. Nếu nó báo *"Visual Studio C++ Build Tools is required"* → trả lời **`y`** + Enter, làm theo màn hình installer hiện ra:
   - giữ nguyên lựa chọn **"Desktop development with C++"** (không cần tick thêm gì) → bấm **Install** → chờ xong (tải ~6 GB).
5. Cài xong Rust: **đóng hết cửa sổ đen, MỞ CỬA SỔ MỚI** (quan trọng — để Rust nhận đường dẫn).

### Bước 2 — Chạy

Vào thư mục giải nén lúc nãy (**`en-vi-align-rust`**) → **nháy đúp file `build-and-run-windows.bat`** → xong. Đợi lần đầu ~5–15 phút (biên dịch + tải model 1 lần duy nhất — xem FAQ). Xem [mục 5](#5-khi-làm-đúng-bạn-sẽ-thấy-gì) để biết bạn sẽ thấy gì.

---

## 3. Cài đặt từng bước — macOS

### Bước 0 — Tải mã nguồn REVA

Vào trang [**Releases**](https://github.com/longme179/Rust-Eng-Vie-Aligner-tool/releases) → tải **`en-vi-align-rust-source-vX.Y.zip`** → giải nén, ví dụ vào `~/reva/en-vi-align-rust`.

### Bước 1 — Cài Rust

1. Mở ứng dụng **Terminal**: nhấn `⌘ Command + Space` → gõ `Terminal` → Enter.
2. Dán lệnh sau vào Terminal rồi Enter (lệnh chính thức từ trang **https://rustup.rs**):

   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Khi nó hỏi `1) Proceed with installation (default)` → **Enter** (chọn 1).
4. Nếu macOS bật cửa sổ hỏi cài **"Command Line Tools"** → bấm **Install** và chờ (đây là bộ biên dịch mà Rust cần).
5. Cài xong, chạy đúng 1 lệnh để Terminal nhận Rust:

   ```bash
   source ~/.cargo/env
   ```

### Bước 2 — Chạy

1. Trong **Terminal**, gõ `bash` **+ một dấu cách**.
2. Mở Finder, tìm file **`build-and-run-macos.sh`** trong thư mục giải nén → **kéo thả file đó vào cửa sổ Terminal** (Terminal tự điền đầy đủ đường dẫn).
3. Nhấn **Enter** → đợi lần đầu ~5–15 phút (biên dịch + tải model 1 lần duy nhất). Xem [mục 5](#5-khi-làm-đúng-bạn-sẽ-thấy-gì).

---

## 4. Chạy REVA

Tóm lại, sau khi cài Rust ở trên (hoặc dùng bản dựng sẵn), bạn chỉ cần **một thao tác duy nhất mỗi lần muốn dùng REVA**:

| Hệ điều hành | Cách chạy |
|---|---|
| **Windows (bản dựng sẵn)** | Giải nén `reva-tool-vX.Y-windows-x64.zip` → nháy đúp `reva-tool.exe` |
| **Windows (từ mã nguồn)** | Nháy đúp `build-and-run-windows.bat` trong thư mục `en-vi-align-rust` |
| **macOS** | Terminal → gõ `bash ` → kéo file `build-and-run-macos.sh` vào → Enter |

File lệnh này TỰ LÀM 3 việc: **biên dịch app → chạy app → mở sẵn trình duyệt** tại **http://localhost:3000**.

> Lần đầu chạy (mọi cách cài), app **tự tải model LaBSE ~450 MB + thư viện ONNX Runtime** về `data/models/` — đúng **1 lần duy nhất**, tự nối lại khi mạng đứt. Trong lúc chờ tải vẫn ghép bình thường bằng bộ từ vựng; tải xong tự nạp và mọi lượt ghép sau dùng kèm LaBSE (huy hiệu góc trái màn hình hiện tiến trình %).

> **"Terminal" là gì?** — Terminal (Windows gọi là CMD/PowerShell) là cửa sổ gõ lệnh chữ đen. Bạn không cần biết lập trình — chỉ cần dán đúng lệnh được chỉ và bấm Enter. Đừng đóng cửa sổ này khi đang dùng REVA (đó là app đang chạy đó!). Tắt app = trở lại cửa sổ, nhấn **Ctrl+C** (giữ phím Ctrl, gõ chữ C).

---

## 5. Khi làm đúng bạn sẽ thấy gì?

### Cửa sổ Terminal trông như thế này

**Windows** (`build-and-run-windows.bat`):

![Terminal Windows — chạy đúng](docs/images/terminal-windows.png)

**macOS** (`build-and-run-macos.sh`):

![Terminal macOS — chạy đúng](docs/images/terminal-macos.png)

*(Ảnh minh hoạ — thời gian biên dịch và đường dẫn trên máy bạn sẽ khác nhau.)*

Dấu hiệu **làm đúng**: thấy dòng `[reva-tool] chạy tại http://localhost:3000` và cửa sổ **không tự đóng** — nó cứ đứng yên như thế là app đang chạy. Trình duyệt sẽ tự mở; nếu không thì tự mở `http://localhost:3000`.

### Trình duyệt mở ra giao diện REVA

Đây là màn hình chính sau khi bấm nút **📄 Bài mẫu** rồi **⚡ Ghép đoạn** (mỗi hàng = một cặp Anh–Việt đã ghép):

![REVA — kết quả ghép](docs/images/reva-ui.png)

- 🟩 hàng **xanh** = khớp (tự khớp hoặc bạn đã bấm ✓) — 🟨 hàng **vàng** = cần soát — 🩷 hàng **hồng** = không khớp (thiếu một phía).
- Mỗi đoạn là **một khối 🧩**: kéo sang hàng khác, đổi thứ tự, tách riêng; Ctrl+Z hoàn tác.
- Xuất file bằng nút **⬇ Xuất file** — chọn **Excel (.xlsx)** để có sẵn màu trong file.

REVA cũng chạy tốt trên điện thoại/máy tính bảng nếu mở `http://localhost:3000` trên chính máy đó:

![REVA trên mobile](docs/images/reva-ui-mobile.png)

### Muốn kiểm tra máy mình cài đúng chưa?

Bấm nút **🧪 Tự kiểm tra** trên góc phải — nếu mọi bài đều ✓ (58/58) là REVA chạy hoàn hảo trên máy bạn:

![REVA — tự kiểm tra](docs/images/reva-selftest.png)

---

## 6. Chạy lại lần sau (rất nhanh)

Lần **đầu** mất ~5–15 phút (biên dịch nếu build từ nguồn + tải model ~450 MB đúng 1 lần duy nhất). Từ lần **thứ hai** trở đi chỉ **~30 giây–1 phút** (nạp model vào RAM ~10–15 giây): chạy lại đúng file lệnh/binary đó — nó nhận ra mọi thứ đã có sẵn và bỏ qua.

## Cập nhật lên bản REVA mới

1. Tải zip bản mới từ [Releases](https://github.com/longme179/Rust-Eng-Vie-Aligner-tool/releases) → giải nén **sang thư mục mới**.
2. Chép thư mục **`data/`** từ thư mục CŨ sang thư mục MỚI (giữ lại model LaBSE + thư viện ONNX Runtime + các phiên đã lưu — **không phải tải lại ~0,5 GB**):
   - Windows (bản dựng sẵn): chép thư mục `data/` nằm cạnh `reva-tool.exe`;
   - build từ mã nguồn: chép `en-vi-align-rust/data/`.
3. Chạy file lệnh/binary trong thư mục mới như bình thường.

---

## 7. Câu hỏi thường gặp

**❓ Máy yếu / không có mạng lúc đầu / thiếu chỗ trống — vẫn dùng được không?**
Được. REVA luôn chạy được ngay bằng bộ từ vựng:
- Windows (bản dựng sẵn): nháy đúp `reva-tool.exe`;
- Windows (từ mã nguồn): mở CMD trong thư mục `en-vi-align-rust` rồi gõ `target\release\reva-tool.exe`;
- macOS: trong Terminal gõ `./target/release/reva-tool`.

App báo *"Chỉ từ vựng — chưa bật LaBSE"* — ghép vẫn chạy 100%, chỉ kém chính xác hơn với văn bản ít số liệu/tên riêng. Khi có mạng lại, **bấm vào huy hiệu góc trái** để tải LaBSE ngay (không cần cài lại gì).

**❓ Văn bản của tôi có bị gửi lên mạng không?**
Không. REVA chạy hoàn toàn trên máy bạn. Mạng chỉ được dùng khi: (1) lần đầu cài/tải model, (2) khi bạn chủ động dùng tính năng "⬇ Tải" từ một URL bài viết.

**❓ Lần đầu chạy lâu cỡ nào, cần bao nhiêu GB?**
Tuỳ cách cài: (a) **bản dựng sẵn** — chỉ chờ tải model ~450 MB (vài phút tuỳ mạng); (b) **build từ mã nguồn** — thêm ~5–15 phút biên dịch. Tổng dung lượng sau khi cài hết: **~0,6 GB** cho bản dựng sẵn (~1,5 GB nếu build từ nguồn; Windows build từ nguồn ~7 GB vì kèm Visual Studio Build Tools).

**❓ Cửa sổ đen tắt ngay / báo lỗi?**
Chụp màn hình lỗi lại rồi xem: thường là (1) build từ nguồn mà chưa cài Rust — cài rồi **chưa mở cửa sổ mới** (phải mở CMD/Terminal MỚI sau khi cài Rust); (2) Windows build từ nguồn chưa có Visual Studio C++ Build Tools (xem Bước 1); (3) thiếu chỗ trống đĩa (~0,6 GB bản dựng sẵn / ~1,5 GB build từ nguồn).

**❓ Nút nào để dừng REVA?**
Về cửa sổ Terminal/cửa sổ đen đang chạy app, nhấn **Ctrl+C** (giữ Ctrl, gõ C). Bản 2.10 LaBSE chạy **ngay trong app** nên dừng app là dừng hết — không còn cửa sổ LaBSE riêng nào nữa.

**❓ LaBSE là gì, tại sao nên bật?**
LaBSE là model hiểu nghĩa câu của Google, từ bản 2.10 chạy **ngay trong chương trình** (tự tải 1 lần ~450 MB, không cần Python). Bộ từ vựng sẵn có của REVA giỏi với bài báo giàu số liệu/tên riêng; LaBSE giỏi mọi loại văn bản vì "hiểu nghĩa" — nhờ vậy các cặp dịch thoáng, lược bớt đoạn vẫn ghép đúng.

**❓ File Excel .xlsx xuất ra mở lên có màu thật không?**
Có — mỗi hàng được tô sẵn màu nền xanh/vàng/hồng nhạt kèm cột "Điểm (%)" và "Trạng thái". CSV/TSV là định dạng văn bản thuần nên không mang màu được — muốn màu, chọn Excel (.xlsx).

**❓ REVA có dịch hộ tôi không?**
Không. REVA chỉ *ghép & sắp xếp* các đoạn đã có sẵn của hai văn bản. Bạn vẫn cần bản dịch (tự dịch hoặc từ dịch giả/máy dịch).

---

## 8. Tính năng chính

- **Ghép đoạn song ngữ** bằng điểm tương đồng (từ điển Anh–Việt ~500 mục, số liệu, tên riêng, vị trí tương đối) + quy hoạch động — không cần GPU.
- **LaBSE cốt lõi** (bản 2.10 — tự tải ~450 MB đúng 1 lần, tự nạp trong binary): hoà trộn điểm từ vựng với cosine LaBSE (nguyên tắc *chỉ nâng không hạ*) → chính xác hơn hẳn; không có LaBSE vẫn ghép được bằng thuần từ vựng.
- **Mô hình khối kéo–thả tự do**: mỗi đoạn là một khối; kéo ghép/tách/đổi thứ tự; **Ctrl+Z hoàn tác 40 bước**; `Rescan LaBSE` chấm lại điểm từng hàng; `Rescan tất cả` chấm lại MỌI hàng một nhấn (hoàn tác cả lô bằng 1 Ctrl+Z).
- **4 trạng thái chuẩn**: Tự khớp · Cần soát · Không khớp · Đã xác nhận — hiển thị bằng màu trong giao diện lẫn file xuất.
- **Xuất 8 định dạng**: Excel `.xlsx` (tự tô màu từng hàng), CSV, TSV, Moses, JSON, HTML (bảng có màu), XLIFF 1.2, và **TMX 1.2 — bộ nhớ dịch nhập thẳng vào OmegaT/Trados**. Đoạn thiếu một phía ghi đúng chữ `none`.
- **Tải bài từ URL**: dán link bài báo tiếng Anh/tiếng Việt — REVA tự trích nội dung, lọc sạch menu/quảng cáo/caption ảnh; **chỉ tải trang công khai** (chặn địa chỉ nội bộ — chống SSRF).
- **Kéo–thả file .txt**: thả file vào ô tiếng Anh/tiếng Việt là nạp ngay (đọc cục bộ, không upload); thả 2 file cùng lúc vào ô nào cũng được (file 1 → ô đó, file 2 → ô còn lại).
- **Chạy kín trên máy bạn**: mặc định chỉ mở trên `127.0.0.1` (máy khác trong mạng KHÔNG truy cập được); muốn chia sẻ LAN thì đặt biến `REVA_BIND=0.0.0.0`.
- **Phiên làm việc độc lập**: ghép bao nhiêu bài liên tiếp không lẫn dữ liệu; tự lưu vào `data/sessions/`, mở lại được.
- **📊 Thống kê số từ** (bản 2.9): nút `Số từ` đếm ngay số từ / ký tự / đoạn mỗi phía — **căn cứ tính giá theo số từ nguồn** (chuẩn ngành dịch thuật); kèm bảng theo trạng thái, phân vùng đã ghép/chưa có bản dịch, nút **Sao chép** chuỗi tiếng Việt sẵn để dán vào thư báo giá. Số từ còn hiện trực tiếp trên đầu ô nhập ngay khi dán bài, trước khi ghép. Quy ước công khai: từ = chuỗi không khoảng trắng Unicode; ký tự = ký tự Unicode sau chuẩn hoá NFC (dán từ macOS hay Windows đều ra cùng con số).
- **🔍 Soát lỗi bản dịch** (bản 2.9): nút `Soát lỗi` chạy 8 phép kiểm theo danh mục chuẩn ngành (Xbench) — **sai số liệu** (bắt cả lỗi dời dấu thập phân `3.5` → `35`; tha số tháng khi phía Anh có tên tháng), **bỏ dịch** (VI giống hệt EN), **độ dài bất thường**, **khoảng trắng thừa**, **ngoặc không cân**, **tên riêng viết hoa mất/thêm** (NASA), **thiếu dấu tiếng Việt**, **Unicode NFD lẫn**. Danh sách vấn đề xếp theo mức Lỗi / Cần xem / Ghi chú — bấm từng mục nhảy thẳng tới hàng trong bảng. Thuần đọc: **không tự sửa gì**, không đụng LaBSE (kết quả giống hệt khi có/không LaBSE).
- **🔎 Tìm / lọc / thay thế hàng loạt** (bản 2.9): ô **Tìm trong bảng** lọc tức thì theo chữ (không phân biệt hoa thường, tô sáng khối chứa chuỗi, đếm n/tổng hàng); nút **Thay thế** xem trước số chỗ rồi áp một lần cho cả cột (Anh hoặc Việt, tuỳ chọn phân biệt hoa thường) — phát hiện “United Nations” dịch 2 kiểu hay lỗi chính tả lặp 40 chỗ thì sửa MỘT lần; **Ctrl+Z hoàn tác cả lô**. Chuỗi tìm là đoạn văn bản thường (không phải regex); 0 chỗ khớp thì không đổi gì.
- **🎨 Giao diện dọn lại cho chuyên nghiệp** (bản 2.9): hệ biểu tượng SVG sắc nét thay cho emoji, bộ màu thống nhất theo token, **chế độ tối** tự theo máy + đổi được bằng một nút, hộp thoại/ngăn kéo/toast làm lại, hỗ trợ `prefers-reduced-motion` — mọi nút và lệnh giữ nguyên chỗ cũ.
- **58 bài tự kiểm** nội bộ (hồi quy lỗi từng gặp + tích hợp LaBSE + Excel màu + 4 bài audit bảo mật + TMX + rescan toàn bộ + 4 bài thống kê số từ + 6 bài soát lỗi QA + 5 bài tìm/thay thế + 2 bài LaBSE native) — bấm `Tự kiểm tra` bất cứ lúc nào.

Chi tiết thuật toán, API HTTP, cấu trúc mã nguồn, bất biến kiểm chứng và lịch sử các bản: xem **[docs/README-KY-THUAT.md](docs/README-KY-THUAT.md)**.

---

## 9. Cho người phát triển

REVA là một binary Rust duy nhất (axum + tokio) nhúng sẵn giao diện. Từ bản **2.10**, LaBSE chạy **ngay trong binary** (crate `ort` nạp động ONNX Runtime; model tự tải ~450 MB vào `data/models/` lần đầu). Dịch vụ LaBSE HTTP riêng kiểu cũ vẫn được hỗ trợ qua biến `REVA_LABSE_URL` cho ai đã có sẵn.

```bash
cargo run --release        # build + chạy tại http://localhost:3000
cargo test                 # test biên dịch — 58 bài selftest đầy đủ chạy qua GET /api/selftest
```

Đổi cổng: `PORT=1709 cargo run --release`. Đổi thư mục dữ liệu: `REVA_DATA_DIR=...`. Xem đầy đủ API + tài liệu kỹ thuật tại **[docs/README-KY-THUAT.md](docs/README-KY-THUAT.md)**.

## Giấy phép

MIT — xem `LICENSE`.
