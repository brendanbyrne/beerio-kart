# OCR Strategy for Mario Kart 8 Deluxe Result Times

**Status:** Research — no decisions made. This document gathers strategies, trade-offs, and prior art so we can pick an approach when OCR work is scheduled. It is not in `design.md` yet.

**Scope decision (locked in via the question that prompted this research):** the capture target is the **results screen**, photographed by the player's phone. Live-during-race OCR and capture-card-based pipelines are out of scope for v1.

---

## 1. The problem in one paragraph

After a Time Trial, the player aims their phone at the TV and snaps a photo of the results screen. The app needs to extract the final time in Mario Kart's time format — for example `1'23"456` (one minute, twenty-three seconds, four hundred fifty-six milliseconds — Mario Kart uses apostrophe and double-quote as separators, not colon and period). Eventually we may also want lap splits, course name, and character, but the final time is the v1 target.

## 2. What makes this easier than generic OCR

Several properties of this problem narrow the search space considerably and matter for the strategy choice below:

- **Closed character set.** Only digits `0–9` plus the separators `'` and `"`. Eleven classes total.
- **Fixed font.** Mario Kart renders these digits in the same stylized font every time. Once a model has seen it, it has seen all of it.
- **Fixed layout.** The results screen always puts the final time in the same spot relative to the screen frame.
- **Fixed format.** The time always matches the regex `\d{1,2}'\d{2}"\d{3}`. Anything that doesn't is wrong, and we can reject and ask for a retake.
- **User-initiated capture.** Unlike a security camera or a real-time pipeline, the player is actively trying to help. We can show an overlay frame and tell them "line up the time inside this box."

## 3. What makes this harder than the speedrun-community OCR pipelines

The existing Mario Kart speedrun community has solved this — Vike's [MK8DX-Video-Autosplitter](https://github.com/VikeMK/MK8DX-Video-Autosplitter) is the canonical tool. **But it solves a different problem:** it reads from a capture card feeding clean digital frames into LiveSplit. We are reading a phone photo of a TV, which means:

- **Perspective and rotation.** The phone is rarely perfectly perpendicular to the screen.
- **Glare and reflections.** TV glass, ceiling lights, the player's own reflection.
- **Color cast.** Different TVs and different display modes produce different RGB.
- **Motion blur and focus.** Hand-held photos with auto-focus that may not have settled.
- **Moiré and refresh banding** if the phone shutter beats with the screen refresh.

So the speedrun community's pure-template-matching approach (which is what video autosplitters use) won't transfer cleanly. We need something that tolerates the phone-camera variance.

## 4. Strategy options, ranked

### Option A — Off-the-shelf OCR, no training (Tesseract or ocrs)

Run a generic OCR engine on the cropped time region. Either Tesseract (via [tesseract.js](https://github.com/naptha/tesseract.js) in the browser or [leptess](https://crates.io/crates/leptess) in Rust) or [ocrs](https://github.com/robertknight/ocrs) (a pure-Rust ONNX-based engine, also compiles to WASM).

**Why it might work:** simple, zero training, ships fast.

**Why it probably won't be good enough:** generic OCR is trained on real-world fonts (Times, Arial, handwriting), not Mario Kart's stylized italic digits. Public benchmarks show Tesseract hitting about 70% character accuracy on real-world unconstrained images and missing text entirely on ~21% of inputs ([TildAlice benchmark](https://tildalice.io/ocr-tesseract-easyocr-paddleocr-benchmark/)). On a stylized font with apostrophe/quote separators it will be worse — the apostrophes and quotes are almost certain to be misread as commas, periods, or backticks unless we constrain the output.

**Verdict:** worth running as a 2-day baseline experiment before investing in anything heavier. The result tells us how much work the rest of the pipeline needs to do.

### Option B — Off-the-shelf OCR with aggressive game-specific scaffolding

Same engine, but wrap it with everything we know about the problem:

1. Crop to the time region first (template matching, or detect the bright digit area by color thresholding).
2. Constrain the character whitelist (Tesseract: `tessedit_char_whitelist=0123456789'"`).
3. Validate the output against the time-format regex; reject and re-prompt the user if it doesn't match.
4. Optionally OCR the same image at 2–3 different rotations/scales and majority-vote.

This is the highest-leverage cheap win. It can plausibly push 70% baseline to 90%+ without any training.

### Option C — Fine-tune Tesseract on the Mario Kart font

Tesseract supports LSTM fine-tuning ([official guide](https://tesseract-ocr.github.io/tessdoc/tess5/TrainingTesseract-5.html)). The recipe is roughly: extract the trained data file, render ~80 lines of training text in the target font, train for ~400 iterations, evaluate. The official docs note that fine-tuning works well "for problems that are close to the existing training data but different in some subtle way, like a particularly unusual font."

**Catch:** we need the actual Mario Kart digit font. We'd either find a community-extracted version or render templates from game screenshots and synthesize a font from them. This is doable but is a side quest.

**Verdict:** an intermediate step if Option B isn't accurate enough but we don't want to build a custom model from scratch.

### Option D — Custom small CNN trained on synthetic data (the technically right answer)

This is what the problem actually wants:

1. **Detection:** find the time region in the photo. Either by template matching against a results-screen template, or by training a tiny detection model.
2. **Segmentation:** split the cropped region into individual character cells. With a fixed format this can be done by simple x-coordinate slicing once the region is normalized.
3. **Classification:** each character cell goes through an 11-class CNN (`0–9`, `'`, `"`).

Training data is generated synthetically: take the MK digit glyphs, render them at many sizes/colors/angles, and apply augmentations that mimic phone-camera variance — Gaussian blur, perspective warp, color jitter, brightness, JPEG compression, fake glare. The community guidance on per-class training data for digit OCR is "100–500 images per character" ([CNN-for-OCR tutorial](https://medium.com/@saminchandeepa/how-to-train-a-custom-cnn-model-for-single-character-ocr-3aca5eb67714)) — easy to hit synthetically.

The resulting model is tiny (a few hundred KB) and runs in single-digit milliseconds even on a phone via ONNX Runtime Web or TensorFlow.js.

**Verdict:** the right long-term solution. It's the smallest, fastest, and most accurate option for this exact narrow task. The cost is one-time setup of the training pipeline and the synthetic data generator.

### Option E — Pure template matching

Precompute one reference image per character. For each candidate cell in the input, compute normalized cross-correlation with each template; highest correlation wins. This is what Vike's autosplitter uses on capture-card frames.

**Verdict:** rejected for our use case. It works because capture-card frames are pixel-stable. Phone-camera frames vary in scale, rotation, and color enough that a simple correlation will miss often. It is, however, a reasonable feature-engineering ingredient inside Option D — for example as the digit detector that finds where the time region is, before the CNN reads it.

## 5. Where the work runs: phone, server, or hybrid

The app is a React PWA with an Axum/Rust backend. "On-device" therefore means **in the browser**, not as a native app — there's no MLKit, no Apple Vision SDK. Browser OCR means WebAssembly. Practical options today:

- **tesseract.js / tesseract-wasm** — about 2–8 MB compressed download for engine + English. Reported throughput is 1–3 seconds per image on a laptop and 2–20 seconds on iPhone-class mobile depending on image size ([tesseract-wasm docs](https://github.com/robertknight/tesseract-wasm)).
- **ONNX Runtime Web** ([docs](https://onnxruntime.ai/docs/tutorials/web/)) — runs custom ONNX models in the browser via WASM, with WebGPU/WebNN acceleration where available. This is the path for shipping a custom-trained CNN to the browser.
- **ocrs in WASM** — works ([repo](https://github.com/robertknight/ocrs)) but the model files are larger and the project is still flagged "early preview."

**Recommendation: server-side first, on-device later.** The reasons:

1. **You will retrain the model.** Initial accuracy will be poor, and you'll need to log failure cases server-side, hand-label them, and retrain. That loop is hard to run if inference is on the phone.
2. **Bigger models are allowed on the server.** Mobile-conscious model size budgets (a few hundred KB) push you toward Option D anyway, but server-side you have headroom to experiment with PaddleOCR's PP-OCRv5 ([13 percentage-point accuracy gain over PP-OCRv4](https://paddlepaddle.github.io/PaddleOCR/main/en/version3.x/algorithm/PP-OCRv5/PP-OCRv5.html)) for ground-truthing.
3. **Latency is irrelevant here.** A race takes 30+ seconds to play. A 500 ms upload + 200 ms OCR is invisible.
4. **No privacy concern.** The image is a photo of a TV showing race results — no faces, no PII.

The phone should still **preprocess before upload**: resize, crop to detected results region, JPEG-encode at modest quality. This is mostly about bandwidth, not OCR.

Move OCR on-device once the model is well-trained and small enough that it doesn't bloat the PWA bundle. Hybrid (try on-device, fall back to server) is a reasonable v3 step but not worth the complexity now.

## 6. Language

You asked about Rust — yes, I think Rust is the right call for the server-side OCR component. The options:

- **`ort` crate** — Rust bindings to ONNX Runtime. This is the cleanest path if you go with a custom CNN: train in PyTorch (Python), export to ONNX, load with `ort` from the Axum process. No Python in production. This is the path I'd recommend.
- **`ocrs` crate** — pure Rust, ONNX under the hood, integrates trivially with Axum. Good fit if Option A/B is sufficient. Latin-only, which is fine for digits.
- **`leptess` crate** — bindings to Tesseract. Works but the Tesseract binary needs to exist on the deploy target. Brittle for containerized deploy, fine on a long-lived server.
- **Python sidecar (FastAPI/PaddleOCR)** — pragmatic if you need PaddleOCR specifically. Adds another runtime and another service to manage. I'd avoid it unless the accuracy gap forces you to.

For training the model itself, **Python is the right tool** — PyTorch + an export-to-ONNX step. The Rust server only needs to do inference.

## 7. Recommended path

The path that minimizes wasted work given how uncertain the accuracy numbers are:

**Phase A — Establish the baseline (1–2 days of work, no commitment).** Take ~50 phone photos of MK8DX results screens — different TVs, lighting, angles. Hand-label the times. This becomes the eval set forever. Then run Option B (Tesseract via `leptess` in Axum, with cropping + digit whitelist + format-regex validation) and measure end-to-end accuracy.

**Phase B — Decide based on the number.** If Option B clears, say, 95% on the eval set, ship it. If it's stuck at 70–90%, jump to Option D. Skip Option C unless we hit a weird case where Tesseract is *almost* right and just needs the font fix.

**Phase D — Custom CNN.** Build a synthetic data generator using extracted MK glyphs. Train an 11-class CNN in PyTorch. Export to ONNX. Run via `ort` in Axum. Iterate using the eval set and any failure cases logged from production.

**Phase E (later, optional) — Move inference on-device.** Once the model is a few hundred KB and stable, ship it via ONNX Runtime Web for offline mode.

## 8. Capture UX (tentative MVP decision, 2026-05-04)

For MVP the capture flow will be: **draw a fixed alignment box on the phone screen, prompt the player to point the phone at the TV until the time region fills the box, then capture.** This narrows the OCR input enough that the Phase A Tesseract baseline has a real shot at being shippable.

This decision is held loosely — it flips if user testing shows the alignment-and-hold flow is too stilted. Specific risks to watch for in testing:

- **TV size variance.** Sizing the alignment box for a 50–65" TV at couch distance means 32" sets force the player closer. There's no good single size that works across all TVs without some accommodation.
- **Hold-still friction.** "Align then tap" creates a pause. An auto-capture-when-stable flow (evaluate frames continuously, fire when alignment + sharpness pass thresholds) removes the tap and feels noticeably smoother. Specify this in the design rather than the tap-to-capture variant.
- **Box scope.** Time Trial results show three lap splits plus a final time. MVP captures the final time only; splits become a later phase.
- **Glare angle.** Straight-on shots maximize glare risk. The frame guide can nudge players to a slight off-axis angle.
- **Escape hatch.** Manual time entry must always be available as a fallback. OCR failures are inevitable; trapping the user in retake-loop hell is the worst UX outcome.

## 9. Things I'd still want to confirm before starting Phase A

- **Can we get the actual MK8DX UI font?** Either via a community-extracted asset or by rendering a clean reference from a capture-card screenshot. This determines how much of Option D's training data we can synthesize vs. having to collect.
- **What does the results screen actually look like across game modes?** Time Trial, VS Race, and Grand Prix presumably differ. The v1 target is Time Trial; the others are out of scope until we see real layouts.
- **Single-time photo vs multi-frame burst?** A burst-and-pick-the-sharpest flow (variance-of-Laplacian as the sharpness metric) is a cheap accuracy boost worth specifying up front.

## 10. Sources

Library and benchmark research:

- [Tesseract vs PaddleOCR vs dots.ocr 3-Way Benchmark 2026 — CodeSOTA](https://www.codesota.com/ocr/paddleocr-vs-tesseract)
- [PaddleOCR vs EasyOCR vs Tesseract benchmark — TildAlice](https://tildalice.io/ocr-tesseract-easyocr-paddleocr-benchmark/)
- [OCR comparison: Tesseract vs EasyOCR vs PaddleOCR vs MMOCR — Toon Beerten](https://toon-beerten.medium.com/ocr-comparison-tesseract-versus-easyocr-vs-paddleocr-vs-mmocr-a362d9c79e66)
- [PP-OCRv5 introduction — PaddleOCR docs](https://paddlepaddle.github.io/PaddleOCR/main/en/version3.x/algorithm/PP-OCRv5/PP-OCRv5.html)

Browser / mobile OCR:

- [tesseract.js — naptha/tesseract.js](https://github.com/naptha/tesseract.js)
- [tesseract-wasm — robertknight/tesseract-wasm](https://github.com/robertknight/tesseract-wasm)
- [ONNX Runtime Web tutorials](https://onnxruntime.ai/docs/tutorials/web/)
- [Run AI models entirely in the browser with ONNX Runtime — DEV](https://dev.to/hexshift/run-ai-models-entirely-in-the-browser-using-webassembly-onnx-runtime-no-backend-required-4lag)

Rust OCR:

- [ocrs — robertknight/ocrs](https://github.com/robertknight/ocrs)
- [ocrs on crates.io](https://crates.io/crates/ocrs)

Tesseract training:

- [Tesseract 5 LSTM training — official docs](https://tesseract-ocr.github.io/tessdoc/tess5/TrainingTesseract-5.html)
- [Fine-tuning Tesseract OCR for German Invoices — statworx](https://www.statworx.com/en/content-hub/blog/fine-tuning-tesseract-ocr-for-german-invoices)

Custom CNN OCR for game-like UI:

- [How to Train a Custom CNN Model for Single Character OCR — Samin Chandeepa](https://medium.com/@saminchandeepa/how-to-train-a-custom-cnn-model-for-single-character-ocr-3aca5eb67714)
- [Video-Game-OCR — leshokunin](https://github.com/leshokunin/Video-Game-OCR)

Mario Kart prior art:

- [MK8DX-Video-Autosplitter — VikeMK](https://github.com/VikeMK/MK8DX-Video-Autosplitter)
- [Load Time Remover and Auto-Splitter forum thread — speedrun.com](https://www.speedrun.com/mk8dx/forums/yeh1o)

PWA camera:

- [How to Access the Camera in a PWA, 2025 guide — SimiCart](https://simicart.com/blog/pwa-camera-access/)

## Document history

- 2026-05-04 — Initial OCR strategy research document. Captures phone-camera-of-results-screen scope, ranks five strategy options (off-the-shelf, off-the-shelf + scaffolding, fine-tuned Tesseract, custom CNN, template matching), recommends server-side first via Rust `ort` crate, lays out a phased Tesseract-baseline → custom-CNN path. No design decisions made — input to a future design.md update.
- 2026-05-04 — Added section 8 recording the tentative MVP capture-UX decision: alignment-box overlay with auto-capture-when-stable, final time only, manual-entry escape hatch always available. Decision is held loosely and may flip if user testing finds the flow too stilted.
