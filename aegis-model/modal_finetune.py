# provn-model/modal_finetune.py
# Fine-tune Gemma 4 E2B using Unsloth + FastLanguageModel on Modal H100
#
# Prerequisites:
#   pip install modal && modal setup
#   modal secret create huggingface-secret HF_TOKEN=hf_... --force
#
# Run:
#   modal run modal_finetune.py::main        # train + eval
#   modal run modal_finetune.py::main_eval   # eval only (after training)
#   modal run modal_finetune.py::main_retrain  # retrain + save merged + eval
#   modal run modal_finetune.py::main_gguf   # export GGUF (run after eval passes)

import json
import modal

app = modal.App("provn-finetune")

vol = modal.Volume.from_name("provn-model-vol", create_if_missing=True)
VOLUME_PATH = "/model"

finetune_image = (
    modal.Image.from_registry("pytorch/pytorch:2.5.1-cuda12.4-cudnn9-devel")
    .apt_install(
        "git", "build-essential", "cmake", "curl",
        "libcurl4-openssl-dev", "libssl-dev", "pciutils",
    )
    .run_commands(
        # Pre-build llama.cpp so Unsloth GGUF export works non-interactively
        "mkdir -p /root/.unsloth && git clone --depth=1 "
        "https://github.com/ggml-org/llama.cpp /root/.unsloth/llama.cpp",
        "cmake /root/.unsloth/llama.cpp -B /root/.unsloth/llama.cpp/build "
        "-DBUILD_SHARED_LIBS=OFF -DGGML_CUDA=OFF -DGGML_NATIVE=OFF",
        "cmake --build /root/.unsloth/llama.cpp/build --config Release -j "
        "--target llama-quantize llama-gguf-split",
        "cp /root/.unsloth/llama.cpp/build/bin/llama-* "
        "/root/.unsloth/llama.cpp/ 2>/dev/null || true",
    )
    .pip_install(
        "unsloth",
        "unsloth_zoo",
        extra_options="--upgrade --force-reinstall --no-cache-dir",
    )
    .pip_install(
        "datasets", "trl", "huggingface_hub", "hf_transfer",
        extra_options="--no-cache-dir",
    )
    .env({"HF_HUB_ENABLE_HF_TRANSFER": "1"})
)

BASE_MODEL = "unsloth/Gemma-4-E2B-it"
MERGED_PATH = f"{VOLUME_PATH}/provn-gemma4-e2b-merged"   # full merged weights
LORA_PATH   = f"{VOLUME_PATH}/provn-gemma4-e2b-lora"     # adapter only
GGUF_PATH   = f"{VOLUME_PATH}/provn-gemma4-e2b-gguf"


def _hf_login():
    import os
    from huggingface_hub import login
    try:
        login(token=os.environ["HF_TOKEN"])
        print("HF login successful.")
    except Exception as e:
        print(f"HF login warning (non-fatal): {e}")


@app.function(
    image=finetune_image,
    gpu="H100",
    timeout=7200,
    volumes={VOLUME_PATH: vol},
    secrets=[modal.Secret.from_name("huggingface-secret")],
)
def finetune():
    """Train on LeakBench, save LoRA + merged model + GGUF to volume."""
    from unsloth import FastLanguageModel
    from datasets import load_dataset
    from trl import SFTTrainer, SFTConfig

    _hf_login()

    train_data_path = f"{VOLUME_PATH}/data/leakbench_train_balanced.jsonl"

    print(f"Loading {BASE_MODEL} (bf16 LoRA)...")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=BASE_MODEL,
        max_seq_length=2048,
        load_in_4bit=False,
        load_in_16bit=True,
        full_finetuning=False,
    )

    model = FastLanguageModel.get_peft_model(
        model,
        r=64,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj",
                        "gate_proj", "up_proj", "down_proj"],
        lora_alpha=128,
        lora_dropout=0,      # 0 = fast-patchable by Unsloth
        bias="none",
        use_gradient_checkpointing="unsloth",
        random_state=3407,
    )

    dataset = load_dataset("json", data_files=train_data_path, split="train")

    SYSTEM = (
        "<|think|>\n"
        "You are a code security classifier. Analyze the code snippet and "
        "respond with exactly one word: 'leak' if it contains secrets, API keys, "
        "system prompts, or proprietary IP. 'clean' if it is safe."
    )

    def format_example(ex):
        messages = [
            {"role": "system",    "content": SYSTEM},
            {"role": "user",      "content": f"Classify this code:\n```\n{ex['code']}\n```"},
            {"role": "assistant", "content": ex["label"]},
        ]
        return {"text": tokenizer.apply_chat_template(messages, tokenize=False)}

    dataset = dataset.map(format_example)
    print(f"Training on {len(dataset)} examples...")

    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=dataset,
        args=SFTConfig(
            output_dir="/tmp/checkpoints",
            per_device_train_batch_size=16,
            gradient_accumulation_steps=2,
            num_train_epochs=10,
            learning_rate=2e-4,
            warmup_steps=20,
            lr_scheduler_type="cosine",
            logging_steps=5,
            save_strategy="no",
            bf16=True,
            fp16=False,
            optim="adamw_8bit",
            max_seq_length=2048,
            dataset_text_field="text",
            report_to="none",
            seed=3407,
        ),
    )
    trainer.train()

    # 1) Save LoRA adapter
    print(f"Saving LoRA adapter → {LORA_PATH}")
    model.save_pretrained(LORA_PATH)
    tokenizer.save_pretrained(LORA_PATH)

    # 2) Save merged model (base + LoRA baked in) — eval loads this, no HF download needed
    print(f"Saving merged model → {MERGED_PATH}")
    model.save_pretrained_merged(MERGED_PATH, tokenizer, save_method="merged_16bit")

    vol.commit()
    print("LoRA + merged model saved to volume.")

    # GGUF export is run separately via main_gguf to avoid corrupting the merged model
    return {"status": "success", "lora": LORA_PATH, "merged": MERGED_PATH}


@app.function(
    image=finetune_image,
    gpu="H100",
    timeout=1800,
    volumes={VOLUME_PATH: vol},
    secrets=[modal.Secret.from_name("huggingface-secret")],
)
def evaluate():
    """Evaluate on LeakBench. Loads merged model from volume — no HF download."""
    import time
    import torch
    from unsloth import FastLanguageModel
    from datasets import load_dataset

    _hf_login()

    # Force reload volume to get latest committed state from finetune container
    vol.reload()

    eval_data_path = f"{VOLUME_PATH}/data/leakbench_eval.jsonl"

    # Verify merged model safetensor is complete before loading
    import os, glob as _glob
    st_files = _glob.glob(f"{MERGED_PATH}/*.safetensors")
    print(f"Safetensor files in merged path: {st_files}")
    for st in st_files:
        size = os.path.getsize(st)
        print(f"  {os.path.basename(st)}: {size/1e9:.2f} GB")

    # Load from volume — no HF network call needed.
    # local_files_only=True also skips Unsloth's get_statistics() HF ping.
    print(f"Loading merged model from {MERGED_PATH}...")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=MERGED_PATH,
        max_seq_length=2048,
        load_in_4bit=False,
        load_in_16bit=True,
        local_files_only=True,
    )
    FastLanguageModel.for_inference(model)

    eval_data = load_dataset("json", data_files=eval_data_path, split="train")
    print(f"Evaluating on {len(eval_data)} cases...")

    # Gemma4Processor wraps a text tokenizer at .tokenizer; use it directly to avoid
    # the multimodal pipeline which chokes on plain-string message content.
    underlying_tok = getattr(tokenizer, "tokenizer", tokenizer)

    tp, fp, tn, fn = 0, 0, 0, 0
    latencies = []

    EVAL_SYSTEM = (
        "<|think|>\n"
        "You are a code security classifier. Analyze the code snippet and "
        "respond with exactly one word: 'leak' if it contains secrets, API keys, "
        "system prompts, or proprietary IP. 'clean' if it is safe."
    )

    for example in eval_data:
        messages = [
            {"role": "system", "content": EVAL_SYSTEM},
            {"role": "user",   "content": f"Classify this code:\n```\n{example['code']}\n```"},
        ]
        # Render chat template to string (bypasses multimodal processor),
        # then tokenize with the underlying text tokenizer.
        text = tokenizer.apply_chat_template(
            messages,
            add_generation_prompt=True,
            tokenize=False,
        )
        enc = underlying_tok(text, return_tensors="pt")
        input_ids = enc["input_ids"].to("cuda")
        attention_mask = enc["attention_mask"].to("cuda")

        t0 = time.time()
        with torch.no_grad():
            outputs = model.generate(
                input_ids=input_ids,
                attention_mask=attention_mask,
                max_new_tokens=4,
                do_sample=False,
                pad_token_id=underlying_tok.eos_token_id,
            )
        latencies.append((time.time() - t0) * 1000)

        new_tokens = outputs[0][input_ids.shape[1]:]
        response   = underlying_tok.decode(new_tokens, skip_special_tokens=True).strip().lower()
        # Parse: take first word that is exactly "leak" or "clean"; fallback to substring
        first_word = response.split()[0] if response.split() else ""
        if first_word in ("leak", "clean"):
            predicted = first_word
        else:
            predicted = "leak" if "leak" in response else "clean"
        actual     = example["label"]
        mark = "✓" if predicted == actual else "✗"
        if predicted != actual:
            print(f"  {mark} [{actual}→{predicted}] {repr(example['code'][:120])}")

        if predicted == "leak"  and actual == "leak":    tp += 1
        elif predicted == "leak"  and actual == "clean": fp += 1
        elif predicted == "clean" and actual == "clean": tn += 1
        else:                                            fn += 1

    recall    = tp / (tp + fn) if (tp + fn) > 0 else 0.0
    fpr       = fp / (fp + tn) if (fp + tn) > 0 else 0.0
    precision = tp / (tp + fp) if (tp + fp) > 0 else 0.0
    f1        = (2 * precision * recall / (precision + recall)
                 if (precision + recall) > 0 else 0.0)
    sorted_lat = sorted(latencies)
    p50 = sorted_lat[len(sorted_lat) // 2]
    p95 = sorted_lat[int(len(sorted_lat) * 0.95)]

    report = {
        "timestamp":        time.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "dataset":          "leakbench_eval",
        "total_cases":      len(eval_data),
        "tp": tp, "fp": fp, "tn": tn, "fn": fn,
        "recall":           round(recall, 4),
        "fpr":              round(fpr, 4),
        "precision":        round(precision, 4),
        "f1":               round(f1, 4),
        "latency_p50_ms":   round(p50, 1),
        "latency_p95_ms":   round(p95, 1),
    }

    with open(f"{VOLUME_PATH}/leakbench_report.json", "w") as f:
        json.dump(report, f, indent=2)
    vol.commit()

    print(json.dumps(report, indent=2))
    if report["recall"] < 0.97:
        print(f"WARNING: Recall {report['recall']:.1%} < 97% target")
    if report["fpr"] > 0.012:
        print(f"WARNING: FPR {report['fpr']:.1%} > 1.2% target")
    if report["latency_p95_ms"] > 200:
        print(f"WARNING: p95 {report['latency_p95_ms']}ms > 200ms target")

    return report


@app.local_entrypoint()
def main():
    import pathlib
    print("=== Provn Layer 3: Gemma 4 E2B fine-tuning ===\n")

    print("Uploading dataset to volume...")
    data_dir = pathlib.Path(__file__).parent / "data"
    with vol.batch_upload(force=True) as batch:
        batch.put_file(str(data_dir / "leakbench_train.jsonl"), "/data/leakbench_train.jsonl")
        batch.put_file(str(data_dir / "leakbench_train_balanced.jsonl"), "/data/leakbench_train_balanced.jsonl")
        batch.put_file(str(data_dir / "leakbench_eval.jsonl"), "/data/leakbench_eval.jsonl")
    print("Dataset uploaded.\n")

    print("Fine-tuning on H100...")
    result = finetune.remote()
    print(f"Training complete: {result}\n")

    print("Evaluating on LeakBench...")
    report = evaluate.remote()
    _print_report(report)


@app.local_entrypoint()
def main_eval():
    """Eval only — merged model must exist in volume."""
    print("Running LeakBench evaluation...")
    report = evaluate.remote()
    _print_report(report)
    print(json.dumps(report, indent=2))


@app.local_entrypoint()
def main_retrain():
    """Full retrain from scratch — uploads data, trains, evaluates."""
    import pathlib
    print("=== Provn full retrain ===\n")
    data_dir = pathlib.Path(__file__).parent / "data"
    with vol.batch_upload(force=True) as batch:
        batch.put_file(str(data_dir / "leakbench_train.jsonl"), "/data/leakbench_train.jsonl")
        batch.put_file(str(data_dir / "leakbench_train_balanced.jsonl"), "/data/leakbench_train_balanced.jsonl")
        batch.put_file(str(data_dir / "leakbench_eval.jsonl"), "/data/leakbench_eval.jsonl")
    result = finetune.remote()
    print(f"Training: {result}")
    report = evaluate.remote()
    _print_report(report)


@app.function(
    image=finetune_image,
    gpu="H100",
    timeout=3600,
    volumes={VOLUME_PATH: vol},
    secrets=[modal.Secret.from_name("huggingface-secret")],
)
def gguf_export():
    """Export merged model to GGUF (Q4_K_M). Run only after eval passes."""
    import os
    from unsloth import FastLanguageModel

    vol.reload()

    if not os.path.isdir(MERGED_PATH):
        raise RuntimeError(f"Merged model not found at {MERGED_PATH} — run main_retrain first")

    _hf_login()

    print(f"Loading merged model from {MERGED_PATH} ...")
    model, tokenizer = FastLanguageModel.from_pretrained(
        model_name=MERGED_PATH,
        max_seq_length=512,
        load_in_4bit=False,
        local_files_only=True,
    )

    print(f"Exporting GGUF (Q4_K_M) to {GGUF_PATH} ...")
    model.save_pretrained_gguf(
        GGUF_PATH,
        tokenizer,
        quantization_method="q4_k_m",
    )
    vol.commit()

    gguf_file = f"{GGUF_PATH}/provn-gemma4-e2b-q4_k_m.gguf"
    size_gb = os.path.getsize(gguf_file) / 1e9 if os.path.isfile(gguf_file) else -1
    print(f"GGUF export complete. File size: {size_gb:.2f} GB")
    return {"status": "success", "path": gguf_file, "size_gb": size_gb}


@app.local_entrypoint()
def main_gguf():
    """Export merged model to GGUF — run after eval passes."""
    print("=== Provn GGUF export ===\n")
    result = gguf_export.remote()
    print(f"Done: {result}")
    print(f"\nTo serve locally:")
    print(f"  modal volume get provn-model-vol provn-gemma4-e2b-gguf ~/.provn/models/")
    print(f"  llama-server -m ~/.provn/models/provn-gemma4-e2b-gguf/provn-gemma4-e2b-q4_k_m.gguf --port 8080")


def _print_report(report):
    print(f"\n{'='*40}")
    print(f"  Recall:    {report['recall']:.1%}")
    print(f"  FPR:       {report['fpr']:.1%}")
    print(f"  F1:        {report['f1']:.1%}")
    print(f"  p50:       {report['latency_p50_ms']}ms")
    print(f"  p95:       {report['latency_p95_ms']}ms")
    print(f"{'='*40}\n")
