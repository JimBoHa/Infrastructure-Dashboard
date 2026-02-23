import type { Metadata } from "next";
import localFont from "next/font/local";

const simLabMono = localFont({
  variable: "--font-simlab-mono",
  src: [{ path: "../../assets/fonts/ShareTechMono-400.ttf", weight: "400" }],
});

const simLabDisplay = localFont({
  variable: "--font-simlab-display",
  src: [
    { path: "../../assets/fonts/Teko-400.ttf", weight: "400" },
    { path: "../../assets/fonts/Teko-500.ttf", weight: "500" },
    { path: "../../assets/fonts/Teko-600.ttf", weight: "600" },
  ],
});

export const metadata: Metadata = {
  title: "Sim Lab Console",
  description: "Industrial control console for Sim Lab demo testing.",
};

export default function SimLabLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <div
      className={`${simLabMono.variable} ${simLabDisplay.variable} min-h-screen bg-card-inset text-foreground`}
    >
      {children}
    </div>
  );
}
