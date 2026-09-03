/**
 * Module 2 — the zone list. Hand-built rather than `GeneratedList` because a zone row is the
 * product's primary object: it carries derived state (verification, routing, whether the zone has
 * any protection at all) that no single metadata field holds, and each row is a link into the
 * zone-centric IA the rest of the portal is organised around.
 */
import { Link } from "react-router-dom";
import { Button, Input, Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@metap/ui";
import { useState } from "react";
import { ENTITIES, useRecords, type ZoneData } from "../api/waf";
import { EmptyState, PageHeader, StatusBadge } from "../components/primitives";

export function ZonesPage() {
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState<string>("");
  // `hostname` is `searchable` in metadata, so the generic list route turns this filter into a
  // substring match server-side rather than something this component has to do client-side.
  const zones = useRecords<ZoneData>(ENTITIES.zones, { hostname: search || undefined, status: status || undefined }, 50);

  return (
    <div>
      <PageHeader
        title="Zones"
        description="Every hostname this tenant protects."
        actions={
          <Button asChild>
            <Link to="/onboarding">Add zone</Link>
          </Button>
        }
      />

      <div className="mb-3 flex flex-wrap gap-2">
        <Input
          className="max-w-xs"
          placeholder="Search hostname…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <div className="flex gap-1">
          {["", "active", "pending", "paused", "suspended"].map((value) => (
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
      </div>

      {zones.isLoading ? (
        <p className="text-sm text-muted-foreground">Loading…</p>
      ) : (zones.data ?? []).length === 0 ? (
        <EmptyState
          title="No zones yet"
          description="Add your first hostname to start protecting it."
          action={
            <Button asChild>
              <Link to="/onboarding">Add zone</Link>
            </Button>
          }
        />
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Hostname</TableHead>
              <TableHead>Status</TableHead>
              <TableHead>Mode</TableHead>
              <TableHead>Domain</TableHead>
              <TableHead>DNS routing</TableHead>
              <TableHead>Protection</TableHead>
              <TableHead className="text-right">Config v.</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {(zones.data ?? []).map((zone) => (
              <TableRow key={zone.id}>
                <TableCell>
                  <Link className="font-medium hover:underline" to={`/zones/${zone.id}`}>
                    {zone.data.hostname}
                  </Link>
                  <div className="text-xs text-muted-foreground">{zone.data.originAddress}</div>
                </TableCell>
                <TableCell>
                  <StatusBadge value={zone.data.status ?? zone.status} />
                </TableCell>
                <TableCell>
                  <StatusBadge value={zone.data.protectionMode} />
                </TableCell>
                <TableCell>
                  <StatusBadge value={zone.data.verificationStatus} />
                </TableCell>
                <TableCell>
                  <StatusBadge value={zone.data.dnsRoutingStatus} />
                </TableCell>
                <TableCell>
                  {zone.data.hasConfig ? (
                    <span className="text-sm text-muted-foreground">configured</span>
                  ) : (
                    <span className="text-sm text-amber-600 dark:text-amber-500">none</span>
                  )}
                </TableCell>
                <TableCell className="text-right tabular-nums">{zone.data.configVersion ?? 0}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </div>
  );
}
