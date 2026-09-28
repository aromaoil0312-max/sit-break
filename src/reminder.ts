import { action, element, formatTime, watch } from "./shared";
element("request").onclick = () => { void action("request_emergency"); };
element("confirm").onclick = () => { void action("confirm_emergency"); };
element("cancel").onclick = () => { void action("cancel_emergency"); };
watch(t => {
  element("time").textContent = formatTime(t.remaining);
  element("instruction").textContent = t.test_mode ? "30秒間、立ってください。" : `${t.break_minutes}分間、立ってください。`;
  element("testMode").hidden = !t.test_mode;
  const waiting = t.emergency_remaining !== null;
  element("request").hidden = waiting;
  element("confirmation").hidden = !waiting;
  element<HTMLButtonElement>("confirm").disabled = t.emergency_remaining !== 0 || t.blocked;
  element("wait").textContent = t.emergency_remaining === 0 ? "緊急時のみ「解除する」を押してください。" : `解除まで${t.emergency_remaining ?? 30}秒お待ちください。`;
  if (t.mode !== "break") element("error").textContent = "";
}).catch(e => { element("error").textContent = String(e); });
