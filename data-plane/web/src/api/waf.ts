/**
 * One typed seam between the portal's screens and the three pillar services.
 *
 * Everything here is either (a) the generic `metap` record API with WAF's entity names filled in,
 * or (b) one of the custom endpoints the services added for the work `docs/13-screen-api-map.md`
 * flagged as "phải tự code" (DNS verification, origin probe, scan dispatch, correlation, alert
 * delivery). Screens import from here rather than calling `apiFetch` directly so an entity name or
 * an endpoint path is written once, in one file, instead of spread across a dozen components.
 *
 * The dev server routes each request to the owning service by entity-name prefix
 * (`../vite.config.ts`), so nothing here needs a base URL — a path is enough, exactly as it will
 * be in production behind one reverse proxy.
 */
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { apiFetch, useAuth } from "@metap/platform-ui";

export const ENTITIES = {
  zones: "waf.zones",
  ddosPolicies: "waf.ddos_policies",
  firewallRules: "waf.firewall_rules",
  scanJobs: "waf.scan_jobs",
  scanFindings: "waf.scan_findings",
  securityEvents: "waf.security_events",
  incidents: "waf.incidents",
  alertPolicies: "waf.alert_policies",
  alertNotifications: "waf.alert_notifications",
} as const;

/** Mirrors `metap`'s `RecordDto` (camelCase over the wire). `data` is the metadata-driven field
 *  bag — every WAF business field lives in there, not on the envelope. */
export type WafRecord<TData = Record<string, unknown>> = {
  id: string;
  entity: string;
  code: string | null;
  status: string | null;
  data: TData;
  version: number;
  createdAt: string;
  updatedAt: string;
  relatedDisplay?: Record<string, string>;
};

export type ListResponse<T> = { data: WafRecord<T>[]; page?: { limit: number; nextCursor: string | null } };
export type SingleResponse<T> = { data: WafRecord<T> };

export type ZoneData = {
  hostname?: string;
  originAddress?: string;
  status?: string;
  protectionMode?: string;
  configVersion?: number;
  hasConfig?: boolean;
  verificationToken?: string;
  verificationMethod?: string;
  verificationStatus?: string;
  dnsRoutingStatus?: string;
  lastDnsCheckAt?: string;
};

export type Zone = WafRecord<ZoneData>;

function qs(params: Record<string, string | number | undefined>): string {
  const entries = Object.entries(params).filter(([, v]) => v !== undefined && v !== "");
  if (entries.length === 0) return "";
  return `?${entries.map(([k, v]) => `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`).join("&")}`;
}

/** Plain record list. `filters` are field-name equality pairs, the same shape the generic list
 *  route takes — a field must be in the entity's list-view `filters` or the backend ignores it. */
export function useRecords<T = Record<string, unknown>>(
  entity: string,
  filters: Record<string, string | number | undefined> = {},
  limit = 30,
  enabled = true,
) {
  const path = `/api/${entity}${qs({ ...filters, limit })}`;
  const { status } = useAuth();
  return useQuery({
    queryKey: ["waf-records", entity, filters, limit],
    queryFn: () => apiFetch<ListResponse<T>>(path),
    select: (response) => response.data,
    enabled: enabled && status === "authenticated",
  });
}

export function useRecord<T = Record<string, unknown>>(entity: string, id: string | undefined) {
  const { status } = useAuth();
  return useQuery({
    queryKey: ["waf-record", entity, id],
    queryFn: () => apiFetch<SingleResponse<T>>(`/api/${entity}/${id}`),
    select: (response) => response.data,
    enabled: Boolean(id) && status === "authenticated",
  });
}

/* ------------------------------------------------------------------ aggregation */

/** Wire shape of `POST /api/{entity}/aggregate` (added to `metap` core for this portal — the
 *  platform had no aggregation at all before, so every "how many" question meant fetching rows
 *  and counting them in the browser). */
export type AggregateSpec = {
  metrics?: string[];
  groupBy?: string;
  bucket?: "hour" | "day" | "week" | "month";
  timeField?: string;
  filters?: Record<string, string | undefined>;
  since?: string;
  until?: string;
  limit?: number;
};

/** One result row: the dimensions the query asked for plus one key per metric. `count` is always
 *  a number; `group` is a string (the backend casts every group key to text so a chart never has
 *  to branch on the underlying column type). */
export type AggregateRow = {
  bucket?: string | null;
  group?: string | null;
  count?: number;
  [metric: string]: string | number | null | undefined;
};

export function useAggregate(entity: string, spec: AggregateSpec, enabled = true) {
  const { status } = useAuth();
  const body = {
    ...spec,
    filters: Object.fromEntries(Object.entries(spec.filters ?? {}).filter(([, v]) => v !== undefined && v !== "")),
  };
  return useQuery({
    queryKey: ["waf-aggregate", entity, body],
    queryFn: () =>
      apiFetch<{ data: AggregateRow[] }>(`/api/${entity}/aggregate`, {
        method: "POST",
        body: JSON.stringify(body),
      }),
    select: (response) => response.data,
    enabled: enabled && status === "authenticated",
  });
}

/** ISO timestamp `days` ago — the `since` every dashboard window is built from. */
export function daysAgo(days: number): string {
  return new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString();
}

/* ------------------------------------------------------------------ custom endpoints */

export type DnsVerifyResult = {
  data: {
    zone: Zone;
    ownershipVerified: boolean;
    dnsRouted: boolean;
    checked: { txt: string[]; cname: string[]; expectedTarget: string };
  };
};

export function verifyDns(zoneId: string) {
  return apiFetch<DnsVerifyResult>(`/api/${ENTITIES.zones}/${zoneId}/verify-dns`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}

export type OriginTestResult = {
  data: { reachable: boolean; status?: number; latencyMs: number; url: string; error?: string };
};

export function testOrigin(zoneId: string) {
  return apiFetch<OriginTestResult>(`/api/${ENTITIES.zones}/${zoneId}/test-origin`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}

/** Recomputes `Zone.hasConfig` from the zone's real policies/rules. Called after any policy/rule
 *  create or delete — the `activate` workflow guard reads that flag, and `PolicyCondition` has no
 *  way to count related records itself. Idempotent, so firing it optimistically is safe. */
export function syncConfigState(zoneId: string) {
  return apiFetch<{ data: { hasConfig: boolean; changed: boolean } }>(
    `/api/${ENTITIES.zones}/${zoneId}/sync-config-state`,
    { method: "POST", body: JSON.stringify({}) },
  );
}

export function runScanJob(jobId: string) {
  return apiFetch<{ data: { dispatched: boolean; detail: string } }>(`/api/${ENTITIES.scanJobs}/${jobId}/run`, {
    method: "POST",
    body: JSON.stringify({}),
  });
}

export function testAlertPolicy(policyId: string) {
  return apiFetch<{ data: { notificationId: string; delivered: boolean; detail: string } }>(
    `/api/${ENTITIES.alertPolicies}/${policyId}/test`,
    { method: "POST", body: JSON.stringify({}) },
  );
}

export function correlateIncidents(zoneId?: string) {
  return apiFetch<{ data: { scannedEvents: number; createdIncidents: string[]; skippedExisting: number } }>(
    "/internal/incidents/correlate",
    { method: "POST", body: JSON.stringify(zoneId ? { zoneId } : {}) },
  );
}

export function evaluateAlerts() {
  return apiFetch<{ data: { policiesEvaluated: number; fired: unknown[] } }>("/internal/alerts/evaluate", {
    method: "POST",
    body: JSON.stringify({}),
  });
}

/* ------------------------------------------------------------------ record mutations */

export function createRecord<T = Record<string, unknown>>(entity: string, data: Record<string, unknown>) {
  return apiFetch<SingleResponse<T>>(`/api/${entity}`, { method: "POST", body: JSON.stringify(data) });
}

export function updateRecord<T = Record<string, unknown>>(
  entity: string,
  id: string,
  version: number,
  data: Record<string, unknown>,
) {
  return apiFetch<SingleResponse<T>>(`/api/${entity}/${id}`, {
    method: "PATCH",
    body: JSON.stringify({ version, data }),
  });
}

export function deleteRecord(entity: string, id: string, version: number) {
  return apiFetch<unknown>(`/api/${entity}/${id}`, { method: "DELETE", body: JSON.stringify({ version }) });
}

export function transitionRecord<T = Record<string, unknown>>(
  entity: string,
  id: string,
  action: string,
  version: number,
  data?: Record<string, unknown>,
) {
  return apiFetch<SingleResponse<T>>(`/api/${entity}/${id}/transitions/${action}`, {
    method: "POST",
    body: JSON.stringify({ version, ...(data ? { data } : {}) }),
  });
}

/** Invalidates every WAF query at once. Coarse on purpose: these screens are small and a stale
 *  count on a dashboard is worse than one extra refetch after a mutation. */
export function useInvalidateWaf() {
  const queryClient = useQueryClient();
  return () => {
    void queryClient.invalidateQueries({ queryKey: ["waf-records"] });
    void queryClient.invalidateQueries({ queryKey: ["waf-record"] });
    void queryClient.invalidateQueries({ queryKey: ["waf-aggregate"] });
  };
}
