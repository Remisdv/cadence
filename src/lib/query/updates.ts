/**
 * Hooks de la version de l'app et de la mise à jour automatique (réexportés par `queries.ts`).
 * Pas de commande Cadence ici : ce sont les plugins Tauri, enveloppés par `lib/updater`.
 */
import { useEffect, useState } from "react";
import { queryOptions, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { isMockIpc } from "@/lib/ipc/transport";
import {
  appVersion,
  checkForUpdate,
  installUpdate,
  restartApp,
  type UpdateProgress,
} from "@/lib/updater";
import { keys } from "./keys";

/** Délai avant la vérification au lancement : laisse le démarrage et la première sync passer. */
const LAUNCH_CHECK_DELAY_MS = 4_000;

const updateCheck = queryOptions({
  queryKey: keys.appUpdate(),
  queryFn: checkForUpdate,
  retry: false,
});

export const useAppVersion = () => useQuery({ queryKey: keys.appVersion(), queryFn: appVersion });

/**
 * Résultat de la dernière vérification (`data` : la mise à jour, `null` si l'app est à jour).
 * Ne se déclenche pas tout seul : la vérification au lancement remplit le cache, `refetch()`
 * relance une recherche à la demande.
 */
export const useAppUpdate = () => useQuery({ ...updateCheck, enabled: false });

/**
 * Vérification silencieuse au lancement : une erreur (hors ligne, pas encore de release) reste
 * dans le cache et n'est montrée que dans Réglages › À propos. Désactivée dans `tauri dev`.
 */
export function useUpdateCheckOnLaunch() {
  const qc = useQueryClient();
  useEffect(() => {
    if (import.meta.env.DEV && !isMockIpc) return;
    const timer = setTimeout(() => void qc.prefetchQuery(updateCheck), LAUNCH_CHECK_DELAY_MS);
    return () => clearTimeout(timer);
  }, [qc]);
}

/** Télécharge et installe la mise à jour trouvée ; `progress` suit le téléchargement. */
export function useInstallUpdate() {
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const mutation = useMutation({
    mutationFn: () => {
      setProgress(null);
      return installUpdate(setProgress);
    },
  });
  return { ...mutation, progress };
}

export const useRestartApp = () => useMutation({ mutationFn: restartApp });
