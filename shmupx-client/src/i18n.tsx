import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { translate, type Language } from "./i18nMessages";
import { readPreferences } from "./components/settingsState";

const I18nContext = createContext({ language: "pt-BR" as Language, locale: "pt-BR", setLanguage: (_language: Language) => {}, t: (text: string, values?: Record<string, string | number>) => translate("pt-BR", text, values) });
export function I18nProvider({ children }: { children: ReactNode }) {
  const [language, setLanguage] = useState<Language>(() => readPreferences().language);
  const t = useCallback((text: string, values?: Record<string, string | number>) => translate(language, text, values), [language]);
  useEffect(() => { document.documentElement.lang = language; }, [language]);
  return <I18nContext.Provider value={{ language, locale: language === "en" ? "en-US" : "pt-BR", setLanguage, t }}>{children}</I18nContext.Provider>;
}
export function useI18n() { return useContext(I18nContext); }
