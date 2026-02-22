export type DevLoginGateParams = {
  nodeEnv?: string;
  enableFlag?: string | null;
  hostname?: string | null;
};

export function shouldOfferDevLogin(params: DevLoginGateParams = {}): boolean {
  const nodeEnv = params.nodeEnv ?? process.env.NODE_ENV;
  if (nodeEnv !== "development") return false;

  const enableFlag =
    params.enableFlag ?? process.env.NEXT_PUBLIC_ENABLE_DEV_LOGIN ?? null;
  if (enableFlag !== "1") return false;

  const hostname =
    params.hostname ??
    (typeof window !== "undefined" ? window.location.hostname : null);
  if (!hostname) return false;

  return hostname === "localhost" || hostname === "127.0.0.1";
}

export function getDevLoginCredentials(): { email: string | null; password: string | null } {
  const emailRaw = process.env.NEXT_PUBLIC_DEV_LOGIN_EMAIL ?? "";
  const passwordRaw = process.env.NEXT_PUBLIC_DEV_LOGIN_PASSWORD ?? "";
  const email = emailRaw.trim();
  const password = passwordRaw.trim();
  return {
    email: email.length > 0 ? email : null,
    password: password.length > 0 ? password : null,
  };
}

