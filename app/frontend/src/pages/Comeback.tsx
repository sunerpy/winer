// The Comebacks page (反击): the user's own lines, which the page's two shortcuts send one at a time
// at random; typing them into the game; and the library a user may download (在线语录), opted into
// twice: a confirmation before the download and a switch after it. Champ select's quick phrases
// have their own page (Phrases.tsx).
import type { ComebackLibrary, ComebackSettings, LibraryLevel } from "@winer/shared";
import { Download, MessageSquareQuote, Trash2, TriangleAlert } from "lucide-react";
import { useState } from "react";

import { errorMessage } from "../lib/backend";
import { LINE_LIMIT } from "../lib/comeback";
import { useT } from "../lib/i18n";
import { useSettings, useStore } from "../lib/store";
import { useAsync } from "../lib/useAsync";
import {
  Button,
  Dialog,
  EmptyState,
  ErrorNote,
  Panel,
  Row,
  Segmented,
  Skeleton,
  Toggle,
  toast,
} from "../ui";
import { GameRightsNote, PageBody } from "./common";
import { Count, TextList } from "./TextList";
import { ComebackHotkeyRow } from "./settings/HotkeyRow";

const LEVELS: readonly LibraryLevel[] = ["mild", "strong", "both"];

/** Saves a change to the page's settings, made on the newest ones. */
function useSave() {
  const store = useStore();
  return (change: (rule: ComebackSettings) => ComebackSettings) =>
    void store
      .updateSettings((settings) => ({ ...settings, comeback: change(settings.comeback) }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
}

/** 反击语句: the user's own lines; winer ships none. */
function LinesPanel() {
  const t = useT();
  const save = useSave();
  const lines = useSettings().comeback.lines;
  return (
    <Panel
      eyebrow={t("comeback.lines")}
      className="col-span-12"
      right={<Count n={lines.length} max={LINE_LIMIT} />}
    >
      <p className="pb-2 text-[12px] leading-4 text-fg-muted">{t("comeback.linesHint")}</p>
      <TextList
        items={lines}
        limit={LINE_LIMIT}
        listLabel={t("comeback.lines")}
        label={(n) => t("comeback.line", { n })}
        removeLabel={(n) => t("comeback.removeLine", { n })}
        newLabel={t("comeback.newLine")}
        placeholder={t("comeback.newLinePlaceholder")}
        empty={
          <EmptyState compact icon={MessageSquareQuote} title={t("comeback.linesEmpty")}>
            {t("comeback.linesEmptyHint")}
          </EmptyState>
        }
        onChange={(change) => save((rule) => ({ ...rule, lines: change(rule.lines) }))}
      />
    </Panel>
  );
}

/** 快捷键: the team's shortcut and the all chat's, each with what the system made of it; and a
 *  warning while there is nothing for them to send. */
function ShortcutsPanel() {
  const t = useT();
  const { lines, library } = useSettings().comeback;
  return (
    <Panel eyebrow={t("comeback.shortcuts")} className="col-span-12">
      {lines.length === 0 && !library.enabled && (
        <p className="flex items-start gap-1.5 pb-2 text-[12px] leading-4 text-warning">
          <TriangleAlert size={13} strokeWidth={2} aria-hidden className="mt-0.5 shrink-0" />
          {t("comeback.noLinesYet")}
        </p>
      )}
      <ComebackHotkeyRow all={false} />
      <ComebackHotkeyRow all />
      <p className="pt-2 text-[11.5px] leading-4 text-fg-subtle">{t("comeback.spacing")}</p>
    </Panel>
  );
}

/** 游戏内发送: typing into the game, off until switched on, said to be third-party input; and the
 *  rights a game started from WeGame needs. */
function GamePanel() {
  const t = useT();
  const save = useSave();
  const inGame = useSettings().comeback.inGame;
  return (
    <Panel eyebrow={t("comeback.gameSection")} className="col-span-12">
      <Row
        label={t("comeback.inGame")}
        help={
          <>
            {t("comeback.inGameHint")}
            <span className="mt-1 flex items-start gap-1.5 text-warning">
              <TriangleAlert size={13} strokeWidth={2} aria-hidden className="mt-0.5 shrink-0" />
              {t("comeback.inGameRisk")}
            </span>
          </>
        }
      >
        <Toggle
          checked={inGame}
          onChange={(next) => save((rule) => ({ ...rule, inGame: next }))}
          label={t("comeback.inGame")}
        />
      </Row>
      <GameRightsNote />
    </Panel>
  );
}

/** 在线语录: what the library is and what sending it risks; a download only after the confirmation
 *  says so again; then its counts, its removal, and the switch that puts it to use, which every
 *  download and removal turns off (the core saves it, and the settings follow). */
function LibraryPanel() {
  const t = useT();
  const store = useStore();
  const save = useSave();
  const settings = useSettings();
  const rule = settings.comeback.library;
  const query = useAsync(() => store.backend.call("comeback_library"), []);
  /** What a download or a removal answered, newer than the status read on opening the page. */
  const [answer, setAnswer] = useState<ComebackLibrary | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState<"download" | "remove" | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
  const status = answer ?? query.data;
  const downloaded = status?.downloaded === true;
  const day = new Intl.DateTimeFormat(settings.general.language, { dateStyle: "medium" });

  const download = async () => {
    setConfirming(false);
    setBusy("download");
    setFailure(null);
    try {
      setAnswer(await store.backend.call("download_comeback_library"));
      toast(t("comeback.libraryDownloaded"), "ok");
    } catch (error) {
      // Every mirror failed, or the bytes were not the pinned file's: nothing changed.
      setFailure(errorMessage(error));
    } finally {
      setBusy(null);
    }
  };
  const remove = async () => {
    setBusy("remove");
    setFailure(null);
    try {
      setAnswer(await store.backend.call("remove_comeback_library"));
      toast(t("comeback.libraryRemoved"), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(null);
    }
  };
  const risk = (
    <p className="flex items-start gap-1.5 text-warning">
      <TriangleAlert size={13} strokeWidth={2} aria-hidden className="mt-1 shrink-0" />
      {t("comeback.libraryRisk")}
    </p>
  );

  return (
    <Panel eyebrow={t("comeback.library")} className="col-span-12">
      <div className="flex max-w-[760px] flex-col gap-1.5 pb-2 text-[12px] leading-5 text-fg-muted">
        <p>{t("comeback.libraryAbout")}</p>
        {risk}
      </div>
      {status === undefined && query.error !== undefined ? (
        <ErrorNote
          title={t("comeback.libraryLoadFailed")}
          detail={errorMessage(query.error)}
          retryLabel={t("common.retry")}
          onRetry={query.reload}
        />
      ) : (
        <Row
          label={t("comeback.libraryState")}
          help={
            status === undefined ? (
              <Skeleton className="mt-1 h-3.5 w-48" />
            ) : downloaded ? (
              t("comeback.libraryCounts", {
                mild: status.mild,
                strong: status.strong,
                date: status.fetchedAt === null ? "—" : day.format(status.fetchedAt),
              })
            ) : (
              t("comeback.libraryNone")
            )
          }
        >
          {downloaded ? (
            <Button
              size="sm"
              variant="ghost"
              icon={Trash2}
              loading={busy === "remove"}
              disabled={busy !== null}
              onClick={() => void remove()}
            >
              {t("comeback.libraryRemove")}
            </Button>
          ) : (
            <Button
              size="sm"
              icon={Download}
              loading={busy === "download"}
              disabled={status === undefined || busy !== null}
              onClick={() => setConfirming(true)}
            >
              {t("comeback.libraryDownload")}
            </Button>
          )}
        </Row>
      )}
      {failure !== null && (
        <p role="alert" className="py-2 text-[12px] leading-4 break-words text-danger">
          {t("comeback.libraryFailed", { message: failure })}
        </p>
      )}
      <Row
        label={t("comeback.libraryUse")}
        help={t(downloaded ? "comeback.libraryUseHint" : "comeback.libraryUseNeedsDownload")}
      >
        <Toggle
          checked={downloaded && rule.enabled}
          disabled={!downloaded}
          onChange={(enabled) =>
            save((value) => ({ ...value, library: { ...value.library, enabled } }))
          }
          label={t("comeback.libraryUse")}
        />
      </Row>
      {downloaded && (
        <Row label={t("comeback.libraryLevel")}>
          <Segmented<LibraryLevel>
            size="sm"
            label={t("comeback.libraryLevel")}
            value={rule.level}
            options={LEVELS.map((level) => ({ value: level, label: t(`comeback.level.${level}`) }))}
            onChange={(level) =>
              save((value) => ({ ...value, library: { ...value.library, level } }))
            }
          />
        </Row>
      )}
      <Dialog
        open={confirming}
        onClose={() => setConfirming(false)}
        title={t("comeback.libraryConfirmTitle")}
        closeLabel={t("common.close")}
        footer={
          <>
            <Button variant="ghost" data-autofocus onClick={() => setConfirming(false)}>
              {t("common.cancel")}
            </Button>
            <Button variant="primary" icon={Download} onClick={() => void download()}>
              {t("comeback.libraryConfirm")}
            </Button>
          </>
        }
      >
        <div className="flex flex-col gap-3 px-5 py-4 text-[12.5px] leading-5 text-fg-muted">
          <p>{t("comeback.libraryAbout")}</p>
          {risk}
          <p>{t("comeback.libraryOptIn")}</p>
        </div>
      </Dialog>
    </Panel>
  );
}

export function ComebackPage() {
  return (
    <PageBody className="grid grid-cols-12 gap-3">
      <LinesPanel />
      <ShortcutsPanel />
      <GamePanel />
      <LibraryPanel />
    </PageBody>
  );
}
