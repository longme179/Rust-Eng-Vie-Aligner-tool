/// labse-worker.ts — điểm vào Web Worker của engine LaBSE.
///
/// Model 449 MB dựng phiên WASM sẽ chặn luồng — chạy trong worker giữ UI
/// luôn hồi đáp. Giao thức thông điệp (cả hai chiều):
///   vào:  {type:'start'} · {type:'embed', id, texts}
///   ra:   {type:'state', state} · {type:'embedProgress', id, done, total}
///         {type:'embedded', id, vectors} · {type:'embedError', id, message}

/// <reference lib="webworker" />
import { startLabse, embedTexts, subscribeLabse } from "./labse-engine";

const ctx = self as unknown as {
  postMessage: (m: unknown) => void;
  addEventListener: (t: "message", h: (ev: MessageEvent) => void) => void;
};

// mọi thay đổi trạng thái (downloading % → loading → ready/failed) đẩy về UI
subscribeLabse((state) => {
  console.info(`[reva-worker] ${state.phase} ${state.pct}%${state.detail ? " — " + state.detail : ""}`);
  ctx.postMessage({ type: "state", state });
});

// HEARTBEAT: nếu worker bị hệ điều hành/Chrome giết âm thầm (OOM), nhịp này
// ngừng — phía chính sẽ biết chết lúc nào thay vì tưởng "đang treo".
let hbCount = 0;
setInterval(() => {
  hbCount++;
  const mem = (
    self as unknown as {
      performance?: { memory?: { usedJSHeapSize?: number } };
    }
  ).performance?.memory;
  ctx.postMessage({
    type: "hb",
    n: hbCount,
    heapMB: mem?.usedJSHeapSize ? Math.round(mem.usedJSHeapSize / 1048576) : -1,
  });
}, 4000);

ctx.addEventListener("message", async (ev: MessageEvent) => {
  const msg = ev.data as
    | { type: "start" }
    | { type: "embed"; id: number; texts: string[] };
  if (!msg || typeof msg.type !== "string") return;
  if (msg.type === "start") {
    try {
      await startLabse(); // lỗi đã đẩy về qua 'state' failed
    } catch {
      /* bỏ qua — state đã phản ánh */
    }
  } else if (msg.type === "embed") {
    try {
      const vectors = await embedTexts(msg.texts, (done, total) =>
        ctx.postMessage({ type: "embedProgress", id: msg.id, done, total })
      );
      ctx.postMessage({ type: "embedded", id: msg.id, vectors });
    } catch (e) {
      ctx.postMessage({
        type: "embedError",
        id: msg.id,
        message: e instanceof Error ? e.message : String(e),
      });
    }
  }
});
