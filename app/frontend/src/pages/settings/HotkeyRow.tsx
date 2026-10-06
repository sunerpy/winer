// The global shortcuts as settings rows: Settings › 通用's, which brings winer up from anywhere, and
// 自动化 › 战力喊话's, which sends the callout. The recorder takes the window's next keydown before
// anything else does (capture phase: the shell's own shortcuts and the dialog's Escape never see
// it), and the shell lets every shortcut go while it listens, or pressing one would act instead of
// being recorded.
import { Keyboard, X } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";

import { errorMessage } from "../../lib/backend";
import { keycaps, record } from "../../lib/hotkey";
import { useT } from "../../lib/i18n";
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

/** One shortcut: its keys, a recorder for a new combination and a ✕ that turns it off. */
function ShortcutRow({
  label,
  hint,
  combo,
  held,
  taken,
  takenMessage,
  onSave,
}: {
  label: string;
  hint: ReactNode;
  combo: string | null;
  /** What the system said about `combo`; `null` until it has. */
  held: Held | null;
  /** The other shortcut's combination, which this one cannot take. */
  taken: string | null;
  takenMessage: string;
  onSave: (combo: string | null) => Promise<void>;
}) {
  const t = useT();
  const store = useStore();
  const [listening, setListening] = useState(false);
  const [keys, setKeys] = useState<string[]>([]);
  const [problem, setProblem] = useState<"invalid" | "taken" | null>(null);
  // The listener lives as long as the recording; it reads the newest of these when a key comes.
  const latest = useRef({ taken, onSave });
  useEffect(() => {
    latest.current = { taken, onSave };
  });

  const save = (next: string | null) =>
    void latest.current
      .onSave(next)
      .catch((error: unknown) => toast(errorMessage(error), "danger"));

  useEffect(() => {
    if (!listening) return undefined;
    const suspend = (suspended: boolean) =>
      void store.backend
        .call("suspend_hotkey", { suspended })
        .then((next) => store.hotkey.set(next))
        .catch(() => undefined);
    suspend(true);
    const onKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopImmediatePropagation();
      const bare = !(event.ctrlKey || event.altKey || event.shiftKey || event.metaKey);
      if (event.key === "Escape" && bare) {
        setListening(false);
        return;
      }
      const result = record(event);
      if (result.kind === "partial") {
        setKeys(result.keys);
        setProblem(null);
      } else if (result.kind === "invalid") {
        setKeys([]);
        setProblem("invalid");
      } else if (result.combo === latest.current.taken) {
        setKeys([]);
        setProblem("taken");
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
      // Done, cancelled or the settings closed: the shortcuts the settings name come back.
      suspend(false);
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
                <span className="text-danger">
                  {problem === "taken" ? takenMessage : t("social.hotkeyInvalid")}
                </span>
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
      taken={settings.automation.callout.hotkey}
      takenMessage={t("callout.hotkeyTakenByCallout")}
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
      taken={settings.general.hotkey}
      takenMessage={t("callout.hotkeyTakenByWindow")}
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
