// Draws the source icon the Tauri icon generator works from.
//
// A rounded square in the accent gradient with a page corner cut out of it.
// Deliberately simple: one shape, one fold, no text, so it stays readable at
// sixteen pixels.
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname } from "node:path";

const size = 1024;
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 1024 1024">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#8b6cf0"/>
      <stop offset="1" stop-color="#5b4bd6"/>
    </linearGradient>
    <linearGradient id="page" x1="0.2" y1="0" x2="0.8" y2="1">
      <stop offset="0" stop-color="#ffffff"/>
      <stop offset="1" stop-color="#e6e3f5"/>
    </linearGradient>
  </defs>
  <rect x="0" y="0" width="1024" height="1024" rx="228" fill="url(#bg)"/>
  <path d="M330 214h242l152 152v444a36 36 0 0 1-36 36H330a36 36 0 0 1-36-36V250a36 36 0 0 1 36-36z" fill="url(#page)"/>
  <path d="M572 214l152 152H608a36 36 0 0 1-36-36z" fill="#c9c2ec"/>
  <rect x="366" y="470" width="290" height="34" rx="17" fill="#5b4bd6"/>
  <rect x="366" y="556" width="230" height="34" rx="17" fill="#8b6cf0"/>
  <rect x="366" y="642" width="170" height="34" rx="17" fill="#b7abf4"/>
</svg>
`;

const target = process.argv[2] ?? "src-tauri/icons/source.svg";
mkdirSync(dirname(target), { recursive: true });
writeFileSync(target, svg);
console.log(`wrote ${target}`);
