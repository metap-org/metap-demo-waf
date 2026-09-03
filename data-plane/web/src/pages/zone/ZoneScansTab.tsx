/**
 * Module 5 — vulnerability scanning, zone view. The portal configures scan jobs and shows
 * findings; it does not run the scan (`docs/13-screen-api-map.md`'s 2026-08-30 correction — the
 * DAST engine is a separate execution concern, the same way `edge-plane` is).
 *
 * "Run now" therefore hands the job to whatever scanner backend is configured and reports back
 * honestly when none is: the job queues, and the response says nothing will pick it up yet.
 */
import { useState } from "react";
import { Link } from "react-router-dom";
import {
  Button,
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  Input,
  Label,
  Select,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
  toast,
} from "@metap/ui";
import { ENTITIES, createRecord, runScanJob, useInvalidateWaf, useRecords } from "../../api/waf";
import { EmptyState, SectionCard, StatusBadge, shortDate } from "../../components/primitives";

type ScanJobData = { zoneId?: string; scanType?: string; schedule?: string; status?: string; lastRunAt?: string };
type FindingData = { scanJobId?: string; severity?: string; category?: string; endpoint?: string; remediationStatus?: string };

export function ZoneScansTab({ zoneId }: { zoneId: string }) {
  const invalidate = useInvalidateWaf();
  const jobs = useRecords<ScanJobData>(ENTITIES.scanJobs, { zoneId }, 50);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [draft, setDraft] = useState<ScanJobData>({ scanType: "passive", schedule: "0 3 * * *" });

  // Findings hang off a scan job, not a zone (`docs/02-domain-model.md`: "một finding luôn thuộc
  // đúng một lần chạy scan"), so the zone's findings are the union of its jobs' findings — hence
  // the id list rather than a `zoneId` filter.
  const jobIds = new Set((jobs.data ?? []).map((job) => job.id));
  const findings = useRecords<FindingData>(ENTITIES.scanFindings, {}, 100, jobIds.size > 0);
  const zoneFindings = (findings.data ?? []).filter((finding) => jobIds.has(String(finding.data.scanJobId ?? "")));

  async function createJob() {
    setBusy(true);
    try {
      await createRecord(ENTITIES.scanJobs, { ...draft, zoneId });
      invalidate();
      setOpen(false);
      toast({ title: "Scan job created", variant: "success" });
    } catch (e) {
      toast({ title: e instanceof Error ? e.message : String(e), variant: "destructive" });
    } finally {
      setBusy(false);
    }
  }

  async function run(jobId: string) {
    setBusy(true);
    try {
      const result = await runScanJob(jobId);
      invalidate();
      toast({ title: result.data.detail, variant: result.data.dispatched ? "success" : "default" });
    } catch (e) {
      toast({ title: e instanceof Error ? e.message : String(e), variant: "destructive" });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mt-4 grid gap-4">
      <SectionCard
        title="Scan jobs"
        description="Scheduled or on-demand vulnerability scans for this zone."
        actions={
          <Button size="sm" onClick={() => setOpen(true)}>
            New scan job
          </Button>
        }
      >
        {(jobs.data ?? []).length === 0 ? (
          <EmptyState title="No scan jobs" description="Create one to start checking this zone for vulnerabilities." />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Type</TableHead>
                <TableHead>Schedule</TableHead>
                <TableHead>Status</TableHead>
                <TableHead>Last run</TableHead>
                <TableHead className="text-right">Actions</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {(jobs.data ?? []).map((job) => (
                <TableRow key={job.id}>
                  <TableCell className="font-medium">{job.data.scanType}</TableCell>
                  <TableCell className="font-mono text-xs">{job.data.schedule || "manual"}</TableCell>
                  <TableCell>
                    <StatusBadge value={job.data.status ?? job.status} />
                  </TableCell>
                  <TableCell className="text-muted-foreground">{shortDate(job.data.lastRunAt)}</TableCell>
                  <TableCell className="text-right">
                    <Button size="sm" variant="outline" onClick={() => run(job.id)} disabled={busy}>
                      Run now
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </SectionCard>

      <SectionCard
        title="Findings"
        description="Everything this zone's scans have turned up."
        actions={
          <Button size="sm" variant="outline" asChild>
            <Link to="/findings">All findings</Link>
          </Button>
        }
      >
        {zoneFindings.length === 0 ? (
          <EmptyState title="No findings" description="Either nothing has been scanned yet, or nothing was found." />
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Severity</TableHead>
                <TableHead>Category</TableHead>
                <TableHead>Endpoint</TableHead>
                <TableHead>Remediation</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {zoneFindings.map((finding) => (
                <TableRow key={finding.id}>
                  <TableCell>
                    <StatusBadge value={finding.data.severity} />
                  </TableCell>
                  <TableCell>{finding.data.category}</TableCell>
                  <TableCell className="font-mono text-xs">{finding.data.endpoint}</TableCell>
                  <TableCell>
                    <StatusBadge value={finding.data.remediationStatus ?? finding.status} />
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
            <DialogTitle>New scan job</DialogTitle>
          </DialogHeader>
          <div className="grid gap-3">
            <div>
              <Label htmlFor="scan-type">Scan type</Label>
              <Select
                id="scan-type"
                value={draft.scanType}
                onChange={(value) => setDraft({ ...draft, scanType: String(value) })}
                options={[
                  { value: "passive", label: "Passive — headers, TLS, exposed files" },
                  { value: "active", label: "Active — probes endpoints" },
                ]}
              />
            </div>
            <div>
              <Label htmlFor="scan-schedule">Schedule (cron)</Label>
              <Input
                id="scan-schedule"
                className="font-mono"
                value={draft.schedule ?? ""}
                onChange={(e) => setDraft({ ...draft, schedule: e.target.value })}
              />
              <p className="mt-1 text-xs text-muted-foreground">
                Leave empty for a manual-only job. Cron expressions are evaluated by `metap-cron`.
              </p>
            </div>
          </div>
          <DialogFooter>
            <Button variant="outline" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button onClick={createJob} disabled={busy}>
              Create
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
