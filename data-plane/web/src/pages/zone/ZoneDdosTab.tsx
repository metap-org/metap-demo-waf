/**
 * Module 3 — DDoS L7 policy. One zone has at most one policy (`docs/02-domain-model.md`: 0..1
 * active), so this is an edit form rather than a list: either the policy exists and you tune it,
 * or it doesn't and you create it.
 *
 * Every write calls `sync-config-state` afterwards, which is what keeps `Zone.hasConfig` (and
 * therefore the `activate` guard) honest — deleting the last policy has to be able to take a zone
 * back to "not configured", not just adding the first one to "configured".
 */
import { useEffect, useState } from "react";
import { Button, Input, Label, Select, Toggle, toast } from "@metap/ui";
import {
  ENTITIES,
  createRecord,
  deleteRecord,
  syncConfigState,
  updateRecord,
  useInvalidateWaf,
  useRecords,
} from "../../api/waf";
import { EmptyState, SectionCard } from "../../components/primitives";

type DdosData = {
  zoneId?: string;
  sensitivity?: string;
  action?: string;
  requestRateThreshold?: number;
  burstWindow?: number;
  enabled?: boolean;
};

const DEFAULTS: DdosData = {
  sensitivity: "medium",
  action: "challenge",
  requestRateThreshold: 500,
  burstWindow: 60,
  enabled: true,
};

export function ZoneDdosTab({ zoneId }: { zoneId: string }) {
  const invalidate = useInvalidateWaf();
  const policies = useRecords<DdosData>(ENTITIES.ddosPolicies, { zoneId }, 1);
  const policy = policies.data?.[0];
  const [draft, setDraft] = useState<DdosData>(DEFAULTS);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (policy) setDraft({ ...DEFAULTS, ...policy.data });
  }, [policy?.id, policy?.version]);

  async function save() {
    setBusy(true);
    try {
      if (policy) {
        await updateRecord(ENTITIES.ddosPolicies, policy.id, policy.version, draft);
      } else {
        await createRecord(ENTITIES.ddosPolicies, { ...draft, zoneId });
      }
      await syncConfigState(zoneId);
      invalidate();
      toast({ title: "DDoS policy saved", variant: "success" });
    } catch (e) {
      toast({ title: e instanceof Error ? e.message : String(e), variant: "destructive" });
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    if (!policy) return;
    setBusy(true);
    try {
      await deleteRecord(ENTITIES.ddosPolicies, policy.id, policy.version);
      await syncConfigState(zoneId);
      invalidate();
      setDraft(DEFAULTS);
      toast({ title: "DDoS policy removed", variant: "success" });
    } catch (e) {
      toast({ title: e instanceof Error ? e.message : String(e), variant: "destructive" });
    } finally {
      setBusy(false);
    }
  }

  if (policies.isLoading) return <p className="mt-4 text-sm text-muted-foreground">Loading…</p>;

  return (
    <div className="mt-4">
      {!policy ? (
        <EmptyState
          title="No DDoS policy on this zone"
          description="Add one to shape how L7 floods are handled."
          action={<Button onClick={save}>Add default policy</Button>}
        />
      ) : null}

      <SectionCard
        title="L7 DDoS policy"
        description="Applies to every request for this zone before firewall rules run."
        actions={
          policy ? (
            <Button variant="outline" size="sm" onClick={remove} disabled={busy}>
              Remove
            </Button>
          ) : null
        }
      >
        <div className="grid gap-4 sm:grid-cols-2">
          <div>
            <Label htmlFor="sensitivity">Sensitivity</Label>
            <Select
              id="sensitivity"
              value={draft.sensitivity}
              onChange={(value) => setDraft({ ...draft, sensitivity: String(value) })}
              options={[
                { value: "low", label: "Low — only obvious floods" },
                { value: "medium", label: "Medium" },
                { value: "high", label: "High" },
                { value: "aggressive", label: "Aggressive — most false positives" },
              ]}
            />
          </div>
          <div>
            <Label htmlFor="action">Action</Label>
            <Select
              id="action"
              value={draft.action}
              onChange={(value) => setDraft({ ...draft, action: String(value) })}
              options={[
                { value: "log", label: "Log only" },
                { value: "challenge", label: "Challenge" },
                { value: "block", label: "Block" },
              ]}
            />
          </div>
          <div>
            <Label htmlFor="threshold">Request rate threshold</Label>
            <Input
              id="threshold"
              type="number"
              value={draft.requestRateThreshold ?? 0}
              onChange={(e) => setDraft({ ...draft, requestRateThreshold: Number(e.target.value) })}
            />
            <p className="mt-1 text-xs text-muted-foreground">Requests per burst window, per source.</p>
          </div>
          <div>
            <Label htmlFor="burst">Burst window (seconds)</Label>
            <Input
              id="burst"
              type="number"
              value={draft.burstWindow ?? 0}
              onChange={(e) => setDraft({ ...draft, burstWindow: Number(e.target.value) })}
            />
          </div>
          <div className="flex items-center gap-2">
            <Toggle
              id="enabled"
              checked={draft.enabled ?? true}
              onCheckedChange={(checked) => setDraft({ ...draft, enabled: checked })}
            />
            <Label htmlFor="enabled">Enabled</Label>
          </div>
        </div>
        <div className="mt-4">
          <Button onClick={save} disabled={busy}>
            {policy ? "Save changes" : "Create policy"}
          </Button>
        </div>
      </SectionCard>
    </div>
  );
}
