"use client";

export default function AnomalyScore({
  score,
}: {
  score: number | null | undefined;
}) {
  if (score == null || Number.isNaN(score)) {
    return null;
  }

  const tone =
    score >= 0.8
      ? "bg-rose-100 text-rose-800"
      : score >= 0.6
        ? "bg-amber-100 text-amber-800"
        : "bg-indigo-100 text-indigo-800";

  return (
    <span className={`inline-flex items-center rounded-full px-2.5 py-1 text-xs font-semibold ${tone}`}>
      Anomaly {score.toFixed(3)}
    </span>
  );
}
