/**
 * Module 10 — tenant settings. Reads and writes `metap`'s tiered config surface
 * (`GET /admin/config`, `PUT /admin/config/{key}`), which already carries branding and session
 * policy, so this screen is a thin editor over it rather than a new settings store.
 *
 * What a key *is* stays declared in Rust (`metap-config`'s key registry): tier, default and
 * validator. This screen therefore renders whatever the backend says exists — adding a key there
 * makes it appear here with no frontend change — and only writes keys the backend marks as
 * tenant-writable, since anything else comes back as a rejection by design.
 */
import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { apiFetch, useAuth } from "@metap/platform-ui";
import { Button, Input, Table, TableBody, TableCell, TableHead, TableHeader, TableRow, toast } from "@metap/ui";
import { PageHeader, SectionCard, StatusBadge } from "../components/primitives";

type ConfigItem = {
  key: string;
  value: unknown;
  level: string;
  overridden: boolean;
  public: boolean;
};

export function SettingsPage() {
  const { status } = useAuth();
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);

  const config = useQuery({
    queryKey: ["waf-tenant-config"],
    queryFn: () => apiFetch<{ data: ConfigItem[] }>("/admin/config"),
    select: (response) => response.data,
    enabled: status === "authenticated",
  });

  async function save(item: ConfigItem) {
    const raw = drafts[item.key];
    if (raw === undefined) return;
    setBusy(true);
    try {
      // Numbers and booleans must go over the wire as their real JSON types — the key's validator
      // in Rust type-checks the value, so sending "30" where a number is expected is a 400.
      let value: unknown = raw;
      if (typeof item.value === "number") value = Number(raw);
      else if (typeof item.value === "boolean") value = raw === "true";
      await apiFetch(`/admin/config/${encodeURIComponent(item.key)}`, {
        method: "PUT",
        body: JSON.stringify({ value }),
      });
      await config.refetch();
      setDrafts((current) => {
        const next = { ...current };
        delete next[item.key];
        return next;
      });
      toast({ title: `${item.key} saved`, variant: "success" });
    } catch (e) {
      toast({ title: e instanceof Error ? e.message : String(e), variant: "destructive" });
    } finally {
      setBusy(false);
    }
  }

  async function reset(item: ConfigItem) {
    setBusy(true);
    try {
      await apiFetch(`/admin/config/${encodeURIComponent(item.key)}`, { method: "DELETE" });
      await config.refetch();
      toast({ title: `${item.key} reset to the platform default`, variant: "success" });
    } catch (e) {
      toast({ title: e instanceof Error ? e.message : String(e), variant: "destructive" });
    } finally {
      setBusy(false);
    }
  }

  const items = config.data ?? [];
  const branding = items.filter((item) => item.key.startsWith("theme."));
  const rest = items.filter((item) => !item.key.startsWith("theme."));

  function renderRows(rows: ConfigItem[]) {
    return rows.map((item) => {
      const current = drafts[item.key] ?? (item.value === null || item.value === undefined ? "" : String(item.value));
      return (
        <TableRow key={item.key}>
          <TableCell>
            <div className="font-mono text-xs">{item.key}</div>
            <div className="mt-1 flex gap-1">
              <StatusBadge value={item.level} />
              {item.public ? <span className="text-[10px] text-muted-foreground">public</span> : null}
            </div>
          </TableCell>
          <TableCell>
            <Input value={current} onChange={(e) => setDrafts({ ...drafts, [item.key]: e.target.value })} />
          </TableCell>
          <TableCell>{item.overridden ? "tenant" : "inherited"}</TableCell>
          <TableCell className="text-right">
            <Button size="sm" onClick={() => save(item)} disabled={busy || drafts[item.key] === undefined}>
              Save
            </Button>
            {item.overridden ? (
              <Button size="sm" variant="ghost" className="ml-1" onClick={() => reset(item)} disabled={busy}>
                Reset
              </Button>
            ) : null}
          </TableCell>
        </TableRow>
      );
    });
  }

  return (
    <div>
      <PageHeader title="Tenant settings" description="Branding and platform behaviour for this tenant." />

      <div className="grid gap-4">
        <SectionCard
          title="Branding"
          description="Shown on the sign-in screen before anyone has a session — served by the public config endpoint."
        >
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Key</TableHead>
                <TableHead>Value</TableHead>
                <TableHead>Source</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>{renderRows(branding)}</TableBody>
          </Table>
        </SectionCard>

        <SectionCard
          title="Platform"
          description="Operator-tier keys are listed for visibility but are refused by every API on purpose."
        >
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Key</TableHead>
                <TableHead>Value</TableHead>
                <TableHead>Source</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>{renderRows(rest)}</TableBody>
          </Table>
        </SectionCard>
      </div>
    </div>
  );
}
