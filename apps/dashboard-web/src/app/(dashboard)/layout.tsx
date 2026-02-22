import AuthGate from "@/components/AuthGate";
import LoadingState from "@/components/LoadingState";
import DevActivityBanner from "@/components/DevActivityBanner";
import SystemBannerSlot from "@/components/SystemBannerSlot";
import SidebarNav from "@/components/SidebarNav";
import DashboardHeader from "@/components/DashboardHeader";
import { DashboardUiProvider } from "@/components/DashboardUiProvider";
import { Suspense } from "react";

export default function DashboardLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <Suspense
      fallback={
        <div className="p-8">
          <LoadingState label="Preparing session…" />
        </div>
      }
    >
      <AuthGate>
        <DashboardUiProvider>
          <div className="min-h-screen overflow-x-auto bg-muted text-foreground [@media(min-width:1024px)_and_(pointer:fine)]:overflow-x-hidden">
            <DashboardHeader />
            <SidebarNav />
            <main className="pt-14 transition-all duration-300 [@media(min-width:1024px)_and_(pointer:fine)]:ps-64">
              <div className="mx-auto w-full max-w-[1440px] space-y-4 px-4 py-4">
                <DevActivityBanner />
                <SystemBannerSlot />
                {children}
              </div>
            </main>
          </div>
        </DashboardUiProvider>
      </AuthGate>
    </Suspense>
  );
}
