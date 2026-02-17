export type SettingsSection = "connection" | "providers" | "preferences" | "about";

export const SETTINGS_SECTIONS: {
  key: SettingsSection;
  label: string;
  icon: string;
}[] = [
  { key: "connection", label: "Connection", icon: "Wifi" },
  { key: "providers", label: "Providers", icon: "KeyRound" },
  { key: "preferences", label: "Preferences", icon: "SlidersHorizontal" },
  { key: "about", label: "About", icon: "Info" },
];
