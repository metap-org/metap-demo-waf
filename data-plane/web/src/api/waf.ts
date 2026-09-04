/**
 * One typed seam between the portal's screens and the three pillar services.
 *
 * Everything here is either (a) the generic `metap` record API with WAF's entity names filled in,
 * or (b) one of the custom endpoints the services added for the work `docs/13-screen-api-map.md`
 * flagged as "phải tự code" (DNS verification, origin probe, scan dispatch, correlation, alert
 * delivery). Screens import from here rather than calling the transport directly so an entity name
 * or a field name is written once, in one file, instead of spread across a dozen components.
 *
 * **Transport: GraphQL, through `waf-graphql-gateway`, not REST** (2026-09-04 — this file used to
 * call `apiFetch<...>('/api/...')` directly; see `../../graphql-gateway/README.md` for the 8
 * custom fields and `../../../metap-docs/docs/frontend-checklist.md` for why). Every exported
 * function/hook here keeps its exact pre-migration name and signature — the 130+ call sites across
 * `pages/*.tsx` needed zero changes, only this file's internals did. `/graphql` is a fixed,
 * entity-agnostic path (the dev proxy and prod nginx both route it straight to the gateway, see
 * `../../vite.config.ts`) — unlike REST's `/api/:entity*`, there's no per-entity routing to do
 * here at all.
 *
 * Generic list/get/create/update/delete/transition go through the schema `metap-graphql`
 * synthesizes from each entity's metadata — no hand-written query per entity. Since a GraphQL
 * selection set can't be "every field" the way a REST JSON body naturally is, and this file's
 * `useRecords<T>`/`useRecord<T>`/etc. are generic over `T` with no per-call-site field list to work
 * from, every record-shaped query/mutation here first resolves the entity's field names via
 * `useEntity`/`fetchEntityFields` (a REST call to `GET /metadata/entities/{entity}`, deliberately
 * left off GraphQL — this is schema reflection, not business data, the same category as `/auth/*`/
 * `/preferences/*`) and builds the selection set from that, then reshapes the flat GraphQL response
 * back into this file's existing `WafRecord<T>` envelope-plus-`data` shape so no caller can tell
 * the transport changed. The 7 custom actions + `aggregate` need no field discovery — their custom
 * resolvers hand back each REST endpoint's JSON response verbatim as the `Json` scalar, so this
 * file's existing `{data: ...}`-shaped result types for them are unchanged byte-for-byte.
 */
import { useQueryClient } from "@tanstack/react-query";
import {
  apiFetch,
  createFieldName,
  deleteFieldName,
  getFieldName,
  graphqlFetch,
  listFieldName,
  transitionFieldName,
  updateFieldName,
  useAuth,
  useEntity,
  useGraphQLQuery,
} from "@metap/platform-ui";
import type { EntityField, EntitySummary } from "@metap/platform-ui";

const GRAPHQL_PATH = "/graphql";

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

export type ListResponse<T> = {
  data: WafRecord<T>[];
  page?: { limit: number; nextCursor: string | null };
};
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

/* ------------------------------------------------------------------ record shape plumbing */

const ENVELOPE_FIELDS = [
  "id",
  "entity",
  "code",
  "status",
  "version",
  "createdAt",
  "updatedAt",
] as const;

/** Builds the GraphQL selection set for 1 record: the fixed envelope plus every field the entity
 *  declares. A `reference` field's GraphQL type is an object, not a scalar, so it needs its own
 *  sub-selection (`fieldName { id }`) rather than a bare field name — `reshapeRecord` below undoes
 *  that nesting back into the plain foreign-key-id string REST used to return in `data.fieldName`. */
function recordSelection(fields: EntityField[]): string {
  const dataFields = fields.map((f) =>
    f.kind === "reference" ? `${f.name} { id }` : f.name,
  );
  return [...ENVELOPE_FIELDS, ...dataFields].join("\n        ");
}

/** Undoes `recordSelection`'s flat GraphQL shape back into this file's `WafRecord<T>` envelope +
 *  `data` bag — the shape every screen in `pages/*.tsx` already expects, unchanged since before
 *  this file's GraphQL migration. */
function reshapeRecord<T>(
  raw: Record<string, unknown>,
  fields: EntityField[],
): WafRecord<T> {
  const data: Record<string, unknown> = {};
  for (const field of fields) {
    const value = raw[field.name];
    data[field.name] =
      field.kind === "reference" && value !== null && typeof value === "object"
        ? ((value as { id?: string }).id ?? null)
        : value;
  }
  return {
    id: raw.id as string,
    entity: raw.entity as string,
    code: (raw.code as string | null) ?? null,
    status: (raw.status as string | null) ?? null,
    version: raw.version as number,
    createdAt: raw.createdAt as string,
    updatedAt: raw.updatedAt as string,
    data: data as T,
  };
}

/** Imperative (non-hook) counterpart to `useEntity` — `createRecord`/`updateRecord`/
 *  `transitionRecord` need the same field list `useRecord` does to build a full record selection
 *  set, but run outside React so they can't use that hook. Cached forever per entity for the tab's
 *  lifetime: entity metadata doesn't change mid-session outside a low-code publish (same
 *  `staleTime: Infinity` reasoning `useEntity`/`useEntities` already rely on), and every one of
 *  these mutations needs the exact same field list anyway. */
const entityFieldsCache = new Map<string, Promise<EntityField[]>>();

function fetchEntityFields(entity: string): Promise<EntityField[]> {
  let cached = entityFieldsCache.get(entity);
  if (!cached) {
    cached = apiFetch<{ data: EntitySummary }>(
      `/metadata/entities/${entity}`,
    ).then((response) => response.data.fields);
    entityFieldsCache.set(entity, cached);
  }
  return cached;
}

/* ------------------------------------------------------------------ record reads */

/** Plain record list. `filters` are field-name equality pairs, the same shape the generic list
 *  route took over REST — a field must be in the entity's list-view `filters` or the backend
 *  ignores it; `list_input_from_args` (`metap-graphql`) turns this exact `{field: value}` shape
 *  into the same `Vec<(String, String)>` the REST route built from a query string, so filtering
 *  behavior (including `hostname`'s substring match) carries over unchanged. */
export function useRecords<T = Record<string, unknown>>(
  entity: string,
  filters: Record<string, string | number | undefined> = {},
  limit = 30,
  enabled = true,
) {
  const { status } = useAuth();
  const authed = enabled && status === "authenticated";
  const entityQuery = useEntity(entity, authed);
  const fields = entityQuery.data?.fields ?? [];
  const query = `query List($filter: Json, $limit: Int) {
    result: ${listFieldName(entity)}(filter: $filter, limit: $limit) {
      records {
        ${recordSelection(fields)}
      }
    }
  }`;
  const variables = {
    filter: Object.fromEntries(
      Object.entries(filters).filter(([, v]) => v !== undefined && v !== ""),
    ),
    limit,
  };
  const result = useGraphQLQuery<
    { result: { records: Record<string, unknown>[] } },
    WafRecord<T>[]
  >(
    ["waf-records", entity, filters, limit],
    GRAPHQL_PATH,
    query,
    variables,
    (raw) =>
      raw.result.records.map((record) => reshapeRecord<T>(record, fields)),
    authed && Boolean(entityQuery.data),
  );
  return {
    ...result,
    isLoading: result.isLoading || (authed && !entityQuery.data),
  };
}

export function useRecord<T = Record<string, unknown>>(
  entity: string,
  id: string | undefined,
) {
  const { status } = useAuth();
  const authed = Boolean(id) && status === "authenticated";
  const entityQuery = useEntity(entity, authed);
  const fields = entityQuery.data?.fields ?? [];
  const query = `query Get($id: ID!) {
    result: ${getFieldName(entity)}(id: $id) {
      ${recordSelection(fields)}
    }
  }`;
  const result = useGraphQLQuery<
    { result: Record<string, unknown> | null },
    WafRecord<T> | undefined
  >(
    ["waf-record", entity, id],
    GRAPHQL_PATH,
    query,
    { id },
    (raw) => (raw.result ? reshapeRecord<T>(raw.result, fields) : undefined),
    authed && Boolean(entityQuery.data),
  );
  return {
    ...result,
    isLoading: result.isLoading || (authed && !entityQuery.data),
  };
}

/* ------------------------------------------------------------------ aggregation */

/** Wire shape of the `aggregate` GraphQL field, itself a thin proxy to what used to be `POST
 *  /api/{entity}/aggregate` (added to `metap` core for this portal — the platform had no
 *  aggregation at all before, so every "how many" question meant fetching rows and counting them
 *  in the browser); `waf-graphql-gateway`'s resolver forwards `spec` to that same REST endpoint
 *  verbatim and hands back its JSON response unchanged, so this shape is unaffected by transport. */
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

const AGGREGATE_QUERY = `query Aggregate($entity: String!, $spec: Json!) {
  result: aggregate(entity: $entity, spec: $spec)
}`;

export function useAggregate(
  entity: string,
  spec: AggregateSpec,
  enabled = true,
) {
  const { status } = useAuth();
  const body = {
    ...spec,
    filters: Object.fromEntries(
      Object.entries(spec.filters ?? {}).filter(
        ([, v]) => v !== undefined && v !== "",
      ),
    ),
  };
  return useGraphQLQuery<{ result: { data: AggregateRow[] } }, AggregateRow[]>(
    ["waf-aggregate", entity, body],
    GRAPHQL_PATH,
    AGGREGATE_QUERY,
    { entity, spec: body },
    (raw) => raw.result.data,
    enabled && status === "authenticated",
  );
}

/** ISO timestamp `days` ago — the `since` every dashboard window is built from. */
export function daysAgo(days: number): string {
  return new Date(Date.now() - days * 24 * 60 * 60 * 1000).toISOString();
}

/* ------------------------------------------------------------------ custom endpoints */

/** Imperative GraphQL call (`createRecord`/the 7 custom actions below are plain async functions,
 *  not hooks, so they can't use `useGraphQLQuery`) — rides the existing session cookie
 *  (`graphqlFetch` with no `token`), same as that hook, no `GET /auth/token` round trip first
 *  (removed 2026-09-04). */
async function graphqlAuthed<T>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<T> {
  return graphqlFetch<T>(GRAPHQL_PATH, query, variables);
}

export type DnsVerifyResult = {
  data: {
    zone: Zone;
    ownershipVerified: boolean;
    dnsRouted: boolean;
    checked: { txt: string[]; cname: string[]; expectedTarget: string };
  };
};

export function verifyDns(zoneId: string) {
  return graphqlAuthed<{ result: DnsVerifyResult }>(
    `mutation VerifyZoneDns($zoneId: ID!) { result: verifyZoneDns(zoneId: $zoneId) }`,
    { zoneId },
  ).then((r) => r.result);
}

export type OriginTestResult = {
  data: {
    reachable: boolean;
    status?: number;
    latencyMs: number;
    url: string;
    error?: string;
  };
};

export function testOrigin(zoneId: string) {
  return graphqlAuthed<{ result: OriginTestResult }>(
    `mutation TestZoneOrigin($zoneId: ID!) { result: testZoneOrigin(zoneId: $zoneId) }`,
    { zoneId },
  ).then((r) => r.result);
}

/** Recomputes `Zone.hasConfig` from the zone's real policies/rules. Called after any policy/rule
 *  create or delete — the `activate` workflow guard reads that flag, and `PolicyCondition` has no
 *  way to count related records itself. Idempotent, so firing it optimistically is safe. */
export function syncConfigState(zoneId: string) {
  return graphqlAuthed<{
    result: { data: { hasConfig: boolean; changed: boolean } };
  }>(
    `mutation SyncZoneConfigState($zoneId: ID!) { result: syncZoneConfigState(zoneId: $zoneId) }`,
    { zoneId },
  ).then((r) => r.result);
}

export function runScanJob(jobId: string) {
  return graphqlAuthed<{
    result: { data: { dispatched: boolean; detail: string } };
  }>(`mutation RunScanJob($jobId: ID!) { result: runScanJob(jobId: $jobId) }`, {
    jobId,
  }).then((r) => r.result);
}

export function testAlertPolicy(policyId: string) {
  return graphqlAuthed<{
    result: {
      data: { notificationId: string; delivered: boolean; detail: string };
    };
  }>(
    `mutation TestAlertPolicy($policyId: ID!) { result: testAlertPolicy(policyId: $policyId) }`,
    {
      policyId,
    },
  ).then((r) => r.result);
}

export function correlateIncidents(zoneId?: string) {
  return graphqlAuthed<{
    result: {
      data: {
        scannedEvents: number;
        createdIncidents: string[];
        skippedExisting: number;
      };
    };
  }>(
    `mutation CorrelateIncidents($zoneId: String) { result: correlateIncidents(zoneId: $zoneId) }`,
    {
      zoneId: zoneId ?? null,
    },
  ).then((r) => r.result);
}

export function evaluateAlerts() {
  return graphqlAuthed<{
    result: { data: { policiesEvaluated: number; fired: unknown[] } };
  }>(`mutation EvaluateAlerts { result: evaluateAlerts }`).then(
    (r) => r.result,
  );
}

/* ------------------------------------------------------------------ record mutations */

export async function createRecord<T = Record<string, unknown>>(
  entity: string,
  data: Record<string, unknown>,
): Promise<SingleResponse<T>> {
  const fields = await fetchEntityFields(entity);
  const query = `mutation Create($data: Json!) {
    result: ${createFieldName(entity)}(data: $data) {
      ${recordSelection(fields)}
    }
  }`;
  const raw = await graphqlAuthed<{ result: Record<string, unknown> }>(query, {
    data,
  });
  return { data: reshapeRecord<T>(raw.result, fields) };
}

export async function updateRecord<T = Record<string, unknown>>(
  entity: string,
  id: string,
  version: number,
  data: Record<string, unknown>,
): Promise<SingleResponse<T>> {
  const fields = await fetchEntityFields(entity);
  const query = `mutation Update($id: ID!, $expectedVersion: Int!, $data: Json!) {
    result: ${updateFieldName(entity)}(id: $id, expectedVersion: $expectedVersion, data: $data) {
      ${recordSelection(fields)}
    }
  }`;
  const raw = await graphqlAuthed<{ result: Record<string, unknown> }>(query, {
    id,
    expectedVersion: version,
    data,
  });
  return { data: reshapeRecord<T>(raw.result, fields) };
}

export async function deleteRecord(
  entity: string,
  id: string,
  version: number,
) {
  const query = `mutation Delete($id: ID!, $expectedVersion: Int!) {
    result: ${deleteFieldName(entity)}(id: $id, expectedVersion: $expectedVersion) {
      id
    }
  }`;
  return graphqlAuthed<{ result: { id: string } }>(query, {
    id,
    expectedVersion: version,
  });
}

export async function transitionRecord<T = Record<string, unknown>>(
  entity: string,
  id: string,
  action: string,
  version: number,
  data?: Record<string, unknown>,
): Promise<SingleResponse<T>> {
  const fields = await fetchEntityFields(entity);
  const query = `mutation Transition($id: ID!, $action: String!, $expectedVersion: Int!, $data: Json) {
    result: ${transitionFieldName(entity)}(
      id: $id
      action: $action
      expectedVersion: $expectedVersion
      data: $data
    ) {
      ${recordSelection(fields)}
    }
  }`;
  const raw = await graphqlAuthed<{ result: Record<string, unknown> }>(query, {
    id,
    action,
    expectedVersion: version,
    data: data ?? null,
  });
  return { data: reshapeRecord<T>(raw.result, fields) };
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
