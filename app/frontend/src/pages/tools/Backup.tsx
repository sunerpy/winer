// Snapshots of the game's settings: the general settings and the key bindings, kept by winer and
// put back from the lobby or the home screen.
import type { BackupChannel, BackupInfo } from "@winer/shared";
import { Archive, FolderOpen, RotateCcw, Trash2, Upload } from "lucide-react";
import { type ChangeEvent, useEffect, useRef, useState } from "react";

import { errorCode, errorMessage } from "../../lib/backend";
import { type MessageKey, useT } from "../../lib/i18n";
import { useLive, useStore } from "../../lib/store";
import { useAsync } from "../../lib/useAsync";
import {
  Badge,
  Button,
  EmptyState,
  ErrorNote,
  IconButton,
  Panel,
  Popover,
  Skeleton,
  toast,
} from "../../ui";
import { type RestoreChoice, backupSize, backupTime, restoreChannels } from "./profile";

const KEEP = 10;
const CHANNEL_LABEL: Record<BackupChannel, MessageKey> = {
  general: "profile.backup.general",
  hotkeys: "profile.backup.hotkeys",
};
const RESTORED: Record<RestoreChoice, MessageKey> = {
  general: "profile.backup.restoredGeneral",
  hotkeys: "profile.backup.restoredHotkeys",
  all: "profile.backup.restoredAll",
};

/** Restore, and the halves of the settings it can put back. */
function RestoreButton({
  backup,
  disabled,
  title,
  onRestore,
}: {
  backup: BackupInfo;
  disabled: boolean;
  title?: string;
  onRestore: (choice: RestoreChoice) => void;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);
  const choices: RestoreChoice[] = [
    ...backup.channels,
    ...(backup.channels.length > 1 ? (["all"] as const) : []),
  ];
  return (
    <div ref={anchor} title={title}>
      <Button
        size="sm"
        icon={RotateCcw}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((value) => !value)}
      >
        {t("profile.backup.restore")}
      </Button>
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        placement="bottom-end"
        className="w-[160px] p-1"
        label={t("profile.backup.restoreMenu")}
      >
        <div role="menu" aria-label={t("profile.backup.restoreMenu")} className="flex flex-col">
          {choices.map((choice) => (
            <button
              key={choice}
              type="button"
              role="menuitem"
              onClick={() => {
                setOpen(false);
                onRestore(choice);
              }}
              className="flex h-8 items-center rounded-6 px-2.5 text-left text-[12.5px] text-fg hover-wash"
            >
              {choice === "all" ? t("profile.backup.all") : t(CHANNEL_LABEL[choice])}
            </button>
          ))}
        </div>
      </Popover>
    </div>
  );
}

export function BackupPanel() {
  const t = useT();
  const store = useStore();
  const phase = useLive((snapshot) => snapshot.phase);
  /** The game reads its settings as it starts: they go back only from the lobby or the home screen. */
  const idle = phase === "None" || phase === "Lobby";
  const loaded = useAsync(() => store.backend.call("get_game_settings_backups"), []);
  const [list, setList] = useState<BackupInfo[] | null>(null);
  const [busy, setBusy] = useState<"create" | "import" | number | null>(null);
  const [confirming, setConfirming] = useState<number | null>(null);
  const file = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (loaded.data) setList(loaded.data);
  }, [loaded.data]);
  // A delete asks once more, briefly.
  useEffect(() => {
    if (confirming === null) return undefined;
    const timer = setTimeout(() => setConfirming(null), 4000);
    return () => clearTimeout(timer);
  }, [confirming]);

  const add = (backup: BackupInfo) =>
    setList((current) =>
      [backup, ...(current ?? []).filter((other) => other.id !== backup.id)]
        .sort((a, b) => b.id - a.id)
        .slice(0, KEEP),
    );

  const create = async () => {
    setBusy("create");
    try {
      add(await store.backend.call("create_game_settings_backup"));
      toast(t("profile.backup.created"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(null);
    }
  };

  const pick = async (event: ChangeEvent<HTMLInputElement>) => {
    const chosen = event.target.files?.[0];
    // The same file picked again is a new pick.
    event.target.value = "";
    if (!chosen) return;
    setBusy("import");
    try {
      const text = await chosen.text();
      add(await store.backend.call("import_game_settings_backup", { text }));
      toast(t("profile.backup.imported"), "ok");
    } catch (error) {
      toast(t("profile.backup.importFailed", { message: errorMessage(error) }), "danger");
    } finally {
      setBusy(null);
    }
  };

  const restore = async (backup: BackupInfo, choice: RestoreChoice) => {
    setBusy(backup.id);
    try {
      await store.backend.call("restore_game_settings_backup", {
        id: backup.id,
        channels: restoreChannels(choice),
      });
      toast(t(RESTORED[choice]), "ok");
    } catch (error) {
      toast(errorCode(error) === "busy" ? t("profile.backup.busy") : errorMessage(error), "danger");
    } finally {
      setBusy(null);
    }
  };

  const remove = async (backup: BackupInfo) => {
    if (confirming !== backup.id) {
      setConfirming(backup.id);
      return;
    }
    setConfirming(null);
    try {
      await store.backend.call("delete_game_settings_backup", { id: backup.id });
      setList((current) => (current ?? []).filter((other) => other.id !== backup.id));
      toast(t("profile.backup.deleted"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    }
  };

  const reveal = (backup: BackupInfo) =>
    void store.backend
      .call("reveal_game_settings_backup", { id: backup.id })
      .catch((error: unknown) => toast(errorMessage(error), "danger"));

  return (
    <Panel
      eyebrow={t("profile.backup.title")}
      className="col-span-12"
      right={
        <>
          <input
            ref={file}
            type="file"
            accept=".json,application/json"
            aria-label={t("profile.backup.import")}
            className="hidden"
            onChange={(event) => void pick(event)}
          />
          <Button
            size="sm"
            variant="ghost"
            icon={Upload}
            loading={busy === "import"}
            onClick={() => file.current?.click()}
          >
            {t("profile.backup.import")}
          </Button>
          <Button
            size="sm"
            variant="accent"
            icon={Archive}
            loading={busy === "create"}
            onClick={() => void create()}
          >
            {t("profile.backup.create")}
          </Button>
        </>
      }
    >
      <p className="pb-2 text-[12px] leading-4 text-fg-muted">{t("profile.backup.hint")}</p>
      {loaded.error !== undefined && list === null ? (
        <ErrorNote
          title={t("profile.backup.loadFailed")}
          detail={errorMessage(loaded.error)}
          retryLabel={t("common.retry")}
          onRetry={loaded.reload}
        />
      ) : list === null ? (
        <div className="flex flex-col gap-2 py-1">
          <Skeleton className="h-8 w-full" />
          <Skeleton className="h-8 w-full" />
        </div>
      ) : list.length === 0 ? (
        <EmptyState compact icon={Archive} title={t("profile.backup.empty")}>
          {t("profile.backup.emptyHint")}
        </EmptyState>
      ) : (
        <ul aria-label={t("profile.backup.title")} className="flex flex-col">
          {list.map((backup) => (
            <li
              key={backup.id}
              className="flex min-h-11 flex-wrap items-center gap-x-3 gap-y-1 border-b border-border py-1.5 last:border-b-0"
            >
              <span className="mono text-[12.5px] text-fg">{backupTime(backup.takenAt)}</span>
              <span className="flex gap-1">
                {backup.channels.map((channel) => (
                  <Badge key={channel}>{t(CHANNEL_LABEL[channel])}</Badge>
                ))}
              </span>
              <span className="mono text-[11px] text-fg-subtle">{backupSize(backup.size)}</span>
              <span className="ml-auto flex items-center gap-1">
                <RestoreButton
                  backup={backup}
                  disabled={!idle || busy === backup.id}
                  title={idle ? undefined : t("profile.backup.busy")}
                  onRestore={(choice) => void restore(backup, choice)}
                />
                <IconButton
                  icon={FolderOpen}
                  label={t("profile.backup.reveal")}
                  onClick={() => reveal(backup)}
                />
                {confirming === backup.id ? (
                  <Button size="sm" variant="danger" onClick={() => void remove(backup)}>
                    {t("profile.backup.confirmDelete")}
                  </Button>
                ) : (
                  <IconButton
                    icon={Trash2}
                    tone="danger"
                    label={t("profile.backup.delete")}
                    onClick={() => void remove(backup)}
                  />
                )}
              </span>
            </li>
          ))}
        </ul>
      )}
      {!idle && list !== null && list.length > 0 && (
        <p className="pt-2 text-[11.5px] text-fg-subtle">{t("profile.backup.busy")}</p>
      )}
    </Panel>
  );
}
