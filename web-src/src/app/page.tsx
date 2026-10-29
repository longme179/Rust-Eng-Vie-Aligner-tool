"use client";

/**
 * REVA Web — bản web của REVA tool (ghép & sắp đoạn văn bản Anh–Việt).
 *
 * Khác bản desktop (Rust, chạy máy cá nhân): TOÀN BỘ tính toán xảy ra trong
 * TRÌNH DUYỆT của người dùng — bộ từ vựng + thuật toán DP được port 1:1 từ
 * src/aligner.rs, còn LaBSE (ONNX int8, cùng file với bản desktop) chạy qua
 * transformers.js và nằm trong Cache Storage của trình duyệt. Nhờ vậy deploy
 * lên Vercel không cần server hay ổ đĩa ghi được gì cả — chỉ phát mã tĩnh.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  alignWithEmb,
  splitParagraphs,
  STATUS_LABEL_VI,
  type Row,
  type Status,
} from "@/lib/reva/aligner";
import {
  startLabse,
  retryLabse,
  subscribeLabse,
  labseReady,
  embedTexts,
  cosineMatrix,
  type LabseState,
} from "@/lib/reva/labse";
import { useToast } from "@/hooks/use-toast";

// ========================= Bài mẫu (cùng bài với bản desktop) =========================

const SAMPLE_EN = `Hanoi hosted the Green Tech Summit 2025 from March 12 to March 15, drawing more than 4,000 participants from 32 countries.

Organizers said the event focused on clean energy, electric vehicles, and artificial intelligence applications in agriculture.

Prime Minister Pham Minh Chinh delivered the opening speech, calling for stronger cooperation between Southeast Asian nations.

More than 150 startups presented their products across three exhibition halls at the National Convention Center.

Vietnamese electric vehicle maker VinFast introduced two new electric scooter models priced at 25 million dong.

German energy firm Siemens signed a memorandum of understanding worth 120 million euros with Hanoi authorities.

A panel on artificial intelligence drew criticism after organizers failed to invite any Vietnamese researchers.

Attendance dropped 8 percent compared with the 2024 summit, largely because of visa delays for Chinese participants.

The city government announced that the summit will return in March 2026 with an expected 6,000 visitors.

Hotels near the venue reported occupancy above 90 percent during the four-day event.`;

const SAMPLE_VI = `Hà Nội tổ chức Hội nghị Công nghệ Xanh 2025 từ ngày 12 tháng 3 đến 15 tháng 3, thu hút hơn 4.000 người tham dự từ 32 quốc gia.

Ban tổ chức cho biết sự kiện tập trung vào năng lượng sạch, xe điện và ứng dụng trí tuệ nhân tạo trong nông nghiệp.

Thủ tướng Phạm Minh Chính phát biểu khai mạc, kêu gọi tăng cường hợp tác giữa các quốc gia Đông Nam Á.

Hơn 150 startup giới thiệu sản phẩm tại ba gian triển lãm ở Trung tâm Hội nghị Quốc gia.

Hãng xe điện Việt Nam VinFast ra mắt hai mẫu xe máy điện mới với giá 25 triệu đồng.

Tập đoàn năng lượng Đức Siemens ký biên bản ghi nhớ trị giá 120 triệu euro với chính quyền Hà Nội.

Lượt tham dự giảm 8 phần trăm so với hội nghị 2024, chủ yếu do chậm cấp thị thực cho người tham dự Trung Quốc.

Chính quyền thành phố thông báo hội nghị sẽ trở lại vào tháng 3 năm 2026 với 6.000 khách dự kiến.

Các khách sạn quanh địa điểm tổ chức ghi nhận công suất trên 90 phần trăm trong bốn ngày sự kiện.`;

const STORAGE_KEY = "reva-web-session";
const THEME_KEY = "reva-theme";

// ========================= Trạng thái ghép =========================

const STATUS_CHIP: Record<Status, string> = {
  auto_matched:
    "bg-emerald-100 text-emerald-900 border-emerald-300 hover:bg-emerald-200 dark:bg-emerald-950/70 dark:text-emerald-300 dark:border-emerald-800 dark:hover:bg-emerald-900/70",
  needs_review:
    "bg-amber-100 text-amber-900 border-amber-300 hover:bg-amber-200 dark:bg-amber-950/70 dark:text-amber-300 dark:border-amber-800 dark:hover:bg-amber-900/70",
  unmatched:
    "bg-rose-100 text-rose-900 border-rose-300 hover:bg-rose-200 dark:bg-rose-950/70 dark:text-rose-300 dark:border-rose-800 dark:hover:bg-rose-900/70",
  human_confirmed:
    "bg-teal-100 text-teal-900 border-teal-400 hover:bg-teal-200 dark:bg-teal-950/70 dark:text-teal-300 dark:border-teal-700 dark:hover:bg-teal-900/70",
};

const STATUS_DOT: Record<Status, string> = {
  auto_matched: "bg-emerald-500",
  needs_review: "bg-amber-500",
  unmatched: "bg-rose-500",
  human_confirmed: "bg-teal-600",
};

/** Vòng trạng thái khi bấm vào nhãn — tôn trọng bất biến (giống bản desktop). */
function cycleStatus(row: Row): Status {
  if (row.enIndex !== null && row.viIndex !== null) {
    if (row.status === "auto_matched") return "needs_review";
    if (row.status === "needs_review") return "human_confirmed";
    return "auto_matched";
  }
  // hàng một phía: chỉ được nằm giữa unmatched ↔ human_confirmed
  return row.status === "unmatched" ? "human_confirmed" : "unmatched";
}

// ========================= Xuất file (client-side) =========================

function escCsv(v: string): string {
  return `"${v.replace(/"/g, '""')}"`;
}

function rowsToCsv(rows: Row[]): string {
  const head = ["STT", "Tiếng Anh", "Tiếng Việt", "Điểm (%)", "Trạng thái"];
  const lines = [head.map(escCsv).join(",")];
  for (const r of rows) {
    lines.push(
      [
        String(r.index + 1),
        r.en ?? "none",
        r.vi ?? "none",
        r.score !== null ? `${Math.round(r.score * 100)}%` : "",
        STATUS_LABEL_VI[r.status],
      ]
        .map(escCsv)
        .join(",")
    );
  }
  return "\uFEFF" + lines.join("\r\n"); // BOM cho Excel tiếng Việt
}

function rowsToTsv(rows: Row[]): string {
  const lines = ["STT\tTiếng Anh\tTiếng Việt\tĐiểm (%)\tTrạng thái"];
  for (const r of rows) {
    const cell = (s: string) => s.replace(/[\t\r\n]+/g, " ");
    lines.push(
      [
        r.index + 1,
        cell(r.en ?? "none"),
        cell(r.vi ?? "none"),
        r.score !== null ? `${Math.round(r.score * 100)}%` : "",
        STATUS_LABEL_VI[r.status],
      ].join("\t")
    );
  }
  return lines.join("\r\n");
}

function escHtml(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function rowsToHtml(rows: Row[]): string {
  const rowHtml = rows
    .map((r) => {
      const bg =
        r.status === "auto_matched"
          ? "#e2efda"
          : r.status === "needs_review"
            ? "#fff2cc"
            : r.status === "unmatched"
              ? "#fadce3"
              : "#d5eeec";
      return `      <tr style="background:${bg}">
        <td style="padding:6px 10px;border:1px solid #ccc">${r.index + 1}</td>
        <td style="padding:6px 10px;border:1px solid #ccc">${escHtml(r.en ?? "none")}</td>
        <td style="padding:6px 10px;border:1px solid #ccc">${escHtml(r.vi ?? "none")}</td>
        <td style="padding:6px 10px;border:1px solid #ccc;text-align:right">${r.score !== null ? `${Math.round(r.score * 100)}%` : "—"}</td>
        <td style="padding:6px 10px;border:1px solid #ccc">${STATUS_LABEL_VI[r.status]}</td>
      </tr>`;
    })
    .join("\n");
  return `<!doctype html>
<html lang="vi"><head><meta charset="utf-8">
<title>Kết quả ghép Anh–Việt — REVA Web</title>
<style>body{font-family:system-ui,sans-serif;margin:24px;color:#134e4a}h1{font-size:18px}table{border-collapse:collapse}th{background:#0f766e;color:#fff}td,th{padding:6px 10px;border:1px solid #ccc}p{color:#555;font-size:13px}</style>
</head><body>
  <h1>Kết quả ghép đoạn Anh–Việt</h1>
  <p>Xuất bởi REVA Web — xanh nhạt: Tự khớp · vàng: Cần soát · hồng: Không khớp · xanh teal: Đã xác nhận.</p>
  <table>
    <thead><tr><th>STT</th><th>Tiếng Anh</th><th>Tiếng Việt</th><th>Điểm</th><th>Trạng thái</th></tr></thead>
    <tbody>
${rowHtml}
    </tbody>
  </table>
</body></html>`;
}

function rowsToJson(rows: Row[], engine: string): string {
  return JSON.stringify(
    {
      tool: "REVA Web",
      version: 1,
      engine,
      exportedAt: new Date().toISOString(),
      rows: rows.map((r) => ({
        stt: r.index + 1,
        en: r.en ?? "none",
        vi: r.vi ?? "none",
        diem: r.score !== null ? Math.round(r.score * 100) + "%" : null,
        trang_thai: STATUS_LABEL_VI[r.status],
      })),
    },
    null,
    2
  );
}

function download(filename: string, mime: string, content: string) {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 4000);
}

// ========================= Component chính =========================

export default function RevaWebPage() {
  const { toast } = useToast();
  const [theme, setTheme] = useState<"light" | "dark">("light");
  const [enText, setEnText] = useState("");
  const [viText, setViText] = useState("");
  const [threshold, setThreshold] = useState(0.3);
  const [rows, setRows] = useState<Row[]>([]);
  const [engine, setEngine] = useState<"lexical" | "lexical+labse">("lexical");
  const [busy, setBusy] = useState<string | null>(null);
  const [labse, setLabse] = useState<LabseState>({ phase: "off", pct: 0, detail: null });
  const restored = useRef(false);

  // ---- khôi phục phiên + theme từ localStorage (trình duyệt cá nhân) ----
  // (chạy đúng MỘT lần khi gắn kết — chủ đích khôi phục phiên làm việc lần trước;
  //  mọi setState khác trong effect đều bị cấm theo quy tắc set-state-in-effect)
  useEffect(() => {
    try {
      const t = localStorage.getItem(THEME_KEY);
      // eslint-disable-next-line react-hooks/set-state-in-effect -- khôi phục 1 lần khi gắn kết
      setTheme(t === "dark" ? "dark" : "light");
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        const s = JSON.parse(raw) as {
          enText?: string;
          viText?: string;
          threshold?: number;
          rows?: Row[];
          engine?: "lexical" | "lexical+labse";
        };
        const restored = {
          en: typeof s.enText === "string" ? s.enText : null,
          vi: typeof s.viText === "string" ? s.viText : null,
          th: typeof s.threshold === "number" ? s.threshold : null,
          rows: Array.isArray(s.rows) ? s.rows : null,
          engine: s.engine === "lexical+labse" || s.engine === "lexical" ? s.engine : null,
        };
        if (restored.en !== null || restored.vi !== null || restored.rows) {
          setEnText(restored.en ?? "");
          setViText(restored.vi ?? "");
          if (restored.th !== null) setThreshold(restored.th);
          if (restored.rows) setRows(restored.rows);
          if (restored.engine) setEngine(restored.engine);
        }
      }
    } catch {
      /* bộ nhớ riêng tư / JSON hỏng — bỏ qua */
    }
    restored.current = true;
  }, []);

  // ---- tự lưu phiên (không gửi đi đâu) ----
  useEffect(() => {
    if (!restored.current) return;
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ enText, viText, threshold, rows, engine })
      );
    } catch {
      /* hết hạn mức bộ nhớ — bỏ qua */
    }
  }, [enText, viText, threshold, rows, engine]);

  // ---- áp theme vào <html> (class dark theo quy ước shadcn) ----
  useEffect(() => {
    document.documentElement.classList.toggle("dark", theme === "dark");
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      /* bỏ qua */
    }
  }, [theme]);

  // ---- theo dõi LaBSE + tự bật (trừ khi máy đang tiết kiệm dữ liệu) ----
  useEffect(() => {
    const prev = { phase: labse.phase };
    const off = subscribeLabse((st) => {
      setLabse(st);
      if (prev.phase !== "ready" && st.phase === "ready") {
        toast({
          title: "LaBSE đã sẵn sàng",
          description:
            "Bộ ghép giờ dùng kèm LaBSE cho mọi lượt ghép mới — model nằm trong trình duyệt của bạn.",
        });
      }
      if (prev.phase !== "failed" && st.phase === "failed") {
        toast({
          title: "Tải LaBSE lỗi",
          description: `${st.detail ?? "không rõ lỗi"} — bấm huy hiệu LaBSE để thử lại. Bộ từ vựng vẫn ghép bình thường.`,
          variant: "destructive",
        });
      }
      prev.phase = st.phase;
    });
    const conn = (
      navigator as unknown as { connection?: { saveData?: boolean } }
    ).connection;
    const noLabse =
      typeof location !== "undefined" && location.search.includes("nolabse");
    if (!conn?.saveData && !noLabse) {
      startLabse().catch(() => undefined); // lỗi đã hiện qua trạng thái
    }
    return off;
  }, []);

  // ---- đếm đoạn / từ ----
  const enParas = useMemo(() => splitParagraphs(enText), [enText]);
  const viParas = useMemo(() => splitParagraphs(viText), [viText]);
  const words = useCallback(
    (t: string) => (t.trim() ? t.trim().split(/\s+/).length : 0),
    []
  );

  const counts = useMemo(() => {
    const c = { total: rows.length, auto: 0, review: 0, unmatched: 0, confirmed: 0 };
    for (const r of rows) {
      if (r.status === "auto_matched") c.auto++;
      else if (r.status === "needs_review") c.review++;
      else if (r.status === "unmatched") c.unmatched++;
      else c.confirmed++;
    }
    return c;
  }, [rows]);

  const canAlign =
    !busy && (enParas.length > 0 || viParas.length > 0);

  // ---- GHÉP ĐOẠN ----
  const doAlign = useCallback(async () => {
    if (busy) return;
    if (enParas.length === 0 && viParas.length === 0) {
      toast({
        title: "Chưa có văn bản",
        description: "Hãy dán ít nhất một bên (tiếng Anh hoặc tiếng Việt) rồi bấm Ghép đoạn.",
        variant: "destructive",
      });
      return;
    }
    let emb: Float64Array | null = null;
    if (labseReady()) {
      try {
        setBusy("Đang nhúng câu bằng LaBSE…");
        const enVecs = await embedTexts(enParas, (d, t) =>
          setBusy(`Đang nhúng câu bằng LaBSE… ${d}/${t}`)
        );
        const viVecs = await embedTexts(viParas);
        emb = cosineMatrix(enVecs, viVecs);
      } catch (e) {
        toast({
          title: "LaBSE trục trặc — tạm dùng bộ từ vựng",
          description: e instanceof Error ? e.message : String(e),
          variant: "destructive",
        });
      }
    }
    setBusy("Đang ghép bằng thuật toán đơn điệu…");
    // nhả một nhịp cho UI kịp vẽ trạng thái bận
    await new Promise((r) => setTimeout(r, 20));
    const res = alignWithEmb(enText, viText, { autoThreshold: threshold }, emb);
    setBusy(null);
    if (!res.ok) {
      toast({ title: "Không ghép được", description: res.error, variant: "destructive" });
      return;
    }
    setRows(res.output.rows);
    setEngine(res.output.engine);
    toast({
      title: `Ghép xong — ${res.output.rows.length} hàng`,
      description:
        res.output.engine === "lexical+labse"
          ? "Động cơ: từ vựng + LaBSE (chính xác cao)."
          : "Động cơ: từ vựng. LaBSE sẵn sàng sẽ tự dùng cho lượt ghép sau.",
    });
  }, [busy, enParas, viParas, enText, viText, threshold, toast]);

  // ---- đổi trạng thái hàng (bấm nhãn) ----
  const cycleRow = useCallback((index: number) => {
    setRows((rs) =>
      rs.map((r) => (r.index === index ? { ...r, status: cycleStatus(r) } : r))
    );
  }, []);

  // ---- đổi ngưỡng: chấm lại nhãn ngay (trừ hàng người dùng đã xác nhận) ----
  const onThreshold = useCallback((val: number) => {
    setThreshold(val);
    setRows((rs) =>
      rs.map((r) =>
        r.score !== null && r.status !== "human_confirmed"
          ? { ...r, status: r.score >= val ? "auto_matched" : "needs_review" }
          : r
      )
    );
  }, []);

  // ---- huy hiệu LaBSE: bấm để bật / thử lại ----
  const onBadge = useCallback(() => {
    if (labse.phase === "ready") {
      toast({
        title: "LaBSE đang chạy",
        description: "Mọi lượt ghép mới đều dùng kèm LaBSE. Model nằm trong trình duyệt của bạn.",
      });
      return;
    }
    if (labse.phase === "failed" || labse.phase === "off") {
      retryLabse().catch(() => undefined);
      toast({
        title: "Đang tải LaBSE",
        description:
          "Model ~450 MB, đúng 1 lần duy nhất — sau đó trình duyệt cache sẵn. Huy hiệu sẽ hiện tiến trình %.",
      });
    } else {
      toast({
        title:
          labse.phase === "downloading"
            ? `Đang tải LaBSE… ${labse.pct}%`
            : "Đang nạp LaBSE vào bộ nhớ",
        description: "Trong lúc chờ, bộ từ vựng vẫn ghép bình thường.",
      });
    }
  }, [labse, toast]);

  // ---- xuất file ----
  const stamp = () => new Date().toISOString().slice(0, 10);
  const doExport = useCallback(
    (kind: "csv" | "tsv" | "html" | "json") => {
      if (!rows.length) return;
      if (kind === "csv")
        download(`reva-ghep-${stamp()}.csv`, "text/csv;charset=utf-8", rowsToCsv(rows));
      if (kind === "tsv")
        download(`reva-ghep-${stamp()}.tsv`, "text/tab-separated-values;charset=utf-8", rowsToTsv(rows));
      if (kind === "html")
        download(`reva-ghep-${stamp()}.html`, "text/html;charset=utf-8", rowsToHtml(rows));
      if (kind === "json")
        download(`reva-ghep-${stamp()}.json`, "application/json", rowsToJson(rows, engine));
      toast({ title: `Đã xuất ${kind.toUpperCase()}`, description: "File đã tải về máy bạn." });
    },
    [rows, engine, toast]
  );

  const copyTsv = useCallback(async () => {
    if (!rows.length) return;
    try {
      await navigator.clipboard.writeText(rowsToTsv(rows));
      toast({ title: "Đã sao chép bảng", description: "Dán trực tiếp vào Excel/Google Sheets." });
    } catch {
      toast({
        title: "Không sao chép được",
        description: "Trình duyệt chặn clipboard — hãy dùng nút xuất TSV.",
        variant: "destructive",
      });
    }
  }, [rows, toast]);

  // ---- render ----
  const badgeUi = (() => {
    switch (labse.phase) {
      case "ready":
        return {
          cls: "bg-emerald-600 text-white border-emerald-700 hover:bg-emerald-700",
          label: "Từ vựng + LaBSE",
          title: "LaBSE chạy tại chỗ trong trình duyệt — mọi lượt ghép mới đều dùng kèm LaBSE.",
        };
      case "downloading":
        return {
          cls: "bg-amber-500 text-white border-amber-600 hover:bg-amber-600",
          label: `Đang tải LaBSE… ${labse.pct}%`,
          title: "Đang tự tải model LaBSE (~450 MB, đúng 1 lần duy nhất — trình duyệt cache sẵn) + thư viện chạy. Trong lúc chờ, bộ từ vựng vẫn ghép bình thường.",
        };
      case "loading":
        return {
          cls: "bg-amber-500 text-white border-amber-600 hover:bg-amber-600",
          label: "Đang nạp LaBSE…",
          title: "File đã đủ — đang dựng phiên suy luận trong trình duyệt (vài giây).",
        };
      case "failed":
        return {
          cls: "bg-rose-600 text-white border-rose-700 hover:bg-rose-700",
          label: "LaBSE lỗi — bấm thử lại",
          title: `${labse.detail ?? "không rõ lỗi"} — bấm huy hiệu này để thử tải lại. Bộ từ vựng vẫn hoạt động bình thường.`,
        };
      default:
        return {
          cls: "bg-slate-500 text-white border-slate-600 hover:bg-slate-600",
          label: "Chỉ từ vựng",
          title: "Chưa bật LaBSE — bấm huy hiệu để tải model (~450 MB, 1 lần duy nhất) và nâng độ chính xác.",
        };
    }
  })();

  return (
    <div className="min-h-screen flex flex-col bg-slate-50 dark:bg-slate-950 text-slate-800 dark:text-slate-100">
      {/* ============ Đầu trang ============ */}
      <header className="sticky top-0 z-40 border-b border-slate-200/80 dark:border-slate-800 bg-white/85 dark:bg-slate-950/85 backdrop-blur">
        <div className="mx-auto max-w-6xl px-4 sm:px-6 h-16 flex items-center gap-3">
          <svg viewBox="0 0 64 64" className="w-9 h-9 shrink-0" aria-hidden="true">
            <rect x="4" y="4" width="56" height="56" rx="14" className="fill-teal-700 dark:fill-teal-600" />
            <path d="M20 44V20h6l4 8 4-8h6v24h-6V32l-4 8-4-8v12z" className="fill-teal-50 dark:fill-teal-50" />
            <circle cx="46" cy="22" r="5" className="fill-teal-300" />
          </svg>
          <div className="min-w-0">
            <h1 className="text-lg font-bold tracking-tight text-teal-800 dark:text-teal-300 leading-tight">
              REVA Web
            </h1>
            <p className="text-xs text-slate-500 dark:text-slate-400 leading-tight truncate">
              Ghép &amp; sắp đoạn Anh–Việt — 100% trong trình duyệt
            </p>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <button
              type="button"
              onClick={onBadge}
              title={badgeUi.title}
              aria-live="polite"
              className={`h-9 px-3 rounded-full border text-xs font-semibold inline-flex items-center gap-2 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/60 ${badgeUi.cls}`}
            >
              <span
                className={`w-2 h-2 rounded-full ${labse.phase === "ready" ? "bg-white animate-pulse" : "bg-white/80"} ${
                  labse.phase === "downloading" || labse.phase === "loading" ? "animate-ping" : ""
                }`}
                aria-hidden="true"
              />
              {badgeUi.label}
            </button>
            <button
              type="button"
              onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
              title={theme === "dark" ? "Chuyển giao diện sáng" : "Chuyển giao diện tối"}
              aria-label={theme === "dark" ? "Chuyển giao diện sáng" : "Chuyển giao diện tối"}
              className="w-9 h-9 rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/60"
            >
              {theme === "dark" ? (
                <svg className="w-4.5 h-4.5 mx-auto" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
                  <circle cx="12" cy="12" r="4" />
                  <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
                </svg>
              ) : (
                <svg className="w-4.5 h-4.5 mx-auto" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                  <path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z" />
                </svg>
              )}
            </button>
          </div>
        </div>
      </header>

      {/* ============ Nội dung ============ */}
      <main className="flex-1 mx-auto w-full max-w-6xl px-4 sm:px-6 py-6 space-y-6">
        {/* giới thiệu ngang */}
        <section
          aria-label="Giới thiệu"
          className="rounded-xl border border-teal-200 dark:border-teal-900 bg-teal-50/70 dark:bg-teal-950/40 px-4 py-3 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm"
        >
          <span className="font-semibold text-teal-800 dark:text-teal-300">
            Dán bài tiếng Anh + bản dịch tiếng Việt → REVA ghép từng đoạn 1–1
          </span>
          <span className="inline-flex items-center gap-1.5 text-xs text-teal-700 dark:text-teal-400">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
            </svg>
            Riêng tư: mọi tính toán chạy tại chỗ, không gửi văn bản đi đâu
          </span>
        </section>

        {/* ô nhập */}
        <section aria-label="Nhập văn bản" className="grid gap-4 md:grid-cols-2">
          {(
            [
              {
                id: "en",
                label: "Tiếng Anh (bản gốc)",
                value: enText,
                set: setEnText,
                paras: enParas.length,
                accent: "text-slate-700 dark:text-slate-200",
                sample: true,
              },
              {
                id: "vi",
                label: "Tiếng Việt (bản dịch)",
                value: viText,
                set: setViText,
                paras: viParas.length,
                accent: "text-teal-700 dark:text-teal-300",
                sample: false,
              },
            ] as const
          ).map((f) => (
            <div
              key={f.id}
              className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-sm flex flex-col"
            >
              <div className="flex items-center justify-between gap-2 px-4 pt-3 pb-2">
                <label htmlFor={`${f.id}Text`} className={`text-sm font-semibold ${f.accent}`}>
                  {f.label}
                </label>
                <div className="flex items-center gap-2">
                  {f.sample && (
                    <button
                      type="button"
                      onClick={() => {
                        setEnText(SAMPLE_EN);
                        setViText(SAMPLE_VI);
                        toast({
                          title: "Đã điền bài mẫu",
                          description: "10 đoạn Anh / 9 đoạn Việt — thiếu 1 đoạn để thử hàng “Không khớp”.",
                        });
                      }}
                      className="text-xs px-2.5 h-7 rounded-md border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/60"
                    >
                      Bài mẫu
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => f.set("")}
                    disabled={!f.value}
                    className="text-xs px-2.5 h-7 rounded-md border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-40 disabled:cursor-not-allowed transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/60"
                  >
                    Xoá
                  </button>
                </div>
              </div>
              <textarea
                id={`${f.id}Text`}
                value={f.value}
                onChange={(e) => f.set(e.target.value)}
                placeholder={
                  f.id === "en"
                    ? "Dán bài tiếng Anh ở đây — mỗi đoạn cách nhau bằng dòng trống…"
                    : "Dán bản dịch tiếng Việt — cứ để nguyên thứ tự đoạn…"
                }
                rows={9}
                spellCheck={false}
                className="flex-1 min-h-[180px] w-full resize-y rounded-b-xl bg-transparent px-4 pb-3 text-sm leading-relaxed outline-none focus-visible:ring-inset focus-visible:ring-2 focus-visible:ring-teal-500/50 placeholder:text-slate-400 dark:placeholder:text-slate-600"
              />
              <div className="px-4 pb-3 -mt-1 text-xs text-slate-500 dark:text-slate-400 flex gap-4">
                <span>
                  <b className="tabular-nums">{f.paras}</b> đoạn
                </span>
                <span>
                  <b className="tabular-nums">{words(f.value)}</b> từ
                </span>
              </div>
            </div>
          ))}
        </section>

        {/* thanh điều khiển */}
        <section
          aria-label="Điều khiển ghép"
          className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-sm px-4 py-3 flex flex-wrap items-center gap-x-5 gap-y-3"
        >
          <button
            type="button"
            onClick={doAlign}
            disabled={!canAlign}
            className="h-11 px-6 rounded-lg bg-teal-700 hover:bg-teal-800 active:scale-[.98] disabled:opacity-50 disabled:cursor-not-allowed text-white font-semibold text-sm inline-flex items-center gap-2 shadow-sm transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/70 focus-visible:ring-offset-2 dark:focus-visible:ring-offset-slate-900"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path d="M13 2 3 14h7l-1 8 11-13h-7l1-7z" />
            </svg>
            Ghép đoạn
          </button>
          <div className="flex items-center gap-3 grow sm:grow-0">
            <label htmlFor="threshold" className="text-xs text-slate-600 dark:text-slate-300 whitespace-nowrap">
              Ngưỡng tự khớp:{" "}
              <b className="tabular-nums text-teal-700 dark:text-teal-300">
                {Math.round(threshold * 100)}%
              </b>
            </label>
            <input
              id="threshold"
              type="range"
              min={5}
              max={90}
              step={1}
              value={Math.round(threshold * 100)}
              onChange={(e) => onThreshold(Number(e.target.value) / 100)}
              className="w-36 accent-teal-700 dark:accent-teal-500 h-9"
            />
          </div>
          <p className="text-xs text-slate-500 dark:text-slate-400 grow min-w-[200px]">
            Cặp có điểm ≥ ngưỡng được tính <b>Tự khớp</b>, thấp hơn thì <b>Cần soát</b>. Kéo ngưỡng
            chấm lại nhãn ngay lập tức (hàng đã xác nhận giữ nguyên).
          </p>
          <span aria-live="polite" className="text-xs font-medium min-h-[1.25rem] w-full sm:w-auto">
            {busy ? (
              <span className="inline-flex items-center gap-2 text-teal-700 dark:text-teal-300">
                <svg className="animate-spin" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" aria-hidden="true">
                  <path d="M21 12a9 9 0 1 1-6.2-8.6" strokeLinecap="round" />
                </svg>
                {busy}
              </span>
            ) : (
              rows.length > 0 && (
                <span className="text-slate-500 dark:text-slate-400">
                  Động cơ: <b>{engine === "lexical+labse" ? "từ vựng + LaBSE" : "từ vựng"}</b> ·
                  bấm nhãn trạng thái để đổi
                </span>
              )
            )}
          </span>
        </section>

        {/* kết quả */}
        {rows.length === 0 && !busy ? (
          <section
            aria-label="Hướng dẫn bắt đầu"
            className="rounded-xl border-2 border-dashed border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900/50 px-6 py-10 text-center space-y-2"
          >
            <svg className="mx-auto w-10 h-10 text-slate-300 dark:text-slate-600" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" aria-hidden="true">
              <path d="M4 6h16M4 12h10M4 18h7" strokeLinecap="round" />
            </svg>
            <p className="text-sm text-slate-600 dark:text-slate-300 font-medium">
              Chưa có kết quả — dán văn bản hai bên (hoặc bấm <b>Bài mẫu</b>) rồi bấm{" "}
              <b>⚡ Ghép đoạn</b>.
            </p>
            <p className="text-xs text-slate-500 dark:text-slate-400">
              Lần đầu dùng, LaBSE (~450 MB) tự tải về trình duyệt — đúng 1 lần duy nhất. Trong lúc
              chờ, bộ từ vựng vẫn ghép bình thường.
            </p>
          </section>
        ) : (
          <section aria-label="Kết quả ghép" className="space-y-3">
            {/* chips thống kê */}
            <div className="flex flex-wrap items-center gap-2 text-xs">
              <StatChip label="Tổng" value={counts.total} cls="bg-slate-200 text-slate-800 dark:bg-slate-800 dark:text-slate-100" dot="bg-slate-500" />
              <StatChip label="Tự khớp" value={counts.auto} cls="bg-emerald-100 text-emerald-900 dark:bg-emerald-950/70 dark:text-emerald-300" dot={STATUS_DOT.auto_matched} />
              <StatChip label="Cần soát" value={counts.review} cls="bg-amber-100 text-amber-900 dark:bg-amber-950/70 dark:text-amber-300" dot={STATUS_DOT.needs_review} />
              <StatChip label="Không khớp" value={counts.unmatched} cls="bg-rose-100 text-rose-900 dark:bg-rose-950/70 dark:text-rose-300" dot={STATUS_DOT.unmatched} />
              <StatChip label="Đã xác nhận" value={counts.confirmed} cls="bg-teal-100 text-teal-900 dark:bg-teal-950/70 dark:text-teal-300" dot={STATUS_DOT.human_confirmed} />
            </div>

            {/* bảng */}
            <div className="rounded-xl border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 shadow-sm overflow-hidden">
              <div className="max-h-[60vh] overflow-auto">
                <table className="w-full min-w-[720px] text-sm border-collapse">
                  <caption className="sr-only">
                    Bảng kết quả ghép đoạn Anh–Việt — bấm nhãn trạng thái để thay đổi
                  </caption>
                  <thead className="sticky top-0 z-10">
                    <tr className="bg-teal-700 dark:bg-teal-800 text-white text-left">
                      <th scope="col" className="w-12 px-3 py-2.5 font-semibold">#</th>
                      <th scope="col" className="px-3 py-2.5 font-semibold">Tiếng Anh</th>
                      <th scope="col" className="px-3 py-2.5 font-semibold">Tiếng Việt</th>
                      <th scope="col" className="w-20 px-3 py-2.5 font-semibold text-right">Điểm</th>
                      <th scope="col" className="w-36 px-3 py-2.5 font-semibold">Trạng thái</th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows.map((r) => (
                      <tr
                        key={r.index}
                        className="border-t border-slate-100 dark:border-slate-800 hover:bg-slate-50 dark:hover:bg-slate-800/50 align-top"
                      >
                        <td className="px-3 py-2.5 text-xs tabular-nums text-slate-500 dark:text-slate-400">
                          {r.index + 1}
                        </td>
                        <td className="px-3 py-2.5 leading-relaxed whitespace-pre-wrap">
                          {r.en ?? (
                            <span className="italic text-slate-400 dark:text-slate-600">none</span>
                          )}
                        </td>
                        <td className="px-3 py-2.5 leading-relaxed whitespace-pre-wrap">
                          {r.vi ?? (
                            <span className="italic text-slate-400 dark:text-slate-600">none</span>
                          )}
                        </td>
                        <td className="px-3 py-2.5 text-right">
                          {r.score !== null ? (
                            <span
                              className={`inline-block min-w-[3.2rem] rounded-md px-1.5 py-0.5 text-xs font-semibold tabular-nums ${
                                r.score >= threshold
                                  ? "bg-emerald-100 text-emerald-800 dark:bg-emerald-950/70 dark:text-emerald-300"
                                  : "bg-amber-100 text-amber-800 dark:bg-amber-950/70 dark:text-amber-300"
                              }`}
                              title={`Điểm gốc ${(r.score * 100).toFixed(1)}%`}
                            >
                              {Math.round(r.score * 100)}%
                            </span>
                          ) : (
                            <span className="text-slate-300 dark:text-slate-600">—</span>
                          )}
                        </td>
                        <td className="px-3 py-2.5">
                          <button
                            type="button"
                            onClick={() => cycleRow(r.index)}
                            title="Bấm để đổi trạng thái: Tự khớp → Cần soát → Đã xác nhận (hàng thiếu một bên: Không khớp ↔ Đã xác nhận)"
                            className={`inline-flex items-center gap-1.5 h-7 px-2.5 rounded-full border text-xs font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/60 ${STATUS_CHIP[r.status]}`}
                          >
                            <span className={`w-1.5 h-1.5 rounded-full ${STATUS_DOT[r.status]}`} aria-hidden="true" />
                            {STATUS_LABEL_VI[r.status]}
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>

            {/* xuất file */}
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-xs text-slate-500 dark:text-slate-400 mr-1">Xuất bảng:</span>
              {(
                [
                  ["csv", "CSV (Excel)", "text/csv"],
                  ["tsv", "TSV", "text/tab-separated-values"],
                  ["html", "HTML (có màu)", "text/html"],
                  ["json", "JSON", "application/json"],
                ] as const
              ).map(([kind, label, mime]) => (
                <button
                  key={kind}
                  type="button"
                  onClick={() => doExport(kind)}
                  data-mime={mime}
                  className="h-9 px-4 rounded-lg border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 hover:bg-slate-100 dark:hover:bg-slate-800 text-xs font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/60"
                >
                  {label}
                </button>
              ))}
              <button
                type="button"
                onClick={copyTsv}
                className="h-9 px-4 rounded-lg border border-teal-600 text-teal-700 dark:text-teal-300 dark:border-teal-700 hover:bg-teal-50 dark:hover:bg-teal-950/50 text-xs font-semibold transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/60"
              >
                Sao chép TSV
              </button>
              <span className="text-xs text-slate-400 dark:text-slate-500">
                · CSV có BOM cho Excel tiếng Việt · HTML tô màu theo trạng thái
              </span>
            </div>
          </section>
        )}
      </main>

      {/* ============ Chân trang (dính đáy) ============ */}
      <footer className="mt-auto border-t border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950">
        <div className="mx-auto max-w-6xl px-4 sm:px-6 py-3 flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-slate-500 dark:text-slate-400">
          <span>
            <b className="text-teal-700 dark:text-teal-400">REVA Web</b> v1 · ghép &amp; sắp đoạn
            Anh–Việt 100% trong trình duyệt
          </span>
          <span className="hidden sm:inline">·</span>
          <span>LaBSE (ONNX int8) chạy tại chỗ — không gửi văn bản đi đâu</span>
          <a
            href="https://github.com/longme179/Rust-Eng-Vie-Aligner-tool"
            target="_blank"
            rel="noreferrer"
            className="ml-auto underline decoration-dotted hover:text-teal-700 dark:hover:text-teal-400 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-teal-500/60 rounded"
          >
            Bản desktop đầy đủ tính năng (Rust) →
          </a>
        </div>
      </footer>
    </div>
  );
}

function StatChip({
  label,
  value,
  cls,
  dot,
}: {
  label: string;
  value: number;
  cls: string;
  dot: string;
}) {
  return (
    <span className={`inline-flex items-center gap-1.5 h-7 px-3 rounded-full font-semibold ${cls}`}>
      <span className={`w-1.5 h-1.5 rounded-full ${dot}`} aria-hidden="true" />
      {label}: <b className="tabular-nums">{value}</b>
    </span>
  );
}
