import type { Metadata } from "next";
import localFont from "next/font/local";
import "./globals.css";
import QueryProvider from "@/components/QueryProvider";
import { AuthProvider } from "@/components/AuthProvider";

const inter = localFont({
  variable: "--font-inter",
  src: [
    { path: "../assets/fonts/Inter-100.ttf", weight: "100", style: "normal" },
    { path: "../assets/fonts/Inter-200.ttf", weight: "200", style: "normal" },
    { path: "../assets/fonts/Inter-300.ttf", weight: "300", style: "normal" },
    { path: "../assets/fonts/Inter-400.ttf", weight: "400", style: "normal" },
    { path: "../assets/fonts/Inter-500.ttf", weight: "500", style: "normal" },
    { path: "../assets/fonts/Inter-600.ttf", weight: "600", style: "normal" },
    { path: "../assets/fonts/Inter-700.ttf", weight: "700", style: "normal" },
    { path: "../assets/fonts/Inter-800.ttf", weight: "800", style: "normal" },
    { path: "../assets/fonts/Inter-900.ttf", weight: "900", style: "normal" },
  ],
});

export const metadata: Metadata = {
  title: "Farm Dashboard",
  description: "Monitor distributed nodes, sensors, and schedules.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className={inter.variable}>
        <QueryProvider>
          <AuthProvider>{children}</AuthProvider>
        </QueryProvider>
      </body>
    </html>
  );
}
