#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""download_model.py — đảm bảo model LaBSE có SẴN trên máy, TẢI 1 LẦN DUY NHẤT.

Từ bản 2.6, model được lưu trong thư mục riêng `labse/model/` (nằm cạnh script,
đổi vị trí được bằng biến môi trường REVA_LABSE_MODEL_DIR) — KHÔNG phụ thuộc cache
của HuggingFace. Nhờ vậy:

  * build / cập nhật mã nguồn bao nhiêu lần cũng KHÔNG tải lại model
    (script thoát ngay trong tích tắc, không cần mạng);
  * xoá libreto `~/.cache/huggingface` hay đổi phiên bản sentence-transformers
    cũng không làm mất model;
  * dịch vụ labse_service.py nạp thẳng từ thư mục này ở CHẾ ĐỘ OFFLINE.

Chạy từ môi trường đã cài sentence-transformers. Chạy lại bất cứ lúc nào —
an toàn và gần như tức thời nếu model đã có.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MODEL_DIR = os.environ.get("REVA_LABSE_MODEL_DIR", os.path.join(HERE, "model"))
HUB_NAME = os.environ.get("LABSE_MODEL", "sentence-transformers/LaBSE")


def _weights_ok(model_dir):
    for w in ("model.safetensors", "pytorch_model.bin"):
        if os.path.isfile(os.path.join(model_dir, w)):
            return True
    return False


def have_local_model(model_dir=MODEL_DIR):
    """True khi labse/model đã đủ (config.json + file trọng số)."""
    return (
        os.path.isdir(model_dir)
        and os.path.isfile(os.path.join(model_dir, "config.json"))
        and _weights_ok(model_dir)
    )


def main():
    if have_local_model():
        print(
            "[labse] model đã có sẵn tại %s — KHÔNG tải lại, KHÔNG cần mạng." % MODEL_DIR
        )
        return 0
    print(
        "[labse] chưa có model cục bộ — tải %s (lần đầu ~1.8 GB, có thể lâu)…" % HUB_NAME
    )
    try:
        from sentence_transformers import SentenceTransformer
    except Exception as e:
        print("Chua cai duoc sentence-transformers: %s" % e)
        return 1
    try:
        m = SentenceTransformer(HUB_NAME)  # hưởng cache HF nếu đã từng tải
    except Exception as e:
        print("Khong tai duoc model tu hub: %s" % e)
        print("  — kiểm tra mạng rồi chạy lại script này.")
        return 1
    os.makedirs(MODEL_DIR, exist_ok=True)
    try:
        m.save(MODEL_DIR)
    except Exception as e:
        print("Khong luu duoc model vao %s: %s" % (MODEL_DIR, e))
        return 1
    if not have_local_model():
        print("Luu xong nhung khong thay file trong so o %s — xem lai!" % MODEL_DIR)
        return 1
    print("[labse] đã lưu model vào %s" % MODEL_DIR)
    print("[labse] từ giờ MỌI lần chạy/build lại đều dùng model này, không tải lại.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
