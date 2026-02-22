import LoadingState from "@/components/LoadingState";
import LoginClient from "@/app/login/LoginClient";
import { Suspense } from "react";

export default function LoginPage() {
  return (
    <Suspense
      fallback={
        <div className="p-10">
          <LoadingState label="Preparing sign-in…" />
        </div>
      }
    >
      <LoginClient />
    </Suspense>
  );
}
