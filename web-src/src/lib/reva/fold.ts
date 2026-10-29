/// fold.ts — "ASCII fold": bỏ dấu tiếng Việt để so khớp không phân biệt dấu.
/// Port 1:1 từ src/fold.rs của bản desktop: "Hà Nội" -> "ha noi", "Đà Nẵng" -> "da nang".

/** Bỏ dấu một chuỗi (giữ chữ cái/số, lowercase). NFKD → lọc combining mark → đ → d. */
export function foldLower(s: string): string {
  const nfkd = s.normalize("NFKD");
  let out = "";
  for (const ch of nfkd) {
    const cp = ch.codePointAt(0)!;
    if (cp >= 0x0300 && cp <= 0x036f) continue; // combining diacritics
    out += ch;
  }
  return out.toLowerCase().replace(/đ/g, "d");
}

/** Chuẩn hoá token số: bỏ dấu phân cách hàng nghìn. "4,000"/"4.000" -> "4000". */
export function foldNumberToken(tok: string): string | undefined {
  if (!tok) return undefined;
  let allDigits = true;
  let hasSep = false;
  let digitRun = 0;
  for (const ch of tok) {
    if (ch >= "0" && ch <= "9") {
      digitRun++;
    } else if (ch === "," || ch === ".") {
      if (digitRun === 0) return undefined; // ".5" hay "5." lệch dạng
      hasSep = true;
    } else {
      allDigits = false;
      break;
    }
  }
  if (!allDigits || digitRun === 0) return undefined;
  return hasSep ? tok.replace(/[^0-9]/g, "") : tok;
}
