// Settings › 通用: the global shortcut that brings winer up from anywhere. The recorder takes the
// window's next keydown before anything else does (capture phase: the shell's own shortcuts and the
// dialog's Escape never see it), and the shell lets the old shortcut go while it listens, or
// pressing it again would hide the window instead of being recorded.
import { Keyboard, X } from "lucide-react";
import { useEffect, useState } from "react";

import { errorMessage } from "../../lib/backend";
import { keycaps, record } from "../../lib/hotkey";
import { useT } from "../../lib/i18n";
import { useHotkeyStatus, useSettings, useStore } from "../../lib/store";
import { Button, IconButton, Kbd, Lamp, Row, toast } from "../../ui";

function Keycaps({ combo }: { combo: string }) {
  return (
    <span className="inline-flex items-center gap-1">
      {keycaps(combo).map((key, index) => (
        <Kbd key={`${key}-${index}`}>{key}</Kbd>
      ))}
    </span>
  );
}

export function HotkeyRow() {
  const t = useT();
  const store = useStore();
  const hotkey = useSettings().general.hotkey;
  const status = useHotkeyStatus();
  const [listening, setListening] = useState(false);
  const [held, setHeld] = useState<string[]>([]);
  const [invalid, setInvalid] = useState(false);

  const save = (combo: string | null) =>
    store
      .updateSettings((settings) => ({
        ...settings,
        general: { ...settings.general, hotkey: combo },
      }))
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
        setHeld(result.keys);
        setInvalid(false);
      } else if (result.kind === "invalid") {
        setHeld([]);
        setInvalid(true);
      } else {
        setListening(false);
        void store
          .updateSettings((settings) => ({
            ...settings,
            general: { ...settings.general, hotkey: result.combo },
          }))
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
      // Done, cancelled or the settings closed: the shortcut the settings name comes back.
      suspend(false);
    };
  }, [listening, store]);

  const start = () => {
    setHeld([]);
    setInvalid(false);
    setListening(true);
  };
  // What the system said about the combination the settings name now, once it has said.
  const settled = status && !status.suspended && status.shortcut === hotkey ? status : null;

  return (
    <Row
      label={t("social.hotkey")}
      help={
        <>
          {t("social.hotkeyHint")}
          {listening ? (
            <span className="mt-1 block text-fg-subtle">
              {invalid ? (
                <span className="text-danger">{t("social.hotkeyInvalid")}</span>
              ) : (
                t("social.hotkeyListeningHint")
              )}
            </span>
          ) : settled?.error ? (
            <span className="mt-1 block text-danger" title={settled.error}>
              {t("social.hotkeyFailed")}
            </span>
          ) : settled?.active ? (
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
          held.length > 0 ? (
            <Keycaps combo={`${held.join("+")}+…`} />
          ) : (
            t("social.hotkeyListening")
          )
        ) : hotkey ? (
          <Keycaps combo={hotkey} />
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
            {hotkey ? t("social.hotkeyChange") : t("social.hotkeyRecord")}
          </Button>
          {hotkey && (
            <IconButton icon={X} label={t("social.hotkeyClear")} onClick={() => void save(null)} />
          )}
        </>
      )}
    </Row>
  );
}
