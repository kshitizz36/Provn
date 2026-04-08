"""
Export fine-tuned Gemma 4 E2B model from Modal volume to local GGUF.
Run after: modal run modal_finetune.py
"""

import modal
import pathlib

app = modal.App("provn-export")
vol = modal.Volume.from_name("aegis-model-vol", create_if_missing=False)

OUTPUT_DIR = pathlib.Path.home() / ".provn" / "models"


@app.function(volumes={"/model": vol}, timeout=300)
def export():
    import os
    import glob

    files = glob.glob("/model/**/*.gguf", recursive=True)
    if not files:
        print("No GGUF files found in volume. Run modal_finetune.py first.")
        return None

    gguf_path = files[0]
    print(f"Found GGUF: {gguf_path}")
    with open(gguf_path, "rb") as f:
        data = f.read()
    return data


@app.local_entrypoint()
def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    out_path = OUTPUT_DIR / "provn-gemma4-e2b-q4km.gguf"

    print("Downloading GGUF from Modal volume...")
    data = export.remote()

    if data is None:
        print("Export failed — no GGUF available.")
        return

    out_path.write_bytes(data)
    size_mb = len(data) / 1024 / 1024
    print(f"Saved {size_mb:.1f} MB → {out_path}")
    print("Layer 3 ready. Run 'provn scan' with semantic enabled in provn.yml.")
