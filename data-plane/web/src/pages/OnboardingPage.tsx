/**
 * Module 1 — "Add a zone", the flow `docs/06-onboarding-rules-lists.md` and
 * `docs/11-onboarding-dns-resolution.md` describe end to end.
 *
 * This is the screen that most clearly is not generic CRUD: creating the record is one of five
 * steps, and the other four (prove domain ownership over DNS, check the origin answers, give the
 * zone its first protection, activate it) each call something bespoke. It also happens to be the
 * flow that exposes why `Zone.hasConfig` exists — the `activate` guard needs both a verified
 * hostname *and* at least one policy or rule, and a workflow guard cannot count related records.
 */
import { Fragment, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Alert,
  AlertDescription,
  Button,
  Input,
  Label,
  Select,
  Stepper,
  StepperConnector,
  StepperGroup,
  StepperItem,
} from "@metap/ui";
import {
  ENTITIES,
  createRecord,
  syncConfigState,
  testOrigin,
  transitionRecord,
  useInvalidateWaf,
  verifyDns,
  type OriginTestResult,
  type Zone,
} from "../api/waf";
import { PageHeader, SectionCard, StatusBadge } from "../components/primitives";

/** The zone's DNS-TXT challenge value.
 *
 * Generated here rather than server-side because `metap`'s create path is metadata-driven — there
 * is no per-entity "before create" hook to mint a token in, and adding one to core for a single
 * app's field would be exactly the business-entity knowledge `metap-*` crates must not carry. The
 * value only has to be unguessable-per-zone, which `crypto.randomUUID()` satisfies; a future
 * `computed` field default would be the cleaner home for it.
 */
function newVerificationToken(): string {
  return `waf-verify-${crypto.randomUUID()}`;
}

type Step = 0 | 1 | 2 | 3;

export function OnboardingPage() {
  const navigate = useNavigate();
  const invalidate = useInvalidateWaf();

  const [step, setStep] = useState<Step>(0);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [hostname, setHostname] = useState("");
  const [originAddress, setOriginAddress] = useState("");
  const [protectionMode, setProtectionMode] = useState("monitor");
  const [zone, setZone] = useState<Zone | null>(null);
  const [dnsResult, setDnsResult] = useState<{
    ownershipVerified: boolean;
    dnsRouted: boolean;
    target: string;
  } | null>(null);
  const [originResult, setOriginResult] = useState<
    OriginTestResult["data"] | null
  >(null);

  async function guard(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  const createZone = () =>
    guard(async () => {
      const response = await createRecord<Zone["data"]>(ENTITIES.zones, {
        hostname: hostname.trim(),
        originAddress: originAddress.trim(),
        protectionMode,
        verificationMethod: "dnsTxt",
        verificationToken: newVerificationToken(),
        verificationStatus: "unverified",
        dnsRoutingStatus: "unknown",
        hasConfig: false,
        configVersion: 1,
      });
      setZone(response.data as Zone);
      invalidate();
      setStep(1);
    });

  const runVerify = () =>
    guard(async () => {
      if (!zone) return;
      const response = await verifyDns(zone.id);
      setZone(response.data.zone);
      setDnsResult({
        ownershipVerified: response.data.ownershipVerified,
        dnsRouted: response.data.dnsRouted,
        target: response.data.checked.expectedTarget,
      });
      invalidate();
    });

  const runOriginTest = () =>
    guard(async () => {
      if (!zone) return;
      const response = await testOrigin(zone.id);
      setOriginResult(response.data);
    });

  /** Step 3 gives the zone its first real protection. A sensible default DDoS policy is enough to
   *  satisfy `hasConfig` and means a new zone is never live with nothing in front of it. */
  const addBaselineProtection = () =>
    guard(async () => {
      if (!zone) return;
      await createRecord(ENTITIES.ddosPolicies, {
        zoneId: zone.id,
        sensitivity: "medium",
        action: "challenge",
        requestRateThreshold: 500,
        burstWindow: 60,
        enabled: true,
      });
      // The flag the `activate` guard reads — recomputed from the records that now exist rather
      // than assumed, so a failed create can't leave the zone claiming a config it doesn't have.
      await syncConfigState(zone.id);
      invalidate();
      setStep(3);
    });

  const activate = () =>
    guard(async () => {
      if (!zone) return;
      // Re-read through the sync endpoint first: `zone.version` in this component is from before
      // `sync-config-state` bumped it, and the transition is version-gated.
      const fresh = await transitionRecord<Zone["data"]>(
        ENTITIES.zones,
        zone.id,
        "activate",
        zone.version + 1,
      );
      setZone(fresh.data as Zone);
      invalidate();
      navigate(`/zones/${zone.id}`);
    });

  const verified = zone?.data.verificationStatus === "verified";

  return (
    <div className="mx-auto max-w-3xl">
      <PageHeader
        title="Add a zone"
        description="Protect a new hostname in four steps."
      />

      <Stepper className="mb-6">
        {(["Details", "Verify domain", "Protection", "Activate"] as const).map(
          (label, index) => (
            <Fragment key={label}>
              {index > 0 ? <StepperConnector /> : null}
              <StepperGroup>
                <StepperItem
                  variant={
                    index < step
                      ? "terminal"
                      : index === step
                        ? "current"
                        : "default"
                  }
                >
                  {label}
                </StepperItem>
              </StepperGroup>
            </Fragment>
          ),
        )}
      </Stepper>

      {error ? (
        <Alert variant="destructive" className="mb-4">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {step === 0 ? (
        <SectionCard
          title="Zone details"
          description="The hostname you want protected and where its traffic should go."
        >
          <div className="grid gap-4">
            <div>
              <Label htmlFor="hostname">Hostname</Label>
              <Input
                id="hostname"
                placeholder="shop.example.com"
                value={hostname}
                onChange={(e) => setHostname(e.target.value)}
              />
            </div>
            <div>
              <Label htmlFor="origin">Origin address</Label>
              <Input
                id="origin"
                placeholder="203.0.113.10 or origin.example.com"
                value={originAddress}
                onChange={(e) => setOriginAddress(e.target.value)}
              />
            </div>
            <div>
              <Label htmlFor="mode">Protection mode</Label>
              <Select
                id="mode"
                value={protectionMode}
                onChange={(value) => setProtectionMode(String(value))}
                options={[
                  {
                    value: "monitor",
                    label: "Monitor — log only, block nothing",
                  },
                  {
                    value: "enforce",
                    label: "Enforce — act on matching rules",
                  },
                ]}
              />
              <p className="mt-1 text-xs text-muted-foreground">
                Start in monitor mode if you want to watch traffic before
                anything is blocked.
              </p>
            </div>
            <div>
              <Button
                onClick={createZone}
                disabled={busy || !hostname.trim() || !originAddress.trim()}
              >
                Create zone
              </Button>
            </div>
          </div>
        </SectionCard>
      ) : null}

      {step === 1 && zone ? (
        <div className="grid gap-4">
          <SectionCard
            title="Prove you own this hostname"
            description="Add this TXT record at your DNS provider, then run the check."
          >
            <div className="rounded-md bg-muted/50 p-3 font-mono text-xs">
              <div>
                <span className="text-muted-foreground">name: </span>
                _waf-verify.{zone.data.hostname}
              </div>
              <div>
                <span className="text-muted-foreground">type: </span>TXT
              </div>
              <div className="break-all">
                <span className="text-muted-foreground">value: </span>
                {zone.data.verificationToken}
              </div>
            </div>
            <div className="mt-3 flex items-center gap-3">
              <Button onClick={runVerify} disabled={busy}>
                Check DNS
              </Button>
              {dnsResult ? (
                <span className="text-sm">
                  ownership{" "}
                  <StatusBadge
                    value={
                      dnsResult.ownershipVerified ? "verified" : "unverified"
                    }
                  />{" "}
                  · routing{" "}
                  <StatusBadge
                    value={dnsResult.dnsRouted ? "routed" : "notRouted"}
                  />
                </span>
              ) : null}
            </div>
            {dnsResult && !dnsResult.dnsRouted ? (
              <p className="mt-2 text-xs text-muted-foreground">
                Routing is informational — point{" "}
                <code>{zone.data.hostname}</code> at{" "}
                <code>{dnsResult.target}</code> when you are ready to send real
                traffic through the edge. It does not block activation.
              </p>
            ) : null}
          </SectionCard>

          <SectionCard
            title="Check the origin"
            description="Optional, but catches a typo before any traffic depends on it."
          >
            <div className="flex items-center gap-3">
              <Button variant="outline" onClick={runOriginTest} disabled={busy}>
                Test origin
              </Button>
              {originResult ? (
                <span className="text-sm">
                  {originResult.reachable
                    ? `reachable — HTTP ${originResult.status} in ${originResult.latencyMs}ms`
                    : `unreachable — ${originResult.error ?? "no response"}`}
                </span>
              ) : null}
            </div>
          </SectionCard>

          <div>
            <Button onClick={() => setStep(2)} disabled={!verified}>
              Continue
            </Button>
            {!verified ? (
              <span className="ml-3 text-xs text-muted-foreground">
                A zone cannot be activated until its hostname is verified.
              </span>
            ) : null}
          </div>
        </div>
      ) : null}

      {step === 2 && zone ? (
        <SectionCard
          title="Baseline protection"
          description="A zone needs at least one policy or rule before it can go live."
        >
          <p className="text-sm text-muted-foreground">
            This adds a DDoS L7 policy with medium sensitivity that challenges
            traffic above 500 requests per minute. You can tune it, or add
            firewall rules, right after activation.
          </p>
          <div className="mt-3 flex gap-2">
            <Button onClick={addBaselineProtection} disabled={busy}>
              Add default DDoS policy
            </Button>
            <Button
              variant="outline"
              onClick={() => navigate(`/zones/${zone.id}`)}
            >
              Configure manually instead
            </Button>
          </div>
        </SectionCard>
      ) : null}

      {step === 3 && zone ? (
        <SectionCard
          title="Activate"
          description="Everything checks out — turn protection on."
        >
          <ul className="mb-4 space-y-1 text-sm">
            <li>
              Hostname <code>{zone.data.hostname}</code> ·{" "}
              <StatusBadge value={zone.data.verificationStatus} />
            </li>
            <li>
              Origin <code>{zone.data.originAddress}</code>
            </li>
            <li>
              Mode <StatusBadge value={zone.data.protectionMode} />
            </li>
          </ul>
          <Button onClick={activate} disabled={busy}>
            Activate zone
          </Button>
        </SectionCard>
      ) : null}
    </div>
  );
}
