/// labse-engine.ts — engine LaBSE chạy trong Web Worker (kiểm được cả trong Node).
///
/// Kiến trúc (rút từ thực chiến trên Chrome headless + sandbox 2 nhân):
///   * TOKENIZER qua @xenova/transformers v2 — file nhỏ, chạy tốt mọi nơi.
///   * MODEL KHÔNG đi qua from_pretrained của transformers.js: tự tải
///     `onnx/model_quantized.onnx` (~450 MB — CÙNG file int8 với bản desktop
///     Rust) bằng fetch có tiến trình, rồi dựng phiên ONNX Runtime TRỰC TIẾP
///     (`InferenceSession.create`) — 4,8 giây trên máy yếu; đường cũ từng TREO
///     VÔ HẠN lúc cache.put 449 MB.
///   * Cache API chỉ dùng để ĐỌC; ghi cache chạy nền kèm timeout — cache hỏng
///     không bao giờ làm treo app.
///   * ORT 1.14 (onnxruntime-web) — wasm self-host tại public/ort/ (không CDN),
///     1 luồng (SharedArrayBuffer cần COOP/COEP mà web tĩnh không có).
///   * Ưu tiên đầu ra `embeddings` → `pooler_output` → mean-pool
///     `last_hidden_state` theo attention mask (đúng thứ tự labse_native.rs)
///     rồi L2-chuẩn hoá — kết quả đồng nhất với bản desktop.

import ortPkg from "./ort-umd-shim";

type TF = typeof import("@xenova/transformers");

/// Kiểu cho ort UMD toàn cục (importScripts trong classic worker — xem ort-umd-shim.ts).
interface OrtSession {
  inputNames: string[];
  run(
    inputs: Record<string, unknown>
  ): Promise<Record<string, { data: unknown; dims: number[] }>>;
}
interface Ort {
  env: {
    wasm: {
      numThreads?: number;
      proxy?: boolean;
      wasmPaths?: string | { wasm?: string; mjs?: string };
    };
  };
  InferenceSession: {
    create(
      src: string | Uint8Array | ArrayBuffer,
      opts?: Record<string, unknown>
    ): Promise<OrtSession>;
  };
  Tensor: new (
    type: "int64",
    data: BigInt64Array,
    dims: [number, number]
  ) => unknown;
}
type ORT = Ort;

export type LabsePhase = "off" | "downloading" | "loading" | "ready" | "failed";

export interface LabseState {
  phase: LabsePhase;
  pct: number;
  detail: string | null;
}

export const MODEL_ID = "Xenova/LaBSE";
export const MODEL_LABEL = "Xenova/LaBSE (ONNX int8)";
const MODEL_URL =
  "https://huggingface.co/Xenova/LaBSE/resolve/main/onnx/model_quantized.onnx";
const CACHE_NAME = "reva-models-v1";

type Listener = (st: LabseState) => void;

let tf: TF | null = null;
let ort: ORT | null = ortPkg;
let tokenizer: InstanceType<TF["AutoTokenizer"]> | null = null;
let session: OrtSession | null = null;
let sessionInputs: Set<string> = new Set();
let state: LabseState = { phase: "off", pct: 0, detail: null };
let starting: Promise<void> | null = null;
const listeners = new Set<Listener>();

export function labseState(): LabseState {
  return state;
}

export function subscribeLabse(cb: Listener): () => void {
  listeners.add(cb);
  cb(state);
  return () => listeners.delete(cb);
}

function setState(next: Partial<LabseState>) {
  state = { ...state, ...next };
  for (const cb of listeners) cb(state);
}

export function labseReady(): boolean {
  return state.phase === "ready" && session !== null;
}

function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

// ========================= Dựng phiên ORT trực tiếp từ URL =========================
//
// TẮT đường "tự tải về buffer JS rồi tạo session": giữ 449 MB ở JavaScript ĐỒNG THỜI
// bản sao 449 MB trong heap wasm đẩy worker lên 1,88 GB RSS → kernel OOM-kill
// âm thầm (đã bắt gặp: nhịp heartbeat ngừng đúng lúc chuyển sang dựng phiên).
// Để ORT TỰ stream URL vào heap wasm: đỉnh ~1,4 GB. Tiến trình % lấy bằng cách
// chặn fetch/XHR trong worker — đếm byte của đúng file model rồi chuyển tiếp.

let interceptorsInstalled = false;

// ---- OPFS (Origin Private File System): bộ nhớ đĩa riêng của app, CÓ SẴN trong
// dedicated worker (khác Cache API). Lưu model 449 MB bằng STREAM (pipeTo ghi
// thẳng xuống đĩa — không tích luỹ RAM); mở lại trang: đọc stream từ đĩa feed
// thẳng vào heap wasm của ORT. Không mạng, không tăng đỉnh bộ nhớ.
const OPFS_NAME = "labse-model.onnx";

async function opfsFile(): Promise<File | null> {
  try {
    const root = await navigator.storage.getDirectory();
    const fh = await root.getFileHandle(OPFS_NAME);
    const f = await fh.getFile();
    return f.size === 471_527_949 ? f : null; // sai kích thước = bản ghi dở dang
  } catch {
    return null;
  }
}

async function opfsWrite(stream: ReadableStream<Uint8Array>): Promise<void> {
  try {
    const root = await navigator.storage.getDirectory();
    const fh = await root.getFileHandle(OPFS_NAME, { create: true });
    const writable = await fh.createWritable();
    await stream.pipeTo(writable); // stream → đĩa, RAM phẳng
    console.info("[reva-labse] đã lưu model vào bộ nhớ riêng — lần sau mở trang nhanh");
  } catch (e) {
    console.warn("[reva-labse] lưu model xuống đĩa lỗi (bỏ qua):", e);
    try {
      const root = await navigator.storage.getDirectory();
      await root.removeEntry(OPFS_NAME);
    } catch {
      /* bỏ qua */
    }
  }
}

function countingBody(
  body: ReadableStream<Uint8Array>,
  total: number,
  onPct: (frac: number) => void
): ReadableStream<Uint8Array> {
  return new ReadableStream<Uint8Array>({
    async start(controller) {
      const reader = body.getReader();
      let received = 0;
      try {
        for (;;) {
          const { done, value } = await reader.read();
          if (done) break;
          received += value.length;
          if (total > 0) onPct(Math.min(1, received / total));
          controller.enqueue(value);
        }
        controller.close();
      } catch (e) {
        controller.error(e);
      }
    },
  });
}

/**
 * Chặn fetch trong worker cho file model LaBSE:
 *  * CÓ bản trong OPFS → stream từ đĩa (mở lại trang: ~5-10 giây, không mạng);
 *  * CHƯA có → fetch thật, đếm byte báo tiến trình, tee song song ghi OPFS.
 * ORT vẫn nhận một Response bình thường.
 */
function installProgressInterceptors(onPct: (frac: number) => void) {
  if (interceptorsInstalled) return;
  interceptorsInstalled = true;
  const origFetch = globalThis.fetch?.bind(globalThis);
  if (!origFetch) return;

  globalThis.fetch = (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url =
      typeof input === "string" ? input : ((input as Request).url ?? String(input));
    if (!url.includes("model_quantized.onnx")) {
      return origFetch(input as RequestInfo, init);
    }
    // ---- đường đĩa TRƯỚC TIÊN: đã có bản OPFS → KHÔNG tốn request mạng
    const local = await opfsFile();
    if (local) {
      onPct(0.02);
      return new Response(countingBody(local.stream(), local.size, onPct), {
        headers: { "content-length": String(local.size) },
        status: 200,
      });
    }
    const resp = await origFetch(input as RequestInfo, init);
    if (!resp.body) return resp;
    const total = Number(resp.headers.get("content-length") || 0);
    if (total <= 0) return resp;
    const [forOrt, forOpfs] = resp.body.tee();
    // ghi OPFS nền — pipeTo tiêu thụ theo tốc độ đĩa (nhanh) → tee buffer nhỏ
    opfsWrite(forOpfs);
    return new Response(countingBody(forOrt, total, onPct), {
      headers: resp.headers,
      status: resp.status,
      statusText: resp.statusText,
    });
  }) as typeof fetch;
}

// ========================= Nhúng =========================

function l2normalize(vec: number[] | Float32Array): Float32Array {
  const out = new Float32Array(vec.length);
  let n = 0;
  for (let i = 0; i < vec.length; i++) n += vec[i] * vec[i];
  if (n > 1e-12) {
    const inv = 1 / Math.sqrt(n);
    for (let i = 0; i < vec.length; i++) out[i] = vec[i] * inv;
  } else {
    for (let i = 0; i < vec.length; i++) out[i] = vec[i];
  }
  return out;
}

function tensorToRows(t: { data: unknown; dims: number[] }): number[][] {
  const [b, d] = [t.dims[0], t.dims[1] ?? t.dims[0]];
  const data = t.data as Float32Array | number[];
  const rows: number[][] = [];
  for (let i = 0; i < b; i++) {
    const row: number[] = [];
    for (let k = 0; k < d; k++) row.push(data[i * d + k]);
    rows.push(row);
  }
  return rows;
}

/** Lấy ma trận [B, D] từ tensor đầu ra — ưu tiên đúng thứ tự như bản Rust. */
function pickEmbeddings(
  out: Record<string, { data: unknown; dims: number[] } | undefined>,
  attentionMask: { data: unknown; dims: number[] } | undefined,
  batch: number
): number[][] {
  const emb = out["embeddings"];
  if (emb && emb.dims.length === 2) return tensorToRows(emb);
  const pooler = out["pooler_output"];
  if (pooler && pooler.dims.length === 2) return tensorToRows(pooler);
  const hidden = out["last_hidden_state"];
  if (hidden && hidden.dims.length === 3) {
    const [b, s, d] = hidden.dims;
    const data = hidden.data as Float32Array | number[];
    const mask = attentionMask?.data as BigInt64Array | number[] | undefined;
    const rows: number[][] = [];
    for (let i = 0; i < b; i++) {
      const acc = new Float64Array(d);
      let count = 0;
      for (let t = 0; t < s; t++) {
        const m = mask ? Number(mask[i * s + t]) : 1;
        if (m === 0) continue;
        count++;
        const base = (i * s + t) * d;
        for (let k = 0; k < d; k++) acc[k] += data[base + k];
      }
      rows.push(Array.from(acc, (x) => (count > 0 ? x / count : 0)));
    }
    return rows;
  }
  throw new Error(`LaBSE không trả tensor đầu ra hợp lệ (batch=${batch})`);
}

async function embedInternal(texts: string[]): Promise<Float32Array[]> {
  if (!tokenizer || !session || !ort) throw new Error("LaBSE chưa sẵn sàng");
  const out: Float32Array[] = [];
  // chia lô 16 — đúng nhịp của bản desktop
  for (let start = 0; start < texts.length; start += 16) {
    const chunk = texts.slice(start, start + 16);
    const enc = (
      tokenizer as unknown as (
        texts: string[],
        opts: { padding: boolean; truncation: boolean }
      ) => Record<string, { data: BigInt64Array | number[]; dims: number[] }>
    )(chunk, { padding: true, truncation: true });
    const inputs: Record<string, unknown> = {};
    for (const key of sessionInputs) {
      const t = enc[key];
      if (!t) throw new Error(`tokenizer thiếu trường ${key}`);
      inputs[key] = new ort.Tensor("int64", t.data as BigInt64Array, t.dims as [number, number]);
    }
    const result = (await session.run(inputs)) as Record<
      string,
      { data: unknown; dims: number[] }
    >;
    const rows = pickEmbeddings(
      result,
      enc["attention_mask"] as { data: unknown; dims: number[] } | undefined,
      chunk.length
    );
    for (const r of rows) out.push(l2normalize(r));
  }
  return out;
}

// ========================= Bootstrap =========================

export function startLabse(): Promise<void> {
  if (starting) return starting;
  starting = (async () => {
    try {
      setState({ phase: "downloading", pct: 0, detail: null });
      // ---- thư viện
      try {
        tf ??= (await import("@xenova/transformers")) as TF;
      } catch (e) {
        throw new Error(`nạp thư viện tokenizer: ${errText(e)}`);
      }
      tf.env.allowLocalModels = false;
      try {
        ort = ortPkg;
        ort.env.wasm.numThreads = 1;
        ort.env.wasm.proxy = false;
        // wasm self-host tại public/ort/ — không phụ thuộc CDN
        ort.env.wasm.wasmPaths = "/ort/";
      } catch (e) {
        throw new Error(`cấu hình ORT: ${errText(e)}`);
      }

      // ---- tokenizer (file nhỏ — dùng bộ cache riêng của transformers.js)
      try {
        setState({ phase: "downloading", pct: 1, detail: "tokenizer" });
        tokenizer ??= await tf.AutoTokenizer.from_pretrained(MODEL_ID, {
          progress_callback: (d: unknown) => {
            const p = (d as { status?: string; progress?: number }).progress;
            if (typeof p === "number" && p > 0) {
              setState({ phase: "downloading", pct: Math.min(49, Math.round(p / 2)), detail: null });
            }
          },
        });
      } catch (e) {
        throw new Error(`nạp tokenizer: ${errText(e)}`);
      }

      // ---- model 450 MB: ORT tự stream URL vào heap wasm (đỉnh ~1,4 GB — an toàn
      // cho máy RAM trung bình); % tiến trình qua interceptor fetch/XHR.
      if (!session) {
        installProgressInterceptors((frac) => {
          if (frac >= 0.999) {
            setState({ phase: "loading", pct: 100, detail: null });
          } else {
            setState({
              phase: "downloading",
              pct: Math.max(50, Math.min(99, 50 + Math.round(frac * 49))),
              detail: null,
            });
          }
        });
        setState({ phase: "downloading", pct: 50, detail: "model LaBSE (~450 MB)" });
        try {
          // ARENA TẮT — quyết định sống còn trên máy RAM trung bình: bản bật arena
          // từng đẩy worker lên 1,85 GB RSS → kernel OOM-kill âm thầm (nhịp heartbeat
          // ngừng giữa chừng). Tắt arena + mem-pattern: heap chỉ còn model + workspace.
          // LaBSE tensor nhỏ nên tốc độ suy luận hầu như không đổi (đo: 99-214 ms/câu).
          session = await ort.InferenceSession.create(MODEL_URL, {
            // graphOpt TẮT — optimizer Level 'all' từng cần ~1 GB workspace cho
            // graph 449 MB, cộng dồn lên 1,88 GB RSS → OOM-kill trên máy 4 GB.
            // Tối ưu hoá bỏ qua: dựng nhanh hơn, suy luận chậm nhẹ (đo 152 ms/câu).
            graphOptimizationLevel: "disabled",
            executionProviders: ["wasm"],
            enableCpuMemArena: false,
            enableMemPattern: false,
          });
        } catch (e) {
          session = null;
          throw new Error(`dựng phiên ONNX từ model 450 MB: ${errText(e)}`);
        }
        sessionInputs = new Set(session.inputNames);
      }

      // ---- smoke test 2 câu (chặn model hỏng im lặng — như bản desktop)
      const v = await embedInternal([
        "Hello, this is a test.",
        "Xin chào, đây là bài kiểm tra.",
      ]);
      if (v.length !== 2 || v[0].length < 64) {
        throw new Error("LaBSE trả vector lạ — model có thể hỏng");
      }
      setState({ phase: "ready", pct: 100, detail: null });
    } catch (e) {
      session = null;
      setState({ phase: "failed", pct: 0, detail: errText(e) || "không rõ lỗi" });
      starting = null; // cho phép thử lại
      throw e;
    }
  })();
  return starting;
}

/** Thử lại sau lỗi. */
export function retryLabse(): Promise<void> {
  if (state.phase === "failed" || state.phase === "off") {
    starting = null;
    return startLabse();
  }
  return starting ?? Promise.resolve();
}

/** Nhúng câu (đã L2-chuẩn hoá). Err nếu LaBSE chưa sẵn sàng. */
export async function embedTexts(
  texts: string[],
  onBatch?: (done: number, total: number) => void
): Promise<Float32Array[]> {
  if (!texts.length) return [];
  if (!labseReady()) {
    throw new Error(
      state.phase === "failed"
        ? `LaBSE lỗi: ${state.detail ?? "?"}`
        : state.phase === "downloading"
          ? `đang tải LaBSE (${state.pct}%)`
          : "LaBSE chưa bật"
    );
  }
  const all: Float32Array[] = [];
  for (let start = 0; start < texts.length; start += 16) {
    const chunk = texts.slice(start, start + 16);
    const part = await embedInternal(chunk);
    all.push(...part);
    onBatch?.(Math.min(texts.length, start + 16), texts.length);
    await new Promise((r) => setTimeout(r, 0)); // nhả luồng sự kiện
  }
  return all;
}
