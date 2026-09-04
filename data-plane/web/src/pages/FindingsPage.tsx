/**
 * Module 5's cross-zone view — the remediation queue. This is the Developer persona's screen
 * (`docs/03-personas-workflows.md`): they own findings and nothing else, so the actions here are
 * the finding workflow's, not the zone's.
 *
 * `open` can go three ways (confirm / accept the risk / mark false positive), which is why this
 * offers a set of actions per row rather than the single "next step" the incident queue uses.
 */
import { useState } from "react";
import {
  Button,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  toast,
} from "@metap/ui";
import {
  ENTITIES,
  transitionRecord,
  useAggregate,
  useInvalidateWaf,
  useRecords,
  type WafRecord,
} from "../api/waf";
import {
  EmptyState,
  PageHeader,
  StatTile,
  StatusBadge,
  SectionCard,
  shortDate,
} from "../components/primitives";

type FindingData = {
  scanJobId?: string;
  severity?: string;
  category?: string;
  endpoint?: string;
  description?: string;
  remediationStatus?: string;
  lastSeenAt?: string;
};

/** Which transitions each state offers — mirrors `scan_finding_entity.rs`'s workflow. */
const ACTIONS: Record<string, { action: string; label: string }[]> = {
  open: [
    { action: "confirm", label: "Confirm" },
    { action: "accept", label: "Accept risk" },
    { action: "markFalsePositive", label: "False positive" },
  ],
  confirmed: [{ action: "markFixed", label: "Mark fixed" }],
  fixed: [],
  falsePositive: [],
  accepted: [],
};

const STATUSES = [
  "",
  "open",
  "confirmed",
  "fixed",
  "falsePositive",
  "accepted",
];

export function FindingsPage() {
  const invalidate = useInvalidateWaf();
  const [status, setStatus] = useState("open");
  const [severity, setSeverity] = useState("");
  const [busy, setBusy] = useState(false);

  const findings = useRecords<FindingData>(
    ENTITIES.scanFindings,
    { remediationStatus: status || undefined, severity: severity || undefined },
    50,
  );
  const bySeverity = useAggregate(ENTITIES.scanFindings, {
    groupBy: "severity",
  });
  const countFor = (group: string) =>
    bySeverity.data?.find((row) => row.group === group)?.count ?? 0;

  async function act(finding: WafRecord<FindingData>, action: string) {
    setBusy(true);
    try {
      await transitionRecord(
        ENTITIES.scanFindings,
        finding.id,
        action,
        finding.version,
      );
      invalidate();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), {
        variant: "destructive",
      });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <PageHeader
        title="Vulnerability findings"
        description="What the scanners found, and what has been done about it."
      />

      <div className="mb-4 grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatTile
          label="Critical"
          value={countFor("critical")}
          tone="danger"
          loading={bySeverity.isLoading}
        />
        <StatTile
          label="High"
          value={countFor("high")}
          tone="danger"
          loading={bySeverity.isLoading}
        />
        <StatTile
          label="Medium"
          value={countFor("medium")}
          tone="warning"
          loading={bySeverity.isLoading}
        />
        <StatTile
          label="Low / info"
          value={countFor("low") + countFor("info")}
          loading={bySeverity.isLoading}
        />
      </div>

      <SectionCard
        title="Findings"
        actions={
          <div className="flex flex-wrap gap-1">
            {STATUSES.map((value) => (
              <Button
                key={value || "all"}
                size="sm"
                variant={status === value ? "default" : "outline"}
                onClick={() => setStatus(value)}
              >
                {value || "All"}
              </Button>
            ))}
            <span className="mx-1 w-px bg-border" aria-hidden />
            {["", "critical", "high", "medium", "low"].map((value) => (
              <Button
                key={value || "any"}
                size="sm"
                variant={severity === value ? "default" : "outline"}
                onClick={() => setSeverity(value)}
              >
                {value || "Any severity"}
              </Button>
            ))}
          </div>
        }
      >
        {(findings.data ?? []).length === 0 ? (
          <EmptyState
            title="Nothing to remediate"
            description="No findings match this filter."
          />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Severity</TableHead>
                <TableHead>Category</TableHead>
                <TableHead>Endpoint</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Last seen</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(findings.data ?? []).map((finding) => {
                const state =
                  finding.data.remediationStatus ?? finding.status ?? "";
                return (
                  <TableRow key={finding.id}>
                    <TableCell>
                      <StatusBadge value={finding.data.severity} />
                    </TableCell>
                    <TableCell>
                      <div className="font-medium">{finding.data.category}</div>
                      {finding.data.description ? (
                        <div className="max-w-[420px] truncate text-xs text-muted-foreground">
                          {finding.data.description}
                        </div>
                      ) : null}
                    </TableCell>
                    <TableCell className="font-mono text-xs">
                      {finding.data.endpoint}
                    </TableCell>
                    <TableCell>
                      <StatusBadge value={state} />
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-muted-foreground">
                      {shortDate(finding.data.lastSeenAt)}
                    </TableCell>
                    <TableCell className="text-right">
                      {(ACTIONS[state] ?? []).map((entry) => (
                        <Button
                          key={entry.action}
                          size="sm"
                          variant="outline"
                          className="ml-1"
                          disabled={busy}
                          onClick={() => act(finding, entry.action)}
                        >
                          {entry.label}
                        </Button>
                      ))}
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        )}
      </SectionCard>
    </div>
  );
}
