#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""wait_health.py — chờ dịch vụ LaBSE sẵn sàng (dùng trong script build Windows)."""
import sys
import time
import urllib.request

URL = "http://127.0.0.1:3031/health"
DEADLINE_SECONDS = 600  # CPU chậm + lần nạp đầu có thể mất vài phút


def main():
    deadline = time.time() + DEADLINE_SECONDS
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(URL, timeout=2) as r:
                print("LaBSE OK:", r.read().decode("utf-8"))
                sys.exit(0)
        except Exception:
            time.sleep(2)
    print("Het thoi gian cho LaBSE — xem labse-service.log")
    sys.exit(1)


if __name__ == "__main__":
    main()
