import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
export interface Tick { mode: "work" | "break"; remaining: number; paused: boolean; blocked: boolean; emergency_remaining: number | null; break_minutes: number; test_mode: boolean; warning: string }
export interface Settings { work_minutes: number; break_minutes: number; sound: boolean; autostart: boolean }
export const element = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
export function formatTime(seconds: number, hours = false): string {
  const s = Math.max(0, Math.ceil(seconds));
  const parts = [String(Math.floor(s / 60) % 60).padStart(2, "0"), String(s % 60).padStart(2, "0")];
  if (hours || s >= 3600) parts.unshift(String(Math.floor(s / 3600)).padStart(2, "0"));
  return parts.join(":");
}
export async function action(command: string, args?: Record<string, unknown>): Promise<boolean> {
  element("error").textContent = "";
  try { await invoke(command, args); return true; }
  catch (e) { element("error").textContent = String(e); return false; }
}
export async function watch(render: (tick: Tick) => void) {
  // Register before the initial snapshot, so a hidden webview cannot miss a transition.
  await listen<Tick>("tick", ({ payload }) => render(payload));
  render(await invoke<Tick>("timer_info"));
}
