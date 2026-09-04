/**
 * One incident, with the events that produced it.
 *
 * The event list is filtered by the incident's zone rather than by a stored link: an `Incident`
 * has no `eventIds` field — `docs/02-domain-model.md` deliberately keeps `eventCount` as a
 * snapshot taken at creation, not a live relation — so this shows the zone's recent stream as
 * context rather than claiming to reconstruct the exact rows that were correlated.
 */
import { Link, useParams } from "react-router-dom";
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
import { useState } from "react";
import {
  ENTITIES,
  transitionRecord,
  useInvalidateWaf,
  useRecord,
  useRecords,
} from "../api/waf";
import {
  PageHeader,
  SectionCard,
  StatusBadge,
  shortDate,
} from "../components/primitives";
import { NEXT_ACTION, type IncidentData } from "./IncidentsPage";

export function IncidentDetailPage() {
  const { incidentId } = useParams<{ incidentId: string }>();
  const invalidate = useInvalidateWaf();
  const [busy, setBusy] = useState(false);
  const incident = useRecord<IncidentData>(ENTITIES.incidents, incidentId);
  const zoneId = incident.data?.data.zoneId;
  const zone = useRecord<{ hostname?: string }>(ENTITIES.zones, zoneId);
  const events = useRecords<{
    sourceIp?: string;
    action?: string;
    requestPath?: string;
    occurredAt?: string;
  }>(ENTITIES.securityEvents, { zoneId }, 25, Boolean(zoneId));

  if (incident.isLoading)
    return <p className="text-sm text-muted-foreground">Loading…</p>;
  if (!incident.data)
    return <p className="text-sm text-muted-foreground">Incident not found.</p>;

  const record = incident.data;
  const state = record.data.status ?? record.status ?? "";
  const next = NEXT_ACTION[state];

  async function advance() {
    if (!incidentId || !next || !incident.data) return;
    setBusy(true);
    try {
      await transitionRecord(
        ENTITIES.incidents,
        incidentId,
        next.action,
        incident.data.version,
      );
      invalidate();
      toast(`Incident ${next.action}d`, { variant: "default" });
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
      <div className="mb-2 text-sm">
        <Link className="text-muted-foreground hover:underline" to="/incidents">
          ← Incidents
        </Link>
      </div>
      <PageHeader
        title={record.data.title ?? "Incident"}
        description={`Raised ${shortDate(record.createdAt)} · ${record.data.eventCount ?? 0} events correlated`}
        actions={
          <>
            <StatusBadge value={record.data.severity} />
            <StatusBadge value={state} />
            {next ? (
              <Button size="sm" onClick={advance} disabled={busy}>
                {next.label}
              </Button>
            ) : null}
          </>
        }
      />

      <div className="grid gap-4">
        <SectionCard title="Details">
          <dl className="grid gap-3 text-sm sm:grid-cols-3">
            <div>
              <dt className="text-xs uppercase text-muted-foreground">Zone</dt>
              <dd className="mt-1">
                <Link className="hover:underline" to={`/zones/${zoneId}`}>
                  {zone.data?.data.hostname ?? zoneId ?? "—"}
                </Link>
              </dd>
            </div>
            <div>
              <dt className="text-xs uppercase text-muted-foreground">
                Assigned to
              </dt>
              <dd className="mt-1">
                {record.data.assignedTo || (
                  <span className="text-muted-foreground">unassigned</span>
                )}
              </dd>
            </div>
            <div>
              <dt className="text-xs uppercase text-muted-foreground">
                Last update
              </dt>
              <dd className="mt-1 text-muted-foreground">
                {shortDate(record.updatedAt)}
              </dd>
            </div>
          </dl>
        </SectionCard>

        <SectionCard
          title="Recent events on this zone"
          description="Context — not the exact correlated set."
        >
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>When</TableHead>
                <TableHead>Action</TableHead>
                <TableHead>Source IP</TableHead>
                <TableHead>Path</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(events.data ?? []).map((event) => (
                <TableRow key={event.id}>
                  <TableCell className="whitespace-nowrap text-muted-foreground">
                    {shortDate(event.data.occurredAt)}
                  </TableCell>
                  <TableCell>
                    <StatusBadge value={event.data.action} />
                  </TableCell>
                  <TableCell className="font-mono text-xs">
                    {event.data.sourceIp}
                  </TableCell>
                  <TableCell className="max-w-[320px] truncate font-mono text-xs">
                    {event.data.requestPath}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </SectionCard>
      </div>
    </div>
  );
}
