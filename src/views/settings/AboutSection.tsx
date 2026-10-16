/**
 * Réglages › À propos : version de l'app et mise à jour automatique.
 *
 * La vérification au lancement est silencieuse (`useUpdateCheckOnLaunch`) : son résultat, ou
 * son erreur, n'apparaît qu'ici. « Rechercher une mise à jour » la relance ; l'installation
 * télécharge l'installeur signé, vérifie sa signature côté Rust, puis propose de redémarrer.
 */
import { Markdown } from "@/components/domain/Markdown";
import { Note } from "@/components/domain/Panels";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Callout } from "@/components/ui/Callout";
import { Card } from "@/components/ui/Card";
import { ProgressBar } from "@/components/ui/ProgressBar";
import { Spinner } from "@/components/ui/Spinner";
import { StatusDot } from "@/components/ui/StatusDot";
import { formatDay } from "@/lib/domain/format";
import { isMockIpc } from "@/lib/ipc/transport";
import { useAppUpdate, useAppVersion, useInstallUpdate, useRestartApp } from "@/lib/query/queries";
import type { AvailableUpdate, UpdateProgress } from "@/lib/updater";

const isWindows = typeof navigator !== "undefined" && /Windows/.test(navigator.userAgent);
const time = new Intl.DateTimeFormat("fr-FR", { hour: "2-digit", minute: "2-digit" });
const megabytes = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 1 });
const mb = (bytes: number) => `${megabytes.format(bytes / 1_000_000)} Mo`;

export function AboutSection() {
  const version = useAppVersion().data;
  const update = useAppUpdate();
  const checked = update.dataUpdatedAt > 0 || update.errorUpdatedAt > 0;

  return (
    <>
      <h2 className="font-display text-title font-semibold">À propos</h2>
      <p className="mt-[7px] text-body-sm leading-[1.6] text-tx2">
        Cadence se met à jour depuis ses releases GitHub. Chaque installeur est signé : sa signature
        est vérifiée sur cette machine avant l'installation.
      </p>

      <Card className="mt-[18px] px-4 py-3.5">
        <div className="flex items-center gap-3">
          <div className="min-w-0 grow">
            <div className="flex items-center gap-2 text-body font-semibold">
              Cadence
              {version && (
                <Badge tone="muted" mono>
                  {version}
                </Badge>
              )}
            </div>
            <div className="mt-0.5 text-caption text-tx3">Tauri 2 · React 19</div>
          </div>
          <Button
            size="sm"
            icon={update.isFetching ? undefined : "refresh"}
            disabled={update.isFetching}
            onClick={() => void update.refetch()}
          >
            {update.isFetching && <Spinner size={12} />}
            Rechercher une mise à jour
          </Button>
        </div>

        <div className="mt-3.5 flex items-center gap-2.5 border-t border-line pt-3 text-caption">
          {update.isFetching ? (
            <span className="text-tx2">Recherche d'une mise à jour…</span>
          ) : update.isError ? (
            <>
              <StatusDot tone="amber" size={8} />
              <span className="min-w-0 grow text-tx2">{update.error.message}</span>
            </>
          ) : !checked ? (
            <>
              <StatusDot tone="neutral" size={8} />
              <span className="text-tx3">Pas encore vérifié depuis le lancement.</span>
            </>
          ) : update.data ? (
            <>
              <StatusDot tone="clay" size={8} />
              <span className="text-tx2">Version {update.data.version} disponible.</span>
            </>
          ) : (
            <>
              <StatusDot tone="green" size={8} />
              <span className="text-tx2">
                Cadence est à jour · vérifié à {time.format(update.dataUpdatedAt)}
              </span>
            </>
          )}
        </div>
      </Card>

      {update.data && !update.isFetching && <UpdateCard update={update.data} />}

      {isMockIpc && (
        <Note className="mt-4">
          Mode démo : la vérification est simulée. Ajoutez{" "}
          <span className="mono">?update=available</span> ou{" "}
          <span className="mono">?update=fail</span> à l'adresse pour voir les autres cas.
        </Note>
      )}
    </>
  );
}

/** Pied de la navigation des réglages : « Cadence 1.0.0 », et l'annonce d'une mise à jour. */
export function AppVersionLine({ onOpen }: { onOpen: () => void }) {
  const version = useAppVersion().data;
  const update = useAppUpdate().data;
  return (
    <>
      Cadence{version && <span className="mono"> {version}</span>}
      {update && (
        <>
          <br />
          <button type="button" onClick={onOpen} className="text-clay hover:underline">
            Version {update.version} disponible
          </button>
        </>
      )}
    </>
  );
}

function UpdateCard({ update }: { update: AvailableUpdate }) {
  const install = useInstallUpdate();
  const restart = useRestartApp();

  return (
    <Card className="mt-3 px-4 py-3.5">
      <div className="flex items-center gap-2.5">
        <div className="min-w-0 grow">
          <div className="text-body font-semibold">Cadence {update.version}</div>
          <div className="mt-0.5 text-caption text-tx3">
            Vous utilisez la version {update.currentVersion}
            {update.date && ` · publiée le ${formatDay(update.date)}`}
          </div>
        </div>
        {install.isSuccess ? (
          <Button
            variant="primary"
            size="sm"
            icon={restart.isPending ? undefined : "refresh"}
            disabled={restart.isPending}
            onClick={() => restart.mutate()}
          >
            {restart.isPending && <Spinner size={12} />}
            Redémarrer maintenant
          </Button>
        ) : (
          <Button
            variant="primary"
            size="sm"
            icon={install.isPending ? undefined : "download"}
            disabled={install.isPending}
            onClick={() => install.mutate()}
          >
            {install.isPending && <Spinner size={12} />}
            Télécharger et installer
          </Button>
        )}
      </div>

      {update.notes && <Markdown source={update.notes} className="mt-3 text-body-sm" />}

      {install.isPending && <DownloadProgress progress={install.progress} />}
      {install.isSuccess && (
        <Callout tone="success" className="mt-3">
          Mise à jour installée : elle prendra effet au redémarrage.
        </Callout>
      )}
      {install.isError && (
        <Callout tone="danger" className="mt-3">
          {install.error.message}
        </Callout>
      )}
      {restart.isError && (
        <Callout tone="danger" className="mt-3">
          Redémarrage impossible : {restart.error.message}. Quittez et relancez Cadence.
        </Callout>
      )}
      {!install.isSuccess && isWindows && (
        <div className="mt-3 text-caption text-tx3">
          Sous Windows, l'installeur ferme Cadence puis la relance sur la nouvelle version.
        </div>
      )}
    </Card>
  );
}

function DownloadProgress({ progress }: { progress: UpdateProgress | null }) {
  const pct = progress?.total ? (progress.downloaded / progress.total) * 100 : 0;
  return (
    <div className="mt-3 flex flex-col gap-1.5">
      <ProgressBar value={pct} label="Téléchargement de la mise à jour" />
      <span className="mono text-caption text-tx3">
        {!progress
          ? "Connexion…"
          : progress.total
            ? `${mb(progress.downloaded)} sur ${mb(progress.total)}`
            : mb(progress.downloaded)}
      </span>
    </div>
  );
}
