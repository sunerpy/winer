import type { Language } from "@winer/shared";
import { useCallback } from "react";

import { useObservable } from "../observable";
import { useStore } from "../store";
import { en } from "./en";
import { type MessageKey, zhCN } from "./zh-CN";

export type { MessageKey };
export type Translate = (key: MessageKey, params?: Record<string, string | number>) => string;

const CATALOGS: Record<Language, Record<MessageKey, string>> = { "zh-CN": zhCN, en };

export function translate(
  language: Language,
  key: MessageKey,
  params?: Record<string, string | number>,
): string {
  const template = CATALOGS[language][key];
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in params ? String(params[name]) : match,
  );
}

export function useLanguage(): Language {
  return useObservable(useStore().settings, (settings) => settings?.general.language ?? "zh-CN");
}

export function useT(): Translate {
  const language = useLanguage();
  return useCallback((key, params) => translate(language, key, params), [language]);
}
