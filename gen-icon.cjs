// 生成 JustToDo 占位图标（纯色 + 简单图形），输出 1024x1024 PNG 作为 tauri icon 源
const fs = require("fs");
const path = require("path");
const zlib = require("zlib");

const SIZE = 1024;

function crc32(buf) {
  let c = ~0;
  for (let i = 0; i < buf.length; i++) {
    c ^= buf[i];
    for (let k = 0; k < 8; k++) c = (c >>> 1) ^ (0xedb88320 & -(c & 1));
  }
  return ~c >>> 0;
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "ascii");
  const crcBuf = Buffer.alloc(4);
  crcBuf.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])), 0);
  return Buffer.concat([len, typeBuf, data, crcBuf]);
}

// RGBA 像素
const raw = Buffer.alloc(SIZE * SIZE * 4);
const bg = [74, 144, 217, 255]; // #4A90D9
const white = [255, 255, 255, 255];
const dark = [31, 35, 41, 255];
const cx = SIZE / 2;
const cy = SIZE / 2;

for (let y = 0; y < SIZE; y++) {
  for (let x = 0; x < SIZE; x++) {
    const i = (y * SIZE + x) * 4;
    // 圆角矩形背景
    const r = 180;
    const inX = Math.min(x, SIZE - 1 - x);
    const inY = Math.min(y, SIZE - 1 - y);
    let inside = true;
    if (inX < r && inY < r) {
      const dx = r - inX;
      const dy = r - inY;
      inside = dx * dx + dy * dy <= r * r;
    }
    if (!inside) {
      raw[i] = 0; raw[i + 1] = 0; raw[i + 2] = 0; raw[i + 3] = 0;
      continue;
    }
    // 画一个白色圆角"勾选"框
    const boxX = cx - 200;
    const boxY = cy - 200;
    const boxS = 400;
    const onBox =
      x >= boxX && x < boxX + boxS && y >= boxY && y < boxY + boxS &&
      (x < boxX + 40 || x >= boxX + boxS - 40 || y < boxY + 40 || y >= boxY + boxS - 40);
    if (onBox) {
      raw[i] = white[0]; raw[i + 1] = white[1]; raw[i + 2] = white[2]; raw[i + 3] = 255;
    } else {
      raw[i] = bg[0]; raw[i + 1] = bg[1]; raw[i + 2] = bg[2]; raw[i + 3] = 255;
    }
    // 勾的对勾
    const onCheck =
      ((x - cx + 40) * 0.7 + (y - cy + 120) > -20) &&
      ((x - cx + 40) * 0.7 + (y - cy + 120) < 20) &&
      x > cx - 120 && x < cx + 120 && y > cy - 60 && y < cy + 140;
    if (onCheck) {
      raw[i] = white[0]; raw[i + 1] = white[1]; raw[i + 2] = white[2]; raw[i + 3] = 255;
    }
  }
}

// 每行加 filter byte (0)
const filtered = Buffer.alloc((SIZE * 4 + 1) * SIZE);
for (let y = 0; y < SIZE; y++) {
  filtered[(y) * (SIZE * 4 + 1)] = 0;
  raw.copy(filtered, y * (SIZE * 4 + 1) + 1, y * SIZE * 4, (y + 1) * SIZE * 4);
}
const idat = zlib.deflateSync(filtered);

const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(SIZE, 0);
ihdr.writeUInt32BE(SIZE, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // color type RGBA
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const png = Buffer.concat([
  sig,
  chunk("IHDR", ihdr),
  chunk("IDAT", idat),
  chunk("IEND", Buffer.alloc(0)),
]);

const outDir = path.join(__dirname, "src-tauri", "icons");
fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, "app-icon.png"), png);
console.log("icon written:", path.join(outDir, "app-icon.png"), png.length, "bytes");
