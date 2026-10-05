import type { Accent, Appearance, Language, Settings, Theme, TierSet } from "@winer/shared";
import {
  BookOpen,
  Check,
  ExternalLink,
  FolderOpen,
  Info,
  Medal,
  Palette,
  RefreshCw,
  SlidersHorizontal,
} from "lucide-react";
import { type KeyboardEvent, type ReactNode, useId, useRef } from "react";

import { modeOf, resolveTheme, systemPrefersDark } from "../../lib/appearance";
import { errorMessage } from "../../lib/backend";
import { cx } from "../../lib/cx";
import { type MessageKey, useT } from "../../lib/i18n";
import { useSettings, useStore, useUpdateStatus } from "../../lib/store";
import { SCHEMES, TIER_NAMES } from "../../lib/tiers";
import { useAsync } from "../../lib/useAsync";
import type { SettingsSection } from "../../shell/navigation";
import {
  Button,
  CommitInput,
  Dialog,
  Lamp,
  Row,
  Segmented,
  Toggle,
  onRadioKeys,
  toast,
} from "../../ui";
import { updateLine } from "./UpdateDialog";

const SECTIONS: { id: SettingsSection; label: MessageKey; icon: typeof Palette }[] = [
  { id: "appearance", label: "settings.appearance", icon: Palette },
  { id: "general", label: "settings.general", icon: SlidersHorizontal },
  { id: "rating", label: "settings.rating", icon: Medal },
  { id: "about", label: "settings.about", icon: Info },
];

const THEMES: Theme[] = ["system", "light", "dark", "graphite", "hextech"];
const ACCENTS: Accent[] = ["default", "gold", "blue", "teal", "green", "orange", "pink", "purple"];
const FONT_SIZES = ["12", "13", "14", "15", "16"] as const;

function useSave() {
  const store = useStore();
  return (change: (settings: Settings) => Settings) =>
    void store
      .updateSettings(change)
      .catch((error: unknown) => toast(errorMessage(error), "danger"));
}

/** A miniature of the window in one theme: rail, title bar and a card. */
function ThemeTile({
  theme,
  selected,
  onSelect,
  label,
}: {
  theme: Theme;
  selected: boolean;
  onSelect: () => void;
  label: string;
}) {
  const resolved = resolveTheme(theme, systemPrefersDark());
  return (
    <button
      type="button"
      role="radio"
      aria-checked={selected}
      data-value={theme}
      tabIndex={selected ? 0 : -1}
      onClick={onSelect}
      className="group flex flex-col items-center gap-1.5"
    >
      <span
        data-theme={resolved}
        data-mode={modeOf(resolved)}
        data-accent="default"
        className={cx(
          "relative block h-[68px] w-[104px] overflow-hidden rounded-10 bg-canvas transition-shadow duration-150",
          selected
            ? "shadow-[0_0_0_2px_var(--accent-text)]"
            : "hairline group-hover:border-border-strong",
        )}
      >
        <span className="absolute inset-y-0 left-0 w-6 border-r border-border bg-nav" />
        <span className="absolute top-0 right-0 left-6 h-3 border-b border-border bg-surface" />
        <span className="absolute top-5 right-2 left-8 h-5 rounded-4 bg-surface hairline" />
        <span className="absolute top-[30px] left-10 h-1.5 w-8 rounded-pill bg-accent" />
        <span className="absolute right-2 bottom-2 left-8 h-3 rounded-4 bg-inset" />
        {theme === "system" && (
          <span className="absolute inset-y-0 right-0 w-1/2 bg-[linear-gradient(90deg,transparent,rgb(0_0_0/0.35))]" />
        )}
      </span>
      <span className={cx("text-[12px]", selected ? "font-medium text-fg" : "text-fg-muted")}>
        {label}
      </span>
    </button>
  );
}

function AppearanceSection() {
  const t = useT();
  const save = useSave();
  const appearance = useSettings().appearance;
  const set = (change: Partial<Appearance>) =>
    save((settings) => ({ ...settings, appearance: { ...settings.appearance, ...change } }));
  const resolved = resolveTheme(appearance.theme, systemPrefersDark());
  const themeLabel = useId();
  return (
    <div>
      <div className="border-b border-border pt-3 pb-4">
        <p id={themeLabel} className="mb-3 text-[14px] font-medium text-fg">
          {t("settings.theme")}
        </p>
        <div
          role="radiogroup"
          aria-labelledby={themeLabel}
          onKeyDown={(event) =>
            onRadioKeys(event, THEMES, appearance.theme, (theme) => set({ theme }))
          }
          className="flex flex-wrap gap-3"
        >
          {THEMES.map((theme) => (
            <ThemeTile
              key={theme}
              theme={theme}
              label={t(`settings.theme.${theme}`)}
              selected={appearance.theme === theme}
              onSelect={() => set({ theme })}
            />
          ))}
        </div>
      </div>
      <Row label={t("settings.accent")}>
        <div
          role="radiogroup"
          aria-label={t("settings.accent")}
          onKeyDown={(event) =>
            onRadioKeys(event, ACCENTS, appearance.accent, (accent) => set({ accent }))
          }
          className="flex items-center gap-1.5"
        >
          {ACCENTS.map((accent) => {
            const selected = appearance.accent === accent;
            return (
              <button
                key={accent}
                type="button"
                role="radio"
                aria-checked={selected}
                aria-label={t(`settings.accent.${accent}`)}
                title={t(`settings.accent.${accent}`)}
                data-value={accent}
                tabIndex={selected ? 0 : -1}
                data-accent={accent}
                data-theme={resolved}
                onClick={() => set({ accent })}
                className={cx(
                  "grid size-6 place-items-center rounded-full transition-transform duration-150 hover:scale-110",
                  // "Follow the theme" is always some theme's colour; a ring keeps it from reading as
                  // a second gold or blue.
                  accent === "default" ? "border-2 border-accent" : "bg-accent",
                  selected && "shadow-[0_0_0_2px_var(--raised),0_0_0_4px_var(--accent)]",
                )}
              >
                {selected ? (
                  <Check
                    size={12}
                    strokeWidth={3}
                    className={accent === "default" ? "text-accent-text" : "text-accent-fg"}
                  />
                ) : (
                  accent === "default" && (
                    <span aria-hidden className="size-2 rounded-full bg-accent" />
                  )
                )}
              </button>
            );
          })}
        </div>
      </Row>
      <Row label={t("settings.density")}>
        <Segmented
          size="sm"
          label={t("settings.density")}
          value={appearance.density}
          onChange={(density) => set({ density })}
          options={[
            { value: "comfortable", label: t("settings.density.comfortable") },
            { value: "compact", label: t("settings.density.compact") },
          ]}
        />
      </Row>
      <Row label={t("settings.fontSize")}>
        <Segmented
          size="sm"
          label={t("settings.fontSize")}
          value={String(appearance.fontSize) as (typeof FONT_SIZES)[number]}
          onChange={(size) => set({ fontSize: Number(size) })}
          options={FONT_SIZES.map((size) => ({ value: size, label: size }))}
        />
      </Row>
      <Row label={t("settings.reduceMotion")} help={t("settings.reduceMotionHint")}>
        <Toggle
          checked={appearance.reduceMotion}
          onChange={(reduceMotion) => set({ reduceMotion })}
          label={t("settings.reduceMotion")}
        />
      </Row>
    </div>
  );
}

/** Each language named in itself, so it can be found from a language one cannot read. */
const LANGUAGES: { value: Language; label: string }[] = [
  { value: "zh-CN", label: "简体中文" },
  { value: "en", label: "English" },
];

function GeneralSection() {
  const t = useT();
  const save = useSave();
  const store = useStore();
  const general = useSettings().general;
  const autostart = useAsync(() => store.backend.call("get_autostart"), []);

  const setAutostart = async (enabled: boolean) => {
    try {
      await store.backend.call("set_autostart", { enabled });
      autostart.reload();
    } catch (error) {
      toast(errorMessage(error), "danger");
    }
  };

  return (
    <div>
      <Row label={t("settings.language")}>
        <div
          role="group"
          aria-label="Language / 语言"
          className="inline-flex rounded-6 bg-inset p-0.5 hairline"
        >
          {LANGUAGES.map((language) => (
            <button
              key={language.value}
              type="button"
              lang={language.value}
              aria-pressed={general.language === language.value}
              onClick={() =>
                save((settings) => ({
                  ...settings,
                  general: { ...settings.general, language: language.value },
                }))
              }
              className={cx(
                "h-7 rounded-[5px] px-3 text-[12.5px] transition-colors duration-150",
                general.language === language.value
                  ? "bg-surface font-medium text-fg shadow-[0_0_0_1px_var(--border)]"
                  : "text-fg-muted hover:text-fg",
              )}
            >
              {language.label}
            </button>
          ))}
        </div>
      </Row>
      <Row label={t("settings.closeToTray")} help={t("settings.closeToTrayHint")}>
        <Toggle
          checked={general.closeToTray}
          onChange={(closeToTray) =>
            save((settings) => ({ ...settings, general: { ...settings.general, closeToTray } }))
          }
          label={t("settings.closeToTray")}
        />
      </Row>
      <Row label={t("settings.augmentDetails")} help={t("settings.augmentDetailsHint")}>
        <Toggle
          checked={general.augmentDetails}
          onChange={(augmentDetails) =>
            save((settings) => ({ ...settings, general: { ...settings.general, augmentDetails } }))
          }
          label={t("settings.augmentDetails")}
        />
      </Row>
      <Row label={t("settings.autostart")} help={t("settings.autostartHint")}>
        <Toggle
          checked={autostart.data ?? false}
          disabled={autostart.data === undefined}
          onChange={(enabled) => void setAutostart(enabled)}
          label={t("settings.autostart")}
        />
      </Row>
    </div>
  );
}

/** The user's own tier names, up to the core's `CalloutRule::MAX_TIERS`. */
const MAX_TIERS = 5;

function RatingSection() {
  const t = useT();
  const store = useStore();
  const settings = useSettings();
  const save = useSave();
  const language = settings.general.language;
  const rule = settings.automation.callout;
  const label = useId();
  const setRule = (
    change: (rule: Settings["automation"]["callout"]) => Settings["automation"]["callout"],
  ) =>
    save((value) => ({
      ...value,
      automation: { ...value.automation, callout: change(value.automation.callout) },
    }));
  const choose = (tiers: TierSet) => setRule((value) => ({ ...value, tiers }));

  return (
    <>
      <p id={label} className="mb-3 text-[14px] font-medium text-fg">
        {t("rating.scheme")}
      </p>
      <div className="@container">
        <div
          role="radiogroup"
          aria-labelledby={label}
          onKeyDown={(event) => onRadioKeys(event, SCHEMES, rule.tiers, choose)}
          className="grid grid-cols-1 gap-2 @[520px]:grid-cols-2"
        >
          {SCHEMES.map((scheme) => {
            const selected = rule.tiers === scheme;
            return (
              <button
                key={scheme}
                type="button"
                role="radio"
                data-value={scheme}
                aria-checked={selected}
                tabIndex={selected ? 0 : -1}
                onClick={() => choose(scheme)}
                className={cx(
                  "flex min-w-0 flex-col gap-1 rounded-8 px-3 py-2.5 text-left transition-colors duration-150 hairline",
                  selected ? "bg-accent-soft shadow-[0_0_0_1.5px_var(--accent)]" : "hover-wash",
                )}
              >
                <span className="flex items-center gap-2 text-[13px] font-medium text-fg">
                  {t(`rating.scheme.${scheme}`)}
                  {selected && (
                    <Check size={13} strokeWidth={2.4} className="text-accent-text" aria-hidden />
                  )}
                </span>
                <span className="text-[11.5px] leading-5 text-fg-subtle">
                  {t(`rating.scheme.${scheme}Hint`)}
                </span>
                {scheme !== "custom" && (
                  <span className="text-[11px] leading-4 text-fg-muted">
                    {TIER_NAMES[scheme][language].join(" → ")}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>

      {rule.tiers === "custom" && (
        <div className="flex flex-col gap-2 border-b border-border py-3">
          <span className="text-[14px] font-medium text-fg">{t("rating.customTiers")}</span>
          <span className="grid grid-cols-5 gap-1.5">
            {Array.from({ length: MAX_TIERS }, (_, index) => (
              <CommitInput
                key={index}
                aria-label={t("rating.tierSlot", { n: index + 1 })}
                value={rule.customTiers[index] ?? ""}
                maxLength={16}
                placeholder={TIER_NAMES.riftFive[language][index]}
                onCommit={(name) =>
                  setRule((value) => {
                    const names = Array.from(
                      { length: MAX_TIERS },
                      (_, at) => value.customTiers[at] ?? "",
                    );
                    names[index] = name;
                    // Trailing blanks are dropped, inner ones kept so every box stays where it was.
                    while (names.length > 0 && !names.at(-1)?.trim()) names.pop();
                    return { ...value, customTiers: names };
                  })
                }
                className="min-w-0"
              />
            ))}
          </span>
          <span className="text-[12px] leading-4 text-fg-muted">{t("rating.customTiersHint")}</span>
        </div>
      )}

      <Row label={t("rating.titles")} help={t("rating.titlesHint")}>
        <Toggle
          checked={settings.general.titles}
          onChange={(titles) =>
            save((value) => ({ ...value, general: { ...value.general, titles } }))
          }
          label={t("rating.titles")}
        />
      </Row>

      <section
        aria-labelledby={`${label}-basis`}
        className="mt-4 rounded-8 bg-inset px-3.5 py-3 hairline"
      >
        <div className="mb-2 flex items-center justify-between gap-3">
          <h3 id={`${label}-basis`} className="text-[13px] font-medium text-fg">
            {t("rating.basis")}
          </h3>
          <Button
            size="sm"
            variant="ghost"
            icon={BookOpen}
            onClick={() =>
              void store.backend
                .call("open_docs", { page: "rating" })
                .catch((error: unknown) => toast(errorMessage(error), "danger"))
            }
          >
            {t("rating.docs")}
          </Button>
        </div>
        <ul className="flex flex-col gap-1.5 text-[12px] leading-5 text-fg-muted">
          <li>{t("rating.basis.form")}</li>
          <li>{t("rating.basis.tiers")}</li>
          <li>{t("rating.basis.game")}</li>
          <li>{t("rating.basis.titles")}</li>
        </ul>
      </section>
    </>
  );
}

function AboutSection({ onOpenUpdate }: { onOpenUpdate: () => void }) {
  const t = useT();
  const store = useStore();
  const info = useAsync(() => store.backend.call("get_app_info"), []);
  const update = useUpdateStatus();
  const busy =
    update.state === "checking" || update.state === "downloading" || update.state === "installing";
  const call = (command: "reveal_logs" | "open_releases") =>
    void store.backend
      .call(command)
      .catch((error: unknown) => toast(errorMessage(error), "danger"));

  return (
    <div>
      <Row label={t("settings.version")}>
        <span className="mono text-[13px] text-fg">{info.data?.version ?? "—"}</span>
      </Row>
      <Row label={t("settings.elevated")}>
        <span className="flex items-center gap-2 text-[12.5px] text-fg">
          <Lamp tone={info.data?.elevated ? "ok" : "idle"} size={6} />
          {info.data
            ? info.data.elevated
              ? t("settings.elevatedYes")
              : t("settings.elevatedNo")
            : "—"}
        </span>
      </Row>
      <Row
        label={t("settings.logs")}
        help={<span className="mono break-all">{info.data?.logDir}</span>}
      >
        <Button size="sm" icon={FolderOpen} onClick={() => call("reveal_logs")}>
          {t("settings.openLogs")}
        </Button>
      </Row>
      <Row label={t("settings.update")} help={updateLine(update, t)}>
        {update.state === "available" ? (
          <Button size="sm" variant="accent" onClick={onOpenUpdate}>
            {t("update.install")}
          </Button>
        ) : (
          <Button
            size="sm"
            icon={RefreshCw}
            loading={busy}
            onClick={() =>
              void store.backend
                .call("check_update")
                .catch((error: unknown) => toast(errorMessage(error), "danger"))
            }
          >
            {t("settings.checkUpdate")}
          </Button>
        )}
        <Button size="sm" variant="ghost" icon={ExternalLink} onClick={() => call("open_releases")}>
          {t("settings.releases")}
        </Button>
      </Row>
      <Row label={t("settings.notices")} help={t("settings.noticesHint")}>
        <details className="w-full">
          <summary className="cursor-pointer text-[12.5px] text-fg-muted hover:text-fg">
            {t("settings.noticesShow")}
          </summary>
          <pre className="mono mt-2 max-h-56 overflow-auto rounded-6 bg-inset p-3 text-[11px] hairline leading-4 whitespace-pre-wrap text-fg-muted">
            {info.data?.notices}
          </pre>
        </details>
      </Row>
      <p className="pt-4 text-[11.5px] leading-5 text-fg-subtle">{t("settings.license")}</p>
    </div>
  );
}

export function SettingsDialog({
  section,
  onSection,
  onClose,
  onOpenUpdate,
}: {
  section: SettingsSection | null;
  onSection: (section: SettingsSection) => void;
  onClose: () => void;
  onOpenUpdate: () => void;
}) {
  const t = useT();
  const tabs = useRef<HTMLDivElement>(null);
  const current = section ?? "appearance";

  const onKeyDown = (event: KeyboardEvent) => {
    const step = event.key === "ArrowDown" ? 1 : event.key === "ArrowUp" ? -1 : 0;
    if (!step) return;
    event.preventDefault();
    const index = SECTIONS.findIndex((entry) => entry.id === current);
    const next = SECTIONS[(index + step + SECTIONS.length) % SECTIONS.length];
    if (!next) return;
    onSection(next.id);
    tabs.current?.querySelector<HTMLButtonElement>(`#settings-tab-${next.id}`)?.focus();
  };

  const panels: Record<SettingsSection, ReactNode> = {
    appearance: <AppearanceSection />,
    general: <GeneralSection />,
    rating: <RatingSection />,
    about: <AboutSection onOpenUpdate={onOpenUpdate} />,
  };

  return (
    <Dialog
      open={section !== null}
      onClose={onClose}
      title={t("settings.title")}
      closeLabel={t("common.close")}
      size="lg"
      dismissOnScrim
    >
      <div className="grid min-h-0 flex-1 grid-cols-[180px_minmax(0,1fr)]">
        <div
          ref={tabs}
          role="tablist"
          aria-orientation="vertical"
          aria-label={t("settings.title")}
          onKeyDown={onKeyDown}
          className="flex flex-col gap-0.5 border-r border-border p-3"
        >
          {SECTIONS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              id={`settings-tab-${id}`}
              type="button"
              role="tab"
              aria-selected={id === current}
              aria-controls={`settings-panel-${id}`}
              tabIndex={id === current ? 0 : -1}
              data-autofocus={id === current || undefined}
              onClick={() => onSection(id)}
              className={cx(
                "flex h-9 items-center gap-2.5 rounded-6 px-2.5 text-[13px] transition-colors duration-150",
                id === current
                  ? "bg-nav-active font-medium text-fg"
                  : "text-fg-muted hover:text-fg hover-wash",
              )}
            >
              <Icon
                size={15}
                strokeWidth={2}
                aria-hidden
                className={id === current ? "text-accent-text" : "text-fg-subtle"}
              />
              {t(label)}
            </button>
          ))}
        </div>
        {SECTIONS.map(({ id }) => (
          <section
            key={id}
            id={`settings-panel-${id}`}
            role="tabpanel"
            aria-labelledby={`settings-tab-${id}`}
            hidden={id !== current}
            className="min-h-0 overflow-y-auto px-6 py-3"
          >
            {id === current && panels[id]}
          </section>
        ))}
      </div>
    </Dialog>
  );
}
