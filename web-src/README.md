# REVA Web — bản web của REVA tool (nhánh `vercel`)

Ghép & sắp đoạn văn bản **Anh–Việt** ngay trong trình duyệt — **không gửi văn bản đi đâu**.

## Deploy lên Vercel (không cần cấu hình gì thêm)

1. Trên Vercel: **Add New → Project → Import Git Repository** → chọn repo này.
2. Chọn **nhánh `vercel`** làm production branch (hoặc giữ `main` nếu bạn chỉ cần bản desktop).
3. Ở mục **Root Directory**: chọn `web-src/` (Vercel tự nhận Next.js).
4. Bấm **Deploy**. Xong — không cần biến môi trường, không cần database.

> Bản Rust (nhánh `main`) là bản **desktop đầy đủ tính năng** (sessions kéo–thả,
> gộp hàng, soát lỗi QA, thống kê số từ, xuất Excel .xlsx, selftest 58 bài).
> Nhánh `vercel` là bản web rút gọn chạy 100% client-side: ghép + soát trạng
> thái + xuất CSV/TSV/HTML/JSON.

## LaBSE chạy trong trình duyệt (phần cốt lõi)

- Model **`Xenova/LaBSE` ONNX int8 (~450 MB) — cùng file với bản desktop Rust** —
  được tải từ Hugging Face **đúng 1 lần**, lưu trong **OPFS** (bộ nhớ đĩa riêng
  của app trong trình duyệt). Các lần mở lại trang: ~10 giây là sẵn sàng.
- Suy luận chạy trong **Web Worker** riêng — giao diện không bao giờ đóng băng.
- Thuật toán ghép (từ vựng liên ngôn ngữ + DP đơn điệu + hoà trộn LaBSE
  `max(từ vựng, 0.4·từ vựng + 0.6·LaBSE)`) là **port 1:1 từ `src/aligner.rs`**
  — kết quả khớp bản desktop (đã đối chiếu từng phần nghìn điểm).

## Phát triển tại chỗ

```bash
cd web-src
bun install        # hoặc npm install
bun run dev        # http://localhost:3000
```

Sửa engine LaBSE (`src/lib/reva/labse-engine.ts`) xong thì chạy lại:

```bash
bun run build:worker   # dựng lại public/labse-worker.js (bundle IIFE)
```

## Cấu trúc chính

```
web-src/
├── src/app/page.tsx              # giao diện REVA Web (ghép + soát + xuất)
├── src/lib/reva/aligner.ts       # port 1:1 từ aligner.rs (parity tuyệt đối)
├── src/lib/reva/lexicon.ts       # từ điển Anh→Việt (sinh từ lexicon.rs)
├── src/lib/reva/labse-engine.ts  # LaBSE: tokenizer v2 + phiên ORT trực tiếp + OPFS
├── src/lib/reva/labse-worker.ts  # Web Worker entry (bundle IIFE)
├── public/labse-worker.js        # worker dựng sẵn (bun run build:worker)
└── public/ort/                   # ONNX Runtime 1.14 wasm (self-host)
```

## Yêu cầu máy của người dùng cuối

- Trình duyệt hiện đại (Chrome/Edge/Firefox/Safari 15.2+).
- Lần đầu: ~450 MB tải + ~1,5 GB RAM trống khi dựng model (watchdog tự báo lỗi
  rõ ràng nếu hết bộ nhớ — bộ từ vựng vẫn ghép bình thường).
