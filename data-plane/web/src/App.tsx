import type { ReactNode } from "react";
import {
  Navigate,
  Route,
  Routes,
  useNavigate,
  useParams,
} from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  AppShellLayout,
  AuthProvider,
  Can,
  LocaleProvider,
  useAuth,
  RecordDetail,
  GeneratedForm,
  GeneratedList,
  UsersAdminPage,
  PoliciesAdminPage,
  CronJobsAdminPage,
  LowCodeEntitiesAdminPage,
  OidcCallbackPage,
} from "@metap/platform-ui";
import type { ShellNavItem } from "@metap/platform-ui";
import { LoginPage } from "./demo/LoginPage";
import { EntitiesPage } from "./demo/EntitiesPage";

function RequireAuth({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  // `useAuth()` dropped `token` for `status` in the cookie-session migration
  // (`docs/roadmap/64-cookie-session-persistence.md` in `../../../metap-docs`) — `status` starts
  // "unknown" until the initial `GET /auth/me` resolves, so redirecting on anything other than
  // a confirmed "anonymous" would bounce a just-logged-in user straight back to `/login` before
  // that check ever gets a chance to see the new session cookie.
  const { status } = useAuth();

  if (status === "unknown") {
    return null;
  }
  if (status === "anonymous") {
    return <Navigate to="/login" replace />;
  }

  // WAF entity nav — mirrors `docs/07-portal-features.md`'s sitemap groupings at the
  // entity level (this is `GeneratedList`/`GeneratedForm` driving the generic metadata-based
  // CRUD, not the full zone-centric IA from that doc — that's a later custom-UI pass, see
  // `docs/13-screen-api-map.md` for what's generic vs. what needs bespoke screens).
  const navItems: ShellNavItem[] = [
    { to: "/", label: "Overview" },
    { to: "/records/waf.zones", label: "Zones" },
    { to: "/records/waf.ddos_policies", label: "DDoS Policies" },
    { to: "/records/waf.firewall_rules", label: "Firewall Rules" },
    { to: "/records/waf.scan_jobs", label: "Scan Jobs" },
    { to: "/records/waf.scan_findings", label: "Scan Findings" },
    { to: "/records/waf.security_events", label: "Security Events" },
    { to: "/records/waf.incidents", label: "Incidents" },
    { to: "/records/waf.alert_policies", label: "Alert Policies" },
    { to: "/records/waf.alert_notifications", label: "Alert Notifications" },
    { to: "/admin/users", label: t("shell.navUsers"), roles: ["admin"] },
    { to: "/admin/policies", label: t("shell.navPolicies"), roles: ["admin"] },
    { to: "/admin/cron-jobs", label: t("shell.navCronJobs"), roles: ["admin"] },
  ];

  return (
    <AppShellLayout brand="WAF Portal" navItems={navItems}>
      {children}
    </AppShellLayout>
  );
}

function RequireAdmin({ children }: { children: ReactNode }) {
  return (
    <Can roles={["admin"]} fallback={<Navigate to="/" replace />}>
      {children}
    </Can>
  );
}

function RecordsRoute() {
  const { entityName } = useParams<{ entityName: string }>();
  if (!entityName) return <div>Missing entity name</div>;
  return <GeneratedList entityName={entityName} />;
}

function NewRecordRoute() {
  const { entityName } = useParams<{ entityName: string }>();
  const navigate = useNavigate();
  if (!entityName) return <div>Missing entity name</div>;
  return (
    <GeneratedForm
      entityName={entityName}
      onSaved={() => navigate(`/records/${entityName}`)}
    />
  );
}

function RecordDetailRoute() {
  const { entityName, id } = useParams<{ entityName: string; id: string }>();
  if (!entityName || !id) return <div>Missing entity or id</div>;
  return <RecordDetail entityName={entityName} id={id} />;
}

function EditRecordRoute() {
  const { entityName, id } = useParams<{ entityName: string; id: string }>();
  const navigate = useNavigate();
  if (!entityName || !id) return <div>Missing entity or id</div>;
  return (
    <GeneratedForm
      entityName={entityName}
      recordId={id}
      onSaved={() => navigate(`/records/${entityName}/${id}`)}
    />
  );
}

export default function App() {
  return (
    <AuthProvider>
      <LocaleProvider>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route path="/auth/oidc/callback" element={<OidcCallbackPage />} />
          <Route
            path="/"
            element={
              <RequireAuth>
                <EntitiesPage />
              </RequireAuth>
            }
          />
          <Route
            path="/records/:entityName"
            element={
              <RequireAuth>
                <RecordsRoute />
              </RequireAuth>
            }
          />
          <Route
            path="/records/:entityName/new"
            element={
              <RequireAuth>
                <NewRecordRoute />
              </RequireAuth>
            }
          />
          <Route
            path="/records/:entityName/:id"
            element={
              <RequireAuth>
                <RecordDetailRoute />
              </RequireAuth>
            }
          />
          <Route
            path="/records/:entityName/:id/edit"
            element={
              <RequireAuth>
                <EditRecordRoute />
              </RequireAuth>
            }
          />
          <Route
            path="/admin/users"
            element={
              <RequireAuth>
                <RequireAdmin>
                  <UsersAdminPage />
                </RequireAdmin>
              </RequireAuth>
            }
          />
          <Route
            path="/admin/policies"
            element={
              <RequireAuth>
                <RequireAdmin>
                  <PoliciesAdminPage />
                </RequireAdmin>
              </RequireAuth>
            }
          />
          <Route
            path="/admin/cron-jobs"
            element={
              <RequireAuth>
                <RequireAdmin>
                  <CronJobsAdminPage />
                </RequireAdmin>
              </RequireAuth>
            }
          />
          <Route
            path="/admin/lowcode"
            element={
              <RequireAuth>
                <RequireAdmin>
                  <LowCodeEntitiesAdminPage />
                </RequireAdmin>
              </RequireAuth>
            }
          />
        </Routes>
      </LocaleProvider>
    </AuthProvider>
  );
}
