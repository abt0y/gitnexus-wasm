#!/usr/bin/env python3
"""
Quantize embedding model for ONNX Runtime Web.

Downloads all-MiniLM-L6-v2 from HuggingFace, converts to ONNX,
and quantizes to INT8 for smaller size and faster inference in WASM.

Usage:
    python scripts/quantize-model.py --output web/public/assets/
"""

import argparse
import os
import sys
from pathlib import Path

def main():
    parser = argparse.ArgumentParser(description="Quantize embedding model for WASM")
    parser.add_argument("--model", default="sentence-transformers/all-MiniLM-L6-v2",
                        help="HuggingFace model name")
    parser.add_argument("--output", default="web/public/assets",
                        help="Output directory")
    parser.add_argument("--skip-download", action="store_true",
                        help="Skip download if model exists")
    args = parser.parse_args()

    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"Model: {args.model}")
    print(f"Output: {output_dir}")

    try:
        from optimum.onnxruntime import ORTModelForFeatureExtraction, ORTQuantizer
        from optimum.onnxruntime.configuration import AutoQuantizationConfig
        from transformers import AutoTokenizer
    except ImportError:
        print("ERROR: Required packages not installed.")
        print("Install with: pip install optimum[onnxruntime] transformers")
        sys.exit(1)

    # Download and convert model
    print("\n[1/4] Downloading model...")
    model_path = output_dir / "model-onnx"

    if not args.skip_download or not model_path.exists():
        model = ORTModelForFeatureExtraction.from_pretrained(
            args.model, export=True
        )
        tokenizer = AutoTokenizer.from_pretrained(args.model)
        model.save_pretrained(model_path)
        tokenizer.save_pretrained(model_path)
        print(f"  ✓ Saved to {model_path}")
    else:
        print(f"  ⏭ Skipped (exists: {model_path})")

    # Quantize model
    print("\n[2/4] Quantizing to INT8...")
    quantizer = ORTQuantizer.from_pretrained(model_path)

    dqconfig = AutoQuantizationConfig.avx512_vnni(
        is_static=False,  # Dynamic quantization for variable inputs
        per_channel=False
    )

    quantizer.quantize(
        save_dir=output_dir / "model-quantized",
        quantization_config=dqconfig,
    )
    print("  ✓ Quantization complete")

    # Copy final model
    print("\n[3/4] Preparing final artifacts...")
    import shutil

    final_model = output_dir / "all-MiniLM-L6-v2-quantized.onnx"
    quantized_model = output_dir / "model-quantized" / "model_quantized.onnx"

    if quantized_model.exists():
        shutil.copy(quantized_model, final_model)
        print(f"  ✓ Final model: {final_model}")
        print(f"  Size: {final_model.stat().st_size / 1024 / 1024:.1f} MB")
    else:
        print("  ✗ Quantized model not found")
        sys.exit(1)

    # Create config
    print("\n[4/4] Creating model config...")
    config = {
        "modelName": "all-MiniLM-L6-v2",
        "dim": 384,
        "maxSeqLength": 512,
        "modelUrl": "./assets/all-MiniLM-L6-v2-quantized.onnx",
        "tokenizerUrl": "./assets/tokenizer.json",
        "quantized": True,
        "format": "onnx"
    }

    import json
    config_path = output_dir / "model-config.json"
    with open(config_path, "w") as f:
        json.dump(config, f, indent=2)
    print(f"  ✓ Config: {config_path}")

    # Cleanup
    print("\n[Cleanup] Removing intermediate files...")
    shutil.rmtree(model_path, ignore_errors=True)
    shutil.rmtree(output_dir / "model-quantized", ignore_errors=True)
    print("  ✓ Done")

    print("\n" + "="*50)
    print("Model quantization complete!")
    print(f"Final model size: {final_model.stat().st_size / 1024 / 1024:.1f} MB")
    print("="*50)

if __name__ == "__main__":
    main()
