// Champ select's quick phrases (选人快捷语): at the end of a teammate's row on the board, a menu of
// the phrases as they would read for that teammate; choosing one posts it to champ select's chat.
import { riotId, type Seat } from "@winer/shared";
import { MessageSquareQuote } from "lucide-react";
import { useRef, useState } from "react";

import { errorMessage } from "../../lib/backend";
import { fillPhrase, phrasesOf, seatLabel } from "../../lib/comeback";
import { useT } from "../../lib/i18n";
import { useCatalog, useSettings, useStore } from "../../lib/store";
import { useShell } from "../../shell/navigation";
import { Button, IconButton, Popover, toast } from "../../ui";

/** The menu for the teammate in cell `cellId`, `number` in the team's list, of champ select
 *  `gameId`: the core sends a phrase only while that champ select is the one held, so a menu left
 *  open from an earlier one sends nothing. */
export function PhraseMenu({
  seat,
  number,
  gameId,
  cellId,
}: {
  seat: Seat;
  number: number;
  gameId: number;
  cellId: number;
}) {
  const t = useT();
  const store = useStore();
  const catalog = useCatalog();
  const { navigate } = useShell();
  const settings = useSettings();
  const language = settings.general.language;
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const anchor = useRef<HTMLDivElement>(null);
  // As the core fills them for this seat: its place, the champion's short name, the game name.
  const place = seatLabel(number, language);
  const champion = catalog?.champions.get(seat.championId)?.shortName ?? "";
  const who = riotId(seat.name) || place;

  const send = async (index: number) => {
    setOpen(false);
    setBusy(true);
    try {
      const text = await store.backend.call("send_phrase", { index, gameId, cellId });
      toast(t("live.phraseSent", { text }), "ok");
    } catch (error) {
      toast(errorMessage(error), "danger");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div ref={anchor}>
      <IconButton
        icon={MessageSquareQuote}
        label={t("live.phraseFor", { name: who })}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={busy}
        onClick={() => setOpen((value) => !value)}
      />
      <Popover
        open={open}
        onClose={() => setOpen(false)}
        anchor={anchor}
        placement="bottom-end"
        className="w-[300px] p-1"
        label={t("live.phrases")}
      >
        <div role="menu" aria-label={t("live.phrases")} className="flex flex-col">
          {phrasesOf(settings.comeback, language).map((phrase, index) => (
            <button
              key={index}
              type="button"
              role="menuitem"
              onClick={() => void send(index)}
              className="rounded-6 px-2.5 py-1.5 text-left text-[12.5px] leading-5 break-words text-fg hover-wash"
            >
              {fillPhrase(phrase, place, champion, seat.name?.gameName ?? "")}
            </button>
          ))}
        </div>
        <div className="mt-1 border-t border-border px-2.5 pt-1">
          <Button
            size="sm"
            variant="link"
            onClick={() => {
              setOpen(false);
              navigate({ page: "phrases" });
            }}
          >
            {t("live.phrasesEdit")}
          </Button>
        </div>
      </Popover>
    </div>
  );
}
