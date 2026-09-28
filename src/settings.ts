import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { action, element, watch, type Settings } from "./shared";
async function refresh() {
  const s = await invoke<Settings>("get_settings");
  element<HTMLInputElement>("work").value = String(s.work_minutes);
  element<HTMLInputElement>("rest").value = String(s.break_minutes);
  element<HTMLInputElement>("sound").checked = s.sound;
  element<HTMLInputElement>("autostart").checked = s.autostart;
  element("message").textContent = ""; element("error").textContent = "";
}
element<HTMLFormElement>("form").onsubmit = async e => {
  e.preventDefault();
  const settings: Settings = { work_minutes: Number(element<HTMLInputElement>("work").value), break_minutes: Number(element<HTMLInputElement>("rest").value), sound: element<HTMLInputElement>("sound").checked, autostart: element<HTMLInputElement>("autostart").checked };
  if (await action("save_settings", { settings })) element("message").textContent = "保存しました。";
};
async function init() {
  await listen("settings_open", () => { void refresh().catch(e => { element("error").textContent = String(e); }); });
  await refresh();
  await watch(t => { element<HTMLFieldSetElement>("fields").disabled = t.mode === "break"; });
}
init().catch(e => { element("error").textContent = String(e); });
