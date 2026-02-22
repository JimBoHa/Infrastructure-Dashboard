"use client";

import PageHeaderCard from "@/components/PageHeaderCard";
import InlineBanner from "@/components/InlineBanner";
import { Card } from "@/components/ui/card";
import NodeButton from "@/features/nodes/components/NodeButton";

const PLAIN_LANGUAGE =
  "Related Sensors finds investigation leads: sensors that “move” around the same times as your focus sensor. We start with raw samples, bucket them into your selected Interval, compute bucket-to-bucket deltas, and turn large deltas into change events. Two signals are combined: (1) event alignment (events match after allowing a small lag/tolerance) and (2) co-occurrence (sensors that light up in the same anomaly buckets). Candidates are scored and sorted into a 0–1 Rank score and an Evidence tier (strong/medium/weak). Rank score is only relative to the sensors evaluated in this run—changing scope/candidate limit can change ranks. Evidence is heuristic coverage, not probability or statistical significance.";

const TOOLTIP_COPY = [
  {
    key: "z threshold",
    text: "Minimum |Δz| for a bucket-to-bucket change to count as an event. Higher = fewer events (less noise, fewer false positives). Lower = more events (more sensitivity, more spurious matches).",
  },
  {
    key: "Max lag (buckets)",
    text: "How far candidates are allowed to lead/lag the focus sensor when matching events. Larger values can surface delayed mechanisms, but increase search space and can introduce accidental matches.",
  },
  {
    key: "Tolerance (buckets)",
    text: "How close two events must be (after applying lag) to be counted as “the same moment”. Larger tolerance increases overlap but can inflate evidence for loosely related sensors.",
  },
  {
    key: "Candidate limit",
    text: "Max number of candidate sensors evaluated for ranking. Higher = more coverage but slower. Rank score is pool-relative, so changing the limit/scope can change ranks.",
  },
  {
    key: "Weights",
    text: "How much each evidence channel contributes to the final Rank score. Increase events to emphasize aligned change events; increase co-occurrence to emphasize shared anomaly buckets; enable Δ corr for context evidence (not required for good results).",
  },
] as const;

export default function RelatedSensorsHowItWorksPage() {
  return (
    <div className="space-y-6">
      <PageHeaderCard
        title="Related Sensors — How it works"
        description="Operator guide: interpret Rank score + Evidence correctly, and understand which knobs change results."
        actions={
          <NodeButton
            size="sm"
            variant="secondary"
            type="button"
            onClick={() => {
              globalThis.location.href = "/analytics/trends";
            }}
          >
            Back to Trends
          </NodeButton>
        }
      />

      <Card className="gap-4 p-6">
        <div className="space-y-2">
          <p className="text-sm font-semibold text-foreground">In plain language (≤120 words)</p>
          <p className="text-sm text-muted-foreground">{PLAIN_LANGUAGE}</p>
        </div>
      </Card>

      <Card className="gap-4 p-6">
        <div className="space-y-2">
          <p className="text-sm font-semibold text-foreground">Pipeline diagram</p>
          <pre className="overflow-auto rounded-lg border border-border bg-card-inset p-4 text-xs text-foreground">
{`Raw samples
  ↓ bucket (Interval)
Buckets (avg/last/…)
  ↓ delta
Δ series
  ↓ threshold + gap rules
Change events
  ↓ event-match (lag/tolerance) + co-occurrence (shared buckets)
Rank score + Evidence tier`}
          </pre>
        </div>
      </Card>

      <InlineBanner tone="warning">
        <div className="space-y-2">
          <p className="text-sm font-semibold text-foreground">Warning (read this)</p>
          <ul className="list-disc space-y-1 pl-4 text-sm text-muted-foreground">
            <li>
              <span className="font-semibold text-foreground">Rank score is not a probability.</span>{" "}
              It is a 0–1 score relative to the candidates evaluated in this run.
            </li>
            <li>
              <span className="font-semibold text-foreground">Evidence is not statistical significance.</span>{" "}
              It is a heuristic “coverage” tier.
            </li>
            <li>
              Results depend on the evaluated candidate pool and the effective Interval (bucket size).
            </li>
          </ul>
        </div>
      </InlineBanner>

      <Card className="gap-4 p-6">
        <div className="space-y-3">
          <p className="text-sm font-semibold text-foreground">Advanced parameter tooltips (copy)</p>
          <ul className="space-y-2 text-sm text-muted-foreground">
            {TOOLTIP_COPY.map((item) => (
              <li key={item.key}>
                <span className="font-semibold text-foreground">{item.key}:</span> {item.text}
              </li>
            ))}
          </ul>
          <p className="text-xs text-muted-foreground">
            Source of truth doc: <code>docs/related-sensors-operator-how-it-works.md</code>
          </p>
        </div>
      </Card>
    </div>
  );
}
