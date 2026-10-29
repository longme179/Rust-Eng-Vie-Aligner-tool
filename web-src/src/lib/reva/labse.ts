/// labse.ts — facade LaBSE cho phía UI (page.tsx).
///
/// Trong TRÌNH DUYỆT: mọi việc nặng (tải model 449 MB, dựng phiên ONNX WASM,
/// nhúng câu) chạy trong Web Worker riêng (public/labse-worker.js — bundle IIFE
/// dựng sẵn bằng esbuild) — main thread chỉ nhận trạng thái + vector nên UI
/// không bao giờ đóng băng. Việc CACHE model nằm trong engine (OPFS — xem
/// labse-engine.ts); facade này lo: worker, heartbeat + watchdog OOM, và đường
/// Node cho script kiểm parity (gọi thẳng engine).

import type { LabsePhase, LabseState } from "./labse-engine";

export type { LabsePhase, LabseState };
// KHÔNG re-export GIÁ TRỊ từ "./labse-engine" — bất kỳ re-export giá trị nào
// cũng kéo @xenova/transformers vào bundle MAIN THREAD (đã từng gây đóng băng).

type Listener = (st: LabseState) => void;

type WorkerMessage =
  | { type: "state"; state: LabseState }
  | { type: "hb"; n: number; heapMB: number }
  | { type: "embedProgress"; id: number; done: number; total: number }
  | { type: "embedded"; id: number; vectors: Float32Array[] }
  | { type: "embedError"; id: number; message: string };

interface Pending {
  resolve: (v: Float32Array[]) => void;
  reject: (e: Error) => void;
  onBatch?: (done: number, total: number) => void;
}

const listeners = new Set<Listener>();
let state: LabseState = { phase: "off", pct: 0, detail: null };
let worker: Worker | null = null;
let startPromise: Promise<void> | null = null;
let nextId = 1;
const pending = new Map<number, Pending>();
let lastHeartbeat = Date.now();

// WATCHDOG: worker bị Chrome/hệ điều hành OOM-kill ÂM THẦM khi nạp model
// 449 MB trên máy RAM căng — nhịp heartbeat ngừng vĩnh viễn, không có sự kiện
// lỗi nào cả. Ngừng > 90 giây trong pha tải/nạp → báo lỗi rõ ràng + dựng lại
// worker, thay vì để huy hiệu treo "đang nạp" mãi mãi.
if (typeof window !== "undefined") {
  setInterval(() => {
    const busy = state.phase === "downloading" || state.phase === "loading";
    if (busy && Date.now() - lastHeartbeat > 90_000) {
      try {
        worker?.terminate();
      } catch {
        /* bỏ qua */
      }
      worker = null;
      startPromise = null;
      setState({
        phase: "failed",
        pct: 0,
        detail:
          "worker LaBSE ngừng hồi đáp — thường là HẾT BỘ NHỚ khi nạp model ~450 MB (cần ~1,5 GB RAM trống). Hãy đóng tab/ứng dụng nặng khác rồi bấm thử lại.",
      });
    }
  }, 10_000);
}

function setState(next: LabseState) {
  const prevPhase = state.phase;
  state = next;
  // chỉ log KHI ĐỔI PHA (không mỗi % — tránh spam console)
  if (prevPhase !== state.phase) {
    console.info(
      `[reva-labse] ${state.phase}${state.detail ? " — " + state.detail : ""}`
    );
  }
  for (const cb of listeners) cb(state);
}

function ensureWorker(): Worker | null {
  // Chỉ dùng Web Worker trong TRÌNH DUYỆT (có window). Bun/Node cũng định nghĩa
  // Worker nhưng theo đường file — phải đi đường engine trực tiếp.
  if (typeof window === "undefined" || typeof Worker === "undefined") {
    return null;
  }
  if (worker) return worker;
  try {
    // Worker dựng sẵn thành file tĩnh (bun run build:worker → public/labse-worker.js):
    // - cùng nguồn gốc → chạy được mọi nơi (dev, Vercel, shared hosting);
    // - KHÔNG dùng `new URL("./labse-worker.ts", import.meta.url)` — Turbopack dev
    //   phục vụ file .ts THÔ (MIME video/mp2t) → worker chết ngay → engine rơi xuống
    //   main thread, đóng băng UI (đã bắt gặp thật).
    // CLASSIC worker (bundle IIFE): BẮT BUỘC — ORT-web 1.14 cần importScripts để
    // nạp glue wasm; module worker không có importScripts → treo vĩnh viễn.
    const url = "/labse-worker.js";
    console.info("[reva-labse] tạo Web Worker:", url);
    worker = new Worker(url);
    worker.addEventListener("message", (ev: MessageEvent) => {
      const msg = ev.data as WorkerMessage;
      if (!msg || typeof msg.type !== "string") return;
      if (msg.type === "hb") {
        lastHeartbeat = Date.now();
      } else if (msg.type === "state") {
        setState(msg.state);
      } else if (msg.type === "embedProgress") {
        pending.get(msg.id)?.onBatch?.(msg.done, msg.total);
      } else if (msg.type === "embedded") {
        const p = pending.get(msg.id);
        if (p) {
          pending.delete(msg.id);
          p.resolve(msg.vectors);
        }
      } else if (msg.type === "embedError") {
        const p = pending.get(msg.id);
        if (p) {
          pending.delete(msg.id);
          p.reject(new Error(msg.message));
        }
      }
    });
    worker.addEventListener("error", (ev: ErrorEvent) => {
      // worker văng (lỗi tải script) → báo failed rõ ràng thay vì treo UI
      setState({
        phase: "failed",
        pct: 0,
        detail: `worker LaBSE: ${ev.message || "không khởi động được"}`,
      });
    });
  } catch (e) {
    console.warn("[reva-labse] tạo Worker thất bại:", e);
    worker = null;
  }
  return worker;
}

// ---- Đường Node (script kiểm parity): gọi thẳng engine ----
type EngineModule = typeof import("./labse-engine");
let engineMod: EngineModule | null = null;
let engineSubscribed = false;
async function getEngine(): Promise<EngineModule> {
  engineMod ??= (await import("./labse-engine")) as EngineModule;
  if (!engineSubscribed) {
    engineSubscribed = true;
    engineMod.subscribeLabse((st) => setState(st));
  }
  return engineMod;
}

// ========================= API công khai =========================

export function labseState(): LabseState {
  return state;
}

export function subscribeLabse(cb: Listener): () => void {
  listeners.add(cb);
  cb(state);
  return () => listeners.delete(cb);
}

export function labseReady(): boolean {
  return state.phase === "ready";
}

/** Bật LaBSE (tải + nạp trong worker). Idempotent. */
export function startLabse(): Promise<void> {
  if (startPromise) return startPromise;
  const w = ensureWorker();
  if (w) {
    startPromise = new Promise<void>((resolve, reject) => {
      const off = subscribeLabse((st) => {
        if (st.phase === "ready") {
          off();
          resolve();
        } else if (st.phase === "failed") {
          off();
          startPromise = null; // cho phép thử lại
          reject(new Error(st.detail ?? "LaBSE lỗi"));
        }
      });
      w.postMessage({ type: "start" });
    });
    return startPromise;
  }
  console.info("[reva-labse] Node — engine chạy trực tiếp (chỉ để kiểm parity)");
  startPromise = getEngine().then((m) => m.startLabse());
  startPromise.catch(() => {
    startPromise = null;
  });
  return startPromise;
}

/** Thử lại sau lỗi. */
export function retryLabse(): Promise<void> {
  if (state.phase === "failed" || state.phase === "off") {
    startPromise = null;
    return startLabse();
  }
  return startPromise ?? Promise.resolve();
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
  const w = ensureWorker();
  if (!w) {
    const m = await getEngine();
    return m.embedTexts(texts, onBatch);
  }
  const id = nextId++;
  return new Promise<Float32Array[]>((resolve, reject) => {
    pending.set(id, { resolve, reject, onBatch });
    w.postMessage({ type: "embed", id, texts });
  });
}

/** Ma trận cosine EN×VI phẳng (hàng trước) từ các vector ĐÃ chuẩn hoá. */
export function cosineMatrix(
  enVecs: Float32Array[],
  viVecs: Float32Array[]
): Float64Array {
  const n = enVecs.length;
  const m = viVecs.length;
  const mtx = new Float64Array(n * m);
  for (let i = 0; i < n; i++) {
    const a = enVecs[i];
    for (let j = 0; j < m; j++) {
      const b = viVecs[j];
      let dot = 0;
      const len = Math.min(a.length, b.length);
      for (let k = 0; k < len; k++) dot += a[k] * b[k];
      mtx[i * m + j] = dot; // đã L2 → dot = cosine
    }
  }
  return mtx;
}
