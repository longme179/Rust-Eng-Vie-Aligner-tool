# THIRD_PARTY — giấy phép mã/dữ liệu bên ngoài dùng trong REVA

> Nguyên tắc: REVA là MIT. Chỉ nhúng mã có giấy phép nới lỏng (MIT/BSD/Apache-2.0/ISC),
> giữ nguyên thông báo bản quyền và ghi nguồn tại đây. Mã copyleft (GPL/LGPL/AGPL) **không** chép.

## 1. Phụ thuộc crate (cargo) — theo Cargo.toml/Cargo.lock

| Crate | Giấy phép | Cách dùng | Ghi chú |
|---|---|---|---|
| `axum` | MIT | HTTP server (framework của app) | |
| `tokio` | MIT | runtime async | |
| `serde` / `serde_json` | MIT | (de)serialization JSON | |
| `ureq` | MIT/Apache-2.0 | HTTP client tải URL + tải model LaBSE | dual-licensed |
| `unicode-normalization` | MIT/Apache-2.0 | NFKD bỏ dấu tiếng Việt (fold) | dual-licensed |
| `rust_xlsxwriter` | MIT | xuất Excel .xlsx | |
| `zip` | MIT | dependency của rust_xlsxwriter (đọc zip trong selftest dùng crate này) | |
| `ort` | MIT/Apache-2.0 | chạy model ONNX LaBSE **ngay trong tiến trình** (bản 2.10) — nạp động thư viện ONNX Runtime, KHÔNG biên dịch/nhúng mã C++ | dual-licensed |
| `tokenizers` | Apache-2.0 | tokenize câu cho LaBSE (bản Rust chính thức của Hugging Face, feature `onig`) | |
| `flate2` | MIT/Apache-2.0 | giải nén gzip khi tự tải model | dual-licensed |
| `tar` | MIT/Apache-2.0 | giải nén `.tgz` thư viện ONNX Runtime | dual-licensed |

## 2. Phần mềm/model TẢI LÚC CHẠY (bản 2.10 — KHÔNG nhúng vào repo/binary)

App tự tải đúng **1 lần** vào `data/models/` (có nối lại khi đứt mạng); không kèm trong repo — repo chỉ lưu mã nguồn.

| Thành phần | Giấy phép | Nguồn | Vai trò |
|---|---|---|---|
| **ONNX Runtime** v1.31.1 (Microsoft) | MIT | `github.com/microsoft/onnxruntime/releases` | thư viện suy luận ONNX (`onnxruntime.dll` / `libonnxruntime.so` / `dylib`), nạp động qua `ort::init_from` |
| **LaBSE** — bản ONNX lượng tử hoá int8 (`model_quantized.onnx`, ~450 MB) | Apache-2.0 | `huggingface.co/Xenova/LaBSE` (model LaBSE gốc của Google là Apache-2.0) | nhúng câu đa ngôn ngữ — cốt lõi ghép chính xác |
| `tokenizer.json` LaBSE (~13 MB) | Apache-2.0 | `huggingface.co/Xenova/LaBSE` | bộ tokenizer chạy bằng crate `tokenizers` |

## 3. Mã nguồn ngoài ĐÃ chép vào REVA

*(hiện tại: KHÔNG có — toàn bộ mã REVA là viết riêng; các thuật toán tham khảo đều là thuật toán đã công bố, không sao chép mã)*

## 4. Mã/dữ liệu đã xem xét và **không** chép (2026-10-18)

| Nguồn | Giấy phép (đã đọc file thật) | Lý do không chép |
|---|---|---|
| [hunalign](https://github.com/danielvarga/hunalign) | LGPL-3.0 (`LICENSE`) | copyleft; chỉ học ý tưởng Gale-Church + 2-pass dictionary (thuật toán công bố) |
| [bleualign](https://github.com/rsennrich/bleualign) | GPL-2.0 (`LICENSE`) | copyleft + cần MT |
| [bertalign](https://github.com/bfsujason/bertalign) | GPL-3.0 (`LICENCE`) | copyleft |
| [bitextor](https://github.com/bitextor/bitextor) | GPL-3.0 (`COPYING`) | copyleft + mục đích khác |
| LF Aligner | GPL-3+ (mô tả langui.ch) | copyleft |
| [EVBCorpus](https://github.com/qhungngo/EVBCorpus) | **không có giấy phép** | không giấy phép → cấm chép dữ liệu |

## 5. Nếu sau này chép mã Apache-2.0 (ví dụ vecalign)

Vecalign ([thompsonb/vecalign](https://github.com/thompsonb/vecalign)) là Apache-2.0 (đã đọc `LICENSE`, repo không kèm NOTICE).
Khi chép: giữ nguyên header bản quyền Apache-2.0 của file gốc, ghi tại mục 2 ở trên kèm commit nguồn và đường dẫn.
