// The Comebacks page (反击): the user's own lines, which the page's two shortcuts send one at a time
// at random; typing them into the game; champ select's quick phrases, which the Live page sends to
// one teammate; and the library a user may download (在线语录), opted into twice: a confirmation
// before the download and a switch after it.
import type { ComebackLibrary, ComebackSettings, Language, LibraryLevel } from "@winer/shared";
import { Download, MessageSquareQuote, Plus, Trash2, TriangleAlert } from "lucide-react";
import { type ReactNode, useState } from "react";

import { errorMessage } from "../lib/backend";
import {
  DEFAULT_PHRASES,
  LINE_LENGTH,
  LINE_LIMIT,
  PHRASE_LIMIT,
  fillPhrase,
} from "../lib/comeback";
import { useT } from "../lib/i18n";
import { useSettings, useStore } from "../lib/store";
import { useAsync } from "../lib/useAsync";
import {
  Badge,
  Button,
  CommitInput,
  Dialog,
  EmptyState,
  ErrorNote,
  IconButton,
  Input,
  Panel,
  Row,
  Segmented,
  Skeleton,
  Toggle,
  toast,
} from "../ui";
import { GameRightsNote, PageBody } from "./common";
import { ComebackHotkeyRow } from "./settings/HotkeyRow";

const LEVELS: readonly LibraryLevel[] = ["mild", "strong", "both"];

/** The teammate the quick phrases are previewed for, in the user's language. */
const SAMPLE: Record<Language, { seat: string; champion: string; name: string }> = {
  "zh-CN": { seat: "5L", champion: "龙王", name: "峡谷清道夫" },
  en: { seat: "P5", champion: "Aurelion Sol", name: "RiftSweeper" },
};

/** Saves a change to the page's settings, made on the newest ones. */
function useSave() {
  const store = useStore();
  return (change: (rule: ComebackSettings) => ComebackSettings) =>
    void store
      .updateSettings((settings) => ({ ...settings, comeback: change(settings.comeback) }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
}

/** `n` of `max`, for a panel's header. */
function Count({ n, max }: { n: number; max: number }) {
  const t = useT();
  return <span className="mono text-[12px] text-fg-subtle">{t("comeback.count", { n, max })}</span>;
}

/** Lines of text one to a row, each edited in place (saved when the field is left) or removed,
 *  and a field below that adds one; `empty` stands in for the rows while there are none. The core
 *  trims, clips and drops blanks on save. */
function TextList({
  items,
  limit,
  listLabel,
  label,
  removeLabel,
  newLabel,
  placeholder,
  empty,
  onChange,
}: {
  items: readonly string[];
  limit: number;
  listLabel: string;
  /** The accessible names of row `n`, counted from 1, and of its remove button. */
  label: (n: number) => string;
  removeLabel: (n: number) => string;
  newLabel: string;
  placeholder: string;
  empty: ReactNode;
  /** A change to the newest list. */
  onChange: (change: (items: readonly string[]) => string[]) => void;
}) {
  const t = useT();
  const [draft, setDraft] = useState("");
  const full = items.length >= limit;
  const add = () => {
    const text = draft.trim();
    if (!text || full) return;
    onChange((list) => [...list, text]);
    setDraft("");
  };
  return (
    <>
      {items.length > 0 ? (
        <ol aria-label={listLabel} className="flex flex-col">
          {items.map((item, index) => (
            <li key={index} className="flex items-center gap-2 border-b border-border py-1.5">
              <span aria-hidden className="mono w-6 shrink-0 text-right text-[11px] text-fg-subtle">
                {index + 1}
              </span>
              <CommitInput
                aria-label={label(index + 1)}
                value={item}
                maxLength={LINE_LENGTH}
                onCommit={(text) =>
                  onChange((list) => list.map((old, at) => (at === index ? text : old)))
                }
                className="min-w-0 flex-1"
              />
              <IconButton
                icon={Trash2}
                tone="danger"
                label={removeLabel(index + 1)}
                onClick={() => onChange((list) => list.filter((_, at) => at !== index))}
              />
            </li>
          ))}
        </ol>
      ) : (
        empty
      )}
      <form
        onSubmit={(event) => {
          event.preventDefault();
          add();
        }}
        className="mt-3 flex items-center gap-2"
      >
        <Input
          aria-label={newLabel}
          value={draft}
          maxLength={LINE_LENGTH}
          placeholder={placeholder}
          disabled={full}
          onChange={(event) => setDraft(event.target.value)}
          className="min-w-0 flex-1"
        />
        <Button type="submit" icon={Plus} disabled={full || draft.trim() === ""}>
          {t("comeback.add")}
        </Button>
      </form>
      {full && (
        <p className="pt-1.5 text-[11.5px] text-fg-subtle">{t("comeback.full", { n: limit })}</p>
      )}
    </>
  );
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

/** 快捷键: the team's shortcut and the all chat's, each with what the system made of it. */
function ShortcutsPanel() {
  const t = useT();
  return (
    <Panel eyebrow={t("comeback.shortcuts")} className="col-span-12">
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

/** 选人快捷语: the templates the Live page offers for a teammate, the language's own while the user
 *  has none, and how they read for a sample teammate. */
function PhrasesPanel() {
  const t = useT();
  const save = useSave();
  const settings = useSettings();
  const language = settings.general.language;
  const own = settings.comeback.phrases;
  const defaults = DEFAULT_PHRASES[language];
  const sample = SAMPLE[language];
  const preview = t("comeback.phrasePreview", sample);
  return (
    <Panel
      eyebrow={t("comeback.phrases")}
      className="col-span-12"
      right={
        <>
          <Count n={own.length} max={PHRASE_LIMIT} />
          <Button
            size="sm"
            variant="ghost"
            disabled={own.length === 0}
            onClick={() => save((rule) => ({ ...rule, phrases: [] }))}
          >
            {t("auto.reset")}
          </Button>
        </>
      }
    >
      <p className="pb-2 text-[12px] leading-4 text-fg-muted">{t("comeback.phrasesHint")}</p>
      <TextList
        items={own}
        limit={PHRASE_LIMIT}
        listLabel={t("comeback.phrases")}
        label={(n) => t("comeback.phrase", { n })}
        removeLabel={(n) => t("comeback.removePhrase", { n })}
        newLabel={t("comeback.newPhrase")}
        placeholder={t("comeback.newPhrasePlaceholder")}
        empty={
          <>
            <ul className="flex flex-col">
              {defaults.map((phrase) => (
                <li
                  key={phrase}
                  className="flex min-h-11 items-center gap-2 border-b border-border py-1.5"
                >
                  <Badge>{t("comeback.phraseDefault")}</Badge>
                  <span className="min-w-0 flex-1 text-[12.5px] break-words text-fg-muted">
                    {phrase}
                  </span>
                </li>
              ))}
            </ul>
            <p className="pt-1.5 text-[11.5px] text-fg-subtle">{t("comeback.phraseDefaultHint")}</p>
          </>
        }
        onChange={(change) => save((rule) => ({ ...rule, phrases: change(rule.phrases) }))}
      />
      <div className="mt-4 flex flex-col gap-2">
        <p className="text-[14px] font-medium text-fg">{preview}</p>
        <ol
          aria-label={preview}
          className="flex flex-col gap-1 rounded-6 bg-inset px-3 py-2 hairline"
        >
          {(own.length > 0 ? own : defaults).map((phrase, index) => (
            <li key={index} className="text-[12.5px] leading-5 break-words text-fg">
              {fillPhrase(phrase, sample.seat, sample.champion, sample.name)}
            </li>
          ))}
        </ol>
      </div>
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
      <PhrasesPanel />
      <LibraryPanel />
    </PageBody>
  );
}
