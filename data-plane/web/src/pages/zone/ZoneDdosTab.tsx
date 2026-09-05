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
import { useTranslation } from "react-i18next";
import {
  Button,
  EmptyState,
  Input,
  Label,
  Select,
  SectionCard,
  Toggle,
  toast,
} from "@metap/ui";
import { useAsyncAction } from "@metap/platform-ui";
import {
  ENTITIES,
  createRecord,
  deleteRecord,
  syncConfigState,
  updateRecord,
  useInvalidateWaf,
  useRecords,
} from "../../api/waf";

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
  const { t } = useTranslation();
  const invalidate = useInvalidateWaf();
  const policies = useRecords<DdosData>(ENTITIES.ddosPolicies, { zoneId }, 1);
  const policy = policies.data?.[0];
  const [draft, setDraft] = useState<DdosData>(DEFAULTS);
  const { busy, run } = useAsyncAction();

  useEffect(() => {
    if (policy) setDraft({ ...DEFAULTS, ...policy.data });
  }, [policy?.id, policy?.version]);

  async function save() {
    await run(async () => {
      if (policy) {
        await updateRecord(
          ENTITIES.ddosPolicies,
          policy.id,
          policy.version,
          draft,
        );
      } else {
        await createRecord(ENTITIES.ddosPolicies, { ...draft, zoneId });
      }
      await syncConfigState(zoneId);
      invalidate();
      toast(t("waf.zoneTabs.ddos.toastSaved"), { variant: "default" });
    });
  }

  async function remove() {
    if (!policy) return;
    await run(async () => {
      await deleteRecord(ENTITIES.ddosPolicies, policy.id, policy.version);
      await syncConfigState(zoneId);
      invalidate();
      setDraft(DEFAULTS);
      toast(t("waf.zoneTabs.ddos.toastRemoved"), { variant: "default" });
    });
  }

  if (policies.isLoading)
    return (
      <p className="mt-4 text-sm text-muted-foreground">
        {t("waf.zoneTabs.ddos.loading")}
      </p>
    );

  return (
    <div className="mt-4">
      {!policy ? (
        <EmptyState
          title={t("waf.zoneTabs.ddos.noPolicy")}
          description={t("waf.zoneTabs.ddos.noPolicyDescription")}
          action={
            <Button onClick={save}>
              {t("waf.zoneTabs.ddos.addDefaultPolicy")}
            </Button>
          }
        />
      ) : null}

      <SectionCard
        title={t("waf.zoneTabs.ddos.title")}
        description={t("waf.zoneTabs.ddos.description")}
        actions={
          policy ? (
            <Button
              variant="outline"
              size="sm"
              onClick={remove}
              disabled={busy}
            >
              {t("waf.zoneTabs.ddos.remove")}
            </Button>
          ) : null
        }
      >
        <div className="grid gap-4 sm:grid-cols-2">
          <div>
            <Label htmlFor="sensitivity">
              {t("waf.zoneTabs.ddos.sensitivity")}
            </Label>
            <Select
              id="sensitivity"
              value={draft.sensitivity}
              onChange={(value) =>
                setDraft({ ...draft, sensitivity: String(value) })
              }
              options={[
                { value: "low", label: t("waf.zoneTabs.ddos.sensitivityLow") },
                {
                  value: "medium",
                  label: t("waf.zoneTabs.ddos.sensitivityMedium"),
                },
                {
                  value: "high",
                  label: t("waf.zoneTabs.ddos.sensitivityHigh"),
                },
                {
                  value: "aggressive",
                  label: t("waf.zoneTabs.ddos.sensitivityAggressive"),
                },
              ]}
            />
          </div>
          <div>
            <Label htmlFor="action">{t("waf.zoneTabs.ddos.action")}</Label>
            <Select
              id="action"
              value={draft.action}
              onChange={(value) =>
                setDraft({ ...draft, action: String(value) })
              }
              options={[
                { value: "log", label: t("waf.zoneTabs.ddos.actionLog") },
                {
                  value: "challenge",
                  label: t("waf.zoneTabs.ddos.actionChallenge"),
                },
                { value: "block", label: t("waf.zoneTabs.ddos.actionBlock") },
              ]}
            />
          </div>
          <div>
            <Label htmlFor="threshold">
              {t("waf.zoneTabs.ddos.threshold")}
            </Label>
            <Input
              id="threshold"
              type="number"
              value={draft.requestRateThreshold ?? 0}
              onChange={(e) =>
                setDraft({
                  ...draft,
                  requestRateThreshold: Number(e.target.value),
                })
              }
            />
            <p className="mt-1 text-xs text-muted-foreground">
              {t("waf.zoneTabs.ddos.thresholdHint")}
            </p>
          </div>
          <div>
            <Label htmlFor="burst">{t("waf.zoneTabs.ddos.burstWindow")}</Label>
            <Input
              id="burst"
              type="number"
              value={draft.burstWindow ?? 0}
              onChange={(e) =>
                setDraft({ ...draft, burstWindow: Number(e.target.value) })
              }
            />
          </div>
          <div className="flex items-center gap-2">
            <Toggle
              id="enabled"
              checked={draft.enabled ?? true}
              onCheckedChange={(checked) =>
                setDraft({ ...draft, enabled: checked })
              }
            />
            <Label htmlFor="enabled">{t("waf.zoneTabs.ddos.enabled")}</Label>
          </div>
        </div>
        <div className="mt-4">
          <Button onClick={save} disabled={busy}>
            {policy
              ? t("waf.zoneTabs.ddos.saveChanges")
              : t("waf.zoneTabs.ddos.createPolicy")}
          </Button>
        </div>
      </SectionCard>
    </div>
  );
}
