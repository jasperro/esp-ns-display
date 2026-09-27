document.addEventListener("DOMContentLoaded", async () => {
  const form = document.getElementById("settings-form");
  const status = document.getElementById("status-msg");

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
      }
    } catch (err) {
      status.textContent = "Failed to load current settings.";
    }
  }

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    status.textContent = "Saving...";
    status.className = "status";

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
        status.textContent = "Settings updated successfully!";
        status.className = "status success";
      } else {
        status.textContent = "Error saving settings.";
      }
    } catch (err) {
      status.textContent = "Network error while saving settings.";
    }
  });

  loadSettings();
});
