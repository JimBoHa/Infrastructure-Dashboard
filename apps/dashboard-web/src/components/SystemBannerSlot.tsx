"use client";

import { usePathname } from "next/navigation";
import SystemBanner from "@/components/SystemBanner";

const SHOW_BANNER_ROUTES = ["/overview"] as const;

const shouldShowBanner = (pathname: string) =>
  SHOW_BANNER_ROUTES.some((route) => pathname === route || pathname.startsWith(`${route}/`));

export default function SystemBannerSlot() {
  const pathname = usePathname();
  if (!shouldShowBanner(pathname)) return null;
  return <SystemBanner />;
}
