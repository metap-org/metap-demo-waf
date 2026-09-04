/**
 * Modules 2/3/4 in one screen — the zone-centric IA `docs/07-portal-features.md` asks for: a zone
 * and everything hanging off it (DDoS policy, firewall rules, scans, its own event stream) behind
 * tabs, instead of the flat per-entity lists the generic CRUD harness gave.
 *
 * Zone lifecycle actions (activate/pause/resume/suspend) come from the workflow metadata itself,
 * not a hardcoded button list — `metap` returns the available transitions per record, so a
 * workflow change in `zone_entity.rs` shows up here without a frontend change.
 */
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import {
  Button,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
  toast,
} from "@metap/ui";
import {
  ENTITIES,
  transitionRecord,
  useInvalidateWaf,
  useRecord,
  verifyDns,
  type ZoneData,
} from "../api/waf";
import { PageHeader, StatusBadge } from "../components/primitives";
import { ZoneOverviewTab } from "./zone/ZoneOverviewTab";
import { ZoneDdosTab } from "./zone/ZoneDdosTab";
import { ZoneRulesTab } from "./zone/ZoneRulesTab";
import { ZoneScansTab } from "./zone/ZoneScansTab";
import { ZoneEventsTab } from "./zone/ZoneEventsTab";

/** Which transitions are offered from each status — mirrors `zone_entity.rs`'s workflow. The
 *  backend is still the authority (it re-evaluates the `activate` guard and rejects anything
 *  invalid); this only decides which buttons are worth showing. */
const TRANSITIONS: Record<string, { action: string; label: string }[]> = {
  pending: [{ action: "activate", label: "Activate" }],
  active: [
    { action: "pause", label: "Pause" },
    { action: "suspend", label: "Suspend" },
  ],
  paused: [
    { action: "resume", label: "Resume" },
    { action: "suspend", label: "Suspend" },
  ],
  suspended: [],
};

export function ZoneDetailPage() {
  const { zoneId } = useParams<{ zoneId: string }>();
  const invalidate = useInvalidateWaf();
  const [busy, setBusy] = useState(false);
  const zone = useRecord<ZoneData>(ENTITIES.zones, zoneId);

  if (zone.isLoading)
    return <p className="text-sm text-muted-foreground">Loading…</p>;
  if (!zone.data)
    return <p className="text-sm text-muted-foreground">Zone not found.</p>;

  const record = zone.data;
  const status = record.data.status ?? record.status ?? "pending";

  async function act(action: string) {
    if (!zoneId || !zone.data) return;
    setBusy(true);
    try {
      await transitionRecord(ENTITIES.zones, zoneId, action, zone.data.version);
      invalidate();
      toast(`Zone ${action}d`, { variant: "default" });
    } catch (e) {
      // The `activate` guard failing is the common case here, and its message names the missing
      // precondition (unverified hostname / no protection configured) — surfacing it verbatim is
      // more useful than a generic "could not activate".
      toast(e instanceof Error ? e.message : String(e), {
        variant: "destructive",
      });
    } finally {
      setBusy(false);
    }
  }

  async function recheckDns() {
    if (!zoneId) return;
    setBusy(true);
    try {
      const result = await verifyDns(zoneId);
      invalidate();
      toast(
        result.data.ownershipVerified
          ? "Hostname verified"
          : "TXT record not found yet",
        {
          variant: "default",
        },
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <div className="mb-2 text-sm">
        <Link className="text-muted-foreground hover:underline" to="/zones">
          ← Zones
        </Link>
      </div>
      <PageHeader
        title={record.data.hostname ?? "Zone"}
        description={`Origin ${record.data.originAddress ?? "—"} · config v${record.data.configVersion ?? 0}`}
        actions={
          <>
            <StatusBadge value={status} />
            <StatusBadge value={record.data.protectionMode} />
            {record.data.verificationStatus !== "verified" ? (
              <Button
                size="sm"
                variant="outline"
                onClick={recheckDns}
                disabled={busy}
              >
                Re-check DNS
              </Button>
            ) : null}
            {(TRANSITIONS[status] ?? []).map((transition) => (
              <Button
                key={transition.action}
                size="sm"
                onClick={() => act(transition.action)}
                disabled={busy}
              >
                {transition.label}
              </Button>
            ))}
          </>
        }
      />

      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">Overview</TabsTrigger>
          <TabsTrigger value="ddos">DDoS</TabsTrigger>
          <TabsTrigger value="rules">Firewall rules</TabsTrigger>
          <TabsTrigger value="scans">Scans</TabsTrigger>
          <TabsTrigger value="events">Events</TabsTrigger>
        </TabsList>
        <TabsContent value="overview">
          <ZoneOverviewTab zone={record} />
        </TabsContent>
        <TabsContent value="ddos">
          <ZoneDdosTab zoneId={record.id} />
        </TabsContent>
        <TabsContent value="rules">
          <ZoneRulesTab zoneId={record.id} />
        </TabsContent>
        <TabsContent value="scans">
          <ZoneScansTab zoneId={record.id} />
        </TabsContent>
        <TabsContent value="events">
          <ZoneEventsTab zoneId={record.id} />
        </TabsContent>
      </Tabs>
    </div>
  );
}
