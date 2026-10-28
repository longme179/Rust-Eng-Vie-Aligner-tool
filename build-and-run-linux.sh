#!/usr/bin/env bash
# ============================================================================
# REVA tool — BUILD & CHẠY (Linux) — bản 2.10 trở lên
# Dùng: mở terminal trong thư mục này rồi chạy:   bash build-and-run-linux.sh
#
# Script này TỰ LÀM 3 việc:
#   1. Biên dịch app Rust từ mã nguồn (cần Rust — tự báo nếu thiếu)
#   2. Chạy app tại http://localhost:3000 — nhấn Ctrl+C để dừng
#   3. Mở sẵn trình duyệt
#
# KHÔNG cần Python nữa (từ bản 2.10): LaBSE chạy NGAY TRONG app.
# Lần đầu chạy, app TỰ TẢI model LaBSE (~450 MB) + thư viện ONNX Runtime
# (~11 MB) về data/models/ — đúng 1 LẦN DUY NHẤT, tự nối lại khi mạng đứt.
# Trong lúc chờ tải vẫn ghép bình thường bằng bộ từ vựng;
# huy hiệu góc trái màn hình hiện tiến trình tải (%).
# KHÔNG có mạng? App vẫn chạy bằng bộ từ vựng — bấm huy hiệu để tải sau.
# ============================================================================
set -euo pipefail
cd "$(dirname "$0")"

echo "== REVA tool — build & chạy (Linux) =="

# ---------- 1) Rust ----------
if ! command -v cargo >/dev/null 2>&1; then
  echo "✗ Chưa có Rust (cargo). Cài bằng lệnh:"
  echo "    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y"
  echo "  sau đó MỞ TERMINAL MỚI (hoặc 'source ~/.cargo/env') rồi chạy lại script này."
  exit 1
fi
echo "» [1/3] Biên dịch Rust (lần đầu ~2–5 phút, lần sau rất nhanh)…"
cargo build --release

# ---------- 2) chạy app ----------
echo "» [2/3] App chạy tại http://localhost:3000 — nhấn Ctrl+C để dừng."
echo "        Lần đầu app tự tải model LaBSE ~450 MB (1 lần duy nhất) —"
echo "        trong lúc chờ vẫn ghép được bằng bộ từ vựng."

# ---------- 3) trình duyệt ----------
( sleep 2; xdg-open "http://localhost:3000" >/dev/null 2>&1 || true ) &
./target/release/reva-tool
echo ""
echo "» App đã dừng. Muốn xoá model để tải lại: xoá thư mục data/models/."
