#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""labse_service.py — dịch vụ nhúng câu LaBSE cục bộ cho "Cánh đôi Anh–Việt".

Chạy 100% trên máy bạn (không gửi văn bản đi đâu):
  * Nạp model LaBSE (sentence-transformers/LaBSE, ~1.8 GB, tải 1 lần rồi cache).
  * Lắng nghe tại 127.0.0.1:3031 (chỉ máy này kết nối được).
  * GET  /health              -> {"status":"ok","model":"LaBSE","dim":768}
  * POST /embed {"texts":[..]} -> {"embeddings":[[...], ...]}  (đã chuẩn hoá L2)

Biến môi trường:
  LABSE_PORT  — cổng (mặc định 3031)
  LABSE_MODEL — tên model (mặc định sentence-transformers/LaBSE)

App Rust tự động nhận diện service này (huy hiệu đổi thành
"Từ vựng + LaBSE — chính xác cao"). Không có service thì app vẫn chạy
bằng bộ từ vựng như thường.
"""
import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

PORT = int(os.environ.get("LABSE_PORT", "3031"))
MODEL_NAME = os.environ.get("LABSE_MODEL", "sentence-transformers/LaBSE")
BATCH = 32          # số câu mỗi lô suy luận (đỡ RAM/CPU)
MAX_TEXTS = 4096    # số đoạn tối đa mỗi request

print("[labse] đang nạp sentence-transformers + LaBSE…", flush=True)
from sentence_transformers import SentenceTransformer  # noqa: E402

_model = None
_dim = 0
_lock = threading.Lock()  # chỉ 1 luồng suy luận cùng lúc (model không thread-safe)


def _model_dim(model) -> int:
    """Lấy số chiều vector — tương thích cả sentence-transformers mới (6.x:
    get_embedding_dimension) lẫn cũ (<=5.x: get_sentence_embedding_dimension)."""
    try:
        return int(model.get_embedding_dimension())
    except AttributeError:
        return int(model.get_sentence_embedding_dimension())


def _encode(texts):
    """Nhúng danh sách câu, trả list vector đã chuẩn hoá L2 (cosine = tích vô hướng)."""
    return _model.encode(
        texts,
        normalize_embeddings=True,
        show_progress_bar=False,
        batch_size=BATCH,
    ).tolist()


class Handler(BaseHTTPRequestHandler):
    def _json(self, code, obj):
        payload = json.dumps(obj).encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)

    def do_GET(self):
        if self.path == "/health":
            self._json(200, {"status": "ok", "model": "LaBSE", "dim": _dim})
        else:
            self._json(404, {"error": "not found"})

    def do_POST(self):
        if self.path != "/embed":
            self._json(404, {"error": "not found"})
            return
        try:
            length = int(self.headers.get("Content-Length", "0"))
            body = json.loads(self.rfile.read(length) or b"{}")
        except Exception as e:
            self._json(400, {"error": "json khong doc duoc: %s" % e})
            return
        texts = body.get("texts")
        if not isinstance(texts, list) or not all(isinstance(t, str) for t in texts):
            self._json(400, {"error": "body phai la {\"texts\": [chuoi, ...]}"})
            return
        if len(texts) > MAX_TEXTS:
            self._json(400, {"error": "qua nhieu doan (toi da %d moi request)" % MAX_TEXTS})
            return
        if not texts:
            self._json(200, {"embeddings": []})
            return
        t0 = time.time()
        try:
            with _lock:
                embeddings = _encode(texts)
        except Exception as e:
            self._json(500, {"error": "loi encode: %s" % e})
            return
        ms = int((time.time() - t0) * 1000)
        print("[labse] nhúng %d đoạn (%d ms)" % (len(texts), ms), flush=True)
        self._json(200, {"embeddings": embeddings})

    def log_message(self, fmt, *args):
        pass  # đã tự log ở handler


def main():
    global _model, _dim
    print("[labse] đang nạp model %s …" % MODEL_NAME, flush=True)
    _model = SentenceTransformer(MODEL_NAME)
    _dim = _model_dim(_model)
    print("[labse] LaBSE sẵn sàng — vector %d chiều" % _dim, flush=True)
    srv = ThreadingHTTPServer(("127.0.0.1", PORT), Handler)
    print("[labse] lắng nghe tại http://127.0.0.1:%d (Ctrl+C để dừng)" % PORT, flush=True)
    try:
        srv.serve_forever()
    except KeyboardInterrupt:
        print("\n[labse] đã dừng.", flush=True)


if __name__ == "__main__":
    main()
