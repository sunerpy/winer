// The chat statuses the window names and the ones the client takes from winer.

/** The statuses the client takes from winer, in the order the switch offers them. */
export const AVAILABILITIES = ["chat", "away", "mobile", "offline"] as const;
export type Availability = (typeof AVAILABILITIES)[number];

export function isAvailability(value: string | null | undefined): value is Availability {
  return (AVAILABILITIES as readonly (string | null | undefined)[]).includes(value);
}

/** The status message the mobile state gets with its switch on (`PresenceRule::MOBILE_MESSAGE`):
 *  the Tencent client names that state 在线分组, and friends read a message as written. */
export const MOBILE_MESSAGE = "手机在线";

/** The statuses that have a name in the window: the client's own `dnd` as well. */
const NAMED_STATUSES = ["chat", "away", "dnd", "mobile", "offline"] as const;
export type NamedStatus = (typeof NAMED_STATUSES)[number];

export function namedStatus(value: string): NamedStatus | null {
  return (NAMED_STATUSES as readonly string[]).includes(value) ? (value as NamedStatus) : null;
}
