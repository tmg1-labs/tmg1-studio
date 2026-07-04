import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

// ---- 型（Rust 側 serde と対応）----
type Dither = "none" | "bayer" | "ed";

interface Segment {
  id: string;
  start_sec: number;
  end_sec: number;
  contrast: number;
  level_lo: number;
  level_hi: number;
  dither: Dither;
}

interface VideoInfo {
  duration: number;
  fps: number;
  width: number;
  height: number;
}

interface ExportResult {
  raw_path: string;
  mp4_path: string;
  frames: number;
}

// ---- 状態 ----
const state = {
  inputPath: null as string | null,
  duration: 0,
  inputFps: 0,
  inputW: 0,
  inputH: 0,
  playhead: 0, // 秒
  segments: [] as Segment[], // [0,duration] を連続被覆・昇順
  exporting: false,
};

let segCounter = 0;
function newSegId(): string {
  segCounter += 1;
  return `seg-${segCounter}`;
}

// ---- DOM ヘルパ ----
const $ = <T extends HTMLElement = HTMLElement>(id: string): T =>
  document.getElementById(id) as T;

const openBtn = $("open-btn") as HTMLButtonElement;
const exportBtn = $("export-btn") as HTMLButtonElement;
const splitBtn = $("split-btn") as HTMLButtonElement;
const deleteBtn = $("delete-btn") as HTMLButtonElement;
const fileInfo = $("file-info");
const outW = $("out-w") as HTMLInputElement;
const outH = $("out-h") as HTMLInputElement;
const outFps = $("out-fps") as HTMLInputElement;
const previewImg = $("preview-img") as HTMLImageElement;
const previewPlaceholder = $("preview-placeholder");
const previewMeta = $("preview-meta");
const segLabel = $("seg-label");
const contrastEl = $("contrast") as HTMLInputElement;
const loEl = $("level-lo") as HTMLInputElement;
const hiEl = $("level-hi") as HTMLInputElement;
const ditherEl = $("dither") as HTMLSelectElement;
const contrastVal = $("contrast-val");
const loVal = $("lo-val");
const hiVal = $("hi-val");
const timelineEl = $("timeline");
const scrubEl = $("scrub") as HTMLInputElement;
const timeReadout = $("time-readout");
const statusEl = $("status");

// ---- ユーティリティ ----
function fmtTime(sec: number): string {
  if (!isFinite(sec)) sec = 0;
  const m = Math.floor(sec / 60);
  const s = sec - m * 60;
  return `${String(m).padStart(2, "0")}:${s.toFixed(2).padStart(5, "0")}`;
}

function setStatus(msg: string, isError = false) {
  statusEl.textContent = msg;
  statusEl.classList.toggle("error", isError);
}

/** playhead を含む区間の index（境界は右側の区間に属する）。 */
function currentIndex(): number {
  const t = state.playhead;
  for (let i = 0; i < state.segments.length; i++) {
    const s = state.segments[i];
    if (t >= s.start_sec && t < s.end_sec) return i;
  }
  return Math.max(0, state.segments.length - 1); // 末尾（t==duration）
}

function currentSegment(): Segment | null {
  return state.segments[currentIndex()] ?? null;
}

// ---- 動画読み込み ----
async function openVideo() {
  const selected = await open({
    multiple: false,
    filters: [
      { name: "動画", extensions: ["mp4", "mov", "avi", "mkv", "webm", "m4v", "gif"] },
    ],
  });
  if (!selected || typeof selected !== "string") return;

  setStatus("ffprobe で解析中…");
  try {
    const info = await invoke<VideoInfo>("probe_video", { path: selected });
    state.inputPath = selected;
    state.duration = info.duration;
    state.inputFps = info.fps;
    state.inputW = info.width;
    state.inputH = info.height;
    state.playhead = 0;

    // 出力 fps の初期値は入力 fps の四捨五入。サイズは 128x64 既定のまま。
    if (info.fps > 0) outFps.value = String(Math.round(info.fps));

    // 単一区間で初期化。
    segCounter = 0;
    state.segments = [makeSegment(0, info.duration)];

    const name = selected.split(/[\\/]/).pop() ?? selected;
    fileInfo.textContent = `${name}  (${info.width}x${info.height}, ${info.fps.toFixed(2)}fps, ${fmtTime(
      info.duration,
    )})`;

    scrubEl.disabled = false;
    scrubEl.value = "0";
    setParamsEnabled(true);
    exportBtn.disabled = false;
    splitBtn.disabled = false;
    deleteBtn.disabled = false;
    previewPlaceholder.style.display = "none";

    renderTimeline();
    syncParamInputs();
    setStatus("読み込み完了");
    schedulePreview();
  } catch (e) {
    setStatus(String(e), true);
  }
}

function makeSegment(start: number, end: number, base?: Segment): Segment {
  return {
    id: newSegId(),
    start_sec: start,
    end_sec: end,
    contrast: base?.contrast ?? 1.0,
    level_lo: base?.level_lo ?? 0,
    level_hi: base?.level_hi ?? 255,
    dither: base?.dither ?? "bayer",
  };
}

// ---- タイムライン描画 ----
function renderTimeline() {
  timelineEl.innerHTML = "";
  const dur = state.duration || 1;
  const activeIdx = currentIndex();

  state.segments.forEach((seg, i) => {
    const left = (seg.start_sec / dur) * 100;
    const width = ((seg.end_sec - seg.start_sec) / dur) * 100;
    const block = document.createElement("div");
    block.className = "seg-block" + (i === activeIdx ? " active" : "");
    block.style.left = `${left}%`;
    block.style.width = `${width}%`;
    block.textContent = `#${i + 1}`;
    block.title = `${fmtTime(seg.start_sec)}–${fmtTime(seg.end_sec)}`;
    block.addEventListener("mousedown", (e) => {
      // ハンドル操作でなければ、その区間の先頭へ playhead を移動して選択。
      if ((e.target as HTMLElement).classList.contains("seg-handle")) return;
      setPlayhead(seg.start_sec + (seg.end_sec - seg.start_sec) / 2);
    });
    timelineEl.appendChild(block);

    // 区間の左境界ハンドル（先頭区間を除く = 隣接境界のみドラッグ可）。
    if (i > 0) {
      const handle = document.createElement("div");
      handle.className = "seg-handle";
      handle.style.left = `${left}%`;
      handle.addEventListener("mousedown", (e) => startBoundaryDrag(e, i));
      timelineEl.appendChild(handle);
    }
  });

  // playhead
  const ph = document.createElement("div");
  ph.className = "playhead";
  ph.style.left = `${(state.playhead / dur) * 100}%`;
  timelineEl.appendChild(ph);

  timeReadout.textContent = `${fmtTime(state.playhead)} / ${fmtTime(state.duration)}`;
}

// ---- 境界ドラッグ ----
function startBoundaryDrag(e: MouseEvent, rightIdx: number) {
  e.preventDefault();
  const leftSeg = state.segments[rightIdx - 1];
  const rightSeg = state.segments[rightIdx];
  const rect = timelineEl.getBoundingClientRect();
  const dur = state.duration || 1;
  const minGap = 0.05; // 秒。区間が潰れないように

  const onMove = (ev: MouseEvent) => {
    const ratio = Math.min(1, Math.max(0, (ev.clientX - rect.left) / rect.width));
    let t = ratio * dur;
    t = Math.max(leftSeg.start_sec + minGap, Math.min(rightSeg.end_sec - minGap, t));
    leftSeg.end_sec = t;
    rightSeg.start_sec = t;
    setPlayhead(t, false);
    renderTimeline();
  };
  const onUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    syncParamInputs();
    schedulePreview();
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// ---- playhead / scrub ----
function setPlayhead(t: number, doPreview = true) {
  state.playhead = Math.min(state.duration, Math.max(0, t));
  const dur = state.duration || 1;
  scrubEl.value = String(Math.round((state.playhead / dur) * 1000));
  renderTimeline();
  syncParamInputs();
  if (doPreview) schedulePreview();
}

scrubEl.addEventListener("input", () => {
  const dur = state.duration || 1;
  const t = (Number(scrubEl.value) / 1000) * dur;
  setPlayhead(t);
});

// ---- 分割・削除 ----
splitBtn.addEventListener("click", () => {
  const idx = currentIndex();
  const seg = state.segments[idx];
  const t = state.playhead;
  if (t <= seg.start_sec + 0.05 || t >= seg.end_sec - 0.05) {
    setStatus("区間の端すぎるため分割できませんにゃ", true);
    return;
  }
  const right = makeSegment(t, seg.end_sec, seg);
  seg.end_sec = t;
  state.segments.splice(idx + 1, 0, right);
  renderTimeline();
  syncParamInputs();
  setStatus(`区間を分割しました（計 ${state.segments.length}）`);
});

deleteBtn.addEventListener("click", () => {
  if (state.segments.length <= 1) {
    setStatus("区間が1つのため削除できませんにゃ", true);
    return;
  }
  const idx = currentIndex();
  // 左隣に結合（先頭なら右隣に結合）して連続被覆を保つ。
  if (idx > 0) {
    state.segments[idx - 1].end_sec = state.segments[idx].end_sec;
  } else {
    state.segments[idx + 1].start_sec = state.segments[idx].start_sec;
  }
  state.segments.splice(idx, 1);
  renderTimeline();
  syncParamInputs();
  setStatus(`区間を結合削除しました（計 ${state.segments.length}）`);
});

// ---- パラメータ入力 ----
function setParamsEnabled(on: boolean) {
  contrastEl.disabled = !on;
  loEl.disabled = !on;
  hiEl.disabled = !on;
  ditherEl.disabled = !on;
}

/** 現在区間の値を UI に反映する（プレビューはしない）。 */
function syncParamInputs() {
  const seg = currentSegment();
  if (!seg) return;
  const idx = currentIndex();
  segLabel.textContent = `区間 #${idx + 1}: ${fmtTime(seg.start_sec)} – ${fmtTime(seg.end_sec)}`;
  contrastEl.value = String(seg.contrast);
  loEl.value = String(seg.level_lo);
  hiEl.value = String(seg.level_hi);
  ditherEl.value = seg.dither;
  contrastVal.textContent = seg.contrast.toFixed(2);
  loVal.textContent = String(seg.level_lo);
  hiVal.textContent = String(seg.level_hi);
}

contrastEl.addEventListener("input", () => {
  const seg = currentSegment();
  if (!seg) return;
  seg.contrast = Number(contrastEl.value);
  contrastVal.textContent = seg.contrast.toFixed(2);
  schedulePreview();
});

loEl.addEventListener("input", () => {
  const seg = currentSegment();
  if (!seg) return;
  let lo = Number(loEl.value);
  if (lo >= seg.level_hi) lo = seg.level_hi - 1; // lo<hi を保つ
  seg.level_lo = lo;
  loEl.value = String(lo);
  loVal.textContent = String(lo);
  schedulePreview();
});

hiEl.addEventListener("input", () => {
  const seg = currentSegment();
  if (!seg) return;
  let hi = Number(hiEl.value);
  if (hi <= seg.level_lo) hi = seg.level_lo + 1;
  seg.level_hi = hi;
  hiEl.value = String(hi);
  hiVal.textContent = String(hi);
  schedulePreview();
});

ditherEl.addEventListener("change", () => {
  const seg = currentSegment();
  if (!seg) return;
  seg.dither = ditherEl.value as Dither;
  schedulePreview();
});

// 出力サイズ変更でもプレビューし直す。
[outW, outH, outFps].forEach((el) =>
  el.addEventListener("change", () => schedulePreview()),
);

// ---- プレビュー（デバウンス）----
let previewTimer: number | undefined;
let previewSeq = 0;
function schedulePreview() {
  if (!state.inputPath) return;
  window.clearTimeout(previewTimer);
  previewTimer = window.setTimeout(runPreview, 150);
}

async function runPreview() {
  const seg = currentSegment();
  if (!state.inputPath || !seg) return;
  const w = Number(outW.value);
  const h = Number(outH.value);
  const seq = ++previewSeq;
  try {
    const dataUrl = await invoke<string>("render_preview", {
      path: state.inputPath,
      timeSec: state.playhead,
      segment: seg,
      width: w,
      height: h,
    });
    if (seq !== previewSeq) return; // 古い結果は破棄
    previewImg.src = dataUrl;
    previewImg.style.display = "block";
    previewMeta.textContent = `${w}x${h}  @ ${fmtTime(state.playhead)}  dither=${seg.dither}  contrast=${seg.contrast.toFixed(
      2,
    )}  level=[${seg.level_lo},${seg.level_hi}]`;
  } catch (e) {
    if (seq === previewSeq) setStatus(String(e), true);
  }
}

// ---- エクスポート ----
exportBtn.addEventListener("click", doExport);

async function doExport() {
  if (!state.inputPath || state.exporting) return;
  const w = Number(outW.value);
  const h = Number(outH.value);
  const fps = Number(outFps.value);
  if (w % 8 !== 0) {
    setStatus("幅は 8 の倍数にしてください（monob のバイト境界）", true);
    return;
  }

  const target = await save({
    defaultPath: "output.raw",
    filters: [{ name: "monob raw", extensions: ["raw"] }],
  });
  if (!target) return;

  state.exporting = true;
  exportBtn.disabled = true;
  setStatus("エクスポート中…");

  try {
    const project = {
      input_path: state.inputPath,
      width: w,
      height: h,
      fps,
      segments: state.segments,
    };
    const result = await invoke<ExportResult>("export", {
      project,
      outPath: target,
    });
    setStatus(
      `完了: ${result.frames} フレーム → ${result.raw_path}（プレビュー mp4: ${result.mp4_path}）`,
    );
  } catch (e) {
    setStatus(String(e), true);
  } finally {
    state.exporting = false;
    exportBtn.disabled = false;
  }
}

// エクスポート進捗イベント。
listen<{ done: number; total: number }>("export-progress", (ev) => {
  setStatus(`エクスポート中… 区間 ${ev.payload.done}/${ev.payload.total}`);
});

// ---- 初期化 ----
openBtn.addEventListener("click", openVideo);
setStatus("「動画を開く」から始めてくださいにゃ");
