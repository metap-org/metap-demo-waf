/**
 * Module 4 — firewall rules. One engine for WAF custom rules, rate limiting and IP/geo firewall
 * (`docs/01-product-vision.md`'s deliberate decision not to ship three separate features), which
 * is why `ruleType` is a field on one entity rather than three entities.
 *
 * Priority is what makes this more than a CRUD list: rules are evaluated in priority order and the
 * first match wins, so the screen has to show that order and let it be changed. Reordering swaps
 * two rules' `priority` values rather than renumbering the whole list — two writes instead of N,
 * and no window where two rules share a priority.
 */
import { useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  EmptyState,
  Input,
  Label,
  SectionCard,
  Select,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  Textarea,
  toast,
} from "@metap/ui";
import {
  ENTITIES,
  createRecord,
  deleteRecord,
  syncConfigState,
  updateRecord,
  useInvalidateWaf,
  useRecords,
  type WafRecord,
} from "../../api/waf";
import { useAsyncAction } from "@metap/platform-ui";
import { StatusBadge } from "../../components/primitives";

type RuleData = {
  zoneId?: string;
  name?: string;
  ruleType?: string;
  priority?: number;
  action?: string;
  enabled?: boolean;
  matchCondition?: unknown;
  rateLimitThreshold?: number;
  rateLimitWindow?: number;
};

const EMPTY: RuleData = {
  name: "",
  ruleType: "waf",
  action: "block",
  priority: 100,
  enabled: true,
  matchCondition: { field: "uri.path", op: "contains", value: "/admin" },
};

export function ZoneRulesTab({ zoneId }: { zoneId: string }) {
  const { t } = useTranslation();
  const invalidate = useInvalidateWaf();
  const rules = useRecords<RuleData>(ENTITIES.firewallRules, { zoneId }, 100);
  const [editing, setEditing] = useState<WafRecord<RuleData> | null>(null);
  const [draft, setDraft] = useState<RuleData>(EMPTY);
  const [conditionText, setConditionText] = useState(
    JSON.stringify(EMPTY.matchCondition, null, 2),
  );
  const [open, setOpen] = useState(false);
  const { busy, run } = useAsyncAction();

  // Sorted here rather than by the list API: `priority` is not in the entity's sortable fields,
  // and the list is small (one zone's rules), so ordering client-side is cheaper than widening
  // the metadata for it.
  const ordered = [...(rules.data ?? [])].sort(
    (a, b) => (a.data.priority ?? 0) - (b.data.priority ?? 0),
  );

  function startCreate() {
    setEditing(null);
    const nextPriority =
      ordered.length > 0
        ? (ordered[ordered.length - 1]?.data.priority ?? 0) + 10
        : 100;
    setDraft({ ...EMPTY, priority: nextPriority });
    setConditionText(JSON.stringify(EMPTY.matchCondition, null, 2));
    setOpen(true);
  }

  function startEdit(rule: WafRecord<RuleData>) {
    setEditing(rule);
    setDraft(rule.data);
    setConditionText(JSON.stringify(rule.data.matchCondition ?? {}, null, 2));
    setOpen(true);
  }

  async function save() {
    await run(async () => {
      let matchCondition: unknown;
      try {
        matchCondition = JSON.parse(conditionText);
      } catch {
        toast(t("waf.zoneTabs.rules.toastInvalidJson"), {
          variant: "destructive",
        });
        return;
      }
      const payload = { ...draft, matchCondition, zoneId };
      if (editing) {
        await updateRecord(
          ENTITIES.firewallRules,
          editing.id,
          editing.version,
          payload,
        );
      } else {
        await createRecord(ENTITIES.firewallRules, payload);
      }
      await syncConfigState(zoneId);
      invalidate();
      setOpen(false);
      toast(t("waf.zoneTabs.rules.toastSaved"), { variant: "default" });
    });
  }

  async function remove(rule: WafRecord<RuleData>) {
    await run(async () => {
      await deleteRecord(ENTITIES.firewallRules, rule.id, rule.version);
      await syncConfigState(zoneId);
      invalidate();
      toast(t("waf.zoneTabs.rules.toastDeleted"), { variant: "default" });
    });
  }

  /** Swaps this rule's priority with its neighbour's — see the file header for why swap, not
   *  renumber. */
  async function move(rule: WafRecord<RuleData>, direction: -1 | 1) {
    const index = ordered.findIndex((r) => r.id === rule.id);
    const neighbour = ordered[index + direction];
    if (!neighbour) return;
    await run(async () => {
      await updateRecord(ENTITIES.firewallRules, rule.id, rule.version, {
        priority: neighbour.data.priority,
      });
      await updateRecord(
        ENTITIES.firewallRules,
        neighbour.id,
        neighbour.version,
        { priority: rule.data.priority },
      );
      invalidate();
    });
  }

  return (
    <div className="mt-4">
      <SectionCard
        title={t("waf.zoneTabs.rules.title")}
        description={t("waf.zoneTabs.rules.description")}
        actions={
          <Button size="sm" onClick={startCreate}>
            {t("waf.zoneTabs.rules.addRule")}
          </Button>
        }
      >
        {ordered.length === 0 ? (
          <EmptyState
            title={t("waf.zoneTabs.rules.noRulesYet")}
            description={t("waf.zoneTabs.rules.noRulesYetDescription")}
          />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead className="w-24">
                  {t("waf.zoneTabs.rules.colPriority")}
                </TableHead>
                <TableHead>{t("waf.zoneTabs.rules.colName")}</TableHead>
                <TableHead>{t("waf.zoneTabs.rules.colType")}</TableHead>
                <TableHead>{t("waf.zoneTabs.rules.colAction")}</TableHead>
                <TableHead>{t("waf.zoneTabs.rules.colEnabled")}</TableHead>
                <TableHead className="text-right">
                  {t("waf.zoneTabs.rules.colActions")}
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {ordered.map((rule, index) => (
                <TableRow key={rule.id}>
                  <TableCell className="tabular-nums">
                    <div className="flex items-center gap-1">
                      {rule.data.priority}
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={busy || index === 0}
                        onClick={() => move(rule, -1)}
                        aria-label={t("waf.zoneTabs.rules.moveUp")}
                      >
                        ↑
                      </Button>
                      <Button
                        size="sm"
                        variant="ghost"
                        disabled={busy || index === ordered.length - 1}
                        onClick={() => move(rule, 1)}
                        aria-label={t("waf.zoneTabs.rules.moveDown")}
                      >
                        ↓
                      </Button>
                    </div>
                  </TableCell>
                  <TableCell className="font-medium">
                    {rule.data.name}
                  </TableCell>
                  <TableCell>
                    <StatusBadge value={rule.data.ruleType} />
                  </TableCell>
                  <TableCell>
                    <StatusBadge value={rule.data.action} />
                  </TableCell>
                  <TableCell>
                    {rule.data.enabled
                      ? t("waf.common.yes")
                      : t("waf.common.no")}
                  </TableCell>
                  <TableCell className="text-right">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => startEdit(rule)}
                    >
                      {t("waf.common.edit")}
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => remove(rule)}
                      disabled={busy}
                    >
                      {t("waf.common.delete")}
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </SectionCard>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {editing
                ? t("waf.zoneTabs.rules.editRule")
                : t("waf.zoneTabs.rules.newRule")}
            </DialogTitle>
          </DialogHeader>
          <div className="grid gap-3">
            <div>
              <Label htmlFor="rule-name">{t("waf.zoneTabs.rules.name")}</Label>
              <Input
                id="rule-name"
                value={draft.name ?? ""}
                onChange={(e) => setDraft({ ...draft, name: e.target.value })}
              />
            </div>
            <div className="grid gap-3 sm:grid-cols-2">
              <div>
                <Label htmlFor="rule-type">
                  {t("waf.zoneTabs.rules.type")}
                </Label>
                <Select
                  id="rule-type"
                  value={draft.ruleType}
                  onValueChange={(value) =>
                    setDraft({ ...draft, ruleType: String(value) })
                  }
                  options={[
                    { value: "waf", label: t("waf.zoneTabs.rules.typeWaf") },
                    {
                      value: "rateLimit",
                      label: t("waf.zoneTabs.rules.typeRateLimit"),
                    },
                    {
                      value: "ipFirewall",
                      label: t("waf.zoneTabs.rules.typeIpFirewall"),
                    },
                    {
                      value: "geoFirewall",
                      label: t("waf.zoneTabs.rules.typeGeoFirewall"),
                    },
                  ]}
                />
              </div>
              <div>
                <Label htmlFor="rule-action">
                  {t("waf.zoneTabs.rules.action")}
                </Label>
                <Select
                  id="rule-action"
                  value={draft.action}
                  onValueChange={(value) =>
                    setDraft({ ...draft, action: String(value) })
                  }
                  options={[
                    {
                      value: "allow",
                      label: t("waf.zoneTabs.rules.actionAllow"),
                    },
                    { value: "log", label: t("waf.zoneTabs.rules.actionLog") },
                    {
                      value: "challenge",
                      label: t("waf.zoneTabs.rules.actionChallenge"),
                    },
                    {
                      value: "block",
                      label: t("waf.zoneTabs.rules.actionBlock"),
                    },
                  ]}
                />
              </div>
            </div>
            {draft.ruleType === "rateLimit" ? (
              <div className="grid gap-3 sm:grid-cols-2">
                <div>
                  <Label htmlFor="rl-threshold">
                    {t("waf.zoneTabs.rules.requests")}
                  </Label>
                  <Input
                    id="rl-threshold"
                    type="number"
                    value={draft.rateLimitThreshold ?? 100}
                    onChange={(e) =>
                      setDraft({
                        ...draft,
                        rateLimitThreshold: Number(e.target.value),
                      })
                    }
                  />
                </div>
                <div>
                  <Label htmlFor="rl-window">
                    {t("waf.zoneTabs.rules.windowSeconds")}
                  </Label>
                  <Input
                    id="rl-window"
                    type="number"
                    value={draft.rateLimitWindow ?? 60}
                    onChange={(e) =>
                      setDraft({
                        ...draft,
                        rateLimitWindow: Number(e.target.value),
                      })
                    }
                  />
                </div>
              </div>
            ) : null}
            <div>
              <Label htmlFor="rule-condition">
                {t("waf.zoneTabs.rules.matchCondition")}
              </Label>
              {/* Raw JSON on purpose: whether this grammar reuses `metap-permission`'s
                  `PolicyCondition` or needs its own (request fields like `uri.path` vs. entity
                  fields) is still an open question in `docs/02-domain-model.md`. A visual builder
                  built on the wrong grammar would be thrown away. */}
              <Textarea
                id="rule-condition"
                rows={5}
                className="font-mono text-xs"
                value={conditionText}
                onChange={(e) => setConditionText(e.target.value)}
              />
              <p className="mt-1 text-xs text-muted-foreground">
                {t("waf.zoneTabs.rules.matchConditionHint")}
              </p>
            </div>
            <div>
              <Label htmlFor="rule-priority">
                {t("waf.zoneTabs.rules.priority")}
              </Label>
              <Input
                id="rule-priority"
                type="number"
                value={draft.priority ?? 100}
                onChange={(e) =>
                  setDraft({ ...draft, priority: Number(e.target.value) })
                }
              />
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              {t("waf.common.cancel")}
            </Button>
            <Button onClick={save} disabled={busy}>
              {t("waf.common.save")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
