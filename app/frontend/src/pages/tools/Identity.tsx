// The profile icon and its border, each one click from what a new account has, with an undo: the
// default icon, and the plainest border (the client draws none without one).
import type { Crest, CrestChange, IconChange } from "@winer/shared";
import { RotateCcw } from "lucide-react";
import { useState } from "react";

import { errorMessage } from "../../lib/backend";
import { useT } from "../../lib/i18n";
import { useLive, useStore } from "../../lib/store";
import { Button, Panel, Row, toast } from "../../ui";
import { Art } from "./Art";

const DEFAULT_ICON = 29;
const PLAIN_CREST: Crest = { kind: "prestige", level: 1 };

const sameCrest = (a: Crest, b: Crest) => a.kind === b.kind && a.level === b.level;

export function IdentityPanel() {
  const t = useT();
  const store = useStore();
  const iconId = useLive((snapshot) => snapshot.me?.iconId ?? 0);
  /** The last change each button made, for its undo. */
  const [icon, setIcon] = useState<IconChange | null>(null);
  const [crest, setCrest] = useState<CrestChange | null>(null);
  const [busy, setBusy] = useState<"icon" | "crest" | null>(null);

  /** Runs one change; `took` says whether the client shows what was asked for. */
  const run = async <T,>(
    key: "icon" | "crest",
    change: () => Promise<T>,
    took: (answer: T) => boolean,
    done: string,
    keep: (answer: T | null) => void,
  ) => {
    setBusy(key);
    try {
      const answer = await change();
      if (took(answer)) {
        toast(done, "ok");
        keep(answer);
      } else {
        toast(t("identity.refused"), "danger");
        keep(null);
      }
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(null);
    }
  };

  const defaultIcon = () =>
    run(
      "icon",
      () => store.backend.call("use_default_icon"),
      (answer) => answer.current === DEFAULT_ICON,
      t("identity.iconDone"),
      (answer) => setIcon(answer && answer.previous !== answer.current ? answer : null),
    );
  const undoIcon = (previous: number) =>
    run(
      "icon",
      () => store.backend.call("set_icon", { icon: previous }),
      (answer) => answer.current === previous,
      t("identity.undone"),
      () => setIcon(null),
    );
  const plainCrest = () =>
    run(
      "crest",
      () => store.backend.call("use_plain_crest"),
      (answer) => sameCrest(answer.current, PLAIN_CREST),
      t("identity.crestDone"),
      (answer) => setCrest(answer && !sameCrest(answer.previous, answer.current) ? answer : null),
    );
  const undoCrest = (previous: Crest) =>
    run(
      "crest",
      () => store.backend.call("set_crest", { crest: previous }),
      (answer) => sameCrest(answer.current, previous),
      t("identity.undone"),
      () => setCrest(null),
    );

  return (
    <Panel eyebrow={t("identity.title")} className="col-span-12 @[900px]:col-span-6">
      <Row label={t("identity.icon")} help={t("identity.iconHint")}>
        <div className="flex items-center gap-1.5">
          {iconId > 0 && (
            <Art
              path={`/lol-game-data/assets/v1/profile-icons/${iconId}.jpg`}
              alt=""
              className="size-7 rounded-full"
            />
          )}
          {icon && (
            <Button
              size="sm"
              variant="ghost"
              icon={RotateCcw}
              loading={busy === "icon"}
              onClick={() => void undoIcon(icon.previous)}
            >
              {t("identity.undo")}
            </Button>
          )}
          <Button
            size="sm"
            loading={busy === "icon" && !icon}
            disabled={busy !== null}
            onClick={() => void defaultIcon()}
          >
            {t("identity.useIcon")}
          </Button>
        </div>
      </Row>
      <Row label={t("identity.crest")} help={t("identity.crestHint")}>
        <div className="flex items-center gap-1.5">
          {crest && (
            <Button
              size="sm"
              variant="ghost"
              icon={RotateCcw}
              loading={busy === "crest"}
              onClick={() => void undoCrest(crest.previous)}
            >
              {t("identity.undo")}
            </Button>
          )}
          <Button
            size="sm"
            loading={busy === "crest" && !crest}
            disabled={busy !== null}
            onClick={() => void plainCrest()}
          >
            {t("identity.useCrest")}
          </Button>
        </div>
      </Row>
    </Panel>
  );
}
