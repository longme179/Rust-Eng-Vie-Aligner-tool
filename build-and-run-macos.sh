#!/usr/bin/env bash
# ============================================================================
# REVA tool — BUILD & CHẠY (macOS)
# Dùng: mở Terminal trong thư mục này rồi chạy:   bash build-and-run-macos.sh
#
# Script này TỰ LÀM 5 việc:
#   1. Biên dịch app Rust từ mã nguồn (cần Rust — tự báo nếu thiếu)
#   2. Tạo môi trường Python riêng (labse-venv) + cài sentence-transformers
#   3. Tải model LaBSE về máy (chỉ lần ĐẦU — cần mạng, ~2 GB, có cache)
#   4. Khởi động dịch vụ LaBSE ở 127.0.0.1:3031 (nền)
#   5. Chạy app tại http://localhost:3000 — nhấn Ctrl+C để dừng
#
# Chạy lại lần sau: script bỏ qua phần đã cài, chỉ build+nạy nhanh.
# Không muốn LaBSE? Vẫn chạy được: ./target/release/reva-tool
# (app sẽ ghép bằng bộ từ vựng và báo "Chỉ từ vựng — chưa bật LaBSE").
# ============================================================================
set -euo pipefail
cd "$(dirname "$0")"

echo "== REVA tool — build & chạy (macOS) =="

# ---------- 1) Rust ----------
if ! command -v cargo >/dev/null 2>&1; then
  echo "✗ Chưa có Rust (cargo). Cài bằng lệnh:"
  echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
  echo "  sau đó MỞ TERMINAL MỚI (hoặc 'source ~/.cargo/env') rồi chạy lại script này."
  exit 1
fi
echo "» Biên dịch Rust (lần đầu ~2–5 phút, lần sau rất nhanh)…"
cargo build --release

# ---------- 2) Python ----------
PY="$(command -v python3 || command -v python || true)"
if [ -z "$PY" ]; then
  echo "✗ Không tìm thấy Python 3 — cài một trong hai cách:"
  echo "    brew install python3        (nếu có Homebrew)"
  echo "    hoặc tải tại https://www.python.org/downloads/macos/"
  echo "  rồi chạy lại script này."
  exit 1
fi
if [ ! -d labse-venv ]; then
  echo "» Tạo môi trường ảo Python (labse-venv)…"
  "$PY" -m venv labse-venv
fi
# shellcheck disable=SC1091
source labse-venv/bin/activate
echo "» Cài sentence-transformers + PyTorch (lần đầu tải ~2 GB — cần mạng)…"
python -m pip install -q --upgrade pip
python -m pip install -q sentence-transformers
echo "» Tải model LaBSE về máy (chỉ lần đầu, có cache sẵn cho lần sau)…"
python labse/download_model.py

# ---------- 3) dịch vụ LaBSE (nền, cổng 3031) ----------
if [ -f labse-service.pid ] && kill -0 "$(cat labse-service.pid)" 2>/dev/null; then
  echo "» Dịch vụ LaBSE đã đang chạy (pid $(cat labse-service.pid)) — giữ nguyên."
else
  echo "» Khởi động dịch vụ LaBSE tại 127.0.0.1:3031 (log: labse-service.log)…"
  nohup python labse/labse_service.py >> labse-service.log 2>&1 &
  echo $! > labse-service.pid
fi

# ---------- 4) chờ LaBSE sẵn sàng ----------
echo "» Chờ LaBSE nạp model (lần đầu sau khi cài có thể vài phút)…"
python labse/wait_health.py

# ---------- 5) chạy app ----------
export REVA_LABSE_URL="http://127.0.0.1:3031"
trap 'echo "» Đang dừng dịch vụ LaBSE…"; [ -f labse-service.pid ] && kill "$(cat labse-service.pid)" 2>/dev/null || true' EXIT
( sleep 2; open "http://localhost:3000" >/dev/null 2>&1 || true ) &
echo "» App chạy tại http://localhost:3000 — nhấn Ctrl+C để dừng (LaBSE cũng được dừng theo)."
./target/release/reva-tool
