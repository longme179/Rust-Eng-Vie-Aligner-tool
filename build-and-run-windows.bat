@echo off
chcp 65001 >nul
setlocal EnableExtensions
cd /d "%~dp0"

rem ============================================================================
rem  REVA tool — BUILD & CHẠY (Windows) — bản 2.10 trở lên
rem  Dùng: nháy đúp file này, hoặc mở CMD/PowerShell trong thư mục rồi chạy:
rem        build-and-run-windows.bat
rem
rem  Script này TỰ LÀM 3 việc:
rem    1. Biên dịch app Rust từ mã nguồn (cần Rust — tự báo nếu thiếu)
rem    2. Chạy app tại http://localhost:3000 — nhấn Ctrl+C để dừng
rem    3. Mở sẵn trình duyệt
rem
rem  KHÔNG cần Python nữa (từ bản 2.10): LaBSE chạy NGAY TRONG app.
rem  Lần đầu chạy, app TỰ TẢI model LaBSE (~450 MB) + thư viện ONNX Runtime
rem  (~17 MB) về data\models\ — đúng 1 LẦN DUY NHẤT, tự nối lại khi mạng đứt.
rem  Trong lúc chờ tải vẫn ghép bình thường bằng bộ từ vựng;
rem  huy hiệu góc trái màn hình hiện tiến trình tải (%).
rem  KHÔNG có mạng? App vẫn chạy bằng bộ từ vựng — bấm huy hiệu để tải sau.
rem ============================================================================

echo == REVA tool — build ^& chạy (Windows) ==

rem ---------- 1) Rust ----------
where cargo >nul 2>nul
if errorlevel 1 (
  echo [X] Chưa có Rust ^(cargo^). Cài đặt như sau:
  echo     1. Tải rustup-init.exe tại https://rustup.rs
  echo     2. Chạy file đó, chọn 1 ^(mặc định^), chờ xong
  echo     3. MỞ CỬA SỔ CMD MỚI rồi chạy lại file này
  pause
  exit /b 1
)
echo [1/3] Biên dịch Rust (lần đầu ~2-5 phút, lần sau rất nhanh)…
cargo build --release
if errorlevel 1 (
  echo [X] Biên dịch Rust lỗi — xem thông báo phía trên.
  pause
  exit /b 1
)

rem ---------- 2) chạy app ----------
echo [2/3] Mở http://localhost:3000 — nhấn Ctrl+C trong cửa sổ này để dừng app.
echo       Lần đầu app tự tải model LaBSE ~450 MB (1 lần duy nhất) —
echo       trong lúc chờ vẫn ghép được bằng bộ từ vựng.
start "" http://localhost:3000

rem ---------- 3) trình duyệt ----------
"%~dp0target\release\reva-tool.exe"
echo.
echo App đã dừng. (Từ bản 2.10 LaBSE chạy ngay trong app — không còn
echo cửa sổ LaBSE riêng.) Muốn xoá model để tải lại: xoá thư mục data\models\.
pause
