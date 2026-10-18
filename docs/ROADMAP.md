# ROADMAP — nghiên cứu các tool cùng mục đích (2026-10-18)

> Nghiên cứu bằng **web thật** trong phiên ngày 2026-10-18 (web-search + đọc trực tiếp repo/LICENSE).
> Mỗi dòng ghi rõ giấy phép **đã đọc file/LICENSE thật** trừ khi ghi "chưa xác minh".
> Nguyên tắc đánh giá: REVA chạy **100% cục bộ** · **không dịch hộ** · nhẹ · người không rành kỹ thuật dùng được.

## 1. Các tool đã xem

| Tool | Giấy phép (bằng chứng) | Thuật toán / ý tưởng đáng học | Phù hợp REVA? |
|---|---|---|---|
| **Hunalign** ([repo](https://github.com/danielvarga/hunalign)) | **LGPL-3.0** — đọc file `LICENSE` | Từ điển + **Gale-Church** (độ dài câu); không có từ điển thì dùng độ dài → **tự dựng từ điển từ kết quả lượt 1 rồi ghép lại lượt 2** (2-pass bootstrap); không ghép chéo (đơn điệu — giống REVA) | ★★★ ý tưởng; ❌ chép mã (LGPL) |
| **Bleualign** ([repo](https://github.com/rsennrich/bleualign)) | **GPL-2.0** — đọc file `LICENSE` | Cần **đầu ra máy dịch** rồi chấm BLEU từng cặp | ✗ mâu thuẫn nguyên tắc "không dịch hộ" + phụ thuộc MT |
| **Vecalign** ([repo](https://github.com/thompsonb/vecalign)) | **Apache-2.0** — đọc file `LICENSE`, **không có NOTICE** | Embedding đa ngôn ngữ (LASER) + xấp xỉ Dynamic Time Warping chạy nhanh với tài liệu rất dài; hỗ trợ các kiểu chồng lấp 1-1, 1-2, 2-1, 1-3, 3-1, 2-2 | ★★ ý tưởng (mẫu chồng lấp tự động); ✓ được phép chép mã nếu cần (phải giữ header bản quyền + ghi THIRD_PARTY) |
| **Bertalign** ([repo](https://github.com/bfsujason/bertalign)) | **GPL-3.0** — đọc file `LICENCE` | sentence-transformers + DP **2 bước** (thô → tinh, thu hẹp vùng tìm) — bản chất là cùng hướng REVA + LaBSE đã làm | ý tưởng trùng hiện trạng; ❌ chép mã (GPL) |
| **LF Aligner** ([trang](https://langui.ch/tool/lf-aligner/)) | **GPL-3+** — đọc mô tả giấy phép trên langui.ch | **Giao diện đồ hoạ bọc hunalign + trình soạn thảo căn chỉnh + xuất TMX** — đúng tệp người dùng của REVA (dịch giả tạo bộ nhớ dịch từ 2 file đã dịch) | ★★★ **đối thủ gần nhất về UX**; bài học: TMX là định dạng "ra" mà mọi CAT tool đọc |
| **OmegaT** ([trang](https://omegat.org)) | **GPL-3.0** — ghi trên trang SourceForge project | CAT tool mã nguồn mở lớn nhất; **không có aligner tích hợp** — nhập TMX từ tool ngoài | ★★★ không cần chép gì: REVA xuất **TMX** thì OmegaT (và Trados/CafeTrillian…) nhập thẳng |
| **Bitextor** ([repo](https://github.com/bitextor/bitextor)) | **GPL-3.0** — đọc file `COPYING` | Pipeline khai thác bitext từ website hàng loạt (crawler + crawler投向 document alignment + sentence alignment) | ✗ mục đích khác (corpus mining, không phải soát từng bài) |
| **EVBCorpus** ([repo](https://github.com/qhungngo/EVBCorpus)) | **KHÔNG có giấy phép** trong README (đã kiểm) | 20 triệu từ Anh–Việt song ngữ — tiềm năng đối chiếu chất lượng ghép | ✗ theo quy tắc: không chép dữ liệu khi không có giấy phép; sau này nếu tác giả cấp phép mới xét dùng cho từ điển |
| **Draftable / YouAlign** | thương mại | so sánh văn bản song song (side-by-side) | tham khảo UX; không mở |

## 2. Đề xuất hướng đi (xếp theo giá trị / rủi ro)

### Nhóm A — đáng làm sớm (giá trị cao, rủi ro thấp)

1. **Xuất TMX 1.2** (bổ sung định dạng thứ 8) — `feat/tmx-export`
   - Vì sao: LF Aligner tồn tại chủ yếu để đổ kết quả vào CAT tool qua TMX; REVA chưa có TMX là thiếu sót lớn nhất so với dòng chảy công việc của dịch giả. TMX 1.2 là XML đơn giản, serialization thuần Rust, dễ test.
   - Rủi ro: gần như 0 (không đụng thuật toán ghép; bất biến giữ nguyên).
2. **Rescan toàn bộ** (một nút chấm lại điểm mọi hàng sau khi kéo–thả) — `feat/rescan-all`
   - Vì sao: sau khi sắp xếp lại 10+ hàng, bấm Rescan từng hàng là cực; đây là mục "việc có thể làm tiếp" đã nằm trong worklog từ v2.5.
   - Rủi ro: thấp (dùng lại `rescan_row` từng hàng, thêm hàm vòng lặp + API + nút).
3. **Kéo–thả file .txt vào ô nhập** (đọc cục bộ bằng FileReader) — `feat/drag-drop-txt`
   - Vì sao: người dùng hiện phải mở file → copy → paste; non-developer thường có bản dịch dạng .txt.
   - Rủi ro: 0 backend (thuần UI); rà bằng agent-browser.

### Nhóm B — cân nhắc sau (giá trị cao hơn nhưng rủi ro/công sức lớn hơn)

4. **Đặc trưng Gale-Church (độ dài)** vào chấm điểm — ý tưởng từ hunalign (thuật toán công bố, không chép mã): giúp văn bản ít số/tên riêng khi chưa bật LaBSE. Rủi ro: thay đổi điểm → phải đối chiếu lại toàn bộ selftest chất lượng; làm riêng PR có bộ so sánh A/B.
5. **2-pass auto-lexicon bootstrap** (hunalign): lượt 1 ghép bằng từ vựng sẵn có → trích cặp "tự khớp" điểm cao → dồn tần suất làm từ điển mở rộng → ghép lượt 2. Rủi ro trung bình; giữ bất biến "điểm từ vựng là sàn" khi hoà trộn.
6. **Gợi ý tự động ghép 1-2 / 2-1** (Vecalign-style overlap): hôm nay người dùng phải tự kéo; tool gợi ý "hàng này có vẻ 1 EN ↔ 2 VI" sẽ giảm thao tác. Rủi ro: cẩn thận không phá bất biến I1/I2/I4/I5; chỉ **gợi ý**, người dùng bấm để áp dụng (đưa vào undo được).
7. **Tách câu trong đoạn dài trước khi ghép** (sentence-level như các tool trên): hiện REVA ghép cấp đoạn văn; dịch sách/lịch thiệp có đoạn siêu dài sẽ cần. Thay đổi lớn, cân nhắc "chế độ nâng cao" opt-in.
8. **So sánh chất lượng trên EVBCorpus** (nếu được cấp phép dữ liệu): bộ đối chiếu chuẩn Anh–Việt để đo precision/recall của REVA so với hunalign/vecalign — biến "selftest 39 bài" thành benchmark ngoài.

### Nhóm C — không làm

- Bleualign-style (cần MT): phá nguyên tắc "không dịch hộ", thêm phụ thuộc nặng.
- Bitextor-style crawling: mục đích khác (khai thác corpus hàng loạt, không phải soát từng bài cho 1 người dùng).
- Copying mã GPL/LGPL (hunalign, bleualign, bertalign, LF Aligner, Bitextor): vi phạm cam kết MIT của REVA. Chỉ học thuật toán đã công bố.

## 3. Liên kết ngôn ngữ Anh–Việt đáng chú ý (đã tìm)

- `EVBCorpus` (không giấy phép — chỉ tham khảo): https://github.com/qhungngo/EVBCorpus
- "Improving Sentence Alignment Method for Low-resource English-Vietnamese" (Paclic 2021): https://aclanthology.org/2021.paclic-1.38.pdf — chưa đọc chi tiết trong phiên này (ghi cho phiên sau).
- Hunalign từng được đối chiếu với Vecalign: "Evaluating automatic sentence alignment approaches" (Nature Sci. Rep. 2023): https://www.nature.com/articles/s41598-023-47479-w — chưa đọc chi tiết.

## 4. Cần máy thật (không làm được trong sandbox)

- Đo thời gian LaBSE thật (model 2 GB) trên Windows/macOS thường dùng — đối chiếu với giới hạn timeout 600 s hiện tại.
- Kiểm TMX xuất từ REVA nhập vào OmegaT/Trados thật (bản quyền Trados không có trong sandbox).
- Đo trải nghiệm kéo–thả khối trên màn cảm ứng Windows/macOS thật.
