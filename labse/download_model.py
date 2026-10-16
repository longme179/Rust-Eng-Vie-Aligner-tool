#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""download_model.py — tải model LaBSE về máy (chỉ lần đầu, có cache của HuggingFace).

Chạy từ môi trường đã cài sentence-transformers. Có thể chạy lại bất cứ lúc nào —
nếu model đã có trong cache thì gần như không tải gì thêm.
"""
import sys

try:
    from sentence_transformers import SentenceTransformer
except Exception as e:
    print("Chua cai duoc sentence-transformers: %s" % e)
    sys.exit(1)

MODEL = "sentence-transformers/LaBSE"

print("Dang tai / kiem tra model %s (~1.8 GB lan dau, co cache)…" % MODEL)
m = SentenceTransformer(MODEL)


def _dim(model):
    try:
        return int(model.get_embedding_dimension())
    except AttributeError:
        return int(model.get_sentence_embedding_dimension())


print("LaBSE san sang: vector %d chieu." % _dim(m))
