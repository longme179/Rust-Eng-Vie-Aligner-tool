/// ort-umd-shim.ts — cung cấp ONNX Runtime cho engine + transformers.js v2
/// bằng bản UMD `public/ort/ort.min.js` nạp qua importScripts trong CLASSIC worker.
///
/// TẠI SAO KHÔNG bundle thẳng `onnxruntime-web` (bản ort-web cho bundler):
/// bản đó tải glue wasm bằng import() động — trong worker (kể cả classic sau
/// khi bundle IIFE) đường này TỪNG TREO VÔ HẠN khi dựng model 449 MB. Bản UMD
/// ort.min.js + importScripts là đường đã KIỂM CHỨNG END-TO-END trên máy này:
/// dựng phiên 449 MB trong ~5 giây, chạy suy luận 214 ms/câu.
///
/// Node (script kiểm): dùng require("onnxruntime-web") nếu có.

type OrtLike = {
  env: {
    wasm: {
      numThreads?: number;
      proxy?: boolean;
      wasmPaths?: string | { wasm?: string; mjs?: string };
    };
  };
  InferenceSession: any;
  Tensor: any;
};

let ort: OrtLike | null = null;

function loadOrt(): OrtLike {
  if (ort) return ort;
  // 1) Classic Web Worker — importScripts UMD (đường đã kiểm chứng).
  if (typeof importScripts === "function") {
    if (!(globalThis as unknown as { ort?: OrtLike }).ort) {
      importScripts("/ort/ort.min.js");
    }
    const g = (globalThis as unknown as { ort?: OrtLike }).ort;
    if (!g) throw new Error("importScripts đã chạy nhưng globalThis.ort vẫn chưa có");
    ort = g;
    return ort;
  }
  // 2) Node/Bun — require gói thật (dist cho node).
  const req = (
    globalThis as unknown as { require?: (name: string) => OrtLike }
  ).require;
  if (typeof req === "function") {
    const m = req("onnxruntime-web");
    ort = ((m as unknown as { default?: OrtLike }).default ?? m) as OrtLike;
    return ort;
  }
  throw new Error(
    "Không nạp được ONNX Runtime: cần classic worker (importScripts) hoặc Node (require)"
  );
}

// Module này phải khởi tạo NGAY khi bundle thực thi (trước khi transformers.js
// v2 đọc env) — nhưng vẫn ném lỗi rõ ràng nếu môi trường không hỗ trợ.
const loaded = loadOrt();

export const env = loaded.env;
export const InferenceSession = loaded.InferenceSession;
export const Tensor = loaded.Tensor;
export default loaded;
