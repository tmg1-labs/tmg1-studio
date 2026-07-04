import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import { load, type Store } from "@tauri-apps/plugin-store";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  t,
  setLocale,
  detectLocale,
  isLocale,
  LOCALES,
  type Locale,
} from "./i18n";

// Vite の define で埋め込まれる表示バージョン（VITE_APP_VERSION → package.json）。
declare const __APP_VERSION__: string;

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

// 保存する編集プロジェクトの形（.tmgproj = JSON）。
interface ProjectFile {
  version: number;
  input_path: string;
  width: number;
  height: number;
  fps: number;
  segments: Segment[];
  play_start: number;
  play_end: number;
}

// 区間パラメータのプリセット（tauri-plugin-store で永続化）。
interface Preset {
  name: string;
  contrast: number;
  level_lo: number;
  level_hi: number;
  dither: Dither;
}

// 初回起動時に seed する組み込みプリセット（howto の実証済みレシピ）。seed 後は
// 通常のプリセットと同様に削除・上書きできる。
const BUILTIN_PRESETS: Preset[] = [
  { name: "Plain (Bayer)", contrast: 1.0, level_lo: 0, level_hi: 255, dither: "bayer" },
  { name: "Level squeeze 32-192 + Bayer", contrast: 1.0, level_lo: 32, level_hi: 192, dither: "bayer" },
  { name: "Threshold (no dither)", contrast: 1.0, level_lo: 0, level_hi: 255, dither: "none" },
  { name: "High contrast + error diffusion", contrast: 1.3, level_lo: 0, level_hi: 255, dither: "ed" },
];

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
  playStart: 0, // 再生範囲の開始（秒）
  playEnd: 0, // 再生範囲の終了（秒）
  playing: false,
};

let objectUrl: string | null = null; // レンダリング済み再生 mp4 の Blob URL（停止しても保持）
let renderValid = false; // objectUrl が現在の範囲/パラメータと一致しているか
let projectPath: string | null = null; // 現在のプロジェクトファイル(.tmgproj)のパス
let dirty = false; // 未保存の変更があるか
let followRaf: number | null = null; // 再生位置追従の rAF ハンドル
let seeking = false; // スクラブでシーク操作中（追従の上書きを抑止）

let segCounter = 0;
function newSegId(): string {
  segCounter += 1;
  return `seg-${segCounter}`;
}

// ---- DOM ヘルパ ----
const $ = <T extends HTMLElement = HTMLElement>(id: string): T =>
  document.getElementById(id) as T;

const newBtn = $("new-btn") as HTMLButtonElement;
const loadBtn = $("load-btn") as HTMLButtonElement;
const saveBtn = $("save-btn") as HTMLButtonElement;
const closeBtn = $("close-btn") as HTMLButtonElement;
const exportBtn = $("export-btn") as HTMLButtonElement;
const settingsBtn = $("settings-btn") as HTMLButtonElement;
const settingsMenu = $("settings-menu");
const appVersionEl = $("app-version");
const confirmModal = $("confirm-modal");
const confirmMsgEl = $("confirm-msg");
const modalSaveBtn = $("modal-save") as HTMLButtonElement;
const modalDiscardBtn = $("modal-discard") as HTMLButtonElement;
const modalCancelBtn = $("modal-cancel") as HTMLButtonElement;
const langSelect = $("lang-select") as HTMLSelectElement;
const splitBtn = $("split-btn") as HTMLButtonElement;
const deleteBtn = $("delete-btn") as HTMLButtonElement;
const fileInfo = $("file-info");
const outW = $("out-w") as HTMLInputElement;
const outH = $("out-h") as HTMLInputElement;
const outFps = $("out-fps") as HTMLInputElement;
const previewImg = $("preview-img") as HTMLImageElement;
const previewVideo = $("preview-video") as HTMLVideoElement;
const previewPlaceholder = $("preview-placeholder");
const previewMeta = $("preview-meta");
const renderProgress = $("render-progress");
const renderProgressFill = $("render-progress-fill");
const renderProgressPct = $("render-progress-pct");
const zoomEl = $("zoom") as HTMLSelectElement;
const playBtn = $("play-btn") as HTMLButtonElement;
const rangeToSegBtn = $("range-to-seg") as HTMLButtonElement;
const rangeSetStartBtn = $("range-set-start") as HTMLButtonElement;
const rangeSetEndBtn = $("range-set-end") as HTMLButtonElement;
const rangeClearBtn = $("range-clear") as HTMLButtonElement;
const segLabel = $("seg-label");
const contrastEl = $("contrast") as HTMLInputElement;
const loEl = $("level-lo") as HTMLInputElement;
const hiEl = $("level-hi") as HTMLInputElement;
const ditherEl = $("dither") as HTMLSelectElement;
const presetListEl = $("preset-list") as HTMLSelectElement;
const presetNameEl = $("preset-name") as HTMLInputElement;
const presetApplyBtn = $("preset-apply") as HTMLButtonElement;
const presetSaveBtn = $("preset-save") as HTMLButtonElement;
const presetDeleteBtn = $("preset-delete") as HTMLButtonElement;
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

function clamp(v: number, lo: number, hi: number): number {
  return Math.min(hi, Math.max(lo, v));
}

// 保存対象（区間・パラメータ・範囲・出力設定）を編集したら未保存フラグを立て、
// レンダリング済み再生キャッシュを無効化する（次の再生で作り直す）。
function markDirty() {
  dirty = true;
  renderValid = false;
}

// 入力パスと ffprobe 情報から編集状態をセットアップする。
// loaded を渡すとプロジェクト読込（区間・範囲・出力設定を復元）、無ければ新規
// （単一区間・範囲全体・出力設定は現在の UI 値のまま = fps 既定 15）。
function applyProject(
  path: string,
  info: VideoInfo,
  loaded?: {
    segments: Segment[];
    playStart: number;
    playEnd: number;
    width: number;
    height: number;
    fps: number;
  },
) {
  state.inputPath = path;
  state.duration = info.duration;
  state.inputFps = info.fps;
  state.inputW = info.width;
  state.inputH = info.height;
  state.playhead = 0;

  segCounter = 0;
  if (loaded) {
    outW.value = String(loaded.width);
    outH.value = String(loaded.height);
    outFps.value = String(loaded.fps);
    // 区間の id は内部用なので振り直して衝突を防ぐ。範囲は尺内にクランプ。
    state.segments = loaded.segments.map((s) => ({ ...s, id: newSegId() }));
    state.playStart = clamp(loaded.playStart, 0, info.duration);
    state.playEnd = clamp(loaded.playEnd, 0, info.duration);
  } else {
    state.segments = [makeSegment(0, info.duration)];
    state.playStart = 0;
    state.playEnd = info.duration;
  }
  stopPlayback();
  discardRender();

  updateFileInfo();

  scrubEl.disabled = false;
  scrubEl.value = "0";
  setParamsEnabled(true);
  exportBtn.disabled = false;
  splitBtn.disabled = false;
  deleteBtn.disabled = false;
  playBtn.disabled = false;
  rangeToSegBtn.disabled = false;
  rangeSetStartBtn.disabled = false;
  rangeSetEndBtn.disabled = false;
  rangeClearBtn.disabled = false;
  previewPlaceholder.style.display = "none";

  // プロジェクトを開いた状態へ（新規/読込ボタンを隠し、保存/閉じるを表示）。
  document.body.classList.remove("no-project");
  updateSaveLabel();

  renderTimeline();
  syncParamInputs();
  schedulePreview();
  dirty = false; // 読み込み直後は未変更
}

// 保存ボタンのラベル: 未保存の新規は「保存」、保存先が確定していれば「上書き」。
function updateSaveLabel() {
  saveBtn.textContent = projectPath ? t("overwrite") : t("save");
}

// ファイル情報表示（state から再構築できるので言語切替時にも呼べる）。
function updateFileInfo() {
  if (!state.inputPath) {
    fileInfo.textContent = t("noVideo");
    return;
  }
  const name = state.inputPath.split(/[\\/]/).pop() ?? state.inputPath;
  fileInfo.textContent = t("fileInfo", {
    name,
    w: state.inputW,
    h: state.inputH,
    fps: state.inputFps.toFixed(2),
    dur: fmtTime(state.duration),
  });
}

// 再生ボタンのラベル（再生状態で切替。言語切替時にも呼ぶ）。
function updatePlayButtonLabel() {
  playBtn.textContent = state.playing ? t("stop") : t("play");
}

// ---- 新規作成（動画読み込みを兼ねる）----
async function openVideo() {
  const selected = await open({
    multiple: false,
    filters: [
      { name: t("dialogVideoFilter"), extensions: ["mp4", "mov", "avi", "mkv", "webm", "m4v", "gif"] },
    ],
  });
  if (!selected || typeof selected !== "string") return;

  setStatus(t("probing"));
  try {
    const info = await invoke<VideoInfo>("probe_video", { path: selected });
    projectPath = null; // 新規はまだ保存先なし → ボタンは「保存」
    applyProject(selected, info);
    setStatus(t("loaded"));
  } catch (e) {
    setStatus(String(e), true);
  }
}

// 3択の確認モーダル（保存する / 保存しない / キャンセル）。
type DiscardChoice = "save" | "discard" | "cancel";
let modalResolve: ((c: DiscardChoice) => void) | null = null;

function askUnsaved(message: string): Promise<DiscardChoice> {
  confirmMsgEl.textContent = message;
  confirmModal.hidden = false;
  return new Promise((resolve) => {
    modalResolve = resolve;
  });
}

function resolveModal(choice: DiscardChoice) {
  confirmModal.hidden = true;
  const r = modalResolve;
  modalResolve = null;
  if (r) r(choice);
}

modalSaveBtn.addEventListener("click", () => resolveModal("save"));
modalDiscardBtn.addEventListener("click", () => resolveModal("discard"));
modalCancelBtn.addEventListener("click", () => resolveModal("cancel"));
// オーバーレイ外側クリック / Esc はキャンセル扱い。
confirmModal.addEventListener("click", (e) => {
  if (e.target === confirmModal) resolveModal("cancel");
});
document.addEventListener("keydown", (e) => {
  if (!confirmModal.hidden && e.key === "Escape") resolveModal("cancel");
});

// 未保存なら確認する（true=続行してよい）。「保存する」を選んだ場合は保存を試み、
// 保存が成功したら続行、キャンセル/失敗なら中断する。
async function confirmUnsaved(message: string): Promise<boolean> {
  if (!dirty) return true;
  const choice = await askUnsaved(message);
  if (choice === "cancel") return false;
  if (choice === "save") return await onSaveClick();
  return true; // discard
}

// プロジェクトを閉じて起動時の状態へ戻す。
async function closeProject() {
  if (!(await confirmUnsaved(t("confirmClose")))) {
    return;
  }
  stopPlayback();
  discardRender();
  state.inputPath = null;
  state.segments = [];
  state.duration = 0;
  state.playhead = 0;
  state.playStart = 0;
  state.playEnd = 0;
  projectPath = null;
  dirty = false;
  previewImg.style.display = "none";
  previewImg.removeAttribute("src");
  updateFileInfo();
  document.body.classList.add("no-project");
  setStatus(t("projectClosed"));
}

// ---- プロジェクト保存 / 読込 ----
// 指定パスへ現在のプロジェクトを書き出す。
async function writeProject(target: string) {
  if (!state.inputPath) return;
  const data: ProjectFile = {
    version: 1,
    input_path: state.inputPath,
    width: Number(outW.value),
    height: Number(outH.value),
    fps: Number(outFps.value),
    segments: state.segments,
    play_start: state.playStart,
    play_end: state.playEnd,
  };
  await invoke("save_project", {
    path: target,
    contents: JSON.stringify(data, null, 2),
  });
}

// 「保存/上書き」ボタン: 保存先が確定していれば上書き、無ければ保存ダイアログ。
// 戻り値 = 保存できたか（ダイアログをキャンセル or 失敗なら false）。
async function onSaveClick(): Promise<boolean> {
  if (!state.inputPath) {
    setStatus(t("needVideoFirst"), true);
    return false;
  }
  try {
    if (projectPath) {
      await writeProject(projectPath);
      dirty = false;
      setStatus(t("projectOverwritten", { path: projectPath }));
      return true;
    } else {
      const target = await save({
        defaultPath: "project.tmgproj",
        filters: [{ name: t("dialogProjectFilter"), extensions: ["tmgproj"] }],
      });
      if (!target) return false;
      await writeProject(target);
      projectPath = target;
      dirty = false;
      updateSaveLabel(); // 以後は「上書き」
      setStatus(t("projectSaved", { path: target }));
      return true;
    }
  } catch (e) {
    setStatus(t("saveFailed", { err: String(e) }), true);
    return false;
  }
}

async function loadProject() {
  const selected = await open({
    multiple: false,
    filters: [{ name: t("dialogProjectFilter"), extensions: ["tmgproj", "json"] }],
  });
  if (!selected || typeof selected !== "string") return;
  setStatus(t("projectLoading"));
  try {
    const text = await invoke<string>("load_project", { path: selected });
    const data = JSON.parse(text) as ProjectFile;
    if (!data || !Array.isArray(data.segments) || !data.input_path) {
      throw new Error(t("projectInvalid"));
    }
    // 参照している入力動画を再解析（尺・サイズ・fps を取得。存在確認も兼ねる）。
    const info = await invoke<VideoInfo>("probe_video", { path: data.input_path });
    projectPath = selected; // 既存プロジェクト → ボタンは「上書き」
    applyProject(data.input_path, info, {
      segments: data.segments,
      playStart: data.play_start ?? 0,
      playEnd: data.play_end ?? info.duration,
      width: data.width,
      height: data.height,
      fps: data.fps,
    });
    setStatus(t("projectLoaded", { path: selected }));
  } catch (e) {
    setStatus(t("projectLoadFailed", { err: String(e) }), true);
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

  // 再生範囲バンド + 両端ハンドル（区間境界にスナップ）。
  if (state.duration > 0) {
    const rs = (state.playStart / dur) * 100;
    const re = (state.playEnd / dur) * 100;
    const band = document.createElement("div");
    band.className = "range-band";
    band.style.left = `${rs}%`;
    band.style.width = `${Math.max(0, re - rs)}%`;
    timelineEl.appendChild(band);

    const hStart = document.createElement("div");
    hStart.className = "range-handle start";
    hStart.style.left = `${rs}%`;
    hStart.title = t("rangeStartHandle");
    hStart.addEventListener("mousedown", (e) => startRangeDrag(e, "start"));
    timelineEl.appendChild(hStart);

    const hEnd = document.createElement("div");
    hEnd.className = "range-handle end";
    hEnd.style.left = `${re}%`;
    hEnd.title = t("rangeEndHandle");
    hEnd.addEventListener("mousedown", (e) => startRangeDrag(e, "end"));
    timelineEl.appendChild(hEnd);
  }

  // playhead
  const ph = document.createElement("div");
  ph.className = "playhead";
  ph.id = "playhead-marker";
  ph.style.left = `${(state.playhead / dur) * 100}%`;
  timelineEl.appendChild(ph);

  updateReadout();
}

/** 時刻読み出し（playhead / 全体 ｜ 範囲）を更新する。 */
function updateReadout() {
  timeReadout.textContent = t("readout", {
    playhead: fmtTime(state.playhead),
    dur: fmtTime(state.duration),
    start: fmtTime(state.playStart),
    end: fmtTime(state.playEnd),
  });
}

// ---- 再生範囲ドラッグ（区間境界・0・終端にスナップ）----
function snapPoints(): number[] {
  const pts = new Set<number>([0, state.duration]);
  for (const s of state.segments) {
    pts.add(s.start_sec);
    pts.add(s.end_sec);
  }
  return [...pts];
}

function startRangeDrag(e: MouseEvent, which: "start" | "end") {
  e.preventDefault();
  e.stopPropagation();
  if (state.playing) {
    stopPlayback();
    runPreview();
  }
  const rect = timelineEl.getBoundingClientRect();
  const dur = state.duration || 1;
  const snapPx = 8;
  const minGap = 0.05;

  const onMove = (ev: MouseEvent) => {
    const ratio = Math.min(1, Math.max(0, (ev.clientX - rect.left) / rect.width));
    let t = ratio * dur;
    // スナップ: 最も近い境界が閾値内なら吸着。
    const snapSec = (snapPx / rect.width) * dur;
    let bestD = snapSec;
    for (const p of snapPoints()) {
      const d = Math.abs(p - t);
      if (d <= bestD) {
        bestD = d;
        t = p;
      }
    }
    if (which === "start") {
      state.playStart = Math.min(t, state.playEnd - minGap);
    } else {
      state.playEnd = Math.max(t, state.playStart + minGap);
    }
    renderTimeline();
  };
  const onUp = () => {
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
    markDirty();
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
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
    markDirty();
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
  let t = (Number(scrubEl.value) / 1000) * dur;
  if (state.playing) {
    // 再生中: 範囲内にクランプして video をシーク（再生は継続）。
    t = Math.min(state.playEnd, Math.max(state.playStart, t));
    seeking = true;
    const rel = Math.min(
      Math.max(0, t - state.playStart),
      Math.max(0, state.playEnd - state.playStart - 0.001),
    );
    previewVideo.currentTime = rel;
    state.playhead = t;
    // 範囲外へドラッグしても表示はクランプ後の位置に補正。
    scrubEl.value = String(Math.round((t / dur) * 1000));
    const marker = document.getElementById("playhead-marker");
    if (marker) marker.style.left = `${(t / dur) * 100}%`;
    updateReadout();
  } else {
    setPlayhead(t);
  }
});

// ドラッグ終了でシークフラグを解除（追従ループが再び位置を反映）。
scrubEl.addEventListener("change", () => {
  seeking = false;
});

// ---- 分割・削除 ----
splitBtn.addEventListener("click", () => {
  const idx = currentIndex();
  const seg = state.segments[idx];
  const ph = state.playhead;
  if (ph <= seg.start_sec + 0.05 || ph >= seg.end_sec - 0.05) {
    setStatus(t("splitTooClose"), true);
    return;
  }
  const right = makeSegment(ph, seg.end_sec, seg);
  seg.end_sec = ph;
  state.segments.splice(idx + 1, 0, right);
  renderTimeline();
  syncParamInputs();
  markDirty();
  setStatus(t("splitDone", { n: state.segments.length }));
});

deleteBtn.addEventListener("click", () => {
  if (state.segments.length <= 1) {
    setStatus(t("deleteOnlyOne"), true);
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
  markDirty();
  setStatus(t("mergeDone", { n: state.segments.length }));
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
  segLabel.textContent = t("segLabel", {
    n: idx + 1,
    start: fmtTime(seg.start_sec),
    end: fmtTime(seg.end_sec),
  });
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
  markDirty();
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
  markDirty();
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
  markDirty();
  schedulePreview();
});

ditherEl.addEventListener("change", () => {
  const seg = currentSegment();
  if (!seg) return;
  seg.dither = ditherEl.value as Dither;
  markDirty();
  schedulePreview();
});

// 出力サイズ変更でもプレビューし直す。
[outW, outH, outFps].forEach((el) =>
  el.addEventListener("change", () => {
    markDirty();
    schedulePreview();
  }),
);

// ---- プレビュー（デバウンス）----
let previewTimer: number | undefined;
let previewSeq = 0;
function schedulePreview() {
  if (!state.inputPath) return;
  // 再生中に編集したら、静止プレビューへ戻す（古いモーションを見せない）。
  if (state.playing) stopPlayback();
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
    applyZoom();
    previewMeta.textContent = t("previewMeta", {
      w,
      h,
      t: fmtTime(state.playhead),
      dither: seg.dither,
      contrast: seg.contrast.toFixed(2),
      lo: seg.level_lo,
      hi: seg.level_hi,
    });
  } catch (e) {
    if (seq === previewSeq) setStatus(String(e), true);
  }
}

// ---- 表示ズーム ----
// window=フレームに合わせる、それ以外は出力ピクセルに対する倍率（近傍拡大で等倍表示）。
// 静止プレビュー(img)と再生(video)の両方に同じ規則を適用する（表示側だけ見える）。
function styleZoom(el: HTMLElement) {
  const mode = zoomEl.value;
  if (mode === "window") {
    // フレームいっぱいに拡大/縮小して収める（アスペクト比維持）。
    // max-width だけだと小さい素材は拡大されないため object-fit: contain を使う。
    el.style.maxWidth = "none";
    el.style.maxHeight = "none";
    el.style.width = "100%";
    el.style.height = "100%";
    el.style.objectFit = "contain";
  } else {
    const z = Number(mode) / 100;
    el.style.objectFit = "fill";
    el.style.maxWidth = "none";
    el.style.maxHeight = "none";
    el.style.width = `${Math.round(Number(outW.value) * z)}px`;
    el.style.height = `${Math.round(Number(outH.value) * z)}px`;
  }
}

function applyZoom() {
  styleZoom(previewImg);
  styleZoom(previewVideo);
}

zoomEl.addEventListener("change", applyZoom);

// ---- 再生（範囲を mp4 化して <video> でループ）----
function revokeUrl() {
  if (objectUrl) {
    URL.revokeObjectURL(objectUrl);
    objectUrl = null;
  }
}

// 再生用レンダリングの進捗バー。
function setRenderProgress(pct: number) {
  const p = Math.max(0, Math.min(100, Math.round(pct)));
  renderProgressFill.style.width = `${p}%`;
  renderProgressPct.textContent = `${p}%`;
}
function showRenderProgress() {
  setRenderProgress(0);
  renderProgress.hidden = false;
}
function hideRenderProgress() {
  renderProgress.hidden = true;
}

// backend の range-progress イベント（0–100%）。
listen<{ percent: number }>("range-progress", (ev) => {
  setRenderProgress(ev.payload.percent);
});

// 再生位置を playhead に追従させる（rAF で軽量更新。renderTimeline やプレビュー
// 再スケジュールは呼ばない＝再生を止めない）。mp4 は範囲先頭を 0 とするので
// 実時刻 = playStart + video.currentTime。
function followTick() {
  if (!state.playing) {
    followRaf = null;
    return;
  }
  const dur = state.duration || 1;
  let t = state.playStart + previewVideo.currentTime;
  if (t > state.playEnd) t = state.playEnd;
  state.playhead = t;
  // シーク操作中はスクラブ/マーカーをユーザに委ね、追従は上書きしない。
  if (!seeking) {
    const marker = document.getElementById("playhead-marker");
    if (marker) marker.style.left = `${(t / dur) * 100}%`;
    scrubEl.value = String(Math.round((t / dur) * 1000));
    updateReadout();
  }
  followRaf = requestAnimationFrame(followTick);
}

// 一時停止（レンダリング済み Blob は保持し、再生ボタンで再レンダリングせず再開できる）。
function stopPlayback() {
  if (followRaf !== null) {
    cancelAnimationFrame(followRaf);
    followRaf = null;
  }
  previewVideo.pause();
  previewVideo.style.display = "none";
  previewImg.style.display = "block";
  state.playing = false;
  updatePlayButtonLabel();
}

// レンダリング結果を完全に破棄（プロジェクト切替・クローズ時）。
function discardRender() {
  previewVideo.pause();
  previewVideo.removeAttribute("src");
  previewVideo.load();
  revokeUrl();
  renderValid = false;
}

// <video> の再生を開始（レンダリング済み前提。追従ループも開始）。
function beginVideoPlayback() {
  previewImg.style.display = "none";
  previewVideo.style.display = "block";
  applyZoom();
  void previewVideo.play().catch(() => {});
  state.playing = true;
  updatePlayButtonLabel();
  followRaf = requestAnimationFrame(followTick);
  setStatus(
    t("playing", {
      start: fmtTime(state.playStart),
      end: fmtTime(state.playEnd),
    }),
  );
}

async function startPlayback() {
  if (!state.inputPath || state.playing) return;
  const w = Number(outW.value);
  const h = Number(outH.value);
  const fps = Number(outFps.value);
  if (w % 8 !== 0) {
    setStatus(t("widthMul8"), true);
    return;
  }
  if (state.playEnd <= state.playStart) {
    setStatus(t("rangeEmpty"), true);
    return;
  }
  // 範囲・パラメータが未変更なら、既存レンダリングをそのまま再生（再レンダリングしない）。
  if (renderValid && objectUrl) {
    beginVideoPlayback();
    return;
  }
  playBtn.disabled = true;
  setStatus(t("rendering"));
  showRenderProgress();
  try {
    const project = {
      input_path: state.inputPath,
      width: w,
      height: h,
      fps,
      segments: state.segments,
    };
    const buf = await invoke<ArrayBuffer>("render_range", {
      project,
      startSec: state.playStart,
      endSec: state.playEnd,
    });
    revokeUrl();
    objectUrl = URL.createObjectURL(new Blob([buf], { type: "video/mp4" }));
    previewVideo.src = objectUrl;
    renderValid = true;
    beginVideoPlayback();
  } catch (e) {
    setStatus(String(e), true);
  } finally {
    hideRenderProgress();
    playBtn.disabled = false;
  }
}

playBtn.addEventListener("click", () => {
  if (state.playing) {
    stopPlayback();
    runPreview();
  } else {
    startPlayback();
  }
});

// 現在の区間を再生範囲に設定（境界そのものなのでスナップ済み）。
rangeToSegBtn.addEventListener("click", () => {
  const seg = currentSegment();
  if (!seg) return;
  if (state.playing) {
    stopPlayback();
    runPreview();
  }
  state.playStart = seg.start_sec;
  state.playEnd = seg.end_sec;
  renderTimeline();
  markDirty();
  setStatus(t("rangeToSegDone", { n: currentIndex() + 1 }));
});

// 範囲を編集したときの共通後処理（再生中なら停止して静止プレビューへ）。
function afterRangeEdited() {
  if (state.playing) {
    stopPlayback();
    runPreview();
  }
  renderTimeline();
  markDirty();
}

// 再生位置（playhead）を範囲の始点に設定。
rangeSetStartBtn.addEventListener("click", () => {
  if (!state.inputPath) return;
  state.playStart = Math.max(0, Math.min(state.playhead, state.playEnd - 0.05));
  afterRangeEdited();
  setStatus(t("rangeStartSet", { t: fmtTime(state.playStart) }));
});

// 再生位置（playhead）を範囲の終点に設定。
rangeSetEndBtn.addEventListener("click", () => {
  if (!state.inputPath) return;
  state.playEnd = Math.min(state.duration, Math.max(state.playhead, state.playStart + 0.05));
  afterRangeEdited();
  setStatus(t("rangeEndSet", { t: fmtTime(state.playEnd) }));
});

// 範囲を全体に戻す（解除）。
rangeClearBtn.addEventListener("click", () => {
  if (!state.inputPath) return;
  state.playStart = 0;
  state.playEnd = state.duration;
  afterRangeEdited();
  setStatus(t("rangeReset"));
});

// ---- エクスポート ----
exportBtn.addEventListener("click", doExport);

async function doExport() {
  if (!state.inputPath || state.exporting) return;
  const w = Number(outW.value);
  const h = Number(outH.value);
  const fps = Number(outFps.value);
  if (w % 8 !== 0) {
    setStatus(t("widthMul8"), true);
    return;
  }

  const target = await save({
    defaultPath: "output.raw",
    filters: [{ name: t("dialogMonobFilter"), extensions: ["raw"] }],
  });
  if (!target) return;

  state.exporting = true;
  exportBtn.disabled = true;
  setStatus(t("exporting"));

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
      t("exportDone", {
        frames: result.frames,
        raw: result.raw_path,
        mp4: result.mp4_path,
      }),
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
  setStatus(t("exportProgress", { done: ev.payload.done, total: ev.payload.total }));
});

// ---- プリセット（tauri-plugin-store で永続化）----
let presetStore: Store | null = null;
let presets: Preset[] = [];

async function initPresets() {
  try {
    // defaults で組み込みプリセットを seed。初回は defaults が返り、以降はディスク
    // 状態が優先される（削除・上書きは保存後にディスクへ反映されるので永続化される）。
    presetStore = await load("presets.json", {
      defaults: { presets: BUILTIN_PRESETS },
      autoSave: true,
    });
    const saved = await presetStore.get<Preset[]>("presets");
    presets = saved ?? [...BUILTIN_PRESETS];
    populatePresetList();
  } catch (e) {
    setStatus(t("presetLoadFailed", { err: String(e) }), true);
  }
}

function populatePresetList() {
  presetListEl.innerHTML = "";
  for (const p of presets) {
    const opt = document.createElement("option");
    opt.value = p.name;
    opt.textContent = p.name;
    presetListEl.appendChild(opt);
  }
}

async function persistPresets() {
  if (!presetStore) return;
  await presetStore.set("presets", presets);
  await presetStore.save();
}

// 選択中プリセットを現在区間に適用。
presetApplyBtn.addEventListener("click", () => {
  const seg = currentSegment();
  if (!seg) {
    setStatus(t("needVideoFirst"), true);
    return;
  }
  const p = presets.find((x) => x.name === presetListEl.value);
  if (!p) return;
  seg.contrast = p.contrast;
  seg.level_lo = p.level_lo;
  seg.level_hi = p.level_hi;
  seg.dither = p.dither;
  syncParamInputs();
  markDirty();
  schedulePreview();
  setStatus(t("presetApplied", { name: p.name, n: currentIndex() + 1 }));
});

// 現在区間のパラメータを名前を付けて保存（同名は上書き）。
presetSaveBtn.addEventListener("click", async () => {
  const seg = currentSegment();
  if (!seg) {
    setStatus(t("needVideoFirst"), true);
    return;
  }
  const name = presetNameEl.value.trim();
  if (!name) {
    setStatus(t("presetNameRequired"), true);
    return;
  }
  const preset: Preset = {
    name,
    contrast: seg.contrast,
    level_lo: seg.level_lo,
    level_hi: seg.level_hi,
    dither: seg.dither,
  };
  const idx = presets.findIndex((x) => x.name === name);
  if (idx >= 0) presets[idx] = preset;
  else presets.push(preset);
  await persistPresets();
  populatePresetList();
  presetListEl.value = name;
  presetNameEl.value = "";
  setStatus(t("presetSaved", { name }));
});

// 選択中プリセットを削除。
presetDeleteBtn.addEventListener("click", async () => {
  const name = presetListEl.value;
  const idx = presets.findIndex((x) => x.name === name);
  if (idx < 0) return;
  presets.splice(idx, 1);
  await persistPresets();
  populatePresetList();
  setStatus(t("presetDeleted", { name }));
});

// ---- 設定メニュー（歯車）----
appVersionEl.textContent = `v${__APP_VERSION__}`;

settingsBtn.addEventListener("click", (e) => {
  e.stopPropagation();
  settingsMenu.hidden = !settingsMenu.hidden;
});
// メニュー外クリックで閉じる。
document.addEventListener("click", (e) => {
  if (!settingsMenu.hidden && !settingsMenu.contains(e.target as Node)) {
    settingsMenu.hidden = true;
  }
});

// アプリ終了（ウィンドウを閉じる）時: 未保存なら確認。
// 保存する/保存しない→終了、キャンセル→終了を止める。
getCurrentWindow().onCloseRequested(async (event) => {
  if (!dirty) return;
  const proceed = await confirmUnsaved(t("confirmQuit"));
  if (!proceed) event.preventDefault();
});

// ---- 言語 ----
let settingsStore: Store | null = null;

function populateLangSelect() {
  langSelect.innerHTML = "";
  for (const l of LOCALES) {
    const opt = document.createElement("option");
    opt.value = l.code;
    opt.textContent = l.label;
    langSelect.appendChild(opt);
  }
}

// data-i18n で拾えない動的 UI を言語切替時に更新する。
function refreshDynamicUi() {
  updateSaveLabel();
  updatePlayButtonLabel();
  updateFileInfo();
  if (state.inputPath) {
    renderTimeline();
    syncParamInputs();
  }
}

async function changeLocale(loc: Locale, persist = true) {
  setLocale(loc); // 静的 DOM(data-i18n)を適用
  refreshDynamicUi();
  langSelect.value = loc;
  if (persist && settingsStore) {
    await settingsStore.set("locale", loc);
    await settingsStore.save();
  }
}

async function initLocale() {
  populateLangSelect();
  let loc: Locale = detectLocale();
  try {
    settingsStore = await load("settings.json", {
      defaults: { locale: "" },
      autoSave: true,
    });
    const saved = await settingsStore.get<string>("locale");
    if (isLocale(saved)) loc = saved;
  } catch {
    // ストア不可時はブラウザ言語で継続。
  }
  await changeLocale(loc, false);
  setStatus(t("statusStart"));
}

langSelect.addEventListener("change", () => {
  const v = langSelect.value;
  if (isLocale(v)) changeLocale(v);
});

// ---- 初期化 ----
newBtn.addEventListener("click", openVideo);
loadBtn.addEventListener("click", loadProject);
saveBtn.addEventListener("click", onSaveClick);
closeBtn.addEventListener("click", closeProject);
initLocale();
initPresets();
