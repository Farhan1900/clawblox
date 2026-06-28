# Recording System

## Goal

Produce replay clips whose content and timing are defined entirely by replay ticks.

This system has one canonical export path:

1. Select exact tick bounds from replay data.
2. Seek replay to exact ticks.
3. Render the selected tick into the stage canvas.
4. Export the canvas frame bytes directly.
5. Encode the resulting frame sequence at a chosen output fps.

No event heuristics, no timestamp repair, and no fallback recorder path are part of final-quality export.

## Invariants

- Tick selection comes from replay data or an explicit manifest, not visual inspection.
- The replay server is the source of truth for current tick.
- Capture-only overrides may disable interpolation and camera smoothing, but normal viewer behavior must stay unchanged.
- Final output timing comes from the exported frame sequence, not browser recorder timestamps.

## Final Export Path

### 1. Exact event selection

Use replay-derived ticks from `.clawrec` analysis or a curated manifest.

### 2. Replay control in page context

Drive replay with explicit `/replay/seek`, `/replay/pause`, `/replay/play`, and `/replay/info` requests from the page context so renderer state and replay state stay in sync.

### 3. Capture-only renderer configuration

During capture only:

- set replay interpolation delay to `0`
- set follow-camera smoothing to `0`
- preserve the WebGL drawing buffer for direct reads

### 4. Deterministic frame export

For each output frame:

1. seek to the target tick
2. confirm tick from `/replay/info`
3. rerender synchronously
4. flush and finish the WebGL context
5. export the `#stage` canvas with `toBlob`

### 5. Encode

Encode `frame_%06d.jpg` with ffmpeg using the chosen output fps.

## Preview Path

Realtime browser recording is allowed only for quick preview/debug work.

It is not the final export path because browser recorder timestamps are not authoritative for replay-time output.

## Why

- `HTMLCanvasElement.captureStream()` and `MediaRecorder` are realtime-oriented browser APIs, which makes them a poor foundation for deterministic replay export.
- `CanvasCaptureMediaStreamTrack.requestFrame()` supports manual capture pacing, but final timing still needs an explicit frame/timestamp model.
- `HTMLCanvasElement.toBlob()` provides direct image export from the canvas.
- `VideoEncoder` is the browser-native primitive for explicit frame encoding with timestamps, but a direct image-sequence plus ffmpeg pipeline is simpler to validate and debug in this repo.
- `Page.captureScreenshot` captures the page, not the canonical render surface contract.

## Sources

- MDN: `HTMLCanvasElement.captureStream()`
  https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/captureStream
- MDN: `CanvasCaptureMediaStreamTrack.requestFrame()`
  https://developer.mozilla.org/en-US/docs/Web/API/CanvasCaptureMediaStreamTrack/requestFrame
- MDN: `HTMLCanvasElement.toBlob()`
  https://developer.mozilla.org/en-US/docs/Web/API/HTMLCanvasElement/toBlob
- MDN: `VideoEncoder`
  https://developer.mozilla.org/en-US/docs/Web/API/VideoEncoder
- Chrome DevTools Protocol: `Page.captureScreenshot`
  https://chromedevtools.github.io/devtools-protocol/tot/Page/#method-captureScreenshot
