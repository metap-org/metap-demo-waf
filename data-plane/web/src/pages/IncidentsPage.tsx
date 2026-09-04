/**
 * Module 7 — incident management. The SOC queue: what correlation raised, who owns it, and how far
 * through the workflow it is.
 *
 * The status filter tabs and the per-row actions both read the workflow declared in
 * `incident_entity.rs` (open → acknowledged → mitigating → resolved), so this screen does not
 * encode its own idea of the lifecycle beyond which action is offered from which state.
 */
import { useState } from "react";
import { Link } from "react-router-dom";
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
  correlateIncidents,
  transitionRecord,
  useInvalidateWaf,
  useRecords,
  type WafRecord,
} from "../api/waf";
import {
  EmptyState,
  PageHeader,
  StatusBadge,
  shortDate,
} from "../components/primitives";

export type IncidentData = {
  zoneId?: string;
  title?: string;
  severity?: string;
  status?: string;
  eventCount?: number;
  assignedTo?: string;
};

/** Next action per state — one row of the workflow graph, kept next to the UI that offers it. */
export const NEXT_ACTION: Record<string, { action: string; label: string }> = {
  open: { action: "acknowledge", label: "Acknowledge" },
  acknowledged: { action: "startMitigating", label: "Start mitigating" },
  mitigating: { action: "resolve", label: "Resolve" },
};

const STATUSES = ["", "open", "acknowledged", "mitigating", "resolved"];

export function IncidentsPage() {
  const invalidate = useInvalidateWaf();
  const [status, setStatus] = useState("open");
  const [busy, setBusy] = useState(false);
  const incidents = useRecords<IncidentData>(
    ENTITIES.incidents,
    { status: status || undefined },
    50,
  );
  const zones = useRecords<{ hostname?: string }>(ENTITIES.zones, {}, 100);

  const hostnameFor = (zoneId?: string) =>
    zones.data?.find((zone) => zone.id === zoneId)?.data.hostname ??
    zoneId ??
    "—";

  async function advance(incident: WafRecord<IncidentData>) {
    const next = NEXT_ACTION[incident.data.status ?? incident.status ?? ""];
    if (!next) return;
    setBusy(true);
    try {
      await transitionRecord(
        ENTITIES.incidents,
        incident.id,
        next.action,
        incident.version,
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

  async function correlate() {
    setBusy(true);
    try {
      const result = await correlateIncidents();
      invalidate();
      toast(
        `${result.data.createdIncidents.length} new incident(s) from ${result.data.scannedEvents} events`,
        {
          variant: "default",
        },
      );
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
        title="Incidents"
        description="Correlated attacks that need a human decision."
        actions={
          <Button variant="outline" onClick={correlate} disabled={busy}>
            Run correlation
          </Button>
        }
      />

      <div className="mb-3 flex gap-1">
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
      </div>

      {(incidents.data ?? []).length === 0 ? (
        <EmptyState
          title="Nothing here"
          description="No incidents match this filter. Run correlation to turn recent events into incidents."
        />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Incident</TableHead>
              <TableHead>Zone</TableHead>
              <TableHead>Severity</TableHead>
              <TableHead>Status</TableHead>
              <TableHead className="text-right">Events</TableHead>
              <TableHead>Raised</TableHead>
              <TableHead className="text-right">Action</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {(incidents.data ?? []).map((incident) => {
              const state = incident.data.status ?? incident.status ?? "";
              const next = NEXT_ACTION[state];
              return (
                <TableRow key={incident.id}>
                  <TableCell>
                    <Link
                      className="font-medium hover:underline"
                      to={`/incidents/${incident.id}`}
                    >
                      {incident.data.title}
                    </Link>
                  </TableCell>
                  <TableCell>
                    <Link
                      className="hover:underline"
                      to={`/zones/${incident.data.zoneId}`}
                    >
                      {hostnameFor(incident.data.zoneId)}
                    </Link>
                  </TableCell>
                  <TableCell>
                    <StatusBadge value={incident.data.severity} />
                  </TableCell>
                  <TableCell>
                    <StatusBadge value={state} />
                  </TableCell>
                  <TableCell className="text-right tabular-nums">
                    {incident.data.eventCount ?? 0}
                  </TableCell>
                  <TableCell className="whitespace-nowrap text-muted-foreground">
                    {shortDate(incident.createdAt)}
                  </TableCell>
                  <TableCell className="text-right">
                    {next ? (
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => advance(incident)}
                        disabled={busy}
                      >
                        {next.label}
                      </Button>
                    ) : (
                      <span className="text-xs text-muted-foreground">
                        done
                      </span>
                    )}
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
