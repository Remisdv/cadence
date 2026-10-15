import type { IssueType } from "@/lib/types";
import { Badge, type BadgeTone } from "@/components/ui/Badge";
import { issueTypeTone } from "@/lib/domain/issues";
import { versionStageMeta, type VersionStage } from "@/lib/domain/versions";

export interface VersionBadgeProps {
  version: string;
  /** Couleur selon le statut (livrée vert, en cours clay, palier bleu, vision violet…). */
  stage?: VersionStage;
  tone?: BadgeTone;
  /** Affiche le préfixe de tag : v0.1.2. */
  tag?: boolean;
}

export function VersionBadge({ version, stage, tone, tag }: VersionBadgeProps) {
  const t = tone ?? (stage ? versionStageMeta[stage].tone : "clay");
  return (
    <Badge tone={t} mono>
      {tag ? `v${version}` : version}
    </Badge>
  );
}

export interface TypeBadgeProps {
  type: IssueType;
  /** `type/feat` (tableau de sprint) ou `feat` (listes). */
  prefixed?: boolean;
}

export function TypeBadge({ type, prefixed }: TypeBadgeProps) {
  return <Badge tone={issueTypeTone[type]}>{prefixed ? `type/${type}` : type}</Badge>;
}
