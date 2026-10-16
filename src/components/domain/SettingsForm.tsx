/**
 * Formulaire des réglages d'un plugin, rendu depuis son JSON Schema (plan § 3.6). Les
 * erreurs par champ viennent de `plugin_settings_set` ; rien n'est écrit tant qu'il en reste.
 */
import { useState } from "react";
import type { PluginSettingsView } from "@/lib/types";
import { SettingRow } from "@/components/domain/Panels";
import { Button } from "@/components/ui/Button";
import { Input, Select, Textarea } from "@/components/ui/Field";
import { Spinner } from "@/components/ui/Spinner";
import { Switch } from "@/components/ui/Switch";
import { parseSettingInput, schemaFields, type SettingField } from "@/lib/domain/plugins";

export interface SettingsFormProps {
  settings: PluginSettingsView;
  saving?: boolean;
  /** Erreurs du dernier enregistrement (par clé ; `""` pour l'ensemble). */
  errors?: PluginSettingsView["errors"];
  onSave: (value: Record<string, unknown>) => void;
}

export function SettingsForm({ settings, saving, errors = [], onSave }: SettingsFormProps) {
  const fields = schemaFields(settings.schema);
  const [draft, setDraft] = useState<Record<string, unknown>>(settings.value);
  const errorOf = (key: string) => errors.find((e) => e.field === key)?.message;
  const general = errors.filter((e) => !fields.some((f) => f.key === e.field));

  if (fields.length === 0) {
    return <p className="text-body-sm text-tx3">Ce plugin ne déclare aucun réglage.</p>;
  }

  const set = (field: SettingField, raw: string | boolean) =>
    setDraft((d) => {
      const next = { ...d };
      const value = parseSettingInput(field, raw);
      if (value === undefined) delete next[field.key];
      else next[field.key] = value;
      return next;
    });

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        onSave(draft);
      }}
    >
      {fields.map((f) => (
        <SettingRow
          key={f.key}
          title={f.required ? `${f.title} *` : f.title}
          description={
            <>
              {f.description}
              {errorOf(f.key) && <span className="block text-red">{errorOf(f.key)}</span>}
            </>
          }
          control={
            <FieldControl
              field={f}
              value={draft[f.key]}
              onChange={(v) => set(f, v)}
              invalid={!!errorOf(f.key)}
            />
          }
        />
      ))}
      {general.map((e) => (
        <p key={e.field + e.message} className="mt-2 text-caption text-red">
          {e.message}
        </p>
      ))}
      <div className="mt-3.5 flex justify-end">
        <Button type="submit" variant="primary" size="sm" disabled={saving}>
          {saving && <Spinner />}
          Enregistrer
        </Button>
      </div>
    </form>
  );
}

function FieldControl({
  field,
  value,
  onChange,
  invalid,
}: {
  field: SettingField;
  value: unknown;
  onChange: (raw: string | boolean) => void;
  invalid: boolean;
}) {
  const label = field.title;
  switch (field.kind) {
    case "boolean":
      return <Switch checked={value === true} onCheckedChange={onChange} aria-label={label} />;
    case "enum":
      return (
        <Select
          size="sm"
          aria-label={label}
          value={String(value ?? "")}
          invalid={invalid}
          onChange={(e) => onChange(e.target.value)}
          className="w-[160px]"
        >
          {!field.required && <option value="">—</option>}
          {field.options.map((o) => (
            <option key={o} value={o}>
              {o}
            </option>
          ))}
        </Select>
      );
    case "multiline":
      return (
        <Textarea
          aria-label={label}
          rows={3}
          value={String(value ?? "")}
          invalid={invalid}
          onChange={(e) => onChange(e.target.value)}
          className="w-[260px]"
        />
      );
    case "integer":
    case "number":
      return (
        <Input
          size="sm"
          mono
          inputMode={field.kind === "integer" ? "numeric" : "decimal"}
          aria-label={label}
          value={value === undefined || value === null ? "" : String(value)}
          invalid={invalid}
          onChange={(e) => onChange(e.target.value)}
          className="w-[90px]"
        />
      );
    default:
      return (
        <Input
          size="sm"
          aria-label={label}
          value={String(value ?? "")}
          invalid={invalid}
          onChange={(e) => onChange(e.target.value)}
          className="w-[200px]"
        />
      );
  }
}
