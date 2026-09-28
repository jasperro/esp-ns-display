document.addEventListener("DOMContentLoaded", () => {
  // --- OLED Canvas Setup ---
  const canvas = document.getElementById("oledCanvas");
  const ctx = canvas.getContext("2d");
  const imgData = ctx.createImageData(128, 64);

  const statusBadge = document.getElementById("statusBadge");
  const statusText = document.getElementById("statusText");

  const frameBytes = new Uint8Array(1024);

  // Initialize display background (Dark slate)
  for (let i = 0; i < imgData.data.length; i += 4) {
    imgData.data[i] = 15; // Red
    imgData.data[i + 1] = 23; // Green
    imgData.data[i + 2] = 42; // Blue
    imgData.data[i + 3] = 255; // Alpha
  }
  ctx.putImageData(imgData, 0, 0);

  // --- SSE Display Stream ---
  function connectStream() {
    const eventSource = new EventSource("/api/display/stream");

    eventSource.onopen = () => {
      statusBadge.className = "status-badge status-connected";
      statusText.textContent = "Live";
    };

    let hexAccumulator = "";

    eventSource.addEventListener("frame", (event) => {
      const chunk = event.data ? event.data.trim() : "";
      hexAccumulator += chunk;

      // Wait until full 2048-char hex frame is accumulated
      if (hexAccumulator.length < 2048) return;

      const hex = hexAccumulator.substring(0, 2048);
      hexAccumulator = hexAccumulator.substring(2048); // keep overflow if any

      for (let i = 0; i < 1024; i++) {
        frameBytes[i] = parseInt(hex.substring(i * 2, i * 2 + 2), 16);
      }

      for (let y = 0; y < 64; y++) {
        const rowOffset = y * 16;
        for (let x = 0; x < 128; x++) {
          const byteIndex = rowOffset + (x >> 3);
          const bitIndex = 7 - (x & 7);
          const isPixelOn = (frameBytes[byteIndex] & (1 << bitIndex)) !== 0;

          const pixelOffset = (y * 128 + x) * 4;

          if (isPixelOn) {
            imgData.data[pixelOffset] = 240;
            imgData.data[pixelOffset + 1] = 248;
            imgData.data[pixelOffset + 2] = 255;
          } else {
            imgData.data[pixelOffset] = 15;
            imgData.data[pixelOffset + 1] = 23;
            imgData.data[pixelOffset + 2] = 42;
          }
        }
      }

      ctx.putImageData(imgData, 0, 0);
    });

    eventSource.onerror = () => {
      statusBadge.className = "status-badge status-disconnected";
      statusText.textContent = "Reconnecting...";
    };
  }

  // --- Settings Form ---
  const form = document.getElementById("settings-form");
  const statusMsg = document.getElementById("status-msg");

  async function loadSettings() {
    try {
      const res = await fetch("/api/settings");
      if (res.ok) {
        const data = await res.json();
        document.getElementById("screen_dwell_ticks").value =
          data.screen_dwell_ticks;
        document.getElementById("toggle_phase_ticks").value =
          data.toggle_phase_ticks;
        document.getElementById("scroll_pause_ticks").value =
          data.scroll_pause_ticks;
      } else {
        statusMsg.textContent = "Failed to load current settings.";
      }
    } catch (err) {
      statusMsg.textContent = "Error connecting to settings API.";
    }
  }

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    statusMsg.textContent = "Saving...";
    statusMsg.className = "status";

    const payload = {
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

      if (res.ok) {
        statusMsg.textContent = "Settings updated successfully!";
        statusMsg.className = "status success";
      } else {
        statusMsg.textContent = "Error saving settings.";
      }
    } catch (err) {
      statusMsg.textContent = "Network error while saving settings.";
    }
  });

  connectStream();
  loadSettings();
});
