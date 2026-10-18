# THIRD_PARTY — giấy phép mã/dữ liệu bên ngoài dùng trong REVA

> Nguyên tắc: REVA là MIT. Chỉ nhúng mã có giấy phép nới lỏng (MIT/BSD/Apache-2.0/ISC),
> giữ nguyên thông báo bản quyền và ghi nguồn tại đây. Mã copyleft (GPL/LGPL/AGPL) **không** chép.

## 1. Phụ thuộc crate (cargo) — theo Cargo.toml/Cargo.lock

| Crate | Giấy phép | Cách dùng | Ghi chú |
|---|---|---|---|
| `axum` | MIT | HTTP server (framework của app) | |
| `tokio` | MIT | runtime async | |
| `serde` / `serde_json` | MIT | (de)serialization JSON | |
| `ureq` | MIT/Apache-2.0 | HTTP client tải URL | dual-licensed |
| `unicode-normalization` | MIT/Apache-2.0 | NFKD bỏ dấu tiếng Việt (fold) | dual-licensed |
| `rust_xlsxwriter` | MIT | xuất Excel .xlsx | |
| `zip` | MIT | dependency của rust_xlsxwriter (đọc zip trong selftest dùng crate này) | |

## 2. Mã nguồn ngoài ĐÃ chép vào REVA

*(hiện tại: KHÔNG có — toàn bộ mã REVA là viết riêng; các thuật toán tham khảo đều là thuật toán đã công bố, không sao chép mã)*

## 3. Mã/dữ liệu đã xem xét và **không** chép (2026-10-18)

| Nguồn | Giấy phép (đã đọc file thật) | Lý do không chép |
|---|---|---|
| [hunalign](https://github.com/danielvarga/hunalign) | LGPL-3.0 (`LICENSE`) | copyleft; chỉ học ý tưởng Gale-Church + 2-pass dictionary (thuật toán công bố) |
| [bleualign](https://github.com/rsennrich/bleualign) | GPL-2.0 (`LICENSE`) | copyleft + cần MT |
| [bertalign](https://github.com/bfsujason/bertalign) | GPL-3.0 (`LICENCE`) | copyleft |
| [bitextor](https://github.com/bitextor/bitextor) | GPL-3.0 (`COPYING`) | copyleft + mục đích khác |
| LF Aligner | GPL-3+ (mô tả langui.ch) | copyleft |
| [EVBCorpus](https://github.com/qhungngo/EVBCorpus) | **không có giấy phép** | không giấy phép → cấm chép dữ liệu |

## 4. Nếu sau này chép mã Apache-2.0 (ví dụ vecalign)

Vecalign ([thompsonb/vecalign](https://github.com/thompsonb/vecalign)) là Apache-2.0 (đã đọc `LICENSE`, repo không kèm NOTICE).
Khi chép: giữ nguyên header bản quyền Apache-2.0 của file gốc, ghi tại mục 2 ở trên kèm commit nguồn và đường dẫn.
