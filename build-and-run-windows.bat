@echo off
chcp 65001 >nul
setlocal EnableExtensions
cd /d "%~dp0"

rem ============================================================================
rem  REVA tool — BUILD & CHẠY (Windows)
rem  Dùng: nháy đúp file này, hoặc mở CMD/PowerShell trong thư mục rồi chạy:
rem        build-and-run-windows.bat
rem
rem  Script này TỰ LÀM 5 việc:
rem    1. Biên dịch app Rust từ mã nguồn (cần Rust — tự báo nếu thiếu)
rem    2. Tạo môi trường Python riêng (labse-venv) + cài sentence-transformers
rem       (CHỈ cài khi chưa có — không nâng cấp, không tải lại PyTorch)
rem    3. Kiểm tra model LaBSE trong labse\model\ (đã có thì bỏ qua ngay —
rem       KHÔNG cần mạng; chưa có mới tải ~2 GB MỘT LẦN rồi lưu vào đó mãi mãi)
rem    4. Khởi động dịch vụ LaBSE ở 127.0.0.1:3031 (cửa sổ riêng, thu nhỏ)
rem    5. Chạy app tại http://localhost:3000 — nhấn Ctrl+C để dừng
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
echo [1/5] Biên dịch Rust (lần đầu ~2-5 phút, lần sau rất nhanh)…
cargo build --release
if errorlevel 1 (
  echo [X] Biên dịch Rust lỗi — xem thông báo phía trên.
  pause
  exit /b 1
)

rem ---------- 2) Python ----------
set "PY="
py -3 --version >nul 2>nul
if not errorlevel 1 set "PY=py -3"
if not defined PY (
  python --version >nul 2>nul
  if not errorlevel 1 set "PY=python"
)
if not defined PY (
  echo [X] Không tìm thấy Python 3 — cài tại https://www.python.org/downloads/
  echo     Khi cài nhớ TICK "Add python.exe to PATH" rồi chạy lại file này.
  pause
  exit /b 1
)
if not exist "labse-venv\Scripts\python.exe" (
  echo [2/5] Tạo môi trường ảo Python (labse-venv)…
  %PY% -m venv labse-venv
  if errorlevel 1 (
    echo [X] Tạo môi trường ảo lỗi — thử cài lại Python có chọn "Add to PATH".
    pause
    exit /b 1
  )
)
set "VPY=%~dp0labse-venv\Scripts\python.exe"

rem CHỈ cài khi chưa có — không nâng cấp: pip nâng cấp sẽ kéo PyTorch mới (~2 GB)
rem và người dùng cảm giác như "tải lại model mỗi lần build".
"%VPY%" -c "import sentence_transformers" >nul 2>nul
if errorlevel 1 (
  echo [2/5] Cài sentence-transformers + PyTorch (lần đầu tải ~2 GB — cần mạng)…
  "%VPY%" -m pip install -q --upgrade pip
  "%VPY%" -m pip install -q sentence-transformers
  if errorlevel 1 (
    echo [X] Cài pip lỗi — kiểm tra kết nối mạng rồi chạy lại.
    pause
    exit /b 1
  )
) else (
  echo [2/5] sentence-transformers đã cài — bỏ qua, không nâng cấp, không tải lại.
)

echo [3/5] Kiểm tra model LaBSE trong labse\model\ (có sẵn thì xong ngay, không cần mạng)…
"%VPY%" "%~dp0labse\download_model.py"
if errorlevel 1 (
  echo [X] Tải model lỗi — kiểm tra mạng và chỗ trống đĩa (cần ~2 GB).
  pause
  exit /b 1
)

rem ---------- 3) dịch vụ LaBSE (cửa sổ riêng, cổng 3031) ----------
echo [4/5] Khởi động dịch vụ LaBSE tại 127.0.0.1:3031 (cửa sổ riêng, thu nhỏ)…
start "LaBSE service - khong tat cua so nay" /min cmd /c "%~dp0labse\start-labse-windows.bat"

rem ---------- 4) chờ LaBSE sẵn sàng ----------
echo [4/5] Chờ LaBSE nạp model (lần đầu sau khi cài có thể vài phút)…
"%VPY%" "%~dp0labse\wait_health.py"
if errorlevel 1 (
  echo [X] LaBSE không khởi động được — mở labse-service.log để xem chi tiết.
  pause
  exit /b 1
)

rem ---------- 5) chạy app ----------
echo [5/5] Mở http://localhost:3000 — nhấn Ctrl+C trong cửa sổ này để dừng app.
start "" http://localhost:3000
set "REVA_LABSE_URL=http://127.0.0.1:3031"
"%~dp0target\release\reva-tool.exe"
echo.
echo App đã dừng. Cửa sổ LaBSE (thu nhỏ) có thể vẫn đang chạy —
echo đóng cửa sổ đó khi bạn muốn dừng hẳn dịch vụ LaBSE.
pause
