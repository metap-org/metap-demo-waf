/**
 * Module 6 at zone scope — the raw event stream behind the charts, plus the one action that turns
 * events into work: correlation.
 *
 * Correlation is a real backend call (`/internal/incidents/correlate`), the same one the scheduler
 * drives — a "correlate now" button that ran different logic than the scheduled sweep would prove
 * nothing about what the product actually does.
 */
import { useState } from "react";
import {
  Button,
  Select,
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
  useInvalidateWaf,
  useRecords,
} from "../../api/waf";
import {
  EmptyState,
  SectionCard,
  StatusBadge,
  shortDate,
} from "../../components/primitives";

type EventData = {
  zoneId?: string;
  triggeredBy?: string;
  triggeredByName?: string;
  action?: string;
  sourceIp?: string;
  requestPath?: string;
  occurredAt?: string;
};

export function ZoneEventsTab({ zoneId }: { zoneId: string }) {
  const invalidate = useInvalidateWaf();
  const [action, setAction] = useState("");
  const [busy, setBusy] = useState(false);
  const events = useRecords<EventData>(
    ENTITIES.securityEvents,
    { zoneId, action: action || undefined },
    50,
  );

  async function correlate() {
    setBusy(true);
    try {
      const result = await correlateIncidents(zoneId);
      invalidate();
      toast(
        `Scanned ${result.data.scannedEvents} events · ${result.data.createdIncidents.length} new incident(s)`,
        { variant: "default" },
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
    <div className="mt-4">
      <SectionCard
        title="Security events"
        description="What the edge did with traffic for this zone."
        actions={
          <>
            <Select
              value={action}
              onChange={(value) => setAction(String(value))}
              options={[
                { value: "", label: "All actions" },
                { value: "blocked", label: "Blocked" },
                { value: "challenged", label: "Challenged" },
                { value: "logged", label: "Logged" },
              ]}
            />
            <Button
              size="sm"
              variant="outline"
              onClick={correlate}
              disabled={busy}
            >
              Correlate now
            </Button>
          </>
        }
      >
        {(events.data ?? []).length === 0 ? (
          <EmptyState
            title="No events"
            description="Nothing has matched a policy or rule for this zone in the stored window."
          />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>When</TableHead>
                <TableHead>Action</TableHead>
                <TableHead>Source IP</TableHead>
                <TableHead>Path</TableHead>
                <TableHead>Triggered by</TableHead>
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
                  <TableCell className="max-w-[280px] truncate font-mono text-xs">
                    {event.data.requestPath}
                  </TableCell>
                  <TableCell>
                    {event.data.triggeredByName ??
                      event.data.triggeredBy ??
                      "—"}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </SectionCard>
    </div>
  );
}
