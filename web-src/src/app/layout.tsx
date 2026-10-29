import type { Metadata, Viewport } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";
import { Toaster } from "@/components/ui/toaster";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "REVA Web — ghép đoạn Anh–Việt trong trình duyệt",
  description:
    "Ghép & sắp đoạn văn bản Anh–Việt 100% ngay trong trình duyệt: bộ từ vựng liên ngôn ngữ + LaBSE (ONNX int8) chạy tại chỗ, không gửi văn bản đi đâu. Bản web của REVA tool — deploy được lên Vercel.",
  keywords: [
    "REVA",
    "ghép đoạn",
    "Anh–Việt",
    "translation alignment",
    "LaBSE",
    "biên dịch",
  ],
  icons: {
    icon: "/favicon.svg",
  },
};

export const viewport: Viewport = {
  themeColor: "#0f766e",
};

const THEME_INIT = `(function(){try{var t=localStorage.getItem("reva-theme");if(!t){t=window.matchMedia&&window.matchMedia("(prefers-color-scheme: dark)").matches?"dark":"light";}if(t==="dark"){document.documentElement.classList.add("dark");}}catch(e){}})();`;

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="vi" suppressHydrationWarning>
      <head>
        <script dangerouslySetInnerHTML={{ __html: THEME_INIT }} />
      </head>
      <body
        className={`${geistSans.variable} ${geistMono.variable} antialiased bg-background text-foreground`}
      >
        {children}
        <Toaster />
      </body>
    </html>
  );
}
