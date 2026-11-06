// The Phrases page (快捷语): champ select's quick phrases, which the Live page sends to one teammate,
// and what goes out around a trade (交换): the shortcut that sends the first phrase to the teammate
// asked to trade, and the two switches that send it, or a thank-you, by themselves. The settings stay
// under `comeback`, where 0.0.19 saved the phrases and their shortcut.
import type { ComebackSettings, Language } from "@winer/shared";

import { errorMessage } from "../lib/backend";
import {
  DEFAULT_PHRASES,
  DEFAULT_THANKS,
  LINE_LENGTH,
  PHRASE_LIMIT,
  fillPhrase,
  thanksOf,
} from "../lib/comeback";
import { useT } from "../lib/i18n";
import { useSettings, useStore } from "../lib/store";
import { Badge, Button, CommitInput, Panel, Row, Toggle, toast } from "../ui";
import { PageBody } from "./common";
import { Count, TextList } from "./TextList";
import { PhraseHotkeyRow } from "./settings/HotkeyRow";

/** The teammate the quick phrases are previewed for, in the user's language. */
const SAMPLE: Record<Language, { seat: string; champion: string; name: string }> = {
  "zh-CN": { seat: "5L", champion: "龙王", name: "峡谷清道夫" },
  en: { seat: "P5", champion: "Aurelion Sol", name: "RiftSweeper" },
};

/** Saves a change to the phrases' settings, made on the newest ones. */
function useSave() {
  const store = useStore();
  return (change: (rule: ComebackSettings) => ComebackSettings) =>
    void store
      .updateSettings((settings) => ({ ...settings, comeback: change(settings.comeback) }))
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
}

/** 快捷语: the templates the Live page offers for a teammate, the language's own while the user has
 *  none, and how they read for a sample teammate. */
function PhrasesPanel() {
  const t = useT();
  const save = useSave();
  const settings = useSettings();
  const language = settings.general.language;
  const own = settings.comeback.phrases;
  const defaults = DEFAULT_PHRASES[language];
  const sample = SAMPLE[language];
  const preview = t("phrase.preview", sample);
  return (
    <Panel
      eyebrow={t("phrase.list")}
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
      <p className="pb-2 text-[12px] leading-4 text-fg-muted">{t("phrase.listHint")}</p>
      <TextList
        items={own}
        limit={PHRASE_LIMIT}
        listLabel={t("phrase.list")}
        label={(n) => t("phrase.item", { n })}
        removeLabel={(n) => t("phrase.remove", { n })}
        newLabel={t("phrase.new")}
        placeholder={t("phrase.newPlaceholder")}
        firstMark={t("phrase.tradeFirst")}
        empty={
          <>
            <ul className="flex flex-col">
              {defaults.map((phrase, index) => (
                <li
                  key={phrase}
                  className="flex min-h-11 items-center gap-2 border-b border-border py-1.5"
                >
                  <Badge>{t("phrase.default")}</Badge>
                  {index === 0 && <Badge tone="info">{t("phrase.tradeFirst")}</Badge>}
                  <span className="min-w-0 flex-1 text-[12.5px] break-words text-fg-muted">
                    {phrase}
                  </span>
                </li>
              ))}
            </ul>
            <p className="pt-1.5 text-[11.5px] text-fg-subtle">{t("phrase.defaultHint")}</p>
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

/** 交换英雄: the shortcut that sends the first phrase to the teammate asked to trade, the switches
 *  that send it by themselves when the user asks and a thank-you once the trade went through, and
 *  the spacing the champ-select chat keeps with the comebacks. */
function TradePanel() {
  const t = useT();
  const save = useSave();
  const settings = useSettings();
  const language = settings.general.language;
  const trade = settings.comeback.trade;
  const sample = SAMPLE[language];
  const saveTrade = (change: Partial<typeof trade>) =>
    save((rule) => ({ ...rule, trade: { ...rule.trade, ...change } }));
  return (
    <Panel eyebrow={t("phrase.trade")} className="col-span-12">
      <PhraseHotkeyRow />
      <Row label={t("phrase.tradeAsk")} help={t("phrase.tradeAskHint")}>
        <Toggle
          checked={trade.onAsk}
          onChange={(onAsk) => saveTrade({ onAsk })}
          label={t("phrase.tradeAsk")}
        />
      </Row>
      <Row label={t("phrase.tradeDone")} help={t("phrase.tradeDoneHint")}>
        <Toggle
          checked={trade.onDone}
          onChange={(onDone) => saveTrade({ onDone })}
          label={t("phrase.tradeDone")}
        />
      </Row>
      <div className="flex flex-col gap-2 py-3">
        <label htmlFor="phrase-thanks" className="text-[14px] font-medium text-fg">
          {t("phrase.thanks")}
        </label>
        <div className="flex items-center gap-2">
          <CommitInput
            id="phrase-thanks"
            value={trade.thanks}
            maxLength={LINE_LENGTH}
            placeholder={DEFAULT_THANKS[language]}
            onCommit={(thanks) => saveTrade({ thanks })}
            className="min-w-0 flex-1"
          />
          <Button
            size="sm"
            variant="ghost"
            aria-label={t("phrase.thanksReset")}
            disabled={trade.thanks === ""}
            onClick={() => saveTrade({ thanks: "" })}
          >
            {t("auto.reset")}
          </Button>
        </div>
        <p className="text-[12px] leading-4 break-words text-fg-muted">
          {t("phrase.thanksPreview", {
            text: fillPhrase(
              thanksOf(settings.comeback, language),
              sample.seat,
              sample.champion,
              sample.name,
            ),
          })}
        </p>
      </div>
      <p className="pt-1 text-[11.5px] leading-4 text-fg-subtle">{t("phrase.spacing")}</p>
    </Panel>
  );
}

export function PhrasesPage() {
  return (
    <PageBody className="grid grid-cols-12 gap-3">
      <PhrasesPanel />
      <TradePanel />
    </PageBody>
  );
}
