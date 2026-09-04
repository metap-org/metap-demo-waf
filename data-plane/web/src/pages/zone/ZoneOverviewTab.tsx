/** Zone overview — traffic shape for this zone plus its current posture, all from the aggregate
 *  API so the counts are real totals rather than "however many rows fit on one page". */
import { BarChart } from "@metap/ui";
import {
  ENTITIES,
  daysAgo,
  useAggregate,
  useRecords,
  type Zone,
} from "../../api/waf";
import {
  SectionCard,
  StatTile,
  StatusBadge,
  TimeSeries,
  dayLabel,
  shortDate,
} from "../../components/primitives";

export function ZoneOverviewTab({ zone }: { zone: Zone }) {
  const filters = { zoneId: zone.id };
  const eventsPerDay = useAggregate(ENTITIES.securityEvents, {
    bucket: "day",
    timeField: "occurredAt",
    since: daysAgo(7),
    filters,
  });
  const byAction = useAggregate(ENTITIES.securityEvents, {
    groupBy: "action",
    timeField: "occurredAt",
    since: daysAgo(7),
    filters,
  });
  const incidents = useAggregate(ENTITIES.incidents, {
    groupBy: "status",
    filters,
  });
  const rules = useRecords(ENTITIES.firewallRules, filters, 100);
  const policies = useRecords(ENTITIES.ddosPolicies, filters, 5);

  const totalEvents = (byAction.data ?? []).reduce(
    (sum, row) => sum + (row.count ?? 0),
    0,
  );
  const blocked =
    byAction.data?.find((row) => row.group === "blocked")?.count ?? 0;
  const openIncidents =
    incidents.data?.find((row) => row.group === "open")?.count ?? 0;

  return (
    <div className="mt-4 grid gap-4">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <StatTile
          label="Events (7d)"
          value={totalEvents}
          loading={byAction.isLoading}
        />
        <StatTile
          label="Blocked (7d)"
          value={blocked}
          tone={blocked > 0 ? "danger" : "default"}
        />
        <StatTile
          label="Open incidents"
          value={openIncidents}
          tone={openIncidents > 0 ? "warning" : "success"}
        />
        <StatTile
          label="Firewall rules"
          value={(rules.data ?? []).length}
          loading={rules.isLoading}
        />
      </div>

      <div className="grid gap-4 lg:grid-cols-3">
        <div className="lg:col-span-2">
          <SectionCard title="Events" description="Last 7 days">
            <TimeSeries
              ariaLabel="Events per day for this zone"
              points={(eventsPerDay.data ?? []).map((row) => ({
                label: dayLabel(row.bucket),
                value: row.count ?? 0,
              }))}
            />
          </SectionCard>
        </div>
        <SectionCard title="By action">
          <BarChart
            height={180}
            ariaLabel="Events by action for this zone"
            data={(byAction.data ?? []).map((row) => ({
              label: row.group ?? "—",
              value: row.count ?? 0,
            }))}
          />
        </SectionCard>
      </div>

      <SectionCard
        title="Posture"
        description="What is currently protecting this zone."
      >
        <dl className="grid gap-3 text-sm sm:grid-cols-2 lg:grid-cols-4">
          <div>
            <dt className="text-xs uppercase text-muted-foreground">
              Domain ownership
            </dt>
            <dd className="mt-1">
              <StatusBadge value={zone.data.verificationStatus} />
            </dd>
          </div>
          <div>
            <dt className="text-xs uppercase text-muted-foreground">
              DNS routing
            </dt>
            <dd className="mt-1">
              <StatusBadge value={zone.data.dnsRoutingStatus} />
            </dd>
          </div>
          <div>
            <dt className="text-xs uppercase text-muted-foreground">
              DDoS policy
            </dt>
            <dd className="mt-1">
              {(policies.data ?? []).length > 0 ? (
                <StatusBadge
                  value={String(policies.data?.[0]?.data.sensitivity ?? "")}
                />
              ) : (
                <span className="text-muted-foreground">none</span>
              )}
            </dd>
          </div>
          <div>
            <dt className="text-xs uppercase text-muted-foreground">
              Last DNS check
            </dt>
            <dd className="mt-1 text-muted-foreground">
              {shortDate(zone.data.lastDnsCheckAt)}
            </dd>
          </div>
        </dl>
      </SectionCard>
    </div>
  );
}
