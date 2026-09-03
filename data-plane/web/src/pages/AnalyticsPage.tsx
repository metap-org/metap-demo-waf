/**
 * Module 6 in full — the analytics screen the dashboard only samples. Window, zone and metric are
 * all user-controlled here, and every panel is one aggregate request.
 *
 * This is the screen that justified adding aggregation to `metap` core: "top 10 source IPs across
 * every zone in the last 7 days" is a `GROUP BY … ORDER BY count DESC LIMIT 10` over the highest-
 * volume table in the product. Doing it by listing rows was never going to be correct, let alone
 * fast.
 */
import { useState } from "react";
import { BarChart, Button, Select, Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@metap/ui";
import { ENTITIES, daysAgo, useAggregate, useRecords } from "../api/waf";
import { EmptyState, PageHeader, SectionCard, StatTile, TimeSeries, dayLabel, shortDate } from "../components/primitives";

const WINDOWS = [
  { value: "1", label: "Last 24 hours", bucket: "hour" as const },
  { value: "7", label: "Last 7 days", bucket: "day" as const },
  { value: "30", label: "Last 30 days", bucket: "day" as const },
];

export function AnalyticsPage() {
  const [windowDays, setWindowDays] = useState("7");
  const [zoneId, setZoneId] = useState("");

  const selected = WINDOWS.find((w) => w.value === windowDays) ?? WINDOWS[1];
  const since = daysAgo(Number(windowDays));
  const filters = zoneId ? { zoneId } : {};

  const zones = useRecords<{ hostname?: string }>(ENTITIES.zones, {}, 100);
  const overTime = useAggregate(ENTITIES.securityEvents, {
    bucket: selected.bucket,
    timeField: "occurredAt",
    since,
    filters,
  });
  const byAction = useAggregate(ENTITIES.securityEvents, {
    groupBy: "action",
    timeField: "occurredAt",
    since,
    filters,
  });
  const byTrigger = useAggregate(ENTITIES.securityEvents, {
    groupBy: "triggeredBy",
    timeField: "occurredAt",
    since,
    filters,
  });
  const topSources = useAggregate(ENTITIES.securityEvents, {
    groupBy: "sourceIp",
    timeField: "occurredAt",
    since,
    filters,
    limit: 10,
  });
  const incidentsBySeverity = useAggregate(ENTITIES.incidents, { groupBy: "severity", filters });

  const totalEvents = (byAction.data ?? []).reduce((sum, row) => sum + (row.count ?? 0), 0);
  const blocked = byAction.data?.find((row) => row.group === "blocked")?.count ?? 0;
  const blockRate = totalEvents > 0 ? Math.round((blocked / totalEvents) * 100) : 0;

  return (
    <div>
      <PageHeader
        title="Analytics"
        description="Traffic and attack shape across the window you choose."
        actions={
          <>
            <Select
              value={zoneId}
              onChange={(value) => setZoneId(String(value))}
              options={[
                { value: "", label: "All zones" },
                ...(zones.data ?? []).map((zone) => ({
                  value: zone.id,
                  label: zone.data.hostname ?? zone.id,
                })),
              ]}
            />
            <div className="flex gap-1">
              {WINDOWS.map((w) => (
                <Button
                  key={w.value}
                  size="sm"
                  variant={windowDays === w.value ? "default" : "outline"}
                  onClick={() => setWindowDays(w.value)}
                >
                  {w.label}
                </Button>
              ))}
            </div>
          </>
        }
      />

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatTile label="Events" value={totalEvents} loading={byAction.isLoading} />
        <StatTile label="Blocked" value={blocked} tone={blocked > 0 ? "danger" : "default"} />
        <StatTile label="Block rate" value={`${blockRate}%`} />
        <StatTile
          label="Distinct sources (top 10 shown)"
          value={(topSources.data ?? []).length}
          loading={topSources.isLoading}
        />
      </div>

      <div className="mt-4 grid gap-4">
        <SectionCard title="Events over time" description={selected.label}>
          <TimeSeries
            height={220}
            ariaLabel="Events over time"
            points={(overTime.data ?? []).map((row) => ({
              label: selected.bucket === "hour" ? shortDate(row.bucket) : dayLabel(row.bucket),
              value: row.count ?? 0,
            }))}
          />
        </SectionCard>

        <div className="grid gap-4 lg:grid-cols-3">
          <SectionCard title="By action">
            <BarChart
              height={180}
              ariaLabel="Events by action"
              data={(byAction.data ?? []).map((row) => ({ label: row.group ?? "—", value: row.count ?? 0 }))}
            />
          </SectionCard>
          <SectionCard title="By trigger" description="DDoS policy vs firewall rule">
            <BarChart
              height={180}
              ariaLabel="Events by trigger"
              data={(byTrigger.data ?? []).map((row) => ({ label: row.group ?? "—", value: row.count ?? 0 }))}
            />
          </SectionCard>
          <SectionCard title="Incidents by severity">
            <BarChart
              height={180}
              ariaLabel="Incidents by severity"
              data={(incidentsBySeverity.data ?? []).map((row) => ({ label: row.group ?? "—", value: row.count ?? 0 }))}
            />
          </SectionCard>
        </div>

        <SectionCard title="Top source IPs" description={`Most active attackers · ${selected.label.toLowerCase()}`}>
          {(topSources.data ?? []).length === 0 ? (
            <EmptyState title="No traffic in this window" />
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Source IP</TableHead>
                  <TableHead className="text-right">Events</TableHead>
                  <TableHead className="text-right">Share</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {(topSources.data ?? []).map((row) => (
                  <TableRow key={row.group ?? "unknown"}>
                    <TableCell className="font-mono text-xs">{row.group ?? "—"}</TableCell>
                    <TableCell className="text-right tabular-nums">{row.count}</TableCell>
                    <TableCell className="text-right tabular-nums text-muted-foreground">
                      {totalEvents > 0 ? `${Math.round(((row.count ?? 0) / totalEvents) * 100)}%` : "—"}
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </SectionCard>
      </div>
    </div>
  );
}
