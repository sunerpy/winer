// The global shortcuts as settings rows: Settings › 通用's, which brings winer up from anywhere,
// 自动化 › 战力喊话's, which sends the callout, and the Comebacks page's two. The recorder takes the
// window's next keydown before anything else does (capture phase: the shell's own shortcuts and the
// dialog's Escape never see it), and the shell lets every shortcut go while it listens, or pressing
// one would act instead of being recorded. No shortcut takes another's combination.
import type { Settings } from "@winer/shared";
import { Keyboard, X } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";

import { errorMessage } from "../../lib/backend";
import { keycaps, record } from "../../lib/hotkey";
import { type Translate, useT } from "../../lib/i18n";
import { useHotkeyStatus, useSettings, useStore } from "../../lib/store";
import { Button, IconButton, Kbd, Lamp, Row, toast } from "../../ui";

export function Keycaps({ combo }: { combo: string }) {
  return (
    <span className="inline-flex items-center gap-1">
      {keycaps(combo).map((key, index) => (
        <Kbd key={`${key}-${index}`}>{key}</Kbd>
      ))}
    </span>
  );
}

/** What the system made of a combination, once it has said. */
interface Held {
  active: boolean;
  error: string | null;
}

/** A combination another shortcut holds, and what to say when it is pressed for this one. */
interface Taken {
  combo: string | null;
  message: string;
}

/** The recorder listening now: starting another ends it, so a page with two rows (the Comebacks
 *  page) records each combination once. */
let recording: (() => void) | null = null;
/** Recorders holding the shortcuts let go; the shell takes them back when the last one ends. */
let suspending = 0;

/** One shortcut: its keys, a recorder for a new combination and a ✕ that turns it off. */
function ShortcutRow({
  label,
  hint,
  combo,
  held,
  taken,
  onSave,
}: {
  label: string;
  hint: ReactNode;
  combo: string | null;
  /** What the system said about `combo`; `null` until it has. */
  held: Held | null;
  /** The other shortcuts' combinations, which this one cannot take. */
  taken: Taken[];
  onSave: (combo: string | null) => Promise<void>;
}) {
  const t = useT();
  const store = useStore();
  const [listening, setListening] = useState(false);
  const [keys, setKeys] = useState<string[]>([]);
  /** What is wrong with the keys just pressed, said in the row. */
  const [problem, setProblem] = useState<string | null>(null);
  // The listener lives as long as the recording; it reads the newest of these when a key comes.
  const latest = useRef({ taken, onSave, t });
  useEffect(() => {
    latest.current = { taken, onSave, t };
  });

  const save = (next: string | null) =>
    void latest.current
      .onSave(next)
      .catch((error: unknown) => toast(errorMessage(error), "danger"));

  useEffect(() => {
    if (!listening) return undefined;
    recording?.();
    const stop = () => setListening(false);
    recording = stop;
    const suspend = (suspended: boolean) =>
      void store.backend
        .call("suspend_hotkey", { suspended })
        .then((next) => store.hotkey.set(next))
        .catch(() => undefined);
    if (suspending++ === 0) suspend(true);
    const onKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopImmediatePropagation();
      const bare = !(event.ctrlKey || event.altKey || event.shiftKey || event.metaKey);
      if (event.key === "Escape" && bare) {
        setListening(false);
        return;
      }
      const result = record(event);
      const holder =
        result.kind === "done"
          ? latest.current.taken.find((other) => other.combo === result.combo)
          : undefined;
      if (result.kind === "partial") {
        setKeys(result.keys);
        setProblem(null);
      } else if (result.kind === "invalid") {
        setKeys([]);
        setProblem(latest.current.t("social.hotkeyInvalid"));
      } else if (holder) {
        setKeys([]);
        setProblem(holder.message);
      } else {
        setListening(false);
        void latest.current
          .onSave(result.combo)
          .catch((error: unknown) => toast(errorMessage(error), "danger"));
      }
    };
    // Another window took the focus: the keys go there now, so stop listening.
    const onBlur = () => setListening(false);
    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", onBlur);
      if (recording === stop) recording = null;
      // Done, cancelled or the settings closed: the shortcuts the settings name come back, once no
      // other recorder has taken over.
      if (--suspending === 0) suspend(false);
    };
  }, [listening, store]);

  const start = () => {
    setKeys([]);
    setProblem(null);
    setListening(true);
  };

  return (
    <Row
      label={label}
      help={
        <>
          {hint}
          {listening ? (
            <span className="mt-1 block text-fg-subtle">
              {problem ? (
                <span className="text-danger">{problem}</span>
              ) : (
                t("social.hotkeyListeningHint")
              )}
            </span>
          ) : held?.error ? (
            <span className="mt-1 block text-danger" title={held.error}>
              {t("social.hotkeyFailed")}
            </span>
          ) : held?.active ? (
            <span className="mt-1 flex items-center gap-1.5 text-fg-subtle">
              <Lamp tone="ok" size={6} />
              {t("social.hotkeyActive")}
            </span>
          ) : null}
        </>
      }
    >
      <span
        role="status"
        aria-live="polite"
        className="inline-flex min-h-7 items-center gap-1 text-[12.5px] text-fg-muted"
      >
        {listening ? (
          keys.length > 0 ? (
            <Keycaps combo={`${keys.join("+")}+…`} />
          ) : (
            t("social.hotkeyListening")
          )
        ) : combo ? (
          <Keycaps combo={combo} />
        ) : (
          t("social.hotkeyOff")
        )}
      </span>
      {listening ? (
        <Button size="sm" onClick={() => setListening(false)}>
          {t("common.cancel")}
        </Button>
      ) : (
        <>
          <Button size="sm" icon={Keyboard} onClick={start}>
            {combo ? t("social.hotkeyChange") : t("social.hotkeyRecord")}
          </Button>
          {combo && (
            <IconButton icon={X} label={t("social.hotkeyClear")} onClick={() => save(null)} />
          )}
        </>
      )}
    </Row>
  );
}

/** Settings › 通用: the shortcut that shows and hides winer. */
export function HotkeyRow() {
  const t = useT();
  const store = useStore();
  const settings = useSettings();
  const hotkey = settings.general.hotkey;
  const status = useHotkeyStatus();
  // What the system said about the combination the settings name now, once it has said.
  const settled = status && !status.suspended && status.shortcut === hotkey ? status : null;
  return (
    <ShortcutRow
      label={t("social.hotkey")}
      hint={t("social.hotkeyHint")}
      combo={hotkey}
      held={settled}
      taken={[
        { combo: settings.automation.callout.hotkey, message: t("callout.hotkeyTakenByCallout") },
        ...comebackTaken(settings, t),
      ]}
      onSave={(combo) =>
        store.updateSettings((value) => ({
          ...value,
          general: { ...value.general, hotkey: combo },
        }))
      }
    />
  );
}

// Callout: the second shortcut, on 自动化 › 战力喊话.

/** 自动化 › 战力喊话: the shortcut that sends the callout, off until a combination is set. */
export function CalloutHotkeyRow() {
  const t = useT();
  const store = useStore();
  const settings = useSettings();
  const combo = settings.automation.callout.hotkey;
  const status = useHotkeyStatus();
  const settled =
    status && !status.suspended && status.callout.shortcut === combo ? status.callout : null;
  return (
    <ShortcutRow
      label={t("callout.hotkey")}
      hint={t("callout.hotkeyHint")}
      combo={combo}
      held={settled}
      taken={[
        { combo: settings.general.hotkey, message: t("callout.hotkeyTakenByWindow") },
        ...comebackTaken(settings, t),
      ]}
      onSave={(hotkey) =>
        store.updateSettings((value) => ({
          ...value,
          automation: {
            ...value.automation,
            callout: { ...value.automation.callout, hotkey },
          },
        }))
      }
    />
  );
}

// The Comebacks page's two shortcuts, beside the window's and the callout's.

/** The comebacks' combinations, which neither the window's nor the callout's shortcut takes. */
function comebackTaken(settings: Settings, t: Translate): Taken[] {
  return [
    { combo: settings.comeback.hotkey, message: t("comeback.hotkeyTakenByTeam") },
    { combo: settings.comeback.allHotkey, message: t("comeback.hotkeyTakenByAll") },
  ];
}

/** The Comebacks page: the shortcut that sends a line to the team, or with `all` to everyone in
 *  the game; off until a combination is set. */
export function ComebackHotkeyRow({ all }: { all: boolean }) {
  const t = useT();
  const store = useStore();
  const settings = useSettings();
  const combo = all ? settings.comeback.allHotkey : settings.comeback.hotkey;
  const status = useHotkeyStatus();
  const slot = status && (all ? status.comebackAll : status.comeback);
  const settled = status && !status.suspended && slot?.shortcut === combo ? slot : null;
  return (
    <ShortcutRow
      label={t(all ? "comeback.allHotkey" : "comeback.hotkey")}
      hint={t(all ? "comeback.allHotkeyHint" : "comeback.hotkeyHint")}
      combo={combo}
      held={settled}
      taken={[
        { combo: settings.general.hotkey, message: t("callout.hotkeyTakenByWindow") },
        { combo: settings.automation.callout.hotkey, message: t("callout.hotkeyTakenByCallout") },
        all
          ? { combo: settings.comeback.hotkey, message: t("comeback.hotkeyTakenByTeam") }
          : { combo: settings.comeback.allHotkey, message: t("comeback.hotkeyTakenByAll") },
      ]}
      onSave={(next) =>
        store.updateSettings((value) => ({
          ...value,
          comeback: all
            ? { ...value.comeback, allHotkey: next }
            : { ...value.comeback, hotkey: next },
        }))
      }
    />
  );
}
