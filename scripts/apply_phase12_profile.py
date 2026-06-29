from pathlib import Path

path = Path("src/app/controller/youtube.rs")
text = path.read_text(encoding="utf-8")
text = text.replace("bridge.status()", "bridge.status_with_profile()", 1)
text = text.replace("bridge.connect(&raw)", "bridge.connect_with_profile(&raw)", 1)
text = text.replace("bridge.disconnect()", "bridge.disconnect_with_profile()", 1)
path.write_text(text, encoding="utf-8")
