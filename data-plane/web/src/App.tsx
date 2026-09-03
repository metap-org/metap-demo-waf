/**
 * WAF Customer Portal — the real product IA, replacing the generic entity harness this app used to
 * be (a nav item per `waf.*` entity, `GeneratedList` behind each one).
 *
 * The organising principle is `docs/07-portal-features.md`'s sitemap: a customer thinks in zones
 * and in work (incidents, findings, alerts), not in tables. So the nav is Dashboard / Zones /
 * Incidents / Findings / Alerting / Analytics, everything about one zone lives behind that zone,
 * and the generic per-entity CRUD screens stay reachable under `/records/*` as an admin/debug
 * escape hatch rather than as the product.
 */
import type { ReactNode } from "react";
import { Navigate, Route, Routes, useNavigate, useParams } from "react-router-dom";
import { useTranslation } from "react-i18next";
import {
  AppShellLayout,
  AuthProvider,
  Can,
  CronJobsAdminPage,
  GeneratedForm,
  GeneratedList,
  LocaleProvider,
  LowCodeEntitiesAdminPage,
  OidcCallbackPage,
  PoliciesAdminPage,
  RecordDetail,
  UsersAdminPage,
  useAuth,
} from "@metap/platform-ui";
import type { ShellNavItem } from "@metap/platform-ui";
import { LoginPage } from "./demo/LoginPage";
import { EntitiesPage } from "./demo/EntitiesPage";
import { DashboardPage } from "./pages/DashboardPage";
import { OnboardingPage } from "./pages/OnboardingPage";
import { ZonesPage } from "./pages/ZonesPage";
import { ZoneDetailPage } from "./pages/ZoneDetailPage";
import { IncidentsPage } from "./pages/IncidentsPage";
import { IncidentDetailPage } from "./pages/IncidentDetailPage";
import { FindingsPage } from "./pages/FindingsPage";
import { AlertingPage } from "./pages/AlertingPage";
import { AnalyticsPage } from "./pages/AnalyticsPage";
import { SettingsPage } from "./pages/SettingsPage";

function RequireAuth({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  // `useAuth()` dropped `token` for `status` in the cookie-session migration
  // (`docs/roadmap/64-cookie-session-persistence.md` in `../../../metap-docs`) — `status` starts
  // "unknown" until the initial `GET /auth/me` resolves, so redirecting on anything other than a
  // confirmed "anonymous" would bounce a just-logged-in user straight back to `/login`.
  const { status } = useAuth();

  if (status === "unknown") {
    return null;
  }
  if (status === "anonymous") {
    return <Navigate to="/login" replace />;
  }

  const navItems: ShellNavItem[] = [
    { to: "/", label: "Dashboard" },
    { to: "/zones", label: "Zones" },
    { to: "/incidents", label: "Incidents" },
    { to: "/findings", label: "Findings" },
    { to: "/alerting", label: "Alerting" },
    { to: "/analytics", label: "Analytics" },
    { to: "/settings", label: "Settings", roles: ["admin"] },
    { to: "/admin/users", label: t("shell.navUsers"), roles: ["admin"] },
    { to: "/admin/policies", label: t("shell.navPolicies"), roles: ["admin"] },
    { to: "/admin/cron-jobs", label: t("shell.navCronJobs"), roles: ["admin"] },
    // The old entity-per-nav-item harness, kept behind one link: still the fastest way to inspect
    // raw records when a product screen doesn't show the field you need.
    { to: "/records", label: "Raw records", roles: ["admin"] },
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
  return <GeneratedForm entityName={entityName} onSaved={() => navigate(`/records/${entityName}`)} />;
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
  return <GeneratedForm entityName={entityName} recordId={id} onSaved={() => navigate(`/records/${entityName}/${id}`)} />;
}

/** Wraps a product route in the shell + auth gate — every one of them needs both, and repeating
 *  the pair 12 times in the route table buries what each route actually is. */
function page(element: ReactNode) {
  return <RequireAuth>{element}</RequireAuth>;
}

function adminPage(element: ReactNode) {
  return (
    <RequireAuth>
      <RequireAdmin>{element}</RequireAdmin>
    </RequireAuth>
  );
}

export default function App() {
  return (
    <AuthProvider>
      <LocaleProvider>
        <Routes>
          <Route path="/login" element={<LoginPage />} />
          <Route path="/auth/oidc/callback" element={<OidcCallbackPage />} />

          <Route path="/" element={page(<DashboardPage />)} />
          <Route path="/onboarding" element={page(<OnboardingPage />)} />
          <Route path="/zones" element={page(<ZonesPage />)} />
          <Route path="/zones/:zoneId" element={page(<ZoneDetailPage />)} />
          <Route path="/incidents" element={page(<IncidentsPage />)} />
          <Route path="/incidents/:incidentId" element={page(<IncidentDetailPage />)} />
          <Route path="/findings" element={page(<FindingsPage />)} />
          <Route path="/alerting" element={page(<AlertingPage />)} />
          <Route path="/analytics" element={page(<AnalyticsPage />)} />
          <Route path="/settings" element={adminPage(<SettingsPage />)} />

          {/* Generic CRUD escape hatch — the whole of the previous app, now one section. */}
          <Route path="/records" element={adminPage(<EntitiesPage />)} />
          <Route path="/records/:entityName" element={page(<RecordsRoute />)} />
          <Route path="/records/:entityName/new" element={page(<NewRecordRoute />)} />
          <Route path="/records/:entityName/:id" element={page(<RecordDetailRoute />)} />
          <Route path="/records/:entityName/:id/edit" element={page(<EditRecordRoute />)} />

          <Route path="/admin/users" element={adminPage(<UsersAdminPage />)} />
          <Route path="/admin/policies" element={adminPage(<PoliciesAdminPage />)} />
          <Route path="/admin/cron-jobs" element={adminPage(<CronJobsAdminPage />)} />
          <Route path="/admin/lowcode" element={adminPage(<LowCodeEntitiesAdminPage />)} />
        </Routes>
      </LocaleProvider>
    </AuthProvider>
  );
}
