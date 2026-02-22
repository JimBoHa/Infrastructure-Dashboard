import type { Metadata } from "next";
import { Share_Tech_Mono, Teko } from "next/font/google";

const simLabMono = Share_Tech_Mono({
  subsets: ["latin"],
  weight: ["400"],
  variable: "--font-simlab-mono",
});

const simLabDisplay = Teko({
  subsets: ["latin"],
  weight: ["400", "500", "600"],
  variable: "--font-simlab-display",
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
