/**
 * Small presentational pieces the WAF screens share. Deliberately local to this app rather than
 * pushed into `@metap/ui`: each one encodes a WAF domain vocabulary (zone status, event action,
 * finding severity), which is exactly the coupling `@metap/ui` — a generic design system — must
 * not take on. Anything here that turns out to be domain-free (`StatTile`, `TimeSeries`) is a
 * candidate to move later, once a second app wants it.
 */
import type { ReactNode } from "react";
import { Badge, Card, CardContent, Skeleton } from "@metap/ui";
import { useTranslation } from "react-i18next";

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <div className="mb-6 flex flex-wrap items-start justify-between gap-3">
      <div>
        <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
        {description ? (
          <p className="mt-1 text-sm text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {actions ? (
        <div className="flex flex-wrap items-center gap-2">{actions}</div>
      ) : null}
    </div>
  );
}

export function StatTile({
  label,
  value,
  hint,
  tone = "default",
  loading,
}: {
  label: string;
  value: ReactNode;
  hint?: string;
  tone?: "default" | "danger" | "warning" | "success";
  loading?: boolean;
}) {
  const toneClass =
    tone === "danger"
      ? "text-destructive"
      : tone === "warning"
        ? "text-amber-600 dark:text-amber-500"
        : tone === "success"
          ? "text-emerald-600 dark:text-emerald-500"
          : "";
  return (
    <Card>
      <CardContent className="p-4">
        <div className="text-xs font-medium uppercase tracking-wide text-muted-foreground">
          {label}
        </div>
        {loading ? (
          <Skeleton className="mt-2 h-8 w-16" />
        ) : (
          <div
            className={`mt-1 text-2xl font-semibold tabular-nums ${toneClass}`}
          >
            {value}
          </div>
        )}
        {hint ? (
          <div className="mt-1 text-xs text-muted-foreground">{hint}</div>
        ) : null}
      </CardContent>
    </Card>
  );
}

/** Maps a WAF enum value onto a badge variant. One place for the whole product's colour language,
 *  so "blocked" never reads as red on one screen and grey on another. */
const TONES: Record<
  string,
  "default" | "secondary" | "destructive" | "outline"
> = {
  // Zone status
  active: "default",
  pending: "secondary",
  paused: "outline",
  suspended: "destructive",
  // Event action
  blocked: "destructive",
  challenged: "secondary",
  logged: "outline",
  // Severity / incident status
  critical: "destructive",
  high: "destructive",
  medium: "secondary",
  low: "outline",
  info: "outline",
  open: "destructive",
  acknowledged: "secondary",
  mitigating: "secondary",
  resolved: "default",
  // Scan job status
  idle: "outline",
  queued: "secondary",
  running: "secondary",
  completed: "default",
  failed: "destructive",
  // Verification / routing
  verified: "default",
  unverified: "outline",
  routed: "default",
  notRouted: "outline",
  sent: "default",
};

export function StatusBadge({ value }: { value?: string | null }) {
  const { t } = useTranslation();
  if (!value) return <span className="text-muted-foreground">—</span>;
  // Falls back to the raw enum value for anything not in `waf.status` (a value this component
  // hasn't been taught about yet reads better as itself than as a missing-key placeholder).
  const key = `waf.status.${value}`;
  const label = t(key, { defaultValue: value });
  return <Badge variant={TONES[value] ?? "outline"}>{label}</Badge>;
}

export function EmptyState({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="rounded-lg border border-dashed p-10 text-center">
      <p className="font-medium">{title}</p>
      {description ? (
        <p className="mt-1 text-sm text-muted-foreground">{description}</p>
      ) : null}
      {action ? <div className="mt-4 flex justify-center">{action}</div> : null}
    </div>
  );
}

export function SectionCard({
  title,
  description,
  actions,
  children,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <Card>
      <CardContent className="p-4">
        <div className="mb-3 flex flex-wrap items-start justify-between gap-2">
          <div>
            <h2 className="text-sm font-semibold">{title}</h2>
            {description ? (
              <p className="text-xs text-muted-foreground">{description}</p>
            ) : null}
          </div>
          {actions ? (
            <div className="flex items-center gap-2">{actions}</div>
          ) : null}
        </div>
        {children}
      </CardContent>
    </Card>
  );
}

/**
 * Time-series line/area chart over the aggregate API's `{bucket, count}` rows.
 *
 * Inline SVG, no chart library — the same call `@metap/ui`'s own `BarChart` made, and for the same
 * reason: one series of counts over time does not justify a dependency, and reading colour from
 * the design tokens (`hsl(var(--primary))`) means it themes with everything else for free.
 */
export function TimeSeries({
  points,
  height = 180,
  ariaLabel,
}: {
  points: { label: string; value: number }[];
  height?: number;
  ariaLabel?: string;
}) {
  const { t } = useTranslation();
  if (points.length === 0) {
    return (
      <div className="py-10 text-center text-sm text-muted-foreground">
        {t("waf.common.noDataInWindow")}
      </div>
    );
  }
  const width = 640;
  const padding = { top: 12, right: 12, bottom: 22, left: 34 };
  const innerW = width - padding.left - padding.right;
  const innerH = height - padding.top - padding.bottom;
  const max = Math.max(...points.map((p) => p.value), 1);
  // A single point has no span to divide by — pin it to the left edge instead of dividing by zero.
  const stepX = points.length > 1 ? innerW / (points.length - 1) : 0;
  const coords = points.map((point, index) => {
    const x = padding.left + index * stepX;
    const y = padding.top + innerH - (point.value / max) * innerH;
    return { x, y, ...point };
  });
  const line = coords
    .map((c, i) => `${i === 0 ? "M" : "L"}${c.x.toFixed(1)},${c.y.toFixed(1)}`)
    .join(" ");
  const area = `${line} L${(padding.left + (points.length - 1) * stepX).toFixed(1)},${padding.top + innerH} L${padding.left},${padding.top + innerH} Z`;

  return (
    <svg
      role="img"
      aria-label={ariaLabel ?? t("waf.common.timeSeries")}
      viewBox={`0 0 ${width} ${height}`}
      className="w-full"
      preserveAspectRatio="none"
    >
      <line
        x1={padding.left}
        y1={padding.top + innerH}
        x2={width - padding.right}
        y2={padding.top + innerH}
        stroke="hsl(var(--border))"
      />
      <text
        x={4}
        y={padding.top + 8}
        className="text-[10px]"
        fill="hsl(var(--muted-foreground))"
      >
        {max}
      </text>
      <path d={area} fill="hsl(var(--primary))" opacity={0.12} />
      <path d={line} fill="none" stroke="hsl(var(--primary))" strokeWidth={2} />
      {coords.map((c) => (
        <circle
          key={c.label}
          cx={c.x}
          cy={c.y}
          r={2.5}
          fill="hsl(var(--primary))"
        >
          <title>{`${c.label}: ${c.value}`}</title>
        </circle>
      ))}
      {/* Only the ends are labelled — a dense axis on a 640-wide viewBox that scales down to a
          phone becomes unreadable overlap, and the per-point tooltip already carries the detail. */}
      <text
        x={padding.left}
        y={height - 6}
        className="text-[10px]"
        fill="hsl(var(--muted-foreground))"
      >
        {points[0]?.label}
      </text>
      <text
        x={width - padding.right}
        y={height - 6}
        textAnchor="end"
        className="text-[10px]"
        fill="hsl(var(--muted-foreground))"
      >
        {points[points.length - 1]?.label}
      </text>
    </svg>
  );
}

/** `2026-09-03T10:00:00Z` → `Sep 3, 10:00`. Buckets are already truncated server-side, so this
 *  only has to be short enough to fit an axis. */
export function shortDate(value?: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function dayLabel(value?: string | null): string {
  if (!value) return "—";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}
