import { action, element, formatTime, watch } from "./shared";
element("pause").onclick = () => { void action("toggle_pause"); };
element("reset").onclick = () => { void action("reset_timer"); };
element("settings").onclick = () => { void action("open_settings"); };
watch(t => {
  element("time").textContent = formatTime(t.remaining, true);
  element("mode").textContent = t.mode === "break" ? "休憩中" : t.paused ? "一時停止中" : t.blocked ? "離席中" : "作業中";
  element("caption").textContent = t.mode === "break" ? "休憩終了まで" : "次の休憩まで";
  element("pause").textContent = t.paused ? "再開" : "一時停止";
  for (const id of ["pause", "reset", "settings"]) element<HTMLButtonElement>(id).disabled = t.mode === "break";
  element("testMode").hidden = !t.test_mode;
  if (t.warning) element("error").textContent = t.warning;
}).catch(e => { element("error").textContent = String(e); });
