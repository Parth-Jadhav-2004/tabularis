import { useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import { useAlert } from "../../hooks/useAlert";
import { ConfirmModal } from "../modals/ConfirmModal";
import { SettingRow, SettingSection } from "./SettingControls";

export function AiStorageSection() {
  const { t } = useTranslation();
  const { showAlert } = useAlert();
  const [action, setAction] = useState<"delete" | "reset" | null>(null);
  const [busy, setBusy] = useState(false);

  const confirm = async () => {
    if (!action || busy) return;
    setBusy(true);
    try {
      if (action === "reset") await invoke("ai_reset_encryption_key", { confirmed: true });
      else await invoke("ai_chat_delete_all");
      showAlert(t(`settings.ai.storage.${action}Success`), { title: t("common.success"), kind: "info" });
      setAction(null);
    } catch {
      showAlert(t("settings.ai.storage.error"), { title: t("common.error"), kind: "error" });
    } finally {
      setBusy(false);
    }
  };

  return (
    <SettingSection title={t("settings.ai.storage.title")} description={t("settings.ai.storage.description")}>
      {(["delete", "reset"] as const).map((item) => (
        <SettingRow key={item} label={t(`settings.ai.storage.${item}`)} description={t(`settings.ai.storage.${item}Description`)}>
          <button type="button" disabled={busy} onClick={() => setAction(item)}
            className="px-3 py-2 rounded-lg border border-accent-error text-accent-error hover:bg-accent-error/10 disabled:opacity-50 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus">
            {t(`settings.ai.storage.${item}`)}
          </button>
        </SettingRow>
      ))}
      <ConfirmModal isOpen={action !== null} onClose={() => { if (!busy) setAction(null); }}
        title={t(`settings.ai.storage.${action ?? "delete"}`)}
        message={t(`settings.ai.storage.${action ?? "delete"}Confirm`)}
        confirmLabel={t("common.confirm")} busy={busy} variant="danger" onConfirm={() => void confirm()} />
    </SettingSection>
  );
}
