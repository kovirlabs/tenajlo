import { commands, type Identity, type Settings } from "../bindings";
import type { Result } from "./result";

export function getSettings(): Promise<Settings> {
  return commands.getSettings();
}

/** Validates and saves; `data` is what was stored. */
export function saveSettings(settings: Settings): Promise<Result<Settings>> {
  return commands.saveSettings(settings);
}

/** Native folder picker; null if cancelled. */
export function chooseFolder(title: string): Promise<string | null> {
  return commands.chooseFolder(title);
}

/** Native file picker; null if cancelled. */
export function chooseFile(title: string): Promise<string | null> {
  return commands.chooseFile(title);
}

export function getGlobalIdentity(): Promise<Result<Identity>> {
  return commands.getGlobalIdentity();
}
