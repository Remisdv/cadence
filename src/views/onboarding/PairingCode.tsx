/** Code d'appairage du device flow, une case par caractère : « WDJB-MJHT ». */
export function PairingCode({ code }: { code: string | null }) {
  // Pendant la demande du code : cases vides de la même taille.
  const chars = code ? [...code] : [..."····-····"];
  return (
    <div
      className="flex gap-2 mono"
      aria-label={code ? `Code ${code}` : "Code en cours de génération"}
    >
      {chars.map((char, i) =>
        char === "-" ? (
          <span key={i} aria-hidden className="flex items-center text-[22px] text-tx3">
            -
          </span>
        ) : (
          <span
            key={i}
            aria-hidden
            className="flex h-12 w-[38px] items-center justify-center rounded-lg border border-line2 bg-p3 text-[22px] font-medium"
          >
            {code ? char : <span className="text-tx3">·</span>}
          </span>
        ),
      )}
    </div>
  );
}
