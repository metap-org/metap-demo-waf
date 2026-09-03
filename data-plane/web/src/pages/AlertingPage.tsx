/**
 * Module 8 — alerting. Two halves that belong on one screen: the policies (configuration) and the
 * notification log (what actually happened), because the only way to trust an alert policy is to
 * see what it has delivered.
 *
 * "Send test" and "Evaluate now" both hit the real backend paths — the same delivery code a
 * scheduled evaluation uses — so a green result here means the channel genuinely works, not that
 * a mock succeeded.
 */
import { useState } from "react";
import {
  Button,
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Label,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  Textarea,
  Toggle,
  toast,
} from "@metap/ui";
import {
  ENTITIES,
  createRecord,
  evaluateAlerts,
  testAlertPolicy,
  updateRecord,
  useInvalidateWaf,
  useRecords,
  type WafRecord,
} from "../api/waf";
import { EmptyState, PageHeader, SectionCard, StatusBadge, shortDate } from "../components/primitives";

type PolicyData = {
  name?: string;
  thresholdCount?: number;
  windowMinutes?: number;
  channels?: Record<string, unknown>;
  enabled?: boolean;
};

type NotificationData = {
  alertPolicyId?: string;
  channel?: string;
  deliveryStatus?: string;
  triggeredAt?: string;
};

const EMPTY: PolicyData = {
  name: "",
  thresholdCount: 50,
  windowMinutes: 15,
  enabled: true,
  channels: { webhook: "" },
};

export function AlertingPage() {
  const invalidate = useInvalidateWaf();
  const policies = useRecords<PolicyData>(ENTITIES.alertPolicies, {}, 50);
  const notifications = useRecords<NotificationData>(ENTITIES.alertNotifications, {}, 25);
  const [open, setOpen] = useState(false);
  const [editing, setEditing] = useState<WafRecord<PolicyData> | null>(null);
  const [draft, setDraft] = useState<PolicyData>(EMPTY);
  const [channelsText, setChannelsText] = useState(JSON.stringify(EMPTY.channels, null, 2));
  const [busy, setBusy] = useState(false);

  function startCreate() {
    setEditing(null);
    setDraft(EMPTY);
    setChannelsText(JSON.stringify(EMPTY.channels, null, 2));
    setOpen(true);
  }

  function startEdit(policy: WafRecord<PolicyData>) {
    setEditing(policy);
    setDraft(policy.data);
    setChannelsText(JSON.stringify(policy.data.channels ?? {}, null, 2));
    setOpen(true);
  }

  async function save() {
    setBusy(true);
    try {
      let channels: unknown;
      try {
        channels = JSON.parse(channelsText);
      } catch {
        toast({ title: "Channels is not valid JSON", variant: "destructive" });
        return;
      }
      const payload = { ...draft, channels };
      if (editing) {
        await updateRecord(ENTITIES.alertPolicies, editing.id, editing.version, payload);
      } else {
        await createRecord(ENTITIES.alertPolicies, payload);
      }
      invalidate();
      setOpen(false);
      toast({ title: "Alert policy saved", variant: "success" });
    } catch (e) {
      toast({ title: e instanceof Error ? e.message : String(e), variant: "destructive" });
    } finally {
      setBusy(false);
    }
  }

  async function sendTest(policy: WafRecord<PolicyData>) {
    setBusy(true);
    try {
      const result = await testAlertPolicy(policy.id);
      invalidate();
      toast({
        title: result.data.delivered ? `Delivered — ${result.data.detail}` : `Not delivered — ${result.data.detail}`,
        variant: result.data.delivered ? "success" : "destructive",
      });
    } catch (e) {
      toast({ title: e instanceof Error ? e.message : String(e), variant: "destructive" });
    } finally {
      setBusy(false);
    }
  }

  async function evaluateNow() {
    setBusy(true);
    try {
      const result = await evaluateAlerts();
      invalidate();
      toast({
        title: `Evaluated ${result.data.policiesEvaluated} policies · ${result.data.fired.length} fired`,
        variant: "success",
      });
    } catch (e) {
      toast({ title: e instanceof Error ? e.message : String(e), variant: "destructive" });
    } finally {
      setBusy(false);
    }
  }

  const policyName = (id?: string) => policies.data?.find((p) => p.id === id)?.data.name ?? id ?? "—";

  return (
    <div>
      <PageHeader
        title="Alerting"
        description="When traffic on a zone crosses a threshold, tell someone."
        actions={
          <>
            <Button variant="outline" onClick={evaluateNow} disabled={busy}>
              Evaluate now
            </Button>
            <Button onClick={startCreate}>New policy</Button>
          </>
        }
      />

      <div className="grid gap-4">
        <SectionCard title="Alert policies" description="Counted per zone — never summed across zones.">
          {(policies.data ?? []).length === 0 ? (
            <EmptyState title="No alert policies" description="Create one to get told when a zone is under attack." />
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Rule</TableHead>
                  <TableHead>Channel</TableHead>
                  <TableHead>Enabled</TableHead>
                  <TableHead className="text-right">Actions</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {(policies.data ?? []).map((policy) => (
                  <TableRow key={policy.id}>
                    <TableCell className="font-medium">{policy.data.name}</TableCell>
                    <TableCell className="text-sm text-muted-foreground">
                      ≥ {policy.data.thresholdCount} events in {policy.data.windowMinutes}m
                    </TableCell>
                    <TableCell className="font-mono text-xs">
                      {Object.keys(policy.data.channels ?? {}).join(", ") || "—"}
                    </TableCell>
                    <TableCell>{policy.data.enabled ? "yes" : "no"}</TableCell>
                    <TableCell className="text-right">
                      <Button size="sm" variant="outline" onClick={() => sendTest(policy)} disabled={busy}>
                        Send test
                      </Button>
                      <Button size="sm" variant="ghost" className="ml-1" onClick={() => startEdit(policy)}>
                        Edit
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </SectionCard>

        <SectionCard title="Delivery log" description="Every firing, sent or failed.">
          {(notifications.data ?? []).length === 0 ? (
            <EmptyState title="Nothing delivered yet" />
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>When</TableHead>
                  <TableHead>Policy</TableHead>
                  <TableHead>Channel</TableHead>
                  <TableHead>Status</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {(notifications.data ?? []).map((notification) => (
                  <TableRow key={notification.id}>
                    <TableCell className="whitespace-nowrap text-muted-foreground">
                      {shortDate(notification.data.triggeredAt ?? notification.createdAt)}
                    </TableCell>
                    <TableCell>{policyName(notification.data.alertPolicyId)}</TableCell>
                    <TableCell>{notification.data.channel}</TableCell>
                    <TableCell>
                      <StatusBadge value={notification.data.deliveryStatus} />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </SectionCard>
      </div>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{editing ? "Edit alert policy" : "New alert policy"}</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3">
            <div>
              <Label htmlFor="policy-name">Name</Label>
              <Input
                id="policy-name"
                value={draft.name ?? ""}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              />
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              <div>
                <Label htmlFor="policy-threshold">Threshold (events)</Label>
                <Input
                  id="policy-threshold"
                  type="number"
                  value={draft.thresholdCount ?? 0}
                  onChange={(e) => setDraft({ ...draft, thresholdCount: Number(e.target.value) })}
                />
              </div>
              <div>
                <Label htmlFor="policy-window">Window (minutes)</Label>
                <Input
                  id="policy-window"
                  type="number"
                  value={draft.windowMinutes ?? 0}
                  onChange={(e) => setDraft({ ...draft, windowMinutes: Number(e.target.value) })}
                />
              </div>
            </div>
            <div>
              <Label htmlFor="policy-channels">Channels</Label>
              <Textarea
                id="policy-channels"
                rows={4}
                className="font-mono text-xs"
                value={channelsText}
                onChange={(e) => setChannelsText(e.target.value)}
              />
              <p className="mt-1 text-xs text-muted-foreground">
                {'{"webhook": "https://…"}'} posts the alert. {'{"email": "…"}'} is logged only — there is no mail
                transport in this product yet.
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Toggle
                id="policy-enabled"
                checked={draft.enabled ?? true}
                onCheckedChange={(checked) => setDraft({ ...draft, enabled: checked })}
              />
              <Label htmlFor="policy-enabled">Enabled</Label>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button onClick={save} disabled={busy}>
              Save
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
