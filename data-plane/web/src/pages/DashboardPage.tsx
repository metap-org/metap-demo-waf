/**
 * Module 6's landing view (`docs/07-portal-features.md`) — "is anything attacking me right now",
 * answered on one screen.
 *
 * Every number here comes from `POST /api/{entity}/aggregate`, the aggregation endpoint added to
 * `metap` core for this portal. Before it existed the only way to draw this screen was to fetch
 * rows and count them in the browser, which is wrong twice over: it caps out at the list API's
 * page limit (so every count silently became "up to 50"), and it moves a whole security-event
 * table over the wire to display one integer.
 */
import { Link } from "react-router-dom";
import { BarChart, Button } from "@metap/ui";
import {
  ENTITIES,
  daysAgo,
  useAggregate,
  useRecords,
  type AggregateRow,
} from "../api/waf";
import {
  EmptyState,
  PageHeader,
  SectionCard,
  StatTile,
  StatusBadge,
  TimeSeries,
  dayLabel,
} from "../components/primitives";

/** Sums `count` across every returned group — the "how many in total" reading of a grouped
 *  aggregate, so one request answers both the tile and the chart next to it. */
function total(rows?: AggregateRow[]): number {
  return (rows ?? []).reduce((sum, row) => sum + (row.count ?? 0), 0);
}

function countFor(rows: AggregateRow[] | undefined, group: string): number {
  return rows?.find((row) => row.group === group)?.count ?? 0;
}

const ACTION_COLORS: Record<string, string> = {
  blocked: "hsl(var(--destructive))",
  challenged: "hsl(var(--primary))",
  logged: "hsl(var(--muted-foreground))",
};

export function DashboardPage() {
  const since24h = daysAgo(1);
  const since7d = daysAgo(7);

  const zonesByStatus = useAggregate(ENTITIES.zones, { groupBy: "status" });
  const eventsByAction = useAggregate(ENTITIES.securityEvents, {
    groupBy: "action",
    timeField: "occurredAt",
    since: since24h,
  });
  const eventsPerDay = useAggregate(ENTITIES.securityEvents, {
    bucket: "day",
    timeField: "occurredAt",
    since: since7d,
  });
  const eventsByZone = useAggregate(ENTITIES.securityEvents, {
    groupBy: "zoneId",
    timeField: "occurredAt",
    since: since7d,
    limit: 5,
  });
  const incidentsByStatus = useAggregate(ENTITIES.incidents, {
    groupBy: "status",
  });
  const findingsBySeverity = useAggregate(ENTITIES.scanFindings, {
    groupBy: "severity",
  });

  const recentIncidents = useRecords(ENTITIES.incidents, { status: "open" }, 5);
  // Only to turn a zoneId into a hostname in the "top zones" table — the aggregate returns the raw
  // id, since grouping happens in the database where the zone's hostname isn't joined in.
  const zones = useRecords<{ hostname?: string }>(ENTITIES.zones, {}, 100);
  const hostnameFor = (zoneId?: string | null) =>
    zones.data?.find((zone) => zone.id === zoneId)?.data.hostname ??
    zoneId ??
    "—";

  const openIncidents = countFor(incidentsByStatus.data, "open");
  const blocked24h = countFor(eventsByAction.data, "blocked");

  return (
    <div>
      <PageHeader
        title="Security overview"
        description="Traffic, attacks and open work across every protected zone."
        actions={
          <Button asChild>
            <Link to="/onboarding">Add zone</Link>
          </Button>
        }
      />

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-5">
        <StatTile
          label="Active zones"
          value={countFor(zonesByStatus.data, "active")}
          hint={`${total(zonesByStatus.data)} total`}
          loading={zonesByStatus.isLoading}
        />
        <StatTile
          label="Events (24h)"
          value={total(eventsByAction.data)}
          loading={eventsByAction.isLoading}
        />
        <StatTile
          label="Blocked (24h)"
          value={blocked24h}
          tone={blocked24h > 0 ? "danger" : "default"}
          loading={eventsByAction.isLoading}
        />
        <StatTile
          label="Open incidents"
          value={openIncidents}
          tone={openIncidents > 0 ? "warning" : "success"}
          loading={incidentsByStatus.isLoading}
        />
        <StatTile
          label="Critical findings"
          value={countFor(findingsBySeverity.data, "critical")}
          tone={
            countFor(findingsBySeverity.data, "critical") > 0
              ? "danger"
              : "default"
          }
          loading={findingsBySeverity.isLoading}
        />
      </div>

      <div className="mt-4 grid gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <SectionCard
            title="Security events"
            description="Last 7 days, by day"
          >
            <TimeSeries
              ariaLabel="Security events per day"
              points={(eventsPerDay.data ?? []).map((row) => ({
                label: dayLabel(row.bucket),
                value: row.count ?? 0,
              }))}
            />
          </SectionCard>
        </div>
        <SectionCard title="By action" description="Last 24 hours">
          {(eventsByAction.data ?? []).length === 0 ? (
            <EmptyState
              title="No events yet"
              description="Nothing has hit the edge in this window."
            />
          ) : (
            <BarChart
              ariaLabel="Events by action"
              height={180}
              data={(eventsByAction.data ?? []).map((row) => ({
                label: row.group ?? "unknown",
                value: row.count ?? 0,
                color: ACTION_COLORS[row.group ?? ""] ?? undefined,
              }))}
            />
          )}
        </SectionCard>
      </div>

      <div className="mt-4 grid gap-4 lg:grid-cols-2">
        <SectionCard title="Most targeted zones" description="Last 7 days">
          {(eventsByZone.data ?? []).length === 0 ? (
            <EmptyState title="Nothing to rank yet" />
          ) : (
            <ul className="divide-y text-sm">
              {(eventsByZone.data ?? []).map((row) => (
                <li
                  key={row.group ?? "unknown"}
                  className="flex items-center justify-between py-2"
                >
                  <Link className="hover:underline" to={`/zones/${row.group}`}>
                    {hostnameFor(row.group)}
                  </Link>
                  <span className="tabular-nums text-muted-foreground">
                    {row.count}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </SectionCard>

        <SectionCard
          title="Open incidents"
          actions={
            <Button variant="outline" size="sm" asChild>
              <Link to="/incidents">View all</Link>
            </Button>
          }
        >
          {(recentIncidents.data ?? []).length === 0 ? (
            <EmptyState
              title="No open incidents"
              description="Correlation has not raised anything."
            />
          ) : (
            <ul className="divide-y text-sm">
              {(recentIncidents.data ?? []).map((incident) => (
                <li
                  key={incident.id}
                  className="flex items-center justify-between gap-3 py-2"
                >
                  <Link
                    className="truncate hover:underline"
                    to={`/incidents/${incident.id}`}
                  >
                    {String(incident.data.title ?? incident.id)}
                  </Link>
                  <StatusBadge value={String(incident.data.severity ?? "")} />
                </li>
              ))}
            </ul>
          )}
        </SectionCard>
      </div>
    </div>
  );
}
