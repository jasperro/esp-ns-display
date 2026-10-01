document.addEventListener("DOMContentLoaded", () => {
  const canvas = document.getElementById("oledCanvas");
  const ctx = canvas.getContext("2d");
  const imgData = ctx.createImageData(128, 64);
  const status = document.getElementById("status");
  const form = document.getElementById("settings-form");
  const statusMsg = document.getElementById("status-msg");
  const frameBytes = new Uint8Array(1024);

  // SSE Stream for OLED Mirror
  function connectStream() {
    const es = new EventSource("/api/display/stream");

    es.onopen = () => {
      status.textContent = "Live";
    };
    es.onerror = () => {
      status.textContent = "Reconnecting...";
    };

    let hexAccumulator = "";

    es.addEventListener("frame", (e) => {
      hexAccumulator += e.data ? e.data.trim() : "";
      if (hexAccumulator.length < 2048) return;

      const hex = hexAccumulator.slice(0, 2048);
      hexAccumulator = hexAccumulator.slice(2048);

      for (let i = 0; i < 1024; i++) {
        frameBytes[i] = parseInt(hex.substring(i * 2, i * 2 + 2), 16);
      }

      for (let y = 0; y < 64; y++) {
        const rowOffset = y * 16;
        for (let x = 0; x < 128; x++) {
          const bitOn =
            (frameBytes[rowOffset + (x >> 3)] & (1 << (7 - (x & 7)))) !== 0;
          const idx = (y * 128 + x) * 4;

          imgData.data[idx] = bitOn ? 255 : 15;
          imgData.data[idx + 1] = bitOn ? 255 : 23;
          imgData.data[idx + 2] = bitOn ? 255 : 42;
          imgData.data[idx + 3] = 255;
        }
      }
      ctx.putImageData(imgData, 0, 0);
    });
  }

  // Load API Settings
  async function loadSettings() {
    try {
      const res = await fetch("/api/settings");
      if (res.ok) {
        const d = await res.json();
        document.getElementById("ns_station_code").value =
          d.ns_station_code || "";
        document.getElementById("ns_api_key").value = d.ns_api_key || "";
        document.getElementById("screen_dwell_ticks").value =
          d.screen_dwell_ticks;
        document.getElementById("toggle_phase_ticks").value =
          d.toggle_phase_ticks;
        document.getElementById("scroll_pause_ticks").value =
          d.scroll_pause_ticks;
      }
    } catch {
      statusMsg.textContent = "Failed to load settings.";
    }
  }

  // Save Settings
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    statusMsg.textContent = "Saving...";

    const payload = {
      ns_station_code: document.getElementById("ns_station_code").value.trim(),
      ns_api_key: document.getElementById("ns_api_key").value.trim(),
      screen_dwell_ticks: parseInt(
        document.getElementById("screen_dwell_ticks").value,
        10,
      ),
      toggle_phase_ticks: parseInt(
        document.getElementById("toggle_phase_ticks").value,
        10,
      ),
      scroll_pause_ticks: parseInt(
        document.getElementById("scroll_pause_ticks").value,
        10,
      ),
    };

    try {
      const res = await fetch("/api/settings", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload),
      });

      statusMsg.textContent = res.ok
        ? "Saved successfully!"
        : "Error saving settings.";
    } catch {
      statusMsg.textContent = "Network error.";
    }
  });

  connectStream();
  loadSettings();
});
