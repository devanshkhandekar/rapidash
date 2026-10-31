let ws = null;

function connectWebSocket() {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
    return;
  }

  ws = new WebSocket("ws://127.0.0.1:17421");

  ws.onopen = () => {
    sendFullChromeState();
  };

  ws.onmessage = async (event) => {
    try {
      const cmd = JSON.parse(event.data);

      if (cmd.action === "DiscardTab" && cmd.tab_id) {
        await chrome.tabs.discard(cmd.tab_id);
      } else if (cmd.action === "CloseTab" && cmd.tab_id) {
        await chrome.tabs.remove(cmd.tab_id);
      } else if (cmd.action === "FocusTab" && cmd.tab_id && cmd.window_id) {
        await chrome.windows.update(cmd.window_id, { focused: true });
        await chrome.tabs.update(cmd.tab_id, { active: true });
      } else if (cmd.action === "HibernateWindow" && cmd.window_id) {
        const tabs = await chrome.tabs.query({ windowId: cmd.window_id });
        for (const t of tabs) {
          if (!t.active && !t.discarded) {
            try { await chrome.tabs.discard(t.id); } catch (_) {}
          }
        }
      } else if (cmd.action === "ToggleExtension" && cmd.ext_id) {
        await chrome.management.setEnabled(cmd.ext_id, cmd.enabled);
      }

      setTimeout(sendFullChromeState, 150);
    } catch (err) {
      console.error("SysGuard command error:", err);
    }
  };

  ws.onclose = () => {
    ws = null;
  };
}

async function sendFullChromeState() {
  if (!ws || ws.readyState !== WebSocket.OPEN) return;

  try {
    const windows = await chrome.windows.getAll({ populate: true, windowTypes: ["normal"] });
    const extensions = await chrome.management.getAll();

    const payload = {
      windows: windows.map((w, idx) => ({
        window_id: w.id,
        window_number: idx + 1,
        focused: w.focused,
        tabs: (w.tabs || []).map((t) => ({
          tab_id: t.id,
          window_id: w.id,
          title: t.title || "Untitled Tab",
          url: t.url || "",
          active: Boolean(t.active),
          discarded: Boolean(t.discarded),
          audible: Boolean(t.audible)
        }))
      })),
      extensions: extensions
        .filter((e) => e.type === "extension")
        .map((e) => ({
          id: e.id,
          name: e.name,
          enabled: Boolean(e.enabled)
        }))
    };

    ws.send(JSON.stringify(payload));
  } catch (err) {
    console.error("SysGuard state error:", err);
  }
}

// Keep service worker alive and push updates every 2 seconds + on tab changes
chrome.alarms.create("sysguard_keepalive", { periodInMinutes: 0.25 });
chrome.alarms.onAlarm.addListener(() => {
  connectWebSocket();
  sendFullChromeState();
});

setInterval(() => {
  connectWebSocket();
  sendFullChromeState();
}, 2000);

chrome.tabs.onUpdated.addListener(() => sendFullChromeState());
chrome.tabs.onRemoved.addListener(() => sendFullChromeState());
chrome.tabs.onActivated.addListener(() => sendFullChromeState());

connectWebSocket();