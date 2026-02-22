import type { NextConfig } from "next";

const normalizeBase = (value: string) => value.replace(/\/$/, "");

const isStaticExport = process.env.FARM_DASHBOARD_STATIC === "1";

const coreApiBase = normalizeBase(
  process.env.FARM_CORE_API_BASE ||
    process.env.NEXT_PUBLIC_API_BASE ||
    "http://127.0.0.1:8000",
);

const setupDaemonBase = normalizeBase(
  process.env.FARM_SETUP_DAEMON_BASE ||
    process.env.NEXT_PUBLIC_SETUP_APP_BASE ||
    "http://127.0.0.1:8800",
);

const nextConfig: NextConfig = {
  allowedDevOrigins: [
    "127.0.0.1",
    "0.0.0.0",
    "localhost",
  ],
  output: isStaticExport ? "export" : undefined,
  trailingSlash: isStaticExport,
  images: { unoptimized: true },
  async rewrites() {
    if (isStaticExport) return [];

    return [
      {
        source: "/api/setup-daemon/:path*",
        destination: `${setupDaemonBase}/api/:path*`,
      },
      { source: "/api/:path*", destination: `${coreApiBase}/api/:path*` },
      { source: "/healthz", destination: `${coreApiBase}/healthz` },
    ];
  },
};

export default nextConfig;
