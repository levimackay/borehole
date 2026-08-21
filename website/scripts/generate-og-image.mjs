// One-off script: generates public/og-image.png — the command-palette
// screenshot, dimmed, with the wordmark and tagline overlaid so the card
// reads on its own as a small social-preview thumbnail. Not part of the
// vite build pipeline — run manually:
//   node scripts/generate-og-image.mjs
import sharp from "sharp";
import { fileURLToPath } from "node:url";
import path from "node:path";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const src = path.resolve(__dirname, "../public/screenshots/command-palette.png");
const out = path.resolve(__dirname, "../public/og-image.png");

const WIDTH = 1200;
const HEIGHT = 630;
// var(--bh-bg-canvas)
const BG = { r: 0x12, g: 0x14, b: 0x17, alpha: 1 };
const ACCENT = "#4fb3ac";
const ACCENT_FG = "#06211f";
const TEXT_PRIMARY = "#e4e6ea";
const TEXT_MUTED = "#9aa1ac";

const overlay = `
<svg width="${WIDTH}" height="${HEIGHT}" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <linearGradient id="scrim" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0%" stop-color="#121417" stop-opacity="0.55"/>
      <stop offset="45%" stop-color="#121417" stop-opacity="0.72"/>
      <stop offset="100%" stop-color="#121417" stop-opacity="0.92"/>
    </linearGradient>
  </defs>
  <rect width="${WIDTH}" height="${HEIGHT}" fill="url(#scrim)"/>
  <rect x="72" y="72" width="56" height="56" rx="10" fill="${ACCENT}"/>
  <text x="100" y="110" font-family="JetBrains Mono, monospace" font-weight="700"
        font-size="26" fill="${ACCENT_FG}" text-anchor="middle">bh</text>
  <text x="150" y="112" font-family="JetBrains Mono, monospace" font-weight="700"
        font-size="34" fill="${TEXT_PRIMARY}">borehole</text>
  <text x="72" y="230" font-family="IBM Plex Sans, sans-serif" font-weight="600"
        font-size="56" fill="${TEXT_PRIMARY}">Understand the code</text>
  <text x="72" y="296" font-family="IBM Plex Sans, sans-serif" font-weight="600"
        font-size="56" fill="${TEXT_PRIMARY}">before you change it.</text>
  <text x="72" y="352" font-family="IBM Plex Sans, sans-serif" font-weight="400"
        font-size="26" fill="${TEXT_MUTED}">Local-first codebase intelligence — evidence-backed, not AI-guessed.</text>
</svg>`;

await sharp(src)
  .resize(WIDTH, HEIGHT, { fit: "contain", background: BG })
  .composite([{ input: Buffer.from(overlay) }])
  .png()
  .toFile(out);

console.log(`Wrote ${out}`);
