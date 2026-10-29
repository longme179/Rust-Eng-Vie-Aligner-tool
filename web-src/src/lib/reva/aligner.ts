/// aligner.ts — port 1:1 từ src/aligner.rs của REVA tool (bản desktop Rust).
///
/// Thuật toán: chấm điểm tương đồng từ vựng liên ngôn ngữ (số, danh từ riêng,
/// từ điển Anh–Việt, bigram nối) + số câu + DP đơn điệu (Needleman–Wunsch).
/// LaBSE (tuỳ chọn): cosine được HOÀ TRỘN theo công thức "chỉ nâng, không hạ"
/// (sàn từ vựng) trước khi vào DP — thuật toán DP giữ nguyên.
///
/// Bất biến an toàn kế thừa từ bản Rust: KHÔNG sentinel -1; mọi chỉ số thiếu là
/// null; DP dùng mảng phẳng; backtrack while (i>0||j>0) với nhánh riêng i==0/j==0.

import { foldLower, foldNumberToken } from "./fold";
import { lookupEn, isCommonWord } from "./lexicon";

// ========================= Hằng số (đồng bộ aligner.rs) =========================

export const MAX_PARAS = 2000;
export const MAX_DP_AREA = 4_000_000;

const W_LEX = 0.9;
const W_SENT = 0.1;
const NUMBER_BOOST = 3.0;
const PROPER_BOOST = 2.2;
const DICT_BOOST = 0.9;
const BIGRAM_W = 0.55;
const GAP = 0.06;
const PAIR_BIAS = 0.3;
const POS_W = 0.1;

// ---- LaBSE ----
export const W_EMB = 0.6;
export const EMB_LO = 0.5;
export const EMB_HI = 0.8;

/** Hoà trộn điểm từ vựng (0..1) với cosine LaBSE (-1..1): max(từ vựng, hoà trộn). */
export function blendWithEmb(composite: number, embCos: number): number {
  const conf = Math.min(1, Math.max(0, (embCos - EMB_LO) / (EMB_HI - EMB_LO)));
  const blended = (1 - W_EMB) * composite + W_EMB * conf;
  return Math.max(composite, blended);
}

// ========================= Trạng thái =========================

export type Status = "auto_matched" | "needs_review" | "unmatched" | "human_confirmed";

export const STATUS_LABEL_VI: Record<Status, string> = {
  auto_matched: "Tự khớp",
  needs_review: "Cần soát",
  unmatched: "Không khớp",
  human_confirmed: "Đã xác nhận",
};

// ========================= Dữ liệu =========================

export interface Row {
  index: number;
  enIndex: number | null;
  viIndex: number | null;
  en: string | null;
  vi: string | null;
  status: Status;
  score: number | null;
}

export interface AlignOptions {
  autoThreshold: number;
}

export const DEFAULT_OPTIONS: AlignOptions = { autoThreshold: 0.3 };

export interface AlignOutput {
  english: string[];
  vietnamese: string[];
  rows: Row[];
  options: AlignOptions;
  /** "lexical" hoặc "lexical+labse" — như bản desktop. */
  engine: "lexical" | "lexical+labse";
}

// ========================= Tách đoạn =========================

/** Tách đoạn: ưu tiên dòng trống; 1 đoạn khổng lồ nhiều dòng → tách theo dòng. */
export function splitParagraphs(text: string): string[] {
  const norm = text.replace(/\r\n/g, "\n").replace(/\r/g, "\n");
  const paras: string[] = [];
  let cur = "";
  let blankPending = false;
  for (const line of norm.split("\n")) {
    const t = line.trim();
    if (!t) {
      if (cur) blankPending = true;
    } else {
      if (blankPending) {
        paras.push(cur);
        cur = "";
        blankPending = false;
      }
      if (cur) cur += " ";
      cur += t;
    }
  }
  if (cur) paras.push(cur);
  if (paras.length <= 1) {
    const single = paras[0];
    const lines = norm
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean);
    if (lines.length > 1) return lines;
    return single !== undefined ? [single] : [];
  }
  return paras;
}

// ========================= Token hoá =========================

const ALNUM = /[\p{L}\p{N}]/u;
const UPPER = /\p{Lu}/u;

interface ParaTokens {
  /** token đã fold (kể cả bigram nối) -> số lần xuất hiện */
  counts: Map<string, number>;
  /** token thuần số */
  numbers: Set<string>;
  /** danh từ riêng (đã fold) trong đoạn */
  proper: Set<string>;
  /** số câu */
  sent: number;
}

function buildParaTokens(para: string): ParaTokens {
  const foldedWords = tokenizeWords(para);
  const counts = new Map<string, number>();
  const numbers = new Set<string>();
  const proper = new Set<string>();

  // Danh từ riêng: quét trên chuỗi GỐC (giữ chữ hoa) — port logic aligner.rs
  // (idx_normal_start luôn true ⇒ chỉ từ ALL-CAPS tính là danh từ riêng).
  for (const word of rawWords(para)) {
    const first = [...word][0];
    if (first && UPPER.test(first)) {
      const f = foldLower(word);
      const allCaps = [...word].every((ch) => UPPER.test(ch)) && [...word].length >= 2;
      const common = isCommonWord(f);
      const longEnough = [...word].length >= 2;
      if (longEnough && !common && allCaps) proper.add(f);
    }
  }

  for (const w of foldedWords) {
    counts.set(w, (counts.get(w) ?? 0) + 1);
    const num = foldNumberToken(w);
    if (num !== undefined) numbers.add(num);
  }
  // bigram nối: "ha noi" -> "hanoi"
  for (let i = 0; i + 1 < foldedWords.length; i++) {
    const joined = foldedWords[i] + foldedWords[i + 1];
    counts.set(joined, (counts.get(joined) ?? 0) + BIGRAM_W);
  }
  return { counts, numbers, proper, sent: countSentences(para) };
}

/** Tách từ (chữ/số liên tiếp), fold + chuẩn hoá số. */
function tokenizeWords(text: string): string[] {
  const out: string[] = [];
  let cur = "";
  let hasDigit = false;
  for (const ch of text) {
    if (ALNUM.test(ch)) {
      cur += ch;
      if (ch >= "0" && ch <= "9") hasDigit = true;
    } else if (cur) {
      pushToken(out, cur, hasDigit);
      cur = "";
      hasDigit = false;
    }
  }
  if (cur) pushToken(out, cur, hasDigit);
  return out;
}

function pushToken(out: string[], raw: string, hasDigit: boolean) {
  const folded = foldLower(raw);
  if (!folded) return;
  if (hasDigit) {
    const num = foldNumberToken(folded);
    if (num !== undefined) {
      out.push(num);
      return;
    }
  }
  out.push(folded);
}

/** Từ gốc (giữ hoa/thường) để nhận diện danh từ riêng. */
function rawWords(text: string): string[] {
  const out: string[] = [];
  let cur = "";
  for (const ch of text) {
    if (ALNUM.test(ch)) {
      cur += ch;
    } else if (cur) {
      out.push(cur);
      cur = "";
    }
  }
  if (cur) out.push(cur);
  return out;
}

function countSentences(para: string): number {
  let n = 0;
  const chars = [...para];
  for (let i = 0; i < chars.length; i++) {
    const c = chars[i];
    if (c === "." || c === "!" || c === "?" || c === "…") {
      const next = chars[i + 1];
      if (next === undefined || /\s/.test(next)) n++;
    }
  }
  return Math.max(1, n);
}

// ========================= Chấm điểm =========================

interface Side {
  ptoks: ParaTokens[];
  df: Map<string, number>;
}

function buildSide(paras: string[]): Side {
  const ptoks = paras.map(buildParaTokens);
  const df = new Map<string, number>();
  for (const pt of ptoks) {
    for (const t of pt.counts.keys()) df.set(t, (df.get(t) ?? 0) + 1);
  }
  return { ptoks, df };
}

function idf(side: Side, token: string): number {
  const n = Math.max(1, side.ptoks.length);
  const d = (side.df.get(token) ?? 0) + 1;
  return Math.log(1 + n / d);
}

/** Lõi chấm điểm: cosine từ vựng có idf + từ điển Anh→Việt + số câu. */
function scoreTokens(pe: ParaTokens, pv: ParaTokens, en: Side, vi: Side): number {
  // ---- trọng số phía Việt
  const wVi = new Map<string, number>();
  let normVi = 0;
  for (const [t, cnt] of pv.counts) {
    let w = cnt * idf(vi, t);
    if (pv.numbers.has(t)) w *= NUMBER_BOOST;
    else if (pv.proper.has(t)) w *= PROPER_BOOST;
    normVi += w * w;
    wVi.set(t, w);
  }

  // ---- trọng số phía Anh (đích + mở rộng từ điển)
  const wEn = new Map<string, number>();
  let normEn = 0;
  for (const [t, cnt] of pe.counts) {
    let w = cnt * idf(en, t);
    if (pe.numbers.has(t)) w *= NUMBER_BOOST;
    else if (pe.proper.has(t)) w *= PROPER_BOOST;
    normEn += w * w;
    wEn.set(t, (wEn.get(t) ?? 0) + w);
  }
  // pseudo-token từ điển Anh→Việt
  const dictExpansions: Array<[string, number]> = [];
  for (const t of pe.counts.keys()) {
    if (pe.numbers.has(t)) continue;
    const trans = lookupEn(t);
    if (trans) {
      const base = idf(en, t) * DICT_BOOST;
      for (const vt of trans) dictExpansions.push([vt, base]);
    }
  }
  for (const [vt, base] of dictExpansions) {
    wEn.set(vt, (wEn.get(vt) ?? 0) + base);
    normEn += base * base;
  }

  // ---- cosine
  let dot = 0;
  for (const [t, w] of wEn) {
    const wv = wVi.get(t);
    if (wv !== undefined) dot += w * wv;
  }
  const denom = Math.sqrt(normEn) * Math.sqrt(normVi);
  const lexical = denom > 1e-12 ? Math.min(1, Math.max(0, dot / denom)) : 0;

  // ---- số câu
  const sa = pe.sent;
  const sb = pv.sent;
  const sent = 1 - Math.abs(sa - sb) / Math.max(sa, sb, 1);

  return W_LEX * lexical + W_SENT * sent;
}

// ========================= DP đơn điệu =========================

/** Ma trận cha: 0 = ghép, 1 = bỏ qua EN, 2 = bỏ qua VI — lưu phẳng i*(m+1)+j. */
function dpAlign(raw: Float64Array, n: number, m: number): Uint8Array {
  const m1 = m + 1;
  const dp = new Float64Array((n + 1) * m1);
  const parent = new Uint8Array((n + 1) * m1);
  dp[0] = 0;
  for (let i = 1; i <= n; i++) {
    dp[i * m1] = dp[(i - 1) * m1] - GAP;
    parent[i * m1] = 1;
  }
  for (let j = 1; j <= m; j++) {
    dp[j] = dp[j - 1] - GAP;
    parent[j] = 2;
  }
  const nn = Math.max(1, n);
  const mm = Math.max(1, m);
  for (let i = 1; i <= n; i++) {
    for (let j = 1; j <= m; j++) {
      const s = raw[(i - 1) * m + (j - 1)];
      const pos = 1 - Math.abs(i / nn - j / mm);
      let best = dp[(i - 1) * m1 + (j - 1)] + s - PAIR_BIAS + POS_W * pos;
      let code = 0;
      const skipE = dp[(i - 1) * m1 + j] - GAP;
      if (skipE > best) {
        best = skipE;
        code = 1;
      }
      const skipV = dp[i * m1 + (j - 1)] - GAP;
      if (skipV > best) {
        best = skipV;
        code = 2;
      }
      dp[i * m1 + j] = best;
      parent[i * m1 + j] = code;
    }
  }
  return parent;
}

// ========================= Ghép chính =========================

export type AlignResult =
  | { ok: true; output: AlignOutput }
  | { ok: false; error: string };

/**
 * Ghép hai văn bản — port `align_with_emb` của bản Rust.
 * `embCos` (tuỳ chọn): ma trận cosine EN×VI phẳng (hàng trước), độ dài đúng n*m —
 * sai kích thước thì bị BỎ QUA an toàn (fallback từ vựng, không văng).
 */
export function alignWithEmb(
  englishText: string,
  vietnameseText: string,
  options: AlignOptions,
  embCos?: Float64Array | number[] | null
): AlignResult {
  const opts: AlignOptions = {
    autoThreshold: Math.min(0.9, Math.max(0.05, options.autoThreshold)),
  };

  const english = splitParagraphs(englishText);
  const vietnamese = splitParagraphs(vietnameseText);

  if (!english.length && !vietnamese.length) {
    return {
      ok: false,
      error:
        "Chưa có văn bản nào để ghép — hãy dán ít nhất một bên (tiếng Anh hoặc tiếng Việt).",
    };
  }
  if (english.length > MAX_PARAS || vietnamese.length > MAX_PARAS) {
    return {
      ok: false,
      error: `Quá nhiều đoạn (tối đa ${MAX_PARAS} đoạn mỗi bên). Hiện có: EN=${english.length}, VI=${vietnamese.length}.`,
    };
  }
  const n = english.length;
  const m = vietnamese.length;
  if (n * m > MAX_DP_AREA) {
    return {
      ok: false,
      error: `Hai văn bản lệch nhau quá nhiều (EN=${n} đoạn, VI=${m} đoạn). Hãy tách nhỏ bài rồi ghép từng phần.`,
    };
  }

  const enSide = buildSide(english);
  const viSide = buildSide(vietnamese);

  const emb =
    embCos && embCos.length === n * m ? (embCos as ArrayLike<number>) : null;

  // ---- ma trận điểm gốc n×m
  const rawScores = new Float64Array(n * m);
  for (let i = 0; i < n; i++) {
    const pe = enSide.ptoks[i];
    for (let j = 0; j < m; j++) {
      const composite = scoreTokens(pe, viSide.ptoks[j], enSide, viSide);
      rawScores[i * m + j] = emb
        ? blendWithEmb(composite, emb[i * m + j])
        : composite;
    }
  }

  const parent = dpAlign(rawScores, n, m);

  // ---- Backtrack AN TOÀN (không sentinel, không trừ xuống dưới 0)
  type Event = { t: 0; a: number; b: number } | { t: 1; a: number } | { t: 2; b: number };
  const events: Event[] = [];
  let i = n;
  let j = m;
  const m1 = m + 1;
  while (i > 0 || j > 0) {
    const code = i === 0 ? 2 : j === 0 ? 1 : parent[i * m1 + j];
    if (code === 0) {
      i--;
      j--;
      events.push({ t: 0, a: i, b: j });
    } else if (code === 1) {
      i--;
      events.push({ t: 1, a: i });
    } else {
      j--;
      events.push({ t: 2, b: j });
    }
  }
  events.reverse();

  // ---- dựng hàng
  const rows: Row[] = [];
  for (const ev of events) {
    if (ev.t === 0) {
      const s = rawScores[ev.a * m + ev.b];
      rows.push({
        index: rows.length,
        enIndex: ev.a,
        viIndex: ev.b,
        en: english[ev.a],
        vi: vietnamese[ev.b],
        status: s >= opts.autoThreshold ? "auto_matched" : "needs_review",
        score: Math.round(s * 1000) / 1000,
      });
    } else if (ev.t === 1) {
      rows.push({
        index: rows.length,
        enIndex: ev.a,
        viIndex: null,
        en: english[ev.a],
        vi: null,
        status: "unmatched",
        score: null,
      });
    } else {
      rows.push({
        index: rows.length,
        enIndex: null,
        viIndex: ev.b,
        en: null,
        vi: vietnamese[ev.b],
        status: "unmatched",
        score: null,
      });
    }
  }

  const output: AlignOutput = {
    english,
    vietnamese,
    rows,
    options: opts,
    engine: emb ? "lexical+labse" : "lexical",
  };
  const inv = validateInvariants(output);
  if (inv) return { ok: false, error: `Vi phạm bất biến nội bộ: ${inv}` };
  return { ok: true, output };
}

// ========================= Bất biến (5 điều — port để tự kiểm) =========================

export function validateInvariants(out: AlignOutput): string | null {
  const n = out.english.length;
  const m = out.vietnamese.length;
  const seenEn = new Array<boolean>(n).fill(false);
  const seenVi = new Array<boolean>(m).fill(false);
  let lastEn: number | null = null;
  let lastVi: number | null = null;
  for (const row of out.rows) {
    const hasEn = row.enIndex !== null;
    const hasVi = row.viIndex !== null;
    if (!hasEn && !hasVi) return `hàng ${row.index}: rỗng cả hai bên`;
    if (row.status === "unmatched" && hasEn && hasVi)
      return `hàng ${row.index}: unmatched nhưng đủ hai bên`;
    if (row.status !== "unmatched" && row.status !== "human_confirmed" && (!hasEn || !hasVi))
      return `hàng ${row.index}: trạng thái ${row.status} nhưng thiếu một bên`;
    const a = row.enIndex;
    if (a !== null) {
      if (a >= n) return `hàng ${row.index}: enIndex ${a} >= ${n}`;
      if (seenEn[a]) return `đoạn tiếng Anh thứ ${a} bị dùng 2 lần`;
      seenEn[a] = true;
      if (lastEn !== null && a <= lastEn) return `I3: đoạn Anh không đơn điệu tại ${a}`;
      lastEn = a;
    }
    const b = row.viIndex;
    if (b !== null) {
      if (b >= m) return `hàng ${row.index}: viIndex ${b} >= ${m}`;
      if (seenVi[b]) return `đoạn tiếng Việt thứ ${b} bị dùng 2 lần`;
      seenVi[b] = true;
      if (lastVi !== null && b <= lastVi) return `I3: đoạn Việt không đơn điệu tại ${b}`;
      lastVi = b;
    }
  }
  for (let k = 0; k < seenEn.length; k++)
    if (!seenEn[k]) return `đoạn tiếng Anh thứ ${k} bị bỏ sót`;
  for (let k = 0; k < seenVi.length; k++)
    if (!seenVi[k]) return `đoạn tiếng Việt thứ ${k} bị bỏ sót`;
  return null;
}
