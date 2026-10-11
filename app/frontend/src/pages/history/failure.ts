import { errorCode, errorMessage } from "../../lib/backend";
import type { Translate } from "../../lib/i18n";

/** A history request that failed, as a note says it: the shard's server being too busy in plain
 *  words, with what helps (asking again later; the client's own history fails meanwhile), anything
 *  else under `title` with the message it came with. */
export function historyFailure(
  error: unknown,
  t: Translate,
  title = t("common.loadFailed"),
): { title: string; detail: string } {
  return errorCode(error) === "unavailable"
    ? { title: t("history.serverBusy"), detail: t("history.serverBusyHint") }
    : { title, detail: errorMessage(error) };
}
